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
mod manifest;
mod pack;
pub mod pack_archive;
#[cfg(feature = "package-acquisition")]
mod package_acquisition;
mod package_catalog;
mod packer;
mod payload;
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
pub use creation::{CreationError, CreationOutcome, CreationRequest, IssuedPack, create};
#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
#[cfg(feature = "embedded-fonts")]
pub use font_catalog::typst_embedded_font_containers;
pub use font_catalog::{
    FontCatalog, FontCatalogEntry, FontCatalogFace, FontContainer, FontContainerError,
    FontContainerFace, FontDisposition,
};
#[cfg(feature = "fs")]
pub use fs_project::{
    FilesystemProjectEntryKind, FilesystemProjectGatherError, FilesystemProjectIssue,
    FilesystemProjectLimitError, FilesystemProjectLimits, FilesystemProjectLimitsError,
    FilesystemProjectOperation, FilesystemProjectPolicyError, FilesystemProjectResource,
    FilesystemProjectSurveyError, IGNORE_FILE, gather_filesystem_project,
};
pub use manifest::PackMetadata;
pub use pack::{
    FILE_EXTENSION, FontCatalogError, FontContainerIdentity, FontFaceIdentity, FontRequirement,
    Pack, PackBuildError, PackBuilder, PackFont, PackFontCatalogFace, PackIdentity,
    PackInvariantError, PackInvariantIssue, PackPathRole, PackageRequirement, PackageTreeIdentity,
};
#[cfg(feature = "package-acquisition")]
pub use package_acquisition::{
    PACKAGE_REGISTRY_NAMESPACE, PACKAGE_REGISTRY_URL, PackageAcquisitionError,
    PackageExpansionLimitError, PackageExpansionLimits, PackageExpansionLimitsError,
    PackageExpansionResource, expand_package_archive, package_archive_url,
};
pub use package_catalog::{
    PackageCatalog, PackageCatalogEntry, PackageCatalogError, PackageCatalogIssue,
    PackageDisposition, PackageTree, PackageTreeError, PackageTreeIssue,
};
#[cfg(feature = "fs")]
pub use packer::{CreationDiagnosticContext, PackOutcome, Packer, PackerError};
pub use payload::PackArchiveBytes;
pub use project_snapshot::{ProjectSnapshot, ProjectSnapshotAssembly, ProjectSnapshotError};
#[cfg(feature = "fs")]
pub use world::OfflineDownloader;

#[cfg(test)]
mod tests;
