#![doc = include_str!(concat!(env!("OUT_DIR"), "/README.md"))]

/// The typst-pack release and embedded Typst engine versions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (Typst ",
    env!("TYPST_PACK_ENGINE_VERSION"),
    ")"
);

mod compilation_artifact_publication;
mod compile;
mod creation;
mod embedded;
mod extract;
mod font_catalog;
#[cfg(feature = "fs")]
mod fs_assembly;
#[cfg(feature = "fs")]
mod fs_fonts;
#[cfg(feature = "fs")]
mod fs_packages;
#[cfg(feature = "fs")]
mod fs_project;
mod manifest;
mod pack;
pub mod pack_archive;
mod pack_extraction;
#[cfg(feature = "package-acquisition")]
mod package_acquisition;
mod package_catalog;
mod package_failure;
mod payload;
mod project_snapshot;
mod world;
mod world_trace;

#[cfg(all(feature = "diagnostics", feature = "fs"))]
#[doc(hidden)]
pub mod cli_support;

pub use compilation_artifact_publication::{
    ArtifactPublicationPlanError, ArtifactPublicationPlanIssue,
    CompilationArtifactPublicationEntry, CompilationArtifactPublicationPlan,
    plan_compilation_artifact_publication,
};
pub use compile::{
    CompilationAccessKind, CompilationAccessObservation, CompilationAccessOutcome,
    CompilationAccessTrace, CompilationArtifact, CompilationDiagnostic, CompilationDocumentSummary,
    CompilationFulfillmentIssue, CompilationFulfillmentReport, CompilationFulfillmentSet,
    CompilationFulfillmentSetError, CompilationFulfillmentSetIssue, CompilationIdentity,
    CompilationLimitError, CompilationLimits, CompilationLimitsError, CompilationOperationOutcome,
    CompilationOutputOrigins, CompilationOutputSpecification, CompilationReport,
    CompilationReportOutcome, CompilationRequestInventory, CompilationRequestIssue,
    CompilationRequestRejection, CompilationResource, CompilationResult, CompilationResultIdentity,
    CompilationStatus, CreationTimestamp, DiagnosticHint, DiagnosticPhase, DiagnosticProducer,
    DiagnosticSeverity, DiagnosticTracepoint, DocumentTime, EffectiveEngineFeature,
    EffectiveRequestValue, EngineIdentity, ExporterIdentity, FontContainerFulfillment,
    FontFulfillmentReport, HtmlOutputSpecification, InvalidCompilationFulfillmentSet, LogicalSpan,
    OutputFormat, PackCompilationRequest, PackCompilationWarning, PackOverrideInventoryEntry,
    PackOverrideSet, PackOverrideSetError, PackOverridesInventory, PackageFulfillmentReport,
    PackageTreeFulfillment, PageRange, PageSelection, PdfOutputSpecification,
    PdfStandardsValidationError, PngOutputSpecification, RequestValueOrigin,
    SvgOutputSpecification, TracepointKind, TypstInputsInventory, TypstTarget, compile,
    parse_page_selection,
};
pub use creation::{
    DependencyDiscoveryRejection, DiscoverySpecification, DiscoverySpecificationError,
    PackCreationError, PackCreationInput, PackCreationOutcome, create,
};
#[cfg(feature = "fs")]
pub use extract::{ExtractError, ExtractOptions, ExtractReport, extract};
#[cfg(feature = "embedded-fonts")]
pub use font_catalog::typst_embedded_font_containers;
pub use font_catalog::{
    FontCatalog, FontCatalogEntry, FontCatalogFace, FontContainer, FontContainerError,
    FontContainerFace, FontDisposition,
};
#[cfg(feature = "fs")]
pub use fs_assembly::{
    FilesystemPackAssembler, FilesystemPackAssemblerConfig, FilesystemPackAssemblyClock,
    FilesystemPackAssemblyCreationError, FilesystemPackAssemblyDiscoveryError,
    FilesystemPackAssemblyError, FilesystemPackAssemblyProfile, FilesystemPackAssemblyRequest,
    PackAssemblyDiagnosticContext, PackAssemblyReport,
};
#[cfg(feature = "fs")]
pub use fs_fonts::{
    FilesystemFontContainerIssue, FilesystemFontEntryKind, FilesystemFontGatherError,
    FilesystemFontIssue, FilesystemFontLimitError, FilesystemFontLimits, FilesystemFontLimitsError,
    FilesystemFontOperation, FilesystemFontResource, FilesystemFontSource,
    FilesystemFontSurveyError, FilesystemFontValidationError, gather_filesystem_font_catalog,
};
#[cfg(feature = "fs")]
pub use fs_packages::{
    AcquiredPackage, FilesystemPackageAcquisitionError, FilesystemPackageAuthority,
    FilesystemPackageEntryKind, FilesystemPackageGatherError, FilesystemPackageIssue,
    FilesystemPackageLimitError, FilesystemPackageLimits, FilesystemPackageLimitsError,
    FilesystemPackageOperation, FilesystemPackageResource, FilesystemPackageSurveyError,
    gather_filesystem_package,
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
    FILE_EXTENSION, FontContainerIdentity, FontFaceIdentity, FontRequirement, Pack, PackBuildError,
    PackBuilder, PackFont, PackFontCatalogFace, PackIdentity, PackInvariantError,
    PackInvariantIssue, PackPathRole, PackageRequirement, PackageTreeIdentity,
};
pub use pack_extraction::{
    PackExtractionEntry, PackExtractionEntryRole, PackExtractionPlan, PackExtractionPlanError,
    PackExtractionPlanIssue, PackExtractionSelection, plan_pack_extraction,
};
#[cfg(feature = "package-acquisition")]
pub use package_acquisition::{
    PACKAGE_REGISTRY_NAMESPACE, PACKAGE_REGISTRY_URL, PackageAcquisitionError,
    PackageArchiveAcquisitionError, PackageExpansionLimitError, PackageExpansionLimits,
    PackageExpansionLimitsError, PackageExpansionResource, acquire_package_archive,
    expand_package_archive, package_archive_url,
};
pub use package_catalog::{
    PackageCatalog, PackageCatalogEntry, PackageCatalogError, PackageCatalogIssue,
    PackageDisposition, PackageTree, PackageTreeError, PackageTreeIssue,
};
pub use package_failure::{
    PackageAcquisitionFailure, PackageAcquisitionFailureReason, PackageAcquisitionFailures,
};
pub use payload::PackArchiveBytes;
pub use project_snapshot::{ProjectSnapshot, ProjectSnapshotAssembly, ProjectSnapshotError};
#[cfg(feature = "fs")]
pub use world::OfflineDownloader;

#[cfg(test)]
mod tests;
