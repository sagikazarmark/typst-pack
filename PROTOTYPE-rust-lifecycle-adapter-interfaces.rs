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
opaque_value!(pub ProjectTreeIdentity);
opaque_value!(pub PackageRequirementIdentity);
opaque_value!(pub FontRequirementIdentity);
opaque_value!(pub DiscoveryRequestCommitment);
opaque_value!(pub DiscoveryVariantIdentity);
opaque_value!(pub DiscoveryTraceIdentity);
opaque_value!(pub DiscoveryCoverageIdentity);
opaque_value!(pub EngineIdentity);
opaque_value!(pub ExporterIdentity);
opaque_value!(pub CompilationRequestCommitment);
opaque_value!(pub EngineNeutralCompilationIntentIdentity);
opaque_value!(pub CompilationIdentity);
opaque_value!(pub CompilationArtifactIdentity);
opaque_value!(pub CompilationResultIdentity);
opaque_value!(pub ClosureExportTreeContentIdentity);
opaque_value!(pub CacheIsolationDomain);
opaque_value!(pub AuthorityInstanceIdentity);
opaque_value!(pub ResourceProfileIdentity);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationalCapabilityClass(Arc<str>);

impl OperationalCapabilityClass {
    pub fn try_new(_value: &str) -> Result<Self, OperationalCapabilityClassRejection> {
        unimplemented!()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalCapabilityClassRejection {
    Empty,
    TooLong,
    NonAscii,
    InvalidNamespace,
    InvalidPath,
    InvalidMajor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationNetworkPolicy {
    NetworkPermitted,
    Offline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedNetworkContract {
    NetworkPermitted,
    NoNetwork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreCommitFacilityRole {
    CreationEvidence,
    PackageAuthority,
    FontAuthority,
    AuthorityPrivateCache,
    SemanticResultCache,
    Source,
    Spool,
    CreationExecution,
    CompilationExecution,
    WorkerControlTransport,
    ReportingSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionPlacement {
    InProcess,
    Worker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationInterruptionStrength {
    Cooperative,
    Isolated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnforcementStrength {
    NotClaimed,
    BestEffort,
    VerifiedHard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnforcementDimension {
    Filesystem,
    Network,
    Cpu,
    Memory,
    ProcessTree,
    TemporaryStorage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnforcementClaim {
    pub dimension: EnforcementDimension,
    pub strength: EnforcementStrength,
    pub covered_scope: Arc<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationAdmissionRefusalReason {
    UnsupportedTrustProfile,
    IncoherentDescriptor,
    UnsatisfiedNetworkPolicy,
    ResourceLimit,
    Capacity,
    EngineWidth,
    ExecutionPlacement,
    InterruptionStrength,
    RequiredReportingUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionConstraint {
    DocumentedProfileCap,
    ConfiguredCapacity,
    VerifiedAvailableCapacity,
    AuthorityPerOperationCap,
    SharedCapacityScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineWidthRequest {
    InheritedUnmanaged,
    Automatic,
    Exact(std::num::NonZeroUsize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineWidthAdmission {
    InheritedUnmanaged,
    Exact {
        requested: EngineWidthRequest,
        admitted: std::num::NonZeroUsize,
        constraints: Vec<AdmissionConstraint>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyConcurrencyAdmission {
    pub requested: std::num::NonZeroUsize,
    pub admitted: std::num::NonZeroUsize,
    pub constraints: Vec<AdmissionConstraint>,
}

pub struct EnforcementAdmissionView<'a> {
    pub requested: &'a [EnforcementClaim],
    pub admitted: &'a [EnforcementClaim],
    pub reached: &'a [EnforcementClaim],
}

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

impl ClosureExportTreeContentIdentity {
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

    pub fn caller_selected_format_receipt_v1() -> Self {
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
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct StableByteValue(Arc<crate::private::StableBacking>);

    impl StableByteValue {
        pub fn from_vec(
            _admission: &OrdinaryAdmission,
            _bytes: Vec<u8>,
        ) -> Result<Self, StableByteValueConstructionError> {
            unimplemented!()
        }

        pub fn from_arc(
            _admission: &OrdinaryAdmission,
            _bytes: Arc<[u8]>,
        ) -> Result<Self, StableByteValueConstructionError> {
            unimplemented!()
        }

        pub fn copy_from_slice(
            _admission: &OrdinaryAdmission,
            _bytes: &[u8],
        ) -> Result<Self, StableByteValueConstructionError> {
            unimplemented!()
        }

        pub fn from_static(
            _admission: &OrdinaryAdmission,
            _bytes: &'static [u8],
        ) -> Result<Self, StableByteValueConstructionError> {
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
        IdentityApplicabilityCeilingExceeded,
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

    #[derive(Clone, Debug)]
    pub struct SpoolOperationRequest {
        pub network: crate::OperationNetworkPolicy,
        pub transfer_concurrency: NonZeroUsize,
        pub interruption: crate::OperationInterruptionStrength,
        pub deadline: OperationDeadline,
        pub cleanup: TransportCleanupRequirement,
        pub required_enforcement: Vec<crate::EnforcementClaim>,
        pub timing_requested: bool,
    }

    #[derive(Clone, Debug)]
    pub struct PackArchiveAcquisitionOperationRequest {
        pub network: crate::OperationNetworkPolicy,
        pub transfer_concurrency: NonZeroUsize,
        pub interruption: crate::OperationInterruptionStrength,
        pub deadline: OperationDeadline,
        pub cleanup: TransportCleanupRequirement,
        pub required_enforcement: Vec<crate::EnforcementClaim>,
        pub timing_requested: bool,
    }

    macro_rules! publication_transport_operation_request {
        ($name:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                pub network: crate::OperationNetworkPolicy,
                pub transfer_concurrency: NonZeroUsize,
                pub interruption: crate::OperationInterruptionStrength,
                pub deadline: OperationDeadline,
                pub commit: PublicationCommitStrength,
                pub cleanup: TransportCleanupRequirement,
                pub required_enforcement: Vec<crate::EnforcementClaim>,
                pub timing_requested: bool,
            }
        };
    }

    publication_transport_operation_request!(PackArchivePublicationOperationRequest);
    publication_transport_operation_request!(ProjectMaterializationPublicationOperationRequest);
    publication_transport_operation_request!(ClosureExportPublicationOperationRequest);
    publication_transport_operation_request!(CompilationDeliveryOperationRequest);

    pub struct SpoolControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<SpoolResourceLimits>,
        expected_identity: Option<ContentIdentity>,
        request: SpoolOperationRequest,
        clock: &'a dyn MonotonicClock,
        interruption: &'a dyn InterruptionSource,
    }

    impl<'a> SpoolControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<SpoolResourceLimits>,
            expected_identity: Option<ContentIdentity>,
            request: SpoolOperationRequest,
            clock: &'a dyn MonotonicClock,
            interruption: &'a dyn InterruptionSource,
        ) -> Result<Self, AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &request.deadline {
                if instant.domain() != clock.domain() {
                    return Err(AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                expected_identity,
                request,
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
        pub fn requested_cleanup(&self) -> TransportCleanupRequirement {
            self.request.cleanup
        }
        pub fn deadline(&self) -> &OperationDeadline {
            &self.request.deadline
        }
        pub fn clock(&self) -> &dyn MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn InterruptionSource {
            self.interruption
        }
        pub fn request(&self) -> &SpoolOperationRequest {
            &self.request
        }
    }

    pub trait SyncSpoolFacility {
        fn descriptor(&self) -> &SpoolFacilityCapabilityDescriptor;

        fn spool(
            &mut self,
            source: &mut dyn SyncByteSource,
            controls: SpoolControls<'_>,
        ) -> SpoolAdapterOutcome;
    }

    pub trait AsyncSpoolFacility {
        type Spool<'a, S>: Future<Output = SpoolAdapterOutcome> + 'a
        where
            Self: 'a,
            S: AsyncByteSource + 'a;

        fn descriptor(&self) -> &SpoolFacilityCapabilityDescriptor;

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
        fn descriptor(&self) -> &SpoolFacilityCapabilityDescriptor {
            unimplemented!()
        }

        fn spool(
            &mut self,
            _source: &mut dyn SyncByteSource,
            _controls: SpoolControls<'_>,
        ) -> SpoolAdapterOutcome {
            unimplemented!()
        }
    }

    impl AsyncSpoolFacility for MemorySpool {
        type Spool<'a, S>
            = std::pin::Pin<Box<dyn Future<Output = SpoolAdapterOutcome> + 'a>>
        where
            Self: 'a,
            S: AsyncByteSource + 'a;

        fn descriptor(&self) -> &SpoolFacilityCapabilityDescriptor {
            unimplemented!()
        }

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
        use super::{SpoolAdapterOutcome, SpoolControls, SyncByteSource, SyncSpoolFacility};
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
            ) -> SpoolAdapterOutcome {
                unimplemented!()
            }
        }

        impl SyncSpoolFacility for NativeSpool {
            fn descriptor(&self) -> &super::SpoolFacilityCapabilityDescriptor {
                unimplemented!()
            }

            fn spool(
                &mut self,
                source: &mut dyn SyncByteSource,
                controls: SpoolControls<'_>,
            ) -> SpoolAdapterOutcome {
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
        receipt: SpoolTransportReceipt,
    }

    impl SpoolOutcome {
        pub(crate) fn try_new(
            _terminal: Result<StableByteValue, SpoolFailure>,
            _receipt: SpoolTransportReceipt,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }

        pub fn terminal(&self) -> &Result<StableByteValue, SpoolFailure> {
            &self.terminal
        }
        pub fn receipt(&self) -> &SpoolTransportReceipt {
            &self.receipt
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SpoolPrimaryFailure {
        Admission(TransportAdmissionRefusalReason),
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
    pub(crate) struct TransportReceipt {
        private: crate::private::TransportReceiptState,
    }

    impl TransportReceipt {
        pub(crate) fn try_new_spool(
            _controls: &SpoolControls<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _content_identity: Option<ContentIdentity>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub(crate) fn try_new_pack_archive_publication(
            _controls: &PackArchivePublicationControls<'_>,
            _archive: &crate::representation::EncodedPackArchive,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _actual_commit: Option<PublicationCommitStrength>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub(crate) fn try_new_project_materialization_publication(
            _controls: &ProjectMaterializationPublicationControls<'_>,
            _plan: &crate::representation::ProjectMaterializationPlan,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _actual_commit: Option<PublicationCommitStrength>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub(crate) fn try_new_closure_export_publication(
            _controls: &ClosureExportPublicationControls<'_>,
            _plan: &crate::representation::ClosureExportPlan,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _actual_commit: Option<PublicationCommitStrength>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub(crate) fn try_new_compilation_delivery(
            _controls: &CompilationDeliveryControls<'_>,
            _transfer: &crate::compilation::CompilationDeliveryTransfer<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _actual_commit: Option<PublicationCommitStrength>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub(crate) fn try_new_pack_archive_acquisition(
            _controls: &AcquisitionTransportControls<'_>,
            _adapter_class: &str,
            _stage: TransportStage,
            _transferred_bytes: u64,
            _content_identity: Option<ContentIdentity>,
            _cleanup: TransportCleanupOutcome,
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn stage(&self) -> TransportStage {
            unimplemented!()
        }

        pub fn role(&self) -> TransportRole {
            unimplemented!()
        }

        pub fn status(&self) -> TransportStatus {
            unimplemented!()
        }

        pub fn object_count(&self) -> u64 {
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
        pub fn identities(&self) -> impl ExactSizeIterator<Item = TransportIdentityRef<'_>> {
            std::iter::empty()
        }
        pub fn subject(&self) -> TransportReceiptSubjectRef<'_> {
            unimplemented!()
        }
        pub fn timing(&self) -> TransportTimingView<'_> {
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
        pub(crate) fn admission(&self) -> TransportAdmissionDispositionView<'_> {
            unimplemented!()
        }
    }

    pub struct TransportAdmissionRefusalView<'a> {
        pub requested_trust: crate::DeploymentTrustProfile,
        pub resource_profile: Option<&'a crate::ResourceProfileIdentity>,
        pub requested_limits: TransportOperationLimitsView<'a>,
        pub requested_network: crate::OperationNetworkPolicy,
        pub covered_roles: &'a [TransportFacilityRole],
        pub contractual_no_network: bool,
        pub requested_structural_network_enforcement: crate::EnforcementStrength,
        pub requested_concurrency: NonZeroUsize,
        pub requested_commit: Option<PublicationCommitStrength>,
        pub requested_cleanup: TransportCleanupRequirement,
        pub interruption: crate::OperationInterruptionStrength,
        pub cancellation_present: bool,
        pub monotonic_domain: &'a crate::MonotonicTimeDomain,
        pub required_enforcement: &'a [crate::EnforcementClaim],
        pub timing_requested: bool,
        pub deadline: &'a OperationDeadline,
        pub reason: TransportAdmissionRefusalReason,
    }

    pub struct TransportAdmissionRecordView<'a> {
        pub requested_trust: crate::DeploymentTrustProfile,
        pub admitted_trust: crate::DeploymentTrustProfile,
        pub resource_profile: Option<&'a crate::ResourceProfileIdentity>,
        pub requested_limits: TransportOperationLimitsView<'a>,
        pub admitted_limits: TransportOperationLimitsView<'a>,
        pub requested_network: crate::OperationNetworkPolicy,
        pub admitted_network: crate::OperationNetworkPolicy,
        pub covered_roles: &'a [TransportFacilityRole],
        pub contractual_no_network: bool,
        pub requested_structural_network_enforcement: crate::EnforcementStrength,
        pub admitted_structural_network_enforcement: crate::EnforcementStrength,
        pub requested_concurrency: NonZeroUsize,
        pub admitted_concurrency: NonZeroUsize,
        pub concurrency_constraints: &'a [crate::AdmissionConstraint],
        pub requested_commit: Option<PublicationCommitStrength>,
        pub admitted_commit: Option<PublicationCommitStrength>,
        pub requested_cleanup: TransportCleanupRequirement,
        pub admitted_cleanup: TransportCleanupRequirement,
        pub requested_interruption: crate::OperationInterruptionStrength,
        pub admitted_interruption: crate::OperationInterruptionStrength,
        pub cancellation_present: bool,
        pub monotonic_domain: &'a crate::MonotonicTimeDomain,
        pub enforcement: crate::EnforcementAdmissionView<'a>,
        pub timing_requested: bool,
        pub timing_reporting_admitted: bool,
        pub deadline: &'a OperationDeadline,
    }

    pub(crate) enum TransportAdmissionDispositionView<'a> {
        Refused(TransportAdmissionRefusalView<'a>),
        Admitted(TransportAdmissionRecordView<'a>),
    }

    pub struct TransportStageLedgerView<'a> {
        private: &'a crate::private::TransportReceiptState,
    }

    impl TransportStageLedgerView<'_> {
        pub fn stages(&self) -> impl ExactSizeIterator<Item = TransportStage> {
            std::iter::empty()
        }

        pub fn primary_terminal_stage(&self) -> TransportStage {
            unimplemented!()
        }

        pub fn transferred_bytes(&self) -> u64 {
            unimplemented!()
        }

        pub fn actual_commit_strength(&self) -> Option<PublicationCommitStrength> {
            unimplemented!()
        }

        pub fn cleanup_outcome(&self) -> TransportCleanupOutcome {
            unimplemented!()
        }

        pub fn residual_locator(&self) -> Option<&ResidualTransportLocator> {
            unimplemented!()
        }

        pub fn exposed_bytes(&self) -> Option<u64> {
            unimplemented!()
        }

        pub fn timing(&self) -> TransportTimingView<'_> {
            unimplemented!()
        }

        pub fn structural_network_enforcement_reached(&self) -> crate::EnforcementStrength {
            unimplemented!()
        }

        pub fn enforcement_reached(&self) -> &[crate::EnforcementClaim] {
            unimplemented!()
        }

        pub fn interruption_winner(&self) -> Option<TransportInterruptionWinner> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportFacilityRole {
        Spool,
        PackArchiveAcquisition,
        PackArchivePublication,
        ProjectMaterializationPublication,
        ClosureExportPublication,
        CompilationDelivery,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportInterruptionWinner {
        TerminalCommitment,
        Cancellation,
        Deadline,
    }

    macro_rules! role_transport_receipt {
        ($name:ident, $state:ident, $refusal:ident, $record:ident, $descriptor:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                private: crate::private::TransportReceiptState,
            }

            pub struct $refusal<'a> {
                pub common: TransportAdmissionRefusalView<'a>,
                pub descriptor: &'a $descriptor,
            }

            pub struct $record<'a> {
                pub common: TransportAdmissionRecordView<'a>,
                pub descriptor: &'a $descriptor,
            }

            pub enum $state<'a> {
                Refused($refusal<'a>),
                Admitted {
                    admission: $record<'a>,
                    stage_ledger: TransportStageLedgerView<'a>,
                },
            }

            impl $name {
                pub fn status(&self) -> TransportStatus {
                    unimplemented!()
                }

                pub fn state(&self) -> $state<'_> {
                    unimplemented!()
                }
            }
        };
    }

    role_transport_receipt!(
        SpoolTransportReceipt,
        SpoolTransportReceiptStateView,
        SpoolTransportAdmissionRefusalView,
        SpoolTransportAdmissionRecordView,
        SpoolFacilityCapabilityDescriptor
    );
    role_transport_receipt!(
        PackArchiveAcquisitionTransportReceipt,
        PackArchiveAcquisitionTransportReceiptStateView,
        PackArchiveAcquisitionTransportAdmissionRefusalView,
        PackArchiveAcquisitionTransportAdmissionRecordView,
        PackArchiveAcquirerCapabilityDescriptor
    );
    role_transport_receipt!(
        PackArchivePublicationTransportReceipt,
        PackArchivePublicationTransportReceiptStateView,
        PackArchivePublicationTransportAdmissionRefusalView,
        PackArchivePublicationTransportAdmissionRecordView,
        PackArchivePublisherCapabilityDescriptor
    );
    role_transport_receipt!(
        ProjectMaterializationPublicationTransportReceipt,
        ProjectMaterializationPublicationTransportReceiptStateView,
        ProjectMaterializationPublicationTransportAdmissionRefusalView,
        ProjectMaterializationPublicationTransportAdmissionRecordView,
        ProjectMaterializationPublisherCapabilityDescriptor
    );
    role_transport_receipt!(
        ClosureExportPublicationTransportReceipt,
        ClosureExportPublicationTransportReceiptStateView,
        ClosureExportPublicationTransportAdmissionRefusalView,
        ClosureExportPublicationTransportAdmissionRecordView,
        ClosureExportPublisherCapabilityDescriptor
    );
    role_transport_receipt!(
        CompilationDeliveryTransportReceipt,
        CompilationDeliveryTransportReceiptStateView,
        CompilationDeliveryTransportAdmissionRefusalView,
        CompilationDeliveryTransportAdmissionRecordView,
        CompilationDeliveryCapabilityDescriptor
    );

    impl SpoolTransportReceipt {
        pub fn expected_content_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn actual_content_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
    }

    impl PackArchiveAcquisitionTransportReceipt {
        pub fn expected_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn acquired_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
    }

    impl PackArchivePublicationTransportReceipt {
        pub fn source_archive_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }
        pub fn output_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn archive_encoding_identity(&self) -> &crate::representation::ArchiveEncodingIdentity {
            unimplemented!()
        }
    }

    impl ProjectMaterializationPublicationTransportReceipt {
        pub fn pack_identity(&self) -> &crate::pack::PackIdentity {
            unimplemented!()
        }
    }

    impl ClosureExportPublicationTransportReceipt {
        pub fn pack_identity(&self) -> &crate::pack::PackIdentity {
            unimplemented!()
        }
        pub fn source_tree_identity(&self) -> &crate::ClosureExportTreeContentIdentity {
            unimplemented!()
        }
        pub fn output_tree_identity(&self) -> Option<&crate::ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
    }

    impl CompilationDeliveryTransportReceipt {
        pub fn compilation_identity(&self) -> &crate::CompilationIdentity {
            unimplemented!()
        }
        pub fn result_identity(&self) -> Option<&crate::CompilationResultIdentity> {
            unimplemented!()
        }
        pub fn artifact_identities(
            &self,
        ) -> impl ExactSizeIterator<Item = &crate::CompilationArtifactIdentity> {
            std::iter::empty()
        }
    }

    pub enum TransportOperationLimitsView<'a> {
        Spool(&'a SpoolResourceLimits),
        Transfer(&'a TransportResourceLimits),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportStage {
        Admission,
        PlanFreeze,
        ReferenceResolution,
        Acquisition,
        Spooling,
        Transfer,
        Verification,
        Commit,
        Cleanup,
        Complete,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum TransportRole {
        Spool,
        PackArchiveAcquisition,
        PackArchivePublication,
        ProjectMaterializationPublication,
        ClosureExportPublication,
        CompilationDelivery,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportStatus {
        Refused,
        Transferred,
        Committed,
        Failed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportAdmissionRefusalReason {
        DeploymentTrustProfileUnavailable,
        OperationNetworkPolicyUnavailable,
        StructuralNetworkEnforcementUnavailable,
        OperationResourceLimitsUnavailable,
        TransportConcurrencyUnavailable,
        InterruptionContractUnavailable,
        PublicationCommitStrengthUnavailable,
        CleanupRequirementUnavailable,
        RequiredEnforcementUnavailable,
        ReportingUnavailable,
        CapacityUnavailable,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportTimingStatus {
        NotRequested,
        Complete,
        Limited,
        Unavailable,
    }

    pub struct TransportTimingView<'a> {
        pub status: TransportTimingStatus,
        pub phases: &'a [TransportPhaseTiming],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct TransportPhaseTiming {
        pub stage: TransportStage,
        pub elapsed_ticks: u64,
    }

    pub enum TransportTimingInput {
        NotRequested,
        Complete(Vec<TransportPhaseTiming>),
        Limited(Vec<TransportPhaseTiming>),
        Unavailable,
    }

    pub(crate) enum TransportIdentityRef<'a> {
        Content(&'a ContentIdentity),
        Pack(&'a crate::pack::PackIdentity),
        ArchiveEncoding(&'a crate::representation::ArchiveEncodingIdentity),
        ClosureExportTree(&'a crate::ClosureExportTreeContentIdentity),
        Compilation(&'a crate::CompilationIdentity),
        Result(&'a crate::CompilationResultIdentity),
        Artifact(&'a crate::CompilationArtifactIdentity),
    }

    pub(crate) enum TransportReceiptSubjectRef<'a> {
        Spool {
            expected: Option<&'a ContentIdentity>,
            actual: Option<&'a ContentIdentity>,
        },
        PackArchiveAcquisition {
            expected_archive: Option<&'a ContentIdentity>,
            acquired_archive: Option<&'a ContentIdentity>,
        },
        PackArchivePublication {
            source_archive: &'a ContentIdentity,
            output_archive: Option<&'a ContentIdentity>,
            archive_encoding: &'a crate::representation::ArchiveEncodingIdentity,
        },
        ProjectMaterializationPublication {
            pack: &'a crate::pack::PackIdentity,
        },
        ClosureExportPublication {
            pack: &'a crate::pack::PackIdentity,
            source_tree: &'a crate::ClosureExportTreeContentIdentity,
            output_tree: Option<&'a crate::ClosureExportTreeContentIdentity>,
        },
        CompilationDelivery {
            compilation: &'a crate::CompilationIdentity,
            result: Option<&'a crate::CompilationResultIdentity>,
            artifacts: &'a [crate::CompilationArtifactIdentity],
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportReceiptRejection {
        WrongRole,
        WrongSubject,
        IncoherentStage,
        IncoherentStatus,
        IncoherentIdentity,
        IncoherentObjectCount,
        IncoherentByteCount,
        IncoherentCommit,
        IncoherentCleanup,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PublicationCommitStrength {
        CompleteCollectionAtomic,
        EachObjectAtomic,
        Streaming,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportCleanupRequirement {
        CompleteBeforeReturn,
        ResidualLocatorPermitted,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportCleanupOutcome {
        NotRequired,
        Complete,
        ResidualReported,
        CleanupFailed,
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

    macro_rules! non_publication_transport_capability_spec {
        ($name:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                pub class: crate::OperationalCapabilityClass,
                pub network: crate::SelectedNetworkContract,
                pub transfer_concurrency: NonZeroUsize,
                pub cleanup_requirements: Vec<TransportCleanupRequirement>,
                pub interruption: crate::OperationInterruptionStrength,
                pub enforcement: Vec<crate::EnforcementClaim>,
                pub timing_reporting: bool,
            }
        };
    }

    macro_rules! publication_transport_capability_spec {
        ($name:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                pub class: crate::OperationalCapabilityClass,
                pub network: crate::SelectedNetworkContract,
                pub transfer_concurrency: NonZeroUsize,
                pub commit_strengths: Vec<PublicationCommitStrength>,
                pub cleanup_requirements: Vec<TransportCleanupRequirement>,
                pub interruption: crate::OperationInterruptionStrength,
                pub enforcement: Vec<crate::EnforcementClaim>,
                pub timing_reporting: bool,
            }
        };
    }

    non_publication_transport_capability_spec!(SpoolFacilityCapabilitySpec);
    non_publication_transport_capability_spec!(PackArchiveAcquirerCapabilitySpec);
    publication_transport_capability_spec!(PackArchivePublisherCapabilitySpec);
    publication_transport_capability_spec!(ProjectMaterializationPublisherCapabilitySpec);
    publication_transport_capability_spec!(ClosureExportPublisherCapabilitySpec);
    publication_transport_capability_spec!(CompilationDeliveryCapabilitySpec);

    macro_rules! transport_capability_descriptor {
        ($name:ident, $state:ident, $spec:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                private: crate::private::$state,
            }

            impl $name {
                pub fn try_new(_spec: $spec) -> Result<Self, TransportAdmissionRefusalReason> {
                    unimplemented!()
                }

                pub fn descriptor_version(&self) -> u32 {
                    1
                }

                pub fn class(&self) -> &crate::OperationalCapabilityClass {
                    unimplemented!()
                }

                pub fn network(&self) -> crate::SelectedNetworkContract {
                    unimplemented!()
                }

                pub fn transfer_concurrency(&self) -> NonZeroUsize {
                    unimplemented!()
                }

                pub fn cleanup_requirements(&self) -> &[TransportCleanupRequirement] {
                    unimplemented!()
                }

                pub fn interruption(&self) -> crate::OperationInterruptionStrength {
                    unimplemented!()
                }

                pub fn enforcement(&self) -> &[crate::EnforcementClaim] {
                    unimplemented!()
                }

                pub fn timing_reporting(&self) -> bool {
                    unimplemented!()
                }
            }
        };
    }

    transport_capability_descriptor!(
        PackArchiveAcquirerCapabilityDescriptor,
        PackArchiveAcquirerCapabilityDescriptorState,
        PackArchiveAcquirerCapabilitySpec
    );
    transport_capability_descriptor!(
        PackArchivePublisherCapabilityDescriptor,
        PackArchivePublisherCapabilityDescriptorState,
        PackArchivePublisherCapabilitySpec
    );
    transport_capability_descriptor!(
        ProjectMaterializationPublisherCapabilityDescriptor,
        ProjectMaterializationPublisherCapabilityDescriptorState,
        ProjectMaterializationPublisherCapabilitySpec
    );
    transport_capability_descriptor!(
        ClosureExportPublisherCapabilityDescriptor,
        ClosureExportPublisherCapabilityDescriptorState,
        ClosureExportPublisherCapabilitySpec
    );
    transport_capability_descriptor!(
        CompilationDeliveryCapabilityDescriptor,
        CompilationDeliveryCapabilityDescriptorState,
        CompilationDeliveryCapabilitySpec
    );
    transport_capability_descriptor!(
        SpoolFacilityCapabilityDescriptor,
        SpoolFacilityCapabilityDescriptorState,
        SpoolFacilityCapabilitySpec
    );

    macro_rules! publication_commit_strengths {
        ($($name:ident),+ $(,)?) => {
            $(
                impl $name {
                    pub fn commit_strengths(&self) -> &[PublicationCommitStrength] {
                        unimplemented!()
                    }
                }
            )+
        };
    }

    publication_commit_strengths!(
        PackArchivePublisherCapabilityDescriptor,
        ProjectMaterializationPublisherCapabilityDescriptor,
        ClosureExportPublisherCapabilityDescriptor,
        CompilationDeliveryCapabilityDescriptor,
    );

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportAdapterStage {
        ReferenceResolution,
        Acquisition,
        Spooling,
        Transfer,
        Verification,
        Commit,
        Cleanup,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct TransportAdapterPhaseTiming {
        pub stage: TransportAdapterStage,
        pub elapsed_ticks: u64,
    }

    pub enum TransportAdapterTimingInput {
        NotRequested,
        Complete(Vec<TransportAdapterPhaseTiming>),
        Limited(Vec<TransportAdapterPhaseTiming>),
        Unavailable,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum SpoolAdapterFailure {
        Source(ByteSourceFailure),
        ExpectedIdentityMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        InternalIntegrity,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PackArchiveAcquisitionAdapterFailure {
        Acquisition,
        Transfer,
        ExpectedIdentityMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        AdapterContractViolation,
    }

    macro_rules! publication_transport_adapter_failure {
        ($($name:ident),+ $(,)?) => {
            $(
                #[derive(Clone, Debug, Eq, PartialEq)]
                pub enum $name {
                    Transfer,
                    ResourceLimit,
                    Commit,
                    Cancelled,
                    Deadline,
                    AdapterContractViolation,
                }
            )+
        };
    }

    publication_transport_adapter_failure!(
        PackArchivePublicationAdapterFailure,
        ProjectMaterializationPublicationAdapterFailure,
        ClosureExportPublicationAdapterFailure,
        CompilationDeliveryAdapterFailure,
    );

    macro_rules! non_publication_transport_adapter_outcome {
        ($name:ident, $terminal:ty, $failure:ty) => {
            pub struct $name {
                terminal: Result<$terminal, $failure>,
                stages: Vec<TransportAdapterStage>,
                transferred_bytes: u64,
                cleanup: TransportCleanupOutcome,
                residual: Option<ResidualTransportLocator>,
                exposed_bytes: Option<u64>,
                timing: TransportAdapterTimingInput,
            }

            impl $name {
                #[allow(clippy::too_many_arguments)]
                pub fn try_new(
                    _terminal: Result<$terminal, $failure>,
                    _stages: Vec<TransportAdapterStage>,
                    _transferred_bytes: u64,
                    _cleanup: TransportCleanupOutcome,
                    _residual: Option<ResidualTransportLocator>,
                    _exposed_bytes: Option<u64>,
                    _timing: TransportAdapterTimingInput,
                ) -> Result<Self, TransportOutcomeRejection> {
                    unimplemented!()
                }
            }
        };
    }

    macro_rules! publication_transport_adapter_outcome {
        ($name:ident, $failure:ty) => {
            pub struct $name {
                terminal: Result<(), $failure>,
                stages: Vec<TransportAdapterStage>,
                transferred_bytes: u64,
                actual_commit: Option<PublicationCommitStrength>,
                cleanup: TransportCleanupOutcome,
                residual: Option<ResidualTransportLocator>,
                exposed_bytes: Option<u64>,
                timing: TransportAdapterTimingInput,
            }

            impl $name {
                #[allow(clippy::too_many_arguments)]
                pub fn try_new(
                    _terminal: Result<(), $failure>,
                    _stages: Vec<TransportAdapterStage>,
                    _transferred_bytes: u64,
                    _actual_commit: Option<PublicationCommitStrength>,
                    _cleanup: TransportCleanupOutcome,
                    _residual: Option<ResidualTransportLocator>,
                    _exposed_bytes: Option<u64>,
                    _timing: TransportAdapterTimingInput,
                ) -> Result<Self, TransportOutcomeRejection> {
                    unimplemented!()
                }
            }
        };
    }

    non_publication_transport_adapter_outcome!(
        SpoolAdapterOutcome,
        StableByteValue,
        SpoolAdapterFailure
    );
    non_publication_transport_adapter_outcome!(
        PackArchiveAcquisitionAdapterOutcome,
        StableByteValue,
        PackArchiveAcquisitionAdapterFailure
    );
    publication_transport_adapter_outcome!(
        PackArchivePublicationAdapterOutcome,
        PackArchivePublicationAdapterFailure
    );
    publication_transport_adapter_outcome!(
        ProjectMaterializationPublicationAdapterOutcome,
        ProjectMaterializationPublicationAdapterFailure
    );
    publication_transport_adapter_outcome!(
        ClosureExportPublicationAdapterOutcome,
        ClosureExportPublicationAdapterFailure
    );
    publication_transport_adapter_outcome!(
        CompilationDeliveryAdapterOutcome,
        CompilationDeliveryAdapterFailure
    );

    pub trait SyncPackArchiveAcquirer {
        type Locator: ?Sized;

        fn descriptor(&self) -> &PackArchiveAcquirerCapabilityDescriptor;

        fn acquire(
            &self,
            locator: &Self::Locator,
            controls: AcquisitionTransportControls<'_>,
        ) -> PackArchiveAcquisitionAdapterOutcome;
    }

    pub trait AsyncPackArchiveAcquirer {
        type Locator: ?Sized;
        type Acquire<'a>: Future<Output = PackArchiveAcquisitionAdapterOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &PackArchiveAcquirerCapabilityDescriptor;

        fn acquire<'a>(
            &'a self,
            locator: &'a Self::Locator,
            controls: AcquisitionTransportControls<'a>,
        ) -> Self::Acquire<'a>;
    }

    pub trait SyncCompilationDelivery {
        type Destination: ?Sized;

        fn descriptor(&self) -> &CompilationDeliveryCapabilityDescriptor;

        fn deliver(
            &self,
            transfer: crate::compilation::CompilationDeliveryTransfer<'_>,
            destination: &Self::Destination,
            controls: CompilationDeliveryControls<'_>,
        ) -> CompilationDeliveryAdapterOutcome;
    }

    pub trait AsyncCompilationDelivery {
        type Destination: ?Sized;
        type Deliver<'a>: Future<Output = CompilationDeliveryAdapterOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &CompilationDeliveryCapabilityDescriptor;

        fn deliver<'a>(
            &'a self,
            transfer: crate::compilation::CompilationDeliveryTransfer<'a>,
            destination: &'a Self::Destination,
            controls: CompilationDeliveryControls<'a>,
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
        request: PackArchiveAcquisitionOperationRequest,
        clock: &'a dyn MonotonicClock,
        interruption: &'a dyn InterruptionSource,
    }

    impl<'a> AcquisitionTransportControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
            expected_identity: Option<ContentIdentity>,
            request: PackArchiveAcquisitionOperationRequest,
            clock: &'a dyn MonotonicClock,
            interruption: &'a dyn InterruptionSource,
        ) -> Result<Self, AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &request.deadline {
                if instant.domain() != clock.domain() {
                    return Err(AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                expected_identity,
                request,
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
        pub fn requested_cleanup(&self) -> TransportCleanupRequirement {
            self.request.cleanup
        }
        pub fn deadline(&self) -> &OperationDeadline {
            &self.request.deadline
        }
        pub fn clock(&self) -> &dyn MonotonicClock {
            self.clock
        }
        pub fn interruption(&self) -> &dyn InterruptionSource {
            self.interruption
        }
        pub fn request(&self) -> &PackArchiveAcquisitionOperationRequest {
            &self.request
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

    macro_rules! publication_transport_controls {
        ($controls:ident, $request:ident) => {
            pub struct $controls<'a> {
                admission: OrdinaryAdmission,
                limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
                request: $request,
                clock: &'a dyn MonotonicClock,
                interruption: &'a dyn InterruptionSource,
            }

            impl<'a> $controls<'a> {
                pub fn try_new(
                    admission: OrdinaryAdmission,
                    limits: AdmittedOperationResourceLimits<TransportResourceLimits>,
                    request: $request,
                    clock: &'a dyn MonotonicClock,
                    interruption: &'a dyn InterruptionSource,
                ) -> Result<Self, AdmissionRefusal> {
                    if let OperationDeadline::At(instant) = &request.deadline {
                        if instant.domain() != clock.domain() {
                            return Err(AdmissionRefusal::MissingEnforcementCapability);
                        }
                    }
                    Ok(Self {
                        admission,
                        limits,
                        request,
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
                pub fn request(&self) -> &$request {
                    &self.request
                }
                pub fn requested_commit(&self) -> PublicationCommitStrength {
                    self.request.commit
                }
                pub fn requested_cleanup(&self) -> TransportCleanupRequirement {
                    self.request.cleanup
                }
                pub fn deadline(&self) -> &OperationDeadline {
                    &self.request.deadline
                }
                pub fn clock(&self) -> &dyn MonotonicClock {
                    self.clock
                }
                pub fn interruption(&self) -> &dyn InterruptionSource {
                    self.interruption
                }
            }
        };
    }

    publication_transport_controls!(
        PackArchivePublicationControls,
        PackArchivePublicationOperationRequest
    );
    publication_transport_controls!(
        ProjectMaterializationPublicationControls,
        ProjectMaterializationPublicationOperationRequest
    );
    publication_transport_controls!(
        ClosureExportPublicationControls,
        ClosureExportPublicationOperationRequest
    );
    publication_transport_controls!(
        CompilationDeliveryControls,
        CompilationDeliveryOperationRequest
    );

    pub(crate) struct TransportOutcome<T> {
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

    macro_rules! role_transport_outcome {
        ($name:ident, $value:ty, $receipt:ty) => {
            pub struct $name {
                terminal: Result<$value, TransportFailure>,
                receipt: $receipt,
            }

            impl $name {
                pub fn terminal(&self) -> &Result<$value, TransportFailure> {
                    &self.terminal
                }

                pub fn receipt(&self) -> &$receipt {
                    &self.receipt
                }
            }
        };
    }

    role_transport_outcome!(
        PackArchiveAcquisitionOutcome,
        StableByteValue,
        PackArchiveAcquisitionTransportReceipt
    );
    role_transport_outcome!(
        PackArchivePublicationTransportOutcome,
        (),
        PackArchivePublicationTransportReceipt
    );
    role_transport_outcome!(
        ProjectMaterializationPublicationTransportOutcome,
        (),
        ProjectMaterializationPublicationTransportReceipt
    );
    role_transport_outcome!(
        ClosureExportPublicationTransportOutcome,
        (),
        ClosureExportPublicationTransportReceipt
    );
    role_transport_outcome!(
        CompilationDeliveryTransportOutcome,
        (),
        CompilationDeliveryTransportReceipt
    );

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportOutcomeRejection {
        SuccessBeforeRequiredStage,
        FailureAfterCommit,
        IncoherentCleanup,
        WrongReport,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum TransportPrimaryFailure {
        Admission(TransportAdmissionRefusalReason),
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
        transport: CompilationDeliveryTransportOutcome,
    }

    impl CompilationDeliveryOutcome {
        pub(crate) fn try_new(
            _plan: crate::compilation::CompilationDeliveryPlan,
            _transport: CompilationDeliveryTransportOutcome,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }

        pub fn report(&self) -> &crate::CompilationReport {
            &self.report
        }
        pub fn transport(&self) -> &CompilationDeliveryTransportOutcome {
            &self.transport
        }
    }

    pub fn deliver_compilation_sync<D: SyncCompilationDelivery + ?Sized>(
        _plan: crate::compilation::CompilationDeliveryPlan,
        _delivery: &D,
        _destination: &D::Destination,
        _controls: CompilationDeliveryControls<'_>,
    ) -> CompilationDeliveryOutcome {
        unimplemented!()
    }

    pub async fn deliver_compilation_async<D: AsyncCompilationDelivery + ?Sized>(
        _plan: crate::compilation::CompilationDeliveryPlan,
        _delivery: &D,
        _destination: &D::Destination,
        _controls: CompilationDeliveryControls<'_>,
    ) -> CompilationDeliveryOutcome {
        unimplemented!()
    }

    pub struct PackArchivePublicationOutcome {
        format: crate::representation::PackArchivePublicationFormatReceipt,
        transport: PackArchivePublicationTransportOutcome,
    }

    impl PackArchivePublicationOutcome {
        pub(crate) fn try_new(
            _archive: &crate::representation::EncodedPackArchive,
            _transport: PackArchivePublicationTransportOutcome,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }
        pub fn format(&self) -> &crate::representation::PackArchivePublicationFormatReceipt {
            &self.format
        }
        pub fn transport(&self) -> &PackArchivePublicationTransportOutcome {
            &self.transport
        }
    }

    pub struct ClosureExportPublicationOutcome {
        format: crate::representation::ClosureExportPublicationFormatReceipt,
        transport: ClosureExportPublicationTransportOutcome,
    }

    impl ClosureExportPublicationOutcome {
        pub(crate) fn try_new(
            _plan: &crate::representation::ClosureExportPlan,
            _transport: ClosureExportPublicationTransportOutcome,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }
        pub fn format(&self) -> &crate::representation::ClosureExportPublicationFormatReceipt {
            &self.format
        }
        pub fn transport(&self) -> &ClosureExportPublicationTransportOutcome {
            &self.transport
        }
    }

    pub struct ProjectMaterializationPublicationOutcome {
        transport: ProjectMaterializationPublicationTransportOutcome,
    }

    impl ProjectMaterializationPublicationOutcome {
        pub(crate) fn try_new(
            _plan: &crate::representation::ProjectMaterializationPlan,
            _transport: ProjectMaterializationPublicationTransportOutcome,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }
        pub fn transport(&self) -> &ProjectMaterializationPublicationTransportOutcome {
            &self.transport
        }
    }

    pub trait SyncPackArchivePublisher {
        type Destination: ?Sized;

        fn descriptor(&self) -> &PackArchivePublisherCapabilityDescriptor;

        fn publish(
            &self,
            archive: &crate::representation::EncodedPackArchive,
            destination: &Self::Destination,
            controls: PackArchivePublicationControls<'_>,
        ) -> PackArchivePublicationAdapterOutcome;
    }

    pub trait SyncProjectMaterializationPublisher {
        type Destination: ?Sized;

        fn descriptor(&self) -> &ProjectMaterializationPublisherCapabilityDescriptor;

        fn publish(
            &self,
            plan: &crate::representation::ProjectMaterializationPlan,
            destination: &Self::Destination,
            controls: ProjectMaterializationPublicationControls<'_>,
        ) -> ProjectMaterializationPublicationAdapterOutcome;
    }

    pub trait SyncClosureExportPublisher {
        type Destination: ?Sized;

        fn descriptor(&self) -> &ClosureExportPublisherCapabilityDescriptor;

        fn publish(
            &self,
            plan: &crate::representation::ClosureExportPlan,
            destination: &Self::Destination,
            controls: ClosureExportPublicationControls<'_>,
        ) -> ClosureExportPublicationAdapterOutcome;
    }

    pub trait AsyncPackArchivePublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = PackArchivePublicationAdapterOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &PackArchivePublisherCapabilityDescriptor;

        fn publish<'a>(
            &'a self,
            archive: &'a crate::representation::EncodedPackArchive,
            destination: &'a Self::Destination,
            controls: PackArchivePublicationControls<'a>,
        ) -> Self::Publish<'a>;
    }

    pub trait AsyncProjectMaterializationPublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = ProjectMaterializationPublicationAdapterOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &ProjectMaterializationPublisherCapabilityDescriptor;

        fn publish<'a>(
            &'a self,
            plan: &'a crate::representation::ProjectMaterializationPlan,
            destination: &'a Self::Destination,
            controls: ProjectMaterializationPublicationControls<'a>,
        ) -> Self::Publish<'a>;
    }

    pub trait AsyncClosureExportPublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = ClosureExportPublicationAdapterOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &ClosureExportPublisherCapabilityDescriptor;

        fn publish<'a>(
            &'a self,
            plan: &'a crate::representation::ClosureExportPlan,
            destination: &'a Self::Destination,
            controls: ClosureExportPublicationControls<'a>,
        ) -> Self::Publish<'a>;
    }

    pub fn spool_sync<S: SyncSpoolFacility + ?Sized>(
        _facility: &mut S,
        _source: &mut dyn SyncByteSource,
        _controls: SpoolControls<'_>,
    ) -> SpoolOutcome {
        unimplemented!()
    }

    pub async fn spool_async<S, B>(
        _facility: &mut S,
        _source: &mut B,
        _controls: SpoolControls<'_>,
    ) -> SpoolOutcome
    where
        S: AsyncSpoolFacility + ?Sized,
        B: AsyncByteSource,
    {
        unimplemented!()
    }

    pub fn acquire_pack_archive_sync<A: SyncPackArchiveAcquirer + ?Sized>(
        _acquirer: &A,
        _locator: &A::Locator,
        _controls: AcquisitionTransportControls<'_>,
    ) -> PackArchiveAcquisitionOutcome {
        unimplemented!()
    }

    pub async fn acquire_pack_archive_async<A: AsyncPackArchiveAcquirer + ?Sized>(
        _acquirer: &A,
        _locator: &A::Locator,
        _controls: AcquisitionTransportControls<'_>,
    ) -> PackArchiveAcquisitionOutcome {
        unimplemented!()
    }

    pub fn publish_pack_archive_sync<P: SyncPackArchivePublisher + ?Sized>(
        _archive: &crate::representation::EncodedPackArchive,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: PackArchivePublicationControls<'_>,
    ) -> PackArchivePublicationOutcome {
        unimplemented!()
    }

    pub async fn publish_pack_archive_async<P: AsyncPackArchivePublisher + ?Sized>(
        _archive: &crate::representation::EncodedPackArchive,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: PackArchivePublicationControls<'_>,
    ) -> PackArchivePublicationOutcome {
        unimplemented!()
    }

    pub fn publish_project_materialization_sync<P: SyncProjectMaterializationPublisher + ?Sized>(
        _plan: &crate::representation::ProjectMaterializationPlan,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: ProjectMaterializationPublicationControls<'_>,
    ) -> ProjectMaterializationPublicationOutcome {
        unimplemented!()
    }

    pub async fn publish_project_materialization_async<
        P: AsyncProjectMaterializationPublisher + ?Sized,
    >(
        _plan: &crate::representation::ProjectMaterializationPlan,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: ProjectMaterializationPublicationControls<'_>,
    ) -> ProjectMaterializationPublicationOutcome {
        unimplemented!()
    }

    pub fn publish_closure_export_sync<P: SyncClosureExportPublisher + ?Sized>(
        _plan: &crate::representation::ClosureExportPlan,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: ClosureExportPublicationControls<'_>,
    ) -> ClosureExportPublicationOutcome {
        unimplemented!()
    }

    pub async fn publish_closure_export_async<P: AsyncClosureExportPublisher + ?Sized>(
        _plan: &crate::representation::ClosureExportPlan,
        _publisher: &P,
        _destination: &P::Destination,
        _controls: ClosureExportPublicationControls<'_>,
    ) -> ClosureExportPublicationOutcome {
        unimplemented!()
    }
}

pub mod pack {
    use super::{
        ContentIdentity, DiscoveryCoverageIdentity, DiscoveryRequestCommitment,
        DiscoveryTraceIdentity, DiscoveryVariantIdentity, DomainValueRejection,
        FontRequirementIdentity, OrdinaryAdmission, PackageRequirementIdentity, ProjectPath,
        ProjectTreeIdentity,
    };
    use std::num::NonZeroU32;
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

        pub fn discovery_engine(&self) -> DiscoveryEngineInspection<'_> {
            unimplemented!()
        }

        pub fn project_tree(&self) -> ProjectTreeInspection<'_> {
            unimplemented!()
        }

        pub fn explicit_conditional_inclusions(
            &self,
        ) -> impl ExactSizeIterator<Item = &ProjectPath> {
            std::iter::empty()
        }

        pub fn discovery_variants(
            &self,
        ) -> impl ExactSizeIterator<Item = DiscoveryVariantInspection<'_>> {
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

        pub fn font_catalog(
            &self,
        ) -> impl ExactSizeIterator<Item = FontFaceIdentityInspection<'_>> {
            std::iter::empty()
        }

        pub fn metadata(&self) -> &PackMetadata {
            unimplemented!()
        }

        pub fn semantic_extensions(
            &self,
        ) -> impl ExactSizeIterator<Item = PackSemanticExtensionInspection<'_>> {
            std::iter::empty()
        }

        pub fn annotations(&self) -> impl ExactSizeIterator<Item = &PackAnnotation> {
            std::iter::empty()
        }

        pub fn guarantees(&self) -> PackGuaranteesInspection {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PackMetadata {
        private: crate::private::PackMetadataState,
    }

    impl PackMetadata {
        pub fn empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _admission: &OrdinaryAdmission,
            _limits: &crate::AdmittedOperationResourceLimits<
                crate::creation::CreationResourceLimits,
            >,
            _title: Option<String>,
            _description: Option<String>,
            _authors: impl IntoIterator<Item = String>,
            _keywords: impl IntoIterator<Item = String>,
        ) -> Result<Self, crate::creation::CreationRequestRejection> {
            unimplemented!()
        }

        pub fn title(&self) -> Option<&str> {
            unimplemented!()
        }
        pub fn description(&self) -> Option<&str> {
            unimplemented!()
        }
        pub fn authors(&self) -> impl ExactSizeIterator<Item = &str> {
            std::iter::empty()
        }
        pub fn keywords(&self) -> impl ExactSizeIterator<Item = &str> {
            std::iter::empty()
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct PackAnnotationIdentifier(crate::private::PackAnnotationIdentifierState);

    impl PackAnnotationIdentifier {
        pub fn parse(
            _admission: &OrdinaryAdmission,
            _limits: &crate::AdmittedOperationResourceLimits<
                crate::creation::CreationResourceLimits,
            >,
            _value: &str,
        ) -> Result<Self, crate::creation::CreationRequestRejection> {
            unimplemented!()
        }

        pub fn as_str(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PackAnnotation {
        private: crate::private::PackAnnotationState,
    }

    impl PackAnnotation {
        pub fn try_new(
            _admission: &OrdinaryAdmission,
            _limits: &crate::AdmittedOperationResourceLimits<
                crate::creation::CreationResourceLimits,
            >,
            _declaration_ordinal: Option<u64>,
            _identifier: PackAnnotationIdentifier,
            _epoch: NonZeroU32,
            _payload: Arc<[u8]>,
        ) -> Result<Self, crate::creation::CreationRequestRejection> {
            unimplemented!()
        }

        pub fn identifier(&self) -> &PackAnnotationIdentifier {
            unimplemented!()
        }
        pub fn epoch(&self) -> NonZeroU32 {
            unimplemented!()
        }
        pub fn payload(&self) -> &[u8] {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum FileRequestKind {
        TypstSource,
        RawFile,
    }

    pub struct DiscoveryEngineInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl DiscoveryEngineInspection<'_> {
        pub fn identity(&self) -> &crate::EngineIdentity {
            unimplemented!()
        }
        pub fn producer_id(&self) -> &str {
            unimplemented!()
        }
        pub fn implementation_name(&self) -> &str {
            unimplemented!()
        }
        pub fn implementation_version(&self) -> VersionInspection {
            unimplemented!()
        }
        pub fn exact_build_fingerprint(&self) -> &[u8] {
            unimplemented!()
        }
        pub fn target_profile(&self) -> &str {
            unimplemented!()
        }
        pub fn qualifiers(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
            std::iter::empty()
        }
        pub fn unicode_xid_version(&self) -> VersionInspection {
            unimplemented!()
        }
        pub fn package_metadata_profile_id(&self) -> &str {
            unimplemented!()
        }
        pub fn font_metadata_profile_id(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct VersionInspection {
        pub major: u32,
        pub minor: u32,
        pub patch: u32,
    }

    pub struct ProjectTreeInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl ProjectTreeInspection<'_> {
        pub fn identity(&self) -> &ProjectTreeIdentity {
            unimplemented!()
        }
        pub fn file_count(&self) -> u32 {
            unimplemented!()
        }
        pub fn aggregate_bytes(&self) -> u64 {
            unimplemented!()
        }
        pub fn files(&self) -> impl ExactSizeIterator<Item = ProjectFileInspection<'_>> {
            std::iter::empty()
        }
    }

    pub struct ProjectFileInspection<'a> {
        pub path: &'a ProjectPath,
        pub content_identity: &'a ContentIdentity,
        pub exact_bytes: u64,
    }

    pub struct DiscoveryVariantInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl DiscoveryVariantInspection<'_> {
        pub fn declaration_ordinal(&self) -> u32 {
            unimplemented!()
        }
        pub fn label(&self) -> Option<&str> {
            unimplemented!()
        }
        pub fn identity(&self) -> &DiscoveryVariantIdentity {
            unimplemented!()
        }
        pub fn request(&self) -> DiscoveryCoverageRequestInspection<'_> {
            unimplemented!()
        }
        pub fn trace(&self) -> DiscoveryTraceInspection<'_> {
            unimplemented!()
        }
        pub fn trace_identity(&self) -> &DiscoveryTraceIdentity {
            unimplemented!()
        }
        pub fn coverage_identity(&self) -> &DiscoveryCoverageIdentity {
            unimplemented!()
        }
    }

    pub struct DiscoveryCoverageRequestInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl DiscoveryCoverageRequestInspection<'_> {
        pub fn target(&self) -> crate::compilation::TypstTarget {
            unimplemented!()
        }
        pub fn inputs(&self) -> impl ExactSizeIterator<Item = DiscoveryInputInspection<'_>> {
            std::iter::empty()
        }
        pub fn document_time(&self) -> crate::compilation::CompilationDocumentTime {
            unimplemented!()
        }
        pub fn features(
            &self,
        ) -> impl ExactSizeIterator<Item = &crate::compilation::EngineFeature> {
            std::iter::empty()
        }
        pub fn overrides(&self) -> impl ExactSizeIterator<Item = DiscoveryOverrideInspection<'_>> {
            std::iter::empty()
        }
    }

    pub struct DiscoverySensitiveValueInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl DiscoverySensitiveValueInspection<'_> {
        pub fn exact_bytes(&self) -> u64 {
            unimplemented!()
        }
        pub fn commitment(&self) -> &DiscoveryRequestCommitment {
            unimplemented!()
        }
    }

    pub struct DiscoveryInputInspection<'a> {
        pub key: &'a crate::TypstInputKey,
        pub value: DiscoverySensitiveValueInspection<'a>,
    }

    pub struct DiscoveryOverrideInspection<'a> {
        pub path: &'a ProjectPath,
        pub value: DiscoverySensitiveValueInspection<'a>,
    }

    pub struct IdentityObjectInspection<'a> {
        pub exact_bytes: u64,
        pub content_identity: &'a ContentIdentity,
    }

    pub struct DiscoveryTraceInspection<'a> {
        private: &'a crate::private::PackInspectionState,
    }

    impl DiscoveryTraceInspection<'_> {
        pub fn project_observations(
            &self,
        ) -> impl ExactSizeIterator<Item = DiscoveryProjectObservationInspection<'_>> {
            std::iter::empty()
        }
        pub fn package_observations(
            &self,
        ) -> impl ExactSizeIterator<Item = DiscoveryPackageObservationInspection<'_>> {
            std::iter::empty()
        }
        pub fn used_font_faces(
            &self,
        ) -> impl ExactSizeIterator<Item = FontFaceIdentityInspection<'_>> {
            std::iter::empty()
        }
    }

    pub enum DiscoveryProjectObservationInspection<'a> {
        BaselineRead {
            path: &'a ProjectPath,
            request_kind: FileRequestKind,
            object: IdentityObjectInspection<'a>,
        },
        OverrideRead {
            path: &'a ProjectPath,
            request_kind: FileRequestKind,
            replacement: DiscoverySensitiveValueInspection<'a>,
        },
        Missing {
            path: &'a ProjectPath,
            request_kind: FileRequestKind,
        },
    }

    pub enum DiscoveryPackageObservationInspection<'a> {
        Read {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            request_kind: FileRequestKind,
            object: IdentityObjectInspection<'a>,
        },
        Missing {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            request_kind: FileRequestKind,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyDisposition {
        Embedded,
        ExternallyFulfilled,
    }

    pub type AcquisitionSourceClass = crate::authority::AcquisitionSourceClass;

    pub struct AcquisitionProvenanceInspection<'a> {
        pub authority_kind: &'a str,
        pub source_class: AcquisitionSourceClass,
        pub logical_origin: Option<&'a str>,
    }

    pub struct PackageRequirementInspection<'a> {
        pub identity: &'a PackageRequirementIdentity,
        pub specification: &'a crate::PackageSpecification,
        pub tree_identity: &'a crate::CompletePackageTreeIdentity,
        pub files: &'a [PackageFileInspection<'a>],
        pub manifest: PackageManifestSummaryInspection<'a>,
        pub disposition: DependencyDisposition,
        pub provenance: AcquisitionProvenanceInspection<'a>,
    }

    pub struct PackageFileInspection<'a> {
        pub path: &'a crate::PackagePath,
        pub content_identity: &'a ContentIdentity,
        pub exact_bytes: u64,
    }

    pub struct PackageManifestSummaryInspection<'a> {
        pub name: &'a str,
        pub version: VersionInspection,
        pub entrypoint: &'a crate::PackagePath,
        pub minimum_compiler_version: Option<VersionInspection>,
    }

    pub struct FontRequirementInspection<'a> {
        pub identity: &'a FontRequirementIdentity,
        pub container: IdentityObjectInspection<'a>,
        pub faces: &'a [FontFaceInspection<'a>],
        pub disposition: DependencyDisposition,
        pub provenance: AcquisitionProvenanceInspection<'a>,
        pub observing_variants: &'a [DiscoveryVariantIdentity],
    }

    #[derive(Clone, Copy)]
    pub struct FontFaceIdentityInspection<'a> {
        pub container_identity: &'a ContentIdentity,
        pub face_index: u32,
    }

    pub struct FontFaceInspection<'a> {
        pub identity: FontFaceIdentityInspection<'a>,
        pub selection: FontSelectionMetadataInspection<'a>,
        pub licensing: FontLicensingMetadataInspection<'a>,
    }

    pub struct FontSelectionMetadataInspection<'a> {
        pub family: &'a str,
        pub style: crate::authority::FontStyle,
        pub weight: crate::authority::FontWeight,
        pub stretch: crate::authority::FontStretch,
        pub flags: crate::authority::FontSelectionFlags,
        pub axes: &'a [crate::authority::FontAxis],
        pub codepoint_coverage: &'a [crate::authority::UnicodeCodepointRange],
    }

    pub struct FontLicensingMetadataInspection<'a> {
        pub fs_type: Option<u16>,
        pub name_records: &'a [LicenseNameRecordInspection<'a>],
    }

    pub struct LicenseNameRecordInspection<'a> {
        pub name_id: u16,
        pub platform_id: u16,
        pub encoding_id: u16,
        pub language_id: u16,
        pub exact_bytes: &'a [u8],
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct PackSemanticExtensionIdentifier(
        crate::private::PackSemanticExtensionIdentifierState,
    );

    impl PackSemanticExtensionIdentifier {
        pub fn as_str(&self) -> &str {
            unimplemented!()
        }
    }

    pub struct PackSemanticExtensionInspection<'a> {
        pub identifier: &'a PackSemanticExtensionIdentifier,
        pub epoch: NonZeroU32,
        pub canonical_payload: &'a [u8],
        pub required_objects: &'a [IdentityObjectInspection<'a>],
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
            _limits: &crate::AdmittedOperationResourceLimits<
                crate::compilation::CompilationResourceLimits,
            >,
            _request: crate::compilation::CompilationRequest,
        ) -> Result<crate::PreparedCompilation, crate::compilation::CompilationRequestRejection>
        {
            unimplemented!()
        }
    }
}

pub mod authority {
    use super::{
        AuthorityInstanceIdentity, CompletePackageTreeIdentity, ContentIdentity,
        FontRequirementIdentity, PackagePath, PackageSpecification, StableByteValue,
    };
    use std::future::Future;
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    pub struct CompletePackageTree {
        private: crate::private::CompletePackageTreeState,
    }

    impl CompletePackageTree {
        pub fn try_from_files(
            _controls: &AcquisitionControls<'_>,
            _files: impl IntoIterator<Item = (PackagePath, StableByteValue)>,
        ) -> Result<Self, CompletePackageTreeRejection> {
            unimplemented!()
        }

        pub fn identity(&self) -> &CompletePackageTreeIdentity {
            unimplemented!()
        }

        pub fn files(&self) -> impl ExactSizeIterator<Item = CompletePackageTreeFileView<'_>> {
            std::iter::empty()
        }
    }

    pub struct CompletePackageTreeFileView<'a> {
        pub path: &'a PackagePath,
        pub bytes: &'a StableByteValue,
        pub content_identity: &'a ContentIdentity,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum CompletePackageTreeRejection {
        Empty,
        DuplicatePath,
        InvalidPath,
        AggregateLengthOverflow,
        FormatCeilingExceeded,
        BudgetExceeded(BudgetExceeded),
    }

    #[derive(Clone, Debug)]
    pub struct FontCatalogSnapshot {
        private: crate::private::FontCatalogState,
    }

    impl FontCatalogSnapshot {
        pub fn try_new(
            _controls: &AcquisitionControls<'_>,
            _candidates: impl IntoIterator<Item = FontCatalogCandidate>,
        ) -> Result<Self, FontCatalogRejection> {
            unimplemented!()
        }

        pub fn candidates(&self) -> impl ExactSizeIterator<Item = &FontCatalogCandidate> {
            std::iter::empty()
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FontContainerAcquisitionIdentity {
        authority: AuthorityInstanceIdentity,
        opaque: Arc<[u8]>,
    }

    impl FontContainerAcquisitionIdentity {
        pub(crate) fn try_new(
            authority: AuthorityInstanceIdentity,
            opaque: Arc<[u8]>,
        ) -> Result<Self, FontCatalogRejection> {
            if opaque.is_empty() {
                return Err(FontCatalogRejection::InvalidAcquisitionIdentity);
            }
            Ok(Self { authority, opaque })
        }

        pub fn opaque(&self) -> &[u8] {
            &self.opaque
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FontStyle {
        Normal,
        Italic,
        Oblique,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FontWeight(u16);

    impl FontWeight {
        pub fn try_new(value: u16) -> Result<Self, FontCatalogRejection> {
            (100..=900)
                .contains(&value)
                .then_some(Self(value))
                .ok_or(FontCatalogRejection::WeightOutOfRange)
        }

        pub fn get(self) -> u16 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FontStretch(u16);

    impl FontStretch {
        pub fn try_from_thousandths(value: u16) -> Result<Self, FontCatalogRejection> {
            (500..=2000)
                .contains(&value)
                .then_some(Self(value))
                .ok_or(FontCatalogRejection::StretchOutOfRange)
        }

        pub fn thousandths(self) -> u16 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FontSelectionFlags(u8);

    impl FontSelectionFlags {
        pub fn try_from_bits(bits: u8) -> Result<Self, FontCatalogRejection> {
            (bits & !0x0f == 0)
                .then_some(Self(bits))
                .ok_or(FontCatalogRejection::UnknownFlagBits)
        }

        pub fn bits(self) -> u8 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct OpenTypeAxisTag([u8; 4]);

    impl OpenTypeAxisTag {
        pub fn from_bytes(bytes: [u8; 4]) -> Self {
            Self(bytes)
        }
        pub fn as_bytes(&self) -> &[u8; 4] {
            &self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FontAxisValue(u32);

    impl FontAxisValue {
        pub fn try_from_be_bytes(bytes: [u8; 4]) -> Result<Self, FontCatalogRejection> {
            let bits = u32::from_be_bytes(bytes);
            let value = f32::from_bits(bits);
            if !value.is_finite() || bits == (-0.0f32).to_bits() {
                return Err(FontCatalogRejection::InvalidAxisValue);
            }
            Ok(Self(bits))
        }

        pub fn to_be_bytes(self) -> [u8; 4] {
            self.0.to_be_bytes()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FontAxis {
        tag: OpenTypeAxisTag,
        minimum: FontAxisValue,
        default: FontAxisValue,
        maximum: FontAxisValue,
    }

    impl FontAxis {
        pub fn try_new(
            tag: OpenTypeAxisTag,
            minimum: FontAxisValue,
            default: FontAxisValue,
            maximum: FontAxisValue,
        ) -> Result<Self, FontCatalogRejection> {
            let min = f32::from_be_bytes(minimum.to_be_bytes());
            let default_value = f32::from_be_bytes(default.to_be_bytes());
            let max = f32::from_be_bytes(maximum.to_be_bytes());
            if min > default_value || default_value > max {
                return Err(FontCatalogRejection::InvalidAxisBounds);
            }
            Ok(Self {
                tag,
                minimum,
                default,
                maximum,
            })
        }

        pub fn tag(&self) -> OpenTypeAxisTag {
            self.tag
        }
        pub fn minimum(&self) -> FontAxisValue {
            self.minimum
        }
        pub fn default(&self) -> FontAxisValue {
            self.default
        }
        pub fn maximum(&self) -> FontAxisValue {
            self.maximum
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct UnicodeCodepointRange {
        first: u32,
        last: u32,
    }

    impl UnicodeCodepointRange {
        pub fn try_new(first: u32, last: u32) -> Result<Self, FontCatalogRejection> {
            let invalid = first > last || last > 0x10ffff || (first <= 0xdfff && last >= 0xd800);
            (!invalid)
                .then_some(Self { first, last })
                .ok_or(FontCatalogRejection::InvalidCodepointRange)
        }

        pub fn first(self) -> u32 {
            self.first
        }
        pub fn last(self) -> u32 {
            self.last
        }
    }

    #[derive(Clone, Debug)]
    pub struct FontCatalogCandidate {
        private: crate::private::FontCatalogCandidateState,
    }

    impl FontCatalogCandidate {
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            _controls: &AcquisitionControls<'_>,
            _container_acquisition: FontContainerAcquisitionIdentity,
            _face_index: u32,
            _family: String,
            _style: FontStyle,
            _weight: FontWeight,
            _stretch: FontStretch,
            _flags: FontSelectionFlags,
            _axes: impl IntoIterator<Item = FontAxis>,
            _codepoint_coverage: impl IntoIterator<Item = UnicodeCodepointRange>,
        ) -> Result<Self, FontCatalogRejection> {
            unimplemented!()
        }

        pub fn container_acquisition(&self) -> &FontContainerAcquisitionIdentity {
            unimplemented!()
        }
        pub fn face_index(&self) -> u32 {
            unimplemented!()
        }
        pub fn family(&self) -> &str {
            unimplemented!()
        }
        pub fn style(&self) -> FontStyle {
            unimplemented!()
        }
        pub fn weight(&self) -> FontWeight {
            unimplemented!()
        }
        pub fn stretch(&self) -> FontStretch {
            unimplemented!()
        }
        pub fn flags(&self) -> FontSelectionFlags {
            unimplemented!()
        }
        pub fn axes(&self) -> impl ExactSizeIterator<Item = &FontAxis> {
            std::iter::empty()
        }
        pub fn codepoint_coverage(
            &self,
        ) -> impl ExactSizeIterator<Item = UnicodeCodepointRange> + '_ {
            std::iter::empty()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum FontCatalogRejection {
        InvalidAcquisitionIdentity,
        WrongAuthority,
        EmptyFamily,
        WeightOutOfRange,
        StretchOutOfRange,
        UnknownFlagBits,
        InvalidAxisValue,
        InvalidAxisBounds,
        InvalidCodepointRange,
        NonCanonicalAxes,
        NonCanonicalCodepointCoverage,
        DuplicateFace,
        FormatCeilingExceeded,
        BudgetExceeded(BudgetExceeded),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum InvalidFontCandidateDisposition {
        Omit,
        WarnAndOmit,
        RejectCatalog,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FontScanPolicy {
        pub invalid_candidate: InvalidFontCandidateDisposition,
        pub unreadable_candidate: InvalidFontCandidateDisposition,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FontScanDiagnostic {
        private: crate::private::FontScanDiagnosticState,
    }

    impl FontScanDiagnostic {
        pub fn try_new(
            _candidate_ordinal: u64,
            _code: &str,
            _safe_message: &str,
        ) -> Result<Self, AuthorityValueRejection> {
            unimplemented!()
        }
        pub fn candidate_ordinal(&self) -> u64 {
            unimplemented!()
        }
        pub fn code(&self) -> &str {
            unimplemented!()
        }
        pub fn safe_message(&self) -> &str {
            unimplemented!()
        }
    }

    pub struct FontCatalogAcquisition {
        private: crate::private::FontCatalogAcquisitionState,
    }

    impl FontCatalogAcquisition {
        pub fn try_new(
            _controls: &AcquisitionControls<'_>,
            _snapshot: FontCatalogSnapshot,
            _scan_policy: FontScanPolicy,
            _diagnostics: impl IntoIterator<Item = FontScanDiagnostic>,
        ) -> Result<Self, FontCatalogRejection> {
            unimplemented!()
        }
        pub fn snapshot(&self) -> &FontCatalogSnapshot {
            unimplemented!()
        }
        pub fn scan_policy(&self) -> &FontScanPolicy {
            unimplemented!()
        }
        pub fn diagnostics(&self) -> impl ExactSizeIterator<Item = &FontScanDiagnostic> {
            std::iter::empty()
        }
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

    impl FontCatalogRequest {
        pub fn requested_scan_policy(&self) -> &FontScanPolicy {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct FontContainerAcquisitionRequest {
        private: crate::private::FontContainerAcquisitionRequestState,
    }

    pub enum FontContainerAcquisitionPurpose<'a> {
        CatalogFace {
            acquisition_identity: &'a FontContainerAcquisitionIdentity,
            face_index: u32,
        },
        ExternalRequirement {
            requirement_identity: &'a FontRequirementIdentity,
            expected_container_identity: &'a ContentIdentity,
            expected_bytes: u64,
            required_face_indices: &'a [u32],
        },
    }

    impl FontContainerAcquisitionRequest {
        pub fn purpose(&self) -> FontContainerAcquisitionPurpose<'_> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct DependencyResolutionEvidence {
        private: crate::private::DependencyEvidenceState,
    }

    impl DependencyResolutionEvidence {
        pub(crate) fn builder(authority: AuthorityInstanceIdentity) -> DependencyEvidenceBuilder {
            DependencyEvidenceBuilder {
                authority,
                private: crate::private::DependencyEvidenceBuilderState,
            }
        }

        pub fn keys(&self) -> impl ExactSizeIterator<Item = &DependencyEvidenceKey> {
            std::iter::empty()
        }

        pub fn facts(&self) -> impl ExactSizeIterator<Item = DependencyEvidenceFactView<'_>> {
            std::iter::empty()
        }
    }

    pub struct DependencyEvidenceFactView<'a> {
        pub key: &'a DependencyEvidenceKey,
        pub outcome: EvidenceFactOutcome,
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
        pub(crate) fn try_new(
            _authority: AuthorityInstanceIdentity,
            _kind: EvidenceFactKind,
            _opaque_key: Arc<[u8]>,
            _immutable_version: Option<Arc<[u8]>>,
        ) -> Result<Self, DependencyEvidenceRejection> {
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
        pub fn try_new(
            _authority_kind: &str,
            _source_class: AcquisitionSourceClass,
            _logical_origin: Option<&str>,
        ) -> Result<Self, AuthorityValueRejection> {
            unimplemented!()
        }

        pub fn authority_kind(&self) -> &str {
            unimplemented!()
        }
        pub fn source_class(&self) -> AcquisitionSourceClass {
            unimplemented!()
        }
        pub fn logical_origin(&self) -> Option<&str> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AcquisitionSourceClass {
        CallerSupplied,
        ExplicitLocal,
        Cache,
        Network,
        SystemFont,
        EngineEmbeddedFont,
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
        ResourceLimit(BudgetExceeded),
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
    pub struct AuthorityEvidenceCapabilities {
        pub immutable_values: bool,
        pub exact_key_revalidation: bool,
        pub opaque_scope_revalidation: bool,
        pub polling: bool,
        pub push_subscription: bool,
        pub cursor_replay: bool,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AuthorityCachePolicy {
        Disabled,
        LookupOnly,
        LookupAndAdmit,
    }

    #[derive(Clone, Debug)]
    pub struct AuthorityPrivateCacheCapabilityProjection {
        pub class: crate::OperationalCapabilityClass,
        pub policy: AuthorityCachePolicy,
        pub isolation_domain_present: bool,
        pub network: crate::SelectedNetworkContract,
    }

    #[derive(Clone, Debug)]
    pub struct PackageAuthorityCapabilityDescriptor {
        private: crate::private::PackageAuthorityCapabilityDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct FontAuthorityCapabilityDescriptor {
        private: crate::private::FontAuthorityCapabilityDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct AuthorityCapabilitySpec {
        pub class: crate::OperationalCapabilityClass,
        pub ordered_source_classes: Vec<AcquisitionSourceClass>,
        pub evidence: AuthorityEvidenceCapabilities,
        pub network: crate::SelectedNetworkContract,
        pub resolution_cache: AuthorityCachePolicy,
        pub private_caches: Vec<AuthorityPrivateCacheCapabilityProjection>,
    }

    macro_rules! authority_descriptor {
        ($name:ident) => {
            impl $name {
                pub fn try_new(
                    _instance: AuthorityInstanceIdentity,
                    _spec: AuthorityCapabilitySpec,
                ) -> Result<Self, AuthorityCapabilityDescriptorRejection> {
                    unimplemented!()
                }

                pub fn descriptor_version(&self) -> u32 {
                    1
                }

                pub fn class(&self) -> &crate::OperationalCapabilityClass {
                    unimplemented!()
                }

                pub fn ordered_source_classes(&self) -> &[AcquisitionSourceClass] {
                    unimplemented!()
                }

                pub fn evidence(&self) -> AuthorityEvidenceCapabilities {
                    unimplemented!()
                }

                pub fn network(&self) -> crate::SelectedNetworkContract {
                    unimplemented!()
                }

                pub fn resolution_cache(&self) -> AuthorityCachePolicy {
                    unimplemented!()
                }

                pub fn private_caches(
                    &self,
                ) -> impl ExactSizeIterator<Item = &AuthorityPrivateCacheCapabilityProjection> {
                    std::iter::empty()
                }
            }
        };
    }

    authority_descriptor!(PackageAuthorityCapabilityDescriptor);
    authority_descriptor!(FontAuthorityCapabilityDescriptor);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AuthorityCapabilityDescriptorRejection {
        WrongRole,
        WrongInstance,
        EmptySources,
        IncoherentCachePolicy,
        IncoherentNetworkContract,
    }

    pub struct AcquisitionBudget {
        private: crate::private::AcquisitionBudgetState,
    }

    impl AcquisitionBudget {
        pub(crate) fn try_new(
            _package_files: u64,
            _largest_package_file_bytes: u64,
            _font_candidates: u64,
            _font_faces: u64,
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

        pub fn reserve_package_file(
            &self,
            _bytes: u64,
        ) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }

        pub fn reserve_font_candidate(&self) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }

        pub fn reserve_font_face(&self) -> Result<BudgetReservation, BudgetExceeded> {
            unimplemented!()
        }
    }

    pub struct BudgetReservation {
        private: crate::private::BudgetReservationState,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct BudgetExceeded {
        pub dimension: AcquisitionBudgetDimension,
        pub requested: u64,
        pub remaining: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AcquisitionBudgetDimension {
        PackageFiles,
        LargestPackageFileBytes,
        FontCandidates,
        FontFaces,
        DownloadedBytes,
        ExpandedBytes,
        StableSpoolBytes,
        RetainedMemoryBytes,
    }

    pub struct AcquisitionControls<'a> {
        authority: &'a AuthorityInstanceIdentity,
        deadline: crate::OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        budget: &'a AcquisitionBudget,
    }

    impl<'a> AcquisitionControls<'a> {
        pub(crate) fn try_new(
            authority: &'a AuthorityInstanceIdentity,
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
                authority,
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

        pub fn evidence_builder(&self) -> DependencyEvidenceBuilder {
            DependencyResolutionEvidence::builder(self.authority.clone())
        }

        pub fn evidence_key(
            &self,
            kind: EvidenceFactKind,
            opaque_key: Arc<[u8]>,
            immutable_version: Option<Arc<[u8]>>,
        ) -> Result<DependencyEvidenceKey, DependencyEvidenceRejection> {
            DependencyEvidenceKey::try_new(
                self.authority.clone(),
                kind,
                opaque_key,
                immutable_version,
            )
        }

        pub fn font_container_acquisition_identity(
            &self,
            opaque: Arc<[u8]>,
        ) -> Result<FontContainerAcquisitionIdentity, FontCatalogRejection> {
            FontContainerAcquisitionIdentity::try_new(self.authority.clone(), opaque)
        }

        pub fn evidence_fence(
            &self,
            confirmed: impl IntoIterator<Item = DependencyEvidenceKey>,
            generation: Arc<[u8]>,
            through_cursor: Option<ProviderCursor>,
        ) -> Result<EvidenceFence, EvidenceValueRejection> {
            EvidenceFence::try_new(
                self.authority.clone(),
                confirmed,
                generation,
                through_cursor,
            )
        }

        pub fn provider_cursor(
            &self,
            opaque: Arc<[u8]>,
        ) -> Result<ProviderCursor, EvidenceValueRejection> {
            ProviderCursor::try_new(self.authority.clone(), opaque)
        }
    }

    #[derive(Clone, Debug)]
    pub struct EvidenceRevalidationRequest {
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
        pub(crate) fn try_new(
            _authority: AuthorityInstanceIdentity,
            _confirmed: impl IntoIterator<Item = DependencyEvidenceKey>,
            _generation: Arc<[u8]>,
            _through_cursor: Option<ProviderCursor>,
        ) -> Result<Self, EvidenceValueRejection> {
            unimplemented!()
        }

        pub fn confirmed(&self) -> impl ExactSizeIterator<Item = &DependencyEvidenceKey> {
            std::iter::empty()
        }
        pub fn generation(&self) -> &[u8] {
            unimplemented!()
        }
        pub fn through_cursor(&self) -> Option<&ProviderCursor> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ProviderCursor {
        private: crate::private::ProviderCursorState,
    }

    impl ProviderCursor {
        pub(crate) fn try_new(
            _provider: AuthorityInstanceIdentity,
            _opaque: Arc<[u8]>,
        ) -> Result<Self, EvidenceValueRejection> {
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
        pub fn safe_code(&self) -> &str {
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
        pub fn safe_code(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct InsufficientEvidenceCapability {
        private: crate::private::InsufficientEvidenceCapabilityState,
    }

    impl InsufficientEvidenceCapability {
        pub fn new(
            _required: AuthorityEvidenceCapabilities,
            _available: AuthorityEvidenceCapabilities,
        ) -> Self {
            unimplemented!()
        }
        pub fn required(&self) -> AuthorityEvidenceCapabilities {
            unimplemented!()
        }
        pub fn available(&self) -> AuthorityEvidenceCapabilities {
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
        fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor;

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
        fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor;

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
        fn descriptor(&self) -> &FontAuthorityCapabilityDescriptor;

        fn catalog(
            &self,
            request: FontCatalogRequest,
            controls: AcquisitionControls<'_>,
        ) -> DependencyAcquisitionOutcome<FontCatalogAcquisition>;

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
        type Catalog<'a>: Future<Output = DependencyAcquisitionOutcome<FontCatalogAcquisition>> + 'a
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
        fn descriptor(&self) -> &FontAuthorityCapabilityDescriptor;

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
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
            _entrypoint: ProjectPath,
            _files: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct DiscoveryVariant {
        private: crate::private::DiscoveryVariantState,
    }

    #[derive(Clone, Debug)]
    pub struct DiscoveryOverrideSet {
        private: crate::private::DiscoveryOverrideSetState,
    }

    impl DiscoveryOverrideSet {
        pub fn empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
            _overrides: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
        ) -> Result<Self, CreationRequestRejection> {
            unimplemented!()
        }
    }

    impl DiscoveryVariant {
        pub fn paged_explicit_empty() -> Self {
            unimplemented!()
        }

        pub fn html_explicit_empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
            _target: crate::compilation::TypstTarget,
            _inputs: impl IntoIterator<Item = (crate::TypstInputKey, crate::TypstInputValue)>,
            _document_time: crate::compilation::CompilationDocumentTime,
            _features: impl IntoIterator<Item = crate::compilation::EngineFeature>,
            _overrides: DiscoveryOverrideSet,
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
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
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
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
            _default: EmbeddingDisposition,
            _choices: impl IntoIterator<Item = (crate::ContentIdentity, EmbeddingDisposition)>,
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
            _limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
            _project: ProjectSnapshot,
            _variants: impl IntoIterator<Item = DiscoveryVariant>,
            _package_embedding: PackageEmbeddingPolicy,
            _font_embedding: FontEmbeddingPolicy,
            _metadata: crate::pack::PackMetadata,
            _annotations: impl IntoIterator<Item = crate::pack::PackAnnotation>,
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
            role: DiscoveryVariantValueRole,
        },
        DiscoveryOverride {
            variant_ordinal: u32,
            path: ProjectPath,
        },
        InclusionMembership,
        PackMetadata,
        PackAnnotationInventory,
        PackAnnotation {
            identifier: crate::pack::PackAnnotationIdentifier,
        },
    }

    #[derive(Clone, Debug)]
    pub enum DiscoveryVariantValueRole {
        Target,
        TypstInput(crate::TypstInputKey),
        DocumentTime,
        Feature(crate::compilation::EngineFeature),
        Label,
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
        request: CreationRequest,
        evidence: CreationInputEvidence,
    }

    impl CreationInput {
        pub fn try_new(
            _request: CreationRequest,
            _evidence: CreationInputEvidence,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CreationEvidenceFenceRequest {
        private: crate::private::CreationEvidenceFenceRequestState,
    }

    impl CreationEvidenceFenceRequest {
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
            _request: &CreationEvidenceFenceRequest,
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
        pub fn safe_code(&self) -> &str {
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
        pub fn safe_code(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct InsufficientCreationEvidenceCapability {
        private: crate::private::CreationEvidenceCapabilityState,
    }

    impl InsufficientCreationEvidenceCapability {
        pub fn new(
            _required: CreationEvidenceCapabilityProjection,
            _available: CreationEvidenceCapabilityProjection,
        ) -> Self {
            unimplemented!()
        }
        pub fn required(&self) -> CreationEvidenceCapabilityProjection {
            unimplemented!()
        }
        pub fn available(&self) -> CreationEvidenceCapabilityProjection {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationEvidenceStability {
        Immutable,
        Versioned,
        Revalidated,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationEvidenceCapabilityProjection {
        pub stability: CreationEvidenceStability,
        pub race_closing_revalidation: bool,
        pub exact_key_revalidation: bool,
        pub opaque_scope_revalidation: bool,
        pub polling: bool,
        pub push_subscription: bool,
        pub cursor_replay: bool,
        pub network: crate::SelectedNetworkContract,
    }

    #[derive(Clone, Debug)]
    pub struct CreationEvidenceCapabilityDescriptor {
        private: crate::private::CreationEvidenceCapabilityDescriptorState,
    }

    impl CreationEvidenceCapabilityDescriptor {
        pub fn try_new(
            _provider: super::AuthorityInstanceIdentity,
            _class: crate::OperationalCapabilityClass,
            _capabilities: CreationEvidenceCapabilityProjection,
        ) -> Result<Self, CreationEvidenceValueRejection> {
            unimplemented!()
        }

        pub fn descriptor_version(&self) -> u32 {
            1
        }

        pub fn class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn capabilities(&self) -> CreationEvidenceCapabilityProjection {
            unimplemented!()
        }
    }

    pub trait SyncCreationEvidence {
        fn provider_identity(&self) -> &super::AuthorityInstanceIdentity;
        fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor;

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
        fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor;

        fn fence<'a>(
            &'a self,
            request: CreationEvidenceFenceRequest,
            controls: super::authority::AcquisitionControls<'a>,
        ) -> Self::Fence<'a>;
    }

    pub struct ImmutableCreationEvidence {
        identity: super::AuthorityInstanceIdentity,
        descriptor: CreationEvidenceCapabilityDescriptor,
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

        fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor {
            &self.descriptor
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

        fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor {
            &self.descriptor
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
        largest_project_file_bytes: u64,
        packages: u64,
        package_files: u64,
        largest_package_file_bytes: u64,
        package_tree_bytes: u64,
        font_containers: u64,
        font_candidates: u64,
        font_faces: u64,
        font_bytes: u64,
        discovery_variants: u64,
        discovery_restarts: u64,
        override_count: u64,
        largest_override_bytes: u64,
        aggregate_override_bytes: u64,
        stable_spool_bytes: u64,
        retained_memory_bytes: u64,
    }

    impl CreationResourceLimits {
        pub fn try_new(spec: CreationResourceLimitSpec) -> Result<Self, crate::AdmissionRefusal> {
            let limits = Self {
                project_files: spec.project_files,
                aggregate_project_bytes: spec.aggregate_project_bytes,
                largest_project_file_bytes: spec.largest_project_file_bytes,
                packages: spec.packages,
                package_files: spec.package_files,
                largest_package_file_bytes: spec.largest_package_file_bytes,
                package_tree_bytes: spec.package_tree_bytes,
                font_containers: spec.font_containers,
                font_candidates: spec.font_candidates,
                font_faces: spec.font_faces,
                font_bytes: spec.font_bytes,
                discovery_variants: spec.discovery_variants,
                discovery_restarts: spec.discovery_restarts,
                override_count: spec.override_count,
                largest_override_bytes: spec.largest_override_bytes,
                aggregate_override_bytes: spec.aggregate_override_bytes,
                stable_spool_bytes: spec.stable_spool_bytes,
                retained_memory_bytes: spec.retained_memory_bytes,
            };
            crate::private::SealedLimitSet::validate(&limits)?;
            Ok(limits)
        }

        pub fn spec(&self) -> CreationResourceLimitSpec {
            CreationResourceLimitSpec {
                project_files: self.project_files,
                aggregate_project_bytes: self.aggregate_project_bytes,
                largest_project_file_bytes: self.largest_project_file_bytes,
                packages: self.packages,
                package_files: self.package_files,
                largest_package_file_bytes: self.largest_package_file_bytes,
                package_tree_bytes: self.package_tree_bytes,
                font_containers: self.font_containers,
                font_candidates: self.font_candidates,
                font_faces: self.font_faces,
                font_bytes: self.font_bytes,
                discovery_variants: self.discovery_variants,
                discovery_restarts: self.discovery_restarts,
                override_count: self.override_count,
                largest_override_bytes: self.largest_override_bytes,
                aggregate_override_bytes: self.aggregate_override_bytes,
                stable_spool_bytes: self.stable_spool_bytes,
                retained_memory_bytes: self.retained_memory_bytes,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationResourceLimitSpec {
        pub project_files: u64,
        pub aggregate_project_bytes: u64,
        pub largest_project_file_bytes: u64,
        pub packages: u64,
        pub package_files: u64,
        pub largest_package_file_bytes: u64,
        pub package_tree_bytes: u64,
        pub font_containers: u64,
        pub font_candidates: u64,
        pub font_faces: u64,
        pub font_bytes: u64,
        pub discovery_variants: u64,
        pub discovery_restarts: u64,
        pub override_count: u64,
        pub largest_override_bytes: u64,
        pub aggregate_override_bytes: u64,
        pub stable_spool_bytes: u64,
        pub retained_memory_bytes: u64,
    }

    #[derive(Clone, Debug)]
    pub struct CreationOperationRequest {
        pub network: crate::OperationNetworkPolicy,
        pub dependency_concurrency: NonZeroUsize,
        pub engine_width: crate::EngineWidthRequest,
        pub requested_ready_jobs: Option<NonZeroUsize>,
        pub requested_queue: Option<usize>,
        pub requested_workers: Option<NonZeroUsize>,
        pub placement: crate::ExecutionPlacement,
        pub interruption: crate::OperationInterruptionStrength,
        pub required_enforcement: Vec<crate::EnforcementClaim>,
        pub deadline: OperationDeadline,
        pub queue_timeout_ticks: Option<u64>,
        pub latency_target_ticks: Option<u64>,
        pub reporting: CreationReportingPolicy,
    }

    #[derive(Clone, Debug)]
    pub struct CreationAdmissionRefusal {
        private: crate::private::CreationAdmissionRefusalState,
    }

    impl CreationAdmissionRefusal {
        pub fn operation_request(&self) -> &CreationOperationRequest {
            unimplemented!()
        }
        pub fn requested_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn requested_limits(&self) -> &CreationResourceLimits {
            unimplemented!()
        }
        pub fn evidence(&self) -> &CreationEvidenceCapabilityDescriptor {
            unimplemented!()
        }
        pub fn packages(&self) -> &super::authority::PackageAuthorityCapabilityDescriptor {
            unimplemented!()
        }
        pub fn fonts(&self) -> &super::authority::FontAuthorityCapabilityDescriptor {
            unimplemented!()
        }
        pub fn execution(&self) -> Option<&CreationExecutionFacilityCapabilityDescriptor> {
            unimplemented!()
        }

        pub fn reason(&self) -> crate::OperationAdmissionRefusalReason {
            unimplemented!()
        }
    }

    pub struct SyncCreationControls<'a, E: ?Sized, P: ?Sized, F: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
        evidence: &'a E,
        packages: &'a P,
        fonts: &'a F,
        request: CreationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        admission_record: crate::private::CreationAdmissionRecordState,
    }

    impl<'a, E: ?Sized, P: ?Sized, F: ?Sized> SyncCreationControls<'a, E, P, F> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_admit(
            _admission: OrdinaryAdmission,
            _limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
            _evidence: &'a E,
            _packages: &'a P,
            _fonts: &'a F,
            _request: CreationOperationRequest,
            _clock: &'a dyn crate::MonotonicClock,
            _interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, CreationAdmissionRefusal>
        where
            E: SyncCreationEvidence,
            P: SyncPackageAuthority,
            F: SyncFontAuthority,
        {
            unimplemented!()
        }
    }

    pub struct AsyncCreationControls<'a, E: ?Sized, P: ?Sized, F: ?Sized, X: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
        evidence: &'a E,
        packages: &'a P,
        fonts: &'a F,
        execution: &'a X,
        request: CreationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        admission_record: crate::private::CreationAdmissionRecordState,
    }

    impl<'a, E: ?Sized, P: ?Sized, F: ?Sized, X: ?Sized> AsyncCreationControls<'a, E, P, F, X> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_admit(
            _admission: OrdinaryAdmission,
            _limits: AdmittedOperationResourceLimits<CreationResourceLimits>,
            _evidence: &'a E,
            _packages: &'a P,
            _fonts: &'a F,
            _execution: &'a X,
            _request: CreationOperationRequest,
            _clock: &'a dyn crate::MonotonicClock,
            _interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, CreationAdmissionRefusal>
        where
            E: AsyncCreationEvidence,
            P: AsyncPackageAuthority,
            F: AsyncFontAuthority,
            X: CreationExecutionFacility,
        {
            unimplemented!()
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

        fn descriptor(&self) -> &CreationExecutionFacilityCapabilityDescriptor;

        fn dispatch<'a>(&'a self, job: ReadyCreationJob) -> Self::Dispatch<'a>;
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct CreationExecutionFacilityCapacity {
        pub requested_ready_jobs: NonZeroUsize,
        pub admitted_ready_jobs: NonZeroUsize,
        pub requested_queue: usize,
        pub admitted_queue: usize,
        pub requested_workers: Option<NonZeroUsize>,
        pub admitted_workers: Option<NonZeroUsize>,
        pub constraints: Vec<crate::AdmissionConstraint>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationExecutionFacilityMaximumCapacity {
        pub ready_jobs: NonZeroUsize,
        pub queue: usize,
        pub workers: Option<NonZeroUsize>,
    }

    #[derive(Clone, Debug)]
    pub struct CreationExecutionFacilityCapabilityDescriptor {
        private: crate::private::CreationExecutionFacilityCapabilityDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct CreationExecutionFacilityCapabilitySpec {
        pub class: crate::OperationalCapabilityClass,
        pub capacity_scope_class: crate::OperationalCapabilityClass,
        pub shared_with_compilation: bool,
        pub supported_placements: Vec<crate::ExecutionPlacement>,
        pub ready_job_capacity: NonZeroUsize,
        pub queue_capacity: usize,
        pub worker_capacity: Option<NonZeroUsize>,
        pub overlapping_jobs_per_worker: bool,
        pub domain_policy: crate::compilation::EngineRuntimeDomainPolicyDescriptor,
        pub execution_network: crate::SelectedNetworkContract,
        pub worker_control_network: Option<crate::SelectedNetworkContract>,
        pub interruption: crate::OperationInterruptionStrength,
        pub worker_protocol: Option<crate::OperationalCapabilityClass>,
        pub parent_verifies_response: bool,
        pub parent_withholds_output: bool,
        pub no_in_process_fallback: bool,
        pub terminate_and_reap: bool,
        pub forced_termination_target_ticks: Option<u64>,
        pub enforcement: Vec<crate::EnforcementClaim>,
    }

    pub struct CreationExecutionFacilityCapabilityView<'a> {
        pub class: &'a crate::OperationalCapabilityClass,
        pub capacity_scope_class: &'a crate::OperationalCapabilityClass,
        pub shared_with_compilation: bool,
        pub supported_placements: &'a [crate::ExecutionPlacement],
        pub maximum_capacity: CreationExecutionFacilityMaximumCapacity,
        pub overlapping_jobs_per_worker: bool,
        pub domain_policy: &'a crate::compilation::EngineRuntimeDomainPolicyDescriptor,
        pub execution_network: crate::SelectedNetworkContract,
        pub worker_control_network: Option<crate::SelectedNetworkContract>,
        pub interruption: crate::OperationInterruptionStrength,
        pub worker_protocol: Option<&'a crate::OperationalCapabilityClass>,
        pub parent_verifies_response: bool,
        pub parent_withholds_output: bool,
        pub no_in_process_fallback: bool,
        pub terminate_and_reap: bool,
        pub forced_termination_target_ticks: Option<u64>,
        pub enforcement: &'a [crate::EnforcementClaim],
    }

    impl CreationExecutionFacilityCapabilityDescriptor {
        pub fn try_new(
            _spec: CreationExecutionFacilityCapabilitySpec,
        ) -> Result<Self, crate::OperationAdmissionRefusalReason> {
            unimplemented!()
        }

        pub fn descriptor_version(&self) -> u32 {
            1
        }

        pub fn class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn capacity_scope_class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn domain_policy(&self) -> &crate::compilation::EngineRuntimeDomainPolicyDescriptor {
            unimplemented!()
        }

        pub fn maximum_capacity(&self) -> CreationExecutionFacilityMaximumCapacity {
            unimplemented!()
        }

        pub fn capabilities(&self) -> CreationExecutionFacilityCapabilityView<'_> {
            unimplemented!()
        }
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
            _parent_assigned_domain: crate::compilation::EngineRuntimeDomainIdentity,
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

        pub fn operational_inventory(&self) -> CreationOperationalInventoryView<'_> {
            unimplemented!()
        }

        pub fn reporting(&self) -> CreationReportingChannelsView<'_> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationReportingPolicy {
        pub timing: bool,
        pub fine_engine_timing: bool,
    }

    pub struct CreationOperationalInventoryView<'a> {
        private: &'a crate::private::CreationReportState,
    }

    impl CreationOperationalInventoryView<'_> {
        pub fn admission(&self) -> CreationAdmissionInventoryView<'_> {
            unimplemented!()
        }
        pub fn resources(&self) -> CreationResourcesInventoryView<'_> {
            unimplemented!()
        }
        pub fn dependency_execution(&self) -> CreationDependencyExecutionInventoryView<'_> {
            unimplemented!()
        }
        pub fn attempt_control(&self) -> CreationAttemptControlInventoryView<'_> {
            unimplemented!()
        }
        pub fn role_execution(&self) -> CreationExecutionInventoryView<'_> {
            unimplemented!()
        }
        pub fn reporting(&self) -> CreationReportingInventoryView<'_> {
            unimplemented!()
        }
    }

    pub struct CreationAdmissionInventoryView<'a> {
        pub requested_trust: crate::DeploymentTrustProfile,
        pub admitted_trust: crate::DeploymentTrustProfile,
        pub requested_network: crate::OperationNetworkPolicy,
        pub admitted_network: crate::OperationNetworkPolicy,
        pub contractual_no_network: bool,
        pub structural_network_enforcement: crate::EnforcementStrength,
        pub enforcement: crate::EnforcementAdmissionView<'a>,
    }

    pub struct CreationResourcesInventoryView<'a> {
        pub profile: Option<&'a crate::ResourceProfileIdentity>,
        pub requested: &'a CreationResourceLimits,
        pub admitted: &'a CreationResourceLimits,
    }

    pub struct CreationDependencyExecutionInventoryView<'a> {
        pub evidence: &'a CreationEvidenceCapabilityDescriptor,
        pub packages: &'a super::authority::PackageAuthorityCapabilityDescriptor,
        pub fonts: &'a super::authority::FontAuthorityCapabilityDescriptor,
        pub offline_roles_covered: &'a [crate::PreCommitFacilityRole],
        pub concurrency: crate::DependencyConcurrencyAdmission,
    }

    pub struct CreationAttemptControlInventoryView<'a> {
        pub deadline: &'a OperationDeadline,
        pub cancellation_present: bool,
        pub monotonic_domain: &'a crate::MonotonicTimeDomain,
        pub queue_timeout_ticks: Option<u64>,
        pub latency_target_ticks: Option<u64>,
        pub requested_interruption: crate::OperationInterruptionStrength,
        pub admitted_interruption: crate::OperationInterruptionStrength,
        pub winner: Option<CreationInterruptionWinner>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationInterruptionWinner {
        TerminalCommitment,
        Cancellation,
        Deadline,
        ForcedTermination,
    }

    pub enum CreationExecutionInventoryView<'a> {
        CallerThread {
            domain: crate::compilation::EngineRuntimeDomainSelectionView<'a>,
            engine_width: crate::EngineWidthAdmission,
        },
        Facility {
            descriptor: &'a CreationExecutionFacilityCapabilityDescriptor,
            domain: crate::compilation::EngineRuntimeDomainSelectionView<'a>,
            engine_width: crate::EngineWidthAdmission,
            capacity: CreationExecutionFacilityCapacity,
            queue_reached: bool,
            dispatch_reached: bool,
            worker_terminated: bool,
            worker_reaped: bool,
        },
    }

    pub struct CreationReportingInventoryView<'a> {
        pub requested: &'a CreationReportingPolicy,
        pub admitted: &'a CreationReportingPolicy,
        pub timing: ReportingChannelStatus,
        pub fine_engine_timing: ReportingChannelStatus,
        pub fine_timing_lease_reached: bool,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ReportingChannelStatus {
        NotRequested,
        Complete,
        Limited,
        Unavailable,
    }

    pub struct CreationReportingChannelsView<'a> {
        private: &'a crate::private::CreationReportState,
    }

    impl CreationReportingChannelsView<'_> {
        pub fn timing(&self) -> ReportingChannelStatus {
            unimplemented!()
        }
        pub fn fine_engine_timing(&self) -> ReportingChannelStatus {
            unimplemented!()
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

    impl CreationRequestRejection {
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn requested_limits(&self) -> &CreationResourceLimits {
            unimplemented!()
        }
        pub fn admitted_limits(&self) -> &CreationResourceLimits {
            unimplemented!()
        }
        pub fn issues(&self) -> impl ExactSizeIterator<Item = CreationRequestIssueView> + '_ {
            std::iter::empty()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationRequestIssueView {
        pub code: CreationRequestIssueCode,
        pub role: CreationRequestRole,
        pub declaration_ordinal: Option<u64>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationRequestRole {
        ProjectSnapshot,
        DiscoveryVariant,
        ExplicitConditionalInclusion,
        DiscoveryOverride,
        PackageEmbeddingPolicy,
        FontEmbeddingPolicy,
        Metadata,
        Annotation,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CreationRequestIssueCode {
        EmptyCollection,
        DuplicateLogicalKey,
        InvalidValue,
        MemberLimitExceeded,
        LargestMemberLimitExceeded,
        AggregateLimitExceeded,
        FormatCeilingExceeded,
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
        AsyncFontAuthority, AsyncPackageAuthority, SyncFontAuthority, SyncPackageAuthority,
    };
    use super::{
        AdmittedOperationResourceLimits, CacheIsolationDomain, CompilationArtifactIdentity,
        CompilationIdentity, CompilationRequestCommitment, CompilationResultIdentity,
        EngineIdentity, EngineNeutralCompilationIntentIdentity, ExporterIdentity,
        FontRequirementIdentity, OperationDeadline, OrdinaryAdmission, Pack,
        PackageRequirementIdentity, ProjectPath, StableByteValue, TypstInputKey, TypstInputValue,
    };
    use std::future::Future;
    use std::num::NonZeroUsize;
    use std::sync::Arc;

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct EngineFeature(Arc<str>);

    impl EngineFeature {
        pub fn parse(
            _admission: &OrdinaryAdmission,
            _value: &str,
        ) -> Result<Self, crate::DomainValueRejection> {
            unimplemented!()
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TypstTarget {
        Paged,
        Html,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SignedUnixSeconds(i64);

    impl SignedUnixSeconds {
        pub fn try_new(value: i64) -> Result<Self, CompilationRequestIssueCode> {
            (value != i64::MIN)
                .then_some(Self(value))
                .ok_or(CompilationRequestIssueCode::OutOfEngineRange)
        }

        pub fn get(self) -> i64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationDocumentTime {
        Absent,
        Exact(SignedUnixSeconds),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PdfCreationTime {
        Omitted,
        Exact(SignedUnixSeconds),
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
            _limits: &AdmittedOperationResourceLimits<CompilationResourceLimits>,
            _overrides: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
        ) -> Self {
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

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum PdfStandard {
        Pdf14,
        Pdf15,
        Pdf16,
        Pdf17,
        Pdf20,
        PdfA1B,
        PdfA1A,
        PdfA2B,
        PdfA2U,
        PdfA2A,
        PdfA3B,
        PdfA3U,
        PdfA3A,
        PdfA4,
        PdfA4F,
        PdfA4E,
        PdfUa1,
    }

    impl PdfStandard {
        pub fn registry_value(self) -> u8 {
            self as u8 + 1
        }
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
            _standards: impl IntoIterator<Item = PdfStandard>,
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

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct PngPixelsPerInch(u32);

    impl PngPixelsPerInch {
        pub fn try_from_f32(value: f32) -> Result<Self, CompilationRequestIssue> {
            if !value.is_finite() || value <= 0.0 {
                return Err(CompilationRequestIssue::InvalidOutputSpecification);
            }
            Ok(Self(value.to_bits()))
        }

        pub fn to_be_bytes(self) -> [u8; 4] {
            self.0.to_be_bytes()
        }
    }

    impl PngOutputSpecification {
        pub fn try_new(
            _pages: PageSelection,
            _pixels_per_inch: PngPixelsPerInch,
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

        pub fn version(&self) -> u32 {
            self.version
        }
        pub fn max_entries(&self) -> u64 {
            self.max_entries
        }
        pub fn max_canonical_entry_bytes(&self) -> u64 {
            self.max_canonical_entry_bytes
        }
    }

    #[derive(Clone, Debug)]
    pub struct CompilationRequest {
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

        pub fn diagnostics(&self) -> Option<&CanonicalDiagnosticPolicy> {
            unimplemented!()
        }

        pub fn try_new(
            _output: CompilationOutputSpecification,
            _diagnostics: CanonicalDiagnosticPolicy,
            _inputs: impl IntoIterator<Item = (TypstInputKey, TypstInputValue)>,
            _features: impl IntoIterator<Item = EngineFeature>,
            _document_time: CompilationDocumentTime,
            _overrides: PackOverrideSet,
        ) -> Self {
            unimplemented!()
        }

        /// Boundedly retains raw declarations so preparation can return one
        /// complete, ordered rejection instead of failing at the first parser.
        pub fn from_declarations(
            _admission: &OrdinaryAdmission,
            _limits: &AdmittedOperationResourceLimits<CompilationResourceLimits>,
            _output: CompilationOutputDeclarations,
            _diagnostics: CanonicalDiagnosticPolicyDeclarations,
            _members: impl IntoIterator<Item = CompilationRequestDeclaration>,
        ) -> Self {
            unimplemented!()
        }
    }

    pub enum CompilationRequestDeclaration {
        PackOverride {
            declaration_ordinal: u64,
            path: String,
            bytes: StableByteValue,
            origin: CompilationInventoryOrigin,
        },
        TypstInput {
            declaration_ordinal: u64,
            key: String,
            value: Vec<u8>,
            origin: CompilationInventoryOrigin,
        },
        DocumentTime {
            declaration_ordinal: u64,
            unix_seconds: Option<i64>,
            origin: CompilationInventoryOrigin,
        },
        Feature {
            declaration_ordinal: u64,
            identifier: String,
            origin: CompilationInventoryOrigin,
        },
    }

    pub struct CanonicalDiagnosticPolicyDeclarations {
        pub members: Vec<CanonicalDiagnosticPolicyDeclaration>,
    }

    pub enum CanonicalDiagnosticPolicyDeclaration {
        Version {
            declaration_ordinal: u64,
            value: u64,
            origin: CompilationInventoryOrigin,
        },
        MaxEntries {
            declaration_ordinal: u64,
            value: u64,
            origin: CompilationInventoryOrigin,
        },
        MaxCanonicalEntryBytes {
            declaration_ordinal: u64,
            value: u64,
            origin: CompilationInventoryOrigin,
        },
    }

    pub struct CompilationOutputDeclarations {
        pub format: CompilationOutputFormatDeclaration,
        pub controls: Vec<CompilationOutputControlDeclaration>,
    }

    pub struct CompilationOutputFormatDeclaration {
        pub declaration_ordinal: u64,
        pub value: String,
        pub origin: CompilationInventoryOrigin,
    }

    pub enum CompilationOutputControlDeclaration {
        PageRange {
            declaration_ordinal: u64,
            start: u64,
            end: u64,
            origin: CompilationInventoryOrigin,
        },
        PdfIdentifier {
            declaration_ordinal: u64,
            value: PdfIdentifierMode,
            origin: CompilationInventoryOrigin,
        },
        PdfCreator {
            declaration_ordinal: u64,
            value: PdfCreatorMode,
            origin: CompilationInventoryOrigin,
        },
        PdfCreationTime {
            declaration_ordinal: u64,
            unix_seconds: Option<i64>,
            origin: CompilationInventoryOrigin,
        },
        PdfStandard {
            declaration_ordinal: u64,
            identifier: String,
            origin: CompilationInventoryOrigin,
        },
        PdfTagging {
            declaration_ordinal: u64,
            value: PdfTagging,
            origin: CompilationInventoryOrigin,
        },
        PixelsPerInch {
            declaration_ordinal: u64,
            value: f64,
            origin: CompilationInventoryOrigin,
        },
        Bleed {
            declaration_ordinal: u64,
            value: bool,
            origin: CompilationInventoryOrigin,
        },
        Pretty {
            declaration_ordinal: u64,
            value: bool,
            origin: CompilationInventoryOrigin,
        },
    }

    pub fn prepare(
        _admission: &OrdinaryAdmission,
        _limits: &AdmittedOperationResourceLimits<CompilationResourceLimits>,
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

        pub fn engine_neutral_intent_identity(&self) -> &EngineNeutralCompilationIntentIdentity {
            unimplemented!()
        }

        pub fn engine_identity(&self) -> &EngineIdentity {
            unimplemented!()
        }

        pub fn exporter_identity(&self) -> &ExporterIdentity {
            unimplemented!()
        }

        pub fn request_inventory(&self) -> CompilationRequestInventoryView<'_> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct CompilationRequestRejection {
        private: crate::private::CompilationRequestRejectionState,
    }

    impl CompilationRequestRejection {
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn requested_limits(&self) -> &CompilationResourceLimits {
            unimplemented!()
        }
        pub fn admitted_limits(&self) -> &CompilationResourceLimits {
            unimplemented!()
        }
        pub fn request_inventory(&self) -> CompilationRequestInventoryView<'_> {
            unimplemented!()
        }

        pub fn issues(&self) -> impl ExactSizeIterator<Item = CompilationRequestIssueView> + '_ {
            std::iter::empty()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilationRequestIssueView {
        pub code: CompilationRequestIssueCode,
        pub role: CompilationRequestInventoryRole,
        pub declaration_ordinal: u64,
        pub referenced_inventory_ordinal: Option<u64>,
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

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum CompilationRequestIssueCode {
        InvalidSyntax,
        NonCanonicalValue,
        DuplicateLogicalKey,
        UnknownPackPath,
        InvalidUtf8Value,
        UnsupportedFeature,
        InapplicableFormatControl,
        IncompatibleOutputControls,
        OutOfEngineRange,
        MemberLimitExceeded,
        AggregateLimitExceeded,
        FormatCeilingExceeded,
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

        pub fn request_inventory(&self) -> CompilationRequestInventoryView<'_> {
            unimplemented!()
        }

        pub fn current_attempt_evidence(&self) -> DependencyEvidenceTableView<'_> {
            unimplemented!()
        }

        pub fn originating_evidence(&self) -> OriginatingEvidenceAvailabilityView<'_> {
            unimplemented!()
        }

        pub fn access_trace(&self) -> CompilationReportAccessTraceView<'_> {
            unimplemented!()
        }

        pub fn result(&self) -> Option<&CompilationResult> {
            unimplemented!()
        }

        pub fn operational_inventory(&self) -> CompilationOperationalInventoryView<'_> {
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

    pub struct CompilationRequestInventoryView<'a> {
        private: &'a crate::private::CompilationRequestInventoryState,
    }

    impl CompilationRequestInventoryView<'_> {
        pub fn entries(
            &self,
        ) -> impl ExactSizeIterator<Item = CompilationRequestInventoryEntryView<'_>> {
            std::iter::empty()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationRequestInventoryStatus {
        Effective,
        SuppliedCanonical,
        RejectedSafeValue,
        InvalidDeclaration,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationRequestInventoryRole {
        Pack,
        PackOverride,
        TypstInput,
        DocumentTime,
        Feature,
        Target,
        Output,
        Diagnostics,
        Engine,
        Exporter,
        PageSelection,
        PdfControl,
        PngPixelsPerInch,
        FormatControl,
        RequestLimit,
    }

    pub enum CompilationRequestInventoryEntryView<'a> {
        Pack {
            identity: &'a crate::pack::PackIdentity,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
        },
        PackOverride {
            path: &'a ProjectPath,
            exact_bytes: u64,
            commitment: Option<&'a CompilationRequestCommitment>,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            declaration_ordinal: Option<u64>,
        },
        TypstInput {
            key: &'a TypstInputKey,
            exact_utf8_bytes: u64,
            commitment: &'a CompilationRequestCommitment,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            declaration_ordinal: Option<u64>,
        },
        DocumentTime {
            value: CompilationDocumentTime,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            declaration_ordinal: Option<u64>,
        },
        Feature {
            value: &'a EngineFeature,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            declaration_ordinal: Option<u64>,
        },
        Target {
            value: TypstTarget,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            declaration_ordinal: Option<u64>,
        },
        Output(CompilationOutputInventoryView<'a>),
        Diagnostics(CanonicalDiagnosticInventoryView<'a>),
        Engine {
            identity: &'a EngineIdentity,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
        },
        Exporter {
            identity: &'a ExporterIdentity,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
        },
        InvalidDeclaration {
            role: CompilationRequestInventoryRole,
            declaration_ordinal: u64,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
            issues: &'a [CompilationRequestIssueCode],
            referenced_safe_inventory_ordinal: Option<u64>,
        },
    }

    pub struct CanonicalDiagnosticInventoryView<'a> {
        pub effective_policy: Option<&'a CanonicalDiagnosticPolicy>,
        pub version: InventoryLeaf<u64>,
        pub max_entries: InventoryLeaf<u64>,
        pub max_canonical_entry_bytes: InventoryLeaf<u64>,
        pub status: CompilationRequestInventoryStatus,
    }

    pub enum CompilationOutputInventoryView<'a> {
        Pdf(PdfOutputInventoryView<'a>),
        Png(PngOutputInventoryView<'a>),
        Svg(SvgOutputInventoryView<'a>),
        Html(HtmlOutputInventoryView),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationOutputFormat {
        Pdf,
        Png,
        Svg,
        Html,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct InventoryLeaf<T> {
        pub value: T,
        pub origin: CompilationInventoryOrigin,
        pub status: CompilationRequestInventoryStatus,
        pub declaration_ordinal: Option<u64>,
    }

    pub enum PageSelectionInventoryView<'a> {
        All {
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
        },
        Ranges(&'a [InventoryLeaf<(u32, u32)>]),
    }

    pub struct PdfOutputInventoryView<'a> {
        pub format: InventoryLeaf<CompilationOutputFormat>,
        pub pages: PageSelectionInventoryView<'a>,
        pub identifier: InventoryLeaf<&'a PdfIdentifierMode>,
        pub creator: InventoryLeaf<&'a PdfCreatorMode>,
        pub creation_time: InventoryLeaf<PdfCreationTime>,
        pub standards: &'a [InventoryLeaf<PdfStandard>],
        pub tagging: InventoryLeaf<PdfTagging>,
        pub pretty: InventoryLeaf<bool>,
        pub status: CompilationRequestInventoryStatus,
    }

    pub struct PngOutputInventoryView<'a> {
        pub format: InventoryLeaf<CompilationOutputFormat>,
        pub pages: PageSelectionInventoryView<'a>,
        pub pixels_per_inch: InventoryLeaf<PngPixelsPerInch>,
        pub bleed: InventoryLeaf<bool>,
        pub status: CompilationRequestInventoryStatus,
    }

    pub struct SvgOutputInventoryView<'a> {
        pub format: InventoryLeaf<CompilationOutputFormat>,
        pub pages: PageSelectionInventoryView<'a>,
        pub bleed: InventoryLeaf<bool>,
        pub pretty: InventoryLeaf<bool>,
        pub status: CompilationRequestInventoryStatus,
    }

    pub struct HtmlOutputInventoryView {
        pub format: InventoryLeaf<CompilationOutputFormat>,
        pub pretty: InventoryLeaf<bool>,
        pub status: CompilationRequestInventoryStatus,
    }

    pub struct CompilationOperationalInventoryView<'a> {
        private: &'a crate::private::CompilationReportState,
    }

    impl CompilationOperationalInventoryView<'_> {
        pub fn admission(&self) -> CompilationAdmissionInventoryView<'_> {
            unimplemented!()
        }
        pub fn resources(&self) -> CompilationResourcesInventoryView<'_> {
            unimplemented!()
        }
        pub fn dependency_execution(&self) -> CompilationDependencyExecutionInventoryView<'_> {
            unimplemented!()
        }
        pub fn attempt_control(&self) -> CompilationAttemptControlInventoryView<'_> {
            unimplemented!()
        }
        pub fn role_execution(&self) -> CompilationExecutionInventoryView<'_> {
            unimplemented!()
        }
        pub fn reporting(&self) -> CompilationReportingInventoryView<'_> {
            unimplemented!()
        }
    }

    pub struct CompilationAdmissionInventoryView<'a> {
        pub requested_trust: crate::DeploymentTrustProfile,
        pub admitted_trust: crate::DeploymentTrustProfile,
        pub requested_network: crate::OperationNetworkPolicy,
        pub admitted_network: crate::OperationNetworkPolicy,
        pub contractual_no_network: bool,
        pub structural_network_enforcement: crate::EnforcementStrength,
        pub enforcement: crate::EnforcementAdmissionView<'a>,
    }

    pub struct CompilationResourcesInventoryView<'a> {
        pub profile: Option<&'a crate::ResourceProfileIdentity>,
        pub requested: &'a CompilationResourceLimits,
        pub admitted: &'a CompilationResourceLimits,
    }

    pub struct CompilationDependencyExecutionInventoryView<'a> {
        pub packages: &'a super::authority::PackageAuthorityCapabilityDescriptor,
        pub fonts: &'a super::authority::FontAuthorityCapabilityDescriptor,
        pub cache_descriptor: Option<&'a SemanticResultCacheCapabilityDescriptor>,
        pub cache_policy: SemanticResultCachePolicy,
        pub cache_lookup: SemanticCacheLookupState,
        pub cache_isolation_domain_present: bool,
        pub offline_roles_covered: &'a [crate::PreCommitFacilityRole],
        pub concurrency: crate::DependencyConcurrencyAdmission,
    }

    pub struct CompilationAttemptControlInventoryView<'a> {
        pub deadline: &'a OperationDeadline,
        pub cancellation_present: bool,
        pub monotonic_domain: &'a crate::MonotonicTimeDomain,
        pub queue_timeout_ticks: Option<u64>,
        pub latency_target_ticks: Option<u64>,
        pub session_supersession: SessionSupersessionInventory,
        pub requested_interruption: crate::OperationInterruptionStrength,
        pub admitted_interruption: crate::OperationInterruptionStrength,
        pub winner: Option<CompilationInterruptionWinner>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionSupersessionInventory {
        NotApplicable,
        Bound,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationInterruptionWinner {
        TerminalCommitment,
        Cancellation,
        Deadline,
        Supersession,
        ForcedTermination,
    }

    pub enum CompilationExecutionInventoryView<'a> {
        CallerThread {
            domain: EngineRuntimeDomainSelectionView<'a>,
            engine_width: crate::EngineWidthAdmission,
        },
        Facility {
            descriptor: &'a CompilationExecutionFacilityCapabilityDescriptor,
            domain: EngineRuntimeDomainSelectionView<'a>,
            engine_width: crate::EngineWidthAdmission,
            capacity: CompilationExecutionFacilityCapacity,
            queue_reached: bool,
            dispatch_reached: bool,
            worker_terminated: bool,
            worker_reaped: bool,
        },
    }

    pub struct CompilationReportingInventoryView<'a> {
        pub requested: &'a CompilationReportingPolicy,
        pub admitted: &'a CompilationReportingPolicy,
        pub diagnostic_projection: ReportingChannelStatus,
        pub diagnostic_sources: ReportingChannelStatus,
        pub timing: ReportingChannelStatus,
        pub fine_engine_timing: ReportingChannelStatus,
        pub fine_timing_lease_reached: bool,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ReportingChannelStatus {
        NotRequested,
        Complete,
        Limited,
        Unavailable,
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
    pub enum SemanticCacheLookupState {
        Disabled,
        Miss,
        VerifiedHit,
        UnavailableContinued,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyEvidenceScope {
        None,
        Partial,
        CompleteCurrentAttempt,
        HistoricalCacheTraceOnly,
    }

    pub struct DependencyEvidenceTableView<'a> {
        private: &'a crate::private::CompilationReportState,
    }

    impl DependencyEvidenceTableView<'_> {
        pub fn entries(&self) -> impl ExactSizeIterator<Item = DependencyEvidenceEntryView<'_>> {
            std::iter::empty()
        }
    }

    pub enum OriginatingEvidenceAvailabilityView<'a> {
        Available(DependencyEvidenceTableView<'a>),
        Unavailable,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct OriginatingEvidenceReference(u64);

    impl OriginatingEvidenceReference {
        pub fn ordinal(self) -> u64 {
            self.0
        }
    }

    pub struct DependencyEvidenceEntryView<'a> {
        pub ordinal: OriginatingEvidenceReference,
        pub subject: DependencyEvidenceSubjectView<'a>,
        pub authority_role: DependencyEvidenceAuthorityRole,
        pub authority_priority: u32,
        pub kind: DependencyEvidenceEntryKind,
        pub key: Option<&'a crate::authority::DependencyEvidenceKey>,
        pub provenance: Option<&'a crate::authority::AcquisitionProvenance>,
        pub phase: DependencyEvidencePhase,
    }

    pub enum DependencyEvidenceSubjectView<'a> {
        ProjectAccess {
            path: &'a ProjectPath,
            request_kind: TypstFileRequestKind,
        },
        PackageRequirement(&'a PackageRequirementIdentity),
        PackageAccess {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            request_kind: TypstFileRequestKind,
        },
        UndeclaredPackageAccess {
            specification: &'a crate::PackageSpecification,
            path: &'a crate::PackagePath,
            request_kind: TypstFileRequestKind,
        },
        FontRequirement(&'a FontRequirementIdentity),
        FontFaceAccess {
            container: &'a crate::ContentIdentity,
            face_index: u32,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyEvidenceAuthorityRole {
        Pack,
        Package,
        Font,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum DependencyEvidenceEntryKind {
        SelectedContent,
        ConfirmedAbsence,
        ConfirmedMembership,
        ConfirmedOrder,
        ConfirmedMetadata,
        SelectedSourceChoice,
        HigherPriorityUnavailable,
        Missing,
        Acquired,
        TransientFailure,
        PermanentFailure,
        InvalidContent,
        IntegrityMismatch,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyEvidencePhase {
        DependencyResolution,
        CompilationKernel,
    }

    pub struct SafeDependencyEvidenceTableView<'a> {
        private: &'a crate::private::CompilationReportProjectionState,
    }

    impl SafeDependencyEvidenceTableView<'_> {
        pub fn entries(
            &self,
        ) -> impl ExactSizeIterator<Item = SafeDependencyEvidenceEntryView<'_>> {
            std::iter::empty()
        }
    }

    pub struct SafeDependencyEvidenceEntryView<'a> {
        pub ordinal: OriginatingEvidenceReference,
        pub subject: DependencyEvidenceSubjectView<'a>,
        pub authority_role: DependencyEvidenceAuthorityRole,
        pub authority_priority: u32,
        pub kind: DependencyEvidenceEntryKind,
        pub sanitized_authority_kind: &'a str,
        pub source_class: Option<crate::pack::AcquisitionSourceClass>,
        pub phase: DependencyEvidencePhase,
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
        pub artifact_identity: &'a CompilationArtifactIdentity,
        pub content_identity: &'a crate::ContentIdentity,
        pub exact_bytes: u64,
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
        pub fn policy(&self) -> &CanonicalDiagnosticPolicy {
            unimplemented!()
        }
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
        pub kind: DiagnosticKind<'a>,
        pub message: &'a str,
        pub spans: &'a [DiagnosticSpanView<'a>],
        pub hints: &'a [String],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticKind<'a> {
        Stable(&'a str),
        EngineSpecific {
            engine: &'a EngineIdentity,
            name: &'a str,
        },
        ExporterSpecific {
            exporter: &'a ExporterIdentity,
            name: &'a str,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct DiagnosticSpanView<'a> {
        pub location: DiagnosticLogicalLocation<'a>,
        pub start: u64,
        pub end: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DiagnosticLogicalLocation<'a> {
        ProjectBaseline(&'a ProjectPath),
        ProjectOverride {
            path: &'a ProjectPath,
            commitment: &'a CompilationRequestCommitment,
        },
        Package {
            requirement: &'a PackageRequirementIdentity,
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
        private: &'a crate::private::CompilationAccessTraceState,
    }

    impl CompilationAccessTraceView<'_> {
        pub fn observations(
            &self,
        ) -> impl ExactSizeIterator<Item = CompilationAccessObservationView<'_>> {
            std::iter::empty()
        }

        pub fn reached_scope(&self) -> CompilationAccessTraceReachedScope {
            unimplemented!()
        }
    }

    pub enum CompilationReportAccessTraceView<'a> {
        NotReached,
        ResultOwned(CompilationAccessTraceView<'a>),
        Partial(CompilationAccessTraceView<'a>),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TypstFileRequestKind {
        TypstSource,
        RawFile,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DependencyFulfillment {
        Embedded,
        External,
    }

    pub struct IdentityObjectView<'a> {
        pub exact_bytes: u64,
        pub content_identity: &'a crate::ContentIdentity,
    }

    pub struct CompilationSensitiveValueView<'a> {
        pub exact_bytes: u64,
        pub commitment: &'a CompilationRequestCommitment,
    }

    pub struct CompilationAccessObservationView<'a> {
        pub semantic: CompilationAccessObservationKindView<'a>,
        pub originating_evidence: &'a [OriginatingEvidenceReference],
    }

    pub enum CompilationAccessObservationKindView<'a> {
        ProjectBaselineRead {
            path: &'a ProjectPath,
            request_kind: TypstFileRequestKind,
            object: IdentityObjectView<'a>,
        },
        ProjectOverrideRead {
            path: &'a ProjectPath,
            request_kind: TypstFileRequestKind,
            replacement: CompilationSensitiveValueView<'a>,
        },
        ProjectLogicalMissing {
            path: &'a ProjectPath,
            request_kind: TypstFileRequestKind,
        },
        ProjectBaselineInvalidAsSource {
            path: &'a ProjectPath,
            object: IdentityObjectView<'a>,
        },
        ProjectOverrideInvalidAsSource {
            path: &'a ProjectPath,
            replacement: CompilationSensitiveValueView<'a>,
        },
        PackageRead {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            request_kind: TypstFileRequestKind,
            object: IdentityObjectView<'a>,
            fulfillment: DependencyFulfillment,
        },
        PackageLogicalMissing {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            request_kind: TypstFileRequestKind,
            fulfillment: DependencyFulfillment,
        },
        PackageInvalidAsSource {
            requirement: &'a PackageRequirementIdentity,
            path: &'a crate::PackagePath,
            object: IdentityObjectView<'a>,
            fulfillment: DependencyFulfillment,
        },
        UndeclaredPackage {
            specification: &'a crate::PackageSpecification,
            path: &'a crate::PackagePath,
            request_kind: TypstFileRequestKind,
        },
        FontFace {
            container: &'a crate::ContentIdentity,
            face_index: u32,
            fulfillment: DependencyFulfillment,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilationAccessTraceReachedScope {
        pub phase: CompilationTracePhase,
        pub project: CompilationTraceClassScope,
        pub packages: CompilationTraceClassScope,
        pub fonts: CompilationTraceClassScope,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationTracePhase {
        DependencyResolution,
        CompilationKernel,
        Export,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationTraceClassScope {
        NotReached,
        Partial,
        CompleteForReachedExecution,
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
        OverrideCount,
        LargestOverride,
        AggregateOverride,
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
        pub override_count: u64,
        pub largest_override_bytes: u64,
        pub aggregate_override_bytes: u64,
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

        pub fn spec(&self) -> &CompilationResourceLimitSpec {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SemanticCacheAvailabilityPolicy {
        Required,
        ContinueOnUnavailable,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SemanticResultCachePolicy {
        Disabled,
        ReadOnly(SemanticCacheAvailabilityPolicy),
        ReadWrite(SemanticCacheAvailabilityPolicy),
        RebuildAndWrite,
    }

    #[derive(Clone, Debug)]
    pub struct SemanticResultCacheCapabilityDescriptor {
        private: crate::private::SemanticResultCacheCapabilityDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct SemanticResultCacheCapabilitySpec {
        pub class: crate::OperationalCapabilityClass,
        pub network: crate::SelectedNetworkContract,
        pub trusted_writer_domain: bool,
        pub authenticated_records: bool,
        pub required_availability: bool,
        pub continue_on_unavailable: bool,
    }

    impl SemanticResultCacheCapabilityDescriptor {
        pub fn try_new(
            _isolation: CacheIsolationDomain,
            _spec: SemanticResultCacheCapabilitySpec,
        ) -> Result<Self, crate::OperationAdmissionRefusalReason> {
            unimplemented!()
        }

        pub fn descriptor_version(&self) -> u32 {
            1
        }

        pub fn class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn network(&self) -> crate::SelectedNetworkContract {
            unimplemented!()
        }

        pub fn capabilities(&self) -> SemanticResultCacheCapabilityView<'_> {
            unimplemented!()
        }
    }

    pub struct SemanticResultCacheCapabilityView<'a> {
        pub class: &'a crate::OperationalCapabilityClass,
        pub isolation_domain_present: bool,
        pub network: crate::SelectedNetworkContract,
        pub trusted_writer_domain: bool,
        pub authenticated_records: bool,
        pub required_availability: bool,
        pub continue_on_unavailable: bool,
    }

    pub enum SyncSemanticCacheLookup<'a, C: ?Sized> {
        Disabled,
        Enabled { cache: &'a C },
    }

    pub enum AsyncSemanticCacheLookup<'a, C: ?Sized> {
        Disabled,
        Enabled { cache: &'a C },
    }

    #[derive(Clone, Debug)]
    pub struct CompilationReportingPolicy {
        pub diagnostic_projection: bool,
        pub diagnostic_source_bundle: bool,
        pub timing: bool,
        pub fine_engine_timing: bool,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationOperationRequest {
        pub network: crate::OperationNetworkPolicy,
        pub cache: SemanticResultCachePolicy,
        pub dependency_concurrency: NonZeroUsize,
        pub engine_width: crate::EngineWidthRequest,
        pub requested_ready_jobs: Option<NonZeroUsize>,
        pub requested_queue: Option<usize>,
        pub requested_workers: Option<NonZeroUsize>,
        pub placement: crate::ExecutionPlacement,
        pub interruption: crate::OperationInterruptionStrength,
        pub deadline: OperationDeadline,
        pub queue_timeout_ticks: Option<u64>,
        pub latency_target_ticks: Option<u64>,
        pub required_enforcement: Vec<crate::EnforcementClaim>,
        pub reporting: CompilationReportingPolicy,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationAdmissionRefusal {
        private: crate::private::CompilationAdmissionRefusalState,
    }

    impl CompilationAdmissionRefusal {
        pub fn operation_request(&self) -> &CompilationOperationRequest {
            unimplemented!()
        }
        pub fn requested_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn requested_limits(&self) -> &CompilationResourceLimits {
            unimplemented!()
        }
        pub fn packages(&self) -> &super::authority::PackageAuthorityCapabilityDescriptor {
            unimplemented!()
        }
        pub fn fonts(&self) -> &super::authority::FontAuthorityCapabilityDescriptor {
            unimplemented!()
        }
        pub fn cache(&self) -> Option<&SemanticResultCacheCapabilityDescriptor> {
            unimplemented!()
        }
        pub fn execution(&self) -> Option<&CompilationExecutionFacilityCapabilityDescriptor> {
            unimplemented!()
        }

        pub fn reason(&self) -> crate::OperationAdmissionRefusalReason {
            unimplemented!()
        }
    }

    pub struct SyncCompilationControls<'a, P: ?Sized, F: ?Sized, C: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
        packages: &'a P,
        fonts: &'a F,
        semantic_cache: SyncSemanticCacheLookup<'a, C>,
        request: CompilationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: Option<&'a dyn crate::InterruptionSource>,
        admission_record: crate::private::CompilationAdmissionRecordState,
    }

    impl<'a, P: ?Sized, F: ?Sized, C: ?Sized> SyncCompilationControls<'a, P, F, C> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_admit(
            _admission: OrdinaryAdmission,
            _limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
            _packages: &'a P,
            _fonts: &'a F,
            _semantic_cache: SyncSemanticCacheLookup<'a, C>,
            _request: CompilationOperationRequest,
            _clock: &'a dyn crate::MonotonicClock,
            _interruption: Option<&'a dyn crate::InterruptionSource>,
        ) -> Result<Self, CompilationAdmissionRefusal>
        where
            P: SyncPackageAuthority,
            F: SyncFontAuthority,
            C: SyncSemanticResultCache,
        {
            unimplemented!()
        }

        pub(crate) fn bind_session(
            self,
            _permit: &crate::session::SessionSupersessionPermit,
            _limits: &crate::session::SessionPreparationLimits,
        ) -> Result<Self, CompilationAdmissionRefusal> {
            unimplemented!()
        }
    }

    pub struct AsyncCompilationControls<'a, P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
        packages: &'a P,
        fonts: &'a F,
        semantic_cache: AsyncSemanticCacheLookup<'a, C>,
        execution: &'a X,
        request: CompilationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: Option<&'a dyn crate::InterruptionSource>,
        admission_record: crate::private::CompilationAdmissionRecordState,
    }

    impl<'a, P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized> AsyncCompilationControls<'a, P, F, C, X> {
        #[allow(clippy::too_many_arguments)]
        pub fn try_admit(
            _admission: OrdinaryAdmission,
            _limits: AdmittedOperationResourceLimits<CompilationResourceLimits>,
            _packages: &'a P,
            _fonts: &'a F,
            _semantic_cache: AsyncSemanticCacheLookup<'a, C>,
            _execution: &'a X,
            _request: CompilationOperationRequest,
            _clock: &'a dyn crate::MonotonicClock,
            _interruption: Option<&'a dyn crate::InterruptionSource>,
        ) -> Result<Self, CompilationAdmissionRefusal>
        where
            P: AsyncPackageAuthority,
            F: AsyncFontAuthority,
            C: AsyncSemanticResultCache,
            X: CompilationExecutionFacility,
        {
            unimplemented!()
        }

        pub(crate) fn bind_session(
            self,
            _permit: &crate::session::SessionSupersessionPermit,
            _limits: &crate::session::SessionPreparationLimits,
        ) -> Result<Self, CompilationAdmissionRefusal> {
            unimplemented!()
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
        match prepare(admission, &controls.limits, pack, request) {
            Ok(prepared) => CompilationTerminal::Report(run_sync(&prepared, controls)),
            Err(rejection) => CompilationTerminal::RequestRejected(rejection),
        }
    }

    pub trait SyncSemanticResultCache {
        fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor;

        fn lookup(&self, request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome;

        fn admit(
            &self,
            request: SemanticCacheAdmissionRequest<'_>,
        ) -> SemanticCacheAdapterAdmissionOutcome;
    }

    pub trait AsyncSemanticResultCache {
        type Lookup<'a>: Future<Output = SemanticCacheLookupOutcome> + 'a
        where
            Self: 'a;

        type Admit<'a>: Future<Output = SemanticCacheAdapterAdmissionOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor;

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
        pub(crate) fn try_new(
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
        pub(crate) fn try_new(
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

    pub enum SemanticCacheAdapterAdmissionOutcome {
        Admitted,
        Unavailable,
        AuthorizationRefused,
        IntegrityFailure,
        ConflictingResult,
        ResourceLimit,
        Cancelled,
        Deadline,
    }

    impl From<SemanticCacheAdapterAdmissionOutcome> for SemanticCacheAdmissionOutcome {
        fn from(value: SemanticCacheAdapterAdmissionOutcome) -> Self {
            match value {
                SemanticCacheAdapterAdmissionOutcome::Admitted => Self::Admitted,
                SemanticCacheAdapterAdmissionOutcome::Unavailable => Self::Unavailable,
                SemanticCacheAdapterAdmissionOutcome::AuthorizationRefused => {
                    Self::AuthorizationRefused
                }
                SemanticCacheAdapterAdmissionOutcome::IntegrityFailure => Self::IntegrityFailure,
                SemanticCacheAdapterAdmissionOutcome::ConflictingResult => Self::ConflictingResult,
                SemanticCacheAdapterAdmissionOutcome::ResourceLimit => Self::ResourceLimit,
                SemanticCacheAdapterAdmissionOutcome::Cancelled => Self::Cancelled,
                SemanticCacheAdapterAdmissionOutcome::Deadline => Self::Deadline,
            }
        }
    }

    pub struct CompilationCacheAdmissionOutcome {
        pub report: CompilationReport,
        pub cache: SemanticCacheAdmissionOutcome,
    }

    pub fn admit_to_cache_sync<C: SyncSemanticResultCache + ?Sized>(
        _report: CompilationReport,
        _cache: &C,
        _clock: &dyn crate::MonotonicClock,
        _interruption: &dyn crate::InterruptionSource,
    ) -> CompilationCacheAdmissionOutcome {
        unimplemented!()
    }

    pub async fn admit_to_cache_async<C: AsyncSemanticResultCache + ?Sized>(
        _report: CompilationReport,
        _cache: &C,
        _clock: &dyn crate::MonotonicClock,
        _interruption: &dyn crate::InterruptionSource,
    ) -> CompilationCacheAdmissionOutcome {
        unimplemented!()
    }

    pub struct NoSemanticResultCache {
        descriptor: SemanticResultCacheCapabilityDescriptor,
    }

    impl NoSemanticResultCache {
        pub fn new(_isolation: CacheIsolationDomain) -> Self {
            unimplemented!()
        }
    }

    impl SyncSemanticResultCache for NoSemanticResultCache {
        fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor {
            &self.descriptor
        }

        fn lookup(&self, _request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome {
            SemanticCacheLookupOutcome::Miss
        }

        fn admit(
            &self,
            _request: SemanticCacheAdmissionRequest<'_>,
        ) -> SemanticCacheAdapterAdmissionOutcome {
            SemanticCacheAdapterAdmissionOutcome::Unavailable
        }
    }

    impl AsyncSemanticResultCache for NoSemanticResultCache {
        type Lookup<'a>
            = std::future::Ready<SemanticCacheLookupOutcome>
        where
            Self: 'a;

        type Admit<'a>
            = std::future::Ready<SemanticCacheAdapterAdmissionOutcome>
        where
            Self: 'a;

        fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor {
            &self.descriptor
        }

        fn lookup<'a>(&'a self, _request: SemanticCacheLookupRequest<'a>) -> Self::Lookup<'a> {
            std::future::ready(SemanticCacheLookupOutcome::Miss)
        }

        fn admit<'a>(&'a self, _request: SemanticCacheAdmissionRequest<'a>) -> Self::Admit<'a> {
            std::future::ready(SemanticCacheAdapterAdmissionOutcome::Unavailable)
        }
    }

    /// Post-commit preparation of a disposable cache record. Failure cannot
    /// mutate or replace the report.
    pub(crate) fn prepare_cache_admission(
        _report: CompilationReport,
        _limits: &CompilationResourceLimits,
    ) -> CompilationCacheRecordPreparation {
        unimplemented!()
    }

    pub(crate) struct CompilationCacheRecordPreparation {
        report: CompilationReport,
        record: Result<SemanticCacheRecord, SemanticCacheRecordFailure>,
    }

    impl CompilationCacheRecordPreparation {
        pub(crate) fn report(&self) -> &CompilationReport {
            &self.report
        }
        pub(crate) fn record(&self) -> Result<&SemanticCacheRecord, &SemanticCacheRecordFailure> {
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
    pub struct EngineRuntimeDomainPolicyDescriptor {
        private: crate::private::EngineRuntimeDomainPolicyDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct EngineRuntimeDomainPolicySpec {
        pub class: crate::OperationalCapabilityClass,
        pub managed: bool,
        pub supported_placements: Vec<crate::ExecutionPlacement>,
        pub width_policy: crate::EngineWidthRequest,
        pub sharing_scope: crate::OperationalCapabilityClass,
        pub exclusive_fine_timing_lease: bool,
    }

    impl EngineRuntimeDomainPolicyDescriptor {
        pub fn try_new(
            _spec: EngineRuntimeDomainPolicySpec,
        ) -> Result<Self, crate::OperationAdmissionRefusalReason> {
            unimplemented!()
        }

        pub fn descriptor_version(&self) -> u32 {
            1
        }

        pub fn class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn width_policy(&self) -> crate::EngineWidthRequest {
            unimplemented!()
        }

        pub fn capabilities(&self) -> EngineRuntimeDomainPolicyView<'_> {
            unimplemented!()
        }
    }

    pub struct EngineRuntimeDomainPolicyView<'a> {
        pub class: &'a crate::OperationalCapabilityClass,
        pub managed: bool,
        pub supported_placements: &'a [crate::ExecutionPlacement],
        pub width_policy: crate::EngineWidthRequest,
        pub sharing_scope: &'a crate::OperationalCapabilityClass,
        pub exclusive_fine_timing_lease: bool,
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct EngineRuntimeDomainIdentity(Arc<str>);

    impl EngineRuntimeDomainIdentity {
        pub fn try_new(value: &str) -> Result<Self, crate::DomainValueRejection> {
            if value.is_empty() {
                return Err(crate::DomainValueRejection::Empty);
            }
            Ok(Self(Arc::from(value)))
        }
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EngineRuntimeDomainManagement {
        InheritedUnmanaged,
        Managed,
    }

    #[derive(Clone, Copy)]
    pub enum EngineRuntimeDomainSelectionView<'a> {
        InheritedUnmanaged,
        Managed {
            identity: &'a EngineRuntimeDomainIdentity,
            placement: crate::ExecutionPlacement,
            width: NonZeroUsize,
            fine_timing_lease_reached: bool,
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct CompilationExecutionFacilityCapacity {
        pub requested_ready_jobs: NonZeroUsize,
        pub admitted_ready_jobs: NonZeroUsize,
        pub requested_queue: usize,
        pub admitted_queue: usize,
        pub requested_workers: Option<NonZeroUsize>,
        pub admitted_workers: Option<NonZeroUsize>,
        pub constraints: Vec<crate::AdmissionConstraint>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilationExecutionFacilityMaximumCapacity {
        pub ready_jobs: NonZeroUsize,
        pub queue: usize,
        pub workers: Option<NonZeroUsize>,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationExecutionFacilityCapabilityDescriptor {
        private: crate::private::CompilationExecutionFacilityCapabilityDescriptorState,
    }

    #[derive(Clone, Debug)]
    pub struct CompilationExecutionFacilityCapabilitySpec {
        pub class: crate::OperationalCapabilityClass,
        pub capacity_scope_class: crate::OperationalCapabilityClass,
        pub shared_with_creation: bool,
        pub supported_placements: Vec<crate::ExecutionPlacement>,
        pub ready_job_capacity: NonZeroUsize,
        pub queue_capacity: usize,
        pub worker_capacity: Option<NonZeroUsize>,
        pub overlapping_jobs_per_worker: bool,
        pub domain_policy: EngineRuntimeDomainPolicyDescriptor,
        pub execution_network: crate::SelectedNetworkContract,
        pub worker_control_network: Option<crate::SelectedNetworkContract>,
        pub interruption: crate::OperationInterruptionStrength,
        pub worker_protocol: Option<crate::OperationalCapabilityClass>,
        pub parent_verifies_response: bool,
        pub parent_withholds_output: bool,
        pub no_in_process_fallback: bool,
        pub terminate_and_reap: bool,
        pub forced_termination_target_ticks: Option<u64>,
        pub enforcement: Vec<crate::EnforcementClaim>,
    }

    pub struct CompilationExecutionFacilityCapabilityView<'a> {
        pub class: &'a crate::OperationalCapabilityClass,
        pub capacity_scope_class: &'a crate::OperationalCapabilityClass,
        pub shared_with_creation: bool,
        pub supported_placements: &'a [crate::ExecutionPlacement],
        pub maximum_capacity: CompilationExecutionFacilityMaximumCapacity,
        pub overlapping_jobs_per_worker: bool,
        pub domain_policy: &'a EngineRuntimeDomainPolicyDescriptor,
        pub execution_network: crate::SelectedNetworkContract,
        pub worker_control_network: Option<crate::SelectedNetworkContract>,
        pub interruption: crate::OperationInterruptionStrength,
        pub worker_protocol: Option<&'a crate::OperationalCapabilityClass>,
        pub parent_verifies_response: bool,
        pub parent_withholds_output: bool,
        pub no_in_process_fallback: bool,
        pub terminate_and_reap: bool,
        pub forced_termination_target_ticks: Option<u64>,
        pub enforcement: &'a [crate::EnforcementClaim],
    }

    impl CompilationExecutionFacilityCapabilityDescriptor {
        pub fn try_new(
            _spec: CompilationExecutionFacilityCapabilitySpec,
        ) -> Result<Self, crate::OperationAdmissionRefusalReason> {
            unimplemented!()
        }

        pub fn descriptor_version(&self) -> u32 {
            1
        }

        pub fn class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn capacity_scope_class(&self) -> &crate::OperationalCapabilityClass {
            unimplemented!()
        }

        pub fn domain_policy(&self) -> &EngineRuntimeDomainPolicyDescriptor {
            unimplemented!()
        }

        pub fn maximum_capacity(&self) -> CompilationExecutionFacilityMaximumCapacity {
            unimplemented!()
        }

        pub fn capabilities(&self) -> CompilationExecutionFacilityCapabilityView<'_> {
            unimplemented!()
        }
    }

    pub trait CompilationExecutionFacility {
        type Dispatch<'a>: Future<Output = CompilationDispatchOutcome> + 'a
        where
            Self: 'a;

        fn descriptor(&self) -> &CompilationExecutionFacilityCapabilityDescriptor;

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
            _parent_assigned_domain: EngineRuntimeDomainIdentity,
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

        pub fn with_canonical_diagnostics(
            self,
            _capability: CanonicalDiagnosticsDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_canonical_evidence(
            self,
            _capability: CanonicalEvidenceDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_diagnostic_sources(
            self,
            _capability: DiagnosticSourcesDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_request_values(self, _capability: RequestValuesDisclosureCapability) -> Self {
            unimplemented!()
        }

        pub fn with_override_bytes(self, _capability: OverrideBytesDisclosureCapability) -> Self {
            unimplemented!()
        }

        pub fn with_backing_locators(
            self,
            _capability: BackingLocatorsDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_adapter_detail(self, _capability: AdapterDetailDisclosureCapability) -> Self {
            unimplemented!()
        }

        pub fn project(
            &self,
            _report: &CompilationReport,
            _limits: &CompilationResourceLimits,
        ) -> CompilationReportProjection {
            unimplemented!()
        }

        pub fn project_terminal(
            &self,
            _terminal: &CompilationTerminal,
            _limits: &CompilationResourceLimits,
        ) -> CompilationTerminalProjection {
            unimplemented!()
        }

        pub fn plan_delivery(
            &self,
            _report: CompilationReport,
            _limits: &CompilationResourceLimits,
        ) -> CompilationDeliveryPlan {
            unimplemented!()
        }
    }

    macro_rules! disclosure_capability {
        ($name:ident, $state:ident) => {
            pub struct $name {
                private: crate::private::$state,
            }

            impl $name {
                pub fn explicitly_granted_by_caller() -> Self {
                    unimplemented!()
                }
            }
        };
    }

    disclosure_capability!(
        CanonicalDiagnosticsDisclosureCapability,
        CanonicalDiagnosticsDisclosureCapabilityState
    );
    disclosure_capability!(
        CanonicalEvidenceDisclosureCapability,
        CanonicalEvidenceDisclosureCapabilityState
    );
    disclosure_capability!(
        DiagnosticSourcesDisclosureCapability,
        DiagnosticSourcesDisclosureCapabilityState
    );
    disclosure_capability!(
        RequestValuesDisclosureCapability,
        RequestValuesDisclosureCapabilityState
    );
    disclosure_capability!(
        OverrideBytesDisclosureCapability,
        OverrideBytesDisclosureCapabilityState
    );
    disclosure_capability!(
        BackingLocatorsDisclosureCapability,
        BackingLocatorsDisclosureCapabilityState
    );
    disclosure_capability!(
        AdapterDetailDisclosureCapability,
        AdapterDetailDisclosureCapabilityState
    );

    pub struct CompilationReportProjection {
        private: crate::private::CompilationReportProjectionState,
    }

    impl CompilationReportProjection {
        pub fn terminal(&self) -> CompilationProjectionTerminalView<'_> {
            unimplemented!()
        }
        pub fn compilation_identity(&self) -> &CompilationIdentity {
            unimplemented!()
        }
        pub fn result_identity(&self) -> Option<&CompilationResultIdentity> {
            unimplemented!()
        }
        pub fn artifact_identities(
            &self,
        ) -> impl ExactSizeIterator<Item = CompilationArtifactDisclosureView<'_>> {
            std::iter::empty()
        }
        pub fn diagnostic_summary(&self) -> DiagnosticDisclosureSummary {
            unimplemented!()
        }
        pub fn canonical_diagnostic_policy(&self) -> &CanonicalDiagnosticPolicy {
            unimplemented!()
        }
        pub fn canonical_diagnostics_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn canonical_diagnostics(
            &self,
        ) -> impl ExactSizeIterator<Item = DiagnosticProjectionEntry<'_>> {
            std::iter::empty()
        }
        pub fn canonical_evidence_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn canonical_evidence(&self) -> Option<CanonicalEvidenceDisclosureView<'_>> {
            unimplemented!()
        }
        pub fn diagnostic_sources_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn diagnostic_sources(&self) -> impl ExactSizeIterator<Item = DisclosedSourceView<'_>> {
            std::iter::empty()
        }
        pub fn request_values_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn request_values(
            &self,
        ) -> impl ExactSizeIterator<Item = DisclosedRequestValueView<'_>> {
            std::iter::empty()
        }
        pub fn override_bytes_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn override_bytes(&self) -> impl ExactSizeIterator<Item = DisclosedOverrideView<'_>> {
            std::iter::empty()
        }
        pub fn backing_locators_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn backing_locators(
            &self,
        ) -> impl ExactSizeIterator<Item = DisclosedBackingLocatorView<'_>> {
            std::iter::empty()
        }
        pub fn adapter_detail_status(&self) -> CompilationDisclosureChannelStatus {
            unimplemented!()
        }
        pub fn adapter_detail(&self) -> impl ExactSizeIterator<Item = &'_ [u8]> {
            std::iter::empty()
        }
    }

    pub enum CompilationProjectionTerminalView<'a> {
        SemanticResult {
            status: CompilationResultStatus,
            document: CompilationDocumentSummaryView<'a>,
        },
        OperationOutcome {
            phase: CompilationPhase,
            cause: &'a CompilationOperationCause,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationResultStatus {
        Succeeded,
        Rejected,
    }

    pub enum CompilationTerminalProjection {
        RequestRejected(CompilationRequestRejectionProjection),
        Report(CompilationReportProjection),
    }

    pub struct CompilationRequestRejectionProjection {
        private: crate::private::CompilationRequestRejectionProjectionState,
    }

    impl CompilationRequestRejectionProjection {
        pub fn request_inventory(&self) -> CompilationRequestInventoryView<'_> {
            unimplemented!()
        }
        pub fn issues(&self) -> impl ExactSizeIterator<Item = CompilationRequestIssueView> + '_ {
            std::iter::empty()
        }
    }

    pub struct CompilationArtifactDisclosureView<'a> {
        pub role: CompilationArtifactRole,
        pub artifact_identity: &'a CompilationArtifactIdentity,
        pub content_identity: &'a crate::ContentIdentity,
        pub exact_bytes: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct DiagnosticDisclosureSummary {
        pub retained_entries: u64,
        pub retained_canonical_bytes: u64,
        pub completion: Option<CanonicalDiagnosticCompletion>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationDisclosureChannelStatus {
        NotRequested,
        Complete,
        Redacted,
        Limited,
        Unavailable,
    }

    pub struct CanonicalEvidenceDisclosureView<'a> {
        pub trace: CompilationAccessTraceView<'a>,
        pub evidence: Option<SafeDependencyEvidenceTableView<'a>>,
    }

    pub struct DisclosedSourceView<'a> {
        pub logical_location: DiagnosticLogicalLocation<'a>,
        pub bytes: &'a StableByteValue,
    }

    pub struct DisclosedRequestValueView<'a> {
        pub key: &'a TypstInputKey,
        pub value: &'a TypstInputValue,
    }

    pub struct DisclosedOverrideView<'a> {
        pub path: &'a ProjectPath,
        pub bytes: &'a StableByteValue,
        pub replacement_content_identity: &'a crate::ContentIdentity,
        pub equals_baseline: bool,
    }

    pub struct DisclosedBackingLocatorView<'a> {
        pub evidence: OriginatingEvidenceReference,
        pub safe_summary: &'a str,
        pub raw: &'a [u8],
    }

    pub struct CompilationDeliveryPlan {
        private: crate::private::CompilationDeliveryPlanState,
    }

    impl CompilationDeliveryPlan {
        pub fn projection(&self) -> &CompilationReportProjection {
            unimplemented!()
        }
        pub fn artifacts(&self) -> impl ExactSizeIterator<Item = CompilationArtifactView<'_>> {
            std::iter::empty()
        }

        pub(crate) fn transfer(&self) -> CompilationDeliveryTransfer<'_> {
            unimplemented!()
        }
    }

    pub struct CompilationDeliveryTransfer<'a> {
        private: &'a crate::private::CompilationDeliveryPlanState,
    }

    impl CompilationDeliveryTransfer<'_> {
        pub fn projection(&self) -> &CompilationReportProjection {
            unimplemented!()
        }
        pub fn artifacts(&self) -> impl ExactSizeIterator<Item = CompilationArtifactView<'_>> {
            std::iter::empty()
        }
    }

    pub struct DiagnosticProjectionEntry<'a> {
        pub phase: DiagnosticPhase,
        pub severity: DiagnosticSeverity,
        pub kind: DiagnosticKind<'a>,
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
        AdmittedOperationResourceLimits, ClosureExportPath, ClosureExportTreeContentIdentity,
        ContentIdentity, OperationDeadline, OrdinaryAdmission, Pack, PackIdentity, ProjectPath,
        StableByteValue,
    };
    use std::sync::Arc;

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

        pub fn spec(&self) -> &PackIngressResourceLimitSpec {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct RepresentationOperationRequest {
        pub network: crate::OperationNetworkPolicy,
        pub interruption: crate::OperationInterruptionStrength,
        pub deadline: OperationDeadline,
        pub required_enforcement: Vec<crate::EnforcementClaim>,
        pub timing_requested: bool,
    }

    pub struct PackIngressControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<PackIngressResourceLimits>,
        request: RepresentationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a> PackIngressControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<PackIngressResourceLimits>,
            request: RepresentationOperationRequest,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &request.deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                request,
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
    pub struct PackArchiveReadExpectations {
        expected_archive_content_identity: Option<ContentIdentity>,
        pack_identity: PackIdentityVerificationMode,
        asserted_archive_encoding_identity: Option<ArchiveEncodingIdentity>,
    }

    impl PackArchiveReadExpectations {
        pub fn new(pack_identity: PackIdentityVerificationMode) -> Self {
            Self {
                expected_archive_content_identity: None,
                pack_identity,
                asserted_archive_encoding_identity: None,
            }
        }

        pub fn with_expected_archive_content_identity(mut self, identity: ContentIdentity) -> Self {
            self.expected_archive_content_identity = Some(identity);
            self
        }

        pub fn with_asserted_archive_encoding_identity(
            mut self,
            identity: ArchiveEncodingIdentity,
        ) -> Self {
            self.asserted_archive_encoding_identity = Some(identity);
            self
        }
    }

    #[derive(Clone, Debug)]
    pub struct ClosureExportImportExpectations {
        pack_identity: PackIdentityVerificationMode,
    }

    impl ClosureExportImportExpectations {
        pub fn new(pack_identity: PackIdentityVerificationMode) -> Self {
            Self { pack_identity }
        }
    }

    pub fn read_pack_archive(
        _archive: StableByteValue,
        _expectations: PackArchiveReadExpectations,
        _controls: PackIngressControls<'_>,
    ) -> PackArchiveReadReport {
        unimplemented!()
    }

    #[derive(Clone)]
    pub struct ClosureExportInput {
        files: Vec<(ClosureExportPath, StableByteValue)>,
    }

    impl ClosureExportInput {
        pub fn try_new(
            _admission: &OrdinaryAdmission,
            _limits: &PackIngressResourceLimits,
            _files: impl IntoIterator<Item = (ClosureExportPath, StableByteValue)>,
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
        _expectations: ClosureExportImportExpectations,
        _controls: PackIngressControls<'_>,
    ) -> ClosureExportImportReport {
        unimplemented!()
    }

    pub struct PackArchiveReadReport {
        terminal: PackArchiveReadTerminal,
        receipt: PackArchiveReadFormatReceipt,
    }

    impl PackArchiveReadReport {
        pub fn terminal(&self) -> &PackArchiveReadTerminal {
            &self.terminal
        }
        pub fn receipt(&self) -> &PackArchiveReadFormatReceipt {
            &self.receipt
        }
        pub fn into_pack(self) -> Result<Pack, Self> {
            unimplemented!()
        }
    }

    pub struct ClosureExportImportReport {
        terminal: ClosureExportImportTerminal,
        receipt: ClosureExportImportFormatReceipt,
    }

    impl ClosureExportImportReport {
        pub fn terminal(&self) -> &ClosureExportImportTerminal {
            &self.terminal
        }
        pub fn receipt(&self) -> &ClosureExportImportFormatReceipt {
            &self.receipt
        }
        pub fn into_pack(self) -> Result<Pack, Self> {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RepresentationAdmissionRefusalReason {
        UnsupportedArchiveEncodingRecipe,
        DeploymentTrustProfileUnavailable,
        OperationNetworkPolicyUnavailable,
        OperationResourceLimitsUnavailable,
        InterruptionContractUnavailable,
        RequiredEnforcementUnavailable,
        ReportingUnavailable,
        CapacityUnavailable,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct RepresentationAdmissionRefusal {
        private: crate::private::RepresentationAdmissionRefusalState,
    }

    impl RepresentationAdmissionRefusal {
        pub fn reason(&self) -> RepresentationAdmissionRefusalReason {
            unimplemented!()
        }

        pub fn requested_controls(&self) -> FormatReceiptControlsView<'_> {
            unimplemented!()
        }
    }

    pub struct RepresentationAdmissionRecordView<'a> {
        pub requested: FormatReceiptControlsView<'a>,
        pub admitted: FormatReceiptControlsView<'a>,
    }

    pub enum RepresentationAdmissionDispositionView<'a> {
        Refused(&'a RepresentationAdmissionRefusal),
        Admitted(RepresentationAdmissionRecordView<'a>),
    }

    pub enum PackArchiveReadTerminal {
        Validated(Pack),
        Invalid(InvalidRepresentation),
        Unsupported(UnsupportedRepresentation),
        ExpectedPackIdentityMismatch,
        InputContentIdentityMismatch,
        ArchiveEncodingAssertionMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        AdmissionRefused(RepresentationAdmissionRefusal),
        InternalIntegrity,
    }

    pub enum ClosureExportImportTerminal {
        Validated(Pack),
        Invalid(InvalidRepresentation),
        Unsupported(UnsupportedRepresentation),
        ExpectedPackIdentityMismatch,
        ResourceLimit,
        Cancelled,
        Deadline,
        AdmissionRefused(RepresentationAdmissionRefusal),
        InternalIntegrity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatReceiptTerminal {
        Success,
        Invalid,
        Unsupported,
        ExpectedPackIdentityMismatch,
        ResourceOutcome,
        Cancelled,
        Deadline,
        AdmissionRefused,
        InternalIntegrity,
        TransportFailure,
        ArchiveEncodingAssertionMismatch,
        InputContentIdentityMismatch,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatReceiptRole {
        PackArchiveEncode,
        PackArchiveRead,
        ClosureExportProject,
        ProjectMaterialization,
        ClosureExportImport,
        PackArchivePublish,
        ClosureExportPublish,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatReceiptStage {
        Admission,
        ReferenceResolution,
        Acquisition,
        Spooling,
        RepresentationFraming,
        ControlRecord,
        ObjectVerification,
        Construction,
        EncodingOrProjection,
        Transfer,
        Commit,
        Cleanup,
        Complete,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatVerificationMode {
        NotApplicable,
        Derive,
        Verify,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ArchiveEncodingAssertionStatus {
        NotAsserted,
        SuppliedButUnevaluated,
        ExternallyAssertedAndByteVerified,
        ExternallyAssertedAndByteMismatched,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatTimingStatus {
        NotRequested,
        Complete,
        Limited,
        Unavailable,
    }

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct FormatReceiptCounters {
        pub input_bytes: Option<u64>,
        pub output_bytes: Option<u64>,
        pub control_record_bytes: Option<u64>,
        pub planned_objects: Option<u64>,
        pub verified_objects: Option<u64>,
        pub aggregate_decoded_bytes: Option<u64>,
        pub file_count: Option<u64>,
    }

    pub struct FormatReceiptCommonView<'a> {
        private: &'a crate::private::FormatReceiptState,
    }

    impl FormatReceiptCommonView<'_> {
        pub fn contract_version(&self) -> u32 {
            1
        }
        pub fn role(&self) -> FormatReceiptRole {
            unimplemented!()
        }
        pub fn terminal(&self) -> FormatReceiptTerminal {
            unimplemented!()
        }
        pub fn stage(&self) -> FormatReceiptStage {
            unimplemented!()
        }
        pub fn counters(&self) -> &FormatReceiptCounters {
            unimplemented!()
        }
        pub fn pack_exposed(&self) -> bool {
            unimplemented!()
        }
        pub fn stable_value_completed(&self) -> bool {
            unimplemented!()
        }
        pub fn timing(&self) -> FormatTimingStatus {
            unimplemented!()
        }
        pub fn adapter_class(&self) -> &str {
            unimplemented!()
        }
        pub fn admission(&self) -> RepresentationAdmissionDispositionView<'_> {
            unimplemented!()
        }
        pub fn publication(&self) -> FormatPublicationStatus {
            unimplemented!()
        }
        pub fn cleanup(&self) -> FormatCleanupStatus {
            unimplemented!()
        }
        pub fn failure_class(&self) -> FormatFailureClass {
            unimplemented!()
        }
        pub fn failure_cause(&self) -> Option<&FormatFailureCauseCode> {
            unimplemented!()
        }
        pub fn validation_rules(&self) -> impl ExactSizeIterator<Item = ValidationRuleCode> + '_ {
            std::iter::empty()
        }
    }

    pub struct FormatReceiptControlsView<'a> {
        pub trust: crate::DeploymentTrustProfile,
        pub network: crate::OperationNetworkPolicy,
        pub resource_profile: &'a crate::ResourceProfileIdentity,
        pub deadline: &'a OperationDeadline,
        pub cancellation_present: bool,
        pub interruption: crate::OperationInterruptionStrength,
        pub publication_strength: Option<crate::transport::PublicationCommitStrength>,
        pub cleanup_strength: Option<crate::transport::TransportCleanupRequirement>,
        pub limits: FormatReceiptLimitsView<'a>,
        pub enforcement: &'a [crate::EnforcementClaim],
        pub timing_requested: bool,
        pub timing_reporting: bool,
    }

    pub enum FormatReceiptLimitsView<'a> {
        PackIngress(&'a PackIngressResourceLimits),
        Representation(&'a RepresentationResourceLimits),
        Transport(&'a crate::transport::TransportResourceLimits),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatPublicationStatus {
        NotApplicable,
        NotStarted {
            requested: crate::transport::PublicationCommitStrength,
            admitted: crate::transport::PublicationCommitStrength,
        },
        Committed {
            requested: crate::transport::PublicationCommitStrength,
            admitted: crate::transport::PublicationCommitStrength,
            actual: crate::transport::PublicationCommitStrength,
        },
        Failed {
            requested: crate::transport::PublicationCommitStrength,
            admitted: crate::transport::PublicationCommitStrength,
            actual: Option<crate::transport::PublicationCommitStrength>,
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum FormatCleanupStatus {
        NotApplicable,
        NotReached {
            requested: crate::transport::TransportCleanupRequirement,
            admitted: crate::transport::TransportCleanupRequirement,
        },
        Reached {
            requested: crate::transport::TransportCleanupRequirement,
            admitted: crate::transport::TransportCleanupRequirement,
            outcome: crate::transport::TransportCleanupOutcome,
        },
    }

    #[derive(Clone, Debug)]
    pub struct FormatReceiptFile {
        pub path: ClosureExportPath,
        pub exact_bytes: u64,
        pub content_identity: ContentIdentity,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatFailureClass {
        NotApplicable,
        ReferenceResolution,
        Acquisition,
        Spooling,
        Transfer,
        Commit,
        Cleanup,
        AdapterContract,
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FormatEnforcementFactIdentifier(Arc<str>);

    impl FormatEnforcementFactIdentifier {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct FormatFailureCauseCode(Arc<str>);

    impl FormatFailureCauseCode {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    macro_rules! format_receipt {
        ($name:ident) => {
            #[derive(Clone, Debug)]
            pub struct $name {
                private: crate::private::FormatReceiptState,
            }

            impl $name {
                pub fn common(&self) -> FormatReceiptCommonView<'_> {
                    unimplemented!()
                }
            }
        };
    }

    format_receipt!(PackArchiveEncodingFormatReceipt);
    format_receipt!(PackArchiveReadFormatReceipt);
    format_receipt!(ClosureExportProjectionFormatReceipt);
    format_receipt!(ClosureExportImportFormatReceipt);
    format_receipt!(ProjectMaterializationProjectionReceipt);
    format_receipt!(PackArchivePublicationFormatReceipt);
    format_receipt!(ClosureExportPublicationFormatReceipt);

    impl PackArchiveEncodingFormatReceipt {
        pub fn control_record_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn source_pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn archive_encoding_identity(&self) -> &ArchiveEncodingIdentity {
            unimplemented!()
        }
        pub fn output_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(&self) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
    }

    impl PackArchiveReadFormatReceipt {
        pub fn input_archive_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }
        pub fn expected_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn expected_archive_matched(&self) -> Option<bool> {
            unimplemented!()
        }
        pub fn control_record_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn derived_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn expected_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn expected_pack_matched(&self) -> Option<bool> {
            unimplemented!()
        }
        pub fn verification_mode(&self) -> FormatVerificationMode {
            unimplemented!()
        }
        pub fn asserted_archive_encoding_identity(&self) -> Option<&ArchiveEncodingIdentity> {
            unimplemented!()
        }
        pub fn encoding_assertion(&self) -> ArchiveEncodingAssertionStatus {
            unimplemented!()
        }
    }

    impl ClosureExportProjectionFormatReceipt {
        pub fn control_record_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn source_pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(&self) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn files(&self) -> &[FormatReceiptFile] {
            unimplemented!()
        }
    }

    impl ClosureExportImportFormatReceipt {
        pub fn control_record_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn derived_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn expected_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn expected_pack_matched(&self) -> Option<bool> {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(&self) -> &ClosureExportTreeContentIdentity {
            unimplemented!()
        }
        pub fn verification_mode(&self) -> FormatVerificationMode {
            unimplemented!()
        }
        pub fn files(&self) -> &[FormatReceiptFile] {
            unimplemented!()
        }
    }

    impl PackArchivePublicationFormatReceipt {
        pub fn source_archive_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }
        pub fn output_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn archive_encoding_identity(&self) -> &ArchiveEncodingIdentity {
            unimplemented!()
        }
    }

    impl ClosureExportPublicationFormatReceipt {
        pub fn source_pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn source_tree_identity(&self) -> &ClosureExportTreeContentIdentity {
            unimplemented!()
        }
        pub fn output_tree_identity(&self) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn files(&self) -> &[FormatReceiptFile] {
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

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ArchiveEncodingIdentity {
        private: crate::private::ArchiveEncodingIdentityState,
    }

    impl ArchiveEncodingIdentity {
        pub fn parse(
            _admission: &OrdinaryAdmission,
            _value: &str,
        ) -> Result<Self, crate::DomainValueRejection> {
            unimplemented!()
        }

        pub fn epoch_2_all_stored_v1() -> Self {
            unimplemented!()
        }

        pub fn as_str(&self) -> &str {
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

        pub fn output_bytes(&self) -> u64 {
            self.output_bytes
        }
        pub fn files(&self) -> u64 {
            self.files
        }
        pub fn stable_spool_bytes(&self) -> u64 {
            self.stable_spool_bytes
        }
        pub fn retained_memory_bytes(&self) -> u64 {
            self.retained_memory_bytes
        }
    }

    pub struct RepresentationControls<'a> {
        admission: OrdinaryAdmission,
        limits: AdmittedOperationResourceLimits<RepresentationResourceLimits>,
        request: RepresentationOperationRequest,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
    }

    impl<'a> RepresentationControls<'a> {
        pub fn try_new(
            admission: OrdinaryAdmission,
            limits: AdmittedOperationResourceLimits<RepresentationResourceLimits>,
            request: RepresentationOperationRequest,
            clock: &'a dyn crate::MonotonicClock,
            interruption: &'a dyn crate::InterruptionSource,
        ) -> Result<Self, crate::AdmissionRefusal> {
            if let OperationDeadline::At(instant) = &request.deadline {
                if instant.domain() != clock.domain() {
                    return Err(crate::AdmissionRefusal::MissingEnforcementCapability);
                }
            }
            Ok(Self {
                admission,
                limits,
                request,
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
        private: crate::private::EncodedPackArchiveState,
    }

    impl EncodedPackArchive {
        pub fn bytes(&self) -> &StableByteValue {
            unimplemented!()
        }
        pub fn pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn encoding_identity(&self) -> &ArchiveEncodingIdentity {
            unimplemented!()
        }
        pub fn archive_content_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(&self) -> &ClosureExportTreeContentIdentity {
            unimplemented!()
        }
    }

    pub struct PackArchiveEncodingReport {
        terminal: Result<EncodedPackArchive, RepresentationFailure>,
        receipt: PackArchiveEncodingFormatReceipt,
        spool_receipt: Option<crate::transport::SpoolTransportReceipt>,
    }

    impl PackArchiveEncodingReport {
        pub fn terminal(&self) -> Result<&EncodedPackArchive, &RepresentationFailure> {
            self.terminal.as_ref()
        }
        pub fn receipt(&self) -> &PackArchiveEncodingFormatReceipt {
            &self.receipt
        }
        pub fn spool_receipt(&self) -> Option<&crate::transport::SpoolTransportReceipt> {
            self.spool_receipt.as_ref()
        }
    }

    pub struct ProjectMaterializationPlan {
        private: crate::private::ProjectMaterializationPlanState,
    }

    pub struct ClosureExportPlan {
        private: crate::private::ClosureExportPlanState,
    }

    pub struct ProjectMaterializationReport {
        terminal: Result<ProjectMaterializationPlan, RepresentationFailure>,
        receipt: ProjectMaterializationProjectionReceipt,
    }

    impl ProjectMaterializationReport {
        pub fn terminal(&self) -> Result<&ProjectMaterializationPlan, &RepresentationFailure> {
            self.terminal.as_ref()
        }
        pub fn receipt(&self) -> &ProjectMaterializationProjectionReceipt {
            &self.receipt
        }
    }

    pub struct ClosureExportProjectionReport {
        terminal: Result<ClosureExportPlan, RepresentationFailure>,
        receipt: ClosureExportProjectionFormatReceipt,
    }

    impl ClosureExportProjectionReport {
        pub fn terminal(&self) -> Result<&ClosureExportPlan, &RepresentationFailure> {
            self.terminal.as_ref()
        }
        pub fn receipt(&self) -> &ClosureExportProjectionFormatReceipt {
            &self.receipt
        }
    }

    impl ProjectMaterializationProjectionReceipt {
        pub fn pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn file_count(&self) -> u64 {
            unimplemented!()
        }
        pub fn aggregate_bytes(&self) -> u64 {
            unimplemented!()
        }
        pub fn files(
            &self,
        ) -> impl ExactSizeIterator<Item = ProjectMaterializationFileReceiptView<'_>> {
            std::iter::empty()
        }
    }

    pub struct ProjectMaterializationFileReceiptView<'a> {
        pub path: &'a ProjectPath,
        pub exact_bytes: u64,
        pub content_identity: &'a ContentIdentity,
    }

    pub fn plan_project_materialization(
        _pack: &Pack,
        _controls: RepresentationControls<'_>,
    ) -> ProjectMaterializationReport {
        unimplemented!()
    }

    pub fn plan_closure_export(
        _pack: &Pack,
        _controls: RepresentationControls<'_>,
    ) -> ClosureExportProjectionReport {
        unimplemented!()
    }

    impl ProjectMaterializationPlan {
        pub fn source_pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn files(&self) -> impl ExactSizeIterator<Item = ProjectMaterializationFile<'_>> {
            std::iter::empty()
        }
    }

    impl ClosureExportPlan {
        pub fn source_pack_identity(&self) -> &PackIdentity {
            unimplemented!()
        }
        pub fn control_record_identity(&self) -> &ContentIdentity {
            unimplemented!()
        }
        pub fn tree_content_identity(&self) -> &ClosureExportTreeContentIdentity {
            unimplemented!()
        }
        pub fn aggregate_bytes(&self) -> u64 {
            unimplemented!()
        }
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
        Blob,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RepresentationFailure {
        Admission(RepresentationAdmissionRefusal),
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
    use super::{CompilationReport, Pack};

    pub struct CompilationSession {
        state: crate::private::SessionState,
    }

    impl CompilationSession {
        pub fn new(_admission: crate::OrdinaryAdmission, _pack: Pack) -> Self {
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

    #[derive(Clone, Debug)]
    pub struct SessionPreparationLimits {
        private: crate::private::SessionPreparationLimitsState,
    }

    impl SessionPreparationLimits {
        pub fn try_caller_selected(
            _limits: crate::compilation::CompilationResourceLimits,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }

        pub fn try_from_adapter_profile(
            _profile: crate::ResourceProfileIdentity,
            _requested: crate::compilation::CompilationResourceLimits,
            _admitted: crate::compilation::CompilationResourceLimits,
        ) -> Result<Self, crate::AdmissionRefusal> {
            unimplemented!()
        }

        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }

        pub fn requested(&self) -> &crate::compilation::CompilationResourceLimits {
            unimplemented!()
        }

        pub fn admitted(&self) -> &crate::compilation::CompilationResourceLimits {
            unimplemented!()
        }
    }

    impl SessionPolicy {
        pub fn latest_only_complete_coverage(_preparation: SessionPreparationLimits) -> Self {
            unimplemented!()
        }

        pub fn latest_only_allow_unverified(_preparation: SessionPreparationLimits) -> Self {
            unimplemented!()
        }

        pub fn preparation_limits(&self) -> &SessionPreparationLimits {
            unimplemented!()
        }
        pub fn mode(&self) -> SessionPolicyMode {
            unimplemented!()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionPolicyMode {
        LatestOnlyCompleteCoverage,
        LatestOnlyAllowUnverified,
    }

    #[derive(Clone, Debug)]
    pub struct StabilizedSessionInput {
        private: crate::private::StabilizedSessionInputState,
    }

    impl StabilizedSessionInput {
        pub fn try_new(
            _request: crate::compilation::CompilationRequest,
            _evidence: SessionRequestEvidence,
            _policy: SessionPolicy,
        ) -> Result<Self, SessionInputRejection> {
            unimplemented!()
        }

        pub fn policy(&self) -> &SessionPolicy {
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
            _sources: impl IntoIterator<Item = RequestSourceEvidenceBinding>,
        ) -> Result<Self, SessionInputRejection> {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct RequestSourceEvidenceBinding {
        private: crate::private::RequestSourceEvidenceBindingState,
    }

    impl RequestSourceEvidenceBinding {
        pub fn try_new(_scope: EvidenceScope, _provider: super::AuthorityInstanceIdentity) -> Self {
            unimplemented!()
        }
        pub fn scope(&self) -> &EvidenceScope {
            unimplemented!()
        }
        pub fn provider(&self) -> &super::AuthorityInstanceIdentity {
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
    pub struct SessionInstanceIdentity(crate::private::SessionInstanceIdentityState);

    impl SessionInstanceIdentity {
        pub fn as_str(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SessionRevision(crate::private::SessionRevisionState);

    impl SessionRevision {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
        pub fn policy(&self) -> &SessionPolicy {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SessionEvaluation(crate::private::SessionEvaluationState);

    impl SessionEvaluation {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn revision(&self) -> &SessionRevision {
            unimplemented!()
        }
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SessionAttemptToken {
        private: crate::private::SessionAttemptTokenState,
    }

    impl SessionAttemptToken {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn evaluation(&self) -> &SessionEvaluation {
            unimplemented!()
        }
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SessionFenceToken {
        private: crate::private::SessionFenceTokenState,
    }

    impl SessionFenceToken {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn evaluation(&self) -> &SessionEvaluation {
            unimplemented!()
        }
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SubscriptionGeneration {
        private: crate::private::SubscriptionGenerationState,
    }

    impl SubscriptionGeneration {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SessionPublicationSequence(crate::private::SessionPublicationSequenceState);

    impl SessionPublicationSequence {
        pub fn ordinal(&self) -> u64 {
            unimplemented!()
        }
    }

    pub enum AcceptedSessionInput {
        Stabilized(StabilizedSessionInput),
        IngestionFailure(SessionIngestionFailure),
    }

    pub enum SessionEvent {
        Accept(AcceptedSessionInput),
        DependencyChanged {
            generation: SubscriptionGeneration,
            change: DependencyChangeNotification,
        },
        NotificationGap {
            generation: SubscriptionGeneration,
            scope: SessionWatchScope,
        },
        Refresh,
        Retry,
        AttemptFinished {
            token: SessionAttemptToken,
            report: CompilationReport,
        },
        AttemptReleased {
            token: SessionAttemptToken,
            release: SessionAttemptRelease,
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionAttemptRelease {
        Reaped,
        AbandonedNoLiveResource,
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
        private: crate::private::SessionAttemptPlanState,
    }

    #[derive(Clone)]
    pub struct SessionSupersessionPermit {
        private: crate::private::SessionSupersessionPermitState,
    }

    impl SessionSupersessionPermit {
        pub fn is_revoked(&self) -> bool {
            unimplemented!()
        }
    }

    impl SessionAttemptPlan {
        pub fn revision(&self) -> &SessionRevision {
            unimplemented!()
        }
        pub fn evaluation(&self) -> &SessionEvaluation {
            unimplemented!()
        }
        pub fn policy(&self) -> &SessionPolicy {
            unimplemented!()
        }
        pub fn prepared_identity(&self) -> &crate::CompilationIdentity {
            unimplemented!()
        }
        pub fn supersession_permit(&self) -> &SessionSupersessionPermit {
            unimplemented!()
        }

        pub fn run_sync<P: ?Sized, F: ?Sized, C: ?Sized>(
            self,
            _controls: crate::compilation::SyncCompilationControls<'_, P, F, C>,
        ) -> Result<CompilationReport, crate::compilation::CompilationAdmissionRefusal>
        where
            P: crate::authority::SyncPackageAuthority,
            F: crate::authority::SyncFontAuthority,
            C: crate::compilation::SyncSemanticResultCache,
        {
            unimplemented!()
        }

        pub async fn run_async<P: ?Sized, F: ?Sized, C: ?Sized, X: ?Sized>(
            self,
            _controls: crate::compilation::AsyncCompilationControls<'_, P, F, C, X>,
        ) -> Result<CompilationReport, crate::compilation::CompilationAdmissionRefusal>
        where
            P: crate::authority::AsyncPackageAuthority,
            F: crate::authority::AsyncFontAuthority,
            C: crate::compilation::AsyncSemanticResultCache,
            X: crate::compilation::CompilationExecutionFacility,
        {
            unimplemented!()
        }
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
        Retiring,
        Retired,
    }

    #[derive(Clone, Debug)]
    pub struct CurrentnessFenceReadPlan {
        private: crate::private::CurrentnessFenceReadPlanState,
    }

    impl CurrentnessFenceReadPlan {
        pub fn request_sources(
            &self,
        ) -> impl ExactSizeIterator<Item = &RequestSourceEvidenceBinding> {
            std::iter::empty()
        }
        pub fn dependency_interests(
            &self,
        ) -> impl ExactSizeIterator<Item = DependencyRevalidationInterest<'_>> {
            std::iter::empty()
        }
        pub fn historical_evidence(&self) -> Option<&DependencyResolutionEvidence> {
            unimplemented!()
        }
        pub fn request_source_cursor(
            &self,
            _source: &RequestSourceEvidenceBinding,
            _opaque: std::sync::Arc<[u8]>,
        ) -> Result<ProviderCursor, SessionAdapterValueRejection> {
            unimplemented!()
        }
    }

    pub struct DependencyRevalidationInterest<'a> {
        pub provider: &'a super::AuthorityInstanceIdentity,
        pub key: &'a DependencyEvidenceKey,
    }

    #[derive(Clone, Debug)]
    pub struct SessionAffectedScopes {
        private: crate::private::SessionAffectedScopesState,
    }

    impl SessionAffectedScopes {
        pub fn try_new(
            _request_sources: impl IntoIterator<Item = EvidenceScope>,
            _dependencies: impl IntoIterator<Item = EvidenceScope>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }

        pub fn request_sources(&self) -> impl ExactSizeIterator<Item = &EvidenceScope> {
            std::iter::empty()
        }

        pub fn dependencies(&self) -> impl ExactSizeIterator<Item = &EvidenceScope> {
            std::iter::empty()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SessionFenceFailure {
        private: crate::private::SessionFenceFailureState,
    }

    impl SessionFenceFailure {
        pub fn try_new(
            _scopes: SessionAffectedScopes,
            _safe_code: &str,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }

        pub fn scopes(&self) -> &SessionAffectedScopes {
            unimplemented!()
        }

        pub fn safe_code(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct SessionProviderObservations {
        private: crate::private::SessionProviderObservationsState,
    }

    impl SessionProviderObservations {
        pub fn try_new(
            _cursors: impl IntoIterator<Item = ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }

        pub fn cursors(&self) -> impl ExactSizeIterator<Item = RoutedProviderCursorView<'_>> {
            std::iter::empty()
        }
    }

    pub enum FenceReadOutcome {
        Read(FenceReadObservation),
        Changed(SessionAffectedScopes),
        Incomplete(SessionWatchCoverage),
        Failed(SessionFenceFailure),
    }

    #[derive(Clone, Debug)]
    pub struct FenceReadObservation {
        private: crate::private::FenceReadObservationState,
    }

    impl FenceReadObservation {
        pub fn try_new(
            _plan: &CurrentnessFenceReadPlan,
            _request_sources: impl IntoIterator<Item = RequestSourceObservation>,
            _evidence: Option<DependencyResolutionEvidence>,
            _provider_observations: SessionProviderObservations,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }

        pub fn dependency_evidence(&self) -> Option<&DependencyResolutionEvidence> {
            unimplemented!()
        }
        pub fn request_sources(&self) -> impl ExactSizeIterator<Item = &RequestSourceObservation> {
            std::iter::empty()
        }
        pub fn provider_observations(&self) -> &SessionProviderObservations {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub struct RequestSourceObservation {
        private: crate::private::RequestSourceObservationState,
    }

    impl RequestSourceObservation {
        pub fn try_new(
            _plan: &CurrentnessFenceReadPlan,
            _source: &RequestSourceEvidenceBinding,
            _identity: crate::CanonicalIdentity,
            _cursor: Option<ProviderCursor>,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
        pub fn scope(&self) -> &EvidenceScope {
            unimplemented!()
        }
        pub fn identity(&self) -> &crate::CanonicalIdentity {
            unimplemented!()
        }
        pub fn cursor(&self) -> Option<&ProviderCursor> {
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
        pub provider: &'a super::AuthorityInstanceIdentity,
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
        pub fn coverage(&self) -> &SessionWatchCoverage {
            unimplemented!()
        }
        pub fn cursors(&self) -> impl ExactSizeIterator<Item = RoutedProviderCursorView<'_>> {
            std::iter::empty()
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
        pub fn request_sources(
            &self,
        ) -> impl ExactSizeIterator<Item = &RequestSourceEvidenceBinding> {
            std::iter::empty()
        }
        pub fn dependency_interests(
            &self,
        ) -> impl ExactSizeIterator<Item = DependencyRevalidationInterest<'_>> {
            std::iter::empty()
        }
        pub fn armed_cursors(&self) -> impl ExactSizeIterator<Item = RoutedProviderCursorView<'_>> {
            std::iter::empty()
        }
    }

    pub struct RoutedProviderCursorView<'a> {
        pub provider: &'a super::AuthorityInstanceIdentity,
        pub cursor: &'a ProviderCursor,
    }

    pub enum FenceConfirmationOutcome {
        Clean(FenceConfirmation),
        Dirty(SessionAffectedScopes),
        Incomplete(SessionWatchCoverage),
        Failed(SessionFenceFailure),
    }

    #[derive(Clone, Debug)]
    pub struct FenceConfirmation {
        private: crate::private::FenceConfirmationState,
    }

    impl FenceConfirmation {
        pub fn try_new(
            _plan: &CurrentnessFenceConfirmationPlan,
            _observations: SessionProviderObservations,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
        pub fn observations(&self) -> &SessionProviderObservations {
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

        pub fn incomplete(_scopes: SessionAffectedScopes) -> Self {
            unimplemented!()
        }
        pub fn view(&self) -> SessionWatchCoverageView<'_> {
            unimplemented!()
        }
    }

    pub enum SessionWatchCoverageView<'a> {
        CompletePush,
        CompletePoll,
        Incomplete(&'a SessionAffectedScopes),
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

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct EvidenceScope {
        private: crate::private::EvidenceScopeState,
    }

    impl EvidenceScope {
        pub fn try_new(_namespaced_value: &str) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }
        pub fn as_str(&self) -> &str {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub enum SessionWatchScope {
        RequestSource(EvidenceScope),
        Dependency(EvidenceScope),
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
        private: crate::private::SessionPublicationState,
    }

    pub enum SessionPublicationTerminalRef<'a> {
        RequestRejected(&'a crate::compilation::CompilationRequestRejection),
        Report(&'a CompilationReport),
        IngestionFailure(&'a SessionIngestionFailure),
    }

    impl SessionPublication {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn sequence(&self) -> &SessionPublicationSequence {
            unimplemented!()
        }
        pub fn revision(&self) -> &SessionRevision {
            unimplemented!()
        }
        pub fn evaluation(&self) -> &SessionEvaluation {
            unimplemented!()
        }
        pub fn terminal(&self) -> SessionPublicationTerminalRef<'_> {
            unimplemented!()
        }
        pub fn currentness(&self) -> &SessionCurrentness {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug)]
    pub enum SessionCurrentness {
        CurrentThroughPush {
            observations: SessionProviderObservations,
        },
        CurrentAsOfPoll {
            observations: SessionProviderObservations,
        },
        Unverified {
            uncovered: SessionAffectedScopes,
        },
        Stale {
            dirty: SessionAffectedScopes,
        },
    }

    #[derive(Clone, Debug)]
    pub struct SessionIngestionFailure {
        private: crate::private::SessionIngestionFailureState,
    }

    impl SessionIngestionFailure {
        pub fn try_new(
            _safe_code: &str,
            _failed_request_sources: impl IntoIterator<Item = EvidenceScope>,
            _policy: SessionPolicy,
        ) -> Result<Self, SessionAdapterValueRejection> {
            unimplemented!()
        }

        pub fn safe_code(&self) -> &str {
            unimplemented!()
        }
        pub fn failed_request_sources(&self) -> impl ExactSizeIterator<Item = &EvidenceScope> {
            std::iter::empty()
        }
        pub fn policy(&self) -> &SessionPolicy {
            unimplemented!()
        }
    }

    pub struct SessionView<'a> {
        private: &'a crate::private::SessionState,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionLifecycle {
        Running,
        Retiring,
        Retired,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SessionActiveAttemptState {
        Active,
        Draining,
    }

    pub struct SessionActiveAttemptView<'a> {
        pub token: &'a SessionAttemptToken,
        pub revision: &'a SessionRevision,
        pub evaluation: &'a SessionEvaluation,
        pub state: SessionActiveAttemptState,
    }

    pub struct SessionPendingRevisionView<'a> {
        pub revision: &'a SessionRevision,
        pub evaluation: &'a SessionEvaluation,
        pub prepared_identity: &'a crate::CompilationIdentity,
    }

    impl SessionView<'_> {
        pub fn session_instance(&self) -> &SessionInstanceIdentity {
            unimplemented!()
        }
        pub fn lifecycle(&self) -> SessionLifecycle {
            unimplemented!()
        }
        pub fn latest_revision(&self) -> Option<&SessionRevision> {
            unimplemented!()
        }
        pub fn latest_evaluation(&self) -> Option<&SessionEvaluation> {
            unimplemented!()
        }
        pub fn active_attempt(&self) -> Option<SessionActiveAttemptView<'_>> {
            unimplemented!()
        }
        pub fn pending_revision(&self) -> Option<SessionPendingRevisionView<'_>> {
            unimplemented!()
        }
        pub fn publication(&self) -> Option<&SessionPublication> {
            unimplemented!()
        }
        pub fn last_successful(&self) -> Option<LastSuccessfulCompilationView<'_>> {
            unimplemented!()
        }
    }

    pub struct LastSuccessfulCompilationView<'a> {
        pub revision: &'a SessionRevision,
        pub evaluation: &'a SessionEvaluation,
        pub publication_sequence: &'a SessionPublicationSequence,
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
        PackMetadataState,
        PackAnnotationIdentifierState,
        PackAnnotationState,
        PackSemanticExtensionIdentifierState,
        ProjectSnapshotState,
        DiscoveryVariantState,
        DiscoveryOverrideSetState,
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
        FontCatalogCandidateState,
        FontCatalogAcquisitionState,
        FontScanDiagnosticState,
        FontCatalogRequestState,
        FontContainerAcquisitionRequestState,
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
        CompilationRequestInventoryState,
        CompilationOutputInventoryState,
        CompilationReportState,
        CompilationResultState,
        CompilationAccessTraceState,
        CompilationResourceLimitsState,
        CacheBudgetState,
        CacheReservationState,
        ReadyCompilationJobState,
        CompilationWorkerRequestState,
        CompilationWorkerResponseVerifierState,
        CompilationJobCompletionState,
        CompilationDisclosureState,
        DiagnosticDisclosureCapabilityState,
        CanonicalDiagnosticsDisclosureCapabilityState,
        CanonicalEvidenceDisclosureCapabilityState,
        DiagnosticSourcesDisclosureCapabilityState,
        RequestValuesDisclosureCapabilityState,
        OverrideBytesDisclosureCapabilityState,
        BackingLocatorsDisclosureCapabilityState,
        AdapterDetailDisclosureCapabilityState,
        CompilationReportProjectionState,
        CompilationRequestRejectionProjectionState,
        CompilationDeliveryPlanState,
        RepresentationReceiptState,
        FormatReceiptState,
        ArchiveEncodingIdentityState,
        EncodedPackArchiveState,
        PackIngressResourceLimitsState,
        ProjectMaterializationPlanState,
        ProjectMaterializationProjectionReceiptState,
        ClosureExportPlanState,
        TransportReceiptState,
        SessionState,
        SessionPolicyState,
        StabilizedSessionInputState,
        SessionRequestEvidenceState,
        RequestSourceEvidenceBindingState,
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
        PackageAuthorityCapabilityDescriptorState,
        FontAuthorityCapabilityDescriptorState,
        CreationEvidenceCapabilityDescriptorState,
        CreationAdmissionRefusalState,
        CreationAdmissionRecordState,
        CreationExecutionFacilityCapabilityDescriptorState,
        SemanticResultCacheCapabilityDescriptorState,
        CompilationAdmissionRefusalState,
        CompilationAdmissionRecordState,
        EngineRuntimeDomainPolicyDescriptorState,
        CompilationExecutionFacilityCapabilityDescriptorState,
        RepresentationAdmissionRefusalState,
        PackArchiveAcquirerCapabilityDescriptorState,
        PackArchivePublisherCapabilityDescriptorState,
        ProjectMaterializationPublisherCapabilityDescriptorState,
        ClosureExportPublisherCapabilityDescriptorState,
        CompilationDeliveryCapabilityDescriptorState,
        SpoolFacilityCapabilityDescriptorState,
        SessionPreparationLimitsState,
        SessionInstanceIdentityState,
        SessionEvaluationState,
        SessionPublicationSequenceState,
        SessionAttemptPlanState,
        SessionSupersessionPermitState,
        SessionAffectedScopesState,
        CurrentnessFenceReadPlanState,
        SessionFenceFailureState,
        SessionProviderObservationsState,
        SessionPublicationState,
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
