#![doc = include_str!("../README.md")]

mod compile;
mod extract;
mod manifest;
mod pack;
mod packer;
mod resource;
mod world;

#[cfg(feature = "cli")]
pub mod cli;

pub use compile::{
    CompilationArtifact, CompilationOutput, CompileError, CompileOptions, OutputFormat, PageRange,
    PageSelection, compile, parse_page_selection,
};
pub use manifest::{
    FORMAT_VERSION, FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata,
    PackagesManifest, ProjectManifest,
};
pub use pack::{
    FILE_EXTENSION, Pack, PackBuildError, PackBuilder, PackFont, PackInvariantError, PackPathRole,
    PackReadError, PackWriteError,
};
pub use world::{PackWorld, PackWorldBuilder};

#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
#[cfg(feature = "fs")]
pub use packer::{DiscoveryTarget, DiscoveryWorld, PackOutcome, PackReport, Packer, PackerError};
#[cfg(feature = "fs")]
pub use world::{OfflineDownloader, SystemPackageLoader};

#[cfg(test)]
mod tests;
