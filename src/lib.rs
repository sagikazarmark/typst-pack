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
    CompilationAccessKind, CompilationAccessObservation, CompilationAccessOutcome,
    CompilationAccessTrace, CompilationArtifact, CompilationAttempt, CompilationDiagnostic,
    CompilationDocumentSummary, CompilationExecutionControls, CompilationFulfillmentReport,
    CompilationIdentity, CompilationOperationOutcome, CompilationReport, CompilationReportOutcome,
    CompilationRequestInventory, CompilationRequestRejection, CompilationResult,
    CompilationResultIdentity, CompilationStatus, CompilationTarget, CompileOptions,
    CreationTimestamp, DiagnosticHint, DiagnosticPhase, DiagnosticProducer, DiagnosticSeverity,
    DiagnosticTracepoint, EffectiveEngineFeature, EffectiveRequestValue, EngineIdentity,
    ExporterIdentity, FontContainerFulfillment, FontFulfillmentReport, LogicalSpan, OutputFormat,
    PackCompilationRequest, PackCompilationWarning, PackCompileError, PackOverrideInventoryEntry,
    PackOverrideSet, PackOverrideSetError, PackOverridesInventory, PackageFulfillmentReport,
    PackageTreeFulfillment, PageRange, PageSelection, PdfStandardsValidationError,
    RequestValueOrigin, TracepointKind, TypstInputsInventory, compile, compile_report,
    parse_page_selection,
};
#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
pub use manifest::{
    DiscoveryEvidence, DiscoveryObservationEvidence, DiscoveryOverrideEvidence, FORMAT_VERSION,
    FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata, PackageManifest,
    PackagesManifest, ProjectManifest,
};
pub use pack::{
    FILE_EXTENSION, FontCatalogError, FontContainerIdentity, FontFaceIdentity, FontRequirement,
    Pack, PackBuildError, PackBuilder, PackFont, PackFontCatalogFace, PackIdentity,
    PackInvariantError, PackPathRole, PackReadError, PackWriteError, PackageRequirement,
    PackageTreeError, PackageTreeIdentity,
};
#[cfg(feature = "fs")]
pub use packer::{
    CreationDiagnosticContext, DiscoveryAccessKind, DiscoveryAccessOutcome,
    DiscoveryInputsInventory, DiscoveryObservation, DiscoveryOverridesInventory, DiscoveryRequest,
    DiscoveryTarget, DiscoveryTrace, DiscoveryVariantReport, PackOutcome, PackReport, Packer,
    PackerError,
};
#[cfg(feature = "fs")]
pub use world::OfflineDownloader;

#[cfg(test)]
mod tests;
