# PROTOTYPE: First-Party CLI and Dagger Contracts

> Throwaway design artifact for
> [Freeze the first-party CLI and Dagger contracts](https://github.com/sagikazarmark/typst-pack/issues/56).
> It is an implementation-planning contract, not production code and not a
> compatibility promise for the current 0.3 surfaces.

Companion primary sources:

- [`PROTOTYPE-first-party-cli-dagger-generated.graphql`](./PROTOTYPE-first-party-cli-dagger-generated.graphql)
  freezes Dagger generated names and nullability.
- [`PROTOTYPE-first-party-cli-dagger-schemas.json`](./PROTOTYPE-first-party-cli-dagger-schemas.json)
  freezes auxiliary inputs, stable JSON fields, union branches, and nullability.
- [`PROTOTYPE-native-cli-profile.json`](./PROTOTYPE-native-cli-profile.json) and
  [`PROTOTYPE-dagger-ci-profile.json`](./PROTOTYPE-dagger-ci-profile.json) freeze
  every first-party numeric default.
- [`PROTOTYPE-first-party-cli-dagger-contracts.html`](./PROTOTYPE-first-party-cli-dagger-contracts.html)
  exercises the terminal, staging, delivery, and watch state model.

## Question

What exact CLI flags and auxiliary schemas, stable JSON envelopes, Dagger
generated names and nullability, representation controls, report behavior,
trust and watch controls, and publication controls make the accepted
six-command CLI and typed Dagger object model implementation-ready?

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
- Expected failures remain queryable. Only Dagger `requirePack`,
  `requireArchive`, `requireTree`, `requireProject`, and `requireSuccess`
  deliberately raise for modeled outcomes.
- Compilation Request Rejection has no fabricated Compilation Report,
  Compilation Identity, dependency evidence, or access trace. Post-commit work
  retains and never reclassifies an immutable report.
- Epoch 2 writing uses the registered `org.typst-pack.archive.all-stored`
  recipe, exposed as `epoch-2-all-stored-v1` and
  `EPOCH_2_ALL_STORED_V1`. No public "canonical Deflate" name survives.

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
generated-client probes.

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

Input kind is selected once; no parser fallback occurs. Expected Content
Identity covers complete archive bytes or canonical Closure Export tree content.
Archive Encoding expectation is archive-only and requires support for that exact
recipe, full Pack validation, exact re-encoding, and byte comparison. Generic
ingress never infers a recipe.

Pack Archive ingress carries a separate `pack_archive_acquisition` Transport
Receipt for bounded stabilization before the role-refined `archive_read` Format
Receipt. Acquisition has `not_applicable` commit strength because it produces an
immutable input value rather than publishing one. Closure Export is already a
finite-tree input and has no Pack Archive acquisition receipt; its format receipt
records the canonical tree identity directly. Content-identity mismatch records
the actual archive identity or actual Closure Export tree identity according to
the selected input kind. Archive acquisition may itself be `failed` at reference,
acquisition, spooling, or cleanup; resource, cancellation, deadline, and
integrity ingress terminals permit either that failed receipt or a completed
transfer followed by a later format failure. Framing, validation, and identity
terminals require acquisition `transferred`.

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
| `--features <FEATURES>` | `TYPST_FEATURES`, then empty | Repeatable comma set: `html`, `a11y-extras`, `bundle`. HTML is derived; bundle is representable but rejected first release. |
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
| `-j`, `--jobs <N>` | Engine Runtime Domain width `W` only. Omitted/0 is admitted auto; 1 sequential; positive exact. |
| `--isolation <in-process|worker>` | in-process; worker adds killability/resource placement, not Hostile. |
| `--deadline <DURATION>` | Attempt through terminal commitment; native default none. |

Package source priority is Typst 0.15.0 data root, cache root, then `@preview`
registry/mirror. Font priority is system, Typst embedded, then configured paths
in declaration order. Ignore flags remove sources before catalog freeze. Only
`unavailable` permits fallback; denial, invalid content, wrong identity, and
integrity failure stop.

Jobs never aliases acquisition `D`, facility `K/Q`, transport `T`, or isolated
workers `P`. Managed processes remove `RAYON_NUM_THREADS`. Unavailable positive
width or prior incompatible initialization is admission failure, not silent
lowering. Complete numeric behavior is in the profile artifacts.

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

Report channels are independent `diagnostics`, `sources`, `request-values`,
`override-bytes`, `backing-locators`, and `adapter-detail`. Identity disclosure
is safe default. Channel states are `not_requested`, `complete`, `redacted`,
`limited`, or `unavailable`. Human renderers neutralize control characters and
never interpret untrusted diagnostic text as ANSI/markup; color decorates only
trusted adapter framing.

The Compilation Report's separate `telemetry` reporting channel contains only
the closed stable phase/duration projection. It is not one of the six disclosure
capabilities. CLI `--timings` is a post-commit experimental Perfetto rendering
of available telemetry and remains outside stable JSON; failure to render it
cannot rewrite the report.

The stable Compilation Result projection always carries diagnostic policy,
counts, completion, and identity but sets its canonical `entries` field to null.
Complete message/hint/span records appear only in the independently authorized
`diagnostics` channel. `evidence_disclosure: identity` similarly sets dependency
evidence and access traces to null; explicit `canonical` disclosure admits their
identity-safe logical projection, still without backing locators. The first-party
Compilation Report always records Semantic Result Cache as disabled; Dagger
graph reuse remains outside the attempt and never emits a cache hit.

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
record `not_attempted` plus reason. Every requested failure contributes exit 1
without changing the terminal/report. Report publication is last and cannot
describe its own write.

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
has no selector. Issuance, encoding, and publication remain separate outcomes.
Metadata is non-identifying. Annotation files use `.ann` and opaque payloads.
Generic semantic extensions require a separately typed first-party interface.

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
Compilation first obtains an immutable terminal, then delivers. Any rejection,
operation, delivery, or requested auxiliary failure exits nonzero. Delivery
cannot rewrite a succeeded result. Persistent result-cache controls are absent.

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

Watch covers the Pack representation, every override/control source, and every
causal external dependency fact. Equal Pack preserves session; changed identity
replaces it. Change/gap/downgrade immediately stales publication and Last
Successful Compilation before replacement work. Only latest revision publishes.
Rejection, operation, delivery, or ingestion failure publishes state and retains
last success. Startup/admission failure, strict coverage loss, fatal watcher
failure, or requested sidecar failure exits 1. There is no automatic terminal
retry or HTTP/live-reload server. `--open` launches only once after initial
nonempty success; empty output is a no-op.

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

The JSON Schema companion is normative for every serialized structure, closed
branch, enum, and nullability rule. Cross-field equality, sorted-uniqueness,
profile tightening, receipt stage monotonicity, and the accepted Format Receipt
Contract v1 role/terminal matrix remain semantic validation rules because JSON
Schema cannot compare their independently encoded values. Inputs are UTF-8 JSON,
at most 1 MiB before payloads, reject BOM, duplicate keys, unknown fields,
unknown major, and trailing data. Relative source paths resolve from control-file
parent. It defines:

- `org.typst-pack.discovery-variant/1`, including absent/unix time;
- `org.typst-pack.override-set/1`;
- `org.typst-pack.embedding-policy/1` with mandatory defaults and exact rules;
- `org.typst-pack.canonical-diagnostic-policy/1`;
- `org.typst-pack.pack-metadata/1` and `.ann` annotation input;
- `org.typst-pack.operation-limits/1`;
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
- Stable codes are contracts; `safe_message` is not. No Rust/Dagger debug layout,
  credential, Dagger ID, or undisclosed raw host path appears.

Compilation operation preserves three layers:

1. Pre-core adapter failure: `adapter_outcome` non-null, terminal/report/identities
   null.
2. Core Compilation Terminal: Request Rejection or Compilation Report.
3. Post-commit delivery and sidecars: sibling outcomes retaining a report.

Creation has the same phase ownership: a pre-core adapter outcome may precede a
Creation Report; request, report, archive encoding receipt, and publication
receipt are nullable siblings with discriminated coherence. Representation
operations name their source Pack Identity and make composed ingress optional,
so a Dagger Pack created in-memory never fabricates an ingress receipt.

| Core state | Compilation ID | Report | Result ID | Semantic artifacts |
| --- | --- | --- | --- | --- |
| Adapter failed | null | null | null | null |
| Request rejected | null | null | null | null |
| Operation outcome | non-null | non-null | null | null |
| Rejected result | non-null | non-null | non-null | null |
| Succeeded result | non-null | non-null | non-null | complete list, possibly empty |

Representation receipt distinguishes validation, unsupported, identity mismatch,
encoding assertion, resource, cancellation, deadline, admission, internal,
transport, commit, and cleanup facts using the complete accepted Format Receipt
Contract v1 registry. Project Materialization uses a separately versioned
adapter projection receipt rather than inventing a Format Receipt role. Cleanup
never replaces primary failure.

Archive and Closure Export publication envelopes carry two closed projections
of the same operation: the role-refined Format Receipt Contract v1 publication
receipt required by the format contract and the role-refined Transport Receipt
required by the adapter contract. Their terminal, commit, identity, count, and
cleanup facts must agree. Encoding/projection receipts never double as
publication receipts. Project Materialization has no Format Receipt role and
therefore carries only its projection receipt plus a Transport Receipt.

Watch currentness is `current_through_push`, `current_as_of_poll`, `unverified`,
or `stale`, never boolean. Publication currentness and Last Successful
Compilation currentness are separate. Last Successful Delivery is separate
again and may name another revision. Session Publication is a closed union of a
Compilation Terminal and an adapter-owned ingestion failure, each carrying its
revision and currentness; delivery remains a sibling field.

## Dagger Contract

### Generated Surface

The GraphQL fixture is the normative desired generated structure for
names/kinds/nullability. This planning artifact is not a claim that absent
implementation source has already passed Dagger generation; the structural
comparison and generated-client compile below are implementation gates. The pinned engine
puts main functions directly on `Query` and constructor arguments on
`Query.with`; it does not generate `Query.typstPack`. Defaulted source args are
nullable in GraphQL. Required File/Directory/custom inputs are non-null IDs;
optional values are nullable IDs. Lists have non-null elements. Dagger CLI/Shell
render lower-camel fields as kebab-case.

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
  online permitted, no cert, profile diagnostic policy/limits, identity
  disclosure, Partially Trusted, 600s, auto jobs.

Dagger rejects duplicate input keys, features, and standards rather than using
positional last-wins. Unix times and u64 values are decimal strings because
GraphQL Int is signed 32-bit. PPI is a finite-positive decimal String because
the pinned Dang SDK cannot reliably expose Float. An override Directory maps
every regular file by root-relative path; links/special nodes reject. Package
and font Directory lists preserve authority priority; no ambient host sources.

Jobs null/0 means admitted auto, 1 sequential, positive exact. Every uncached
lifecycle bundle runs in a fresh managed process with one fixed domain and no
`RAYON_NUM_THREADS`; K/Q/T/P remain profile controls. Dagger may graph-cache the
complete immutable bundle; reuse is outer graph behavior, not a fresh attempt.
Sibling fields evaluate one lazy bundle once; `require*` never reruns it.

### Outcome Nullability

| Object/state | Evidence | Nullable value |
| --- | --- | --- |
| PackCreation issued/failed | operation report | Pack non-null only issued |
| PackIngress validated/other | receipt | Pack non-null only validated |
| Representation adapter/core outcome | operation receipt; staging receipt | File/Directory non-null only after immutable representation and container-local staging both succeed |
| Compilation adapter failed | operation; terminal/report null | all identities/artifacts null |
| Request rejected | operation+terminal; report null | all identities/artifacts null |
| Operation failed | operation+terminal+report | Compilation ID only |
| Result rejected | operation+terminal+report | compilation/result IDs; artifacts null |
| Result succeeded | operation+terminal+report | IDs; artifacts only after staging succeeds, possibly empty |

Compilation `operation` is always present. `terminal` is null only on adapter
failure; `report` exists only on report branch. Staging status/receipt describes
complete container-local artifact materialization. `requireSuccess` requires a
succeeded result and staging. Ordinary modeled outcomes do not become GraphQL
errors before `require*`; malformed GraphQL, protocol failure, or impossible
internal coherence may raise.

Archive encoding, Closure Export, and Project Materialization use the same
two-layer rule: `status` describes the immutable format/projection operation,
while `stagingStatus` and `stagingReceipt` describe container-local File or
Directory construction. A staging failure never rewrites a succeeded core
receipt; `requireArchive`, `requireTree`, and `requireProject` require both.

Status mappings are exact: `PackCreation.PACK_ISSUED` iff its Creation Report
issued a Pack; `PackIngress.VALIDATED` iff its ingress receipt succeeded and
exposed a Pack; `TypstPackRepresentationStatus.SUCCEEDED` iff the immutable
encoding/projection receipt succeeded, regardless of later staging. Adapter
preparation maps to `ADAPTER_FAILED`; Project Materialization request defects
map to `REQUEST_REJECTED`; resource, cancellation, deadline, admission, and
integrity terminals map by the same named receipt terminal. Staging status never
participates in those mappings.

Deterministic files:

| Field | Value |
| --- | --- |
| PackCreation.report | `creation-operation.json` |
| PackIngress.receipt | `pack-ingress-operation.json` |
| Pack.inspect | `pack-inspection.json` |
| PackArchiveEncoding | `pack.typk` plus `archive-encoding-operation.json` and `archive-staging.json` |
| Compilation.operation | `compile-operation.json` |
| Compilation.terminal/report | `compilation-terminal.json` / nullable `compilation-report.json` |
| Compilation.stagingReceipt | `compilation-staging.json` |
| ClosureExport / materialization | pure tree plus operation and staging receipts |

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

1. Parse argument shape and trust token without files.
2. Refuse trust or pre-parse enforcement capability.
3. Resolve/validate limits and side-effect-free profiles.
4. Normalize and statically collision-check plans.
5. Boundedly stabilize raw inputs.
6. Check expected input Content Identity.
7. Validate safe framing, canonical control, objects, whole Pack, and unsupported
   state in Epoch 2 order.
8. Check expected Pack Identity after complete derivation.
9. Verify asserted archive recipe by supported exact re-encoding.
10. Stabilize overrides and resolve adapter defaults.
11. Request Rejection occurs before Compilation Identity/authority/report.
12. Deterministic compiler/exporter rejection is Result; dynamic pre-commit
    authority/resource/deadline/execution/isolation failure is Operation Outcome.
13. Cancellation/deadline/supersession before terminal commitment wins under the
    core ordering; later signals cannot mutate it.
14. Cache admission, encoding, publication, dependencies, timing, rendering,
    viewer, and response are post-commit siblings.
15. Commit wins over later cancellation/deadline; cleanup never replaces primary
    transfer/commit failure.

## Reconciliations

- [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59)
  controls over the earlier prototype: request rejection has no report.
- [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57)
  controls over the stale Rust fixture helper `epoch_2_canonical_deflate`; the
  implementation renames it to all-Stored v1.
- Pack Override limits already required by variation are explicit despite their
  omission from the Rust fixture limit record.
- Stable Pack Inspection is complete logical state despite the Rust fixture's
  subset of accessors; implementation extends projection without exposing the
  Pack Control Record.

The clean break removes current `extract`, Resource Slot/provider flags,
`--embed-fonts`, `--include-typst-embedded-fonts`, `--no-vendor-packages`, direct
truncation, unbounded stdin, default Dagger cache volume, monolithic Dagger
create/compile, and ignored Rayon initialization error. ADR-0002's one private
whole-Pack seam remains. Resource Slot parts of ADR-0004/0005 are historical;
Typst parity parts remain input.

## Implementation Verification Gate

- Snapshot parsed options, defaults, conflicts, environment fallbacks, six
  commands, and aliases against Typst 0.15.0. Help prose/wrapping is not stable.
- Validate all auxiliary schemas including duplicate-key/unknown-field failures.
- Golden-test JSON names, branches, nullability, versions, and precedence.
- Compare a canonical structural generated schema, sorting types/fields/args/
  enum members while checking names/kinds/type references/nullability; compile a
  generated client.
- Exercise every Dagger nullable branch and prove sibling reuse/require no rerun.
- Prove adapter failure and request rejection lack reports; delivery preserves one.
- Reproduce independent Epoch 2 all-Stored corpus vectors.
- Test stdout exclusivity, dynamic collisions, commit refusal, cleanup precedence,
  and latest-only watch publication.
- Test push, poll, unverified, stale, Last Successful Compilation, and distinct
  Last Successful Delivery.
- Verify every `native-cli/1` and `dagger-ci/1` field at its first observable seam.

No implementation or release work belongs to this prototype branch.
