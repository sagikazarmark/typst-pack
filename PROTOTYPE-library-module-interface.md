# PROTOTYPE: Library Module and Interface Architecture

> Throwaway design artifact for the wayfinding ticket
> [Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39).
> This is an interface sketch, not production code and not a compatibility
> promise. It deliberately omits fields and implementations where they do not
> affect the public seam being tested.

## Question

Which lifecycle artifacts should be public modules or values, and which deep
interfaces, seams, and adapters give Rust callers a small library-first surface
across in-memory, filesystem, WASM, service, Dagger, and test use while keeping
Typst integration local?

## Verdict under test

Use an operation-owned hybrid:

- `Pack`, `PreparedCompilation`, `CompilationReport`, `CompilationResult`,
  `CompilationOperationOutcome`, `CompilationSession`, and `StableByteValue`
  are opaque public lifecycle values.
- `creation`, `compilation`, `representation`, `transport`, and `session` are
  deep operation modules.
- `authority` owns the narrow Package Authority and Font Authority seams shared
  by creation and compilation.
- `Pack::inspect` and `Pack::prepare` are thin forwarding conveniences. Pack
  creation, representation ingress, encoding, projection, and transport stay
  operation-owned.
- The embedded Typst engine and exporters are closed first-party
  implementations. `typst::World`, upstream document values, the Compilation
  Kernel, and the Compilation Dependency Snapshot are private.
- The base crate has no Cargo features. It contains the in-memory semantic core,
  sync and async orchestration interfaces, and a finite set of sealed
  Stable Byte Value spool implementations. Filesystem, CLI, Dagger, service,
  and product-specific concerns live in companion or consumer adapter crates.

## Lifecycle ownership

| Lifecycle artifact | Public? | Owner |
| --- | --- | --- |
| Editable source project | No | Caller or adapter |
| Project Snapshot | Yes | `creation` input value |
| Discovery World | No separate value | Frozen internally from an admitted creation request and its controls |
| Discovery Snapshot | No | `creation` attempt state |
| Discovery Trace | Read-only | Pack Inspection |
| Pack issuance candidate | No | `creation` implementation |
| Pack | Yes | Opaque root value |
| Pack Control Record | No | `representation` implementation |
| Prepared Compilation | Yes | `compilation` |
| Compilation Dependency Snapshot | No | Compilation Driver implementation |
| Compilation Kernel | No | `compilation` implementation |
| Compilation Report | Yes | `compilation` terminal attempt account |
| Compilation Result | Yes | Immutable semantic terminal value |
| Compilation Operation Outcome | Yes | Immutable operational terminal value |
| Compilation Output Artifact | Yes | Owned by a successful Compilation Result |
| Compilation Session | Yes | `session` reducer |
| Transport Locator | No | Adapter-owned operation input |
| Transport Receipt | Yes | `transport` operation evidence |

A Pack can be created only by:

1. Successful Pack creation and Pack Issuance.
2. Fully validated Pack Archive ingress.
3. Fully validated Closure Export import.

There is no public Pack builder, unchecked constructor, public Pack Manifest
constructor, or public `typst::World` escape hatch.

## Crate topology

```text
typst-pack          featureless semantic core, representations, drivers,
                    sealed memory/native/browser spools, memory adapters
typst-pack-fs       project snapshot acquisition, confined filesystem
                    authorities, watch adapters, archive/tree/artifact I/O
typst-pack-cli      CLI composition and presentation
consumer crates     service, object-store, HTTP/gRPC, Dagger, browser product
                    adapters and policy
```

The core-owned native and browser spools are a deliberate exception to target
adapter placement. Rust has no friend-crate visibility: a truly sealed Stable
Byte Value backing must be implemented in the crate that seals it. Companion
adapters may configure these spools and feed them bounded sources, but cannot
invent new backing implementations.

## Public surface

The crate root re-exports the lifecycle values most callers need:

```rust,ignore
pub use pack::{Pack, PackIdentity, PackInspection};
pub use compilation::{
    CompilationOperationOutcome,
    CompilationReport,
    CompilationResult,
    PreparedCompilation,
};
pub use session::CompilationSession;
pub use transport::StableByteValue;

pub mod authority;
pub mod creation;
pub mod compilation;
pub mod pack;
pub mod representation;
pub mod session;
pub mod transport;
```

There are intentionally no public `world`, `kernel`, `manifest`, `engine`,
`exporter`, or generic `storage` modules.

## Pack

`Pack` is an opaque, immutable, cheaply cloned semantic value. It owns all
whole-Pack invariants but does not own archive or filesystem operations.

```rust,ignore
#[derive(Clone)]
pub struct Pack(Arc<private::ValidatedPack>);

impl Pack {
    pub fn identity(&self) -> &PackIdentity;

    // Thin forwarding convenience for pack::inspect(self).
    pub fn inspect(&self) -> PackInspection;

    // Thin forwarding convenience for compilation::prepare(self, request).
    pub fn prepare(
        &self,
        request: CompilationRequest,
    ) -> Result<PreparedCompilation, CompilationRequestRejection>;
}

#[derive(Clone)]
pub struct PackInspection(Arc<private::PackInspection>);
```

`PackInspection` is a side-effect-free owned immutable snapshot of the complete
static Pack contract. It is cheap to clone and suitable for Rust, WASM, and
language bindings without retaining a borrow of the Pack. It cannot acquire
dependencies, compile, apply Pack Overrides, expose transport details, or
mutate the Pack.

## Creation

Semantic creation accepts a complete immutable Project Snapshot. Filesystem,
editor, Dagger, and browser adapters stabilize mutable project state before it
crosses this seam.

```rust,ignore
pub struct ProjectSnapshot {
    // Canonical project path -> Stable Byte Value.
    state: private::ProjectTree,
}

impl ProjectSnapshot {
    pub fn from_files(
        entrypoint: ProjectPath,
        files: impl IntoIterator<Item = (ProjectPath, StableByteValue)>,
    ) -> Result<Self, ProjectSnapshotRejection>;
}

pub struct CreationRequest {
    pub project: ProjectSnapshot,
    pub variants: NonEmptyVec<DiscoveryVariant>,
    pub explicit_conditional_inclusions: Vec<ProjectPath>,
    pub package_embedding: PackageEmbeddingPolicy,
    pub font_embedding: FontEmbeddingPolicy,
    pub metadata: PackMetadata,
}

pub struct SyncCreationControls<'a> {
    pub trust: DeploymentTrustProfile,
    pub packages: &'a dyn SyncPackageAuthority,
    pub fonts: &'a dyn SyncFontAuthority,
    pub limits: CreationResourceLimits,
    pub deadline: CreationDeadline,
    pub cancellation: &'a dyn CancellationSource,
    pub reporting: CreationReportingPolicy,
}

pub struct AsyncCreationControls<'a, P, F, X> {
    pub trust: DeploymentTrustProfile,
    pub packages: &'a P,
    pub fonts: &'a F,
    pub facility: &'a X,
    pub limits: CreationResourceLimits,
    pub deadline: CreationDeadline,
    pub cancellation: &'a dyn CancellationSource,
    pub reporting: CreationReportingPolicy,
}

pub struct SyncCreator;

impl SyncCreator {
    pub fn run(
        request: CreationRequest,
        controls: SyncCreationControls<'_>,
    ) -> CreationReport;
}

pub struct AsyncCreator;

impl AsyncCreator {
    pub async fn run<P, F, X>(
        request: CreationRequest,
        controls: AsyncCreationControls<'_, P, F, X>,
    ) -> CreationReport
    where
        P: AsyncPackageAuthority,
        F: AsyncFontAuthority,
        X: CreationExecutionFacility;
}

pub struct CreationReport {
    inventory: CreationRequestInventory,
    diagnostics: CanonicalCreationDiagnostics,
    terminal: CreationTerminal,
}

pub enum CreationTerminal {
    Issued(Pack),
    Failed(CreationFailure),
}
```

The associated futures are browser-local by default. The returned creator
future is `Send` when the concrete authority and facility associated futures,
captured controls, and references satisfy call-site `Send` bounds; the
interface does not duplicate local and native authority trait families.

The admitted request plus controls freeze one internal Discovery World. The
interface does not expose that implementation state as another caller-managed
object.

Both creators share one synchronous discovery and issuance implementation.
The Asynchronous Creator may acquire a complete Font Catalog Snapshot before
discovery. When synchronous discovery first reveals an exact Package
Specification or selected Font Container whose bytes are not staged, the
attempt returns an internal acquisition request; the driver acquires it
asynchronously and restarts that isolated Discovery Variant. Partial traces
from an interrupted pass are discarded. After dependencies reach a fixed
point, both creators perform the same discovery, snapshot validation, assembly,
replay, and issuance path.

This restart behavior is observable through attempt telemetry, not Pack
semantics. It must be bounded by distinct-package, selected-font-container, and
restart limits.

Creation owns these invariants:

- The Project Snapshot is finite, canonical, and immutable for the attempt.
- Discovery Variants are nonempty and have unique canonical identities.
- Each variant uses isolated compiler state.
- Declaration order controls reporting; coverage identity is set-like.
- Exact package trees and selected Font Containers freeze on first use.
- Explicit Conditional Inclusions name existing baseline files.
- Every variant succeeds before assembly.
- Discovery evidence is revalidated before issuance.
- Every assembled-Pack replay reproduces its Discovery Trace.
- No failure exposes a Pack.

Creation has at least two complete engine executions per successful Discovery
Variant: discovery and assembled-Pack replay. Async restarts may add more.

## Authorities

The authority module owns role-specific seams shared by creation and
compilation. It does not expose file-at-a-time Typst callbacks.

```rust,ignore
pub trait SyncPackageAuthority: Send + Sync {
    fn acquire(
        &self,
        request: PackageAcquisitionRequest<'_>,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<CompletePackageTree>;
}

pub trait SyncFontAuthority: Send + Sync {
    fn catalog(
        &self,
        request: FontCatalogRequest,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<FontCatalogSnapshot>;

    fn acquire_container(
        &self,
        request: FontContainerAcquisitionRequest<'_>,
        controls: AcquisitionControls<'_>,
    ) -> DependencyAcquisitionOutcome<StableByteValue>;
}

pub trait AsyncPackageAuthority {
    type Acquire<'a>: Future<
        Output = DependencyAcquisitionOutcome<CompletePackageTree>,
    > + 'a
    where
        Self: 'a;

    fn acquire<'a>(
        &'a self,
        request: PackageAcquisitionRequest<'a>,
        controls: AcquisitionControls<'a>,
    ) -> Self::Acquire<'a>;
}

pub trait AsyncFontAuthority {
    type Catalog<'a>: Future<
        Output = DependencyAcquisitionOutcome<FontCatalogSnapshot>,
    > + 'a
    where
        Self: 'a;

    type AcquireContainer<'a>: Future<
        Output = DependencyAcquisitionOutcome<StableByteValue>,
    > + 'a
    where
        Self: 'a;

    fn catalog<'a>(
        &'a self,
        request: FontCatalogRequest,
        controls: AcquisitionControls<'a>,
    ) -> Self::Catalog<'a>;

    fn acquire_container<'a>(
        &'a self,
        request: FontContainerAcquisitionRequest<'a>,
        controls: AcquisitionControls<'a>,
    ) -> Self::AcquireContainer<'a>;
}

pub enum DependencyAcquisitionOutcome<T> {
    Acquired {
        value: T,
        evidence: DependencyResolutionEvidence,
    },
    Unavailable(DependencyFailure),
    TransientFailure(DependencyFailure),
    PermanentFailure(DependencyFailure),
    InvalidContent(DependencyFailure),
    IntegrityMismatch(DependencyFailure),
}
```

Only `Unavailable` permits fallback. Fallback order belongs inside a composed
authority adapter. Semantic creation and compilation independently validate
every returned Complete Package Tree, Font Catalog Snapshot, and Font
Container.

Sync and async traits deliberately duplicate a narrow interface. This prevents
hidden blocking bridges, async-runtime assumptions, and a second semantic
compilation path.

## Compilation

Preparation is side-effect-free. Drivers own attempts and always return one
Compilation Report after admission.

```rust,ignore
pub struct CompilationRequest {
    pub overrides: PackOverrideSet,
    pub inputs: TypstInputs,
    pub features: FeatureSet,
    pub document_time: CompilationDocumentTime,
    pub output: CompilationOutputSpecification,
}

#[derive(Clone)]
pub struct PreparedCompilation(Arc<private::PreparedCompilation>);

impl PreparedCompilation {
    pub fn identity(&self) -> &CompilationIdentity;
    pub fn intent(&self) -> &EngineNeutralCompilationIntent;
    pub fn inventory(&self) -> &CompilationRequestInventory;
}

pub fn prepare(
    pack: &Pack,
    request: CompilationRequest,
) -> Result<PreparedCompilation, CompilationRequestRejection>;

pub struct SyncCompilationControls<'a> {
    pub trust: DeploymentTrustProfile,
    pub packages: &'a dyn SyncPackageAuthority,
    pub fonts: &'a dyn SyncFontAuthority,
    pub cache: CachePolicy<'a>,
    pub limits: CompilationResourceLimits,
    pub deadline: CompilationAttemptDeadline,
    pub cancellation: &'a dyn CancellationSource,
    pub monotonic_time: &'a dyn MonotonicTime,
    pub reporting: CompilationReportingPolicy,
}

pub struct AsyncCompilationControls<'a, P, F, X> {
    pub trust: DeploymentTrustProfile,
    pub packages: &'a P,
    pub fonts: &'a F,
    pub facility: &'a X,
    pub cache: CachePolicy<'a>,
    pub limits: CompilationResourceLimits,
    pub deadline: CompilationAttemptDeadline,
    pub cancellation: &'a dyn CancellationSource,
    pub monotonic_time: &'a dyn MonotonicTime,
    pub reporting: CompilationReportingPolicy,
}

pub struct SyncCompilationDriver;

impl SyncCompilationDriver {
    pub fn run(
        prepared: &PreparedCompilation,
        controls: SyncCompilationControls<'_>,
    ) -> CompilationReport;
}

pub struct AsyncCompilationDriver;

impl AsyncCompilationDriver {
    pub async fn run<P, F, X>(
        prepared: &PreparedCompilation,
        controls: AsyncCompilationControls<'_, P, F, X>,
    ) -> CompilationReport
    where
        P: AsyncPackageAuthority,
        F: AsyncFontAuthority,
        X: CompilationExecutionFacility;
}

pub struct CompilationReport {
    inventory: CompilationRequestInventory,
    evidence: DependencyResolutionEvidence,
    reached_scope: ReachedDependencyScope,
    reporting: CompilationReportingChannels,
    terminal: CompilationTerminal,
}

pub enum CompilationTerminal {
    Result(CompilationResult),
    OperationOutcome(CompilationOperationOutcome),
}

pub struct CompilationResult {
    identity: CompilationResultIdentity,
    status: CompilationResultStatus,
    document: CompilationDocumentSummary,
    diagnostics: CanonicalCompilationDiagnostics,
    access_trace: CompilationAccessTrace,
}

pub enum CompilationResultStatus {
    Succeeded {
        artifacts: Vec<CompilationOutputArtifact>,
    },
    Rejected,
}
```

As with creation, the associated futures permit browser-local execution. A
native caller that needs to spawn the complete driver future adds `Send` bounds
to the concrete authority and Compilation Execution Facility associated
futures at its call site.

Preparation:

- applies every deterministic core default;
- derives the target and required features;
- validates and canonicalizes the full semantic request;
- strictly preflights every Pack Override, including unused overrides;
- attests the exact embedded Engine Identity and Exporter Identity;
- performs no authority access, transport, filesystem work, or Typst
  execution; and
- aggregates independently detectable rejections in canonical order.

Drivers:

- acquire and verify every external Package Requirement and Font Requirement
  before the Compilation Kernel starts;
- build one private Compilation Dependency Snapshot;
- report independent acquisition outcomes in canonical requirement order;
- invoke exactly the same private synchronous Compilation Kernel;
- classify deterministic compiler or exporter rejection as a Compilation
  Result; and
- classify acquisition, dynamic resource, cancellation, deadline, isolation,
  execution, and infrastructure failures as Compilation Operation Outcomes.

The Synchronous Compilation Driver runs on the caller's thread. It introduces
no hidden runtime, network, filesystem, process, thread pool, or background
work. The Asynchronous Compilation Driver uses explicit async authorities and a
Compilation Execution Facility to dispatch the same opaque synchronous kernel
job.

Compilation is whole-value. Compilation Output Artifacts exist as complete
Stable Byte Values before Compilation Delivery starts. Delivery can stream
those values with backpressure but cannot provide compile-time artifact
streaming.

## Representation

Representation owns semantic validation, encoding, and complete projection
plans. It never owns a destination or publication commit.

```rust,ignore
pub fn read_pack_archive(
    archive: StableByteValue,
    verification: PackIdentityVerificationMode,
    controls: PackIngressControls,
) -> PackIngressReport;

pub fn import_closure_export(
    export: ClosureExportInput,
    verification: PackIdentityVerificationMode,
    controls: PackIngressControls,
) -> PackIngressReport;

pub fn encode_pack_archive(
    pack: &Pack,
    encoding: ArchiveEncodingIdentity,
    spool: &mut dyn SyncSpoolFacility,
    controls: RepresentationControls,
) -> PackArchiveEncodingReport;

pub fn plan_project_materialization(
    pack: &Pack,
) -> ProjectMaterializationPlan<'_>;

pub fn plan_closure_export(
    pack: &Pack,
    epoch: PackFormatEpoch,
) -> Result<ClosureExportPlan<'_>, RepresentationRejection>;
```

Pack Archive encoding and Pack Archive publication are separate operations.
Project Materialization and Closure Export are distinct projection plans.
Compilation Delivery receives an immutable Compilation Report only after
Compilation Terminal Commitment.

## Transport and sealed Stable Byte Values

```rust,ignore
#[derive(Clone)]
pub struct StableByteValue(Arc<private::SealedStableBacking>);

impl StableByteValue {
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Self;
    pub fn len(&self) -> u64;
    pub fn content_identity(&self) -> &ContentIdentity;
    pub fn read_exact_at(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), StableReadError>;
}

pub trait SyncByteSource {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, SourceReadError>;
}

pub trait AsyncByteSource {
    type Read<'a>: Future<Output = Result<usize, SourceReadError>> + 'a
    where
        Self: 'a;

    fn read<'a>(&'a mut self, destination: &'a mut [u8]) -> Self::Read<'a>;
}

pub trait SyncSpoolFacility {
    fn acquire(
        &mut self,
        source: &mut dyn SyncByteSource,
        controls: SpoolControls,
    ) -> TransportOutcome<StableByteValue>;
}

pub trait AsyncSpoolFacility {
    type Acquire<'a>: Future<Output = TransportOutcome<StableByteValue>> + 'a
    where
        Self: 'a;

    fn acquire<'a, S>(
        &'a mut self,
        source: &'a mut S,
        controls: SpoolControls,
    ) -> Self::Acquire<'a>
    where
        S: AsyncByteSource + 'a;
}
```

Callers can implement bounded byte sources, but cannot implement Stable Byte
Value backing. The core supplies a finite set:

- `MemorySpool` on every target;
- `NativeSpool` on supported native targets, using core-owned stable local
  storage; and
- `BrowserSpool` on supported WASM targets, using a core-owned Blob or OPFS
  strategy where the target can satisfy the exact synchronous-read contract.

Unsupported targets must refuse a spool strategy rather than silently weaken
the Stable Byte Value contract.

Transport exposes role-specific operation interfaces rather than one universal
store:

```rust,ignore
pub trait PackArchiveAcquirer { /* locator -> Stable Byte Value + receipt */ }
pub trait PackArchivePublisher { /* encoded archive -> destination */ }
pub trait ProjectMaterializationPublisher { /* absent project tree */ }
pub trait ClosureExportPublisher { /* absent closure tree */ }
pub trait CompilationDelivery { /* report + complete artifact inventory */ }
```

Every Transport Operation admits capabilities and controls, freezes a complete
plan, transfers boundedly, verifies exact identities, commits once, cleans up,
and returns one Transport Receipt. Cancellation before commit wins;
cancellation after commit cannot rewrite success.

## Compilation Session

Compilation Session is a synchronous reducer. It owns semantic orchestration
state and emits effects; it owns no driver, authority, cache, watcher, timer,
runtime, Compilation Execution Facility, transport, or delivery adapter.

```rust,ignore
pub struct CompilationSession {
    state: private::SessionState,
}

impl CompilationSession {
    pub fn new(pack: Pack, policy: SessionPolicy) -> Self;

    pub fn apply(
        &mut self,
        event: SessionEvent,
    ) -> Result<Vec<SessionEffect>, SessionEventRejection>;

    pub fn view(&self) -> SessionView<'_>;
}

pub enum SessionEvent {
    AcceptRequest(CompilationRequest),
    DependencyChanged(DependencyChangeNotification),
    WatchCoverageChanged(SessionWatchCoverage),
    Refresh,
    Retry,
    AttemptFinished {
        token: SessionAttemptToken,
        report: CompilationReport,
    },
}

pub enum SessionEffect {
    StartAttempt {
        token: SessionAttemptToken,
        prepared: PreparedCompilation,
    },
    InterruptAttempt {
        token: SessionAttemptToken,
    },
    ReplaceSubscriptions {
        plan: SubscriptionPlan,
    },
    Publish {
        publication: SessionPublication,
    },
}
```

`CompilationSession::apply` invokes the side-effect-free compilation
preparation implementation directly when it accepts a request. Successful
preparation may emit `StartAttempt`; a Compilation Request Rejection becomes
the terminal evaluation for that revision and may emit `Publish` without an
attempt. Only executable work, interruption, subscription replacement, and
publication cross the effect seam.

Only the latest desired revision may win Session Publication. Late attempt
tokens cannot publish over a newer revision. Last Successful Compilation stays
available as one immutable whole result and is explicitly stale whenever its
Session Currentness no longer holds.

## Common call paths

### In-memory Pack creation

```rust,ignore
use typst_pack::{StableByteValue, creation, memory};

let project = creation::ProjectSnapshot::from_files(
    "main.typ".parse()?,
    [(
        "main.typ".parse()?,
        StableByteValue::from_bytes(
            b"#rect(width: 20pt, height: 20pt)".as_slice(),
        ),
    )],
)?;

let request = creation::CreationRequest {
    project,
    variants: [creation::DiscoveryVariant::paged_explicit_empty()]
        .try_into()?,
    explicit_conditional_inclusions: vec![],
    package_embedding: creation::PackageEmbeddingPolicy::EmbedAll,
    font_embedding: creation::FontEmbeddingPolicy::EmbedAll,
    metadata: Default::default(),
};

let packages = memory::PackageAuthority::empty();
let fonts = memory::FontAuthority::empty();

let report = creation::SyncCreator::run(
    request,
    creation::SyncCreationControls {
        trust: DeploymentTrustProfile::Trusted,
        packages: &packages,
        fonts: &fonts,
        limits: creation_limits,
        deadline: CreationDeadline::None,
        cancellation: &memory::NeverCancel,
        reporting: CreationReportingPolicy::Canonical,
    },
);

let pack = match report.into_terminal() {
    creation::CreationTerminal::Issued(pack) => pack,
    creation::CreationTerminal::Failed(failure) => return Err(failure.into()),
};
```

No archive is encoded, no filesystem is consulted, and no environment or clock
default enters the request.

### Synchronous compilation

```rust,ignore
use typst_pack::compilation;

let prepared = pack.prepare(compilation::CompilationRequest {
    overrides: Default::default(),
    inputs: Default::default(),
    features: Default::default(),
    document_time: CompilationDocumentTime::Absent,
    output: PdfOutputSpecification::core_defaults().into(),
})?;

let report = compilation::SyncCompilationDriver::run(
    &prepared,
    compilation::SyncCompilationControls {
        trust: DeploymentTrustProfile::Trusted,
        packages: &packages,
        fonts: &fonts,
        cache: CachePolicy::Disabled,
        limits: compilation_limits,
        deadline: CompilationAttemptDeadline::None,
        cancellation: &memory::NeverCancel,
        monotonic_time: &memory::MonotonicTime,
        reporting: CompilationReportingPolicy::None,
    },
);

match report.terminal() {
    CompilationTerminal::Result(result) if result.succeeded() => {
        let artifact = result.artifacts().single()?;
        consume(artifact.bytes());
    }
    CompilationTerminal::Result(result) => render(result.diagnostics()),
    CompilationTerminal::OperationOutcome(outcome) => handle(outcome),
}
```

The driver returns a report rather than flattening semantic rejection and
operational failure into `Result<CompilationResult, Error>`.

### Asynchronous service compilation and delivery

```rust,ignore
let report = compilation::AsyncCompilationDriver::run(
    &prepared,
    compilation::AsyncCompilationControls {
        trust: DeploymentTrustProfile::PartiallyTrusted,
        packages: &package_service,
        fonts: &font_service,
        facility: &bounded_worker_pool,
        cache: CachePolicy::Use(&shared_cache),
        limits,
        deadline,
        cancellation: &request_cancellation,
        monotonic_time: &service_clock,
        reporting,
    },
).await;

let delivery = object_store_delivery
    .deliver(
        &report,
        CompilationReportDisclosure::Identity,
        &destination,
        delivery_controls,
    )
    .await;
```

Dependency acquisition, synchronous Compilation Kernel dispatch, and
Compilation Delivery remain three explicit operations. Delivery failure cannot
replace the report's Compilation Result.

### Runtime-neutral session loop

```rust,ignore
let mut session = CompilationSession::new(pack, SessionPolicy::latest_only());

for effect in session.apply(SessionEvent::AcceptRequest(request))? {
    match effect {
        SessionEffect::StartAttempt { token, prepared } => {
            executor.start(token, prepared);
        }
        SessionEffect::InterruptAttempt { token } => executor.interrupt(token),
        SessionEffect::ReplaceSubscriptions { plan } => watcher.replace(plan),
        SessionEffect::Publish { publication } => ui.publish(publication),
    }
}
```

## Dependency strategy

| Dependency class | Treatment |
| --- | --- |
| In-process | Canonical identity, Pack invariants, request preparation, Compilation Kernel, exporters, diagnostics, report construction. No adapter. |
| Local-substitutable | Project snapshots, confined filesystem publication, and local stable storage. Concrete filesystem and memory adapters; no filesystem port in semantic interfaces. |
| Remote but owned | Package/font services, caches, execution workers, spool services, and delivery endpoints. Narrow ports with HTTP/gRPC/queue and in-memory adapters. |
| True external | Upstream package registries, network font providers, object stores, and similar providers. Role-specific authority or transport ports with mock adapters in tests. |

Ordinary native, browser/WASM, container, and in-process adapters must refuse
the Hostile Deployment Trust Profile. Conditional Hostile support belongs to a
separate verified pre-parse confinement operation around ingress, dependency
validation, Typst execution, export, and parent-owned publication. It is not a
mode on the ordinary Compilation Execution Facility.

## Depth and deletion test

| Deep module | Complexity hidden behind its interface | If deleted |
| --- | --- | --- |
| `creation` | Variant identity, isolated discovery, stable snapshots, traces, package/font freezing, embedding, assembly, replay, issuance | Every source adapter must reproduce closure discovery and issuance rules. |
| `pack` | Canonical Pack state, identities, requirements, complete static inspection | Creation, representation, and compilation must duplicate whole-Pack invariants. |
| `compilation` | Defaults, Pack Override preflight, requirement verification, private Typst World, kernel, exporters, diagnostics, identities, terminal commitment | Sync, async, session, CLI, and service callers rebuild the same semantic path. |
| `representation` | Epoch dispatch, CBOR/ZIP validation, identity agreement, archive plans, deterministic projections | Every transport adapter reimplements format validity and projection semantics. |
| `transport` | Admission, bounded transfer, spooling, identity checks, backpressure, commit, cleanup, disclosure, receipts | Every target invents inconsistent movement and publication behavior. |
| `session` | Revisions, dirtiness, supersession, subscriptions, currentness, publication, last success | Every editor, watch command, and service actor reimplements race-prone orchestration. |

`authority` is a real seam because creation and compilation each have memory,
filesystem, and service adapters. It remains narrow: candidate acquisition and
typed outcomes, not semantic Pack or compilation policy.

## Interface facts callers must know

- `Pack`, Prepared Compilation, Compilation Report, Compilation Result, and
  Compilation Session snapshots are immutable and cheap to clone unless an
  accessor explicitly returns owned bytes.
- Project Snapshot construction and Pack ingress are linear in admitted bytes.
- Pack creation performs at least two full engine runs per successful Discovery
  Variant.
- Preparation is side-effect-free and linear in request declarations and Pack
  Override bytes not already content-identified.
- The sync driver blocks the caller's thread through complete export.
- The async driver may acquire dependencies concurrently, but kernel execution
  and export remain synchronous and whole-value.
- Queue time consumes the Compilation Attempt Deadline.
- In-process cancellation cannot forcibly stop a running non-cooperative Typst
  kernel. Isolated interruption may kill and reap a worker.
- Artifacts may use stable local backing but are complete before exposure.
- No operation consults ambient filesystem, environment, clock, cache, fonts,
  packages, network, runtime, or output destination.

## Review verdict

The prototype embodies the selected decisions:

- operation-owned deep modules with thin `Pack` forwarding;
- immutable Project Snapshot input and an internal admitted Discovery World;
- no public Pack builder;
- separate sync and async authority traits, with generic associated futures and
  call-site `Send` bounds;
- bounded acquire-and-restart for asynchronous discovery;
- closed first-party engine and exporter implementations;
- separate representation and transport modules;
- companion target adapter crates around a featureless core;
- sealed Stable Byte Value backing with finite core-owned spool
  implementations;
- an owned immutable Pack Inspection snapshot; and
- a Compilation Session reducer that performs pure preparation internally and
  emits only semantic effects.
