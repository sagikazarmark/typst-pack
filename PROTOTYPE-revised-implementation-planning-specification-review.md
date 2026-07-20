# PROTOTYPE: Revised Implementation-Planning Specification Review

> Throwaway review artifact for [Approve the revised implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/62). It is not an implementation contract and must not be merged as-is.

## Status

**Review verdict: not approved for implementation planning.**

The four corrective decisions created by [Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58) close the previously identified identity-registry, writer-recipe, discovery request-kind, and broad public-projection gaps. Their accepted artifacts still do not compose into one implementable contract. The remaining blockers are precise interface-shape and cross-artifact contradictions, not implementation work or undifferentiated fog.

Implementation planning should wait for the corrective route below.

## Reading Model

- [`CONTEXT.md`](https://github.com/sagikazarmark/typst-pack/blob/main/CONTEXT.md) is the canonical glossary.
- [Redesign typst-pack as a library-first portable Typst project system](https://github.com/sagikazarmark/typst-pack/issues/27) is the low-resolution decision index.
- [Implementation-planning specification review](https://github.com/sagikazarmark/typst-pack/blob/a0fecd139ce1fbdf6bb68ec61564379438a6fb1c/PROTOTYPE-implementation-planning-specification-review.md) is the previous connective review and remains historical because its verdict was rejection.
- [Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md) is the base representation contract.
- [Compilation-Family Value and Identity Registry](https://github.com/sagikazarmark/typst-pack/blob/a482145a0c790bb67d7f6d4424777519c614876e/PROTOTYPE-compilation-family-identity-registry.md) amends that base with compilation-family values, identity kinds 14 through 19, and the SHA-256 applicability erratum.
- [Epoch 2 writer and interoperability corpus](https://github.com/sagikazarmark/typst-pack/blob/23d8bea991c0d55f3aeaee4cd3137c67c8d9d496/PROTOTYPE-epoch-2-writer-interoperability-corpus.md) amends the base with the one-recipe initial writer registry and corpus revision 1.
- [Completed Rust lifecycle and receipt interfaces](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.md), its [Rust fixture](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs), and its [consumer probe](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-consumer.rs) replace the earlier Rust prototype.
- [Reconciled first-party adapter contract](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-first-party-cli-dagger-contracts.md) and its [GraphQL](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-first-party-cli-dagger-generated.graphql), [JSON schemas](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-first-party-cli-dagger-schemas.json), [native profile](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-native-cli-profile.json), [Dagger profile](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-dagger-ci-profile.json), and [lifecycle model](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-first-party-cli-dagger-contracts.html) replace the earlier adapter prototype.
- A resolution comment owns its decision. A later artifact controls an earlier artifact only where the resolution explicitly replaces, amends, or corrects it. This review does not silently resolve disagreements between accepted sources.

## Confirmed Closure

The joined review confirmed these prior blockers are closed:

- The compilation-family registry freezes deterministic CBOR values, exact domain-separated transcripts, identity kinds 14 through 19, complete nested semantic projections, and independently reproducible golden vectors.
- The initial Epoch 2 writer registry contains only `epoch-2-all-stored-v1`; Deflate is mandatory reader behavior, not an unregistered writer claim. Generic ingress never infers an Archive Encoding Identity.
- Discovery and compilation traces use only the canonical `typst-source` and `raw-file` request kinds.
- The Rust contract now exposes complete font-candidate metadata, lossless Pack Inspection, compilation rejection inventories, structured traces and evidence, artifact identities, seven disclosure capabilities, archive-only expectations, and separate Format and Transport Receipt types.
- The adapter artifacts preserve six CLI commands, typed Dagger lifecycle objects, strict identity kinds, accepted and rejected request inventories, canonical evidence joins, archive-only expectations, distinct Format and Transport outcomes, and the two accepted first-party profiles.

These closures are necessary but not sufficient for approval.

## Approval Blockers

### Session Preparation Has No Implementable Transition

The accepted session owns pure preparation and must publish either a Compilation Request Rejection or an executable attempt. The Rust fixture requires explicit `CompilationResourceLimits` for [`compilation::prepare`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4122-L4128), but neither [`CompilationSession::new`, `SessionPolicy`, nor `StabilizedSessionInput`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L6831-L6882) carries those limits. The event/effect vocabulary has no preparation completion, while [`StartAttempt` already requires a `PreparedCompilation`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L6927-L6991).

A conforming implementation must therefore invent hidden limits, prepare outside the reducer, or add an undeclared transition. None can publish the accepted pre-attempt request-rejection branch without changing the interface.

### Operational Reports Require Facts Their Inputs Cannot Supply

The [Attempt Operational Inventory](https://github.com/sagikazarmark/typst-pack/blob/a482145a0c790bb67d7f6d4424777519c614876e/PROTOTYPE-compilation-family-identity-registry.md#L1066-L1087) requires authority classes, cache and offline policy, requested and admitted limits, `D`, `K`, `Q`, Engine Runtime Domain identity and width `W`, isolation, worker capacity `P`, interruption, and reporting state. [Define engine parallelism and adapter profile ownership](https://github.com/sagikazarmark/typst-pack/issues/55) additionally requires every managed report to retain requested and admitted `W`.

The Rust output views promise these facts, but their inputs do not:

- [`AuthorityCapabilities`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L2537-L2543) has no sanitized authority class.
- [`SyncCompilationControls` and `AsyncCompilationControls`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5065-L5159) have no complete cache/offline or requested-domain-width descriptor.
- [`CompilationExecutionFacilityCapacity`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L5523-L5537) has only `K/Q`, while the report view promises an independently owned `P`.
- The strict adapter schema inserts requested/admitted `adapter_jobs` inside the Creation Report but the [core Creation Execution Inventory](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L3581-L3620) cannot expose it. Compilation JSON drops the requested value entirely.

Adapters cannot derive these values from concrete Rust type names or observed success without inventing operational state.

### Bounded Builders And Creation Rejections Are Incomplete

Authority-produced Complete Package Trees and Font Catalogs receive an [`AcquisitionBudget`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L2545-L2596), but that budget can reserve only four byte dimensions. It cannot enforce package-file count, largest package file, font-candidate count, or font-face count at their first collection seam even though the public creation limits contain the corresponding independent dimensions.

`CreationRequestRejection` is returned by public constructors before creation can produce a report, but it is [opaque and has no typed public projection](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L3664-L3700). A CLI, Dagger adapter, or service cannot serialize its deterministic issues without parsing debug text or inventing semantics.

### Representation Admission And Receipt Facts Do Not Compose

Representation Receipt Contract version 1 requires exact requested and admitted controls, enforcement facts, publication and cleanup strengths, and reached stages. Its admission-refusal rule retains facts established before the reached stage. The writer corpus specifically requires refusal of an unregistered encode recipe and an unsupported asserted read recipe before effects or interpretation.

The accepted interfaces cannot represent those cases truthfully:

- [`TransportReceiptAdmissionView`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L823-L844) exposes requested but not admitted publication/cleanup strengths, offline policy, or enforcement facts, and its stage registry omits reference resolution, acquisition, spooling, and verification.
- [`TransportControls`](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L1152-L1209) cannot supply those missing facts to the public receipt constructors.
- The Format Receipt has only `not asserted`, `externally asserted and byte-verified`, and `externally asserted and mismatched` states for archive reads. The strict [archive-read JSON schema](https://github.com/sagikazarmark/typst-pack/blob/43f2af9ca91c3d884a5fa1103c4cb83bb4a35bf8/PROTOTYPE-first-party-cli-dagger-schemas.json#L788-L820) therefore cannot serialize [the unsupported asserted-recipe case](https://github.com/sagikazarmark/typst-pack/blob/23d8bea991c0d55f3aeaee4cd3137c67c8d9d496/PROTOTYPE-epoch-2-writer-interoperability-corpus.md#L398-L406) without either erasing the supplied identity or falsely claiming comparison.
- The encode schema clears the selected recipe identity on admission refusal, including the corpus's unregistered-recipe refusal, although the selection is known before effects.

The accepted receipt contract does not yet decide whether a supplied but unevaluated recipe identity belongs in the Format Receipt, the outer operation inventory, or both. That is a semantic reporting decision, not schema-only implementation work.

### The Adapter Watch Model Is Not The Rust Session Model

The interactive lifecycle artifact executes a different state machine from the frozen Rust interface:

- It treats a request rejection as `ATTEMPT_FINISHED`, while the Rust event can finish only with a `CompilationReport`; request rejection exists before a Prepared Compilation and attempt.
- It gives ingestion failure an attempt token, while the Rust `IngestionFailed` event has none.
- It feeds `PUBLISH` and `RETIRE_SUBSCRIPTIONS` back as events and invents `RECONCILED_EQUAL`; the Rust contract defines the first two as effects and has no event corresponding to the third.
- Accepting a second revision while an attempt is active overwrites the active attempt token without retaining one active plus one latest pending revision or emitting `InterruptAttempt`.

The model is explicitly non-wire, but it is the accepted concrete evidence for how watch behavior composes. It cannot guide implementation against the accepted reducer until regenerated from the same event/effect vocabulary.

### Request Inventory Has Remaining Lossless-Projection Defects

The registry requires origins on every accepted scalar or collection member, per-field diagnostic-policy origins, and an optional safe-node reference on each invalid-declaration marker. The [Rust inventory projection](https://github.com/sagikazarmark/typst-pack/blob/c78a6c11e16fddf8830fb52cd7152e293c0587c9/PROTOTYPE-rust-lifecycle-adapter-interfaces.rs#L4267-L4358) omits Engine and Exporter origins, collapses diagnostic-policy origins, and omits the marker's safe-node reference. A strict adapter cannot serialize the registry exactly without reconstruction rules outside the core.

## Deterministic Corrections

These defects follow mechanically from already accepted contracts and do not need separate decision tickets, but the corrected artifacts must include them:

- Align `PdfStandard` ordering or remove its public derived `Ord`; the current enum order differs from the frozen numeric identity order.
- Give `ArchiveEncodingIdentity` typed equality like every other known identity wrapper.
- Remove adapter prose that still says override limits and lossless Pack Inspection are absent from the corrected Rust fixture.
- Keep first-party requested/admitted `jobs` reporting adapter-owned and represent it consistently for both creation and compilation.
- Preserve validation commands, tool versions, fixtures, and executable assertions so schema, GraphQL, profile, lifecycle, and compile claims are independently reproducible from the accepted commit.

## Corrective Route

| Decision ticket | Question owned | Blocking |
| --- | --- | --- |
| **Define session preparation and pre-attempt terminal semantics** | How exact preparation limits enter a Pack-bound reducer, how preparation returns either a Prepared Compilation or request rejection, and how ingestion failure, active/pending revisions, interruption, and publication tokens compose | Unblocked |
| **Freeze operational capability and execution-report inputs** | Which explicit capability descriptors supply authority class, cache/offline policy, requested/admitted `D/K/Q/W/P`, isolation, profile, interruption, and reporting facts without inference | Unblocked |
| **Define pre-admission representation and transport receipt semantics** | Where selected or asserted unsupported recipe identities live, which status means supplied but unevaluated, and how requested/admitted transport strengths, stages, offline policy, and enforcement facts compose | Unblocked |
| **Reconcile Rust lifecycle implementability** | What corrected Rust 1.92 fixture and consumer close bounded builder, rejection, inventory, session, execution, and receipt reachability while preserving seven deep modules | Blocked by the three decisions above |
| **Regenerate first-party adapters and watch semantics** | What strict schemas, GraphQL, profiles, lifecycle model, and replayable validation harness serialize only facts reachable through the corrected Rust and format contracts | Blocked by **Reconcile Rust lifecycle implementability** |
| **Approve the corrected implementation-planning specification** | What immutable connective specification and precedence ledger assemble the accepted map and corrected artifacts without contradiction or omission | Blocked by **Regenerate first-party adapters and watch semantics** |

The first three decisions can run in parallel. The route does not reopen the destination, the Pack product model, the compilation-family identities, the all-Stored writer choice, the seven-module architecture, or the clean-break migration direction.

## Planning And Release Gates

The following remain implementation or prerelease work rather than new wayfinding decisions:

- materialize the Epoch 2 and compilation-family corpora and obtain independent decoder or derivation agreement;
- implement runtime semantics behind the frozen interfaces and verify limit, cancellation, race, and receipt behavior;
- run actual Dagger generation, structural comparison, and generated-client compilation;
- inventory the `v0.3.1` baseline, execute the coordinated `0.4.0` clean break, and write migration material;
- tune benchmarks below admitted ceilings and gather platform/confinement evidence; and
- execute packaging, alpha, beta, release-candidate, stable, and rollback gates.

No implementation ticket should treat the current Rust and adapter prototypes as a jointly complete contract before the corrective route closes.

## Recommendation

Reject [Approve the revised implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/62), retain the accepted direction, and chart the six-ticket corrective route above. No in-scope fog remains: every newly exposed question is precise enough to be a ticket.
