#![doc = include_str!("../docs/opendal-integration.md")]

#[cfg(feature = "package-reading")]
mod assembler;
mod read;
mod write;

pub use ::opendal::Operator;
#[cfg(feature = "package-reading")]
pub use assembler::{AssembledPack, OpenDalPackAssembler};
#[cfg(feature = "package-reading")]
pub use read::insert_package;
pub use read::{
    FontSource, PackageRead, PackageSource, read_fonts, read_pack_archive, read_package,
    read_project,
};
pub use write::{
    WriteError, WriteOutcome, WritePolicy, write_compilation_artifacts, write_pack_archive,
    write_pack_extraction_plan, write_package_archive,
};

use crate::LimitExceeded;

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
    #[error("{exceeded} while reading {path:?}")]
    Limit {
        path: String,
        exceeded: LimitExceeded,
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
    /// The number of artifact keys does not match the number of artifacts.
    #[error("expected {expected} artifact key(s), got {actual}")]
    ArtifactKeyCount { expected: usize, actual: usize },
    /// The read project files do not form a valid project.
    #[error(transparent)]
    Project(#[from] crate::PackInvariantError),
    /// A selected font object is not a valid Font Container.
    #[error("font object {path:?} is not a valid font container")]
    Font {
        path: String,
        #[source]
        source: crate::FontContainerError,
    },
    /// Read package files do not form a valid Package Tree.
    #[error(transparent)]
    PackageTree(#[from] crate::PackageTreeError),
    /// A read package archive could not be expanded.
    #[cfg(feature = "package-reading")]
    #[error(transparent)]
    PackageArchive(#[from] crate::PackageReadError),
    /// The Package Catalog rejected a read package.
    #[error(transparent)]
    PackageCatalog(#[from] crate::PackageCatalogError),
    /// Pack creation failed.
    #[cfg(feature = "package-reading")]
    #[error(transparent)]
    Creation(Box<crate::CreateError>),
    /// Pack Creation reported a package missing again after it was read.
    #[cfg(feature = "package-reading")]
    #[error("Pack Creation requested package {spec} again after it was read")]
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

/// Checks that `key` is a relative, normalized object key: nonempty segments
/// separated by single slashes, no `.`/`..`, backslashes, control characters,
/// or surrounding whitespace. Such a key cannot alias another key after
/// OpenDAL's path normalization or escape the prefix it is joined to.
pub(crate) fn check_key(key: &str) -> Result<(), Error> {
    let invalid = |reason| {
        Err(Error::InvalidPath {
            path: key.to_owned(),
            reason,
        })
    };
    if key.is_empty() {
        return invalid("an object key cannot be empty");
    }
    if key.trim() != key {
        // OpenDAL trims surrounding whitespace from every path.
        return invalid("an object key cannot start or end with whitespace");
    }
    if key.chars().any(|c| c == '\\' || c.is_control()) {
        return invalid("an object key cannot contain backslashes or control characters");
    }
    for segment in key.split('/') {
        match segment {
            "" => return invalid("an object key cannot have empty segments or slash ends"),
            "." | ".." => return invalid("an object key cannot contain dot segments"),
            _ => {}
        }
    }
    Ok(())
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
    use super::{check_key, check_prefix};

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
