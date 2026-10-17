//! Bounded reads of Pack Assembly inputs and Pack Archives.

use ::opendal::{EntryMode, ErrorKind, Operator};
use futures_util::TryStreamExt;
use typst::syntax::package::PackageSpec;

use super::{Error, check_key, check_prefix, storage};
use crate::{
    FontCatalog, FontCatalogEntry, FontContainer, FontDisposition, LimitExceeded, ProjectSnapshot,
    ReadLimits, read_layout,
};

/// Running totals charged against one operation's [`ReadLimits`].
struct Budget {
    limits: ReadLimits,
    entries: u64,
    bytes: u64,
}

impl Budget {
    fn new(limits: &ReadLimits) -> Self {
        Self {
            limits: *limits,
            entries: 0,
            bytes: 0,
        }
    }
}

fn limit(path: &str, exceeded: LimitExceeded) -> Error {
    Error::Limit {
        path: path.to_owned(),
        exceeded,
    }
}

/// Reads one object after checking its advertised length against the
/// per-file and remaining total limits, so an oversized object is never
/// downloaded. `Ok(None)` means the object is absent.
async fn read_object(
    op: &Operator,
    path: &str,
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, Error> {
    let length = match op.stat(path).await {
        Ok(metadata) => metadata.content_length(),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage(path, "stat")(error)),
    };
    if length > budget.limits.max_file_bytes {
        return Err(limit(
            path,
            LimitExceeded {
                resource: "file bytes",
                limit: budget.limits.max_file_bytes,
            },
        ));
    }
    LimitExceeded::add(
        &mut budget.bytes,
        length,
        budget.limits.max_total_bytes,
        "total bytes",
    )
    .map_err(|exceeded| limit(path, exceeded))?;
    if length == 0 {
        // A ranged read of an empty object is unsatisfiable on some services.
        return Ok(Some(Vec::new()));
    }
    // The range caps the transfer at the stat-observed length even when the
    // object grows concurrently.
    let buffer = op
        .read_with(path)
        .range(0..length)
        .await
        .map_err(storage(path, "read"))?;
    Ok(Some(buffer.to_vec()))
}

/// Recursively lists `prefix` and reads every selected file, returning
/// prefix-relative paths in sorted order. Every listed entry counts toward
/// `max_entries`. A listing that reports a missing prefix reads as empty.
async fn read_tree(
    op: &Operator,
    prefix: &str,
    select: impl Fn(&str) -> bool,
    budget: &mut Budget,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut lister = match op.lister_with(prefix).recursive(true).await {
        Ok(lister) => lister,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage(prefix, "list")(error)),
    };
    let mut selected = Vec::new();
    while let Some(entry) = lister.try_next().await.map_err(storage(prefix, "list"))? {
        LimitExceeded::add(&mut budget.entries, 1, budget.limits.max_entries, "entries")
            .map_err(|exceeded| limit(prefix, exceeded))?;
        let path = entry.path();
        let Some(relative) = path.strip_prefix(prefix) else {
            return Err(Error::InvalidPath {
                path: path.to_owned(),
                reason: "the listing yielded an entry outside the requested prefix",
            });
        };
        match entry.metadata().mode() {
            EntryMode::DIR => continue,
            EntryMode::FILE if !relative.is_empty() && !relative.ends_with('/') => {}
            _ => {
                return Err(Error::InvalidPath {
                    path: path.to_owned(),
                    reason: "the listing yielded an entry that is neither a file nor a directory",
                });
            }
        }
        if select(relative) {
            selected.push((path.to_owned(), relative.to_owned()));
        }
    }
    selected.sort_by(|left, right| left.1.cmp(&right.1));

    let mut files = Vec::with_capacity(selected.len());
    for (path, relative) in selected {
        let bytes = read_object(op, &path, budget).await?.ok_or_else(|| {
            storage(&path, "read")(::opendal::Error::new(
                ErrorKind::NotFound,
                "a listed object vanished before it was read",
            ))
        })?;
        files.push((relative, bytes));
    }
    Ok(files)
}

fn require_listing(op: &Operator) -> Result<(), Error> {
    let capability = op.info().capability();
    if !(capability.list && capability.list_with_recursive) {
        return Err(Error::Unsupported {
            capability: "recursive listing",
        });
    }
    require_read(op)
}

fn require_read(op: &Operator) -> Result<(), Error> {
    let capability = op.info().capability();
    if !(capability.stat && capability.read) {
        return Err(Error::Unsupported {
            capability: "stat and read",
        });
    }
    Ok(())
}

/// Reads every file below `prefix` into a [`ProjectSnapshot`].
///
/// All listed files are selected; `.typkignore` is an ordinary project file
/// here (ignore rules apply only to filesystem reading). Directory markers
/// are skipped. The snapshot validates paths, collisions, and the
/// entrypoint. The listing is one observation, not a storage snapshot.
pub async fn read_project(
    op: &Operator,
    prefix: &str,
    entrypoint: &str,
    limits: &ReadLimits,
) -> Result<ProjectSnapshot, Error> {
    check_prefix(prefix)?;
    require_listing(op)?;
    let files = read_tree(op, prefix, |_| true, &mut Budget::new(limits)).await?;
    Ok(ProjectSnapshot::new(entrypoint, files)?)
}

/// One storage prefix of Font Containers and the disposition its fonts get.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontSource {
    /// A prefix (`""` or ending in `/`) holding font files.
    pub prefix: String,
    /// Whether the Pack embeds fonts from this source.
    pub disposition: FontDisposition,
}

/// Reads `.ttf`, `.ttc`, `.otf`, and `.otc` objects (case-insensitive) below
/// each source in order and validates each as a [`FontContainer`].
///
/// Entries are cataloged in source order, then path order. `limits` is
/// shared across all sources.
pub async fn read_fonts(
    op: &Operator,
    sources: &[FontSource],
    limits: &ReadLimits,
) -> Result<FontCatalog, Error> {
    for source in sources {
        check_prefix(&source.prefix)?;
    }
    if !sources.is_empty() {
        require_listing(op)?;
    }
    let mut budget = Budget::new(limits);
    let mut catalog = FontCatalog::new();
    for source in sources {
        let files = read_tree(
            op,
            &source.prefix,
            read_layout::is_font_container_key,
            &mut budget,
        )
        .await?;
        for (relative, bytes) in files {
            let container = FontContainer::new(bytes).map_err(|source_error| Error::Font {
                path: format!("{}{relative}", source.prefix),
                source: source_error,
            })?;
            catalog.push(FontCatalogEntry::new(container, source.disposition));
        }
    }
    Ok(catalog)
}

/// One place a package may be read from, with the storage layout it uses.
#[derive(Clone, Debug)]
pub enum PackageSource {
    /// Package Trees at `{prefix}{namespace}/{name}/{version}/`, Typst's
    /// package directory layout.
    Trees { operator: Operator, prefix: String },
    /// Raw `.tar.gz` archives at `{prefix}{namespace}/{name}/{version}.tar.gz`.
    ArchiveCache { operator: Operator, prefix: String },
    /// The official registry layout,
    /// `{prefix}{namespace}/{name}-{version}.tar.gz`. Only the `preview`
    /// namespace is served; other specifications skip this source.
    Registry { operator: Operator, prefix: String },
}

/// What [`read_package`] found for one specification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageRead {
    /// A Package Tree's files, with package-relative paths.
    Tree(Vec<(String, Vec<u8>)>),
    /// Raw archive bytes from an [`PackageSource::ArchiveCache`].
    CachedArchive(Vec<u8>),
    /// Raw archive bytes from a [`PackageSource::Registry`].
    RegistryArchive(Vec<u8>),
    /// Every applicable source was definitely absent.
    Unavailable,
}

/// Reads one package from the first source that holds it.
///
/// Sources are tried in order and only definite absence advances to the next
/// one; any other failure is returned. A tree is present when its prefix
/// lists at least one file. Raw archives are returned unexpanded; with the
/// `package-reading` feature, [`insert_package`] expands and validates them.
/// For trees, `limits` bounds the listing and file bytes; for archives,
/// `max_file_bytes` bounds the archive.
pub async fn read_package(
    sources: &[PackageSource],
    spec: &PackageSpec,
    limits: &ReadLimits,
) -> Result<PackageRead, Error> {
    for source in sources {
        let mut budget = Budget::new(limits);
        match source {
            PackageSource::Trees { operator, prefix } => {
                check_prefix(prefix)?;
                require_listing(operator)?;
                let tree = format!("{prefix}{}/", read_layout::package_tree_key(spec));
                let files = read_tree(operator, &tree, |_| true, &mut budget).await?;
                if !files.is_empty() {
                    return Ok(PackageRead::Tree(files));
                }
            }
            PackageSource::ArchiveCache { operator, prefix } => {
                check_prefix(prefix)?;
                require_read(operator)?;
                let key = read_layout::package_archive_cache_key(spec);
                if let Some(bytes) =
                    read_object(operator, &format!("{prefix}{key}"), &mut budget).await?
                {
                    return Ok(PackageRead::CachedArchive(bytes));
                }
            }
            PackageSource::Registry { operator, prefix } => {
                check_prefix(prefix)?;
                let Some(key) = read_layout::official_registry_archive_key(spec) else {
                    continue;
                };
                require_read(operator)?;
                if let Some(bytes) =
                    read_object(operator, &format!("{prefix}{key}"), &mut budget).await?
                {
                    return Ok(PackageRead::RegistryArchive(bytes));
                }
            }
        }
    }
    Ok(PackageRead::Unavailable)
}

/// Turns a [`PackageRead`] into a cataloged Package Tree, keeping
/// `failures` in step for resumed Pack Creation.
///
/// On success the tree is inserted and any earlier failure for `spec` is
/// removed. `Unavailable` records a `NotFound` failure and returns `Ok`. A
/// tree, archive, or catalog rejection records a failure (`MalformedArchive`
/// for malformed archive contents, otherwise `Other`) and returns the error;
/// resuming Pack Creation then attaches the failure to the import.
#[cfg(feature = "package-reading")]
pub fn insert_package(
    catalog: &mut crate::PackageCatalog,
    failures: &mut crate::PackageReadFailures,
    spec: &PackageSpec,
    read: PackageRead,
    disposition: crate::PackageDisposition,
    limits: ReadLimits,
) -> Result<(), Error> {
    use crate::{PackageReadError, PackageReadFailure, PackageReadFailureReason};

    let tree = match read {
        PackageRead::Tree(files) => {
            crate::PackageTree::from_owned_entries(files).map_err(Error::from)
        }
        PackageRead::CachedArchive(bytes) | PackageRead::RegistryArchive(bytes) => {
            crate::expand_package_archive(spec.clone(), &bytes, limits).map_err(Error::from)
        }
        PackageRead::Unavailable => {
            failures.insert(PackageReadFailure::new(
                spec.clone(),
                PackageReadFailureReason::NotFound,
            ));
            return Ok(());
        }
    };
    let result = tree.and_then(|tree| {
        catalog
            .insert(spec.clone(), tree, disposition)
            .map_err(Error::from)
    });
    match result {
        Ok(()) => {
            failures.remove(spec);
            Ok(())
        }
        Err(error) => {
            let reason = match &error {
                Error::PackageArchive(
                    PackageReadError::MalformedArchive { .. }
                    | PackageReadError::InvalidPackageTree { .. },
                ) => PackageReadFailureReason::MalformedArchive { detail: None },
                _ => PackageReadFailureReason::Other { detail: None },
            };
            failures.insert(PackageReadFailure::new(spec.clone(), reason));
            Err(error)
        }
    }
}

/// Reads the exact bytes of one Pack Archive object of at most `max_bytes`.
///
/// Decode the result with [`crate::pack_archive::decode`]; decoding borrows
/// the bytes, so they stay available if decoding fails.
pub async fn read_pack_archive(
    op: &Operator,
    path: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, Error> {
    check_key(path)?;
    require_read(op)?;
    let mut budget = Budget::new(&ReadLimits {
        max_entries: 1,
        max_file_bytes: max_bytes,
        max_total_bytes: max_bytes,
    });
    let bytes = read_object(op, path, &mut budget).await?.ok_or_else(|| {
        storage(path, "read")(::opendal::Error::new(
            ErrorKind::NotFound,
            "the Pack Archive object is absent",
        ))
    })?;
    Ok(bytes)
}
