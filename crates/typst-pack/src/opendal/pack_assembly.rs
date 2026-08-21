//! OpenDAL read for Pack Assembly inputs.
//!
//! # Complete Pack Assembly
//!
//! The storage operations are caller-polled async operations. Their completed,
//! owned values compose through the existing synchronous Pack Creation loop:
//!
//! ```no_run
//! # #[cfg(feature = "package-reading")]
//! # mod complete {
//! use std::collections::HashSet;
//!
//! use typst::foundations::Dict;
//! use typst_pack::opendal::{Location, OperatorBindings};
//! use typst_pack::opendal::pack_assembly::{
//!     FontReadEntry, FontReadLimits, FontReadRequest, FontSource,
//!     PackageReadLimits, PackageReadRequest, PackageTreeSource,
//!     ProjectReadEntry, ProjectReadLimits, ProjectReadRequest,
//!     read_fonts, read_package, read_project, insert_read_package,
//! };
//! use typst_pack::opendal::write::{
//!     PackageCacheArchiveWriteRequest, write_package_cache_archive,
//! };
//! use typst_pack::{
//!     DiscoverySpecification, DocumentTime, FontCatalog, FontCatalogEntry, FontContainer,
//!     FontDisposition, Pack, PackCreationInput, PackCreationOutcome,
//!     PackageReadFailures, PackageCatalog, PackageDisposition,
//!     PackageExpansionLimits, ProjectSnapshotAssembly, TypstTarget, create,
//! };
//!
//! async fn assemble(bindings: &OperatorBindings) -> Result<Pack, Box<dyn std::error::Error>> {
//!     let project_request = ProjectReadRequest::new(
//!         "project:/sources/document/".parse::<Location>()?,
//!         ProjectReadLimits::reference_v1(),
//!     )?;
//!     let (_, project_entries) = read_project(bindings, &project_request).await?.into_parts();
//!     let project = ProjectSnapshotAssembly::new("main.typ").assemble(
//!         project_entries.into_iter().map(ProjectReadEntry::into_parts),
//!     )?;
//!
//!     let font_request = FontReadRequest::new(
//!         [FontSource::new(
//!             "fonts:/catalog/".parse::<Location>()?,
//!             FontDisposition::Embedded,
//!         )],
//!         FontReadLimits::reference_v1(),
//!     )?;
//!     let (_, font_entries) = read_fonts(bindings, &font_request).await?.into_parts();
//!     let mut fonts = FontCatalog::new();
//!     for entry in font_entries {
//!         let (_, _, _, disposition, bytes) = FontReadEntry::into_parts(entry);
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
//!     let mut failures = PackageReadFailures::new();
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
//!                     let request = PackageReadRequest::new(
//!                         spec,
//!                         [tree_source.clone()],
//!                         Some(archive_cache.clone()),
//!                         Some(registry.clone()),
//!                         PackageReadLimits::reference_v1(),
//!                     )?;
//!                     let read = read_package(bindings, &request).await?;
//!                     let insertion = insert_read_package(
//!                         &mut packages,
//!                         &mut failures,
//!                         read,
//!                         PackageDisposition::Embedded,
//!                         PackageExpansionLimits::reference_v1(),
//!                     );
//!                     match insertion {
//!                         Ok(Some(residue)) => {
//!                             let write = PackageCacheArchiveWriteRequest::new(
//!                                 residue.destination().clone(),
//!                             )?;
//!                             let _cache_result = write_package_cache_archive(
//!                                 bindings,
//!                                 &write,
//!                                 residue.bytes(),
//!                             ).await;
//!                             // Cache failure is separate evidence and does not
//!                             // invalidate the inserted Package Tree.
//!                         }
//!                         Ok(None) => {}
//!                         // Insertion retained the mapped Package Read Failure.
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

use super::read::recursive::{
    RecursiveReadLimits, RecursiveReadOperation, RecursiveReadResource, RecursiveReadSelection,
    RecursiveSurveyIssue, RecursiveSurveyIssueKind, read_recursive_prefix, read_recursive_prefixes,
};
use super::{BoxError, Location, LocationRoleError, OperatorResolver};
use crate::FontDisposition;
use crate::limits::{LimitError, Limits, ResourceKind};
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

/// Named finite ceilings for one OpenDAL Project Read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectReadCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_files: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

impl ProjectReadCeilings {
    /// The first-party version-1 Project Read profile.
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

/// A resource bounded during OpenDAL Project Read.
pub type ProjectReadResource = ResourceKind<9>;

#[allow(non_upper_case_globals)]
impl ResourceKind<9> {
    pub const ListedEntries: Self = Self::new(0);
    pub const ListedPathBytes: Self = Self::new(1);
    pub const TotalListedPathBytes: Self = Self::new(2);
    pub const SelectedFiles: Self = Self::new(3);
    pub const ObjectBytes: Self = Self::new(4);
    pub const TotalBytes: Self = Self::new(5);
}

/// Mandatory finite limits for OpenDAL Project Read.
pub type ProjectReadLimits = Limits<ProjectReadResource>;

impl Limits<ProjectReadResource> {
    /// Validates all named read ceilings.
    #[track_caller]
    pub fn new(ceilings: ProjectReadCeilings) -> Self {
        let limits = Self::from_ceilings([
            ceilings.listed_entries,
            ceilings.listed_path_bytes,
            ceilings.total_listed_path_bytes,
            ceilings.selected_files,
            ceilings.object_bytes,
            ceilings.total_bytes,
            0,
        ])
        .assert_probe_resources([
            ProjectReadResource::ListedEntries,
            ProjectReadResource::ListedPathBytes,
            ProjectReadResource::TotalListedPathBytes,
            ProjectReadResource::SelectedFiles,
            ProjectReadResource::ObjectBytes,
            ProjectReadResource::TotalBytes,
        ]);
        assert!(
            ceilings.object_bytes <= ceilings.total_bytes,
            "the ObjectBytes ceiling {} exceeds the TotalBytes ceiling {}",
            ceilings.object_bytes,
            ceilings.total_bytes
        );
        limits
    }

    /// The validated first-party version-1 Project Read limits.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            1_000_000,
            64 * 1024,
            256 * 1024 * 1024,
            100_000,
            256 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            0,
        ])
    }

    pub const fn listed_entries(&self) -> u64 {
        self.ceilings[0]
    }

    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings[1]
    }

    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    pub const fn selected_files(&self) -> u64 {
        self.ceilings[3]
    }

    pub const fn object_bytes(&self) -> u64 {
        self.ceilings[4]
    }

    pub const fn total_bytes(&self) -> u64 {
        self.ceilings[5]
    }
}

/// Project Read exceeded or could not account for a mandatory limit.
pub type ProjectReadLimitError = LimitError<ProjectReadResource>;

/// A validated request to read every yielded file below one prefix.
#[derive(Clone, Debug)]
pub struct ProjectReadRequest {
    source: Location,
    limits: ProjectReadLimits,
}

impl ProjectReadRequest {
    /// Validates a prefix source and retains its mandatory limits.
    pub fn new(
        source: Location,
        limits: ProjectReadLimits,
    ) -> Result<Self, ProjectReadRequestError> {
        if let Err(role_error) = source.require_prefix() {
            return Err(ProjectReadRequestError::InvalidSourceRole {
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

    /// The mandatory finite Project Read limits.
    pub const fn limits(&self) -> ProjectReadLimits {
        self.limits
    }
}

/// A reason a Project Read request is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectReadRequestError {
    #[error("project source {location} is not a prefix: {source}")]
    InvalidSourceRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// One exact path-and-byte entry read below a project prefix.
pub struct ProjectReadEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl ProjectReadEntry {
    /// The operation path relative to the requested prefix.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The exact bytes observed by the completed object read.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The read byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether this read object was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the owned path and exact bytes for Project Snapshot assembly.
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for ProjectReadEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectReadEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact entries read from one project prefix.
pub struct ProjectRead {
    source: Location,
    entries: Vec<ProjectReadEntry>,
}

impl ProjectRead {
    /// The normalized prefix from which entries were read.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// Read entries in relative operation-path order.
    pub fn entries(&self) -> &[ProjectReadEntry] {
        &self.entries
    }

    /// Recovers the source and owned entries for Project Snapshot assembly.
    pub fn into_parts(self) -> (Location, Vec<ProjectReadEntry>) {
        (self.source, self.entries)
    }
}

impl fmt::Debug for ProjectRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectRead")
            .field("source", &self.source)
            .field("entries", &self.entries)
            .finish()
    }
}

/// An unsupported yielded OpenDAL entry kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProjectReadEntryKind {
    Unknown,
}

/// One structural issue found while surveying a project prefix.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectReadIssue {
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
        kind: ProjectReadEntryKind,
    },
}

/// The nonempty canonical set of structural project survey issues.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "project survey failed")
)]
pub struct ProjectReadSurveyError {
    issues: Vec<ProjectReadIssue>,
}

impl ProjectReadSurveyError {
    /// Every independently detectable issue in canonical order.
    pub fn issues(&self) -> &[ProjectReadIssue] {
        &self.issues
    }
}

/// Reads every file entry yielded below one project prefix.
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
///     PackageReadFailures, PackageCatalog, ProjectSnapshotAssembly,
///     TypstTarget, create,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     ProjectReadEntry, ProjectReadRequest, read_project,
/// };
///
/// async fn read_and_create(
///     bindings: &OperatorBindings,
///     request: &ProjectReadRequest,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let (_, entries) = read_project(bindings, request).await?.into_parts();
///     let project = ProjectSnapshotAssembly::new("main.typ").assemble(
///         entries.into_iter().map(ProjectReadEntry::into_parts),
///     )?;
///     let packages = PackageCatalog::new();
///     let fonts = FontCatalog::new();
///     let package_failures = PackageReadFailures::new();
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
pub async fn read_project<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &ProjectReadRequest,
) -> Result<ProjectRead, ProjectReadError> {
    let source = request.source().clone();
    let entries = read_recursive_prefix(
        resolver,
        request.source(),
        RecursiveReadSelection::AllFiles,
        request.limits().into(),
        &ProjectReadOperation {
            source_location: request.source(),
        },
    )
    .await?
    .into_iter()
    .map(|object| ProjectReadEntry {
        relative_path: object.relative_path,
        bytes: object.bytes,
    })
    .collect();

    Ok(ProjectRead { source, entries })
}

/// A failure while reading a project through OpenDAL.
///
/// This error's own `Display` and `Debug` omit native resolver and OpenDAL
/// messages. Rendering its complete source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Project Read failed for binding {binding} at prefix operation path {operation_path:?}{failed_path}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
    failed_path = failed_path_context(.failed_path.as_deref()),
)]
pub struct ProjectReadError {
    source_location: Location,
    failed_path: Option<String>,
    #[source]
    cause: RedactedError<ProjectReadErrorCause>,
}

impl ProjectReadError {
    /// The normalized project prefix whose read failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The selected object's operation path when one object read failed.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The typed cause of this failure.
    pub fn cause(&self) -> &ProjectReadErrorCause {
        self.cause.inner()
    }

    fn new(
        source_location: &Location,
        failed_path: Option<String>,
        cause: ProjectReadErrorCause,
    ) -> Self {
        Self {
            source_location: source_location.clone(),
            failed_path,
            cause: RedactedError::new(cause),
        }
    }
}

/// The typed cause of an OpenDAL Project Read failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectReadErrorCause {
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
    Structural(#[source] ProjectReadSurveyError),
    #[error("a Project Read limit failed")]
    Limit(#[source] ProjectReadLimitError),
}

struct ProjectReadOperation<'a> {
    source_location: &'a Location,
}

impl RecursiveReadOperation for ProjectReadOperation<'_> {
    type Error = ProjectReadError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> ProjectReadError {
        unreachable!("ProjectReadRequest validates the prefix role")
    }

    fn resolve_operator(&self, _: usize, source: BoxError) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        _: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, _: usize, source: ::opendal::Error) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::List(source),
        )
    }

    fn read(&self, _: usize, operation_path: String, source: ::opendal::Error) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            Some(operation_path),
            ProjectReadErrorCause::Read(source),
        )
    }

    fn listed_object_absent(
        &self,
        _: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            Some(operation_path),
            ProjectReadErrorCause::ListedObjectAbsent(source),
        )
    }

    fn structural(&self, _: usize, issues: Vec<RecursiveSurveyIssue>) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::Structural(ProjectReadSurveyError {
                issues: issues.into_iter().map(map_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        _: usize,
        resource: RecursiveReadResource,
        ceiling: u64,
        _: u64,
    ) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::Limit(ProjectReadLimitError::exceeded(
                map_resource(resource),
                ceiling,
            )),
        )
    }

    fn accounting_overflow(&self, _: usize, resource: RecursiveReadResource) -> ProjectReadError {
        ProjectReadError::new(
            self.source_location,
            None,
            ProjectReadErrorCause::Limit(ProjectReadLimitError::AccountingOverflow {
                resource: map_resource(resource),
            }),
        )
    }
}

impl From<ProjectReadLimits> for RecursiveReadLimits {
    fn from(limits: ProjectReadLimits) -> Self {
        Self::new(
            limits.listed_entries(),
            limits.listed_path_bytes(),
            limits.total_listed_path_bytes(),
            limits.selected_files(),
            limits.object_bytes(),
            limits.total_bytes(),
        )
    }
}

fn map_resource(resource: RecursiveReadResource) -> ProjectReadResource {
    match resource {
        RecursiveReadResource::ListedEntries => ProjectReadResource::ListedEntries,
        RecursiveReadResource::ListedPathBytes => ProjectReadResource::ListedPathBytes,
        RecursiveReadResource::TotalListedPathBytes => ProjectReadResource::TotalListedPathBytes,
        RecursiveReadResource::SelectedObjects => ProjectReadResource::SelectedFiles,
        RecursiveReadResource::ObjectBytes => ProjectReadResource::ObjectBytes,
        RecursiveReadResource::TotalBytes => ProjectReadResource::TotalBytes,
        _ => unreachable!("unknown recursive read resource"),
    }
}

fn map_issue(issue: RecursiveSurveyIssue) -> ProjectReadIssue {
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            ProjectReadIssue::ListedPathOutsidePrefix { operation_path }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            ProjectReadIssue::PrefixMarkerWhereFileRequired { operation_path }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            ProjectReadIssue::EmptyRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath => {
            ProjectReadIssue::InvalidRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::DuplicateListedObject => {
            ProjectReadIssue::DuplicateListedObject { operation_path }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => ProjectReadIssue::UnsupportedEntryKind {
            operation_path,
            kind: ProjectReadEntryKind::Unknown,
        },
    }
}

/// Named finite ceilings for one OpenDAL Font Read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontReadCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_containers: u64,
    pub container_bytes: u64,
    pub total_bytes: u64,
}

impl FontReadCeilings {
    /// The first-party version-1 Font Read profile.
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

/// A resource bounded across one OpenDAL Font Read.
pub type FontReadResource = ResourceKind<10>;

#[allow(non_upper_case_globals)]
impl ResourceKind<10> {
    pub const ListedEntries: Self = Self::new(0);
    pub const ListedPathBytes: Self = Self::new(1);
    pub const TotalListedPathBytes: Self = Self::new(2);
    pub const SelectedContainers: Self = Self::new(3);
    pub const ContainerBytes: Self = Self::new(4);
    pub const TotalBytes: Self = Self::new(5);
}

/// Mandatory finite limits for OpenDAL Font Read.
pub type FontReadLimits = Limits<FontReadResource>;

impl Limits<FontReadResource> {
    /// Validates all named read ceilings.
    #[track_caller]
    pub fn new(ceilings: FontReadCeilings) -> Self {
        let limits = Self::from_ceilings([
            ceilings.listed_entries,
            ceilings.listed_path_bytes,
            ceilings.total_listed_path_bytes,
            ceilings.selected_containers,
            ceilings.container_bytes,
            ceilings.total_bytes,
            0,
        ])
        .assert_probe_resources([
            FontReadResource::ListedEntries,
            FontReadResource::ListedPathBytes,
            FontReadResource::TotalListedPathBytes,
            FontReadResource::SelectedContainers,
            FontReadResource::ContainerBytes,
            FontReadResource::TotalBytes,
        ]);
        assert!(
            ceilings.container_bytes <= ceilings.total_bytes,
            "the ContainerBytes ceiling {} exceeds the TotalBytes ceiling {}",
            ceilings.container_bytes,
            ceilings.total_bytes
        );
        limits
    }

    /// The validated first-party version-1 Font Read limits.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            100_000,
            64 * 1024,
            64 * 1024 * 1024,
            16_384,
            256 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            0,
        ])
    }

    pub const fn listed_entries(&self) -> u64 {
        self.ceilings[0]
    }

    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings[1]
    }

    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    pub const fn selected_containers(&self) -> u64 {
        self.ceilings[3]
    }

    pub const fn container_bytes(&self) -> u64 {
        self.ceilings[4]
    }

    pub const fn total_bytes(&self) -> u64 {
        self.ceilings[5]
    }
}

/// Font Read exceeded or could not account for a mandatory limit.
pub type FontReadLimitError = LimitError<FontReadResource>;

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

/// A validated request to read caller-ordered OpenDAL font prefixes.
#[derive(Clone, Debug)]
pub struct FontReadRequest {
    sources: Vec<FontSource>,
    limits: FontReadLimits,
}

impl FontReadRequest {
    /// Validates every source role before accepting the request.
    pub fn new(
        sources: impl IntoIterator<Item = FontSource>,
        limits: FontReadLimits,
    ) -> Result<Self, FontReadRequestRejection> {
        let sources = sources.into_iter().collect::<Vec<_>>();
        let issues = sources
            .iter()
            .enumerate()
            .filter_map(|(source_index, configured)| {
                configured.source.require_prefix().err().map(|source| {
                    FontReadRequestIssue::InvalidSourceRole {
                        source_index,
                        location: configured.source.clone(),
                        source,
                    }
                })
            })
            .collect::<Vec<_>>();
        if !issues.is_empty() {
            return Err(FontReadRequestRejection { issues });
        }
        Ok(Self { sources, limits })
    }

    /// Font sources in caller order.
    pub fn sources(&self) -> &[FontSource] {
        &self.sources
    }

    /// The mandatory finite limits shared across every configured source.
    pub const fn limits(&self) -> FontReadLimits {
        self.limits
    }
}

/// Every invalid source role in a rejected Font Read request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "Font Read request rejected")
)]
pub struct FontReadRequestRejection {
    issues: Vec<FontReadRequestIssue>,
}

impl FontReadRequestRejection {
    /// Invalid source roles in caller source order.
    pub fn issues(&self) -> &[FontReadRequestIssue] {
        &self.issues
    }
}

/// One invalid source role in a Font Read request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontReadRequestIssue {
    #[error("font source {source_index} at {location} is not a prefix: {source}")]
    InvalidSourceRole {
        source_index: usize,
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// One exact Font Container selected and read from a configured source.
pub struct FontReadEntry {
    source_index: usize,
    source: Location,
    relative_path: String,
    disposition: FontDisposition,
    bytes: Vec<u8>,
}

impl FontReadEntry {
    /// The configured source's caller-order index.
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    /// The normalized prefix from which this entry was read.
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

    /// The read byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether this read container is empty.
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

impl fmt::Debug for FontReadEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontReadEntry")
            .field("source_index", &self.source_index)
            .field("source", &self.source)
            .field("relative_path", &self.relative_path)
            .field("disposition", &self.disposition)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact Font Containers read from caller-ordered sources.
pub struct FontRead {
    sources: Vec<FontSource>,
    entries: Vec<FontReadEntry>,
}

impl FontRead {
    /// Configured font sources in caller order.
    pub fn sources(&self) -> &[FontSource] {
        &self.sources
    }

    /// Read entries in source order, then relative operation-path order.
    pub fn entries(&self) -> &[FontReadEntry] {
        &self.entries
    }

    /// Recovers the configured sources and exact read entries.
    pub fn into_parts(self) -> (Vec<FontSource>, Vec<FontReadEntry>) {
        (self.sources, self.entries)
    }
}

impl fmt::Debug for FontRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontRead")
            .field("sources", &self.sources)
            .field("entries", &self.entries)
            .finish()
    }
}

/// An unsupported yielded OpenDAL entry kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FontReadEntryKind {
    Unknown,
}

/// One structural issue found while surveying configured font prefixes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FontReadIssue {
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
        kind: FontReadEntryKind,
    },
}

/// The nonempty canonical set of structural font survey issues.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{message}",
    message = aggregate_issue_message(.issues.as_slice(), "font survey failed")
)]
pub struct FontReadSurveyError {
    issues: Vec<FontReadIssue>,
}

impl FontReadSurveyError {
    /// Every independently detectable issue in source and path order.
    pub fn issues(&self) -> &[FontReadIssue] {
        &self.issues
    }
}

/// Reads suffix-selected Font Containers from caller-ordered prefixes.
///
/// `.ttf`, `.ttc`, `.otf`, and `.otc` suffixes are matched
/// case-insensitively. Directory markers and non-font entries are ignored. All
/// selected entries come only from completed listing observations; those
/// observations make no storage snapshot or coexistence claim.
///
/// ```no_run
/// use typst_pack::{
///     DiscoverySpecification, FontCatalog, FontCatalogEntry, FontContainer,
///     PackCreationInput, PackageReadFailures, PackageCatalog,
///     ProjectSnapshot, create,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     FontReadRequest, read_fonts,
/// };
///
/// async fn read_fonts_and_create(
///     bindings: &OperatorBindings,
///     request: &FontReadRequest,
///     project: &ProjectSnapshot,
///     packages: &PackageCatalog,
///     package_failures: &PackageReadFailures,
///     discovery: &DiscoverySpecification,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let (_, read) = read_fonts(bindings, request).await?.into_parts();
///     let mut fonts = FontCatalog::new();
///     for entry in read {
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
pub async fn read_fonts<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &FontReadRequest,
) -> Result<FontRead, FontReadError> {
    let locations = request
        .sources()
        .iter()
        .map(FontSource::source)
        .collect::<Vec<_>>();
    let read = read_recursive_prefixes(
        resolver,
        &locations,
        RecursiveReadSelection::FontContainers,
        request.limits().into(),
        &FontReadOperation {
            sources: request.sources(),
        },
    )
    .await?;

    let sources = request.sources().to_vec();
    let entries = read
        .into_iter()
        .enumerate()
        .flat_map(|(source_index, objects)| {
            let source = sources[source_index].clone();
            objects.into_iter().map(move |object| FontReadEntry {
                source_index,
                source: source.source.clone(),
                relative_path: object.relative_path,
                disposition: source.disposition,
                bytes: object.bytes,
            })
        })
        .collect();

    Ok(FontRead { sources, entries })
}

/// A failure while reading Font Containers through OpenDAL.
///
/// This error's own `Display` and `Debug` omit native resolver and OpenDAL
/// messages. Rendering its complete source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Font Read failed at source {source_index} for binding {binding} at prefix operation path {operation_path:?}{failed_path}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
    failed_path = failed_path_context(.failed_path.as_deref()),
)]
pub struct FontReadError {
    source_index: usize,
    source_location: Location,
    failed_path: Option<String>,
    #[source]
    cause: RedactedError<FontReadErrorCause>,
}

impl FontReadError {
    /// The caller-order index of the source at which read failed.
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    /// The normalized font prefix at which read failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The selected object's operation path when one object read failed.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The typed cause of this failure.
    pub fn cause(&self) -> &FontReadErrorCause {
        self.cause.inner()
    }

    fn new(
        source_index: usize,
        source_location: &Location,
        failed_path: Option<String>,
        cause: FontReadErrorCause,
    ) -> Self {
        Self {
            source_index,
            source_location: source_location.clone(),
            failed_path,
            cause: RedactedError::new(cause),
        }
    }
}

/// The typed cause of an OpenDAL Font Read failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FontReadErrorCause {
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
    Structural(#[source] FontReadSurveyError),
    #[error("a Font Read limit failed")]
    Limit(#[source] FontReadLimitError),
}

struct FontReadOperation<'a> {
    sources: &'a [FontSource],
}

impl FontReadOperation<'_> {
    fn error(
        &self,
        source_index: usize,
        failed_path: Option<String>,
        cause: FontReadErrorCause,
    ) -> FontReadError {
        FontReadError::new(
            source_index,
            self.sources[source_index].source(),
            failed_path,
            cause,
        )
    }
}

impl RecursiveReadOperation for FontReadOperation<'_> {
    type Error = FontReadError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> FontReadError {
        unreachable!("FontReadRequest validates every prefix role")
    }

    fn resolve_operator(&self, source_index: usize, source: BoxError) -> FontReadError {
        self.error(
            source_index,
            None,
            FontReadErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        source_index: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> FontReadError {
        self.error(
            source_index,
            None,
            FontReadErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, source_index: usize, source: ::opendal::Error) -> FontReadError {
        self.error(source_index, None, FontReadErrorCause::List(source))
    }

    fn read(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> FontReadError {
        self.error(
            source_index,
            Some(operation_path),
            FontReadErrorCause::Read(source),
        )
    }

    fn listed_object_absent(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> FontReadError {
        self.error(
            source_index,
            Some(operation_path),
            FontReadErrorCause::ListedObjectAbsent(source),
        )
    }

    fn structural(&self, source_index: usize, issues: Vec<RecursiveSurveyIssue>) -> FontReadError {
        self.error(
            source_index,
            None,
            FontReadErrorCause::Structural(FontReadSurveyError {
                issues: issues.into_iter().map(map_font_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
        ceiling: u64,
        _: u64,
    ) -> FontReadError {
        self.error(
            source_index,
            None,
            FontReadErrorCause::Limit(FontReadLimitError::exceeded(
                map_font_resource(resource),
                ceiling,
            )),
        )
    }

    fn accounting_overflow(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
    ) -> FontReadError {
        self.error(
            source_index,
            None,
            FontReadErrorCause::Limit(FontReadLimitError::AccountingOverflow {
                resource: map_font_resource(resource),
            }),
        )
    }
}

impl From<FontReadLimits> for RecursiveReadLimits {
    fn from(limits: FontReadLimits) -> Self {
        Self::new(
            limits.listed_entries(),
            limits.listed_path_bytes(),
            limits.total_listed_path_bytes(),
            limits.selected_containers(),
            limits.container_bytes(),
            limits.total_bytes(),
        )
    }
}

fn map_font_resource(resource: RecursiveReadResource) -> FontReadResource {
    match resource {
        RecursiveReadResource::ListedEntries => FontReadResource::ListedEntries,
        RecursiveReadResource::ListedPathBytes => FontReadResource::ListedPathBytes,
        RecursiveReadResource::TotalListedPathBytes => FontReadResource::TotalListedPathBytes,
        RecursiveReadResource::SelectedObjects => FontReadResource::SelectedContainers,
        RecursiveReadResource::ObjectBytes => FontReadResource::ContainerBytes,
        RecursiveReadResource::TotalBytes => FontReadResource::TotalBytes,
        _ => unreachable!("unknown recursive read resource"),
    }
}

fn map_font_issue(issue: RecursiveSurveyIssue) -> FontReadIssue {
    let source_index = issue.source_index;
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            FontReadIssue::ListedPathOutsidePrefix {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            FontReadIssue::PrefixMarkerWhereFileRequired {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            FontReadIssue::EmptyRelativeOperationPath {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath => {
            FontReadIssue::InvalidRelativeOperationPath {
                source_index,
                operation_path,
            }
        }
        RecursiveSurveyIssueKind::DuplicateListedObject => FontReadIssue::DuplicateListedObject {
            source_index,
            operation_path,
        },
        RecursiveSurveyIssueKind::UnsupportedEntryKind => FontReadIssue::UnsupportedEntryKind {
            source_index,
            operation_path,
            kind: FontReadEntryKind::Unknown,
        },
    }
}
