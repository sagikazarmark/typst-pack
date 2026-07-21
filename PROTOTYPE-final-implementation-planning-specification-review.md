# PROTOTYPE: Final Implementation-Planning Specification Review

> Throwaway closure review for
> [Assemble and approve the final implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/79).
> It is decision evidence, not the implementation contract, and must not be
> merged as production documentation.

## Verdict

**Not approved for implementation planning.**

The format, identity, receipt, resource law, and adapter directions are
coherent, and the final prototype validation passes the cases it executes.
Joined review nevertheless found three implementation-blocking defects in two
interface clusters:

1. the public creation resource view omits accepted reached category facts; and
2. Compilation Session neither receives the exact one-shot preparation policy
   and limits nor binds a Start Attempt plan to fallible operation admission
   without losing the reportless admission-refusal branch.

The final narrative also contains a reversed compilation-precedence sequence
and examples that disagree with the compile-checked fixture. The required
connective precedence ledger, canonical vocabulary snapshot, ADR disposition,
and claim-to-gate matrix can now be stated precisely, but publishing them as an
approved specification would conceal the interface gaps rather than resolve
them.

These findings are narrow and correctable in one integrated contract
regeneration. No product, Pack, format, identity, trust, migration, or
seven-module architecture decision is reopened. No additional unresolved
design question was identified in this review; regeneration and another joined
review remain required.

## Reviewed Evidence

This review joins rather than individually trusts:

- the accepted decisions indexed by
  [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27);
- [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77);
- [Define aggregate creation and representation resource accounting](https://github.com/sagikazarmark/typst-pack/issues/76);
- [Define session preparation and pre-attempt terminal semantics](https://github.com/sagikazarmark/typst-pack/issues/68);
- [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42);
- [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43);
- [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78), including the Rust 1.92 fixture, external-consumer probe, strict schemas, desired GraphQL, first-party profiles, serializer probe, session trace viewer, and locked replay harness at commit [`a92c6b3`](https://github.com/sagikazarmark/typst-pack/commit/a92c6b3);
- [`CONTEXT.md`](./CONTEXT.md) and every accepted ADR under [`docs/adr`](./docs/adr); and
- the rejected predecessor reviews, used only as historical defect evidence.

The review treats a passing compile probe as type-shape evidence and a passing
replay as evidence for its executed assertions. Neither proves that omitted
state can be supplied or that unimplemented bodies can preserve accepted
ownership and precedence.

## Confirmed Closure

The joined review found no implementation-blocking contradiction in these
previously corrected areas:

- Pack Format Epoch 2 structure, canonical CBOR, narrow ZIP and Closure Export
  profiles, base validation order, and kinds 1 through 13;
- compilation-family kinds 14 through 19, identity transcripts, SHA-256
  applicability ceiling, and independent vectors;
- the single `epoch-2-all-stored-v1` writer recipe and required Stored,
  Deflate, and mixed-method reader cases;
- preparation-before-admission semantics for one-shot compilation in the
  accepted decision and Rust terminal tree; session preparation remains a
  blocker below;
- operation-owned requested, semantic-subject, capability, admitted, and
  reached fact sources;
- paired publication refusal, explicit admission stages, public transport
  object count, and cleanup requirement versus cleanup outcome;
- Font Scan Policy request, admission, and reached ownership;
- `NotSelected`, `InheritedUnmanaged`, and `Managed` Engine Runtime Domain
  selection;
- declarative creation and representation accounting law, logical `F_pack` and
  `L_pack`, physical `C/B/P/N`, archive `A`, materialization `M/J`, and
  output-to-ingress profile arithmetic; lossless reached creation reporting
  remains a blocker below;
- seven deep Rust modules and opaque role-specific lifecycle values;
- six first-party CLI commands, typed Dagger lifecycle objects, strict JSON
  branches, and profile-owned defaults rather than profile-inferred evidence;
  and
- reportless session attempt-admission refusal in the reducer event vocabulary;
  executable admission binding remains a blocker below.

Actual runtime behavior, production serializers, corpus materialization,
independent decoder agreement, real Dagger generation, platform enforcement,
and release execution remain implementation or prerelease gates rather than
planning contradictions.

## Implementation Blockers

### Reached Creation Categories Are Not Lossless

The accepted resource decision requires reports and receipts to expose
requested, admitted, and reached category totals, aggregate totals, exact
outputs, and peak occupancy from the sealed operation ledger.

[`CreationResourceLimits`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4711-L4810)
declares project files and largest file, package and package-file dimensions,
font candidates and faces, discovery variants and restarts, overrides,
aggregates, spool, and memory. The public
[`CreationResourceReachedView`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5403-L5418)
omits at least:

- package count;
- largest reached project file;
- largest reached package member;
- Font Catalog candidate count;
- font face count;
- discovery variant count; and
- discovery restart count.

The strict schema and serializer probe repeat the narrower projection. An
adapter cannot report these accepted reached category facts through a public
Rust accessor and therefore must omit or invent them. The corrected sealed view,
strict schema, serializer provenance probe, and mutation cases must carry every
category and aggregate dimension.

### Session Preparation Does Not Use The One-Shot Contract

One-shot preparation requires an explicit
[`CompilationPreparationPolicy`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L6046-L6050)
and
[`CompilationPreparationLimits`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L6052-L6059),
then returns a Prepared Compilation or Compilation Request Rejection before any
operation admission.

The session instead stores
[`SessionPreparationLimits`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L9430-L9461),
which wraps requested and admitted `CompilationResourceLimits` and contains no
Compilation Preparation Policy. `SessionPolicy` exposes no policy or exact
one-shot preparation limits. `CompilationResourceLimits` carries noncanonical
diagnostic projection limits, not the canonical diagnostic entry and byte
ceilings required by Compilation Preparation Limits. The harness silently
substitutes selected profile fields and hardcodes both policy booleans, but
neither derivation is part of the generic Rust session contract.

An implementation must therefore invent a policy, invent a numeric mapping, or
run a different preparation path. Each violates
[Define session preparation and pre-attempt terminal semantics](https://github.com/sagikazarmark/typst-pack/issues/68),
which requires exact revision-owned pure preparation, and
[Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77),
which forbids hidden defaults and facility appraisal before preparation.

The corrected session policy must own the exact Compilation Preparation Policy
and Limits used by the same private preparation implementation as one-shot
compilation. A derivation from a broader resource profile is valid only if every
field and origin is frozen and inspectable in the public contract.

### Start Attempt Has No Fallible Admission-Binding Seam

The accepted session flow is:

```text
Start Attempt
    -> operation admission
        -> Attempt Admission Refused
        -> admitted token-bound attempt
            -> Attempt Finished
```

The final fixture cannot express that composition safely:

- [`SyncCompilationControls::try_admit`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L7331-L7371)
  consumes a Prepared Compilation and may return Compilation Admission Refusal.
- `bind_session` may also refuse but is crate-private.
- [`SessionAttemptPlan::run_sync`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L9755-L9765)
  accepts already admitted controls and can return only Compilation Report.
- the asynchronous path has the same mismatch.

The adapter receiving `StartAttempt` can call public operation admission with a
clone of `plan.prepared()` and can return that first refusal through the exact
`AttemptAdmissionRefused` event. It cannot call the later private binding
operation, however, and safe public code can pair the plan with controls
admitted for a different Prepared Compilation or limits. If `run_sync` performs
the fallible binding internally, its return type cannot preserve that second
reportless refusal. Unwrapping, fabricating a report, dropping the refusal, or
admitting twice all contradict the accepted lifecycle and the requirement that
incorrect composition be unrepresentable at the public seam.

The corrected seam must consume the Session Attempt Plan and exact operation
facilities once, then return either the token-bound Compilation Admission
Refusal needed by the reducer event or an admitted session-attempt value. The
admitted value must bind by construction the plan, Prepared Compilation,
Session Attempt Token, supersession permit, admitted limits, and Operation
Admission Record. Running that value must be infallible with respect to
admission and return only a Compilation Report.

## Connective Specification Defects

These are deterministic corrections, not new design questions, but an approved
artifact must not preserve them.

### Compilation Failure Precedence Is Reversed In One Sequence

The accepted and type-visible order is pure semantic preparation, then either
Compilation Request Rejection or Prepared Compilation, then Compilation
Operation admission. The numbered sequence in
[`PROTOTYPE-first-party-cli-dagger-contracts.md`](./PROTOTYPE-first-party-cli-dagger-contracts.md#L1465-L1470)
instead places operation admission before request rejection. This conflicts
with the same artifact's verdict and compile flow, the Rust terminal tree, the
semantic harness, and
[Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77).

The corrected order is:

1. stabilize adapter inputs and resolve explicit semantic defaults;
2. run pure bounded semantic preparation;
3. return Compilation Request Rejection or Prepared Compilation;
4. admit only the Prepared Compilation;
5. return reportless Compilation Admission Refusal or run one admitted attempt;
6. commit one immutable Compilation Report; and
7. run cache admission, delivery, publication, disclosure, rendering, and
   response transport as later operations that cannot reclassify the report.

### Narrative Examples Disagree With The Fixture

The Rust narrative still sketches `prepare` with Compilation Resource Limits
instead of Preparation Policy and Preparation Limits, and `run_sync` with both
a Prepared Compilation and controls even though controls already own the
prepared value. Its common creation path passes admitted limits where
[`ProjectSnapshot::try_from_files`](./PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4271-L4278)
requires a Creation Resource Ledger.

The compile-checked fixture controls signatures, but leaving contradictory
examples would force implementers to guess which interface is intended. The
regenerated narrative and every adapter example must compile against the exact
fixture or be mechanically extracted from a checked consumer.

### Vocabulary And ADR Disposition Are Still Historical

The current glossary at commit `a92c6b3`, Git blob
`b871514b5c2c66c5562272a6002521c043e191b3`, already separates Transport
Cleanup Requirement from Transport Cleanup Outcome. It does not define the
accepted Operation Capability Descriptor, Operation Admission Record,
Operational Capability Class, Operation Network Policy, Format Receipt,
Representation Admission Refusal, Compilation Preparation Policy and Limits,
Session Evaluation, Session Ingestion Failure, Superseded Session Attempt, or
Session Retirement. Existing ADR statuses also leave Resource Slots, the
two-command-only CLI shape, and deferred watch looking authoritative.

The successor connective specification must pin one corrected glossary
snapshot, distinguish glossary-owned concepts from exact Rust-only names, and
record:

- **ADR-0002: Make Pack the Owner of Whole-Pack Invariants** is partially
  superseded: the private whole-Pack construction seam survives; the Epoch 1
  TOML manifest, old ZIP profile and unknown-entry assumptions, Resource Slot,
  and old builder details do not.
- **ADR-0003: Centralize External Resource Reference Semantics** remains
  superseded and has historical value only.
- **ADR-0004: Model Resource Slots and Resource Providers** is fully
  superseded; the target has neither concept nor compatibility surface.
- **ADR-0005: Align the CLI with Embedded Typst** is partially superseded:
  exact embedded-Typst parity where semantics match and the `create` and
  `compile` commands survive; the two-command-only shape, obsolete options,
  Resource Slot surface, deferred watch, and old Dagger shape do not.

Updating ADR files and publishing the superseding target ADR remains migration
execution, but the planning specification must make the target disposition
unambiguous now.

## Required Precedence Model

The successor specification can use this already-determined authority ledger.
A source controls only its declared subject. Dates, issue numbers, branch
ancestry, executable form, and passing tests never establish precedence.

| Authority class | Controlling source | Controlled clauses and disposition |
| --- | --- | --- |
| Destination and scope | [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27) | Destination, planning boundary, Notes, and out-of-scope work. Its one-line decision gists defer to each linked resolution. |
| Product and domain decisions | Each accepted child ticket's resolution comment | The answer to that ticket's stated question. A linked artifact is authoritative only where the resolution expressly adopts it. |
| Canonical vocabulary | The corrected snapshot to be pinned by the successor specification | Names and domain definitions only. The current `a92c6b3` glossary is baseline evidence and is non-authoritative for the missing terms listed above. |
| Pack Format Epoch 2 | [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57) and its adopted artifact | Base control record, kinds 1 through 13, narrow ZIP and Closure Export profiles, validation, and base receipts. |
| Compilation-family identities | [Freeze the compilation-family value and identity registry](https://github.com/sagikazarmark/typst-pack/issues/66) and its adopted artifact | Kinds 14 through 19, compilation-family values and transcripts, and the SHA-256 applicability correction over the broader base object ceiling. |
| Writer and corpus delta | [Freeze the Epoch 2 writer and interoperability corpus](https://github.com/sagikazarmark/typst-pack/issues/64) and its adopted artifact | The sole initial all-Stored writer, reader corpus additions, receipt vectors, and corpus authorship rules over the base format artifact. |
| Operation inputs and receipts | [Freeze operational capability and execution-report inputs](https://github.com/sagikazarmark/typst-pack/issues/71) and [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70) | Requested, capability, admission, reached, representation, and transport baselines, except where explicitly corrected below. |
| Final lifecycle corrections | [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77) | Preparation-before-admission, five fact sources, paired publication receipts, Font Scan Policy, capability scope, placement, isolation, domain selection, refusal stage, object count, and cleanup corrections over all earlier lifecycle artifacts. |
| Aggregate resource law | [Define aggregate creation and representation resource accounting](https://github.com/sagikazarmark/typst-pack/issues/76) | Category plus aggregate creation accounting, logical and physical representation accounting, occupancy, all-Stored planning, and same-profile coherence over earlier category-only or generic representation limits. |
| Exact final contract baseline | Commit [`a92c6b3`](https://github.com/sagikazarmark/typst-pack/commit/a92c6b3), adopted by [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78) | Unaffected exact Rust, schema, GraphQL-target, profile, and serializer details only. It is rejected for the three blockers and deterministic prose defects in this review until a corrected regeneration explicitly replaces them. |
| Corrected exact contracts | The future **Correct the final Rust and first-party contract realization** resolution and adopted commit | Replaces `a92c6b3` only for its named correction set, then becomes the exact interface and wire authority for the successor joined review. |
| Verification | [Define the test architecture and verification matrix](https://github.com/sagikazarmark/typst-pack/issues/42), extended by later decision-specific gates | Required evidence classes and claim ownership. Prototype checks prove only their executed cases. |
| Migration and release | [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43) | `v0.3.1` baseline, no intermediate release or shims, recovery and recreation rather than conversion, package train, prerelease gates, rollback, and documentation. |
| ADR disposition | The successor specification, followed during implementation by the superseding target ADR and status edits | Surviving rationale and explicitly superseded clauses listed above. Old `Accepted` status does not override the clean-sheet target. |
| Rejected reviews | None | Defect and traceability evidence only. Their connective prose is not normative. |
| Current source and signed `v0.3.1` | None for the target | Migration inventory and behavioral evidence only. Resource Slot-era `main` has no compatibility or target authority. |

A later accepted source changes an earlier clause only when its resolution
explicitly supersedes, amends, replaces, or corrects that subject, and the edge
is limited to the named clauses. Any same-subject conflict remaining after
these edges is an approval blocker; an implementer may not choose by apparent
specificity, convenience, executable form, or recency.

## Required Claim-To-Gate Matrix

The successor specification must freeze the following claim families. Each row
must link its accepted authority, planning evidence, required implementation
evidence, and release gate.

| Claim family | Accepted authority | Existing planning evidence | Required implementation evidence | Gate |
| --- | --- | --- | --- | --- |
| Pack closure, discovery, evidence fence, replay, issuance | [Define the Pack contract and portability guarantees](https://github.com/sagikazarmark/typst-pack/issues/32), [Define project discovery and creation semantics](https://github.com/sagikazarmark/typst-pack/issues/34), and [Reconcile Pack creation evidence and coverage semantics](https://github.com/sagikazarmark/typst-pack/issues/60) | Rust fixture exposes the values and type flow; bodies are absent | Native public-seam, property, replay, and mutable-source fault suites | Alpha: executable vertical path. Release candidate: full claim matrix. |
| Package, font, override, authority, and provenance | [Define package lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/45), [Define font lifecycle and authority](https://github.com/sagikazarmark/typst-pack/issues/38), [Define compilation-scoped project variation](https://github.com/sagikazarmark/typst-pack/issues/41), and [Define dependency, provenance, and invalidation identities](https://github.com/sagikazarmark/typst-pack/issues/48) | Compile probes cover contract reachability only | Authority conformance, malformed input, fallback, embedding, fulfillment, provenance, and override matrices | Beta: feature complete. Release candidate: exhaustive dependency matrix. |
| Epoch 2 validity and convergence | [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57) | Adopted normative artifact fixes schemas, algorithms, and corpus manifest; corpus bytes remain unmaterialized | Materialized corpus, independent authorship, two decoders, parser properties, malformed vectors, and fuzzing | Hard writer gate before any public alpha writes Epoch 2. |
| All-Stored writer and Archive Encoding Identity | [Freeze the Epoch 2 writer and interoperability corpus](https://github.com/sagikazarmark/typst-pack/issues/64) | Writer recipe, identity, and vector requirements are frozen; final replay checks selected arithmetic only | Byte-exact golden reproduction and complete Stored, Deflate, and mixed reader corpus | Hard writer gate before any public alpha writes Epoch 2. |
| Compilation-family identities and reproducibility | [Freeze the compilation-family value and identity registry](https://github.com/sagikazarmark/typst-pack/issues/66) and [Define reproducibility and engine-compatibility levels](https://github.com/sagikazarmark/typst-pack/issues/49) | Adopted registry contains independent vectors; Rust exposes opaque typed identities | Normative vectors, included/excluded field properties, environment perturbation, and finite cross-engine corpus | Alpha: executable identity vectors. Release candidate: every advertised compatibility level. |
| Preparation, compilation, diagnostics, and disclosure | [Define compilation input, output, and diagnostic contracts](https://github.com/sagikazarmark/typst-pack/issues/40) and [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59) | Fixture and schemas represent branches; the session preparation gap remains open | Public terminal, all-format, rejection, result/outcome, redaction, commitment, and diagnostic suites | Alpha: one vertical path. Beta: feature complete. Release candidate: full format and terminal matrix. |
| Sync/async, facilities, interruption, and isolation | [Define execution, I/O, and cancellation boundaries](https://github.com/sagikazarmark/typst-pack/issues/46), [Define engine parallelism and adapter profile ownership](https://github.com/sagikazarmark/typst-pack/issues/55), and [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77) | Compile fixture represents controls and outcomes; bodies and worker protocol are absent | Sync/async equivalence, explicit-clock fault injection, race models, queue and worker failure, termination, and reaping | Beta: execution paths complete. Release candidate: positive evidence for every advertised strength. |
| Representation, transport, receipts, and publication | [Define extraction and materialization semantics](https://github.com/sagikazarmark/typst-pack/issues/35), [Define storage, reference, and transport adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/52), [Define pre-admission representation and transport receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/70), and [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77) | Final replay checks selected paired receipts, object counts, and publication fences | Stage fault injection, backpressure, cancellation, commit/cleanup races, filesystem atomicity, and semantic cross-receipt mutation rejection | Beta: roles complete. Release candidate: every shipped sink and publication-strength claim. |
| Session, watch, cache, and currentness | [Define watch and incremental-session semantics](https://github.com/sagikazarmark/typst-pack/issues/51), [Define cache and session-storage topology](https://github.com/sagikazarmark/typst-pack/issues/54), and [Define session preparation and pre-attempt terminal semantics](https://github.com/sagikazarmark/typst-pack/issues/68) | Reducer vocabulary and illustrative viewer cover selected scenarios; preparation and binding remain open | Executable reducer refinement, generated event sequences, read/arm/confirm races, watcher gaps, and cache authorization/corruption | Beta: reducer and selected cache topology complete. Release candidate: native watch/currentness paths. |
| Aggregate resources and performance | [Prototype representative scale and resource limits](https://github.com/sagikazarmark/typst-pack/issues/50) and [Define aggregate creation and representation resource accounting](https://github.com/sagikazarmark/typst-pack/issues/76) | Profiles and replay verify selected relationships; reached creation reporting remains open | Limit-minus-one/limit/plus-one, overflow, occupancy, deduplication, lying iterators, and same-profile round trips | Every change: resource-boundary suite. Release candidate: pinned production-equivalent benchmarks. |
| Trust, integrity, and confinement | [Define trust, integrity, and confinement guarantees](https://github.com/sagikazarmark/typst-pack/issues/47) and [Establish enforceable confinement guarantees by target platform](https://github.com/sagikazarmark/typst-pack/issues/53) | Primary-source platform research and early Hostile refusal type shapes exist | Integrity/substitution, parser/path/protocol fuzzing, platform tests, and mandatory first-release Hostile refusal | Release candidate: every shipped claim is positively verified or explicitly refused. |
| Rust module and target architecture | [Prototype the library module and interface architecture](https://github.com/sagikazarmark/typst-pack/issues/39) and the corrected successor to [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78) | `a92c6b3` compiles seven modules and an external consumer but is rejected for the named defects | Exact Rust 1.92, compile-fail forbidden seams, external consumer, featureless WASM build, and browser execution | Alpha: package graph and vertical interface fixed. Release candidate: interfaces, MSRV, and target claims frozen. |
| CLI and Dagger | The corrected successor to [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78) | Strict schemas, hand-authored GraphQL target, profiles, and replay exist; actual Dagger parity is deferred | Typst parity snapshots, strict JSON and precedence, actual Dagger generation, introspection, comparison, and generated-client compilation | Beta: six commands and typed graph complete. Release candidate: packaged CLI and Dagger surfaces frozen and verified. |
| Clean break and coordinated release | [Define the clean-break migration and release strategy](https://github.com/sagikazarmark/typst-pack/issues/43) | Baseline and gate decisions exist; exact inventory, guide, target ADR, and packaging do not | Exact `v0.3.1` inventory before old-seam deletion, compiling migration examples, dry runs, clean installs, checksums, tagged Dagger ingestion, and post-publish vectors | Alpha: migration guide and no-intermediate-release guard. Stable: coordinated train and post-publication verification. |

The first-release claim manifest is featureless `typst-pack`,
`typst-pack-fs`, `typst-pack-cli`, and the tagged Dagger module at Rust 1.92.
The core remains suitable for WASM, but no browser or service adapter and no
Hostile-capable deployment ship in the first release.

## Corrective Route

The smallest route is two proposed `wayfinder:prototype` tickets in one linear
chain. Separate reconciliation tickets would reopen accepted semantics and
duplicate integration across the same fixture, schemas, serializers, and
harness.

| Proposed ticket | Question owned | Blocking |
| --- | --- | --- |
| **Correct the final Rust and first-party contract realization** | What smallest corrected Rust 1.92 fixture, external-consumer probe, narrative, strict schemas, desired GraphQL, first-party profiles, serializer-provenance probe, session trace model, and locked replay makes every accepted creation resource and session state lossless and unrepresentable incorrectly: all reached creation categories are publicly projected; Session Policy carries the exact one-shot Compilation Preparation Policy and Limits; and each Session Attempt Plan enters one fallible token-bound admission seam yielding reportless Attempt Admission Refused or an admitted attempt bound to the plan, Prepared Compilation, controls, token, supersession permit, admitted limits, and Operation Admission Record? It must also repair the identified precedence and narrative defects without changing any other accepted decision. | Unblocked |
| **Assemble and approve the definitive implementation-planning specification** | After **Correct the final Rust and first-party contract realization** is accepted, what single immutable connective product, architecture, canonical vocabulary, target ADR disposition, verification, and clean-break migration specification applies the clause-level precedence ledger and claim-to-gate matrix, preserves every accepted decision, contains no implementation-blocking contradiction or omission, and is approved for implementation planning? | Blocked only by **Correct the final Rust and first-party contract realization** |

The first ticket corrects realization, not resource or session semantics. The
second owns final assembly of the already-specified precedence, vocabulary,
ADR, claim-to-gate, and migration sections and leaves runtime implementation and
production verification outside the map.

## Map Effect

Recording this review will resolve
[Assemble and approve the final implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/79)
as not approved rather than repurposing it. The two proposed tickets must be new
children of the map with the blocking edge above. No new fog is needed: both
questions are precise. Commit `a92c6b3` remains the accepted baseline for
unaffected exact details, while this review and the correction ticket own the
known residual defects.

## Verification Performed

This review independently reran the final prototype validation on Rust 1.96.1
from `PROTOTYPE-first-party-cli-dagger-validation`:

- `cargo fmt --all -- --check` passed;
- `cargo test --locked --all-targets` passed, including the public transport
  object-count test and all fixture, consumer, and serializer targets;
- `cargo run --locked` passed with 367 schema definitions, 1,501 local
  references, 40 direct cases, 45 generated cases, 17 capability constants, 31
  mechanically linked serializer leaves, 17 poison derivations, 46 semantic
  cases, seven illustrative session scenarios, and six publication fences; and
- the review artifact passed Git's whitespace-error check after staging.

[Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78)
also records successful Rust 1.92 container validation. The independent rerun
did not repeat that container lane. The checks establish compile shape, strict
schema and selected semantic replay only. The GraphQL file is a hand-authored
target, the HTML is a structural trace viewer rather than reducer execution,
and actual Dagger generation, runtime bodies, production serializers, corpus
bytes, platform enforcement, and release gates remain deferred.

## Residual Implementation Gates

After a successor approval, these remain execution rather than decision work:

- implement runtime bodies and production serializers behind the seven modules;
- materialize the Epoch 2 corpus and obtain independent decoder agreement;
- run actual Dagger generation, introspection, structural parity, and generated
  client compilation;
- implement native semantic, property, model, fuzz, fault, compatibility,
  platform, confinement, and benchmark suites;
- inventory `v0.3.1` and begin the migration guide before deleting or renaming
  old seams, publish the target ADR after successor approval, and execute one
  coordinated `0.4.0` clean break with no intermediate release or shims;
- recover historical Packs with the pinned legacy implementation and recreate
  them under the target rather than converting them; and
- package, prerelease, release, verify, and if necessary roll forward the
  versioned train.

## Recommendation

Reject approval in this pass. Preserve every accepted product, format,
identity, architecture, receipt, resource-law, adapter, verification, and
migration decision; correct the three realization defects in one integrated
contract prototype; and then assemble the final connective specification. No
implementation effort should treat commit `a92c6b3` as jointly complete until
that route closes.
