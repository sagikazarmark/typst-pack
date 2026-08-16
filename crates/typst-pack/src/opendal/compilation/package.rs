use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use typst::syntax::package::PackageSpec;

use super::super::OperatorResolver;
use super::super::acquisition::recursive::{
    AcquisitionRole, RecursiveAcquisitionError, RecursiveAcquisitionLimits,
    RecursiveAcquisitionResource, RecursiveSourcesAcquisitionError, RecursiveSurveyIssue,
    RecursiveSurveyIssueKind, RequiredRecursiveSurvey,
    survey_required_recursive_prefixes_with_operators,
};
use super::super::acquisition::{
    ExactObjectLimitError, ExactPathAcquisitionError, ResolvedOperator, ResolvedOperators,
    acquire_exact_path,
};
use super::super::{Location, LocationRoleError};
use crate::{Pack, PackIdentity, PackageTree, PackageTreeError, PackageTreeFulfillment};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompilationPackageAcquisitionCeilings {
    pub(crate) listed_entries: u64,
    pub(crate) listed_path_bytes: u64,
    pub(crate) total_listed_path_bytes: u64,
    pub(crate) file_objects: u64,
    pub(crate) object_bytes: u64,
    pub(crate) total_bytes: u64,
}

impl CompilationPackageAcquisitionCeilings {
    pub(crate) const fn reference_v1() -> Self {
        Self {
            listed_entries: 1_000_000,
            listed_path_bytes: 64 * 1024,
            total_listed_path_bytes: 256 * 1024 * 1024,
            file_objects: 500_000,
            object_bytes: 64 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilationPackageAcquisitionResource {
    ListedEntries,
    ListedPathBytes,
    TotalListedPathBytes,
    FileObjects,
    ObjectBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum CompilationPackageAcquisitionLimitError {
    #[error(
        "OpenDAL Compilation Package Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: CompilationPackageAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Compilation Package Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: CompilationPackageAcquisitionResource,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum CompilationPackageAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: CompilationPackageAcquisitionResource,
        ceiling: u64,
    },
    #[error("the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}")]
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompilationPackageAcquisitionLimits {
    ceilings: CompilationPackageAcquisitionCeilings,
}

impl CompilationPackageAcquisitionLimits {
    pub(crate) fn new(
        ceilings: CompilationPackageAcquisitionCeilings,
    ) -> Result<Self, CompilationPackageAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                CompilationPackageAcquisitionResource::ObjectBytes,
                ceilings.object_bytes,
            ),
            (
                CompilationPackageAcquisitionResource::TotalBytes,
                ceilings.total_bytes,
            ),
        ] {
            if ceiling == u64::MAX {
                return Err(CompilationPackageAcquisitionLimitsError::CannotProbe {
                    resource,
                    ceiling,
                });
            }
        }
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(
                CompilationPackageAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: ceilings.object_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }
        Ok(Self { ceilings })
    }

    pub(crate) const fn reference_v1() -> Self {
        Self {
            ceilings: CompilationPackageAcquisitionCeilings::reference_v1(),
        }
    }

    pub(crate) const fn listed_entries(self) -> u64 {
        self.ceilings.listed_entries
    }

    pub(crate) const fn listed_path_bytes(self) -> u64 {
        self.ceilings.listed_path_bytes
    }

    pub(crate) const fn total_listed_path_bytes(self) -> u64 {
        self.ceilings.total_listed_path_bytes
    }

    pub(crate) const fn file_objects(self) -> u64 {
        self.ceilings.file_objects
    }

    pub(crate) const fn object_bytes(self) -> u64 {
        self.ceilings.object_bytes
    }

    pub(crate) const fn total_bytes(self) -> u64 {
        self.ceilings.total_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackageTreeSource {
    spec: PackageSpec,
    source: Location,
    provenance: Option<String>,
    cache_hit: bool,
}

impl PackageTreeSource {
    pub(crate) fn new(spec: PackageSpec, source: Location) -> Self {
        Self {
            spec,
            source,
            provenance: None,
            cache_hit: false,
        }
    }

    pub(crate) fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub(crate) fn with_cache_hit(mut self, cache_hit: bool) -> Self {
        self.cache_hit = cache_hit;
        self
    }

    pub(crate) fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) const fn cache_hit(&self) -> bool {
        self.cache_hit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilationPackagePreflightIssue {
    MissingSource {
        spec: PackageSpec,
    },
    DuplicateSource {
        spec: PackageSpec,
    },
    EmbeddedSource {
        spec: PackageSpec,
    },
    UndeclaredSource {
        spec: PackageSpec,
    },
    InvalidSourceRole {
        spec: PackageSpec,
        location: Location,
        source: LocationRoleError,
    },
    ListedEntryLimitExceeded {
        ceiling: u64,
        declared_at_least: u64,
    },
    FileObjectLimitExceeded {
        ceiling: u64,
        declared: u64,
    },
    TotalByteLimitExceeded {
        ceiling: u64,
        declared: u64,
    },
}

impl CompilationPackagePreflightIssue {
    fn spec(&self) -> Option<&PackageSpec> {
        match self {
            Self::MissingSource { spec }
            | Self::DuplicateSource { spec }
            | Self::EmbeddedSource { spec }
            | Self::UndeclaredSource { spec }
            | Self::InvalidSourceRole { spec, .. } => Some(spec),
            Self::ListedEntryLimitExceeded { .. }
            | Self::FileObjectLimitExceeded { .. }
            | Self::TotalByteLimitExceeded { .. } => None,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::MissingSource { .. } => 0,
            Self::DuplicateSource { .. } => 1,
            Self::EmbeddedSource { .. } => 2,
            Self::UndeclaredSource { .. } => 3,
            Self::InvalidSourceRole { .. } => 4,
            Self::ListedEntryLimitExceeded { .. } => 5,
            Self::FileObjectLimitExceeded { .. } => 6,
            Self::TotalByteLimitExceeded { .. } => 7,
        }
    }
}

pub(crate) struct CompilationPackagePreflight {
    pack_identity: PackIdentity,
    targets: Vec<CompilationPackageTarget>,
    issues: Vec<CompilationPackagePreflightIssue>,
    limits: CompilationPackageAcquisitionLimits,
}

impl CompilationPackagePreflight {
    pub(crate) const fn pack_identity(&self) -> PackIdentity {
        self.pack_identity
    }

    pub(crate) fn targets(&self) -> &[CompilationPackageTarget] {
        &self.targets
    }

    pub(crate) fn issues(&self) -> &[CompilationPackagePreflightIssue] {
        &self.issues
    }
}

#[derive(Clone)]
pub(crate) struct CompilationPackageTarget {
    spec: PackageSpec,
    source: Location,
    provenance: Option<String>,
    cache_hit: bool,
    resolved: Option<ResolvedOperator>,
}

impl fmt::Debug for CompilationPackageTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageTarget")
            .field("spec", &self.spec)
            .field("source", &self.source)
            .field("provenance", &self.provenance)
            .field("cache_hit", &self.cache_hit)
            .field("resolved", &self.resolved.is_some())
            .finish()
    }
}

impl CompilationPackageTarget {
    pub(crate) fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) const fn cache_hit(&self) -> bool {
        self.cache_hit
    }
}

pub(crate) fn preflight_compilation_packages(
    pack: &Pack,
    sources: impl IntoIterator<Item = PackageTreeSource>,
    limits: CompilationPackageAcquisitionLimits,
) -> CompilationPackagePreflight {
    let requirements = pack
        .package_requirements()
        .iter()
        .map(|requirement| (requirement.spec().to_string(), requirement))
        .collect::<BTreeMap<_, _>>();
    let external = requirements
        .iter()
        .filter(|(_, requirement)| !requirement.is_embedded())
        .map(|(key, requirement)| (key.clone(), *requirement))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    let mut supplied_external = BTreeSet::new();
    let mut issues = Vec::new();
    let mut targets = Vec::new();

    for configured in sources {
        let key = configured.spec.to_string();
        if !seen.insert(key.clone()) {
            duplicates.insert(key.clone());
        }
        match requirements.get(&key) {
            Some(requirement) if requirement.is_embedded() => {
                issues.push(CompilationPackagePreflightIssue::EmbeddedSource {
                    spec: configured.spec.clone(),
                });
            }
            Some(_) => {
                supplied_external.insert(key);
                targets.push(CompilationPackageTarget {
                    spec: configured.spec.clone(),
                    source: configured.source.clone(),
                    provenance: configured.provenance.clone(),
                    cache_hit: configured.cache_hit,
                    resolved: None,
                });
            }
            None => issues.push(CompilationPackagePreflightIssue::UndeclaredSource {
                spec: configured.spec.clone(),
            }),
        }
        if let Err(source) = configured.source.require_prefix() {
            issues.push(CompilationPackagePreflightIssue::InvalidSourceRole {
                spec: configured.spec,
                location: configured.source,
                source,
            });
        }
    }

    for (key, requirement) in &external {
        if !supplied_external.contains(key) {
            issues.push(CompilationPackagePreflightIssue::MissingSource {
                spec: requirement.spec().clone(),
            });
        }
    }
    for key in duplicates {
        let spec = requirements
            .get(&key)
            .map(|requirement| requirement.spec().clone())
            .unwrap_or_else(|| key.parse().expect("a canonical package key reparses"));
        issues.push(CompilationPackagePreflightIssue::DuplicateSource { spec });
    }

    let (declared_files, declared_bytes) = external.values().fold((0u64, 0u64), |totals, value| {
        (
            totals.0.saturating_add(value.file_count()),
            totals.1.saturating_add(value.byte_length()),
        )
    });
    if declared_files > limits.listed_entries() {
        issues.push(CompilationPackagePreflightIssue::ListedEntryLimitExceeded {
            ceiling: limits.listed_entries(),
            declared_at_least: declared_files,
        });
    }
    if declared_files > limits.file_objects() {
        issues.push(CompilationPackagePreflightIssue::FileObjectLimitExceeded {
            ceiling: limits.file_objects(),
            declared: declared_files,
        });
    }
    if declared_bytes > limits.total_bytes() {
        issues.push(CompilationPackagePreflightIssue::TotalByteLimitExceeded {
            ceiling: limits.total_bytes(),
            declared: declared_bytes,
        });
    }

    issues.sort_by(
        |left_issue, right_issue| match (left_issue.spec(), right_issue.spec()) {
            (Some(left_spec), Some(right_spec)) => left_spec
                .to_string()
                .cmp(&right_spec.to_string())
                .then_with(|| left_issue.rank().cmp(&right_issue.rank()))
                .then_with(|| issue_location_order(left_issue, right_issue)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => left_issue.rank().cmp(&right_issue.rank()),
        },
    );
    issues.dedup();
    if issues.is_empty() {
        targets.sort_by_key(|target| target.spec.to_string());
    } else {
        targets.clear();
    }

    CompilationPackagePreflight {
        pack_identity: pack.identity(),
        targets,
        issues,
        limits,
    }
}

fn issue_location_order(
    left: &CompilationPackagePreflightIssue,
    right: &CompilationPackagePreflightIssue,
) -> Ordering {
    match (left, right) {
        (
            CompilationPackagePreflightIssue::InvalidSourceRole { location: left, .. },
            CompilationPackagePreflightIssue::InvalidSourceRole {
                location: right, ..
            },
        ) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

#[derive(Clone)]
pub(crate) struct CompilationPackageFileTarget {
    package_index: usize,
    operation_path: String,
    relative_path: String,
}

impl CompilationPackageFileTarget {
    pub(crate) fn operation_path(&self) -> &str {
        &self.operation_path
    }

    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

impl fmt::Debug for CompilationPackageFileTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageFileTarget")
            .field("package_index", &self.package_index)
            .field("operation_path", &self.operation_path)
            .field("relative_path", &self.relative_path)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompilationPackageReservation {
    bytes: u64,
}

impl CompilationPackageReservation {
    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }
}

pub(crate) struct CompilationPackageFileAcquisitionEntry {
    package_index: usize,
    relative_path: String,
    bytes: Vec<u8>,
}

impl fmt::Debug for CompilationPackageFileAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageFileAcquisitionEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

pub(crate) struct CompilationPackageAcquisitionEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl CompilationPackageAcquisitionEntry {
    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for CompilationPackageAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageAcquisitionEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

pub(crate) struct CompilationPackageAcquisition {
    spec: PackageSpec,
    source: Location,
    provenance: Option<String>,
    cache_hit: bool,
    entries: Vec<CompilationPackageAcquisitionEntry>,
}

impl CompilationPackageAcquisition {
    pub(crate) fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) const fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    pub(crate) fn entries(&self) -> &[CompilationPackageAcquisitionEntry] {
        &self.entries
    }
}

impl fmt::Debug for CompilationPackageAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageAcquisition")
            .field("spec", &self.spec)
            .field("source", &self.source)
            .field("provenance", &self.provenance)
            .field("cache_hit", &self.cache_hit)
            .field("entries", &self.entries)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilationPackageAcquisitionEntryKind {
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilationPackageAcquisitionIssue {
    ListedPathOutsidePrefix {
        spec: PackageSpec,
        operation_path: String,
    },
    PrefixMarkerWhereFileRequired {
        spec: PackageSpec,
        operation_path: String,
    },
    EmptyRelativeOperationPath {
        spec: PackageSpec,
        operation_path: String,
    },
    UnsupportedEntryKind {
        spec: PackageSpec,
        operation_path: String,
        kind: CompilationPackageAcquisitionEntryKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("Compilation Package Tree survey failed with {} issue(s)", .issues.len())]
pub(crate) struct CompilationPackageAcquisitionSurveyError {
    issues: Vec<CompilationPackageAcquisitionIssue>,
}

impl CompilationPackageAcquisitionSurveyError {
    pub(crate) fn issues(&self) -> &[CompilationPackageAcquisitionIssue] {
        &self.issues
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilationPackageAcquisitionTarget {
    PackageTree {
        spec: PackageSpec,
    },
    PackageFile {
        spec: PackageSpec,
        relative_path: String,
    },
}

pub(crate) struct CompilationPackageRole<E> {
    packages: Vec<CompilationPackageTarget>,
    files: Vec<CompilationPackageFileTarget>,
    limits: CompilationPackageAcquisitionLimits,
    marker: PhantomData<fn() -> E>,
}

impl<E> CompilationPackageRole<E> {
    pub(crate) const fn total_bytes(&self) -> u64 {
        self.limits.total_bytes()
    }

    pub(crate) async fn survey(&mut self) -> Result<(), CompilationPackageAcquisitionError<E>> {
        let sources = self
            .packages
            .iter()
            .map(|target| {
                (
                    &target.source,
                    target
                        .resolved
                        .as_ref()
                        .expect("compilation package targets are appraised before survey")
                        .clone(),
                )
            })
            .collect::<Vec<_>>();
        let plans =
            match survey_required_recursive_prefixes_with_operators(&sources, self.limits.into())
                .await
                .map_err(|error| map_recursive_error(&self.packages, error))?
            {
                RequiredRecursiveSurvey::Complete(plans) => plans,
                RequiredRecursiveSurvey::PrefixAbsent { source_index } => {
                    return Err(CompilationPackageAcquisitionError::for_package(
                        &self.packages[source_index],
                        CompilationPackageAcquisitionErrorCause::PrefixAbsent,
                    ));
                }
            };

        self.files = plans
            .into_iter()
            .enumerate()
            .flat_map(|(package_index, plan)| {
                plan.selected
                    .into_iter()
                    .map(move |path| CompilationPackageFileTarget {
                        package_index,
                        operation_path: path.operation_path,
                        relative_path: path.relative_path,
                    })
            })
            .collect();
        Ok(())
    }

    pub(crate) fn total_bytes_exhausted(
        &self,
        target: &CompilationPackageFileTarget,
    ) -> CompilationPackageAcquisitionError<E> {
        let ceiling = self.limits.total_bytes();
        CompilationPackageAcquisitionError::for_file(
            &self.packages,
            target,
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::Exceeded {
                    resource: CompilationPackageAcquisitionResource::TotalBytes,
                    ceiling,
                    observed_at_least: ceiling + 1,
                },
            ),
        )
    }

    pub(crate) fn target_spec(&self, target: &CompilationPackageFileTarget) -> &PackageSpec {
        &self.packages[target.package_index].spec
    }

    pub(crate) fn finish(
        self,
        entries: Vec<CompilationPackageFileAcquisitionEntry>,
    ) -> Vec<CompilationPackageAcquisition> {
        let mut grouped = (0..self.packages.len())
            .map(|_| Vec::new())
            .collect::<Vec<Vec<CompilationPackageAcquisitionEntry>>>();
        for entry in entries {
            grouped[entry.package_index].push(CompilationPackageAcquisitionEntry {
                relative_path: entry.relative_path,
                bytes: entry.bytes,
            });
        }
        self.packages
            .into_iter()
            .zip(grouped)
            .map(|(package, mut entries)| {
                entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
                CompilationPackageAcquisition {
                    spec: package.spec,
                    source: package.source,
                    provenance: package.provenance,
                    cache_hit: package.cache_hit,
                    entries,
                }
            })
            .collect()
    }
}

impl<E> fmt::Debug for CompilationPackageRole<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageRole")
            .field("packages", &self.packages)
            .field("files", &self.files)
            .field("limits", &self.limits)
            .finish()
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn prepare_compilation_package_role<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    mut preflight: CompilationPackagePreflight,
) -> Result<CompilationPackageRole<R::Error>, CompilationPackageAcquisitionError<R::Error>> {
    debug_assert!(preflight.issues.is_empty());
    for target in &mut preflight.targets {
        let appraised = resolved
            .resolve(target.source.binding())
            .map_err(|source| {
                CompilationPackageAcquisitionError::for_package(
                    target,
                    CompilationPackageAcquisitionErrorCause::ResolveOperator(source),
                )
            })?;
        if !(appraised.list && appraised.list_with_recursive && appraised.read) {
            return Err(CompilationPackageAcquisitionError::for_package(
                target,
                CompilationPackageAcquisitionErrorCause::UnsupportedCapabilities {
                    list: appraised.list,
                    list_with_recursive: appraised.list_with_recursive,
                    read: appraised.read,
                },
            ));
        }
        target.resolved = Some(appraised);
    }
    Ok(CompilationPackageRole {
        packages: preflight.targets,
        files: Vec::new(),
        limits: preflight.limits,
        marker: PhantomData,
    })
}

impl<E> AcquisitionRole for CompilationPackageRole<E> {
    type Target = CompilationPackageFileTarget;
    type Reservation = CompilationPackageReservation;
    type RawResult = CompilationPackageFileAcquisitionEntry;
    type Failure = CompilationPackageAcquisitionError<E>;
    type Acquire<'a>
        = Pin<Box<dyn Future<Output = Result<Self::RawResult, Self::Failure>> + Send + 'a>>
    where
        Self: 'a;

    fn targets(&self) -> &[Self::Target] {
        &self.files
    }

    fn reserve(&mut self, _: &Self::Target) -> Result<Self::Reservation, Self::Failure> {
        Ok(CompilationPackageReservation {
            bytes: self.limits.object_bytes(),
        })
    }

    fn acquire<'a>(
        &'a self,
        target: &'a Self::Target,
        reservation: Self::Reservation,
    ) -> Self::Acquire<'a> {
        Box::pin(async move {
            let package = &self.packages[target.package_index];
            let bytes = acquire_exact_path(
                &package
                    .resolved
                    .as_ref()
                    .expect("compilation package targets are appraised before reads")
                    .operator,
                &target.operation_path,
                reservation.bytes(),
                self.limits.object_bytes(),
            )
            .await
            .map_err(|error| {
                CompilationPackageAcquisitionError::for_file(
                    &self.packages,
                    target,
                    map_exact_path_error(error),
                )
            })?;
            Ok(CompilationPackageFileAcquisitionEntry {
                package_index: target.package_index,
                relative_path: target.relative_path.clone(),
                bytes,
            })
        })
    }
}

pub(crate) struct CompilationPackageAcquisitionError<E> {
    target: CompilationPackageAcquisitionTarget,
    source_location: Location,
    failed_path: Option<String>,
    cause: CompilationPackageAcquisitionErrorCause<E>,
}

impl<E> CompilationPackageAcquisitionError<E> {
    fn for_package(
        target: &CompilationPackageTarget,
        cause: CompilationPackageAcquisitionErrorCause<E>,
    ) -> Self {
        Self {
            target: CompilationPackageAcquisitionTarget::PackageTree {
                spec: target.spec.clone(),
            },
            source_location: target.source.clone(),
            failed_path: None,
            cause,
        }
    }

    fn for_file(
        packages: &[CompilationPackageTarget],
        target: &CompilationPackageFileTarget,
        cause: CompilationPackageAcquisitionErrorCause<E>,
    ) -> Self {
        Self {
            target: CompilationPackageAcquisitionTarget::PackageFile {
                spec: packages[target.package_index].spec.clone(),
                relative_path: target.relative_path.clone(),
            },
            source_location: packages[target.package_index].source.clone(),
            failed_path: Some(target.operation_path.clone()),
            cause,
        }
    }

    pub(crate) fn target(&self) -> &CompilationPackageAcquisitionTarget {
        &self.target
    }

    pub(crate) fn source_location(&self) -> &Location {
        &self.source_location
    }

    pub(crate) fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub(crate) fn cause(&self) -> &CompilationPackageAcquisitionErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for CompilationPackageAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Compilation Package Acquisition failed for {:?}, binding {}, and prefix operation path {:?}",
            self.target,
            self.source_location.binding(),
            self.source_location.operation_path(),
        )?;
        if let Some(path) = &self.failed_path {
            write!(formatter, " while reading object operation path {path:?}")?;
        }
        write!(formatter, ": {}", self.cause.label())
    }
}

impl<E> fmt::Debug for CompilationPackageAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationPackageAcquisitionError")
            .field("target", &self.target)
            .field("binding", self.source_location.binding())
            .field("role", &"recursive prefix")
            .field("operation_path", &self.source_location.operation_path())
            .field("failed_path", &self.failed_path)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for CompilationPackageAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            CompilationPackageAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            CompilationPackageAcquisitionErrorCause::UnsupportedCapabilities { .. }
            | CompilationPackageAcquisitionErrorCause::PrefixAbsent => None,
            CompilationPackageAcquisitionErrorCause::List(source)
            | CompilationPackageAcquisitionErrorCause::Read(source)
            | CompilationPackageAcquisitionErrorCause::ListedObjectAbsent(source) => Some(source),
            CompilationPackageAcquisitionErrorCause::PackageStructural(source) => Some(source),
            CompilationPackageAcquisitionErrorCause::InvalidPackageTree(source) => Some(source),
            CompilationPackageAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CompilationPackageAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    List(opendal::Error),
    Read(opendal::Error),
    PrefixAbsent,
    ListedObjectAbsent(opendal::Error),
    PackageStructural(CompilationPackageAcquisitionSurveyError),
    InvalidPackageTree(PackageTreeError),
    Limit(CompilationPackageAcquisitionLimitError),
}

impl<E> CompilationPackageAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedCapabilities { .. } => {
                "required Package Tree capabilities are unsupported"
            }
            Self::List(_) => "the Package Tree listing failed",
            Self::Read(_) => "a listed Package Tree object read failed",
            Self::PrefixAbsent => "the Package Tree prefix is absent",
            Self::ListedObjectAbsent(_) => "a listed Package Tree object became absent",
            Self::PackageStructural(_) => "the Package Tree listing had structural issues",
            Self::InvalidPackageTree(_) => "the listed objects do not form a Package Tree",
            Self::Limit(_) => "a Compilation Package Acquisition limit failed",
        }
    }
}

fn map_recursive_error<E>(
    targets: &[CompilationPackageTarget],
    error: RecursiveSourcesAcquisitionError<std::convert::Infallible>,
) -> CompilationPackageAcquisitionError<E> {
    let target = &targets[error.source_index];
    let (failed_path, cause) = match error.source {
        RecursiveAcquisitionError::InvalidLocationRole(_) => {
            unreachable!("compilation package preflight validates every prefix role")
        }
        RecursiveAcquisitionError::ResolveOperator(source) => match source {},
        RecursiveAcquisitionError::UnsupportedCapabilities {
            list,
            list_with_recursive,
            read,
        } => (
            None,
            CompilationPackageAcquisitionErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            },
        ),
        RecursiveAcquisitionError::List(source) => {
            (None, CompilationPackageAcquisitionErrorCause::List(source))
        }
        RecursiveAcquisitionError::Read {
            operation_path,
            source,
        } => (
            Some(operation_path),
            CompilationPackageAcquisitionErrorCause::Read(source),
        ),
        RecursiveAcquisitionError::ListedObjectAbsent {
            operation_path,
            source,
        } => (
            Some(operation_path),
            CompilationPackageAcquisitionErrorCause::ListedObjectAbsent(source),
        ),
        RecursiveAcquisitionError::Structural(issues) => (
            None,
            CompilationPackageAcquisitionErrorCause::PackageStructural(
                CompilationPackageAcquisitionSurveyError {
                    issues: issues
                        .into_iter()
                        .map(|issue| map_survey_issue(targets, issue))
                        .collect(),
                },
            ),
        ),
        RecursiveAcquisitionError::InvalidPackageTree(source) => (
            None,
            CompilationPackageAcquisitionErrorCause::InvalidPackageTree(source),
        ),
        RecursiveAcquisitionError::Limit {
            resource,
            ceiling,
            observed_at_least,
        } => (
            None,
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::Exceeded {
                    resource: map_resource(resource),
                    ceiling,
                    observed_at_least,
                },
            ),
        ),
        RecursiveAcquisitionError::AccountingOverflow { resource } => (
            None,
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::AccountingOverflow {
                    resource: map_resource(resource),
                },
            ),
        ),
    };
    CompilationPackageAcquisitionError {
        target: CompilationPackageAcquisitionTarget::PackageTree {
            spec: target.spec.clone(),
        },
        source_location: target.source.clone(),
        failed_path,
        cause,
    }
}

fn map_survey_issue(
    targets: &[CompilationPackageTarget],
    issue: RecursiveSurveyIssue,
) -> CompilationPackageAcquisitionIssue {
    let spec = targets[issue.source_index].spec.clone();
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            CompilationPackageAcquisitionIssue::ListedPathOutsidePrefix {
                spec,
                operation_path: issue.operation_path,
            }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            CompilationPackageAcquisitionIssue::PrefixMarkerWhereFileRequired {
                spec,
                operation_path: issue.operation_path,
            }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            CompilationPackageAcquisitionIssue::EmptyRelativeOperationPath {
                spec,
                operation_path: issue.operation_path,
            }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            CompilationPackageAcquisitionIssue::UnsupportedEntryKind {
                spec,
                operation_path: issue.operation_path,
                kind: CompilationPackageAcquisitionEntryKind::Unknown,
            }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath
        | RecursiveSurveyIssueKind::DuplicateListedObject => {
            unreachable!("core Package Tree path preflight owns package path shape")
        }
    }
}

fn map_resource(resource: RecursiveAcquisitionResource) -> CompilationPackageAcquisitionResource {
    match resource {
        RecursiveAcquisitionResource::ListedEntries => {
            CompilationPackageAcquisitionResource::ListedEntries
        }
        RecursiveAcquisitionResource::ListedPathBytes => {
            CompilationPackageAcquisitionResource::ListedPathBytes
        }
        RecursiveAcquisitionResource::TotalListedPathBytes => {
            CompilationPackageAcquisitionResource::TotalListedPathBytes
        }
        RecursiveAcquisitionResource::SelectedObjects => {
            CompilationPackageAcquisitionResource::FileObjects
        }
        RecursiveAcquisitionResource::ObjectBytes => {
            CompilationPackageAcquisitionResource::ObjectBytes
        }
        RecursiveAcquisitionResource::TotalBytes => {
            CompilationPackageAcquisitionResource::TotalBytes
        }
    }
}

fn map_exact_path_error<E>(
    error: ExactPathAcquisitionError,
) -> CompilationPackageAcquisitionErrorCause<E> {
    match error {
        ExactPathAcquisitionError::ObjectAbsent(source) => {
            CompilationPackageAcquisitionErrorCause::ListedObjectAbsent(source)
        }
        ExactPathAcquisitionError::Read(source) => {
            CompilationPackageAcquisitionErrorCause::Read(source)
        }
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::Exceeded {
            ceiling,
            observed_at_least,
        }) => CompilationPackageAcquisitionErrorCause::Limit(
            CompilationPackageAcquisitionLimitError::Exceeded {
                resource: CompilationPackageAcquisitionResource::ObjectBytes,
                ceiling,
                observed_at_least,
            },
        ),
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow) => {
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::AccountingOverflow {
                    resource: CompilationPackageAcquisitionResource::ObjectBytes,
                },
            )
        }
    }
}

impl From<CompilationPackageAcquisitionLimits> for RecursiveAcquisitionLimits {
    fn from(limits: CompilationPackageAcquisitionLimits) -> Self {
        Self {
            listed_entries: limits.listed_entries(),
            listed_path_bytes: limits.listed_path_bytes(),
            total_listed_path_bytes: limits.total_listed_path_bytes(),
            selected_objects: limits.file_objects(),
            object_bytes: limits.object_bytes(),
            total_bytes: limits.total_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("acquired Package Tree for {spec} is invalid: {source}")]
pub(crate) struct CompilationPackageConversionError {
    spec: PackageSpec,
    #[source]
    source: PackageTreeError,
}

impl CompilationPackageConversionError {
    pub(crate) fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub(crate) fn cause(&self) -> &PackageTreeError {
        &self.source
    }
}

pub(crate) fn convert_compilation_packages(
    acquisitions: Vec<CompilationPackageAcquisition>,
) -> Result<Vec<PackageTreeFulfillment>, CompilationPackageConversionError> {
    acquisitions
        .into_iter()
        .map(|acquisition| {
            let tree = PackageTree::from_owned_entries(
                acquisition
                    .entries
                    .into_iter()
                    .map(CompilationPackageAcquisitionEntry::into_parts),
            )
            .map_err(|source| CompilationPackageConversionError {
                spec: acquisition.spec.clone(),
                source,
            })?;
            let mut fulfillment = PackageTreeFulfillment::new(acquisition.spec, tree)
                .cache_hit(acquisition.cache_hit);
            if let Some(provenance) = acquisition.provenance {
                fulfillment = fulfillment.provenance(provenance);
            }
            Ok(fulfillment)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::Pack;
    use crate::opendal::OperatorResolver;
    use crate::opendal::acquisition::ResolvedOperators;
    use crate::opendal::acquisition::recursive::AcquisitionRole;
    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
        PendingPoint, ReadScript, ReadStep, ScriptedService,
    };
    use crate::opendal::{Location, LocationRoleError, OperatorBinding};

    use super::*;

    #[test]
    fn compilation_package_limits_are_named_finite_and_internally_consistent() {
        let reference = CompilationPackageAcquisitionLimits::reference_v1();
        assert_eq!(reference.listed_entries(), 1_000_000);
        assert_eq!(reference.listed_path_bytes(), 64 * 1024);
        assert_eq!(reference.total_listed_path_bytes(), 256 * 1024 * 1024);
        assert_eq!(reference.file_objects(), 500_000);
        assert_eq!(reference.object_bytes(), 64 * 1024 * 1024);
        assert_eq!(reference.total_bytes(), 2 * 1024 * 1024 * 1024);

        for resource in [
            CompilationPackageAcquisitionResource::ObjectBytes,
            CompilationPackageAcquisitionResource::TotalBytes,
        ] {
            let mut ceilings = CompilationPackageAcquisitionCeilings::reference_v1();
            match resource {
                CompilationPackageAcquisitionResource::ObjectBytes => {
                    ceilings.object_bytes = u64::MAX;
                }
                CompilationPackageAcquisitionResource::TotalBytes => {
                    ceilings.total_bytes = u64::MAX;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                CompilationPackageAcquisitionLimits::new(ceilings),
                Err(CompilationPackageAcquisitionLimitsError::CannotProbe {
                    resource,
                    ceiling: u64::MAX,
                })
            );
        }

        assert_eq!(
            CompilationPackageAcquisitionLimits::new(CompilationPackageAcquisitionCeilings {
                listed_entries: 1,
                listed_path_bytes: 1,
                total_listed_path_bytes: 1,
                file_objects: 1,
                object_bytes: 5,
                total_bytes: 4,
            }),
            Err(
                CompilationPackageAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: 5,
                    total_bytes: 4,
                }
            )
        );
    }

    #[test]
    fn package_preflight_aggregates_coverage_roles_duplicates_and_declared_limits() {
        let pack = package_pack();
        let external: typst::syntax::package::PackageSpec =
            "@preview/external:1.0.0".parse().unwrap();
        let embedded: typst::syntax::package::PackageSpec =
            "@preview/embedded:1.0.0".parse().unwrap();
        let undeclared: typst::syntax::package::PackageSpec =
            "@preview/undeclared:1.0.0".parse().unwrap();
        let sources = [
            PackageTreeSource::new(embedded.clone(), location("embedded/")),
            PackageTreeSource::new(undeclared.clone(), location("undeclared/")),
            PackageTreeSource::new(external.clone(), location("not-a-prefix")),
            PackageTreeSource::new(external.clone(), location("duplicate/")),
        ];
        let limits = package_limits(1, 128, 512, 1, 5, 5);

        let first = preflight_compilation_packages(&pack, sources.clone(), limits);
        let second = preflight_compilation_packages(&pack, sources.into_iter().rev(), limits);

        assert_eq!(first.issues(), second.issues());
        assert_eq!(
            first.issues(),
            [
                CompilationPackagePreflightIssue::MissingSource {
                    spec: "@preview/a:1.0.0".parse().unwrap(),
                },
                CompilationPackagePreflightIssue::EmbeddedSource { spec: embedded },
                CompilationPackagePreflightIssue::DuplicateSource {
                    spec: external.clone(),
                },
                CompilationPackagePreflightIssue::InvalidSourceRole {
                    spec: external,
                    location: location("not-a-prefix"),
                    source: LocationRoleError::PrefixMissingTrailingSlash,
                },
                CompilationPackagePreflightIssue::UndeclaredSource { spec: undeclared },
                CompilationPackagePreflightIssue::ListedEntryLimitExceeded {
                    ceiling: 1,
                    declared_at_least: 3,
                },
                CompilationPackagePreflightIssue::FileObjectLimitExceeded {
                    ceiling: 1,
                    declared: 3,
                },
                CompilationPackagePreflightIssue::TotalByteLimitExceeded {
                    ceiling: 5,
                    declared: 7,
                },
            ]
        );
        assert!(first.targets().is_empty());
    }

    #[test]
    fn accepted_package_targets_are_canonical_and_keep_operational_metadata() {
        let pack = package_pack();
        let first: typst::syntax::package::PackageSpec = "@preview/a:1.0.0".parse().unwrap();
        let second: typst::syntax::package::PackageSpec =
            "@preview/external:1.0.0".parse().unwrap();
        let sources = [
            PackageTreeSource::new(second.clone(), location("second/"))
                .with_provenance("registry mirror")
                .with_cache_hit(true),
            PackageTreeSource::new(first.clone(), location("first/")),
        ];

        let preflight = preflight_compilation_packages(
            &pack,
            sources,
            CompilationPackageAcquisitionLimits::reference_v1(),
        );

        assert!(preflight.issues().is_empty());
        assert_eq!(preflight.pack_identity(), pack.identity());
        assert_eq!(
            preflight
                .targets()
                .iter()
                .map(|target| target.spec().to_string())
                .collect::<Vec<_>>(),
            ["@preview/a:1.0.0", "@preview/external:1.0.0"]
        );
        assert_eq!(preflight.targets()[1].source(), &location("second/"));
        assert_eq!(preflight.targets()[1].provenance(), Some("registry mirror"));
        assert!(preflight.targets()[1].cache_hit());
    }

    #[test]
    fn package_role_surveys_before_exposing_canonical_reservable_file_reads() {
        let lists = [
            ListScript::new(
                "a/",
                1,
                [
                    ListStep::page([ListEntry::file("a/lib.typ")]),
                    ListStep::replace_read(
                        ReadScript::new("a/lib.typ", 1, [ReadStep::chunk(b"after listing")])
                            .unwrap(),
                    ),
                ],
            )
            .unwrap(),
            ListScript::new(
                "external/",
                2,
                [ListStep::page([
                    ListEntry::file("external/lib.typ"),
                    ListEntry::file("external/extra.typ"),
                ])],
            )
            .unwrap(),
        ];
        let reads = [
            ReadScript::new("a/lib.typ", 1, [ReadStep::chunk(b"before listing")]).unwrap(),
            ReadScript::new("external/extra.typ", 1, [ReadStep::chunk(b"12")]).unwrap(),
            ReadScript::new("external/lib.typ", 1, [ReadStep::chunk(b"1234")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), lists, reads, 16);
        let resolver = CountingResolver::new(service.operator());
        let pack = package_pack();
        let sources = [
            PackageTreeSource::new(
                "@preview/external:1.0.0".parse().unwrap(),
                location("external/"),
            )
            .with_provenance("mirror")
            .with_cache_hit(true),
            PackageTreeSource::new("@preview/a:1.0.0".parse().unwrap(), location("a/")),
        ];
        let preflight =
            preflight_compilation_packages(&pack, sources, package_limits(8, 128, 1024, 8, 16, 32));
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_compilation_package_role(&mut resolved, preflight).unwrap();

        assert_eq!(resolver.calls(), 1);
        assert!(role.targets().is_empty());
        expect_ready(pin!(role.survey())).unwrap();
        assert_eq!(
            role.targets()
                .iter()
                .map(|target| (role.target_spec(target).to_string(), target.relative_path()))
                .collect::<Vec<_>>(),
            [
                ("@preview/a:1.0.0".to_owned(), "lib.typ"),
                ("@preview/external:1.0.0".to_owned(), "extra.typ"),
                ("@preview/external:1.0.0".to_owned(), "lib.typ"),
            ]
        );
        assert!(matches!(
            role.total_bytes_exhausted(&role.targets()[0]).cause(),
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::Exceeded {
                    resource: CompilationPackageAcquisitionResource::TotalBytes,
                    ceiling: 32,
                    observed_at_least: 33,
                }
            )
        ));

        let mut entries = Vec::new();
        for target in role.targets().to_vec() {
            let reservation = role.reserve(&target).unwrap();
            assert_eq!(reservation.bytes(), 16);
            entries.push(expect_ready(pin!(role.acquire(&target, reservation))).unwrap());
        }
        let raw = role.finish(entries);
        assert_eq!(raw.len(), 2);
        assert_eq!(raw[1].provenance(), Some("mirror"));
        assert!(raw[1].cache_hit());
        assert_eq!(
            raw[1]
                .entries()
                .iter()
                .map(|entry| (entry.relative_path(), entry.bytes()))
                .collect::<Vec<_>>(),
            [
                ("extra.typ", b"12".as_slice()),
                ("lib.typ", b"1234".as_slice())
            ]
        );

        let fulfillments = convert_compilation_packages(raw).unwrap();
        assert_eq!(fulfillments.len(), 2);
        assert_eq!(
            fulfillments[0].tree().file("lib.typ"),
            Some(b"after listing".as_slice())
        );
        assert_eq!(
            fulfillments[1].tree().file("lib.typ"),
            Some(b"1234".as_slice())
        );
    }

    #[test]
    fn package_surveys_share_actual_file_and_path_budgets_across_targets() {
        let pack = two_single_file_package_pack();
        let lists = [
            ListScript::new("a/", 1, [ListStep::page([ListEntry::file("a/lib.typ")])]).unwrap(),
            ListScript::new(
                "b/",
                2,
                [ListStep::page([
                    ListEntry::file("b/extra.typ"),
                    ListEntry::file("b/lib.typ"),
                ])],
            )
            .unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), lists, [], 8);
        let resolver = CountingResolver::new(service.operator());
        let preflight = preflight_compilation_packages(
            &pack,
            [
                PackageTreeSource::new("@preview/a:1.0.0".parse().unwrap(), location("a/")),
                PackageTreeSource::new("@preview/b:1.0.0".parse().unwrap(), location("b/")),
            ],
            package_limits(8, 128, 1024, 2, 8, 16),
        );
        assert!(preflight.issues().is_empty());
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_compilation_package_role(&mut resolved, preflight).unwrap();

        let error = expect_ready(pin!(role.survey())).unwrap_err();

        assert!(matches!(
            error.target(),
            CompilationPackageAcquisitionTarget::PackageTree { spec }
                if spec.to_string() == "@preview/b:1.0.0"
        ));
        assert!(matches!(
            error.cause(),
            CompilationPackageAcquisitionErrorCause::Limit(
                CompilationPackageAcquisitionLimitError::Exceeded {
                    resource: CompilationPackageAcquisitionResource::FileObjects,
                    ceiling: 2,
                    observed_at_least: 3,
                }
            )
        ));
    }

    #[test]
    fn package_survey_preserves_core_path_errors_and_empty_prefix_absence() {
        let pack = single_package_pack();
        let invalid_list = ListScript::new(
            "package/",
            2,
            [ListStep::page([
                ListEntry::file("package/assets"),
                ListEntry::file("package/assets/logo.svg"),
            ])],
        )
        .unwrap();
        let invalid_service = ScriptedService::new(Capabilities::all(), [invalid_list], [], 8);
        let invalid_resolver = CountingResolver::new(invalid_service.operator());
        let mut invalid = prepared_single_package_role(
            &pack,
            &invalid_resolver,
            CompilationPackageAcquisitionLimits::reference_v1(),
        );

        let error = expect_ready(pin!(invalid.survey())).unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationPackageAcquisitionErrorCause::InvalidPackageTree(source)
                if source.issues().len() == 1
        ));

        let empty_list = ListScript::new("package/", 0, []).unwrap();
        let empty_service = ScriptedService::new(Capabilities::all(), [empty_list], [], 4);
        let empty_resolver = CountingResolver::new(empty_service.operator());
        let mut empty = prepared_single_package_role(
            &pack,
            &empty_resolver,
            CompilationPackageAcquisitionLimits::reference_v1(),
        );
        let error = expect_ready(pin!(empty.survey())).unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationPackageAcquisitionErrorCause::PrefixAbsent
        ));

        let lists = [
            ListScript::new("a/", 0, []).unwrap(),
            ListScript::new("b/", 0, [ListStep::failure(ErrorKind::PermissionDenied)]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), lists, [], 4);
        let resolver = CountingResolver::new(service.operator());
        let pack = two_single_file_package_pack();
        let preflight = preflight_compilation_packages(
            &pack,
            [
                PackageTreeSource::new("@preview/a:1.0.0".parse().unwrap(), location("a/")),
                PackageTreeSource::new("@preview/b:1.0.0".parse().unwrap(), location("b/")),
            ],
            CompilationPackageAcquisitionLimits::reference_v1(),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_compilation_package_role(&mut resolved, preflight).unwrap();
        let error = expect_ready(pin!(role.survey())).unwrap_err();
        assert!(matches!(
            error.target(),
            CompilationPackageAcquisitionTarget::PackageTree { spec }
                if spec.to_string() == "@preview/a:1.0.0"
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
    fn package_conversion_preserves_authoritative_tree_errors_without_partial_values() {
        let spec: PackageSpec = "@preview/example:1.0.0".parse().unwrap();
        let acquisition = CompilationPackageAcquisition {
            spec: spec.clone(),
            source: location("package/"),
            provenance: None,
            cache_hit: false,
            entries: vec![
                CompilationPackageAcquisitionEntry {
                    relative_path: "lib.typ".to_owned(),
                    bytes: b"first".to_vec(),
                },
                CompilationPackageAcquisitionEntry {
                    relative_path: "./lib.typ".to_owned(),
                    bytes: b"second".to_vec(),
                },
            ],
        };

        let error = convert_compilation_packages(vec![acquisition]).unwrap_err();

        assert_eq!(error.spec(), &spec);
        assert!(matches!(
            error.cause().issues(),
            [crate::PackageTreeIssue::DuplicatePath { path }] if path == "lib.typ"
        ));
    }

    #[test]
    fn dropping_a_package_file_read_discards_bytes_and_the_reservation() {
        let pending = PendingPoint::new();
        let list = ListScript::new(
            "package/",
            1,
            [ListStep::page([ListEntry::file("package/lib.typ")])],
        )
        .unwrap();
        let read = ReadScript::new(
            "package/lib.typ",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::pending(pending.clone()),
            ],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
        let resolver = CountingResolver::new(service.operator());
        let pack = single_package_pack();
        let mut role = prepared_single_package_role(
            &pack,
            &resolver,
            CompilationPackageAcquisitionLimits::reference_v1(),
        );
        expect_ready(pin!(role.survey())).unwrap();
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target).unwrap();

        {
            let mut acquisition = pin!(role.acquire(&target, reservation));
            assert!(matches!(
                acquisition
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            assert!(pending.was_observed());
        }

        assert_eq!(
            service.cancellations(),
            [DroppedOperation::Read {
                id: 1,
                path: "package/lib.typ".to_owned(),
            }]
        );
    }

    #[test]
    fn memory_backend_surveys_and_acquires_a_package_tree() {
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        expect_ready(pin!(
            operator.write("package/lib.typ", b"memory package".to_vec())
        ))
        .unwrap();
        let resolver = CountingResolver::new(operator);
        let pack = single_package_pack();
        let mut role = prepared_single_package_role(
            &pack,
            &resolver,
            CompilationPackageAcquisitionLimits::reference_v1(),
        );

        expect_ready(pin!(role.survey())).unwrap();
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target).unwrap();
        let entry = expect_ready(pin!(role.acquire(&target, reservation))).unwrap();
        let fulfillment = convert_compilation_packages(role.finish(vec![entry])).unwrap();

        assert_eq!(
            fulfillment[0].tree().file("lib.typ"),
            Some(b"memory package".as_slice())
        );
    }

    fn package_pack() -> Pack {
        Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .external_package_file(
                "@preview/external:1.0.0".parse().unwrap(),
                "lib.typ",
                b"1234".to_vec(),
            )
            .unwrap()
            .external_package_file(
                "@preview/external:1.0.0".parse().unwrap(),
                "extra.typ",
                b"12".to_vec(),
            )
            .unwrap()
            .external_package_file(
                "@preview/a:1.0.0".parse().unwrap(),
                "lib.typ",
                b"a".to_vec(),
            )
            .unwrap()
            .package_file(
                "@preview/embedded:1.0.0".parse().unwrap(),
                "lib.typ",
                b"embedded".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    fn single_package_pack() -> Pack {
        Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .external_package_file(
                "@preview/example:1.0.0".parse().unwrap(),
                "lib.typ",
                b"expected".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    fn two_single_file_package_pack() -> Pack {
        Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .external_package_file(
                "@preview/a:1.0.0".parse().unwrap(),
                "lib.typ",
                b"a".to_vec(),
            )
            .unwrap()
            .external_package_file(
                "@preview/b:1.0.0".parse().unwrap(),
                "lib.typ",
                b"b".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap()
    }

    fn prepared_single_package_role(
        pack: &Pack,
        resolver: &CountingResolver,
        limits: CompilationPackageAcquisitionLimits,
    ) -> CompilationPackageRole<Infallible> {
        let preflight = preflight_compilation_packages(
            pack,
            [PackageTreeSource::new(
                "@preview/example:1.0.0".parse().unwrap(),
                location("package/"),
            )],
            limits,
        );
        let mut resolved = ResolvedOperators::new(resolver);
        prepare_compilation_package_role(&mut resolved, preflight).unwrap()
    }

    fn package_limits(
        listed_entries: u64,
        listed_path_bytes: u64,
        total_listed_path_bytes: u64,
        file_objects: u64,
        object_bytes: u64,
        total_bytes: u64,
    ) -> CompilationPackageAcquisitionLimits {
        CompilationPackageAcquisitionLimits::new(CompilationPackageAcquisitionCeilings {
            listed_entries,
            listed_path_bytes,
            total_listed_path_bytes,
            file_objects,
            object_bytes,
            total_bytes,
        })
        .unwrap()
    }

    fn location(path: &str) -> Location {
        Location::from_operation_path(OperatorBinding::new("packages").unwrap(), path).unwrap()
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
