#![doc = include_str!(concat!(env!("OUT_DIR"), "/README.md"))]

/// The typst-pack release and embedded Typst engine versions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (Typst ",
    env!("TYPST_PACK_ENGINE_VERSION"),
    ")"
);

mod compile;
mod creation;
mod embedded;
mod extract;
mod font_catalog;
#[cfg(feature = "fs")]
mod fs_packages;
#[cfg(feature = "fs")]
mod fs_project;
mod ignore_policy;
mod manifest;
mod pack;
mod packer;
mod project_snapshot;
mod world;
mod world_trace;

#[cfg(all(feature = "diagnostics", feature = "fs"))]
#[doc(hidden)]
pub mod cli_support;

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
pub use creation::{
    CreationError, CreationOutcome, CreationRequest, IssuedPack, PackageDisposition,
    ResolvedPackageTree, create,
};
#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
#[cfg(feature = "embedded-fonts")]
pub use font_catalog::typst_embedded_font_containers;
pub use font_catalog::{
    CandidateFontCatalog, CandidateFontContainer, CandidateFontFace, FontDisposition,
};
pub use ignore_policy::{IGNORE_FILE, ProjectIgnorePolicy, ProjectIgnorePolicyError};
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
pub use project_snapshot::{
    ProjectSnapshot, ProjectSnapshotAssembly, ProjectSnapshotBudget, ProjectSnapshotError,
};
#[cfg(feature = "fs")]
pub use world::OfflineDownloader;

#[cfg(test)]
mod tests;
