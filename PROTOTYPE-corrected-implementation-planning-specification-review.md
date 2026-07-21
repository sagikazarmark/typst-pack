# Corrected Implementation-Planning Specification Review

**Prototype status:** throwaway closure review and decision evidence, not a production specification.

## Verdict

**Not approved for implementation planning.**

The corrected Rust and first-party adapter artifacts compile, and the adapter replay harness passes, but the joined contract still contains states that cannot be represented without invention or loss. The effort also does not yet have the single immutable connective product, architecture, verification, and migration specification with an explicit precedence ledger required by [Approve the corrected implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/72).

Implementation planning against the current artifacts would force implementers to decide accepted semantics while writing code. That is beyond the map's destination.

## Reviewed Evidence

The review joins these final corrective artifacts rather than treating their individual validation claims as sufficient:

- [Define session preparation and pre-attempt terminal semantics](https://github.com/sagikazarmark/typst-pack/issues/68)
- [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70)
- [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71)
- [Reconcile Rust lifecycle implementability](https://github.com/sagikazarmark/typst-pack/issues/73), including Rust contract commit [`00cfca1`](https://github.com/sagikazarmark/typst-pack/commit/00cfca1987559586066b726c6257d8dd664c3a3c)
- [Regenerate first-party adapters and watch semantics](https://github.com/sagikazarmark/typst-pack/issues/69), including adapter contract commit [`be572ca`](https://github.com/sagikazarmark/typst-pack/commit/be572ca21ee95e3ef149a40365404c2b6ac2c1cc)
- The accepted decisions indexed by [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27), especially [Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50), [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42), and [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43)

## Implementation Blockers

### Paired publication receipts cannot preserve admission refusal

The Rust transport contract has role-specific refusal reasons for network enforcement, concurrency, publication strength, and cleanup, but the paired Format Receipt can express only a `RepresentationAdmissionRefusalReason`. Its publication and cleanup projections have no refused-before-admission branch, while unconditional booleans expose reached facts even on refusal.

Evidence:

- [`TransportAdmissionRefusalReason`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1317-L1330)
- [`RepresentationAdmissionRefusalReason`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L7798-L7833)
- Format publication and cleanup status projections at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L7959-L8043)
- External-consumer exhaustive matches at [`PROTOTYPE-rust-lifecycle-adapter-consumer.rs`](./PROTOTYPE-rust-lifecycle-adapter-consumer.rs#L2299-L2341)

This makes weaker-commit and cleanup-capability refusal lossy, contrary to [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70).

### Font Scan Policy has no operation-owned source

`CreationOperationRequest` does not carry `FontScanPolicy`, and the creation dependency inventory does not report it. The consumer probe manufactures `WarnAndOmit` inside its authority adapter instead of receiving the explicit creation-operation input required by [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71).

Evidence:

- Creation request and dependency inventory at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4507-L4521) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4904-L4910)
- Existing request accessor at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L3255-L3263)
- Adapter-manufactured policy at [`PROTOTYPE-rust-lifecycle-adapter-consumer.rs`](./PROTOTYPE-rust-lifecycle-adapter-consumer.rs#L838-L875)

### Pre-dispatch reports must invent an engine domain

The compilation execution inventory always requires an `EngineRuntimeDomainSelectionView`, but that view has no not-reached or not-selected state. A semantic-cache hit or failure before dispatch must therefore claim inherited unmanaged execution or fabricate a managed identity, placement, and width, despite [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71) requiring no selected domain before dispatch.

Evidence:

- Compilation execution inventory at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5906-L5921)
- Closed domain selection at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L7087-L7096)
- External-consumer exhaustive match at [`PROTOTYPE-rust-lifecycle-adapter-consumer.rs`](./PROTOTYPE-rust-lifecycle-adapter-consumer.rs#L1956-L1972)

### Operational admission inventories remain incomplete

Creation and compilation requests and their six-section report projections omit granted non-secret capability scopes. Their report projections also omit requested and admitted placement and selected isolation as frozen facts. Implementations would have to infer those facts from a reached domain, where one exists.

Evidence:

- Operation requests at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4507-L4521) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L6555-L6571)
- Admission views at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4888-L4896) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5852-L5860)
- Role execution views at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4931-L4946) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5906-L5921)

### Compilation preparation and admission ordering contradict each other

The adapter narrative places Compilation Operation admission before preparation can produce a Compilation Request Rejection. The strict rejection schema instead requires unadmitted adapter jobs and carries no admission record. The contract must choose one precedence and serialize its consequences consistently.

Evidence:

- Narrative ordering at [`PROTOTYPE-first-party-cli-dagger-contracts.md`](./PROTOTYPE-first-party-cli-dagger-contracts.md#L663-L667) and [`PROTOTYPE-first-party-cli-dagger-contracts.md`](./PROTOTYPE-first-party-cli-dagger-contracts.md#L1374-L1379)
- Rejection branches at [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L1325-L1326) and [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L1946)

The recommended precedence is pure semantic preparation before operational admission. A Compilation Request Rejection therefore has requested lexical jobs but no admitted width or Operation Admission Record. This matches the terminal ownership already accepted by [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71).

### Refused receipts lose their admission stage

The strict Format and Transport Receipt refusal branches omit stage `admission`, although [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70) requires that stage on both families. The stage-bearing common effect projection exists only after successful admission.

Evidence:

- Claimed common receipt projection at [`PROTOTYPE-first-party-cli-dagger-contracts.md`](./PROTOTYPE-first-party-cli-dagger-contracts.md#L1049-L1065)
- Refused Format branch at [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L802-L814)
- Refused Transport branch at [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L1798-L1801)

### Required transport object count is not publicly reachable

The strict admitted Transport Receipt requires `object_count`. The public role receipts expose a `TransportStageLedgerView` without that accessor; only a crate-private generic receipt exposes it. The serializer probe cannot obtain the field, and the replay harness inserts a literal.

Evidence:

- Required schema field at [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L1792-L1796)
- Crate-private accessor at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L881-L882) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L962-L979)
- Public stage-ledger surface at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1071-L1118) and [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1155-L1169)
- Probe and harness gap at [`PROTOTYPE-first-party-cli-dagger-serializer-probe.rs`](./PROTOTYPE-first-party-cli-dagger-serializer-probe.rs#L198-L213) and [`PROTOTYPE-first-party-cli-dagger-validation/src/main.rs`](./PROTOTYPE-first-party-cli-dagger-validation/src/main.rs#L1234-L1240)

### Aggregate creation resource laws are missing

[Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50) requires project files and complete package trees to debit one total file budget, and project, package, and font bytes to debit aggregate decoded, spool, and memory budgets. The corrected Rust contract and strict schemas expose only independent category limits.

The first-party profiles therefore admit category sums beyond their own ingress ceilings. `native-cli/1` can admit 200,000 project-plus-package files against a 100,000-file ingress ceiling and 7 GiB of project/package/font bytes against a 4 GiB decoded-ingress ceiling. `dagger-ci/1` has the analogous 200,000-file and 14-GiB category sums against 100,000-file and 8-GiB ingress ceilings.

Evidence:

- Rust limits at [`PROTOTYPE-rust-lifecycle-adapter-interfaces.rs`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4413-L4504)
- Strict limits schema at [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json#L321-L355)
- Native profile at [`PROTOTYPE-native-cli-profile.json`](./PROTOTYPE-native-cli-profile.json#L17-L46)
- Dagger profile at [`PROTOTYPE-dagger-ci-profile.json`](./PROTOTYPE-dagger-ci-profile.json#L17-L46)

An implementation would have to invent aggregate accounting or permit creation and representation output that the same adapter cannot read.

## Joined-Specification Gaps

### No connective specification or precedence ledger exists

The final Rust and adapter artifacts are vertical contracts. They do not form the required single product, architecture, verification, and migration specification. Every existing connective review artifact rejects approval, and the final adapter artifact records only a partial list of replacements.

A successor artifact must state clause-level authority among foundational decisions, later corrections, final interface and schema commits, format deltas, the canonical glossary, accepted historical ADRs, and rejected review artifacts. Until then, an implementer cannot determine the controlling statement when two accepted sources differ.

### Canonical vocabulary and accepted ADRs still compete with the target

The immutable glossary carried by the reviewed prototype commits predates the final representation/transport and session corrections. Accepted ADRs still describe Resource Slots and defer watch, while the clean-sheet target removes Resource Slots and specifies watch. The eventual connective artifact must identify exact surviving and superseded clauses and the migration handoff must require the target ADR and status updates; chronology alone is not a precedence rule.

The public and wire name `cleanup_strength` also survives after [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70) separated a cleanup requirement from a cleanup outcome. The corrected contract should use `cleanup_requirement` without an unreleased compatibility alias.

### Verification evidence is not a claim-to-gate matrix

[Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42) describes the semantic strategy, while final corrective artifacts list narrower probes and implementation gates. No immutable artifact joins every accepted claim to its authoritative decision, prototype evidence, required native/property/model/platform test, and prerelease gate.

The adapter replay also does not semantically reject cross-session or cross-revision publication identity mismatches. JSON Schema cannot enforce those joins, so the locked semantic harness must. The current watch viewer is illustrative shorthand, not an exact serialized-state or reducer-execution proof.

## Corrective Route

The minimum decision route is:

1. Resolve the remaining lifecycle and receipt contract gaps: paired-refusal projection, explicit Font Scan Policy, no-domain-selected execution, complete admission inventories, pure-preparation precedence, refusal stages, public transport object counts, and canonical cleanup vocabulary.
2. Resolve aggregate creation and representation resource accounting: category and aggregate budgets, first-seam debiting, memory and spool composition, and first-party output-to-ingress coherence.
3. Regenerate one final Rust and first-party adapter contract set from those decisions, including strict schemas, profiles, serializer provenance, semantic publication-fence checks, and boundary/coherence replay cases.
4. Assemble and review one immutable connective specification with an explicit precedence ledger, joined verification matrix, canonical vocabulary snapshot, ADR disposition, and clean-break migration handoff.

The first two decisions are independent. Regeneration waits on both. Final assembly and approval waits on regeneration. No additional in-scope fog is visible.

## Verification Performed

- Both Rust interface and external-consumer fixtures compile with warnings denied under Rust 1.92 and a newer host toolchain.
- The adapter `cargo fmt --all -- --check` and `cargo check --locked --all-targets` checks pass.
- The locked adapter replay passes its advertised 355 schema definitions, 1,411 local references, 40 direct cases, 45 generated cases, 20 semantic cases, 16 capability constants, six publication sequences, and two delivery wrappers.
- The successful checks prove syntax and the cases they execute. They do not close the unreachable states, cross-artifact contradictions, aggregate resource omission, or missing connective specification above.

## Residual Implementation Gates

After a successor approval, the following remain execution rather than wayfinding decisions:

- implement runtime bodies and integrate the seven public modules;
- materialize the Epoch 2 corpus and prove independent decoder agreement;
- add native semantic, property, model, fuzz, fault, compatibility, and platform suites;
- perform actual Dagger generation, introspection, structural parity, and generated-client compilation;
- verify worker termination, reaping, confinement, cleanup, and target-specific enforcement;
- inventory the exact `v0.3.1` public surface, write the migration guide, publish the superseding target ADR, and execute the `0.4.0` release;
- tune benchmarks and adapter defaults without weakening frozen ceilings or identities.
