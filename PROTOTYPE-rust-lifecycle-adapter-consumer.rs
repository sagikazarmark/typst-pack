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
    AcquiredDependency, AcquisitionBudget, AcquisitionControls, AcquisitionProvenance,
    AsyncPackageAuthority, AuthorityCapabilities, AuthorityFailure, AuthorityFailureClass,
    CompletePackageTree, DependencyAcquisitionOutcome, DependencyEvidenceKey,
    DependencyResolutionEvidence, EvidenceFactKind, EvidenceFactOutcome, EvidenceFence,
    EvidenceRevalidationOutcome, EvidenceRevalidationRequest, FontCatalogCandidate,
    FontCatalogRequest, FontCatalogSnapshot, FontContainerAcquisitionRequest,
    PackageAcquisitionRequest, SyncFontAuthority, SyncPackageAuthority,
};
use tp::compilation::{
    CanonicalDiagnosticPolicy, CompilationDispatchOutcome, CompilationExecutionFacility,
    CompilationReportProjection, CompilationReportTerminalRef, CompilationReportingPolicy,
    CompilationRequest, CompilationResourceLimitSpec, CompilationResourceLimits,
    EngineRuntimeDomainDescriptor, EngineRuntimeDomainPlacement, ExecutionFacilityCapacity,
    NoSemanticResultCache, ReadyCompilationJob, SemanticCacheAdmissionOutcome,
    SemanticCacheAdmissionRequest, SemanticCacheLookupOutcome, SemanticCacheLookupRequest,
    SyncCompilationControls, SyncSemanticCacheLookup, SyncSemanticResultCache,
};
use tp::creation::{
    CreationDispatchOutcome, CreationEvidenceCapabilities, CreationEvidenceFenceOutcome,
    CreationEvidenceFenceRequest, CreationExecutionFacility, CreationInput, CreationInputEvidence,
    CreationRequest, CreationResourceLimits, DiscoveryVariant, FontEmbeddingPolicy,
    PackageEmbeddingPolicy, ProjectSnapshot, ReadyCreationJob, SyncCreationControls,
    SyncCreationEvidence,
};
use tp::session::{
    ArmedSubscriptions, FenceConfirmation, FenceConfirmationOutcome, FenceReadObservation,
    FenceReadOutcome, RequestSourceObservation, SessionEffect, SessionEvent, SessionWatchCoverage,
    SubscriptionArmOutcome,
};
use tp::transport::{
    AcquisitionTransportControls, CompilationDeliveryOutcome, MemorySpool,
    PublicationCommitStrength, SyncCompilationDelivery, SyncPackArchiveAcquirer, SyncSpoolFacility,
    TransportCleanupOutcome, TransportControls, TransportOutcome, TransportReceipt, TransportStage,
};
use tp::{
    AdmittedOperationResourceLimits, AuthorityInstanceIdentity, CacheIsolationDomain,
    CanonicalIdentity, DeploymentTrustProfile, FontContainerIdentity, InterruptionSource,
    MonotonicClock, MonotonicInstant, MonotonicTimeDomain, OperationDeadline, OrdinaryAdmission,
    PackagePath, PackageSpecification, ProjectPath, StableByteValue,
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
        let bytes = StableByteValue::from_static(controls.admission(), b"archive");
        let receipt = TransportReceipt::try_new_acquisition(
            &controls,
            "test.archive-acquirer",
            TransportStage::Transferred,
            bytes.len(),
            Some(bytes.content_identity().clone()),
            TransportCleanupOutcome::NotRequired,
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
        projection: CompilationReportProjection,
        _destination: &Self::Destination,
        controls: TransportControls<'_>,
    ) -> CompilationDeliveryOutcome {
        let _ = projection.includes_exact_diagnostic_text();
        let _ = projection.diagnostics().count();
        let receipt = TransportReceipt::try_new_transport(
            &controls,
            "test.delivery",
            TransportStage::Committed,
            0,
            None,
            Some(PublicationCommitStrength::CompleteCollectionAtomic),
            TransportCleanupOutcome::NotRequired,
        )
        .unwrap();
        let transport = TransportOutcome::try_new(Ok(()), receipt).unwrap();
        CompilationDeliveryOutcome::try_new(projection, transport).unwrap()
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
    let admission = OrdinaryAdmission::try_new(DeploymentTrustProfile::Trusted).unwrap();
    let _ = SuccessfulPackages {
        admission: admission.clone(),
        identity: AuthorityInstanceIdentity::try_new("test.success-packages").unwrap(),
    };
    let _ = SuccessfulFonts {
        container: FontContainerIdentity::parse(&admission, "sha256:test-font").unwrap(),
        admission,
        identity: AuthorityInstanceIdentity::try_new("test.success-fonts").unwrap(),
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

    fn capacity(&self) -> ExecutionFacilityCapacity {
        ExecutionFacilityCapacity {
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

    fn dispatch<'a>(&'a self, _job: ReadyCreationJob) -> Self::Dispatch<'a> {
        ready(CreationDispatchOutcome::Refused)
    }
}

fn test_domain() -> EngineRuntimeDomainDescriptor {
    EngineRuntimeDomainDescriptor {
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
    ) -> DependencyAcquisitionOutcome<FontCatalogSnapshot> {
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
        let bytes = StableByteValue::from_static(&self.admission, b"[package]");
        let tree = CompletePackageTree::try_from_files([(path, bytes)]).unwrap();
        DependencyAcquisitionOutcome::Acquired(AcquiredDependency {
            value: tree,
            evidence: self.evidence(),
            provenance: AcquisitionProvenance::try_new("test.memory").unwrap(),
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
    container: FontContainerIdentity,
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
            provenance: AcquisitionProvenance::try_new("test.memory").unwrap(),
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
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<FontCatalogSnapshot> {
        self.acquired(
            FontCatalogSnapshot::try_new([FontCatalogCandidate {
                container: self.container.clone(),
                face_index: 0,
                family: "Test".into(),
                style: "Regular".into(),
            }])
            .unwrap(),
        )
    }

    fn acquire_container(
        &self,
        _request: FontContainerAcquisitionRequest,
        _controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<StableByteValue> {
        self.acquired(StableByteValue::from_static(&self.admission, b"font"))
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
        64,
        256 * 1024 * 1024,
        128,
        512,
        256 * 1024 * 1024,
        16,
        128,
        512 * 1024 * 1024,
        768 * 1024 * 1024,
    )
    .unwrap();

    let main = ProjectPath::parse(&admission, "main.typ").unwrap();
    let bytes = StableByteValue::from_static(&admission, b"Hello");
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
    )
    .unwrap();

    let creation_report = tp::creation::create_sync(input, creation_controls);
    let Ok(pack) = creation_report.into_pack() else {
        return;
    };

    let diagnostics = CanonicalDiagnosticPolicy::try_new(1, 5_000, 8 * 1024 * 1024).unwrap();
    let request = CompilationRequest::pdf(diagnostics);
    let prepared = pack.prepare(&admission, request).unwrap();
    let no_cache =
        NoSemanticResultCache::new(CacheIsolationDomain::try_new("test.cache-isolation").unwrap());
    let controls = SyncCompilationControls::try_new(
        admission,
        AdmittedOperationResourceLimits::try_caller_selected(compilation_limits()).unwrap(),
        package_trait,
        font_trait,
        SyncSemanticCacheLookup::Disabled::<NoSemanticResultCache>,
        NonZeroUsize::new(4).unwrap(),
        OperationDeadline::None,
        &clock,
        &interruption,
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

fn typecheck_async_future_policy() {
    let admission = OrdinaryAdmission::try_new(DeploymentTrustProfile::Trusted).unwrap();
    let domain = MonotonicTimeDomain::try_new("test.async-monotonic").unwrap();
    let clock = TestClock { domain };
    let interruption = NeverInterrupt;
    let budget = AcquisitionBudget::try_new(1024, 1024, 1024, 1024).unwrap();
    let controls =
        AcquisitionControls::try_new(OperationDeadline::None, &clock, &interruption, &budget)
            .unwrap();
    let request = PackageAcquisitionRequest {
        specification: PackageSpecification::parse(&admission, "@test/example:1.0.0").unwrap(),
        expected_tree_identity: None,
    };

    let local = LocalPackages {
        identity: AuthorityInstanceIdentity::try_new("test.local-packages").unwrap(),
        marker: Rc::new(()),
    };
    let _local_future = local.acquire(request.clone(), controls);

    let controls =
        AcquisitionControls::try_new(OperationDeadline::None, &clock, &interruption, &budget)
            .unwrap();
    let send = SendPackages {
        identity: AuthorityInstanceIdentity::try_new("test.send-packages").unwrap(),
    };
    assert_send(send.acquire(request, controls));
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

fn main() {}
