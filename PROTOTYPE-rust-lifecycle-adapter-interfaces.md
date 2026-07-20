# PROTOTYPE: Reconciled Rust Lifecycle Implementability

> Throwaway design artifact for
> [Reconcile Rust lifecycle implementability](https://github.com/sagikazarmark/typst-pack/issues/73),
> correcting the accepted baseline from
> [Complete the Rust lifecycle and receipt interfaces](https://github.com/sagikazarmark/typst-pack/issues/65)
> with the decisions in
> [Define session preparation and pre-attempt terminal semantics](https://github.com/sagikazarmark/typst-pack/issues/68),
> [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71), and
> [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70).
> This freezes the recommended public contract for implementation planning. It
> is not production code or a compatibility promise for the current 0.3 crate.

The companion
[`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs)
is a stable-Rust interface fixture. Its bodies are deliberately absent; it
compile-checks module placement, ownership, type flow, generic-associated-future
traits, and visibility without implementing the destination.

## Question

What corrected Rust 1.92 contract and external-consumer probe make every
accepted session, bounded builder, creation rejection, semantic and operational
inventory, execution, representation, and receipt state constructible and
inspectable through exactly seven deep modules without adding generic seams or
changing accepted semantics?

## Verdict

Adopt the corrected operation-owned hybrid from
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
12. Represent every known Epoch 2 and compilation-family identity position with
    its exact opaque Rust type. Generic identities never stand in for known
    kinds, and core-derived identities have no public digest constructor.
13. Make Font Catalog candidates complete and validated while keeping candidate
    acquisition identity operational and distinct from exact container Content
    Identity. Package-tree and Font Catalog collection consume the active
    Acquisition Budget incrementally.
14. Carry metadata and annotations through creation and evidence fencing, and
    make Pack Inspection lossless for the validated Pack's complete static
    contract while preserving commitment-only sensitive request values.
15. Expose one typed semantic Compilation Request Inventory, a separate typed
    Attempt Operational Inventory, the exact complete/partial access-observation
    enum, report-local evidence references, both Artifact and Content Identity,
    and seven independently gated disclosure channels.
16. Put override count, largest-value, and aggregate-byte limits at creation and
    compilation declaration seams. Report distinct creation and compilation
    facility capacities and operational inventories.
17. Split Pack Archive read expectations from Closure Export import
    expectations, remove the pre-preparation Compilation Identity hint, and
    register only `epoch-2-all-stored-v1` for writing while retaining Deflate
    reader support.
18. Use core-owned role-specific Format Receipts and subject-bound Transport
    Receipts. Archive and Closure Export publication outcomes retain both; a
    Transport Receipt never replaces the representation fact record.
19. Require every operation-causal authority, cache, evidence provider, Engine
    Runtime Domain, execution facility, spool, and transport adapter to expose
    one immutable role-specific capability descriptor from the exact object.
    Admission freezes a separate requested/admitted record; reports append only
    reached facts. Classes, profiles, concrete types, placement, and success are
    never inference sources.
20. Replace flat operational inventories with six closed sections for
    admission, resources, dependency execution, attempt control, role
    execution, and reporting. Requested, admitted, not-applicable, and reached
    `D/K/Q/W/P/T` facts remain distinguishable.
21. Make collection budgets count package files, largest package members, Font
    Catalog candidates, and faces at their first retaining seam. Creation
    Request Rejection is externally inspectable and cannot appear inside a
    Creation Report.
22. Preserve origin, status, declaration ordinal, diagnostic-policy leaves,
    and safe-node joins on every Compilation Request Inventory position.
23. Make session preparation limits revision-owned. Preparation happens
    synchronously inside the reducer and yields either request rejection or a
    Prepared Compilation; ingestion failure remains tokenless. Session
    Instance, Evaluation, Attempt, Fence, Subscription Generation, and
    Publication Sequence values are opaque but inspectable, and attempt plans
    carry a synchronously revocable supersession permit.
24. Model representation and transport admission as discriminated refusal or
    admitted branches. A well-formed unsupported archive recipe retains its
    selected or asserted identity; archive assertions record
    `supplied-but-unevaluated` until exact comparison is reached. Transport uses
    six role-specific opaque receipts and a complete stage ledger; cleanup,
    residual state, and exposure are independent.

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

### Typed identities

The public Rust vocabulary follows the frozen global kind registry exactly.
Known positions use `ContentIdentity`, `ProjectTreeIdentity`,
`CompletePackageTreeIdentity`, `PackageRequirementIdentity`,
`FontRequirementIdentity`, `DiscoveryRequestCommitment`,
`DiscoveryVariantIdentity`, `DiscoveryTraceIdentity`,
`DiscoveryCoverageIdentity`, `EngineIdentity`, `PackIdentity`,
`ArchiveEncodingIdentity`, `ClosureExportTreeContentIdentity`,
`CompilationRequestCommitment`, `ExporterIdentity`,
`EngineNeutralCompilationIntentIdentity`, `CompilationIdentity`,
`CompilationArtifactIdentity`, or `CompilationResultIdentity`. A generic
`CanonicalIdentity` is never returned for one of these schema-1 positions.

Core-derived identities expose inspection only, not unchecked construction.
External parsing exists only where a caller can legitimately supply an expected
identity. Prepared Compilation exposes both Engine-Neutral Compilation Intent
Identity and Compilation Identity; artifact views expose both role-bound
Compilation Artifact Identity and exact-byte Content Identity.

## Stable Byte Values

`StableByteValue` is a finite, immutable, exact byte sequence whose complete
range remains synchronously readable for its lifetime. Its public constructors
take ownership or copy into a sealed backing:

```rust
pub struct StableByteValue(Arc<private::StableBacking>);

impl StableByteValue {
    pub fn from_vec(admission: &OrdinaryAdmission, bytes: Vec<u8>) -> Result<Self, StableByteValueConstructionError>;
    pub fn from_arc(admission: &OrdinaryAdmission, bytes: Arc<[u8]>) -> Result<Self, StableByteValueConstructionError>;
    pub fn copy_from_slice(admission: &OrdinaryAdmission, bytes: &[u8]) -> Result<Self, StableByteValueConstructionError>;
    pub fn from_static(admission: &OrdinaryAdmission, bytes: &'static [u8]) -> Result<Self, StableByteValueConstructionError>;
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
| `creation::CreationResourceLimits` | Project inventory, variants, package trees and files, largest package member, Font Containers, Font Catalog candidates and faces, discovery restarts, overrides, stable spool, retained memory |
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
The Format Receipt contract requires a resource-profile identifier even for a
generic caller, so the core records its frozen
`caller-selected-format-receipt-v1` identity when no adapter profile exists;
general creation and compilation inventories continue to report profile absent.
The immutable record may be shared across calls, but each operation creates a
new private budget ledger. Counters, reservations, pinned bytes, queue places,
and deadlines are never reused.

Limits reach the first observable seam. Project Snapshot and Creation Request
construction receive Creation Resource Limits before consuming caller
collections. Package and Font Authorities receive an operation-private
`AcquisitionBudget` through inspectable Acquisition Controls and reserve each
package file, largest member, Font Catalog candidate, face, downloaded byte,
expanded byte, spool byte, and retained byte before retaining it. Iterators do
not need an honest size hint. Cache adapters receive an equivalent
`CacheBudget`. Spool, transport, acquisition, and cache controls expose
immutable views of their admitted limits, expected identities, deadlines,
clocks, interruption sources, and role-specific commit or cleanup requirements.

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

Creation Request additionally owns validated Pack Metadata and opaque Pack
Annotations. The evidence-subject vocabulary covers metadata, annotation
membership, and each annotation payload, so mutable origins cannot change
non-identifying Pack state between discovery and issuance. Generic semantic
extension construction remains absent: only understood core-owned extension
values may enter a Pack.

Creation receives a parallel `CreationInputEvidence` value and a role-specific
evidence adapter:

```rust
pub struct CreationInput {
    /* request-bound private state */
}

// CreationInput::try_new(request, evidence) rejects mismatched evidence.

pub trait SyncCreationEvidence {
    fn descriptor(&self) -> &CreationEvidenceCapabilityDescriptor;
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
The descriptor states immutable, versioned, or revalidated stability; exact-key
and opaque-scope revalidation; polling, push, and cursor replay; race-closing
fence support; and its selected network contract.

Project Snapshot, Discovery Variant, override, inclusion, metadata, annotation,
and Creation Request builders return one inspectable Creation Request Rejection
with closed issue code, role, and optional source ordinal. They enforce every
count, largest-member, aggregate, and format ceiling while consuming the input.
Every builder that accepts raw strings or bytes also receives Ordinary Admission,
so Hostile is refused before those values are interpreted.
Rejection precedes operational admission and never appears inside a Creation
Report.

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

Operational refusal precedes a Creation Report and retains the complete request,
safe projections of every appraised role descriptor, and one closed refusal
reason. An admitted Creation Report distinguishes source change, revalidation
failure, and insufficient evidence capability; none degrades into a warning or
exposes a Pack. Its six sections report admission, resources, dependency
execution, attempt control, creation-engine execution, and reporting. They
retain requested and admitted trust, network, enforcement, limits, `D/K/Q/W/P`,
interruption, domain, isolation, deadline, queue, worker, and channel facts
without reconstructing them from a profile or concrete type.

Discovery Variant construction covers the complete Discovery Coverage Request:
target, Typst input map, Compilation Document Time, engine features, and
discovery-only Pack Overrides, plus a non-identifying label. Package and Font
Embedding Policies support all-embedded, all-external, or exact mixed
dispositions keyed by Package Specification or exact Font Container Content
Identity. The
policy is deterministic and each discovered requirement's resulting disposition
enters Pack Identity.

Pack Inspection is lossless for the validated Pack's static control contract:
discovery engine descriptor; project inventory and explicit inclusions; complete
coverage requests using exact sizes and Discovery Request Commitments; typed
Discovery Traces and their identities; package and font requirement inventories,
catalog order, descriptors, licensing, dispositions, and provenance; metadata;
understood semantic extensions; annotations; and derived guarantees. It neither
acquires object bytes nor recovers discarded raw discovery values.

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
    fn descriptor(&self) -> &PackageAuthorityCapabilityDescriptor;
    // acquire(...), revalidate(...)
}
```

This admits browser-local `Rc`-backed adapters. A native executor that spawns
the complete driver future adds `P: Send + Sync` and
`for<'a> P::Acquire<'a>: Send` at its own seam. GAT traits are intentionally not
dyn-compatible; runtime-selected adapters use concrete enums or adapter-owned
local/Send erasing wrappers rather than a second semantic contract.

The exact authority object owns its immutable
`PackageAuthorityCapabilityDescriptor` or `FontAuthorityCapabilityDescriptor`.
It carries descriptor version 1, an Operational Capability Class, ordered safe
source classes, exact evidence and watch capabilities, selected network
contract, authority-private resolution-cache policy, and safe projections of
private content caches. The descriptor is obtained from the executable object;
callers never pass a detached descriptor beside a different authority.
Operational Capability Class follows
`<reverse-dns-namespace>/<lower-kebab-path>/<positive-major>`, is ASCII and at
most 255 bytes, and is descriptive rather than proof.

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
candidates use validated finite builders that consume the active Acquisition
Controls while collecting. Those controls also issue the only externally usable
authority-bound Dependency Resolution Evidence builder; callers cannot forge
evidence by supplying an Authority Instance Identity. A Font Catalog candidate carries family, typed style,
weight, stretch, registered flags, canonical binary32 axes, and canonical
Unicode scalar coverage. Its authority-bound
`FontContainerAcquisitionIdentity` is only a lazy operational round-trip key;
the core derives and verifies exact Font Container Content Identity after a
selected container is acquired. Acquisition Budget and Acquisition Controls
construction is private to the driver; authority adapters reserve file count,
largest file, candidate, face, and byte dimensions through the supplied ledger
but cannot substitute a different one. Font Catalog success
also returns the exact Font Scan Policy and every deterministic scan diagnostic,
so omit/warn/reject behavior is never silent. Dependency Evidence Keys and
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
prepare(&OrdinaryAdmission, &CompilationResourceLimits, &Pack, CompilationRequest)
    -> Result<PreparedCompilation, CompilationRequestRejection>

run_sync(&PreparedCompilation, admitted SyncCompilationControls)
    -> CompilationReport

run_async(&PreparedCompilation, admitted AsyncCompilationControls)
    -> CompilationReport
```

`compile_sync` and corresponding adapter conveniences may compose preparation
and one attempt into `CompilationTerminal`, but they cannot flatten or discard
the stages.

Prepared Compilation and every report expose the same typed semantic request
inventory; request rejection exposes its bounded supplied inventory and ordered
issues without inventing a Compilation Identity. Attempt controls occupy a
separate six-part typed operational inventory. Reports expose current-attempt
evidence, originating-evidence availability, and either a result-owned or
partial access trace through the same closed project/package/font observation
enum and reached-scope record.

Every semantic inventory node carries its applicable origin, status, and source
ordinal. Output and Canonical Diagnostic Policy retain per-leaf origins. Engine
and Exporter retain attested origins and statuses. An invalid-declaration marker
retains its optional join to exactly one safe supplied node, so a consumer can
follow the issue without reconstructing a path, key, identity, or commitment.
The external probe destructures every field rather than hiding omissions behind
`..` patterns.

Compilation control admission is outer and reportless on refusal. Its safe
refusal records the complete operation request, every appraised descriptor, and
one closed reason. An admitted report exposes six non-interchangeable sections:
admission, resources, dependency execution, attempt control, kernel execution,
and reporting. They retain exact requested/admitted network policy, cache
policy and lookup state, authority and private-cache capabilities, `D/K/Q/W/P`,
domain selection, isolation, interruption and winner, enforcement, queue and
worker facts, and channel states.

Compilation Result exposes complete immutable semantic structure: document
target and page count; format-bearing artifact roles, Compilation Artifact
Identities, Content Identities, sizes, and bytes; the typed Compilation Access
Trace; and canonical diagnostic policy, phase, severity, kind, spans, messages,
hints, and completion. A limited Canonical Diagnostic Envelope identifies the
first omitted ordinal, phase, and limiting dimension. Identity disclosure
contains artifact metadata but never artifact bytes or a raw-report escape;
delivery receives bytes through a separate core-produced plan. Creation Report
has its own phase-ordered structured diagnostic view plus operational inventory
and reporting-channel status. Diagnostic spans distinguish Pack baseline,
committed Pack Override, and Package Requirement locations.

Compilation Request construction covers Typst inputs, selected engine features,
Compilation Document Time, a Pack Override Set, Canonical Diagnostic Policy, and
one complete PDF, PNG, SVG, or HTML output specification. Format-specific types
own page selection, PPI, bleed, pretty printing, PDF identifier and creator
modes, PDF Creation Time, standards, and tagging. The core derives target and
required HTML features and deterministically rejects the representable but
unsupported bundle feature and other contradictory combinations during
preparation.

Canonical conveniences construct already-valid values. The lower-level
`CompilationRequest::from_declarations` path boundedly retains raw paths, keys,
times, feature identifiers, output controls, origins, and declaration ordinals;
it never returns an early semantic error. Preparation then returns one ordered
Compilation Request Rejection containing every independently detectable issue
and safe supplied inventory node. Unknown Pack paths retain path and size but no
Compilation Request Commitment; malformed values never receive invented typed
values.

## Semantic Result Cache

Semantic Result Cache lookup is a real public seam because memory, persistent,
browser, and service adapters vary there. Content caches, dependency-resolution
caches, archive-encoding caches, and transport reuse stay private to their
owning adapters.

The core owns a disposable private-epoch cache-record codec. Cache adapters
store and retrieve an opaque `SemanticCacheRecord` backed by Stable Byte Value:

```rust
pub trait SyncSemanticResultCache {
    fn descriptor(&self) -> &SemanticResultCacheCapabilityDescriptor;
    fn lookup(
        &self,
        request: SemanticCacheLookupRequest<'_>,
    ) -> SemanticCacheLookupOutcome;

    fn admit(
        &self,
        request: SemanticCacheAdmissionRequest<'_>,
    ) -> SemanticCacheAdapterAdmissionOutcome;
}
```

The descriptor belongs to the exact cache object and supplies class, Cache
Isolation Domain presence, selected network contract, trusted-domain or
authenticated-record writer capability, and supported availability behavior.
The admitted policy is exactly disabled, read-only, read-write, or
rebuild-and-write. A cache hit records no engine dispatch or selected runtime
domain and preserves its originating result and trace.

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
cache outcome. The adapter-level outcome reports only reached storage facts;
core-owned record preparation failure is added only by the core. Likewise, the
core-owned `CompilationDeliveryOutcome` carries
the report beside its Transport Outcome and receipt. A delivery adapter receives
only a borrowed transfer view and returns a report-free Transport Outcome; the
core composes the report-bearing outcome after the adapter returns, so selected
disclosure cannot be bypassed through the completion type. Admission or delivery
failure cannot mutate, replace, detach from, or reclassify the report. This
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

    fn descriptor(&self) -> &CompilationExecutionFacilityCapabilityDescriptor;
    fn dispatch<'a>(
        &'a self,
        job: ReadyCompilationJob,
    ) -> Self::Dispatch<'a>;
}
```

The bound descriptor supplies class, capacity-scope class and sharing relation,
supported placement, maximum `K/Q/P`, worker overlap, Engine Runtime Domain
policy, interruption, worker protocol, parent verification and output
withholding, no-fallback, selected execution and optional worker-control network
contracts, and enforcement. The operation request separately
states requested `K/Q/P/W`; admission records requested, admitted, constrained,
and not-applicable positions. Caller-thread execution makes `K/Q/P` not
applicable, and in-process facility execution makes `P` not applicable.

Engine Runtime Domain policy and reached selection are separate. Inherited
unmanaged selection carries neither an invented identity nor width. Managed
selection carries the parent-attested exact domain identity, placement, positive
`W`, and whether an exclusive fine-timing lease was reached. Exact width is
admitted unchanged or refused; automatic width resolves before managed engine
work.

The facility may call `ReadyCompilationJob::run_in_process`, or use
`into_worker_request(parent_assigned_domain)` and the paired response verifier for an ordinary isolated
worker. It cannot construct a ready job, Compilation Dependency Snapshot,
successful completion, report, result, Engine Identity, or Exporter Identity.

Creation has a parallel role-specific facility and
`CreationExecutionFacilityCapacity` because discovery/replay queue semantics are
not Compilation Execution Facility `K`/`Q`. Both may identify the same fixed
Engine Runtime Domain, but their capacities remain non-interchangeable types and
separate report facts. Neither initializes a global scheduler for a generic
library caller.

A worker-isolated creation holds one worker lease and one domain across the
whole attempt. Compilation dispatches one ready kernel/export job. A worker
string cannot assert its own identity; the parent binds the dispatch token and
domain before encoding and verifies the complete response before exposure.

The worker protocol is operational, opaque, versioned, bounded, and verified by
the parent. Kernel isolation supplies killability and resource placement, not a
Hostile guarantee.

Memory Spool's asynchronous implementation returns a real local future capable
of awaiting repeated pending source reads; it is not constrained to `Send` or a
single immediately-ready poll.

## Representation And Transport

Representation ingress uses distinct expectation types:

```rust
pub struct PackArchiveReadExpectations { /* archive, Pack, optional recipe assertion */ }
pub struct ClosureExportImportExpectations { /* Pack verification only */ }
```

This makes archive Content Identity and asserted-recipe mismatches reachable
without permitting Archive Encoding Identity on Closure Export. Typed request
construction failure produces no receipt. A well-formed unsupported recipe is
a Representation Admission Refusal before effects or interpretation. Archive
encoding always retains the selected identity and source Pack Identity; archive
read always retains the asserted identity and reports
`supplied-but-unevaluated` until complete supported exact re-encoding and byte
comparison are actually reached. Closure Export
Tree Content Identity remains a derived representation fact; adding an expected
tree mismatch would require a new Format Receipt contract version. Invalid and
unsupported terminals carry stable validation rule codes in fixed precedence.
Core-owned role-specific Format Receipts expose a shared contract-v1 envelope
plus only the identity, verification, and file payload legal for their role.
Their admission view is discriminated: Refused owns the complete requested
controls and one reason but no admitted or reached facts; Admitted owns one
requested/admitted record and permits reached counters, identities, timing,
publication, cleanup, exposure, and structured failure facts.

Representation owns validated Pack Archive ingress, validated Closure Export
import, registered Archive Encoding Identity selection, deterministic Pack
Archive encoding, baseline-only Project Materialization plans, and lossless
Closure Export plans. The plans expose canonically ordered paths, Stable Byte
Values, and identities. Publishing those values is later transport work.

Project Materialization entries use Project Path. Closure Export uses a distinct
namespaced Closure Export Path and contains only the control record plus
deduplicated blobs; semantic roles remain in `pack.cbor`. The two finite-tree
contracts cannot be confused through one generic projection type.

Transport keeps separate sync and GAT-async roles for spooling, Pack Archive
acquisition and publication, Project Materialization publication, Closure Export
publication, and Compilation Delivery. There is no universal store, receipt, or
public receipt-payload enum. Each exact adapter object supplies a role-specific
capability descriptor and returns only role-specific reached facts. Core-owned
operation functions admit the request and construct exactly one of six opaque
subject-bound receipt types.

Each receipt has a discriminated Refused or Admitted branch. Refused retains the
complete request, safe descriptor projection, and one reason, with no admission
record or actual fact. Admitted retains one Operation Admission Record and one
sealed role-legal stage ledger over `admission`, `plan-freeze`,
`reference-resolution`, `acquisition`, `spooling`, `transfer`, `verification`,
`commit`, `cleanup`, and `complete`. A role never requested has no receipt.
Adapters return only role-specific reached-stage inputs. Their stage vocabulary
excludes core-owned `admission`, `plan-freeze`, and `complete`; core orchestration
validates role legality and composes those stages into the sealed receipt ledger.
Adapter timing uses the same restricted adapter-stage vocabulary.

Requested cleanup requirement, cleanup outcome, residual locator, and exposed
bytes are independent, so exposure and cleanup failure may coexist. Requested,
admitted, and actual commit strengths remain distinct; a weaker sink is refused
before transfer. Archive and Closure Export publication outcomes derive their
Format and Transport Receipts from one private operation record and retain both.
An archive encoder also retains any independent spool receipt rather than
folding transport into representation success.

## Compilation Session

Compilation Session owns revisions, latest-only attempt ordering, supersession,
currentness state, publication, and Last Successful Compilation. It owns no
authority, cache, watcher, subscription handle, clock, runtime, execution
facility, transport, delivery adapter, or persistent store.

Each accepted stabilized input carries one immutable `SessionPolicy` with exact
caller-selected or adapter-profile requested/admitted Compilation Resource
Limits. Accept performs bounded preparation synchronously and purely. It creates
either a request-rejection candidate or a Prepared Compilation; only the latter
can own a Session Attempt Token. A typed Session Ingestion Failure carries its
failed request-source scopes and policy but is tokenless and never prepares.

Session Instance, Revision, Evaluation, Attempt, Fence, Subscription Generation,
and Publication Sequence are separate opaque values with routing and ordering
accessors. Tokens are not externally constructible. One attempt plan binds its
Prepared Compilation and a session-owned synchronously revocable supersession
permit. Recording a newer accepted revision revokes eligibility before an
`InterruptAttempt` effect wakes or kills work; the effect is not the
linearization point.

At most one attempt is active or draining and at most one latest prepared
revision is pending. Newer input replaces an older pending revision. A matching
late completion still clears the active slot and activates the latest pending
revision even when the old evaluation cannot publish. Request rejection and
ingestion failure need no slot and may reconcile while older work drains.

The reducer protocol is explicit:

```text
request rejection, Compilation Report, or ingestion-failure candidate
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
| `Accept(Stabilized or IngestionFailure)` | `StartAttempt`, `InterruptAttempt` |
| `AttemptFinished`, `AttemptReleased` | `ReadFence` |
| `FenceReadFinished` | `ArmSubscriptions` |
| `SubscriptionsArmed` | `ConfirmFence` |
| `FenceConfirmed` | `Publish`, then `RetireSubscriptions` |
| `DependencyChanged`, `NotificationGap` | Dirties the candidate and may restart reconciliation |

Old-session completions, old-generation notifications, duplicates, and stale
fence responses are normal asynchronous races and return
`SessionTransition::Ignored`, not protocol errors. A completion for the matching
active draining token is consumed rather than ignored.

Read, subscription, and confirmation plans are inspectable by adapters. They
group exact scopes, Dependency Evidence Keys, provider cursors, generations,
and handoff strategy by the internal provider identity needed to route work;
ordinary reports still expose only report-safe descriptor projections. Read and
confirmation outcomes retain exact changed,
dirty, uncovered, and failed request-source and dependency scopes. Dependency
evidence is optional when preparation rejected before dependency resolution.
Complete polling retains a coherent set of zero or more provider cursors rather
than selecting one privileged cursor. Equality is recognized from ordinary
observations; there is no `ReconciledEqual` event.

A Semantic Result Cache hit follows exactly the same fence. It is never current
by itself. `SessionPublicationTerminalRef` preserves distinct Request Rejection,
Compilation Report, and ingestion-failure branches. Request rejection may be
current relative to fully covered mutable request sources without inventing
dependency evidence. Each publication carries Session Instance, Revision,
Evaluation, and Publication Sequence. Last Successful Compilation exposes its
originating revision, evaluation, publication sequence, and independent
currentness; a newer rejection, operation outcome, ingestion failure, or
delivery failure does not replace it, and it becomes stale when its fence no
longer holds.

The public view exposes `Running`, `Retiring`, or `Retired`, the latest revision
and evaluation, active or draining attempt, latest pending prepared revision,
publication, and Last Successful Compilation. Shutdown rejects new input and
retry, revokes unpublished work, retires active and proposed subscriptions, and
interrupts active work. Retirement completes only after every attempt or arm
operation returns, is reaped, or is abandoned with proof that no live resource
remains. Publication and subscription retirement have no completion events.

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
let creation_limits = creation::CreationResourceLimits::try_new(
    creation::CreationResourceLimitSpec { /* every explicit dimension */ },
)?;
let admitted_creation_limits =
    AdmittedOperationResourceLimits::try_caller_selected(creation_limits)?;

let project = creation::ProjectSnapshot::try_from_files(
    &admission,
    &admitted_creation_limits,
    ProjectPath::parse(&admission, "main.typ")?,
    [(ProjectPath::parse(&admission, "main.typ")?,
      StableByteValue::from_static(&admission, b"Hello")?)],
)?;

let request = creation::CreationRequest::try_new(
    &admitted_creation_limits,
    project,
    [creation::DiscoveryVariant::paged_explicit_empty()],
    creation::PackageEmbeddingPolicy::embed_all(),
    creation::FontEmbeddingPolicy::embed_all(),
    pack::PackMetadata::empty(),
    [],
)?;

let evidence = creation::CreationInputEvidence::caller_owned_immutable(&request);
let input = creation::CreationInput::try_new(request, evidence)?;

let creation_controls = creation::SyncCreationControls::try_admit(
    admission.clone(),
    admitted_creation_limits,
    &creation_evidence,
    &package_authority,
    &font_authority,
    creation_operation_request,
    &clock,
    &interruption,
)?;

let pack = creation::create_sync(input, creation_controls)
    .into_pack()
    .map_err(CreateError::from_report)?;

let prepared = pack.prepare(&admission, &compilation_limits, compilation_request)?;
let report = compilation::run_sync(&prepared, compilation_controls);
```

An asynchronous browser worker uses the same authority contracts with local,
non-`Send` futures, Memory Spool, one local Engine Runtime Domain, and an inline
ready-job facility. A native service adds `Send` bounds at its executor seam and
may choose native spooling and an isolated facility. Neither target type enters
the semantic values or base trait definitions.

Post-commit work is explicit:

```rust,ignore
let cache_outcome = compilation::admit_to_cache_sync(
    report.clone(),
    &cache,
    &clock,
    &interruption,
);

let plan = CompilationReportDisclosure::identity()
    .plan_delivery(report.clone(), compilation_limits);
let delivery_outcome = transport::deliver_compilation_sync(
    plan,
    &delivery,
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
  -D warnings \
  --crate-type=lib \
  --crate-name=typst_pack_interface \
  PROTOTYPE-rust-lifecycle-adapter-interfaces.rs \
  -o /tmp/libtypst_pack_interface.rlib

rustc --edition=2024 \
  -D warnings \
  PROTOTYPE-rust-lifecycle-adapter-consumer.rs \
  --extern typst_pack_interface=/tmp/libtypst_pack_interface.rlib
```

The downstream probe constructs every authority, cache, evidence, runtime,
creation, compilation, spool, and transport descriptor family; the common sync
lifecycle; session preparation and ingestion branches; sync trait objects; and
external authority, evidence, delivery, and publication roles. It proves both a
local non-`Send` GAT future and a call-site `Send` future. It traverses complete
Pack Inspection; every semantic inventory origin, status, declaration ordinal,
diagnostic leaf, and safe-node join; all six creation and compilation
operational sections; complete and partial traces; evidence; artifacts;
disclosure; request rejection; session instance/evaluation/token/publication
views; representation refusal and assertion states; every role-specific
Transport Receipt admission and stage-ledger branch; cleanup, residual and
exposure; and paired publication receipts. The public adapter traits return
reached facts and cannot construct a receipt.
Runtime behavior remains deliberately unimplemented because this map plans the
destination rather than building it.

The same two commands pass under `rustc 1.92.0` in the official `rust:1.92`
container, matching the repository's declared `rust-version`.

## Planning Consequence

This contract is sufficient for
[Regenerate first-party adapters and watch semantics](https://github.com/sagikazarmark/typst-pack/issues/69)
to serialize the core-owned views and receipts without inventing lifecycle or
identity semantics. It introduces no new decision ticket and leaves no new
in-scope fog. Exact private layout, helper decomposition, cache eviction,
allocator strategy, and benchmark tuning below the frozen limits remain
implementation choices.
