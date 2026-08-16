use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::acquisition::recursive::AcquisitionRole;
use super::acquisition::{
    ExactObjectLimitError, ExactPathAcquisitionError, ResolvedOperator, ResolvedOperators,
    acquire_exact_path,
};
use super::{Location, LocationRoleError, OperatorResolver};
use crate::{Pack, PackIdentity, PackOverrideSet, PackOverrideSetError};

mod font;
mod package;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackOverrideAcquisitionCeilings {
    pub(crate) objects: u64,
    pub(crate) object_bytes: u64,
    pub(crate) total_bytes: u64,
}

impl PackOverrideAcquisitionCeilings {
    pub(crate) const fn reference_v1() -> Self {
        Self {
            objects: 100_000,
            object_bytes: 256 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PackOverrideAcquisitionResource {
    Objects,
    ObjectBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackOverrideAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: PackOverrideAcquisitionResource,
        ceiling: u64,
    },
    #[error("the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}")]
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackOverrideAcquisitionLimits {
    ceilings: PackOverrideAcquisitionCeilings,
}

impl PackOverrideAcquisitionLimits {
    pub(crate) fn new(
        ceilings: PackOverrideAcquisitionCeilings,
    ) -> Result<Self, PackOverrideAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                PackOverrideAcquisitionResource::ObjectBytes,
                ceilings.object_bytes,
            ),
            (
                PackOverrideAcquisitionResource::TotalBytes,
                ceilings.total_bytes,
            ),
        ] {
            if ceiling == u64::MAX {
                return Err(PackOverrideAcquisitionLimitsError::CannotProbe { resource, ceiling });
            }
        }
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(
                PackOverrideAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: ceilings.object_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }

        Ok(Self { ceilings })
    }

    pub(crate) const fn reference_v1() -> Self {
        Self {
            ceilings: PackOverrideAcquisitionCeilings::reference_v1(),
        }
    }

    pub(crate) const fn objects(self) -> u64 {
        self.ceilings.objects
    }

    pub(crate) const fn object_bytes(self) -> u64 {
        self.ceilings.object_bytes
    }

    pub(crate) const fn total_bytes(self) -> u64 {
        self.ceilings.total_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackOverrideAcquisitionLimitError {
    #[error(
        "OpenDAL Pack Override Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: PackOverrideAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Pack Override Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: PackOverrideAcquisitionResource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackOverrideSource {
    path: String,
    source: Location,
}

impl PackOverrideSource {
    pub(crate) fn new(path: impl Into<String>, source: Location) -> Self {
        Self {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PackOverridePreflightIssue {
    InvalidPath {
        supplied: String,
        source: PackOverrideSetError,
    },
    MissingTarget {
        path: String,
    },
    DuplicateTarget {
        path: String,
    },
    InvalidSourceRole {
        path: String,
        location: Location,
        source: LocationRoleError,
    },
    ObjectLimitExceeded {
        ceiling: u64,
        observed_at_least: u64,
    },
}

impl PackOverridePreflightIssue {
    fn path(&self) -> Option<&str> {
        match self {
            Self::InvalidPath { supplied, .. } => Some(supplied),
            Self::MissingTarget { path }
            | Self::DuplicateTarget { path }
            | Self::InvalidSourceRole { path, .. } => Some(path),
            Self::ObjectLimitExceeded { .. } => None,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::InvalidPath { .. } => 0,
            Self::MissingTarget { .. } => 1,
            Self::DuplicateTarget { .. } => 2,
            Self::InvalidSourceRole { .. } => 3,
            Self::ObjectLimitExceeded { .. } => 4,
        }
    }
}

pub(crate) struct PackOverridePreflight {
    pack_identity: PackIdentity,
    targets: Vec<PackOverrideTarget>,
    issues: Vec<PackOverridePreflightIssue>,
    limits: PackOverrideAcquisitionLimits,
}

impl PackOverridePreflight {
    pub(crate) const fn pack_identity(&self) -> PackIdentity {
        self.pack_identity
    }

    pub(crate) fn targets(&self) -> &[PackOverrideTarget] {
        &self.targets
    }

    pub(crate) fn issues(&self) -> &[PackOverridePreflightIssue] {
        &self.issues
    }
}

pub(crate) fn preflight_pack_overrides(
    pack: &Pack,
    sources: impl IntoIterator<Item = PackOverrideSource>,
    limits: PackOverrideAcquisitionLimits,
) -> PackOverridePreflight {
    let mut source_count = 0u64;
    let mut source_count_overflowed = false;
    let mut object_limit_exceeded = false;
    let mut issues = Vec::new();
    let mut targets = Vec::new();
    let mut canonical_paths = BTreeSet::new();
    let mut duplicate_paths = BTreeSet::new();
    let mut missing_paths = BTreeSet::new();

    for configured in sources {
        match source_count.checked_add(1) {
            Some(count) => source_count = count,
            None => source_count_overflowed = true,
        }
        if source_count_overflowed || source_count > limits.objects() {
            object_limit_exceeded = true;
            targets.clear();
        }
        let canonical = match Pack::canonical_project_path(configured.path()) {
            Ok(path) => Some(path),
            Err(message) => {
                issues.push(PackOverridePreflightIssue::InvalidPath {
                    supplied: configured.path.clone(),
                    source: PackOverrideSetError::InvalidProjectPath {
                        path: configured.path.clone(),
                        message,
                    },
                });
                None
            }
        };

        let issue_path = canonical.as_deref().unwrap_or(configured.path());
        if let Err(source) = configured.source.require_object() {
            issues.push(PackOverridePreflightIssue::InvalidSourceRole {
                path: issue_path.to_owned(),
                location: configured.source.clone(),
                source,
            });
        }

        if let Some(path) = canonical {
            if !canonical_paths.insert(path.clone()) {
                duplicate_paths.insert(path.clone());
            }
            if pack.file(&path).is_none() {
                missing_paths.insert(path.clone());
            }
            if !object_limit_exceeded {
                targets.push(PackOverrideTarget {
                    path,
                    source: configured.source,
                    resolved: None,
                });
            }
        }
    }

    issues.extend(
        duplicate_paths
            .into_iter()
            .map(|path| PackOverridePreflightIssue::DuplicateTarget { path }),
    );
    issues.extend(
        missing_paths
            .into_iter()
            .map(|path| PackOverridePreflightIssue::MissingTarget { path }),
    );

    if object_limit_exceeded {
        issues.push(PackOverridePreflightIssue::ObjectLimitExceeded {
            ceiling: limits.objects(),
            observed_at_least: limits.objects().saturating_add(1),
        });
    }

    issues.sort_by(|left, right| match (left.path(), right.path()) {
        (Some(left_path), Some(right_path)) => left_path
            .cmp(right_path)
            .then_with(|| left.rank().cmp(&right.rank()))
            .then_with(|| match (left, right) {
                (
                    PackOverridePreflightIssue::InvalidSourceRole { location: left, .. },
                    PackOverridePreflightIssue::InvalidSourceRole {
                        location: right, ..
                    },
                ) => left.cmp(right),
                _ => std::cmp::Ordering::Equal,
            }),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.rank().cmp(&right.rank()),
    });

    if issues.is_empty() {
        targets.sort_by(|left, right| left.path.cmp(&right.path));
    } else {
        targets.clear();
    }

    PackOverridePreflight {
        pack_identity: pack.identity(),
        targets,
        issues,
        limits,
    }
}

#[derive(Clone)]
pub(crate) struct PackOverrideTarget {
    path: String,
    source: Location,
    resolved: Option<ResolvedOperator>,
}

impl PackOverrideTarget {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }
}

impl fmt::Debug for PackOverrideTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackOverrideTarget")
            .field("path", &self.path)
            .field("source", &self.source)
            .field("resolved", &self.resolved.is_some())
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PackOverrideReservation {
    bytes: u64,
}

impl PackOverrideReservation {
    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }
}

pub(crate) struct PackOverrideAcquisitionEntry {
    path: String,
    source: Location,
    bytes: Vec<u8>,
}

impl PackOverrideAcquisitionEntry {
    fn new(path: String, source: Location, bytes: Vec<u8>) -> Self {
        Self {
            path,
            source,
            bytes,
        }
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn into_parts(self) -> (String, Location, Vec<u8>) {
        (self.path, self.source, self.bytes)
    }
}

impl fmt::Debug for PackOverrideAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackOverrideAcquisitionEntry")
            .field("path", &self.path)
            .field("source", &self.source)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

pub(crate) struct PackOverrideRole<E> {
    targets: Vec<PackOverrideTarget>,
    limits: PackOverrideAcquisitionLimits,
    marker: PhantomData<fn() -> E>,
}

impl<E> PackOverrideRole<E> {
    pub(crate) const fn total_bytes(&self) -> u64 {
        self.limits.total_bytes()
    }

    pub(crate) fn total_bytes_exhausted(
        &self,
        target: &PackOverrideTarget,
    ) -> PackOverrideAcquisitionError<E> {
        let ceiling = self.limits.total_bytes();
        PackOverrideAcquisitionError {
            path: target.path.clone(),
            source_location: target.source.clone(),
            cause: PackOverrideAcquisitionErrorCause::Limit(
                PackOverrideAcquisitionLimitError::Exceeded {
                    resource: PackOverrideAcquisitionResource::TotalBytes,
                    ceiling,
                    observed_at_least: ceiling + 1,
                },
            ),
        }
    }
}

impl<E> fmt::Debug for PackOverrideRole<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackOverrideRole")
            .field("targets", &self.targets)
            .field("limits", &self.limits)
            .finish()
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn prepare_pack_override_role<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    mut preflight: PackOverridePreflight,
) -> Result<PackOverrideRole<R::Error>, PackOverrideAcquisitionError<R::Error>> {
    debug_assert!(preflight.issues.is_empty());
    for target in &mut preflight.targets {
        let appraised = resolved
            .resolve(target.source.binding())
            .map_err(|source| PackOverrideAcquisitionError {
                path: target.path.clone(),
                source_location: target.source.clone(),
                cause: PackOverrideAcquisitionErrorCause::ResolveOperator(source),
            })?;
        if !appraised.read {
            return Err(PackOverrideAcquisitionError {
                path: target.path.clone(),
                source_location: target.source.clone(),
                cause: PackOverrideAcquisitionErrorCause::ReadUnsupported,
            });
        }
        target.resolved = Some(appraised);
    }

    Ok(PackOverrideRole {
        targets: preflight.targets,
        limits: preflight.limits,
        marker: PhantomData,
    })
}

impl<E> AcquisitionRole for PackOverrideRole<E> {
    type Target = PackOverrideTarget;
    type Reservation = PackOverrideReservation;
    type RawResult = PackOverrideAcquisitionEntry;
    type Failure = PackOverrideAcquisitionError<E>;
    type Acquire<'a>
        = Pin<Box<dyn Future<Output = Result<Self::RawResult, Self::Failure>> + Send + 'a>>
    where
        Self: 'a;

    fn targets(&self) -> &[Self::Target] {
        &self.targets
    }

    fn reserve(&mut self, _: &Self::Target) -> Result<Self::Reservation, Self::Failure> {
        Ok(PackOverrideReservation {
            bytes: self.limits.object_bytes(),
        })
    }

    fn acquire<'a>(
        &'a self,
        target: &'a Self::Target,
        reservation: Self::Reservation,
    ) -> Self::Acquire<'a> {
        Box::pin(async move {
            let resolved = target
                .resolved
                .as_ref()
                .expect("Pack Override targets are appraised before scheduling");
            let bytes = acquire_exact_path(
                &resolved.operator,
                target.source.dispatch_path(),
                reservation.bytes(),
                self.limits.object_bytes(),
            )
            .await
            .map_err(|error| PackOverrideAcquisitionError {
                path: target.path.clone(),
                source_location: target.source.clone(),
                cause: map_exact_path_error(error),
            })?;

            Ok(PackOverrideAcquisitionEntry::new(
                target.path.clone(),
                target.source.clone(),
                bytes,
            ))
        })
    }
}

fn map_exact_path_error<E>(
    error: ExactPathAcquisitionError,
) -> PackOverrideAcquisitionErrorCause<E> {
    match error {
        ExactPathAcquisitionError::ObjectAbsent(source) => {
            PackOverrideAcquisitionErrorCause::ObjectAbsent(source)
        }
        ExactPathAcquisitionError::Read(source) => PackOverrideAcquisitionErrorCause::Read(source),
        ExactPathAcquisitionError::Limit(source) => {
            PackOverrideAcquisitionErrorCause::Limit(match source {
                ExactObjectLimitError::Exceeded {
                    ceiling,
                    observed_at_least,
                } => PackOverrideAcquisitionLimitError::Exceeded {
                    resource: PackOverrideAcquisitionResource::ObjectBytes,
                    ceiling,
                    observed_at_least,
                },
                ExactObjectLimitError::AccountingOverflow => {
                    PackOverrideAcquisitionLimitError::AccountingOverflow {
                        resource: PackOverrideAcquisitionResource::ObjectBytes,
                    }
                }
            })
        }
    }
}

pub(crate) struct PackOverrideAcquisitionError<E> {
    path: String,
    source_location: Location,
    cause: PackOverrideAcquisitionErrorCause<E>,
}

impl<E> PackOverrideAcquisitionError<E> {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn source_location(&self) -> &Location {
        &self.source_location
    }

    pub(crate) fn cause(&self) -> &PackOverrideAcquisitionErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for PackOverrideAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Pack Override Acquisition failed for project path {:?}, binding {}, and exact-object operation path {:?}: {}",
            self.path,
            self.source_location.binding(),
            self.source_location.operation_path(),
            self.cause.label(),
        )
    }
}

impl<E> fmt::Debug for PackOverrideAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackOverrideAcquisitionError")
            .field("path", &self.path)
            .field("binding", self.source_location.binding())
            .field("role", &"exact object")
            .field("operation_path", &self.source_location.operation_path())
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackOverrideAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackOverrideAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            PackOverrideAcquisitionErrorCause::ReadUnsupported => None,
            PackOverrideAcquisitionErrorCause::ObjectAbsent(source)
            | PackOverrideAcquisitionErrorCause::Read(source) => Some(source),
            PackOverrideAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PackOverrideAcquisitionErrorCause<E> {
    ResolveOperator(E),
    ReadUnsupported,
    ObjectAbsent(opendal::Error),
    Read(opendal::Error),
    Limit(PackOverrideAcquisitionLimitError),
}

impl<E> PackOverrideAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::ReadUnsupported => "read capability is unsupported",
            Self::ObjectAbsent(_) => "the exact object is absent",
            Self::Read(_) => "the exact object read failed",
            Self::Limit(_) => "the Pack Override byte limit failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PackOverrideConversionError {
    #[error("the acquired Pack Overrides belong to a different Pack")]
    PackMismatch {
        expected: PackIdentity,
        actual: PackIdentity,
    },
    #[error(transparent)]
    PackOverride(#[from] PackOverrideSetError),
}

pub(crate) fn convert_pack_overrides(
    expected: PackIdentity,
    pack: &Pack,
    entries: Vec<PackOverrideAcquisitionEntry>,
) -> Result<PackOverrideSet, PackOverrideConversionError> {
    let actual = pack.identity();
    if actual != expected {
        return Err(PackOverrideConversionError::PackMismatch { expected, actual });
    }

    entries
        .into_iter()
        .try_fold(PackOverrideSet::new(pack), |overrides, entry| {
            overrides
                .replace(entry.path, entry.bytes)
                .map_err(PackOverrideConversionError::PackOverride)
        })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::opendal::acquisition::ResolvedOperators;
    use crate::opendal::acquisition::recursive::AcquisitionRole;
    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, OperationControls, PendingPoint, ReadScript, ReadStep,
        ScriptedService,
    };
    use crate::opendal::{Location, OperatorBinding, OperatorResolver};
    use crate::{Pack, PackOverrideSetError};

    use super::*;

    #[test]
    fn pack_override_limits_are_named_finite_and_internally_consistent() {
        let reference = PackOverrideAcquisitionLimits::reference_v1();
        assert_eq!(reference.objects(), 100_000);
        assert_eq!(reference.object_bytes(), 256 * 1024 * 1024);
        assert_eq!(reference.total_bytes(), 2 * 1024 * 1024 * 1024);

        for resource in [
            PackOverrideAcquisitionResource::ObjectBytes,
            PackOverrideAcquisitionResource::TotalBytes,
        ] {
            let mut ceilings = PackOverrideAcquisitionCeilings::reference_v1();
            match resource {
                PackOverrideAcquisitionResource::ObjectBytes => ceilings.object_bytes = u64::MAX,
                PackOverrideAcquisitionResource::TotalBytes => ceilings.total_bytes = u64::MAX,
                PackOverrideAcquisitionResource::Objects => unreachable!(),
            }
            assert_eq!(
                PackOverrideAcquisitionLimits::new(ceilings),
                Err(PackOverrideAcquisitionLimitsError::CannotProbe {
                    resource,
                    ceiling: u64::MAX,
                })
            );
        }

        assert_eq!(
            PackOverrideAcquisitionLimits::new(PackOverrideAcquisitionCeilings {
                objects: 1,
                object_bytes: 5,
                total_bytes: 4,
            }),
            Err(
                PackOverrideAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: 5,
                    total_bytes: 4,
                }
            )
        );
        assert!(
            PackOverrideAcquisitionLimits::new(PackOverrideAcquisitionCeilings {
                objects: u64::MAX,
                object_bytes: 0,
                total_bytes: 0,
            })
            .is_ok()
        );
    }

    #[test]
    fn preflight_uses_pack_path_authority_and_aggregates_canonical_facts() {
        let pack = pack();
        let limits = limits(3, 8, 16);
        let sources = [
            source("missing.typ", "objects/missing.typ"),
            source("main.typ", "objects/main-a.typ"),
            PackOverrideSource::new("", location("objects/bad.typ")),
            source("./main.typ", "objects/main-b.typ"),
            PackOverrideSource::new("chapter.typ", location("prefix/")),
        ];

        let first = preflight_pack_overrides(&pack, sources.clone(), limits);
        let second = preflight_pack_overrides(&pack, sources.into_iter().rev(), limits);

        assert_eq!(first.issues(), second.issues());
        assert_eq!(
            first.issues(),
            [
                PackOverridePreflightIssue::InvalidPath {
                    supplied: String::new(),
                    source: PackOverrideSetError::InvalidProjectPath {
                        path: String::new(),
                        message: "invalid project file path \"\": \"path must name a root-relative file\""
                            .to_owned(),
                    },
                },
                PackOverridePreflightIssue::InvalidSourceRole {
                    path: "chapter.typ".to_owned(),
                    location: location("prefix/"),
                    source: crate::opendal::LocationRoleError::ObjectHasTrailingSlash,
                },
                PackOverridePreflightIssue::DuplicateTarget {
                    path: "main.typ".to_owned(),
                },
                PackOverridePreflightIssue::MissingTarget {
                    path: "missing.typ".to_owned(),
                },
                PackOverridePreflightIssue::ObjectLimitExceeded {
                    ceiling: 3,
                    observed_at_least: 4,
                },
            ]
        );
        assert!(first.targets().is_empty());
    }

    #[test]
    fn accepted_preflight_targets_are_unique_and_canonical() {
        let pack = pack();
        let preflight = preflight_pack_overrides(
            &pack,
            [
                source("main.typ", "objects/main.typ"),
                source("chapter.typ", "objects/chapter.typ"),
            ],
            limits(2, 8, 16),
        );

        assert!(preflight.issues().is_empty());
        assert_eq!(preflight.pack_identity(), pack.identity());
        assert_eq!(
            preflight
                .targets()
                .iter()
                .map(PackOverrideTarget::path)
                .collect::<Vec<_>>(),
            ["chapter.typ", "main.typ"]
        );
    }

    #[test]
    fn same_target_preflight_issues_follow_documented_variant_order() {
        let preflight = preflight_pack_overrides(
            &pack(),
            [
                PackOverrideSource::new("missing.typ", location("first/")),
                PackOverrideSource::new("missing.typ", location("second/")),
            ],
            limits(2, 8, 16),
        );

        assert!(matches!(
            preflight.issues(),
            [
                PackOverridePreflightIssue::MissingTarget { .. },
                PackOverridePreflightIssue::DuplicateTarget { .. },
                PackOverridePreflightIssue::InvalidSourceRole { .. },
                PackOverridePreflightIssue::InvalidSourceRole { .. },
            ]
        ));
    }

    #[test]
    fn role_resolves_each_binding_once_and_acquires_owned_exact_bytes() {
        let scripts = [
            ReadScript::new(
                "objects/chapter.typ",
                2,
                [ReadStep::chunk(b"owned "), ReadStep::chunk(b"chapter")],
            )
            .unwrap(),
            ReadScript::new("objects/main.typ", 1, [ReadStep::chunk(b"owned main")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [], scripts, 16);
        let resolver = CountingResolver::new(service.operator());
        let pack = pack();
        let preflight = preflight_pack_overrides(
            &pack,
            [
                source("main.typ", "objects/main.typ"),
                source("chapter.typ", "objects/chapter.typ"),
            ],
            limits(2, 16, 32),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_pack_override_role(&mut resolved, preflight).unwrap();

        assert_eq!(resolver.calls(), 1);
        assert_eq!(role.total_bytes(), 32);
        assert!(matches!(
            role.total_bytes_exhausted(&role.targets()[0]).cause(),
            PackOverrideAcquisitionErrorCause::Limit(PackOverrideAcquisitionLimitError::Exceeded {
                resource: PackOverrideAcquisitionResource::TotalBytes,
                ceiling: 32,
                observed_at_least: 33,
            })
        ));
        let mut results = Vec::new();
        for target in role.targets().to_vec() {
            let reservation = role.reserve(&target).unwrap();
            assert_eq!(reservation.bytes(), 16);
            results.push(expect_ready(pin!(role.acquire(&target, reservation))).unwrap());
        }

        assert_eq!(results[0].path(), "chapter.typ");
        assert_eq!(results[0].source(), &location("objects/chapter.typ"));
        assert_eq!(results[0].bytes(), b"owned chapter");
        assert_eq!(results[1].bytes(), b"owned main");
        assert_eq!(
            results
                .into_iter()
                .map(PackOverrideAcquisitionEntry::into_parts)
                .collect::<Vec<_>>(),
            [
                (
                    "chapter.typ".to_owned(),
                    location("objects/chapter.typ"),
                    b"owned chapter".to_vec(),
                ),
                (
                    "main.typ".to_owned(),
                    location("objects/main.typ"),
                    b"owned main".to_vec(),
                ),
            ]
        );
    }

    #[test]
    fn exact_object_failures_keep_target_context_and_typed_causes() {
        let unavailable = ScriptedService::new(
            Capabilities {
                list: false,
                list_with_recursive: false,
                read: false,
            },
            [],
            [],
            4,
        );
        let resolver = CountingResolver::new(unavailable.operator());
        let pack = pack();
        let preflight = preflight_pack_overrides(
            &pack,
            [source("main.typ", "objects/main.typ")],
            limits(1, 4, 4),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let error = prepare_pack_override_role(&mut resolved, preflight).unwrap_err();
        assert_eq!(error.path(), "main.typ");
        assert_eq!(error.source_location(), &location("objects/main.typ"));
        assert!(matches!(
            error.cause(),
            PackOverrideAcquisitionErrorCause::ReadUnsupported
        ));

        let script = ReadScript::new("objects/main.typ", 1, [ReadStep::chunk(b"12345")]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [script], 4);
        let resolver = CountingResolver::new(service.operator());
        let preflight = preflight_pack_overrides(
            &pack,
            [source("main.typ", "objects/main.typ")],
            limits(1, 4, 4),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_pack_override_role(&mut resolved, preflight).unwrap();
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target).unwrap();
        let error = expect_ready(pin!(role.acquire(&target, reservation))).unwrap_err();
        assert!(matches!(
            error.cause(),
            PackOverrideAcquisitionErrorCause::Limit(PackOverrideAcquisitionLimitError::Exceeded {
                resource: PackOverrideAcquisitionResource::ObjectBytes,
                ceiling: 4,
                observed_at_least: 5,
            })
        ));
    }

    #[test]
    fn resolver_failures_remain_typed_while_outer_diagnostics_are_safe() {
        let pack = pack();
        let preflight = preflight_pack_overrides(
            &pack,
            [source("main.typ", "objects/main.typ")],
            limits(1, 4, 4),
        );
        let resolver = RejectingResolver;
        let mut resolved = ResolvedOperators::new(&resolver);
        let error = prepare_pack_override_role(&mut resolved, preflight).unwrap_err();

        assert!(matches!(
            error.cause(),
            PackOverrideAcquisitionErrorCause::ResolveOperator(ResolveFailure)
        ));
        assert!(!error.to_string().contains("secret endpoint"));
        assert!(!format!("{error:?}").contains("secret endpoint"));
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "secret endpoint"
        );
    }

    #[test]
    fn absence_and_post_yield_failure_remain_distinct() {
        let pack = pack();
        let absent = ScriptedService::new(Capabilities::all(), [], [], 4);
        let resolver = CountingResolver::new(absent.operator());
        let error = acquire_one(&pack, &resolver, 8).unwrap_err();
        assert!(matches!(
            error.cause(),
            PackOverrideAcquisitionErrorCause::ObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let script = ReadScript::new(
            "objects/main.typ",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::failure(ErrorKind::NotFound),
            ],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [script], 4);
        let resolver = CountingResolver::new(service.operator());
        let error = acquire_one(&pack, &resolver, 8).unwrap_err();
        assert!(matches!(
            error.cause(),
            PackOverrideAcquisitionErrorCause::Read(source)
                if source.kind() == ErrorKind::NotFound
        ));
    }

    #[test]
    fn dropping_a_role_read_discards_bytes_and_the_scheduler_owned_reservation() {
        let controls = OperationControls::new();
        let held = controls.hold_read(0);
        let pending = PendingPoint::new();
        let script = ReadScript::new(
            "objects/main.typ",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::pending(pending.clone()),
            ],
        )
        .unwrap();
        let service =
            ScriptedService::new_controlled(Capabilities::all(), [], [script], controls, 8);
        let resolver = CountingResolver::new(service.operator());
        let pack = pack();
        let preflight = preflight_pack_overrides(
            &pack,
            [source("main.typ", "objects/main.typ")],
            limits(1, 8, 8),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_pack_override_role(&mut resolved, preflight).unwrap();
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target).unwrap();
        assert_eq!(reservation.bytes(), 8);

        {
            let mut acquisition = pin!(role.acquire(&target, reservation));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            held.release();
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(
            service.cancellations(),
            [DroppedOperation::Read {
                id: 0,
                path: "objects/main.typ".to_owned(),
            }]
        );
    }

    #[test]
    fn conversion_is_pack_bound_and_preserves_authoritative_override_errors() {
        let expected_pack = pack();
        let other_pack = Pack::builder("main.typ")
            .file("main.typ", b"other".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let entry = PackOverrideAcquisitionEntry::new(
            "main.typ".to_owned(),
            location("objects/main.typ"),
            b"replacement".to_vec(),
        );

        let mismatch =
            convert_pack_overrides(expected_pack.identity(), &other_pack, vec![entry]).unwrap_err();
        assert_eq!(
            mismatch,
            PackOverrideConversionError::PackMismatch {
                expected: expected_pack.identity(),
                actual: other_pack.identity(),
            }
        );

        let duplicate = convert_pack_overrides(
            expected_pack.identity(),
            &expected_pack,
            vec![
                PackOverrideAcquisitionEntry::new(
                    "main.typ".to_owned(),
                    location("objects/one.typ"),
                    b"one".to_vec(),
                ),
                PackOverrideAcquisitionEntry::new(
                    "main.typ".to_owned(),
                    location("objects/two.typ"),
                    b"two".to_vec(),
                ),
            ],
        )
        .unwrap_err();
        assert_eq!(
            duplicate,
            PackOverrideConversionError::PackOverride(PackOverrideSetError::DuplicateProjectPath {
                path: "main.typ".to_owned(),
            })
        );
    }

    #[test]
    fn memory_backend_acquires_exact_override_bytes_without_listing() {
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        expect_ready(pin!(
            operator.write("objects/main.typ", b"memory override".to_vec())
        ))
        .unwrap();
        let resolver = CountingResolver::new(operator);
        let entry = acquire_one(&pack(), &resolver, 32).unwrap();

        assert_eq!(entry.bytes(), b"memory override");
        assert_eq!(resolver.calls(), 1);
    }

    #[allow(clippy::result_large_err)]
    fn acquire_one(
        pack: &Pack,
        resolver: &CountingResolver,
        ceiling: u64,
    ) -> Result<PackOverrideAcquisitionEntry, PackOverrideAcquisitionError<Infallible>> {
        let preflight = preflight_pack_overrides(
            pack,
            [source("main.typ", "objects/main.typ")],
            limits(1, ceiling, ceiling),
        );
        let mut resolved = ResolvedOperators::new(resolver);
        let mut role = prepare_pack_override_role(&mut resolved, preflight)?;
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target)?;
        expect_ready(pin!(role.acquire(&target, reservation)))
    }

    fn pack() -> Pack {
        Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .file("chapter.typ", b"chapter".to_vec())
            .unwrap()
            .build()
            .unwrap()
    }

    fn limits(objects: u64, object_bytes: u64, total_bytes: u64) -> PackOverrideAcquisitionLimits {
        PackOverrideAcquisitionLimits::new(PackOverrideAcquisitionCeilings {
            objects,
            object_bytes,
            total_bytes,
        })
        .unwrap()
    }

    fn source(path: &str, operation_path: &str) -> PackOverrideSource {
        PackOverrideSource::new(path, location(operation_path))
    }

    fn location(operation_path: &str) -> Location {
        Location::from_operation_path(OperatorBinding::new("overrides").unwrap(), operation_path)
            .unwrap()
    }

    fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
        match poll_once(future.as_mut()) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
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

    struct RejectingResolver;

    impl OperatorResolver for RejectingResolver {
        type Error = ResolveFailure;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            Err(ResolveFailure)
        }
    }

    #[derive(Debug)]
    struct ResolveFailure;

    impl std::fmt::Display for ResolveFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("secret endpoint")
        }
    }

    impl std::error::Error for ResolveFailure {}
}
