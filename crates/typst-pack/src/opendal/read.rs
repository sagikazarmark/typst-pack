//! Bounded reads of projects, fonts, packages, and pack archives.

use ::opendal::{EntryMode, ErrorKind, Operator};
use futures_util::TryStreamExt;
use typst::syntax::package::PackageSpec;

use super::{
    Error, archive_cache_key, changed, check_key, check_prefix, is_font_key, key_issue, stat,
    storage, stream, tree_key,
};
use crate::pack::names_pack_path;
use crate::{LimitExceeded, ProjectSnapshot, ReadLimits, font, package};

/// Running totals charged against one operation's [`ReadLimits`].
struct Budget {
    limits: ReadLimits,
    entries: u64,
    bytes: u64,
}

impl Budget {
    fn new(limits: ReadLimits) -> Self {
        Self {
            limits,
            entries: 0,
            bytes: 0,
        }
    }
}

fn limit(path: &str, source: LimitExceeded) -> Error {
    Error::Limit {
        path: path.to_owned(),
        source,
    }
}

/// Reads one object after checking its advertised length against the
/// per-file and remaining total limits, so an oversized object is never
/// downloaded. `Ok(None)` means the object is absent.
///
/// The bytes read must match the stat: the read is conditional on the stat's
/// ETag where supported, and a read whose length differs from the stat's
/// fails instead of returning a prefix or more bytes than were budgeted.
async fn read_object(
    op: &Operator,
    path: &str,
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, Error> {
    let Some(metadata) = stat(op, path).await? else {
        return Ok(None);
    };
    let length = metadata.content_length();
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
    let mut chunks = stream(op, path, &metadata).await?;
    let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
    while let Some(buffer) = chunks.try_next().await.map_err(storage(path, "read"))? {
        for chunk in buffer {
            if (bytes.len() + chunk.len()) as u64 > length {
                return Err(changed(path));
            }
            bytes.extend_from_slice(&chunk);
        }
    }
    if bytes.len() as u64 != length {
        return Err(changed(path));
    }
    Ok(Some(bytes))
}

/// Recursively lists `prefix` and reads every selected file, returning
/// prefix-relative paths in sorted order. Every listed entry counts toward
/// `max_entries`. A listing that reports a missing prefix reads as empty. A
/// selected entry whose relative path is not a normalized object key fails
/// with [`Error::InvalidPath`].
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
            if let Some(reason) = key_issue(relative) {
                // Such a key would alias another one or name a file outside
                // the tree once OpenDAL normalizes it.
                return Err(Error::InvalidPath {
                    path: path.to_owned(),
                    reason,
                });
            }
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
/// All listed files are selected except `.typk` paths (a pack is never a
/// project file); `.typkignore` is an ordinary project file here (ignore
/// rules apply only to filesystem reading). Directory markers are skipped. The
/// snapshot validates paths, collisions, and the entrypoint. The listing is
/// one observation, not a storage snapshot.
pub async fn read_project(
    op: &Operator,
    prefix: &str,
    entrypoint: &str,
    limits: ReadLimits,
) -> Result<ProjectSnapshot, Error> {
    check_prefix(prefix)?;
    require_listing(op)?;
    let select = |path: &str| !names_pack_path(path);
    let files = read_tree(op, prefix, select, &mut Budget::new(limits)).await?;
    Ok(ProjectSnapshot::new(entrypoint, files)?)
}

/// One storage prefix of font files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontSource {
    /// A prefix (`""` or ending in `/`) holding font files.
    pub prefix: String,
    /// Whether created packs embed fonts from this source.
    pub embedded: bool,
}

/// Reads `.ttf`, `.ttc`, `.otf`, and `.otc` objects (case-insensitive) below
/// each source as font containers, in source order, then path order.
///
/// `limits` is shared across all sources.
pub async fn read_fonts(
    op: &Operator,
    sources: &[FontSource],
    limits: ReadLimits,
) -> Result<Vec<font::Entry>, Error> {
    for source in sources {
        check_prefix(&source.prefix)?;
    }
    if !sources.is_empty() {
        require_listing(op)?;
    }
    let mut budget = Budget::new(limits);
    let mut entries = Vec::new();
    for source in sources {
        let files = read_tree(op, &source.prefix, is_font_key, &mut budget).await?;
        for (relative, bytes) in files {
            let container = font::Container::new(bytes).ok_or_else(|| Error::Font {
                path: format!("{}{relative}", source.prefix),
            })?;
            entries.push(font::Entry {
                container,
                embedded: source.embedded,
            });
        }
    }
    Ok(entries)
}

/// One place a package may be read from, with the storage layout it uses.
#[derive(Clone, Debug)]
pub enum PackageSource {
    /// Package trees at `{prefix}{namespace}/{name}/{version}/`, Typst's
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
#[non_exhaustive]
pub enum PackageRead {
    /// A package tree's files, with package-relative paths.
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
    limits: ReadLimits,
) -> Result<PackageRead, Error> {
    for source in sources {
        let mut budget = Budget::new(limits);
        match source {
            PackageSource::Trees { operator, prefix } => {
                check_prefix(prefix)?;
                require_listing(operator)?;
                let tree = format!("{prefix}{}/", tree_key(spec));
                let files = read_tree(operator, &tree, |_| true, &mut budget).await?;
                if !files.is_empty() {
                    return Ok(PackageRead::Tree(files));
                }
            }
            PackageSource::ArchiveCache { operator, prefix } => {
                check_prefix(prefix)?;
                require_read(operator)?;
                let key = archive_cache_key(spec);
                if let Some(bytes) =
                    read_object(operator, &format!("{prefix}{key}"), &mut budget).await?
                {
                    return Ok(PackageRead::CachedArchive(bytes));
                }
            }
            PackageSource::Registry { operator, prefix } => {
                check_prefix(prefix)?;
                let Some(key) = package::registry::archive_key(spec) else {
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

/// Adds what [`read_package`] found to a creation request's packages,
/// keeping its package failures in step.
///
/// On success the tree is cataloged and any earlier failure for `spec` is
/// removed. `Unavailable` records a `NotFound` failure and returns `Ok`. A
/// tree, archive, or catalog rejection records a failure (`MalformedArchive`
/// for malformed archive contents, otherwise `Other`) carrying the reason and
/// returns the error; resumed creation then reports the failure at the
/// import.
#[cfg(feature = "package-reading")]
pub fn insert_package(
    request: &mut crate::create::Request,
    spec: &PackageSpec,
    read: PackageRead,
    embedded: bool,
    limits: ReadLimits,
) -> Result<(), Error> {
    use crate::package::ReadFailure;
    use crate::package::registry::{ExpandError, expand_archive};

    let tree = match read {
        PackageRead::Tree(files) => package::Tree::new(files).map_err(Error::from),
        PackageRead::CachedArchive(bytes) | PackageRead::RegistryArchive(bytes) => {
            expand_archive(&bytes, limits).map_err(Error::from)
        }
        PackageRead::Unavailable => {
            request
                .package_failures
                .insert(spec.clone(), ReadFailure::NotFound);
            return Ok(());
        }
    };
    let result = tree.and_then(|tree| {
        request
            .packages
            .insert(spec.clone(), tree, embedded)
            .map_err(Error::from)
    });
    match result {
        Ok(()) => {
            request.package_failures.remove(spec);
            Ok(())
        }
        Err(error) => {
            // The failure's own Display already says "malformed archive", so
            // its detail is the inner reason.
            let reason = match &error {
                Error::PackageArchive(ExpandError::Malformed(reason)) => {
                    ReadFailure::MalformedArchive(Some(reason.clone()))
                }
                Error::PackageArchive(ExpandError::Invalid(invalid)) => {
                    ReadFailure::MalformedArchive(Some(invalid.to_string()))
                }
                _ => ReadFailure::Other(Some(crate::chain(&error))),
            };
            request.package_failures.insert(spec.clone(), reason);
            Err(error)
        }
    }
}

/// Reads the exact bytes of one pack archive object of at most `max_bytes`.
///
/// Decode the result with [`archive::decode`](crate::archive::decode);
/// decoding borrows the bytes, so they stay available if decoding fails.
pub async fn read_archive(op: &Operator, path: &str, max_bytes: u64) -> Result<Vec<u8>, Error> {
    check_key(path)?;
    require_read(op)?;
    let mut budget = Budget::new(ReadLimits {
        max_entries: 1,
        max_file_bytes: max_bytes,
        max_total_bytes: max_bytes,
    });
    let bytes = read_object(op, path, &mut budget).await?.ok_or_else(|| {
        storage(path, "read")(::opendal::Error::new(
            ErrorKind::NotFound,
            "the pack archive object is absent",
        ))
    })?;
    Ok(bytes)
}
