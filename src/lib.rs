#![doc = include_str!("../README.md")]

mod compile;
mod extract;
mod manifest;
mod pack;
mod packer;
mod world;

#[cfg(feature = "cli")]
pub mod cli;

pub use compile::{
    CompileError, CompileOptions, CompileOutput, OutputFormat, PageRange, compile, parse_pages,
};
pub use manifest::{
    FORMAT_VERSION, FontManifest, MANIFEST_PATH, Manifest, ManifestError, Metadata,
    PackagesManifest, ProjectManifest,
};
pub use pack::{
    FILE_EXTENSION, Pack, PackBuildError, PackBuilder, PackFont, PackReadError, PackWriteError,
};
pub use world::{PackWorld, PackWorldBuilder, PackWorldError};

#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
#[cfg(feature = "fs")]
pub use packer::{
    DiscoveryWorld, PackOutcome, PackReport, Packer, PackerError, ProjectResourcePolicy,
};
#[cfg(feature = "fs")]
pub use world::{OfflineDownloader, SystemPackageLoader};

#[cfg(test)]
mod tests;
