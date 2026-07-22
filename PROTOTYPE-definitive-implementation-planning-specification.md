# PROTOTYPE: Definitive Implementation-Planning Specification

> Throwaway connective design artifact for
> [Assemble and approve the definitive implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/81),
> the final decision in
> [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27).
> It approves an immutable contract set for implementation planning. It is not
> runtime implementation, a production serializer or corpus, a migration tool,
> a release, or a compatibility promise for the current 0.3 surfaces.

## Question

What single connective specification joins the corrected product, architecture,
vocabulary, format, identity, Rust, first-party adapter, verification, and
clean-break migration decisions without changing their accepted semantics or
concealing remaining implementation work?

## Verdict

Approve this specification and its immutable authority bundle for implementation
planning. It is decision-complete for the destination: the product contract,
seven-module architecture, public lifecycle, first-release claims and nonclaims,
failure and fact precedence, canonical vocabulary, target ADR disposition,
verification ownership and gates, and clean-break migration are fixed.

Implementation must reproduce these contracts rather than infer behavior from
prototype bodies, profiles, concrete types, success statuses, or historical
artifacts. Private layout, helper decomposition, allocator strategy, cache
eviction, benchmark tuning below frozen limits, and equivalent internal
algorithms remain implementation choices.

## Scope

This specification freezes what `0.4.0` must mean and how each shipped claim must
be verified. It includes:

- the library-first product and quality contract;
- Pack creation, validity, representation, compilation, transport, and Session
  lifecycle semantics;
- exact public module ownership and cross-module seams;
- Pack Format Epoch 2 and compilation-family identities;
- first-party CLI and Dagger contracts;
- the first-release bill of materials, claims, and explicit nonclaims;
- verification ownership, test combinations, and release gates; and
- a coordinated clean break from signed `v0.3.1` to `0.4.0`.

It does not implement the seven modules, production serializers, schemas,
corpora, adapters, tests, migration, packaging, release, or deployment.

## Normative Authority

### Immutable bundle

The destination is defined by four immutable artifact commits plus the accepted
issue resolutions they realize:

| Subject | Immutable authority | Exact scope |
| --- | --- | --- |
| Pack Format Epoch 2 | [`a490abc`](https://github.com/sagikazarmark/typst-pack/commit/a490abc80af173422049ced1bf02585ddf7fc298) | Canonical CBOR, global kinds 1-13, narrow ZIP and ZIP64 profile, Stored/Deflate reading, Closure Export, format ceilings, validation precedence, identities, and base Format Receipt semantics. |
| Compilation-family registry | [`a482145`](https://github.com/sagikazarmark/typst-pack/commit/a482145a0c790bb67d7f6d4424777519c614876e) | Global kinds 14-19, compilation value projections, identity transcripts, SHA-256 payload ceiling, independent vectors, evidence, disclosure, and comparison contracts. |
| Initial Epoch 2 writer and corpus delta | [`23d8bea`](https://github.com/sagikazarmark/typst-pack/commit/23d8bea991c0d55f3aeaee4cd3137c67c8d9d496) | Exactly one first-release all-Stored writer and its Archive Encoding Identity; required Stored, Deflate, and mixed-method reader corpus additions. |
| Corrected Rust and first-party adapter contracts | [`95d5bb0`](https://github.com/sagikazarmark/typst-pack/commit/95d5bb06d08db51e599fffbaf7f4c0c56c7441c9) | Exact Rust 1.92 public shape, seven modules, lifecycle and receipt interfaces, CLI/Dagger contract, strict schemas, first-party profiles, serializer provenance, Session/watch protocol, and locked planning replay. |

The bundle deliberately does not exist in one Git tree. Immutable links join it;
copying historical artifacts into one tree would create a second uncontrolled
authority.

### Clause-level precedence

1. Accepted issue resolutions control the product or domain question each issue
   names. The map's
   [Decisions so far](https://github.com/sagikazarmark/typst-pack/issues/27)
   is the index; each linked ticket owns its detail.
2. [`a490abc`](https://github.com/sagikazarmark/typst-pack/commit/a490abc80af173422049ced1bf02585ddf7fc298)
   controls the base Epoch 2 format.
3. [`a482145`](https://github.com/sagikazarmark/typst-pack/commit/a482145a0c790bb67d7f6d4424777519c614876e)
   extends that format's global kind registry only for compilation-family kinds
   14-19 and their exact projections and transcripts.
4. [`23d8bea`](https://github.com/sagikazarmark/typst-pack/commit/23d8bea991c0d55f3aeaee4cd3137c67c8d9d496)
   changes the base format only for the initial writer registry and completed
   interoperability corpus: the writer is all-Stored, while readers still accept
   conforming Stored, Deflate, and mixed-method Epoch 2 archives.
5. [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71)
   and [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70)
   control the requested, descriptor, admission, reached, representation, and
   transport baselines except where the following corrections apply.
6. [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77)
   controls preparation-before-admission, the five fact sources, Font Scan
   Policy, capability scope, placement, isolation, Engine Runtime Domain
   selection, refusal stage, subject object count, paired publication receipts,
   and cleanup requirement/outcome over every earlier lifecycle artifact.
7. [Define aggregate creation and representation resource accounting](https://github.com/sagikazarmark/typst-pack/issues/76)
   controls category-plus-aggregate Creation accounting, logical and physical
   representation dimensions, occupancy, all-Stored planning, and same-profile
   coherence over earlier category-only or generic representation limits.
8. [`a92c6b3`](https://github.com/sagikazarmark/typst-pack/commit/a92c6b3),
   adopted by [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78),
   remains the exact baseline only for unaffected Rust, schema, GraphQL-target,
   profile, serializer, and replay details inherited by its correction.
9. [`95d5bb0`](https://github.com/sagikazarmark/typst-pack/commit/95d5bb06d08db51e599fffbaf7f4c0c56c7441c9),
   adopted by [Correct the final Rust and first-party contract realization](https://github.com/sagikazarmark/typst-pack/issues/80),
   is the exact Rust and first-party adapter authority. It replaces
   [`a92c6b3`](https://github.com/sagikazarmark/typst-pack/commit/a92c6b3)
   only for complete reached Creation Resource projection, exact
   profile-attributed Session preparation, the sealed token-bound Session
   attempt-admission and completion seam, corrected failure precedence, and
   corresponding examples, schemas, profiles, and replay. Unaffected clauses
   remain inherited.
10. [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42)
   controls verification ownership and evidence standards.
11. [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43)
   controls migration and release ordering.
12. A newer artifact changes an older clause only through an explicit,
   subject-limited supersession edge. Date, ancestry, executable form, generated
   appearance, or passing prototype checks does not establish precedence.
13. The rejected review at
   [`926a49f`](https://github.com/sagikazarmark/typst-pack/blob/926a49fd73e3d781764d4e5df9d5194ca3147be4/PROTOTYPE-final-implementation-planning-specification-review.md)
   is traceability and defect evidence only. Its connective requirements are
   incorporated here; its reached-resource and Session blocker statements were
   closed by [`95d5bb0`](https://github.com/sagikazarmark/typst-pack/commit/95d5bb06d08db51e599fffbaf7f4c0c56c7441c9).
14. This specification resolves only cross-artifact connection, precedence,
    claims, gates, and disposition. If it appears to restate an exact field,
    schema, value, or algorithm differently from its subject authority, that
    authority controls and the discrepancy is a specification defect.

## Product Contract

### Purpose and priorities

`typst-pack` packages and executes reusable Typst compilation closures. The Rust
library is the primary interface, the CLI is the primary first-party adapter,
and Dagger is the typed CI adapter. Browser/WASM and service requirements shape
the featureless core without adding target-specific semantic dependencies or a
first-release browser or service product.

The quality order is semantic depth, portability, reproducibility,
observability, and predictable operation before convenience, throughput,
compactness, or compatibility with provisional surfaces.

### Pack contract

A Pack is an immutable, canonically validated, replay-verified compilation
closure with one fixed entrypoint. It contains every project file reached by its
successful Discovery Variants or named by Explicit Conditional Inclusions,
records exact package and font requirements, and permits no undeclared ambient
dependency fallback.

A Pack exists only after Pack Issuance or complete validation of a supported Pack
Archive or Closure Export. There is no public draft, mutable, partially valid,
updateable, or unchecked Pack and no Resource Slot or Resource Provider role.

Every valid Pack is portable: another host can compile it when exact declared
external package and font fulfillments are supplied. A Pack is self-contained
only when every package and font dependency is embedded. Neither property alone
claims that an attempt is offline, environment-independent, exactly
reproducible, cross-engine compatible, authenticated, or confined.

Pack Identity commits to canonical logical compilation state and excludes
representation encoding, source-host metadata, acquisition locations,
non-identifying provenance, metadata, and annotations.

### Creation

Adapters stabilize a finite Project Snapshot, explicit Discovery Variants,
semantic request values, metadata and annotations, and explicit Package and Font
Authorities before the semantic seam. Creation then:

1. admits exact controls and bound facility descriptors;
2. executes each ordered Discovery Variant in isolation;
3. performs bounded package/font acquire-and-restart while discarding partial
   traces;
4. retains only causal evidence;
5. establishes a race-closing Creation Evidence Fence;
6. constructs the whole Pack through one private invariant seam;
7. replays every variant against the frozen Discovery Snapshot without
   reacquisition; and
8. exposes the Pack only at Pack Issuance.

Request construction rejection precedes operation admission. Operation refusal
precedes a Creation Report. An admitted creation failure remains a Creation
Report and exposes no Pack.

### Variation and dependencies

One immutable Pack-bound Pack Override Set is the only compilation-scoped
project variation. Overrides may replace bytes for existing project paths,
including the entrypoint, but cannot add, delete, or rename paths or modify
package or font content.

Package and font fulfillment is exact, authority-mediated, independently
validated, and fail-closed. Authorities own source routing and private caches;
only the closed Unavailable outcome permits fallback. Semantic modules never
consult ambient files, packages, fonts, environment, clocks, caches, or network.

### Compilation lifecycle

```text
validated Pack + exact semantic request
`-- pure bounded preparation under exact Preparation Policy and Limits
    |-- Compilation Request Rejection
    `-- Prepared Compilation
        `-- fresh Compilation Operation admission
            |-- reportless Compilation Admission Refusal
            `-- admitted Compilation Attempt
                `-- exactly one immutable Compilation Report
                    |-- Compilation Result
                    `-- Compilation Operation Outcome
```

Request Rejection creates no Prepared Compilation, Compilation Identity,
operation admission, report, dependency evidence, or trace. Deterministic
compiler or exporter rejection is a Compilation Result. Dynamic authority,
resource, cancellation, deadline, execution, isolation, or integrity failure
before Compilation Terminal Commitment is a Compilation Operation Outcome.

Synchronous and asynchronous drivers acquire and verify exact dependencies,
construct a private immutable Compilation Dependency Snapshot, and invoke the
same private synchronous Compilation Kernel. The kernel has no ambient I/O,
clock, environment, mutable source, asynchronous interface, or public engine or
exporter seam.

Cache admission, delivery, publication, disclosure, rendering, viewer launch,
and response transport occur after terminal commitment. They retain and cannot
mutate, detach, replace, or reclassify the Compilation Report.

### Identity and reproducibility

Identity equality always includes the typed kind, identity schema, algorithm,
and digest. A generic digest, version string, successful run, profile, placement,
or concrete implementation type is never enough.

An Exact Reproducibility Claim is an explicit relation from one Compilation
Identity to a baseline Compilation Result Identity. Cross-engine claims compare distinct
Compilation Identities derived from one Engine-Neutral Compilation Intent at one
named cumulative level: Request Compatible, Closure Compatible, Structurally
Compatible, or Exactly Reproducible. No broader claim is inferred.

### Representation and transport

Pack Archive and Closure Export are strict Epoch 2 representations of one
logical Pack and share byte-identical canonical `pack.cbor`. Representation owns
validation, encoding, and finite plans. Transport separately owns locator
resolution, acquisition, spooling, transfer, commit, cleanup, and delivery.

Every well-formed attempted role produces its exact role-specific receipt.
Format and Transport Receipts are separate sibling facts; neither replaces the
other. Request, immutable subject, descriptor from the exact bound object,
admission, and reached ledger are five non-interchangeable fact sources.

### Caching and Sessions

The only public semantic cache seam stores complete immutable semantic results.
Lookup happens before commitment; cache admission is a distinct post-commit
operation. A hit preserves its original result and trace and is historical
evidence, not proof of Session Currentness.

A Compilation Session is a caller-owned, Pack-bound synchronous reducer. It owns
revisions, evaluations, latest-only scheduling, supersession, fences,
currentness, publication, Last Successful Compilation, and retirement. It owns
no authority, cache, watcher, clock, runtime, execution facility, transport,
delivery adapter, or persistent store.

Each revision owns one exact profile-attributed Session Preparation. Accept
performs the same pure preparation as one-shot compilation. A Session Attempt
Plan is the sole fallible token-bound attempt-admission seam; it yields either a
token-bound reportless refusal or an admitted attempt, and only that admitted
attempt can create the matching Session Attempt Completion.

Currentness follows `ReadFence -> ArmSubscriptions -> ConfirmFence -> Publish`.
Cache success, report success, a notification, or an adapter profile cannot
prove currentness. Newer accepted work synchronously revokes publication
eligibility before interruption is requested; late matching completion still
clears the draining slot without publishing.

## Seven-Module Architecture

The `typst-pack` crate exposes exactly seven public modules:

| Module | Owns | Complexity hidden behind its interface |
| --- | --- | --- |
| `creation` | Project Snapshot, request construction and rejection, input evidence, creation limits and accounting, sync/async creation, reports, and Pack Issuance | Discovery World and Snapshot, isolated variants, package/font restart convergence, causal evidence selection, fencing, whole-Pack assembly, replay, and issuance. |
| `pack` | Opaque Pack, Pack Identity, complete immutable Pack Inspection, metadata, annotations, and thin inspect/prepare conveniences | Canonical logical Pack state and one private whole-Pack construction seam shared by issuance and validated ingress. No builder, manifest constructor, mutation, or unchecked constructor. |
| `authority` | Separate sync and GAT-async Package and Font Authority roles, complete package trees, font catalogs and containers, fulfillment, provenance, evidence, and bound capability descriptors | Source routing, fallback, acquisition, and authority-private resolution/content caches. No file-at-a-time Typst World interface and no global `Send` or `Sync` bound. |
| `compilation` | Preparation, request rejection, Prepared Compilation, sync/async controls and drivers, reports/results/outcomes, semantic-result cache, disclosures, delivery plans, and role execution facilities | Defaults, override preflight, request identities, dependency snapshots, the single synchronous kernel, exporters, diagnostics, traces, and terminal commitment. |
| `representation` | Epoch 2 archive ingress, Closure Export import, registered archive encoding, Project Materialization and Closure Export plans, role limits, and Format Receipts | Canonical CBOR, narrow ZIP, validation precedence, logical/physical accounting, representation identities, and deterministic planning. It does not publish destinations. |
| `transport` | Stable Byte Value, bounded sync/async sources and spooling, six role-specific transport adapters and receipts, publication, and Compilation Delivery | Sealed backing layouts, backpressure, verification, commit, cleanup, residual/exposure facts, and locators. There is no universal store, receipt payload union, or third-party backing trait. |
| `session` | Compilation Session, exact Session Preparation, revisions and evaluations, token-bound attempt plans, fencing and subscriptions, currentness, publication, Last Successful Compilation, and retirement | Reducer state, latest-only scheduling, supersession, routing tokens, fence generations, and publication linearization. It cannot accept an independently paired token and report. |

The crate root re-exports only the lifecycle values most callers carry:

```rust
CompilationOperationOutcome
CompilationReport
CompilationResult
CompilationTerminal
PreparedCompilation
Pack
PackIdentity
PackInspection
CompilationSession
StableByteValue
```

There is no generic public `memory`, `storage`, `runtime`, `world`, `engine`,
`kernel`, `manifest`, or `adapter` module.

The package topology is:

```text
typst-pack       featureless semantic core, representations, drivers,
                 sealed memory/native stable backings, memory helpers
typst-pack-fs    source stabilization, confined filesystem authorities,
                 watch and publication adapters, native-spool policy
typst-pack-cli   first-party CLI composition and presentation
Dagger module    tagged typed CI adapter over immutable File and Directory
consumer crates service, browser, and product-specific adapters
```

### Cross-module invariants

- Semantic values and identities never absorb locators, queues, mutable-source
  handles, profile defaults, timing, cache telemetry, or output destinations.
- Operation Capability Descriptors come from exact executable objects. Classes,
  profiles, concrete types, placement, and success never manufacture requested,
  admitted, selected, enforced, or reached facts.
- Reusable admitted-limit configuration creates fresh private counters and
  reservations for every operation. Each limit is enforced at the first seam
  that can observe and retain the value.
- Creation Reports expose all twenty reached resource dimensions: project files,
  aggregate project bytes, largest project file, packages, package files,
  largest package file, package-tree bytes, font containers, font candidates,
  font faces, font bytes, Discovery Variants, discovery restarts, aggregate file
  bindings, aggregate logical bytes, override count, largest override,
  aggregate override bytes, peak stable-spool bytes, and peak retained memory.
- Whole-Pack construction, discovery and issuance, compilation preparation,
  dependency-snapshot construction, Compilation Kernel, terminal commitment,
  Epoch 2 codecs, Stable Byte Value backing, cache-record codec, and Session
  reducer state remain private seams.
- Deleting one of the seven modules would redistribute its policy across callers
  or adapters. Exposing arbitrary engines, Worlds, stores, mutable sources,
  watchers, or Stable Byte Value backings would create a second semantic path.

## Failure and Fact Precedence

First-party adapters apply this ordering and skip inapplicable steps:

1. Parse argument or GraphQL shape and trust token without opening content.
2. Refuse unsupported trust or pre-parse enforcement capability.
3. Resolve and validate side-effect-free profiles and limits.
4. Normalize adapter defaults and statically collision-check plans.
5. Admit each requested acquisition, spool, or transport role before its effects.
6. Boundedly stabilize raw inputs.
7. For creation, construct the bounded Creation Request; return Creation Request
   Rejection before operation admission or a report.
8. Admit creation against exact bound descriptors and exact width; refuse rather
   than lower unavailable exact requests.
9. Run admitted creation to one Creation Report; failed issuance exposes no Pack.
10. Admit representation, including selected or asserted recipe support.
11. Check an expected Archive Content Identity when requested.
12. Validate framing, canonical control, objects, whole-Pack invariants, and
    unsupported state in Epoch 2 precedence.
13. Check an expected Pack Identity after complete derivation.
14. Verify an asserted archive recipe only by supported exact re-encoding and
    byte comparison.
15. Stabilize overrides and resolve every adapter semantic default.
16. Run pure preparation under exact Compilation Preparation Policy and Limits;
    Request Rejection wins before operational appraisal.
17. Admit only the resulting Prepared Compilation; refusal retains its
    Compilation Identity but creates no terminal or report.
18. Run one admitted attempt and commit one immutable Compilation Report.
19. Cancellation, deadline, or supersession recorded before terminal commitment
    wins; a later signal cannot mutate the committed terminal.
20. Cache admission, delivery, publication, disclosure, rendering, viewer, and
    response transport are later operations and cannot reclassify the report.
21. Commit wins over later interruption. Cleanup never replaces an earlier
    primary failure, but may be the first failure after successful commit while
    commit and exposure remain true.

At every operation seam, the complete request controls requested facts, the
bound descriptor controls offered facts, the Operation Admission Record controls
admitted facts, and the reached ledger controls reached facts. No other source
may fill a missing position.

## Canonical Vocabulary Snapshot

[`CONTEXT.md`](./CONTEXT.md) in the same immutable specification commit is the
canonical vocabulary snapshot. Exact contract names use those terms without
synonyms. This final snapshot adds the previously missing connective terms:

- Operational Capability Class;
- Operation Capability Descriptor;
- Operation Admission Record;
- Operation Network Policy;
- Format Receipt;
- Representation Admission Refusal;
- Compilation Preparation Policy;
- Compilation Preparation Limits;
- Session Evaluation;
- Session Ingestion Failure;
- Superseded Session Attempt; and
- Session Retirement.

`SessionAttemptCompletion` remains an exact Rust contract name rather than a
domain glossary term. `Transport Cleanup Requirement` and `Transport Cleanup
Outcome` retain their existing definitions. `CONTEXT.md` describes domain
meaning only; exact Rust fields and wire spellings remain in the corrected
contract artifacts.

## Target ADR Disposition

Implementation must record the destination as `ADR-0006: Adopt the library-first
Epoch 2 architecture`. This planning artifact fixes the disposition but does not
rewrite current ADR history.

| ADR | Target status after ADR-0006 is accepted | Disposition |
| --- | --- | --- |
| ADR-0002: Make Pack the Owner of Whole-Pack Invariants | Partially superseded by ADR-0006 | Retain the single private whole-Pack construction seam. Supersede Epoch 1 TOML Manifest, old ZIP and unknown-entry assumptions, Resource Slot, and old builder details with Epoch 2 creation and representation contracts. |
| ADR-0003: Centralize External Resource Reference Semantics | Superseded by ADR-0004 | Preserve the direct historical edge. ADR-0004, not ADR-0006, was the decision that immediately replaced it. Note that ADR-0004 is later superseded. |
| ADR-0004: Model Resource Slots and Resource Providers | Superseded by ADR-0006 | Remove Resource Slot and Resource Provider behavior entirely. Preserve the original record unchanged as history. |
| ADR-0005: Align the CLI with Embedded Typst | Partially superseded by ADR-0006 | Retain exact embedded-Typst parity where semantics match and the `create` and `compile` commands. Supersede the two-command-only shape, obsolete options, Resource Slot behavior, deferred watch, and old Dagger shape. |

ADR-0006 and the historical ADR status edits must state these exact reciprocal
edges without rewriting the original Context, Decision, or Consequences.

## First-Release Claim Manifest

### Bill of materials

- One coordinated `0.4.0` train: featureless `typst-pack`, `typst-pack-fs`, and
  `typst-pack-cli`, with the Dagger module from the same immutable repository tag.
- Rust `1.92` as the minimum supported Rust version.
- Typst `0.15.0` as the embedded CLI parity baseline.
- CLI commands `create`, `inspect`, `compile`/`c`, `watch`/`w`, `materialize`,
  and `convert`. There is no `validate`, generic `extract`, or generic transport
  command.
- Dagger immutable lifecycle objects over `File` and `Directory`. Modeled
  failures remain queryable; only `requirePack`, `requireArchive`, `requireTree`,
  `requireProject`, and `requireSuccess` deliberately raise.
- Adapter profiles exactly `native-cli/1` and `dagger-ci/1`; stable first-party
  JSON major `1`. Profiles are admission inputs and ceilings, never report facts.
- Partially Trusted as the first-party default. Every ordinary Hostile request is
  refused as `HostileUnavailableInFirstRelease` before content interpretation.
- First-party Semantic Result Cache disabled with no CLI flag and no inferred
  Dagger graph-cache behavior.
- Pack Format Epoch 2 writing with only selector `epoch-2-all-stored-v1`, constant
  `EPOCH_2_ALL_STORED_V1`, recipe `org.typst-pack.archive.all-stored` epoch `1`,
  and Archive Encoding Identity
  `typst-pack:archive-encoding:1:sha256:4e338d8a54d234ca28392ecf79386944757e0e4adf750192e21311d6b2491170`.
- Epoch 2 reading of conforming Stored, Deflate, and mixed-method archives. No
  Deflate writer recipe is registered.
- Stable Byte Value backing on every target through static, contiguous immutable
  memory, chunked immutable memory, and Memory Spool; Linux, macOS, and Windows
  also expose target-gated core-owned `transport::native::NativeSpool`.

### Claims that require positive evidence

- Packs are canonically valid, immutable, portable, closed over their recorded
  coverage, and replay-verified before issuance.
- Self-contained Packs need no external package or font fulfillment.
- Exact dependency bytes and outcomes reach one featureless synchronous kernel
  through semantically equivalent synchronous and asynchronous drivers.
- Every supported representation validates to the same logical Pack and Pack
  Identity; the sole writer emits its exact registered encoding.
- Typed identities, results, diagnostics, evidence, receipts, and Session
  currentness obey their frozen projections and precedence.
- Each advertised commit, cleanup, interruption, isolation, offline, platform,
  and target guarantee holds at its named strength.
- CLI and Dagger expose only their frozen adapter semantics and obtain every wire
  fact from an approved source.

### Explicit nonclaims

- The planning fixtures are not runtime implementation, production code,
  generated Dagger parity, a materialized interoperability corpus, or proof of
  production behavior.
- No browser or service adapter ships. A WASM-suitable core is not a browser
  product claim.
- No browser Blob, OPFS, WASI, arbitrary native backing, or third-party Stable
  Byte Value backing is supported.
- No ordinary native, CLI, OCI/Dagger, browser, or in-process WASM surface claims
  Hostile handling. An isolated worker claims killability and resource placement,
  not hostile-input confinement.
- In-process execution makes no hard whole-attempt CPU, memory, crash-containment,
  or prompt non-cooperative-kernel cancellation claim.
- Pack validity and expected-identity verification do not authenticate a
  publisher, provide remote attestation, prove confinement, or make emitted
  artifacts, diagnostics, or archives safe for downstream parsers.
- Portability and self-containment do not imply exact output equality across
  engines or platforms and do not prove that an operation used no network.
- Dagger has no watch or Session surface, terminal UI, stdio, viewer, host-path
  facade, Hostile mode, persistent result cache, generic store/publication,
  Resource Slots/Providers, Epoch 1, migration/repair, or generic semantic
  extension interface.
- A returned Dagger `File` or `Directory` proves only immutable container-local
  staging, not host export or external publication.
- There is no Resource Slot compatibility surface, Epoch 1 reader, archive
  conversion, public Pack builder, Pack World, public Typst World, generic
  engine/exporter seam, compatibility alias, or dual serializer.

## Verification Contract

### Evidence ownership

| Owner | Evidence it owns |
| --- | --- |
| Native Rust | Semantic authority for Pack creation and invariants, identities, authorities, compilation, representations, transport, Sessions, public-seam properties and models, fault injection, compatibility, and resource boundaries. Shared fixtures use public interfaces and introduce no friend seam or unchecked constructor. |
| CLI and native adapters | Filesystem, process, environment, terminal, watch, publication, packaging, and embedded-Typst parity behavior introduced by those adapters. Production serializers prove every leaf comes from a Rust accessor/branch, explicit adapter input, closed derivation, or schema constant. |
| Dagger | A deliberately small typed-adapter contract: schema and omissions; representative creation, ingress, and inspection; one Document Format and one multi-artifact Page Format; one override and external-authority path; Project Materialization and Closure Export return shapes; queryable failures and `require*`; profile admission; staging; and Hostile refusal. |
| Platform and deployment suites | Only operating-system, runtime, confinement, atomicity, quota, process, and watcher guarantees claimed for that platform. |

Dagger does not own archive corpora, identity properties, authority permutations,
all output controls, resource boundaries, Session models, filesystem races,
browser/service behavior, cross-platform release testing, or benchmarks. Running
native suites through Dagger is orchestration, not duplicate semantic evidence.

### Claim-to-gate matrix

| Claim family | Accepted authority | Existing planning evidence, not runtime proof | Required implementation evidence | Gate |
| --- | --- | --- | --- | --- |
| Pack closure, discovery, fence, replay, issuance | [Pack contract](https://github.com/sagikazarmark/typst-pack/issues/32), [creation](https://github.com/sagikazarmark/typst-pack/issues/34), [evidence and coverage](https://github.com/sagikazarmark/typst-pack/issues/60) | Corrected public type flow and accepted creation decisions | Native public-seam/property tests, ordered variants, restart convergence, mutable-source/missing-probe faults, assembled-Pack replay, and no premature Pack exposure | Alpha vertical path; complete matrix at RC |
| Packages, fonts, overrides, authorities, provenance | [Packages](https://github.com/sagikazarmark/typst-pack/issues/45), [fonts](https://github.com/sagikazarmark/typst-pack/issues/38), [overrides](https://github.com/sagikazarmark/typst-pack/issues/41), [evidence](https://github.com/sagikazarmark/typst-pack/issues/48) | Constructible traits and consumer probe | Native conformance, malformed inputs, fallback only after Unavailable, embedding/fulfillment matrix, provenance invariance, override preflight and use | Feature-complete at beta; exhaustive matrix at RC |
| Epoch 2 validity and representation convergence | [Epoch 2](https://github.com/sagikazarmark/typst-pack/issues/57) | Frozen normative format, registry, validation, and corpus contracts | Materialized independent valid/invalid/boundary corpus, two-decoder agreement, parser properties, malformed vectors, fuzzing, Archive/Closure identity convergence | Hard gate before any public alpha writes Epoch 2 |
| All-Stored writer and Archive Encoding Identity | [Writer and corpus](https://github.com/sagikazarmark/typst-pack/issues/64) | Frozen sole writer, exact identity, and planning arithmetic | Byte-exact golden reproduction plus complete Stored, Deflate, mixed reader, unsupported-recipe, and read-receipt vectors | Hard gate before public alpha |
| Compilation-family identities and reproducibility | [Identity registry](https://github.com/sagikazarmark/typst-pack/issues/66), [reproducibility](https://github.com/sagikazarmark/typst-pack/issues/49) | Frozen kinds 14-19, transcripts, typed identities, and independent vectors | Reproduce every vector; included/excluded-field properties; environment perturbation; finite Engine-Neutral Intent corpus; separate proof for every advertised level | Identity vectors at alpha; all claims at RC |
| Preparation, compilation, diagnostics, disclosure | [Compilation](https://github.com/sagikazarmark/typst-pack/issues/40), [reporting](https://github.com/sagikazarmark/typst-pack/issues/59), [corrected contract](https://github.com/sagikazarmark/typst-pack/issues/80) | Corrected terminal shape, schemas, artifacts, diagnostics, evidence, seven disclosure channels | Native all-format terminal matrix, aggregate rejection, Result/Outcome distinction, commitments, redaction, diagnostic limits, zero-artifact pages, immutable post-commit behavior | Vertical path at alpha; feature-complete beta; full matrix at RC |
| Sync/async, facilities, interruption, isolation | [Execution](https://github.com/sagikazarmark/typst-pack/issues/46), [parallelism](https://github.com/sagikazarmark/typst-pack/issues/55), [lifecycle corrections](https://github.com/sagikazarmark/typst-pack/issues/77) | Role-specific facility and `D/K/Q/W/P` shape | Semantic equivalence, explicit-clock/barrier faults, commitment races, queue/worker failures, cancellation, termination, reaping, and no hidden work after return | Paths at beta; every strength proven at RC |
| Representation, transport, receipts, publication | [Projection](https://github.com/sagikazarmark/typst-pack/issues/35), [transport](https://github.com/sagikazarmark/typst-pack/issues/52), [admission](https://github.com/sagikazarmark/typst-pack/issues/70), [receipt corrections](https://github.com/sagikazarmark/typst-pack/issues/77) | Selected refusal, receipt, stage, count, cleanup, commit, and fence replay | Native role conformance, stage fault injection, backpressure, interruption races, cross-receipt mutation rejection, and filesystem atomicity for each sink | Roles at beta; every sink/strength at RC |
| Sessions, watch, cache, currentness | [Sessions](https://github.com/sagikazarmark/typst-pack/issues/51), [cache topology](https://github.com/sagikazarmark/typst-pack/issues/54), [preparation](https://github.com/sagikazarmark/typst-pack/issues/68), [corrected contract](https://github.com/sagikazarmark/typst-pack/issues/80) | Corrected Session Preparation, plan, admission, completion, schemas, replay, and structural viewer | Executable reducer refinement, generated events, read/arm/confirm races, notification gaps, latest-only promotion, cache authorization/corruption, Current and Last Successful Compilation/Delivery | Reducer/cache topology at beta; native watch/currentness at RC |
| Aggregate resources and performance | [Scale and limits](https://github.com/sagikazarmark/typst-pack/issues/50), [aggregate accounting](https://github.com/sagikazarmark/typst-pack/issues/76), [complete reached view](https://github.com/sagikazarmark/typst-pack/issues/80) | Profiles, all twenty reached Creation fields, arithmetic, occupancy, and round-trip planning replay | `limit-1/limit/limit+1`, overflow, lying iterators, occupancy transfer/release, logical/physical deduplication, same-profile round trips, production-equivalent benchmarks | Boundary suite on every change; pinned benchmarks at RC |
| Trust, integrity, confinement | [Trust](https://github.com/sagikazarmark/typst-pack/issues/47), [platform confinement](https://github.com/sagikazarmark/typst-pack/issues/53) | Integrity types, platform research, mandatory Hostile refusal | Integrity/substitution tests, archive/path/font/protocol fuzzing, real OS and production-equivalent enforcement tests; explicit refusal where positive evidence is absent | Every shipped claim proven or refused at RC; skip is not success |
| Rust modules, packages, targets, MSRV | [Module architecture](https://github.com/sagikazarmark/typst-pack/issues/39), [corrected contract](https://github.com/sagikazarmark/typst-pack/issues/80) | Seven-module fixture, external consumer, GAT/local-future and compile-fail probes, recorded Rust 1.92 checks | Exact Rust 1.92 workspace, forbidden-seam compile-fail suite, downstream consumer, featureless `wasm32-unknown-unknown` build and actual browser execution for any claimed core path | Graph/interface at alpha; interface, MSRV, targets frozen at RC |
| CLI, strict JSON, Dagger | [CLI/Dagger shape](https://github.com/sagikazarmark/typst-pack/issues/37), [corrected contract](https://github.com/sagikazarmark/typst-pack/issues/80) | Strict planning schemas/profiles/markers, hand-authored GraphQL target, locked replay | Typst 0.15.0 parity; production strict-JSON and precedence tests; actual Dagger generation, introspection, structural comparison, branch execution, and generated-client compilation | Six commands and typed graph at beta; packaged surfaces at RC |
| Clean break and coordinated release | [Migration and release](https://github.com/sagikazarmark/typst-pack/issues/43) | Frozen `v0.3.1` baseline, no-shim policy, recreation and release sequence | Exact old-surface inventory, migration examples, no-intermediate-release guard, dry runs, clean installs, checksums, tagged Dagger ingestion, post-publication vectors | Guide/guard at alpha; coordinated train and checks at stable |

### Required combination lanes

There is no Cargo feature powerset. Exhaustive semantic axes are PDF, PNG, SVG,
and HTML; package embedded/external by font embedded/external; sync/async;
Pack-ingress Verify/Derive; succeeded/rejected semantic result; every closed
Compilation Operation Outcome; complete-collection atomic, each-object atomic,
and streaming publication; and each supported Typst feature independently and
pairwise, including required HTML derivation and unsupported bundle rejection.

Trust, offline policy, backing, reporting, page selection, output controls,
authority composition, interruption, and target adapter use pairwise generation,
with every generated case still crossing the native contract suite.

Required execution lanes are:

- Linux on exact Rust 1.92 for core/public-interface checks and featureless WASM
  compilation;
- Linux on pinned stable for full semantic, property, filesystem, CLI, worker,
  documentation, lint, and end-to-end suites;
- Windows and macOS on pinned stable for path, filesystem, publication, process,
  watcher, CLI, refusal, and packaged-binary smoke tests;
- `wasm32-unknown-unknown` plus a pinned headless browser for the claimed
  memory/spooling/Worker/paged/HTML/Hostile-refusal core path;
- Dagger/OCI for typed graph, immutable objects, representative lifecycle,
  profile admission, and Hostile refusal;
- a production-equivalent policy suite for each advertised Hostile-capable
  deployment, if any; and
- nightly fuzzing, Miri, concurrency models, sanitizers, long schedules, and
  generated boundaries as tooling evidence rather than MSRV requirements.

### Mandatory first-party gate

Before release, implementation must also:

- snapshot options, defaults, conflicts, environment fallbacks, six commands,
  and aliases against Typst 0.15.0;
- regenerate strict major-1 schemas and reject duplicate keys, BOM, trailing
  data, unknown fields/branches/majors, old aliases, and invalid nullability;
- generate GraphQL from actual Dagger source, compare canonical structure,
  compile a generated client, exercise nullable branches, and prove sibling
  reuse and `require*` no-rerun behavior;
- prove every reportless and terminal-less creation/compilation boundary;
- prove first-party result caching is disabled at request, admission/refusal,
  dependency inventory, and provenance seams;
- audit every wire leaf to one approved source, including all six operational
  sections and `D/K/Q/W/P/T` applicability and reach;
- poison exact-positive width equalities and prove unavailable width refuses
  rather than lowers;
- exercise all seven Format Receipt roles, all six Transport Receipt
  role/subject pairs, both admission branches, legal stage ledgers, assertions,
  and independent commit, cleanup, residual, and exposure facts;
- prove every Pack Archive Encoding retains its mandatory Format Receipt and an
  independent Spool Transport Receipt exactly when spooling was attempted;
- reproduce the independent Epoch 2 all-Stored vectors;
- test stdout exclusivity, collisions, publication-strength refusal, cleanup
  precedence, and outer-channel reason/failure rules;
- test exact Session events/effects, synchronous Accept preparation, tokenless
  rejection/ingestion, retry, active/draining and pending bounds, push/poll/gaps,
  retirement, Last Successful Compilation, and Last Successful Delivery;
- prove delivery wrappers retain the same publication's Session Instance,
  Revision, Evaluation, Publication Sequence, Result Identity, and immutable
  delivery outcome; and
- rerun the locked harness from the implementation parent with recorded case
  counts and prove every profile field at its real admission seam.

### Present evidence and its limits

At [`95d5bb0`](https://github.com/sagikazarmark/typst-pack/commit/95d5bb06d08db51e599fffbaf7f4c0c56c7441c9),
the planning fixture and harness recorded passing format, check, all-target test,
doctest, JSON, profile, and replay lanes locally. The official Rust 1.92 lane
independently passed check, all-target tests, compile-fail doctests, and replay.
The replay reports Draft 2020-12 with 368 definitions and 1,513 local references,
40 direct and 47 generated schema cases, 48 semantic cases, 32 mechanically
linked serializer leaves, 19 poison derivations, seven Session/watch scenarios,
and six publication fences.

That evidence proves planning syntax, public type shape, strict-schema cases, and
the assertions actually replayed. Bodies remain unimplemented, the GraphQL
fixture is hand-authored, the viewer is structural, production serialization and
actual Dagger generation are absent, and Epoch 2 corpus bytes are not yet
materialized. None of those implementation gates is claimed complete here.

## Clean-Break Migration

### Compatibility invariants

- Signed `v0.3.1` is the sole shipped baseline.
- Resource Slot-era `main` is unreleased and receives no release, reader,
  migrator, aliases, fixture corpus, or compatibility promise.
- There is no deprecation release, intermediate release, public dual
  architecture, Rust compatibility module, old-feature facade, CLI alias mode,
  flat-Dagger wrapper, historical field alias, or dual serializer.
- The `0.4.0` reader does not accept the `v0.3.1` TOML/Deflate
  `format-version = 1` dialect. Epoch 2 never synthesizes missing historical
  state.
- Ordinary `typst-pack = "0.3"` requirements do not select `0.4.0`.
- A narrowly compatible critical parser-security or recovery-only `0.3.x` patch
  may come from the signed legacy line, but is not a migration bridge and cannot
  import the unreleased intermediate architecture.

### Surface movement

| Previous surface | `0.4.0` destination |
| --- | --- |
| `typst-pack` features `fs`, `cli`, `embedded-fonts` | Featureless core plus `typst-pack-fs` and/or `typst-pack-cli` |
| `Pack::builder`, `PackBuilder`, `PackWorld`, direct World compilation, mutable options | Validated creation or ingress, then preparation and sync/async Compilation Driver |
| `cargo install typst-pack --features cli` | Install `typst-pack-cli`; binary remains `typst-pack` |
| `extract` for an editable project | `materialize`; Pack Identity is intentionally not preserved |
| `extract` for complete lossless state | `convert --to closure` |
| External Project Resources or unreleased Resource Slots/Providers | Include exact baseline bytes; use Pack Overrides only for permitted replacement |
| Four-command CLI | Six commands: create, inspect, compile, watch, materialize, convert |
| Flat Dagger create/inspect/extract/compile | Typed creation, ingress, Pack, representation/projection, and Compilation objects; `require*` for fail-fast CI |
| Exit-code inference | Ordinary nonzero behavior plus structured reports when terminal class matters |
| Historical archive | Preserve and recover with pinned `v0.3.1`; recreate an Epoch 2 Pack, never convert |

### Implementation-to-release sequence

1. Approve this specification before runtime implementation resolves any choice
   implicitly.
2. Before deleting old seams, capture an exact `v0.3.1` Rust, feature, CLI,
   Dagger, and archive inventory and draft `docs/migrations/0.3-to-0.4.md`.
3. Block publication of intermediate `main`; use unpublished `0.4.0-alpha.0`
   development versions until public-alpha gates pass.
4. Establish one-version workspace order: `typst-pack`, then `typst-pack-fs`,
   then `typst-pack-cli`; bind Dagger to the same repository tag.
5. Replace public seams in coherent vertical slices. Internal temporary
   scaffolding may coexist, but no release exposes both architectures; accepting
   a target seam deletes its superseded public path.
6. Keep `main` green through native public-seam tests. Preserve behavioral
   intent, not obsolete Resource Slot, Epoch 1, feature, builder, or test shape.
7. Materialize and execute Epoch 2 schema, corpus, and writer gates before any
   public prerelease writes Epoch 2.
8. Public alpha requires the fixed package graph, frozen Epoch 2, an executable
   create/read/inspect/encode/prepare/compile vertical path, migration guide, and
   an enforced no-intermediate-publication guard.
9. Beta requires the Rust interfaces, filesystem crate, six CLI commands, typed
   Dagger graph, representations, transports, Sessions, and selected cache
   topology to be feature-complete for the bill of materials.
10. Release candidate freezes Rust, package, CLI, Dagger, and documentation
    surfaces and passes the complete claim matrix on packaged clean-install
    paths. A semantic correction requires another candidate.
11. Stable `0.4.0` follows at least one public candidate and a feedback window,
    with no semantic or interface change after the last candidate and all
    coordinated publication and post-publication checks passing.

Any Epoch 2 archive emitted by a public prerelease is a permanent readability
obligation for `0.4.0` and `0.4.x`. A format defect requires a new epoch decision,
never reinterpretation in place.

### Historical Pack recovery, not conversion

1. Retain the original archive unchanged.
2. Identify its exact producer and dialect; integer format version 1 is
   insufficient.
3. Use pinned `v0.3.1` only to inspect and recover bytes it understands.
4. Reconstruct an explicit source project and recover required packages, fonts,
   inputs, times, features, external-resource baselines, and representative
   values.
5. Create a fresh Pack with explicit Discovery Variants and authorities and
   require successful issuance and replay.
6. Compile representative requests and compare diagnostics and artifacts against
   caller-selected acceptance criteria.

The recreated Pack has a new Pack Identity. Missing source, dependencies,
baseline bytes, or representative evidence can make recreation impossible; no
trace, default, empty baseline, or identity may be invented. Target `convert`
does not read or convert the legacy dialect.

Keep the signed `v0.3.1` tag, crate, source, release binaries, checksums, and
tagged documentation available indefinitely. The `0.3` line is frozen for
feature work; any allowed critical parser-security or recovery patch remains a
narrow legacy fix, not a bridge.

Legacy tooling lacks the target limits, validation, trust profiles, and
confinement and is suitable only for trusted legacy input by default. Recovering
an untrusted archive requires separately verified hardened isolation with no
credentials or network, hard CPU, memory, disk, and process limits, and staged
output; an ordinary container or Dagger invocation is insufficient.

Do not build a legacy reader without evidenced demand. Any future reader must be
a separate read-only package or process that emits candidate content with an
explicit evidence and omission report; it cannot become a core dependency,
construct a target Pack, claim conversion, or silently discard unknown material.

### Coordinated publication and rollback

Build crates, binary, checksums, installer, Dagger schema, documentation, and
vectors from one frozen commit. Run the complete release matrix, publication dry
runs, clean-registry installs, packaged-binary smoke tests, Pack vectors, and
Dagger ingestion against those artifacts.

Publish `typst-pack`, `typst-pack-fs`, then `typst-pack-cli`, verifying registry
visibility after each. Only then create the signed `v0.4.0` tag at the tested
commit, publish assets, expose Dagger from that tag, and post-verify dependency
resolution, CLI installation, checksums, Dagger ingestion, and Epoch 2 vectors.

A partial publication resumes missing artifacts from the same commit and
version. A defective published subset is yanked where appropriate and the whole
train advances to a new prerelease or patch. Versions and tags are never reused,
replaced, or force-moved. Rollback means pinning `v0.3.1` or the last good
`0.4.x` while issuing a forward fix.

## Implementation Boundary

Approval authorizes implementation planning, not implementation or release. The
following remain future work and must satisfy the gates above:

- implementing the seven Rust modules and production serializers;
- materializing and independently decoding the Epoch 2 corpus;
- implementing native, property, model, fuzz, platform, and benchmark suites;
- generating and publishing the real Dagger schema and client;
- creating ADR-0006 and applying its recorded status changes;
- inventorying users, writing and executing migration guidance, or recreating
  historical Packs; and
- packaging, prereleasing, publishing, tagging, deployment, and rollback.

The separate Restate compilation service, web editors, storage backends,
authentication, IAM, credential provisioning, publisher-signing systems,
upstream Typst changes, unrelated release automation, and project administration
remain outside this redesign map. Consumer adapters may be planned separately
against this contract, but they do not expand its first-release bill of
materials.

No new decision ticket or in-scope fog remains. If implementation discovers a
contradiction in the normative bundle, it must stop and return the exact clause
conflict to a new decision rather than silently choosing one interpretation.

## Approval Basis

The final corrected contract resolves the only blockers from the rejected joined
review: complete reached Creation Resource reporting, exact Session Preparation,
and sealed token-bound attempt admission and completion. The authority ledger
above prevents unrelated historical or prototype text from overriding those
corrections, while the claim manifest and verification matrix prevent planning
artifacts from being presented as implementation evidence.

On that basis, this is the recommended and approved answer for every remaining
design branch in this map. The destination is ready to hand off for
implementation planning.
