//! PROTOTYPE: compile-checked public interface fixture for typst-pack 0.4.
//!
//! This is a throwaway design artifact, not production code. Bodies are omitted
//! deliberately; the fixture checks module placement, ownership, type flow, and
//! stable-Rust trait shapes without creating a second implementation.

#![allow(dead_code)]

use std::sync::Arc;
use std::task::{Context, Poll};

pub use compilation::{
    CompilationOperationOutcome, CompilationReport, CompilationResult, CompilationTerminal,
    PreparedCompilation,
};
pub use pack::{Pack, PackIdentity, PackInspection};
pub use session::CompilationSession;
pub use transport::StableByteValue;

macro_rules! opaque_value {
    ($vis:vis $name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        $vis struct $name(Arc<str>);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

opaque_value!(pub CanonicalIdentity);
opaque_value!(pub ContentIdentity);
opaque_value!(pub ProjectPath);
opaque_value!(pub ClosureExportPath);
opaque_value!(pub PackageSpecification);
opaque_value!(pub PackagePath);
opaque_value!(pub TypstInputKey);
opaque_value!(pub TypstInputValue);
opaque_value!(pub FontContainerIdentity);
opaque_value!(pub EngineIdentity);
opaque_value!(pub ExporterIdentity);
opaque_value!(pub CompilationIdentity);
opaque_value!(pub CompilationResultIdentity);
opaque_value!(pub CacheIsolationDomain);
opaque_value!(pub AuthorityInstanceIdentity);
opaque_value!(pub ResourceProfileIdentity);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompletePackageTreeIdentity(Arc<str>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainValueRejection {
    Empty,
    InvalidSyntax,
    InvalidNamespace,
    NonCanonical,
}

impl ProjectPath {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl PackageSpecification {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl PackagePath {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl ClosureExportPath {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl TypstInputKey {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl TypstInputValue {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl ContentIdentity {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl CanonicalIdentity {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl CompletePackageTreeIdentity {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FontContainerIdentity {
    pub fn parse(
        _admission: &OrdinaryAdmission,
        _value: &str,
    ) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl AuthorityInstanceIdentity {
    pub fn try_new(_namespaced_value: &str) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl CacheIsolationDomain {
    pub fn try_new(_opaque_value: &str) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

impl ResourceProfileIdentity {
    pub fn try_new(_name: &str, _version: u32) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeploymentTrustProfile {
    Trusted,
    PartiallyTrusted,
    Hostile,
}

/// Admission to the ordinary in-process and ordinary isolated interfaces.
///
/// Construction refuses Hostile before an operation receives input bytes.
#[derive(Clone, Debug)]
pub struct OrdinaryAdmission {
    requested: DeploymentTrustProfile,
    admitted: DeploymentTrustProfile,
}

impl OrdinaryAdmission {
    pub fn try_new(requested: DeploymentTrustProfile) -> Result<Self, AdmissionRefusal> {
        match requested {
            DeploymentTrustProfile::Trusted | DeploymentTrustProfile::PartiallyTrusted => {
                Ok(Self {
                    requested,
                    admitted: requested,
                })
            }
            DeploymentTrustProfile::Hostile => {
                Err(AdmissionRefusal::HostileUnavailableInFirstRelease)
            }
        }
    }

    pub fn requested(&self) -> DeploymentTrustProfile {
        self.requested
    }

    pub fn admitted(&self) -> DeploymentTrustProfile {
        self.admitted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    HostileUnavailableInFirstRelease,
    MissingEnforcementCapability,
    InvalidResourceLimits,
    CapacityUnavailable,
}

pub trait OperationLimitSet: private::SealedLimitSet + Clone {}

/// Reusable immutable configuration. Consumption counters are newly allocated
/// by every operation and are never retained here.
#[derive(Clone, Debug)]
pub struct AdmittedOperationResourceLimits<L> {
    profile: Option<ResourceProfileIdentity>,
    requested: Arc<L>,
    admitted: Arc<L>,
}

impl<L: OperationLimitSet> AdmittedOperationResourceLimits<L> {
    pub fn try_from_adapter_profile(
        profile: ResourceProfileIdentity,
        requested: L,
        admitted: L,
    ) -> Result<Self, AdmissionRefusal> {
        requested.validate()?;
        admitted.validate()?;
        if !admitted.no_looser_than(&requested) {
            return Err(AdmissionRefusal::InvalidResourceLimits);
        }

        Ok(Self {
            profile: Some(profile),
            requested: Arc::new(requested),
            admitted: Arc::new(admitted),
        })
    }

    pub fn profile(&self) -> Option<&ResourceProfileIdentity> {
        self.profile.as_ref()
    }

    pub fn requested(&self) -> &L {
        &self.requested
    }

    pub fn admitted(&self) -> &L {
        &self.admitted
    }
}

impl<L: OperationLimitSet> AdmittedOperationResourceLimits<L> {
    pub fn try_caller_selected(limits: L) -> Result<Self, AdmissionRefusal> {
        limits.validate()?;
        Ok(Self {
            profile: None,
            requested: Arc::new(limits.clone()),
            admitted: Arc::new(limits),
        })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTimeDomain(Arc<str>);

impl MonotonicTimeDomain {
    pub fn try_new(_namespaced_value: &str) -> Result<Self, DomainValueRejection> {
        unimplemented!()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonotonicInstant {
    domain: MonotonicTimeDomain,
    ticks: u64,
}

impl MonotonicInstant {
    pub fn try_new(domain: MonotonicTimeDomain, ticks: u64) -> Self {
        Self { domain, ticks }
    }

    pub fn domain(&self) -> &MonotonicTimeDomain {
        &self.domain
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn checked_add_ticks(&self, ticks: u64) -> Option<Self> {
        Some(Self {
            domain: self.domain.clone(),
            ticks: self.ticks.checked_add(ticks)?,
        })
    }
}

pub trait MonotonicClock {
    fn domain(&self) -> &MonotonicTimeDomain;
    fn now(&self) -> MonotonicInstant;
    fn poll_at(&self, deadline: MonotonicInstant, context: &mut Context<'_>) -> Poll<()>;
}

pub trait InterruptionSource {
    fn interrupted(&self) -> bool;
    fn poll_interrupted(&self, context: &mut Context<'_>) -> Poll<()>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationDeadline {
    None,
    At(MonotonicInstant),
}

pub mod transport {
    use super::{
        AdmissionRefusal, AdmittedOperationResourceLimits, ContentIdentity, InterruptionSource,
        MonotonicClock, OperationDeadline, OrdinaryAdmission,
    };
    use std::future::Future;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct StableByteValue(Arc<crate::private::StableBacking>);

    impl StableByteValue {
        pub fn from_vec(_admission: &OrdinaryAdmission, _bytes: Vec<u8>) -> Self {
            unimplemented!()
        }

        pub fn from_arc(_admission: &OrdinaryAdmission, _bytes: Arc<[u8]>) -> Self {
            unimplemented!()
        }

        pub fn copy_from_slice(_admission: &OrdinaryAdmission, _bytes: &[u8]) -> Self {
            unimplemented!()
        }

        pub fn from_static(_admission: &OrdinaryAdmission, _bytes: &'static [u8]) -> Self {
            unimplemented!()
        }

        pub fn from_chunks(
            _admission: &OrdinaryAdmission,
            _chunks: impl IntoIterator<Item = Arc<[u8]>>,
        ) -> Result<Self, StableByteValueConstructionError> {
            unimplemented!()
        }

        pub fn len(&self) -> u64 {
            unimplemented!()
        }

        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }

        pub fn content_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }

        pub fn read_exact_at(
            &self,
            _offset: u64,
            _destination: &mut [u8],
        ) -> Result<(), StableReadError> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum StableByteValueConstructionError {
        LengthOverflow,
        TargetCapacityExceeded,
        BackingFinalizationFailed,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum StableReadError {
        OutOfBounds,
        BackingUnavailable,
        InternalIntegrity,
    }

    pub trait SyncByteSource {
        fn read(&mut self, destination: &mut [u8]) -> Result<usize, ByteSourceFailure>;
    }

    pub trait AsyncByteSource {
        type Read<'a>: Future<Output = Result<usize, ByteSourceFailure>> + 'a
        where
            Self: 'a;

        fn read<'a>(&'a mut self, destination: &'a mut [u8]) -> Self::Read<'a>;
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ByteSourceFailure {
        Unavailable,
        ReadFailed,
        AdapterContractViolation,
    }

    #[derive(Clone, Debug)]
    pub struct SpoolResourceLimits {
        source_bytes: u64,
        stable_spool_bytes: u64,
        in_flight_bytes: u64,
        retained_memory_bytes: u64,
    }

    impl SpoolResourceLimits {
        pub fn try_new(
            source_bytes: u64,
            stable_spool_bytes: u64,
            in_flight_bytes: u64,
            retained_memory_bytes: u64,
        ) -> Result<Self, AdmissionRefusal> {
            let limits = Self {
                source_bytes,
                stable_spool_bytes,
                in_flight_bytes,
                retained_memory_bytes,
            };
            crate::private::SealedLimitSet::validate(&limits)?;
            Ok(limits)
        }

        pub fn source_bytes(&self) -> u64 {
            self.source_bytes
        }
        pub fn stable_spool_bytes(&self) -> u64 {
            self.stable_spool_bytes
        }
        pub fn in_flight_bytes(&self) -> u64 {
            self.in_flight_bytes
        }
        pub fn retained_memory_bytes(&self) -> u64 {
            self.retained_memory_bytes
        }
    }

    pub struct SpoolControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<SpoolResourceLimits>,
        expected_identity: Option<ContentIdentity>,
        cleanup: RequestedCleanupStrength,
        deadline: OperationDeadline,
        clock: &'a dyn MonotonicClock,
        interruption: &'a dyn InterruptionSource,
    }

    impl<'a> SpoolControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<SpoolResourceLimits>,
            expected_identity: Option<ContentIdentity>,
            cleanup: RequestedCleanupStrength,
            deadline: OperationDeadline,
            clock: &'a dyn MonotonicClock,
            interruption: &'a dyn InterruptionSource,
        ) -> Result<Self, AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                expected_identity,
                cleanup,
                deadline,
                clock,
                interruption,
            })
        }

        pub fn admission(&self) -> &OrdinaryAdmission {
            &self.admission
        }
        pub fn limits(&self) -> &SpoolResourceLimits {
            self.limits.admitted()
        }
        pub fn expected_identity(&self) -> Option<&ContentIdentity> {
            self.expected_identity.as_ref()
        }
        pub fn requested_cleanup(&self) -> RequestedCleanupStrength {
            self.cleanup
        }
        pub fn deadline(&self) -> &OperationDeadline {
            &self.deadline
        }
        pub fn clock(&self) -> &dyn MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn InterruptionSource {
            self.interruption
        }
    }

    pub trait SyncSpoolFacility {
        fn spool(
            &mut self,
            source: &mut dyn SyncByteSource,
            controls: SpoolControls<'_>,
        ) -> SpoolOutcome;
    }

    pub trait AsyncSpoolFacility {
        type Spool<'a, S>: Future<Output = SpoolOutcome> + 'a
        where
            Self: 'a,
            S: AsyncByteSource + 'a;

        fn spool<'a, S>(
            &'a mut self,
            source: &'a mut S,
            controls: SpoolControls<'a>,
        ) -> Self::Spool<'a, S>
        where
            S: AsyncByteSource + 'a;
    }

    pub struct MemorySpool;

    impl MemorySpool {
        pub fn new() -> Self {
            Self
        }
    }

    impl SyncSpoolFacility for MemorySpool {
        fn spool(
            &mut self,
            _source: &mut dyn SyncByteSource,
            _controls: SpoolControls<'_>,
        ) -> SpoolOutcome {
            unimplemented!()
        }
    }

    impl AsyncSpoolFacility for MemorySpool {
        type Spool<'a, S>
            = std::pin::Pin<Box<dyn Future<Output = SpoolOutcome> + 'a>>
        where
            Self: 'a,
            S: AsyncByteSource + 'a;

        fn spool<'a, S>(
            &'a mut self,
            _source: &'a mut S,
            _controls: SpoolControls<'a>,
        ) -> Self::Spool<'a, S>
        where
            S: AsyncByteSource + 'a,
        {
            unimplemented!()
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub mod native {
        use super::{SpoolControls, SpoolOutcome, SyncByteSource, SyncSpoolFacility};
        use std::path::Path;

        /// Core-owned backing; the adapter selects an operation-private root.
        pub struct NativeSpool {
            private: crate::private::NativeSpoolState,
        }

        impl NativeSpool {
            pub fn try_new(_admitted_parent: &Path) -> Result<Self, NativeSpoolConfigurationError> {
                unimplemented!()
            }

            pub fn spool(
                &mut self,
                _source: &mut dyn SyncByteSource,
                _controls: SpoolControls<'_>,
            ) -> SpoolOutcome {
                unimplemented!()
            }
        }

        impl SyncSpoolFacility for NativeSpool {
            fn spool(
                &mut self,
                source: &mut dyn SyncByteSource,
                controls: SpoolControls<'_>,
            ) -> SpoolOutcome {
                self.spool(source, controls)
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum NativeSpoolConfigurationError {
            RootUnavailable,
            RootNotPrivate,
            UnsupportedTarget,
        }
    }

    pub struct SpoolOutcome {
        terminal: Result<StableByteValue, SpoolFailure>,
        receipt: TransportReceipt,
    }

    impl SpoolOutcome {
        pub fn try_new(
            _terminal: Result<StableByteValue, SpoolFailure>,
            _receipt: TransportReceipt,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }

        pub fn terminal(&self) -> &Result<StableByteValue, SpoolFailure> {
            &self.terminal
        }
        pub fn receipt(&self) -> &TransportReceipt {
            &self.receipt
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SpoolPrimaryFailure {
        Admission(AdmissionRefusal),
        Source(ByteSourceFailure),
        ExpectedIdentityMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        InternalIntegrity,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SpoolFailure {
        pub primary: SpoolPrimaryFailure,
        pub cleanup: TransportCleanupOutcome,
    }

    #[derive(Clone, Debug)]
    pub struct TransportReceipt {
        private: crate::private::TransportReceiptState,
    }

    impl TransportReceipt {
        pub fn try_new_spool(
            _controls: &SpoolControls<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _content_identity: Option<ContentIdentity>,
            _cleanup: TransportCleanupOutcome,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn try_new_transport(
            _controls: &TransportControls<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _content_identity: Option<ContentIdentity>,
            _actual_commit: Option<PublicationCommitStrength>,
            _cleanup: TransportCleanupOutcome,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn try_new_acquisition(
            _controls: &AcquisitionTransportControls<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _content_identity: Option<ContentIdentity>,
            _cleanup: TransportCleanupOutcome,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn stage(&self) -> TransportStage {
            unimplemented!()
        }

        pub fn transferred_bytes(&self) -> u64 {
            unimplemented!()
        }

        pub fn cleanup(&self) -> &TransportCleanupOutcome {
            unimplemented!()
        }
        pub fn content_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn actual_commit(&self) -> Option<PublicationCommitStrength> {
            unimplemented!()
        }
        pub fn requested_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn admitted_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn adapter_class(&self) -> &str {
            unimplemented!()
        }
        pub fn admission(&self) -> TransportReceiptAdmissionView<'_> {
            unimplemented!()
        }
    }

    pub struct TransportReceiptAdmissionView<'a> {
        pub requested_limits: TransportOperationLimitsView<'a>,
        pub admitted_limits: TransportOperationLimitsView<'a>,
        pub requested_commit: Option<PublicationCommitStrength>,
        pub requested_cleanup: RequestedCleanupStrength,
        pub deadline: &'a OperationDeadline,
    }

    pub enum TransportOperationLimitsView<'a> {
        Spool(&'a SpoolResourceLimits),
        Transfer(&'a TransportResourceLimits),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportStage {
        Admitted,
        Transferring,
        Transferred,
        Committed,
        Cleanup,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportReceiptRejection {
        IncoherentStage,
        IncoherentByteCount,
        IncoherentCleanup,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PublicationCommitStrength {
        CompleteCollectionAtomic,
        EachObjectAtomic,
        Streaming,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RequestedCleanupStrength {
        CompleteBeforeReturn,
        ResidualMayBeReported,
        NonRetractableAccepted,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportCleanupOutcome {
        NotRequired,
        Complete,
        ResidualReported { locator: ResidualTransportLocator },
        NonRetractable { exposed_bytes: u64 },
        Failed,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ResidualTransportLocator {
        private: crate::private::ResidualTransportLocatorState,
    }

    impl ResidualTransportLocator {
        pub fn try_new(
            _safe_summary: &str,
            _capability_gated_raw: Arc<[u8]>,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn safe_summary(&self) -> &str {
            unimplemented!()
        }
        pub fn disclose_raw<'a>(
            &'a self,
            _capability: &'a ResidualLocatorDisclosureCapability,
        ) -> &'a [u8] {
            unimplemented!()
        }
    }

    pub struct ResidualLocatorDisclosureCapability {
        private: crate::private::ResidualLocatorDisclosureCapabilityState,
    }

    impl ResidualLocatorDisclosureCapability {
        pub fn explicitly_granted_by_caller() -> Self {
            unimplemented!()
        }
    }

    pub trait SyncPackArchiveAcquirer {
        type Locator: ?Sized;

        fn acquire(
            &self,
            locator: &Self::Locator,
            controls: AcquisitionTransportControls<'_>,
        ) -> TransportOutcome<StableByteValue>;
    }

    pub trait AsyncPackArchiveAcquirer {
        type Locator: ?Sized;
        type Acquire<'a>: Future<Output = TransportOutcome<StableByteValue>> + 'a
        where
            Self: 'a;

        fn acquire<'a>(
            &'a self,
            locator: &'a Self::Locator,
            controls: AcquisitionTransportControls<'a>,
        ) -> Self::Acquire<'a>;
    }

    pub trait SyncCompilationDelivery {
        type Destination: ?Sized;

        fn deliver(
            &self,
            projection: crate::compilation::CompilationReportProjection,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> CompilationDeliveryOutcome;
    }

    pub trait AsyncCompilationDelivery {
        type Destination: ?Sized;
        type Deliver<'a>: Future<Output = CompilationDeliveryOutcome> + 'a
        where
            Self: 'a;

        fn deliver<'a>(
            &'a self,
            projection: crate::compilation::CompilationReportProjection,
            destination: &'a Self::Destination,
            controls: TransportControls<'a>,
        ) -> Self::Deliver<'a>;
    }

    #[derive(Clone, Debug)]
    pub struct TransportResourceLimits {
        objects: u64,
        aggregate_bytes: u64,
        largest_object_bytes: u64,
        in_flight_bytes: u64,
        transfer_concurrency: u64,
    }

    pub struct AcquisitionTransportControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
        expected_identity: Option<ContentIdentity>,
        cleanup: RequestedCleanupStrength,
        deadline: OperationDeadline,
        clock: &'a dyn MonotonicClock,
        interruption: &'a dyn InterruptionSource,
    }

    impl<'a> AcquisitionTransportControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
            expected_identity: Option<ContentIdentity>,
            cleanup: RequestedCleanupStrength,
            deadline: OperationDeadline,
            clock: &'a dyn MonotonicClock,
            interruption: &'a dyn InterruptionSource,
        ) -> Result<Self, AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                expected_identity,
                cleanup,
                deadline,
                clock,
                interruption,
            })
        }

        pub fn admission(&self) -> &OrdinaryAdmission {
            &self.admission
        }
        pub fn limits(&self) -> &TransportResourceLimits {
            self.limits.admitted()
        }
        pub fn expected_identity(&self) -> Option<&ContentIdentity> {
            self.expected_identity.as_ref()
        }
        pub fn requested_cleanup(&self) -> RequestedCleanupStrength {
            self.cleanup
        }
        pub fn deadline(&self) -> &OperationDeadline {
            &self.deadline
        }
        pub fn clock(&self) -> &dyn MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn InterruptionSource {
            self.interruption
        }
    }

    impl TransportResourceLimits {
        pub fn try_new(
            objects: u64,
            aggregate_bytes: u64,
            largest_object_bytes: u64,
            in_flight_bytes: u64,
            transfer_concurrency: u64,
        ) -> Result<Self, AdmissionRefusal> {
            let limits = Self {
                objects,
                aggregate_bytes,
                largest_object_bytes,
                in_flight_bytes,
                transfer_concurrency,
            };
            crate::private::SealedLimitSet::validate(&limits)?;
            Ok(limits)
        }

        pub fn objects(&self) -> u64 {
            self.objects
        }
        pub fn aggregate_bytes(&self) -> u64 {
            self.aggregate_bytes
        }
        pub fn largest_object_bytes(&self) -> u64 {
            self.largest_object_bytes
        }
        pub fn in_flight_bytes(&self) -> u64 {
            self.in_flight_bytes
        }
        pub fn transfer_concurrency(&self) -> u64 {
            self.transfer_concurrency
        }
    }

    pub struct TransportControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
        commit: PublicationCommitStrength,
        cleanup: RequestedCleanupStrength,
        deadline: OperationDeadline,
        clock: &'a dyn MonotonicClock,
        interruption: &'a dyn InterruptionSource,
    }

    impl<'a> TransportControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
            commit: PublicationCommitStrength,
            cleanup: RequestedCleanupStrength,
            deadline: OperationDeadline,
            clock: &'a dyn MonotonicClock,
            interruption: &'a dyn InterruptionSource,
        ) -> Result<Self, AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                commit,
                cleanup,
                deadline,
                clock,
                interruption,
            })
        }

        pub fn admission(&self) -> &OrdinaryAdmission {
            &self.admission
        }
        pub fn limits(&self) -> &TransportResourceLimits {
            self.limits.admitted()
        }
        pub fn requested_commit(&self) -> PublicationCommitStrength {
            self.commit
        }
        pub fn requested_cleanup(&self) -> RequestedCleanupStrength {
            self.cleanup
        }
        pub fn deadline(&self) -> &OperationDeadline {
            &self.deadline
        }
        pub fn clock(&self) -> &dyn MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn InterruptionSource {
            self.interruption
        }
    }

    pub struct TransportOutcome<T> {
        terminal: Result<T, TransportFailure>,
        receipt: TransportReceipt,
    }

    impl<T> TransportOutcome<T> {
        pub fn try_new(
            _terminal: Result<T, TransportFailure>,
            _receipt: TransportReceipt,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }

        pub fn terminal(&self) -> &Result<T, TransportFailure> {
            &self.terminal
        }
        pub fn receipt(&self) -> &TransportReceipt {
            &self.receipt
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportOutcomeRejection {
        SuccessBeforeRequiredStage,
        FailureAfterCommit,
        IncoherentCleanup,
        WrongReport,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportPrimaryFailure {
        Admission(AdmissionRefusal),
        Acquisition,
        Transfer,
        ExpectedIdentityMismatch,
        ResourceLimit,
        Commit,
        Cancelled,
        Deadline,
        AdapterContractViolation,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct TransportFailure {
        pub primary: TransportPrimaryFailure,
        pub cleanup: TransportCleanupOutcome,
    }

    pub struct CompilationDeliveryOutcome {
        report: crate::CompilationReport,
        transport: TransportOutcome<()>,
    }

    impl CompilationDeliveryOutcome {
        pub fn try_new(
            _projection: crate::compilation::CompilationReportProjection,
            _transport: TransportOutcome<()>,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }

        pub fn report(&self) -> &crate::CompilationReport {
            &self.report
        }
        pub fn transport(&self) -> &TransportOutcome<()> {
            &self.transport
        }
    }

    pub trait SyncPackArchivePublisher {
        type Destination: ?Sized;

        fn publish(
            &self,
            archive: &crate::representation::EncodedPackArchive,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> TransportOutcome<()>;
    }

    pub trait SyncProjectMaterializationPublisher {
        type Destination: ?Sized;

        fn publish(
            &self,
            plan: &crate::representation::ProjectMaterializationPlan,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> TransportOutcome<()>;
    }

    pub trait SyncClosureExportPublisher {
        type Destination: ?Sized;

        fn publish(
            &self,
            plan: &crate::representation::ClosureExportPlan,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> TransportOutcome<()>;
    }

    pub trait AsyncPackArchivePublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = TransportOutcome<()>> + 'a
        where
            Self: 'a;

        fn publish<'a>(
            &'a self,
            archive: &'a crate::representation::EncodedPackArchive,
            destination: &'a Self::Destination,
            controls: TransportControls<'a>,
        ) -> Self::Publish<'a>;
    }

    pub trait AsyncProjectMaterializationPublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = TransportOutcome<()>> + 'a
        where
            Self: 'a;

        fn publish<'a>(
            &'a self,
            plan: &'a crate::representation::ProjectMaterializationPlan,
            destination: &'a Self::Destination,
            controls: TransportControls<'a>,
        ) -> Self::Publish<'a>;
    }

    pub trait AsyncClosureExportPublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = TransportOutcome<()>> + 'a
        where
            Self: 'a;

        fn publish<'a>(
            &'a self,
            plan: &'a crate::representation::ClosureExportPlan,
            destination: &'a Self::Destination,
            controls: TransportControls<'a>,
        ) -> Self::Publish<'a>;
    }
}

pub mod pack {
    use super::{CompilationIdentity, DomainValueRejection, OrdinaryAdmission, ProjectPath};
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct Pack(Arc<crate::private::ValidatedPack>);

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct PackIdentity(crate::private::PackIdentityState);

    impl PackIdentity {
        pub fn parse(
            _admission: &OrdinaryAdmission,
            _value: &str,
        ) -> Result<Self, DomainValueRejection> {
            unimplemented!()
        }

        pub fn as_str(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct PackInspection {
        private: crate::private::PackInspectionState,
    }

    impl PackInspection {
        pub fn identity(&self) -> &PackIdentity {
            unimplemented!()
        }

        pub fn entrypoint(&self) -> &ProjectPath {
            unimplemented!()
        }

        pub fn project_files(&self) -> impl ExactSizeIterator<Item = ProjectFileInspection<'_>> {
            std::iter::empty()
        }

        pub fn package_requirements(
            &self,
        ) -> impl ExactSizeIterator<Item = PackageRequirementInspection<'_>> {
            std::iter::empty()
        }

        pub fn font_requirements(
            &self,
        ) -> impl ExactSizeIterator<Item = FontRequirementInspection<'_>> {
            std::iter::empty()
        }

        pub fn discovery_coverage(
            &self,
        ) -> impl ExactSizeIterator<Item = DiscoveryCoverageInspection<'_>> {
            std::iter::empty()
        }

        pub fn guarantees(&self) -> PackGuaranteesInspection {
            unimplemented!()
        }
    }

    pub struct ProjectFileInspection<'a> {
        pub path: &'a ProjectPath,
        pub content_identity: &'a crate::ContentIdentity,
        pub exact_bytes: u64,
    }

    pub struct PackageRequirementInspection<'a> {
        pub specification: &'a crate::PackageSpecification,
        pub tree_identity: &'a crate::CompletePackageTreeIdentity,
        pub embedded: bool,
    }

    pub struct FontRequirementInspection<'a> {
        pub container_identity: &'a crate::FontContainerIdentity,
        pub embedded: bool,
    }

    pub struct DiscoveryCoverageInspection<'a> {
        pub identity: &'a crate::CanonicalIdentity,
        pub label: Option<&'a str>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct PackGuaranteesInspection {
        pub portable: bool,
        pub self_contained: bool,
    }

    impl Pack {
        pub fn identity(&self) -> &PackIdentity {
            unimplemented!()
        }

        pub fn inspect(&self) -> PackInspection {
            unimplemented!()
        }

        pub fn prepare(
            &self,
            _admission: &OrdinaryAdmission,
            _request: crate::compilation::CompilationRequest,
        ) -> Result<crate::PreparedCompilation, crate::compilation::CompilationRequestRejection>
        {
            unimplemented!()
        }

        pub fn compilation_identity_hint(
            &self,
            _request: &crate::compilation::CompilationRequest,
        ) -> Option<CompilationIdentity> {
            None
        }
    }
}

pub mod authority {
    use super::{
        AuthorityInstanceIdentity, CompletePackageTreeIdentity, FontContainerIdentity, PackagePath,
        PackageSpecification, StableByteValue,
    };
    use std::future::Future;
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    pub struct CompletePackageTree {
        private: crate::private::CompletePackageTreeState,
    }

    impl CompletePackageTree {
        pub fn try_from_files(
            _files: impl IntoIterator<Item = (PackagePath, StableByteValue)>,
        ) -> Result<Self, CompletePackageTreeRejection> {
            unimplemented!()
        }

        pub fn identity(&self) -> &CompletePackageTreeIdentity {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CompletePackageTreeRejection {
        Empty,
        DuplicatePath,
        InvalidPath,
        AggregateLengthOverflow,
        FormatCeilingExceeded,
    }

    #[derive(Clone, Debug)]
    pub struct FontCatalogSnapshot {
        private: crate::private::FontCatalogState,
    }

    impl FontCatalogSnapshot {
        pub fn try_new(
            _candidates: impl IntoIterator<Item = FontCatalogCandidate>,
        ) -> Result<Self, FontCatalogRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct FontCatalogCandidate {
        pub container: FontContainerIdentity,
        pub face_index: u32,
        pub family: String,
        pub style: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum FontCatalogRejection {
        DuplicateFace,
        InvalidMetadata,
        FormatCeilingExceeded,
    }

    #[derive(Clone, Debug)]
    pub struct PackageAcquisitionRequest {
        pub specification: PackageSpecification,
        pub expected_tree_identity: Option<CompletePackageTreeIdentity>,
    }

    #[derive(Clone, Debug)]
    pub struct FontCatalogRequest {
        private: crate::private::FontCatalogRequestState,
    }

    #[derive(Clone, Debug)]
    pub struct FontContainerAcquisitionRequest {
        pub identity: FontContainerIdentity,
    }

    #[derive(Clone, Debug)]
    pub struct DependencyResolutionEvidence {
        private: crate::private::DependencyEvidenceState,
    }

    impl DependencyResolutionEvidence {
        pub fn builder(authority: AuthorityInstanceIdentity) -> DependencyEvidenceBuilder {
            DependencyEvidenceBuilder {
                authority,
                private: crate::private::DependencyEvidenceBuilderState,
            }
        }

        pub fn keys(&self) -> impl ExactSizeIterator<Item = &DependencyEvidenceKey> {
            std::iter::empty()
        }
    }

    pub struct DependencyEvidenceBuilder {
        authority: AuthorityInstanceIdentity,
        private: crate::private::DependencyEvidenceBuilderState,
    }

    impl DependencyEvidenceBuilder {
        pub fn record(
            &mut self,
            _key: DependencyEvidenceKey,
            _outcome: EvidenceFactOutcome,
        ) -> Result<(), DependencyEvidenceRejection> {
            unimplemented!()
        }

        pub fn finish(self) -> Result<DependencyResolutionEvidence, DependencyEvidenceRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EvidenceFactKind {
        Content,
        Absence,
        Membership,
        Order,
        Metadata,
        SourceChoice,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EvidenceFactOutcome {
        Selected,
        HigherPriorityUnavailable,
        Missing,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum DependencyEvidenceRejection {
        WrongAuthority,
        DuplicateContradiction,
        IncompleteSelection,
        InvalidOpaqueKey,
    }

    #[derive(Clone, Debug)]
    pub struct DependencyEvidenceKey {
        private: crate::private::DependencyEvidenceKeyState,
    }

    impl DependencyEvidenceKey {
        pub fn try_new(
            _authority: AuthorityInstanceIdentity,
            _kind: EvidenceFactKind,
            _opaque_key: Arc<[u8]>,
            _immutable_version: Option<Arc<[u8]>>,
        ) -> Result<Self, DependencyEvidenceRejection> {
            unimplemented!()
        }

        pub fn authority(&self) -> &AuthorityInstanceIdentity {
            unimplemented!()
        }
        pub fn kind(&self) -> EvidenceFactKind {
            unimplemented!()
        }
        pub fn opaque_key(&self) -> &[u8] {
            unimplemented!()
        }
        pub fn immutable_version(&self) -> Option<&[u8]> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct AcquisitionProvenance {
        private: crate::private::AcquisitionProvenanceState,
    }

    impl AcquisitionProvenance {
        pub fn try_new(_sanitized_class: &str) -> Result<Self, AuthorityValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum AuthorityValueRejection {
        Empty,
        NotNamespaced,
        ContainsSensitiveDetail,
        InvalidCode,
    }

    pub struct AcquiredDependency<T> {
        pub value: T,
        pub evidence: DependencyResolutionEvidence,
        pub provenance: AcquisitionProvenance,
    }

    pub enum DependencyAcquisitionOutcome<T> {
        Acquired(AcquiredDependency<T>),
        Failed(AuthorityFailure),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct AuthorityFailure {
        private: crate::private::AuthorityFailureState,
    }

    impl AuthorityFailure {
        pub fn try_new(
            _class: AuthorityFailureClass,
            _code: &str,
            _safe_message: &str,
        ) -> Result<Self, AuthorityValueRejection> {
            unimplemented!()
        }

        pub fn class(&self) -> AuthorityFailureClass {
            unimplemented!()
        }
        pub fn code(&self) -> &str {
            unimplemented!()
        }
        pub fn safe_message(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AuthorityFailureClass {
        Unavailable,
        Transient,
        Permanent,
        InvalidContent,
        IntegrityMismatch,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct AuthorityCapabilities {
        pub revalidation: bool,
        pub push_subscription: bool,
        pub cursor_replay: bool,
        pub offline: bool,
    }

    pub struct AcquisitionBudget {
        private: crate::private::AcquisitionBudgetState,
    }

    impl AcquisitionBudget {
        pub fn try_new(
            _downloaded_bytes: u64,
            _expanded_bytes: u64,
            _stable_spool_bytes: u64,
            _retained_memory_bytes: u64,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }

        pub fn reserve_download(&self, _bytes: u64) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }

        pub fn reserve_expanded(&self, _bytes: u64) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }

        pub fn reserve_stable_spool(
            &self,
            _bytes: u64,
        ) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }

        pub fn reserve_retained(&self, _bytes: u64) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }
    }

    pub struct BudgetReservation {
        private: crate::private::BudgetReservationState,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct BudgetExceeded {
        pub requested: u64,
        pub remaining: u64,
    }

    pub struct AcquisitionControls<'a> {
        deadline: crate::OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        budget: &'a AcquisitionBudget,
    }

    impl<'a> AcquisitionControls<'a> {
        pub fn try_new(
            deadline: crate::OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
            budget: &'a AcquisitionBudget,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let crate::OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                deadline,
                clock,
                interruption,
                budget,
            })
        }

        pub fn deadline(&self) -> &crate::OperationDeadline {
            &self.deadline
        }
        pub fn clock(&self) -> &dyn crate::MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn crate::InterruptionSource {
            self.interruption
        }
        pub fn budget(&self) -> &AcquisitionBudget {
            self.budget
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceRevalidationRequest {
        pub authority: AuthorityInstanceIdentity,
        pub keys: Vec<DependencyEvidenceKey>,
    }

    pub enum EvidenceRevalidationOutcome {
        Clean(EvidenceFence),
        Changed(EvidenceChange),
        InsufficientCapability(InsufficientEvidenceCapability),
        Failed(EvidenceFailure),
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceFence {
        private: crate::private::EvidenceFenceState,
    }

    impl EvidenceFence {
        pub fn try_new(
            _authority: AuthorityInstanceIdentity,
            _confirmed: impl IntoIterator<Item = DependencyEvidenceKey>,
            _generation: Arc<[u8]>,
            _through_cursor: Option<ProviderCursor>,
        ) -> Result<Self, EvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ProviderCursor {
        private: crate::private::ProviderCursorState,
    }

    impl ProviderCursor {
        pub fn try_new(
            _provider: AuthorityInstanceIdentity,
            _opaque: Arc<[u8]>,
        ) -> Result<Self, EvidenceValueRejection> {
            unimplemented!()
        }

        pub fn provider(&self) -> &AuthorityInstanceIdentity {
            unimplemented!()
        }
        pub fn opaque(&self) -> &[u8] {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceChange {
        private: crate::private::EvidenceChangeState,
    }

    impl EvidenceChange {
        pub fn try_new(_safe_code: &str) -> Result<Self, EvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceFailure {
        private: crate::private::EvidenceFailureState,
    }

    impl EvidenceFailure {
        pub fn try_new(_safe_code: &str) -> Result<Self, EvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct InsufficientEvidenceCapability {
        private: crate::private::InsufficientEvidenceCapabilityState,
    }

    impl InsufficientEvidenceCapability {
        pub fn new(_required: AuthorityCapabilities, _available: AuthorityCapabilities) -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum EvidenceValueRejection {
        Empty,
        WrongAuthority,
        IncompleteCoverage,
        IncoherentCursor,
        InvalidSafeCode,
    }

    pub trait SyncPackageAuthority {
        fn instance_identity(&self) -> &AuthorityInstanceIdentity;
        fn capabilities(&self) -> AuthorityCapabilities;

        fn acquire(
            &self,
            request: PackageAcquisitionRequest,
            controls: AcquisitionControls<'_>,
        ) -> DependencyAcquisitionOutcome<CompletePackageTree>;

        fn revalidate(
            &self,
            request: EvidenceRevalidationRequest,
            controls: AcquisitionControls<'_>,
        ) -> EvidenceRevalidationOutcome;
    }

    pub trait AsyncPackageAuthority {
        type Acquire<'a>: Future<Output = DependencyAcquisitionOutcome<CompletePackageTree>> + 'a
        where
            Self: 'a;

        type Revalidate<'a>: Future<Output = EvidenceRevalidationOutcome> + 'a
        where
            Self: 'a;

        fn instance_identity(&self) -> &AuthorityInstanceIdentity;
        fn capabilities(&self) -> AuthorityCapabilities;

        fn acquire<'a>(
            &'a self,
            request: PackageAcquisitionRequest,
            controls: AcquisitionControls<'a>,
        ) -> Self::Acquire<'a>;

        fn revalidate<'a>(
            &'a self,
            request: EvidenceRevalidationRequest,
            controls: AcquisitionControls<'a>,
        ) -> Self::Revalidate<'a>;
    }

    pub trait SyncFontAuthority {
        fn instance_identity(&self) -> &AuthorityInstanceIdentity;
        fn capabilities(&self) -> AuthorityCapabilities;

        fn catalog(
            &self,
            request: FontCatalogRequest,
            controls: AcquisitionControls<'_>,
        ) -> DependencyAcquisitionOutcome<FontCatalogSnapshot>;

        fn acquire_container(
            &self,
            request: FontContainerAcquisitionRequest,
            controls: AcquisitionControls<'_>,
        ) -> DependencyAcquisitionOutcome<StableByteValue>;

        fn revalidate(
            &self,
            request: EvidenceRevalidationRequest,
            controls: AcquisitionControls<'_>,
        ) -> EvidenceRevalidationOutcome;
    }

    pub trait AsyncFontAuthority {
        type Catalog<'a>: Future<Output = DependencyAcquisitionOutcome<FontCatalogSnapshot>> + 'a
        where
            Self: 'a;

        type AcquireContainer<'a>: Future<Output = DependencyAcquisitionOutcome<StableByteValue>>
            + 'a
        where
            Self: 'a;

        type Revalidate<'a>: Future<Output = EvidenceRevalidationOutcome> + 'a
        where
            Self: 'a;

        fn instance_identity(&self) -> &AuthorityInstanceIdentity;
        fn capabilities(&self) -> AuthorityCapabilities;

        fn catalog<'a>(
            &'a self,
            request: FontCatalogRequest,
            controls: AcquisitionControls<'a>,
        ) -> Self::Catalog<'a>;

        fn acquire_container<'a>(
            &'a self,
            request: FontContainerAcquisitionRequest,
            controls: AcquisitionControls<'a>,
        ) -> Self::AcquireContainer<'a>;

        fn revalidate<'a>(
            &'a self,
            request: EvidenceRevalidationRequest,
            controls: AcquisitionControls<'a>,
        ) -> Self::Revalidate<'a>;
    }
}

pub mod creation {
    use super::authority::{
        AsyncFontAuthority, AsyncPackageAuthority, SyncFontAuthority, SyncPackageAuthority,
    };
    use super::{
        AdmittedOperationResourceLimits, OperationDeadline, OrdinaryAdmission, Pack, ProjectPath,
        StableByteValue,
    };
    use std::future::Future;
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    pub struct ProjectSnapshot {
        private: Arc<crate::private::ProjectSnapshotState>,
    }

    impl ProjectSnapshot {
        pub fn try_from_files(
            _admission: &OrdinaryAdmission,
            _limits: &CreationResourceLimits,
            _entrypoint: ProjectPath,
            _files: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
        ) -> Result<Self, ProjectSnapshotRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ProjectSnapshotRejection {
        Empty,
        InvalidPath,
        DuplicatePath,
        MissingEntrypoint,
        AggregateLengthOverflow,
        FormatCeilingExceeded,
    }

    #[derive(Clone, Debug)]
    pub struct DiscoveryVariant {
        private: crate::private::DiscoveryVariantState,
    }

    impl DiscoveryVariant {
        pub fn paged_explicit_empty() -> Self {
            unimplemented!()
        }

        pub fn html_explicit_empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _target: crate::compilation::TypstTarget,
            _inputs: impl IntoIterator<Item = (crate::TypstInputKey, crate::TypstInputValue)>,
            _document_time: crate::compilation::CompilationDocumentTime,
            _features: impl IntoIterator<Item = crate::compilation::EngineFeature>,
            _overrides: crate::compilation::PackOverrideSet,
            _label: Option<String>,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EmbeddingDisposition {
        Embedded,
        ExternallyFulfilled,
    }

    #[derive(Clone, Debug)]
    pub struct PackageEmbeddingPolicy {
        private: crate::private::PackageEmbeddingPolicyState,
    }

    impl PackageEmbeddingPolicy {
        pub fn embed_all() -> Self {
            unimplemented!()
        }

        pub fn externally_fulfill_all() -> Self {
            unimplemented!()
        }

        pub fn try_by_specification(
            _default: EmbeddingDisposition,
            _choices: impl IntoIterator<Item = (crate::PackageSpecification, EmbeddingDisposition)>,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct FontEmbeddingPolicy {
        private: crate::private::FontEmbeddingPolicyState,
    }

    impl FontEmbeddingPolicy {
        pub fn embed_all() -> Self {
            unimplemented!()
        }

        pub fn externally_fulfill_all() -> Self {
            unimplemented!()
        }

        pub fn try_by_container(
            _default: EmbeddingDisposition,
            _choices: impl IntoIterator<Item = (crate::FontContainerIdentity, EmbeddingDisposition)>,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CreationRequest {
        private: crate::private::CreationRequestState,
    }

    impl CreationRequest {
        pub fn try_new(
            _limits: &CreationResourceLimits,
            _project: ProjectSnapshot,
            _variants: impl IntoIterator<Item = DiscoveryVariant>,
            _package_embedding: PackageEmbeddingPolicy,
            _font_embedding: FontEmbeddingPolicy,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }

        pub fn include(self, _path: ProjectPath) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CreationInputEvidence {
        private: crate::private::CreationInputEvidenceState,
    }

    impl CreationInputEvidence {
        pub fn caller_owned_immutable(_request: &CreationRequest) -> Self {
            unimplemented!()
        }

        pub fn versioned(
            _request: &CreationRequest,
            _source: super::AuthorityInstanceIdentity,
            _bindings: impl IntoIterator<Item = CreationEvidenceBinding>,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }

        pub fn revalidatable(
            _request: &CreationRequest,
            _provider: super::AuthorityInstanceIdentity,
            _bindings: impl IntoIterator<Item = CreationEvidenceBinding>,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CreationEvidenceBinding {
        private: crate::private::CreationEvidenceBindingState,
    }

    impl CreationEvidenceBinding {
        pub fn try_new(
            _subject: CreationEvidenceSubject,
            _key: super::authority::DependencyEvidenceKey,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub enum CreationEvidenceSubject {
        ProjectFile(ProjectPath),
        VariantValue {
            variant_ordinal: u32,
            role: String,
        },
        DiscoveryOverride {
            variant_ordinal: u32,
            path: ProjectPath,
        },
        InclusionMembership,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CreationEvidenceValueRejection {
        WrongRequest,
        DuplicateSubject,
        WrongProvider,
        MissingBinding,
        InvalidRole,
    }

    pub struct CreationInput {
        pub request: CreationRequest,
        pub evidence: CreationInputEvidence,
    }

    #[derive(Clone, Debug)]
    pub struct CreationEvidenceFenceRequest {
        private: crate::private::CreationEvidenceFenceRequestState,
    }

    impl CreationEvidenceFenceRequest {
        pub fn provider(&self) -> &super::AuthorityInstanceIdentity {
            unimplemented!()
        }
        pub fn keys(
            &self,
        ) -> impl ExactSizeIterator<Item = &super::authority::DependencyEvidenceKey> {
            std::iter::empty()
        }
        pub fn subjects(&self) -> impl ExactSizeIterator<Item = CreationEvidenceSubject> + '_ {
            std::iter::empty()
        }
    }

    pub enum CreationEvidenceFenceOutcome {
        Clean(CreationEvidenceFence),
        SourceChanged(SourceChanged),
        RevalidationFailed(EvidenceRevalidationFailure),
        InsufficientCapability(InsufficientCreationEvidenceCapability),
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    #[derive(Clone, Debug)]
    pub struct CreationEvidenceFence {
        private: crate::private::CreationEvidenceFenceState,
    }

    impl CreationEvidenceFence {
        pub fn try_new(
            _provider: super::AuthorityInstanceIdentity,
            _confirmed: impl IntoIterator<Item = super::authority::DependencyEvidenceKey>,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SourceChanged {
        private: crate::private::SourceChangedState,
    }

    impl SourceChanged {
        pub fn try_new(_safe_code: &str) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceRevalidationFailure {
        private: crate::private::CreationEvidenceFailureState,
    }

    impl EvidenceRevalidationFailure {
        pub fn try_new(_safe_code: &str) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct InsufficientCreationEvidenceCapability {
        private: crate::private::CreationEvidenceCapabilityState,
    }

    impl InsufficientCreationEvidenceCapability {
        pub fn new(
            _required: CreationEvidenceCapabilities,
            _available: CreationEvidenceCapabilities,
        ) -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationEvidenceCapabilities {
        pub immutable_or_versioned: bool,
        pub race_closing_revalidation: bool,
    }

    pub trait SyncCreationEvidence {
        fn provider_identity(&self) -> &super::AuthorityInstanceIdentity;
        fn capabilities(&self) -> CreationEvidenceCapabilities;

        fn fence(
            &self,
            request: CreationEvidenceFenceRequest,
            controls: super::authority::AcquisitionControls<'_>,
        ) -> CreationEvidenceFenceOutcome;
    }

    pub trait AsyncCreationEvidence {
        type Fence<'a>: Future<Output = CreationEvidenceFenceOutcome> + 'a
        where
            Self: 'a;

        fn provider_identity(&self) -> &super::AuthorityInstanceIdentity;
        fn capabilities(&self) -> CreationEvidenceCapabilities;

        fn fence<'a>(
            &'a self,
            request: CreationEvidenceFenceRequest,
            controls: super::authority::AcquisitionControls<'a>,
        ) -> Self::Fence<'a>;
    }

    pub struct ImmutableCreationEvidence {
        identity: super::AuthorityInstanceIdentity,
    }

    impl ImmutableCreationEvidence {
        pub fn new() -> Self {
            unimplemented!()
        }
    }

    impl SyncCreationEvidence for ImmutableCreationEvidence {
        fn provider_identity(&self) -> &super::AuthorityInstanceIdentity {
            &self.identity
        }

        fn capabilities(&self) -> CreationEvidenceCapabilities {
            CreationEvidenceCapabilities {
                immutable_or_versioned: true,
                race_closing_revalidation: true,
            }
        }

        fn fence(
            &self,
            _request: CreationEvidenceFenceRequest,
            _controls: super::authority::AcquisitionControls<'_>,
        ) -> CreationEvidenceFenceOutcome {
            unimplemented!()
        }
    }

    impl AsyncCreationEvidence for ImmutableCreationEvidence {
        type Fence<'a>
            = std::future::Ready<CreationEvidenceFenceOutcome>
        where
            Self: 'a;

        fn provider_identity(&self) -> &super::AuthorityInstanceIdentity {
            &self.identity
        }

        fn capabilities(&self) -> CreationEvidenceCapabilities {
            CreationEvidenceCapabilities {
                immutable_or_versioned: true,
                race_closing_revalidation: true,
            }
        }

        fn fence<'a>(
            &'a self,
            _request: CreationEvidenceFenceRequest,
            _controls: super::authority::AcquisitionControls<'a>,
        ) -> Self::Fence<'a> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CreationResourceLimits {
        project_files: u64,
        aggregate_project_bytes: u64,
        packages: u64,
        package_tree_bytes: u64,
        font_containers: u64,
        font_faces: u64,
        font_bytes: u64,
        discovery_variants: u64,
        discovery_restarts: u64,
        stable_spool_bytes: u64,
        retained_memory_bytes: u64,
    }

    impl CreationResourceLimits {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            project_files: u64,
            aggregate_project_bytes: u64,
            packages: u64,
            package_tree_bytes: u64,
            font_containers: u64,
            font_faces: u64,
            font_bytes: u64,
            discovery_variants: u64,
            discovery_restarts: u64,
            stable_spool_bytes: u64,
            retained_memory_bytes: u64,
        ) -> Result<Self, crate::AdmissionRefusal> {
            let limits = Self {
                project_files,
                aggregate_project_bytes,
                packages,
                package_tree_bytes,
                font_containers,
                font_faces,
                font_bytes,
                discovery_variants,
                discovery_restarts,
                stable_spool_bytes,
                retained_memory_bytes,
            };
            crate::private::SealedLimitSet::validate(&limits)?;
            Ok(limits)
        }
    }

    pub struct SyncCreationControls<'a, E: ?Sized, P: ?Sized, F: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
        evidence: &'a E,
        packages: &'a P,
        fonts: &'a F,
        acquisition_concurrency: NonZeroUsize,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a, E: ?Sized, P: ?Sized, F: ?Sized> SyncCreationControls<'a, E, P, F> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
            evidence: &'a E,
            packages: &'a P,
            fonts: &'a F,
            acquisition_concurrency: NonZeroUsize,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                evidence,
                packages,
                fonts,
                acquisition_concurrency,
                deadline,
                clock,
                interruption,
            })
        }
    }

    pub struct AsyncCreationControls<'a, E: ?Sized, P: ?Sized, F: ?Sized, X: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
        evidence: &'a E,
        packages: &'a P,
        fonts: &'a F,
        execution: &'a X,
        acquisition_concurrency: NonZeroUsize,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a, E: ?Sized, P: ?Sized, F: ?Sized, X: ?Sized> AsyncCreationControls<'a, E, P, F, X> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
            evidence: &'a E,
            packages: &'a P,
            fonts: &'a F,
            execution: &'a X,
            acquisition_concurrency: NonZeroUsize,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                evidence,
                packages,
                fonts,
                execution,
                acquisition_concurrency,
                deadline,
                clock,
                interruption,
            })
        }
    }

    pub fn create_sync<E: ?Sized, P: ?Sized, F: ?Sized>(
        _input: CreationInput,
        _controls: SyncCreationControls<'_, E, P, F>,
    ) -> CreationReport
    where
        E: SyncCreationEvidence,
        P: SyncPackageAuthority,
        F: SyncFontAuthority,
    {
        unimplemented!()
    }

    pub async fn create_async<E: ?Sized, P: ?Sized, F: ?Sized, X: ?Sized>(
        _input: CreationInput,
        _controls: AsyncCreationControls<'_, E, P, F, X>,
    ) -> CreationReport
    where
        E: AsyncCreationEvidence,
        P: AsyncPackageAuthority,
        F: AsyncFontAuthority,
        X: CreationExecutionFacility,
    {
        unimplemented!()
    }

    pub trait CreationExecutionFacility {
        type Dispatch<'a>: Future<Output = CreationDispatchOutcome> + 'a
        where
            Self: 'a;

        fn domain(&self) -> &crate::compilation::EngineRuntimeDomainDescriptor;

        fn dispatch<'a>(&'a self, job: ReadyCreationJob) -> Self::Dispatch<'a>;
    }

    pub struct ReadyCreationJob {
        private: crate::private::ReadyCreationJobState,
    }

    impl ReadyCreationJob {
        pub fn run_in_process(self) -> CreationJobCompletion {
            unimplemented!()
        }

        pub fn into_worker_request(
            self,
        ) -> (CreationWorkerRequest, CreationWorkerResponseVerifier) {
            unimplemented!()
        }
    }

    pub struct CreationWorkerRequest {
        private: crate::private::CreationWorkerRequestState,
    }

    impl CreationWorkerRequest {
        pub fn encode(self) -> StableByteValue {
            unimplemented!()
        }

        pub fn execute_in_worker(_request: StableByteValue) -> StableByteValue {
            unimplemented!()
        }
    }

    pub struct CreationWorkerResponseVerifier {
        private: crate::private::CreationWorkerResponseVerifierState,
    }

    impl CreationWorkerResponseVerifier {
        pub fn verify(
            self,
            _response: StableByteValue,
        ) -> Result<CreationJobCompletion, CreationExecutionFailure> {
            unimplemented!()
        }
    }

    pub struct CreationJobCompletion {
        private: crate::private::CreationJobCompletionState,
    }

    pub enum CreationDispatchOutcome {
        Completed(CreationJobCompletion),
        QueueFull,
        QueueTimeout,
        Refused,
        WorkerFailed,
        ProtocolFailed,
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    #[derive(Clone, Debug)]
    pub struct CreationReport {
        private: Arc<crate::private::CreationReportState>,
    }

    pub enum CreationTerminalRef<'a> {
        Issued(&'a Pack),
        Failed(&'a CreationFailure),
    }

    impl CreationReport {
        pub fn terminal(&self) -> CreationTerminalRef<'_> {
            unimplemented!()
        }

        pub fn into_pack(self) -> Result<Pack, Self> {
            unimplemented!()
        }

        pub fn phases(&self) -> impl ExactSizeIterator<Item = CreationPhase> {
            std::iter::empty()
        }

        pub fn diagnostics(&self) -> impl ExactSizeIterator<Item = CreationDiagnosticView<'_>> {
            std::iter::empty()
        }
    }

    pub struct CreationDiagnosticView<'a> {
        pub phase: CreationPhase,
        pub scope: CreationDiagnosticScope<'a>,
        pub severity: crate::compilation::DiagnosticSeverity,
        pub code: &'a str,
        pub safe_message: &'a str,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationDiagnosticScope<'a> {
        Operation,
        DiscoveryVariant {
            ordinal: u32,
            label: Option<&'a str>,
        },
        ReplayVariant {
            ordinal: u32,
            label: Option<&'a str>,
        },
    }

    #[derive(Clone, Debug)]
    pub struct CreationRequestRejection {
        private: crate::private::CreationRequestRejectionState,
    }

    #[derive(Clone, Debug)]
    pub struct CreationFailure {
        pub phase: CreationPhase,
        pub kind: CreationFailureKind,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationPhase {
        Admission,
        Discovery,
        Acquisition,
        EvidenceFence,
        Assembly,
        Replay,
        Issuance,
    }

    #[derive(Clone, Debug)]
    pub enum CreationFailureKind {
        Admission(crate::AdmissionRefusal),
        RequestRejected(CreationRequestRejection),
        Authority(super::authority::AuthorityFailure),
        SourceChanged(SourceChanged),
        RevalidationFailed(EvidenceRevalidationFailure),
        InsufficientEvidenceCapability(InsufficientCreationEvidenceCapability),
        ResourceLimit,
        Cancelled,
        Deadline,
        Execution(CreationExecutionFailure),
        ReplayDrift,
        InternalIntegrity,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CreationExecutionFailure {
        Queue,
        Worker,
        Protocol,
        Isolation,
        InternalIntegrity,
    }
}

pub mod compilation {
    use super::authority::{
        AsyncFontAuthority, AsyncPackageAuthority, DependencyResolutionEvidence, SyncFontAuthority,
        SyncPackageAuthority,
    };
    use super::{
        AdmittedOperationResourceLimits, CacheIsolationDomain, CompilationIdentity,
        CompilationResultIdentity, EngineIdentity, ExporterIdentity, OperationDeadline,
        OrdinaryAdmission, Pack, ProjectPath, StableByteValue, TypstInputKey, TypstInputValue,
    };
    use std::future::Future;
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EngineFeature {
        AccessibilityExtras,
        Bundle,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TypstTarget {
        Paged,
        Html,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationDocumentTime {
        Absent,
        UnixSeconds(i64),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PdfCreationTime {
        Omitted,
        UnixSeconds(i64),
    }

    #[derive(Clone, Debug)]
    pub struct PackOverrideSet {
        private: crate::private::PackOverrideSetState,
    }

    impl PackOverrideSet {
        pub fn empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _overrides: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct PageSelection {
        private: crate::private::PageSelectionState,
    }

    impl PageSelection {
        pub fn all() -> Self {
            unimplemented!()
        }

        pub fn try_from_ranges(
            _ranges: impl IntoIterator<Item = (u32, u32)>,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PdfTagging {
        Automatic,
        Enabled,
        Disabled,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PdfIdentifierMode {
        Automatic,
        Omit,
        Custom(String),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PdfCreatorMode {
        Automatic,
        Omit,
        Custom(String),
    }

    #[derive(Clone, Debug)]
    pub struct PdfOutputSpecification {
        private: crate::private::PdfOutputSpecificationState,
    }

    impl PdfOutputSpecification {
        pub fn core_defaults() -> Self {
            unimplemented!()
        }

        pub fn try_with_controls(
            self,
            _pages: PageSelection,
            _identifier: PdfIdentifierMode,
            _creator: PdfCreatorMode,
            _creation_time: PdfCreationTime,
            _standards: impl IntoIterator<Item = String>,
            _tagging: PdfTagging,
            _pretty: bool,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct PngOutputSpecification {
        private: crate::private::PngOutputSpecificationState,
    }

    impl PngOutputSpecification {
        pub fn try_new(
            _pages: PageSelection,
            _pixels_per_inch: f64,
            _bleed: bool,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SvgOutputSpecification {
        private: crate::private::SvgOutputSpecificationState,
    }

    impl SvgOutputSpecification {
        pub fn new(_pages: PageSelection, _bleed: bool, _pretty: bool) -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct HtmlOutputSpecification {
        private: crate::private::HtmlOutputSpecificationState,
    }

    impl HtmlOutputSpecification {
        pub fn new(_pretty: bool) -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub enum CompilationOutputSpecification {
        Pdf(PdfOutputSpecification),
        Png(PngOutputSpecification),
        Svg(SvgOutputSpecification),
        Html(HtmlOutputSpecification),
    }

    #[derive(Clone, Debug)]
    pub struct CanonicalDiagnosticPolicy {
        version: u32,
        max_entries: u64,
        max_canonical_entry_bytes: u64,
    }

    impl CanonicalDiagnosticPolicy {
        pub fn try_new(
            _version: u32,
            _max_entries: u64,
            _max_canonical_entry_bytes: u64,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CompilationRequest {
        diagnostics: CanonicalDiagnosticPolicy,
        private: crate::private::CompilationRequestState,
    }

    impl CompilationRequest {
        pub fn pdf(_diagnostics: CanonicalDiagnosticPolicy) -> Self {
            unimplemented!()
        }

        pub fn png(_diagnostics: CanonicalDiagnosticPolicy) -> Self {
            unimplemented!()
        }

        pub fn svg(_diagnostics: CanonicalDiagnosticPolicy) -> Self {
            unimplemented!()
        }

        pub fn html(_diagnostics: CanonicalDiagnosticPolicy) -> Self {
            unimplemented!()
        }

        pub fn diagnostics(&self) -> &CanonicalDiagnosticPolicy {
            &self.diagnostics
        }

        pub fn try_new(
            _output: CompilationOutputSpecification,
            _diagnostics: CanonicalDiagnosticPolicy,
            _inputs: impl IntoIterator<Item = (TypstInputKey, TypstInputValue)>,
            _features: impl IntoIterator<Item = EngineFeature>,
            _document_time: CompilationDocumentTime,
            _overrides: PackOverrideSet,
        ) -> Result<Self, CompilationRequestIssue> {
            unimplemented!()
        }
    }

    pub fn prepare(
        _admission: &OrdinaryAdmission,
        _pack: &Pack,
        _request: CompilationRequest,
    ) -> Result<PreparedCompilation, CompilationRequestRejection> {
        unimplemented!()
    }

    #[derive(Clone)]
    pub struct PreparedCompilation(Arc<crate::private::PreparedCompilationState>);

    impl PreparedCompilation {
        pub fn identity(&self) -> &CompilationIdentity {
            unimplemented!()
        }

        pub fn engine_identity(&self) -> &EngineIdentity {
            unimplemented!()
        }

        pub fn exporter_identity(&self) -> &ExporterIdentity {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CompilationRequestRejection {
        private: crate::private::CompilationRequestRejectionState,
    }

    impl CompilationRequestRejection {
        pub fn issues(&self) -> impl ExactSizeIterator<Item = CompilationRequestIssue> + '_ {
            std::iter::empty()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CompilationRequestIssue {
        InvalidPackOverride,
        InvalidTypstInput,
        UnsupportedFeature,
        InvalidOutputSpecification,
        InvalidCanonicalDiagnosticPolicy,
        PackFormatCeilingExceeded,
    }

    #[derive(Clone)]
    pub enum CompilationTerminal {
        RequestRejected(CompilationRequestRejection),
        Report(CompilationReport),
    }

    #[derive(Clone)]
    pub struct CompilationReport(Arc<crate::private::CompilationReportState>);

    pub enum CompilationReportTerminalRef<'a> {
        Result(&'a CompilationResult),
        OperationOutcome(&'a CompilationOperationOutcome),
    }

    impl CompilationReport {
        pub fn terminal(&self) -> CompilationReportTerminalRef<'_> {
            unimplemented!()
        }

        pub fn evidence(&self) -> &DependencyResolutionEvidence {
            unimplemented!()
        }

        pub fn result(&self) -> Option<&CompilationResult> {
            unimplemented!()
        }

        pub fn inventory(&self) -> CompilationAttemptInventoryView<'_> {
            unimplemented!()
        }

        pub fn cache_provenance(&self) -> SemanticCacheProvenance {
            unimplemented!()
        }

        pub fn evidence_scope(&self) -> DependencyEvidenceScope {
            unimplemented!()
        }

        pub fn compilation_identity(&self) -> &CompilationIdentity {
            unimplemented!()
        }
    }

    pub struct CompilationAttemptInventoryView<'a> {
        private: &'a crate::private::CompilationReportState,
    }

    impl CompilationAttemptInventoryView<'_> {
        pub fn trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn acquisition_concurrency(&self) -> NonZeroUsize {
            unimplemented!()
        }
        pub fn engine_runtime_domain(&self) -> EngineRuntimeDomainDescriptor {
            unimplemented!()
        }
        pub fn deadline(&self) -> &OperationDeadline {
            unimplemented!()
        }
        pub fn entries(&self) -> impl ExactSizeIterator<Item = CompilationInventoryEntry<'_>> {
            std::iter::empty()
        }
    }

    pub struct CompilationInventoryEntry<'a> {
        pub key: &'a str,
        pub origin: CompilationInventoryOrigin,
        pub semantic: bool,
        pub safe_value: &'a str,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationInventoryOrigin {
        CallerSupplied,
        CoreDefaulted,
        CoreDerived,
        AdapterResolved,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SemanticCacheProvenance {
        Disabled,
        Miss,
        HitVerified,
        UnavailableContinued,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyEvidenceScope {
        None,
        Partial,
        CompleteCurrentAttempt,
        HistoricalCacheTraceOnly,
    }

    #[derive(Clone)]
    pub struct CompilationResult(Arc<crate::private::CompilationResultState>);

    impl CompilationResult {
        pub fn identity(&self) -> &CompilationResultIdentity {
            unimplemented!()
        }

        pub fn succeeded(&self) -> bool {
            unimplemented!()
        }

        pub fn artifacts(&self) -> impl ExactSizeIterator<Item = CompilationArtifactView<'_>> {
            std::iter::empty()
        }

        pub fn diagnostics(&self) -> CanonicalDiagnosticEnvelopeView<'_> {
            unimplemented!()
        }

        pub fn document_summary(&self) -> CompilationDocumentSummaryView<'_> {
            unimplemented!()
        }

        pub fn access_trace(&self) -> CompilationAccessTraceView<'_> {
            unimplemented!()
        }
    }

    pub struct CompilationArtifactView<'a> {
        pub role: CompilationArtifactRole,
        pub bytes: &'a StableByteValue,
        pub identity: &'a crate::ContentIdentity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationArtifactRole {
        Document {
            format: DocumentFormat,
        },
        Page {
            format: PageFormat,
            source_page_number: u32,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DocumentFormat {
        Pdf,
        Html,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PageFormat {
        Png,
        Svg,
    }

    pub struct CanonicalDiagnosticEnvelopeView<'a> {
        private: &'a crate::private::CompilationResultState,
    }

    impl CanonicalDiagnosticEnvelopeView<'_> {
        pub fn retained_entries(&self) -> u64 {
            unimplemented!()
        }
        pub fn retained_canonical_bytes(&self) -> u64 {
            unimplemented!()
        }
        pub fn completion(&self) -> CanonicalDiagnosticCompletion {
            unimplemented!()
        }
        pub fn entries(&self) -> impl ExactSizeIterator<Item = CanonicalDiagnosticEntryView<'_>> {
            std::iter::empty()
        }
    }

    pub struct CanonicalDiagnosticEntryView<'a> {
        pub phase: DiagnosticPhase,
        pub severity: DiagnosticSeverity,
        pub kind: &'a str,
        pub message: &'a str,
        pub spans: &'a [DiagnosticSpanView<'a>],
        pub hints: &'a [String],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct DiagnosticSpanView<'a> {
        pub location: DiagnosticLogicalLocation<'a>,
        pub start: u64,
        pub end: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticLogicalLocation<'a> {
        Project(&'a ProjectPath),
        Package {
            specification: &'a crate::PackageSpecification,
            path: &'a crate::PackagePath,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticPhase {
        Compilation,
        Export,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CanonicalDiagnosticCompletion {
        Complete,
        Limited {
            first_omitted_ordinal: u64,
            first_omitted_phase: DiagnosticPhase,
            dimension: DiagnosticLimitDimension,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticLimitDimension {
        EntryCount,
        CanonicalEntryBytes,
    }

    pub struct CompilationDocumentSummaryView<'a> {
        private: &'a crate::private::CompilationResultState,
    }

    impl CompilationDocumentSummaryView<'_> {
        pub fn target(&self) -> TypstTarget {
            unimplemented!()
        }
        pub fn source_page_count(&self) -> Option<u32> {
            unimplemented!()
        }
    }

    pub struct CompilationAccessTraceView<'a> {
        private: &'a crate::private::CompilationResultState,
    }

    impl CompilationAccessTraceView<'_> {
        pub fn observations(
            &self,
        ) -> impl ExactSizeIterator<Item = CompilationAccessObservation<'_>> {
            std::iter::empty()
        }
    }

    pub struct CompilationAccessObservation<'a> {
        pub logical_identity: &'a str,
        pub outcome: &'a str,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationOperationOutcome {
        pub phase: CompilationPhase,
        pub cause: CompilationOperationCause,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationPhase {
        Admission,
        SemanticCacheLookup,
        DependencyResolution,
        DependencyAcquisition,
        DependencyVerification,
        Spooling,
        ReadyDispatchQueue,
        CompilationKernel,
        Export,
        ReportFinalization,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CompilationOperationCause {
        DependencyResolution(super::authority::AuthorityFailure),
        CacheUnavailable,
        CacheAuthorization,
        CacheIntegrity,
        CacheConflict,
        ResourceLimit(ResourceLimitFailure),
        Deadline,
        Cancelled,
        Superseded,
        Execution(CompilationExecutionFailure),
        Isolation(CompilationExecutionFailure),
        InternalIntegrity,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ResourceLimitFailure {
        pub dimension: ResourceLimitDimension,
        pub limit: u64,
        pub observed: Option<u64>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ResourceLimitDimension {
        DependencyCount,
        DownloadedBytes,
        ExpandedBytes,
        LargestDependency,
        StableSpool,
        PageCount,
        ArtifactCount,
        ArtifactBytes,
        RasterPixels,
        RetainedMemory,
        DiagnosticProjection,
        DiagnosticSource,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationResourceLimits {
        private: crate::private::CompilationResourceLimitsState,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationResourceLimitSpec {
        pub dependencies: u64,
        pub downloaded_dependency_bytes: u64,
        pub expanded_dependency_bytes: u64,
        pub largest_dependency_bytes: u64,
        pub stable_spool_bytes: u64,
        pub pages: u64,
        pub artifacts: u64,
        pub largest_artifact_bytes: u64,
        pub aggregate_artifact_bytes: u64,
        pub largest_raster_pixels: u64,
        pub aggregate_raster_pixels: u64,
        pub retained_memory_bytes: u64,
        pub diagnostic_projection_entries: u64,
        pub diagnostic_projection_bytes: u64,
        pub diagnostic_source_bindings: u64,
        pub diagnostic_source_blobs: u64,
        pub largest_diagnostic_source_bytes: u64,
        pub aggregate_diagnostic_source_bytes: u64,
        pub diagnostic_source_metadata_bytes: u64,
    }

    impl CompilationResourceLimits {
        pub fn try_new(
            _spec: CompilationResourceLimitSpec,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SemanticCacheAvailabilityPolicy {
        Required,
        ContinueOnUnavailable,
    }

    pub enum SyncSemanticCacheLookup<'a, C: ?Sized> {
        Disabled,
        Enabled {
            cache: &'a C,
            availability: SemanticCacheAvailabilityPolicy,
        },
    }

    pub enum AsyncSemanticCacheLookup<'a, C: ?Sized> {
        Disabled,
        Enabled {
            cache: &'a C,
            availability: SemanticCacheAvailabilityPolicy,
        },
    }

    #[derive(Clone, Debug)]
    pub struct CompilationReportingPolicy {
        pub diagnostic_projection: bool,
        pub diagnostic_source_bundle: bool,
        pub timing: bool,
    }

    pub struct SyncCompilationControls<'a, P: ?Sized, F: ?Sized, C: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
        packages: &'a P,
        fonts: &'a F,
        semantic_cache: SyncSemanticCacheLookup<'a, C>,
        acquisition_concurrency: NonZeroUsize,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        reporting: CompilationReportingPolicy,
    }

    impl<'a, P: ?Sized, F: ?Sized, C: ?Sized> SyncCompilationControls<'a, P, F, C> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
            packages: &'a P,
            fonts: &'a F,
            semantic_cache: SyncSemanticCacheLookup<'a, C>,
            acquisition_concurrency: NonZeroUsize,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
            reporting: CompilationReportingPolicy,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                packages,
                fonts,
                semantic_cache,
                acquisition_concurrency,
                deadline,
                clock,
                interruption,
                reporting,
            })
        }
    }

    pub struct AsyncCompilationControls<'a, P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
        packages: &'a P,
        fonts: &'a F,
        semantic_cache: AsyncSemanticCacheLookup<'a, C>,
        execution: &'a X,
        acquisition_concurrency: NonZeroUsize,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        reporting: CompilationReportingPolicy,
    }

    impl<'a, P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized> AsyncCompilationControls<'a, P, F, C, X> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
            packages: &'a P,
            fonts: &'a F,
            semantic_cache: AsyncSemanticCacheLookup<'a, C>,
            execution: &'a X,
            acquisition_concurrency: NonZeroUsize,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
            reporting: CompilationReportingPolicy,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                packages,
                fonts,
                semantic_cache,
                execution,
                acquisition_concurrency,
                deadline,
                clock,
                interruption,
                reporting,
            })
        }
    }

    pub fn run_sync<P: ?Sized, F: ?Sized, C: ?Sized>(
        _prepared: &PreparedCompilation,
        _controls: SyncCompilationControls<'_, P, F, C>,
    ) -> CompilationReport
    where
        P: SyncPackageAuthority,
        F: SyncFontAuthority,
        C: SyncSemanticResultCache,
    {
        unimplemented!()
    }

    pub async fn run_async<P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized>(
        _prepared: &PreparedCompilation,
        _controls: AsyncCompilationControls<'_, P, F, C, X>,
    ) -> CompilationReport
    where
        P: AsyncPackageAuthority,
        F: AsyncFontAuthority,
        C: AsyncSemanticResultCache,
        X: CompilationExecutionFacility,
    {
        unimplemented!()
    }

    pub fn compile_sync<P: ?Sized, F: ?Sized, C: ?Sized>(
        admission: &OrdinaryAdmission,
        pack: &Pack,
        request: CompilationRequest,
        controls: SyncCompilationControls<'_, P, F, C>,
    ) -> CompilationTerminal
    where
        P: SyncPackageAuthority,
        F: SyncFontAuthority,
        C: SyncSemanticResultCache,
    {
        match prepare(admission, pack, request) {
            Ok(prepared) => CompilationTerminal::Report(run_sync(&prepared, controls)),
            Err(rejection) => CompilationTerminal::RequestRejected(rejection),
        }
    }

    pub trait SyncSemanticResultCache {
        fn isolation_domain(&self) -> &CacheIsolationDomain;

        fn lookup(&self, request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome;

        fn admit(
            &self,
            request: SemanticCacheAdmissionRequest<'_>,
        ) -> SemanticCacheAdmissionOutcome;
    }

    pub trait AsyncSemanticResultCache {
        type Lookup<'a>: Future<Output = SemanticCacheLookupOutcome> + 'a
        where
            Self: 'a;

        type Admit<'a>: Future<Output = SemanticCacheAdmissionOutcome> + 'a
        where
            Self: 'a;

        fn isolation_domain(&self) -> &CacheIsolationDomain;

        fn lookup<'a>(&'a self, request: SemanticCacheLookupRequest<'a>) -> Self::Lookup<'a>;

        fn admit<'a>(&'a self, request: SemanticCacheAdmissionRequest<'a>) -> Self::Admit<'a>;
    }

    pub struct SemanticCacheLookupRequest<'a> {
        pub identity: &'a CompilationIdentity,
        pub limits: &'a CompilationResourceLimits,
        pub controls: CacheOperationControls<'a>,
    }

    #[derive(Clone)]
    pub struct SemanticCacheRecord {
        bytes: StableByteValue,
    }

    impl SemanticCacheRecord {
        pub fn from_untrusted_bytes(bytes: StableByteValue) -> Self {
            Self { bytes }
        }

        pub fn bytes(&self) -> &StableByteValue {
            &self.bytes
        }
    }

    pub enum SemanticCacheLookupOutcome {
        Miss,
        Candidate(SemanticCacheRecord),
        Unavailable,
        AuthorizationRefused,
        IntegrityFailure,
        ConflictingResult,
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    pub struct SemanticCacheAdmissionRequest<'a> {
        pub identity: &'a CompilationIdentity,
        pub record: &'a SemanticCacheRecord,
        pub controls: CacheOperationControls<'a>,
    }

    pub struct CacheBudget {
        private: crate::private::CacheBudgetState,
    }

    impl CacheBudget {
        pub fn try_new(
            _record_bytes: u64,
            _retained_memory_bytes: u64,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }

        pub fn reserve_record(
            &self,
            _bytes: u64,
        ) -> Result<CacheReservation, ResourceLimitFailure> {
            unimplemented!()
        }

        pub fn reserve_retained(
            &self,
            _bytes: u64,
        ) -> Result<CacheReservation, ResourceLimitFailure> {
            unimplemented!()
        }
    }

    pub struct CacheReservation {
        private: crate::private::CacheReservationState,
    }

    pub struct CacheOperationControls<'a> {
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        budget: &'a CacheBudget,
    }

    impl<'a> CacheOperationControls<'a> {
        pub fn try_new(
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
            budget: &'a CacheBudget,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                deadline,
                clock,
                interruption,
                budget,
            })
        }

        pub fn deadline(&self) -> &OperationDeadline {
            &self.deadline
        }
        pub fn clock(&self) -> &dyn crate::MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn crate::InterruptionSource {
            self.interruption
        }
        pub fn budget(&self) -> &CacheBudget {
            self.budget
        }
    }

    pub enum SemanticCacheAdmissionOutcome {
        Admitted,
        RecordPreparationFailed(SemanticCacheRecordFailure),
        Unavailable,
        AuthorizationRefused,
        IntegrityFailure,
        ConflictingResult,
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    pub struct CompilationCacheAdmissionOutcome {
        pub report: CompilationReport,
        pub cache: SemanticCacheAdmissionOutcome,
    }

    pub fn admit_to_cache_sync<C: SyncSemanticResultCache + ?Sized>(
        preparation: CompilationCacheRecordPreparation,
        cache: &C,
        controls: CacheOperationControls<'_>,
    ) -> CompilationCacheAdmissionOutcome {
        let report = preparation.report;
        let record = match preparation.record {
            Ok(record) => record,
            Err(failure) => {
                return CompilationCacheAdmissionOutcome {
                    report,
                    cache: SemanticCacheAdmissionOutcome::RecordPreparationFailed(failure),
                };
            }
        };
        let request = SemanticCacheAdmissionRequest {
            identity: report.compilation_identity(),
            record: &record,
            controls,
        };
        let outcome = cache.admit(request);
        CompilationCacheAdmissionOutcome {
            report,
            cache: outcome,
        }
    }

    pub async fn admit_to_cache_async<C: AsyncSemanticResultCache + ?Sized>(
        preparation: CompilationCacheRecordPreparation,
        cache: &C,
        controls: CacheOperationControls<'_>,
    ) -> CompilationCacheAdmissionOutcome {
        let report = preparation.report;
        let record = match preparation.record {
            Ok(record) => record,
            Err(failure) => {
                return CompilationCacheAdmissionOutcome {
                    report,
                    cache: SemanticCacheAdmissionOutcome::RecordPreparationFailed(failure),
                };
            }
        };
        let request = SemanticCacheAdmissionRequest {
            identity: report.compilation_identity(),
            record: &record,
            controls,
        };
        let outcome = cache.admit(request).await;
        CompilationCacheAdmissionOutcome {
            report,
            cache: outcome,
        }
    }

    pub struct NoSemanticResultCache {
        isolation: CacheIsolationDomain,
    }

    impl NoSemanticResultCache {
        pub fn new(isolation: CacheIsolationDomain) -> Self {
            Self { isolation }
        }
    }

    impl SyncSemanticResultCache for NoSemanticResultCache {
        fn isolation_domain(&self) -> &CacheIsolationDomain {
            &self.isolation
        }

        fn lookup(&self, _request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome {
            SemanticCacheLookupOutcome::Miss
        }

        fn admit(
            &self,
            _request: SemanticCacheAdmissionRequest<'_>,
        ) -> SemanticCacheAdmissionOutcome {
            SemanticCacheAdmissionOutcome::Unavailable
        }
    }

    impl AsyncSemanticResultCache for NoSemanticResultCache {
        type Lookup<'a>
            = std::future::Ready<SemanticCacheLookupOutcome>
        where
            Self: 'a;

        type Admit<'a>
            = std::future::Ready<SemanticCacheAdmissionOutcome>
        where
            Self: 'a;

        fn isolation_domain(&self) -> &CacheIsolationDomain {
            &self.isolation
        }

        fn lookup<'a>(&'a self, _request: SemanticCacheLookupRequest<'a>) -> Self::Lookup<'a> {
            std::future::ready(SemanticCacheLookupOutcome::Miss)
        }

        fn admit<'a>(&'a self, _request: SemanticCacheAdmissionRequest<'a>) -> Self::Admit<'a> {
            std::future::ready(SemanticCacheAdmissionOutcome::Unavailable)
        }
    }

    /// Post-commit preparation of a disposable cache record. Failure cannot
    /// mutate or replace the report.
    pub fn prepare_cache_admission(
        _report: CompilationReport,
        _limits: &CompilationResourceLimits,
    ) -> CompilationCacheRecordPreparation {
        unimplemented!()
    }

    pub struct CompilationCacheRecordPreparation {
        report: CompilationReport,
        record: Result<SemanticCacheRecord, SemanticCacheRecordFailure>,
    }

    impl CompilationCacheRecordPreparation {
        pub fn report(&self) -> &CompilationReport {
            &self.report
        }
        pub fn record(&self) -> Result<&SemanticCacheRecord, &SemanticCacheRecordFailure> {
            self.record.as_ref()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SemanticCacheRecordFailure {
        NoSemanticResult,
        ResourceLimit,
        InternalIntegrity,
    }

    #[derive(Clone, Debug)]
    pub struct EngineRuntimeDomainDescriptor {
        pub placement: EngineRuntimeDomainPlacement,
        pub width: Option<NonZeroUsize>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EngineRuntimeDomainPlacement {
        InheritedUnmanaged,
        ManagedInProcess,
        ManagedIsolatedWorker,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ExecutionFacilityCapacity {
        pub simultaneous_ready_jobs: NonZeroUsize,
        pub ready_queue: usize,
    }

    pub trait CompilationExecutionFacility {
        type Dispatch<'a>: Future<Output = CompilationDispatchOutcome> + 'a
        where
            Self: 'a;

        fn domain(&self) -> &EngineRuntimeDomainDescriptor;
        fn capacity(&self) -> ExecutionFacilityCapacity;

        fn dispatch<'a>(&'a self, job: ReadyCompilationJob) -> Self::Dispatch<'a>;
    }

    pub struct ReadyCompilationJob {
        private: crate::private::ReadyCompilationJobState,
    }

    impl ReadyCompilationJob {
        pub fn run_in_process(self) -> CompilationJobCompletion {
            unimplemented!()
        }

        pub fn into_worker_request(
            self,
        ) -> (CompilationWorkerRequest, CompilationWorkerResponseVerifier) {
            unimplemented!()
        }
    }

    pub struct CompilationWorkerRequest {
        private: crate::private::CompilationWorkerRequestState,
    }

    impl CompilationWorkerRequest {
        pub fn encode(self) -> StableByteValue {
            unimplemented!()
        }

        pub fn execute_in_worker(_request: StableByteValue) -> StableByteValue {
            unimplemented!()
        }
    }

    pub struct CompilationWorkerResponseVerifier {
        private: crate::private::CompilationWorkerResponseVerifierState,
    }

    impl CompilationWorkerResponseVerifier {
        pub fn verify(
            self,
            _response: StableByteValue,
        ) -> Result<CompilationJobCompletion, CompilationExecutionFailure> {
            unimplemented!()
        }
    }

    pub struct CompilationJobCompletion {
        private: crate::private::CompilationJobCompletionState,
    }

    pub enum CompilationDispatchOutcome {
        Completed(CompilationJobCompletion),
        QueueFull,
        QueueTimeout,
        Refused,
        WorkerFailed,
        ProtocolFailed,
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CompilationExecutionFailure {
        Queue,
        Worker,
        Protocol,
        Isolation,
        InternalIntegrity,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationReportDisclosure {
        private: crate::private::CompilationDisclosureState,
    }

    impl CompilationReportDisclosure {
        pub fn identity() -> Self {
            unimplemented!()
        }

        pub fn canonical_diagnostics(_capability: DiagnosticTextDisclosureCapability) -> Self {
            unimplemented!()
        }

        pub fn project(
            &self,
            _report: &CompilationReport,
            _limits: &CompilationResourceLimits,
        ) -> CompilationReportProjection {
            unimplemented!()
        }
    }

    pub struct DiagnosticTextDisclosureCapability {
        private: crate::private::DiagnosticDisclosureCapabilityState,
    }

    impl DiagnosticTextDisclosureCapability {
        pub fn explicitly_granted_by_caller() -> Self {
            unimplemented!()
        }
    }

    pub struct CompilationReportProjection {
        private: crate::private::CompilationReportProjectionState,
    }

    impl CompilationReportProjection {
        pub fn report(&self) -> &CompilationReport {
            unimplemented!()
        }
        pub fn includes_exact_diagnostic_text(&self) -> bool {
            unimplemented!()
        }
        pub fn diagnostics(&self) -> impl ExactSizeIterator<Item = DiagnosticProjectionEntry<'_>> {
            std::iter::empty()
        }
        pub fn artifacts(&self) -> impl ExactSizeIterator<Item = CompilationArtifactView<'_>> {
            std::iter::empty()
        }
    }

    pub struct DiagnosticProjectionEntry<'a> {
        pub phase: DiagnosticPhase,
        pub severity: DiagnosticSeverity,
        pub kind: &'a str,
        pub message: &'a str,
        pub spans: &'a [DiagnosticSpanView<'a>],
        pub hints: &'a [String],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticSeverity {
        Warning,
        Error,
    }
}

pub mod representation {
    use super::{
        AdmittedOperationResourceLimits, ClosureExportPath, ContentIdentity, OperationDeadline,
        OrdinaryAdmission, Pack, PackIdentity, ProjectPath, StableByteValue,
    };

    #[derive(Clone, Debug)]
    pub struct PackIngressResourceLimits {
        private: crate::private::PackIngressResourceLimitsState,
    }

    #[derive(Clone, Debug)]
    pub struct PackIngressResourceLimitSpec {
        pub archive_bytes: u64,
        pub control_record_bytes: u64,
        pub decoded_closure_bytes: u64,
        pub largest_file_bytes: u64,
        pub archive_entries: u64,
        pub files: u64,
        pub maximum_expansion_ratio: u64,
        pub stable_spool_bytes: u64,
        pub retained_memory_bytes: u64,
    }

    impl PackIngressResourceLimits {
        pub fn try_new(
            _spec: PackIngressResourceLimitSpec,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }
    }

    pub struct PackIngressControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<PackIngressResourceLimits>,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a> PackIngressControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<PackIngressResourceLimits>,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                deadline,
                clock,
                interruption,
            })
        }
    }

    #[derive(Clone, Debug)]
    pub enum PackIdentityVerificationMode {
        Derive,
        Verify(PackIdentity),
    }

    #[derive(Clone, Debug)]
    pub struct PackIngressExpectations {
        pub input_content_identity: Option<ContentIdentity>,
        pub pack_identity: PackIdentityVerificationMode,
        pub archive_encoding: Option<ArchiveEncodingIdentity>,
    }

    pub fn read_pack_archive(
        _archive: StableByteValue,
        _expectations: PackIngressExpectations,
        _controls: PackIngressControls<'_>,
    ) -> PackIngressReport {
        unimplemented!()
    }

    #[derive(Clone)]
    pub struct ClosureExportInput {
        files: Vec<(ClosureExportPath, StableByteValue)>,
    }

    impl ClosureExportInput {
        pub fn try_new(
            _admission: &OrdinaryAdmission,
            _files: Vec<(ClosureExportPath, StableByteValue)>,
        ) -> Result<Self, ClosureExportInputRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ClosureExportInputRejection {
        Empty,
        DuplicatePath,
        InvalidNamespace,
        AggregateLengthOverflow,
    }

    pub fn import_closure_export(
        _export: ClosureExportInput,
        _expectations: PackIngressExpectations,
        _controls: PackIngressControls<'_>,
    ) -> PackIngressReport {
        unimplemented!()
    }

    pub struct PackIngressReport {
        pub terminal: PackIngressTerminal,
        pub receipt: RepresentationReceipt,
    }

    pub enum PackIngressTerminal {
        Validated(Pack),
        Invalid(InvalidRepresentation),
        Unsupported(UnsupportedRepresentation),
        ExpectedPackIdentityMismatch,
        InputContentIdentityMismatch,
        ArchiveEncodingAssertionMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        AdmissionRefused(crate::AdmissionRefusal),
        InternalIntegrity,
    }

    #[derive(Clone, Debug)]
    pub struct RepresentationReceipt {
        private: crate::private::RepresentationReceiptState,
    }

    impl RepresentationReceipt {
        pub fn validation_rules(&self) -> impl ExactSizeIterator<Item = ValidationRuleCode> + '_ {
            std::iter::empty()
        }

        pub fn input_content_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct InvalidRepresentation {
        pub first_rule: ValidationRuleCode,
        pub all_rules: Vec<ValidationRuleCode>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct UnsupportedRepresentation {
        pub rule: ValidationRuleCode,
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct ValidationRuleCode(String);

    impl ValidationRuleCode {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Clone, Debug)]
    pub struct ArchiveEncodingIdentity {
        private: crate::private::ArchiveEncodingIdentityState,
    }

    impl ArchiveEncodingIdentity {
        pub fn epoch_2_canonical_deflate() -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct RepresentationResourceLimits {
        output_bytes: u64,
        files: u64,
        stable_spool_bytes: u64,
        retained_memory_bytes: u64,
    }

    impl RepresentationResourceLimits {
        pub fn try_new(
            output_bytes: u64,
            files: u64,
            stable_spool_bytes: u64,
            retained_memory_bytes: u64,
        ) -> Result<Self, crate::AdmissionRefusal> {
            let limits = Self {
                output_bytes,
                files,
                stable_spool_bytes,
                retained_memory_bytes,
            };
            crate::private::SealedLimitSet::validate(&limits)?;
            Ok(limits)
        }
    }

    pub struct RepresentationControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<RepresentationResourceLimits>,
        deadline: OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a> RepresentationControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<RepresentationResourceLimits>,
            deadline: OperationDeadline,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                deadline,
                clock,
                interruption,
            })
        }
    }

    pub fn encode_pack_archive<S>(
        _pack: &Pack,
        _encoding: ArchiveEncodingIdentity,
        _spool: &mut S,
        _controls: RepresentationControls<'_>,
    ) -> PackArchiveEncodingReport
    where
        S: crate::transport::SyncSpoolFacility,
    {
        unimplemented!()
    }

    pub struct EncodedPackArchive {
        pub bytes: StableByteValue,
        pub pack_identity: PackIdentity,
        pub encoding_identity: ArchiveEncodingIdentity,
    }

    pub struct PackArchiveEncodingReport {
        pub terminal: Result<EncodedPackArchive, RepresentationFailure>,
        pub receipt: RepresentationReceipt,
    }

    pub struct ProjectMaterializationPlan {
        private: crate::private::ProjectMaterializationPlanState,
    }

    pub struct ClosureExportPlan {
        private: crate::private::ClosureExportPlanState,
    }

    pub fn plan_project_materialization(
        _pack: &Pack,
        _controls: RepresentationControls<'_>,
    ) -> Result<ProjectMaterializationPlan, RepresentationFailure> {
        unimplemented!()
    }

    pub fn plan_closure_export(
        _pack: &Pack,
        _controls: RepresentationControls<'_>,
    ) -> Result<ClosureExportPlan, RepresentationFailure> {
        unimplemented!()
    }

    impl ProjectMaterializationPlan {
        pub fn files(&self) -> impl ExactSizeIterator<Item = ProjectMaterializationFile<'_>> {
            std::iter::empty()
        }
    }

    impl ClosureExportPlan {
        pub fn entries(&self) -> impl ExactSizeIterator<Item = ClosureExportEntry<'_>> {
            std::iter::empty()
        }
    }

    pub struct ProjectMaterializationFile<'a> {
        pub path: &'a ProjectPath,
        pub bytes: &'a StableByteValue,
        pub identity: &'a ContentIdentity,
    }

    pub struct ClosureExportEntry<'a> {
        pub path: &'a ClosureExportPath,
        pub role: ClosureExportEntryRole,
        pub bytes: &'a StableByteValue,
        pub identity: &'a ContentIdentity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ClosureExportEntryRole {
        ControlRecord,
        ProjectFile,
        PackageFile,
        FontContainer,
        Extension,
        Annotation,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RepresentationFailure {
        Admission,
        ResourceLimit,
        Encoding,
        Spooling,
        Cancelled,
        Deadline,
        InternalIntegrity,
    }
}

pub mod session {
    use super::authority::{DependencyEvidenceKey, DependencyResolutionEvidence, ProviderCursor};
    use super::{CompilationReport, CompilationTerminal, Pack};

    pub struct CompilationSession {
        state: crate::private::SessionState,
    }

    impl CompilationSession {
        pub fn new(
            _admission: crate::OrdinaryAdmission,
            _pack: Pack,
            _policy: SessionPolicy,
        ) -> Self {
            unimplemented!()
        }

        pub fn apply(
            &mut self,
            _event: SessionEvent,
        ) -> Result<SessionTransition, SessionEventRejection> {
            unimplemented!()
        }

        pub fn view(&self) -> SessionView<'_> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SessionPolicy {
        private: crate::private::SessionPolicyState,
    }

    impl SessionPolicy {
        pub fn latest_only_complete_coverage() -> Self {
            unimplemented!()
        }

        pub fn latest_only_allow_unverified() -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct StabilizedSessionInput {
        private: crate::private::StabilizedSessionInputState,
    }

    impl StabilizedSessionInput {
        pub fn try_new(
            _request: crate::compilation::CompilationRequest,
            _evidence: SessionRequestEvidence,
        ) -> Result<Self, SessionInputRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SessionRequestEvidence {
        private: crate::private::SessionRequestEvidenceState,
    }

    impl SessionRequestEvidence {
        pub fn caller_owned_immutable() -> Self {
            unimplemented!()
        }

        pub fn revalidatable(
            _scopes: impl IntoIterator<Item = EvidenceScope>,
        ) -> Result<Self, SessionInputRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SessionInputRejection {
        MissingEvidence,
        DuplicateScope,
        UnsupportedPolicy,
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SessionRevision(crate::private::SessionRevisionState);

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SessionAttemptToken {
        private: crate::private::SessionAttemptTokenState,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SessionFenceToken {
        private: crate::private::SessionFenceTokenState,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SubscriptionGeneration {
        private: crate::private::SubscriptionGenerationState,
    }

    pub enum SessionEvent {
        Accept(StabilizedSessionInput),
        IngestionFailed(SessionIngestionFailure),
        DependencyChanged {
            generation: SubscriptionGeneration,
            change: DependencyChangeNotification,
        },
        NotificationGap {
            generation: SubscriptionGeneration,
            scope: EvidenceScope,
        },
        Refresh,
        Retry,
        AttemptFinished {
            token: SessionAttemptToken,
            report: CompilationReport,
        },
        FenceReadFinished {
            token: SessionFenceToken,
            outcome: FenceReadOutcome,
        },
        SubscriptionsArmed {
            token: SessionFenceToken,
            outcome: SubscriptionArmOutcome,
        },
        FenceConfirmed {
            token: SessionFenceToken,
            outcome: FenceConfirmationOutcome,
        },
        Shutdown,
    }

    pub enum SessionEffect {
        StartAttempt {
            token: SessionAttemptToken,
            plan: SessionAttemptPlan,
        },
        InterruptAttempt {
            token: SessionAttemptToken,
        },
        ReadFence {
            token: SessionFenceToken,
            plan: CurrentnessFenceReadPlan,
        },
        ArmSubscriptions {
            token: SessionFenceToken,
            plan: SubscriptionPlan,
        },
        ConfirmFence {
            token: SessionFenceToken,
            plan: CurrentnessFenceConfirmationPlan,
        },
        RetireSubscriptions {
            generation: SubscriptionGeneration,
        },
        Publish {
            publication: SessionPublication,
        },
    }

    pub struct SessionAttemptPlan {
        pub prepared: crate::PreparedCompilation,
        pub revision: SessionRevision,
        pub policy: SessionPolicy,
    }

    pub enum SessionTransition {
        Applied(Vec<SessionEffect>),
        Ignored(SessionIgnoredEvent),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SessionIgnoredEvent {
        PreviousSessionInstance,
        SupersededRevision,
        StaleAttempt,
        StaleFence,
        OldSubscription,
        DuplicateCompletion,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SessionEventRejection {
        MalformedToken,
        ImpossibleTransition,
        AdapterContractViolation,
    }

    #[derive(Clone, Debug)]
    pub struct CurrentnessFenceReadPlan {
        pub request_sources: Vec<EvidenceScope>,
        pub dependency_keys: Vec<DependencyEvidenceKey>,
        pub historical_evidence: Option<DependencyResolutionEvidence>,
    }

    pub enum FenceReadOutcome {
        Read(FenceReadObservation),
        Changed,
        Incomplete(SessionWatchCoverage),
        Failed,
    }

    #[derive(Clone, Debug)]
    pub struct FenceReadObservation {
        private: crate::private::FenceReadObservationState,
    }

    impl FenceReadObservation {
        pub fn try_new(
            _request_sources: impl IntoIterator<Item = RequestSourceObservation>,
            _evidence: DependencyResolutionEvidence,
            _cursors: impl IntoIterator<Item = ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct RequestSourceObservation {
        private: crate::private::RequestSourceObservationState,
    }

    impl RequestSourceObservation {
        pub fn try_new(
            _scope: EvidenceScope,
            _identity: crate::CanonicalIdentity,
            _cursor: Option<ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SubscriptionPlan {
        private: crate::private::SubscriptionPlanState,
    }

    impl SubscriptionPlan {
        pub fn generation(&self) -> &SubscriptionGeneration {
            unimplemented!()
        }
        pub fn interests(&self) -> impl ExactSizeIterator<Item = SubscriptionInterest<'_>> {
            std::iter::empty()
        }
    }

    pub struct SubscriptionInterest<'a> {
        pub scope: &'a EvidenceScope,
        pub keys: &'a [DependencyEvidenceKey],
        pub after: Option<&'a ProviderCursor>,
    }

    pub enum SubscriptionArmOutcome {
        Armed(ArmedSubscriptions),
        Incomplete(SessionWatchCoverage),
        Failed,
    }

    #[derive(Clone, Debug)]
    pub struct ArmedSubscriptions {
        private: crate::private::ArmedSubscriptionsState,
    }

    impl ArmedSubscriptions {
        pub fn try_new(
            _plan: &SubscriptionPlan,
            _coverage: SessionWatchCoverage,
            _cursors: impl IntoIterator<Item = ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CurrentnessFenceConfirmationPlan {
        private: crate::private::FenceConfirmationPlanState,
    }

    impl CurrentnessFenceConfirmationPlan {
        pub fn generation(&self) -> &SubscriptionGeneration {
            unimplemented!()
        }
        pub fn request_sources(&self) -> impl ExactSizeIterator<Item = &EvidenceScope> {
            std::iter::empty()
        }
        pub fn dependency_keys(&self) -> impl ExactSizeIterator<Item = &DependencyEvidenceKey> {
            std::iter::empty()
        }
        pub fn armed_cursors(&self) -> impl ExactSizeIterator<Item = &ProviderCursor> {
            std::iter::empty()
        }
    }

    pub enum FenceConfirmationOutcome {
        Clean(FenceConfirmation),
        Dirty,
        Incomplete(SessionWatchCoverage),
        Failed,
    }

    #[derive(Clone, Debug)]
    pub struct FenceConfirmation {
        private: crate::private::FenceConfirmationState,
    }

    impl FenceConfirmation {
        pub fn try_new(
            _plan: &CurrentnessFenceConfirmationPlan,
            _cursors: impl IntoIterator<Item = ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SessionWatchCoverage {
        private: crate::private::SessionWatchCoverageState,
    }

    impl SessionWatchCoverage {
        pub fn complete_push() -> Self {
            unimplemented!()
        }

        pub fn complete_poll() -> Self {
            unimplemented!()
        }

        pub fn incomplete(_scopes: impl IntoIterator<Item = EvidenceScope>) -> Self {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct DependencyChangeNotification {
        private: crate::private::DependencyChangeState,
    }

    impl DependencyChangeNotification {
        pub fn try_new(
            _scope: EvidenceScope,
            _cursor: Option<ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceScope {
        private: crate::private::EvidenceScopeState,
    }

    impl EvidenceScope {
        pub fn try_new(_namespaced_value: &str) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SessionAdapterValueRejection {
        Empty,
        WrongPlan,
        DuplicateCursor,
        IncompleteCoverage,
        InvalidNamespace,
    }

    #[derive(Clone)]
    pub struct SessionPublication {
        pub revision: SessionRevision,
        pub terminal: SessionPublicationTerminal,
        pub currentness: SessionCurrentness,
    }

    #[derive(Clone)]
    pub enum SessionPublicationTerminal {
        Compilation(CompilationTerminal),
        IngestionFailure(SessionIngestionFailure),
    }

    #[derive(Clone, Debug)]
    pub enum SessionCurrentness {
        CurrentThroughPush { cursors: Vec<ProviderCursor> },
        CurrentAsOfPoll { observation: ProviderCursor },
        Unverified { uncovered: Vec<EvidenceScope> },
        Stale { dirty: Vec<EvidenceScope> },
    }

    #[derive(Clone, Debug)]
    pub struct SessionIngestionFailure {
        private: crate::private::SessionIngestionFailureState,
    }

    impl SessionIngestionFailure {
        pub fn try_new(_safe_code: &str) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    pub struct SessionView<'a> {
        pub publication: Option<&'a SessionPublication>,
        pub last_successful: Option<LastSuccessfulCompilationView<'a>>,
        private: &'a crate::private::SessionState,
    }

    pub struct LastSuccessfulCompilationView<'a> {
        pub revision: &'a SessionRevision,
        pub result: &'a super::CompilationResult,
        pub currentness: &'a SessionCurrentness,
    }
}

macro_rules! impl_operation_limits {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl private::SealedLimitSet for $ty {
                fn validate(&self) -> Result<(), AdmissionRefusal> { unimplemented!() }
                fn no_looser_than(&self, _requested: &Self) -> bool { unimplemented!() }
            }
            impl OperationLimitSet for $ty {}
        )+
    };
}

impl_operation_limits!(
    transport::SpoolResourceLimits,
    transport::TransportResourceLimits,
    creation::CreationResourceLimits,
    compilation::CompilationResourceLimits,
    representation::PackIngressResourceLimits,
    representation::RepresentationResourceLimits,
);

mod private {
    use std::sync::Arc;

    macro_rules! state {
        ($($name:ident),+ $(,)?) => {
            $(#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)] pub struct $name;)+
        };
    }

    pub trait SealedLimitSet {
        fn validate(&self) -> Result<(), crate::AdmissionRefusal>;
        fn no_looser_than(&self, requested: &Self) -> bool
        where
            Self: Sized;
    }

    #[derive(Clone)]
    pub enum StableBacking {
        Static(&'static [u8]),
        Contiguous(Arc<[u8]>),
        Chunked(Arc<[Arc<[u8]>]>),
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Native(Arc<NativeStableBacking>),
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[derive(Clone)]
    pub struct NativeStableBacking;

    state!(
        NativeSpoolState,
        ResidualTransportLocatorState,
        ResidualLocatorDisclosureCapabilityState,
        ValidatedPack,
        PackIdentityState,
        PackInspectionState,
        ProjectSnapshotState,
        DiscoveryVariantState,
        CreationRequestState,
        PackageEmbeddingPolicyState,
        FontEmbeddingPolicyState,
        CreationInputEvidenceState,
        CreationEvidenceBindingState,
        CreationEvidenceFenceRequestState,
        CreationEvidenceFenceState,
        SourceChangedState,
        CreationEvidenceFailureState,
        CreationEvidenceCapabilityState,
        ReadyCreationJobState,
        CreationWorkerRequestState,
        CreationWorkerResponseVerifierState,
        CreationJobCompletionState,
        CreationReportState,
        CreationRequestRejectionState,
        CompletePackageTreeState,
        FontCatalogState,
        FontCatalogRequestState,
        DependencyEvidenceState,
        DependencyEvidenceBuilderState,
        AcquisitionBudgetState,
        BudgetReservationState,
        DependencyEvidenceKeyState,
        AcquisitionProvenanceState,
        AuthorityFailureState,
        EvidenceFenceState,
        EvidenceChangeState,
        EvidenceFailureState,
        InsufficientEvidenceCapabilityState,
        ProviderCursorState,
        CompilationRequestState,
        PackOverrideSetState,
        PageSelectionState,
        PdfOutputSpecificationState,
        PngOutputSpecificationState,
        SvgOutputSpecificationState,
        HtmlOutputSpecificationState,
        PreparedCompilationState,
        CompilationRequestRejectionState,
        CompilationReportState,
        CompilationResultState,
        CompilationResourceLimitsState,
        CacheBudgetState,
        CacheReservationState,
        ReadyCompilationJobState,
        CompilationWorkerRequestState,
        CompilationWorkerResponseVerifierState,
        CompilationJobCompletionState,
        CompilationDisclosureState,
        DiagnosticDisclosureCapabilityState,
        CompilationReportProjectionState,
        RepresentationReceiptState,
        ArchiveEncodingIdentityState,
        PackIngressResourceLimitsState,
        ProjectMaterializationPlanState,
        ClosureExportPlanState,
        TransportReceiptState,
        SessionState,
        SessionPolicyState,
        StabilizedSessionInputState,
        SessionRequestEvidenceState,
        SessionRevisionState,
        SessionAttemptTokenState,
        SessionFenceTokenState,
        SubscriptionGenerationState,
        FenceReadObservationState,
        RequestSourceObservationState,
        SubscriptionPlanState,
        ArmedSubscriptionsState,
        FenceConfirmationPlanState,
        FenceConfirmationState,
        SessionWatchCoverageState,
        DependencyChangeState,
        EvidenceScopeState,
        SessionIngestionFailureState,
    );

    // These are the only semantic implementation seams. Adapters never receive
    // their concrete state: they only move sealed jobs and immutable values.
    pub fn construct_validated_pack() -> ValidatedPack {
        unimplemented!()
    }

    pub fn run_discovery_and_issue() -> CreationReportState {
        unimplemented!()
    }

    pub fn build_compilation_dependency_snapshot() {
        unimplemented!()
    }

    pub fn run_synchronous_compilation_kernel() -> CompilationResultState {
        unimplemented!()
    }

    pub fn commit_compilation_terminal() -> CompilationReportState {
        unimplemented!()
    }

    pub fn validate_epoch_2_representation() -> ValidatedPack {
        unimplemented!()
    }
}
