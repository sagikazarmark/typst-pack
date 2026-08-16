use std::{error::Error, fmt};

use typst::syntax::package::PackageSpec;

use super::super::acquisition::recursive::{
    RecursiveAcquisitionError, RecursiveAcquisitionLimits, RecursiveAcquisitionResource,
    RecursiveAcquisitionSelection, RecursiveSourcesAcquisitionError, RecursiveSurveyIssue,
    RecursiveSurveyIssueKind, acquire_first_present_recursive_prefix,
};
use super::super::{Location, LocationRoleError, OperatorResolver};
use crate::acquisition_layout;
use crate::package_catalog::PackageTreeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackageTreeAcquisitionCeilings {
    pub(crate) listed_entries: u64,
    pub(crate) listed_path_bytes: u64,
    pub(crate) total_listed_path_bytes: u64,
    pub(crate) selected_files: u64,
    pub(crate) object_bytes: u64,
    pub(crate) total_bytes: u64,
}

impl PackageTreeAcquisitionCeilings {
    pub(crate) const fn reference_v1() -> Self {
        Self {
            listed_entries: 100_000,
            listed_path_bytes: 64 * 1024,
            total_listed_path_bytes: 64 * 1024 * 1024,
            selected_files: 50_000,
            object_bytes: 64 * 1024 * 1024,
            total_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PackageTreeAcquisitionResource {
    ListedEntries,
    ListedPathBytes,
    TotalListedPathBytes,
    SelectedFiles,
    ObjectBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackageTreeAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: PackageTreeAcquisitionResource,
        ceiling: u64,
    },
    #[error("the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}")]
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackageTreeAcquisitionLimits {
    ceilings: PackageTreeAcquisitionCeilings,
}

impl PackageTreeAcquisitionLimits {
    pub(crate) fn new(
        ceilings: PackageTreeAcquisitionCeilings,
    ) -> Result<Self, PackageTreeAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                PackageTreeAcquisitionResource::ObjectBytes,
                ceilings.object_bytes,
            ),
            (
                PackageTreeAcquisitionResource::TotalBytes,
                ceilings.total_bytes,
            ),
        ] {
            if ceiling == u64::MAX {
                return Err(PackageTreeAcquisitionLimitsError::CannotProbe { resource, ceiling });
            }
        }
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(
                PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: ceilings.object_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }
        Ok(Self { ceilings })
    }

    pub(crate) const fn reference_v1() -> Self {
        Self {
            ceilings: PackageTreeAcquisitionCeilings::reference_v1(),
        }
    }

    pub(crate) const fn listed_entries(&self) -> u64 {
        self.ceilings.listed_entries
    }

    pub(crate) const fn listed_path_bytes(&self) -> u64 {
        self.ceilings.listed_path_bytes
    }

    pub(crate) const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings.total_listed_path_bytes
    }

    pub(crate) const fn selected_files(&self) -> u64 {
        self.ceilings.selected_files
    }

    pub(crate) const fn object_bytes(&self) -> u64 {
        self.ceilings.object_bytes
    }

    pub(crate) const fn total_bytes(&self) -> u64 {
        self.ceilings.total_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackageTreeAcquisitionLimitError {
    #[error(
        "OpenDAL Package Tree Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: PackageTreeAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Package Tree Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: PackageTreeAcquisitionResource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackageTreeSource {
    source: Location,
}

impl PackageTreeSource {
    pub(crate) fn new(source: Location) -> Self {
        Self { source }
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }
}

pub(crate) struct PackageTreeAcquisitionEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl PackageTreeAcquisitionEntry {
    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(crate) fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for PackageTreeAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeAcquisitionEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

pub(crate) struct PackageTreeAcquisition {
    spec: PackageSpec,
    source_index: usize,
    configured_source: Location,
    candidate_location: Location,
    entries: Vec<PackageTreeAcquisitionEntry>,
}

impl PackageTreeAcquisition {
    pub(crate) fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub(crate) fn source_index(&self) -> usize {
        self.source_index
    }

    pub(crate) fn configured_source(&self) -> &Location {
        &self.configured_source
    }

    pub(crate) fn candidate_location(&self) -> &Location {
        &self.candidate_location
    }

    pub(crate) fn entries(&self) -> &[PackageTreeAcquisitionEntry] {
        &self.entries
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PackageSpec,
        usize,
        Location,
        Location,
        Vec<PackageTreeAcquisitionEntry>,
    ) {
        (
            self.spec,
            self.source_index,
            self.configured_source,
            self.candidate_location,
            self.entries,
        )
    }
}

impl fmt::Debug for PackageTreeAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeAcquisition")
            .field("spec", &self.spec)
            .field("source_index", &self.source_index)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("entries", &self.entries)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PackageTreeAcquisitionEntryKind {
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackageTreeAcquisitionIssue {
    #[error("listed operation path {operation_path:?} is outside the Package Tree prefix")]
    ListedPathOutsidePrefix { operation_path: String },
    #[error("listed operation path {operation_path:?} is a prefix marker where a file is required")]
    PrefixMarkerWhereFileRequired { operation_path: String },
    #[error("listed operation path {operation_path:?} has an empty relative path")]
    EmptyRelativeOperationPath { operation_path: String },
    #[error("listed operation path {operation_path:?} has unsupported kind {kind:?}")]
    UnsupportedEntryKind {
        operation_path: String,
        kind: PackageTreeAcquisitionEntryKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackageTreeAcquisitionSurveyError {
    issues: Vec<PackageTreeAcquisitionIssue>,
}

impl PackageTreeAcquisitionSurveyError {
    pub(crate) fn issues(&self) -> &[PackageTreeAcquisitionIssue] {
        &self.issues
    }
}

impl fmt::Display for PackageTreeAcquisitionSurveyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            issue.fmt(formatter)
        } else {
            write!(
                formatter,
                "Package Tree survey failed with {} issue(s)",
                self.issues.len()
            )
        }
    }
}

impl Error for PackageTreeAcquisitionSurveyError {}

pub(crate) struct PackageTreeSourceAcquisitionError<E> {
    source_index: usize,
    configured_source: Location,
    candidate_location: Option<Location>,
    failed_path: Option<String>,
    cause: PackageTreeSourceAcquisitionErrorCause<E>,
}

impl<E> PackageTreeSourceAcquisitionError<E> {
    pub(crate) fn source_index(&self) -> usize {
        self.source_index
    }

    pub(crate) fn configured_source(&self) -> &Location {
        &self.configured_source
    }

    pub(crate) fn candidate_location(&self) -> Option<&Location> {
        self.candidate_location.as_ref()
    }

    pub(crate) fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub(crate) fn cause(&self) -> &PackageTreeSourceAcquisitionErrorCause<E> {
        &self.cause
    }

    fn from_recursive(
        sources: &[PackageTreeSource],
        child: &str,
        error: RecursiveSourcesAcquisitionError<E>,
    ) -> Self {
        let source_index = error.source_index;
        let configured_source = sources[source_index].source.clone();
        let candidate_location = configured_source
            .require_prefix()
            .is_ok()
            .then(|| compose_candidate(&configured_source, child));
        let (failed_path, cause) = match error.source {
            RecursiveAcquisitionError::InvalidLocationRole(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::InvalidSourceRole(source),
            ),
            RecursiveAcquisitionError::ResolveOperator(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::ResolveOperator(source),
            ),
            RecursiveAcquisitionError::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::UnsupportedCapabilities {
                    list,
                    list_with_recursive,
                    read,
                },
            ),
            RecursiveAcquisitionError::List(source) => {
                (None, PackageTreeSourceAcquisitionErrorCause::List(source))
            }
            RecursiveAcquisitionError::Read {
                operation_path,
                source,
            } => (
                Some(operation_path),
                PackageTreeSourceAcquisitionErrorCause::Read(source),
            ),
            RecursiveAcquisitionError::ListedObjectAbsent {
                operation_path,
                source,
            } => (
                Some(operation_path),
                PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source),
            ),
            RecursiveAcquisitionError::Structural(issues) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Structural(
                    PackageTreeAcquisitionSurveyError {
                        issues: issues.into_iter().map(map_issue).collect(),
                    },
                ),
            ),
            RecursiveAcquisitionError::InvalidPackageTree(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source),
            ),
            RecursiveAcquisitionError::Limit {
                resource,
                ceiling,
                observed_at_least,
            } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::Exceeded {
                        resource: map_resource(resource),
                        ceiling,
                        observed_at_least,
                    },
                ),
            ),
            RecursiveAcquisitionError::AccountingOverflow { resource } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::AccountingOverflow {
                        resource: map_resource(resource),
                    },
                ),
            ),
        };
        Self {
            source_index,
            configured_source,
            candidate_location,
            failed_path,
            cause,
        }
    }
}

impl<E> fmt::Display for PackageTreeSourceAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Package Tree Acquisition failed at source {} for binding {} at configured operation path {:?}",
            self.source_index,
            self.configured_source.binding(),
            self.configured_source.operation_path(),
        )?;
        if let Some(candidate) = &self.candidate_location {
            write!(
                formatter,
                " using candidate prefix operation path {:?}",
                candidate.operation_path()
            )?;
        }
        if let Some(path) = &self.failed_path {
            write!(formatter, " while reading object operation path {path:?}")?;
        }
        write!(formatter, ": {}", self.cause.label())
    }
}

impl<E> fmt::Debug for PackageTreeSourceAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeSourceAcquisitionError")
            .field("source_index", &self.source_index)
            .field("binding", self.configured_source.binding())
            .field(
                "configured_operation_path",
                &self.configured_source.operation_path(),
            )
            .field(
                "candidate_prefix_operation_path",
                &self
                    .candidate_location
                    .as_ref()
                    .map(Location::operation_path),
            )
            .field("failed_path", &self.failed_path)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackageTreeSourceAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackageTreeSourceAcquisitionErrorCause::InvalidSourceRole(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::UnsupportedCapabilities { .. } => None,
            PackageTreeSourceAcquisitionErrorCause::List(source)
            | PackageTreeSourceAcquisitionErrorCause::Read(source)
            | PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::Structural(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PackageTreeSourceAcquisitionErrorCause<E> {
    InvalidSourceRole(LocationRoleError),
    ResolveOperator(E),
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    List(opendal::Error),
    Read(opendal::Error),
    ListedObjectAbsent(opendal::Error),
    Structural(PackageTreeAcquisitionSurveyError),
    InvalidPackageTree(PackageTreeError),
    Limit(PackageTreeAcquisitionLimitError),
}

impl<E> PackageTreeSourceAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::InvalidSourceRole(_) => "the configured source is not a prefix",
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedCapabilities { .. } => {
                "required listing or read capability is unsupported"
            }
            Self::List(_) => "the recursive listing failed",
            Self::Read(_) => "a listed Package Tree object read failed",
            Self::ListedObjectAbsent(_) => "a listed Package Tree object was absent when read",
            Self::Structural(_) => "the completed listing had structural issues",
            Self::InvalidPackageTree(_) => "the completed listing does not form a Package Tree",
            Self::Limit(_) => "a Package Tree Acquisition limit failed",
        }
    }
}

pub(crate) async fn acquire_package_tree_candidates<R: OperatorResolver + ?Sized>(
    resolver: &R,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeAcquisitionLimits,
) -> Result<Option<PackageTreeAcquisition>, PackageTreeSourceAcquisitionError<R::Error>> {
    let child = format!("{}/", acquisition_layout::package_tree_key(spec));
    let candidates = sources.iter().map(|source| {
        source.source.require_prefix()?;
        Ok(compose_candidate(&source.source, &child))
    });
    let Some((source_index, candidate_location, objects)) = acquire_first_present_recursive_prefix(
        resolver,
        candidates,
        RecursiveAcquisitionSelection::PackageTree,
        limits.into(),
    )
    .await
    .map_err(|error| PackageTreeSourceAcquisitionError::from_recursive(sources, &child, error))?
    else {
        return Ok(None);
    };

    Ok(Some(PackageTreeAcquisition {
        spec: spec.clone(),
        source_index,
        configured_source: sources[source_index].source.clone(),
        candidate_location,
        entries: objects
            .into_iter()
            .map(|object| PackageTreeAcquisitionEntry {
                relative_path: object.relative_path,
                bytes: object.bytes,
            })
            .collect(),
    }))
}

fn compose_candidate(source: &Location, child: &str) -> Location {
    source
        .compose(child)
        .expect("a package key composed below a canonical prefix remains canonical")
}

impl From<PackageTreeAcquisitionLimits> for RecursiveAcquisitionLimits {
    fn from(limits: PackageTreeAcquisitionLimits) -> Self {
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

fn map_resource(resource: RecursiveAcquisitionResource) -> PackageTreeAcquisitionResource {
    match resource {
        RecursiveAcquisitionResource::ListedEntries => {
            PackageTreeAcquisitionResource::ListedEntries
        }
        RecursiveAcquisitionResource::ListedPathBytes => {
            PackageTreeAcquisitionResource::ListedPathBytes
        }
        RecursiveAcquisitionResource::TotalListedPathBytes => {
            PackageTreeAcquisitionResource::TotalListedPathBytes
        }
        RecursiveAcquisitionResource::SelectedObjects => {
            PackageTreeAcquisitionResource::SelectedFiles
        }
        RecursiveAcquisitionResource::ObjectBytes => PackageTreeAcquisitionResource::ObjectBytes,
        RecursiveAcquisitionResource::TotalBytes => PackageTreeAcquisitionResource::TotalBytes,
    }
}

fn map_issue(issue: RecursiveSurveyIssue) -> PackageTreeAcquisitionIssue {
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            PackageTreeAcquisitionIssue::ListedPathOutsidePrefix { operation_path }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            PackageTreeAcquisitionIssue::PrefixMarkerWhereFileRequired { operation_path }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            PackageTreeAcquisitionIssue::EmptyRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            PackageTreeAcquisitionIssue::UnsupportedEntryKind {
                operation_path,
                kind: PackageTreeAcquisitionEntryKind::Unknown,
            }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath
        | RecursiveSurveyIssueKind::DuplicateListedObject => {
            unreachable!("Package Tree path issues are owned by core preflight")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;
    use typst::syntax::package::PackageSpec;

    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
        PendingPoint, ReadScript, ReadStep, ScriptedService,
    };
    use crate::opendal::{Location, OperatorBinding, OperatorBindings, OperatorResolver};
    use crate::{PackageTree, PackageTreeIssue};

    use super::{
        PackageTreeAcquisitionCeilings, PackageTreeAcquisitionLimitError,
        PackageTreeAcquisitionLimits, PackageTreeAcquisitionLimitsError,
        PackageTreeAcquisitionResource, PackageTreeSource, PackageTreeSourceAcquisitionErrorCause,
        acquire_package_tree_candidates,
    };

    #[test]
    fn empty_candidate_falls_through_and_present_candidate_stops_fallback() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new("first/preview/example/1.2.3/", 0, []).unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    2,
                    [ListStep::page([
                        ListEntry::file("second/preview/example/1.2.3/z.typ"),
                        ListEntry::file("second/preview/example/1.2.3/a.typ"),
                    ])],
                )
                .unwrap(),
            ],
            [
                ReadScript::new(
                    "second/preview/example/1.2.3/a.typ",
                    1,
                    [ReadStep::chunk(b"a")],
                )
                .unwrap(),
                ReadScript::new(
                    "second/preview/example/1.2.3/z.typ",
                    1,
                    [ReadStep::chunk(b"z")],
                )
                .unwrap(),
            ],
            16,
        );
        let binding = OperatorBinding::new("trees").unwrap();
        let resolver = CountingResolver::new(service.operator());
        let sources = [
            PackageTreeSource::new(
                Location::from_operation_path(binding.clone(), "first/").unwrap(),
            ),
            PackageTreeSource::new(Location::from_operation_path(binding, "second/").unwrap()),
            PackageTreeSource::new("unreached:/not-a-prefix".parse().unwrap()),
        ];

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &resolver,
            &"@preview/example:1.2.3".parse().unwrap(),
            &sources,
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();

        assert_eq!(acquisition.source_index(), 1);
        assert_eq!(acquisition.configured_source(), sources[1].source());
        assert_eq!(
            acquisition.candidate_location().operation_path(),
            "second/preview/example/1.2.3/"
        );
        assert_eq!(
            acquisition
                .entries()
                .iter()
                .map(|entry| (entry.relative_path(), entry.bytes()))
                .collect::<Vec<_>>(),
            [("a.typ", b"a".as_slice()), ("z.typ", b"z".as_slice())]
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            2
        );
        assert_eq!(resolver.calls(), 1);
    }

    #[test]
    fn named_limits_keep_the_finite_reference_profile_and_validate_payload_ceilings() {
        let reference = PackageTreeAcquisitionCeilings::reference_v1();
        assert_eq!(reference.listed_entries, 100_000);
        assert_eq!(reference.listed_path_bytes, 64 * 1024);
        assert_eq!(reference.total_listed_path_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.selected_files, 50_000);
        assert_eq!(reference.object_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.total_bytes, 512 * 1024 * 1024);

        let narrowed = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: u64::MAX,
            listed_path_bytes: u64::MAX,
            total_listed_path_bytes: u64::MAX,
            total_bytes: reference.object_bytes,
            ..reference
        })
        .unwrap();
        assert_eq!(narrowed.listed_entries(), u64::MAX);
        assert_eq!(narrowed.listed_path_bytes(), u64::MAX);
        assert_eq!(narrowed.total_listed_path_bytes(), u64::MAX);
        assert_eq!(narrowed.selected_files(), reference.selected_files);
        assert_eq!(narrowed.object_bytes(), reference.object_bytes);
        assert_eq!(narrowed.total_bytes(), reference.object_bytes);

        for (resource, ceilings) in [
            (
                PackageTreeAcquisitionResource::ObjectBytes,
                PackageTreeAcquisitionCeilings {
                    object_bytes: u64::MAX,
                    total_bytes: u64::MAX,
                    ..reference
                },
            ),
            (
                PackageTreeAcquisitionResource::TotalBytes,
                PackageTreeAcquisitionCeilings {
                    total_bytes: u64::MAX,
                    ..reference
                },
            ),
        ] {
            assert!(matches!(
                PackageTreeAcquisitionLimits::new(ceilings),
                Err(PackageTreeAcquisitionLimitsError::CannotProbe {
                    resource: actual,
                    ceiling: u64::MAX,
                }) if actual == resource
            ));
        }
        assert!(matches!(
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 2,
                total_bytes: 1,
                ..reference
            }),
            Err(
                PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: 2,
                    total_bytes: 1,
                }
            )
        ));
    }

    #[test]
    fn tree_resources_map_shared_survey_and_payload_boundaries() {
        let reference = PackageTreeAcquisitionCeilings::reference_v1();
        let survey_cases = [
            (
                PackageTreeAcquisitionResource::ListedEntries,
                PackageTreeAcquisitionCeilings {
                    listed_entries: 0,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeAcquisitionResource::ListedPathBytes,
                PackageTreeAcquisitionCeilings {
                    listed_path_bytes: 1,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeAcquisitionResource::TotalListedPathBytes,
                PackageTreeAcquisitionCeilings {
                    total_listed_path_bytes: 0,
                    ..reference
                },
                ListEntry::file("trees/preview/example/1.2.3/a.typ"),
            ),
            (
                PackageTreeAcquisitionResource::SelectedFiles,
                PackageTreeAcquisitionCeilings {
                    selected_files: 0,
                    ..reference
                },
                ListEntry::file("trees/preview/example/1.2.3/a.typ"),
            ),
        ];
        for (resource, ceilings, entry) in survey_cases {
            let service = ScriptedService::new(
                Capabilities::all(),
                [
                    ListScript::new("trees/preview/example/1.2.3/", 1, [ListStep::page([entry])])
                        .unwrap(),
                ],
                [],
                8,
            );
            let bindings = configured(&service);
            let error = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeAcquisitionLimits::new(ceilings).unwrap(),
            )))
            .unwrap_err();
            assert!(matches!(
                error.cause(),
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::Exceeded {
                        resource: actual,
                        ..
                    }
                ) if *actual == resource
            ));
        }

        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                "trees/preview/example/1.2.3/",
                1,
                [ListStep::page([ListEntry::file(
                    "trees/preview/example/1.2.3/a.typ",
                )])],
            )
            .unwrap()],
            [ReadScript::new(
                "trees/preview/example/1.2.3/a.typ",
                1,
                [ReadStep::chunk(b"four")],
            )
            .unwrap()],
            8,
        );
        let bindings = configured(&service);
        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 3,
                total_bytes: 8,
                ..reference
            })
            .unwrap(),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::ObjectBytes,
                    ceiling: 3,
                    observed_at_least: 4,
                }
            )
        ));

        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                "trees/preview/example/1.2.3/",
                2,
                [ListStep::page([
                    ListEntry::file("trees/preview/example/1.2.3/a.typ"),
                    ListEntry::file("trees/preview/example/1.2.3/b.typ"),
                ])],
            )
            .unwrap()],
            [
                ReadScript::new(
                    "trees/preview/example/1.2.3/a.typ",
                    1,
                    [ReadStep::chunk(b"12")],
                )
                .unwrap(),
                ReadScript::new(
                    "trees/preview/example/1.2.3/b.typ",
                    1,
                    [ReadStep::chunk(b"34")],
                )
                .unwrap(),
            ],
            12,
        );
        let bindings = configured(&service);
        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 3,
                total_bytes: 3,
                ..reference
            })
            .unwrap(),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::TotalBytes,
                    ceiling: 3,
                    observed_at_least: 4,
                }
            )
        ));
    }

    #[test]
    fn listing_limits_are_shared_across_absent_candidates() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(
                    "first/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "first/preview/example/1.2.3/dir/",
                    )])],
                )
                .unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "second/preview/example/1.2.3/long-directory/",
                    )])],
                )
                .unwrap(),
            ],
            [],
            8,
        );
        let bindings = configured(&service);
        let limits = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: 1,
            ..PackageTreeAcquisitionCeilings::reference_v1()
        })
        .unwrap();

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            limits,
        )))
        .unwrap_err();

        assert_eq!(error.source_index(), 1);
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::ListedEntries,
                    ceiling: 1,
                    observed_at_least: 2,
                }
            )
        ));
    }

    #[test]
    fn listing_permutations_preserve_canonical_order_and_exact_boundaries() {
        let candidate = "trees/preview/example/1.2.3/";
        let paths = [format!("{candidate}a"), format!("{candidate}b")];
        for entries in [
            [ListEntry::file(&paths[0]), ListEntry::file(&paths[1])],
            [ListEntry::file(&paths[1]), ListEntry::file(&paths[0])],
        ] {
            let service = ScriptedService::new(
                Capabilities::all(),
                [ListScript::new(candidate, 2, [ListStep::page(entries)]).unwrap()],
                [
                    ReadScript::new(&paths[0], 1, [ReadStep::chunk(b"a")]).unwrap(),
                    ReadScript::new(&paths[1], 1, [ReadStep::chunk(b"b")]).unwrap(),
                ],
                12,
            );
            let bindings = configured(&service);
            let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeAcquisitionLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();
            assert_eq!(
                acquisition
                    .entries()
                    .iter()
                    .map(|entry| entry.relative_path())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
        }

        let object = format!("{candidate}a");
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])])
                    .unwrap(),
            ],
            [ReadScript::new(&object, 1, [ReadStep::chunk(b"a")]).unwrap()],
            8,
        );
        let exact = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: 1,
            listed_path_bytes: 29,
            total_listed_path_bytes: 33,
            selected_files: 1,
            object_bytes: 1,
            total_bytes: 1,
        })
        .unwrap();
        let bindings = configured(&service);
        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            exact,
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.entries()[0].bytes(), b"a");
    }

    #[test]
    fn completed_empty_observations_exhaust_to_absence() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new("first/preview/example/1.2.3/", 0, []).unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "second/preview/example/1.2.3/empty/",
                    )])],
                )
                .unwrap(),
            ],
            [],
            8,
        );
        let bindings = configured(&service);

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap();

        assert!(acquisition.is_none());
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn core_preflight_canonicalizes_before_reads_and_owned_entries_build_the_final_tree() {
        let candidate = "trees/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                2,
                [ListStep::page([
                    ListEntry::file(format!("{candidate}./lib.typ")),
                    ListEntry::file(format!("{candidate}empty.typ")),
                ])],
            )
            .unwrap()],
            [
                ReadScript::new(
                    format!("{candidate}./lib.typ"),
                    1,
                    [ReadStep::chunk(b"library")],
                )
                .unwrap(),
                ReadScript::new(format!("{candidate}empty.typ"), 0, []).unwrap(),
            ],
            12,
        );
        let bindings = configured(&service);

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.spec(), &spec());
        assert_eq!(acquisition.entries()[0].relative_path(), "empty.typ");
        assert_eq!(acquisition.entries()[0].len(), 0);
        assert!(acquisition.entries()[0].is_empty());
        assert_eq!(acquisition.entries()[1].relative_path(), "lib.typ");

        let (actual_spec, index, configured, candidate, entries) = acquisition.into_parts();
        assert_eq!(actual_spec, spec());
        assert_eq!(index, 0);
        assert_eq!(configured.operation_path(), "trees/");
        assert_eq!(candidate.operation_path(), "trees/preview/example/1.2.3/");
        let tree = PackageTree::from_owned_entries(
            entries
                .into_iter()
                .map(super::PackageTreeAcquisitionEntry::into_parts),
        )
        .unwrap();
        assert_eq!(tree.file("empty.typ"), Some(b"".as_slice()));
        assert_eq!(tree.file("lib.typ"), Some(b"library".as_slice()));
    }

    #[test]
    fn core_package_tree_conflicts_are_typed_and_terminal_before_reads() {
        let first = "first/preview/example/1.2.3/";
        let second = "second/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(
                    first,
                    2,
                    [ListStep::page([
                        ListEntry::file(format!("{first}assets")),
                        ListEntry::file(format!("{first}assets/logo.svg")),
                    ])],
                )
                .unwrap(),
                ListScript::new(
                    second,
                    1,
                    [ListStep::page([ListEntry::file(format!(
                        "{second}unreached.typ"
                    ))])],
                )
                .unwrap(),
            ],
            [],
            12,
        );
        let bindings = configured(&service);

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();

        let PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source) = error.cause()
        else {
            panic!("unexpected cause: {:?}", error.cause());
        };
        assert_eq!(
            source.issues(),
            [PackageTreeIssue::PathTreeConflict {
                ancestor: "assets".to_owned(),
                descendant: "assets/logo.svg".to_owned(),
            }]
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn envelope_issues_are_aggregated_and_do_not_reach_lower_candidates() {
        let candidate = "trees/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                2,
                [ListStep::page([
                    ListEntry::unknown(format!("{candidate}unknown")),
                    ListEntry::file("outside/file.typ"),
                ])],
            )
            .unwrap()],
            [],
            8,
        );
        let bindings = configured(&service);

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        let PackageTreeSourceAcquisitionErrorCause::Structural(survey) = error.cause() else {
            panic!("unexpected cause: {:?}", error.cause());
        };
        assert_eq!(survey.issues().len(), 2);
        assert!(matches!(
            &survey.issues()[0],
            super::PackageTreeAcquisitionIssue::ListedPathOutsidePrefix { operation_path }
                if operation_path == "outside/file.typ"
        ));
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn mutation_is_observed_but_disappearance_and_list_not_found_are_terminal() {
        let candidate = "trees/preview/example/1.2.3/";
        let changing = format!("{candidate}changing.typ");
        let replacement =
            ReadScript::new(&changing, 1, [ReadStep::chunk(b"bytes after listing")]).unwrap();
        let mutation_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                1,
                [
                    ListStep::page([ListEntry::file(&changing)]),
                    ListStep::replace_read(replacement),
                ],
            )
            .unwrap()],
            [ReadScript::new(&changing, 1, [ReadStep::chunk(b"bytes during listing")]).unwrap()],
            8,
        );
        let mutation_bindings = configured(&mutation_service);
        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &mutation_bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.entries()[0].bytes(), b"bytes after listing");

        let absent_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                1,
                [ListStep::page([ListEntry::file(format!(
                    "{candidate}gone.typ"
                ))])],
            )
            .unwrap()],
            [],
            8,
        );
        let absent_bindings = configured(&absent_service);
        let absent = expect_ready(pin!(acquire_package_tree_candidates(
            &absent_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        assert_eq!(
            absent.failed_path(),
            Some("trees/preview/example/1.2.3/gone.typ")
        );
        assert!(matches!(
            absent.cause(),
            PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let list_failure_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(candidate, 0, [ListStep::failure(ErrorKind::NotFound)]).unwrap()],
            [],
            4,
        );
        let list_failure_bindings = configured(&list_failure_service);
        let list_failure = expect_ready(pin!(acquire_package_tree_candidates(
            &list_failure_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        assert!(matches!(
            list_failure.cause(),
            PackageTreeSourceAcquisitionErrorCause::List(source)
                if source.kind() == ErrorKind::NotFound
        ));
    }

    #[test]
    fn cancellation_drops_the_reached_operation_without_reaching_fallback() {
        let candidate = "trees/preview/example/1.2.3/";
        let list_pending = PendingPoint::new();
        let list_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(candidate, 0, [ListStep::pending(list_pending.clone())]).unwrap()],
            [],
            4,
        );
        let list_bindings = configured(&list_service);
        let sources = [source("trees/"), source("unreached/")];
        {
            let requested_spec = spec();
            let mut acquisition = pin!(acquire_package_tree_candidates(
                &list_bindings,
                &requested_spec,
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(list_pending.was_observed());
        }
        assert_eq!(
            list_service.cancellations(),
            [DroppedOperation::List {
                id: 0,
                path: candidate.to_owned(),
            }]
        );

        let read_pending = PendingPoint::new();
        let object = format!("{candidate}pending.typ");
        let read_service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])])
                    .unwrap(),
            ],
            [ReadScript::new(&object, 0, [ReadStep::pending(read_pending.clone())]).unwrap()],
            8,
        );
        let read_bindings = configured(&read_service);
        {
            let requested_spec = spec();
            let mut acquisition = pin!(acquire_package_tree_candidates(
                &read_bindings,
                &requested_spec,
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(read_pending.was_observed());
        }
        assert_eq!(
            read_service.cancellations(),
            [DroppedOperation::Read {
                id: 1,
                path: object,
            }]
        );
        assert_eq!(
            read_service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn memory_acquires_candidates_below_root_and_non_root_configured_prefixes() {
        for (configured, object, expected_candidate) in [
            (
                "",
                "preview/example/1.2.3/lib.typ",
                "preview/example/1.2.3/",
            ),
            (
                "packages/",
                "packages/preview/example/1.2.3/lib.typ",
                "packages/preview/example/1.2.3/",
            ),
        ] {
            let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
            expect_ready(pin!(operator.write(object, b"memory package".to_vec()))).unwrap();
            let binding = OperatorBinding::new("trees").unwrap();
            let bindings = OperatorBindings::new([(binding.clone(), operator)]).unwrap();
            let sources = [PackageTreeSource::new(
                Location::from_operation_path(binding, configured).unwrap(),
            )];

            let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();

            assert_eq!(
                acquisition.candidate_location().operation_path(),
                expected_candidate
            );
            assert_eq!(acquisition.entries()[0].relative_path(), "lib.typ");
            assert_eq!(acquisition.entries()[0].bytes(), b"memory package");
        }
    }

    fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    fn spec() -> PackageSpec {
        "@preview/example:1.2.3".parse().unwrap()
    }

    fn source(path: &str) -> PackageTreeSource {
        PackageTreeSource::new(
            Location::from_operation_path(OperatorBinding::new("trees").unwrap(), path).unwrap(),
        )
    }

    fn configured(service: &ScriptedService) -> OperatorBindings {
        OperatorBindings::new([(OperatorBinding::new("trees").unwrap(), service.operator())])
            .unwrap()
    }

    struct CountingResolver {
        calls: Cell<usize>,
        operator: opendal::Operator,
    }

    impl CountingResolver {
        fn new(operator: opendal::Operator) -> Self {
            Self {
                calls: Cell::new(0),
                operator,
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl OperatorResolver for CountingResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.operator.clone())
        }
    }
}
