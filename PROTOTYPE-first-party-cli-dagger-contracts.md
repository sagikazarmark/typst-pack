# PROTOTYPE: Final Rust And First-Party Adapter Contract Regeneration

> Throwaway design artifact for
> [Regenerate the final Rust and first-party adapter contracts](https://github.com/sagikazarmark/typst-pack/issues/78).
> It is an implementation-planning contract, not production code and not a
> compatibility promise for the current 0.3 surfaces.
> Under the standing premise that no external or persisted v1 consumer exists,
> this final prototype corrects the issue-69 adapter baseline in place under
> [Define aggregate creation and representation resource accounting](https://github.com/sagikazarmark/typst-pack/issues/76)
> and [Reconcile residual lifecycle and receipt semantics](https://github.com/sagikazarmark/typst-pack/issues/77).
> The strict adapter contract and every adapter schema remain major/version 1;
> there are no compatibility aliases, legacy branches, or dual serializers.

Companion planning fixtures:

- [`PROTOTYPE-first-party-cli-dagger-generated.graphql`](./PROTOTYPE-first-party-cli-dagger-generated.graphql)
  declares the generated-surface target; it is not evidence that the actual
  implementation currently generates it.
- [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json)
  declares the strict schema target to validate against this corrected contract.
- [`PROTOTYPE-native-cli-profile.json`](./PROTOTYPE-native-cli-profile.json) and
  [`PROTOTYPE-dagger-ci-profile.json`](./PROTOTYPE-dagger-ci-profile.json) freeze
  every first-party numeric default.
- [`PROTOTYPE-first-party-cli-dagger-contracts.html`](./PROTOTYPE-first-party-cli-dagger-contracts.html)
  is a throwaway trace viewer over the final Rust session vocabulary. It renders
  checked immutable scenarios and deliberately implements no second reducer.
- [`PROTOTYPE-first-party-cli-dagger-validation`](./PROTOTYPE-first-party-cli-dagger-validation)
  is the locked replayable validation harness. Its Cargo targets contain compile
  probes for the final Rust fixture, prior consumer, mechanically linked
  serializer-source markers, and validator.

## Question

What regenerated Rust 1.92 lifecycle fixture, strict JSON schemas, desired
GraphQL structure, first-party profiles, serializer probe, watch model, and
locked replay harness make every accepted lifecycle, receipt, terminal,
resource, and publication-fence state constructible, inspectable, lossless, and
cross-artifact coherent without adapter-invented facts?

## Verdict

Freeze two first-party adapters over one lifecycle:

- The CLI has `create`, `inspect`, `compile`, `watch`, `materialize`, and
  `convert`. Only `compile` and `watch` have visible aliases `c` and `w`.
- Compile spellings follow embedded Typst 0.15.0 where values mean the same
  thing. Pack identity, Pack Overrides, separate times, trust, limits, reports,
  representation, and publication are explicit typst-pack differences.
- Dagger exposes immutable lifecycle objects over `File` and `Directory`. It
  has no watch, terminal, stdout, viewer, host-path, or Hostile facade.
- Both adapters emit the same versioned projection families. These are adapter
  documents, not serialized Rust values and not Pack Format epochs.
- Every wire leaf is auditable to one Rust accessor or discriminated branch, one
  explicit adapter input, one closed adapter derivation declared by this
  contract, or one schema constant. Profiles, concrete types, placement labels,
  and success status are never substitute evidence for a requested, admitted,
  selected, or reached fact.
- Expected failures remain queryable. Only Dagger `requirePack`,
  `requireArchive`, `requireTree`, `requireProject`, and `requireSuccess`
  deliberately raise for modeled outcomes.
- Creation request-construction failure is the outer typed
  `CreationRequestRejection`; it precedes operation admission and can never be
  represented as a `CreationReport`.
- Compilation Request Rejection has no fabricated Compilation Report,
  Compilation Identity, dependency evidence, or access trace. Post-commit work
  retains and never reclassifies an immutable report.
- Pure compilation preparation runs before operational admission. Request
  Rejection therefore wins over unavailable jobs or facilities; only a Prepared
  Compilation can yield a reportless Admission Refusal or an admitted attempt.
- Requests, immutable subjects, exact bound descriptors, successful admission,
  and sealed reached ledgers are the five non-interchangeable fact sources.
  Role-specific capability scope, execution placement, isolation, and Engine
  Runtime Domain selection are serialized independently.
- Creation and Pack resource accounting separates category and aggregate
  logical dimensions from physical representation and peak occupancy. Archive,
  Closure Export, and materialization limits are role-specific rather than one
  representation bag.
- Epoch 2 writing uses the registered `org.typst-pack.archive.all-stored`
  recipe, exposed as `epoch-2-all-stored-v1` and
  `EPOCH_2_ALL_STORED_V1`. It is the only first-release writer. Stored,
  Deflate, and mixed-method archives remain required reader inputs, but no
  Deflate writer recipe is registered.

## Frozen Baselines

| Subject | Contract |
| --- | --- |
| CLI parity | Typst CLI 0.15.0 |
| Pack writer | Pack Format Epoch 2 |
| Archive writer recipe | `org.typst-pack.archive.all-stored`, recipe epoch 1 |
| Native adapter profile | `native-cli/1` |
| Dagger adapter profile | `dagger-ci/1` |
| Stable JSON major | 1 |
| Trust default | Partially Trusted |
| Semantic Result Cache | disabled; no first-party flag |
| Canonical Diagnostic Policy | 20,000 entries and 64 MiB canonical entry bytes |

None means "latest". Upgrades deliberately update snapshots, schemas, and
generated-client probes. Because this contract is unreleased, all corrected
version-1 shapes replace their prior prototype definitions in place. A producer
emits only the corrected spelling and branch; a consumer accepts no old alias.

## Serializer Source Rule

Each serialized leaf must have exactly one of these sources:

1. A value returned by the corresponding regenerated final-Rust accessor or
   discriminated branch in the companion fixture.
2. An explicit adapter input retained without semantic reinterpretation, such
   as the lexical jobs class, a destination spelling, or a presentation option.
3. A closed adapter derivation defined in this document, such as mapping one
   Rust enum variant to its lower-snake-case wire token or computing a CLI exit
   status from the complete set of requested operation outcomes.
4. A constant fixed by adapter schema major 1, such as the schema name,
   producer discriminator, capability class, or contract version.

A serializer must not recover a missing leaf from a resource profile, Rust or
GraphQL concrete type, requested placement, facility class, terminal success,
or neighboring value. In particular, profiles supply requested policy ceilings
to admission; they never manufacture admitted or reached report values. A
closed derivation operates only on its named source branch and must serialize
`not_applicable`, `not_reached`, or null when that branch says so rather than
guessing a value.

### Bound Capability Classes

Every executable authority, evidence provider, Engine Runtime Domain policy,
execution facility, spool, and transport adapter owns its immutable
role-specific descriptor. The adapter obtains the descriptor from that exact
object, serializes descriptor version 1 and its safe capability projection, and
then serializes the separate operation request, admission, and reached facts.
The following Operational Capability Class strings are exact constants:

| Role | Native CLI | Dagger CI |
| --- | --- | --- |
| Creation Evidence | `org.typst-pack/native-cli/creation-evidence/1` | `org.typst-pack/dagger/creation-evidence/1` |
| Package Authority | `org.typst-pack/native-cli/package-authority/1` | `org.typst-pack/dagger/package-authority/1` |
| Font Authority | `org.typst-pack/native-cli/font-authority/1` | `org.typst-pack/dagger/font-authority/1` |
| Authority-private content cache, when present | `org.typst-pack/native-cli/authority-content-cache/1` | `org.typst-pack/dagger/authority-content-cache/1` |
| Engine Runtime Domain policy | `org.typst-pack/native-cli/engine-domain/1` | `org.typst-pack/dagger/engine-domain/1` |
| Engine Runtime Domain sharing scope | `org.typst-pack/native-cli/engine-runtime-sharing-scope/1` | `org.typst-pack/dagger/engine-runtime-sharing-scope/1` |
| Shared execution capacity scope | `org.typst-pack/native-cli/shared-execution-capacity/1` | `org.typst-pack/dagger/shared-execution-capacity/1` |
| Creation Execution Facility | `org.typst-pack/native-cli/creation-facility/1` | `org.typst-pack/dagger/creation-facility/1` |
| Compilation Execution Facility | `org.typst-pack/native-cli/compilation-facility/1` | `org.typst-pack/dagger/compilation-facility/1` |
| Reporting Facility | `org.typst-pack/native-cli/reporting/1` | `org.typst-pack/dagger/reporting/1` |
| Ready-job worker protocol, when present | `org.typst-pack/native-cli/ready-job-worker/1` | `org.typst-pack/dagger/ready-job-worker/1` |
| Spool Facility | `org.typst-pack/native-cli/spool-facility/1` | `org.typst-pack/dagger/spool-facility/1` |
| Pack Archive Acquirer | `org.typst-pack/native-cli/pack-archive-acquirer/1` | `org.typst-pack/dagger/pack-archive-acquirer/1` |
| Pack Archive Publisher | `org.typst-pack/native-cli/pack-archive-publisher/1` | `org.typst-pack/dagger/pack-archive-publisher/1` |
| Project Materialization Publisher | `org.typst-pack/native-cli/project-materialization-publisher/1` | `org.typst-pack/dagger/project-materialization-publisher/1` |
| Closure Export Publisher | `org.typst-pack/native-cli/closure-export-publisher/1` | `org.typst-pack/dagger/closure-export-publisher/1` |
| Compilation Delivery | `org.typst-pack/native-cli/compilation-delivery/1` | `org.typst-pack/dagger/compilation-delivery/1` |

The eight capability constants inherited from issue 71 are Creation Evidence,
Package Authority, Font Authority, Authority-private content cache, Engine
Runtime Domain policy, Creation Execution Facility, Compilation Execution
Facility, and Ready-job worker protocol. This contract preserves those strings
byte-for-byte. The two sharing-scope constants and the six transport-role
constants are frozen here under the same grammar; their names are constants, not
evidence of a reached capability.

Semantic Result Cache is disabled coherently at every first-party compilation
seam. `CompilationOperationRequest.cache` is `Disabled`; operational admission
binds the disabled cache lookup branch and no cache object or descriptor; an
admission refusal therefore also exposes no cache descriptor. An admitted
report's dependency-execution inventory has `cache_descriptor: null`,
`cache_policy: disabled`, `cache_lookup: disabled`, and
`cache_isolation_domain_present: false`, while report cache provenance is
`disabled`. No serializer may infer a descriptor, hit, isolation domain, lookup,
or admission from Dagger graph reuse, an authority-private content cache, a
concrete no-cache helper type, or successful compilation.

### Operational Inventories

Every admitted Creation Report and Compilation Report serializes all six
role-specific sections below. The two report families have distinct wire types;
there is no generic operational inventory and no `kernel_execution` alias for
the final `role_execution` section.

| Section | Creation source | Compilation source |
| --- | --- | --- |
| `admission` | requested/admitted trust, network, enforcement, role-specific capability scopes, placement, and isolation | the same compilation-specific facts; reached state never lives in admission |
| `resources` | profile plus requested/admitted limits and sealed reached category, aggregate, and peak-occupancy totals | profile plus requested and admitted `CompilationResourceLimits` |
| `dependency_execution` | bound Creation Evidence, Package Authority, and Font Authority descriptors; requested/admitted/reached capability scopes; Font Scan Policy; offline roles; requested/admitted `D` | bound Package/Font descriptors; requested/admitted/reached scopes; optional cache descriptor, policy, lookup, isolation-domain presence, offline roles, and requested/admitted `D` |
| `attempt_control` | deadline, cancellation presence, monotonic domain, queue timeout, latency target, requested/admitted interruption, and reached winner | the same plus explicit session-supersession applicability |
| `role_execution` | exact caller-thread or facility branch, bound facility/domain descriptors, `K/Q/W/P` request/admission/applicability, domain selection, and reached queue/dispatch/termination/reap facts | the corresponding compilation-specific branch and types |
| `reporting` | requested/admitted timing policies and reached channel/lease states | requested/admitted diagnostic, source, timing, and fine-timing policies plus reached channel/lease states |

The dimension vocabulary is fixed: dependency acquisition concurrency `D`,
ready-job concurrency `K`, ready queue `Q`, Engine Runtime Domain width `W`,
isolated worker capacity `P`, and transport concurrency `T`. `D` comes only from
the operation request and `DependencyConcurrencyAdmission`; `K/Q/P` come only
from the execution request, the bound role descriptor, and the role-specific
capacity admission; `W` comes only from `EngineWidthRequest`,
`EngineWidthAdmission`, and the reached domain-selection branch; `T` comes only
from the transport request, bound transport descriptor, transport admission,
and stage ledger. A caller-thread branch explicitly makes `K/Q/P` not
applicable. Native in-process facility execution makes `P` not applicable;
for native creation specifically, `P` is applicable only when isolated creation
was selected and is otherwise not applicable. The same rule applies to native
compilation. Dagger always requests its managed worker topology, so its actual
creation or compilation facility admission carries `P`. `T` is not a Creation
or Compilation Report fact and is serialized only in the separate transport
receipt. Reached state is a separate branch or marker from the final Rust view:
for example, admitted `T` does not mean transfer was reached, and a reached
transfer stage does not invent a measured peak concurrency. None of these facts
is copied from the profile.

Capability grants expose closed role/use/coverage projections with complete or
redacted status; no credential, root, locator, concrete Rust type, facility
instance, or extension bag enters ordinary reports. Requested and admitted
execution placement use `caller_thread`, `in_process_facility`, or
`worker_facility`; isolation is a separate explicit contract. Reached placement
or worker setup does not imply a selected Engine Runtime Domain. Cache hits,
dependency failures, queue refusal, cancellation before dispatch, and worker
setup failure before assignment serialize domain `not_selected`; only actual
unmanaged or managed execution selects another branch.

### Resource Accounting

Creation atomically debits category limits together with `F_create`, the number
of Project Snapshot and `(Package Specification, Package Path)` bindings, and
`L_create`, their exact bytes plus each selected Font Container once. Discovery
override bytes remain on their separate override budgets. Issued and ingressed
Packs report logical `F_pack/L_pack` independently from physical `C/B/P/N`:
control-record bytes, deduplicated blob count and bytes, and representation entry
count `N = B + 1`. Equal logical bindings charge repeatedly; physical blobs
deduplicate only by full typed Content Identity.

Archive, Closure Export, and Project Materialization limits are separate wire
objects. Epoch 2 all-Stored encoding computes checked local records `R`, central
directory `D`, ZIP64 trailer `Z`, and exact archive bytes `A` before effects.
Under first-party ceilings:

```text
A = C + P + 138 + 252B + (76 when N >= 65,535, otherwise 0)
```

Stable spool and retained memory are operation-private peak-live occupancy, not
cumulative content counters. Every successful first-party Archive or Closure
Export preflights the same unmodified profile's logical, physical, largest,
entry, exact-output, and expansion ingress ceilings.

| Dimension | `native-cli/1` | `dagger-ci/1` |
| --- | ---: | ---: |
| `F_create`, `F_pack` | 100,000 | 100,000 |
| `L_create`, `L_pack`, `P` | 4 GiB | 8 GiB |
| `C` | 64 MiB | 64 MiB |
| `N`, transport objects | 200,000 | 200,000 |
| Largest blob | 512 MiB | 512 MiB |
| Archive `A` | 1 GiB | 2 GiB |
| Closure payload `C + P` | 4,362,076,160 | 8,657,043,456 |
| Materialization `M/J` | 100,000 / 8 GiB | 100,000 / 16 GiB |
| Peak spool / retained memory | 16 GiB / 4 GiB | 32 GiB / 8 GiB |

## CLI Grammar

```text
typst-pack create [OPTIONS] <INPUT> [OUTPUT]
typst-pack inspect [OPTIONS] <PACK>
typst-pack compile [OPTIONS] <PACK> [OUTPUT]
typst-pack watch [OPTIONS] <PACK> [OUTPUT]
typst-pack materialize [OPTIONS] <PACK> <DESTINATION>
typst-pack convert [OPTIONS] <PACK> <OUTPUT> --to <archive|closure>
```

There is no `validate`, `extract`, Resource Slot, Resource Provider, unchecked
Pack builder, epoch-1 compatibility, generic store, or generic transport
command. Every ingress fully validates before exposing a Pack.

### Global Options

| Option | Default | Contract |
| --- | --- | --- |
| `--color[=<auto|always|never>]` | `auto`; bare flag is `always` | Human stderr framing only. |
| `--cert <PATH>` | `TYPST_CERT`, then absent | Network package authority for create/compile/watch; invalid elsewhere. |
| `--trust <trusted|partially-trusted|hostile>` | `partially-trusted` | Applies to every composed operation. Hostile parses and is refused before content interpretation. |
| `--limits <FILE>` | adapter profile | Strict `org.typst-pack.operation-limits/1` tightening overlay. |
| `-h`, `--help`; `-V`, `--version` | - | Exit 0 without content work. Version prints typst-pack and Typst versions. |

Global options work before or after the subcommand. The adapter parses CLI or
GraphQL shape and the trust token first. It refuses Hostile before opening any
path, reading bytes, initializing an engine domain, or sampling a clock. Only
then may it open and validate `--limits`; no other locator is touched first.
An outer Ordinary Admission refusal is adapter-owned: the operation is
`adapter_failed` and has no Format Receipt. The nested Format Receipt
`admission_refused` terminal is reserved for a later role-level refusal after
outer admission already succeeded.

`DURATION` is `none` or one unsigned integer followed by `ms`, `s`, `m`, or
`h`. Fractions, signs, whitespace, and compounds reject. Poll intervals exclude
`none` and zero.

`UNIX_SECONDS` follows Typst 0.15.0's ordinary Rust `i64` lexical acceptance:
ASCII `[+-]?[0-9]+`, including a leading `+` and leading zeroes, but no
whitespace. Admission normalizes it to the canonical signed decimal string
(`+000` and `-0` become `0`) before semantic preparation. It must fit `i64` and
the engine datetime range. Lexical or `i64` failure is usage exit 2;
engine-range or semantic combination failure is request rejection exit 1. JSON
uses only the normalized form.

### Pack Input And Expectations

Every `<PACK>` command accepts:

```text
--expect-pack-id <CANONICAL_ID>
--expect-content-id <CANONICAL_ID>
--expect-archive-encoding-id <CANONICAL_ID>
--ingress-deadline <DURATION> # default: none
```

| Input | Kind |
| --- | --- |
| Regular file | Pack Archive |
| Directory | Closure Export |
| `-` | Pack Archive stream, boundedly spooled before parsing |
| Anything else or changing node | adapter ingestion failure |

Input kind is selected once; no parser fallback occurs. `--expect-content-id`
means expected complete Pack Archive Content Identity and is archive-only. A
Closure Export accepts only expected Pack Identity; its Tree Content Identity is
a derived representation fact and has no expectation or mismatch terminal in
contract v1. Archive Encoding expectation is also archive-only and requires
support for that exact recipe, full Pack validation, exact re-encoding, and byte
comparison. Generic ingress never infers a recipe. A well-formed but unsupported
asserted recipe is a typed Representation Admission Refusal before effects; the
Pack Archive Read Format Receipt still retains the supplied assertion and marks
it `supplied_but_unevaluated`. If another earlier admitted terminal prevents the
exact comparison, that same status remains reportable. Only a reached exact
comparison may report externally asserted and byte-verified or byte-mismatched.
Malformed expectation construction is a typed outer input rejection and creates
no Format Receipt.

Closure Export wrapper file paths are exactly `typst-pack/pack.cbor` and
`typst-pack/blobs/sha256/<64 lowercase hex>`. Bare `pack.cbor`, bare `blobs/`,
logical project paths, aliases, and any other wrapper path are invalid.

Pack Archive ingress carries a separate `pack_archive_acquisition` Transport
Receipt for bounded stabilization before the role-refined `pack_archive_read` Format
Receipt. Acquisition has null requested and actual commit because it produces an
immutable input value rather than publishing one. Closure Export is already a
finite-tree input and has no Pack Archive acquisition receipt; its format receipt
records the canonical tree identity directly. Input Content Identity mismatch
therefore records only expected and actual archive identities. Archive
acquisition may itself be `failed` at reference, acquisition, spooling, or
cleanup; resource, cancellation, deadline, and integrity ingress terminals
permit either that failed receipt or a completed transfer followed by a later
format failure. Framing, validation, and identity terminals require acquisition
`transferred`. A Transport Receipt is never fabricated for a role that was not
attempted.

`org.typst-pack.pack-ingress-operation/1` is the common CLI and Dagger operation
envelope. `kind: adapter_failed` owns a non-null adapter outcome and no Format
Receipt; `kind: format_terminal` owns the role-specific Pack Archive Read or
Closure Export Import Format Receipt. Archive acquisition remains a sibling
Transport Receipt. The Dagger `receipt` field names this complete operation
envelope, not the nested Format Receipt alone. Within a Format Receipt,
`admission` is a closed `refused` or `admitted` branch. Refusal retains complete
requested controls and its reason, with no admitted or reached facts; admission
retains both requested and admitted controls and all independently reached
representation facts.

No expected Pack Identity means Derive. Human and machine results show the
derived identity and `internal_validity_only`; they never imply substitution
protection. A supplied identity means Verify. No mismatch exposes a Pack.

### Shared Compilation Surface

`compile` and `watch` share every option below. Watch adds only session controls
and target-specific publication rules.

#### Semantic Request

| Option | Default | Contract |
| --- | --- | --- |
| `--input <KEY=VALUE>` | empty | Repeatable. Split first `=`, trim; empty key rejects; later duplicate wins as Typst 0.15.0. |
| `--features <FEATURES>` | `TYPST_FEATURES`, then empty | Repeatable comma set of canonical Feature Identifiers. Typst 0.15.0 recognizes `html` and `a11y-extras`; `bundle` and other unsupported identifiers remain representable and become typed request rejection. |
| `--document-time <absent|UNIX_SECONDS>` | command-start wall clock | Exact Compilation Document Time. |
| `--pdf-creation-time <omit|UNIX_SECONDS>` | command-start wall clock for PDF | PDF-only exact value. |
| `--creation-timestamp <UNIX_SECONDS>` | `SOURCE_DATE_EPOCH`, then absent | Convenience sets document time and PDF creation time; conflicts with either advanced option. |
| `--override <PACK_PATH=FILE>` | empty | Repeatable stabilized replacement; duplicate canonical paths reject. |
| `--override-file <FILE>` | empty | Repeatable strict `org.typst-pack.override-set/1`; supports Pack paths containing `=`. |
| `--diagnostic-policy <FILE>` | profile semantic default | Strict `org.typst-pack.canonical-diagnostic-policy/1`; identity-bearing; each dimension must be no greater than the selected profile. |

Absent time controls make CLI sample one Unix second after admission and before
Pack ingress. That value is used for document and PDF creation time and is
reported as adapter-resolved. PDF uses UTC, intentionally avoiding Typst's
ambient local-offset fallback. Watch freezes the value for its whole process.
Dagger instead defaults document time to Absent and PDF time to Omitted.

Flat override syntax splits the first `=`; Pack paths containing `=` use a file.
Sources are named regular files. The complete set is stabilized before
preparation. Stdin, directories, links, additions, deletions, unknown Pack paths,
duplicates across sources, and changing files reject.

#### Tagged Output

| Option | Applies | Default |
| --- | --- | --- |
| `-f`, `--format <pdf|png|svg|html>` | all | infer known supplied extension; omitted output is PDF; unknown/extensionless supplied path requires flag |
| `--pages <RANGES>` | PDF, PNG, SVG | all |
| `--pretty` | PDF, SVG, HTML | false |
| `--ppi <FINITE_POSITIVE_DECIMAL>` | PNG | 144 |
| `--bleed` | PNG, SVG | false |
| `--pdf-standard <STANDARD>` | PDF | empty repeatable comma set |
| `--pdf-tags`; `--no-pdf-tags` | PDF | automatic; flags conflict |
| `--pdf-identifier <auto|omit|custom:TEXT>` | PDF | auto |
| `--pdf-creator <auto|omit|custom:TEXT>` | PDF | auto |

Format-inapplicable values reject before Pack I/O, even at another format's
default. Such CLI-only applicability failures and zero/negative/NaN/infinite
PPI are usage exit 2. Accepted-shaped semantic request rejections exit 1.

`FINITE_POSITIVE_DECIMAL` accepts the finite subset of Rust `f32` decimal
syntax: optional `+`, digits with an optional decimal point or a leading decimal
point with digits, and an optional signed decimal exponent. Admission rounds
once to IEEE-754 binary32 using round-to-nearest ties-to-even, rejects zero,
underflow-to-zero, overflow, NaN, and infinity, and records the resulting eight
lowercase hexadecimal bits as the canonical semantic PPI. CLI and Dagger use
the same parser and therefore cannot preserve lexical spelling as meaning.

Automatic PDF tagging enables all-page output and disables explicit page
selection with a canonical warning. Explicit tags plus page selection, or no
tags plus a tag-required standard, reject. Standards are Typst 0.15.0's `1.4`,
`1.5`, `1.6`, `1.7`, `2.0`, `a-1b`, `a-1a`, `a-2b`, `a-2u`, `a-2a`, `a-3b`,
`a-3u`, `a-3a`, `a-4`, `a-4f`, `a-4e`, and `ua-1`.

Page grammar is `N`, `N-M`, `N-`, and `-M`; one-based Source Page Numbers form
a canonical set. Compile templates support `{p}`, `{0p}`, `{n}`, and `{t}`;
`{n}` aliases `{0p}`. Multiple artifacts require a page placeholder. A valid
zero-page result succeeds with an empty artifact set; named delivery commits an
empty collection without mutation, while stdout delivery fails.

#### Authorities And Execution

| Option | Contract |
| --- | --- |
| `--font-path <DIR>` | Repeatable; `TYPST_FONT_PATHS` and platform path delimiter. |
| `--ignore-system-fonts` | `TYPST_IGNORE_SYSTEM_FONTS`. |
| `--ignore-embedded-fonts` | `TYPST_IGNORE_EMBEDDED_FONTS`; never hides Pack-embedded fonts. |
| `--package-path <DIR>` | `TYPST_PACKAGE_PATH`. |
| `--package-cache-path <DIR>` | `TYPST_PACKAGE_CACHE_PATH`; authority-private content cache, not result cache. |
| `--offline` | No attempt facility performs network I/O; later delivery is separate. |
| `-j`, `--jobs <N>` | Engine Runtime Domain width `W` only. Omitted is `omitted_automatic`; explicit `0` is `explicit_zero_automatic`; positive is `exact_positive`. |
| `--isolation <in-process|worker>` | in-process; worker adds killability/resource placement, not Hostile. |
| `--deadline <DURATION>` | Attempt through terminal commitment; native default none. |

Package source priority is Typst 0.15.0 data root, cache root, then `@preview`
registry/mirror. Font priority is system, Typst embedded, then configured paths
in declaration order. Ignore flags remove sources before catalog freeze. Only
`unavailable` permits fallback; denial, invalid content, wrong identity, and
integrity failure stop.

The lexical jobs record is adapter inventory outside every core Creation Report
and Compilation Report, and its shape is identical for `create` and `compile` in
both first-party adapters. It retains exactly one lexical class:
`omitted_automatic`, `explicit_zero_automatic`, or `exact_positive`. The first
two normalize to `EngineWidthRequest::Automatic`; the third normalizes to
`EngineWidthRequest::Exact(N)`. A negative value, sign-only value, overflow, or
other non-unsigned spelling rejects before lifecycle admission. The normalized
`EngineWidthRequest` is always retained after successful lexical parsing.

The operation envelope correlates that adapter record with the existing final
Rust request/admission sources rather than adding another semantic copy. An
Admission Refusal exposes the core request at
`admission_refusal.operation_request.engine_width`. An admitted Creation Report
or Compilation Report exposes the same core request through the existing
`report.operational_inventory.role_execution.engine_width.requested` member of
`EngineWidthAdmission`; its admitted `W` and selected domain are adjacent in the
same `role_execution` inventory. The report branches therefore do not add a
duplicate outer `operation_request`. Schema validation requires both automatic
lexical classes to have normalized `automatic` and core requested `automatic`,
and requires `exact_positive` to have normalized `exact` and core requested
`exact`, for both refusal and report branches.

For every admitted report, `adapter_jobs.admitted`,
`report.operational_inventory.role_execution.engine_width.admitted`, and
`report.operational_inventory.role_execution.domain.width` are the same admitted
`W`. For `exact_positive`, those three values must additionally equal
`adapter_jobs.normalized.width` and
`report.operational_inventory.role_execution.engine_width.requested.width`.
On Admission Refusal, the applicable exact equality is between normalized width
and `admission_refusal.operation_request.engine_width.width`; there is no
admitted or selected-domain value. These dynamic equalities are mandatory
semantic invariants for both create and compile harness paths because Draft
2020-12 cannot compare independently encoded values. Exact positive width is an
exact admission request: it may not be lowered, rounded, clamped, or replaced by
automatic width. If that exact width cannot be admitted, operation admission
refuses; when width availability is the deciding failure its closed reason is
`engine_width`. Refusal never fabricates an admitted width or report.

Jobs never alters or aliases acquisition `D`, facility `K/Q`, isolated worker
capacity `P`, or transport `T`. Managed processes remove `RAYON_NUM_THREADS`.
Unavailable exact positive width or prior incompatible initialization is
admission refusal, not silent lowering. The profiles expose separate creation
and compilation role ceilings over one adapter-owned pool. Native `P` is
applicable only when `--isolation worker` is selected and is otherwise reported
not applicable from the execution branch. Dagger always uses its managed worker
topology, so admitted Dagger creation and compilation inventories report their
actual requested/admitted `P`; no profile value is copied into a report.

#### Reporting And Publication

| Option | Contract |
| --- | --- |
| `--diagnostic-format <human|short>` | stderr only; human default. |
| `--report <PATH|->` | Stable operation JSON; `-` is one stdout producer. |
| `--report-include <CHANNELS>` | Repeatable comma list; requires report. |
| `--deps <PATH|->`; `--deps-format <json|zero|make>` | JSON default; post-commit build dependencies. |
| `--timings <PATH>` | Experimental Perfetto projection, not stable JSON. |
| `--open[=<VIEWER>]` | First nonempty delivered filesystem artifact. |
| `--publication-commit <auto|complete-collection-atomic|each-object-atomic|streaming>` | Minimum sink strength. |
| `--publication-deadline <DURATION>` | Delivery only; native default none. |

Report capabilities are independent `diagnostics`, `evidence`, `sources`,
`request-values`, `override-bytes`, `backing-locators`, and `adapter-detail`.
`evidence` is the CLI and GraphQL token; its stable JSON field is
`canonical_evidence`. Identity-only disclosure is the safe default. Channel
states are `not_requested`, `complete`, `redacted`, `limited`, or `unavailable`.
Human renderers neutralize control characters and never interpret untrusted
diagnostic text as ANSI/markup; color decorates only trusted adapter framing.

Outer adapter channels are separate from those report disclosure channels and
use one reason/failure rule everywhere they occur: compile `adapter_input`,
create post-commit timings, and compile post-commit dependencies, timings, and
viewer. `not_requested`, `succeeded`, and `complete` require null `reason` and
null `failure`. `not_attempted` requires a nonempty `reason` naming the earlier
terminal or inapplicability that prevented an attempt, and requires null
`failure`. `redacted`, `limited`, and `unavailable` require a nonempty reason and
null failure; `failed` requires both a nonempty reason and the typed failure.
The standalone `not_attempted_channel` definition has the same nonempty-reason
rule. Report disclosure channels have no `not_attempted` state and are
unchanged.

Stable `report_projection` is exactly the public `CompilationReportProjection`:
the semantic-result status and document or operation phase and cause;
Compilation Identity; optional Result Identity; artifact role, Artifact
Identity, Content Identity, and exact bytes; diagnostic summary and canonical
policy; and the seven channel status/data pairs. Projected artifacts have no
ordinal or presentation name. The projector has no telemetry accessor, so
stable report projection has no telemetry records. Requested/actual timing
status remains only in Attempt Operational Inventory; CLI `--timings` is a
post-commit experimental rendering outside stable report projection.

The operation/report wrapper separately and mandatorily serializes accepted
`request_inventory`, all six sections of `attempt_inventory`, and cache
provenance from the public `CompilationReport` accessors. `attempt_inventory` is
the wire projection of `CompilationOperationalInventoryView`; its strict final
section name is `role_execution`. The adapter lexical jobs record is a sibling,
not part of either core inventory. These metadata fields are not nested into
`report_projection` and cannot be presented as disclosure-projector output.
Complete message/hint/span records appear only in `canonical_diagnostics`.
`request_values` entries expose only key and raw Typst input value. An
`override_bytes` entry exposes only path, raw bytes, replacement Content
Identity, and `equals_baseline`; neither disclosure view exposes a Compilation
Request Commitment.

The `canonical_evidence` channel carries the applicable Compilation Access Trace
plus its report-local, identity-safe Dependency Resolution Evidence table. Its
serialized `CanonicalEvidenceDisclosureView` contains exactly `trace` and
`evidence`; terminal ownership is already determined by the projected terminal
and is not redundantly serialized. The view never carries evidence keys,
credentials, backing locators, or raw request values. `complete`, `redacted`,
and `limited` require a non-null evidence array that resolves every report-local
reference. When originating evidence is absent, the channel is `unavailable`
with null value rather than a trace with a fabricated or missing table.
The first-party Compilation operation request, admission, and Report preserve
the disabled Semantic Result Cache state defined above. Dagger graph reuse
remains outside the attempt and never emits a descriptor, isolation domain,
lookup, admission, or hit.

Dependency JSON remains `{"inputs":[...],"outputs":[...]}`. Inputs are named
Pack representation members, override/control sources, then causal external
package/font backing files in logical requirement order. Missing probes and
network locators are excluded absent locator disclosure. Normalized paths are
deduplicated. JSON rejects non-UTF-8; zero uses NUL native bytes; make escapes
representable paths. Outputs are committed paths in artifact-role order, empty
for committed empty delivery, and absent after failed delivery.

Post-commit order is artifact delivery, dependencies, timings, one initial
viewer launch, then report publication. Failure does not suppress later
evidence-producing steps; viewer requires nonempty filesystem delivery. Skips
record `not_attempted` plus a nonempty reason identifying the earlier terminal
or inapplicability. Every requested failure contributes exit 1 without changing
the terminal/report. Report publication is last and cannot describe its own
write. `not_attempted` belongs only to these outer adapter sidecar outcomes; it
is never a Transport Receipt branch.

`auto` commit is complete-collection atomic for one file, each-object atomic for
several template files, and streaming for stdout. Stronger unsupported requests
are refused. No file is truncated in place. Static locator/pattern collisions
are checked before Pack I/O; exact artifact expansion and collision checks occur
after terminal commitment and before transfer, and refusal preserves the report.

Cleanup minima are fixed: named file/tree/spool staging is complete before
return; stdout is non-retractable accepted. Receipts separately report requested
and actual cleanup. Native ingress, attempts, representation, and publication
default to no deadline. Dagger defaults each to 600s.

### `create`

```text
typst-pack create [OPTIONS] <INPUT> [OUTPUT]
```

Input is a named source file. Output defaults by replacing its extension with
`.typk`; `-` is streaming. Options:

| Option | Default |
| --- | --- |
| `--root <DIR>` | `TYPST_ROOT`, then input parent |
| `--target <paged|html>` | paged |
| shared `--input`, `--features`, `--document-time`, `--creation-timestamp` | one flat variant |
| `--discovery-override <PACK_PATH=FILE>` | empty |
| repeatable `--variant-file <FILE>` | empty |
| repeatable `--include <PATH_OR_GLOB>` | empty |
| `--package-embedding <embed|external>`; `--font-embedding <embed|external>` | embed |
| `--embedding-policy <FILE>` | absent; conflicts with explicitly supplied simple policies |
| `--metadata <FILE>`; repeatable `--annotation <FILE>` | empty |
| all font/package authority flags, offline, jobs, isolation, deadline | profile |
| diagnostic format, timings, report, report include | human/no files |
| `--replace` | false; invalid for stdout |
| publication commit/deadline | auto/none |

Without variant files, flat flags create exactly one explicit variant. Any file
conflicts with explicitly supplied flat target/input/features/time/override and
prevents those environment fallbacks from being read. Files preserve order and
never inherit flat values.

Include selectors are nonempty case-sensitive root-relative strings using `/`,
with no leading slash, backslash, colon, empty, `.`, or `..` segment. `*` and `?`
do not cross `/`; ASCII `[abc]`/`[a-z]`; `**` only as a whole segment crosses
directories. No escaping, negation, brace, locale, or shell expansion. Dot names
are ordinary. Literal directories recurse in canonical order. Every selector
matches a regular file. Links/special nodes reject. Results dedupe and sort by
Project Path; no metadata file is injected.

After discovery fixes the closure and before assembly/replay, creation establishes
a race-closing Creation Evidence Fence. Immutable Dagger inputs use immutable
evidence. Mutable CLI project/package/font/cache/network sources must version or
revalidate every causal content, absence, membership, order, metadata, and
source-choice fact. Change, provider mismatch, incomplete capability, or dirt
prevents Pack Issuance. Replay uses the frozen Discovery Snapshot without
reacquisition.

Create always issues Epoch 2 and uses `epoch-2-all-stored-v1`; with one recipe it
has no selector. Successful first-party archive encoding and archive publication
use exactly
`typst-pack:archive-encoding:1:sha256:4e338d8a54d234ca28392ecf79386944757e0e4adf750192e21311d6b2491170`.
Archive reader assertions remain open to any typed Archive Encoding Identity.
Issuance, encoding, and publication remain separate outcomes. The create
operation's archive-encoding member is the complete Pack Archive Encoding Report,
not a flattened receipt: it always retains its independent
`PackArchiveEncodingFormatReceipt` and retains the separate
`SpoolTransportReceipt` whenever a spool operation was attempted. Absence of a
spool attempt is null, never a synthesized successful spool receipt.
Metadata is non-identifying. Annotation files use `.ann` and opaque payloads.
Generic semantic extensions require a separately typed first-party interface.

Both first-party profiles default Font Scan Policy to `warn_and_omit` for invalid
and unreadable candidates, but a profile is not report evidence. The resolved
policy enters `CreationOperationRequest`; creation admission matches it unchanged
against the exact bound Font Authority descriptor or refuses with
`font_scan_policy_unavailable` before scanning. The Creation Report dependency
inventory records requested, admitted, and `not_reached` or `applied` policy plus
deterministic diagnostics. A returned-policy mismatch fails internal integrity.

Creation request construction is a separate outer terminal. Project Snapshot,
variant, override, inclusion, embedding-policy, metadata, annotation, and final
Creation Request builders aggregate their bounded issues into one
`CreationRequestRejection`. Its wire projection comes directly from
`resource_profile`, requested limits, admitted limits, and the ordered
`CreationRequestIssueView` values with code, role, and optional declaration
ordinal. It occurs before Creation Operation admission and therefore has no
Creation Admission record, Creation Report, phase ledger, authority access,
discovery, encoding, or publication. It must not be flattened to
`adapter_failed` or inserted into a Creation Report.

The strict creation operation union is `adapter_failed`, `request_rejected`,
`admission_refused`, or `creation_report`.
Admission refusal serializes the complete `CreationOperationRequest`, requested
trust, profile and requested limits, the safe bound Creation Evidence, Package
Authority, Font Authority, and optional Creation Execution Facility descriptors,
and the closed refusal reason. It has no report. Only successful operation
admission may produce a Creation Report.

The Creation Report is a core terminal, not an adapter failure bag. It serializes
the reached creation phases, ordered safe diagnostics, all six
`CreationOperationalInventoryView` sections, reporting channels, and one closed
typed failure when issuance fails. Failure detail is serialized only when the
Rust branch exposes it: authority class/code/safe message, Source Changed or
Revalidation Failed safe code, Insufficient Evidence Capability required and
available projections, or the closed resource/interruption/execution/replay/
integrity branch. The operation envelope keeps lexical jobs, request-owned Font
Scan Policy, typed construction/admission terminal, Creation Report, complete
archive-encoding report with its format/spool receipts, publication, and
post-commit timing as separate siblings. No earlier terminal fabricates a core
report.

### `inspect`

```text
typst-pack inspect [OPTIONS] <PACK>
```

`-f, --format <text|json>` defaults text. JSON is
`org.typst-pack.pack-inspection-operation/1`, combining ingress and complete
logical Pack Inspection. It never acquires dependencies, compiles, applies
overrides, infers archive recipe, or projects. Failure leaves stdout empty.
The inspection projection includes every project binding and exact identity,
the full Discovery Coverage Requests and ordered trace observations, complete
package tree inventories and manifest summaries, complete font selection and
licensing descriptors plus catalog order, metadata, and extension/annotation
payload identities and sizes. It does not expose the Pack Control Record bytes.

### `compile`

```text
typst-pack compile [OPTIONS] <PACK> [OUTPUT]
typst-pack c [OPTIONS] <PACK> [OUTPUT]
```

Named Pack plus omitted output replaces the Pack extension with selected format;
stdin requires output. Document output is file/stdout; page output is a template.
Compilation first creates its non-null outer operation envelope and runs pure,
bounded semantic preparation before any operation facility is appraised.
Preparation yields Request Rejection or a Prepared Compilation. Only the latter
enters Compilation Operation admission; refusal retains Compilation Identity but
has no Compilation Terminal or Report, while admitted execution yields an
immutable Compilation Report. Only the latter can
be delivered. Any rejection, operation, delivery, or requested auxiliary failure
exits nonzero. Delivery cannot rewrite a succeeded result. Persistent
result-cache controls are absent.

Every accepted Compilation Report retains a nonempty complete accepted
Compilation Request Inventory in Pack, override, Typst input, document time,
feature, target, tagged output, diagnostic policy, Engine Identity, and Exporter
Identity order. Every status-bearing accepted node is exactly `effective`; it
contains no `supplied_canonical`, invalid-declaration, or rejected-safe marker.
Semantic validation requires Pack, Compilation Document Time, target, output,
diagnostics, Engine, and Exporter entries plus canonical ordering and uniqueness.
The serializer preserves the Rust-provided origin on every node, including
Engine and Exporter Identity, and preserves the separate origin, status, and
optional declaration ordinal of every output and Canonical Diagnostic Policy
leaf. It does not replace those origins with one adapter-level default. The
report also retains the six-part Attempt Operational Inventory for admission,
resources, dependency execution, attempt control, role execution, and reporting,
including the bound role descriptors and requested/admitted/reached distinctions
defined above.

A Request Rejection uses a different frozen safe inventory union and ordered
issue list. It never invents Engine or Exporter entries. The only
`rejected_safe_value` is a canonical unknown Pack Override path with exact byte
count, caller ordinal, and null commitment. Every other canonical value retained
by a rejected request is exactly `supplied_canonical`; a supplied-canonical Pack
Override always carries its Compilation Request Commitment. Invalid markers are
limited to `pack_override`, `typst_input`, `document_time`, `feature`,
`page_selection`, `pdf_control`, `png_pixels_per_inch`, `format_control`,
`diagnostics` (the adapter spelling of diagnostic-policy), and `request_limit`.
Each invalid marker preserves role, declaration ordinal, origin, status, ordered
issue codes, and its optional join to exactly one safe supplied inventory
ordinal. Each request issue likewise preserves its optional referenced inventory
ordinal. Canonical Diagnostic Policy declarations retain per-leaf origins even
when rejected. A rejection always contains at least one issue and has no
Compilation Identity or report; because attestation was not reached it invents
no Engine or Exporter entry.

Compilation Access Trace request kinds are exactly `typst-source` and
`raw-file`. Its closed observation union distinguishes baseline read, override
read, logical missing, baseline invalid as source, override invalid as source,
package read, package logical missing, package invalid as source, undeclared
package access, and font-face access. Every observation carries sorted nonempty
report-local originating-evidence references. Result-owned and partial traces
carry the reached phase plus project/package/font completeness; only the
`evidence` capability projects the applicable trace into stable JSON
`canonical_evidence`. The projected immutable terminal determines whether that
trace came from a semantic Result or an Operation Outcome; no `trace_ownership`
field is serialized.

### `watch`

```text
typst-pack watch [OPTIONS] <PACK> [OUTPUT]
typst-pack w [OPTIONS] <PACK> [OUTPUT]
```

Additional controls:

| Option | Default | Contract |
| --- | --- | --- |
| `--watch-mode <auto|notify|poll>` | auto | notify requires complete push; poll coherent complete poll; auto may combine. |
| `--poll-interval <POSITIVE_DURATION>` | 1s | Explicit value forces polling participation under auto. |
| `--allow-incomplete-watch` | false | Allows only visibly unverified publication. |

Output/report/deps/timings cannot use stdout. Document output is one atomically
replaced file. Page output is a managed directory, not template. Explicit page
output requires `--format`; omitted named-Pack output is
`<PACK_STEM>-<FORMAT>-pages`; stdin requires output.

The page root is absent or an existing `org.typst-pack.watch-output-root/1`.
Each revision stages `generations/<session>/<revision>/`, including an empty
generation, then atomically replaces `current.json` using
`org.typst-pack.watch-output-pointer/1`. The pointer carries session/revision,
Pack and Result identities, currentness, generation, and ordered artifacts. Its
`desired_revision` is the newest accepted observation; `publication_revision`
is the revision that produced the retained artifacts, so an immediate stale
rewrite never pretends old bytes came from the new revision.
Readers use the pointer only. New Pack Identity creates a session namespace and
rebinds the pointer. Current plus one previous generation is retained; older or
abandoned state is boundedly cleaned after commit. Cleanup failure does not roll
back the pointer. Startup cleans only schema-owned unreferenced state.

Watch is one CLI composition over the final Rust `CompilationSession`; Dagger
still exposes no watch or session API. A session is constructed with and bound
to one validated Pack. Re-reading equal Pack Identity may feed the same session,
but a changed Pack Identity starts a new Session Instance, revision namespace,
publication, Last Successful Compilation, and subscription namespace. There is
no Pack-change session event.

Each accepted stabilized request state becomes a new Session Revision whose
`SessionPolicy` owns the exact requested/admitted preparation limits. `Accept`
performs bounded semantic preparation synchronously inside the reducer. Success
creates a Prepared Compilation and may create an attempt token. Compilation
Request Rejection becomes a tokenless candidate. Adapter ingestion failure is
also tokenless, retains its failed request-source scopes and policy, and never
calls preparation. Neither branch fabricates a report or occupies the attempt
slot.

The latest-only scheduler permits at most one active-or-draining attempt and at
most one latest pending Prepared Compilation. A newer prepared revision replaces
an older pending one and synchronously revokes the older attempt's supersession
permit before `InterruptAttempt` is emitted. A late matching completion still
clears its active/draining slot and activates the latest pending revision even
when that completion cannot publish. `Retry` creates a new Session Evaluation in
the same Session Revision; it does not create a revision or mutate the prepared
semantic request. There is no automatic terminal retry.

Attempt admission happens after the reducer emits `StartAttempt`. If admission
refuses, `AttemptAdmissionRefused { token, refusal }` clears only the exact
active token and starts the latest eligible pending Prepared Compilation. It is
reportless: it emits no `AttemptFinished`, creates no publication candidate or
fence, cannot claim currentness, and cannot replace Last Successful Compilation.

The event and effect vocabularies are exact and independent; rows do not imply a
one-to-one event/effect mapping.

| Session events |
| --- |
| `Accept(AcceptedSessionInput::Stabilized | AcceptedSessionInput::IngestionFailure)` |
| `DependencyChanged { generation, change }` |
| `NotificationGap { generation, scope }` |
| `Refresh` |
| `Retry` |
| `AttemptFinished { token, report }` |
| `AttemptAdmissionRefused { token, refusal }` |
| `AttemptReleased { token, release }` |
| `FenceReadFinished { token, outcome }` |
| `SubscriptionsArmed { token, outcome }` |
| `FenceConfirmed { token, outcome }` |
| `Shutdown` |

| Session effects |
| --- |
| `StartAttempt { token, plan }` |
| `InterruptAttempt { token }` |
| `ReadFence { token, plan }` |
| `ArmSubscriptions { token, plan }` |
| `ConfirmFence { token, plan }` |
| `RetireSubscriptions { generation }` |
| `Publish { publication }` |

There are no equal-reconciliation, preparation-completed,
publication-completed, retirement-completed, or Pack-change events. Publication
and subscription retirement are effects with no completion event. Old-session
completions, superseded revisions, stale attempts or fences, old subscription
notifications, and duplicate completions return the exact ignored transition
and cannot mutate current state. Malformed tokens and impossible adapter
transitions remain typed event rejections.

Currentness follows `ReadFence -> ArmSubscriptions -> ConfirmFence -> Publish`.
The read and confirmation outcomes themselves report equality or dirtiness;
notification gaps and observations during confirmation dirty the candidate and
restart the ordinary fence sequence. Request-source scopes and dependency scopes
remain distinct in every affected-scope value. Read and confirmation plans route
by provider identity and may carry zero, one, or many provider observations and
cursors; no provider or cursor is privileged or invented for an empty set.
Dependency evidence may be absent when preparation rejected before dependency
resolution.

Complete push publishes `current_through_push`; one coherent complete poll
publishes `current_as_of_poll`. Incomplete coverage blocks publication by
default; `--allow-incomplete-watch` permits only `unverified` with exact uncovered
request and dependency scopes. Dirt produces `stale` with exact dirty scopes. A
tokenless ingestion failure may publish only as stale or unverified, never as
known current.

The public Rust lifecycle is exactly `SessionLifecycle::Running`,
`SessionLifecycle::Retiring`, or `SessionLifecycle::Retired`, serialized as
`running`, `retiring`, or `retired`. Its view independently exposes latest
revision/evaluation, active or draining attempt, latest pending prepared
revision, Session Publication, and Last Successful Compilation. Shutdown rejects
new input and retries, revokes
unpublished work, retires proposed and active subscriptions, and interrupts live
work. Retirement reaches `retired` only after attempt/arming resources return,
are reaped, or are abandoned with proof that no live resource remains.

Session Publication, Last Successful Compilation, current delivery, and Last
Successful Delivery are independent adapter state. A newer rejection, Operation
Outcome, ingestion failure, or delivery failure may publish without replacing
Last Successful Compilation. The completed Current Delivery wrapper and Last
Successful Delivery wrapper each retain the exact Session Instance, Revision,
Evaluation, Publication Sequence, Result Identity, and immutable
`CompilationDeliveryOutcome` for the publication they attempted. Those fence
fields are captured from that publication and result; completion never rekeys an
older outcome to the session's latest values. Delivery may therefore finish
after a newer publication while remaining attributable to its original fence.
Only a committed delivery of a succeeded Result advances Last Successful
Delivery. Dagger's one-shot container staging is independent again: it never
becomes Session Publication or delivery and remains absent from the CLI session
reducer.

Watch covers the Pack representation, every override/control request source,
and every causal dependency fact. Change or gap immediately stales Session
Publication and Last Successful Compilation before replacement work. Startup or
operation admission failure, strict coverage loss, fatal watcher failure, or a
requested sidecar failure exits 1. There is no HTTP/live-reload server.
`--open` launches only once after initial nonempty success; empty output is a
no-op.

`--report PATH` atomically replaces `org.typst-pack.watch-state/1` on each
publication and immediate stale/unverified transition. No implicit sidecar.
Without `--report`, document-output currentness is still emitted immediately on
stderr as trusted adapter framing; the retained document file is last-good data
and never itself claims currentness. Machine consumers that need currentness use
`--report`; page consumers always use the mandatory `current.json`, which is
atomically rewritten for stale and coverage transitions before replacement work.

### `materialize`

```text
typst-pack materialize [OPTIONS] <PACK> <DESTINATION>
```

Destination must not exist. Commit is fixed complete-collection atomic with
complete-before-return cleanup. Only baseline project files appear. Metadata,
packages, fonts, extensions, annotations, overrides, receipts, and generated
manifests do not. No force/merge/overwrite/content/commit option exists. It
accepts publication deadline and report.

Project Materialization owns a `ProjectMaterializationReport` whose terminal is
either succeeded or exactly `admission`, `resource_limit`, `encoding`,
`spooling`, `cancelled`, `deadline`, or `internal_integrity`. Its projection
receipt is a role-specific `ProjectMaterializationProjectionReceipt` with the
same common Format Receipt contract-version, role, terminal, stage, counters,
exposure/completion, timing, adapter class, refused-or-admitted controls,
publication, cleanup, and failure semantics as every other Format Receipt. Its
role payload additionally contains the exact Pack Identity, file count,
aggregate bytes, and ordered Project Path, exact-byte, and Content Identity file
inventory. It has no request-rejection terminal. Its format publication state is
not applicable because publication, when requested after a succeeded plan, is a
separate `ProjectMaterializationPublicationTransportReceipt` and outer
`TransportOutcome`.

### `convert`

```text
typst-pack convert [OPTIONS] <PACK> <OUTPUT> --to <archive|closure>
```

| Option | Default |
| --- | --- |
| `--to <archive|closure>` | required |
| `--epoch <2>` | literal 2 |
| `--archive-encoding <epoch-2-all-stored-v1>` | only recipe; archive only |
| `--replace` | false; named archive only |
| publication commit/deadline/report | auto/none/absent |

Archive output is named/stdout and create-new unless replace. Closure output is
an absent complete-collection-atomic directory. Source/destination cannot alias.
Conversion preserves representable extensions/annotations and never discovers,
acquires, repairs, defaults, materializes, or silently loses data.

### Streams And Exit

One stdout producer: archive, one artifact, dependencies, operation JSON, or
inspection. Diagnostics, progress, Derive assurance, currentness, warnings, and
status use stderr. Named success leaves stdout empty.

| Condition | Exit |
| --- | --- |
| Complete finite success, help, version | 0 |
| CLI syntax/enum/cardinality/declared conflict | 2 |
| Adapter, ingress, request, result, operation, publication, requested post-commit failure | 1 |
| Warning with success | 0 |
| Watch signal or broken pipe | platform-native signal status |

Automation consumes JSON status, not a new exit taxonomy.

## Auxiliary Inputs And Stable JSON

The strict final-contract schema target is normative for every serialized structure,
closed branch, enum, and nullability rule. A checked companion schema is a
planning target, not a claim that an implementation already emits conforming
documents.
Cross-field equality, sorted uniqueness, profile tightening, receipt-stage
legality, and the Format Receipt Contract v1 role/terminal matrix remain semantic
validation rules because JSON Schema cannot compare independently encoded
values. Inputs are UTF-8 JSON, at most 1 MiB before payloads, reject BOM,
duplicate keys, unknown fields, unknown major, compatibility aliases, and
trailing data. Relative source paths resolve from the control-file parent. The
regenerated strict schema defines:

- `org.typst-pack.discovery-variant/1`, including absent/unix time;
- `org.typst-pack.override-set/1`;
- `org.typst-pack.embedding-policy/1` with mandatory defaults and exact rules;
- `org.typst-pack.canonical-diagnostic-policy/1`;
- `org.typst-pack.pack-metadata/1` and `.ann` annotation input;
- `org.typst-pack.operation-limits/1`;
- `org.typst-pack.adapter-resource-profile/1`, with role-specific creation and
  compilation execution capacities over one shared adapter pool, creation
  package-file/largest-member/Font Catalog candidate limits, isolated creation
  worker capacity, and the first-party `warn_and_omit` Font Scan Policy default;
  and
- all ingress, inspection, creation, compilation, representation, staging,
  transport, watch-state, and watch-pointer output documents.

Duplicate input/override/policy selectors reject after schema parse. Unmatched
embedding rules reject. Every numeric operational override is a decimal string
and may only tighten the named profile. Authority-cache occupancy is part of
this overlay; Dagger uses the minimum of the `dagger-ci/1` cap, constructor
quota, and operation override. Operational limits stay separate from semantic
diagnostic policy, jobs width, and deadlines. Canonical Diagnostic Policy is
admitted independently by proving both dimensions are no greater than the
profile defaults; equality and tighter values are valid, while an attempted
increase is a pre-preparation adapter outcome.

Operation Limits and resource profiles remain schema version 1 and replace their
earlier prototype shapes in place. These unreleased artifacts have no persisted
or external v1 consumer requiring a compatibility branch. Profiles are adapter
inputs to admission and are never serializer fallback values for a report.

Every first-party creation, compilation, Format Receipt, and Transport Receipt
view has a non-null profile owned by its producer discriminator. `cli` requires
exactly `native-cli/1`; `dagger` requires exactly `dagger-ci/1`. The receipt
document header and `adapter_class` must name the same producer. Nullable
generic-core profile accessors are outside these first-party schemas. Both producer profiles still
reserve their creation and compilation role views against one adapter-owned
shared execution pool.

Stable output rules:

- The checked JSON Schema is strict producer conformance for major 1, minor 0.
  A consumer implements the evolution rule below rather than applying that
  exact producer schema to a future minor document.
- UTF-8 JSON plus one trailing newline.
- Header `{schema:{name,major,minor},producer:{...}}`.
- Lower snake case fields/enums and required `kind` on unions.
- Potentially large/signed values are canonical decimal strings; identities are
  complete typed strings; disclosed bytes are unpadded base64url.
- Arrays retain domain order. Branch-impossible fields are explicit null.
- Major changes include field/type/nullability/branch/precedence/meaning changes.
  Minor may add fields or open-registry codes. Consumers reject unknown major,
  accept newer minor, ignore unknown object members, and reject unknown closed
  terminal kinds.
- Major 1 has one current spelling for every field and branch. Producers never
  emit, and consumers never accept, a compatibility alias from an earlier
  unreleased prototype.
- Stable codes are contracts; `safe_message` is not. No Rust/Dagger debug layout,
  credential, Dagger ID, or undisclosed raw host path appears.

Generic identity strings are not accepted where a role is known. Pack, Content,
Project Tree, Complete Package Tree, package/font requirement, discovery,
Engine, Exporter, Compilation, Result, Artifact, Archive Encoding, and Closure
Export Tree identities each use their own typed grammar. Sensitive disclosure
records are also role-specific: diagnostic entries, diagnostic source bytes,
Typst request values, Pack Override bytes and equality detail, backing locators,
and adapter detail cannot be interchanged through a generic record bag.

Compilation operation preserves five layers:

1. Pre-core adapter failure: `adapter_outcome` non-null, terminal/report/identities
   null.
2. Pure preparation Request Rejection: preparation policy/limits and safe request
   inventory are non-null; there is no Prepared Compilation, identity,
   operational admission, report, or selected domain.
3. Compilation Operation admission refusal: Prepared Compilation Identity,
   operation request, appraised bound
   descriptors, and typed refusal are non-null; terminal/report/identities are
   absent except for the prepared Compilation Identity.
4. Admitted execution has one immutable Compilation Report.
5. Post-commit delivery and sidecars are sibling outcomes retaining a report.

Creation preserves four pre/post boundaries: adapter ingestion, typed Creation
Request construction, Creation Operation admission, and an admitted Creation
Report. The operation union discriminates adapter failure,
`CreationRequestRejection`, `CreationAdmissionRefusal`, and `CreationReport`;
only the last may be followed by issuance-dependent archive encoding and
publication. The lexical jobs record, request/rejection, admission refusal,
report, complete archive-encoding report with independent format/spool receipts,
and publication receipt are coherent siblings, not nullable hints from which a
terminal is inferred. Representation operations
name their source Pack Identity and make composed ingress optional, so a Dagger
Pack created in memory never fabricates an ingress receipt.
Container-local Dagger staging is never embedded in a representation,
materialization, or compilation operation envelope: each `stagingReceipt` is a
separate `representation-staging` or `compilation-staging` document and cannot
rewrite the immutable operation it follows.
Representation staging is a closed union: `archive_file` uses Archive Content
Identity and no file inventory, `closure_export_directory` uses Closure Export
Tree Content Identity and Closure Export paths, and `project_directory` uses Pack
Identity and Project Path inventory. The branches cannot exchange source
identity or path types.

| Outer/core state | Compilation ID | Report | Result ID | Semantic artifacts |
| --- | --- | --- | --- | --- |
| Adapter failed | null | null | null | null |
| Compilation admission refused | non-null Prepared Compilation Identity | null | null | null |
| Request rejected | null | null | null | null |
| Operation outcome | non-null | non-null | null | null |
| Rejected result | non-null | non-null | non-null | null |
| Succeeded result | non-null | non-null | non-null | complete list, possibly empty |

Format Receipt Contract v1 has seven role-specific stable views:
`pack_archive_encode`, `pack_archive_read`, `closure_export_project`,
`project_materialization`, `closure_export_import`, `pack_archive_publish`, and
`closure_export_publish`. They share the exact `FormatReceiptCommonView`:
contract version, role, terminal, stage, role-specific accounting, Pack exposure, stable-value
completion, timing, adapter class, admission, publication, cleanup, failure
class/cause, and ordered validation rules. The first-party JSON document adds
only the standard schema/producer header around that common projection. Each
view then exposes only its legal typed Pack, archive, control-record, Archive
Encoding, Closure Export Tree, verification/assertion, and file fields.

Format admission is always discriminated. Non-publication `refused` carries the
complete requested `FormatReceiptControlsView` and one
`RepresentationAdmissionRefusalReason`. Publication instead carries a
transport-typed refusal or admission projected from the same private record as
its sibling Transport Receipt. Both refusal receipts require stage `admission`
and the same exact transport reason, with no admitted or reached facts.
`admitted` carries separate requested and admitted controls; role-specific
logical, physical, output, and occupancy accounting, identities, exposure,
completion, timing, publication, cleanup, and failure are serialized only as
reached by their Rust accessors.

The role/terminal matrix, stage monotonicity, accounting coherence, identity
coherence, and fixed-precedence validation rules are semantic validation rules.
The schema fixes every role's accounting shape; there is no generic nullable
counter bag. Numeric equality and ordering between accounting, identities, and
file arrays remain semantic validation. Archive encode/read,
Closure Export project/import, and Project Materialization success report
publication `not_applicable`; archive and Closure Export publication success
report `committed`. No success role may report `not_started`.

Pack Archive Read always preserves every supplied expectation: expected Archive
Content Identity, expected Pack Identity/verification mode, and asserted Archive
Encoding Identity. Expected-match booleans remain null until comparison is
reached. An asserted recipe is `supplied_but_unevaluated` until supported exact
re-encoding and byte comparison is reached, including on an earlier refusal or
terminal; only that comparison can produce verified or mismatched status.
Archive encoding always preserves the selected Archive Encoding Identity and
source Pack Identity even when later work fails.

Every Pack Archive Encoding operation projects the final Rust
`PackArchiveEncodingReport` without folding its receipts together. The
`PackArchiveEncodingFormatReceipt` is mandatory and records the representation
operation. The separately typed `SpoolTransportReceipt` is present whenever the
spool role was attempted, including refusal or failure, and absent only when no
spool role ran. Format success cannot imply a spool receipt, and spool success or
failure cannot replace the Format Receipt.

Project Materialization uses its exact Rust
`ProjectMaterializationProjectionReceipt`, including the common Format Receipt
semantics and its Pack/file payload. Its projection receipt reports format
publication `not_applicable`; optional publication is a distinct transport
operation. Cleanup never replaces primary failure.

Archive and Closure Export publication outcomes carry two closed projections of
the same operation: the role-refined Format Receipt Contract v1 publication
receipt and a role-refined `TransportOutcome` containing its Transport Receipt.
Their success/failure status, role, committed state, and registered Archive
Encoding Identity are constrained locally. Identity and count equality between
the two receipts remains semantic validation because JSON Schema cannot compare
independently encoded fields. Encoding/projection receipts never double as
publication receipts. Project Materialization carries its projection Format
Receipt plus an optional outer `TransportOutcome` and
`ProjectMaterializationPublicationTransportReceipt`.

Transport Receipt has six closed role/subject pairs: Spool, Pack Archive
acquisition, Pack Archive publication, Project Materialization publication,
Closure Export publication, and Compilation Delivery. Each has its own bound
capability descriptor and opaque receipt type; there is no generic public payload
union.

Each Transport request carries one required role/use scope over one frozen
subject; the exact bound descriptor advertises its offered scope, admission
freezes requested and admitted scopes, and the stage ledger records reached
scope. Role, use, coverage, and complete/redacted status must agree and obey
requested-subset-of-offered and reached-subset-of-admitted rules. A transport
class or successful terminal never substitutes for those facts.

The receipt state is exactly `admission_refused` or admitted. Refusal serializes
stage `admission`, the complete operation request, safe role descriptor projection, and one admission
reason, with no admission record, stage ledger, actual commit, cleanup outcome,
residual, exposure, transferred count, or timing reached fact. `admitted`
serializes the role-specific admission record and complete role-legal stage
ledger over `admission`, `plan_freeze`, `reference_resolution`, `acquisition`,
`spooling`, `transfer`, `verification`, `commit`, `cleanup`, and `complete`.
Roles serialize only legal reached stages. Core orchestration contributes
admission, plan-freeze, and complete; an adapter cannot claim those stages from
a success flag. There is no `not_attempted` Transport Receipt; an unattempted
role has no receipt.

Every admitted role ledger obtains `object_count` through
`TransportStageLedgerView::object_count`: one for Archive and spool, exact `N`
for Closure Export, exact `M` for materialization, and exact artifact inventory
count for delivery. It is frozen at plan-freeze and partial transfer never
changes it.

Requested commit, admitted commit, actual commit,
`requested_cleanup_requirement`, `admitted_cleanup_requirement`,
`cleanup_outcome`, residual locator, exposed bytes, structural-network
enforcement reached, general enforcement reached, and interruption winner are
independent leaves. Actual commit appears only if commit was reached. Cleanup
failure may coexist with exposed bytes and a residual; cleanup never rewrites an earlier
primary outcome. First-party outcomes always name `native-cli/1` or
`dagger-ci/1`. Primary failure belongs to the typed outer `SpoolOutcome` or
`TransportOutcome`, never the receipt. The standard schema/producer header is
adapter metadata outside the Rust accessor projection.

Subjects bind receipt facts to the exact expected/actual spool bytes, acquired
archive, encoded archive, Pack, Closure Export tree, or Compilation Result and
artifacts. Residual cleanup exposes only a safe summary in the receipt. Raw
locator bytes exist only in the independent
`org.typst-pack.residual-locator-disclosure/1` post-commit projection guarded by
its own explicit capability, never in Compilation Report `adapter_detail`.

Watch currentness is `current_through_push`, `current_as_of_poll`, `unverified`,
or `stale`, never boolean. Publication currentness and Last Successful
Compilation currentness are separate. Last Successful Delivery is separate
again and may name another revision. Session Publication is a closed union of
Request Rejection, Compilation Report, and adapter-owned ingestion failure, each
carrying Session Instance, Revision, Evaluation, Publication Sequence, and
currentness. Watch State also preserves `running`, `retiring`, `retired`, active
or draining attempt, and latest pending prepared revision. Current Delivery and
Last Successful Delivery are sibling wrappers, not part of Session Publication;
each carries Session Instance, Revision, Evaluation, Publication Sequence,
Result Identity, and its immutable `CompilationDeliveryOutcome`. Only a committed
delivery of a succeeded Result advances Last Successful Delivery.

## Dagger Contract

### Generated Surface

The GraphQL fixture is a desired generated-structure planning target for
names/kinds/nullability and must be validated against the corrected version-1
branches. It is not evidence that actual Dagger implementation source
exists or that Dagger generation, introspection, and generated-client compilation
already pass. Exact parity with actual generated output is an implementation
gate below, not a prototype claim. The pinned engine puts main functions
directly on `Query` and constructor arguments on
`Query.with`; it does not generate `Query.typstPack`. Defaulted source args are
nullable in GraphQL. Required File/Directory/custom inputs are non-null IDs;
optional values are nullable IDs. Lists have non-null elements. Dagger CLI/Shell
render lower-camel fields as kebab-case.

The planning GraphQL fixture's `Compilation.terminal` nullability follows the
outer operation branch: it is null for adapter failure and Compilation Admission
Refusal, non-null for Request Rejection and every admitted report branch. A null
terminal never implies which outer failure occurred; callers inspect the
non-null operation envelope.

Object graph:

```text
Query
|- discoveryVariant / annotation / reportDisclosure
|- pdf / png / svg / html
|- create -> PackCreation
|- readArchive -> PackIngress
`- readClosure -> PackIngress

Pack
|- identity / inspect
|- archive -> PackArchiveEncoding
|- closureExport -> ClosureExport
|- materialize -> ProjectMaterialization
`- compile -> Compilation
```

### Defaults And Semantics

- `with`: no cache when both args null. An authority CacheVolume must pair with
  positive quota bytes. It binds one Cache Isolation Domain from volume
  capability, authorized Dagger caller, and exact authority composition;
  indexes are namespaced and occupancy cannot exceed the supplied quota or the
  `dagger-ci/1` cap of 8 GiB (`8589934592` bytes).
  It stores independently verified package/font content only, never results.
- Discovery Variant: null label, paged, empty inputs/features, Absent time, no
  overrides. `variants:null` creates one convenience variant; empty rejects;
  nonempty replaces convenience values and conflicts with non-default ones.
- PDF: all pages, auto identifier/creator/tagging, null custom values, Omitted
  time, empty standards, not pretty. PNG: all pages, PPI decimal string `144`,
  no bleed. SVG/HTML not pretty, SVG no bleed.
- Create: `main.typ`, empty includes/authorities/metadata, embed all, online
  permitted, no cert, Partially Trusted, `dagger-ci/1`, 600s, auto jobs.
- Ingress: no expectations, Partially Trusted, `dagger-ci/1`, 600s.
- Archive: Epoch 2, all-Stored v1, Partially Trusted, profile, 600s.
- Closure/materialize: Epoch 2 where applicable, same trust/profile/deadline.
- Compile: no overrides, empty inputs/features/authorities, Absent document time,
  online permitted, no cert, profile diagnostic policy/limits, identity-only
  disclosure with all seven `reportDisclosure` booleans false, Partially
  Trusted, 600s, auto jobs.

Every Dagger Compilation operation uses the first-party disabled Semantic Result
Cache contract above: the operation request and operational admission bind the
disabled cache branch, the descriptor is null, isolation-domain presence is
false, and lookup and report provenance are disabled. Dagger graph reuse changes
none of those fields.

Dagger rejects duplicate input keys, features, and standards rather than using
positional last-wins. Unix times and u64 values are decimal strings because
GraphQL Int is signed 32-bit. PPI is a finite-positive decimal String because
the pinned Dang SDK cannot reliably expose Float. An override Directory maps
every regular file by root-relative path; links/special nodes reject. Package
and font Directory lists preserve authority priority; no ambient host sources.

Jobs null is `omitted_automatic`, integer `0` is
`explicit_zero_automatic`, and a positive decimal is `exact_positive`, using the
same adapter jobs record and normalization as CLI create/compile. Negative and
malformed values are GraphQL argument failures before lifecycle admission. Every
uncached lifecycle bundle runs in a fresh managed worker process with one fixed
domain and no `RAYON_NUM_THREADS`; its creation and compilation requests admit
actual `K/Q/W/P`, while each transport request separately admits `T`. The
`dagger-ci/1` profile only caps those admissions. Dagger may graph-cache the
complete immutable bundle; reuse is outer graph behavior, not a fresh attempt.
Sibling fields evaluate one lazy bundle once; `require*` never reruns it.

### Outcome Nullability

| Object/state | Evidence | Nullable value |
| --- | --- | --- |
| PackCreation adapter failed | operation envelope; no rejection/admission/report | Pack null |
| PackCreation request rejected | typed Creation Request Rejection; no admission/report | Pack null |
| PackCreation admission refused | typed Creation Admission Refusal; no report | Pack null |
| PackCreation report failed/issued | Creation Report | Pack non-null only issued |
| PackIngress validated/other | receipt | Pack non-null only validated |
| Representation adapter/core outcome | operation receipt; staging receipt | File/Directory non-null only after immutable representation and container-local staging both succeed |
| Compilation adapter failed | operation; terminal/report null | all identities/artifacts null |
| Compilation admission refused | operation with typed refusal; terminal/report null | Compilation Identity non-null; Result Identity/artifacts null |
| Request rejected | operation+terminal; report null | all identities/artifacts null |
| Operation failed | operation+terminal+report | Compilation ID only |
| Result rejected | operation+terminal+report | compilation/result IDs; artifacts null |
| Result succeeded | operation+terminal+report | IDs; artifacts only after staging succeeds, possibly empty |

Creation `report`, representation `receipt`, and materialization `receipt` are
complete operation envelopes, not their nested Rust report or receipt alone.
Compilation `operation` is always present. `terminal` is null on adapter failure
and Compilation Admission Refusal; the latter remains a typed branch in the
operation envelope and never fabricates a terminal. Request Rejection has a
terminal but no report. Admitted execution has both terminal and report.
Staging status/receipt describes complete container-local artifact
materialization. `requireSuccess` requires a succeeded result and staging.
Ordinary modeled outcomes do not become GraphQL errors before `require*`;
malformed GraphQL, protocol failure, or impossible internal coherence may raise.

This rule is exhaustive: adapter failure, Creation Request Rejection, Creation
Admission Refusal, failed Creation Report, Representation Admission Refusal,
invalid or unsupported representation, Compilation Admission Refusal,
Compilation Request Rejection, Compilation Operation Outcome, rejected
Compilation Result, transport refusal, transport failure, and Dagger staging
failure are modeled data. Only the corresponding `requirePack`, `requireArchive`,
`requireTree`, `requireProject`, or `requireSuccess` accessor turns a modeled
outcome into a GraphQL error.

Staging is a separate one-shot operation and cannot select or rewrite a
Compilation Terminal. A succeeded zero-artifact Result stages successfully as an
immutable empty `Directory`; failed staging exposes no partial `Directory`.
Reducer-only `not_started` and `started` staging states are never GraphQL values.
The Dagger projection emits only final `NOT_APPLICABLE`, `SUCCEEDED`, or `FAILED`
after the one-shot operation reaches a wire-visible terminal. Static artifact
delivery may start only for a succeeded Result, including its zero-artifact empty
collection, never for a rejected Result or Operation Outcome.

Archive encoding, Closure Export, and Project Materialization use the same
two-layer rule: `status` describes the immutable format/projection operation,
while `stagingStatus` and `stagingReceipt` describe container-local File or
Directory construction. A staging failure never rewrites a succeeded core
receipt; `requireArchive`, `requireTree`, and `requireProject` require both.
For archive encoding, the immutable-operation layer itself contains the
independent mandatory `PackArchiveEncodingFormatReceipt` and nullable
`SpoolTransportReceipt`; Dagger staging is a third, later fact and cannot absorb
either receipt.

Status mappings are exact closed derivations from the discriminated source
branch. `PackCreation.ADAPTER_FAILED`, `REQUEST_REJECTED`,
`ADMISSION_REFUSED`, `CREATION_FAILED`, and `PACK_ISSUED` correspond one-to-one
to the four operation branches and the failed/issued Creation Report terminal;
none is inferred from Pack nullability. `PackIngress.ADAPTER_FAILED` iff the
common operation envelope has `kind: adapter_failed`, and
`PackIngress.VALIDATED` iff its admitted Format Receipt succeeded and exposed a
Pack. A refused Format Receipt maps to `ADMISSION_REFUSED` and retains the
refusal. `TypstPackRepresentationStatus.SUCCEEDED` iff the immutable admitted
encoding/projection report succeeded, regardless of later staging. Resource,
spooling, cancellation, deadline, admission, encoding, and integrity statuses
map from their exact Representation terminal. Representation has no request
rejection, and staging status never participates in these mappings.
`TypstPackCompilationStatus.ADAPTER_FAILED`, `ADMISSION_REFUSED`, and
`REQUEST_REJECTED` map directly from their distinct outer/terminal branches;
`RESULT_SUCCEEDED`, `RESULT_REJECTED`, and `OPERATION_FAILED` require an admitted
report and map from its exact report terminal.

Deterministic files:

| Field | Value |
| --- | --- |
| PackCreation.report | `creation-operation.json`, the complete creation operation envelope |
| PackIngress.receipt | `pack-ingress-operation.json`, the common operation envelope with nested role receipt |
| Pack.inspect | `pack-inspection.json` |
| PackArchiveEncoding | `pack.typk` plus complete `archive-encoding-operation.json` with independent Pack Archive Encoding Format and optional Spool Transport receipts, plus `archive-staging.json` |
| Compilation.operation | `compile-operation.json` |
| Compilation.terminal/report | `compilation-terminal.json` only for Request Rejection or admitted execution; `compilation-report.json` only for the admitted report branch |
| Compilation.stagingReceipt | `compilation-staging.json` |
| ClosureExport / materialization | pure tree plus complete operation envelope and staging receipt |

Artifact names are `output.pdf`, `output.html`, `page-<source-page>.png`, and
`page-<source-page>.svg`, unpadded. Names are presentation, not identity. A
returned Dagger File/Directory proves container-local staging only; receipts
make no claim about later host export or external publication.

Dagger intentionally omits watch/session/recovery, stdio/color/rendering/viewer,
Hostile/downgrade, Resource Slots/providers/extraction, ambient sources/time,
universal stores/publication, persistent result cache/default cache volume,
artifact streaming/incomplete directories, epoch 1/lossy migration/repair,
generic semantic extensions, and format-inapplicable controls.

## Failure Precedence

Commands skip inapplicable steps. For `create`, request construction and Creation
Operation admission/execution precede issuance-dependent representation work;
for Pack ingress, the creation steps do not run.

1. Parse argument shape and trust token without files.
2. Refuse trust or pre-parse enforcement capability.
3. Resolve/validate limits and side-effect-free profiles.
4. Normalize jobs/default inputs and statically collision-check plans.
5. Admit each requested acquisition/spool/transport role before its effects.
6. Boundedly stabilize raw inputs.
7. For create, construct the bounded Creation Request; return typed
   `CreationRequestRejection` before Creation Operation admission/report.
8. Admit the Creation Operation against the bound descriptors, refusing rather
   than lowering an unavailable exact-positive engine width; refusal has no
   Creation Report.
9. Run an admitted creation to its Creation Report; failure exposes no Pack and
   prevents issuance-dependent representation or publication.
10. Admit the representation role, including support for every requested or
   asserted archive recipe; preserve typed refusal and supplied-but-unevaluated
   assertions.
11. Check expected Pack Archive Content Identity when archive ingress requested it.
12. Validate safe framing, canonical control, objects, whole Pack, and unsupported
    state in Epoch 2 order.
13. Check expected Pack Identity after complete derivation.
14. Verify an admitted asserted archive recipe by supported exact re-encoding.
15. Stabilize overrides and resolve adapter defaults.
16. Admit the Compilation Operation against the bound descriptors, the exact
    requested engine width, and the explicitly disabled Semantic Result Cache
    branch; an unavailable exact-positive width is refused rather than lowered,
    and refusal remains in the operation envelope with no terminal or report.
17. Compilation Request Rejection occurs before Compilation
    Identity/authority/report.
18. Deterministic compiler/exporter rejection is Result; dynamic pre-commit
    authority/resource/deadline/execution/isolation failure is Operation Outcome.
19. Cancellation/deadline/supersession before terminal commitment wins under the
    core ordering; later signals cannot mutate it.
20. Archive encoding, publication, dependencies, timing, rendering, viewer, and
    response are later siblings of the immutable semantic terminal they consume;
    first-party Semantic Result Cache admission is absent.
21. Commit wins over later cancellation/deadline. Cleanup never replaces an
    earlier transfer/commit failure, but cleanup is the primary stage when it is
    the first failure after a successful commit; actual commit and exposure stay true.

## Reconciliations

- [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59)
  controls over the earlier prototype: request rejection has no report.
- [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57)
  controls over the stale Deflate-named Rust fixture helper; the implementation
  renames it to all-Stored v1.
- The regenerated final Rust fixture controls creation construction rejection,
  operation-bound descriptors and inventories, request-inventory origins,
  representation/transport admission, receipts, and session semantics over the
  prior adapter baseline `43f2af9`.
- The final Rust Creation Resource Limits, Pack Inspection, request inventory,
  and receipt accessors are serialized without adapter-created semantic fields
  or omissions.

The clean break removes current `extract`, Resource Slot/provider flags,
`--embed-fonts`, `--include-typst-embedded-fonts`, `--no-vendor-packages`, direct
truncation, unbounded stdin, default Dagger cache volume, monolithic Dagger
create/compile, and ignored Rayon initialization error. ADR-0002's one private
whole-Pack seam remains. Resource Slot parts of ADR-0004/0005 are historical;
Typst parity parts remain input.

## Prototype Validation

The validation harness checks the hand-authored planning artifacts; it is not
evidence of actual adapter or generated-Dagger parity. Run these commands from
`PROTOTYPE-first-party-cli-dagger-validation`:

```console
$ cargo fmt --all -- --check
$ cargo check --locked --all-targets
$ cargo test --locked --all-targets
$ cargo run --locked
$ jq empty fixtures/cases.json ../PROTOTYPE-first-party-cli-dagger-schemas.json ../PROTOTYPE-native-cli-profile.json ../PROTOTYPE-dagger-ci-profile.json
$ diff -u <(jq -S '[.schema_cases[], .generated_schema_cases[], .semantic_generated_cases[]] | map(.id) | sort' fixtures/cases.json) <(jq -S '[.schema_cases[], .generated_schema_cases[], .semantic_generated_cases[]] | map(.id) | sort | unique' fixtures/cases.json)
$ diff -u <(jq -S '[.capability_constants[] | ["org.typst-pack/native-cli/\(.role)/1", "org.typst-pack/dagger/\(.role)/1"]]' fixtures/cases.json) <(jq -S --slurpfile schema ../PROTOTYPE-first-party-cli-dagger-schemas.json '[.capability_constants[] | .definition as $definition | $schema[0]."$defs"[$definition].enum]' fixtures/cases.json)
$ diff -u <(jq -S '[del(.engine_runtime_domain) | paths | map(tostring) | join("/")] | sort' ../PROTOTYPE-native-cli-profile.json) <(jq -S '[del(.engine_runtime_domain) | paths | map(tostring) | join("/")] | sort' ../PROTOTYPE-dagger-ci-profile.json)
$ git -C .. diff --check -- PROTOTYPE-first-party-cli-dagger-validation PROTOTYPE-first-party-cli-dagger-contracts.md
```

`cargo fmt`, all three `jq`/`diff` checks, the profile-path comparison, and
`git diff --check` exit 0 with no output. `cargo check --locked --all-targets`
exits 0 after compiling the final interface, prior consumer, serializer probe,
and harness. The deterministic `cargo run --locked` summary is:

```text
final contract validation: ok
json-schema: Draft 2020-12, 367 definitions, 1501 local refs, 40 direct + 45 generated cases
profiles: 2 valid; final aggregate, representation, and transport relationships verified
capabilities: 17 producer-correlated constants and first-party trust/cache constraints verified
graphql: 29 types; hand-authored target topology, statuses, and nullability verified
html: 7 scenarios; 9/12 allowed events covered, 7 allowed effects, 6 publish fences and 2 delivery wrappers structurally verified
serializer sources: 31 mechanically linked leaves; 17 poison derivations declared; 46 semantic cases checked
dagger generated parity: DEFERRED (implementation gate not executed)
```

Every source-manifest leaf is mechanically tied either to a compile-probed Rust
accessor marker or to a schema pointer checked by the harness. This proves the
listed representative links, not exhaustive coverage of every schema leaf or a
serializer implementation. The poison derivations are audited declarations;
where no forbidden implementation exists, they are not claimed as executed
rejections. Schema-negative fixtures and the 46 cross-field semantic cases are
executed separately.

The exact-positive create and compile cases compare normalized width, the core
exact Engine Width Request, outer admitted jobs, admitted Engine Width, and the
selected managed-domain width, with one independent mismatch case per value. A
separate exact-positive admission-refusal case compares normalized width with the
refused operation request and requires outer admitted jobs to remain null.

HTML delivery validation structurally inspects the embedded helper and immutable
snapshot source for complete, coherent Current Delivery and Last Successful
Delivery wrappers. It does not parse a complete JavaScript AST or execute the
script.

The harness targets duplicate JSON keys, a UTF-8 BOM, noncanonical numeric
strings, and bytes after the one newline-terminated JSON document, but validates
planning fixtures only. No `dagger check` or clippy gate was run or claimed.
Actual generated Dagger parity remains deferred: generation, introspection,
structural comparison, and generated-client compilation are not reported as
passing.

## Implementation Verification Gate

- Snapshot parsed options, defaults, conflicts, environment fallbacks, six
  commands, and aliases against Typst 0.15.0. Help prose/wrapping is not stable.
- Regenerate and validate every strict major-1 auxiliary schema, including
  duplicate-key, unknown-field, old-alias, and unknown-branch failures.
- Golden-test JSON names, branches, nullability, versions, and precedence.
- Generate the GraphQL schema from actual Dagger implementation source, compare
  a canonical structural form while checking names, kinds, type references and
  nullability, and compile a generated client. The checked planning fixture alone
  does not satisfy this gate.
- Exercise every Dagger nullable branch and prove sibling reuse/require no rerun.
- Prove every modeled Dagger failure remains queryable and only `require*`
  accessors raise.
- Prove adapter failure, Creation Request Rejection, Creation Admission Refusal,
  Compilation Admission Refusal, and Compilation Request Rejection lack reports;
  prove adapter and Compilation Admission Refusal also lack a terminal, while
  Request Rejection retains one.
- Prove first-party Semantic Result Cache is disabled coherently in operation
  request, admission/refusal, dependency inventory, and report provenance, with
  no inferred descriptor, isolation domain, lookup, admission, or hit.
- Audit every wire leaf against the serializer source rule, including exact
  capability classes and descriptors, all six creation/compilation inventory
  sections, lexical jobs, and explicit `D/K/Q/W/P/T` applicability and reach.
  For both create and compile, schema-test automatic and exact lexical/core
  request kind correlation on Admission Refusal and admitted report branches.
- Extend the locked semantic harness for both create and compile to compare
  exact-positive normalized width, core requested exact width, outer admitted
  `W`, `EngineWidthAdmission.admitted`, and selected managed-domain width; poison
  each independently and prove exact admission refuses instead of lowering when
  the requested width is unavailable.
- Exercise representation refusal, every archive expectation/assertion state,
  all seven Format Receipt roles, both Transport Receipt branches, complete legal
  stage ledgers, and independent commit/cleanup/residual/exposure facts.
- Prove every Pack Archive Encoding Report retains its mandatory independent
  Pack Archive Encoding Format Receipt and its independent Spool Transport
  Receipt exactly when the spool role was attempted.
- Reproduce independent Epoch 2 all-Stored corpus vectors.
- Test stdout exclusivity, dynamic collisions, commit refusal, cleanup precedence,
  and latest-only watch publication. For every outer adapter channel usage,
  reject null/empty reasons on `not_attempted`, require the reason to identify an
  earlier terminal or inapplicability, and reject non-null reasons on
  `not_requested`, `succeeded`, and `complete`.
- Test the exact final-Rust session events/effects, synchronous Accept
  preparation, tokenless rejection/ingestion, same-revision retry, active/draining
  and pending bounds, push, poll, zero-provider observations, scope separation,
  retirement, Last Successful Compilation, and distinct Last Successful
  Delivery. Assert the removed event names are not accepted.
- Prove Current Delivery and Last Successful Delivery each retain Session
  Instance, Revision, Evaluation, Publication Sequence, Result Identity, and the
  immutable delivery outcome from the same publication fence.
- Rerun the locked validation harness from the parent, record exact direct and
  generated case counts, and either mechanically link the source manifest and
  poison list to schema/accessor coverage or continue labeling them audited
  declarations plus compile probes.
- Verify every `native-cli/1` and `dagger-ci/1` profile field at its admission
  seam, including creation package files, largest package file, Font Catalog
  candidates, and isolated worker capacity, and prove no report copies profile
  values without a Rust admission/reached source.

No implementation or release work belongs to this prototype branch.
