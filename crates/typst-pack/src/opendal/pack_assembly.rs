//! OpenDAL acquisition for Pack Assembly inputs.
//!
//! # Complete Pack Assembly
//!
//! The storage operations are caller-polled async operations. Their completed,
//! owned values compose through the existing synchronous Pack Creation loop:
//!
//! ```no_run
//! # #[cfg(feature = "package-acquisition")]
//! # mod complete {
//! use std::collections::HashSet;
//!
//! use typst::foundations::Dict;
//! use typst_pack::opendal::{Location, OperatorBindings};
//! use typst_pack::opendal::pack_assembly::{
//!     FontAcquisitionEntry, FontAcquisitionLimits, FontAcquisitionRequest, FontSource,
//!     PackageAcquisitionLimits, PackageAcquisitionRequest, PackageTreeSource,
//!     ProjectAcquisitionEntry, ProjectAcquisitionLimits, ProjectAcquisitionRequest,
//!     acquire_fonts, acquire_package, acquire_project, insert_acquired_package,
//! };
//! use typst_pack::opendal::publication::{
//!     PackageCacheArchivePublicationRequest, publish_package_cache_archive,
//! };
//! use typst_pack::{
//!     DiscoverySpecification, DocumentTime, FontCatalog, FontCatalogEntry, FontContainer,
//!     FontDisposition, Pack, PackCreationInput, PackCreationOutcome,
//!     PackageAcquisitionFailures, PackageCatalog, PackageDisposition,
//!     PackageExpansionLimits, ProjectSnapshotAssembly, TypstTarget, create,
//! };
//!
//! async fn assemble(bindings: &OperatorBindings) -> Result<Pack, Box<dyn std::error::Error>> {
//!     let project_request = ProjectAcquisitionRequest::new(
//!         "project:/sources/document/".parse::<Location>()?,
//!         ProjectAcquisitionLimits::reference_v1(),
//!     )?;
//!     let (_, project_entries) = acquire_project(bindings, &project_request).await?.into_parts();
//!     let project = ProjectSnapshotAssembly::new("main.typ").assemble(
//!         project_entries.into_iter().map(ProjectAcquisitionEntry::into_parts),
//!     )?;
//!
//!     let font_request = FontAcquisitionRequest::new(
//!         [FontSource::new(
//!             "fonts:/catalog/".parse::<Location>()?,
//!             FontDisposition::Embedded,
//!         )],
//!         FontAcquisitionLimits::reference_v1(),
//!     )?;
//!     let (_, font_entries) = acquire_fonts(bindings, &font_request).await?.into_parts();
//!     let mut fonts = FontCatalog::new();
//!     for entry in font_entries {
//!         let (_, _, _, disposition, bytes) = FontAcquisitionEntry::into_parts(entry);
//!         fonts.push(FontCatalogEntry::new(FontContainer::new(bytes)?, disposition));
//!     }
//!
//!     let tree_source = PackageTreeSource::new("packages:/trees/".parse::<Location>()?);
//!     let archive_cache = "packages:/cache/".parse::<Location>()?;
//!     let registry = "registry:/packages/".parse::<Location>()?;
//!     let discovery = DiscoverySpecification::new(
//!         TypstTarget::Paged,
//!         Dict::new(),
//!         DocumentTime::Absent,
//!         [],
//!     )?;
//!     let mut packages = PackageCatalog::new();
//!     let mut failures = PackageAcquisitionFailures::new();
//!     let mut attempted = HashSet::new();
//!
//!     loop {
//!         match create(PackCreationInput {
//!             project: &project,
//!             packages: &packages,
//!             fonts: &fonts,
//!             package_failures: &failures,
//!             discovery: &discovery,
//!             metadata: None,
//!         })? {
//!             PackCreationOutcome::Created { pack, warnings: _ } => return Ok(pack),
//!             PackCreationOutcome::MissingPackageSpecifications(missing) => {
//!                 for spec in missing {
//!                     if !attempted.insert(spec.to_string()) {
//!                         return Err("Pack Creation repeated an attempted specification".into());
//!                     }
//!                     let request = PackageAcquisitionRequest::new(
//!                         spec,
//!                         [tree_source.clone()],
//!                         Some(archive_cache.clone()),
//!                         Some(registry.clone()),
//!                         PackageAcquisitionLimits::reference_v1(),
//!                     )?;
//!                     let acquisition = acquire_package(bindings, &request).await?;
//!                     let insertion = insert_acquired_package(
//!                         &mut packages,
//!                         &mut failures,
//!                         acquisition,
//!                         PackageDisposition::Embedded,
//!                         PackageExpansionLimits::reference_v1(),
//!                     );
//!                     match insertion {
//!                         Ok(Some(residue)) => {
//!                             let publication = PackageCacheArchivePublicationRequest::new(
//!                                 residue.destination().clone(),
//!                             )?;
//!                             let _cache_result = publish_package_cache_archive(
//!                                 bindings,
//!                                 &publication,
//!                                 residue.bytes(),
//!                             ).await;
//!                             // Cache failure is separate evidence and does not
//!                             // invalidate the inserted Package Tree.
//!                         }
//!                         Ok(None) => {}
//!                         // Insertion retained the mapped Package Acquisition Failure.
//!                         // Resume so Dependency Discovery can attach it to the import.
//!                         Err(_error) => {}
//!                     }
//!                 }
//!             }
//!         }
//!     }
//! }
//! # }
//! ```

mod package;

pub use package::*;

use std::fmt;

use super::acquisition::recursive::{
    RecursiveAcquisitionLimits, RecursiveAcquisitionOperation, RecursiveAcquisitionResource,
    RecursiveAcquisitionSelection, RecursiveSurveyIssue, RecursiveSurveyIssueKind,
    acquire_recursive_prefix, acquire_recursive_prefixes,
};
use super::{BoxError, Location, LocationRoleError, OperatorResolver};
use crate::FontDisposition;
use crate::redacted_error::RedactedError;

fn aggregate_issue_message<T: fmt::Display>(issues: &[T], summary: &str) -> String {
    if let [issue] = issues {
        issue.to_string()
    } else {
        format!("{summary} with {} issue(s)", issues.len())
    }
}

fn failed_path_context(path: Option<&str>) -> String {
    path.map(|path| format!(" while reading object operation path {path:?}"))
        .unwrap_or_default()
}

/// Named finite ceilings for one OpenDAL Project Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_files: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

impl ProjectAcquisitionCeilings {
    /// The first-party version-1 Project Acquisition profile.
    pub const fn reference_v1() -> Self {
        Self {
            listed_entries: 1_000_000,
            listed_path_bytes: 64 * 1024,
            total_listed_path_bytes: 256 * 1024 * 1024,
            selected_files: 100_000,
            object_bytes: 256 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// A resource bounded during OpenDAL Project Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProjectAcquisitionResource {
    ListedEntries,
    ListedPathBytes,
    TotalListedPathBytes,
    SelectedFiles,
    ObjectBytes,
    TotalBytes,
}

/// A supplied Project Acquisition ceiling is internally inconsistent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: ProjectAcquisitionResource,
        ceiling: u64,
    },
    #[error("the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}")]
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}

/// Mandatory finite limits for OpenDAL Project Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectAcquisitionLimits {
    ceilings: ProjectAcquisitionCeilings,
}

impl ProjectAcquisitionLimits {
    /// Validates all named acquisition ceilings.
    pub fn new(
        ceilings: ProjectAcquisitionCeilings,
    ) -> Result<Self, ProjectAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                ProjectAcquisitionResource::ObjectBytes,
                ceilings.object_bytes,
            ),
            (ProjectAcquisitionResource::TotalBytes, ceilings.total_bytes),
        ] {
            if ceiling == u64::MAX {
                return Err(ProjectAcquisitionLimitsError::CannotProbe { resource, ceiling });
            }
        }
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(ProjectAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                object_bytes: ceilings.object_bytes,
                total_bytes: ceilings.total_bytes,
            });
        }
        Ok(Self { ceilings })
    }

    /// The validated first-party version-1 Project Acquisition limits.
    pub const fn reference_v1() -> Self {
        Self {
            ceilings: ProjectAcquisitionCeilings::reference_v1(),
        }
    }

    pub const fn listed_entries(&self) -> u64 {
        self.ceilings.listed_entries
    }

    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings.listed_path_bytes
    }

    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings.total_listed_path_bytes
    }

    pub const fn selected_files(&self) -> u64 {
        self.ceilings.selected_files
    }

    pub const fn object_bytes(&self) -> u64 {
        self.ceilings.object_bytes
    }

    pub const fn total_bytes(&self) -> u64 {
        self.ceilings.total_bytes
    }
}

/// Project Acquisition exceeded or could not account for a mandatory limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectAcquisitionLimitError {
    #[error(
        "OpenDAL Project Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: ProjectAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Project Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: ProjectAcquisitionResource,
    },
}

/// A validated request to acquire every yielded file below one prefix.
#[derive(Clone, Debug)]
pub struct ProjectAcquisitionRequest {
    source: Location,
    limits: ProjectAcquisitionLimits,
}

impl ProjectAcquisitionRequest {
    /// Validates a prefix source and retains its mandatory limits.
    pub fn new(
        source: Location,
        limits: ProjectAcquisitionLimits,
    ) -> Result<Self, ProjectAcquisitionRequestError> {
        if let Err(role_error) = source.require_prefix() {
            return Err(ProjectAcquisitionRequestError::InvalidSourceRole {
                location: source,
                source: role_error,
            });
        }
        Ok(Self { source, limits })
    }

    /// The normalized project prefix.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// The mandatory finite Project Acquisition limits.
    pub const fn limits(&self) -> ProjectAcquisitionLimits {
        self.limits
    }
}

/// A reason a Project Acquisition request is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectAcquisitionRequestError {
    #[error("project source {location} is not a prefix: {source}")]
    InvalidSourceRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// One exact path-and-byte entry acquired below a project prefix.
pub struct ProjectAcquisitionEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl ProjectAcquisitionEntry {
    /// The operation path relative to the requested prefix.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The exact bytes observed by the completed object read.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The acquired byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether this acquired object was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the owned path and exact bytes for Project Snapshot assembly.
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for ProjectAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectAcquisitionEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact entries acquired from one project prefix.
pub struct ProjectAcquisition {
    source: Location,
    entries: Vec<ProjectAcquisitionEntry>,
}

impl ProjectAcquisition {
    /// The normalized prefix from which entries were acquired.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// Acquired entries in relative operation-path order.
    pub fn entries(&self) -> &[ProjectAcquisitionEntry] {
        &self.entries
    }

    /// Recovers the source and owned entries for Project Snapshot assembly.
    pub fn into_parts(self) -> (Location, Vec<ProjectAcquisitionEntry>) {
        (self.source, self.entries)
    }
}

impl fmt::Debug for ProjectAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectAcquisition")
            .field("source", &self.source)
            .field("entries", &self.entries)
            .finish()
    }
}

/// An unsupported yielded OpenDAL entry kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProjectAcquisitionEntryKind {
    Unknown,
}

/// One structural issue found while surveying a project prefix.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectAcquisitionIssue {
    #[error("listed operation path {operation_path:?} is outside the project prefix")]
    ListedPathOutsidePrefix { operation_path: String },
    #[error("listed operation path {operation_path:?} is a prefix marker where a file is required")]
    PrefixMarkerWhereFileRequired { operation_path: String },
    #[error("listed operation path {operation_path:?} has an empty relative path")]
    EmptyRelativeOperationPath { operation_path: String },
    #[error("listed operation path {operation_path:?} is not a valid relative operation path")]
    InvalidRelativeOperationPath { operation_path: String },
    #[error("listed object {operation_path:?} was yielded more than once")]
    DuplicateListedObject { operation_path: String },
    #[error("listed operation path {operation_path:?} has unsupported kind {kind:?}")]
    UnsupportedEntryKind {
        operation_path: String,
        kind: ProjectAcquisitionEntryKind,
    },
}

/// The nonempty canonical set of structural project survey issues.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "project survey failed")
)]
pub struct ProjectAcquisitionSurveyError {
    issues: Vec<ProjectAcquisitionIssue>,
}

impl ProjectAcquisitionSurveyError {
    /// Every independently detectable issue in canonical order.
    pub fn issues(&self) -> &[ProjectAcquisitionIssue] {
        &self.issues
    }
}

/// Acquires every file entry yielded below one project prefix.
///
/// Directory markers are ignored. `.typkignore` is an ordinary file; Project
/// Snapshot assembly remains authoritative for canonical project paths,
/// collisions, `.typk` exclusion, entrypoint presence, bytes, and ordering.
/// The listing is one observation, not a storage snapshot or coexistence claim.
///
/// ```no_run
/// use typst::foundations::Dict;
/// use typst_pack::{
///     DiscoverySpecification, DocumentTime, FontCatalog, PackCreationInput,
///     PackageAcquisitionFailures, PackageCatalog, ProjectSnapshotAssembly,
///     TypstTarget, create,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     ProjectAcquisitionEntry, ProjectAcquisitionRequest, acquire_project,
/// };
///
/// async fn acquire_and_create(
///     bindings: &OperatorBindings,
///     request: &ProjectAcquisitionRequest,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let (_, entries) = acquire_project(bindings, request).await?.into_parts();
///     let project = ProjectSnapshotAssembly::new("main.typ").assemble(
///         entries.into_iter().map(ProjectAcquisitionEntry::into_parts),
///     )?;
///     let packages = PackageCatalog::new();
///     let fonts = FontCatalog::new();
///     let package_failures = PackageAcquisitionFailures::new();
///     let discovery = DiscoverySpecification::new(
///         TypstTarget::Paged,
///         Dict::new(),
///         DocumentTime::Absent,
///         [],
///     )?;
///     let _outcome = create(PackCreationInput {
///         project: &project,
///         packages: &packages,
///         fonts: &fonts,
///         package_failures: &package_failures,
///         discovery: &discovery,
///         metadata: None,
///     })?;
///     Ok(())
/// }
/// ```
pub async fn acquire_project<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &ProjectAcquisitionRequest,
) -> Result<ProjectAcquisition, ProjectAcquisitionError> {
    let source = request.source().clone();
    let entries = acquire_recursive_prefix(
        resolver,
        request.source(),
        RecursiveAcquisitionSelection::AllFiles,
        request.limits().into(),
        &ProjectAcquisitionOperation {
            source_location: request.source(),
        },
    )
    .await?
    .into_iter()
    .map(|object| ProjectAcquisitionEntry {
        relative_path: object.relative_path,
        bytes: object.bytes,
    })
    .collect();

    Ok(ProjectAcquisition { source, entries })
}

/// A failure while acquiring a project through OpenDAL.
///
/// This error's own `Display` and `Debug` omit native resolver and OpenDAL
/// messages. Rendering its complete source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Project Acquisition failed for binding {binding} at prefix operation path {operation_path:?}{failed_path}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
    failed_path = failed_path_context(.failed_path.as_deref()),
)]
pub struct ProjectAcquisitionError {
    source_location: Location,
    failed_path: Option<String>,
    #[source]
    cause: RedactedError<ProjectAcquisitionErrorCause>,
}

impl ProjectAcquisitionError {
    /// The normalized project prefix whose acquisition failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The selected object's operation path when one object read failed.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The typed cause of this failure.
    pub fn cause(&self) -> &ProjectAcquisitionErrorCause {
        self.cause.inner()
    }

    fn new(
        source_location: &Location,
        failed_path: Option<String>,
        cause: ProjectAcquisitionErrorCause,
    ) -> Self {
        Self {
            source_location: source_location.clone(),
            failed_path,
            cause: RedactedError::new(cause),
        }
    }
}

/// The typed cause of an OpenDAL Project Acquisition failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectAcquisitionErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("required listing or read capability is unsupported")]
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    #[error("the recursive listing failed")]
    List(#[source] ::opendal::Error),
    #[error("a listed object read failed")]
    Read(#[source] ::opendal::Error),
    #[error("a listed object was absent when read")]
    ListedObjectAbsent(#[source] ::opendal::Error),
    #[error("the completed listing had structural issues")]
    Structural(#[source] ProjectAcquisitionSurveyError),
    #[error("a Project Acquisition limit failed")]
    Limit(#[source] ProjectAcquisitionLimitError),
}

struct ProjectAcquisitionOperation<'a> {
    source_location: &'a Location,
}

impl RecursiveAcquisitionOperation for ProjectAcquisitionOperation<'_> {
    type Error = ProjectAcquisitionError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> ProjectAcquisitionError {
        unreachable!("ProjectAcquisitionRequest validates the prefix role")
    }

    fn resolve_operator(&self, _: usize, source: BoxError) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        _: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, _: usize, source: ::opendal::Error) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::List(source),
        )
    }

    fn read(
        &self,
        _: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            Some(operation_path),
            ProjectAcquisitionErrorCause::Read(source),
        )
    }

    fn listed_object_absent(
        &self,
        _: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            Some(operation_path),
            ProjectAcquisitionErrorCause::ListedObjectAbsent(source),
        )
    }

    fn structural(&self, _: usize, issues: Vec<RecursiveSurveyIssue>) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::Structural(ProjectAcquisitionSurveyError {
                issues: issues.into_iter().map(map_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        _: usize,
        resource: RecursiveAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    ) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::Exceeded {
                resource: map_resource(resource),
                ceiling,
                observed_at_least,
            }),
        )
    }

    fn accounting_overflow(
        &self,
        _: usize,
        resource: RecursiveAcquisitionResource,
    ) -> ProjectAcquisitionError {
        ProjectAcquisitionError::new(
            self.source_location,
            None,
            ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::AccountingOverflow {
                resource: map_resource(resource),
            }),
        )
    }
}

impl From<ProjectAcquisitionLimits> for RecursiveAcquisitionLimits {
    fn from(limits: ProjectAcquisitionLimits) -> Self {
        Self {
            listed_entries: limits.listed_entries(),
            listed_path_bytes: limits.listed_path_bytes(),
            total_listed_path_bytes: limits.total_listed_path_bytes(),
            selected_objects: limits.selected_files(),
            object_bytes: limits.object_bytes(),
            total_bytes: limits.total_bytes(),
        }
    }
}

fn map_resource(resource: RecursiveAcquisitionResource) -> ProjectAcquisitionResource {
    match resource {
        RecursiveAcquisitionResource::ListedEntries => ProjectAcquisitionResource::ListedEntries,
        RecursiveAcquisitionResource::ListedPathBytes => {
            ProjectAcquisitionResource::ListedPathBytes
        }
        RecursiveAcquisitionResource::TotalListedPathBytes => {
            ProjectAcquisitionResource::TotalListedPathBytes
        }
        RecursiveAcquisitionResource::SelectedObjects => ProjectAcquisitionResource::SelectedFiles,
        RecursiveAcquisitionResource::ObjectBytes => ProjectAcquisitionResource::ObjectBytes,
        RecursiveAcquisitionResource::TotalBytes => ProjectAcquisitionResource::TotalBytes,
    }
}

fn map_issue(issue: RecursiveSurveyIssue) -> ProjectAcquisitionIssue {
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            ProjectAcquisitionIssue::ListedPathOutsidePrefix { operation_path }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            ProjectAcquisitionIssue::PrefixMarkerWhereFileRequired { operation_path }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            ProjectAcquisitionIssue::EmptyRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath => {
            ProjectAcquisitionIssue::InvalidRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::DuplicateListedObject => {
            ProjectAcquisitionIssue::DuplicateListedObject { operation_path }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            ProjectAcquisitionIssue::UnsupportedEntryKind {
                operation_path,
                kind: ProjectAcquisitionEntryKind::Unknown,
            }
        }
    }
}

/// Named finite ceilings for one OpenDAL Font Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_containers: u64,
    pub container_bytes: u64,
    pub total_bytes: u64,
}

impl FontAcquisitionCeilings {
    /// The first-party version-1 Font Acquisition profile.
    pub const fn reference_v1() -> Self {
        Self {
            listed_entries: 100_000,
            listed_path_bytes: 64 * 1024,
            total_listed_path_bytes: 64 * 1024 * 1024,
            selected_containers: 16_384,
            container_bytes: 256 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// A resource bounded across one OpenDAL Font Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FontAcquisitionResource {
    ListedEntries,
    ListedPathBytes,
    TotalListedPathBytes,
    SelectedContainers,
    ContainerBytes,
    TotalBytes,
}

/// A supplied Font Acquisition ceiling is internally inconsistent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: FontAcquisitionResource,
        ceiling: u64,
    },
    #[error(
        "the ContainerBytes ceiling {container_bytes} exceeds the TotalBytes ceiling {total_bytes}"
    )]
    ContainerBytesExceedTotalBytes {
        container_bytes: u64,
        total_bytes: u64,
    },
}

/// Mandatory finite limits for OpenDAL Font Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontAcquisitionLimits {
    ceilings: FontAcquisitionCeilings,
}

impl FontAcquisitionLimits {
    /// Validates all named acquisition ceilings.
    pub fn new(ceilings: FontAcquisitionCeilings) -> Result<Self, FontAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                FontAcquisitionResource::ContainerBytes,
                ceilings.container_bytes,
            ),
            (FontAcquisitionResource::TotalBytes, ceilings.total_bytes),
        ] {
            if ceiling == u64::MAX {
                return Err(FontAcquisitionLimitsError::CannotProbe { resource, ceiling });
            }
        }
        if ceilings.container_bytes > ceilings.total_bytes {
            return Err(FontAcquisitionLimitsError::ContainerBytesExceedTotalBytes {
                container_bytes: ceilings.container_bytes,
                total_bytes: ceilings.total_bytes,
            });
        }
        Ok(Self { ceilings })
    }

    /// The validated first-party version-1 Font Acquisition limits.
    pub const fn reference_v1() -> Self {
        Self {
            ceilings: FontAcquisitionCeilings::reference_v1(),
        }
    }

    pub const fn listed_entries(&self) -> u64 {
        self.ceilings.listed_entries
    }

    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings.listed_path_bytes
    }

    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings.total_listed_path_bytes
    }

    pub const fn selected_containers(&self) -> u64 {
        self.ceilings.selected_containers
    }

    pub const fn container_bytes(&self) -> u64 {
        self.ceilings.container_bytes
    }

    pub const fn total_bytes(&self) -> u64 {
        self.ceilings.total_bytes
    }
}

/// Font Acquisition exceeded or could not account for a mandatory limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontAcquisitionLimitError {
    #[error(
        "OpenDAL Font Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: FontAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Font Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow { resource: FontAcquisitionResource },
}

/// One explicitly configured OpenDAL prefix of Font Containers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontSource {
    source: Location,
    disposition: FontDisposition,
}

impl FontSource {
    /// Associates one prefix with the disposition of every selected container.
    pub fn new(source: Location, disposition: FontDisposition) -> Self {
        Self {
            source,
            disposition,
        }
    }

    /// The normalized Font Container prefix.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// The disposition every selected container from this source carries.
    pub const fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// A validated request to acquire caller-ordered OpenDAL font prefixes.
#[derive(Clone, Debug)]
pub struct FontAcquisitionRequest {
    sources: Vec<FontSource>,
    limits: FontAcquisitionLimits,
}

impl FontAcquisitionRequest {
    /// Validates every source role before accepting the request.
    pub fn new(
        sources: impl IntoIterator<Item = FontSource>,
        limits: FontAcquisitionLimits,
    ) -> Result<Self, FontAcquisitionRequestRejection> {
        let sources = sources.into_iter().collect::<Vec<_>>();
        let issues = sources
            .iter()
            .enumerate()
            .filter_map(|(source_index, configured)| {
                configured.source.require_prefix().err().map(|source| {
                    FontAcquisitionRequestIssue::InvalidSourceRole {
                        source_index,
                        location: configured.source.clone(),
                        source,
                    }
                })
            })
            .collect::<Vec<_>>();
        if !issues.is_empty() {
            return Err(FontAcquisitionRequestRejection { issues });
        }
        Ok(Self { sources, limits })
    }

    /// Font sources in caller order.
    pub fn sources(&self) -> &[FontSource] {
        &self.sources
    }

    /// The mandatory finite limits shared across every configured source.
    pub const fn limits(&self) -> FontAcquisitionLimits {
        self.limits
    }
}

/// Every invalid source role in a rejected Font Acquisition request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "Font Acquisition request rejected")
)]
pub struct FontAcquisitionRequestRejection {
    issues: Vec<FontAcquisitionRequestIssue>,
}

impl FontAcquisitionRequestRejection {
    /// Invalid source roles in caller source order.
    pub fn issues(&self) -> &[FontAcquisitionRequestIssue] {
        &self.issues
    }
}

/// One invalid source role in a Font Acquisition request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontAcquisitionRequestIssue {
    #[error("font source {source_index} at {location} is not a prefix: {source}")]
    InvalidSourceRole {
        source_index: usize,
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// One exact Font Container selected and acquired from a configured source.
pub struct FontAcquisitionEntry {
    source_index: usize,
    source: Location,
    relative_path: String,
    disposition: FontDisposition,
    bytes: Vec<u8>,
}

impl FontAcquisitionEntry {
    /// The configured source's caller-order index.
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    /// The normalized prefix from which this entry was acquired.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// The selected operation path relative to its source prefix.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The explicit disposition inherited from the configured source.
    pub const fn disposition(&self) -> FontDisposition {
        self.disposition
    }

    /// The exact bytes observed by the completed object read.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The acquired byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether this acquired container is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers all owned source evidence and exact bytes.
    pub fn into_parts(self) -> (usize, Location, String, FontDisposition, Vec<u8>) {
        (
            self.source_index,
            self.source,
            self.relative_path,
            self.disposition,
            self.bytes,
        )
    }
}

impl fmt::Debug for FontAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontAcquisitionEntry")
            .field("source_index", &self.source_index)
            .field("source", &self.source)
            .field("relative_path", &self.relative_path)
            .field("disposition", &self.disposition)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact Font Containers acquired from caller-ordered sources.
pub struct FontAcquisition {
    sources: Vec<FontSource>,
    entries: Vec<FontAcquisitionEntry>,
}

impl FontAcquisition {
    /// Configured font sources in caller order.
    pub fn sources(&self) -> &[FontSource] {
        &self.sources
    }

    /// Acquired entries in source order, then relative operation-path order.
    pub fn entries(&self) -> &[FontAcquisitionEntry] {
        &self.entries
    }

    /// Recovers the configured sources and exact acquired entries.
    pub fn into_parts(self) -> (Vec<FontSource>, Vec<FontAcquisitionEntry>) {
        (self.sources, self.entries)
    }
}

impl fmt::Debug for FontAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontAcquisition")
            .field("sources", &self.sources)
            .field("entries", &self.entries)
            .finish()
    }
}

/// An unsupported yielded OpenDAL entry kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FontAcquisitionEntryKind {
    Unknown,
}

/// One structural issue found while surveying configured font prefixes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontAcquisitionIssue {
    #[error(
        "font source {source_index} listed operation path {operation_path:?} outside its prefix"
    )]
    ListedPathOutsidePrefix {
        source_index: usize,
        operation_path: String,
    },
    #[error(
        "font source {source_index} listed operation path {operation_path:?} as a prefix marker where a file is required"
    )]
    PrefixMarkerWhereFileRequired {
        source_index: usize,
        operation_path: String,
    },
    #[error(
        "font source {source_index} listed operation path {operation_path:?} with an empty relative path"
    )]
    EmptyRelativeOperationPath {
        source_index: usize,
        operation_path: String,
    },
    #[error("font source {source_index} listed invalid relative operation path {operation_path:?}")]
    InvalidRelativeOperationPath {
        source_index: usize,
        operation_path: String,
    },
    #[error("font source {source_index} listed object {operation_path:?} more than once")]
    DuplicateListedObject {
        source_index: usize,
        operation_path: String,
    },
    #[error(
        "font source {source_index} listed operation path {operation_path:?} with unsupported kind {kind:?}"
    )]
    UnsupportedEntryKind {
        source_index: usize,
        operation_path: String,
        kind: FontAcquisitionEntryKind,
    },
}

/// The nonempty canonical set of structural font survey issues.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "font survey failed")
)]
pub struct FontAcquisitionSurveyError {
    issues: Vec<FontAcquisitionIssue>,
}

impl FontAcquisitionSurveyError {
    /// Every independently detectable issue in source and path order.
    pub fn issues(&self) -> &[FontAcquisitionIssue] {
        &self.issues
    }
}

/// Acquires suffix-selected Font Containers from caller-ordered prefixes.
///
/// `.ttf`, `.ttc`, `.otf`, and `.otc` suffixes are matched
/// case-insensitively. Directory markers and non-font entries are ignored. All
/// selected entries come only from completed listing observations; those
/// observations make no storage snapshot or coexistence claim.
///
/// ```no_run
/// use typst_pack::{
///     DiscoverySpecification, FontCatalog, FontCatalogEntry, FontContainer,
///     PackCreationInput, PackageAcquisitionFailures, PackageCatalog,
///     ProjectSnapshot, create,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     FontAcquisitionRequest, acquire_fonts,
/// };
///
/// async fn acquire_fonts_and_create(
///     bindings: &OperatorBindings,
///     request: &FontAcquisitionRequest,
///     project: &ProjectSnapshot,
///     packages: &PackageCatalog,
///     package_failures: &PackageAcquisitionFailures,
///     discovery: &DiscoverySpecification,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let (_, acquired) = acquire_fonts(bindings, request).await?.into_parts();
///     let mut fonts = FontCatalog::new();
///     for entry in acquired {
///         let (_, _, _, disposition, bytes) = entry.into_parts();
///         let container = FontContainer::new(bytes)?;
///         fonts.push(FontCatalogEntry::new(container, disposition));
///     }
///     let _outcome = create(PackCreationInput {
///         project,
///         packages,
///         fonts: &fonts,
///         package_failures,
///         discovery,
///         metadata: None,
///     })?;
///     Ok(())
/// }
/// ```
pub async fn acquire_fonts<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &FontAcquisitionRequest,
) -> Result<FontAcquisition, FontAcquisitionError> {
    let locations = request
        .sources()
        .iter()
        .map(FontSource::source)
        .collect::<Vec<_>>();
    let acquired = acquire_recursive_prefixes(
        resolver,
        &locations,
        RecursiveAcquisitionSelection::FontContainers,
        request.limits().into(),
        &FontAcquisitionOperation {
            sources: request.sources(),
        },
    )
    .await?;

    let sources = request.sources().to_vec();
    let entries = acquired
        .into_iter()
        .enumerate()
        .flat_map(|(source_index, objects)| {
            let source = sources[source_index].clone();
            objects.into_iter().map(move |object| FontAcquisitionEntry {
                source_index,
                source: source.source.clone(),
                relative_path: object.relative_path,
                disposition: source.disposition,
                bytes: object.bytes,
            })
        })
        .collect();

    Ok(FontAcquisition { sources, entries })
}

/// A failure while acquiring Font Containers through OpenDAL.
///
/// This error's own `Display` and `Debug` omit native resolver and OpenDAL
/// messages. Rendering its complete source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Font Acquisition failed at source {source_index} for binding {binding} at prefix operation path {operation_path:?}{failed_path}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
    failed_path = failed_path_context(.failed_path.as_deref()),
)]
pub struct FontAcquisitionError {
    source_index: usize,
    source_location: Location,
    failed_path: Option<String>,
    #[source]
    cause: RedactedError<FontAcquisitionErrorCause>,
}

impl FontAcquisitionError {
    /// The caller-order index of the source at which acquisition failed.
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    /// The normalized font prefix at which acquisition failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The selected object's operation path when one object read failed.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The typed cause of this failure.
    pub fn cause(&self) -> &FontAcquisitionErrorCause {
        self.cause.inner()
    }

    fn new(
        source_index: usize,
        source_location: &Location,
        failed_path: Option<String>,
        cause: FontAcquisitionErrorCause,
    ) -> Self {
        Self {
            source_index,
            source_location: source_location.clone(),
            failed_path,
            cause: RedactedError::new(cause),
        }
    }
}

/// The typed cause of an OpenDAL Font Acquisition failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FontAcquisitionErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("required listing or read capability is unsupported")]
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    #[error("a recursive listing failed")]
    List(#[source] ::opendal::Error),
    #[error("a listed Font Container read failed")]
    Read(#[source] ::opendal::Error),
    #[error("a listed Font Container was absent when read")]
    ListedObjectAbsent(#[source] ::opendal::Error),
    #[error("the completed listings had structural issues")]
    Structural(#[source] FontAcquisitionSurveyError),
    #[error("a Font Acquisition limit failed")]
    Limit(#[source] FontAcquisitionLimitError),
}

struct FontAcquisitionOperation<'a> {
    sources: &'a [FontSource],
}

impl FontAcquisitionOperation<'_> {
    fn error(
        &self,
        source_index: usize,
        failed_path: Option<String>,
        cause: FontAcquisitionErrorCause,
    ) -> FontAcquisitionError {
        FontAcquisitionError::new(
            source_index,
            self.sources[source_index].source(),
            failed_path,
            cause,
        )
    }
}

impl RecursiveAcquisitionOperation for FontAcquisitionOperation<'_> {
    type Error = FontAcquisitionError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> FontAcquisitionError {
        unreachable!("FontAcquisitionRequest validates every prefix role")
    }

    fn resolve_operator(&self, source_index: usize, source: BoxError) -> FontAcquisitionError {
        self.error(
            source_index,
            None,
            FontAcquisitionErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        source_index: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            None,
            FontAcquisitionErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, source_index: usize, source: ::opendal::Error) -> FontAcquisitionError {
        self.error(source_index, None, FontAcquisitionErrorCause::List(source))
    }

    fn read(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            Some(operation_path),
            FontAcquisitionErrorCause::Read(source),
        )
    }

    fn listed_object_absent(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            Some(operation_path),
            FontAcquisitionErrorCause::ListedObjectAbsent(source),
        )
    }

    fn structural(
        &self,
        source_index: usize,
        issues: Vec<RecursiveSurveyIssue>,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            None,
            FontAcquisitionErrorCause::Structural(FontAcquisitionSurveyError {
                issues: issues.into_iter().map(map_font_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        source_index: usize,
        resource: RecursiveAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            None,
            FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
                resource: map_font_resource(resource),
                ceiling,
                observed_at_least,
            }),
        )
    }

    fn accounting_overflow(
        &self,
        source_index: usize,
        resource: RecursiveAcquisitionResource,
    ) -> FontAcquisitionError {
        self.error(
            source_index,
            None,
            FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::AccountingOverflow {
                resource: map_font_resource(resource),
            }),
        )
    }
}

impl From<FontAcquisitionLimits> for RecursiveAcquisitionLimits {
    fn from(limits: FontAcquisitionLimits) -> Self {
        Self {
            listed_entries: limits.listed_entries(),
            listed_path_bytes: limits.listed_path_bytes(),
            total_listed_path_bytes: limits.total_listed_path_bytes(),
            selected_objects: limits.selected_containers(),
            object_bytes: limits.container_bytes(),
            total_bytes: limits.total_bytes(),
        }
    }
}

fn map_font_resource(resource: RecursiveAcquisitionResource) -> FontAcquisitionResource {
    match resource {
        RecursiveAcquisitionResource::ListedEntries => FontAcquisitionResource::ListedEntries,
        RecursiveAcquisitionResource::ListedPathBytes => FontAcquisitionResource::ListedPathBytes,
        RecursiveAcquisitionResource::TotalListedPathBytes => {
            FontAcquisitionResource::TotalListedPathBytes
        }
        RecursiveAcquisitionResource::SelectedObjects => {
            FontAcquisitionResource::SelectedContainers
        }
        RecursiveAcquisitionResource::ObjectBytes => FontAcquisitionResource::ContainerBytes,
        RecursiveAcquisitionResource::TotalBytes => FontAcquisitionResource::TotalBytes,
    }
}

fn map_font_issue(issue: RecursiveSurveyIssue) -> FontAcquisitionIssue {
    let source_index = issue.source_index;
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            FontAcquisitionIssue::ListedPathOutsidePrefix {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            FontAcquisitionIssue::PrefixMarkerWhereFileRequired {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            FontAcquisitionIssue::EmptyRelativeOperationPath {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath => {
            FontAcquisitionIssue::InvalidRelativeOperationPath {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::DuplicateListedObject => {
            FontAcquisitionIssue::DuplicateListedObject {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            FontAcquisitionIssue::UnsupportedEntryKind {
                source_index,
                operation_path,
                kind: FontAcquisitionEntryKind::Unknown,
            }
        }
    }
}
