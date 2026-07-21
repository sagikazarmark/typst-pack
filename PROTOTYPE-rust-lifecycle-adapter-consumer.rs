//! External-consumer compile probe for the Rust interface prototype.
//!
//! This file is compiled against the prototype as an rlib. It is not executed.

#![allow(dead_code)]

extern crate typst_pack_interface as tp;

use std::future::{Future, Ready, ready};
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

use tp::authority::{
    AcquiredDependency, AcquisitionControls, AcquisitionProvenance, AsyncPackageAuthority,
    AuthorityFailure, AuthorityFailureClass, CompletePackageTree, DependencyAcquisitionOutcome,
    DependencyResolutionEvidence, EvidenceFactKind, EvidenceFactOutcome,
    EvidenceRevalidationOutcome, EvidenceRevalidationRequest, FontAuthorityCapabilityDescriptor,
    FontAuthorityCapabilityScopeProjection, FontAuthorityCapabilitySpec, FontAxis, FontAxisValue,
    FontCatalogAcquisition, FontCatalogCandidate, FontCatalogRequest, FontCatalogSnapshot,
    FontContainerAcquisitionRequest, FontSelectionFlags, FontStretch, FontStyle, FontWeight,
    OpenTypeAxisTag, PackageAcquisitionRequest, PackageAuthorityCapabilityDescriptor,
    SyncFontAuthority, SyncPackageAuthority, UnicodeCodepointRange,
};
use tp::compilation::{
    CanonicalDiagnosticPolicy, CompilationDispatchOutcome, CompilationExecutionFacility,
    CompilationExecutionFacilityCapabilityDescriptor, CompilationPreparationLimits,
    CompilationPreparationPolicy, CompilationReportTerminalRef, CompilationReportingPolicy,
    CompilationRequest, CompilationResourceLimitSpec, CompilationResourceLimits,
    NoSemanticResultCache, ReadyCompilationJob, SemanticCacheAdapterAdmissionOutcome,
    SemanticCacheAdmissionRequest, SemanticCacheLookupOutcome, SemanticCacheLookupRequest,
    SemanticResultCacheCapabilityDescriptor, SyncCompilationControls, SyncSemanticCacheLookup,
    SyncSemanticResultCache,
};
use tp::creation::{
    CreationDispatchOutcome, CreationEvidenceCapabilityDescriptor, CreationEvidenceFenceOutcome,
    CreationEvidenceFenceRequest, CreationExecutionFacility, CreationInput, CreationInputEvidence,
    CreationReportingPolicy, CreationRequest, CreationResourceLedger, CreationResourceLimitSpec,
    CreationResourceLimits, DiscoveryVariant, FontEmbeddingPolicy, PackageEmbeddingPolicy,
    ProjectSnapshot, ReadyCreationJob, SyncCreationControls, SyncCreationEvidence,
};
use tp::session::{
    ArmedSubscriptions, FenceConfirmation, FenceConfirmationOutcome, FenceReadObservation,
    FenceReadOutcome, RequestSourceObservation, SessionEffect, SessionEvent,
    SessionProviderObservations, SessionWatchCoverage, SubscriptionArmOutcome,
};
use tp::transport::{
    AcquisitionTransportControls, ClosureExportPublicationControls,
    CompilationDeliveryAdapterOutcome, CompilationDeliveryCapabilityDescriptor,
    CompilationDeliveryControls, MemorySpool, PackArchiveAcquirerCapabilityDescriptor,
    PackArchiveAcquisitionAdapterOutcome, PackArchivePublicationControls,
    ProjectMaterializationPublicationControls, PublicationCommitStrength, SyncCompilationDelivery,
    SyncPackArchiveAcquirer, SyncSpoolFacility, TransportAdapterStage, TransportCleanupOutcome,
};
use tp::{
    AdmittedOperationResourceLimits, AuthorityInstanceIdentity, CacheIsolationDomain,
    CanonicalIdentity, DeploymentTrustProfile, InterruptionSource, MonotonicClock,
    MonotonicInstant, MonotonicTimeDomain, OperationDeadline, OrdinaryAdmission, PackagePath,
    ProjectPath, StableByteValue,
};

struct TestClock {
    domain: MonotonicTimeDomain,
}

impl MonotonicClock for TestClock {
    fn domain(&self) -> &MonotonicTimeDomain {
        &self.domain
    }

    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::try_new(self.domain.clone(), 0)
    }

    fn poll_at(&self, _deadline: MonotonicInstant, _context: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}

struct NeverInterrupt;

impl InterruptionSource for NeverInterrupt {
    fn interrupted(&self) -> bool {
        false
    }

    fn poll_interrupted(&self, _context: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}

struct TestArchiveAcquirer {
    descriptor: PackArchiveAcquirerCapabilityDescriptor,
}

impl SyncPackArchiveAcquirer for TestArchiveAcquirer {
    type Locator = str;

    fn descriptor(&self) -> &PackArchiveAcquirerCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire(
        &self,
        _locator: &Self::Locator,
        controls: AcquisitionTransportControls<'_>,
    ) -> PackArchiveAcquisitionAdapterOutcome {
        let bytes = StableByteValue::from_static(controls.admission(), b"archive").unwrap();
        let exact_bytes = bytes.len();
        PackArchiveAcquisitionAdapterOutcome::try_new(
            Ok(bytes),
            vec![TransportAdapterStage::Transfer],
            exact_bytes,
            TransportCleanupOutcome::NotRequired,
            None,
            None,
            tp::transport::TransportAdapterTimingInput::NotRequested,
        )
        .unwrap()
    }
}

impl tp::transport::AsyncPackArchiveAcquirer for TestArchiveAcquirer {
    type Locator = str;
    type Acquire<'a>
        = Ready<PackArchiveAcquisitionAdapterOutcome>
    where
        Self: 'a;

    fn descriptor(&self) -> &PackArchiveAcquirerCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire<'a>(
        &'a self,
        _locator: &'a Self::Locator,
        controls: AcquisitionTransportControls<'a>,
    ) -> Self::Acquire<'a> {
        let bytes = StableByteValue::from_static(controls.admission(), b"archive").unwrap();
        let exact_bytes = bytes.len();
        ready(
            PackArchiveAcquisitionAdapterOutcome::try_new(
                Ok(bytes),
                vec![TransportAdapterStage::Transfer],
                exact_bytes,
                TransportCleanupOutcome::NotRequired,
                None,
                None,
                tp::transport::TransportAdapterTimingInput::NotRequested,
            )
            .unwrap(),
        )
    }
}

struct TestCache {
    descriptor: SemanticResultCacheCapabilityDescriptor,
}

impl SyncSemanticResultCache for TestCache {
    fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor {
        &self.descriptor
    }

    fn lookup(&self, request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome {
        let _reservation = request.controls.budget().reserve_record(1).unwrap();
        SemanticCacheLookupOutcome::Miss
    }

    fn admit(
        &self,
        request: SemanticCacheAdmissionRequest<'_>,
    ) -> SemanticCacheAdapterAdmissionOutcome {
        let _reservation = request.controls.budget().reserve_retained(1).unwrap();
        let _ = request.record.bytes();
        SemanticCacheAdapterAdmissionOutcome::Admitted
    }
}

struct TestDelivery {
    descriptor: CompilationDeliveryCapabilityDescriptor,
}

impl SyncCompilationDelivery for TestDelivery {
    type Destination = str;

    fn descriptor(&self) -> &CompilationDeliveryCapabilityDescriptor {
        &self.descriptor
    }

    fn deliver(
        &self,
        transfer: tp::compilation::CompilationDeliveryTransfer<'_>,
        _destination: &Self::Destination,
        _controls: CompilationDeliveryControls<'_>,
    ) -> CompilationDeliveryAdapterOutcome {
        let _ = transfer.projection().canonical_diagnostics_status();
        let _ = transfer.projection().canonical_diagnostics().count();
        CompilationDeliveryAdapterOutcome::try_new(
            Ok(()),
            vec![TransportAdapterStage::Transfer],
            0,
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            None,
            None,
            tp::transport::TransportAdapterTimingInput::NotRequested,
        )
        .unwrap()
    }
}

struct TestArchivePublisher {
    descriptor: tp::transport::PackArchivePublisherCapabilityDescriptor,
}

impl tp::transport::SyncPackArchivePublisher for TestArchivePublisher {
    type Destination = str;

    fn descriptor(&self) -> &tp::transport::PackArchivePublisherCapabilityDescriptor {
        &self.descriptor
    }

    fn publish(
        &self,
        archive: &tp::representation::EncodedPackArchive,
        _destination: &Self::Destination,
        _controls: PackArchivePublicationControls<'_>,
    ) -> tp::transport::PackArchivePublicationAdapterOutcome {
        tp::transport::PackArchivePublicationAdapterOutcome::try_new(
            Ok(()),
            vec![TransportAdapterStage::Transfer],
            archive.bytes().len(),
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            None,
            None,
            tp::transport::TransportAdapterTimingInput::NotRequested,
        )
        .unwrap()
    }
}

struct TestClosurePublisher {
    descriptor: tp::transport::ClosureExportPublisherCapabilityDescriptor,
}

impl tp::transport::SyncClosureExportPublisher for TestClosurePublisher {
    type Destination = str;

    fn descriptor(&self) -> &tp::transport::ClosureExportPublisherCapabilityDescriptor {
        &self.descriptor
    }

    fn publish(
        &self,
        plan: &tp::representation::ClosureExportPlan,
        _destination: &Self::Destination,
        _controls: ClosureExportPublicationControls<'_>,
    ) -> tp::transport::ClosureExportPublicationAdapterOutcome {
        tp::transport::ClosureExportPublicationAdapterOutcome::try_new(
            Ok(()),
            vec![TransportAdapterStage::Transfer],
            plan.aggregate_bytes(),
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            None,
            None,
            tp::transport::TransportAdapterTimingInput::NotRequested,
        )
        .unwrap()
    }
}

struct TestMaterializationPublisher {
    descriptor: tp::transport::ProjectMaterializationPublisherCapabilityDescriptor,
}

impl tp::transport::SyncProjectMaterializationPublisher for TestMaterializationPublisher {
    type Destination = str;

    fn descriptor(&self) -> &tp::transport::ProjectMaterializationPublisherCapabilityDescriptor {
        &self.descriptor
    }

    fn publish(
        &self,
        plan: &tp::representation::ProjectMaterializationPlan,
        _destination: &Self::Destination,
        _controls: ProjectMaterializationPublicationControls<'_>,
    ) -> tp::transport::ProjectMaterializationPublicationAdapterOutcome {
        let transferred = plan.files().map(|file| file.bytes.len()).sum();
        tp::transport::ProjectMaterializationPublicationAdapterOutcome::try_new(
            Ok(()),
            vec![TransportAdapterStage::Transfer],
            transferred,
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            None,
            None,
            tp::transport::TransportAdapterTimingInput::NotRequested,
        )
        .unwrap()
    }
}

fn assert_spool_traits<T: SyncSpoolFacility>() {}

fn typecheck_role_helpers() {
    assert_spool_traits::<MemorySpool>();
    let _: Option<TestCompilationFacility> = None;
    let _: Option<TestCreationFacility> = None;
    let _: Option<TestCache> = None;
    let _: Option<TestArchiveAcquirer> = None;
    let _: Option<TestDelivery> = None;
    let _: Option<TestArchivePublisher> = None;
    let _: Option<TestClosurePublisher> = None;
    let _: Option<TestMaterializationPublisher> = None;
    let admission = OrdinaryAdmission::try_new(DeploymentTrustProfile::Trusted).unwrap();
    let package_identity = AuthorityInstanceIdentity::try_new("test.success-packages").unwrap();
    let _ = SuccessfulPackages {
        admission: admission.clone(),
        descriptor: package_descriptor(&package_identity, "org.example/success-packages/1"),
        identity: package_identity,
    };
    let font_identity = AuthorityInstanceIdentity::try_new("test.success-fonts").unwrap();
    let _ = SuccessfulFonts {
        admission,
        descriptor: font_descriptor(&font_identity, "org.example/success-fonts/1"),
        identity: font_identity,
    };
}

fn typecheck_transport_adapter_facts() {
    let reached = vec![
        TransportAdapterStage::ReferenceResolution,
        TransportAdapterStage::Acquisition,
        TransportAdapterStage::Spooling,
        TransportAdapterStage::Transfer,
        TransportAdapterStage::Verification,
        TransportAdapterStage::Commit,
        TransportAdapterStage::Cleanup,
    ];
    let _ = tp::transport::SpoolAdapterOutcome::try_new(
        Err(tp::transport::SpoolAdapterFailure::Cancelled),
        reached.clone(),
        0,
        TransportCleanupOutcome::CleanupFailed,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Complete(vec![
            tp::transport::TransportAdapterPhaseTiming {
                stage: TransportAdapterStage::Spooling,
                elapsed_ticks: 1,
            },
        ]),
    );
    let _ = PackArchiveAcquisitionAdapterOutcome::try_new(
        Err(tp::transport::PackArchiveAcquisitionAdapterFailure::Deadline),
        reached.clone(),
        0,
        TransportCleanupOutcome::ResidualReported,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Unavailable,
    );
    let _ = tp::transport::PackArchivePublicationAdapterOutcome::try_new(
        Err(tp::transport::PackArchivePublicationAdapterFailure::Commit),
        reached.clone(),
        1,
        None,
        TransportCleanupOutcome::CleanupFailed,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Unavailable,
    );
    let _ = tp::transport::ProjectMaterializationPublicationAdapterOutcome::try_new(
        Err(tp::transport::ProjectMaterializationPublicationAdapterFailure::Transfer),
        reached.clone(),
        1,
        None,
        TransportCleanupOutcome::CleanupFailed,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Unavailable,
    );
    let _ = tp::transport::ClosureExportPublicationAdapterOutcome::try_new(
        Err(tp::transport::ClosureExportPublicationAdapterFailure::Cancelled),
        reached.clone(),
        1,
        Some(PublicationCommitStrength::Streaming),
        TransportCleanupOutcome::CleanupFailed,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Unavailable,
    );
    let _ = CompilationDeliveryAdapterOutcome::try_new(
        Err(tp::transport::CompilationDeliveryAdapterFailure::Deadline),
        reached,
        1,
        Some(PublicationCommitStrength::Streaming),
        TransportCleanupOutcome::CleanupFailed,
        None,
        Some(1),
        tp::transport::TransportAdapterTimingInput::Unavailable,
    );
}

fn typecheck_transport_controls(
    admission: &OrdinaryAdmission,
    clock: &dyn MonotonicClock,
    interruption: &dyn InterruptionSource,
) {
    let spool_limits = tp::transport::SpoolResourceLimits::try_new(1024, 1024, 1024, 1024).unwrap();
    let _ = tp::transport::SpoolControls::try_new(
        admission.clone(),
        AdmittedOperationResourceLimits::try_caller_selected(spool_limits).unwrap(),
        None,
        tp::transport::SpoolOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::Spool,
                tp::transport::TransportPermittedUse::StableAcquisition,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();

    let limits = tp::transport::TransportResourceLimits::try_new(16, 1024, 1024, 1024, 1).unwrap();
    let admitted = || AdmittedOperationResourceLimits::try_caller_selected(limits.clone()).unwrap();
    let _ = AcquisitionTransportControls::try_new(
        admission.clone(),
        admitted(),
        None,
        tp::transport::PackArchiveAcquisitionOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::PackArchiveAcquisition,
                tp::transport::TransportPermittedUse::ArchiveAcquisition,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();
    let _ = PackArchivePublicationControls::try_new(
        admission.clone(),
        admitted(),
        tp::transport::PackArchivePublicationOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            commit: PublicationCommitStrength::CompleteCollectionAtomic,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::PackArchivePublication,
                tp::transport::TransportPermittedUse::ArchivePublication,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();
    let _ = ProjectMaterializationPublicationControls::try_new(
        admission.clone(),
        admitted(),
        tp::transport::ProjectMaterializationPublicationOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            commit: PublicationCommitStrength::CompleteCollectionAtomic,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::ProjectMaterializationPublication,
                tp::transport::TransportPermittedUse::MaterializationPublication,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();
    let _ = ClosureExportPublicationControls::try_new(
        admission.clone(),
        admitted(),
        tp::transport::ClosureExportPublicationOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            commit: PublicationCommitStrength::CompleteCollectionAtomic,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::ClosureExportPublication,
                tp::transport::TransportPermittedUse::ClosureExportPublication,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();
    let _ = CompilationDeliveryControls::try_new(
        admission.clone(),
        admitted(),
        tp::transport::CompilationDeliveryOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            commit: PublicationCommitStrength::CompleteCollectionAtomic,
            cleanup_requirement: tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            required_enforcement: vec![],
            timing_requested: true,
            required_scope: transport_scope(
                tp::transport::TransportFacilityRole::CompilationDelivery,
                tp::transport::TransportPermittedUse::CompilationDelivery,
            ),
        },
        clock,
        interruption,
    )
    .unwrap();
}

fn transport_scope(
    role: tp::transport::TransportFacilityRole,
    permitted_use: tp::transport::TransportPermittedUse,
) -> tp::transport::TransportCapabilityScopeProjection {
    tp::transport::TransportCapabilityScopeProjection {
        role,
        permitted_uses: vec![permitted_use],
        coverage: tp::transport::TransportCoverageClass::OneFrozenSubject,
        completeness: tp::CapabilityProjectionCompleteness::Complete,
    }
}

struct EmptyByteSource;

impl tp::transport::SyncByteSource for EmptyByteSource {
    fn read(&mut self, _destination: &mut [u8]) -> Result<usize, tp::transport::ByteSourceFailure> {
        Ok(0)
    }
}

#[allow(clippy::too_many_arguments)]
fn typecheck_transport_operations(
    spool: &mut MemorySpool,
    source: &mut EmptyByteSource,
    spool_controls: tp::transport::SpoolControls<'_>,
    acquirer: &TestArchiveAcquirer,
    acquisition_controls: AcquisitionTransportControls<'_>,
    async_acquisition_controls: AcquisitionTransportControls<'_>,
    archive: &tp::representation::EncodedPackArchive,
    archive_publisher: &TestArchivePublisher,
    archive_controls: PackArchivePublicationControls<'_>,
    materialization_plan: &tp::representation::ProjectMaterializationPlan,
    materialization_publisher: &TestMaterializationPublisher,
    materialization_controls: ProjectMaterializationPublicationControls<'_>,
    closure_plan: &tp::representation::ClosureExportPlan,
    closure_publisher: &TestClosurePublisher,
    closure_controls: ClosureExportPublicationControls<'_>,
    delivery_plan: tp::compilation::CompilationDeliveryPlan,
    delivery: &TestDelivery,
    delivery_controls: CompilationDeliveryControls<'_>,
) {
    let spool_outcome = tp::transport::spool_sync(spool, source, spool_controls);
    inspect_spool_transport_state(spool_outcome.receipt().state());

    let acquisition = tp::transport::acquire_pack_archive_sync(
        acquirer,
        "memory://archive",
        acquisition_controls,
    );
    let _async_acquisition = tp::transport::acquire_pack_archive_async(
        acquirer,
        "memory://archive",
        async_acquisition_controls,
    );
    let archive_publication = tp::transport::publish_pack_archive_sync(
        archive,
        archive_publisher,
        "memory://archive-output",
        archive_controls,
    );
    let materialization = tp::transport::publish_project_materialization_sync(
        materialization_plan,
        materialization_publisher,
        "memory://project-output",
        materialization_controls,
    );
    let closure_publication = tp::transport::publish_closure_export_sync(
        closure_plan,
        closure_publisher,
        "memory://closure-output",
        closure_controls,
    );
    inspect_publication_composition(&archive_publication, &closure_publication, &materialization);

    let delivery = tp::transport::deliver_compilation_sync(
        delivery_plan,
        delivery,
        "memory://compilation-output",
        delivery_controls,
    );
    inspect_remaining_transport_roles(&acquisition, &delivery);
}

struct TestCompilationFacility {
    descriptor: CompilationExecutionFacilityCapabilityDescriptor,
}

impl CompilationExecutionFacility for TestCompilationFacility {
    type Dispatch<'a>
        = Ready<CompilationDispatchOutcome>
    where
        Self: 'a;

    fn descriptor(&self) -> &CompilationExecutionFacilityCapabilityDescriptor {
        &self.descriptor
    }

    fn dispatch<'a>(&'a self, _job: ReadyCompilationJob) -> Self::Dispatch<'a> {
        ready(CompilationDispatchOutcome::Refused)
    }
}

struct TestCreationFacility {
    descriptor: tp::creation::CreationExecutionFacilityCapabilityDescriptor,
}

impl CreationExecutionFacility for TestCreationFacility {
    type Dispatch<'a>
        = Ready<CreationDispatchOutcome>
    where
        Self: 'a;

    fn descriptor(&self) -> &tp::creation::CreationExecutionFacilityCapabilityDescriptor {
        &self.descriptor
    }

    fn dispatch<'a>(&'a self, _job: ReadyCreationJob) -> Self::Dispatch<'a> {
        ready(CreationDispatchOutcome::Refused)
    }
}

struct TestPackages {
    identity: AuthorityInstanceIdentity,
    descriptor: PackageAuthorityCapabilityDescriptor,
}

impl SyncPackageAuthority for TestPackages {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire(
        &self,
        _request: PackageAcquisitionRequest,
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<tp::authority::CompletePackageTree> {
        DependencyAcquisitionOutcome::Failed(
            AuthorityFailure::try_new(
                AuthorityFailureClass::Unavailable,
                "test.package.unavailable",
                "unavailable",
            )
            .unwrap(),
        )
    }

    fn revalidate(
        &self,
        _request: EvidenceRevalidationRequest,
        _controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        EvidenceRevalidationOutcome::Failed(
            tp::authority::EvidenceFailure::try_new("test.package.no-revalidation").unwrap(),
        )
    }
}

struct TestFonts {
    identity: AuthorityInstanceIdentity,
    descriptor: FontAuthorityCapabilityDescriptor,
}

impl SyncFontAuthority for TestFonts {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &FontAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn catalog(
        &self,
        _request: FontCatalogRequest,
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<FontCatalogAcquisition> {
        DependencyAcquisitionOutcome::Failed(
            AuthorityFailure::try_new(
                AuthorityFailureClass::Unavailable,
                "test.font.unavailable",
                "unavailable",
            )
            .unwrap(),
        )
    }

    fn acquire_container(
        &self,
        _request: FontContainerAcquisitionRequest,
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<StableByteValue> {
        DependencyAcquisitionOutcome::Failed(
            AuthorityFailure::try_new(
                AuthorityFailureClass::Unavailable,
                "test.font.unavailable",
                "unavailable",
            )
            .unwrap(),
        )
    }

    fn revalidate(
        &self,
        _request: EvidenceRevalidationRequest,
        _controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        EvidenceRevalidationOutcome::Failed(
            tp::authority::EvidenceFailure::try_new("test.font.no-revalidation").unwrap(),
        )
    }
}

struct SuccessfulPackages {
    admission: OrdinaryAdmission,
    identity: AuthorityInstanceIdentity,
    descriptor: PackageAuthorityCapabilityDescriptor,
}

impl SuccessfulPackages {
    fn evidence(&self, controls: &AcquisitionControls<'_>) -> DependencyResolutionEvidence {
        let key = controls
            .evidence_key(EvidenceFactKind::Content, Arc::from(&b"package"[..]), None)
            .unwrap();
        let mut builder = controls.evidence_builder();
        builder.record(key, EvidenceFactOutcome::Selected).unwrap();
        builder.finish().unwrap()
    }
}

impl SyncPackageAuthority for SuccessfulPackages {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire(
        &self,
        _request: PackageAcquisitionRequest,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<CompletePackageTree> {
        let _download = controls.budget().reserve_download(7).unwrap();
        let _expanded = controls.budget().reserve_expanded(7).unwrap();
        let _spool = controls.budget().reserve_stable_spool(7).unwrap();
        let _retained = controls.budget().reserve_retained(7).unwrap();
        let _file = controls.budget().reserve_package_file(7).unwrap();
        let path = PackagePath::parse(&self.admission, "typst.toml").unwrap();
        let bytes = StableByteValue::from_static(&self.admission, b"[package]").unwrap();
        let tree = CompletePackageTree::try_from_files(&controls, [(path, bytes)]).unwrap();
        DependencyAcquisitionOutcome::Acquired(AcquiredDependency {
            value: tree,
            evidence: self.evidence(&controls),
            provenance: AcquisitionProvenance::try_new(
                "test.memory",
                tp::authority::AcquisitionSourceClass::CallerSupplied,
                None,
            )
            .unwrap(),
        })
    }

    fn revalidate(
        &self,
        request: EvidenceRevalidationRequest,
        controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        for key in &request.keys {
            let _ = key.kind();
            let _ = key.opaque_key();
            let _ = key.immutable_version();
        }
        let cursor = controls
            .provider_cursor(Arc::from(&b"package-cursor"[..]))
            .unwrap();
        EvidenceRevalidationOutcome::Clean(
            controls
                .evidence_fence(request.keys, Arc::from(&b"generation-1"[..]), Some(cursor))
                .unwrap(),
        )
    }
}

struct SuccessfulFonts {
    admission: OrdinaryAdmission,
    identity: AuthorityInstanceIdentity,
    descriptor: FontAuthorityCapabilityDescriptor,
}

impl SuccessfulFonts {
    fn acquired<T>(
        &self,
        controls: &AcquisitionControls<'_>,
        value: T,
    ) -> DependencyAcquisitionOutcome<T> {
        let key = controls
            .evidence_key(EvidenceFactKind::Content, Arc::from(&b"font"[..]), None)
            .unwrap();
        let mut builder = controls.evidence_builder();
        builder.record(key, EvidenceFactOutcome::Selected).unwrap();
        DependencyAcquisitionOutcome::Acquired(AcquiredDependency {
            value,
            evidence: builder.finish().unwrap(),
            provenance: AcquisitionProvenance::try_new(
                "test.memory",
                tp::authority::AcquisitionSourceClass::CallerSupplied,
                None,
            )
            .unwrap(),
        })
    }
}

impl SyncFontAuthority for SuccessfulFonts {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &FontAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn catalog(
        &self,
        _request: FontCatalogRequest,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<FontCatalogAcquisition> {
        let _candidate = controls.budget().reserve_font_candidate().unwrap();
        let _face = controls.budget().reserve_font_face().unwrap();
        let axis = FontAxis::try_new(
            OpenTypeAxisTag::from_bytes(*b"wght"),
            FontAxisValue::try_from_be_bytes(100.0f32.to_be_bytes()).unwrap(),
            FontAxisValue::try_from_be_bytes(400.0f32.to_be_bytes()).unwrap(),
            FontAxisValue::try_from_be_bytes(900.0f32.to_be_bytes()).unwrap(),
        )
        .unwrap();
        let candidate = FontCatalogCandidate::try_new(
            &controls,
            controls
                .font_container_acquisition_identity(Arc::from(&b"font-container-0"[..]))
                .unwrap(),
            0,
            "Test".into(),
            FontStyle::Normal,
            FontWeight::try_new(400).unwrap(),
            FontStretch::try_from_thousandths(1000).unwrap(),
            FontSelectionFlags::try_from_bits(0).unwrap(),
            [axis],
            [UnicodeCodepointRange::try_new(0x20, 0x7e).unwrap()],
        )
        .unwrap();
        let snapshot = FontCatalogSnapshot::try_new(&controls, [candidate]).unwrap();
        let catalog = FontCatalogAcquisition::try_new(
            &controls,
            snapshot,
            tp::authority::FontScanPolicy {
                invalid_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
                unreadable_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
            },
            [],
        )
        .unwrap();
        self.acquired(&controls, catalog)
    }

    fn acquire_container(
        &self,
        request: FontContainerAcquisitionRequest,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<StableByteValue> {
        match request.purpose() {
            tp::authority::FontContainerAcquisitionPurpose::CatalogFace {
                acquisition_identity,
                face_index,
            } => {
                let _ = acquisition_identity.opaque();
                let _ = face_index;
            }
            tp::authority::FontContainerAcquisitionPurpose::ExternalRequirement {
                requirement_identity,
                expected_container_identity,
                expected_bytes,
                required_face_indices,
            } => {
                let _ = requirement_identity.as_str();
                let _ = expected_container_identity.as_str();
                let _ = expected_bytes;
                let _ = required_face_indices;
            }
        }
        self.acquired(
            &controls,
            StableByteValue::from_static(&self.admission, b"font").unwrap(),
        )
    }

    fn revalidate(
        &self,
        request: EvidenceRevalidationRequest,
        controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        let cursor = controls
            .provider_cursor(Arc::from(&b"font-cursor"[..]))
            .unwrap();
        EvidenceRevalidationOutcome::Clean(
            controls
                .evidence_fence(request.keys, Arc::from(&b"generation-1"[..]), Some(cursor))
                .unwrap(),
        )
    }
}

struct ImmutableCreationEvidence {
    identity: AuthorityInstanceIdentity,
    descriptor: CreationEvidenceCapabilityDescriptor,
}

impl SyncCreationEvidence for ImmutableCreationEvidence {
    fn provider_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor {
        &self.descriptor
    }

    fn fence(
        &self,
        request: CreationEvidenceFenceRequest,
        _controls: AcquisitionControls<'_>,
    ) -> CreationEvidenceFenceOutcome {
        CreationEvidenceFenceOutcome::Clean(
            tp::creation::CreationEvidenceFence::try_new(&request, []).unwrap(),
        )
    }
}

fn compilation_limits() -> CompilationResourceLimits {
    CompilationResourceLimits::try_new(CompilationResourceLimitSpec {
        dependencies: 128,
        downloaded_dependency_bytes: 64 * 1024 * 1024,
        expanded_dependency_bytes: 256 * 1024 * 1024,
        largest_dependency_bytes: 32 * 1024 * 1024,
        override_count: 100_000,
        largest_override_bytes: 512 * 1024 * 1024,
        aggregate_override_bytes: 4 * 1024 * 1024 * 1024,
        stable_spool_bytes: 512 * 1024 * 1024,
        pages: 500,
        artifacts: 500,
        largest_artifact_bytes: 128 * 1024 * 1024,
        aggregate_artifact_bytes: 512 * 1024 * 1024,
        largest_raster_pixels: 32_000_000,
        aggregate_raster_pixels: 256_000_000,
        retained_memory_bytes: 768 * 1024 * 1024,
        diagnostic_projection_entries: 5_000,
        diagnostic_projection_bytes: 8 * 1024 * 1024,
        diagnostic_source_bindings: 5_000,
        diagnostic_source_blobs: 5_000,
        largest_diagnostic_source_bytes: 8 * 1024 * 1024,
        aggregate_diagnostic_source_bytes: 8 * 1024 * 1024,
        diagnostic_source_metadata_bytes: 8 * 1024 * 1024,
    })
    .unwrap()
}

fn typecheck_sync_common_path() {
    let admission = OrdinaryAdmission::try_new(DeploymentTrustProfile::Trusted).unwrap();
    let domain = MonotonicTimeDomain::try_new("test.monotonic").unwrap();
    let clock = TestClock { domain };
    let interruption = NeverInterrupt;

    let creation_limits = CreationResourceLimits::try_new(CreationResourceLimitSpec {
        project_files: 10_000,
        aggregate_project_bytes: 512 * 1024 * 1024,
        largest_project_file_bytes: 128 * 1024 * 1024,
        packages: 64,
        package_files: 100_000,
        largest_package_file_bytes: 128 * 1024 * 1024,
        package_tree_bytes: 256 * 1024 * 1024,
        font_containers: 128,
        font_candidates: 2_000,
        font_faces: 512,
        font_bytes: 256 * 1024 * 1024,
        discovery_variants: 16,
        discovery_restarts: 128,
        override_count: 100_000,
        largest_override_bytes: 512 * 1024 * 1024,
        aggregate_override_bytes: 4 * 1024 * 1024 * 1024,
        aggregate_file_bindings: 100_000,
        aggregate_logical_bytes: 512 * 1024 * 1024,
        stable_spool_bytes: 512 * 1024 * 1024,
        retained_memory_bytes: 768 * 1024 * 1024,
    })
    .unwrap();
    let creation_limits =
        AdmittedOperationResourceLimits::try_caller_selected(creation_limits).unwrap();
    let creation_resources = CreationResourceLedger::try_new(creation_limits).unwrap();

    let main = ProjectPath::parse(&admission, "main.typ").unwrap();
    let bytes = StableByteValue::from_static(&admission, b"Hello").unwrap();
    let project = ProjectSnapshot::try_from_files(
        &admission,
        &creation_resources,
        main.clone(),
        [(main, bytes)],
    )
    .unwrap();
    let request = CreationRequest::try_new(
        creation_resources.limits(),
        project,
        [DiscoveryVariant::paged_explicit_empty()],
        PackageEmbeddingPolicy::embed_all(),
        FontEmbeddingPolicy::embed_all(),
        tp::pack::PackMetadata::empty(),
        [],
    )
    .unwrap();
    let input = CreationInput::try_new(
        request.clone(),
        CreationInputEvidence::caller_owned_immutable(&request),
    )
    .unwrap();

    let package_identity = AuthorityInstanceIdentity::try_new("test.packages").unwrap();
    let packages = TestPackages {
        descriptor: package_descriptor(&package_identity, "org.example/test-packages/1"),
        identity: package_identity,
    };
    let font_identity = AuthorityInstanceIdentity::try_new("test.fonts").unwrap();
    let fonts = TestFonts {
        descriptor: font_descriptor(&font_identity, "org.example/test-fonts/1"),
        identity: font_identity,
    };
    let evidence_identity = AuthorityInstanceIdentity::try_new("test.creation-evidence").unwrap();
    let evidence = ImmutableCreationEvidence {
        descriptor: creation_evidence_descriptor(&evidence_identity),
        identity: evidence_identity,
    };

    let package_trait: &dyn SyncPackageAuthority = &packages;
    let font_trait: &dyn SyncFontAuthority = &fonts;
    let evidence_trait: &dyn SyncCreationEvidence = &evidence;
    let reporting = reporting_descriptor();

    let creation_controls = SyncCreationControls::try_admit(
        admission.clone(),
        creation_resources,
        evidence_trait,
        package_trait,
        font_trait,
        tp::creation::CreationOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            dependency_concurrency: NonZeroUsize::new(4).unwrap(),
            engine_width: tp::EngineWidthRequest::InheritedUnmanaged,
            requested_ready_jobs: None,
            requested_queue: None,
            requested_workers: None,
            font_scan_policy: tp::authority::FontScanPolicy {
                invalid_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
                unreadable_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
            },
            required_capabilities: tp::creation::CreationRequiredCapabilityGrants::bind_sync(
                evidence_trait,
                package_trait,
                font_trait,
                &reporting,
                tp::creation::CreationRequiredCapabilityScopes {
                    evidence: tp::creation::CreationEvidenceCapabilityScopeProjection {
                        permitted_uses: vec![
                            tp::creation::CreationEvidencePermittedUse::Stabilization,
                            tp::creation::CreationEvidencePermittedUse::Revalidation,
                        ],
                        coverage: tp::creation::CreationEvidenceCoverageClass::ExactOperationInputs,
                        completeness: tp::CapabilityProjectionCompleteness::Complete,
                    },
                    packages: package_scope(),
                    fonts: font_scope(),
                    execution: None,
                    reporting: tp::compilation::ReportingCapabilityScopeProjection {
                        permitted_uses: vec![],
                        coverage: tp::compilation::ReportingCoverageClass::SelectedReportChannels,
                        completeness: tp::CapabilityProjectionCompleteness::Complete,
                    },
                },
            ),
            requested_execution_placement: tp::ExecutionPlacement::CallerThread,
            requested_isolation: tp::OperationIsolationRequest::InProcess(
                tp::InProcessIsolationContract {
                    claimed_enforcement: vec![],
                },
            ),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            queue_timeout_ticks: None,
            latency_target_ticks: None,
            required_enforcement: vec![],
            reporting: CreationReportingPolicy {
                timing: false,
                fine_engine_timing: false,
            },
        },
        &clock,
        &interruption,
    )
    .unwrap();

    let creation_report = tp::creation::create_sync(input, creation_controls);
    let Ok(pack) = creation_report.into_pack() else {
        return;
    };

    let diagnostics = CanonicalDiagnosticPolicy::try_new(1, 5_000, 8 * 1024 * 1024).unwrap();
    let request = CompilationRequest::pdf(diagnostics);
    let compilation_limits = compilation_limits();
    let compilation_limits =
        AdmittedOperationResourceLimits::try_caller_selected(compilation_limits).unwrap();
    let prepared = pack
        .prepare(
            &admission,
            &CompilationPreparationPolicy {
                reject_unknown_engine_features: true,
                require_canonical_diagnostic_policy: true,
            },
            &CompilationPreparationLimits {
                override_count: 100_000,
                largest_override_bytes: 512 * 1024 * 1024,
                aggregate_override_bytes: 4 * 1024 * 1024 * 1024,
                diagnostic_entries: 5_000,
                diagnostic_entry_bytes: 8 * 1024 * 1024,
            },
            request,
        )
        .unwrap();
    let no_cache =
        NoSemanticResultCache::new(CacheIsolationDomain::try_new("test.cache-isolation").unwrap());
    let controls = SyncCompilationControls::try_admit(
        prepared.clone(),
        admission,
        compilation_limits,
        package_trait,
        font_trait,
        SyncSemanticCacheLookup::Disabled::<NoSemanticResultCache>,
        tp::compilation::CompilationOperationRequest {
            network: tp::OperationNetworkPolicy::Offline,
            cache: tp::compilation::SemanticResultCachePolicy::Disabled,
            dependency_concurrency: NonZeroUsize::new(4).unwrap(),
            engine_width: tp::EngineWidthRequest::InheritedUnmanaged,
            requested_ready_jobs: None,
            requested_queue: None,
            requested_workers: None,
            required_capabilities: tp::compilation::CompilationRequiredCapabilityGrants::bind_sync(
                package_trait,
                font_trait,
                &SyncSemanticCacheLookup::Disabled::<NoSemanticResultCache>,
                &reporting,
                tp::compilation::CompilationRequiredCapabilityScopes {
                    packages: package_scope(),
                    fonts: font_scope(),
                    cache: None,
                    execution: None,
                    reporting: tp::compilation::ReportingCapabilityScopeProjection {
                        permitted_uses: vec![],
                        coverage: tp::compilation::ReportingCoverageClass::SelectedReportChannels,
                        completeness: tp::CapabilityProjectionCompleteness::Complete,
                    },
                },
            ),
            requested_execution_placement: tp::ExecutionPlacement::CallerThread,
            requested_isolation: tp::OperationIsolationRequest::InProcess(
                tp::InProcessIsolationContract {
                    claimed_enforcement: vec![],
                },
            ),
            interruption: tp::OperationInterruptionStrength::Cooperative,
            deadline: OperationDeadline::None,
            queue_timeout_ticks: None,
            latency_target_ticks: None,
            required_enforcement: vec![],
            reporting: CompilationReportingPolicy {
                diagnostic_projection: false,
                diagnostic_source_bundle: false,
                timing: false,
                fine_engine_timing: false,
            },
        },
        &clock,
        Some(&interruption),
    )
    .unwrap();

    let report = tp::compilation::run_sync(controls);
    if let CompilationReportTerminalRef::Result(result) = report.terminal() {
        let _ = result.diagnostics();
        let _ = result.document_summary();
        for artifact in result.artifacts() {
            let _ = artifact.bytes.len();
        }
    }
    let _ = no_cache;
}

struct LocalPackages {
    identity: AuthorityInstanceIdentity,
    descriptor: PackageAuthorityCapabilityDescriptor,
    marker: Rc<()>,
}

impl AsyncPackageAuthority for LocalPackages {
    type Acquire<'a>
        = std::pin::Pin<
        Box<
            dyn Future<Output = DependencyAcquisitionOutcome<tp::authority::CompletePackageTree>>
                + 'a,
        >,
    >
    where
        Self: 'a;

    type Revalidate<'a>
        = std::pin::Pin<Box<dyn Future<Output = EvidenceRevalidationOutcome> + 'a>>
    where
        Self: 'a;

    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire<'a>(
        &'a self,
        _request: PackageAcquisitionRequest,
        _controls: AcquisitionControls<'a>,
    ) -> Self::Acquire<'a> {
        let marker = Rc::clone(&self.marker);
        Box::pin(async move {
            let _ = marker;
            DependencyAcquisitionOutcome::Failed(
                AuthorityFailure::try_new(
                    AuthorityFailureClass::Unavailable,
                    "test.local.unavailable",
                    "unavailable",
                )
                .unwrap(),
            )
        })
    }

    fn revalidate<'a>(
        &'a self,
        _request: EvidenceRevalidationRequest,
        _controls: AcquisitionControls<'a>,
    ) -> Self::Revalidate<'a> {
        Box::pin(async {
            EvidenceRevalidationOutcome::Failed(
                tp::authority::EvidenceFailure::try_new("test.local.no-revalidation").unwrap(),
            )
        })
    }
}

struct SendPackages {
    identity: AuthorityInstanceIdentity,
    descriptor: PackageAuthorityCapabilityDescriptor,
}

impl AsyncPackageAuthority for SendPackages {
    type Acquire<'a>
        = Ready<DependencyAcquisitionOutcome<tp::authority::CompletePackageTree>>
    where
        Self: 'a;
    type Revalidate<'a>
        = Ready<EvidenceRevalidationOutcome>
    where
        Self: 'a;

    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor {
        &self.descriptor
    }

    fn acquire<'a>(
        &'a self,
        _request: PackageAcquisitionRequest,
        _controls: AcquisitionControls<'a>,
    ) -> Self::Acquire<'a> {
        ready(DependencyAcquisitionOutcome::Failed(
            AuthorityFailure::try_new(
                AuthorityFailureClass::Unavailable,
                "test.send.unavailable",
                "unavailable",
            )
            .unwrap(),
        ))
    }

    fn revalidate<'a>(
        &'a self,
        _request: EvidenceRevalidationRequest,
        _controls: AcquisitionControls<'a>,
    ) -> Self::Revalidate<'a> {
        ready(EvidenceRevalidationOutcome::Failed(
            tp::authority::EvidenceFailure::try_new("test.send.no-revalidation").unwrap(),
        ))
    }
}

fn assert_send<T: Send>(_value: T) {}

fn typecheck_async_future_policy<'a>(
    local: &'a LocalPackages,
    send: &'a SendPackages,
    local_request: PackageAcquisitionRequest,
    send_request: PackageAcquisitionRequest,
    local_controls: AcquisitionControls<'a>,
    send_controls: AcquisitionControls<'a>,
) {
    let _local_future = local.acquire(local_request, local_controls);
    assert_send(send.acquire(send_request, send_controls));
}

fn drive_session_effect(
    admission: &OrdinaryAdmission,
    session: &mut tp::CompilationSession,
    _authority: &AuthorityInstanceIdentity,
    effect: SessionEffect,
) {
    match effect {
        SessionEffect::StartAttempt { plan, .. } => {
            let _ = plan.prepared_identity();
            let _ = plan.revision();
            let _ = plan.evaluation();
            let _ = plan.policy().mode();
            let _ = plan.supersession_permit().is_revoked();
        }
        SessionEffect::InterruptAttempt { token } => {
            let _ = token.ordinal();
        }
        SessionEffect::ReadFence { token, plan } => {
            let evidence = plan.historical_evidence().cloned();
            for dependency in plan.dependency_interests() {
                let _ = (dependency.provider.as_str(), dependency.key.kind());
            }
            let mut cursors = Vec::new();
            let observations: Vec<_> = plan
                .request_sources()
                .map(|source| {
                    let _ = source.provider().as_str();
                    let cursor = plan
                        .request_source_cursor(source, Arc::from(&b"cursor"[..]))
                        .unwrap();
                    cursors.push(cursor.clone());
                    RequestSourceObservation::try_new(
                        &plan,
                        source,
                        CanonicalIdentity::parse(admission, "test.request-source").unwrap(),
                        Some(cursor),
                    )
                    .unwrap()
                })
                .collect();
            let providers = SessionProviderObservations::try_new(cursors).unwrap();
            let observation =
                FenceReadObservation::try_new(&plan, observations, evidence, providers).unwrap();
            for source in observation.request_sources() {
                let _ = (
                    source.scope().as_str(),
                    source.identity().as_str(),
                    source.cursor(),
                );
            }
            inspect_provider_observations(observation.provider_observations());
            let outcome = FenceReadOutcome::Read(observation);
            inspect_fence_read_outcome(&outcome);
            inspect_session_transition(
                session.apply(SessionEvent::FenceReadFinished { token, outcome }),
            );
        }
        SessionEffect::ArmSubscriptions { token, plan } => {
            for interest in plan.interests() {
                let _ = interest.provider.as_str();
                let _ = interest.scope;
                let _ = interest.keys;
                let _ = interest.after;
            }
            let armed =
                ArmedSubscriptions::try_new(&plan, SessionWatchCoverage::complete_push(), [])
                    .unwrap();
            inspect_watch_coverage(armed.coverage());
            for cursor in armed.cursors() {
                let _ = (cursor.provider.as_str(), cursor.cursor.opaque());
            }
            let outcome = SubscriptionArmOutcome::Armed(armed);
            inspect_subscription_arm_outcome(&outcome);
            inspect_session_transition(
                session.apply(SessionEvent::SubscriptionsArmed { token, outcome }),
            );
        }
        SessionEffect::ConfirmFence { token, plan } => {
            let _ = plan.request_sources().count();
            for interest in plan.dependency_interests() {
                let _ = (interest.provider.as_str(), interest.key.kind());
            }
            for cursor in plan.armed_cursors() {
                let _ = (cursor.provider.as_str(), cursor.cursor.opaque());
            }
            let providers = SessionProviderObservations::try_new([]).unwrap();
            let confirmation = FenceConfirmation::try_new(&plan, providers).unwrap();
            inspect_provider_observations(confirmation.observations());
            let outcome = FenceConfirmationOutcome::Clean(confirmation);
            inspect_fence_confirmation_outcome(&outcome);
            inspect_session_transition(
                session.apply(SessionEvent::FenceConfirmed { token, outcome }),
            );
        }
        SessionEffect::RetireSubscriptions { generation } => {
            let _ = generation.ordinal();
        }
        SessionEffect::Publish { publication } => {
            let _ = publication.session_instance().as_str();
            let _ = publication.sequence().ordinal();
            let _ = publication.revision();
            let _ = publication.evaluation();
            let _ = publication.currentness();
        }
    }
}

fn inspect_watch_coverage(coverage: &SessionWatchCoverage) {
    match coverage.view() {
        tp::session::SessionWatchCoverageView::CompletePush
        | tp::session::SessionWatchCoverageView::CompletePoll => {}
        tp::session::SessionWatchCoverageView::Incomplete(scopes) => {
            let _ = (
                scopes.request_sources().count(),
                scopes.dependencies().count(),
            );
        }
    }
}

fn inspect_affected_scopes(scopes: &tp::session::SessionAffectedScopes) {
    let _ = (
        scopes.request_sources().count(),
        scopes.dependencies().count(),
    );
}

fn inspect_provider_observations(observations: &SessionProviderObservations) {
    for cursor in observations.cursors() {
        let _ = (cursor.provider.as_str(), cursor.cursor.opaque());
    }
}

fn inspect_fence_read_outcome(outcome: &FenceReadOutcome) {
    match outcome {
        FenceReadOutcome::Read(value) => {
            let _ = value.request_sources().count();
            let _ = value.dependency_evidence();
            inspect_provider_observations(value.provider_observations());
        }
        FenceReadOutcome::Changed(scopes) => inspect_affected_scopes(scopes),
        FenceReadOutcome::Incomplete(coverage) => inspect_watch_coverage(coverage),
        FenceReadOutcome::Failed(failure) => {
            inspect_affected_scopes(failure.scopes());
            let _ = failure.safe_code();
        }
    }
}

fn inspect_subscription_arm_outcome(outcome: &SubscriptionArmOutcome) {
    match outcome {
        SubscriptionArmOutcome::Armed(value) => {
            inspect_watch_coverage(value.coverage());
            for cursor in value.cursors() {
                let _ = (cursor.provider.as_str(), cursor.cursor.opaque());
            }
        }
        SubscriptionArmOutcome::Incomplete(coverage) => inspect_watch_coverage(coverage),
        SubscriptionArmOutcome::Failed => {}
    }
}

fn inspect_fence_confirmation_outcome(outcome: &FenceConfirmationOutcome) {
    match outcome {
        FenceConfirmationOutcome::Clean(value) => {
            inspect_provider_observations(value.observations());
        }
        FenceConfirmationOutcome::Dirty(scopes) => inspect_affected_scopes(scopes),
        FenceConfirmationOutcome::Incomplete(coverage) => inspect_watch_coverage(coverage),
        FenceConfirmationOutcome::Failed(failure) => {
            inspect_affected_scopes(failure.scopes());
            let _ = failure.safe_code();
        }
    }
}

fn inspect_session_transition(
    transition: Result<tp::session::SessionTransition, tp::session::SessionEventRejection>,
) {
    match transition {
        Ok(tp::session::SessionTransition::Applied(effects)) => {
            for effect in effects {
                match effect {
                    SessionEffect::StartAttempt { token, plan } => {
                        let _ = (token.ordinal(), plan.prepared_identity());
                    }
                    SessionEffect::InterruptAttempt { token } => {
                        let _ = token.ordinal();
                    }
                    SessionEffect::ReadFence { token, plan } => {
                        let _ = (token.ordinal(), plan.request_sources().count());
                    }
                    SessionEffect::ArmSubscriptions { token, plan } => {
                        let _ = (token.ordinal(), plan.interests().count());
                    }
                    SessionEffect::ConfirmFence { token, plan } => {
                        let _ = (token.ordinal(), plan.request_sources().count());
                    }
                    SessionEffect::RetireSubscriptions { generation } => {
                        let _ = generation.ordinal();
                    }
                    SessionEffect::Publish { publication } => {
                        let _ = publication.sequence().ordinal();
                    }
                }
            }
        }
        Ok(tp::session::SessionTransition::Ignored(reason)) => match reason {
            tp::session::SessionIgnoredEvent::PreviousSessionInstance
            | tp::session::SessionIgnoredEvent::SupersededRevision
            | tp::session::SessionIgnoredEvent::StaleAttempt
            | tp::session::SessionIgnoredEvent::StaleFence
            | tp::session::SessionIgnoredEvent::OldSubscription
            | tp::session::SessionIgnoredEvent::DuplicateCompletion => {}
        },
        Err(reason) => match reason {
            tp::session::SessionEventRejection::MalformedToken
            | tp::session::SessionEventRejection::ImpossibleTransition
            | tp::session::SessionEventRejection::AdapterContractViolation
            | tp::session::SessionEventRejection::Retiring
            | tp::session::SessionEventRejection::Retired => {}
        },
    }
}

fn inspect_session_attempt_release(release: tp::session::SessionAttemptRelease) {
    match release {
        tp::session::SessionAttemptRelease::Reaped
        | tp::session::SessionAttemptRelease::AbandonedNoLiveResource => {}
    }
}

fn inspect_pack_object(object: tp::pack::IdentityObjectInspection<'_>) {
    let _ = (object.exact_bytes, object.content_identity);
}

fn inspect_discovery_sensitive(value: tp::pack::DiscoverySensitiveValueInspection<'_>) {
    let _ = (value.exact_bytes(), value.commitment());
}

fn inspect_complete_pack(pack: &tp::Pack) {
    let inspection = pack.inspect();
    let _ = inspection.identity();
    let _ = inspection.entrypoint();
    let engine = inspection.discovery_engine();
    let _ = engine.identity();
    let _ = engine.producer_id();
    let _ = engine.implementation_name();
    let implementation_version = engine.implementation_version();
    let _ = (
        implementation_version.major,
        implementation_version.minor,
        implementation_version.patch,
    );
    let _ = engine.exact_build_fingerprint();
    let _ = engine.target_profile();
    for (name, value) in engine.qualifiers() {
        let _ = (name, value);
    }
    let unicode_xid_version = engine.unicode_xid_version();
    let _ = (
        unicode_xid_version.major,
        unicode_xid_version.minor,
        unicode_xid_version.patch,
    );
    let _ = engine.package_metadata_profile_id();
    let _ = engine.font_metadata_profile_id();

    let project = inspection.project_tree();
    let _ = project.identity();
    let _ = project.file_count();
    let _ = project.aggregate_bytes();
    for file in project.files() {
        let _ = file.path;
        let _ = file.content_identity;
        let _ = file.exact_bytes;
    }
    for path in inspection.explicit_conditional_inclusions() {
        let _ = path;
    }

    for variant in inspection.discovery_variants() {
        let _ = variant.declaration_ordinal();
        let _ = variant.label();
        let _ = variant.identity();
        let _ = variant.trace_identity();
        let _ = variant.coverage_identity();
        let request = variant.request();
        let _ = request.target();
        let _ = request.document_time();
        for input in request.inputs() {
            let _ = input.key;
            let _ = input.value.exact_bytes();
            let _ = input.value.commitment();
        }
        for feature in request.features() {
            let _ = feature.as_str();
        }
        for override_ in request.overrides() {
            let _ = override_.path;
            inspect_discovery_sensitive(override_.value);
        }
        for observation in variant.trace().project_observations() {
            match observation {
                tp::pack::DiscoveryProjectObservationInspection::BaselineRead {
                    path,
                    request_kind,
                    object,
                } => {
                    let _ = (path, request_kind);
                    inspect_pack_object(object);
                }
                tp::pack::DiscoveryProjectObservationInspection::OverrideRead {
                    path,
                    request_kind,
                    replacement,
                } => {
                    let _ = (path, request_kind);
                    inspect_discovery_sensitive(replacement);
                }
                tp::pack::DiscoveryProjectObservationInspection::Missing { path, request_kind } => {
                    let _ = (path, request_kind);
                }
            }
        }
        for observation in variant.trace().package_observations() {
            match observation {
                tp::pack::DiscoveryPackageObservationInspection::Read {
                    requirement,
                    path,
                    request_kind,
                    object,
                } => {
                    let _ = (requirement, path, request_kind);
                    inspect_pack_object(object);
                }
                tp::pack::DiscoveryPackageObservationInspection::Missing {
                    requirement,
                    path,
                    request_kind,
                } => {
                    let _ = (requirement, path, request_kind);
                }
            }
        }
        for face in variant.trace().used_font_faces() {
            let _ = (face.container_identity, face.face_index);
        }
    }

    for package in inspection.package_requirements() {
        let _ = package.identity;
        let _ = package.specification;
        let _ = package.tree_identity;
        for file in package.files {
            let _ = (file.path, file.content_identity, file.exact_bytes);
        }
        let version = package.manifest.version;
        let _ = (
            package.manifest.name,
            version.major,
            version.minor,
            version.patch,
            package.manifest.entrypoint,
        );
        if let Some(version) = package.manifest.minimum_compiler_version {
            let _ = (version.major, version.minor, version.patch);
        }
        let _ = package.disposition;
        let _ = (
            package.provenance.authority_kind,
            package.provenance.source_class,
            package.provenance.logical_origin,
        );
    }
    for font in inspection.font_requirements() {
        let _ = font.identity;
        let _ = (font.container.content_identity, font.container.exact_bytes);
        let _ = font.disposition;
        let _ = (
            font.provenance.authority_kind,
            font.provenance.source_class,
            font.provenance.logical_origin,
        );
        for variant in font.observing_variants {
            let _ = variant;
        }
        for face in font.faces {
            let _ = face.identity.container_identity;
            let _ = face.identity.face_index;
            let _ = face.selection.family;
            let _ = (
                face.selection.style,
                face.selection.weight.get(),
                face.selection.stretch.thousandths(),
                face.selection.flags.bits(),
            );
            for axis in face.selection.axes {
                let _ = (
                    axis.tag().as_bytes(),
                    axis.minimum().to_be_bytes(),
                    axis.default().to_be_bytes(),
                    axis.maximum().to_be_bytes(),
                );
            }
            for range in face.selection.codepoint_coverage {
                let _ = (range.first(), range.last());
            }
            let _ = face.licensing.fs_type;
            for name in face.licensing.name_records {
                let _ = (
                    name.name_id,
                    name.platform_id,
                    name.encoding_id,
                    name.language_id,
                    name.exact_bytes,
                );
            }
        }
    }
    for face in inspection.font_catalog() {
        let _ = (face.container_identity, face.face_index);
    }
    let metadata = inspection.metadata();
    let _ = (metadata.title(), metadata.description());
    for author in metadata.authors() {
        let _ = author;
    }
    for keyword in metadata.keywords() {
        let _ = keyword;
    }
    for extension in inspection.semantic_extensions() {
        let _ = extension.identifier.as_str();
        let _ = extension.epoch;
        let _ = extension.canonical_payload;
        for object in extension.required_objects {
            let _ = (object.content_identity, object.exact_bytes);
        }
    }
    for annotation in inspection.annotations() {
        let _ = annotation.identifier().as_str();
        let _ = annotation.epoch();
        let _ = annotation.payload();
    }
    let guarantees = inspection.guarantees();
    let _ = (guarantees.portable, guarantees.self_contained);
}

fn inspect_inventory_leaf<T>(leaf: &tp::compilation::InventoryLeaf<T>) {
    let _ = (
        &leaf.value,
        leaf.origin,
        leaf.status,
        leaf.declaration_ordinal,
    );
}

fn inspect_page_selection(selection: tp::compilation::PageSelectionInventoryView<'_>) {
    match selection {
        tp::compilation::PageSelectionInventoryView::All { origin, status } => {
            let _ = (origin, status);
        }
        tp::compilation::PageSelectionInventoryView::Ranges(ranges) => {
            for range in ranges {
                inspect_inventory_leaf(range);
            }
        }
    }
}

fn inspect_request_inventory(inventory: tp::compilation::CompilationRequestInventoryView<'_>) {
    for entry in inventory.entries() {
        match entry {
            tp::compilation::CompilationRequestInventoryEntryView::Pack {
                identity,
                origin,
                status,
            } => {
                let _ = identity.as_str();
                let _ = (origin, status);
            }
            tp::compilation::CompilationRequestInventoryEntryView::PackOverride {
                path,
                exact_bytes,
                commitment,
                origin,
                status,
                declaration_ordinal,
            } => {
                let _ = path.as_str();
                let _ = commitment.map(|value| value.as_str());
                let _ = (exact_bytes, origin, status, declaration_ordinal);
            }
            tp::compilation::CompilationRequestInventoryEntryView::TypstInput {
                key,
                exact_utf8_bytes,
                commitment,
                origin,
                status,
                declaration_ordinal,
            } => {
                let _ = key.as_str();
                let _ = commitment.as_str();
                let _ = (exact_utf8_bytes, origin, status, declaration_ordinal);
            }
            tp::compilation::CompilationRequestInventoryEntryView::DocumentTime {
                value,
                origin,
                status,
                declaration_ordinal,
            } => {
                let _ = (value, origin, status, declaration_ordinal);
            }
            tp::compilation::CompilationRequestInventoryEntryView::Feature {
                value,
                origin,
                status,
                declaration_ordinal,
            } => {
                let _ = value.as_str();
                let _ = (origin, status, declaration_ordinal);
            }
            tp::compilation::CompilationRequestInventoryEntryView::Target {
                value,
                origin,
                status,
                declaration_ordinal,
            } => {
                let _ = (value, origin, status, declaration_ordinal);
            }
            tp::compilation::CompilationRequestInventoryEntryView::Output(output) => match output {
                tp::compilation::CompilationOutputInventoryView::Pdf(value) => {
                    inspect_inventory_leaf(&value.format);
                    inspect_page_selection(value.pages);
                    inspect_inventory_leaf(&value.identifier);
                    inspect_inventory_leaf(&value.creator);
                    inspect_inventory_leaf(&value.creation_time);
                    for standard in value.standards {
                        inspect_inventory_leaf(standard);
                    }
                    inspect_inventory_leaf(&value.tagging);
                    inspect_inventory_leaf(&value.pretty);
                    let _ = value.status;
                }
                tp::compilation::CompilationOutputInventoryView::Png(value) => {
                    inspect_inventory_leaf(&value.format);
                    inspect_page_selection(value.pages);
                    inspect_inventory_leaf(&value.pixels_per_inch);
                    inspect_inventory_leaf(&value.bleed);
                    let _ = value.status;
                }
                tp::compilation::CompilationOutputInventoryView::Svg(value) => {
                    inspect_inventory_leaf(&value.format);
                    inspect_page_selection(value.pages);
                    inspect_inventory_leaf(&value.bleed);
                    inspect_inventory_leaf(&value.pretty);
                    let _ = value.status;
                }
                tp::compilation::CompilationOutputInventoryView::Html(value) => {
                    inspect_inventory_leaf(&value.format);
                    inspect_inventory_leaf(&value.pretty);
                    let _ = value.status;
                }
            },
            tp::compilation::CompilationRequestInventoryEntryView::Diagnostics(value) => {
                let _ = value.effective_policy;
                inspect_inventory_leaf(&value.version);
                inspect_inventory_leaf(&value.max_entries);
                inspect_inventory_leaf(&value.max_canonical_entry_bytes);
                let _ = value.status;
            }
            tp::compilation::CompilationRequestInventoryEntryView::Engine {
                identity,
                origin,
                status,
            } => {
                let _ = identity.as_str();
                let _ = (origin, status);
            }
            tp::compilation::CompilationRequestInventoryEntryView::Exporter {
                identity,
                origin,
                status,
            } => {
                let _ = identity.as_str();
                let _ = (origin, status);
            }
            tp::compilation::CompilationRequestInventoryEntryView::InvalidDeclaration {
                role,
                origin,
                status,
                declaration_ordinal,
                issues,
                referenced_safe_inventory_ordinal,
            } => {
                let _ = (
                    role,
                    origin,
                    status,
                    declaration_ordinal,
                    issues,
                    referenced_safe_inventory_ordinal,
                );
            }
        }
    }
}

fn inspect_trace(trace: tp::compilation::CompilationAccessTraceView<'_>) {
    let _ = trace.reached_scope();
    for observation in trace.observations() {
        let _ = observation.originating_evidence;
        match observation.semantic {
            tp::compilation::CompilationAccessObservationKindView::ProjectBaselineRead {
                path,
                request_kind,
                object,
            } => {
                let _ = (path, request_kind, object);
            }
            tp::compilation::CompilationAccessObservationKindView::ProjectOverrideRead {
                path,
                request_kind,
                replacement,
            } => {
                let _ = (path, request_kind, replacement);
            }
            tp::compilation::CompilationAccessObservationKindView::ProjectLogicalMissing {
                path,
                request_kind,
            } => {
                let _ = (path, request_kind);
            }
            tp::compilation::CompilationAccessObservationKindView::ProjectBaselineInvalidAsSource {
                path,
                object,
            } => {
                let _ = (path, object);
            }
            tp::compilation::CompilationAccessObservationKindView::ProjectOverrideInvalidAsSource {
                path,
                replacement,
            } => {
                let _ = (path, replacement);
            }
            tp::compilation::CompilationAccessObservationKindView::PackageRead {
                requirement,
                path,
                request_kind,
                object,
                fulfillment,
            } => {
                let _ = (requirement, path, request_kind, object, fulfillment);
            }
            tp::compilation::CompilationAccessObservationKindView::PackageLogicalMissing {
                requirement,
                path,
                request_kind,
                fulfillment,
            } => {
                let _ = (requirement, path, request_kind, fulfillment);
            }
            tp::compilation::CompilationAccessObservationKindView::PackageInvalidAsSource {
                requirement,
                path,
                object,
                fulfillment,
            } => {
                let _ = (requirement, path, object, fulfillment);
            }
            tp::compilation::CompilationAccessObservationKindView::UndeclaredPackage {
                specification,
                path,
                request_kind,
            } => {
                let _ = (specification, path, request_kind);
            }
            tp::compilation::CompilationAccessObservationKindView::FontFace {
                container,
                face_index,
                fulfillment,
            } => {
                let _ = (container, face_index, fulfillment);
            }
        }
    }
}

fn inspect_domain_selection(selection: tp::compilation::EngineRuntimeDomainSelectionView<'_>) {
    match selection {
        tp::compilation::EngineRuntimeDomainSelectionView::NotSelected => {}
        tp::compilation::EngineRuntimeDomainSelectionView::InheritedUnmanaged => {}
        tp::compilation::EngineRuntimeDomainSelectionView::Managed {
            identity,
            width,
            fine_timing_lease_reached,
        } => {
            let _ = (identity.as_str(), width, fine_timing_lease_reached);
        }
    }
}

fn inspect_compilation_contract(
    prepared: &tp::PreparedCompilation,
    rejection: &tp::compilation::CompilationRequestRejection,
    report: &tp::CompilationReport,
    limits: &CompilationResourceLimits,
) {
    let _ = prepared.engine_neutral_intent_identity().as_str();
    let _ = prepared.identity().as_str();
    let _ = prepared.engine_identity().as_str();
    let _ = prepared.exporter_identity().as_str();
    inspect_request_inventory(prepared.request_inventory());
    inspect_request_inventory(rejection.request_inventory());
    let _ = rejection.issues().count();
    inspect_request_inventory(report.request_inventory());
    let operational = report.operational_inventory();
    let admission = operational.admission();
    let _ = (
        admission.requested_trust,
        admission.admitted_trust,
        admission.requested_network,
        admission.admitted_network,
        admission.contractual_no_network,
        admission.structural_network_enforcement,
        admission.enforcement,
        admission.requested_capability_scopes,
        admission.admitted_capability_scopes,
        admission.requested_execution_placement,
        admission.admitted_execution_placement,
        admission.requested_isolation,
        admission.admitted_isolation,
    );
    let resources = operational.resources();
    let _ = (resources.profile, resources.requested, resources.admitted);
    let dependencies = operational.dependency_execution();
    let _ = (
        dependencies.packages,
        dependencies.fonts,
        dependencies.cache_descriptor,
        dependencies.cache_policy,
        dependencies.cache_lookup,
        dependencies.cache_isolation_domain_present,
        dependencies.offline_roles_covered,
        dependencies.concurrency,
        dependencies.reached_package_scope,
        dependencies.reached_font_scope,
        dependencies.reached_cache_scope,
    );
    let attempt = operational.attempt_control();
    let _ = (
        attempt.deadline,
        attempt.cancellation_present,
        attempt.monotonic_domain,
        attempt.queue_timeout_ticks,
        attempt.latency_target_ticks,
        attempt.session_supersession,
        attempt.requested_interruption,
        attempt.admitted_interruption,
        attempt.winner,
    );
    match operational.role_execution() {
        tp::compilation::CompilationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
            reached_placement,
            reached_isolation,
        } => {
            inspect_domain_selection(domain);
            let _ = (engine_width, reached_placement, reached_isolation);
        }
        tp::compilation::CompilationExecutionInventoryView::Facility {
            descriptor,
            domain,
            capacity,
            queue_reached,
            dispatch_reached,
            worker_terminated,
            worker_reaped,
            engine_width,
            reached_scope,
            reached_placement,
            reached_isolation,
        } => {
            let _ = descriptor.class();
            inspect_domain_selection(domain);
            let _ = (
                capacity,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
                engine_width,
                reached_scope,
                reached_placement,
                reached_isolation,
            );
        }
    }
    let reporting = operational.reporting();
    let _ = (
        reporting.requested,
        reporting.admitted,
        reporting.diagnostic_projection,
        reporting.diagnostic_sources,
        reporting.timing,
        reporting.fine_engine_timing,
        reporting.fine_timing_lease_reached,
        reporting.reached_scope,
    );
    for evidence in report.current_attempt_evidence().entries() {
        let _ = evidence.ordinal.ordinal();
        let _ = evidence.subject;
        let _ = evidence.kind;
        let _ = evidence.key;
    }
    match report.originating_evidence() {
        tp::compilation::OriginatingEvidenceAvailabilityView::Available(table) => {
            let _ = table.entries().count();
        }
        tp::compilation::OriginatingEvidenceAvailabilityView::Unavailable => {}
    }
    match report.access_trace() {
        tp::compilation::CompilationReportAccessTraceView::NotReached => {}
        tp::compilation::CompilationReportAccessTraceView::ResultOwned(trace)
        | tp::compilation::CompilationReportAccessTraceView::Partial(trace) => {
            inspect_trace(trace);
        }
    }
    if let Some(result) = report.result() {
        let _ = result.identity().as_str();
        inspect_trace(result.access_trace());
        for artifact in result.artifacts() {
            let _ = artifact.role;
            let _ = artifact.artifact_identity.as_str();
            let _ = artifact.content_identity.as_str();
            let _ = artifact.exact_bytes;
            let _ = artifact.bytes.len();
        }
        let diagnostics = result.diagnostics();
        let _ = diagnostics.policy();
        let _ = diagnostics.completion();
        let _ = diagnostics.entries().count();
    }

    let disclosure = tp::compilation::CompilationReportDisclosure::identity()
        .with_canonical_diagnostics(
            tp::compilation::CanonicalDiagnosticsDisclosureCapability::explicitly_granted_by_caller(
            ),
        )
        .with_canonical_evidence(
            tp::compilation::CanonicalEvidenceDisclosureCapability::explicitly_granted_by_caller(),
        )
        .with_diagnostic_sources(
            tp::compilation::DiagnosticSourcesDisclosureCapability::explicitly_granted_by_caller(),
        )
        .with_request_values(
            tp::compilation::RequestValuesDisclosureCapability::explicitly_granted_by_caller(),
        )
        .with_override_bytes(
            tp::compilation::OverrideBytesDisclosureCapability::explicitly_granted_by_caller(),
        )
        .with_backing_locators(
            tp::compilation::BackingLocatorsDisclosureCapability::explicitly_granted_by_caller(),
        )
        .with_adapter_detail(
            tp::compilation::AdapterDetailDisclosureCapability::explicitly_granted_by_caller(),
        );
    let projection = disclosure.project(report, limits);
    let terminal_projection = disclosure.project_terminal(
        &tp::compilation::CompilationTerminal::Report(report.clone()),
        limits,
    );
    match terminal_projection {
        tp::compilation::CompilationTerminalProjection::RequestRejected(value) => {
            inspect_request_inventory(value.request_inventory());
            let _ = value.issues().count();
        }
        tp::compilation::CompilationTerminalProjection::Report(value) => {
            let _ = value.terminal();
        }
    }
    let _ = projection.terminal();
    let _ = projection.compilation_identity();
    let _ = projection.result_identity();
    let _ = projection.artifact_identities().count();
    let _ = projection.diagnostic_summary();
    let _ = projection.canonical_diagnostics_status();
    let _ = projection.canonical_diagnostics().count();
    let _ = projection.canonical_evidence_status();
    let _ = projection.canonical_evidence();
    let _ = projection.diagnostic_sources_status();
    let _ = projection.diagnostic_sources().count();
    let _ = projection.request_values_status();
    let _ = projection.request_values().count();
    let _ = projection.override_bytes_status();
    let _ = projection.override_bytes().count();
    let _ = projection.backing_locators_status();
    let _ = projection.backing_locators().count();
    let _ = projection.adapter_detail_status();
    let _ = projection.adapter_detail().count();
}

fn typecheck_postcommit_cache<C>(
    report: tp::CompilationReport,
    cache: &C,
    clock: &dyn MonotonicClock,
    interruption: &dyn InterruptionSource,
) where
    C: SyncSemanticResultCache + tp::compilation::AsyncSemanticResultCache,
{
    let _ = tp::compilation::admit_to_cache_sync(report.clone(), cache, clock, interruption);
    let _future = tp::compilation::admit_to_cache_async(report, cache, clock, interruption);
}

fn inspect_creation_report(report: &tp::creation::CreationReport) {
    if let tp::creation::CreationTerminalRef::Failed(failure) = report.terminal() {
        let _ = failure.phase;
        match &failure.kind {
            tp::creation::CreationFailureKind::Authority(value) => {
                let _ = (value.class(), value.code(), value.safe_message());
            }
            tp::creation::CreationFailureKind::SourceChanged(value) => {
                let _ = value.safe_code();
            }
            tp::creation::CreationFailureKind::RevalidationFailed(value) => {
                let _ = value.safe_code();
            }
            tp::creation::CreationFailureKind::InsufficientEvidenceCapability(value) => {
                let _ = (value.required(), value.available());
            }
            tp::creation::CreationFailureKind::ResourceLimit
            | tp::creation::CreationFailureKind::Cancelled
            | tp::creation::CreationFailureKind::Deadline
            | tp::creation::CreationFailureKind::ReplayDrift
            | tp::creation::CreationFailureKind::InternalIntegrity => {}
            tp::creation::CreationFailureKind::Execution(value) => {
                let _ = value;
            }
        }
    }
    let inventory = report.operational_inventory();
    let admission = inventory.admission();
    let _ = (
        admission.requested_trust,
        admission.admitted_trust,
        admission.requested_network,
        admission.admitted_network,
        admission.contractual_no_network,
        admission.structural_network_enforcement,
        admission.enforcement,
        admission.requested_capability_scopes,
        admission.admitted_capability_scopes,
        admission.requested_execution_placement,
        admission.admitted_execution_placement,
        admission.requested_isolation,
        admission.admitted_isolation,
    );
    let resources = inventory.resources();
    let _ = (
        resources.profile,
        resources.requested,
        resources.admitted,
        resources.reached.aggregate_file_bindings,
        resources.reached.aggregate_logical_bytes,
        resources.reached.peak_stable_spool_bytes,
        resources.reached.peak_retained_memory_bytes,
    );
    let dependencies = inventory.dependency_execution();
    let _ = (
        dependencies.evidence,
        dependencies.packages,
        dependencies.fonts,
        dependencies.offline_roles_covered,
        dependencies.concurrency,
        dependencies.font_scan_policy,
        dependencies.reached_evidence_scope,
        dependencies.reached_package_scope,
        dependencies.reached_font_scope,
    );
    let attempt = inventory.attempt_control();
    let _ = (
        attempt.deadline,
        attempt.cancellation_present,
        attempt.monotonic_domain,
        attempt.queue_timeout_ticks,
        attempt.latency_target_ticks,
        attempt.requested_interruption,
        attempt.admitted_interruption,
        attempt.winner,
    );
    match inventory.role_execution() {
        tp::creation::CreationExecutionInventoryView::CallerThread {
            domain,
            engine_width,
            reached_placement,
            reached_isolation,
        } => {
            inspect_domain_selection(domain);
            let _ = (engine_width, reached_placement, reached_isolation);
        }
        tp::creation::CreationExecutionInventoryView::Facility {
            descriptor,
            domain,
            capacity,
            queue_reached,
            dispatch_reached,
            worker_terminated,
            worker_reaped,
            engine_width,
            reached_scope,
            reached_placement,
            reached_isolation,
        } => {
            let _ = descriptor.class();
            inspect_domain_selection(domain);
            let _ = (
                capacity,
                queue_reached,
                dispatch_reached,
                worker_terminated,
                worker_reaped,
                engine_width,
                reached_scope,
                reached_placement,
                reached_isolation,
            );
        }
    }
    let reporting = inventory.reporting();
    let _ = (
        reporting.requested,
        reporting.admitted,
        reporting.timing,
        reporting.fine_engine_timing,
        reporting.fine_timing_lease_reached,
    );
    let _ = report.phases().count();
    let _ = report.diagnostics().count();
    let _ = report.reporting().timing();
    let _ = report.reporting().fine_engine_timing();
}

fn inspect_format_receipt(common: tp::representation::FormatReceiptCommonView<'_>) {
    let _ = common.contract_version();
    let _ = common.role();
    let _ = common.terminal();
    let _ = common.stage();
    inspect_format_accounting(common.accounting());
    let _ = common.pack_exposed();
    let _ = common.stable_value_completed();
    match common.representation_admission() {
        None => {}
        Some(tp::representation::RepresentationAdmissionDispositionView::Refused(refusal)) => {
            inspect_format_controls(refusal.requested_controls());
            let _ = refusal.reason();
        }
        Some(tp::representation::RepresentationAdmissionDispositionView::Admitted(record)) => {
            inspect_format_controls(record.requested);
            inspect_format_controls(record.admitted);
        }
    }
    inspect_format_publication(common.publication());
    inspect_format_cleanup(common.cleanup_status());
    let _ = common.timing();
    let _ = common.adapter_class();
    let _ = common.failure_class();
    let _ = common.failure_cause();
    let _ = common.validation_rules().count();
}

fn inspect_format_accounting(accounting: tp::representation::FormatReceiptAccountingView<'_>) {
    match accounting {
        tp::representation::FormatReceiptAccountingView::PackArchive {
            logical,
            physical,
            occupancy,
            input_bytes,
            planned_output_bytes,
            produced_output_bytes,
            completed_output_bytes,
        } => {
            let _ = (
                logical.file_bindings,
                logical.decoded_bytes,
                physical.blob_count,
                physical.blob_bytes,
                physical.representation_entries,
                occupancy.peak_stable_spool_bytes,
                occupancy.peak_retained_memory_bytes,
                input_bytes,
                planned_output_bytes,
                produced_output_bytes,
                completed_output_bytes,
            );
        }
        tp::representation::FormatReceiptAccountingView::ClosureExport {
            logical,
            physical,
            occupancy,
            planned_payload_bytes,
            produced_payload_bytes,
            completed_payload_bytes,
        } => {
            let _ = (
                logical.file_bindings,
                physical.blob_count,
                occupancy.peak_stable_spool_bytes,
                planned_payload_bytes,
                produced_payload_bytes,
                completed_payload_bytes,
            );
        }
        tp::representation::FormatReceiptAccountingView::ProjectMaterialization {
            file_count,
            planned_output_bytes,
            produced_output_bytes,
            completed_output_bytes,
            occupancy,
        } => {
            let _ = (
                file_count,
                planned_output_bytes,
                produced_output_bytes,
                completed_output_bytes,
                occupancy.peak_retained_memory_bytes,
            );
        }
        tp::representation::FormatReceiptAccountingView::Publication => {}
    }
}

fn inspect_format_publication(status: tp::representation::FormatPublicationStatus) {
    match status {
        tp::representation::FormatPublicationStatus::NotApplicable => {}
        tp::representation::FormatPublicationStatus::NotStarted {
            requested,
            admitted,
        } => {
            let _ = (requested, admitted);
        }
        tp::representation::FormatPublicationStatus::Committed {
            requested,
            admitted,
            actual,
        } => {
            let _ = (requested, admitted, actual);
        }
        tp::representation::FormatPublicationStatus::Failed {
            requested,
            admitted,
            actual,
        } => {
            let _ = (requested, admitted, actual);
        }
    }
}

fn inspect_format_cleanup(status: tp::representation::FormatCleanupStatus) {
    match status {
        tp::representation::FormatCleanupStatus::NotApplicable => {}
        tp::representation::FormatCleanupStatus::NotReached {
            requested_cleanup_requirement,
            admitted_cleanup_requirement,
        } => {
            let _ = (requested_cleanup_requirement, admitted_cleanup_requirement);
        }
        tp::representation::FormatCleanupStatus::Reached {
            requested_cleanup_requirement,
            admitted_cleanup_requirement,
            cleanup_outcome,
        } => {
            let _ = (
                requested_cleanup_requirement,
                admitted_cleanup_requirement,
                cleanup_outcome,
            );
        }
    }
}

fn inspect_format_controls(controls: tp::representation::FormatReceiptControlsView<'_>) {
    let _ = (
        controls.trust,
        controls.network,
        controls.resource_profile,
        controls.deadline,
        controls.cancellation_present,
        controls.interruption,
        controls.publication_strength,
        controls.cleanup_requirement,
        controls.enforcement,
        controls.timing_requested,
        controls.timing_reporting,
    );
    match controls.limits {
        tp::representation::FormatReceiptLimitsView::PackIngress(value) => {
            let _ = value.spec();
        }
        tp::representation::FormatReceiptLimitsView::PackArchiveEncoding(value) => {
            let _ = value.output_bytes;
        }
        tp::representation::FormatReceiptLimitsView::ClosureExport(value) => {
            let _ = value.payload_bytes;
        }
        tp::representation::FormatReceiptLimitsView::ProjectMaterialization(value) => {
            let _ = (value.files, value.output_bytes);
        }
        tp::representation::FormatReceiptLimitsView::Transport(value) => {
            let _ = value.aggregate_bytes();
        }
    }
}

fn inspect_representation_reports(
    archive_read: &tp::representation::PackArchiveReadReport,
    closure_import: &tp::representation::ClosureExportImportReport,
    archive_encoding: &tp::representation::PackArchiveEncodingReport,
    closure_projection: &tp::representation::ClosureExportProjectionReport,
    materialization: &tp::representation::ProjectMaterializationReport,
) {
    inspect_format_receipt(archive_read.receipt().common());
    let _ = archive_read.receipt().input_archive_identity();
    let _ = archive_read.receipt().expected_archive_identity();
    let _ = archive_read.receipt().expected_archive_matched();
    let _ = archive_read.receipt().control_record_identity();
    let _ = archive_read.receipt().derived_pack_identity();
    let _ = archive_read.receipt().expected_pack_identity();
    let _ = archive_read.receipt().expected_pack_matched();
    let _ = archive_read.receipt().verification_mode();
    let _ = archive_read.receipt().asserted_archive_encoding_identity();
    match archive_read.receipt().encoding_assertion() {
        tp::representation::ArchiveEncodingAssertionStatus::NotAsserted
        | tp::representation::ArchiveEncodingAssertionStatus::SuppliedButUnevaluated
        | tp::representation::ArchiveEncodingAssertionStatus::ExternallyAssertedAndByteVerified
        | tp::representation::ArchiveEncodingAssertionStatus::ExternallyAssertedAndByteMismatched =>
            {}
    }
    inspect_format_receipt(closure_import.receipt().common());
    let _ = closure_import.receipt().control_record_identity();
    let _ = closure_import.receipt().derived_pack_identity();
    let _ = closure_import.receipt().expected_pack_identity();
    let _ = closure_import.receipt().expected_pack_matched();
    let _ = closure_import.receipt().closure_export_tree_identity();
    let _ = closure_import.receipt().verification_mode();
    let _ = closure_import.receipt().files();
    inspect_format_receipt(archive_encoding.receipt().common());
    let _ = archive_encoding.receipt().control_record_identity();
    let _ = archive_encoding.receipt().source_pack_identity();
    let _ = archive_encoding.receipt().archive_encoding_identity();
    let _ = archive_encoding.receipt().output_archive_identity();
    let _ = archive_encoding.receipt().closure_export_tree_identity();
    if let Some(receipt) = archive_encoding.spool_receipt() {
        let _ = receipt.status();
        inspect_spool_transport_state(receipt.state());
        let _ = receipt.expected_content_identity();
        let _ = receipt.actual_content_identity();
    }
    inspect_format_receipt(closure_projection.receipt().common());
    let _ = closure_projection.receipt().control_record_identity();
    let _ = closure_projection.receipt().source_pack_identity();
    let _ = closure_projection.receipt().closure_export_tree_identity();
    let _ = closure_projection.receipt().files();
    inspect_format_receipt(materialization.receipt().common());
    let _ = materialization.receipt().pack_identity();
    let _ = materialization.receipt().files().count();
}

fn inspect_transport_refusal(value: tp::transport::TransportAdmissionRefusalView<'_>) {
    let _ = (
        value.stage,
        value.requested_trust,
        value.resource_profile,
        value.requested_limits,
        value.requested_network,
        value.covered_roles,
        value.contractual_no_network,
        value.requested_structural_network_enforcement,
        value.requested_concurrency,
        value.requested_commit,
        value.requested_cleanup_requirement,
        value.interruption,
        value.cancellation_present,
        value.monotonic_domain,
        value.required_enforcement,
        value.timing_requested,
        value.deadline,
        value.reason,
    );
}

fn inspect_transport_admission(value: tp::transport::TransportAdmissionRecordView<'_>) {
    let _ = (
        value.requested_trust,
        value.admitted_trust,
        value.resource_profile,
        value.requested_limits,
        value.admitted_limits,
        value.requested_network,
        value.admitted_network,
        value.covered_roles,
        value.contractual_no_network,
        value.requested_structural_network_enforcement,
        value.admitted_structural_network_enforcement,
        value.requested_concurrency,
        value.admitted_concurrency,
        value.concurrency_constraints,
        value.requested_commit,
        value.admitted_commit,
        value.requested_cleanup_requirement,
        value.admitted_cleanup_requirement,
        value.requested_interruption,
        value.admitted_interruption,
        value.cancellation_present,
        value.monotonic_domain,
        value.enforcement,
        value.timing_requested,
        value.timing_reporting_admitted,
        value.deadline,
    );
}

macro_rules! role_transport_state_inspector {
    ($function:ident, $state:ident) => {
        fn $function(state: tp::transport::$state<'_>) {
            match state {
                tp::transport::$state::Refused(value) => {
                    let _ = value.descriptor.class();
                    inspect_transport_refusal(value.common);
                }
                tp::transport::$state::Admitted {
                    admission,
                    stage_ledger,
                } => {
                    let _ = admission.descriptor.class();
                    inspect_transport_admission(admission.common);
                    inspect_transport_ledger(stage_ledger);
                }
            }
        }
    };
}

role_transport_state_inspector!(
    inspect_spool_transport_state,
    SpoolTransportReceiptStateView
);
role_transport_state_inspector!(
    inspect_acquisition_transport_state,
    PackArchiveAcquisitionTransportReceiptStateView
);
role_transport_state_inspector!(
    inspect_archive_publication_transport_state,
    PackArchivePublicationTransportReceiptStateView
);
role_transport_state_inspector!(
    inspect_materialization_transport_state,
    ProjectMaterializationPublicationTransportReceiptStateView
);
role_transport_state_inspector!(
    inspect_closure_publication_transport_state,
    ClosureExportPublicationTransportReceiptStateView
);
role_transport_state_inspector!(
    inspect_delivery_transport_state,
    CompilationDeliveryTransportReceiptStateView
);

fn inspect_transport_ledger(ledger: tp::transport::TransportStageLedgerView<'_>) {
    let _ = ledger.stages().count();
    let _ = ledger.primary_terminal_stage();
    let _ = ledger.object_count();
    let _ = ledger.transferred_bytes();
    let _ = ledger.actual_commit_strength();
    let _ = ledger.cleanup_outcome();
    let _ = ledger.residual_locator();
    let _ = ledger.exposed_bytes();
    let _ = ledger.timing().status;
    let _ = ledger.structural_network_enforcement_reached();
    let _ = ledger.enforcement_reached();
    let _ = ledger.interruption_winner();
}

#[allow(clippy::too_many_arguments)]
fn typecheck_representation_operations(
    admission: &OrdinaryAdmission,
    archive: StableByteValue,
    expected_archive: tp::ContentIdentity,
    expected_pack: tp::pack::PackIdentity,
    closure_input: tp::representation::ClosureExportInput,
    archive_controls: tp::representation::PackIngressControls<'_>,
    closure_controls: tp::representation::PackIngressControls<'_>,
    pack: &tp::Pack,
    encode_controls: tp::representation::PackArchiveEncodingControls<'_>,
    closure_projection_controls: tp::representation::ClosureExportControls<'_>,
    materialization_controls: tp::representation::ProjectMaterializationControls<'_>,
    spool: &mut MemorySpool,
) {
    let unregistered = tp::representation::ArchiveEncodingIdentity::parse(
        admission,
        "org.example/permanently-unregistered/1",
    )
    .unwrap();
    let _unsupported_assertion = tp::representation::PackArchiveReadExpectations::new(
        tp::representation::PackIdentityVerificationMode::Verify(expected_pack.clone()),
    )
    .with_asserted_archive_encoding_identity(unregistered);
    let archive_expectations = tp::representation::PackArchiveReadExpectations::new(
        tp::representation::PackIdentityVerificationMode::Verify(expected_pack.clone()),
    )
    .with_expected_archive_content_identity(expected_archive)
    .with_asserted_archive_encoding_identity(
        tp::representation::ArchiveEncodingIdentity::epoch_2_all_stored_v1(),
    );
    let archive_report =
        tp::representation::read_pack_archive(archive, archive_expectations, archive_controls);
    inspect_format_receipt(archive_report.receipt().common());

    let closure_expectations = tp::representation::ClosureExportImportExpectations::new(
        tp::representation::PackIdentityVerificationMode::Verify(expected_pack),
    );
    let closure_report = tp::representation::import_closure_export(
        closure_input,
        closure_expectations,
        closure_controls,
    );
    inspect_format_receipt(closure_report.receipt().common());

    let encoding = tp::representation::encode_pack_archive(
        pack,
        tp::representation::ArchiveEncodingIdentity::epoch_2_all_stored_v1(),
        spool,
        encode_controls,
    );
    inspect_format_receipt(encoding.receipt().common());
    let closure = tp::representation::plan_closure_export(pack, closure_projection_controls);
    inspect_format_receipt(closure.receipt().common());
    if let Ok(plan) = closure.terminal() {
        let _ = (plan.entry_count(), plan.payload_bytes());
        for entry in plan.entries() {
            match entry.role {
                tp::representation::ClosureExportEntryRole::ControlRecord
                | tp::representation::ClosureExportEntryRole::Blob => {}
            }
        }
    }
    let materialization =
        tp::representation::plan_project_materialization(pack, materialization_controls);
    let _ = materialization.receipt().files().count();
    if let Ok(plan) = materialization.terminal() {
        let _ = (plan.file_count(), plan.output_bytes());
    }
    let _ = admission;
}

fn typecheck_unsupported_archive_encode(
    admission: &OrdinaryAdmission,
    pack: &tp::Pack,
    spool: &mut MemorySpool,
    controls: tp::representation::PackArchiveEncodingControls<'_>,
) {
    let encoding = tp::representation::ArchiveEncodingIdentity::parse(
        admission,
        "org.example/permanently-unregistered/1",
    )
    .unwrap();
    let report = tp::representation::encode_pack_archive(pack, encoding, spool, controls);
    inspect_format_receipt(report.receipt().common());
}

fn inspect_publication_composition(
    archive: &tp::transport::PackArchivePublicationOutcome,
    closure: &tp::transport::ClosureExportPublicationOutcome,
    materialization: &tp::transport::ProjectMaterializationPublicationOutcome,
) {
    inspect_format_receipt(archive.format().common());
    match archive.format().publication_admission() {
        tp::representation::PublicationFormatAdmissionDispositionView::Refused { transport } => {
            inspect_transport_refusal(transport);
        }
        tp::representation::PublicationFormatAdmissionDispositionView::Admitted { transport } => {
            inspect_transport_admission(transport);
        }
    }
    let _ = archive.format().source_pack_identity();
    let _ = archive.format().source_archive_identity();
    let _ = archive.format().output_archive_identity();
    let _ = archive.format().archive_encoding_identity();
    let _ = archive.format().source_tree_identity();
    let _ = archive.format().entries();
    let archive_receipt = archive.transport().receipt();
    let _ = archive_receipt.status();
    inspect_archive_publication_transport_state(archive_receipt.state());
    let _ = archive_receipt.source_archive_identity();
    let _ = archive_receipt.output_archive_identity();
    let _ = archive_receipt.archive_encoding_identity();
    inspect_format_receipt(closure.format().common());
    match closure.format().publication_admission() {
        tp::representation::PublicationFormatAdmissionDispositionView::Refused { transport } => {
            inspect_transport_refusal(transport);
        }
        tp::representation::PublicationFormatAdmissionDispositionView::Admitted { transport } => {
            inspect_transport_admission(transport);
        }
    }
    let _ = closure.format().source_pack_identity();
    let _ = closure.format().source_tree_identity();
    let _ = closure.format().output_tree_identity();
    let _ = closure.format().files();
    let closure_receipt = closure.transport().receipt();
    let _ = closure_receipt.status();
    inspect_closure_publication_transport_state(closure_receipt.state());
    let _ = closure_receipt.pack_identity();
    let _ = closure_receipt.source_tree_identity();
    let _ = closure_receipt.output_tree_identity();
    let materialization_receipt = materialization.transport().receipt();
    let _ = materialization_receipt.status();
    inspect_materialization_transport_state(materialization_receipt.state());
    let _ = materialization_receipt.pack_identity();
}

fn typecheck_creation_descriptors(
    admission: &OrdinaryAdmission,
    limits: &AdmittedOperationResourceLimits<CreationResourceLimits>,
    project: ProjectSnapshot,
) {
    let metadata = tp::pack::PackMetadata::try_new(
        admission,
        limits,
        Some("Example".into()),
        Some("Portable project".into()),
        ["Example Author".into()],
        ["example".into(), "portable".into()],
    )
    .unwrap();
    let annotation_id =
        tp::pack::PackAnnotationIdentifier::parse(admission, limits, "org.example.build.ann")
            .unwrap();
    let annotation = tp::pack::PackAnnotation::try_new(
        admission,
        limits,
        Some(0),
        annotation_id.clone(),
        std::num::NonZeroU32::new(1).unwrap(),
        Arc::from(&b"annotation"[..]),
    )
    .unwrap();
    let request = CreationRequest::try_new(
        limits,
        project,
        [DiscoveryVariant::paged_explicit_empty()],
        PackageEmbeddingPolicy::embed_all(),
        FontEmbeddingPolicy::embed_all(),
        metadata,
        [annotation],
    )
    .unwrap();
    let _ = CreationInputEvidence::caller_owned_immutable(&request);
    let _ = tp::creation::CreationEvidenceSubject::PackMetadata;
    let _ = tp::creation::CreationEvidenceSubject::PackAnnotationInventory;
    let _ = tp::creation::CreationEvidenceSubject::PackAnnotation {
        identifier: annotation_id,
    };
}

fn typecheck_rejectable_compilation_declarations(
    admission: &OrdinaryAdmission,
    limits: &AdmittedOperationResourceLimits<CompilationResourceLimits>,
    bytes: StableByteValue,
) {
    let output = tp::compilation::CompilationOutputDeclarations {
        format: tp::compilation::CompilationOutputFormatDeclaration {
            declaration_ordinal: 0,
            value: "png".into(),
            origin: tp::compilation::CompilationInventoryOrigin::CallerSupplied,
        },
        controls: vec![
            tp::compilation::CompilationOutputControlDeclaration::PageRange {
                declaration_ordinal: 1,
                start: 0,
                end: 0,
                origin: tp::compilation::CompilationInventoryOrigin::CallerSupplied,
            },
            tp::compilation::CompilationOutputControlDeclaration::PixelsPerInch {
                declaration_ordinal: 2,
                value: f64::NAN,
                origin: tp::compilation::CompilationInventoryOrigin::CallerSupplied,
            },
            tp::compilation::CompilationOutputControlDeclaration::PdfTagging {
                declaration_ordinal: 3,
                value: tp::compilation::PdfTagging::Enabled,
                origin: tp::compilation::CompilationInventoryOrigin::CallerSupplied,
            },
        ],
    };
    let diagnostics = tp::compilation::CanonicalDiagnosticPolicyDeclarations {
        members: vec![
            tp::compilation::CanonicalDiagnosticPolicyDeclaration::Version {
                declaration_ordinal: 4,
                value: 1,
                origin: tp::compilation::CompilationInventoryOrigin::AdapterResolved,
            },
            tp::compilation::CanonicalDiagnosticPolicyDeclaration::MaxEntries {
                declaration_ordinal: 5,
                value: 5_000,
                origin: tp::compilation::CompilationInventoryOrigin::AdapterResolved,
            },
            tp::compilation::CanonicalDiagnosticPolicyDeclaration::MaxCanonicalEntryBytes {
                declaration_ordinal: 6,
                value: 8 * 1024 * 1024,
                origin: tp::compilation::CompilationInventoryOrigin::AdapterResolved,
            },
        ],
    };
    let request = CompilationRequest::from_declarations(
        admission,
        limits,
        output,
        diagnostics,
        [
            tp::compilation::CompilationRequestDeclaration::PackOverride {
                declaration_ordinal: 0,
                path: "../invalid".into(),
                bytes,
                origin: tp::compilation::CompilationInventoryOrigin::CallerSupplied,
            },
        ],
    );
    let _ = request.diagnostics();
}

fn capability_class(value: &str) -> tp::OperationalCapabilityClass {
    tp::OperationalCapabilityClass::try_new(value).unwrap()
}

fn package_scope() -> tp::authority::PackageAuthorityCapabilityScopeProjection {
    tp::authority::PackageAuthorityCapabilityScopeProjection {
        permitted_uses: vec![
            tp::authority::AuthorityPermittedUse::Resolution,
            tp::authority::AuthorityPermittedUse::Acquisition,
            tp::authority::AuthorityPermittedUse::Revalidation,
        ],
        coverage: tp::authority::PackageAuthorityCoverageClass::DeclaredDependencyRequirements,
        completeness: tp::CapabilityProjectionCompleteness::Complete,
    }
}

fn font_scope() -> FontAuthorityCapabilityScopeProjection {
    FontAuthorityCapabilityScopeProjection {
        permitted_uses: vec![
            tp::authority::AuthorityPermittedUse::Resolution,
            tp::authority::AuthorityPermittedUse::Acquisition,
            tp::authority::AuthorityPermittedUse::Revalidation,
        ],
        coverage: tp::authority::FontAuthorityCoverageClass::DeclaredDependencyRequirements,
        completeness: tp::CapabilityProjectionCompleteness::Complete,
    }
}

fn authority_capability_spec(class: &str) -> tp::authority::AuthorityCapabilitySpec {
    tp::authority::AuthorityCapabilitySpec {
        class: capability_class(class),
        ordered_source_classes: vec![tp::authority::AcquisitionSourceClass::CallerSupplied],
        evidence: tp::authority::AuthorityEvidenceCapabilities {
            immutable_values: true,
            exact_key_revalidation: true,
            opaque_scope_revalidation: true,
            polling: true,
            push_subscription: true,
            cursor_replay: true,
        },
        network: tp::SelectedNetworkContract::NoNetwork,
        resolution_cache: tp::authority::AuthorityCachePolicy::Disabled,
        private_caches: vec![],
        offered_scope: package_scope(),
    }
}

fn package_descriptor(
    identity: &AuthorityInstanceIdentity,
    class: &str,
) -> PackageAuthorityCapabilityDescriptor {
    PackageAuthorityCapabilityDescriptor::try_new(
        identity.clone(),
        authority_capability_spec(class),
    )
    .unwrap()
}

fn font_descriptor(
    identity: &AuthorityInstanceIdentity,
    class: &str,
) -> FontAuthorityCapabilityDescriptor {
    let base = authority_capability_spec(class);
    FontAuthorityCapabilityDescriptor::try_new(
        identity.clone(),
        FontAuthorityCapabilitySpec {
            class: base.class,
            ordered_source_classes: base.ordered_source_classes,
            evidence: base.evidence,
            network: base.network,
            resolution_cache: base.resolution_cache,
            private_caches: base.private_caches,
            supported_font_scan_policies: vec![tp::authority::FontScanPolicy {
                invalid_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
                unreadable_candidate: tp::authority::InvalidFontCandidateDisposition::WarnAndOmit,
            }],
            offered_scope: font_scope(),
        },
    )
    .unwrap()
}

fn creation_evidence_descriptor(
    identity: &AuthorityInstanceIdentity,
) -> CreationEvidenceCapabilityDescriptor {
    CreationEvidenceCapabilityDescriptor::try_new(
        identity.clone(),
        capability_class("org.example/creation-evidence/1"),
        tp::creation::CreationEvidenceCapabilityProjection {
            stability: tp::creation::CreationEvidenceStability::Immutable,
            race_closing_revalidation: true,
            exact_key_revalidation: true,
            opaque_scope_revalidation: true,
            polling: true,
            push_subscription: true,
            cursor_replay: true,
            network: tp::SelectedNetworkContract::NoNetwork,
        },
        tp::creation::CreationEvidenceCapabilityScopeProjection {
            permitted_uses: vec![
                tp::creation::CreationEvidencePermittedUse::Stabilization,
                tp::creation::CreationEvidencePermittedUse::Revalidation,
                tp::creation::CreationEvidencePermittedUse::Subscription,
            ],
            coverage: tp::creation::CreationEvidenceCoverageClass::ExactOperationInputs,
            completeness: tp::CapabilityProjectionCompleteness::Complete,
        },
    )
    .unwrap()
}

fn engine_domain_policy() -> tp::compilation::EngineRuntimeDomainPolicyDescriptor {
    tp::compilation::EngineRuntimeDomainPolicyDescriptor::try_new(
        tp::compilation::EngineRuntimeDomainPolicySpec {
            class: capability_class("org.example/engine-domain/1"),
            managed: true,
            supported_placements: vec![tp::ExecutionPlacement::InProcessFacility],
            width_policy: tp::EngineWidthRequest::Exact(NonZeroUsize::new(1).unwrap()),
            sharing_scope: capability_class("org.example/engine-sharing/1"),
            exclusive_fine_timing_lease: true,
        },
    )
    .unwrap()
}

fn reporting_descriptor() -> tp::compilation::ReportingCapabilityDescriptor {
    tp::compilation::ReportingCapabilityDescriptor::try_new(
        capability_class("org.example/reporting/1"),
        tp::compilation::ReportingCapabilityScopeProjection {
            permitted_uses: vec![
                tp::compilation::ReportingPermittedUse::DiagnosticProjection,
                tp::compilation::ReportingPermittedUse::DiagnosticSourceBundle,
                tp::compilation::ReportingPermittedUse::Timing,
                tp::compilation::ReportingPermittedUse::FineEngineTiming,
            ],
            coverage: tp::compilation::ReportingCoverageClass::SelectedReportChannels,
            completeness: tp::CapabilityProjectionCompleteness::Complete,
        },
    )
    .unwrap()
}

fn typecheck_capability_descriptors() {
    let packages_identity = AuthorityInstanceIdentity::try_new("test.package-authority").unwrap();
    let packages = PackageAuthorityCapabilityDescriptor::try_new(
        packages_identity,
        authority_capability_spec("org.example/package-authority/1"),
    )
    .unwrap();
    let fonts_identity = AuthorityInstanceIdentity::try_new("test.font-authority").unwrap();
    let fonts = font_descriptor(&fonts_identity, "org.example/font-authority/1");
    let evidence = CreationEvidenceCapabilityDescriptor::try_new(
        AuthorityInstanceIdentity::try_new("test.creation-evidence").unwrap(),
        capability_class("org.example/creation-evidence/1"),
        tp::creation::CreationEvidenceCapabilityProjection {
            stability: tp::creation::CreationEvidenceStability::Immutable,
            race_closing_revalidation: true,
            exact_key_revalidation: true,
            opaque_scope_revalidation: true,
            polling: true,
            push_subscription: true,
            cursor_replay: true,
            network: tp::SelectedNetworkContract::NoNetwork,
        },
        tp::creation::CreationEvidenceCapabilityScopeProjection {
            permitted_uses: vec![
                tp::creation::CreationEvidencePermittedUse::Stabilization,
                tp::creation::CreationEvidencePermittedUse::Revalidation,
                tp::creation::CreationEvidencePermittedUse::Subscription,
            ],
            coverage: tp::creation::CreationEvidenceCoverageClass::ExactOperationInputs,
            completeness: tp::CapabilityProjectionCompleteness::Complete,
        },
    )
    .unwrap();
    let cache = SemanticResultCacheCapabilityDescriptor::try_new(
        CacheIsolationDomain::try_new("test.cache").unwrap(),
        tp::compilation::SemanticResultCacheCapabilitySpec {
            class: capability_class("org.example/semantic-result-cache/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            trusted_writer_domain: true,
            authenticated_records: false,
            required_availability: true,
            continue_on_unavailable: true,
            offered_scope: tp::compilation::SemanticCacheCapabilityScopeProjection {
                permitted_uses: vec![
                    tp::compilation::SemanticCachePermittedUse::Lookup,
                    tp::compilation::SemanticCachePermittedUse::Admission,
                ],
                coverage: tp::compilation::SemanticCacheCoverageClass::OneIsolationDomain,
                completeness: tp::CapabilityProjectionCompleteness::Complete,
            },
        },
    )
    .unwrap();
    let creation_facility = tp::creation::CreationExecutionFacilityCapabilityDescriptor::try_new(
        tp::creation::CreationExecutionFacilityCapabilitySpec {
            class: capability_class("org.example/creation-facility/1"),
            capacity_scope_class: capability_class("org.example/shared-engine-pool/1"),
            shared_with_compilation: true,
            supported_placements: vec![tp::ExecutionPlacement::InProcessFacility],
            ready_job_capacity: NonZeroUsize::new(2).unwrap(),
            queue_capacity: 2,
            worker_capacity: None,
            overlapping_jobs_per_worker: false,
            domain_policy: engine_domain_policy(),
            execution_network: tp::SelectedNetworkContract::NoNetwork,
            worker_control_network: None,
            interruption: tp::OperationInterruptionStrength::Cooperative,
            worker_protocol: None,
            worker_protocol_version: None,
            parent_verifies_response: true,
            parent_withholds_output: true,
            no_in_process_fallback: true,
            terminate_and_reap: false,
            forced_termination_target_ticks: None,
            enforcement: vec![],
            offered_scope: tp::creation::CreationExecutionCapabilityScopeProjection {
                permitted_uses: vec![
                    tp::creation::CreationExecutionPermittedUse::InProcessDispatch,
                ],
                coverage: tp::creation::CreationExecutionCoverageClass::ReadyJobs,
                completeness: tp::CapabilityProjectionCompleteness::Complete,
            },
        },
    )
    .unwrap();
    let compilation_facility = CompilationExecutionFacilityCapabilityDescriptor::try_new(
        tp::compilation::CompilationExecutionFacilityCapabilitySpec {
            class: capability_class("org.example/compilation-facility/1"),
            capacity_scope_class: capability_class("org.example/shared-engine-pool/1"),
            shared_with_creation: true,
            supported_placements: vec![tp::ExecutionPlacement::InProcessFacility],
            ready_job_capacity: NonZeroUsize::new(2).unwrap(),
            queue_capacity: 2,
            worker_capacity: None,
            overlapping_jobs_per_worker: false,
            domain_policy: engine_domain_policy(),
            execution_network: tp::SelectedNetworkContract::NoNetwork,
            worker_control_network: None,
            interruption: tp::OperationInterruptionStrength::Cooperative,
            worker_protocol: None,
            worker_protocol_version: None,
            parent_verifies_response: true,
            parent_withholds_output: true,
            no_in_process_fallback: true,
            terminate_and_reap: false,
            forced_termination_target_ticks: None,
            enforcement: vec![],
            offered_scope: tp::compilation::CompilationExecutionCapabilityScopeProjection {
                permitted_uses: vec![
                    tp::compilation::CompilationExecutionPermittedUse::InProcessDispatch,
                ],
                coverage: tp::compilation::CompilationExecutionCoverageClass::ReadyJobs,
                completeness: tp::CapabilityProjectionCompleteness::Complete,
            },
        },
    )
    .unwrap();
    let archive_acquirer = PackArchiveAcquirerCapabilityDescriptor::try_new(
        tp::transport::PackArchiveAcquirerCapabilitySpec {
            class: capability_class("org.example/archive-acquirer/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            supported_cleanup_requirements: vec![
                tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            ],
            interruption: tp::OperationInterruptionStrength::Cooperative,
            enforcement: vec![],
            timing_reporting: true,
            offered_scope: transport_scope(
                tp::transport::TransportFacilityRole::PackArchiveAcquisition,
                tp::transport::TransportPermittedUse::ArchiveAcquisition,
            ),
        },
    )
    .unwrap();
    let archive_publisher = tp::transport::PackArchivePublisherCapabilityDescriptor::try_new(
        tp::transport::PackArchivePublisherCapabilitySpec {
            class: capability_class("org.example/archive-publisher/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            commit_strengths: vec![PublicationCommitStrength::CompleteCollectionAtomic],
            supported_cleanup_requirements: vec![
                tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            ],
            interruption: tp::OperationInterruptionStrength::Cooperative,
            enforcement: vec![],
            timing_reporting: true,
            offered_scope: transport_scope(
                tp::transport::TransportFacilityRole::PackArchivePublication,
                tp::transport::TransportPermittedUse::ArchivePublication,
            ),
        },
    )
    .unwrap();
    let closure_publisher = tp::transport::ClosureExportPublisherCapabilityDescriptor::try_new(
        tp::transport::ClosureExportPublisherCapabilitySpec {
            class: capability_class("org.example/closure-publisher/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            commit_strengths: vec![PublicationCommitStrength::CompleteCollectionAtomic],
            supported_cleanup_requirements: vec![
                tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            ],
            interruption: tp::OperationInterruptionStrength::Cooperative,
            enforcement: vec![],
            timing_reporting: true,
            offered_scope: transport_scope(
                tp::transport::TransportFacilityRole::ClosureExportPublication,
                tp::transport::TransportPermittedUse::ClosureExportPublication,
            ),
        },
    )
    .unwrap();
    let materialization =
        tp::transport::ProjectMaterializationPublisherCapabilityDescriptor::try_new(
            tp::transport::ProjectMaterializationPublisherCapabilitySpec {
                class: capability_class("org.example/materialization-publisher/1"),
                network: tp::SelectedNetworkContract::NoNetwork,
                transfer_concurrency: NonZeroUsize::new(1).unwrap(),
                commit_strengths: vec![PublicationCommitStrength::CompleteCollectionAtomic],
                supported_cleanup_requirements: vec![
                    tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
                ],
                interruption: tp::OperationInterruptionStrength::Cooperative,
                enforcement: vec![],
                timing_reporting: true,
                offered_scope: transport_scope(
                    tp::transport::TransportFacilityRole::ProjectMaterializationPublication,
                    tp::transport::TransportPermittedUse::MaterializationPublication,
                ),
            },
        )
        .unwrap();
    let delivery = CompilationDeliveryCapabilityDescriptor::try_new(
        tp::transport::CompilationDeliveryCapabilitySpec {
            class: capability_class("org.example/compilation-delivery/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            commit_strengths: vec![PublicationCommitStrength::CompleteCollectionAtomic],
            supported_cleanup_requirements: vec![
                tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            ],
            interruption: tp::OperationInterruptionStrength::Cooperative,
            enforcement: vec![],
            timing_reporting: true,
            offered_scope: transport_scope(
                tp::transport::TransportFacilityRole::CompilationDelivery,
                tp::transport::TransportPermittedUse::CompilationDelivery,
            ),
        },
    )
    .unwrap();
    let spool = tp::transport::SpoolFacilityCapabilityDescriptor::try_new(
        tp::transport::SpoolFacilityCapabilitySpec {
            class: capability_class("org.example/spool/1"),
            network: tp::SelectedNetworkContract::NoNetwork,
            transfer_concurrency: NonZeroUsize::new(1).unwrap(),
            supported_cleanup_requirements: vec![
                tp::transport::TransportCleanupRequirement::CompleteBeforeReturn,
            ],
            interruption: tp::OperationInterruptionStrength::Cooperative,
            enforcement: vec![],
            timing_reporting: true,
            offered_scope: transport_scope(
                tp::transport::TransportFacilityRole::Spool,
                tp::transport::TransportPermittedUse::StableAcquisition,
            ),
        },
    )
    .unwrap();
    let cache = TestCache { descriptor: cache };
    let creation_facility = TestCreationFacility {
        descriptor: creation_facility,
    };
    let compilation_facility = TestCompilationFacility {
        descriptor: compilation_facility,
    };
    let archive_acquirer = TestArchiveAcquirer {
        descriptor: archive_acquirer,
    };
    let archive_publisher = TestArchivePublisher {
        descriptor: archive_publisher,
    };
    let closure_publisher = TestClosurePublisher {
        descriptor: closure_publisher,
    };
    let materialization = TestMaterializationPublisher {
        descriptor: materialization,
    };
    let delivery = TestDelivery {
        descriptor: delivery,
    };
    let creation_capabilities = creation_facility.descriptor().capabilities();
    let compilation_capabilities = compilation_facility.descriptor().capabilities();
    let _ = (
        creation_capabilities.execution_network,
        creation_capabilities.worker_control_network,
        compilation_capabilities.execution_network,
        compilation_capabilities.worker_control_network,
    );
    let _ = (
        archive_acquirer.descriptor().class(),
        archive_acquirer.descriptor().network(),
        archive_acquirer.descriptor().transfer_concurrency(),
        archive_acquirer
            .descriptor()
            .supported_cleanup_requirements(),
        archive_acquirer.descriptor().interruption(),
        archive_acquirer.descriptor().enforcement(),
        archive_acquirer.descriptor().timing_reporting(),
        archive_publisher.descriptor.commit_strengths(),
        closure_publisher.descriptor.commit_strengths(),
        materialization.descriptor.commit_strengths(),
        delivery.descriptor().commit_strengths(),
        spool.class(),
    );
    let _ = (
        packages.class(),
        fonts.class(),
        evidence.class(),
        cache.descriptor().class(),
        creation_facility.descriptor().class(),
        compilation_facility.descriptor().class(),
        archive_acquirer,
        archive_publisher,
        closure_publisher,
        materialization,
        delivery,
        spool,
    );
}

fn inspect_creation_rejection(rejection: &tp::creation::CreationRequestRejection) {
    let _ = (
        rejection.resource_profile(),
        rejection.requested_limits().spec(),
        rejection.admitted_limits().spec(),
    );
    for issue in rejection.issues() {
        let _ = (issue.code, issue.role, issue.declaration_ordinal);
    }
}

fn typecheck_session_inputs(
    admission: OrdinaryAdmission,
    pack: &tp::Pack,
    request: CompilationRequest,
    limits: CompilationResourceLimits,
) {
    let preparation =
        tp::session::SessionPreparationLimits::try_caller_selected(limits.clone()).unwrap();
    let policy = tp::session::SessionPolicy::latest_only_complete_coverage(preparation);
    let _ = policy.mode();
    let stabilized = tp::session::StabilizedSessionInput::try_new(
        request,
        tp::session::SessionRequestEvidence::caller_owned_immutable(),
        policy,
    )
    .unwrap();
    let mut session = tp::CompilationSession::new(admission, pack.clone());
    inspect_session_transition(session.apply(SessionEvent::Accept(
        tp::session::AcceptedSessionInput::Stabilized(stabilized),
    )));

    let failure_policy = tp::session::SessionPolicy::latest_only_allow_unverified(
        tp::session::SessionPreparationLimits::try_caller_selected(limits).unwrap(),
    );
    let scope = tp::session::EvidenceScope::try_new("org.example/request-source").unwrap();
    let failure = tp::session::SessionIngestionFailure::try_new(
        "org.example.ingestion-failure",
        [scope.clone()],
        failure_policy,
    )
    .unwrap();
    inspect_session_transition(session.apply(SessionEvent::Accept(
        tp::session::AcceptedSessionInput::IngestionFailure(failure),
    )));

    let provider = AuthorityInstanceIdentity::try_new("test.session-provider").unwrap();
    let zero = SessionProviderObservations::try_new([]).unwrap();
    let binding =
        tp::session::RequestSourceEvidenceBinding::try_new(scope.clone(), provider.clone());
    let _ = binding.scope().as_str();
    let _ = tp::session::SessionRequestEvidence::revalidatable([binding]).unwrap();
    let affected = tp::session::SessionAffectedScopes::try_new([scope.clone()], []).unwrap();
    let _ = affected.request_sources().count();
    let _ = affected.dependencies().count();
    let fence_failure = tp::session::SessionFenceFailure::try_new(
        tp::session::SessionAffectedScopes::try_new([scope], []).unwrap(),
        "org.example.fence-failure",
    )
    .unwrap();
    let _ = fence_failure.scopes();
    let _ = fence_failure.safe_code();
    let currentness = tp::session::SessionCurrentness::CurrentAsOfPoll { observations: zero };
    inspect_session_currentness(&currentness);
    inspect_session_view(session.view());
}

fn inspect_session_currentness(currentness: &tp::session::SessionCurrentness) {
    match currentness {
        tp::session::SessionCurrentness::CurrentThroughPush { observations }
        | tp::session::SessionCurrentness::CurrentAsOfPoll { observations } => {
            inspect_provider_observations(observations);
        }
        tp::session::SessionCurrentness::Unverified { uncovered } => {
            let _ = uncovered.request_sources().count();
            let _ = uncovered.dependencies().count();
        }
        tp::session::SessionCurrentness::Stale { dirty } => {
            let _ = dirty.request_sources().count();
            let _ = dirty.dependencies().count();
        }
    }
}

fn inspect_session_view(view: tp::session::SessionView<'_>) {
    let _ = view.session_instance().as_str();
    let _ = view.lifecycle();
    let _ = view.latest_revision();
    let _ = view.latest_evaluation();
    if let Some(active) = view.active_attempt() {
        let _ = active.token.session_instance();
        let _ = active.token.evaluation();
        let _ = active.token.ordinal();
        let _ = (active.revision, active.evaluation, active.state);
    }
    if let Some(pending) = view.pending_revision() {
        let _ = (
            pending.revision,
            pending.evaluation,
            pending.prepared_identity,
        );
    }
    if let Some(publication) = view.publication() {
        let _ = publication.session_instance().as_str();
        let _ = publication.sequence().ordinal();
        let _ = publication.revision();
        let _ = publication.evaluation();
        inspect_session_currentness(publication.currentness());
        match publication.terminal() {
            tp::session::SessionPublicationTerminalRef::RequestRejected(rejection) => {
                inspect_request_inventory(rejection.request_inventory());
            }
            tp::session::SessionPublicationTerminalRef::Report(report) => {
                let _ = report.compilation_identity();
            }
            tp::session::SessionPublicationTerminalRef::IngestionFailure(failure) => {
                let _ = failure.safe_code();
                let _ = failure.failed_request_sources().count();
                let _ = failure.policy().preparation_limits().admitted();
            }
        }
    }
    if let Some(success) = view.last_successful() {
        let _ = (
            success.revision,
            success.evaluation,
            success.publication_sequence,
            success.result,
        );
        inspect_session_currentness(success.currentness);
    }
}

fn execute_session_plan<
    P: SyncPackageAuthority + ?Sized,
    F: SyncFontAuthority + ?Sized,
    C: SyncSemanticResultCache + ?Sized,
>(
    plan: tp::session::SessionAttemptPlan,
    controls: SyncCompilationControls<'_, P, F, C>,
) {
    let _ = plan.run_sync(controls);
}

fn inspect_remaining_transport_roles(
    acquisition: &tp::transport::PackArchiveAcquisitionOutcome,
    delivery: &tp::transport::CompilationDeliveryOutcome,
) {
    let acquired = acquisition.receipt();
    inspect_acquisition_transport_state(acquired.state());
    let _ = acquired.expected_archive_identity();
    let _ = acquired.acquired_archive_identity();

    let delivered = delivery.transport().receipt();
    inspect_delivery_transport_state(delivered.state());
    let _ = delivered.compilation_identity();
    let _ = delivered.result_identity();
    let _ = delivered.artifact_identities().count();
}

fn typecheck_registry_ordering() {
    let mut standards = [
        tp::compilation::PdfStandard::PdfUa1,
        tp::compilation::PdfStandard::PdfA3A,
        tp::compilation::PdfStandard::PdfA1B,
        tp::compilation::PdfStandard::Pdf14,
    ];
    standards.sort();
    for (index, standard) in standards.into_iter().enumerate() {
        let _ = (index, standard.registry_value());
    }
    let built_in = tp::representation::ArchiveEncodingIdentity::epoch_2_all_stored_v1();
    let _ = built_in == built_in.clone();
}

fn main() {}
