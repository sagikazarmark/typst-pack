# PROTOTYPE: Rust Lifecycle and Adapter Interfaces

> Throwaway design artifact for
> [Freeze the Rust lifecycle and adapter interfaces](https://github.com/sagikazarmark/typst-pack/issues/61).
> This freezes the recommended public contract for implementation planning. It
> is not production code or a compatibility promise for the current 0.3 crate.

The companion
[`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs)
is a stable-Rust interface fixture. Its bodies are deliberately absent; it
compile-checks module placement, ownership, type flow, generic-associated-future
traits, and visibility without implementing the destination.

## Question

What compilable public Rust interface and private seam contract realizes the
accepted lifecycle, including module ownership, Stable Byte Value backings,
Operation Resource Limit placement and reuse, authority evidence revalidation,
session fencing events, sync and async traits, semantic caching, and
first-release Hostile behavior without leaking target dependencies into the
featureless core?

## Verdict

Freeze the operation-owned hybrid from
[Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39),
with the following concrete refinements:

1. Keep exactly seven public modules: `creation`, `pack`, `authority`,
   `compilation`, `representation`, `transport`, and `session`.
2. Keep semantic lifecycle values opaque and immutable. There is no public Pack
   builder, Pack Control Record, Pack Manifest constructor, Typst World,
   Compilation Dependency Snapshot, Compilation Kernel, engine, exporter, or
   unchecked constructor.
3. Keep Project Snapshot pure. Mutable-source evidence crosses a parallel
   operational seam and must establish a Creation Evidence Fence before Pack
   assembly and replay.
4. Use separate role-specific synchronous and asynchronous traits. Async traits
   use generic associated futures, impose no global `Send` or `Sync` bound, and
   let native executors add those bounds at their call sites.
5. Keep Stable Byte Value backing sealed. All targets get static, contiguous,
   chunked, and memory-spool values. Linux, macOS, and Windows additionally get
   a core-owned, target-gated `NativeSpool`; no first-release browser Blob,
   OPFS, WASI, or other native backing is claimed.
6. Use operation-specific resource-limit records wrapped in one reusable
   `AdmittedOperationResourceLimits<L>`. Configuration may be cloned and reused;
   every operation allocates fresh counters and reservations.
7. Preserve the exact terminal tree. Preparation yields a Prepared Compilation
   or Compilation Request Rejection. An attempt yields one Compilation Report,
   which contains one Compilation Result or Compilation Operation Outcome.
8. Let a Semantic Result Cache participate in pre-commit lookup, but make cache
   admission a distinct post-commit operation whose failure cannot change the
   immutable Compilation Report.
9. Keep Compilation Session as a synchronous reducer with explicit read, arm,
   confirm, publish, and retire events and effects. A cache hit is only a
   historical candidate until this live fence succeeds.
10. Give creation and compilation separate execution-facility interfaces. They
    may report the same Engine Runtime Domain, but they do not share one
    ambiguous queue contract.
11. Include Hostile in the requested trust vocabulary, but make every ordinary
    first-release admission return `HostileUnavailableInFirstRelease` before
    input-dependent interpretation. Publish no Hostile facility trait until a
    complete raw-ingress-to-parent-verified-output implementation exists.

This is the recommended answer for every design branch. It favors semantic
depth and honest guarantees over a smaller but leaky facade.

## Public Topology

```text
typst-pack
├── authority       package/font acquisition and evidence ports
├── compilation     preparation, drivers, terminals, cache and ready jobs
├── creation        Project Snapshot, evidence fence and Pack Issuance
├── pack            opaque Pack identity and inspection
├── representation  Epoch 2 ingress, encoding and projection plans
├── session         runtime-neutral reducer and currentness protocol
└── transport       stable bytes, spooling, role-specific movement and receipts
```

The crate root re-exports only the lifecycle values most callers carry:

```rust
pub use compilation::{
    CompilationOperationOutcome,
    CompilationReport,
    CompilationResult,
    CompilationTerminal,
    PreparedCompilation,
};
pub use pack::{Pack, PackIdentity, PackInspection};
pub use session::CompilationSession;
pub use transport::StableByteValue;
```

Memory helpers stay under their owning modules. There is no generic `memory`,
`storage`, `runtime`, `world`, `engine`, `kernel`, or `adapter` module.

The intended crate topology is:

```text
typst-pack          featureless semantic core, representations, drivers,
                    sealed memory/native stable backings, memory helpers
typst-pack-fs       source stabilization, confined filesystem authorities,
                    watch and publication adapters, native-spool policy
typst-pack-cli      first-party CLI composition and presentation
consumer crates     service, Dagger, browser and product-specific adapters
```

The target-gated native spool is a deliberate narrow exception to adapter
placement. Rust has no friend-crate mechanism: a backing implemented in a
companion crate cannot remain sealed. The filesystem adapter selects and admits
a parent location; the core creates an exclusive operation-private root beneath
it, then finalizes, owns, reads, and cleans the backing without exposing its path
through Stable Byte Value.

## Lifecycle Values

| Value | Visibility | Legal construction |
| --- | --- | --- |
| Project Snapshot | Public immutable input | Validated finite canonical path-to-Stable-Byte-Value tree |
| Pack | Public opaque | Pack Issuance, validated Pack Archive ingress, or validated Closure Export import |
| Pack Inspection | Public immutable projection | `Pack::inspect` |
| Prepared Compilation | Public opaque | Side-effect-free preparation only |
| Compilation Request Rejection | Public immutable terminal | Failed semantic preparation |
| Compilation Report | Public opaque immutable terminal account | One Compilation Attempt |
| Compilation Result | Public opaque semantic value | Private Compilation Kernel and complete export |
| Compilation Operation Outcome | Public immutable operational value | Attempt failure before Compilation Terminal Commitment |
| Compilation Terminal | Public closed union | Compilation Request Rejection or Compilation Report |
| Compilation Session | Public caller-owned reducer | `CompilationSession::new` |
| Stable Byte Value | Public opaque immutable bytes | Core-owned memory or completed spool backing |
| Discovery World and Discovery Snapshot | Private | Creation attempt implementation |
| Compilation Dependency Snapshot | Private | Compilation Driver implementation |
| Typst World and document | Private | Compilation Kernel implementation |
| Pack Control Record | Private | Representation implementation |
| Transport Locator | Adapter-owned | Never semantic state |

`Pack::prepare` is a thin forwarding convenience. Creation, representation,
transport, execution, delivery, and sessions remain operation-owned modules.

## Stable Byte Values

`StableByteValue` is a finite, immutable, exact byte sequence whose complete
range remains synchronously readable for its lifetime. Its public constructors
take ownership or copy into a sealed backing:

```rust
pub struct StableByteValue(Arc<private::StableBacking>);

impl StableByteValue {
    pub fn from_vec(admission: &OrdinaryAdmission, bytes: Vec<u8>) -> Self;
    pub fn from_arc(admission: &OrdinaryAdmission, bytes: Arc<[u8]>) -> Self;
    pub fn copy_from_slice(admission: &OrdinaryAdmission, bytes: &[u8]) -> Self;
    pub fn from_static(admission: &OrdinaryAdmission, bytes: &'static [u8]) -> Self;
    pub fn from_chunks(
        admission: &OrdinaryAdmission,
        chunks: impl IntoIterator<Item = Arc<[u8]>>,
    ) -> Result<Self, StableByteValueConstructionError>;
    pub fn len(&self) -> u64;
    pub fn content_identity(&self) -> &ContentIdentity;
    pub fn read_exact_at(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), StableReadError>;
}
```

The sealed backing set for the first release is:

| Backing | Targets | Decision |
| --- | --- | --- |
| Static bytes | All | Supported |
| Contiguous immutable memory | All | Supported |
| Chunked immutable memory | All | Supported |
| Memory spool | All | Supported |
| Core-owned native local spool | Linux, macOS, Windows | Supported behind target-gated `transport::native::NativeSpool` |
| Browser Blob | Browser | Transport source only; asynchronous reads do not satisfy Stable Byte Value |
| Browser OPFS | Browser | Deferred until a concrete core-owned synchronous worker implementation is verified |
| Third-party backing trait | Any | Rejected; it permits false stability claims |

External adapters implement bounded sync or GAT-async byte sources and choose a
core spool. They cannot implement a Stable Byte Value backing.

## Resource Limits

Operation Resource Limits are explicit and have no core numeric defaults. The
public contract uses operation-specific records rather than one universal bag:

| Record | Owned dimensions |
| --- | --- |
| `creation::CreationResourceLimits` | Project inventory, variants, packages, fonts, discovery restarts, stable spool, retained memory |
| `representation::PackIngressResourceLimits` | Archive/control/decoded bytes, entries, files, expansion, spool, retained memory |
| `representation::RepresentationResourceLimits` | Encoded/projected objects and bytes, spool, retained memory |
| `compilation::CompilationResourceLimits` | Dependencies, dependency bytes, pages, artifacts, raster work, spool, retained memory, reporting channels |
| `transport::SpoolResourceLimits` | Source, stable spool, in-flight and retained-memory bytes |
| `transport::TransportResourceLimits` | Object count, aggregate/largest/in-flight bytes and transfer concurrency |

`AdmittedOperationResourceLimits<L>` records caller-requested values, admitted
values, and an optional first-party adapter profile identity. A generic library
caller uses checked `try_caller_selected`; a shipped adapter uses checked
`try_from_adapter_profile`. Operation limit records have private fields and
validated constructors, so neither limits nor admitted capacity can be forged.
The immutable record may be shared across calls, but each operation creates a
new private budget ledger. Counters, reservations, pinned bytes, queue places,
and deadlines are never reused.

Limits reach the first observable seam. Project Snapshot and Creation Request
construction receive Creation Resource Limits before consuming caller
collections. Package and Font Authorities receive an operation-private
`AcquisitionBudget` through inspectable Acquisition Controls and reserve
downloaded, expanded, spooled, and retained bytes incrementally. Cache adapters
receive an equivalent `CacheBudget`. Spool, transport, acquisition, and cache
controls expose immutable views of their admitted limits, expected identities,
deadlines, clocks, interruption sources, and role-specific commit or cleanup
requirements.

These controls remain separate:

| Dimension | Owner |
| --- | --- |
| Pack Format ceilings | Epoch 2 representation validity |
| Canonical Diagnostic Policy | Semantic Compilation Request and Compilation Identity |
| Dependency acquisition concurrency `D` | Creation or Compilation Driver |
| Ready-job concurrency `K` and queue `Q` | Role-specific execution facility |
| Engine/Rayon width `W` | Engine Runtime Domain fixed before managed work |
| Transport concurrency `T` | Transport adapter |
| Isolated worker capacity `P` | Isolated facility |

Preparation applies semantic validity and immutable Pack Format ceilings. It
does not consume attempt limits. Every execution of a reusable Prepared
Compilation performs fresh operational admission; a stricter attempt limit can
produce `CompilationOperationCause::ResourceLimit` before effects without
invalidating the Prepared Compilation or changing Compilation Identity.

## Creation And Evidence

Project Snapshot contains only canonical paths and Stable Byte Values. It never
contains source paths, Transport Locators, Dependency Evidence Keys, revalidation
callbacks, watchers, generations, or authority handles.

Creation receives a parallel `CreationInputEvidence` value and a role-specific
evidence adapter:

```rust
pub struct CreationInput {
    pub request: CreationRequest,
    pub evidence: CreationInputEvidence,
}

pub trait SyncCreationEvidence {
    fn fence(
        &self,
        request: CreationEvidenceFenceRequest,
        controls: AcquisitionControls<'_>,
    ) -> CreationEvidenceFenceOutcome;
}

pub trait AsyncCreationEvidence {
    type Fence<'a>: Future<Output = CreationEvidenceFenceOutcome> + 'a
    where
        Self: 'a;
    // fence(...)
}
```

`CreationInputEvidence::caller_owned_immutable` plus the core-supplied
`ImmutableCreationEvidence` adapter is the explicit attestation for in-memory
values. `versioned` and `revalidatable` bind adapter-produced evidence subjects
and keys to one provider identity. Creation compares that binding with the
provider and its advertised capabilities during admission, before discovery. A
mutable adapter must retain enough operational evidence to revalidate every
causal content, absence, membership, order, metadata, and source-choice fact.

The private creator owns this sequence:

```text
ordinary admission
-> freeze Project Snapshot, variants, authorities and exact values
-> isolated per-variant discovery
-> bounded package/font acquire-and-restart; discard partial traces
-> select only causal evidence
-> establish race-closing Creation Evidence Fence
-> whole-Pack construction
-> replay against frozen Discovery Snapshot without reacquisition
-> Pack Issuance
-> discard raw sensitive values, keys, revalidators and locators
```

The Creation Report distinguishes source change, revalidation failure, and
insufficient evidence capability. None degrades into a warning or exposes a
Pack.

Discovery Variant construction covers the complete Discovery Coverage Request:
target, Typst input map, Compilation Document Time, engine features, and
discovery-only Pack Overrides, plus a non-identifying label. Package and Font
Embedding Policies support all-embedded, all-external, or exact mixed
dispositions keyed by Package Specification or Font Container Identity. The
policy is deterministic and each discovered requirement's resulting disposition
enters Pack Identity.

## Authorities

Package Authority and Font Authority are separate because their values,
selection behavior, evidence, and validation differ. They return complete
stable values, sanitized provenance, and causal Dependency Resolution Evidence;
they never expose Typst's file-at-a-time World callbacks.

Each role has a sync trait and a GAT-async trait. The traits require neither
`Send` nor `Sync`:

```rust
pub trait AsyncPackageAuthority {
    type Acquire<'a>: Future<
        Output = DependencyAcquisitionOutcome<CompletePackageTree>,
    > + 'a
    where
        Self: 'a;

    type Revalidate<'a>: Future<Output = EvidenceRevalidationOutcome> + 'a
    where
        Self: 'a;
    // acquire(...), revalidate(...)
}
```

This admits browser-local `Rc`-backed adapters. A native executor that spawns
the complete driver future adds `P: Send + Sync` and
`for<'a> P::Acquire<'a>: Send` at its own seam. GAT traits are intentionally not
dyn-compatible; runtime-selected adapters use concrete enums or adapter-owned
local/Send erasing wrappers rather than a second semantic contract.

Only `DependencyAcquisitionOutcome::Failed` with
`AuthorityFailureClass::Unavailable` permits fallback. The composed authority
owns source ordering and must include causally relevant higher-priority misses
in its evidence. Authorization denial is permanent failure, never
unavailability. One failure class field is the sole source of truth, so a
fallback variant cannot contradict its payload. Semantic modules independently
validate every returned package tree, font catalog, and Font Container.

An authority without revalidation remains legal for one-shot compilation. It
cannot satisfy mutable Pack creation or complete Session Watch Coverage.

Adapter-produced values have controlled public construction paths rather than
unchecked semantic constructors. Complete Package Tree and Font Catalog
candidates use validated finite builders. Dependency Evidence Keys and
Dependency Resolution Evidence use an authority-bound builder. Acquisition
Provenance and failure details accept only sanitized namespaced codes. Evidence
fences, provider cursors, transport receipts, and session fence observations
have coherence-checking constructors. The downstream compile probe implements
the public adapter traits to ensure their required outcomes are constructible.

## Compilation Terminals

The frozen tree is:

```text
validated Pack + exact semantic request
└── preparation
    ├── Compilation Request Rejection
    └── Prepared Compilation
        └── Compilation Attempt
            └── Compilation Report
                ├── Compilation Result
                └── Compilation Operation Outcome
                    └── Compilation Terminal Commitment
                        ├── optional cache admission outcome
                        ├── optional delivery outcome
                        └── optional session publication
```

`CompilationTerminal` names only the outer one-shot union:

```rust
pub enum CompilationTerminal {
    RequestRejected(CompilationRequestRejection),
    Report(CompilationReport),
}
```

The report exposes its inner branch through `CompilationReportTerminalRef`.
Request rejection never fabricates a report, Compilation Identity, dependency
evidence, or access trace. Deterministic compiler/exporter rejection is a
Compilation Result. Dynamic resource, authority, cancellation, deadline,
facility, and isolation failure before commitment is a Compilation Operation
Outcome. Work after commitment returns a separate role-specific outcome that
retains the immutable report.

The primitive reusable seams are:

```rust
prepare(&OrdinaryAdmission, &Pack, CompilationRequest)
    -> Result<PreparedCompilation, CompilationRequestRejection>

run_sync(&PreparedCompilation, SyncCompilationControls)
    -> CompilationReport

run_async(&PreparedCompilation, AsyncCompilationControls)
    -> CompilationReport
```

`compile_sync` and corresponding adapter conveniences may compose preparation
and one attempt into `CompilationTerminal`, but they cannot flatten or discard
the stages.

Compilation Report, Result, and disclosure views expose complete immutable
structure: request-inventory origin and semantic classification; operational
profile and Engine Runtime Domain; cache and evidence scope; document target and
page count; format-bearing artifact roles and bytes; Compilation Access Trace;
and canonical diagnostic phase, severity, kind, spans, message, hints, and
completion. A limited Canonical Diagnostic Envelope identifies the first
omitted ordinal, phase, and limiting dimension. Creation Report has its own
phase-ordered structured diagnostic view with Discovery Variant or replay scope.
Diagnostic spans use typed logical project or package locations rather than
assuming every source belongs to the project tree.

Compilation Request construction covers Typst inputs, selected engine features,
Compilation Document Time, a Pack Override Set, Canonical Diagnostic Policy, and
one complete PDF, PNG, SVG, or HTML output specification. Format-specific types
own page selection, PPI, bleed, pretty printing, PDF identifier and creator
modes, PDF Creation Time, standards, and tagging. The core derives target and
required HTML features and deterministically rejects the representable but
unsupported bundle feature and other contradictory combinations during
preparation.

## Semantic Result Cache

Semantic Result Cache lookup is a real public seam because memory, persistent,
browser, and service adapters vary there. Content caches, dependency-resolution
caches, archive-encoding caches, and transport reuse stay private to their
owning adapters.

The core owns a disposable private-epoch cache-record codec. Cache adapters
store and retrieve an opaque `SemanticCacheRecord` backed by Stable Byte Value:

```rust
pub trait SyncSemanticResultCache {
    fn lookup(
        &self,
        request: SemanticCacheLookupRequest<'_>,
    ) -> SemanticCacheLookupOutcome;

    fn admit(
        &self,
        request: SemanticCacheAdmissionRequest<'_>,
    ) -> SemanticCacheAdmissionOutcome;
}
```

Each cache instance exposes one fixed Cache Isolation Domain; callers do not
assert a different domain per lookup. A tenant-aware storage implementation
creates one authorized adapter view per domain. `NoSemanticResultCache` gives
disabled paths an inferable concrete type without inventing cache behavior.

Persistent adapters reconstruct `SemanticCacheRecord` from untrusted bytes; the
core verifies identity, complete result structure, artifact bytes, authorization
context, and current resource admission before use. A malformed, unauthorized,
conflicting, or over-limit candidate produces a Compilation Operation Outcome
without silent same-attempt fallback. A clean miss and policy-authorized
availability failure may proceed.

A hit preserves the original Compilation Result, Compilation Result Identity,
Compilation Access Trace, diagnostics, and artifacts. The fresh report records
current cache provenance and skipped acquisition separately.

Cache admission is post-commit:

```text
Compilation Report committed
-> report-carrying cache-record preparation
-> cache.admit(...) when preparation succeeded
-> report-carrying admission outcome
```

`CompilationCacheAdmissionOutcome` carries the immutable report beside the
cache outcome. Likewise, `CompilationDeliveryOutcome` carries the report beside
its Transport Outcome and receipt. Admission or delivery failure cannot mutate,
replace, detach from, or reclassify the report. This
supersedes the earlier implication that a read-write driver cache policy could
hide cache writes inside the immutable Compilation Report.

## Execution Facilities

Synchronous drivers run on the caller's thread and bypass a facility.
Asynchronous creation and compilation use distinct facility traits, each taking
an opaque ready job after asynchronous acquisition is complete.

```rust
pub trait CompilationExecutionFacility {
    type Dispatch<'a>: Future<Output = CompilationDispatchOutcome> + 'a
    where
        Self: 'a;

    fn domain(&self) -> &EngineRuntimeDomainDescriptor;
    fn capacity(&self) -> ExecutionFacilityCapacity;
    fn dispatch<'a>(
        &'a self,
        job: ReadyCompilationJob,
    ) -> Self::Dispatch<'a>;
}
```

The facility may call `ReadyCompilationJob::run_in_process`, or use
`into_worker_request` and the paired response verifier for an ordinary isolated
worker. It cannot construct a ready job, Compilation Dependency Snapshot,
successful completion, report, result, Engine Identity, or Exporter Identity.

Creation has a parallel role-specific facility because discovery/replay queue
semantics are not Compilation Execution Facility `K`/`Q`. Both may identify the
same fixed Engine Runtime Domain. Neither initializes a global scheduler for a
generic library caller.

The worker protocol is operational, opaque, versioned, bounded, and verified by
the parent. Kernel isolation supplies killability and resource placement, not a
Hostile guarantee.

Memory Spool's asynchronous implementation returns a real local future capable
of awaiting repeated pending source reads; it is not constrained to `Send` or a
single immediately-ready poll.

## Representation And Transport

Representation ingress takes one explicit `PackIngressExpectations` value:

```rust
pub struct PackIngressExpectations {
    pub input_content_identity: Option<ContentIdentity>,
    pub pack_identity: PackIdentityVerificationMode,
    pub archive_encoding: Option<ArchiveEncodingIdentity>,
}
```

This makes the content-identity mismatch and asserted-recipe mismatch terminal
branches reachable. Invalid and unsupported terminals carry stable validation
rule codes in fixed precedence, and Representation Receipt exposes the rules and
derived input identity rather than collapsing validation to prose.

Representation owns validated Pack Archive ingress, validated Closure Export
import, registered Archive Encoding Identity selection, deterministic Pack
Archive encoding, baseline-only Project Materialization plans, and lossless
Closure Export plans. The plans expose canonically ordered paths, Stable Byte
Values, and identities. Publishing those values is later transport work.

Project Materialization entries use Project Path. Closure Export uses a distinct
namespaced Closure Export Path and role-tagged entries for its control record,
project files, package files, Font Containers, extensions, and annotations. The
two finite-tree contracts cannot be confused through one generic projection
type.

Transport keeps separate sync and GAT-async roles for Pack Archive acquisition
and publication, Project Materialization publication, Closure Export
publication, and Compilation Delivery. There is no universal store. Adapters
construct Transport Receipt through its coherence-checking constructor.
Requested cleanup strength and actual cleanup outcome are distinct, and a
failure retains its primary stage/cause beside cleanup completion, residual
state, or non-retractable exposure. Cleanup can never replace the primary
failure.

Transport Receipt exposes one admission view containing exact requested and
admitted role-specific limits, trust, profile, requested commit and cleanup
strengths, deadline, actual commit, transferred identity and bytes, adapter
class, and cleanup result. Residual locators provide a safe summary by default
and raw adapter detail only through an explicit disclosure capability.

## Compilation Session

Compilation Session owns revisions, latest-only attempt ordering, supersession,
currentness state, publication, and Last Successful Compilation. It owns no
authority, cache, watcher, subscription handle, clock, runtime, execution
facility, transport, delivery adapter, or persistent store.

The reducer protocol is explicit:

```text
candidate Compilation Terminal or ingestion failure
-> ReadFence over mutable request sources and causal dependency evidence
-> keep old subscriptions active
-> ArmSubscriptions from read cursors or before confirmation
-> ConfirmFence after arming/replay
-> retain notifications and gaps received during confirmation as dirtiness
-> atomically install clean evidence, cursors, coverage and new generation
-> Publish only after token/revision/generation/coverage recheck
-> RetireSubscriptions for the old generation
```

The frozen event/effect vocabulary appears in the fixture:

| Events | Effects |
| --- | --- |
| `Accept`, `IngestionFailed` | `StartAttempt`, `InterruptAttempt` |
| `AttemptFinished` | `ReadFence` |
| `FenceReadFinished` | `ArmSubscriptions` |
| `SubscriptionsArmed` | `ConfirmFence` |
| `FenceConfirmed` | `Publish`, then `RetireSubscriptions` |
| `DependencyChanged`, `NotificationGap` | Dirties the candidate and may restart reconciliation |

Late attempt completions, old-generation notifications, duplicate completions,
and stale fence responses are normal asynchronous races and return
`SessionTransition::Ignored`, not protocol errors.

Read, subscription, and confirmation plans are inspectable by adapters. They
group exact scopes, Dependency Evidence Keys, provider cursors, generations,
and handoff strategy. Read completion includes scope-keyed mutable request-source
observations as well as dependency evidence, so a clean fence proves both sides
of Session Currentness.

A Semantic Result Cache hit follows exactly the same fence. It is never current
by itself. `SessionPublicationTerminal` preserves the distinct Compilation
Terminal and adapter-owned ingestion-failure branches. Request rejection may be
current relative to fully covered mutable request sources without inventing
dependency evidence. Each publication carries its Session Revision. Last
Successful Compilation exposes its originating revision and independent
currentness; a newer rejection, operation outcome, ingestion failure, or
delivery failure does not replace it, and it becomes stale when its fence no
longer holds.

Session Recovery Record remains caller-owned application data. Restoring it
creates a new session instance and requires fresh ingestion, evidence, and
subscriptions; attempts and currentness never resume across processes.

## Hostile Behavior

The first release has no complete raw-ingress-to-verified-output Hostile
facility on ordinary native, CLI, OCI/Dagger, browser, or in-process WASM
surfaces. The interface therefore makes refusal explicit rather than allowing a
marker trait or adapter assertion:

```rust
let admission = OrdinaryAdmission::try_new(
    DeploymentTrustProfile::Hostile,
);

assert!(matches!(
    admission,
    Err(AdmissionRefusal::HostileUnavailableInFirstRelease),
));
```

Every ordinary creation, representation, compilation, projection, transport,
and delivery control requires `OrdinaryAdmission`. Constructors that first turn
raw paths or bytes into typed or stable values also require that admission.
Raw-input adapters therefore cannot invoke path parsing, byte stabilization, or
semantic parsing before typed refusal. `Pack::prepare` also requires the
admission explicitly before Pack Override preflight or semantic request
interpretation.
An Isolated Compilation Worker does not upgrade the profile because its seam
begins after Pack and dependency interpretation.

A future Hostile facility is a new sealed lifecycle interface, not an
implementation of Compilation Execution Facility. It must begin with raw finite
inputs, enforce pre-parse hard quotas and kill/reap, return a bounded protocol,
verify the complete result in the parent, and keep publication parent-owned.

## Private Seams

| Private seam | What it owns and hides |
| --- | --- |
| Whole-Pack construction | All Epoch 2 invariants, canonical paths, exact requirements, traces, identities and Pack commitment |
| Discovery and issuance | Discovery World/Snapshot, per-variant worlds, acquire-and-restart, causal evidence selection, fence, replay and issuance |
| Compilation preparation | Defaults, Canonical Diagnostic Policy, Pack Override preflight, Discovery Coverage match, request inventory and identities |
| Dependency snapshot construction | External fulfillment, verification, stable backing and complete exact bindings |
| Compilation Kernel | One synchronous Typst compile/export path, Typst World/document, diagnostics and access trace |
| Terminal commitment | Cancellation/deadline/supersession race and immutable report finalization |
| Epoch 2 representation | Canonical CBOR, narrow ZIP, Closure Export, validation precedence and receipts |
| Stable backing enum | Memory/chunk/native layouts, exact reads, identity and cleanup |
| Cache-record codec | Disposable serialization and complete independent verification |
| Session reducer state | Revisions, tokens, candidates, fences, generations, publication and Last Successful Compilation |

Deleting any of these modules would redistribute policy across multiple callers
or adapters. By contrast, exposing an arbitrary engine, Typst World, generic
store, mutable source, watcher, or Stable Byte Value backing would create a
shallow interface and a second semantic path.

## Common Paths

An in-memory synchronous caller performs no hidden filesystem, network,
environment, wall-clock, cache, runtime, or scheduler access:

```rust,ignore
let creation_limits = creation::CreationResourceLimits::try_new(/* explicit dimensions */)?;

let project = creation::ProjectSnapshot::try_from_files(
    &admission,
    &creation_limits,
    ProjectPath::parse(&admission, "main.typ")?,
    [(ProjectPath::parse(&admission, "main.typ")?,
      StableByteValue::from_static(&admission, b"Hello"))],
)?;

let request = creation::CreationRequest::try_new(
    &creation_limits,
    project,
    [creation::DiscoveryVariant::paged_explicit_empty()],
    creation::PackageEmbeddingPolicy::embed_all(),
    creation::FontEmbeddingPolicy::embed_all(),
)?;

let input = creation::CreationInput {
    evidence: creation::CreationInputEvidence::caller_owned_immutable(&request),
    request,
};

let pack = creation::create_sync(input, creation_controls)
    .into_pack()
    .map_err(CreateError::from_report)?;

let prepared = pack.prepare(&admission, compilation_request)?;
let report = compilation::run_sync(&prepared, compilation_controls);
```

An asynchronous browser worker uses the same authority contracts with local,
non-`Send` futures, Memory Spool, one local Engine Runtime Domain, and an inline
ready-job facility. A native service adds `Send` bounds at its executor seam and
may choose native spooling and an isolated facility. Neither target type enters
the semantic values or base trait definitions.

Post-commit work is explicit:

```rust,ignore
let preparation = compilation::prepare_cache_admission(
    report.clone(),
    compilation_limits,
);
let cache_outcome = compilation::admit_to_cache_sync(
    preparation,
    &cache,
    cache_controls,
);

let projection = CompilationReportDisclosure::identity()
    .project(&report, compilation_limits);
let delivery_outcome = delivery.deliver(
    projection,
    destination,
    delivery_controls,
);
```

Both outcomes retain or refer to the immutable report; neither can rewrite it.

## Alternatives Compared

| Shape | Strength | Rejection reason |
| --- | --- | --- |
| One-shot facade only | Smallest common call | Hides preparation reuse and collapses terminal/effect distinctions |
| Typestate pipeline | Compile-time sequencing | Leaks the lifecycle into generic parameters and makes async adapters and bindings unwieldy |
| Universal store/runtime traits | Superficially flexible | Collapses role-specific authority, fallback, commit, cleanup and disclosure semantics |
| Fully open engines/backings | Maximum extension | Permits second semantic paths and unverifiable stability/reproducibility claims |
| Operation-owned hybrid | Deep modules, strong locality, exact advanced seam | Recommended; thin conveniences can serve common callers without losing information |

Four independent interface explorations converged on the operation-owned hybrid.
The differences were ergonomic rather than architectural. This artifact adopts
the strongest shared result: operation-specific limits, explicit post-commit
cache admission, separate role facilities, sealed native/memory backings, and a
fully explicit session fence.

## Superseded Current Seams

The 0.4 clean break removes rather than wraps:

- public `PackBuilder` and direct Pack construction;
- public `PackWorld` and arbitrary `typst::World` compilation;
- Pack read/write mixed into the Pack module;
- filesystem discovery inside the semantic core;
- current Resource Slot and Resource Provider behavior;
- TOML Pack Manifest as Pack construction interface;
- flattened `Result<CompilationOutput, CompileError>` behavior;
- target feature flags that combine core semantics, filesystem and CLI.

ADR-0002's single private whole-Pack construction seam remains valid. Its Epoch
1 TOML/ZIP details are superseded by the Epoch 2 contract. ADR-0004 and the
Resource Slot portions of ADR-0005 are historical inputs superseded by the
clean-sheet Pack contract; CLI parity decisions unrelated to Resource Slots
remain input to the first-party adapter contract.

## Verification

The fixture and a downstream consumer compile as standalone Rust 2024 crates:

```sh
rustc --edition=2024 \
  --crate-type=lib \
  --crate-name=typst_pack_interface \
  PROTOTYPE-rust-lifecycle-adapter-interfaces.rs \
  -o /tmp/libtypst_pack_interface.rlib

rustc --edition=2024 \
  PROTOTYPE-rust-lifecycle-adapter-consumer.rs \
  --extern typst_pack_interface=/tmp/libtypst_pack_interface.rlib
```

The downstream probe constructs the common sync lifecycle, uses sync trait
objects, implements external authorities and evidence, consumes report/result
views, and proves both a local non-`Send` GAT future and a call-site `Send`
future. Together the checks cover the seven-module topology, sealed-value
visibility, validated adapter-produced candidates, role-specific facilities,
terminal tree, cache records, and reducer event/effect flow. Runtime behavior
remains deliberately unimplemented because this map plans the destination
rather than building it.

The same two commands pass under `rustc 1.92.0` in the official `rust:1.92`
container, matching the repository's declared `rust-version`.

## Planning Consequence

This contract is sufficient for
[Freeze the first-party CLI and Dagger contracts](https://github.com/sagikazarmark/typst-pack/issues/56)
to freeze generated adapter surfaces without inventing lifecycle semantics. It
introduces no new decision ticket and leaves no new in-scope fog. Exact private
layout, helper decomposition, cache eviction, allocator strategy, and benchmark
tuning below the frozen limits remain implementation choices.
