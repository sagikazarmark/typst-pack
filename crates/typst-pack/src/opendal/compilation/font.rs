use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use super::super::acquisition::recursive::AcquisitionRole;
use super::super::acquisition::{
    ExactObjectLimitError, ExactPathAcquisitionError, ResolvedOperator, ResolvedOperators,
    acquire_exact_path,
};
use super::super::{Location, LocationRoleError, OperatorResolver};
use crate::{
    FontContainer, FontContainerError, FontContainerFulfillment, FontContainerIdentity, Pack,
    PackIdentity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompilationFontAcquisitionCeilings {
    pub(crate) containers: u64,
    pub(crate) container_bytes: u64,
    pub(crate) total_bytes: u64,
}

impl CompilationFontAcquisitionCeilings {
    pub(crate) const fn reference_v1() -> Self {
        Self {
            containers: 16_384,
            container_bytes: 256 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilationFontAcquisitionResource {
    Containers,
    ContainerBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum CompilationFontAcquisitionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: CompilationFontAcquisitionResource,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompilationFontAcquisitionLimits {
    ceilings: CompilationFontAcquisitionCeilings,
}

impl CompilationFontAcquisitionLimits {
    pub(crate) fn new(
        ceilings: CompilationFontAcquisitionCeilings,
    ) -> Result<Self, CompilationFontAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                CompilationFontAcquisitionResource::ContainerBytes,
                ceilings.container_bytes,
            ),
            (
                CompilationFontAcquisitionResource::TotalBytes,
                ceilings.total_bytes,
            ),
        ] {
            if ceiling == u64::MAX {
                return Err(CompilationFontAcquisitionLimitsError::CannotProbe {
                    resource,
                    ceiling,
                });
            }
        }
        if ceilings.container_bytes > ceilings.total_bytes {
            return Err(
                CompilationFontAcquisitionLimitsError::ContainerBytesExceedTotalBytes {
                    container_bytes: ceilings.container_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }

        Ok(Self { ceilings })
    }

    pub(crate) const fn reference_v1() -> Self {
        Self {
            ceilings: CompilationFontAcquisitionCeilings::reference_v1(),
        }
    }

    pub(crate) const fn containers(self) -> u64 {
        self.ceilings.containers
    }

    pub(crate) const fn container_bytes(self) -> u64 {
        self.ceilings.container_bytes
    }

    pub(crate) const fn total_bytes(self) -> u64 {
        self.ceilings.total_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FontContainerSource {
    expected_identity: FontContainerIdentity,
    source: Location,
    provenance: Option<String>,
    licensing: Option<String>,
}

impl FontContainerSource {
    pub(crate) fn new(expected_identity: FontContainerIdentity, source: Location) -> Self {
        Self {
            expected_identity,
            source,
            provenance: None,
            licensing: None,
        }
    }

    pub(crate) fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub(crate) fn with_licensing(mut self, licensing: impl Into<String>) -> Self {
        self.licensing = Some(licensing.into());
        self
    }

    pub(crate) const fn expected_identity(&self) -> FontContainerIdentity {
        self.expected_identity
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) fn licensing(&self) -> Option<&str> {
        self.licensing.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilationFontPreflightIssue {
    MissingSource {
        identity: FontContainerIdentity,
    },
    DuplicateSource {
        identity: FontContainerIdentity,
    },
    EmbeddedSource {
        identity: FontContainerIdentity,
    },
    UndeclaredSource {
        identity: FontContainerIdentity,
    },
    InvalidSourceRole {
        identity: FontContainerIdentity,
        location: Location,
        source: LocationRoleError,
    },
    ContainerLimitExceeded {
        ceiling: u64,
        declared: u64,
    },
    ContainerByteLimitExceeded {
        identity: FontContainerIdentity,
        ceiling: u64,
        declared: u64,
    },
    TotalByteLimitExceeded {
        ceiling: u64,
        declared: u64,
    },
}

impl CompilationFontPreflightIssue {
    fn identity(&self) -> Option<FontContainerIdentity> {
        match self {
            Self::MissingSource { identity }
            | Self::DuplicateSource { identity }
            | Self::EmbeddedSource { identity }
            | Self::UndeclaredSource { identity }
            | Self::InvalidSourceRole { identity, .. }
            | Self::ContainerByteLimitExceeded { identity, .. } => Some(*identity),
            Self::ContainerLimitExceeded { .. } | Self::TotalByteLimitExceeded { .. } => None,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::MissingSource { .. } => 0,
            Self::DuplicateSource { .. } => 1,
            Self::EmbeddedSource { .. } => 2,
            Self::UndeclaredSource { .. } => 3,
            Self::InvalidSourceRole { .. } => 4,
            Self::ContainerLimitExceeded { .. } => 5,
            Self::ContainerByteLimitExceeded { .. } => 6,
            Self::TotalByteLimitExceeded { .. } => 7,
        }
    }

    fn is_limit(&self) -> bool {
        matches!(
            self,
            Self::ContainerLimitExceeded { .. }
                | Self::ContainerByteLimitExceeded { .. }
                | Self::TotalByteLimitExceeded { .. }
        )
    }
}

pub(crate) struct CompilationFontPreflight {
    pack_identity: PackIdentity,
    targets: Vec<CompilationFontTarget>,
    issues: Vec<CompilationFontPreflightIssue>,
    limits: CompilationFontAcquisitionLimits,
}

impl CompilationFontPreflight {
    pub(crate) const fn pack_identity(&self) -> PackIdentity {
        self.pack_identity
    }

    pub(crate) fn targets(&self) -> &[CompilationFontTarget] {
        &self.targets
    }

    pub(crate) fn issues(&self) -> &[CompilationFontPreflightIssue] {
        &self.issues
    }
}

#[derive(Clone)]
pub(crate) struct CompilationFontTarget {
    expected_identity: FontContainerIdentity,
    source: Location,
    provenance: Option<String>,
    licensing: Option<String>,
    resolved: Option<ResolvedOperator>,
}

impl CompilationFontTarget {
    pub(crate) const fn expected_identity(&self) -> FontContainerIdentity {
        self.expected_identity
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) fn licensing(&self) -> Option<&str> {
        self.licensing.as_deref()
    }
}

impl fmt::Debug for CompilationFontTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationFontTarget")
            .field("expected_identity", &self.expected_identity)
            .field("source", &self.source)
            .field("provenance", &self.provenance)
            .field("licensing", &self.licensing)
            .field("resolved", &self.resolved.is_some())
            .finish()
    }
}

pub(crate) fn preflight_compilation_fonts(
    pack: &Pack,
    sources: impl IntoIterator<Item = FontContainerSource>,
    limits: CompilationFontAcquisitionLimits,
) -> CompilationFontPreflight {
    let requirements = pack
        .font_requirements()
        .iter()
        .map(|requirement| (requirement.container_identity(), requirement))
        .collect::<BTreeMap<_, _>>();
    let external = requirements
        .iter()
        .filter(|(_, requirement)| !requirement.is_embedded())
        .map(|(identity, requirement)| (*identity, *requirement))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    let mut supplied_external = BTreeSet::new();
    let mut issues = Vec::new();
    let mut targets = Vec::new();

    for configured in sources {
        let identity = configured.expected_identity;
        if !seen.insert(identity) {
            duplicates.insert(identity);
        }
        match requirements.get(&identity) {
            Some(requirement) if requirement.is_embedded() => {
                issues.push(CompilationFontPreflightIssue::EmbeddedSource { identity });
            }
            Some(_) => {
                supplied_external.insert(identity);
                targets.push(CompilationFontTarget {
                    expected_identity: identity,
                    source: configured.source.clone(),
                    provenance: configured.provenance.clone(),
                    licensing: configured.licensing.clone(),
                    resolved: None,
                });
            }
            None => {
                issues.push(CompilationFontPreflightIssue::UndeclaredSource { identity });
            }
        }
        if let Err(source) = configured.source.require_object() {
            issues.push(CompilationFontPreflightIssue::InvalidSourceRole {
                identity,
                location: configured.source,
                source,
            });
        }
    }

    for identity in external.keys() {
        if !supplied_external.contains(identity) {
            issues.push(CompilationFontPreflightIssue::MissingSource {
                identity: *identity,
            });
        }
    }
    issues.extend(
        duplicates
            .into_iter()
            .map(|identity| CompilationFontPreflightIssue::DuplicateSource { identity }),
    );

    let declared_containers = u64::try_from(external.len()).unwrap_or(u64::MAX);
    if declared_containers > limits.containers() {
        issues.push(CompilationFontPreflightIssue::ContainerLimitExceeded {
            ceiling: limits.containers(),
            declared: declared_containers,
        });
    }
    let mut declared_total = 0u64;
    for requirement in external.values() {
        let declared = requirement.container_length();
        declared_total = match declared_total.checked_add(declared) {
            Some(total) => total,
            // TotalBytes is finite, so overflow is already conclusive evidence
            // of this preflight issue even though its API retains one u64 fact.
            None => u64::MAX,
        };
        if declared > limits.container_bytes() {
            issues.push(CompilationFontPreflightIssue::ContainerByteLimitExceeded {
                identity: requirement.container_identity(),
                ceiling: limits.container_bytes(),
                declared,
            });
        }
    }
    if declared_total > limits.total_bytes() {
        issues.push(CompilationFontPreflightIssue::TotalByteLimitExceeded {
            ceiling: limits.total_bytes(),
            declared: declared_total,
        });
    }

    issues.sort_by(|left, right| match (left.is_limit(), right.is_limit()) {
        (false, false) => left
            .identity()
            .cmp(&right.identity())
            .then_with(|| left.rank().cmp(&right.rank()))
            .then_with(|| issue_location_order(left, right)),
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        (true, true) => left
            .rank()
            .cmp(&right.rank())
            .then_with(|| left.identity().cmp(&right.identity())),
    });
    issues.dedup();
    if issues.is_empty() {
        targets.sort_by_key(|target| target.expected_identity);
    } else {
        targets.clear();
    }

    CompilationFontPreflight {
        pack_identity: pack.identity(),
        targets,
        issues,
        limits,
    }
}

fn issue_location_order(
    left: &CompilationFontPreflightIssue,
    right: &CompilationFontPreflightIssue,
) -> Ordering {
    match (left, right) {
        (
            CompilationFontPreflightIssue::InvalidSourceRole { location: left, .. },
            CompilationFontPreflightIssue::InvalidSourceRole {
                location: right, ..
            },
        ) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum CompilationFontAcquisitionLimitError {
    #[error(
        "OpenDAL Compilation Font Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: CompilationFontAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("OpenDAL Compilation Font Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: CompilationFontAcquisitionResource,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompilationFontReservation {
    bytes: u64,
}

impl CompilationFontReservation {
    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }
}

pub(crate) struct FontContainerAcquisitionEntry {
    expected_identity: FontContainerIdentity,
    source: Location,
    provenance: Option<String>,
    licensing: Option<String>,
    bytes: Vec<u8>,
}

impl FontContainerAcquisitionEntry {
    fn new(target: &CompilationFontTarget, bytes: Vec<u8>) -> Self {
        Self {
            expected_identity: target.expected_identity,
            source: target.source.clone(),
            provenance: target.provenance.clone(),
            licensing: target.licensing.clone(),
            bytes,
        }
    }

    pub(crate) const fn expected_identity(&self) -> FontContainerIdentity {
        self.expected_identity
    }

    pub(crate) fn source(&self) -> &Location {
        &self.source
    }

    pub(crate) fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub(crate) fn licensing(&self) -> Option<&str> {
        self.licensing.as_deref()
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        FontContainerIdentity,
        Location,
        Option<String>,
        Option<String>,
        Vec<u8>,
    ) {
        (
            self.expected_identity,
            self.source,
            self.provenance,
            self.licensing,
            self.bytes,
        )
    }
}

impl fmt::Debug for FontContainerAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontContainerAcquisitionEntry")
            .field("expected_identity", &self.expected_identity)
            .field("source", &self.source)
            .field("provenance_present", &self.provenance.is_some())
            .field("licensing_present", &self.licensing.is_some())
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

pub(crate) struct CompilationFontRole<E> {
    targets: Vec<CompilationFontTarget>,
    limits: CompilationFontAcquisitionLimits,
    marker: PhantomData<fn() -> E>,
}

impl<E> CompilationFontRole<E> {
    pub(crate) const fn total_bytes(&self) -> u64 {
        self.limits.total_bytes()
    }

    pub(crate) fn total_bytes_exhausted(
        &self,
        target: &CompilationFontTarget,
    ) -> CompilationFontAcquisitionError<E> {
        let ceiling = self.limits.total_bytes();
        CompilationFontAcquisitionError::for_target(
            target,
            CompilationFontAcquisitionErrorCause::Limit(
                CompilationFontAcquisitionLimitError::Exceeded {
                    resource: CompilationFontAcquisitionResource::TotalBytes,
                    ceiling,
                    observed_at_least: ceiling + 1,
                },
            ),
        )
    }
}

impl<E> fmt::Debug for CompilationFontRole<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationFontRole")
            .field("targets", &self.targets)
            .field("limits", &self.limits)
            .finish()
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn prepare_compilation_font_role<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    mut preflight: CompilationFontPreflight,
) -> Result<CompilationFontRole<R::Error>, CompilationFontAcquisitionError<R::Error>> {
    debug_assert!(preflight.issues.is_empty());
    for target in &mut preflight.targets {
        let appraised = resolved
            .resolve(target.source.binding())
            .map_err(|source| {
                CompilationFontAcquisitionError::for_target(
                    target,
                    CompilationFontAcquisitionErrorCause::ResolveOperator(source),
                )
            })?;
        if !appraised.read {
            return Err(CompilationFontAcquisitionError::for_target(
                target,
                CompilationFontAcquisitionErrorCause::ReadUnsupported,
            ));
        }
        target.resolved = Some(appraised);
    }

    Ok(CompilationFontRole {
        targets: preflight.targets,
        limits: preflight.limits,
        marker: PhantomData,
    })
}

impl<E> AcquisitionRole for CompilationFontRole<E> {
    type Target = CompilationFontTarget;
    type Reservation = CompilationFontReservation;
    type RawResult = FontContainerAcquisitionEntry;
    type Failure = CompilationFontAcquisitionError<E>;
    type Acquire<'a>
        = Pin<Box<dyn Future<Output = Result<Self::RawResult, Self::Failure>> + Send + 'a>>
    where
        Self: 'a;

    fn targets(&self) -> &[Self::Target] {
        &self.targets
    }

    fn reserve(&mut self, _: &Self::Target) -> Result<Self::Reservation, Self::Failure> {
        Ok(CompilationFontReservation {
            bytes: self.limits.container_bytes(),
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
                .expect("Compilation Font targets are appraised before scheduling");
            let bytes = acquire_exact_path(
                &resolved.operator,
                target.source.dispatch_path(),
                reservation.bytes(),
                self.limits.container_bytes(),
            )
            .await
            .map_err(|error| {
                CompilationFontAcquisitionError::for_target(target, map_exact_path_error(error))
            })?;

            Ok(FontContainerAcquisitionEntry::new(target, bytes))
        })
    }
}

fn map_exact_path_error<E>(
    error: ExactPathAcquisitionError,
) -> CompilationFontAcquisitionErrorCause<E> {
    match error {
        ExactPathAcquisitionError::ObjectAbsent(source) => {
            CompilationFontAcquisitionErrorCause::ObjectAbsent(source)
        }
        ExactPathAcquisitionError::Read(source) => {
            CompilationFontAcquisitionErrorCause::Read(source)
        }
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::Exceeded {
            ceiling,
            observed_at_least,
        }) => CompilationFontAcquisitionErrorCause::Limit(
            CompilationFontAcquisitionLimitError::Exceeded {
                resource: CompilationFontAcquisitionResource::ContainerBytes,
                ceiling,
                observed_at_least,
            },
        ),
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow) => {
            CompilationFontAcquisitionErrorCause::Limit(
                CompilationFontAcquisitionLimitError::AccountingOverflow {
                    resource: CompilationFontAcquisitionResource::ContainerBytes,
                },
            )
        }
    }
}

pub(crate) struct CompilationFontAcquisitionError<E> {
    identity: FontContainerIdentity,
    source_location: Location,
    cause: CompilationFontAcquisitionErrorCause<E>,
}

impl<E> CompilationFontAcquisitionError<E> {
    fn for_target(
        target: &CompilationFontTarget,
        cause: CompilationFontAcquisitionErrorCause<E>,
    ) -> Self {
        Self {
            identity: target.expected_identity,
            source_location: target.source.clone(),
            cause,
        }
    }

    pub(crate) const fn identity(&self) -> FontContainerIdentity {
        self.identity
    }

    pub(crate) fn source_location(&self) -> &Location {
        &self.source_location
    }

    pub(crate) fn cause(&self) -> &CompilationFontAcquisitionErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for CompilationFontAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Compilation Font Acquisition failed for Font Container Identity {:?}, binding {}, and exact-object operation path {:?}: {}",
            self.identity,
            self.source_location.binding(),
            self.source_location.operation_path(),
            self.cause.label(),
        )
    }
}

impl<E> fmt::Debug for CompilationFontAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationFontAcquisitionError")
            .field("identity", &self.identity)
            .field("binding", self.source_location.binding())
            .field("role", &"exact object")
            .field("operation_path", &self.source_location.operation_path())
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for CompilationFontAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            CompilationFontAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            CompilationFontAcquisitionErrorCause::ReadUnsupported => None,
            CompilationFontAcquisitionErrorCause::ObjectAbsent(source)
            | CompilationFontAcquisitionErrorCause::Read(source) => Some(source),
            CompilationFontAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CompilationFontAcquisitionErrorCause<E> {
    ResolveOperator(E),
    ReadUnsupported,
    ObjectAbsent(opendal::Error),
    Read(opendal::Error),
    Limit(CompilationFontAcquisitionLimitError),
}

impl<E> CompilationFontAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::ReadUnsupported => "read capability is unsupported",
            Self::ObjectAbsent(_) => "the exact Font Container object is absent",
            Self::Read(_) => "the exact Font Container object read failed",
            Self::Limit(_) => "a Compilation Font Acquisition limit failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("acquired Font Container {identity:?} is invalid: {source}")]
pub(crate) struct CompilationFontConversionError {
    identity: FontContainerIdentity,
    #[source]
    source: FontContainerError,
}

impl CompilationFontConversionError {
    pub(crate) const fn identity(&self) -> FontContainerIdentity {
        self.identity
    }

    pub(crate) const fn cause(&self) -> &FontContainerError {
        &self.source
    }
}

pub(crate) fn convert_compilation_fonts(
    entries: Vec<FontContainerAcquisitionEntry>,
) -> Result<Vec<FontContainerFulfillment>, CompilationFontConversionError> {
    entries
        .into_iter()
        .map(|entry| {
            let container = FontContainer::new(entry.bytes).map_err(|source| {
                CompilationFontConversionError {
                    identity: entry.expected_identity,
                    source,
                }
            })?;
            let mut fulfillment = FontContainerFulfillment::new(entry.expected_identity, container);
            if let Some(provenance) = entry.provenance {
                fulfillment = fulfillment.provenance(provenance);
            }
            if let Some(licensing) = entry.licensing {
                fulfillment = fulfillment.licensing(licensing);
            }
            Ok(fulfillment)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "embedded-fonts")]
    use std::cell::Cell;
    #[cfg(feature = "embedded-fonts")]
    use std::convert::Infallible;
    #[cfg(feature = "embedded-fonts")]
    use std::future::Future;
    #[cfg(feature = "embedded-fonts")]
    use std::pin::pin;
    #[cfg(feature = "embedded-fonts")]
    use std::task::{Context, Poll, Waker};

    #[cfg(feature = "embedded-fonts")]
    use opendal::ErrorKind;

    #[cfg(feature = "embedded-fonts")]
    use crate::opendal::OperatorResolver;
    #[cfg(feature = "embedded-fonts")]
    use crate::opendal::acquisition::ResolvedOperators;
    #[cfg(feature = "embedded-fonts")]
    use crate::opendal::acquisition::recursive::AcquisitionRole;
    #[cfg(feature = "embedded-fonts")]
    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, OperationLogEntry, PendingPoint, ReadScript, ReadStep,
        ScriptedService,
    };
    #[cfg(feature = "embedded-fonts")]
    use crate::opendal::{Location, LocationRoleError, OperatorBinding};

    use super::*;

    #[test]
    fn compilation_font_limits_are_named_finite_and_internally_consistent() {
        let reference = CompilationFontAcquisitionLimits::reference_v1();
        assert_eq!(reference.containers(), 16_384);
        assert_eq!(reference.container_bytes(), 256 * 1024 * 1024);
        assert_eq!(reference.total_bytes(), 2 * 1024 * 1024 * 1024);

        for resource in [
            CompilationFontAcquisitionResource::ContainerBytes,
            CompilationFontAcquisitionResource::TotalBytes,
        ] {
            let mut ceilings = CompilationFontAcquisitionCeilings::reference_v1();
            match resource {
                CompilationFontAcquisitionResource::ContainerBytes => {
                    ceilings.container_bytes = u64::MAX;
                }
                CompilationFontAcquisitionResource::TotalBytes => {
                    ceilings.total_bytes = u64::MAX;
                }
                CompilationFontAcquisitionResource::Containers => unreachable!(),
            }
            assert_eq!(
                CompilationFontAcquisitionLimits::new(ceilings),
                Err(CompilationFontAcquisitionLimitsError::CannotProbe {
                    resource,
                    ceiling: u64::MAX,
                })
            );
        }

        assert_eq!(
            CompilationFontAcquisitionLimits::new(CompilationFontAcquisitionCeilings {
                containers: 1,
                container_bytes: 5,
                total_bytes: 4,
            }),
            Err(
                CompilationFontAcquisitionLimitsError::ContainerBytesExceedTotalBytes {
                    container_bytes: 5,
                    total_bytes: 4,
                }
            )
        );
        assert!(
            CompilationFontAcquisitionLimits::new(CompilationFontAcquisitionCeilings {
                containers: u64::MAX,
                container_bytes: 0,
                total_bytes: 0,
            })
            .is_ok()
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_preflight_aggregates_coverage_roles_duplicates_and_declared_limits() {
        let (pack, _external, embedded) = font_pack();
        let requirements = pack
            .font_requirements()
            .iter()
            .filter(|requirement| !requirement.is_embedded())
            .collect::<Vec<_>>();
        let first_external = requirements[0].container_identity();
        let second_external = requirements[1].container_identity();
        let undeclared = crate::FontContainerIdentity::from_bytes(b"undeclared");
        let declared_total = requirements
            .iter()
            .map(|requirement| requirement.container_length())
            .sum::<u64>();
        let per_container = requirements
            .iter()
            .map(|requirement| requirement.container_length())
            .min()
            .unwrap()
            - 1;
        let sources = [
            FontContainerSource::new(embedded, location("embedded.otf")),
            FontContainerSource::new(undeclared, location("undeclared.otf")),
            FontContainerSource::new(second_external, location("not-an-object/")),
            FontContainerSource::new(second_external, location("duplicate.otf")),
        ];
        let limits = font_limits(1, per_container, declared_total - 1);

        let first = preflight_compilation_fonts(&pack, sources.clone(), limits);
        let second = preflight_compilation_fonts(&pack, sources.into_iter().rev(), limits);

        assert_eq!(first.issues(), second.issues());
        assert!(first.targets().is_empty());
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::MissingSource {
                    identity: first_external,
                })
        );
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::DuplicateSource {
                    identity: second_external,
                })
        );
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::EmbeddedSource { identity: embedded })
        );
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::UndeclaredSource {
                    identity: undeclared,
                })
        );
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::InvalidSourceRole {
                    identity: second_external,
                    location: location("not-an-object/"),
                    source: LocationRoleError::ObjectHasTrailingSlash,
                })
        );
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::ContainerLimitExceeded {
                    ceiling: 1,
                    declared: 2,
                })
        );
        for requirement in requirements {
            assert!(first.issues().contains(
                &CompilationFontPreflightIssue::ContainerByteLimitExceeded {
                    identity: requirement.container_identity(),
                    ceiling: per_container,
                    declared: requirement.container_length(),
                }
            ));
        }
        assert!(
            first
                .issues()
                .contains(&CompilationFontPreflightIssue::TotalByteLimitExceeded {
                    ceiling: declared_total - 1,
                    declared: declared_total,
                })
        );
        let limit_issues = first
            .issues()
            .iter()
            .filter(|issue| issue.is_limit())
            .collect::<Vec<_>>();
        assert!(matches!(
            limit_issues.as_slice(),
            [
                CompilationFontPreflightIssue::ContainerLimitExceeded { .. },
                CompilationFontPreflightIssue::ContainerByteLimitExceeded { .. },
                CompilationFontPreflightIssue::ContainerByteLimitExceeded { .. },
                CompilationFontPreflightIssue::TotalByteLimitExceeded { .. },
            ]
        ));
        assert!(
            limit_issues[1].identity().unwrap() < limit_issues[2].identity().unwrap(),
            "per-container limit issues must be identity-canonical"
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_role_resolves_once_and_acquires_canonical_owned_containers_with_metadata() {
        let (pack, external, _) = font_pack();
        let first_bytes = typst_kit::fonts::embedded()
            .next()
            .unwrap()
            .0
            .data()
            .to_vec();
        let mut second_bytes = first_bytes.clone();
        second_bytes.push(0);
        let scripts = [
            ReadScript::new("objects/first.otf", 1, [ReadStep::chunk(&first_bytes)]).unwrap(),
            ReadScript::new("objects/second.otf", 1, [ReadStep::chunk(&second_bytes)]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [], scripts, 8);
        let resolver = CountingResolver::new(service.operator());
        let sources = [
            FontContainerSource::new(external[1], location("objects/second.otf"))
                .with_provenance("font mirror")
                .with_licensing("caller assertion"),
            FontContainerSource::new(external[0], location("objects/first.otf")),
        ];
        let total = u64::try_from(first_bytes.len() + second_bytes.len()).unwrap();
        let preflight = preflight_compilation_fonts(
            &pack,
            sources,
            font_limits(2, u64::try_from(second_bytes.len()).unwrap(), total),
        );

        assert!(preflight.issues().is_empty());
        assert_eq!(preflight.pack_identity(), pack.identity());
        assert!(
            preflight
                .targets()
                .is_sorted_by_key(|target| target.expected_identity())
        );
        let metadata_target = preflight
            .targets()
            .iter()
            .find(|target| target.expected_identity() == external[1])
            .unwrap();
        assert_eq!(metadata_target.provenance(), Some("font mirror"));
        assert_eq!(metadata_target.licensing(), Some("caller assertion"));

        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_compilation_font_role(&mut resolved, preflight).unwrap();
        assert_eq!(resolver.calls(), 1);
        assert_eq!(role.total_bytes(), total);
        assert!(matches!(
            role.total_bytes_exhausted(&role.targets()[0]).cause(),
            CompilationFontAcquisitionErrorCause::Limit(
                CompilationFontAcquisitionLimitError::Exceeded {
                    resource: CompilationFontAcquisitionResource::TotalBytes,
                    ceiling,
                    observed_at_least,
                }
            ) if *ceiling == total && *observed_at_least == total + 1
        ));

        let mut entries = Vec::new();
        for target in role.targets().to_vec() {
            let reservation = role.reserve(&target).unwrap();
            assert_eq!(
                reservation.bytes(),
                u64::try_from(second_bytes.len()).unwrap()
            );
            entries.push(expect_ready(pin!(role.acquire(&target, reservation))).unwrap());
        }
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            let expected = if entry.expected_identity() == external[0] {
                first_bytes.as_slice()
            } else {
                second_bytes.as_slice()
            };
            assert_eq!(entry.bytes(), expected);
        }
        let metadata = entries
            .iter()
            .find(|entry| entry.expected_identity() == external[1])
            .unwrap();
        assert_eq!(metadata.source(), &location("objects/second.otf"));
        assert_eq!(metadata.provenance(), Some("font mirror"));
        assert_eq!(metadata.licensing(), Some("caller assertion"));
        assert!(!format!("{metadata:?}").contains("caller assertion"));
        assert!(
            !service
                .log()
                .entries()
                .iter()
                .any(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_conversion_preserves_authoritative_container_errors_without_partial_values() {
        let (_, external, _) = font_pack();
        let valid = typst_kit::fonts::embedded()
            .next()
            .unwrap()
            .0
            .data()
            .to_vec();
        let entries = vec![
            FontContainerAcquisitionEntry {
                expected_identity: external[0],
                source: location("objects/valid.otf"),
                provenance: Some("mirror".to_owned()),
                licensing: Some("caller assertion".to_owned()),
                bytes: valid.clone(),
            },
            FontContainerAcquisitionEntry {
                expected_identity: external[1],
                source: location("objects/invalid.otf"),
                provenance: None,
                licensing: None,
                bytes: b"not a font".to_vec(),
            },
        ];

        let error = convert_compilation_fonts(entries).unwrap_err();

        assert_eq!(error.identity(), external[1]);
        assert_eq!(error.cause(), &crate::FontContainerError::NoReadableFace);

        let fulfillment = convert_compilation_fonts(vec![FontContainerAcquisitionEntry {
            expected_identity: external[1],
            source: location("objects/mismatched.otf"),
            provenance: Some("mirror".to_owned()),
            licensing: Some("caller assertion".to_owned()),
            bytes: valid,
        }])
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(fulfillment.expected_identity(), external[1]);
        assert_eq!(fulfillment.container().identity(), external[0]);
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn exact_object_failures_keep_font_context_typed_causes_and_safe_diagnostics() {
        let (pack, identity, bytes) = single_font_pack();
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
        let preflight = single_font_preflight(
            &pack,
            identity,
            CompilationFontAcquisitionLimits::reference_v1(),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let error = prepare_compilation_font_role(&mut resolved, preflight).unwrap_err();
        assert_eq!(error.identity(), identity);
        assert_eq!(error.source_location(), &location("objects/font.otf"));
        assert!(matches!(
            error.cause(),
            CompilationFontAcquisitionErrorCause::ReadUnsupported
        ));

        let mut oversized = bytes.clone();
        oversized.push(0);
        let script = ReadScript::new("objects/font.otf", 1, [ReadStep::chunk(&oversized)]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [script], 4);
        let resolver = CountingResolver::new(service.operator());
        let ceiling = u64::try_from(bytes.len()).unwrap();
        let error =
            acquire_one(&pack, identity, &resolver, font_limits(1, ceiling, ceiling)).unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationFontAcquisitionErrorCause::Limit(
                CompilationFontAcquisitionLimitError::Exceeded {
                    resource: CompilationFontAcquisitionResource::ContainerBytes,
                    ceiling: observed_ceiling,
                    observed_at_least,
                }
            ) if *observed_ceiling == ceiling && *observed_at_least == ceiling + 1
        ));

        let rejecting = RejectingResolver;
        let preflight = single_font_preflight(
            &pack,
            identity,
            CompilationFontAcquisitionLimits::reference_v1(),
        );
        let mut resolved = ResolvedOperators::new(&rejecting);
        let error = prepare_compilation_font_role(&mut resolved, preflight).unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationFontAcquisitionErrorCause::ResolveOperator(ResolveFailure)
        ));
        assert!(!error.to_string().contains("secret endpoint"));
        assert!(!format!("{error:?}").contains("secret endpoint"));
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "secret endpoint"
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn absence_post_yield_failure_and_dropped_reads_remain_distinct() {
        let (pack, identity, bytes) = single_font_pack();
        let absent = ScriptedService::new(Capabilities::all(), [], [], 4);
        let resolver = CountingResolver::new(absent.operator());
        let error = acquire_one(
            &pack,
            identity,
            &resolver,
            CompilationFontAcquisitionLimits::reference_v1(),
        )
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationFontAcquisitionErrorCause::ObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let failing = ReadScript::new(
            "objects/font.otf",
            1,
            [
                ReadStep::chunk(&bytes[..16]),
                ReadStep::failure(ErrorKind::NotFound),
            ],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [failing], 4);
        let resolver = CountingResolver::new(service.operator());
        let error = acquire_one(
            &pack,
            identity,
            &resolver,
            CompilationFontAcquisitionLimits::reference_v1(),
        )
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            CompilationFontAcquisitionErrorCause::Read(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let pending = PendingPoint::new();
        let held = ReadScript::new(
            "objects/font.otf",
            1,
            [
                ReadStep::chunk(&bytes[..16]),
                ReadStep::pending(pending.clone()),
            ],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [held], 4);
        let resolver = CountingResolver::new(service.operator());
        let preflight = single_font_preflight(
            &pack,
            identity,
            CompilationFontAcquisitionLimits::reference_v1(),
        );
        let mut resolved = ResolvedOperators::new(&resolver);
        let mut role = prepare_compilation_font_role(&mut resolved, preflight).unwrap();
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target).unwrap();
        {
            let mut acquisition = pin!(role.acquire(&target, reservation));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }
        assert_eq!(
            service.cancellations(),
            [DroppedOperation::Read {
                id: 0,
                path: "objects/font.otf".to_owned(),
            }]
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn memory_backend_acquires_and_converts_an_exact_font_container() {
        let (pack, identity, bytes) = single_font_pack();
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        expect_ready(pin!(operator.write("objects/font.otf", bytes.clone()))).unwrap();
        let resolver = CountingResolver::new(operator);

        let entry = acquire_one(
            &pack,
            identity,
            &resolver,
            CompilationFontAcquisitionLimits::reference_v1(),
        )
        .unwrap();
        assert_eq!(entry.bytes(), bytes);
        let fulfillment = convert_compilation_fonts(vec![entry])
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(fulfillment.expected_identity(), identity);
        assert_eq!(fulfillment.container().data(), bytes);
    }

    #[cfg(feature = "embedded-fonts")]
    fn font_pack() -> (
        crate::Pack,
        Vec<crate::FontContainerIdentity>,
        crate::FontContainerIdentity,
    ) {
        let base = typst_kit::fonts::embedded()
            .next()
            .unwrap()
            .0
            .data()
            .to_vec();
        let mut second = base.clone();
        second.push(0);
        let mut embedded = base.clone();
        embedded.extend_from_slice(&[0, 0]);
        let external = vec![
            crate::FontContainerIdentity::from_bytes(&base),
            crate::FontContainerIdentity::from_bytes(&second),
        ];
        let embedded_identity = crate::FontContainerIdentity::from_bytes(&embedded);
        let pack = crate::Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .external_font(base, 0)
            .unwrap()
            .external_font(second, 0)
            .unwrap()
            .font(embedded, 0)
            .unwrap()
            .build()
            .unwrap();
        (pack, external, embedded_identity)
    }

    #[cfg(feature = "embedded-fonts")]
    fn single_font_pack() -> (crate::Pack, crate::FontContainerIdentity, Vec<u8>) {
        let bytes = typst_kit::fonts::embedded()
            .next()
            .unwrap()
            .0
            .data()
            .to_vec();
        let identity = crate::FontContainerIdentity::from_bytes(&bytes);
        let pack = crate::Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .external_font(bytes.clone(), 0)
            .unwrap()
            .build()
            .unwrap();
        (pack, identity, bytes)
    }

    #[cfg(feature = "embedded-fonts")]
    fn single_font_preflight(
        pack: &crate::Pack,
        identity: crate::FontContainerIdentity,
        limits: CompilationFontAcquisitionLimits,
    ) -> CompilationFontPreflight {
        let preflight = preflight_compilation_fonts(
            pack,
            [FontContainerSource::new(
                identity,
                location("objects/font.otf"),
            )],
            limits,
        );
        assert!(preflight.issues().is_empty());
        preflight
    }

    #[cfg(feature = "embedded-fonts")]
    #[allow(clippy::result_large_err)]
    fn acquire_one(
        pack: &crate::Pack,
        identity: crate::FontContainerIdentity,
        resolver: &CountingResolver,
        limits: CompilationFontAcquisitionLimits,
    ) -> Result<FontContainerAcquisitionEntry, CompilationFontAcquisitionError<Infallible>> {
        let preflight = single_font_preflight(pack, identity, limits);
        let mut resolved = ResolvedOperators::new(resolver);
        let mut role = prepare_compilation_font_role(&mut resolved, preflight)?;
        let target = role.targets()[0].clone();
        let reservation = role.reserve(&target)?;
        expect_ready(pin!(role.acquire(&target, reservation)))
    }

    #[cfg(feature = "embedded-fonts")]
    fn font_limits(
        containers: u64,
        container_bytes: u64,
        total_bytes: u64,
    ) -> CompilationFontAcquisitionLimits {
        CompilationFontAcquisitionLimits::new(CompilationFontAcquisitionCeilings {
            containers,
            container_bytes,
            total_bytes,
        })
        .unwrap()
    }

    #[cfg(feature = "embedded-fonts")]
    fn location(path: &str) -> Location {
        Location::from_operation_path(OperatorBinding::new("fonts").unwrap(), path).unwrap()
    }

    #[cfg(feature = "embedded-fonts")]
    fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
        match poll_once(future.as_mut()) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    #[cfg(feature = "embedded-fonts")]
    fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    #[cfg(feature = "embedded-fonts")]
    struct CountingResolver {
        calls: Cell<usize>,
        operator: opendal::Operator,
    }

    #[cfg(feature = "embedded-fonts")]
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

    #[cfg(feature = "embedded-fonts")]
    impl OperatorResolver for CountingResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.operator.clone())
        }
    }

    #[cfg(feature = "embedded-fonts")]
    struct RejectingResolver;

    #[cfg(feature = "embedded-fonts")]
    impl OperatorResolver for RejectingResolver {
        type Error = ResolveFailure;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            Err(ResolveFailure)
        }
    }

    #[cfg(feature = "embedded-fonts")]
    #[derive(Debug)]
    struct ResolveFailure;

    #[cfg(feature = "embedded-fonts")]
    impl std::fmt::Display for ResolveFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("secret endpoint")
        }
    }

    #[cfg(feature = "embedded-fonts")]
    impl std::error::Error for ResolveFailure {}
}
