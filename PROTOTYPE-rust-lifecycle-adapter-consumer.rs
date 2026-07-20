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
    AcquiredDependency, AcquisitionControls, AcquisitionProvenance,
    AsyncPackageAuthority, AuthorityCapabilities, AuthorityFailure, AuthorityFailureClass,
    CompletePackageTree, DependencyAcquisitionOutcome, DependencyEvidenceKey,
    DependencyResolutionEvidence, EvidenceFactKind, EvidenceFactOutcome, EvidenceFence,
    EvidenceRevalidationOutcome, EvidenceRevalidationRequest, FontCatalogCandidate,
    FontCatalogAcquisition, FontCatalogRequest, FontCatalogSnapshot, FontContainerAcquisitionIdentity,
    FontContainerAcquisitionRequest, FontAxis, FontAxisValue, FontSelectionFlags, FontStretch,
    FontStyle, FontWeight, OpenTypeAxisTag, PackageAcquisitionRequest, SyncFontAuthority,
    SyncPackageAuthority, UnicodeCodepointRange,
};
use tp::compilation::{
    CanonicalDiagnosticPolicy, CompilationDispatchOutcome,
    CompilationExecutionFacility, CompilationReportTerminalRef, CompilationReportingPolicy,
    CompilationRequest, CompilationResourceLimitSpec, CompilationResourceLimits,
    CompilationExecutionFacilityCapacity, EngineRuntimeDomainDescriptor,
    EngineRuntimeDomainIdentity, EngineRuntimeDomainPlacement, NoSemanticResultCache,
    ReadyCompilationJob, SemanticCacheAdmissionOutcome,
    SemanticCacheAdmissionRequest, SemanticCacheLookupOutcome, SemanticCacheLookupRequest,
    SyncCompilationControls, SyncSemanticCacheLookup, SyncSemanticResultCache,
};
use tp::creation::{
    CreationDispatchOutcome, CreationEvidenceCapabilities, CreationEvidenceFenceOutcome,
    CreationEvidenceFenceRequest, CreationExecutionFacility, CreationExecutionFacilityCapacity,
    CreationInput, CreationInputEvidence, CreationReportingPolicy, CreationRequest,
    CreationResourceLimits, DiscoveryVariant, FontEmbeddingPolicy,
    PackageEmbeddingPolicy, ProjectSnapshot, ReadyCreationJob, SyncCreationControls,
    SyncCreationEvidence,
};
use tp::session::{
    ArmedSubscriptions, FenceConfirmation, FenceConfirmationOutcome, FenceReadObservation,
    FenceReadOutcome, RequestSourceObservation, SessionEffect, SessionEvent, SessionWatchCoverage,
    SubscriptionArmOutcome,
};
use tp::transport::{
    AcquisitionTransportControls, MemorySpool,
    PublicationCommitStrength, SyncCompilationDelivery, SyncPackArchiveAcquirer, SyncSpoolFacility,
    TransportCleanupOutcome, TransportControls, TransportOutcome, TransportReceipt, TransportStage,
};
use tp::{
    AdmittedOperationResourceLimits, AuthorityInstanceIdentity, CacheIsolationDomain,
    CanonicalIdentity, DeploymentTrustProfile, InterruptionSource,
    MonotonicClock, MonotonicInstant, MonotonicTimeDomain, OperationDeadline, OrdinaryAdmission,
    PackagePath, ProjectPath, StableByteValue,
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

struct TestArchiveAcquirer;

impl SyncPackArchiveAcquirer for TestArchiveAcquirer {
    type Locator = str;

    fn acquire(
        &self,
        _locator: &Self::Locator,
        controls: AcquisitionTransportControls<'_>,
    ) -> TransportOutcome<StableByteValue> {
        let bytes = StableByteValue::from_static(controls.admission(), b"archive").unwrap();
        let receipt = TransportReceipt::try_new_pack_archive_acquisition(
            &controls,
            "test.archive-acquirer",
            TransportStage::Transferred,
            bytes.len(),
            Some(bytes.content_identity().clone()),
            TransportCleanupOutcome::NotRequired,
            tp::transport::TransportTimingInput::NotRequested,
        )
        .unwrap();
        TransportOutcome::try_new(Ok(bytes), receipt).unwrap()
    }
}

struct TestCache {
    isolation: CacheIsolationDomain,
}

impl SyncSemanticResultCache for TestCache {
    fn isolation_domain(&self) -> &CacheIsolationDomain {
        &self.isolation
    }

    fn lookup(&self, request: SemanticCacheLookupRequest<'_>) -> SemanticCacheLookupOutcome {
        let _reservation = request.controls.budget().reserve_record(1).unwrap();
        SemanticCacheLookupOutcome::Miss
    }

    fn admit(&self, request: SemanticCacheAdmissionRequest<'_>) -> SemanticCacheAdmissionOutcome {
        let _reservation = request.controls.budget().reserve_retained(1).unwrap();
        let _ = request.record.bytes();
        SemanticCacheAdmissionOutcome::Admitted
    }
}

struct TestDelivery;

impl SyncCompilationDelivery for TestDelivery {
    type Destination = str;

    fn deliver(
        &self,
        transfer: tp::compilation::CompilationDeliveryTransfer<'_>,
        _destination: &Self::Destination,
        controls: TransportControls<'_>,
    ) -> TransportOutcome<()> {
        let _ = transfer.projection().canonical_diagnostics_status();
        let _ = transfer.projection().canonical_diagnostics().count();
        let receipt = TransportReceipt::try_new_compilation_delivery(
            &controls,
            &transfer,
            "test.delivery",
            TransportStage::Complete,
            0,
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            tp::transport::TransportTimingInput::NotRequested,
        )
        .unwrap();
        TransportOutcome::try_new(Ok(()), receipt).unwrap()
    }
}

struct TestArchivePublisher;

impl tp::transport::SyncPackArchivePublisher for TestArchivePublisher {
    type Destination = str;

    fn publish(
        &self,
        archive: &tp::representation::EncodedPackArchive,
        _destination: &Self::Destination,
        controls: TransportControls<'_>,
    ) -> tp::transport::PackArchivePublicationOutcome {
        let receipt = TransportReceipt::try_new_pack_archive_publication(
            &controls,
            archive,
            "test.archive-publisher",
            TransportStage::Complete,
            archive.bytes().len(),
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            tp::transport::TransportTimingInput::NotRequested,
        )
        .unwrap();
        let transport = TransportOutcome::try_new(Ok(()), receipt).unwrap();
        tp::transport::PackArchivePublicationOutcome::try_new(archive, transport).unwrap()
    }
}

struct TestClosurePublisher;

impl tp::transport::SyncClosureExportPublisher for TestClosurePublisher {
    type Destination = str;

    fn publish(
        &self,
        plan: &tp::representation::ClosureExportPlan,
        _destination: &Self::Destination,
        controls: TransportControls<'_>,
    ) -> tp::transport::ClosureExportPublicationOutcome {
        let receipt = TransportReceipt::try_new_closure_export_publication(
            &controls,
            plan,
            "test.closure-publisher",
            TransportStage::Complete,
            plan.aggregate_bytes(),
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            tp::transport::TransportTimingInput::NotRequested,
        )
        .unwrap();
        let transport = TransportOutcome::try_new(Ok(()), receipt).unwrap();
        tp::transport::ClosureExportPublicationOutcome::try_new(plan, transport).unwrap()
    }
}

struct TestMaterializationPublisher;

impl tp::transport::SyncProjectMaterializationPublisher for TestMaterializationPublisher {
    type Destination = str;

    fn publish(
        &self,
        plan: &tp::representation::ProjectMaterializationPlan,
        _destination: &Self::Destination,
        controls: TransportControls<'_>,
    ) -> tp::transport::ProjectMaterializationPublicationOutcome {
        let transferred = plan.files().map(|file| file.bytes.len()).sum();
        let receipt = TransportReceipt::try_new_project_materialization_publication(
            &controls,
            plan,
            "test.materialization-publisher",
            TransportStage::Complete,
            transferred,
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
            tp::transport::TransportTimingInput::NotRequested,
        )
        .unwrap();
        let transport = TransportOutcome::try_new(Ok(()), receipt).unwrap();
        tp::transport::ProjectMaterializationPublicationOutcome::try_new(plan, transport).unwrap()
    }
}

fn assert_spool_traits<T: SyncSpoolFacility>() {}

fn typecheck_role_helpers() {
    assert_spool_traits::<MemorySpool>();
    let _ = TestCompilationFacility {
        domain: test_domain(),
    };
    let _ = TestCreationFacility {
        domain: test_domain(),
    };
    let _ = TestCache {
        isolation: CacheIsolationDomain::try_new("test.cache").unwrap(),
    };
    let _ = TestDelivery;
    let _ = TestArchivePublisher;
    let _ = TestClosurePublisher;
    let _ = TestMaterializationPublisher;
    let admission = OrdinaryAdmission::try_new(DeploymentTrustProfile::Trusted).unwrap();
    let _ = SuccessfulPackages {
        admission: admission.clone(),
        identity: AuthorityInstanceIdentity::try_new("test.success-packages").unwrap(),
    };
    let identity = AuthorityInstanceIdentity::try_new("test.success-fonts").unwrap();
    let _ = SuccessfulFonts {
        container: FontContainerAcquisitionIdentity::try_new(
            identity.clone(),
            Arc::from(&b"font-container-0"[..]),
        )
        .unwrap(),
        admission,
        identity,
    };
}

struct TestCompilationFacility {
    domain: EngineRuntimeDomainDescriptor,
}

impl CompilationExecutionFacility for TestCompilationFacility {
    type Dispatch<'a>
        = Ready<CompilationDispatchOutcome>
    where
        Self: 'a;

    fn domain(&self) -> &EngineRuntimeDomainDescriptor {
        &self.domain
    }

    fn capacity(&self) -> CompilationExecutionFacilityCapacity {
        CompilationExecutionFacilityCapacity {
            simultaneous_ready_jobs: NonZeroUsize::new(1).unwrap(),
            ready_queue: 1,
        }
    }

    fn dispatch<'a>(&'a self, _job: ReadyCompilationJob) -> Self::Dispatch<'a> {
        ready(CompilationDispatchOutcome::Refused)
    }
}

struct TestCreationFacility {
    domain: EngineRuntimeDomainDescriptor,
}

impl CreationExecutionFacility for TestCreationFacility {
    type Dispatch<'a>
        = Ready<CreationDispatchOutcome>
    where
        Self: 'a;

    fn domain(&self) -> &EngineRuntimeDomainDescriptor {
        &self.domain
    }

    fn capacity(&self) -> CreationExecutionFacilityCapacity {
        CreationExecutionFacilityCapacity {
            simultaneous_ready_jobs: NonZeroUsize::new(1).unwrap(),
            ready_queue: 1,
        }
    }

    fn dispatch<'a>(&'a self, _job: ReadyCreationJob) -> Self::Dispatch<'a> {
        ready(CreationDispatchOutcome::Refused)
    }
}

fn test_domain() -> EngineRuntimeDomainDescriptor {
    EngineRuntimeDomainDescriptor {
        identity: EngineRuntimeDomainIdentity::try_new("test.engine-domain").unwrap(),
        placement: EngineRuntimeDomainPlacement::ManagedInProcess,
        width: NonZeroUsize::new(1),
    }
}

struct TestPackages {
    identity: AuthorityInstanceIdentity,
}

impl SyncPackageAuthority for TestPackages {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: false,
            push_subscription: false,
            cursor_replay: false,
            offline: true,
        }
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
}

impl SyncFontAuthority for TestFonts {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: false,
            push_subscription: false,
            cursor_replay: false,
            offline: true,
        }
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
}

impl SuccessfulPackages {
    fn evidence(&self) -> DependencyResolutionEvidence {
        let key = DependencyEvidenceKey::try_new(
            self.identity.clone(),
            EvidenceFactKind::Content,
            Arc::from(&b"package"[..]),
            None,
        )
        .unwrap();
        let mut builder = DependencyResolutionEvidence::builder(self.identity.clone());
        builder.record(key, EvidenceFactOutcome::Selected).unwrap();
        builder.finish().unwrap()
    }
}

impl SyncPackageAuthority for SuccessfulPackages {
    fn instance_identity(&self) -> &AuthorityInstanceIdentity {
        &self.identity
    }

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: true,
            push_subscription: true,
            cursor_replay: true,
            offline: true,
        }
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
        let path = PackagePath::parse(&self.admission, "typst.toml").unwrap();
        let bytes = StableByteValue::from_static(&self.admission, b"[package]").unwrap();
        let tree = CompletePackageTree::try_from_files(&controls, [(path, bytes)]).unwrap();
        DependencyAcquisitionOutcome::Acquired(AcquiredDependency {
            value: tree,
            evidence: self.evidence(),
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
        _controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        for key in &request.keys {
            let _ = key.authority();
            let _ = key.kind();
            let _ = key.opaque_key();
            let _ = key.immutable_version();
        }
        EvidenceRevalidationOutcome::Clean(
            EvidenceFence::try_new(
                self.identity.clone(),
                request.keys,
                Arc::from(&b"generation-1"[..]),
                None,
            )
            .unwrap(),
        )
    }
}

struct SuccessfulFonts {
    admission: OrdinaryAdmission,
    identity: AuthorityInstanceIdentity,
    container: FontContainerAcquisitionIdentity,
}

impl SuccessfulFonts {
    fn acquired<T>(&self, value: T) -> DependencyAcquisitionOutcome<T> {
        let key = DependencyEvidenceKey::try_new(
            self.identity.clone(),
            EvidenceFactKind::Content,
            Arc::from(&b"font"[..]),
            None,
        )
        .unwrap();
        let mut builder = DependencyResolutionEvidence::builder(self.identity.clone());
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

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: true,
            push_subscription: true,
            cursor_replay: true,
            offline: true,
        }
    }

    fn catalog(
        &self,
        _request: FontCatalogRequest,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<FontCatalogAcquisition> {
        let axis = FontAxis::try_new(
            OpenTypeAxisTag::from_bytes(*b"wght"),
            FontAxisValue::try_from_be_bytes(100.0f32.to_be_bytes()).unwrap(),
            FontAxisValue::try_from_be_bytes(400.0f32.to_be_bytes()).unwrap(),
            FontAxisValue::try_from_be_bytes(900.0f32.to_be_bytes()).unwrap(),
        )
        .unwrap();
        let candidate = FontCatalogCandidate::try_new(
            &controls,
            self.container.clone(),
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
        let snapshot = FontCatalogSnapshot::try_new(
                &controls,
                self.identity.clone(),
                [candidate],
            )
            .unwrap();
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
        self.acquired(catalog)
    }

    fn acquire_container(
        &self,
        request: FontContainerAcquisitionRequest,
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<StableByteValue> {
        match request.purpose() {
            tp::authority::FontContainerAcquisitionPurpose::CatalogFace {
                acquisition_identity,
                face_index,
            } => {
                let _ = acquisition_identity.authority();
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
        self.acquired(StableByteValue::from_static(&self.admission, b"font").unwrap())
    }

    fn revalidate(
        &self,
        request: EvidenceRevalidationRequest,
        _controls: AcquisitionControls<'_>,
    ) -> EvidenceRevalidationOutcome {
        EvidenceRevalidationOutcome::Clean(
            EvidenceFence::try_new(
                self.identity.clone(),
                request.keys,
                Arc::from(&b"generation-1"[..]),
                None,
            )
            .unwrap(),
        )
    }
}

struct ImmutableCreationEvidence {
    identity: AuthorityInstanceIdentity,
}

impl SyncCreationEvidence for ImmutableCreationEvidence {
    fn provider_identity(&self) -> &AuthorityInstanceIdentity {
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
        _controls: AcquisitionControls<'_>,
    ) -> CreationEvidenceFenceOutcome {
        CreationEvidenceFenceOutcome::Clean(
            tp::creation::CreationEvidenceFence::try_new(self.identity.clone(), []).unwrap(),
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

    let creation_limits = CreationResourceLimits::try_new(
        10_000,
        512 * 1024 * 1024,
        128 * 1024 * 1024,
        64,
        256 * 1024 * 1024,
        128,
        512,
        256 * 1024 * 1024,
        16,
        128,
        100_000,
        512 * 1024 * 1024,
        4 * 1024 * 1024 * 1024,
        512 * 1024 * 1024,
        768 * 1024 * 1024,
    )
    .unwrap();

    let main = ProjectPath::parse(&admission, "main.typ").unwrap();
    let bytes = StableByteValue::from_static(&admission, b"Hello").unwrap();
    let project = ProjectSnapshot::try_from_files(
        &admission,
        &creation_limits,
        main.clone(),
        [(main, bytes)],
    )
    .unwrap();
    let request = CreationRequest::try_new(
        &creation_limits,
        project,
        [DiscoveryVariant::paged_explicit_empty()],
        PackageEmbeddingPolicy::embed_all(),
        FontEmbeddingPolicy::embed_all(),
        tp::pack::PackMetadata::empty(),
        [],
    )
    .unwrap();
    let input = CreationInput {
        evidence: CreationInputEvidence::caller_owned_immutable(&request),
        request,
    };

    let packages = TestPackages {
        identity: AuthorityInstanceIdentity::try_new("test.packages").unwrap(),
    };
    let fonts = TestFonts {
        identity: AuthorityInstanceIdentity::try_new("test.fonts").unwrap(),
    };
    let evidence = ImmutableCreationEvidence {
        identity: AuthorityInstanceIdentity::try_new("test.creation-evidence").unwrap(),
    };

    let package_trait: &dyn SyncPackageAuthority = &packages;
    let font_trait: &dyn SyncFontAuthority = &fonts;
    let evidence_trait: &dyn SyncCreationEvidence = &evidence;

    let creation_controls = SyncCreationControls::try_new(
        admission.clone(),
        AdmittedOperationResourceLimits::try_caller_selected(creation_limits.clone()).unwrap(),
        evidence_trait,
        package_trait,
        font_trait,
        NonZeroUsize::new(4).unwrap(),
        OperationDeadline::None,
        &clock,
        &interruption,
        CreationReportingPolicy {
            timing: false,
            fine_engine_timing: false,
        },
    )
    .unwrap();

    let creation_report = tp::creation::create_sync(input, creation_controls);
    let Ok(pack) = creation_report.into_pack() else {
        return;
    };

    let diagnostics = CanonicalDiagnosticPolicy::try_new(1, 5_000, 8 * 1024 * 1024).unwrap();
    let request = CompilationRequest::pdf(diagnostics);
    let compilation_limits = compilation_limits();
    let prepared = pack
        .prepare(&admission, &compilation_limits, request)
        .unwrap();
    let no_cache =
        NoSemanticResultCache::new(CacheIsolationDomain::try_new("test.cache-isolation").unwrap());
    let controls = SyncCompilationControls::try_new(
        admission,
        AdmittedOperationResourceLimits::try_caller_selected(compilation_limits).unwrap(),
        package_trait,
        font_trait,
        SyncSemanticCacheLookup::Disabled::<NoSemanticResultCache>,
        NonZeroUsize::new(4).unwrap(),
        OperationDeadline::None,
        &clock,
        Some(&interruption),
        CompilationReportingPolicy {
            diagnostic_projection: false,
            diagnostic_source_bundle: false,
            timing: false,
        },
    )
    .unwrap();

    let report = tp::compilation::run_sync(&prepared, controls);
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

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: false,
            push_subscription: false,
            cursor_replay: false,
            offline: true,
        }
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

    fn capabilities(&self) -> AuthorityCapabilities {
        AuthorityCapabilities {
            revalidation: false,
            push_subscription: false,
            cursor_replay: false,
            offline: true,
        }
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
    authority: &AuthorityInstanceIdentity,
    effect: SessionEffect,
) {
    match effect {
        SessionEffect::StartAttempt { plan, .. } => {
            let _ = plan.prepared.identity();
            let _ = plan.revision;
            let _ = plan.policy;
        }
        SessionEffect::InterruptAttempt { .. } => {}
        SessionEffect::ReadFence { token, plan } => {
            let mut builder =
                tp::authority::DependencyResolutionEvidence::builder(authority.clone());
            for key in plan.dependency_keys {
                builder
                    .record(key, tp::authority::EvidenceFactOutcome::Selected)
                    .unwrap();
            }
            let evidence = builder.finish().unwrap();
            let observations = plan.request_sources.into_iter().map(|scope| {
                RequestSourceObservation::try_new(
                    scope,
                    CanonicalIdentity::parse(admission, "test.request-source").unwrap(),
                    None,
                )
                .unwrap()
            });
            let observation = FenceReadObservation::try_new(observations, evidence, []).unwrap();
            let _ = session.apply(SessionEvent::FenceReadFinished {
                token,
                outcome: FenceReadOutcome::Read(observation),
            });
        }
        SessionEffect::ArmSubscriptions { token, plan } => {
            for interest in plan.interests() {
                let _ = interest.scope;
                let _ = interest.keys;
                let _ = interest.after;
            }
            let armed =
                ArmedSubscriptions::try_new(&plan, SessionWatchCoverage::complete_push(), [])
                    .unwrap();
            let _ = session.apply(SessionEvent::SubscriptionsArmed {
                token,
                outcome: SubscriptionArmOutcome::Armed(armed),
            });
        }
        SessionEffect::ConfirmFence { token, plan } => {
            let _ = plan.request_sources().count();
            let _ = plan.dependency_keys().count();
            let _ = plan.armed_cursors().count();
            let confirmation = FenceConfirmation::try_new(&plan, []).unwrap();
            let _ = session.apply(SessionEvent::FenceConfirmed {
                token,
                outcome: FenceConfirmationOutcome::Clean(confirmation),
            });
        }
        SessionEffect::RetireSubscriptions { .. } => {}
        SessionEffect::Publish { publication } => {
            let _ = publication.revision;
            let _ = publication.currentness;
        }
    }
}

fn inspect_complete_pack(pack: &tp::Pack) {
    let inspection = pack.inspect();
    let _ = inspection.identity();
    let _ = inspection.entrypoint();
    let engine = inspection.discovery_engine();
    let _ = engine.identity();
    let _ = engine.producer_id();
    let _ = engine.implementation_name();
    let _ = engine.implementation_version();
    let _ = engine.exact_build_fingerprint();
    let _ = engine.target_profile();
    let _ = engine.qualifiers().count();
    let _ = engine.unicode_xid_version();
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
    let _ = inspection.explicit_conditional_inclusions().count();

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
            let _ = override_.value.commitment();
        }
        for observation in variant.trace().project_observations() {
            match observation {
                tp::pack::DiscoveryProjectObservationInspection::BaselineRead { .. }
                | tp::pack::DiscoveryProjectObservationInspection::OverrideRead { .. }
                | tp::pack::DiscoveryProjectObservationInspection::Missing { .. } => {}
            }
        }
        for observation in variant.trace().package_observations() {
            match observation {
                tp::pack::DiscoveryPackageObservationInspection::Read { .. }
                | tp::pack::DiscoveryPackageObservationInspection::Missing { .. } => {}
            }
        }
        let _ = variant.trace().used_font_faces().count();
    }

    for package in inspection.package_requirements() {
        let _ = package.identity;
        let _ = package.specification;
        let _ = package.tree_identity;
        let _ = package.files;
        let _ = package.manifest.entrypoint;
        let _ = package.disposition;
        let _ = package.provenance.source_class;
    }
    for font in inspection.font_requirements() {
        let _ = font.identity;
        let _ = font.container.content_identity;
        let _ = font.disposition;
        let _ = font.provenance.source_class;
        let _ = font.observing_variants;
        for face in font.faces {
            let _ = face.identity.container_identity;
            let _ = face.identity.face_index;
            let _ = face.selection.family;
            let _ = face.selection.weight;
            let _ = face.selection.axes;
            let _ = face.selection.codepoint_coverage;
            let _ = face.licensing.fs_type;
            let _ = face.licensing.name_records;
        }
    }
    let _ = inspection.font_catalog().count();
    let _ = inspection.metadata().authors().count();
    for extension in inspection.semantic_extensions() {
        let _ = extension.identifier.as_str();
        let _ = extension.epoch;
        let _ = extension.canonical_payload;
        let _ = extension.required_objects;
    }
    for annotation in inspection.annotations() {
        let _ = annotation.identifier();
        let _ = annotation.epoch();
        let _ = annotation.payload();
    }
    let _ = inspection.guarantees();
}

fn inspect_request_inventory(inventory: tp::compilation::CompilationRequestInventoryView<'_>) {
    for entry in inventory.entries() {
        match entry {
            tp::compilation::CompilationRequestInventoryEntryView::Pack { identity, .. } => {
                let _ = identity.as_str();
            }
            tp::compilation::CompilationRequestInventoryEntryView::PackOverride {
                path,
                commitment,
                ..
            } => {
                let _ = path.as_str();
                let _ = commitment.map(|value| value.as_str());
            }
            tp::compilation::CompilationRequestInventoryEntryView::TypstInput {
                key,
                commitment,
                ..
            } => {
                let _ = key.as_str();
                let _ = commitment.as_str();
            }
            tp::compilation::CompilationRequestInventoryEntryView::DocumentTime { value, .. } => {
                let _ = value;
            }
            tp::compilation::CompilationRequestInventoryEntryView::Feature { value, .. } => {
                let _ = value.as_str();
            }
            tp::compilation::CompilationRequestInventoryEntryView::Target { value, .. } => {
                let _ = value;
            }
            tp::compilation::CompilationRequestInventoryEntryView::Output(output) => {
                match output {
                    tp::compilation::CompilationOutputInventoryView::Pdf(value) => {
                        let _ = value.format;
                        let _ = value.pages;
                        let _ = value.identifier;
                        let _ = value.creator;
                        let _ = value.creation_time;
                        let _ = value.standards;
                        let _ = value.tagging;
                        let _ = value.pretty;
                        let _ = value.status;
                    }
                    tp::compilation::CompilationOutputInventoryView::Png(value) => {
                        let _ = value.format;
                        let _ = value.pages;
                        let _ = value.pixels_per_inch;
                        let _ = value.bleed;
                        let _ = value.status;
                    }
                    tp::compilation::CompilationOutputInventoryView::Svg(value) => {
                        let _ = value.format;
                        let _ = value.pages;
                        let _ = value.bleed;
                        let _ = value.pretty;
                        let _ = value.status;
                    }
                    tp::compilation::CompilationOutputInventoryView::Html(value) => {
                        let _ = value.format;
                        let _ = value.pretty;
                        let _ = value.status;
                    }
                }
            }
            tp::compilation::CompilationRequestInventoryEntryView::Diagnostics { policy, .. } => {
                let _ = policy;
            }
            tp::compilation::CompilationRequestInventoryEntryView::Engine(identity) => {
                let _ = identity.as_str();
            }
            tp::compilation::CompilationRequestInventoryEntryView::Exporter(identity) => {
                let _ = identity.as_str();
            }
            tp::compilation::CompilationRequestInventoryEntryView::InvalidDeclaration {
                role,
                declaration_ordinal,
                issues,
            } => {
                let _ = role;
                let _ = declaration_ordinal;
                let _ = issues;
            }
        }
    }
}

fn inspect_trace(trace: tp::compilation::CompilationAccessTraceView<'_>) {
    let _ = trace.reached_scope();
    for observation in trace.observations() {
        let _ = observation.originating_evidence;
        match observation.semantic {
            tp::compilation::CompilationAccessObservationKindView::ProjectBaselineRead { .. }
            | tp::compilation::CompilationAccessObservationKindView::ProjectOverrideRead { .. }
            | tp::compilation::CompilationAccessObservationKindView::ProjectLogicalMissing { .. }
            | tp::compilation::CompilationAccessObservationKindView::ProjectBaselineInvalidAsSource { .. }
            | tp::compilation::CompilationAccessObservationKindView::ProjectOverrideInvalidAsSource { .. }
            | tp::compilation::CompilationAccessObservationKindView::PackageRead { .. }
            | tp::compilation::CompilationAccessObservationKindView::PackageLogicalMissing { .. }
            | tp::compilation::CompilationAccessObservationKindView::PackageInvalidAsSource { .. }
            | tp::compilation::CompilationAccessObservationKindView::UndeclaredPackage { .. }
            | tp::compilation::CompilationAccessObservationKindView::FontFace { .. } => {}
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
    for entry in report.attempt_inventory().entries() {
        match entry {
            tp::compilation::CompilationAttemptInventoryEntryView::Admission { .. }
            | tp::compilation::CompilationAttemptInventoryEntryView::Resources { .. }
            | tp::compilation::CompilationAttemptInventoryEntryView::DependencyExecution { .. }
            | tp::compilation::CompilationAttemptInventoryEntryView::AttemptControl { .. }
            | tp::compilation::CompilationAttemptInventoryEntryView::KernelExecution(_)
            | tp::compilation::CompilationAttemptInventoryEntryView::Reporting(_) => {}
        }
    }
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
            tp::compilation::CanonicalDiagnosticsDisclosureCapability::explicitly_granted_by_caller(),
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

fn inspect_creation_report(report: &tp::creation::CreationReport) {
    let inventory = report.operational_inventory();
    let _ = inventory.requested_trust();
    let _ = inventory.admitted_trust();
    let _ = inventory.resource_profile();
    let _ = inventory.requested_limits();
    let _ = inventory.requested_limits().spec();
    let _ = inventory.admitted_limits();
    let _ = inventory.admitted_limits().spec();
    let _ = inventory.acquisition_concurrency();
    match inventory.execution() {
        tp::creation::CreationExecutionInventoryView::CallerThread { domain } => {
            let _ = domain.identity.as_str();
        }
        tp::creation::CreationExecutionInventoryView::Facility { domain, capacity } => {
            let _ = domain;
            let _ = capacity.simultaneous_ready_jobs;
            let _ = capacity.ready_queue;
        }
    }
    let _ = inventory.deadline();
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
    let _ = common.counters();
    let _ = common.pack_exposed();
    let _ = common.stable_value_completed();
    let requested = common.requested_controls();
    let _ = requested.resource_profile;
    match requested.limits {
        tp::representation::FormatReceiptLimitsView::PackIngress(value) => {
            let _ = value.spec();
        }
        tp::representation::FormatReceiptLimitsView::Representation(value) => {
            let _ = value.output_bytes();
        }
        tp::representation::FormatReceiptLimitsView::Transport(value) => {
            let _ = value.aggregate_bytes();
        }
    }
    let _ = common.admitted_controls();
    let _ = common.publication();
    let _ = common.cleanup();
    let _ = common.timing();
    let _ = common.adapter_class();
    let _ = common.failure_class();
    let _ = common.failure_cause();
    let _ = common.validation_rules().count();
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
    let _ = archive_read.receipt().encoding_assertion();
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
    inspect_format_receipt(closure_projection.receipt().common());
    let _ = closure_projection.receipt().control_record_identity();
    let _ = closure_projection.receipt().source_pack_identity();
    let _ = closure_projection.receipt().closure_export_tree_identity();
    let _ = closure_projection.receipt().files();
    let _ = materialization.receipt().pack_identity();
    let _ = materialization.receipt().files().count();
}

fn inspect_transport_receipt(receipt: &TransportReceipt) {
    let _ = receipt.role();
    let _ = receipt.status();
    let _ = receipt.stage();
    let _ = receipt.object_count();
    let _ = receipt.transferred_bytes();
    let _ = receipt.identities().count();
    let _ = receipt.admission();
    let _ = receipt.cleanup();
    let _ = receipt.actual_commit();
    let _ = receipt.resource_profile();
    let _ = receipt.adapter_class();
    let _ = receipt.timing().status;
    match receipt.subject() {
        tp::transport::TransportReceiptSubjectRef::Spool { expected, actual } => {
            let _ = expected;
            let _ = actual;
        }
        tp::transport::TransportReceiptSubjectRef::PackArchiveAcquisition {
            expected_archive,
            acquired_archive,
        } => {
            let _ = expected_archive;
            let _ = acquired_archive;
        }
        tp::transport::TransportReceiptSubjectRef::PackArchivePublication {
            source_archive,
            output_archive,
            archive_encoding,
        } => {
            let _ = source_archive;
            let _ = output_archive;
            let _ = archive_encoding;
        }
        tp::transport::TransportReceiptSubjectRef::ProjectMaterializationPublication { pack } => {
            let _ = pack;
        }
        tp::transport::TransportReceiptSubjectRef::ClosureExportPublication {
            pack,
            source_tree,
            output_tree,
        } => {
            let _ = pack;
            let _ = source_tree;
            let _ = output_tree;
        }
        tp::transport::TransportReceiptSubjectRef::CompilationDelivery {
            compilation,
            result,
            artifacts,
        } => {
            let _ = compilation;
            let _ = result;
            let _ = artifacts;
        }
    }
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
    encode_controls: tp::representation::RepresentationControls<'_>,
    closure_projection_controls: tp::representation::RepresentationControls<'_>,
    materialization_controls: tp::representation::RepresentationControls<'_>,
    spool: &mut MemorySpool,
) {
    let archive_expectations = tp::representation::PackArchiveReadExpectations::new(
        tp::representation::PackIdentityVerificationMode::Verify(expected_pack.clone()),
    )
    .with_expected_archive_content_identity(expected_archive)
    .with_asserted_archive_encoding_identity(
        tp::representation::ArchiveEncodingIdentity::epoch_2_all_stored_v1(),
    );
    let archive_report = tp::representation::read_pack_archive(
        archive,
        archive_expectations,
        archive_controls,
    );
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
    let _ = admission;
}

fn inspect_publication_composition(
    archive: &tp::transport::PackArchivePublicationOutcome,
    closure: &tp::transport::ClosureExportPublicationOutcome,
    materialization: &tp::transport::ProjectMaterializationPublicationOutcome,
) {
    inspect_format_receipt(archive.format().common());
    let _ = archive.format().source_archive_identity();
    let _ = archive.format().output_archive_identity();
    let _ = archive.format().archive_encoding_identity();
    inspect_transport_receipt(archive.transport().receipt());
    inspect_format_receipt(closure.format().common());
    let _ = closure.format().source_pack_identity();
    let _ = closure.format().source_tree_identity();
    let _ = closure.format().output_tree_identity();
    let _ = closure.format().files();
    inspect_transport_receipt(closure.transport().receipt());
    inspect_transport_receipt(materialization.transport().receipt());
}

fn typecheck_creation_descriptors(
    admission: &OrdinaryAdmission,
    limits: &CreationResourceLimits,
    project: ProjectSnapshot,
) {
    let metadata = tp::pack::PackMetadata::try_new(
        admission,
        Some("Example".into()),
        Some("Portable project".into()),
        ["Example Author".into()],
        ["example".into(), "portable".into()],
    )
    .unwrap();
    let annotation_id =
        tp::pack::PackAnnotationIdentifier::parse(admission, "org.example.build.ann").unwrap();
    let annotation = tp::pack::PackAnnotation::try_new(
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
    limits: &CompilationResourceLimits,
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
        [tp::compilation::CompilationRequestDeclaration::PackOverride {
            declaration_ordinal: 0,
            path: "../invalid".into(),
            bytes,
        }],
    );
    let _ = request.diagnostics();
}

fn main() {}
