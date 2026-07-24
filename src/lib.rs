#![doc = include_str!("../README.md")]

mod compile;
mod embedded;
mod extract;
mod manifest;
mod pack;
mod packer;
#[cfg(feature = "fs")]
mod project_snapshot;
mod world;
mod world_trace;

#[cfg(feature = "cli")]
pub mod cli;

pub use compile::{
    CompilationAccessKind, CompilationAccessObservation, CompilationAccessOutcome,
    CompilationAccessTrace, CompilationArtifact, CompilationDiagnostic, CompilationDocumentSummary,
    CompilationFulfillmentReport, CompilationIdentity, CompilationOperationOutcome,
    CompilationOutputOrigins, CompilationOutputSpecification, CompilationReport,
    CompilationReportOutcome, CompilationRequestInventory, CompilationRequestIssue,
    CompilationRequestRejection, CompilationResult, CompilationResultIdentity, CompilationStatus,
    CreationTimestamp, DiagnosticHint, DiagnosticPhase, DiagnosticProducer, DiagnosticSeverity,
    DiagnosticTracepoint, DocumentTime, EffectiveEngineFeature, EffectiveRequestValue,
    EngineIdentity, ExporterIdentity, FontContainerFulfillment, FontFulfillmentReport,
    HtmlOutputSpecification, LogicalSpan, OutputFormat, PackCompilationRequest,
    PackCompilationWarning, PackOverrideInventoryEntry, PackOverrideSet, PackOverrideSetError,
    PackOverridesInventory, PackageFulfillmentReport, PackageTreeFulfillment, PageRange,
    PageSelection, PdfOutputSpecification, PdfStandardsValidationError, PngOutputSpecification,
    RequestValueOrigin, SvgOutputSpecification, TracepointKind, TypstInputsInventory, TypstTarget,
    compile, parse_page_selection,
};
#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
pub use manifest::{
    FORMAT_VERSION, FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata,
    PackageManifest, PackagesManifest, ProjectManifest,
};
pub use pack::{
    FILE_EXTENSION, FontCatalogError, FontContainerIdentity, FontFaceIdentity, FontRequirement,
    Pack, PackBuildError, PackBuilder, PackFont, PackFontCatalogFace, PackIdentity,
    PackInvariantError, PackPathRole, PackReadError, PackWriteError, PackageRequirement,
    PackageTreeError, PackageTreeIdentity,
};
#[cfg(feature = "fs")]
pub use packer::{CreationDiagnosticContext, PackOutcome, Packer, PackerError};
#[cfg(feature = "fs")]
pub use world::OfflineDownloader;

#[cfg(test)]
mod tests;
