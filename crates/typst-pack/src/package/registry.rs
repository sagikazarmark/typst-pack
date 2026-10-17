//! Reading packages from the official Typst Universe registry with a
//! transport of the caller's choice.
//!
//! Creation reports the exact specifications it needs and was not given.
//! [`archive_url`] names where the registry serves each one, and
//! [`expand_archive`] turns the fetched bytes into a [`Tree`]. Fetching is the
//! caller's: nothing here needs an HTTP client, so a host whose only network
//! access is asynchronous can use these helpers.

use std::io::Read;

use typst::syntax::package::PackageSpec;

use super::Tree;
use crate::payload::SharedBytes;
use crate::{LimitExceeded, PackError, ReadLimits};

/// The URL of the official Typst Universe registry.
const URL: &str = "https://packages.typst.org";

/// The URL of the archive the registry serves for `spec`, or `None` when the
/// registry does not serve its namespace (only `preview` is served).
///
/// A Typst import always names an exact version, so no index lookup is
/// involved.
pub fn archive_url(spec: &PackageSpec) -> Option<String> {
    let key = super::registry_archive_key(spec)?;
    Some(format!("{URL}/{key}"))
}

/// Expands the gzip-compressed tar the registry serves into a package tree.
///
/// Regular files become tree entries and directories are skipped; links and
/// every other member kind are rejected, as are paths that cannot name a
/// package file. `limits` bounds the member count, each file, and the total
/// expanded size; the decompressed stream itself is bounded too, so a
/// compression bomb stops at the limit.
pub fn expand_archive(archive: &[u8], limits: ReadLimits) -> Result<Tree, ExpandError> {
    let malformed = |error: std::io::Error| ExpandError::Malformed(error.to_string());

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
        LimitExceeded::add(&mut entries, 1, limits.max_entries, "archive members")?;
        let path = String::from_utf8(entry.path_bytes().into_owned()).map_err(|error| {
            ExpandError::Malformed(format!(
                "member name {:?} is not valid UTF-8",
                error.as_bytes()
            ))
        })?;
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err(ExpandError::Malformed(format!(
                "member {path:?} is not a regular file or directory"
            )));
        }
        let size = entry.size();
        LimitExceeded::check(size, limits.max_file_bytes, "file bytes")?;
        LimitExceeded::add(&mut total, size, limits.max_total_bytes, "total bytes")?;
        let mut data = Vec::with_capacity(size.min(1 << 20) as usize);
        entry.read_to_end(&mut data).map_err(malformed)?;
        files.push((path, SharedBytes::new(data)));
    }

    Ok(Tree::from_shared(files)?)
}

/// A package archive that could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExpandError {
    /// The archive exceeds a read limit.
    #[error("the package archive exceeded a limit: {0}")]
    Limit(#[from] LimitExceeded),
    /// The bytes are not a gzip-compressed tar of regular files.
    #[error("the package archive is malformed: {0:?}")]
    Malformed(String),
    /// The files do not form a valid package tree.
    #[error("the package archive does not hold a valid package tree: {0}")]
    Invalid(#[from] PackError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_preview_namespace_has_a_registry_url() {
        let served = "@preview/example:1.2.3".parse().unwrap();
        assert_eq!(
            archive_url(&served).as_deref(),
            Some("https://packages.typst.org/preview/example-1.2.3.tar.gz")
        );
        for unserved in ["@local/example:1.0.0", "@custom/example:1.0.0"] {
            assert_eq!(archive_url(&unserved.parse().unwrap()), None, "{unserved}");
        }
    }
}
