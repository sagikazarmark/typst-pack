# PROTOTYPE: typst-pack Implementation-Planning Specification Review

> Throwaway review artifact for [Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58). It is not an implementation contract and must not be merged as-is.

## Status

**Review verdict: not approved for implementation planning.**

The accepted decisions define a coherent product, architecture, and migration direction, and they resolve the six questions surfaced by [Audit architecture synthesis for implementation readiness](https://github.com/sagikazarmark/typst-pack/issues/44). The resulting normative artifacts do not yet compose into one implementable contract, however. The remaining problems are precise cross-artifact contradictions and missing public projections, not undifferentiated fog.

Implementation planning should wait for the four successor decisions under [Recommended route](#recommended-route).

## Reading model

- This document is the single connective specification used for the approval review. It summarizes joins and links to the ticket or artifact that owns each detail rather than copying registries, interfaces, schemas, profiles, matrices, or migration inventories.
- [`CONTEXT.md`](https://github.com/sagikazarmark/typst-pack/blob/main/CONTEXT.md) is the canonical glossary.
- [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27) is the low-resolution decision index.
- [Architecture specification synthesis prototype](https://github.com/sagikazarmark/typst-pack/blob/c5219ae30060d2c1df5567146fef0e50395b1be4/PROTOTYPE-architecture-specification.md) supplies the earlier detailed connective view.
- [Pack Format Epoch 2 normative contract prototype](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md), [Rust lifecycle and adapter interfaces prototype](https://github.com/sagikazarmark/typst-pack/blob/e6968a8118b8271f4c9fefbb539158b7b000c795/PROTOTYPE-rust-lifecycle-adapter-interfaces.md), and [first-party CLI and Dagger contract prototype](https://github.com/sagikazarmark/typst-pack/blob/0617f5ac45add5346df410a98f4013ff43109d33/PROTOTYPE-first-party-cli-dagger-contracts.md) are the accepted detailed baselines.
- A resolution comment or accepted artifact owns its decision. If two accepted sources disagree, the disagreement is an approval blocker; this synthesis never silently chooses one.

## Destination and scope

The destination is a decision-complete product, architecture, and migration specification for typst-pack as a library-first portable Typst project system. It must leave implementation planning no product, semantic, interface, persisted-representation, target-guarantee, verification, or migration choice that could produce incompatible implementations.

Implementation, optimization, release execution, deployment, the separate Restate compilation service, web products, storage backends, authentication, publisher trust infrastructure, and changes to upstream Typst remain outside the destination.

## Product contract

typst-pack packages and executes reusable Typst compilation closures. The featureless Rust library core is the primary interface, the CLI is the primary first-party adapter, and Dagger is a typed CI adapter. Browser/WASM and remote-service use constrain portability and capability honesty without becoming first-party products or normative resource profiles.

A valid **Pack** is immutable, replay-verified, and closed over one fixed entrypoint, every successfully observed or explicitly included baseline project file, exact Package Requirements, exact Font Requirements, and the Discovery Coverage Identities that establish its finite coverage. Every valid Pack is portable. A Self-Contained Pack additionally embeds every package and font dependency. Portability, self-containment, offline operation, environment independence, exact same-engine reproducibility, and cross-engine compatibility remain distinct claims.

One immutable Pack Override Set supplies compilation-scoped replacements for existing project paths. It cannot add, delete, or rename paths, change the fixed entrypoint identity, or alter package or font content. Undeclared dependencies and ambient semantic fallback fail closed.

Semantic depth, portability, reproducibility, observability, and predictable operation outrank convenience, throughput, compactness, and compatibility with provisional behavior.

## Architecture

### Lifecycle through-line

```text
mutable, remote, or caller-owned inputs
    -> adapter stabilization and Ordinary Admission
    -> immutable Project Snapshot plus separate operational evidence
    -> one Discovery World and isolated Discovery Variants
    -> causal evidence fence, assembly, replay, and Pack Issuance
    -> opaque validated Pack
    -> side-effect-free compilation preparation
    -> request rejection or immutable Prepared Compilation
    -> verified Semantic Result Cache hit
       or exact dependency acquisition, verification, and one synchronous Compilation Kernel
    -> immutable Compilation Report
    -> independent caching, delivery, publication, or session reduction
```

Semantic state is immutable and identity-bearing. Operational state is explicit and replaceable and does not enter semantic identity unless it resolves a declared semantic value. Filesystems, network access, asynchronous runtimes, mutable caches, clocks, environment defaults, schedulers, processes, output destinations, and watch subscriptions live outside the semantic kernel.

### Deep modules

The public Rust architecture has exactly seven deep modules:

| Module | Interface responsibility |
| --- | --- |
| `creation` | Project snapshots, creation requests, sync and async creation, creation reports, discovery, evidence fencing, replay, and issuance |
| `pack` | Opaque Pack, Pack Identity, Pack Inspection, and thin lifecycle conveniences |
| `authority` | Separate package and font authority roles for sync and async callers |
| `compilation` | Preparation, prepared values, drivers, reports, results, outcomes, diagnostics, and semantic-result caching |
| `representation` | Pack Archive and Closure Export ingress, encoding, and projection plans |
| `transport` | Stable values, bounded acquisition and spooling, publication, delivery, cleanup, and receipts |
| `session` | Caller-owned reduction over revisions, evidence, attempts, currentness, and publication |

There is no public arbitrary Typst World, Pack builder, unchecked constructor, Pack Control Record, generic storage abstraction, engine or exporter seam, watcher, runtime, or backing trait. Opaque lifecycle values carry behavior behind these interfaces; target-specific adapters satisfy explicit role seams.

### Creation and Pack lifecycle

One creation attempt admits a pure immutable Project Snapshot, exact variant request values, explicit package and font authorities, and separate operation-scoped evidence for mutable sources. Successful project observations contribute their logical path regardless of baseline or discovery-override provenance; each included path contributes the Project Snapshot's baseline bytes. Explicit Conditional Inclusions are the only source of unobserved project paths.

After successful isolated discovery, creation selects every causal content, absence, membership, ordering, metadata, and source-choice fact. A race-closing Creation Evidence Fence must prove agreement with the frozen Discovery Snapshot before assembly and replay. Replay performs no reacquisition and must reproduce every Discovery Trace. Changed or incomplete evidence prevents Pack Issuance; there is no hidden retry and no partially valid Pack.

Discovery Coverage matching compares one exact canonical source-evaluation projection: target, Typst inputs, Compilation Document Time, engine features, and project overrides represented by Discovery Request Commitments. It excludes exporter controls, implementation identities, authorities, and operational controls. Matching records finite replay evidence; it is not an allowlist, artifact-reproducibility claim, or Session Currentness claim.

Package discovery freezes complete logical package trees. Font discovery freezes exact Font Containers, used Font Face Identities, and Pack Font Catalog order. Embedding is selected independently per requirement. Any project, dependency, catalog, coverage, or embedding change requires fresh creation and Pack Issuance.

### Compilation and reporting

Compilation uses a staged terminal tree:

```text
adapter stabilization and Pack ingress
├─ failure -> role-specific adapter or transport outcome
└─ validated Pack plus exact semantic request
   └─ preparation
      ├─ Compilation Request Rejection
      └─ Prepared Compilation
         └─ Compilation Attempt
            └─ Compilation Report
               └─ exactly one Compilation Result or Compilation Operation Outcome
                  └─ Compilation Terminal Commitment
                     └─ zero or more post-commit adapter operations
```

Preparation canonicalizes the full semantic request, applies deterministic defaults, validates Pack Overrides, attests implementation identities, and returns either a rejection or a Prepared Compilation. A rejection has no report, dependency evidence, access trace, or Compilation Identity.

Synchronous and asynchronous drivers acquire and verify exact dependencies before invoking the same private synchronous kernel. A report owns exactly one semantic result or one pre-commit operational outcome. Deterministic compiler or exporter rejection is a semantic result. Delivery and other post-commit operations retain the immutable report and cannot rewrite its terminal.

Canonical Diagnostic Policy is explicit semantic request data and contributes to Compilation Identity. A result owns a deterministic complete or limited Canonical Diagnostic Envelope; a limited envelope preserves whole entries and records the first omitted ordinal, phase, and limiting dimension without changing result status. Rendering, source bundles, and other diagnostic projections remain separately bounded operational channels.

Safe Pack Override projections use role-bound Compilation Request Commitments. They do not disclose raw replacement bytes, replacement Content Identities, baseline comparison, equality, or eventual use. Discovery Request Commitments remain a separate domain.

### Evidence, caching, and sessions

Compilation Request Inventory, Dependency Resolution Evidence, and Compilation Access Trace remain separate views of supplied values, causal authority facts, and Typst observations. Logical identity, exact content identity, backing evidence, acquisition provenance, and access kind remain distinct.

The sole public semantic cache maps one Compilation Identity to a complete immutable Compilation Result within a Cache Isolation Domain. A verified hit may complete a one-shot attempt before dependency acquisition while preserving the original result and trace. In a Compilation Session it is only a historical candidate until fresh stabilization, causal evidence revalidation, and race-free subscription handoff establish Session Currentness.

A Compilation Session is a caller-owned synchronous reducer bound to one Pack. It snapshots mutable inputs into revisions, permits at most one active attempt and one latest pending revision, reconciles explicit watch coverage, and publishes only the newest eligible terminal. Last Successful Compilation, Session Currentness, and delivery success remain separate facts.

### Trust, resources, and concurrency

Every interpretation path selects a Deployment Trust Profile before input-dependent work. Validity and integrity checks never weaken with trust policy. The first release admits Trusted and Partially Trusted operations and returns a typed refusal for Hostile; it publishes no partial Hostile facility. Future Hostile support still requires a verified pre-parse OS or runtime boundary, hard quotas, kill and reap, bounded protocol, parent verification, and parent-owned publication.

Confinement does not guarantee semantic honesty after a confined worker is compromised, nor does typst-pack guarantee the safety of downstream consumers that parse Compilation Output Artifacts.

Format ceilings are immutable representation-validity rules. Operation Resource Limits are role-specific operational admission. Native CLI and Dagger/CI profiles are shipped, versioned Adapter Resource Defaults; browser and service values are non-binding Reference Resource Profiles.

Engine concurrency has independent owners:

| Symbol | Owner | Meaning |
| --- | --- | --- |
| `D` | driver and authorities | dependency acquisition and verification concurrency |
| `K` | Compilation Execution Facility | simultaneous ready kernel and exporter dispatches |
| `Q` | Compilation Execution Facility | ready-dispatch queue capacity |
| creation capacity | Creation Execution Facility | separately named discovery and replay admission and queue semantics |
| `W` | Engine Runtime Domain | fixed internal Typst/Rayon parallelism width |
| `T` | transport adapter | acquisition, publication, or delivery transfer concurrency |
| `P` | isolated facility | live worker process or runtime capacity |

Generic Rust use inherits host engine behavior. First-party `--jobs` controls only `W` and is fixed before managed engine work. Fine engine timing is complete for one attempt only when the Engine Runtime Domain excludes overlapping engine work.

## Representations and transport

Pack Archive and Closure Export are strict representations of one logical Pack and carry the same canonical Pack Control Record bytes. Epoch 2 uses core-deterministic CBOR, closed registries, typed domain-separated identities, a narrow ZIP profile, an exact namespaced finite-tree profile, fixed validation precedence, role-refined receipts, and an independent interoperability corpus contract.

Pack Inspection, Project Materialization, and Closure Export are distinct. Inspection is side-effect-free. Materialization emits baseline project files only. Closure Export losslessly represents semantic Pack state. Projection never repairs paths, acquires external dependencies, merges into existing trees, or presents partial publication as complete.

Transport adapters resolve locators and streams into stable owned values before semantic use. Acquisition, encoding, projection, publication, and Compilation Delivery retain separate outcomes and receipts. Publication Commit Strength and Transport Cleanup Strength are admitted before effects; cleanup never overwrites the primary failure.

## First-party adapters

The CLI has six lifecycle commands: `create`, `inspect`, `compile`/`c`, `watch`/`w`, `materialize`, and `convert`. It follows embedded Typst 0.15.0 parsing where semantics match and records intentional differences. Strict major-1/minor-0 JSON structures cover inputs and terminal envelopes. The adapter resolves filesystem, environment, wall-clock, authority, resource, trust, and output defaults before crossing semantic seams.

Dagger exposes immutable typed lifecycle objects over `File` and `Directory`, with ordinary expected failures queryable and `require*` accessors raising. It omits watch and sessions, stdin/stdout, terminal presentation, arbitrary host paths, generic object stores, true compile-time artifact streaming, and Hostile execution. Representation success and container-local staging remain separate.

Both adapters default to identity disclosure and gate exact diagnostics, evidence, sources, raw values, Pack Override bytes, backing locators, and adapter detail independently. Watch uses latest-only Session Publication and keeps currentness, last successful compilation, and last successful delivery separate.

## Verification and migration

Native Rust tests are the semantic authority. Public-seam contract suites, representation vectors, properties, models, adapter conformance, deterministic faults, fuzzing, races, platform confinement tests, boundary tests, and production-equivalent benchmarks cover their owning claims. Dagger checks cover typed adapter wiring and representative lifecycle paths without duplicating core semantics.

The clean break targets coordinated `0.4.0` releases from signed `v0.3.1`. There is no deprecation release, dual architecture, legacy public module, old Pack reader, Resource Slot compatibility layer, field alias, or historical Pack converter. Historical Packs are preserved with their exact trusted legacy implementation and recreated from explicit evidence, producing new Pack Identities.

Epoch 2 is the sole new Pack format baseline and must not have a public writer prerelease until its normative corpus and independent readers pass. A prerelease writer creates a permanent compatibility obligation for its exact epoch contract.

## Resolved prior joins

| Decision | Original gap closed |
| --- | --- |
| [Reconcile Pack creation evidence and coverage semantics](https://github.com/sagikazarmark/typst-pack/issues/60) | Separated pure snapshots, mutable evidence, closure membership, issuance fencing, and exact coverage matching |
| [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57) | Supplied the canonical control record, registries, Pack identity transcripts, validation order, representations, receipt model, and corpus contract |
| [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59) | Fixed staged terminals, semantic diagnostic bounds, safe commitments, cache-hit reporting, and session currentness |
| [Define engine parallelism and adapter profile ownership](https://github.com/sagikazarmark/typst-pack/issues/55) | Separated the Engine Runtime Domain, execution facilities, concurrency dimensions, timing, and normative versus reference profiles |
| [Freeze the Rust lifecycle and adapter interfaces](https://github.com/sagikazarmark/typst-pack/issues/61) | Froze seven public modules and a compile-checked interface contract with sealed stable bytes and explicit role seams |
| [Freeze the first-party CLI and Dagger contracts](https://github.com/sagikazarmark/typst-pack/issues/56) | Selected exact command, schema, GraphQL, profile, receipt, disclosure, staging, and watch surfaces |

These decisions answer their original questions. Approval additionally requires their accepted artifacts to agree at every shared seam.

## Approval blockers

| Blocker | Contradiction or omission | Required decision |
| --- | --- | --- |
| Compilation-family identity registry | The Epoch 2 artifact [delegates Compilation Request Commitments and compilation, result, and artifact identities to a separate registry](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md#L585-L605), while [Define dependency, provenance, and invalidation identities](https://github.com/sagikazarmark/typst-pack/issues/48) delegated canonical encoding to the format contract. Independent implementations can therefore produce different cache keys, result identities, watch equality, and artifact identities. | Freeze the canonical compilation value model, including feature identifiers and numeric encodings, then every compilation-family projection, transcript, kind/schema identifier, and independent golden vector. |
| Font catalog construction | The Rust interface's [`FontCatalogCandidate`](https://github.com/sagikazarmark/typst-pack/blob/e6968a8118b8271f4c9fefbb539158b7b000c795/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1325-L1343) omits weight, stretch, flags, axes, and codepoint coverage required by [Define font lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/38) and the [Epoch 2 Font Requirement descriptors](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md#L495-L526). | Freeze a complete validated candidate interface that can reproduce deterministic selection without eager acquisition or invented defaults. |
| Creation and inspection surface | [`CreationRequest`](https://github.com/sagikazarmark/typst-pack/blob/e6968a8118b8271f4c9fefbb539158b7b000c795/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1928-L1945) cannot carry the metadata and annotations required by the first-party contract, while [`PackInspection`](https://github.com/sagikazarmark/typst-pack/blob/e6968a8118b8271f4c9fefbb539158b7b000c795/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1195-L1261) cannot expose the required complete requests, traces, inventories, extensions, and annotations. | Freeze the missing input and lossless inspection projections at the public Rust seam. |
| Terminal, trace, artifact, and disclosure surface | `CompilationRequestRejection` exposes issues but not the safe request inventory required by the strict adapter schema. Report-level partial access traces, full structured observations, Compilation Artifact Identity, and independently gated disclosure channels are likewise not publicly obtainable from the frozen Rust fixture. | Freeze immutable request-inventory, partial-trace, structured observation, artifact-identity, and disclosure views without adapter reconstruction of semantic state. |
| Limits and execution reporting | Override limits are present in first-party profiles but absent from override construction and preparation. Complete Package Tree and Font Catalog candidate construction can collect without an acquisition budget. Creation facility capacity and Creation Report operational inventory do not expose separately named creation admission, queue, domain, profile, and reporting facts. | Put every limit at its first observable public seam, including incremental reservations in authority-produced builders, and freeze separate creation and compilation facility capacity/reporting contracts. |
| Representation and transport receipts | The adapter schemas require role-refined format and transport receipts, but the Rust representation seam exposes only a small validation receipt, Closure Export planning returns no format receipt, publishers return only transport outcomes, and transport receipts omit required subject identities. | Freeze core-owned receipt views and composition that prove format, subject identity, byte/count, commit, cleanup, and transport coherence. |
| Discovery trace schema | Epoch 2 distinguishes source and raw-file request kinds, while the strict first-party JSON schema uses `load` and `probe`, which describe a different axis from request kind and duplicate outcome information. | Keep the canonical registry fixed, add cross-artifact corpus vectors, and align the strict adapter schema with it. |
| Ambiguous public representation methods | `Pack::compilation_identity_hint` has no eligibility or correctness contract, while Closure Export import accepts an Archive Encoding Identity expectation that the adapter contract defines as archive-only. | Remove these methods or freeze exact behavior, and preserve archive-only expectations across Rust and adapters. |
| Epoch 2 writer corpus | The only registered first-party writer is `epoch-2-all-stored-v1`, but the mandatory corpus still requires a second Archive Encoding Identity and successful Deflate writer receipts without fixing a compressor recipe or golden bytes. | Register and freeze an exact Deflate writer recipe or remove Deflate writer requirements while retaining narrow-profile reader support. |

These are implementation-planning blockers because each permits incompatible public behavior, identity, representation, or target guarantees. Corpus materialization, actual Dagger code generation, generated-client compilation, implementation tests, and benchmark tuning remain later gates once the contracts are internally complete.

## Recommended route

Create four decision tickets and then repeat approval:

1. **Freeze the compilation-family value and identity registry**: own the canonical semantic value encodings, every compilation-family projection and transcript, identity kind/schema identifiers, and independent vectors. It is unblocked.
2. **Freeze the Epoch 2 writer and interoperability corpus**: reconcile writer recipes and corpus vectors, including source/raw-file observations, against the already-frozen Format Receipt semantics. It reopens neither Format Receipt nor Transport Receipt semantics. It is unblocked and may run in parallel with the identity registry.
3. **Complete the Rust lifecycle and receipt interfaces**: revise the compile-checked fixture and consumer for complete font metadata; creation, inspection, rejection, trace, artifact, and disclosure views; incremental authority-builder and override limits; separately named creation facility capacity and reporting; exact representation expectation scopes; and public Format Receipt and Transport Receipt views with coherent composition. It is blocked only by the identity registry and may run in parallel with the Epoch 2 writer decision.
4. **Reconcile the first-party adapter schemas and profiles**: regenerate the strict schemas, GraphQL shape, profiles, and lifecycle model against both corrected contracts, including canonical request-kind fields, exact receipt serialization, and role-specific execution reporting. It is blocked by the Epoch 2 writer and Rust interface decisions.

A successor approval ticket is blocked by the first-party adapter reconciliation. No implementation ticket should use the current accepted artifacts as a jointly complete contract before that review closes.

## Non-blocking implementation parameters

Private type layout, helper decomposition, parser libraries, buffering, allocation strategy, cache eviction algorithms, operator-owned cache capacity below admitted limits, deployment-specific authority composition, visible caller retry policy, benchmark tuning below ceilings, package ownership administration, docs.rs presentation, and final MSRV selection before the first public alpha remain implementation or release-planning choices.

There is no hidden retry. Any retry permitted by a caller or adapter creates a visible new attempt. If an implementation choice changes a public interface, persisted representation, semantic identity, guarantee, migration obligation, or normative first-party profile, it graduates into decision work.

## Traceability

| Decision | Contribution |
| --- | --- |
| [Establish upstream Typst project and filesystem semantics](https://github.com/sagikazarmark/typst-pack/issues/28) | Rooted logical identity, confinement limits, and observable dependency behavior |
| [Establish upstream Typst compilation and artifact constraints](https://github.com/sagikazarmark/typst-pack/issues/29) | Synchronous whole-value kernel, cancellation limits, streaming limits, and process-global behavior |
| [Survey real portable Typst project workflows](https://github.com/sagikazarmark/typst-pack/issues/30) | Evidence for distinct portability, authority, compatibility, output, and invalidation contracts |
| [Survey project override and layering precedents](https://github.com/sagikazarmark/typst-pack/issues/31) | Logical/backing separation, replacement semantics, and independent authorities |
| [Define the Pack contract and portability guarantees](https://github.com/sagikazarmark/typst-pack/issues/32) | Closed replay-verified Pack, portability levels, and replacement-only overrides |
| [Prioritize product journeys and quality attributes](https://github.com/sagikazarmark/typst-pack/issues/33) | Product priority, target constraints, and quality ordering |
| [Define project discovery and creation semantics](https://github.com/sagikazarmark/typst-pack/issues/34) | Discovery World, variants, traces, revalidation, replay, and issuance |
| [Define extraction and materialization semantics](https://github.com/sagikazarmark/typst-pack/issues/35) | Inspection, baseline materialization, and lossless Closure Export |
| [Define the Pack format, invariants, and compatibility model](https://github.com/sagikazarmark/typst-pack/issues/36) | Epoch model, deterministic control record, representations, identities, and validation |
| [Prototype the CLI and Dagger interfaces](https://github.com/sagikazarmark/typst-pack/issues/37) | Six-command CLI and typed Dagger lifecycle direction |
| [Define font lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/38) | Whole-container identity, used faces, ordered catalog, and exact fulfillment |
| [Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39) | Seven deep modules and operation-owned lifecycle direction |
| [Define compilation input, output, and diagnostic contracts](https://github.com/sagikazarmark/typst-pack/issues/40) | Preparation, outputs, diagnostics, reports, and semantic identities |
| [Define compilation-scoped project variation](https://github.com/sagikazarmark/typst-pack/issues/41) | Pack Override Set, preflight, source behavior, and provenance |
| [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42) | Native semantic authority, matrices, properties, fuzzing, and platform evidence |
| [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43) | `v0.3.1` baseline, `0.4.0` clean break, recreation, and release gates |
| [Audit architecture synthesis for implementation readiness](https://github.com/sagikazarmark/typst-pack/issues/44) | Prior synthesis, six precise gaps, successor criteria, and implementation boundary |
| [Define package lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/45) | Exact package-tree identity, authority, fulfillment, and rebuild-only updates |
| [Define execution, I/O, and cancellation boundaries](https://github.com/sagikazarmark/typst-pack/issues/46) | Dual drivers, one kernel, snapshots, deadlines, commitment, and delivery |
| [Define trust, integrity, and confinement guarantees](https://github.com/sagikazarmark/typst-pack/issues/47) | Trust profiles, integrity chain, capabilities, and confinement claims |
| [Define dependency, provenance, and invalidation identities](https://github.com/sagikazarmark/typst-pack/issues/48) | Request, evidence, and access views plus identity and invalidation rules |
| [Define reproducibility and engine-compatibility levels](https://github.com/sagikazarmark/typst-pack/issues/49) | Independent reproducibility predicates and cross-engine compatibility levels |
| [Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50) | Format ceilings and target-specific operational profiles |
| [Define watch and incremental-session semantics](https://github.com/sagikazarmark/typst-pack/issues/51) | Revisions, evidence coverage, fencing, currentness, and latest-only publication |
| [Define storage, reference, and transport adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/52) | Stable values, role transports, receipts, commitment, and cleanup |
| [Establish enforceable confinement guarantees by target platform](https://github.com/sagikazarmark/typst-pack/issues/53) | Platform capability matrix and Hostile enforcement requirements |
| [Define cache and session-storage topology](https://github.com/sagikazarmark/typst-pack/issues/54) | Public semantic cache, private reuse, isolation domains, and recovery records |
| [Define engine parallelism and adapter profile ownership](https://github.com/sagikazarmark/typst-pack/issues/55) | Engine Runtime Domain, concurrency ownership, timing, and profile status |
| [Freeze the first-party CLI and Dagger contracts](https://github.com/sagikazarmark/typst-pack/issues/56) | Exact adapter surfaces, schemas, profiles, receipts, disclosure, and watch model |
| [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57) | Canonical format, registries, validation, receipts, writer recipe, and corpus contract |
| [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59) | Staged terminals, bounded canonical diagnostics, commitments, and cache/session join |
| [Reconcile Pack creation evidence and coverage semantics](https://github.com/sagikazarmark/typst-pack/issues/60) | Pure snapshots, causal fence, override-observed closure, and coverage matching |
| [Freeze the Rust lifecycle and adapter interfaces](https://github.com/sagikazarmark/typst-pack/issues/61) | Compile-checked module, lifecycle, authority, transport, cache, and session interfaces |

## Approval recommendation

Reject approval in this pass. Preserve every accepted semantic direction, resolve the four cross-artifact decisions in dependency order, and then review one revised connective specification against the corrected normative artifacts. No implementation work should infer an answer to any blocker above.
