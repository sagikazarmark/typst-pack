# Architecture review — August 2026

Reviewed at commit 3e58ca6 (v0.4.0, 0.5.0 in preparation). Line counts are
measured; estimates are accurate to ±10–15%. This document records findings
and intent only — it is deliberately not a normative spec, contains no type
declarations, and requires no ratification. Issues implementing it link here
by step number.

## Verdict

The domain core is good and must be protected: `Pack::construct` whole-pack
validation, the hand-rolled zip-safety layer, the hermetic `PackWorld`, the
resumable creation protocol, the feature-graph egress isolation, and the
structural project closure. The over-engineering is in the implementation
style around that core: per-operation error/evidence vocabularies, limit
families copied per operation, adapter hierarchies duplicated by an absolute
anti-abstraction rule, and a spec-first process that produced specifications
larger than the code they specify.

Realistic target: half to two-thirds of the current codebase, with no loss of
capability.

## Key measurements

| Measure | Value |
| --- | --- |
| Workspace total | 73,310 lines of Rust |
| Names exported from lib.rs | ~195 (a typical user needs ~12) |
| Error types crate-wide | ~120 (92 in the adapters; ~6 per callable operation) |
| Hand-written Debug/Display/Error impls (OpenDAL) | 85 (~1,800 lines; caused by generic `E` on error types) |
| `*Limits` families | 13 structs / ~39 types; 25 `reference_v1()` profiles; zero custom-limit call sites in the repo |
| OpenDAL module | ~10,400 production lines for ~450 lines of actual I/O (~77% ceremony/duplication/dead) |
| Dead code | `src/opendal/compilation/` subtree: 2,687 production lines (4,563 with tests); its documented compilation-read entry point does not exist |
| compile.rs | 3,742 lines; ~2 lines of identity/inventory/fulfillment/report machinery per line of compilation |
| Library tests | 27,238 lines; ~34–43% ceremony pinning (mock self-tests, CI-YAML/Cargo.toml text asserts, pointer-equality tests) |
| Glossary | CONTEXT.md defines ~60 capitalized terms in 490 lines |

Other load-bearing findings:

- The CLI — the only first-party consumer — never reads `CompilationIdentity`,
  `CompilationResultIdentity`, `CompilationRequestInventory`,
  `RequestValueOrigin`, `CompilationAccessTrace`, or the fulfillment-report
  types.
- `Pack::identity()` rehashes every project file and is triggered five times
  per `compile()` call.
- Success-path commit-certainty accessors on former filesystem write receipts
  et al.) return compile-time constants.
- Two identically named `workflow_evidence!` macros generate two incompatible
  Pack Extraction write-receipt types, distinguished only by module path.
- The live OpenDAL read path is sequential; the GAT-based scheduling protocol
  that would provide bounded fan-out has no production caller.
- The wasm guarantee is a featureless `cargo build` only; the `opendal`
  feature is claimed wasm-compatible but is not part of the wasm CI check.

## Root causes

1. **ADR-0013** — per-operation error vocabulary with no shared vocabulary,
   plus generic `E` on adapter errors, produces the ~120-type error surface
   and the manual impl burden.
2. **ADR-0012** — the (correct) bounding principle executed as copied limit
   families rather than one parameterized family.
3. **ADR-0014 + ADR-0015** — an absolute anti-abstraction rule forces
   parallel fs/OpenDAL type hierarchies with verbatim-duplicated error
   strings; combined with a spec-first process (a 2,679-line normative spec
   with its own ratification ticket), no review pass ever asked whether the
   transcribed types needed to exist.

## Terminology direction

Target: CONTEXT.md from ~60 terms to ~25.

- Delete the "Lifecycle" meta-vocabulary as glossary entries (coding
  convention, not domain).
- Demote write receipt / progress / commit certainty to a field on the
  error type.
- Unify verbs across adapters as one read/write pair, with no adapter-specific
  synonym table.
- Merge Engine Identity and Exporter Identity into one implementation
  identity with a role; one shared canonical-identity newtype instead of ~32
  constant accessors across 8 types.
- Request Inventory / Commitment / Access Trace / Request Value Origin have
  no consumers: remove, or reduce to a single `inputs_commitment()` if a
  concrete need exists.
- Keep: Dependency Discovery, Pack Assembly vs. Pack Creation, Document
  Format / Page Format, Source Page Number.

## 0.5 scope cut (decisions on the existing backlog)

- **#212**: drop the reservation scheduler; ship per-role APIs (done) or a
  thin composite with plain bounded concurrency + byte budget. Settles the
  fate of the dead `opendal/compilation/` subtree.
- **#219 + #220**: one MinIO suite carrying only what Memory cannot prove
  (pagination, real listing, authorization errors, `If-None-Match: *`).
- **#226 + #227 + #206**: one docs ticket (three tickets currently write one
  document).
- **#228**: keep tri-platform CI and MSRV; drop OpenDAL-specific fuzz targets
  and the per-operation `Send`-assert matrix.
- **Keep unconditionally**: #213's `If-None-Match` contract test, #204 parity
  proof, #205's dependency half (caret-vs-pin), #224.

Result: backlog 13 → ~5 issues, critical path 7 → 3 deep.

## Action plan

| Step | Work | Impact | When |
| --- | --- | --- | --- |
| 1 | 0.5 scope decisions above; delete or gate the dead `opendal/compilation/` subtree | backlog 13→5, −4,500 lines | before 0.5 |
| 2 | De-genericize errors: `Box<dyn Error + Send + Sync>` at the adapter boundary, `thiserror` everywhere; remove remap-only internal error types | −3,000+ lines, 85 manual impls | 0.5/0.6 |
| 3 | One `Limits<R>`/`LimitError<R>` family with per-operation presets on both adapters; defaultable limits parameters | ~39 types → 3 | 0.6 |
| 4 | One shared receipt/progress vocabulary; merge the `workflow_evidence!` macros; Commit Certainty becomes an error field | −2,000 lines | 0.6 |
| 5 | Core slimming: delete provenance layer; merge Engine/Exporter identity; fold inventory types into digest computation; cache `PackIdentity`; split compile.rs; shared `paths.rs` | −700+ lines, −10+ exported types | 0.6 |
| 6 | Test diet: drop mock self-tests, CI-text asserts, pointer tests; keep conformance corpus, differential oracle, fuzzing | −2,400 lines | ongoing |
| 7 | ADR revision: make ADR-0012/0013/0014/0015 graduated ("shared data shapes allowed, shared behavioral traits not"); add code review to the spec-first process | curbs future growth | before steps 3–4 |
| 8 | Library helpers from CLI pain points: override-path preflight, font-requirement resolution, write-path matching | −200 lines CLI ceremony | as needed |
| 9 | Glossary diet per the terminology direction; rewrite README "Features" in user language | readability | with 0.6 |

Steps 3 and 4 are blocked by step 7: as written, ADR-0014/0015 forbid the
shared shapes those steps introduce.

## Terminology outcome

Issue #240 selected **read/write** for every storage adapter. Filesystem and
OpenDAL functions, modules, errors, limits, progress, and receipts now use that
pair; the old adapter-specific verb split has no compatibility aliases. Pack
Archive convenience names that already used `read_pack`, `write_pack`,
`open_pack`, and `save_pack` remain unchanged.
