#![doc = include_str!("../../docs/opendal-integration.md")]

#[cfg(feature = "package-reading")]
mod assembler;
mod read;
mod write;

pub use ::opendal::Operator;
#[cfg(feature = "package-reading")]
pub use assembler::Assembler;
#[cfg(feature = "package-reading")]
pub use read::insert_package;
pub use read::{
    FontSource, PackageRead, PackageSource, read_archive, read_fonts, read_package, read_project,
};
pub use write::{
    WriteError, WriteOutcome, WritePolicy, write_archive, write_artifacts, write_extraction,
    write_package_archive,
};

use typst::syntax::package::PackageSpec;

use crate::{LimitExceeded, PackError, font};

/// A failure of an OpenDAL adapter operation.
///
/// `Display` names the storage path and operation but never the operator's
/// endpoint or credentials. The native OpenDAL error, which can carry
/// backend-provided context, remains available through
/// [`std::error::Error::source`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A storage operation failed.
    #[error("storage {operation} failed for {path:?}")]
    Storage {
        path: String,
        operation: &'static str,
        #[source]
        source: Box<::opendal::Error>,
    },
    /// A read limit was exceeded.
    #[error("cannot read {path:?}")]
    Limit {
        path: String,
        #[source]
        source: LimitExceeded,
    },
    /// The operator does not advertise a capability the operation needs.
    #[error("the operator does not support {capability}")]
    Unsupported { capability: &'static str },
    /// An existing object holds different bytes than a `CreateOrVerify` write.
    #[error("object {path:?} already exists with different bytes")]
    Conflict { path: String },
    /// A caller-supplied or listed path is not usable as a storage path.
    #[error("invalid storage path {path:?}: {reason}")]
    InvalidPath { path: String, reason: &'static str },
    /// Object keys a write would use are invalid or repeated; nothing was
    /// written. Each key is listed with the reason.
    #[error("invalid object keys: {}", describe_keys(.keys))]
    InvalidKeys { keys: Vec<(String, &'static str)> },
    /// The number of artifact keys does not match the number of artifacts.
    #[error("expected {expected} artifact key(s), got {actual}")]
    ArtifactKeyCount { expected: usize, actual: usize },
    /// The objects read do not form a valid project or package tree, or the
    /// catalog rejected a package.
    #[error(transparent)]
    Invalid(#[from] PackError),
    /// A selected font object is not a readable font.
    #[error("font object {path:?} is not a readable font")]
    Font { path: String },
    /// A read package archive could not be expanded.
    #[cfg(feature = "package-reading")]
    #[error(transparent)]
    PackageArchive(#[from] crate::package::registry::ExpandError),
    /// Pack creation failed.
    #[cfg(feature = "package-reading")]
    #[error(transparent)]
    Create(Box<crate::create::Error>),
    /// A pack archive object is not a valid pack.
    #[error("cannot decode pack {path:?}")]
    Decode {
        path: String,
        #[source]
        source: Box<crate::archive::DecodeError>,
    },
    /// Creation reported a package missing again after it was read.
    #[cfg(feature = "package-reading")]
    #[error("creation requested package {spec} again after it was read")]
    RepeatedPackage {
        spec: typst::syntax::package::PackageSpec,
    },
}

pub(crate) fn storage(
    path: &str,
    operation: &'static str,
) -> impl FnOnce(::opendal::Error) -> Error {
    let path = path.to_owned();
    move |source| Error::Storage {
        path,
        operation,
        source: Box::new(source),
    }
}

fn describe_keys(keys: &[(String, &'static str)]) -> String {
    keys.iter()
        .map(|(key, reason)| format!("{key:?} ({reason})"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Where one specification's tree lives in a directory of trees:
/// `namespace/name/version`, Typst's package directory layout.
fn tree_key(spec: &PackageSpec) -> String {
    format!("{}/{}/{}", spec.namespace, spec.name, spec.version)
}

/// Where one specification's archive lives in a cache of raw archives.
fn archive_cache_key(spec: &PackageSpec) -> String {
    format!("{}.tar.gz", tree_key(spec))
}

/// Whether a storage key ends in a font container extension. Unlike a path,
/// a key that is nothing but the extension, such as `.ttf`, qualifies.
fn is_font_key(key: &str) -> bool {
    key.rsplit_once('.')
        .is_some_and(|(_, suffix)| font::is_container_extension(suffix))
}

/// Stats `path`; `Ok(None)` means the object is absent.
async fn stat(op: &Operator, path: &str) -> Result<Option<::opendal::Metadata>, Error> {
    match op.stat(path).await {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == ::opendal::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage(path, "stat")(error)),
    }
}

/// Opens one streamed read of the whole object that `metadata` (from
/// [`stat`]) describes. When the operator supports conditional reads and the
/// stat returned an ETag, the read is conditional on it, so a replaced object
/// fails with [`ErrorKind::ConditionNotMatch`](::opendal::ErrorKind).
async fn stream(
    op: &Operator,
    path: &str,
    metadata: &::opendal::Metadata,
) -> Result<::opendal::BufferStream, Error> {
    let mut reader = op.reader_with(path);
    if op.info().capability().read_with_if_match
        && let Some(etag) = metadata.etag()
    {
        reader = reader.if_match(etag);
    }
    let reader = reader.await.map_err(read_failed(path))?;
    // The whole object in one request: a range capped at the stat length
    // would silently return a prefix of a grown object.
    reader.into_stream(..).await.map_err(read_failed(path))
}

/// The error of a read through [`stream`]. A failed `If-Match` means the
/// object was replaced after its stat, so the error is marked temporary:
/// retrying the operation is safe, as with [`changed`].
fn read_failed(path: &str) -> impl FnOnce(::opendal::Error) -> Error {
    let storage = storage(path, "read");
    move |error| {
        if error.kind() == ::opendal::ErrorKind::ConditionNotMatch {
            storage(error.set_temporary())
        } else {
            storage(error)
        }
    }
}

/// The error for an object whose read disagrees with its stat: it was
/// replaced, grew, or shrank in between. Retrying the operation is safe.
fn changed(path: &str) -> Error {
    storage(path, "read")(
        ::opendal::Error::new(
            ::opendal::ErrorKind::ConditionNotMatch,
            "the object changed between stat and read",
        )
        .set_temporary(),
    )
}

/// Why `key` is not a relative, normalized object key, if it is not: keys are
/// nonempty segments separated by single slashes, with no `.`/`..`,
/// backslashes, control characters, or surrounding whitespace. Such a key
/// cannot alias another key after OpenDAL's path normalization or escape the
/// prefix it is joined to.
fn key_issue(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        return Some("an object key cannot be empty");
    }
    if key.trim() != key {
        // OpenDAL trims surrounding whitespace from every path.
        return Some("an object key cannot start or end with whitespace");
    }
    if key.chars().any(|c| c == '\\' || c.is_control()) {
        return Some("an object key cannot contain backslashes or control characters");
    }
    key.split('/').find_map(|segment| match segment {
        "" => Some("an object key cannot have empty segments or slash ends"),
        "." | ".." => Some("an object key cannot contain dot segments"),
        _ => None,
    })
}

/// Checks that `key` is a relative, normalized object key (see [`key_issue`]).
pub(crate) fn check_key(key: &str) -> Result<(), Error> {
    match key_issue(key) {
        Some(reason) => Err(Error::InvalidPath {
            path: key.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

/// Checks that `prefix` is the root (`""`) or a normalized key followed by `/`.
pub(crate) fn check_prefix(prefix: &str) -> Result<(), Error> {
    if prefix.is_empty() {
        return Ok(());
    }
    match prefix.strip_suffix('/') {
        Some(key) => check_key(key).map_err(|_| Error::InvalidPath {
            path: prefix.to_owned(),
            reason: "a prefix must be empty or a normalized key ending in `/`",
        }),
        None => Err(Error::InvalidPath {
            path: prefix.to_owned(),
            reason: "a prefix must be empty or end in `/`",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_derive_from_namespace_name_and_version() {
        let spec: PackageSpec = "@preview/nested-name:10.0.1".parse().unwrap();
        assert_eq!(tree_key(&spec), "preview/nested-name/10.0.1");
        assert_eq!(
            archive_cache_key(&spec),
            "preview/nested-name/10.0.1.tar.gz"
        );
    }

    #[test]
    fn font_container_names_are_matched_case_insensitively() {
        for (key, supported) in [
            ("container.ttf", true),
            ("container.TTF", true),
            ("fonts/nested/container.otc", true),
            ("container.Ttc", true),
            ("container.oTf", true),
            (".ttf", true),
            ("container.woff", false),
            ("container.ttf.txt", false),
            ("container", false),
            ("ttf", false),
            ("fonts.ttf/container", false),
            ("", false),
        ] {
            assert_eq!(is_font_key(key), supported, "{key:?}");
        }
    }

    #[test]
    fn keys_must_be_relative_and_normalized() {
        for key in [
            "a",
            "a/b.txt",
            "dir/sub/file",
            "tree%/page%2F.svg",
            "café/%",
            "a\u{feff}",
            "a b",
        ] {
            assert!(check_key(key).is_ok(), "{key:?}");
        }
        for key in [
            "",
            "/a",
            "a/",
            "a//b",
            "./a",
            "a/../b",
            "..",
            "a\\b",
            "a\nb",
            "a/.",
            " a",
            "a ",
            "a\u{00a0}",
        ] {
            assert!(check_key(key).is_err(), "{key:?}");
        }
    }

    #[test]
    fn prefixes_are_the_root_or_end_in_a_slash() {
        for prefix in ["", "a/", "a/b/"] {
            assert!(check_prefix(prefix).is_ok(), "{prefix:?}");
        }
        for prefix in ["/", "a", "/a/", "a//", "../", "a/./"] {
            assert!(check_prefix(prefix).is_err(), "{prefix:?}");
        }
    }
}
