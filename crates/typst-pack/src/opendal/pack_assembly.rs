//! OpenDAL acquisition for Pack Assembly inputs.

use std::{error::Error, fmt};

use super::acquisition::recursive::{
    RecursiveAcquisitionError, RecursiveAcquisitionLimits, RecursiveAcquisitionResource,
    RecursiveAcquisitionSelection, RecursiveSurveyIssue, RecursiveSurveyIssueKind,
    acquire_recursive_prefix,
};
use super::{Location, LocationRoleError, OperatorResolver};

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAcquisitionSurveyError {
    issues: Vec<ProjectAcquisitionIssue>,
}

impl ProjectAcquisitionSurveyError {
    /// Every independently detectable issue in canonical order.
    pub fn issues(&self) -> &[ProjectAcquisitionIssue] {
        &self.issues
    }
}

impl fmt::Display for ProjectAcquisitionSurveyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            issue.fmt(formatter)
        } else {
            write!(
                formatter,
                "project survey failed with {} issue(s)",
                self.issues.len()
            )
        }
    }
}

impl Error for ProjectAcquisitionSurveyError {}

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
) -> Result<ProjectAcquisition, ProjectAcquisitionError<R::Error>> {
    let source = request.source().clone();
    let entries = acquire_recursive_prefix(
        resolver,
        request.source(),
        RecursiveAcquisitionSelection::AllFiles,
        request.limits().into(),
    )
    .await
    .map_err(|error| ProjectAcquisitionError::from_recursive(source.clone(), error))?
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
/// messages. Rendering the complete source chain may disclose backend context.
pub struct ProjectAcquisitionError<E> {
    source_location: Location,
    failed_path: Option<String>,
    cause: ProjectAcquisitionErrorCause<E>,
}

impl<E> ProjectAcquisitionError<E> {
    /// The normalized project prefix whose acquisition failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The selected object's operation path when one object read failed.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The typed cause of this failure.
    pub fn cause(&self) -> &ProjectAcquisitionErrorCause<E> {
        &self.cause
    }

    fn from_recursive(source_location: Location, error: RecursiveAcquisitionError<E>) -> Self {
        let (failed_path, cause) = match error {
            RecursiveAcquisitionError::InvalidLocationRole(_) => {
                unreachable!("ProjectAcquisitionRequest validates the prefix role")
            }
            RecursiveAcquisitionError::ResolveOperator(source) => {
                (None, ProjectAcquisitionErrorCause::ResolveOperator(source))
            }
            RecursiveAcquisitionError::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            } => (
                None,
                ProjectAcquisitionErrorCause::UnsupportedCapabilities {
                    list,
                    list_with_recursive,
                    read,
                },
            ),
            RecursiveAcquisitionError::List(source) => {
                (None, ProjectAcquisitionErrorCause::List(source))
            }
            RecursiveAcquisitionError::Read {
                operation_path,
                source,
            } => (
                Some(operation_path),
                ProjectAcquisitionErrorCause::Read(source),
            ),
            RecursiveAcquisitionError::ListedObjectAbsent {
                operation_path,
                source,
            } => (
                Some(operation_path),
                ProjectAcquisitionErrorCause::ListedObjectAbsent(source),
            ),
            RecursiveAcquisitionError::Structural(issues) => (
                None,
                ProjectAcquisitionErrorCause::Structural(ProjectAcquisitionSurveyError {
                    issues: issues.into_iter().map(map_issue).collect(),
                }),
            ),
            RecursiveAcquisitionError::InvalidPackageTree(_) => {
                unreachable!("project acquisition does not run Package Tree preflight")
            }
            RecursiveAcquisitionError::Limit {
                resource,
                ceiling,
                observed_at_least,
            } => (
                None,
                ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::Exceeded {
                    resource: map_resource(resource),
                    ceiling,
                    observed_at_least,
                }),
            ),
            RecursiveAcquisitionError::AccountingOverflow { resource } => (
                None,
                ProjectAcquisitionErrorCause::Limit(
                    ProjectAcquisitionLimitError::AccountingOverflow {
                        resource: map_resource(resource),
                    },
                ),
            ),
        };
        Self {
            source_location,
            failed_path,
            cause,
        }
    }
}

impl<E> fmt::Display for ProjectAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Project Acquisition failed for binding {} at prefix operation path {:?}",
            self.source_location.binding(),
            self.source_location.operation_path(),
        )?;
        if let Some(path) = &self.failed_path {
            write!(formatter, " while reading object operation path {path:?}")?;
        }
        write!(formatter, ": {}", self.cause.label())
    }
}

impl<E> fmt::Debug for ProjectAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectAcquisitionError")
            .field("binding", self.source_location.binding())
            .field("role", &"prefix")
            .field("operation_path", &self.source_location.operation_path())
            .field("failed_path", &self.failed_path)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for ProjectAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            ProjectAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            ProjectAcquisitionErrorCause::UnsupportedCapabilities { .. } => None,
            ProjectAcquisitionErrorCause::List(source)
            | ProjectAcquisitionErrorCause::Read(source)
            | ProjectAcquisitionErrorCause::ListedObjectAbsent(source) => Some(source),
            ProjectAcquisitionErrorCause::Structural(source) => Some(source),
            ProjectAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

/// The typed cause of an OpenDAL Project Acquisition failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProjectAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    List(::opendal::Error),
    Read(::opendal::Error),
    ListedObjectAbsent(::opendal::Error),
    Structural(ProjectAcquisitionSurveyError),
    Limit(ProjectAcquisitionLimitError),
}

impl<E> ProjectAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedCapabilities { .. } => {
                "required listing or read capability is unsupported"
            }
            Self::List(_) => "the recursive listing failed",
            Self::Read(_) => "a listed object read failed",
            Self::ListedObjectAbsent(_) => "a listed object was absent when read",
            Self::Structural(_) => "the completed listing had structural issues",
            Self::Limit(_) => "a Project Acquisition limit failed",
        }
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
