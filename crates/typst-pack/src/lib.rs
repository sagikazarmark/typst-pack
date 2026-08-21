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
mod domain;
mod embedded;
#[cfg(feature = "fs")]
mod error_display;
#[cfg(feature = "fs")]
mod filesystem_write;
mod font_catalog;
#[cfg(feature = "fs")]
mod fs_assembly;
#[cfg(feature = "fs")]
mod fs_fonts;
#[cfg(feature = "fs")]
mod fs_packages;
#[cfg(feature = "fs")]
mod fs_project;
mod identity;
mod limits;
mod manifest;
#[cfg(feature = "opendal")]
pub mod opendal;
mod pack;
pub mod pack_archive;
mod pack_extraction;
mod package_catalog;
mod package_failure;
#[cfg(feature = "package-reading")]
mod package_reading;
mod paths;
mod payload;
mod project_snapshot;
mod read_layout;
#[cfg(feature = "opendal")]
mod redacted_error;
mod world;
mod world_trace;
mod write;

#[cfg(all(feature = "diagnostics", feature = "fs"))]
#[doc(hidden)]
pub mod cli_support;

pub use compile::{
    CompilationAccessKind, CompilationAccessObservation, CompilationAccessOutcome,
    CompilationAccessTrace, CompilationArtifact, CompilationDiagnostic, CompilationDocumentSummary,
    CompilationFulfillmentIssue, CompilationFulfillmentReport, CompilationFulfillmentSet,
    CompilationFulfillmentSetError, CompilationFulfillmentSetIssue, CompilationLimitError,
    CompilationLimits, CompilationOperationOutcome, CompilationOutputSpecification,
    CompilationReport, CompilationReportOutcome, CompilationRequestIssue,
    CompilationRequestRejection, CompilationResource, CompilationResult, CompilationStatus,
    CreationTimestamp, DiagnosticHint, DiagnosticPhase, DiagnosticProducer, DiagnosticSeverity,
    DiagnosticTracepoint, FontContainerFulfillment, FontFulfillmentReport, HtmlOutputSpecification,
    ImplementationIdentity, ImplementationRole, InvalidCompilationFulfillmentSet, LogicalSpan,
    OutputFormat, PackCompilationRequest, PackCompilationWarning, PackOverrideSet,
    PackOverrideSetError, PackageFulfillmentReport, PackageTreeFulfillment, PageRange,
    PageSelection, PdfOutputSpecification, PdfStandardsValidationError, PngOutputSpecification,
    SvgOutputSpecification, TracepointKind, compile, compile_with_limits, parse_page_selection,
    resolve_external_font_requirements,
};
pub use creation::{
    DependencyDiscoveryRejection, DiscoverySpecification, DiscoverySpecificationError,
    PackCreationError, PackCreationInput, PackCreationOutcome, create,
};
pub use domain::{DocumentTime, TypstTarget};
#[cfg(feature = "fs")]
pub use filesystem_write::{
    CompilationArtifactPathWriteError, CompilationArtifactWriteError,
    CompilationArtifactWriteIssue, FilesystemDestinationEntryKind, FilesystemMergePolicy,
    FilesystemWriteErrorCause, FilesystemWritePathError, FilesystemWritePhase,
    FilesystemWritePreflightIssue, PackExtractionWriteError, resolve_filesystem_write_paths,
    write_compilation_artifacts_to_filesystem_paths, write_pack_extraction_plan_to_filesystem,
};
#[cfg(all(feature = "fs", fuzzing))]
#[doc(hidden)]
pub use filesystem_write::{
    FilesystemWriteFaultProbe, write_pack_extraction_plan_to_filesystem_with_fault_probe,
};
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
    FilesystemFontContainerIssue, FilesystemFontEntryKind, FilesystemFontIssue,
    FilesystemFontLimitError, FilesystemFontLimits, FilesystemFontOperation,
    FilesystemFontReadError, FilesystemFontResource, FilesystemFontSource,
    FilesystemFontSurveyError, FilesystemFontValidationError, read_filesystem_fonts,
};
#[cfg(feature = "fs")]
pub use fs_packages::{
    FilesystemPackageAuthority, FilesystemPackageAuthorityReadError, FilesystemPackageEntryKind,
    FilesystemPackageIssue, FilesystemPackageLimitError, FilesystemPackageLimits,
    FilesystemPackageOperation, FilesystemPackageReadError, FilesystemPackageResource,
    FilesystemPackageSurveyError, ReadPackage, read_filesystem_package,
};
#[cfg(feature = "fs")]
pub use fs_project::{
    FilesystemProjectEntryKind, FilesystemProjectIssue, FilesystemProjectLimitError,
    FilesystemProjectLimits, FilesystemProjectOperation, FilesystemProjectPolicyError,
    FilesystemProjectReadError, FilesystemProjectResource, FilesystemProjectSurveyError,
    IGNORE_FILE, read_filesystem_project,
};
pub use identity::{CanonicalIdentity, CanonicalIdentityRole};
pub use limits::{LimitError, Limits, Resource, ResourceKind};
pub use manifest::PackMetadata;
pub use pack::{
    FILE_EXTENSION, FontFaceIdentity, FontRequirement, Pack, PackBuildError, PackBuilder, PackFont,
    PackFontCatalogFace, PackInvariantError, PackInvariantIssue, PackPathRole, PackageRequirement,
};
pub use pack_extraction::{
    PackExtractionEntry, PackExtractionEntryRole, PackExtractionPlan, PackExtractionPlanError,
    PackExtractionPlanIssue, PackExtractionSelection, plan_pack_extraction,
};
pub use package_catalog::{
    PackageCatalog, PackageCatalogEntry, PackageCatalogError, PackageCatalogIssue,
    PackageDisposition, PackageTree, PackageTreeError, PackageTreeIssue,
};
pub use package_failure::{PackageReadFailure, PackageReadFailureReason, PackageReadFailures};
#[cfg(feature = "package-reading")]
pub use package_reading::{
    PACKAGE_REGISTRY_NAMESPACE, PACKAGE_REGISTRY_URL, PackageArchiveReadError,
    PackageExpansionLimitError, PackageExpansionLimits, PackageExpansionResource, PackageReadError,
    expand_package_archive, package_archive_url, read_package_archive,
};
pub use payload::PackArchiveBytes;
pub use project_snapshot::{
    ProjectSnapshot, ProjectSnapshotAssembly, ProjectSnapshotError, ProjectSnapshotIssue,
};
pub use write::{
    CommitCertainty, CompilationArtifactWriteEntry, CompilationArtifactWriteProgress,
    CompilationArtifactWriteReceipt, PackExtractionWriteEntry, PackExtractionWriteProgress,
    PackExtractionWriteReceipt, WriteKeyOutcome,
};

#[cfg(test)]
mod tests;
