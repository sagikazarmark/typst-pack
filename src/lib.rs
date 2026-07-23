#![doc = include_str!("../README.md")]

mod compile;
mod embedded;
mod extract;
mod manifest;
mod pack;
mod packer;
mod resource;
mod world;

#[cfg(feature = "cli")]
pub mod cli;

pub use compile::{
    CompilationArtifact, CompilationDiagnostic, CompilationIdentity, CompilationOperationOutcome,
    CompilationOutput, CompilationRequestInventory, CompilationRequestRejection, CompilationResult,
    CompilationStatus, CompileError, CompileOptions, CreationTimestamp, DiagnosticHint,
    DiagnosticPhase, DiagnosticProducer, DiagnosticSeverity, DiagnosticTracepoint,
    EffectiveEngineFeature, EffectiveRequestValue, EngineIdentity, ExporterIdentity,
    FontContainerFulfillment, LogicalSpan, OutputFormat, PackCompilationRequest,
    PackCompilationWarning, PackCompileError, PackOverrideInventoryEntry, PackOverrideSet,
    PackOverrideSetError, PackOverridesInventory, PageRange, PageSelection,
    PdfStandardsValidationError, RequestValueOrigin, TracepointKind, TypstInputsInventory, compile,
    compile_pack, parse_page_selection,
};
pub use manifest::{
    FORMAT_VERSION, FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata,
    PackagesManifest, ProjectManifest,
};
pub use pack::{
    FILE_EXTENSION, FontCatalogError, FontContainerIdentity, FontFaceIdentity, FontRequirement,
    Pack, PackBuildError, PackBuilder, PackFont, PackFontCatalogFace, PackIdentity,
    PackInvariantError, PackPathRole, PackReadError, PackWriteError,
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
