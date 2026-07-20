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
    pub fn try_caller_selected(
        limits: L,
    ) -> Result<Self, AdmissionRefusal> {
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
            _timing: TransportTimingInput,
        ) -> Result<Self, TransportReceiptRejection> {
            unimplemented!()
        }

        pub fn try_new_pack_archive_publication(
            _controls: &TransportControls<'_>,
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

        pub fn try_new_project_materialization_publication(
            _controls: &TransportControls<'_>,
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

        pub fn try_new_closure_export_publication(
            _controls: &TransportControls<'_>,
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

        pub fn try_new_compilation_delivery(
            _controls: &TransportControls<'_>,
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

        pub fn try_new_pack_archive_acquisition(
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
        pub fn identities(
            &self,
        ) -> impl ExactSizeIterator<Item = TransportIdentityRef<'_>> {
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
        Complete,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportRole {
        Spool,
        PackArchiveAcquisition,
        PackArchivePublication,
        ProjectMaterializationPublication,
        ClosureExportPublication,
        CompilationDelivery,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TransportStatus {
        NotAttempted,
        Refused,
        Transferred,
        Committed,
        Failed,
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

    pub enum TransportIdentityRef<'a> {
        Content(&'a ContentIdentity),
        Pack(&'a crate::pack::PackIdentity),
        ArchiveEncoding(&'a crate::representation::ArchiveEncodingIdentity),
        ClosureExportTree(&'a crate::ClosureExportTreeContentIdentity),
        Compilation(&'a crate::CompilationIdentity),
        Result(&'a crate::CompilationResultIdentity),
        Artifact(&'a crate::CompilationArtifactIdentity),
    }

    pub enum TransportReceiptSubjectRef<'a> {
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
            transfer: crate::compilation::CompilationDeliveryTransfer<'_>,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> TransportOutcome<()>;
    }

    pub trait AsyncCompilationDelivery {
        type Destination: ?Sized;
        type Deliver<'a>: Future<Output = TransportOutcome<()>> + 'a
        where
            Self: 'a;

        fn deliver<'a>(
            &'a self,
            transfer: crate::compilation::CompilationDeliveryTransfer<'a>,
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
        pub(crate) fn try_new(
            _plan: crate::compilation::CompilationDeliveryPlan,
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

    pub fn deliver_compilation_sync<D: SyncCompilationDelivery + ?Sized>(
        _plan: crate::compilation::CompilationDeliveryPlan,
        _delivery: &D,
        _destination: &D::Destination,
        _controls: TransportControls<'_>,
    ) -> CompilationDeliveryOutcome {
        unimplemented!()
    }

    pub async fn deliver_compilation_async<D: AsyncCompilationDelivery + ?Sized>(
        _plan: crate::compilation::CompilationDeliveryPlan,
        _delivery: &D,
        _destination: &D::Destination,
        _controls: TransportControls<'_>,
    ) -> CompilationDeliveryOutcome {
        unimplemented!()
    }

    pub struct PackArchivePublicationOutcome {
        format: crate::representation::PackArchivePublicationFormatReceipt,
        transport: TransportOutcome<()>,
    }

    impl PackArchivePublicationOutcome {
        pub fn try_new(
            _archive: &crate::representation::EncodedPackArchive,
            _transport: TransportOutcome<()>,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }
        pub fn format(&self) -> &crate::representation::PackArchivePublicationFormatReceipt {
            &self.format
        }
        pub fn transport(&self) -> &TransportOutcome<()> {
            &self.transport
        }
    }

    pub struct ClosureExportPublicationOutcome {
        format: crate::representation::ClosureExportPublicationFormatReceipt,
        transport: TransportOutcome<()>,
    }

    impl ClosureExportPublicationOutcome {
        pub fn try_new(
            _plan: &crate::representation::ClosureExportPlan,
            _transport: TransportOutcome<()>,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
        }
        pub fn format(&self) -> &crate::representation::ClosureExportPublicationFormatReceipt {
            &self.format
        }
        pub fn transport(&self) -> &TransportOutcome<()> {
            &self.transport
        }
    }

    pub struct ProjectMaterializationPublicationOutcome {
        transport: TransportOutcome<()>,
    }

    impl ProjectMaterializationPublicationOutcome {
        pub fn try_new(
            _plan: &crate::representation::ProjectMaterializationPlan,
            _transport: TransportOutcome<()>,
        ) -> Result<Self, TransportOutcomeRejection> {
            unimplemented!()
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
        ) -> PackArchivePublicationOutcome;
    }

    pub trait SyncProjectMaterializationPublisher {
        type Destination: ?Sized;

        fn publish(
            &self,
            plan: &crate::representation::ProjectMaterializationPlan,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> ProjectMaterializationPublicationOutcome;
    }

    pub trait SyncClosureExportPublisher {
        type Destination: ?Sized;

        fn publish(
            &self,
            plan: &crate::representation::ClosureExportPlan,
            destination: &Self::Destination,
            controls: TransportControls<'_>,
        ) -> ClosureExportPublicationOutcome;
    }

    pub trait AsyncPackArchivePublisher {
        type Destination: ?Sized;
        type Publish<'a>: Future<Output = PackArchivePublicationOutcome> + 'a
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
        type Publish<'a>: Future<Output = ProjectMaterializationPublicationOutcome> + 'a
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
        type Publish<'a>: Future<Output = ClosureExportPublicationOutcome> + 'a
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
    use super::{
        ContentIdentity, DiscoveryCoverageIdentity, DiscoveryRequestCommitment,
        DiscoveryTraceIdentity, DiscoveryVariantIdentity, DomainValueRejection,
        FontRequirementIdentity, OrdinaryAdmission,
        PackageRequirementIdentity, ProjectPath, ProjectTreeIdentity,
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
            _title: Option<String>,
            _description: Option<String>,
            _authors: impl IntoIterator<Item = String>,
            _keywords: impl IntoIterator<Item = String>,
        ) -> Result<Self, PackMetadataRejection> {
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

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PackMetadataRejection {
        EmptyField,
        DuplicateKeyword,
        FormatCeilingExceeded,
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct PackAnnotationIdentifier(crate::private::PackAnnotationIdentifierState);

    impl PackAnnotationIdentifier {
        pub fn parse(
            _admission: &OrdinaryAdmission,
            _value: &str,
        ) -> Result<Self, PackAnnotationRejection> {
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
            _identifier: PackAnnotationIdentifier,
            _epoch: NonZeroU32,
            _payload: Arc<[u8]>,
        ) -> Result<Self, PackAnnotationRejection> {
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

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum PackAnnotationRejection {
        InvalidIdentifier,
        WrongIdentifierClass,
        DuplicateIdentifier,
        FormatCeilingExceeded,
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
            _limits: &crate::compilation::CompilationResourceLimits,
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
            _authority: AuthorityInstanceIdentity,
            _candidates: impl IntoIterator<Item = FontCatalogCandidate>,
        ) -> Result<Self, FontCatalogRejection> {
            unimplemented!()
        }

        pub fn authority(&self) -> &AuthorityInstanceIdentity {
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
        pub fn try_new(
            authority: AuthorityInstanceIdentity,
            opaque: Arc<[u8]>,
        ) -> Result<Self, FontCatalogRejection> {
            if opaque.is_empty() {
                return Err(FontCatalogRejection::InvalidAcquisitionIdentity);
            }
            Ok(Self { authority, opaque })
        }

        pub fn authority(&self) -> &AuthorityInstanceIdentity {
            &self.authority
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
            let invalid = first > last
                || last > 0x10ffff
                || (first <= 0xdfff && last >= 0xd800);
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
        pub(crate) fn try_new(
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
        pub dimension: AcquisitionBudgetDimension,
        pub requested: u64,
        pub remaining: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum AcquisitionBudgetDimension {
        DownloadedBytes,
        ExpandedBytes,
        StableSpoolBytes,
        RetainedMemoryBytes,
    }

    pub struct AcquisitionControls<'a> {
        deadline: crate::OperationDeadline,
        clock: &'a dyn crate::MonotonicClock,
        interruption: &'a dyn crate::InterruptionSource,
        budget: &'a AcquisitionBudget,
    }

    impl<'a> AcquisitionControls<'a> {
        pub(crate) fn try_new(
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

    #[derive(Clone, Debug)]
    pub struct DiscoveryOverrideSet {
        private: crate::private::DiscoveryOverrideSetState,
    }

    impl DiscoveryOverrideSet {
        pub fn empty() -> Self {
            unimplemented!()
        }

        pub fn try_new(
            _limits: &CreationResourceLimits,
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
            _limits: &CreationResourceLimits,
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
        largest_project_file_bytes: u64,
        packages: u64,
        package_tree_bytes: u64,
        font_containers: u64,
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
        #[allow(clippy::too_many_arguments)]
        pub fn try_new(
            project_files: u64,
            aggregate_project_bytes: u64,
            largest_project_file_bytes: u64,
            packages: u64,
            package_tree_bytes: u64,
            font_containers: u64,
            font_faces: u64,
            font_bytes: u64,
            discovery_variants: u64,
            discovery_restarts: u64,
            override_count: u64,
            largest_override_bytes: u64,
            aggregate_override_bytes: u64,
            stable_spool_bytes: u64,
            retained_memory_bytes: u64,
        ) -> Result<Self, crate::AdmissionRefusal> {
            let limits = Self {
                project_files,
                aggregate_project_bytes,
                largest_project_file_bytes,
                packages,
                package_tree_bytes,
                font_containers,
                font_faces,
                font_bytes,
                discovery_variants,
                discovery_restarts,
                override_count,
                largest_override_bytes,
                aggregate_override_bytes,
                stable_spool_bytes,
                retained_memory_bytes,
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
                package_tree_bytes: self.package_tree_bytes,
                font_containers: self.font_containers,
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
        pub package_tree_bytes: u64,
        pub font_containers: u64,
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
        reporting: CreationReportingPolicy,
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
            reporting: CreationReportingPolicy,
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
                reporting,
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
        reporting: CreationReportingPolicy,
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
            reporting: CreationReportingPolicy,
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
                reporting,
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
        fn capacity(&self) -> CreationExecutionFacilityCapacity;

        fn dispatch<'a>(&'a self, job: ReadyCreationJob) -> Self::Dispatch<'a>;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CreationExecutionFacilityCapacity {
        pub simultaneous_ready_jobs: NonZeroUsize,
        pub ready_queue: usize,
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
        pub fn requested_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn admitted_trust(&self) -> crate::DeploymentTrustProfile {
            unimplemented!()
        }
        pub fn resource_profile(&self) -> Option<&crate::ResourceProfileIdentity> {
            unimplemented!()
        }
        pub fn requested_limits(&self) -> &CreationResourceLimits {
            unimplemented!()
        }
        pub fn admitted_limits(&self) -> &CreationResourceLimits {
            unimplemented!()
        }
        pub fn acquisition_concurrency(&self) -> NonZeroUsize {
            unimplemented!()
        }
        pub fn execution(&self) -> CreationExecutionInventoryView<'_> {
            unimplemented!()
        }
        pub fn deadline(&self) -> &OperationDeadline {
            unimplemented!()
        }
    }

    pub enum CreationExecutionInventoryView<'a> {
        CallerThread {
            domain: &'a crate::compilation::EngineRuntimeDomainDescriptor,
        },
        Facility {
            domain: &'a crate::compilation::EngineRuntimeDomainDescriptor,
            capacity: CreationExecutionFacilityCapacity,
        },
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
            _limits: &CompilationResourceLimits,
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
        PdfA1A,
        PdfA1B,
        PdfA2A,
        PdfA2B,
        PdfA2U,
        PdfA3A,
        PdfA3B,
        PdfA3U,
        PdfA4,
        PdfA4F,
        PdfA4E,
        PdfUa1,
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
            _limits: &CompilationResourceLimits,
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
        _limits: &CompilationResourceLimits,
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

        pub fn engine_neutral_intent_identity(
            &self,
        ) -> &EngineNeutralCompilationIntentIdentity {
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

        pub fn evidence(&self) -> &DependencyResolutionEvidence {
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

        pub fn attempt_inventory(&self) -> CompilationAttemptInventoryView<'_> {
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
        },
        Output(CompilationOutputInventoryView<'a>),
        Diagnostics {
            policy: &'a CanonicalDiagnosticPolicy,
            origin: CompilationInventoryOrigin,
            status: CompilationRequestInventoryStatus,
        },
        Engine(&'a EngineIdentity),
        Exporter(&'a ExporterIdentity),
        InvalidDeclaration {
            role: CompilationRequestInventoryRole,
            declaration_ordinal: u64,
            issues: &'a [CompilationRequestIssueCode],
        },
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
        pub fn entries(
            &self,
        ) -> impl ExactSizeIterator<Item = CompilationAttemptInventoryEntryView<'_>> {
            std::iter::empty()
        }
    }

    pub enum CompilationAttemptInventoryEntryView<'a> {
        Admission {
            requested: crate::DeploymentTrustProfile,
            admitted: crate::DeploymentTrustProfile,
        },
        Resources {
            profile: Option<&'a crate::ResourceProfileIdentity>,
            requested: &'a CompilationResourceLimits,
            admitted: &'a CompilationResourceLimits,
        },
        DependencyExecution {
            package_authority_class: &'a str,
            font_authority_class: &'a str,
            cache: SemanticCacheProvenance,
            cache_availability: Option<SemanticCacheAvailabilityPolicy>,
            isolation_domain: Option<&'a CacheIsolationDomain>,
            offline: bool,
            acquisition_concurrency: NonZeroUsize,
        },
        AttemptControl {
            deadline: &'a OperationDeadline,
            cancellation_present: bool,
            monotonic_domain: &'a crate::MonotonicTimeDomain,
            interruption: CompilationInterruptionStrength,
        },
        KernelExecution(CompilationExecutionInventoryView<'a>),
        Reporting(CompilationReportingInventoryView<'a>),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilationInterruptionStrength {
        Cooperative,
        Isolated,
    }

    pub enum CompilationExecutionInventoryView<'a> {
        CallerThread {
            domain: &'a EngineRuntimeDomainDescriptor,
        },
        Facility {
            domain: &'a EngineRuntimeDomainDescriptor,
            capacity: CompilationExecutionFacilityCapacity,
            isolated_worker_capacity: Option<NonZeroUsize>,
        },
    }

    pub struct CompilationReportingInventoryView<'a> {
        pub requested: &'a CompilationReportingPolicy,
        pub diagnostic_projection: ReportingChannelStatus,
        pub diagnostic_sources: ReportingChannelStatus,
        pub timing: ReportingChannelStatus,
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
        interruption: Option<&'a dyn crate::InterruptionSource>,
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
            interruption: Option<&'a dyn crate::InterruptionSource>,
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
        interruption: Option<&'a dyn crate::InterruptionSource>,
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
            interruption: Option<&'a dyn crate::InterruptionSource>,
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
        match prepare(admission, controls.limits.admitted(), pack, request) {
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
        pub identity: EngineRuntimeDomainIdentity,
        pub placement: EngineRuntimeDomainPlacement,
        pub width: Option<NonZeroUsize>,
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
    pub enum EngineRuntimeDomainPlacement {
        InheritedUnmanaged,
        ManagedInProcess,
        ManagedIsolatedWorker,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilationExecutionFacilityCapacity {
        pub simultaneous_ready_jobs: NonZeroUsize,
        pub ready_queue: usize,
    }

    pub trait CompilationExecutionFacility {
        type Dispatch<'a>: Future<Output = CompilationDispatchOutcome> + 'a
        where
            Self: 'a;

        fn domain(&self) -> &EngineRuntimeDomainDescriptor;
        fn capacity(&self) -> CompilationExecutionFacilityCapacity;

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

        pub fn with_request_values(
            self,
            _capability: RequestValuesDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_override_bytes(
            self,
            _capability: OverrideBytesDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_backing_locators(
            self,
            _capability: BackingLocatorsDisclosureCapability,
        ) -> Self {
            unimplemented!()
        }

        pub fn with_adapter_detail(
            self,
            _capability: AdapterDetailDisclosureCapability,
        ) -> Self {
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
        pub fn diagnostic_sources(
            &self,
        ) -> impl ExactSizeIterator<Item = DisclosedSourceView<'_>> {
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
        pub fn override_bytes(
            &self,
        ) -> impl ExactSizeIterator<Item = DisclosedOverrideView<'_>> {
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

        pub fn with_expected_archive_content_identity(
            mut self,
            identity: ContentIdentity,
        ) -> Self {
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
            Self {
                pack_identity,
            }
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
        AdmissionRefused(crate::AdmissionRefusal),
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
        AdmissionRefused(crate::AdmissionRefusal),
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
        NotApplicable,
        NotAsserted,
        ExternallyAssertedAndByteVerified,
        ExternallyAssertedAndMismatched,
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
        pub fn requested_controls(&self) -> FormatReceiptControlsView<'_> {
            unimplemented!()
        }
        pub fn admitted_controls(&self) -> Option<FormatReceiptControlsView<'_>> {
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
        pub offline: bool,
        pub resource_profile: &'a crate::ResourceProfileIdentity,
        pub deadline_present: bool,
        pub cancellation_present: bool,
        pub publication_strength: Option<crate::transport::PublicationCommitStrength>,
        pub cleanup_strength: Option<crate::transport::RequestedCleanupStrength>,
        pub limits: FormatReceiptLimitsView<'a>,
        pub enforcement_facts: &'a [FormatEnforcementFactIdentifier],
    }

    pub enum FormatReceiptLimitsView<'a> {
        PackIngress(&'a PackIngressResourceLimits),
        Representation(&'a RepresentationResourceLimits),
        Transport(&'a crate::transport::TransportResourceLimits),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FormatPublicationStatus {
        pub requested: Option<crate::transport::PublicationCommitStrength>,
        pub admitted: Option<crate::transport::PublicationCommitStrength>,
        pub state: FormatPublicationState,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatPublicationState {
        NotApplicable,
        NotStarted,
        Committed,
        Failed,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct FormatCleanupStatus {
        pub requested: Option<crate::transport::RequestedCleanupStrength>,
        pub admitted: Option<crate::transport::RequestedCleanupStrength>,
        pub state: FormatCleanupState,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum FormatCleanupState {
        NotApplicable,
        Complete,
        ResidualExists,
        ExposedBytesNonRetractable,
        Failed,
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
    format_receipt!(PackArchivePublicationFormatReceipt);
    format_receipt!(ClosureExportPublicationFormatReceipt);

    impl PackArchiveEncodingFormatReceipt {
        pub fn control_record_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn source_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn archive_encoding_identity(&self) -> Option<&ArchiveEncodingIdentity> {
            unimplemented!()
        }
        pub fn output_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(
            &self,
        ) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
    }

    impl PackArchiveReadFormatReceipt {
        pub fn input_archive_identity(&self) -> Option<&ContentIdentity> {
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
        pub fn asserted_archive_encoding_identity(
            &self,
        ) -> Option<&ArchiveEncodingIdentity> {
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
        pub fn source_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn closure_export_tree_identity(
            &self,
        ) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn files(&self) -> Option<&[FormatReceiptFile]> {
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
        pub fn closure_export_tree_identity(
            &self,
        ) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn verification_mode(&self) -> FormatVerificationMode {
            unimplemented!()
        }
        pub fn files(&self) -> Option<&[FormatReceiptFile]> {
            unimplemented!()
        }
    }

    impl PackArchivePublicationFormatReceipt {
        pub fn source_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn output_archive_identity(&self) -> Option<&ContentIdentity> {
            unimplemented!()
        }
        pub fn archive_encoding_identity(&self) -> Option<&ArchiveEncodingIdentity> {
            unimplemented!()
        }
    }

    impl ClosureExportPublicationFormatReceipt {
        pub fn source_pack_identity(&self) -> Option<&PackIdentity> {
            unimplemented!()
        }
        pub fn source_tree_identity(&self) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn output_tree_identity(&self) -> Option<&ClosureExportTreeContentIdentity> {
            unimplemented!()
        }
        pub fn files(&self) -> Option<&[FormatReceiptFile]> {
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
    }

    impl PackArchiveEncodingReport {
        pub fn terminal(&self) -> Result<&EncodedPackArchive, &RepresentationFailure> {
            self.terminal.as_ref()
        }
        pub fn receipt(&self) -> &PackArchiveEncodingFormatReceipt {
            &self.receipt
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

    #[derive(Clone, Debug)]
    pub struct ProjectMaterializationProjectionReceipt {
        private: crate::private::ProjectMaterializationProjectionReceiptState,
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
