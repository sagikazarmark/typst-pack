//! Package reading for a Pack Assembler that supplies its own transport.
//!
//! Creation reports the exact specifications its representative request needs
//! and was not given. These helpers cover the two transformations between that
//! report and a resolved tree — constructing the registry URL of a
//! specification, and expanding the archive bytes fetched from it into a
//! Package Tree — so a caller reimplements neither the registry layout
//! nor the archive encoding. Fetching those bytes is the adapter's own
//! obligation and stays outside: nothing here needs an HTTP client, so the
//! helpers are usable on a host whose only network access is asynchronous.

use std::io::Read;

use typst::syntax::package::PackageSpec;

use crate::package_catalog::{PackageTree, PackageTreeError};
use crate::payload::SharedBytes;
use crate::{LimitExceeded, ReadLimits, read_layout};

/// A failure while reading exact Package Archive bytes from a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageArchiveReadError {
    #[error(transparent)]
    Limit(#[from] LimitExceeded),
    #[error("failed to read Package Archive bytes: {0}")]
    Read(#[source] std::io::Error),
}

/// The URL of the package registry these helpers describe the layout of, the
/// official Typst Universe registry. There is no standardized registry
/// protocol, so the layout is this registry's own.
pub const PACKAGE_REGISTRY_URL: &str = read_layout::PACKAGE_REGISTRY_URL;

/// The one package namespace the registry serves. A specification in any other
/// namespace is resolved from wherever its namespace lives, which the registry
/// layout says nothing about.
pub const PACKAGE_REGISTRY_NAMESPACE: &str = read_layout::PACKAGE_REGISTRY_NAMESPACE;

/// Reads at most `max_bytes` of compressed Package Archive bytes.
///
/// A known size is checked before the reader is touched; the stream is still
/// bounded because a size declaration is only a hint.
pub fn read_package_archive(
    reader: impl Read,
    known_size: Option<u64>,
    max_bytes: u64,
) -> Result<Vec<u8>, PackageArchiveReadError> {
    if let Some(size) = known_size {
        LimitExceeded::check(size, max_bytes, "package archive bytes")?;
    }
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(PackageArchiveReadError::Read)?;
    LimitExceeded::check(bytes.len() as u64, max_bytes, "package archive bytes")?;
    Ok(bytes)
}

/// The URL of the archive holding one exact package specification's Package
/// Tree.
///
/// No index lookup is involved: creation only ever reports fully versioned
/// specifications, because a Typst import specification always carries an exact
/// version.
pub fn package_archive_url(spec: &PackageSpec) -> Result<String, PackageReadError> {
    read_layout::official_registry_archive_url(spec)
        .ok_or_else(|| PackageReadError::UnservedNamespace { spec: spec.clone() })
}

/// Expands the gzip-compressed tar a registry serves for one exact package
/// specification into a Package Tree.
///
/// Regular files become tree entries and directories are skipped; links and
/// every other member kind are rejected, as are paths that cannot name a
/// package file. `limits` bounds the member count, each file, and the total
/// expanded size; the decompressed stream itself is bounded too, so a
/// compression bomb stops at the limit.
pub fn expand_package_archive(
    spec: PackageSpec,
    archive: &[u8],
    limits: ReadLimits,
) -> Result<PackageTree, PackageReadError> {
    let malformed = |error: std::io::Error| PackageReadError::MalformedArchive {
        spec: spec.clone(),
        message: error.to_string(),
    };
    let limited = |source| PackageReadError::Limit {
        spec: spec.clone(),
        source,
    };

    // Headers, padding, and name extensions take at most a few blocks per
    // member beyond the file contents.
    let stream_limit = limits
        .max_total_bytes
        .saturating_add(limits.max_entries.saturating_mul(4096))
        .saturating_add(1024);
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive).take(stream_limit));
    let mut files = Vec::new();
    let mut entries = 0;
    let mut total = 0;
    for entry in tar.entries().map_err(malformed)? {
        let mut entry = entry.map_err(malformed)?;
        LimitExceeded::add(&mut entries, 1, limits.max_entries, "archive members")
            .map_err(limited)?;
        let path = String::from_utf8(entry.path_bytes().into_owned()).map_err(|error| {
            PackageReadError::MalformedArchive {
                spec: spec.clone(),
                message: format!("member name {:?} is not valid UTF-8", error.as_bytes()),
            }
        })?;
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(PackageReadError::MalformedArchive {
                spec: spec.clone(),
                message: format!("member {path:?} is not a regular file or directory"),
            });
        }
        let size = entry.size();
        LimitExceeded::check(size, limits.max_file_bytes, "file bytes").map_err(limited)?;
        LimitExceeded::add(&mut total, size, limits.max_total_bytes, "total bytes")
            .map_err(limited)?;
        let mut data = Vec::with_capacity(size.min(1 << 20) as usize);
        entry.read_to_end(&mut data).map_err(malformed)?;
        files.push((path, SharedBytes::new(data)));
    }

    PackageTree::from_shared_entries(files)
        .map_err(|source| PackageReadError::InvalidPackageTree { spec, source })
}

/// A failure while reading one Package Tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageReadError {
    /// The registry does not serve the specification's namespace, so it has no
    /// URL there.
    #[error(
        "the registry serves only the `{PACKAGE_REGISTRY_NAMESPACE}` namespace, and {spec} is not in it"
    )]
    UnservedNamespace { spec: PackageSpec },
    /// The archive exceeds a read limit.
    #[error("the archive for {spec} exceeded a limit: {source}")]
    Limit {
        spec: PackageSpec,
        source: LimitExceeded,
    },
    /// The bytes are not the archive a registry serves for the specification.
    #[error("the archive for {spec} is malformed: {message:?}")]
    MalformedArchive { spec: PackageSpec, message: String },
    /// The archive does not expand into a valid Package Tree.
    #[error("the archive for {spec} does not contain a valid package tree: {source}")]
    InvalidPackageTree {
        spec: PackageSpec,
        source: PackageTreeError,
    },
}
