# PROTOTYPE: Compilation-Family Value and Identity Registry

> Throwaway decision artifact for
> [Freeze the compilation-family value and identity registry](https://github.com/sagikazarmark/typst-pack/issues/66).
> It is a planning contract, not production documentation or a compatibility
> promise for unpublished prototype identities.

## Status

**Recommended contract: adopt this registry as the compilation-family identity
baseline.**

This artifact closes the circular delegation found by
[Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58):
the Pack Format Epoch 2 contract deliberately leaves compilation identities to
the compilation contract, while the earlier compilation decisions left their
canonical encoding to the format contract.

The recommendation keeps the accepted semantic model and freezes only the
missing representation choices:

- one global typed identity namespace, adding contiguous kinds `14` through
  `19` after the Epoch 2 assignments;
- the exact Epoch 2 SHA-256 transcript and deterministic CBOR profile, without
  a second commitment algorithm;
- exact canonical values for requests, outputs, diagnostics, dependency
  observations, artifacts, and results;
- a platform-qualified Exporter Identity;
- distinct engine-specific identities and engine-neutral comparison
  projections; and
- byte-level golden vectors for every new identity kind.

## Question

What single normative registry freezes the canonical semantic value encodings,
domain-separated projections and transcripts, identity kind and schema
identifiers, and independent golden vectors for Compilation Request
Commitments, Engine-Neutral Compilation Intent, Compilation Identity,
Compilation Result Identity, Compilation Artifact Identity, and every nested
identity-bearing value so independent implementations produce exactly equal
identities?

## Normative Scope

The words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, and
MAY have their RFC 2119 meanings.

Subject only to the SHA-256 applicability erratum stated below, this registry
imports these parts of the accepted
[Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md)
without modification:

- the RFC 8949 Section 4.2.1 deterministic CBOR profile;
- SHA-256 algorithm value `1`;
- the typed Identity wire tuple and equality rules;
- the exact identity transcript;
- canonical project and package paths;
- the Package Specification tuple, Unicode-XID validation, equality, and
  canonical interpretation under the active Engine Descriptor's declared
  Unicode table;
- namespaced identifiers;
- target, request-kind, disposition, and document-time encodings;
- feature-identifier syntax;
- full typed-identity ordering; and
- existing identity kinds `1` through `13`.

This registry does not add compilation values to the Epoch 2 Pack Control
Record, change any existing Pack identity, or require a Pack Format Epoch bump.
It allocates globally unique kinds for values that live outside Pack
representations.

The SHA-256 applicability ceiling below is a normative erratum to the imported
Epoch 2 format-ceiling text and has precedence over its wider identity-bearing
object limit. Every other imported rule remains unchanged.

This registry freezes logical and byte-level interoperability. It does not
require a Rust layout, serializer library, cache layout, backing representation,
or transport encoding. A public Rust interface may use opaque validated types,
but those types MUST represent exactly the values frozen here.

## Canonical Foundation

### Permitted Values

Every structured payload below is one complete deterministic CBOR array with no
trailing bytes. It uses only:

- unsigned integers;
- byte strings;
- exact valid UTF-8 text strings;
- arrays;
- booleans; and
- `null` only where the grammar permits it.

Negative CBOR integers, CBOR floating-point values, tags, bignums,
indefinite-length values, maps, `undefined`, and alternate spellings are
forbidden in identity payloads.

Text is preserved as exact UTF-8. Implementations MUST NOT normalize, trim,
case-fold, replace, or otherwise rewrite it. Collections described as sets MUST
already be in their field-specific canonical order and contain no duplicate
semantic member.

```cddl
u32 = uint .le 4294967295
positive-u32 = 1..4294967295
u63 = uint .le 9223372036854775807
digest32 = bstr .size 32
possibly-empty-text = tstr .size (0..4294967295)
nonempty-text = tstr .size (1..4294967295)
namespaced-id = nonempty-text

generic-identity = [
  kind: u32,
  schema: u32,
  algorithm: u32,
  digest: bstr,
]

identity = [
  kind: u32,
  schema: u32,
  algorithm: 1,
  digest: digest32,
]

content-id = [1, 1, 1, digest32]
package-requirement-id = [4, 1, 1, digest32]
font-requirement-id = [5, 1, 1, digest32]
engine-id = [10, 1, 1, digest32]
pack-id = [11, 1, 1, digest32]

compilation-commitment-id = [14, 1, 1, digest32]
exporter-id = [15, 1, 1, digest32]
engine-neutral-intent-id = [16, 1, 1, digest32]
compilation-id = [17, 1, 1, digest32]
compilation-artifact-id = [18, 1, 1, digest32]
compilation-result-id = [19, 1, 1, digest32]

identity-object = [exact-size: u63, content: content-id]
compilation-sensitive-value = [
  exact-size: u63,
  commitment: compilation-commitment-id,
]

version = [major: u32, minor: u32, patch: u32]
package-spec = [
  namespace: nonempty-text,
  name: nonempty-text,
  version: version,
]
signed-unix-seconds = [sign: 0 / 1, magnitude: u63]
```

For signed Unix seconds, sign `0` means nonnegative and sign `1` means
negative. Negative zero is invalid. A semantic time MUST also fit the embedded
engine's admitted datetime range. The current range excludes the otherwise
unrepresentable `i64::MIN` magnitude.

`package-spec` is the exact Epoch 2 Package Specification value. Namespace and
name validation uses the Unicode-XID version declared by the active Engine
Descriptor. Adapter lexical spelling does not survive admission; semantic
equality is the exact namespace UTF-8 bytes, name UTF-8 bytes, and three numeric
version components.

FIPS 180-4 defines SHA-256 only for messages shorter than `2^64` bits. The
fixed transcript before `P` is 36 bytes, so every payload used with algorithm
`1` MUST satisfy:

```text
byte_length(P) <= floor((2^64 - 1) / 8) - 36
               = 2305843009213693915
               = 2^61 - 37
```

This is the effective upper bound for raw Exact Content Identity bytes and for
every structured identity payload, regardless of a wider field's syntactic
`u63` range. An `identity-object` exact size cannot exceed this bound. A
structured commitment may admit fewer raw bytes because its complete encoded
payload, including role and context, must fit the same bound.

This applicability ceiling corrects the Epoch 2 format-ceiling clause that
allows identity-bearing object lengths through `2^63 - 1`: the `u63` wire field
remains, but a value too large for the registered digest transcript is invalid.
Future algorithms may register a different applicability bound; they do not
weaken algorithm `1`.

### Exact Identity Transcript

For kind `K`, schema `S`, algorithm SHA-256, and payload bytes `P`:

```text
transcript =
    h'74797073742d7061636b206964656e7469747900'
    || u32be(S)
    || u32be(K)
    || u64be(byte_length(P))
    || P

digest = SHA-256(transcript)
identity = [K, S, 1, digest]
```

The fixed prefix is ASCII `typst-pack identity` followed by NUL. Schema remains
before kind because that is the accepted Epoch 2 transcript. The algorithm is
selected by the typed tuple and is not repeated in its digest input.

Kind is the cryptographic domain separator. Compilation Request Commitments do
not use a second prefix, HMAC, salt, or deployment key. Kind `14` separates them
from Exact Content Identity kind `1` and Discovery Request Commitment kind `6`.

Identity equality compares the complete `(kind, schema, algorithm, digest)`
tuple. Human rendering remains:

```text
typst-pack:<kind-name>:<schema-decimal>:sha256:<64-lowercase-hex>
```

An unknown generic tuple, when a diagnostic renderer chooses to display it,
uses only numeric components:

```text
typst-pack:kind-<kind-decimal>:<schema-decimal>:alg-<algorithm-decimal>:<lowercase-digest-hex>
```

Numeric components use shortest unsigned decimal without leading zeroes. Digest
hex uses exactly two lowercase digits per carried byte.

A bare digest is never an identity. A short `sha256:<hex>` spelling is permitted
only where kind and schema are fixed by an enclosing contract.

`generic-identity` is only a syntactic carrier for preserving an otherwise
unknown nonzero tuple. Kind, schema, and algorithm value `0` are invalid. A
generic carrier may compare or render an unassigned nonzero tuple but cannot
infer its digest length or payload grammar. Every semantic position in schema
`1` instead uses `identity` or a more specific typed alias and therefore
requires algorithm `1`, a 32-byte digest, and the exact registered kind.
An otherwise known kind, schema, and algorithm with the wrong digest length is
invalid rather than unsupported.

### Identity Kind Registry

Existing kinds `1` through `13` retain their Epoch 2 definitions byte for byte.
This registry assigns:

| Kind | Name | Schema 1 payload |
| ---: | --- | --- |
| `14` | `compilation-request-commitment` | Role, Pack Identity, logical key, exact size, and private raw bytes |
| `15` | `exporter` | Exact format-specific exporter descriptor |
| `16` | `engine-neutral-compilation-intent` | Complete effective semantic request without implementation identities |
| `17` | `compilation` | Intent, Engine, and Exporter identities |
| `18` | `compilation-artifact` | Compilation Identity, artifact role, and exact output object |
| `19` | `compilation-result` | Complete canonical semantic result projection |

Value `0` remains invalid. Assigned numbers are permanent and MUST NOT be
reused. Schema numbers are local to a kind. Changing a payload grammar, closed
enum meaning, ordering rule, or identity inclusion rule requires a new schema
for its owning kind.

Nested values do not receive independent kinds merely because they contribute
to an enclosing identity. A kind is allocated only when the value needs a
standalone typed identity at a semantic seam.

The Engine Identity used by compilation is existing kind `10`, schema `1`, with
the exact Epoch 2 Engine Descriptor payload. There is no second compilation
engine kind.

## Compilation Request Commitment

### Roles and Payload

```cddl
compilation-request-commitment-payload =
  [
    role: 1,
    pack: pack-id,
    input-key: nonempty-text,
    exact-utf8-byte-length: u63,
    exact-utf8-bytes: bstr,
  ] /
  [
    role: 2,
    pack: pack-id,
    project-path: nonempty-text,
    exact-byte-length: u63,
    exact-replacement-bytes: bstr,
  ]
```

| Value | Role |
| ---: | --- |
| `1` | Typst input value |
| `2` | Pack Override |

Both roles bind Pack Identity to prevent commitment transplantation and reduce
cross-Pack equality disclosure. The logical key prevents equality reuse across
roles or different keys within one Pack.

For role `1`:

- the key is the exact admitted Typst input key;
- the byte string MUST be valid UTF-8 and equal the exact UTF-8 encoding of the
  supplied Typst input value; and
- empty values are valid.

For role `2`:

- the path MUST satisfy the Epoch 2 canonical project-path rules and identify a
  project file contained by the bound Pack; and
- replacement bytes are arbitrary and may be empty or invalid UTF-8.

The explicit size MUST equal the byte-string length. Preparation derives each
commitment from the active raw request value. A caller cannot assert a digest in
place of supplying that value.

The public safe descriptor is only:

```cddl
[exact-size: u63, commitment: compilation-commitment-id]
```

It excludes raw bytes, replacement Content Identity, baseline Content Identity,
byte-equality status, ingestion location, and eventual use. The active request
may retain raw values until its lifecycle permits disposal. Persisting a
commitment payload or transcript would persist the private value and defeats
the safe projection.

Compilation and Discovery Request Commitments are independently derived even
for the same raw value. Neither substitutes for the other. Commitments are
unkeyed deterministic digests: they reduce accidental disclosure but do not
provide encryption, guessing resistance for low-entropy values, or proof of
authorization.

## Exporter Identity

Exporter Identity describes the actual format-specific implementation used to
produce output artifacts.

```cddl
qualifier = [name: nonempty-text, value: nonempty-text]

exporter-payload = [
  producer-id: nonempty-text,
  implementation-name: nonempty-text,
  implementation-version: version,
  exact-build-fingerprint: bstr,
  target-profile: nonempty-text,
  qualifiers: [* qualifier],
  output-format: 0 / 1 / 2 / 3,
]
```

Producer identifiers use the Epoch 2 lowercase reverse-DNS rules. Qualifiers
are a set sorted by `(name UTF-8 bytes, value UTF-8 bytes)` and qualifier names
are unique.

Output format values are:

| Value | Format |
| ---: | --- |
| `0` | PDF |
| `1` | PNG |
| `2` | SVG |
| `3` | HTML |

The descriptor MUST distinguish every exporter build, backend, dependency,
compile-time feature, target, profile, or platform class for which the producer
does not guarantee exact behavior. The core attests it from the actual exporter;
a caller cannot supply or weaken it.

Two implementations MAY share one Exporter Identity only when their producer
accepts exact-result responsibility across that identity class. Compatibility
without that guarantee is expressed through cross-engine comparison, not by
reusing an identity.

## Engine-Neutral Compilation Intent

### Payload

```cddl
engine-neutral-compilation-intent-payload = [
  pack: pack-id,
  target: 0 / 1,
  inputs: [* [input-key: nonempty-text, value: compilation-sensitive-value]],
  document-time: signed-unix-seconds / null,
  features: [* nonempty-text],
  overrides: [* [project-path: nonempty-text, value: compilation-sensitive-value]],
  output: output-specification,
  diagnostics: canonical-diagnostic-policy,
]
```

The request is complete and effective:

- Pack Identity identifies the closed project and dependency contract.
- Inputs include every supplied member, including unused values.
- Document time is exact or explicitly absent.
- Features include every effective feature, including core-derived `html`.
- Overrides include every member, including unused and byte-identical members.
- Output carries every effective format control and default.
- Canonical Diagnostic Policy is explicit semantic request data.

Inputs sort by input-key UTF-8 bytes. Features use the Epoch 2 lowercase ASCII
syntax `[a-z][a-z0-9-]{0,63}`, sort by exact bytes, and are unique. Overrides
sort by canonical project-path UTF-8 bytes. Input keys and override paths are
unique.

Target values remain Epoch 2 values `0` paged and `1` HTML. PDF, PNG, and SVG
require paged; HTML requires HTML. HTML output includes exactly one effective
`html` feature. First-release preparation rejects the syntactically representable
`bundle` feature before an intent or Compilation Identity exists.

The public semantic type is an opaque validated Feature Identifier using the
open Epoch 2 syntax, not a Rust enum discriminant. Schema 1 first-release
support is nevertheless exact:

- `html` is valid only with HTML output; it is core-derived when absent and
  canonicalizes to the same one effective member when explicitly supplied;
- `a11y-extras` is caller-selectable where the embedded Engine admits it;
- `bundle` is representable in supplied and rejected request inventory but is
  rejected during preparation; and
- every other syntactically valid identifier is rejected as unsupported by the
  exact Engine before an intent identity exists.

A future Engine may admit another syntactically valid feature without changing
the array grammar, but cross-engine Request Compatibility still requires both
implementations to represent the exact effective identifier set. Exact Engine
Identity prevents silently treating unequal support as the same compilation.

Request inventory origin, declaration spelling and order, Discovery Coverage
match, adapter default source, authority, trust, resource limits, cache policy,
deadline, cancellation, isolation, concurrency, timing, disclosure, destination,
filename, and transport are excluded.

Equal effective semantic values have equal Engine-Neutral Compilation Intent
Identity whether they were caller-supplied, core-defaulted, core-derived, or
adapter-resolved.

### Page Selection

```cddl
page-selection =
  [0] /
  [1, ranges: [+ [first: positive-u32, last: positive-u32]]]
```

Tag `0` means all Source Page Numbers. Tag `1` is a nonempty finite subset.
Ranges are inclusive, sorted by `first` then `last`, and satisfy:

- `first <= last`;
- no overlap;
- no adjacency, because adjacent ranges MUST be merged; and
- the sequence `[[1, 4294967295]]` MUST canonicalize to `[0]`.

Adapter syntax with an omitted lower bound canonicalizes to `1`; an omitted
upper bound canonicalizes to `4294967295`. Declaration order, duplicate pages,
overlap, and alternate range partitioning do not survive preparation. Zero and
values above `u32` are invalid.

A finite selection may match no produced source page and yield a successful
empty Page Format result.

### Output Specification

```cddl
pdf-identifier-mode = [0] / [1] / [2, possibly-empty-text]
pdf-creator-mode = [0] / [1] / [2, possibly-empty-text]

pdf-standard = 1..17

output-specification =
  [
    format: 0,
    pages: page-selection,
    identifier: pdf-identifier-mode,
    creator: pdf-creator-mode,
    creation-time: signed-unix-seconds / null,
    standards: [* pdf-standard],
    tagging: 0 / 1 / 2,
    pretty: bool,
  ] /
  [
    format: 1,
    pages: page-selection,
    pixels-per-inch-binary32: bstr .size 4,
    bleed: bool,
  ] /
  [
    format: 2,
    pages: page-selection,
    bleed: bool,
    pretty: bool,
  ] /
  [
    format: 3,
    pretty: bool,
  ]
```

Identifier and creator modes are:

| Value | Meaning |
| ---: | --- |
| `[0]` | automatic |
| `[1]` | omitted |
| `[2, text]` | exact custom UTF-8 text |

Empty custom text remains distinct from automatic and omitted. PDF Creation
Time is independently exact or omitted (`null`). It is never implicitly copied
from Compilation Document Time by the core.

PDF standard values are:

| Value | Standard | Value | Standard |
| ---: | --- | ---: | --- |
| `1` | PDF 1.4 | `10` | PDF/A-2a |
| `2` | PDF 1.5 | `11` | PDF/A-3b |
| `3` | PDF 1.6 | `12` | PDF/A-3u |
| `4` | PDF 1.7 | `13` | PDF/A-3a |
| `5` | PDF 2.0 | `14` | PDF/A-4 |
| `6` | PDF/A-1b | `15` | PDF/A-4f |
| `7` | PDF/A-1a | `16` | PDF/A-4e |
| `8` | PDF/A-2b | `17` | PDF/UA-1 |
| `9` | PDF/A-2u |  |  |

Standards are the selected semantic set, sorted numerically and unique. Empty is
the core default. Preparation rejects combinations the pinned exporter cannot
represent together; it does not invent an explicit PDF-version standard when
the caller selected none.

PDF tagging values are:

| Value | Meaning |
| ---: | --- |
| `0` | automatic |
| `1` | explicitly enabled |
| `2` | explicitly disabled |

Automatic is retained rather than reduced to a boolean because automatic with
a page subset derives tagging disabled and emits the canonical exporter-preflight
warning `org.typst-pack.pdf.tagging-disabled-for-page-selection`. Explicitly
enabled tagging with a page subset, or a tag-required standard with disabled
tags, is a request rejection.

PNG pixels per inch is the exact four-byte big-endian IEEE-754 binary32 bit
pattern. It MUST be finite and strictly positive. Both zero encodings, negative
values, infinities, and NaNs are invalid. Positive subnormal values are valid if
the target exporter admits them. Decimal adapters round once using
round-to-nearest, ties-to-even. Lexical decimal spelling is excluded. The core
default `144` is `h'43100000'`.

Core defaults are:

| Format | Defaults |
| --- | --- |
| PDF | all pages, automatic identifier and creator, omitted creation time, no standards, automatic tagging, not pretty |
| PNG | all pages, `144` PPI, no bleed |
| SVG | all pages, no bleed, not pretty |
| HTML | not pretty |

### Canonical Diagnostic Policy

```cddl
canonical-diagnostic-policy = [
  version: 1,
  maximum-retained-entries: u63,
  maximum-canonical-entry-bytes: u63,
]
```

Both limits may be zero. The featureless core has no hidden numeric default.
An adapter may resolve a documented policy before preparation, but the effective
policy contributes to intent identity.

## Compilation Identity

```cddl
compilation-payload = [
  intent: engine-neutral-intent-id,
  engine: engine-id,
  exporter: exporter-id,
]
```

The Exporter Identity format MUST equal the intent output format. Engine and
Exporter identities are attested by the core and are absent from Engine-Neutral
Compilation Intent.

Preparation returns either a Compilation Request Rejection or a Prepared
Compilation with exactly one Compilation Identity. A rejection has no intent
identity, Compilation Identity, dependency evidence, access trace, report,
result identity, or artifact identity.

An operation outcome after successful preparation retains the Compilation
Identity but has no Compilation Result Identity. A partial access trace reached
by that failed attempt is report data and receives no aggregate semantic
identity.

## Canonical Result Values

### Artifact Role and Identity

```cddl
artifact-role = [
  format: 0 / 1 / 2 / 3,
  source-page-number: positive-u32 / null,
]

compilation-artifact-payload = [
  compilation: compilation-id,
  role: artifact-role,
  object: identity-object,
]
```

PDF (`0`) and HTML (`3`) require `null` Source Page Number. PNG (`1`) and SVG
(`2`) require a positive Source Page Number. The exact size is carried beside
the Exact Content Identity to support bounded verification and collision
detection; emitted bytes are not repeated in the artifact payload.

Filenames, conventional extensions, destination, declaration ordinal, exporter
completion order, backing storage, publication, and transport are excluded.

### Compilation Document Summary

```cddl
compilation-document-summary = [
  target: 0 / 1,
  source-page-count: u32 / null,
]
```

HTML requires `null`. A paged result carries the total Source Page Number count
before page selection whenever a paged document was produced. A compiler
rejection before document production may use `null`; an exporter rejection
after document production MUST carry the count. Every successful paged result
MUST carry the count, including a successful page selection that emits no
artifacts.

### Canonical Diagnostics

```cddl
schema-1-stable-diagnostic-kind =
  "org.typst-pack.pdf.tagging-disabled-for-page-selection"

diagnostic-kind =
  [class: 0, stable-library-kind: schema-1-stable-diagnostic-kind] /
  [class: 1, implementation-specific-kind: namespaced-id]

diagnostic-project-location =
  [class: 0, project-path: nonempty-text, provenance: 0] /
  [class: 0, project-path: nonempty-text, provenance: 1,
    commitment: compilation-commitment-id]

diagnostic-package-location = [
  class: 1,
  requirement: package-requirement-id,
  package-path: nonempty-text,
]

diagnostic-span = [
  location: diagnostic-project-location / diagnostic-package-location,
  start-byte: u63,
  end-byte-exclusive: u63,
]

canonical-diagnostic = [
  phase: 0 / 1,
  severity: 0 / 1,
  kind: diagnostic-kind,
  spans: [* diagnostic-span],
  message: possibly-empty-text,
  hints: [* possibly-empty-text],
]

diagnostic-completion =
  [status: 0] /
  [
    status: 1,
    first-omitted-zero-based-ordinal: u63,
    phase: 0 / 1,
    limiting-dimension: 0 / 1,
  ]

canonical-diagnostic-envelope = [
  policy: canonical-diagnostic-policy,
  retained: [* canonical-diagnostic],
  completion: diagnostic-completion,
]
```

Phases are `0` compiler and `1` exporter. Severities are `0` warning and `1`
error. Kind class `0` is a stable library-defined namespaced identifier. Class
`1` is implementation-specific detail scoped by the exact Engine Identity for
compiler phase and Exporter Identity for exporter phase.

Project provenance `0` is Pack baseline and provenance `1` is Pack Override.
The latter carries the exact Compilation Request Commitment. Package locations
use Package Requirement Identity rather than a package-specification spelling.

Source conversion is exact and precedes result construction:

1. Start with the exact baseline, override, or package file bytes selected by
   the matching source observation.
2. If the bytes begin with exactly `EF BB BF`, remove that one prefix. A second
   consecutive BOM is ordinary UTF-8 source content.
3. Decode the remaining bytes using strict UTF-8. No replacement character or
   lossy conversion is permitted.
4. Invalid UTF-8 produces the matching invalid-as-source observation and cannot
   be the location of a diagnostic span.

Span offsets are zero-based, half-open byte offsets in the resulting UTF-8
source text. `start <= end`. Every span MUST be bounds-checked against that
effective source while the result is constructed, before optional source
retention or disposal. Optional Diagnostic Source Bundle policy can therefore
never change canonical diagnostic validity. Span and hint order is intentional.

Every diagnostic span location MUST resolve to a successful source-read
observation in the same result-owned Compilation Access Trace and MUST carry the
same baseline, override, or package logical identity. Missing and
invalid-as-source observations cannot be span locations.

Canonical order is:

1. compiler diagnostics in deterministic engine order;
2. library-generated exporter-preflight diagnostics in their registered order;
3. exporter diagnostics in deterministic exporter order.

Schema 1 has one closed library diagnostic. It is emitted exactly once if and
only if the prepared output is PDF, page selection is not All, and tagging mode
is Automatic:

```text
[
  phase: 1,
  severity: 0,
  kind: [0, "org.typst-pack.pdf.tagging-disabled-for-page-selection"],
  spans: [],
  message: "PDF tagging was disabled because a page subset was selected.",
  hints: [],
]
```

It is the first exporter-phase diagnostic and precedes implementation exporter
diagnostics. It is absent for All pages and for explicit tagging modes. Any
future stable library kind, trigger, value, or ordering change requires a new
Compilation Result Identity schema rather than a late schema-1 registration.

Implementation diagnostics MUST use namespaced kind identifiers under the
imported Epoch 2 syntax. Canonical message and hint text is produced only from
the exact semantic engine/exporter result. Implementations MUST NOT inject host
paths, backing locators, cache state, timing, retries, rendering, terminal
formatting, or other operational facts. Source-authored text remains exact even
if its characters happen to resemble an operational path.

Completion status `0` is Complete. Status `1` is Limited. Limiting dimension
`0` is entry count and `1` is aggregate canonical entry bytes.

The byte budget is the sum of the standalone deterministic CBOR encoding length
of each complete `canonical-diagnostic`. It excludes enclosing array framing,
policy, and completion. The retained sequence is the maximal canonical-order
prefix fitting both limits. Entry count is tested before bytes for the first
omitted entry and wins when both dimensions reject that same entry. No field or
string is partially truncated. The completion record is outside the entry-byte
budget.

Retained counts and encoded-byte totals exposed by interfaces are rederived and
do not appear separately in the identity projection. A Limited envelope remains
a valid semantic result. Omitted errors still produce rejected status; omitted
warnings never change successful status. Operational inability to construct the
required envelope produces an operation outcome and no result.

The registered warning has this standalone canonical encoding and length:

```text
canonical-diagnostic bytes (125):
860100820078366f72672e74797073742d7061636b2e7064662e74616767696e672d64697361626c65642d666f722d706167652d73656c656374696f6e80783c5044462074616767696e67207761732064697361626c656420626563617573652061207061676520737562736574207761732073656c65637465642e80

complete envelope under policy [1, 1, 1000] (135 bytes):
838301011903e881860100820078366f72672e74797073742d7061636b2e7064662e74616767696e672d64697361626c65642d666f722d706167652d73656c656374696f6e80783c5044462074616767696e67207761732064697361626c656420626563617573652061207061676520737562736574207761732073656c65637465642e808100

entry-count-limited envelope under policy [1, 0, 0] (11 bytes):
8383010000808401000100
```

### Semantic Dependency Projection

Every full Compilation Access Trace observation contains one semantic
observation below plus a canonically ordered set of non-identifying Originating
Evidence References. A reference resolves only in the Compilation Report that
originated the trace. Compilation Result Identity uses the pure projection
`semantic_dependency_projection(trace)`, which removes only those references
and returns the three arrays below without otherwise rewriting an observation.

```cddl
project-observation =
  [tag: 0, project-path: nonempty-text, request-kind: 0 / 1,
    object: identity-object] /
  [tag: 1, project-path: nonempty-text, request-kind: 0 / 1,
    override: compilation-sensitive-value] /
  [tag: 2, project-path: nonempty-text, request-kind: 0 / 1] /
  [tag: 3, project-path: nonempty-text, request-kind: 0,
    object: identity-object] /
  [tag: 4, project-path: nonempty-text, request-kind: 0,
    override: compilation-sensitive-value]

package-observation =
  [tag: 0, requirement: package-requirement-id,
    package-path: nonempty-text, request-kind: 0 / 1,
    object: identity-object, fulfillment: 0 / 1] /
  [tag: 1, requirement: package-requirement-id,
    package-path: nonempty-text, request-kind: 0 / 1,
    fulfillment: 0 / 1] /
  [tag: 2, requirement: package-requirement-id,
    package-path: nonempty-text, request-kind: 0,
    object: identity-object, fulfillment: 0 / 1] /
  [tag: 3, undeclared-package: package-spec,
    package-path: nonempty-text, request-kind: 0 / 1]

font-observation = [
  container: content-id,
  face-index: u32,
  fulfillment: 0 / 1,
]

semantic-dependency-projection = [
  project: [* project-observation],
  packages: [* package-observation],
  fonts: [* font-observation],
]
```

Project tags are baseline read `0`, override read `1`, logical missing `2`,
baseline invalid as source `3`, and override invalid as source `4`. Tags `3`
and `4` are valid only for Typst source request kind `0`; they identify exact
bytes that were selected but could not become source. Request kind `1` is raw
file, matching Epoch 2.

Package tags are read `0`, logical missing `1`, invalid as source `2`, and
undeclared Package Requirement `3`. The last form records a package
specification requested outside the closed Pack and has no invented requirement
identity or content. Fulfillment `0` is embedded and `1` is external. Font
fulfillment uses the same values. A Pack fixes requirement dispositions, but
retaining fulfillment makes the trace self-describing and agrees with the
accepted public provenance model.

Project observations sort by `(project path UTF-8 bytes, request kind, tag)`.
Package observations sort by `(Package Specification namespace bytes, name
bytes, numeric version, package path UTF-8 bytes, request kind, tag)`, deriving
the specification from a declared requirement for tags `0` through `2` and
using the carried specification for tag `3`. Font observations sort by
`(container Content Identity tuple, face index)`. Every collection is a set.
Each logical request has at most one terminal observation. Access order and
repeat count are absent.

The projection MUST satisfy all of these referential invariants:

1. Every baseline read or baseline invalid-as-source path is contained by the
   Pack project tree, has no active intent Override, and its Object Descriptor
   exactly equals that binding.
2. Every override read or override invalid-as-source path is contained by the
   Pack, appears exactly once in the intent Override Set, and has the same size
   and Compilation Request Commitment.
3. A project logical-missing path is absent from the Pack tree and the intent
   Override Set.
4. Every declared package observation resolves to one Pack Package Requirement.
   A read or invalid-as-source object exactly equals the named Complete Package
   Tree binding; logical missing names no member of that tree.
5. An undeclared-package observation names no Package Requirement in the Pack
   with that exact Package Specification.
6. Package and font fulfillment exactly equals the Pack requirement
   disposition. A contained embedded dependency never reports external and an
   external requirement never reports embedded.
7. Every font observation names one face in the Pack Font Catalog and the
   corresponding Font Requirement.
8. Every diagnostic span resolves to a successful source-read observation as
   required by Canonical Diagnostics.
9. The result document target equals the intent target.
10. The result-owned semantic projection equals
    `semantic_dependency_projection(result.access_trace)` byte for byte.
11. For a source request, strict UTF-8 success uses the corresponding read tag
    and strict UTF-8 failure uses the corresponding invalid-as-source tag. An
    active override is never retried against or represented as baseline.
12. Any undeclared-package observation requires rejected result status, no
    artifacts, and at least one compiler error in the complete diagnostic
    stream before Canonical Diagnostic Policy limitation. The retained envelope
    may omit that error, but limitation never changes rejected status.

The projection excludes:

- Dependency Evidence Keys and evidence-reference identifiers;
- authority and provider identities;
- cache use and source class;
- filesystem paths, URLs, object keys, and Backing Dependency Locators;
- retries, redirects, timing, and concurrency;
- acquisition and Pack-creation provenance; and
- reporting-channel state.

Acquisition, integrity, resource, cancellation, deadline, worker, and
infrastructure failure produces a Compilation Operation Outcome rather than a
generic semantic observation or Compilation Result Identity. A semantic-cache
hit preserves the original full trace, including its Originating Evidence
References, and the semantic projection exactly. It never retargets those
references to fresh evidence. Fresh cache-hit evidence belongs only to the new
Compilation Report, whose evidence scope states whether originating evidence is
historical or unavailable.

### Compilation Result Identity

```cddl
compilation-result-payload = [
  compilation: compilation-id,
  status: 0 / 1,
  document: compilation-document-summary,
  diagnostics: canonical-diagnostic-envelope,
  dependencies: semantic-dependency-projection,
  artifacts: [* compilation-artifact-id],
]
```

Status `0` is succeeded and status `1` is rejected.

Result invariants are:

- rejected results own no artifacts and encode an empty artifact array;
- successful PDF and HTML own exactly one artifact of the matching document
  role;
- successful PNG and SVG own one artifact per selected existing Source Page
  Number, possibly none;
- Page Format artifacts are unique and ordered by increasing Source Page
  Number;
- every artifact refers to this result's Compilation Identity and matching
  output format;
- every artifact object is rederived from its exact owned bytes; and
- the artifact identity array exactly matches the complete owned artifact set.

A successful empty Page Format result and a rejected result both have empty
artifact arrays. Status distinguishes them. Adapter `null` versus `[]`
presentation does not create another semantic state.

Request rejections, Compilation Reports, Compilation Operation Outcomes,
Compilation Terminals, deliveries, cache admissions, and session publications
have no competing semantic identity.

## Identity Inclusion Matrix

| Value | Commitment | Intent | Compilation | Artifact | Result |
| --- | --- | --- | --- | --- | --- |
| Pack Identity | direct | direct | transitive | transitive | transitive |
| Raw input or override bytes | direct | through commitment | transitive | transitive | transitive |
| Input key or override path | direct | direct | transitive | transitive | transitive |
| Inputs, features, times, target, output controls | no | direct | transitive | transitive | transitive |
| Canonical Diagnostic Policy | no | direct | transitive | transitive | direct in envelope |
| Request origin and classification | no | excluded | excluded | excluded | excluded |
| Engine Identity | no | excluded | direct | transitive | transitive |
| Exporter Identity | no | excluded | direct | transitive | transitive |
| Result status and document summary | no | no | no | no | direct |
| Canonical Diagnostic Envelope | no | no | no | no | direct |
| Semantic dependency projection | no | no | no | no | direct |
| Evidence, backing provenance, cache, timing | no | excluded | excluded | excluded | excluded |
| Artifact role and exact output object | no | requested role | requested role | direct | through artifact identities |
| Filename, destination, delivery | no | excluded | excluded | excluded | excluded |

## Adjacent Non-Identity Values

### Compilation Request Inventory

The Compilation Request Inventory is canonical audit data, not an identity
payload. The public core model uses a closed typed semantic inventory rather
than stringly names and values. Its accepted entries are:

| Order | Entry | Safe effective value |
| ---: | --- | --- |
| `1` | Pack | Pack Identity |
| `2` | Pack Override | canonical path, exact size, Compilation Request Commitment; path order |
| `3` | Typst input | exact key, exact UTF-8 size, Compilation Request Commitment; key order |
| `4` | Compilation Document Time | exact signed seconds or Absent |
| `5` | engine feature | effective feature identifier; identifier order |
| `6` | target | paged or HTML |
| `7` | output | the complete tagged Output Specification |
| `8` | diagnostics | Canonical Diagnostic Policy |
| `9` | engine | Engine Identity, once attested |
| `10` | exporter | Exporter Identity, once attested |

Every accepted scalar or collection member records origin as
`caller-supplied`, `core-defaulted`, `core-derived`, or `adapter-resolved` and
classification `semantic`. The tagged Output entry is a tree whose format and
each applicable control carry their own origin, so caller-supplied pages can
coexist with a core-defaulted identifier mode and adapter-resolved PDF Creation
Time. Collection containers have no synthetic origin; each input, feature,
override, standard, and page-range member carries its own. Origins do not affect
identity. Inapplicable format controls do not appear; their absence follows from
the tagged Output Specification.

Preparation owns one Semantic Request Inventory. A Prepared Compilation owns
the complete accepted inventory. A Compilation Request Rejection owns the
inventory reached from the supplied request plus its ordered issues, but no
identity is derived from rejected entries. Every Compilation Report retains the
Prepared Compilation's semantic inventory unchanged and appends a separate
typed Attempt Operational Inventory from its Execution Controls and observed
attempt facts.

A semantic inventory node has one closed status:

- `effective`: a canonical value that belongs to a Prepared Compilation;
- `supplied-canonical`: a canonical supplied leaf retained by a rejected
  request, with no claim that the whole request has an effective value; or
- `rejected-safe-value`: one role-specific non-semantic safe value permitted
  below; or
- `invalid-declaration`: a marker with no value.

The first two statuses use exactly the typed safe value shapes in the accepted
entry table, including nested per-field origins. For example, a canonical
contained Pack path can retain path, size, and commitment as
`supplied-canonical` even if another declaration duplicates it. The only
schema-1 `rejected-safe-value` is `unknown-pack-override-target`, containing a
canonical project path and exact replacement size but no Compilation Request
Commitment, because commitment role `2` requires a contained Pack path. A
malformed path has no value node and receives only a marker. Valid controls in an
invalid combination remain supplied-canonical leaf nodes; no invalid complete
Output Specification is invented.

Every supplied node in a rejected inventory carries its declaration ordinal as
non-semantic reporting data. Nodes sort by role, canonical logical key, then
declaration ordinal, so duplicate canonical keys remain deterministic.

An `invalid-declaration` marker has exactly:

- role `pack-override`, `typst-input`, `document-time`, `feature`,
  `page-selection`, `pdf-control`, `png-ppi`, `format-control`,
  `diagnostic-policy`, or `request-limit`;
- declaration ordinal when a canonical logical key is unavailable;
- optional reference to one `supplied-canonical` or `rejected-safe-value`
  inventory node when a safe logical value exists; and
- one or more issue codes in the fixed order below.

Issue-code order is:

1. `invalid-syntax`;
2. `noncanonical-value`;
3. `duplicate-logical-key`;
4. `unknown-pack-path`;
5. `invalid-utf8-value`;
6. `unsupported-feature`;
7. `inapplicable-format-control`;
8. `incompatible-output-controls`;
9. `out-of-engine-range`;
10. `member-limit-exceeded`;
11. `aggregate-limit-exceeded`; and
12. `format-ceiling-exceeded`.

Rejected entries sort first by the accepted inventory role order above, then by
canonical logical key where one exists, then declaration ordinal, then by
issue-code order. An entry MUST NOT invent a canonical path, key, effective
value, commitment, Engine Identity, or Exporter Identity. An unknown Pack path
MUST NOT receive a Compilation Request Commitment. Raw sensitive values remain
inside the active rejected request and are absent from the safe inventory when a
valid role-bound commitment cannot be derived.

Input and override entries never expose replacement Content Identity, baseline
identity, byte equality, or eventual use. Observed use belongs to the
Compilation Access Trace. Adapter ingestion failures before a typed
Compilation Request exists remain adapter inventory rather than fabricated core
entries.

The Attempt Operational Inventory is a separate closed ordered view:

1. admission: requested and admitted Deployment Trust Profile;
2. resources: optional Adapter Resource Profile plus requested and admitted
   Compilation Resource Limits;
3. dependency execution: package and font authority classes, cache policy and
   Cache Isolation Domain presence, offline policy, and acquisition concurrency
   `D`;
4. attempt control: deadline presence, cancellation presence, monotonic time
   domain, and Compilation Interruption Strength;
5. kernel execution: sync caller-thread or async facility, admitted facility
   capacity `K/Q`, Engine Runtime Domain identity and width `W`, isolation mode,
   and worker capacity `P` where applicable; and
6. reporting: requested telemetry, Diagnostic Source Bundle, and Diagnostic
   Projection policies with their reached channel state.

Each variant exposes typed requested, admitted, effective, and reached fields
only where that concept applies; absence is explicit. Authority instances,
cache handles, clocks, interruption sources, workers, and credentials remain
facilities rather than inventory values. The order above is the public iterator
order. No arbitrary name/value extension exists in schema 1, and no member
contributes to a compilation-family identity.

### Dependency Resolution Evidence Table

One originating Compilation Report owns one immutable report-local Dependency
Resolution Evidence Table. `OriginatingEvidenceReference` is the zero-based
`u63` ordinal of an entry in that table. It never contains an opaque key or
authority identity itself.

Each full in-process evidence entry contains an ordinal, one typed subject,
authority binding, one closed Evidence Entry Kind, optional authority-bound
Dependency Evidence Key as prescribed below, sanitized Acquisition Provenance,
and phase. The typed subjects deliberately separate requirement fulfillment from
kernel access:

- project access: project path and request kind;
- package requirement: Package Requirement Identity;
- package access: Package Requirement Identity, package path, and request kind;
- undeclared package access: Package Specification, package path, and request
  kind;
- font requirement: Font Requirement Identity; or
- font face access: Font Container Content Identity and face index.

Evidence Entry Kinds are the sole source of fact and outcome classification:

| Value | Kind | Derived fact kind | Derived outcome |
| ---: | --- | --- | --- |
| `0` | selected-content | content | selected |
| `1` | confirmed-absence | absence | confirmed-absence |
| `2` | confirmed-membership | membership | selected |
| `3` | confirmed-order | order | selected |
| `4` | confirmed-metadata | metadata | selected |
| `5` | selected-source-choice | source-choice | selected |
| `6` | higher-priority-unavailable | source-choice | higher-priority-unavailable |
| `7` | missing | absence | missing |
| `8` | acquired | content | acquired |
| `9` | transient-failure | source-choice | transient-failure |
| `10` | permanent-failure | source-choice | permanent-failure |
| `11` | invalid-content | content | invalid-content |
| `12` | integrity-mismatch | content | integrity-mismatch |

The complete validity matrix is:

| Subject | Authority role | Instance | Priority | Allowed kinds | Key | Provenance | Phase |
| --- | --- | --- | ---: | --- | --- | --- | --- |
| Project access | Pack | absent | `0` | `0`, `1`, `2`, `5` | absent | fixed Pack | kernel |
| Embedded package requirement | Pack | absent | `0` | `0`, `2`, `3`, `4`, `5` | absent | fixed Pack | dependency resolution |
| External package requirement | Package | required | source priority | `0` through `12` | required for `0` through `8`; absent for `9` through `12` | required, source class `0..3` | dependency resolution |
| Embedded package access | Pack | absent | `0` | `0`, `1` | absent | fixed Pack | kernel |
| External package access | Package | required | selected source priority | `0`, `1` | required | required, source class `0..3` | kernel |
| Undeclared package access | Pack | absent | `0` | `1` only | absent | fixed Pack | kernel |
| Embedded font requirement | Pack | absent | `0` | `0`, `2`, `3`, `4`, `5` | absent | fixed Pack | dependency resolution |
| External font requirement | Font | required | source priority | `0` through `12` | required for `0` through `8`; absent for `9` through `12` | required, source class `0..5` | dependency resolution |
| Embedded font face access | Pack | absent | `0` | `0` only | absent | fixed Pack | kernel |
| External font face access | Font | required | selected source priority | `0` only | required | required, source class `0..5` | kernel |

Pack provenance is authority kind `org.typst-pack.pack` with absent source class.
Source-class values are the imported Epoch 2 registry:

| Value | Source class |
| ---: | --- |
| `0` | caller-supplied |
| `1` | explicit local source |
| `2` | cache |
| `3` | network |
| `4` | system font source |
| `5` | engine-embedded font source |

An undeclared-package-access subject has exactly one Pack-role kind `1`
confirmed-absence entry proving absence from the immutable Pack requirement set.
It has no authority instance, external Dependency Evidence Key, acquisition
provenance, Package Authority call, package-cache lookup, or fallback. External
authority access for an undeclared package is an adapter or implementation
contract violation, never another evidence branch.

The authority contract owns opaque key bytes and immutable versions.
Credentials and Backing Dependency Locators never enter the table. Invalid
role, instance, priority, kind, key, provenance, source-class, or phase
combinations are rejected by the coherence-checking constructor.

Entries sort by:

1. subject class project access, package requirement, package access,
   undeclared package access, font requirement, then font face access;
2. the semantic subject order used by the dependency projection;
3. authority-priority ordinal;
4. Evidence Entry Kind numeric order; and
5. unsigned bytewise Dependency Evidence Key bytes, with absent key first.

Ordinals are assigned only after that sort. Every trace reference MUST resolve
to an entry whose subject can causally support that observation. References in
one observation sort numerically and are unique. For each observation, the
reference set MUST equal the complete causal subset of the table: selected
content or absence; every membership, order, metadata, and source-choice fact
that affected resolution; and every higher-priority unavailable or missing fact
that enabled fallback. It contains no causally irrelevant entry. Every
observation therefore has a nonempty reference set, including Pack-owned facts.
An external package access references both the requirement-level fulfillment
facts established during dependency resolution and the path-level access fact
established during the kernel. An external font-face observation similarly
references requirement-level container fulfillment and the face-level kernel
fact. Requirement-level evidence remains in the report even when a fulfilled
external requirement is never observed by the kernel.

A Partial Compilation Access Trace carries this closed reached-scope record:

```cddl
trace-reached-scope = [
  phase: 0 / 1 / 2,
  project: 0 / 1 / 2,
  packages: 0 / 1 / 2,
  fonts: 0 / 1 / 2,
]
```

Phases are dependency resolution `0`, Compilation Kernel `1`, and export `2`.
Per-class scope is not reached `0`, partial `1`, or complete for the actual
reached execution `2`. Every partial or result-owned trace carries this
non-identifying scope. A compiler-rejected result may end in phase `1`; an
exporter-rejected or successful result reaches phase `2`. Complete never claims
that a later phase was reached. Cache records preserve the originating scope
exactly.

The capability-gated canonical-evidence disclosure projects an evidence table
entry to:

```cddl
safe-evidence-subject =
  [class: 0, project-path: nonempty-text, request-kind: 0 / 1] /
  [class: 1, requirement: package-requirement-id] /
  [class: 2, requirement: package-requirement-id,
    package-path: nonempty-text, request-kind: 0 / 1] /
  [class: 3, undeclared-package: package-spec,
    package-path: nonempty-text, request-kind: 0 / 1] /
  [class: 4, requirement: font-requirement-id] /
  [class: 5, container: content-id, face-index: u32]

safe-evidence-entry = [
  ordinal: u63,
  subject: safe-evidence-subject,
  authority-role: 0 / 1 / 2,
  authority-priority: u32,
  evidence-entry-kind: 0..12,
  sanitized-authority-kind: namespaced-id,
  source-class: 0 / 1 / 2 / 3 / 4 / 5 / null,
  phase: 0 / 1,
]
```

Authority-role values are Pack `0`, Package `1`, and Font `2`. Evidence Entry
Kind and source-class values use the tables above. The safe subject uses exact
logical identities but no raw request value. The projection excludes authority
instance identity, Dependency Evidence Key and immutable version, provenance
detail beyond registered source class, credentials, messages, and backing
locators. A trace disclosure uses its semantic observations plus numeric
references into this safe table. If originating evidence is unavailable on a
cache hit, the channel reports unavailable rather than fabricating entries or
remapping references.

### Partial Access Traces

The public trace uses one closed observation enum with the exact project,
package, and font semantic variants defined above. Each observation additionally
carries a nonempty set of opaque `OriginatingEvidenceReference` values sorted by
their report-local unsigned ordinal. The references are non-secret joins into
the originating report's Dependency Resolution Evidence; they are not evidence
keys, backing locators, canonical identities, or semantic ordering inputs.

A Compilation Result owns a Compilation Access Trace complete for its recorded
reached scope. A Compilation Report with an operation outcome owns a Partial
Compilation Access Trace using the same observation enum and scope. Nested
Content Identities and commitments remain typed, but reached scope and a partial
trace receive no aggregate identity and never contribute to a Compilation
Result Identity.

A semantic cache record preserves the result-owned trace, reached scope, and
originating reference ordinals exactly. A cache-hit report records new evidence
separately and never rewrites the cached trace. The report may state that the old
evidence table is unavailable; dangling references then remain non-resolvable
historical references rather than being dropped or rebound.

### Safe Disclosure

Owning an in-process Rust `CompilationResult` permits typed inspection of its
complete immutable semantic state, including artifacts, canonical diagnostics,
and Compilation Access Trace. This is not by itself authorization to serialize,
render, deliver, or otherwise disclose that state outside the caller's process
authority.

All first-party rendering, response, and delivery seams accept a core-produced
`CompilationReportDisclosure` rather than a raw report. Its identity-safe
default contains terminal branch, compilation and result identities, artifact
roles and identities, and diagnostic counts and completion, but not complete
diagnostic entries or traces.

The projector gates these channels independently:

| Channel | Capability-gated value |
| --- | --- |
| canonical diagnostics | exact phases, severities, kinds, spans, messages, and hints |
| canonical evidence | identity-safe Compilation Access Trace and Dependency Resolution Evidence projection |
| diagnostic sources | Diagnostic Source Bundle |
| request values | raw Typst input values |
| override bytes | raw Pack Override bytes and authorized content/equality detail |
| backing locators | Backing Dependency Locators |
| adapter detail | raw adapter-specific detail |

Artifact bytes leave the process only through an admitted Compilation Delivery
or explicit caller-controlled read; identity disclosure alone contains no
artifact bytes. A capability authorizes only its named channel. Disclosure
choices, redaction, projection limits, and failures do not change the source
result or any identity.

## Cross-Engine Comparison Projections

Engine-specific Compilation, Artifact, and Result identities remain unequal
when Engine or Exporter Identity differs. Cross-engine compatibility uses
separate canonical comparison values.

### Portable Diagnostic Envelope

```cddl
portable-diagnostic-kind =
  [class: 0, stable-library-kind: schema-1-stable-diagnostic-kind] /
  [class: 1]

portable-diagnostic = [
  phase: 0 / 1,
  severity: 0 / 1,
  kind: portable-diagnostic-kind,
  spans: [* diagnostic-span],
]

portable-diagnostic-envelope = [
  policy: canonical-diagnostic-policy,
  retained: [* portable-diagnostic],
  completion: diagnostic-completion,
]
```

Implementation-specific kind detail, message, and hints are absent. Stable
library kinds remain exact.

The portable envelope projects the already bounded Canonical Diagnostic
Envelope; it does not rerun diagnostic retention under another byte policy.
Message or hint length can therefore indirectly change which complete entries
survive and the Limited completion record even though wording is absent from a
portable entry. Structurally Compatible requires equality of that projected
retained structure and completion. This preserves the accepted requirement that
Canonical Diagnostic Policy has one semantic result, rather than inventing a
second cross-engine diagnostic budget.

### Structural and Exact Projections

```cddl
artifact-role-object = [artifact-role, identity-object]

structural-engine-neutral-result = [
  status: 0 / 1,
  document: compilation-document-summary,
  diagnostics: portable-diagnostic-envelope,
  dependencies: semantic-dependency-projection,
  artifact-roles: [* artifact-role],
]

exact-engine-neutral-result = [
  status: 0 / 1,
  document: compilation-document-summary,
  diagnostics: canonical-diagnostic-envelope,
  dependencies: semantic-dependency-projection,
  artifacts: [* artifact-role-object],
]
```

These projections have no schema-1 identity kinds. They are comparison subjects
inside a Cross-Engine Compatibility Claim.

- Request Compatible compares Engine-Neutral Compilation Intent Identity.
- Closure Compatible additionally requires the target to reach a semantic
  result using only the Pack contract and exact declared fulfillments.
- Structurally Compatible compares `structural-engine-neutral-result`.
- Exactly Reproducible compares `exact-engine-neutral-result`.

## Validation and Collision Rules

Every implementation MUST enforce:

1. Every identity is derived or rederived whenever its preimage is available at
   the seam that creates or admits it.
2. Every nested identity has the exact required kind, schema `1`, and algorithm
   `1`.
3. Inputs, features, overrides, standards, page ranges, observations, and
   artifacts are canonical, sorted, and duplicate-free.
4. Target, effective HTML feature, output format, and Exporter Identity format
   agree.
5. PPI is one positive finite binary32 value encoded as four big-endian bytes.
6. Each commitment agrees with Pack Identity, role, key or path, exact size,
   and active raw value.
7. Result diagnostic policy exactly equals the intent policy.
8. The diagnostic envelope is the maximal fitting prefix and its completion
   record names the actual first omitted entry and winning limit.
9. Result status, intent target, document summary, artifact cardinality, roles,
   order, exact objects, and identities are coherent.
10. Every dependency observation satisfies the Pack, intent, trace-projection,
    disposition, and diagnostic-span referential invariants above.
11. Semantic dependency projection contains no evidence key, backing locator,
     cache, timing, retry, or acquisition-provenance field.
12. A cache hit returns the original semantic result, complete trace,
     diagnostics, projection, artifacts, and identities without reconstruction
     from current telemetry.
13. Every SHA-256 payload obeys the `2^61 - 37` byte applicability ceiling.
14. Cross-engine projections contain no engine-specific Compilation, Artifact,
     or Result identity.

Identity verification has explicit context:

- At semantic creation, exact bytes or a canonical structured payload are the
  preimage and the implementation MUST derive the identity rather than accept a
  caller digest.
- A nested identity may be consumed from an already validated opaque Pack,
  Prepared Compilation, Compilation Result, or Stable Byte Value without
  reopening private state solely to rederive it.
- Representation ingress and cache admission rederive every identity whose
  preimage is carried by that representation or record.
- Compilation cache lookup rederives active Compilation Request Commitments
  from the current raw request, then rederives the intent and Compilation
  Identity before comparing a candidate. A cache record never needs to persist
  commitment preimages.
- A preimage deliberately unavailable at a later seam is verified by its
  validated owner and typed context, not by trusting an untyped digest.

If equal typed identities are observed with unequal available canonical payloads
or exact bytes, the operation stops with internal integrity failure. It MUST NOT
choose a value, return a cache miss, retry under another role, fall back to
baseline bytes, or treat the difference as ordinary invalidation. Cross-kind or
cross-schema equal digest bytes are not a collision because typed identities
differ.

Collision behavior is tested with an injected digest oracle. No test claims a
fabricated SHA-256 collision.

An unassigned nonzero standalone kind, schema, or algorithm at a
`generic-identity` carrier is unsupported. Reserved zero values are invalid. A
wrong or unknown tuple in a known schema-1 position is invalid. Unknown enum
values are invalid. Implementations MUST NOT approximately interpret a newer
schema or select from a version range.

## Golden Vector Set A

This vector chain exercises every new kind. It intentionally uses two
well-typed synthetic fixture identities so it can test compilation encoders
without embedding a complete Pack and Engine vector:

```text
Pack Identity fixture:
  [11, 1, 1, h'0000000000000000000000000000000000000000000000000000000000000000']

Engine Identity fixture:
  [10, 1, 1, h'1111111111111111111111111111111111111111111111111111111111111111']
```

Set A is an encoder and transcript vector, not a semantic-validity vector. A
semantic constructor MUST NOT accept either synthetic tuple as a validated Pack
or Engine without its preimage and owning validation. The interoperability
corpus additionally requires a fully recursive chain rooted in real accepted
Epoch 2 Pack and Engine vectors.

The actual Exact Content Identity for empty bytes is:

```text
typst-pack:exact-content:1:sha256:
952f774f5c8fa28427b28972d7d44841d62a679f3a9f5b7f62536f728b1a033d
```

Its complete wire tuple is:

```text
840101015820952f774f5c8fa28427b28972d7d44841d62a679f3a9f5b7f62536f728b1a033d
```

Hex blocks below permit ASCII whitespace for display. Remove whitespace before
decoding.

### Kind 14: Compilation Request Commitment

Abstract value: Typst input key `x`, value `y`, bound to the fixture Pack.

```text
payload:
8501840b0101582000000000000000000000000000000000000000000000000000000000000000006178014179

transcript:
74797073742d7061636b206964656e7469747900000000010000000e000000000000002d
8501840b0101582000000000000000000000000000000000000000000000000000000000000000006178014179

digest:
026a262a8685b8f8725e2e4ca865b2d02edca6040212035ffe202f799ab75be0

identity wire tuple:
840e01015820026a262a8685b8f8725e2e4ca865b2d02edca6040212035ffe202f799ab75be0
```

Human rendering:

```text
typst-pack:compilation-request-commitment:1:sha256:026a262a8685b8f8725e2e4ca865b2d02edca6040212035ffe202f799ab75be0
```

### Kind 15: Exporter Identity

Abstract value:

```text
[
  "org.typst-pack.test",
  "fixture-exporter",
  [0, 15, 0],
  h'',
  "fixture-target",
  [],
  0,
]
```

```text
payload:
87736f72672e74797073742d7061636b2e7465737470666978747572652d6578706f7274657283000f00406e666978747572652d7461726765748000

transcript:
74797073742d7061636b206964656e7469747900000000010000000f000000000000003c
87736f72672e74797073742d7061636b2e7465737470666978747572652d6578706f7274657283000f00406e666978747572652d7461726765748000

digest:
be8ebf1d2b11e8864a05ba28afed620c477fd11e3037b9c748a42cde5273bfd8

identity wire tuple:
840f01015820be8ebf1d2b11e8864a05ba28afed620c477fd11e3037b9c748a42cde5273bfd8
```

### Kind 16: Engine-Neutral Compilation Intent

Abstract value: fixture Pack, paged target, input `x=y`, absent document time,
no features, no overrides, default PDF controls, and diagnostic policy `[1,0,0]`.

```text
payload:
88840b010158200000000000000000000000000000000000000000000000000000000000000000
00818261788201840e01015820026a262a8685b8f8725e2e4ca865b2d02edca6040212035ffe202f799ab75be0
f680808800810081008100f68000f483010000

transcript:
74797073742d7061636b206964656e746974790000000001000000100000000000000067
88840b010158200000000000000000000000000000000000000000000000000000000000000000
00818261788201840e01015820026a262a8685b8f8725e2e4ca865b2d02edca6040212035ffe202f799ab75be0
f680808800810081008100f68000f483010000

digest:
4bae55d3da834fdb5d0f34d0cc9346b36302344be8f9ab8440d714903f5a2398

identity wire tuple:
8410010158204bae55d3da834fdb5d0f34d0cc9346b36302344be8f9ab8440d714903f5a2398
```

### Kind 17: Compilation Identity

Abstract value: the intent above, fixture Engine, and fixture Exporter.

```text
payload:
838410010158204bae55d3da834fdb5d0f34d0cc9346b36302344be8f9ab8440d714903f5a2398
840a010158201111111111111111111111111111111111111111111111111111111111111111
840f01015820be8ebf1d2b11e8864a05ba28afed620c477fd11e3037b9c748a42cde5273bfd8

transcript:
74797073742d7061636b206964656e746974790000000001000000110000000000000073
838410010158204bae55d3da834fdb5d0f34d0cc9346b36302344be8f9ab8440d714903f5a2398
840a010158201111111111111111111111111111111111111111111111111111111111111111
840f01015820be8ebf1d2b11e8864a05ba28afed620c477fd11e3037b9c748a42cde5273bfd8

digest:
20cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80

identity wire tuple:
84110101582020cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80
```

### Kind 18: Compilation Artifact Identity

Abstract value: the Compilation Identity above, PDF document role `[0,null]`,
and a zero-byte object with the actual empty Content Identity.

```text
payload:
8384110101582020cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80
8200f6
8200840101015820952f774f5c8fa28427b28972d7d44841d62a679f3a9f5b7f62536f728b1a033d

transcript:
74797073742d7061636b206964656e746974790000000001000000120000000000000052
8384110101582020cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80
8200f6
8200840101015820952f774f5c8fa28427b28972d7d44841d62a679f3a9f5b7f62536f728b1a033d

digest:
b8b07ec7f1a12e9e1443b3dc0e01127f65ba1a443d14ea52c707354903f54f34

identity wire tuple:
841201015820b8b07ec7f1a12e9e1443b3dc0e01127f65ba1a443d14ea52c707354903f54f34
```

### Kind 19: Compilation Result Identity

Abstract value: succeeded PDF result, one source page, complete empty diagnostic
envelope under policy `[1,0,0]`, empty semantic dependency projection, and the
single artifact above.

```text
payload:
8684110101582020cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80
00
820001
8383010000808100
83808080
81841201015820b8b07ec7f1a12e9e1443b3dc0e01127f65ba1a443d14ea52c707354903f54f34

transcript:
74797073742d7061636b206964656e74697479000000000100000013000000000000005e
8684110101582020cdb9481c3ba07d89e42e460d18816f4a4316bc44c30ffcdd374d208be04a80
00
820001
8383010000808100
83808080
81841201015820b8b07ec7f1a12e9e1443b3dc0e01127f65ba1a443d14ea52c707354903f54f34

digest:
6cfb4659c9fc25e0531aceaa2a589996d4c8baae618d41f0cb4f248b143f366b

identity wire tuple:
8413010158206cfb4659c9fc25e0531aceaa2a589996d4c8baae618d41f0cb4f248b143f366b
```

The six new digests were derived with a small independent deterministic-CBOR
encoder and independently checked by piping each transcript through OpenSSL
SHA-256. A production implementation and corpus generator MUST independently
reproduce these bytes; the production identity implementation cannot be the
sole oracle.

## Interoperability Corpus Contract

The implementation corpus MUST publish, for every positive vector:

```text
abstract-value.json
payload.bin
transcript.bin
digest.hex
wire-tuple.cbor
expected.json
```

Golden bytes are immutable. New vectors may be added, but changing an accepted
payload, transcript, digest, enum meaning, or identity tuple requires explicit
schema and compatibility review.

The corpus MUST cover:

- every new kind and every closed enum branch;
- SHA-256 payload-length admission at `2^61 - 38`, `2^61 - 37`, and
  `2^61 - 36` through pure checked length arithmetic or an explicitly
  non-wire injected digest oracle;
- every `u32` and `u63` boundary used by a compilation value;
- reserved-zero invalidity versus unassigned-nonzero generic unsupported state;
- Compilation versus Discovery commitment separation;
- equal raw values under different Packs, roles, keys, and paths;
- empty, NUL-containing, non-ASCII, NFC, NFD, and arbitrary invalid-UTF-8
  override values;
- unused inputs, features, overrides, and byte-identical overrides;
- declaration reordering that canonicalizes to equal identities;
- explicit and absent times, negative zero rejection, and engine-range edges;
- every format, PDF mode, standard, tagging mode, and boolean control;
- PPI decimal aliases, adjacent binary32 values, the smallest positive
  subnormal, zero, underflow-to-zero, infinity, and NaN;
- page-range overlap, adjacency, duplicates, reordering, open ends, all-pages
  normalization, `u32` bounds, and a selection matching no pages;
- complete empty, warning, and error diagnostic envelopes;
- the exact registered PDF tagging warning entry, trigger, non-trigger cases,
  complete envelope, and Limited envelope bytes printed above;
- count-limited, byte-limited, simultaneous-limit, zero-budget, exact-boundary,
  and first-entry-too-large envelopes;
- a rejected result whose identifying error is omitted and a successful result
  whose warning is omitted;
- different omitted suffixes producing equal identities when retained prefix
  and Limited record are equal;
- baseline, override, missing, invalid-source, embedded, external, and font
  observations;
- undeclared-package observation;
- baseline use or baseline fallback while an Override is active, and read versus
  invalid-as-source tag inversion;
- an undeclared-package rejected result and invalid successful or artifact-owning
  variants of that result;
- wrong Pack project object, undeclared override, wrong override commitment,
  unknown Package Requirement, wrong package object, false missing outcome,
  wrong fulfillment, undeclared font face, and trace/projection disagreement;
- observation reordering and repeat-count exclusion;
- evidence-table subject and authority ordering, reference resolution,
  duplicate references, wrong-subject references, reached-scope states, and
  safe projection removal of opaque keys and authority instances;
- unused externally fulfilled package and font requirements with requirement-
  level evidence but no kernel observation;
- external package read, missing, and invalid-source observations whose causal
  references join requirement-level dependency-resolution evidence to
  path-level kernel evidence;
- omitted selected evidence, omitted higher-priority misses, incomplete
  source-choice evidence, and causally irrelevant extra references;
- undeclared-package Pack membership absence with proof that Package Authority
  and cache adapters are not called;
- evidence-key, authority, backing-location, cache, retry, and timing mutations
  leaving Result Identity unchanged;
- compiler rejection without a page count and exporter rejection with a page
  count;
- compiler-rejected result trace scope at kernel phase and successful/exporter-
  rejected scope at export phase;
- strict UTF-8 success and failure, one stripped leading BOM, a second retained
  BOM, BOM-only source, multibyte span boundaries, and unconditional
  out-of-bounds span rejection independent of source-bundle policy;
- PDF and HTML single-artifact success, PNG and SVG multi-artifact success, and
  zero-artifact Page Format success;
- exporter completion order canonicalizing to Source Page Number order;
- equal bytes under different artifact roles or Compilation Identities;
- an operation outcome with a partial trace and no Result Identity;
- fresh execution and semantic-cache hit preserving the original result;
- cache-hit preservation of full original trace reference ordinals without
  rebinding them to fresh evidence;
- equal intent under different Engine or Exporter identities, yielding unequal
  Compilation, Artifact, and Result identities;
- structural comparison excluding wording, hints, implementation kind detail,
  and artifact bytes while retaining any wording-induced canonical limitation
  boundary;
- exact engine-neutral comparison despite unequal engine-specific identities;
- malformed or noncanonical CBOR, forbidden floats, wrong arity, unknown enum,
  unsorted sets, duplicate members, wrong nested identity kinds, wrong schemas,
  stale digests, and length mismatches; and
- injected typed-identity collision behavior.

The corpus MUST also include one fully recursively verifiable positive chain
from real Epoch 2 Content, Engine, project, dependency, and Pack payloads through
all six compilation kinds. Synthetic Set A remains an encoder oracle only.

At least two independent derivations MUST pass the complete corpus. At least one
must not call the production serializer or identity implementation; a small
non-Rust reference implementation is preferred.

The payload-length boundary cases are virtual admission tests and are exempt
from `payload.bin`, `transcript.bin`, and actual SHA-256 execution. Positive
golden vectors remain fully materialized and reasonably sized; no corpus gate
requires a multi-exabyte file or digest computation.

## Reconciliations

This contract applies the later accepted decisions where earlier artifacts
disagree:

- Safe Pack Override projections follow
  [Reconcile terminal reporting and bounded diagnostics](https://github.com/sagikazarmark/typst-pack/issues/59),
  not the earlier baseline-identity and equality disclosure in
  [Define compilation-scoped project variation](https://github.com/sagikazarmark/typst-pack/issues/41).
- Canonical diagnostic limitation is semantic and identity-bearing. Diagnostic
  Projection and Diagnostic Source Bundle limits remain operational.
- Request rejection has no report or Compilation Identity.
- PNG PPI is binary32, matching the frozen first-party adapter contract; the
  earlier Rust fixture's `f64` is not a semantic value.
- Canonical diagnostics have compiler and exporter phases. Library-generated
  diagnostics occupy registered semantic positions rather than introducing a
  third phase.
- PDF standards preserve the selected set, including the empty default. The
  exporter may derive an internal PDF version, but that implementation detail is
  already scoped by Exporter Identity and does not create a hidden request
  option.
- A paged compiler rejection may lack a page count; every result reached after
  paged document production carries one.
- The adapter's evidence links and provenance fields are public reporting
  values, not the result identity projection.
- The effective SHA-256 applicability ceiling is stricter than the earlier
  Epoch 2 `u63` object ceiling and controls every algorithm-1 identity.

The contract does not reopen Pack identity, Pack format receipts, transport
receipts, archive writer recipes, execution outcomes, resource profile numbers,
or first-party adapter command shape.

## Public Interface Consequences

[Complete the Rust lifecycle and receipt interfaces](https://github.com/sagikazarmark/typst-pack/issues/65)
can now expose validated opaque values without choosing semantics:

- `CompilationRequestCommitment`, `ExporterIdentity`,
  `EngineNeutralCompilationIntentIdentity`, `CompilationIdentity`,
  `CompilationArtifactIdentity`, and `CompilationResultIdentity` represent the
  exact typed tuples here.
- PNG construction accepts or derives one validated binary32 PPI value rather
  than retaining `f64` as semantic state.
- Artifact views expose both Compilation Artifact Identity and exact Content
  Identity.
- Structured diagnostic and access-trace views expose the exact canonical
  variants here.
- The feature interface uses an opaque validated identifier and preparation
  applies the exact first-release support rules here.
- Request-rejection inventory, the accepted semantic inventory, attempt
  operational inventory, report-level partial trace, and disclosure views
  remain public but non-identifying.
- Result ownership and external disclosure remain distinct: complete semantic
  access is available in-process, while rendering and delivery require the
  independently gated disclosure projector.
- Cache codecs rederive identities with available preimages, compare active
  request commitments during lookup, and preserve original result and trace
  values on a hit.

The first-party adapter reconciliation can serialize these values into its
stable JSON and GraphQL projections without hashing adapter documents or
reconstructing semantic state.

That reconciliation MUST revise, rather than copy into the Rust core, the
accepted prototype's stale projection shapes:

- its third `library` diagnostic phase becomes the registered two-phase
  placement above;
- string diagnostic kinds become stable or implementation-specific tagged
  kinds;
- path-only spans gain typed project provenance or Package Requirement
  locations;
- generic dependency and access observations become the typed project,
  package, and font variants above;
- evidence references and acquisition provenance stay non-identifying sibling
  reporting data; and
- artifact ordinal and presentation name remain adapter fields outside
  Compilation Artifact Identity.

## Alternatives Rejected

| Alternative | Rejection reason |
| --- | --- |
| A second overlapping compilation registry | Typed equality and human rendering require globally unique kind numbers. |
| A distant numeric block beginning at `65536` | It adds allocation policy and larger common encodings without a concrete compatibility benefit; kinds `14` through `19` are free and already globally scoped. |
| A separate commitment prefix or HMAC | Unique kind supplies domain separation; keys or salts would break global deterministic reproduction. |
| Replacement Content Identity in safe projections | It leaks cross-role equality and conflicts with the accepted commitment model. |
| Decimal or CBOR floating-point PPI | Alternate spellings and encoder choices break cross-language equality. |
| Binary64 PPI | The accepted first-party semantic parser rounds once to binary32 and the embedded exporter consumes that precision. |
| RFC 3339 or lexical time strings | Equivalent spellings and offsets introduce aliases; accepted semantics use whole Unix seconds. |
| Effective PDF version in place of selected standards | It invents a request value not present in the accepted output specification and can erase meaningful standard selections. |
| Hashing request inventories or adapter JSON | Origins, operational controls, disclosure, and schema presentation are intentionally non-semantic. |
| A third diagnostic phase for library diagnostics | Accepted semantics require compiler diagnostics before exporter diagnostics with library-generated entries at fixed positions. |
| Evidence references in Result Identity | They make equal semantic results depend on backing source and current-attempt operation state. |
| Bare nested digests | They permit cross-kind and cross-schema substitution. |
| Compatibility aliases for draft identities | No compilation-family identities have shipped; aliases would create two truths before the first release. |

## Planning Consequence

This registry supplies the canonical compilation value model that the Epoch 2,
Rust, cache, session, CLI, Dagger, and cross-engine contracts share. It closes
the identity blocker without changing the seven-module architecture or creating
a new public generic seam.

No new decision ticket or in-scope fog is surfaced. Private type layout, helper
decomposition, serializer choice, cache record encoding, corpus materialization,
and production test implementation remain implementation work.
