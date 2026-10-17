#![doc = include_str!(concat!(env!("OUT_DIR"), "/README.md"))]

/// The typst-pack release and embedded Typst engine versions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (Typst ",
    env!("TYPST_PACK_ENGINE_VERSION"),
    ")"
);

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
pub use digest::Digest;
pub use limits::{LimitExceeded, ReadLimits};
pub use manifest::Metadata;
pub use pack::{FILE_EXTENSION, Pack, PackBuilder, PackError, PackIssue};
pub use project::ProjectSnapshot;
pub use world::Sources;

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
