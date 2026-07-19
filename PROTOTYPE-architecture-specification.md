# PROTOTYPE: typst-pack Architecture Specification Synthesis

> Throwaway review artifact for [Audit architecture synthesis for implementation readiness](https://github.com/sagikazarmark/typst-pack/issues/44). It is not the implementation contract and must not be merged as-is.

## Status

**Review verdict: not approved for implementation planning.**

The accepted decisions form a coherent product and architecture direction, but they do not yet form one independently implementable contract. The gaps are narrow enough to ticket precisely, so there is no remaining undifferentiated fog. The blocking decisions and recommended resolutions are listed under [Approval blockers](#approval-blockers).

## Reading model

- [`CONTEXT.md`](https://github.com/sagikazarmark/typst-pack/blob/main/CONTEXT.md) is the canonical glossary.
- [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27) is the map and low-resolution decision index.
- Each linked closed decision ticket owns its detailed answer and rationale.
- This document owns only the connective view needed to review whether those answers compose. It does not restate field registries, complete interface sketches, resource tables, test matrices, or migration inventories.
- If this synthesis conflicts with a resolution comment, the conflict is an approval blocker rather than an implicit override.

## Destination

The destination is a decision-complete product, architecture, and migration specification for typst-pack as a library-first portable Typst project system. It must leave implementation planning no product, semantic, interface, representation, target, verification, or migration choice that would produce incompatible implementations.

Implementation, optimization, release execution, deployment, the separate Restate compilation service, web products, storage backends, authentication, publisher trust infrastructure, and changes to upstream Typst remain outside the destination.

## Product contract

typst-pack packages and executes reusable Typst compilation closures. The Rust library is the primary interface; the CLI is the primary first-party adapter. Dagger is a typed CI adapter. Browser/WASM and remote-service use constrain the library without adding target-specific dependencies to its featureless core.

A valid **Pack** is immutable, replay-verified, and closed over one fixed entrypoint, contained project files, exact Package Requirements, exact Font Requirements, and the successful Discovery Variants and Explicit Conditional Inclusions that establish its coverage. A Pack exists only after canonical validation and Pack Issuance, or after equivalent complete validation of a supported representation.

Every valid Pack is a **Portable Pack**. A **Self-Contained Pack** additionally embeds every package and font dependency. Neither property by itself promises offline execution, environment independence, exact reproducibility, or cross-engine compatibility; those are separate, testable operation or result claims.

Compilation accepts explicit Pack-contained variation through one immutable Pack Override Set. Overrides replace existing project bytes only. They cannot add, delete, or rename paths, change entrypoint identity, or alter package or font content. Undeclared dependencies and ambient semantic fallback fail closed.

The quality order is semantic depth, portability, reproducibility, observability, and predictable operation before convenience, throughput, archive compactness, or compatibility with provisional behavior. This ordering comes from [Prioritize product journeys and quality attributes](https://github.com/sagikazarmark/typst-pack/issues/33).

## Architecture

### Through-line

```text
mutable or remote inputs
    -> adapter stabilization and explicit authorities
    -> Project Snapshot and Discovery World
    -> isolated Discovery Variants and Discovery Traces
    -> revalidation, assembly, replay, and Pack Issuance
    -> opaque validated Pack
    -> side-effect-free compilation preparation
    -> exact dependency acquisition and verification
    -> one synchronous Compilation Kernel
    -> immutable Compilation Report
    -> independent delivery, publication, caching, or session reduction
```

Semantic state is immutable and identity-bearing. Operational state is explicit, replaceable, and excluded from semantic identity unless it resolves a declared semantic value. Filesystems, network access, asynchronous runtimes, mutable caches, clocks, environment defaults, schedulers, processes, output destinations, and watch subscriptions live outside the semantic kernel.

### Deep modules

The accepted module direction from [Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39) has seven public modules:

| Module | Interface responsibility | Complexity hidden behind the seam |
| --- | --- | --- |
| `creation` | Project snapshots, creation requests, sync and async creation, creation reports | Discovery World validation, isolated variants, traces, restarts, revalidation, replay, issuance |
| `pack` | Opaque Pack, Pack Identity, Pack Inspection, thin lifecycle conveniences | Canonical logical state and the one private whole-Pack construction path |
| `authority` | Separate package and font authority roles for sync and async callers | Source ordering, fallback, acquisition, provenance, evidence, private resolution and content caches |
| `compilation` | Preparation, prepared values, drivers, reports, results, operation outcomes | Override preflight, dependency snapshots, the Typst kernel, exporters, identities, terminal commitment |
| `representation` | Archive ingress and encoding, inspection and projection plans | Epoch dispatch, Pack Control Record, ZIP profile, identities, format ceilings |
| `transport` | Stable values, bounded acquisition and spooling, publication, delivery, receipts | Backpressure, verification, commit, cleanup, disclosure, target transport details |
| `session` | Caller-owned reducer over revisions, evidence, attempts, currentness, and publication | Race-free subscription replacement, latest-only scheduling, stale last-success semantics |

There is no public arbitrary Typst `World`, Pack builder, Pack Manifest constructor, unchecked constructor, generic storage abstraction, engine/exporter seam, or target runtime in the core interface. The seven-module direction is accepted, but exact compilable interfaces remain an approval blocker.

### Creation and Pack lifecycle

Adapters stabilize one immutable Project Snapshot, exact request values, and explicit package and font authorities before semantic creation. One attempt fixes one Discovery World and a nonempty ordered list of Discovery Variants. Variants execute with isolated compiler state and produce canonical Discovery Traces. The Pack closure is the union of successful observations and Explicit Conditional Inclusions.

Package discovery freezes complete logical package trees. Font discovery freezes exact whole Font Containers, used Font Face Identities, and their discovery-relative Pack Font Catalog ordering. Embedding is chosen independently per Package Requirement and Font Requirement; external fulfillment remains exact and verified.

Creation revalidates mutable evidence, assembles from a frozen Discovery Snapshot, and replays every variant without reacquisition. Replay must reproduce each Discovery Trace. There is no draft or partially valid Pack, no mutation of an issued Pack, and no update operation: changed project, package, font, catalog, coverage, or embedding state requires fresh creation and issuance.

Detailed ownership remains in:

- [Define the Pack contract and portability guarantees](https://github.com/sagikazarmark/typst-pack/issues/32)
- [Define project discovery and creation semantics](https://github.com/sagikazarmark/typst-pack/issues/34)
- [Define package lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/45)
- [Define font lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/38)
- [Define compilation-scoped project variation](https://github.com/sagikazarmark/typst-pack/issues/41)
- [Define dependency, provenance, and invalidation identities](https://github.com/sagikazarmark/typst-pack/issues/48)

### Compilation lifecycle

Compilation has three public phases:

1. Side-effect-free preparation canonicalizes the complete semantic request, applies deterministic defaults, validates the Pack Override Set, attests Engine Identity and Exporter Identity, and returns either a Prepared Compilation or Compilation Request Rejection.
2. A synchronous or asynchronous driver executes one Compilation Attempt under explicit Compilation Execution Controls. Both drivers acquire and verify exact dependencies before invoking the same private synchronous Compilation Kernel.
3. Compilation Delivery transports a committed immutable report and its complete artifact collection. Delivery has its own outcome and cannot mutate or replace the Compilation Report.

The kernel has no ambient I/O or asynchronous interface. Upstream Typst compiles and exports whole values synchronously, offers no cooperative kernel cancellation, and does not support true artifact streaming. Async acquisition, bounded spooling, admission, queueing, isolation, cancellation, deadlines, retries, timing, and output backpressure therefore surround the kernel.

A Compilation Report contains exactly one semantic Compilation Result or one Compilation Operation Outcome. Deterministic compiler or exporter rejection is a semantic result. Acquisition, infrastructure, cancellation, deadline, isolation, dynamic resource, and integrity failures before Compilation Terminal Commitment are operational. Failures after commitment belong to delivery or another adapter operation.

Detailed ownership remains in:

- [Establish upstream Typst compilation and artifact constraints](https://github.com/sagikazarmark/typst-pack/issues/29)
- [Define compilation input, output, and diagnostic contracts](https://github.com/sagikazarmark/typst-pack/issues/40)
- [Define execution, I/O, and cancellation boundaries](https://github.com/sagikazarmark/typst-pack/issues/46)
- [Define reproducibility and engine-compatibility levels](https://github.com/sagikazarmark/typst-pack/issues/49)

### Evidence, caching, and sessions

Compilation Request Inventory, Dependency Resolution Evidence, and Compilation Access Trace are separate views. They respectively describe supplied and resolved request values, causally relevant authority facts, and observations made by Typst. Logical identity, exact content identity, backing evidence, acquisition provenance, and access kind remain distinct.

Only one semantic cache seam is public: a caller-owned Semantic Result Cache keyed by Compilation Identity and storing complete immutable Compilation Results. Authority resolution caches, immutable content caches, transport reuse, and Session Recovery Records remain role-private. Caches never become authority, validity, authenticity, or Session Currentness.

A Compilation Session is a caller-driven reducer bound to one Pack. It snapshots mutable inputs into monotonic revisions, reconstructs live causal evidence, coordinates race-free watch coverage, permits at most one active attempt and one latest pending revision, and publishes only the newest eligible result. Last Successful Compilation, Session Currentness, and delivery success remain separate facts.

Detailed ownership remains in:

- [Define watch and incremental-session semantics](https://github.com/sagikazarmark/typst-pack/issues/51)
- [Define cache and session-storage topology](https://github.com/sagikazarmark/typst-pack/issues/54)

### Trust and target enforcement

Every interpretation operation selects one Deployment Trust Profile before input-dependent work. Validity and integrity checks never weaken with trust policy.

- Trusted permits defensive in-process handling without adversarial containment claims.
- Partially Trusted treats content as abusive while trusting deployment code, but ordinary in-process execution cannot promise compromise containment or hard whole-operation limits.
- Hostile requires a verified pre-parse OS or runtime boundary around the complete interpretation path, hard quotas, kill and reap, bounded terminal protocol, parent verification, and parent-owned publication.

Ordinary native, OCI/Dagger, browser, and in-process WASM shapes must refuse Hostile. Conditional support requires a target implementation that proves the entire contract; an Isolated Compilation Worker used only around the kernel is not sufficient.

Format ceilings are immutable representation-validity rules. Adapter Resource Defaults are stricter operational policy. The full target matrix remains in [Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50), while platform enforcement evidence remains in [Establish enforceable confinement guarantees by target platform](https://github.com/sagikazarmark/typst-pack/issues/53).

Detailed trust, transport, and confinement ownership remains in:

- [Define trust, integrity, and confinement guarantees](https://github.com/sagikazarmark/typst-pack/issues/47)
- [Define storage, reference, and transport adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/52)

## Representations

Pack Archive and Closure Export are strict representations of one logical Pack. They carry identical canonical Pack Control Record bytes and content-address exact bytes. Project bytes are always present; embedded package and font bytes are present; external requirements retain complete exact descriptors without acquiring their content.

Pack Format Epoch 2 selects deterministic CBOR for the control record and a narrow deterministic ZIP profile for Pack Archive. Logical paths exist only in the control record. Whole-Pack validation rederives every available identity, validates the complete closed model, and constructs the opaque Pack through one private path shared by creation and all representation ingress.

Pack Inspection, Project Materialization, and Closure Export are separate operations. Inspection is side-effect-free. Materialization emits baseline project files only. Closure Export is a lossless deterministic representation of semantic Pack state. Projection never repairs or normalizes paths, acquires external dependencies, merges into existing trees, or exposes partial success as a complete publication.

The accepted format model and invariant classes live in:

- [Define extraction and materialization semantics](https://github.com/sagikazarmark/typst-pack/issues/35)
- [Define the Pack format, invariants, and compatibility model](https://github.com/sagikazarmark/typst-pack/issues/36)

The concrete epoch-2 schema and interoperability contract remain an approval blocker.

## First-party adapters

### CLI

The target CLI has six lifecycle commands: `create`, `inspect`, `compile`, `watch`, `materialize`, and `convert`. `compile` and `watch` may keep the short aliases `c` and `w`. There is no separate `validate` because every ingress validates, and no generic `extract` because Project Materialization and Closure Export have different contracts.

The CLI keeps exact embedded-Typst spellings where semantics match and names every intentional difference. It resolves filesystem, environment, wall-clock, authority, resource, trust, and output defaults before crossing semantic seams. Compilation and delivery remain separate even when one command composes them. Standard output is a streaming no-rollback sink and is valid only for a single artifact.

### Dagger

Dagger exposes immutable typed lifecycle objects over `File` and `Directory`, not a flat CLI wrapper. It represents creation, validated ingress, Pack inspection and representation, compilation status and reports, artifact collections, materialization, and Closure Export. Format-specific output constructors prevent inapplicable arguments.

Dagger intentionally omits watch and sessions, stdin and stdout, terminal presentation, arbitrary host paths, generic object stores, true compile-time streaming, and Hostile execution. It defaults to Partially Trusted and returns complete staged values.

The accepted interaction model lives in [Prototype the CLI and Dagger interfaces](https://github.com/sagikazarmark/typst-pack/issues/37). Exact public flags, schemas, generated names, nullability, and terminal envelopes remain an approval blocker.

## Verification

Native Rust tests are the semantic authority. Public-seam contract suites cover creation, Pack construction, authorities, preparation, drivers, representations, transport, and sessions. Internal tests are reserved for canonical encodings, private whole-Pack construction, and integrity branches that public interfaces cannot isolate.

The verification architecture combines independently authored representation vectors, property and model tests, adapter conformance suites, deterministic fault injection, fuzzing, race and linearization models, platform confinement tests, boundary tests, and production-equivalent benchmarks. Dagger tests cover representative typed lifecycle paths and omissions rather than duplicate core semantics.

The exhaustive and pairwise matrices, platform lanes, and release gates remain in [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42).

## Migration and release

The clean break targets coordinated `0.4.0` releases of `typst-pack`, `typst-pack-fs`, and `typst-pack-cli`, with the Dagger module tied to the same immutable repository tag. The featureless core stays separate from native filesystem and CLI adapters. All publishable crates move as one compatible release train.

The sole shipped compatibility baseline is signed `v0.3.1`. Unreleased Resource Slot-era behavior on `main` receives no reader, aliases, migration corpus, or compatibility promise. There is no deprecation release, dual public architecture, public legacy module, old Pack reader, or compatibility shim.

Historical Packs are preserved, recovered with their exact trusted legacy implementation, and recreated from explicit source, requests, dependencies, and fonts. They are never converted into epoch 2. Missing evidence is reported rather than synthesized, and recreated Packs have new Pack Identities.

Epoch 2 must be frozen before a public writer prerelease. Any prerelease that writes it creates a permanent compatibility obligation for stable `0.4.x`; a format flaw requires another Pack Format Epoch.

The complete migration inventory, vertical replacement order, prerelease gates, rollback policy, and documentation obligations remain in [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43).

## Approval blockers

### [Reconcile Pack creation evidence and coverage semantics](https://github.com/sagikazarmark/typst-pack/issues/60)

Accepted creation and format decisions disagree about override-only observations, and the immutable Project Snapshot does not currently expose the revalidation capability required by issuance and sessions. Discovery Coverage matching also lacks one canonical projection against later compilation requests.

**Recommended resolution:** define project closure over every successfully observed logical project path regardless of baseline or override provenance, while always retaining baseline bytes. Treat Project Snapshot bytes as semantically stable but pair mutable-source acquisition with explicit Dependency Evidence Keys and a revalidation capability outside the snapshot. Define Discovery Coverage matching over the shared engine-neutral semantic fields present in both a Discovery Variant and later compilation, not exporter controls or implementation identities.

### [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57)

The format decision selects structures and invariants but does not supply the CDDL, integer and enum registries, identity-schema identifiers, exact transcript encoding, validation order, independent vectors, or a proven narrow-ZIP enforcement strategy. Archive ingress cannot infer a writer-recipe Archive Encoding Identity that is neither stored nor uniquely implied by bytes.

**Recommended resolution:** make the schema, registries, transcript, algorithms, validation pseudocode, and valid/invalid vector corpus normative assets. Validate ZIP framing and ranges before decompression rather than relying on a permissive high-level reader. Return Archive Encoding Identity only from encoding; ingress returns archive Content Identity and may report an encoding identity only when externally asserted and independently verified.

### [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59)

Preparation rejection is defined as occurring before a Compilation Report, while first-party adapters currently promise a report-like object for every status. Complete canonical diagnostics are identity-bearing, but the resource profile permits bounded diagnostic collection without specifying identity behavior. Override replacement identity is described both as public content identity and as a sensitive commitment.

**Recommended resolution:** introduce an adapter terminal union whose branches are Compilation Request Rejection, Compilation Report, and post-report Delivery Outcome; do not broaden the core Compilation Report. Enforce a semantic diagnostic ceiling before terminal commitment and include an explicit canonical truncation marker in the result, while separately limiting noncanonical rendering and source bundles. Use role-bound Compilation Request Commitments in safe public projections and retain exact content identities only in authorized internal or sensitive views. A semantic-cache hit can complete a one-shot attempt but cannot establish Session Currentness without reconstructed live evidence.

### [Define engine parallelism and adapter profile ownership](https://github.com/sagikazarmark/typst-pack/issues/55)

Worker caps do not define their relationship to Typst's process-global Rayon behavior, CLI jobs controls, concurrent in-process compilations, request-isolated timing, or isolated workers. Browser and service resource values are presented as defaults although typst-pack does not ship those products.

**Recommended resolution:** make Execution Facility concurrency the operation-admission control and treat in-process Typst parallelism as process policy, not per-request semantic state. A first-party CLI jobs setting configures the process before work begins and conflicts with incompatible concurrent settings; isolated workers may own independent pools. Publish browser and service values as non-binding reference profiles unless first-party adapters are added; native CLI and Dagger/CI values remain shipped Adapter Resource Defaults.

### [Freeze the Rust lifecycle and adapter interfaces](https://github.com/sagikazarmark/typst-pack/issues/61)

The module prototype is intentionally non-binding. Exact signatures, visibility, error enums, stable-value construction, limit ownership, evidence revalidation, session fencing events, cache traits, and Hostile lifecycle entry are not yet compilable as one interface. Examples also imply a public memory module not present in the accepted seven-module list.

**Recommended resolution:** publish a compile-checked interface fixture for the seven public modules and keep memory helpers under their owning modules rather than adding a generic adapter module. Keep target-specific spool backings in companion adapters behind sealed core-owned Stable Byte Value constructors. Separate semantic preparation from execution admission: format ceilings and semantic request validity apply during preparation; each attempt admits explicit operational limits and may reject a reusable Prepared Compilation before effects when those limits are too strict. Add authority revalidation/subscription interfaces and explicit session fence events. Refuse Hostile in the initial first-party interfaces unless a complete raw-ingress-to-verified-result facility is specified and shipped.

### [Freeze the first-party CLI and Dagger contracts](https://github.com/sagikazarmark/typst-pack/issues/56)

The interaction prototype selects commands and object shapes but leaves public flag names, auxiliary schemas, stable JSON envelopes, generated Dagger names and nullability, archive encoding controls, watch coverage, trust/isolation, publication strength, and invalid-ingress behavior open.

**Recommended resolution:** generate the CLI reference, JSON Schemas, and Dagger schema from one versioned adapter contract checked against the frozen Rust interface. Preserve lifecycle distinctions in adapter terminal unions, require explicit opt-in for incomplete watch coverage, and expose only target capabilities that the adapter can enforce.

### [Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58)

After the preceding tickets close, repeat this synthesis against their resolution comments and normative assets. Approval requires no contradictory invariant, no public interface choice left to implementation, complete traceability, and an explicit statement that remaining variability is operator policy or implementation technique rather than unresolved product architecture.

## Non-blocking implementation parameters

The following choices need values during implementation and release planning but do not create incompatible product architectures when they stay within accepted constraints: exact internal type layout, private helper decomposition, cache capacity and eviction, deployment-specific authority composition, retry policy, benchmark tuning below format ceilings, package ownership administration, docs.rs presentation, and final MSRV selection before the first public alpha.

If implementation work reveals that one of these changes a public interface, persisted representation, semantic identity, guarantee, or migration obligation, it graduates into decision work rather than being silently chosen.

## Traceability

| Decision | Contribution to this synthesis |
| --- | --- |
| [Establish upstream Typst project and filesystem semantics](https://github.com/sagikazarmark/typst-pack/issues/28) | Lexical rooted identities, upstream confinement limits, dependency and watch observation limits |
| [Establish upstream Typst compilation and artifact constraints](https://github.com/sagikazarmark/typst-pack/issues/29) | Synchronous whole-value kernel, cancellation and streaming limits, process-global behavior |
| [Survey real portable Typst project workflows](https://github.com/sagikazarmark/typst-pack/issues/30) | Evidence for distinct portability, authority, compatibility, output, and invalidation contracts |
| [Survey project override and layering precedents](https://github.com/sagikazarmark/typst-pack/issues/31) | Logical/backing separation, replacement semantics, independent authorities |
| [Define the Pack contract and portability guarantees](https://github.com/sagikazarmark/typst-pack/issues/32) | Pack closure, replay verification, portability levels, replacement-only overrides |
| [Prioritize product journeys and quality attributes](https://github.com/sagikazarmark/typst-pack/issues/33) | Library and CLI priority, target constraints, quality ordering |
| [Define project discovery and creation semantics](https://github.com/sagikazarmark/typst-pack/issues/34) | Discovery World, variants, traces, revalidation, replay, issuance |
| [Define extraction and materialization semantics](https://github.com/sagikazarmark/typst-pack/issues/35) | Inspection, baseline materialization, lossless Closure Export |
| [Define the Pack format, invariants, and compatibility model](https://github.com/sagikazarmark/typst-pack/issues/36) | Epoch 2, deterministic CBOR, narrow ZIP, identities, validation and extensions |
| [Prototype the CLI and Dagger interfaces](https://github.com/sagikazarmark/typst-pack/issues/37) | Six-command CLI and typed immutable Dagger lifecycle |
| [Define font lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/38) | Whole-container identity, used faces, ordered catalog, exact fulfillment |
| [Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39) | Seven-module deep architecture and operation-owned lifecycle direction |
| [Define compilation input, output, and diagnostic contracts](https://github.com/sagikazarmark/typst-pack/issues/40) | Preparation, requests, outputs, diagnostics, reports, semantic identities |
| [Define compilation-scoped project variation](https://github.com/sagikazarmark/typst-pack/issues/41) | Pack Override Set, strict preflight, bytes/source behavior and provenance |
| [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42) | Native semantic authority, matrices, properties, fuzzing, platform evidence |
| [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43) | `v0.3.1` baseline, `0.4.0` clean break, recreation, release gates |
| [Define package lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/45) | Exact package tree identity, authority, fulfillment and immutable updates |
| [Define execution, I/O, and cancellation boundaries](https://github.com/sagikazarmark/typst-pack/issues/46) | Dual drivers, one kernel, dependency snapshots, deadlines, commitment, delivery |
| [Define trust, integrity, and confinement guarantees](https://github.com/sagikazarmark/typst-pack/issues/47) | Trust profiles, integrity chain, capabilities and confinement claims |
| [Define dependency, provenance, and invalidation identities](https://github.com/sagikazarmark/typst-pack/issues/48) | Request, evidence and access views; identity and invalidation rules |
| [Define reproducibility and engine-compatibility levels](https://github.com/sagikazarmark/typst-pack/issues/49) | Independent reproducibility predicates and cross-engine compatibility levels |
| [Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50) | Format ceilings and target-specific operational profiles |
| [Define watch and incremental-session semantics](https://github.com/sagikazarmark/typst-pack/issues/51) | Revisions, evidence coverage, fencing, currentness and latest-only publication |
| [Define storage, reference, and transport adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/52) | Stable values, role-specific transport, receipts, commit and cleanup |
| [Establish enforceable confinement guarantees by target platform](https://github.com/sagikazarmark/typst-pack/issues/53) | Honest platform capability matrix and Hostile enforcement requirements |
| [Define cache and session-storage topology](https://github.com/sagikazarmark/typst-pack/issues/54) | Public semantic cache, private reuse, isolation domains and recovery records |

## Approval recommendation

Reject approval in this pass. Preserve the accepted direction, resolve the six named blockers in dependency order, and then use [Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58) for the final review. No implementation ticket should treat this prototype as normative.
