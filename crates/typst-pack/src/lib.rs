#![doc = include_str!(concat!(env!("OUT_DIR"), "/README.md"))]
#![forbid(unsafe_code)]

/// The typst-pack release and embedded Typst engine versions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (Typst ",
    env!("TYPST_PACK_ENGINE_VERSION"),
    ")"
);

/// The version of the embedded Typst engine, such as `0.15.1`; packs this
/// crate creates record it as their `typst-version`.
pub const TYPST_VERSION: &str = env!("TYPST_PACK_ENGINE_VERSION");

pub mod archive;
pub mod compile;
pub mod create;
mod digest;
pub mod extract;
pub mod font;
#[cfg(feature = "fs")]
pub mod fs;
mod limits;
mod manifest;
#[cfg(feature = "opendal")]
pub mod opendal;
mod pack;
pub mod package;
mod paths;
mod payload;
mod project;
mod world;

pub use compile::compile;
pub use create::create;
pub use digest::{Digest, InvalidDigest};
pub use limits::{LimitExceeded, ReadLimits};
pub use manifest::Metadata;
pub use pack::{FILE_EXTENSION, Pack, PackBuilder, PackError, PackIssue};
pub use project::ProjectSnapshot;
pub use world::Sources;

/// An error and its `source()` chain as one line, `error: source: ...`, for
/// recording an error as a message.
#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
pub(crate) fn chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
}

/// Formats an aggregate error as its only issue, or a summary followed by
/// every issue.
pub(crate) fn list(summary: &str, issues: &[impl std::fmt::Display]) -> String {
    match issues {
        [issue] => issue.to_string(),
        issues => {
            let issues = issues.iter().map(ToString::to_string).collect::<Vec<_>>();
            format!("{summary}: {}", issues.join("; "))
        }
    }
}

#[cfg(test)]
mod tests;

/// Compile-checks the library guide's examples, which use the filesystem
/// adapter. The guide is only staged in the workspace; see `build.rs`.
#[cfg(all(doctest, feature = "fs", typst_pack_library_guide))]
#[doc = include_str!(concat!(env!("OUT_DIR"), "/library-guide.md"))]
struct LibraryGuide;
