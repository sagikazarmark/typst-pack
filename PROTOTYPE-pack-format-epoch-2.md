# PROTOTYPE: Pack Format Epoch 2 Normative Contract

> Throwaway decision artifact for [Freeze Pack Format Epoch 2 normative contract](https://github.com/sagikazarmark/typst-pack/issues/57). It is not production documentation and must not be merged as-is.

## Status

**Accepted recommendation: use this contract as the Pack Format Epoch 2 design baseline.**

This document closes the implementation-blocking gaps identified by [Audit architecture synthesis for implementation readiness](https://github.com/sagikazarmark/typst-pack/issues/44): concrete schema and registries, exact identity transcripts, deterministic validation, strict ZIP and Closure Export profiles, receipt semantics, and the required independent interoperability corpus.

It incorporates the later correction in [Reconcile Pack creation evidence and coverage semantics](https://github.com/sagikazarmark/typst-pack/issues/60): project closure contains every successfully observed project path regardless of baseline or discovery-override provenance, and always contains baseline bytes for that path.

The contract also narrows Archive Encoding Identity. It is an encoder-side recipe attestation. Generic ingress cannot infer it from archive bytes.

## Normative Scope

The words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, and MAY have their RFC 2119 meanings.

Epoch 2 normatively depends on:

- RFC 8949 Section 4.2.1 for core deterministic CBOR encoding;
- RFC 8610 for CDDL notation;
- FIPS 180-4 for SHA-256;
- PKWARE APPNOTE 6.3.10 for ZIP framing and ZIP64 fields; and
- RFC 1951 for raw Deflate streams.

When this profile is narrower than one of those specifications, this profile controls. A conforming implementation MUST NOT accept a broader encoding and silently rewrite it into this profile.

This document freezes logical and byte-level interoperability. It does not require a particular parser library, in-memory representation, Rust type layout, buffering strategy, concurrency strategy, or publication transport encoding.

Epoch 1 TOML/ZIP data, field aliases, Resource Slots, and parser fallback are not part of Epoch 2. An Epoch 2 reader MUST NOT interpret historical data as Epoch 2 and MUST NOT try multiple schemas until one succeeds.

## Representation Model

Pack Archive and Closure Export are strict representations of one logical Pack. Both carry:

```text
typst-pack/pack.cbor
typst-pack/blobs/sha256/<64 lowercase hexadecimal digits>
```

`pack.cbor` is one canonical Pack Control Record. Blob files contain decoded exact bytes. Logical project and package paths occur only in the control record.

The physical blob set is exactly the deduplicated union of:

- every contained baseline project file;
- every file in an embedded Complete Package Tree;
- every embedded Font Container; and
- every object required by an understood Pack Semantic Extension.

An external Package Requirement or Font Requirement carries complete exact descriptors but does not require its bytes. If the same digest is physically present for another embedded role, that coincidence does not change the external disposition.

No directory entry, host path, permission, owner, timestamp, link, inode, compression choice, or publication location is logical Pack state.

## Canonical CBOR Profile

The Pack Control Record MUST be one RFC 8949 Section 4.2.1 core deterministic CBOR data item with no trailing bytes.

The Epoch 2 profile permits only:

- unsigned integers;
- byte strings;
- valid UTF-8 text strings;
- arrays;
- maps;
- booleans; and
- `null` only where the schema explicitly permits it.

It forbids:

- negative integers;
- floating-point values;
- tags and bignums;
- indefinite-length values;
- `undefined` and unassigned simple values;
- CBOR sequences; and
- non-shortest integer or length encodings.

Every map, array, byte string, and text string has definite length. Maps use RFC 8949 Section 4.2.1 bytewise lexicographic ordering of each key's deterministic encoded bytes. Duplicate encoded map keys are invalid. The corpus includes a map whose Section 4.2.1 bytewise order differs from Section 4.2.3 length-first order and accepts only the Section 4.2.1 form.

Core record maps have only fixed unsigned-integer keys. Every key listed for a record is mandatory. Unknown keys and omitted keys are invalid. Empty arrays represent empty collections. Optional values use explicit `null`; a reader never supplies a missing default.

Text is preserved as exact UTF-8. A reader MUST NOT normalize, trim, case-fold, or otherwise rewrite it.

Arrays identified below as sets MUST already use their field-specific canonical order and MUST contain no duplicate semantic member. Discovery Variant declaration order, Pack Font Catalog order, metadata author order, and license-name-record order are intentional lists rather than sets.

CBOR nesting counts the top-level map as depth 1. Entering any contained array or map increments depth by one. Scalars do not increment it. Depth 32 is valid; depth 33 is invalid.

### Permanent Epoch Dispatch Prelude

Every Pack Control Record epoch MUST remain a definite-length CBOR map whose first canonical pair is unsigned key `0` and an unsigned epoch number. A dispatcher validates enough CBOR framing to read that pair without applying an epoch-local schema.

- Missing key `0`, a non-unsigned epoch, an indefinite map, or malformed framing is invalid.
- A well-formed unimplemented epoch is unsupported.
- Epoch 2 then parses the entire item under this closed schema.

## Pack Control Record Schema

The CDDL below defines wire shape. The prose constraints and registries following it are equally normative.

```cddl
pack-control-record = {
  0: 2,                              ; Pack Format Epoch
  1: identity,                       ; claimed Pack Identity
  2: engine,
  3: [+ discovery-variant],          ; declaration order
  4: path,                           ; fixed entrypoint
  5: tree,                           ; baseline project tree
  6: [* path],                       ; effective explicit inclusions
  7: [* package-requirement],
  8: [* font-requirement],
  9: [* font-face-identity],          ; intentional catalog order
  10: pack-metadata,
  11: [* semantic-extension],
  12: [* annotation],
}

identity = [
  kind: uint .le 4294967295,
  schema: uint .le 4294967295,
  algorithm: 1,
  digest: bstr .size 32,
]

object = {
  0: uint .le 9223372036854775807,    ; exact byte length
  1: identity,                       ; exact Content Identity
}

file = {
  0: path,
  1: object,
}

tree = {
  0: identity,                       ; claimed tree identity
  1: uint .le 4294967295,            ; file count
  2: uint .le 9223372036854775807,    ; aggregate bytes
  3: [* file],
}

engine = {
  0: identity,
  1: engine-descriptor,
}

engine-descriptor = {
  0: namespaced-id,                  ; producer
  1: nonempty-text,                  ; implementation name
  2: version,
  3: bstr,                           ; producer-defined exact build fingerprint
  4: nonempty-text,                  ; target/profile identity
  5: [* qualifier],
  6: version,                        ; Unicode XID table version
  7: namespaced-id,                  ; package metadata profile
  8: namespaced-id,                  ; font metadata profile
}

qualifier = {
  0: nonempty-text,
  1: nonempty-text,
}

version = {
  0: uint .le 4294967295,
  1: uint .le 4294967295,
  2: uint .le 4294967295,
}

version-bound = {
  0: uint .le 4294967295,
  1: uint .le 4294967295 / null,
  2: uint .le 4294967295 / null,
}

discovery-variant = {
  0: nonempty-text / null,            ; public label
  1: discovery-coverage-request,
  2: identity,                        ; Discovery Variant Identity
  3: discovery-trace,
  4: identity,                        ; Discovery Trace Identity
  5: identity,                        ; Discovery Coverage Identity
}

discovery-coverage-request = {
  0: target,
  1: [* discovery-input],
  2: document-time / null,
  3: [* feature-id],
  4: [* discovery-override],
}

discovery-input = {
  0: nonempty-text,
  1: sensitive-value,
}

discovery-override = {
  0: path,
  1: sensitive-value,
}

sensitive-value = {
  0: uint .le 9223372036854775807,
  1: identity,                        ; Discovery Request Commitment
}

document-time = {
  0: 0 / 1,                           ; nonnegative / negative
  1: uint .le 9223372036854775807,    ; absolute Unix seconds
}

discovery-trace = {
  0: [* project-observation],
  1: [* package-observation],
  2: [* font-face-identity],
}

project-observation = baseline-read / override-read / project-missing

baseline-read = {
  0: 0,
  1: path,
  2: request-kind,
  3: object,
}

override-read = {
  0: 1,
  1: path,
  2: request-kind,
  3: sensitive-value,
}

project-missing = {
  0: 2,
  1: path,
  2: request-kind,
}

package-observation = package-read / package-missing

package-read = {
  0: 0,
  1: identity,                        ; Package Requirement Identity
  2: path,                            ; package-relative path
  3: request-kind,
  4: object,
}

package-missing = {
  0: 1,
  1: identity,
  2: path,
  3: request-kind,
}

package-requirement = {
  0: identity,
  1: package-spec,
  2: tree,
  3: package-manifest-summary,
  4: disposition,
  5: provenance,
}

package-spec = {
  0: nonempty-text,                   ; namespace
  1: nonempty-text,                   ; name
  2: version,
}

package-manifest-summary = {
  0: nonempty-text,                   ; name
  1: version,
  2: path,                            ; package entrypoint
  3: version-bound / null,            ; minimum compiler version
}

font-requirement = {
  0: identity,
  1: object,                          ; whole Font Container
  2: [+ font-face-descriptor],
  3: disposition,
  4: provenance,
}

font-face-descriptor = {
  0: font-face-identity,
  1: font-selection-metadata,
  2: font-licensing-metadata,
}

font-face-identity = [
  container-content-identity: identity,
  container-local-index: uint .le 4294967295,
]

font-selection-metadata = {
  0: nonempty-text,                   ; family
  1: font-style,
  2: 100..900,                        ; weight
  3: 500..2000,                       ; thousandths
  4: uint .le 15,                     ; registered flags
  5: [* font-axis],
  6: [* codepoint-range],
}

font-axis = {
  0: bstr .size 4,                    ; OpenType tag
  1: bstr .size 4,                    ; min IEEE-754 binary32 bits
  2: bstr .size 4,                    ; default bits
  3: bstr .size 4,                    ; max bits
}

codepoint-range = [
  first: uint .le 1114111,
  last: uint .le 1114111,
]

font-licensing-metadata = {
  0: uint .le 65535 / null,           ; OpenType OS/2 fsType
  1: [* license-name-record],
}

license-name-record = {
  0: 13 / 14,                         ; OpenType name ID
  1: uint .le 65535,                  ; platform ID
  2: uint .le 65535,                  ; encoding ID
  3: uint .le 65535,                  ; language ID
  4: bstr,                            ; exact name-record bytes
}

provenance = {
  0: namespaced-id,                   ; stable public authority kind
  1: source-class,
  2: namespaced-id / null,             ; non-secret logical origin
}

pack-metadata = {
  0: nonempty-text / null,            ; title
  1: nonempty-text / null,            ; description
  2: [* nonempty-text],               ; author presentation order
  3: [* nonempty-text],               ; sorted keyword set
}

semantic-extension = {
  0: semantic-extension-id,
  1: 1..4294967295,
  2: bstr,                            ; extension-defined canonical payload
  3: [* object],                      ; embedded required objects
}

annotation = {
  0: annotation-id,
  1: 1..4294967295,
  2: bstr,                            ; opaque exact payload
}

target = 0 / 1
request-kind = 0 / 1
disposition = 0 / 1
font-style = 0 / 1 / 2
source-class = 0 / 1 / 2 / 3 / 4 / 5

path = nonempty-text
feature-id = nonempty-text
namespaced-id = nonempty-text
semantic-extension-id = nonempty-text
annotation-id = nonempty-text
nonempty-text = tstr .size (1..4294967295)
```

CDDL controls above define shape; semantic constraints that CDDL cannot express remain in the prose below.

## Field Semantics

### Canonical Paths

A canonical project or package path is a nonempty UTF-8 text string with `/` separators and all of these properties:

- no leading or trailing `/`;
- no empty, `.` or `..` segment;
- no NUL byte `0x00`, ASCII colon `0x3a`, or backslash `0x5c` anywhere;
- at most 65,535 UTF-8 bytes total;
- at most 1,024 segments; and
- at most 4,096 UTF-8 bytes per segment.

Path equality and order compare exact UTF-8 bytes. Trees reject duplicate paths and file-versus-descendant conflicts. Destination platform incompatibility is a projection failure; it never permits renaming or path rewriting.

These byte rules reject `/` roots, `//` roots, Windows drive forms such as `C:foo`, backslash UNC/device forms, and URI schemes without platform-dependent recognition. Basenames reserved only by a destination platform, such as a Windows device name, remain canonical logical paths and may cause that destination projection to fail.

### Engine Descriptor

The producer and profile fields are lowercase reverse-DNS identifiers. The exact build fingerprint and qualifiers are producer-attested opaque values. Qualifiers are a set sorted by `(name UTF-8 bytes, value UTF-8 bytes)` and names are unique.

The descriptor MUST distinguish every build, backend, target, enabled engine facility, or platform class for which the producer does not promise exact behavior. The Engine Identity is rederived from the descriptor. A caller cannot replace the engine's attested descriptor.

The declared Unicode version fixes the XID_Start and XID_Continue tables used to validate Package Specifications. A package namespace and name each contain one or more Unicode scalar values. The first is XID_Start or `_`; each later value is XID_Continue, `_`, or `-`. No normalization occurs. A package version is exactly three unsigned 32-bit components.

Package and font metadata profiles identify exact byte-to-metadata derivation contracts. A reader claiming support for an Engine Descriptor MUST support both profiles; an otherwise well-formed Pack using an unknown profile is unsupported. Profiles are independent of producer implementation language.

The initial Typst 0.15 profile identifiers are:

- `org.typst.typst-0-15.package-metadata`; and
- `org.typst.typst-0-15.font-metadata`.

The package profile uses TOML 1.0 UTF-8 parsing as accepted by Typst tag `v0.15.0` with `toml` 0.8.19, the Typst 0.15 `PackageManifest` field model, and `unicode-ident` 1.0.16. Its required Engine Descriptor Unicode version is 16.0.0. `package.name`, `package.version`, and `package.entrypoint` are required. `package.compiler` is optional. Unknown package-manifest fields are preserved in the Complete Package Tree bytes but do not enter the summary. A Version Bound with absent minor and present patch is invalid. Compatibility compares the Engine Descriptor version lexicographically against every component present in the bound and requires the engine to be greater than or equal to that prefix. Package entrypoint text is validated as one canonical package path.

Every registered package profile declares exactly one Unicode XID table version. A descriptor/profile mismatch is invalid; a well-formed unimplemented registered profile or Unicode table is unsupported.

The font profile is the byte-for-byte metadata behavior of Typst tag `v0.15.0` `FontInfo::from_ttf` with `ttf-parser` 0.25.1, including its family-name exception table, font variant derivation, supported variation axes, flags, and Unicode cmap coverage. It additionally reads OpenType OS/2 `fsType` unchanged when present and preserves exact Name table records with IDs 13 and 14. The profile's upstream source files and exception data are normative registry assets and are content-identified in the interoperability corpus. This pins behavior rather than requiring a Rust implementation. A port in another language conforms when it reproduces the same metadata vectors.

### Discovery Coverage Request

The request contains exactly:

- effective target;
- complete Typst input map;
- exact or absent Compilation Document Time;
- complete effective engine-feature set; and
- complete discovery-only project override set.

Target values are:

| Value | Target |
| ---: | --- |
| `0` | paged |
| `1` | html |

Feature identifiers are lowercase ASCII strings matching `[a-z][a-z0-9-]{0,63}`. They are sorted by exact bytes and unique. This avoids changing the Pack Format Epoch whenever an Engine Identity adds a feature while still making every effective feature explicit. Epoch 2 validates syntax and identity contribution; the exact Engine profile owns feature support, and successful creation guarantees that every recorded feature was accepted by that Engine.

Inputs are sorted by input-key UTF-8 bytes and have unique keys. Overrides are sorted by canonical path and have unique paths. The raw values do not appear in a Pack.

Document-time sign `0` means nonnegative and sign `1` means negative. Negative zero is invalid. The magnitude is exact whole Unix seconds. Subsecond values are not part of the current semantic contract.

The request excludes output format beyond its derived target, page selection, exporter controls, PDF Creation Time, Engine Identity, Exporter Identity, authority and cache choice, trust, resources, timing, isolation, delivery, labels, and declaration order.

A later compilation matches coverage only by exact typed Discovery Variant Identity equality. The result is `Matched`, identifying the unique Discovery Variant and Discovery Coverage Identity, or `Unmatched`. There is no partial, nearest, subset, or commitment-only match. Matching never gates preparation or execution. It proves only that the exact source-evaluation request was replayed as closed under the recorded discovery Engine; it does not promise equal artifacts or diagnostics, authorize undeclared dependencies, establish external requirement availability, claim compatibility under another Engine, or establish Session Currentness.

### Discovery Traces

Project observations are sorted by `(path, request kind, observation tag)`. Package observations are sorted by `(Package Requirement Identity, path, request kind, observation tag)`. Used font faces are sorted by `(container Content Identity, face index)`. Each array is a set. Access order and repeat count are never represented.

For one variant, each `(project path, request kind)` has at most one terminal observation. A successful observation MUST be override-read exactly when that variant declares an override for the path and MUST be baseline-read otherwise. A variant cannot record both baseline and override success for one logical request. A package `(requirement, path, request kind)` likewise has at most one terminal observation.

Request-kind values are:

| Value | Kind |
| ---: | --- |
| `0` | Typst source request |
| `1` | raw-file request |

A baseline read's Object Descriptor must equal the project tree binding. An override read carries exact size and a Discovery Request Commitment, never replacement bytes, replacement Content Identity, or a blob reference.

An override-only successful read has this exact effect:

- the trace contains an override-read observation;
- the project tree contains the same path and its baseline bytes;
- the override bytes are absent from the physical blob set unless independently needed as other content;
- the path counts as observed and is not retained as an Explicit Conditional Inclusion; and
- the observation's size and commitment equal that variant's override declaration.

An override target not successfully read by any variant MUST occur in the effective Explicit Conditional Inclusion set. Otherwise the Pack is invalid.

### Project Closure

The project tree is exactly:

```text
all successfully observed project paths
    union
all effective Explicit Conditional Inclusion paths
```

Every member contributes its baseline bytes. A successfully observed path contributes regardless of baseline or discovery-override provenance. Effective inclusions are sorted, unique, and disjoint from observed paths. Creation may warn about a declared overlap, but issuance stores only observed origin for the overlapping path.

The fixed entrypoint is in the project tree and is successfully observed as source by every Discovery Variant, with baseline or override provenance matching that variant's request.

### Complete Package Trees

Package Requirements are sorted by `(namespace UTF-8 bytes, name UTF-8 bytes, major, minor, patch)` and each Package Specification occurs once.

A Complete Package Tree contains every addressable regular file below the acquired package root. Its files are sorted by canonical package-relative path. It contains a canonical `typst.toml`, and the summary records only the fields required for Pack validity:

- package name;
- package version;
- canonical package entrypoint; and
- optional minimum compiler version.

The summary name and version equal the Package Specification. The entrypoint exists in the tree. The active discovery Engine version satisfies the optional bound.

For external disposition, ingress can rederive the tree identity, count, and aggregate length from descriptors but cannot validate unavailable bytes or rederive metadata from them. Such checks are deferred and REQUIRED during External Package Fulfillment before compilation. Deferred metadata MUST NOT be treated as fresh authority or used to skip fulfillment verification.

### Font Requirements

Font Requirements are sorted by whole-container Content Identity. Equal container bytes collapse into one requirement. Face descriptors are a nonempty set sorted by face index. The Font Requirement Identity binds the whole-container descriptor and required face indices; selection and licensing metadata are verified deterministic derivatives.

Font-style values are:

| Value | Style |
| ---: | --- |
| `0` | normal |
| `1` | italic |
| `2` | oblique |

Font flag bits are:

| Bit | Meaning |
| ---: | --- |
| `0` | monospace |
| `1` | serif |
| `2` | contains an OpenType MATH table |
| `3` | variable font |

Unknown flag bits are invalid.

Axes are sorted by their four exact OpenType tag bytes and tags are unique. Axis values are exact big-endian IEEE-754 binary32 bit patterns carried as byte strings, not CBOR floats. Values MUST be finite. Positive zero is the only valid zero encoding. For each axis, `min <= default <= max` under IEEE-754 numeric comparison.

Coverage is a sorted list of inclusive Unicode scalar-value ranges. Each range has `first <= last`; ranges do not overlap or touch. Surrogate code points are excluded.

License records contain only OpenType name IDs 13 and 14 and are ordered by `(name ID, platform ID, encoding ID, language ID, raw bytes)`. They are advisory and make no legal claim.

The Pack Font Catalog contains every required Font Face Identity exactly once, contains no other face, and preserves discovery-relative catalog order. Its order contributes to Pack Identity.

For an external font, ingress validates descriptor consistency but defers byte parsing, face-index existence, selection metadata, and licensing metadata checks until External Font Fulfillment. Exact whole-container verification occurs before a Compilation Dependency Snapshot is built.

### Disposition

Disposition values are:

| Value | Meaning |
| ---: | --- |
| `0` | embedded |
| `1` | externally fulfilled |

Disposition is explicit and contributes to Pack Identity. Embedded content is authoritative and never falls back to an authority. External fulfillment verifies the exact requirement and cannot introduce undeclared content.

### Provenance

Source-class values are:

| Value | Source class |
| ---: | --- |
| `0` | caller-supplied |
| `1` | explicit local source |
| `2` | cache |
| `3` | network |
| `4` | system font source |
| `5` | engine-embedded font source |

Package provenance permits values `0` through `3`; font provenance permits all values. Authority kind and optional logical origin are lowercase reverse-DNS identifiers. The closed shape cannot encode credentials, absolute paths, URLs, object keys, Dependency Evidence Keys, or free-form provider messages.

Provenance is public and non-identifying. It does not contribute to Package Requirement, Font Requirement, or Pack Identity.

### Metadata, Extensions, And Annotations

Metadata keywords are a sorted unique set by exact UTF-8 bytes. Metadata and provenance are excluded from Pack Identity but remain part of exact control-record bytes.

A namespaced identifier contains at least two lowercase DNS-style labels separated by `.`. Each label begins and ends with `[a-z0-9]`, contains only `[a-z0-9-]`, and has at most 63 bytes. The complete identifier has at most 253 bytes. The first label begins with `[a-z]`.

Semantic-extension identifiers end in the label `.sem`; annotation identifiers end in `.ann`. These syntactically disjoint classes make promotion impossible. Registries permanently assign each full identifier and epoch within its class; an assigned meaning is never reused.

Semantic extensions are sorted by `(identifier, extension epoch)` and unique by identifier. Their payload MUST have one extension-defined canonical encoding. Their object descriptors are sorted by Content Identity and unique. Every object is embedded. A reader must understand and validate the exact extension epoch before exposing the Pack. Unknown semantic extensions are unsupported, not ignorable.

Annotations are sorted by `(identifier, annotation epoch)` and unique by identifier. Their payload is bounded opaque bytes. Unknown annotations are accepted and preserved exactly. An annotation cannot affect validity beyond its envelope, compilation, dependencies, guarantees, authorization, or Pack Identity, and cannot reference blobs.

The interoperability corpus reserves these identifiers only for tests:

- `org.typst-pack.test.sem`, epoch 1; and
- `org.typst-pack.test.ann`, epoch 1.

The test semantic extension payload is canonical CBOR `{0: uint}` and its object inventory may contain zero or one Object Descriptor. Production Packs MUST NOT use test identifiers.

## Registries

Registry value `0` is reserved and invalid unless a table explicitly assigns it. Unknown values in a known Epoch 2 core enum are invalid. Every identity position in the Epoch 2 Pack Control Record requires a specific kind, schema 1, and algorithm 1; a different or unknown value there is invalid. Unsupported identity tuples apply only to future explicitly generic interfaces outside this control-record schema.

### Digest Algorithm Registry

| Value | Algorithm |
| ---: | --- |
| `1` | SHA-256 |

### Identity Kind Registry

Each kind begins with schema `1`. A schema number is local to its kind.

| Kind | Name | Schema 1 payload |
| ---: | --- | --- |
| `1` | exact-content | exact raw bytes |
| `2` | project-tree | canonical File Descriptor array |
| `3` | complete-package-tree | canonical File Descriptor array |
| `4` | package-requirement | Package Specification and Complete Package Tree Identity |
| `5` | font-requirement | Font Container Object Descriptor and required face-index set |
| `6` | discovery-request-commitment | role, logical key, size, and private raw bytes |
| `7` | discovery-variant | Discovery Coverage Request |
| `8` | discovery-trace | Discovery Trace |
| `9` | discovery-coverage | Discovery Variant Identity and Discovery Trace Identity |
| `10` | engine | Engine Descriptor |
| `11` | pack | Pack Identity projection |
| `12` | archive-encoding | exact archive writer recipe |
| `13` | closure-export-tree-content | canonical Closure Export file inventory |

Compilation Request Commitments, Engine-Neutral Compilation Intents, Compilation Identities, Compilation Result Identities, and Compilation Artifact Identities use the same transcript rules but are not serialized by Epoch 2 and do not receive format-local registry numbers here. Their exact projection registry belongs to the compilation contract rather than the Pack representation. An implementation MUST NOT invent Epoch 2 numbers for them.

### Commitment Role Registry

| Value | Role |
| ---: | --- |
| `1` | Typst input value |
| `2` | discovery-only project override |

### Other Enum Registries

Target, request kind, disposition, font style, font flags, source class, observation tags, and document-time sign are assigned in the field-semantics sections above. No aliases exist.

## Identity Transcript And Projections

### Typed Equality And Wire Form

Identity equality compares the complete tuple `(kind, schema, algorithm, digest)`. A bare digest is never sufficient. The canonical wire form is the four-element CBOR array defined above.

Human rendering is:

```text
typst-pack:<kind-name>:<schema-decimal>:sha256:<64-lowercase-hex>
```

`sha256:<hex>` is permitted only where kind and schema are fixed by context, such as a blob path.

### Exact Transcript

For kind `K`, schema `S`, algorithm SHA-256, and payload `P`:

```text
transcript =
    h'74797073742d7061636b206964656e7469747900'
    || u32be(S)
    || u32be(K)
    || u64be(byte_length(P))
    || P

digest = SHA-256(transcript)
```

The fixed bytes are ASCII `typst-pack identity` followed by NUL. Integers are unsigned fixed-width big-endian values. The algorithm is selected by the typed tuple and is not duplicated in its own digest input. Epoch 2 supports only SHA-256.

Exact Content Identity uses the exact content bytes as `P`. Every structured identity uses the canonical CBOR array projection below as `P`, not the containing control-record map.

### Structured Projections

Every structured payload is exactly one of these complete array grammars. No control-record map survives into an identity payload.

```cddl
u32 = uint .le 4294967295
u63 = uint .le 9223372036854775807
digest32 = bstr .size 32
projection-text = tstr .size (1..4294967295)
projection-path = projection-text

content-id = [1, 1, 1, digest32]
project-tree-id = [2, 1, 1, digest32]
package-tree-id = [3, 1, 1, digest32]
package-requirement-id = [4, 1, 1, digest32]
font-requirement-id = [5, 1, 1, digest32]
discovery-commitment-id = [6, 1, 1, digest32]
discovery-variant-id = [7, 1, 1, digest32]
discovery-trace-id = [8, 1, 1, digest32]
discovery-coverage-id = [9, 1, 1, digest32]
engine-id = [10, 1, 1, digest32]
pack-id = [11, 1, 1, digest32]
archive-encoding-id = [12, 1, 1, digest32]
closure-tree-id = [13, 1, 1, digest32]

identity-version = [major: u32, minor: u32, patch: u32]
identity-object = [exact-size: u63, content: content-id]
identity-file = [path: projection-path, exact-size: u63, content: content-id]
identity-package-spec = [
  namespace: projection-text,
  name: projection-text,
  version: identity-version,
]
identity-sensitive-value = [exact-size: u63, commitment: discovery-commitment-id]
identity-font-face = [container: content-id, face-index: u32]
identity-document-time = [sign: 0 / 1, magnitude: u63]

project-tree-payload = [* identity-file]
package-tree-payload = [* identity-file]
package-requirement-payload = [identity-package-spec, package-tree-id]
font-requirement-payload = [identity-object, [+ u32]]

discovery-commitment-payload = [
  role: 1 / 2,
  logical-key-or-path: projection-text,
  exact-size: u63,
  raw-value-bytes: bstr,
]

discovery-variant-payload = [
  target: 0 / 1,
  inputs: [* [projection-text, identity-sensitive-value]],
  document-time: identity-document-time / null,
  features: [* projection-text],
  overrides: [* [projection-path, identity-sensitive-value]],
]

identity-project-observation =
  [0, projection-path, 0 / 1, identity-object] /
  [1, projection-path, 0 / 1, identity-sensitive-value] /
  [2, projection-path, 0 / 1]

identity-package-observation =
  [0, package-requirement-id, projection-path, 0 / 1, identity-object] /
  [1, package-requirement-id, projection-path, 0 / 1]

discovery-trace-payload = [
  [* identity-project-observation],
  [* identity-package-observation],
  [* identity-font-face],
]

discovery-coverage-payload = [discovery-variant-id, discovery-trace-id]

identity-qualifier = [name: projection-text, value: projection-text]
engine-payload = [
  producer-id: projection-text,
  implementation-name: projection-text,
  engine-version: identity-version,
  build-fingerprint: bstr,
  target-profile: projection-text,
  qualifiers: [* identity-qualifier],
  unicode-version: identity-version,
  package-metadata-profile-id: projection-text,
  font-metadata-profile-id: projection-text,
]

semantic-extension-payload = [
  identifier: projection-text,
  extension-epoch: 1..4294967295,
  payload: bstr,
  objects: [* identity-object],
]

pack-payload = [
  discovery-engine: engine-id,
  entrypoint: projection-path,
  project-tree: project-tree-id,
  effective-inclusions: [* projection-path],
  coverage: [* discovery-coverage-id],
  packages: [* [package-requirement-id, 0 / 1]],
  fonts: [* [font-requirement-id, 0 / 1]],
  font-catalog: [* identity-font-face],
  extensions: [* semantic-extension-payload],
]

closure-tree-payload = [*
  [wrapper-path: projection-text, exact-size: u63, content: content-id]
]
```

Project Tree and Complete Package Tree payloads exclude stored count and aggregate size, which are rederived. Font face indices are sorted and nonempty. Derived font metadata and provenance are excluded from Font Requirement Identity.

The public Sensitive Value Descriptor stores only size and Discovery Request Commitment Identity. Role and logical key are supplied by the containing request field and MUST agree with the transcript. Typst input values contribute exact UTF-8 bytes without normalization; discovery overrides contribute exact arbitrary file bytes. Commitments make no confidentiality claim against guessing.

Discovery Coverage Identities sort by complete typed tuple rather than declaration order. Requirement pairs retain requirement canonical order. Pack Font Catalog order remains intentional.

Full typed identity tuples sort numerically by kind, schema, and algorithm, then by unsigned bytewise digest order.

Pack Identity excludes:

- Pack Format Epoch and field numbers;
- the claimed Pack Identity;
- Discovery Variant labels and declaration order;
- metadata and sanitized provenance;
- Pack Annotations;
- package and font summaries that are deterministic derivatives;
- archive and Closure Export encoding;
- compression and entry order;
- source-host metadata; and
- derived `portable` and `self-contained` flags.

Every valid Pack is portable. Self-containment is true exactly when every package and font disposition is embedded. Neither flag is serialized.

Closure Export Tree Content payload uses `closure-tree-payload` in `typst-pack/pack.cbor` then lowercase digest-path order. It is a representation identity used by receipts, not Pack Identity.

### Field-To-Identity-Kind Registry

| Position | Required kind |
| --- | --- |
| top-level claimed identity | Pack, 11 |
| Engine record identity | Engine, 10 |
| project tree identity | Project Tree, 2 |
| Complete Package Tree identity | Complete Package Tree, 3 |
| every Object Descriptor and Font Container | Exact Content, 1 |
| Sensitive Value Descriptor | Discovery Request Commitment, 6 |
| Discovery Variant identity | Discovery Variant, 7 |
| Discovery Trace identity | Discovery Trace, 8 |
| Discovery Coverage identity | Discovery Coverage, 9 |
| Package Requirement record and trace link | Package Requirement, 4 |
| Font Requirement record | Font Requirement, 5 |
| Font Face container identity | Exact Content, 1 |
| semantic-extension object | Exact Content, 1 |
| Archive Encoding payload or receipt | Archive Encoding, 12 |
| Closure Export tree receipt | Closure Export Tree Content, 13 |
| receipt actual/expected input, output Archive, and control record | Exact Content, 1 |
| receipt source, derived, and expected Pack | Pack, 11 |
| receipt file inventory | Exact Content, 1 |

Every position uses schema 1 and algorithm 1. A different tuple is invalid.

### Canonical Collection Order

| Collection | Order |
| --- | --- |
| project and package files | path UTF-8 bytes |
| effective inclusions | path UTF-8 bytes |
| inputs | input-key UTF-8 bytes |
| overrides | project-path UTF-8 bytes |
| engine features | feature-id ASCII bytes |
| project observations | path, request kind, observation tag |
| package observations | requirement identity tuple, path, request kind, tag |
| trace font uses | container identity tuple, face index |
| package requirements | namespace, name, numeric version |
| font requirements | container Content Identity tuple |
| required faces | face index |
| Discovery Coverage set | full identity tuple |
| semantic extensions and annotations | identifier, epoch |
| extension objects | Content Identity tuple |
| engine qualifiers | name, value |
| font axes | four tag bytes |
| codepoint ranges | first codepoint |
| metadata keywords | exact UTF-8 bytes |

## Whole-Pack Invariants

The authoritative construction seam enforces all of these rules:

1. The top-level epoch is exactly 2 and every field, enum, identity, path, count, size, collection, and ceiling obeys this contract.
2. The nonempty Discovery Variant list has unique canonical Discovery Variant Identities and unique non-null labels.
3. Every Discovery Variant has exactly one request, one trace, one rederived trace identity, and one rederived Discovery Coverage Identity.
4. Variant declaration order and labels do not affect Discovery Variant, Discovery Coverage, or Pack Identity.
5. The project tree equals all successfully observed project paths plus effective Explicit Conditional Inclusions.
6. Every project tree binding contains baseline bytes. A discovery override never creates a second project-file role.
7. Effective inclusions are disjoint from observed paths.
8. The fixed entrypoint exists and is successfully observed as source by every variant.
9. Each variant has at most one terminal project observation per `(path, request kind)` and at most one package observation per `(requirement, path, request kind)`.
10. Every successful project observation uses override provenance if and only if that variant declares an override for the path.
11. Every baseline project read exactly matches the baseline project Object Descriptor.
12. Every override read exactly matches one override in that variant's request by path, size, and commitment.
13. Every override targets a contained baseline project file. An override never read by any variant is effectively included.
14. A project missing probe names no contained project path.
15. Every package trace reference resolves to one declared Package Requirement, and Package Requirements are exactly the package specifications reached by successful discovery.
16. A package read exactly matches its Complete Package Tree descriptor; a package missing probe names no member of that tree.
17. Every Complete Package Tree has correct canonical paths, file count, aggregate size, tree identity, manifest summary, package-spec agreement, compiler bound, and contained entrypoint to the extent its bytes are available.
18. Font Requirements are exactly the union of faces used by Discovery Traces. Each has a nonempty face set and valid unique indices.
19. When Font Container bytes are available, every face parses and its selection and licensing metadata agrees with the record under the Engine Descriptor's profile.
20. The Pack Font Catalog contains every required face once, no undeclared face, and preserves discovery-relative selection order.
21. Embedded and external dispositions are explicit and disjoint. Embedded content is authoritative.
22. The physical blob set exactly equals the deduplicated required set. Missing required and unreferenced blobs are invalid.
23. Every available blob agrees with its exact size, Content Identity, descriptors, and aggregate identities.
24. Incidental physical bytes never change an external disposition.
25. Every semantic extension identifier has `.sem` class, is understood, validates its payload and object inventory, and cannot weaken core invariants or cause ambient lookup.
26. Every annotation identifier has `.ann` class and annotations are never consulted during semantic construction or compilation.
27. Every leaf, tree, requirement, variant, trace, coverage, engine, and Pack Identity that can be rederived is rederived and compared.
28. Equal typed identities associated with unequal available payloads are a fatal collision or internal-integrity condition, never fallback or a cache miss.
29. An externally supplied expected Pack Identity is compared only after complete internal validation and Pack Identity derivation.
30. A constructed Pack is immutable, inspectable, writable to every supported Epoch 2 representation, and usable without a second whole-Pack validity pass.

## Narrow ZIP Profile

### Archive Layout

A Pack Archive is one ZIP archive beginning at offset zero with no prefix or trailing bytes.

Local records are contiguous from offset zero. The central directory begins immediately after the final entry payload. ZIP64 end records, when required, immediately follow the central directory. The ordinary End of Central Directory record is the final 22 bytes and has no comment.

The archive has:

- exactly one disk;
- no archive or entry comments;
- no data descriptors;
- no encryption;
- no digital signatures;
- no archive extra-data records;
- no central-directory encryption;
- no directory, link, device, sparse, or external-reference entries; and
- no unknown records or entries.

The first entry is Stored `typst-pack/pack.cbor`. Remaining entries are exactly the required blobs in lowercase digest order. Local-record order and central-directory order are identical.

Blob methods are Stored method 0 or Deflate method 8. `pack.cbor` MUST use Stored method 0.

### Fixed Header Values

| Field | Required value |
| --- | --- |
| general-purpose flags | `0x0800` (UTF-8 only) |
| DOS time | `0x0000` |
| DOS date | `0x0021` (1980-01-01) |
| version made by | `0x032d` (Unix, ZIP 4.5) |
| internal attributes | `0x0000` |
| external attributes | `0x81a40000` (`0100644` regular file) |
| file and archive comments | empty |

Wrapper names are exact ASCII despite the required UTF-8 flag. Names use `/` and have no aliasing.

Version needed to extract is:

| Entry condition | Value |
| --- | ---: |
| Stored without any entry ZIP64 field | `10` |
| Deflate without any entry ZIP64 field | `20` |
| any entry size or local-header offset uses ZIP64 | `45` |

Local and central headers MUST agree after ZIP64 expansion on name, flags, method, CRC-32, compressed size, decoded size, and local-header offset. Every planned range is in bounds and nonoverlapping. Record and payload layout is exact and gap-free.

Every local and central extra-field area is empty unless this profile requires exactly one ZIP64 extra field with ID `0x0001`. Timestamp, NTFS, Unicode-path, padding, and every other extra-field ID are invalid even when another ZIP reader understands them.

### ZIP64

Only ZIP's designated size, offset, count, and disk fields use sentinels:

- local and central compressed and decoded sizes require ZIP64 at `>= 0xffffffff`;
- central local-header offset requires ZIP64 at `>= 0xffffffff`;
- EOCD entry counts require ZIP64 at `>= 0xffff`; and
- EOCD central-directory size and offset require ZIP64 at `>= 0xffffffff`.

CRC-32, filename length, attributes, and every other field retain every literal bit pattern, including all-ones values. Multidisk fields remain zero and never use ZIP64 in this profile.

For a local header, if either size requires ZIP64, both 32-bit size fields are `0xffffffff` and one ZIP64 extra field `0x0001` contains exactly decoded size then compressed size as little-endian `u64` values.

For a central header, each overflowing size or local-header offset independently uses its sentinel. One ZIP64 extra field is present only when required and contains only the required values in PKWARE order: decoded size, compressed size, local-header offset. Disk start is never present.

ZIP64 EOCD and locator records appear exactly when an ordinary EOCD entry count, central-directory size, or central-directory offset requires a sentinel. The ZIP64 EOCD data size is exactly 44 and has no extensible sector; version made by is `0x032d`, version needed is 45, both disk numbers are zero, both entry counts equal the actual count, and directory size and offset equal the reconstructed directory. The locator names disk zero, points to the physical ZIP64 EOCD offset, and reports total disks one. Ordinary EOCD disk fields are zero, its two counts agree after sentinel expansion, and every other field uses its exact value when it fits and the sentinel only when required.

Superfluous ZIP64, duplicate ZIP64 fields, optional aliases, wrong field order, wrong lengths, and values disagreeing with reconstructed physical values are invalid.

### Deflate

Method 8 is one raw RFC 1951 stream with no zlib or gzip wrapper.

A reader MUST:

1. validate the complete ZIP range plan before invoking Deflate;
2. admit declared compressed and decoded sizes before allocating or decoding;
3. require `ceil(decoded_size / 2048) <= max(1, compressed_size)`, where unsigned ceiling division is exact and overflow-free;
4. account output incrementally against declared size, format ceilings, and operation budgets;
5. consume every non-Huffman field, including length, distance, and code-length-repeat extra bits, least-significant-bit first; reject oversubscribed Huffman trees; require a complete code-length tree; permit an incomplete literal/length or distance tree only for the RFC single-symbol length-1 case; require literal/length symbol 256; permit an empty distance tree only when no length symbol is emitted; and reject reserved symbols and invalid distances;
6. require the `BFINAL=1` block to end in the final compressed byte; compressed blocks end at symbol 256, while stored blocks end after their declared `LEN` bytes;
7. require every unused high bit after the final logical bit to be zero;
8. reject concatenated streams and trailing compressed bytes;
9. require actual decoded length to equal the declaration;
10. verify ZIP CRC-32; and
11. rederive kind 1, schema 1 Exact Content Identity using the normative identity transcript, compare its digest component with the blob filename, and compare its full typed identity with every Object Descriptor.

Stored entries perform the same decoded-size, CRC-32, and item 11 Exact Content Identity checks without Deflate.

### Reader Strategy

A custom range parser is not mandatory. A ZIP library is conforming only if the implementation can observe and reject every forbidden record, flag, extra field, range inconsistency, alias, overlap, trailing byte, and Deflate remainder before exposing a Pack. A permissive high-level extraction result is not sufficient evidence of conformance.

## Closure Export Profile

### Exact Tree

A Closure Export immutable finite-tree snapshot has exactly this structure:

```text
<root>/
  typst-pack/
    pack.cbor
    blobs/
      sha256/
        <64 lowercase hexadecimal digits>
```

The root has exactly one child, `typst-pack`. `typst-pack` has exactly `pack.cbor` and `blobs`. `blobs` has exactly `sha256`. `sha256` has exactly the required blob files. Structural directories may be explicit tree nodes; no additional empty directory is allowed.

All files are ordinary finite immutable regular files. Links, special nodes, aliases, duplicate names, case-colliding wrapper names, and file-versus-directory conflicts are invalid. A snapshot adapter consults stable node identity only to reject two paths naming one hard-linked node; node identity never enters representation identity or metadata. An adapter unable to establish non-aliasing refuses the snapshot. Host permissions, ownership, timestamps, ACLs, inode identity after the alias check, and enumeration order are ignored.

`pack.cbor` bytes are byte-for-byte the canonical Pack Control Record used by Pack Archive. Blob bytes are decoded exact bytes. No logical project or package path appears in the wrapper namespace.

### Import

Import accepts one complete immutable finite-tree snapshot, never a live mutable directory. It:

1. validates exact wrapper nodes and node kinds;
2. reads and validates `pack.cbor`;
3. derives the required physical blob set;
4. requires exact equality with the physical file inventory;
5. verifies blobs in digest order; and
6. enters the same private whole-Pack constructor as Archive ingress and in-memory creation.

Import never repairs declarations, recomputes stale identities, acquires external dependencies, reruns discovery, or creates baseline content from override bytes.

Same-epoch export-import-export reproduces exact `pack.cbor` bytes and the exact wrapper path/byte inventory.

### Publication

Canonical filesystem publication requires:

- an absent destination root; an existing empty directory is still a conflict;
- one complete canonical write plan before destination effects;
- private confined staging under an enforcement-owned parent;
- exclusive regular-file creation without link following;
- verification of every staged size and Content Identity;
- one complete-collection atomic no-replace commit; and
- bounded cleanup before failure returns.

A filesystem unable to provide the requested atomic no-replace directory commit MUST refuse publication. Staging names, buffering, file modes, and durability policy are adapter details and do not change Closure Export bytes.

## Validation Algorithm

### Terminal Classes

| Terminal | Validity | Meaning |
| --- | --- | --- |
| success | valid | a validated Pack or completed representation value exists |
| invalid | invalid | bytes violate the known Epoch 2 contract |
| unsupported | unknown | a well-formed extensibility point is not implemented |
| expected-identity-mismatch | valid | internal Pack is valid but external expectation differs; no Pack is exposed |
| archive-encoding-assertion-mismatch | valid | Pack is internally valid but exact bytes do not match the asserted registered recipe; no Pack is exposed |
| input-content-identity-mismatch | unknown | stable input bytes do not match an externally expected Content Identity and are not interpreted |
| resource-outcome | unknown | an operation budget stopped validation before validity was established |
| cancelled or deadline | unknown | operation interrupted before commitment |
| admission-refused | unknown | requested controls or optional recipe verification are unsupported before effects |
| internal-integrity | unknown | implementation state or a real typed-identity collision is inconsistent |

Unknown Epoch, semantic-extension epoch, Engine package-metadata profile, and Engine font-metadata profile are unsupported. Every identity position in this control record requires its field-specific known kind, schema 1, and algorithm 1; another value is invalid. Unknown core fields, core enums, ZIP flags, methods, records, and extra fields in known Epoch 2 are invalid.

### Ordered Pipeline

```text
read_epoch2_archive(
    input,
    expected_archive_content_identity_or_none,
    verification_mode,
    asserted_archive_encoding_identity_or_none,
    controls,
    report_cap,
):
    ; verification_mode is Derive or Verify(expected_pack_identity)
    admitted = admit_controls_or_return_operational_outcome(controls)
    if asserted recipe is unsupported:
        return admission_refused
    require input is one known-length random-access Stable Byte Value
    if input.exact_length > 2^63 - 1:
        return invalid_issue(phase=1, code="zip.archive-size-ceiling")
    if input.exact_length > admitted.input_bytes:
        return resource_outcome(stage=admission, validity=unknown)
    archive_content_identity = content_identity(input.exact_bytes)
    if expected Archive Content Identity is present and differs:
        return input_content_identity_mismatch_without_interpretation

    zip_plan = parse_and_validate_narrow_zip_fail_fast(input, admitted)
    control_bytes = exact_stored_bytes(zip_plan.first_entry)
    control_content_identity = content_identity(control_bytes)

    record = decode_one_canonical_cbor_item_fail_fast(control_bytes)
    require record.epoch == 2

    issues = validate_closed_schema(record)
    if issues is nonempty:
        return canonical_invalidity_report(issues, report_cap)

    unsupported = collect_unsupported_extensions_and_profiles(record)
    expected_objects = derive_required_physical_blob_set(record)
    issues += compare_planned_and_required_inventory(zip_plan, expected_objects)

    for object in expected_objects in digest order:
        exact_bytes = bounded_decode_and_verify(object)
        issues += validate_size_crc_sha_and_descriptor_agreement(object, exact_bytes)

    issues += validate_paths_and_tree_conflicts(record)
    issues += validate_project_closure_and_entrypoint(record)
    issues += validate_variants_commitments_and_traces(record)
    issues += validate_discovery_coverage(record)
    issues += validate_packages(record, available_bytes)
    issues += validate_fonts_and_catalog(record, available_bytes)
    issues += validate_known_extensions_annotations_metadata_provenance(record)
    issues += rederive_available_leaf_and_aggregate_identities(record)

    if issues is nonempty:
        return canonical_invalidity_report(issues, report_cap)

    candidate = construct_core_candidate_without_unknown_handlers(record, verified_objects)
    derived_pack_identity = derive_pack_identity(candidate)
    if record.claimed_pack_identity != derived_pack_identity:
        return invalid_issue(phase=120, code="pack.claimed-identity-mismatch")

    if unsupported is nonempty:
        return canonical_unsupported_report(unsupported)

    if verification_mode is Verify(expected_pack_identity):
        require expected_pack_identity == derived_pack_identity
            else return expected_identity_mismatch_without_exposing_pack

    if asserted Archive Encoding Identity is present:
        reproduced = encode_exact_representation_input_with_asserted_recipe(
            record.exact_bytes,
            verified_objects,
        )
        if reproduced.exact_bytes != input.exact_bytes:
            return archive_encoding_assertion_mismatch_without_exposing_pack

    pack = authoritative_whole_pack_construct(candidate)
    commit_validated_pack(pack)
    return pack, read_receipt
```

Closure Export import replaces ZIP planning with exact finite-tree planning and then uses the same schema, object, semantic, and construction stages.

External checks have one precedence: admission capability, statically known format ceiling, admitted input-byte limit, expected input Content Identity, internal representation validity, expected Pack Identity, then optional Archive Encoding assertion. No mismatch exposes a Pack. Corpus precedence vectors combine each adjacent pair and require the earlier terminal.

Writers accept only a constructed Pack. They perform representation planning and object verification but no second whole-Pack semantic validation pass.

### Immediate Failure And Issue Collection

Unsafe representation framing, arithmetic overflow, out-of-bounds or overlapping ranges, and any condition that prevents a safe immutable plan fail immediately with phase 1. Malformed or non-canonical CBOR framing fails immediately with phase 2. A well-formed unsupported epoch returns unsupported in phase 2. Within Epoch 2, the complete known core schema and every extension or profile envelope are validated first; any core invalidity wins. Only a schema-valid record may return unsupported for a semantic extension or Engine metadata profile, in phase 10, ordered by identifier.

Once a safe plan exists, independently detectable issues are collected in this phase order:

| Phase | Name |
| ---: | --- |
| `1` | Pack Archive or Closure Export framing |
| `2` | CBOR item framing and canonical encoding |
| `10` | control schema and registry |
| `20` | physical object inventory and bytes |
| `30` | paths and tree conflicts |
| `40` | entrypoint and project closure |
| `50` | Discovery Coverage Requests and commitments |
| `60` | Discovery Traces |
| `70` | Discovery Coverage bindings |
| `80` | Package Requirements |
| `90` | Font Requirements and catalog |
| `100` | extensions, annotations, metadata, and provenance |
| `110` | leaf and aggregate identity claims |
| `120` | claimed Pack Identity |

Within a phase, issues sort by `(subject kind, canonical subject key, rule code, occurrence)`. Paths use exact UTF-8 bytes. Variants use declaration index. Packages use Package Specification. Font catalog issues use catalog index; other font issues use container identity and face index. Extensions use identifier and epoch.

A malformed parent suppresses dependent descendant checks to prevent cascades. A report cap MUST be at least one. Reaching it after invalidity is established returns an incomplete invalidity report. Reaching an operation budget before invalidity is established returns validity unknown.

The primary stable rule-code prefixes are:

```text
cbor.*       canonical CBOR and closed schema
zip.*        ZIP framing, records, headers, ranges, ZIP64, Deflate
tree.*       Closure Export wrapper and canonical logical trees
object.*     object inventory, size, CRC, and content identity
path.*       canonical paths and tree conflicts
project.*    entrypoint, project closure, inclusion, override observation
variant.*    Discovery Coverage Request and Discovery Variant
trace.*      Discovery Trace observations
coverage.*   Discovery Coverage binding
package.*    Package Requirement and Complete Package Tree
font.*       Font Requirement, metadata, and Pack Font Catalog
extension.*  semantic extensions and annotations
identity.*   typed identity, transcript, and aggregate identity
pack.*       whole-Pack and claimed Pack Identity
```

The complete core rule-code registry is:

| Phase | Subject | Codes |
| ---: | --- | --- |
| 1 | archive | `zip.archive-size-ceiling`, `zip.prefix-forbidden`, `zip.trailing-data`, `zip.multidisk`, `zip.comment`, `zip.forbidden-record`, `zip.forbidden-extra-field`, `zip.forbidden-entry-type`, `zip.header-constant`, `zip.control-entry`, `zip.entry-name`, `zip.entry-order`, `zip.header-mismatch`, `zip.crc-mismatch`, `zip.size-mismatch`, `zip.range-invalid`, `zip.zip64-invalid`, `zip.deflate-invalid` |
| 1 | Closure wrapper path | `tree.wrapper-invalid`, `tree.node-type`, `tree.alias`, `tree.path-conflict`, `tree.inventory-mismatch` |
| 2 | CBOR pointer | `cbor.malformed`, `cbor.nonshortest`, `cbor.indefinite`, `cbor.map-order`, `cbor.duplicate-key`, `cbor.forbidden-type`, `cbor.invalid-utf8`, `cbor.trailing-data`, `cbor.depth` |
| 10 | CBOR pointer | `cbor.type-mismatch`, `cbor.value-range`, `cbor.shape`, `cbor.unknown-enum`, `cbor.unknown-field`, `cbor.missing-field`, `cbor.invalid-null`, `cbor.collection-order`, `cbor.duplicate-member`, `identity.tuple-invalid` |
| 20 | object digest | `object.missing`, `object.extra`, `object.size-mismatch`, `object.content-identity-mismatch`, `object.identity-collision` |
| 30 | logical path | `path.syntax`, `path.limit`, `path.duplicate`, `path.file-descendant-conflict` |
| 40 | project path or pack | `project.entrypoint`, `project.closure`, `project.inclusion`, `project.override-target`, `project.override-provenance`, `project.observation` |
| 50 | variant index | `variant.empty-set`, `variant.duplicate`, `variant.label`, `variant.request`, `variant.commitment`, `variant.identity` |
| 60 | variant index | `trace.order`, `trace.duplicate`, `trace.reference`, `trace.outcome`, `trace.content`, `trace.identity` |
| 70 | variant index | `coverage.binding`, `coverage.duplicate`, `coverage.identity` |
| 80 | Package Specification | `package.requirement-set`, `package.spec`, `package.tree`, `package.manifest`, `package.compiler`, `package.disposition`, `package.metadata`, `package.identity` |
| 90 | Font Container and optional face | `font.requirement-set`, `font.faces`, `font.face-index`, `font.metadata`, `font.catalog`, `font.disposition`, `font.identity` |
| 100 | extension identifier and epoch | `extension.identifier`, `extension.class`, `extension.duplicate`, `extension.envelope`, `extension.object`, `extension.handler`, `extension.annotation-semantic-effect` |
| 110 | nearest owning semantic subject | `identity.transcript`, `identity.leaf`, `identity.aggregate` |
| 120 | pack | `pack.claimed-identity-mismatch` |

The subject key is the canonical key for that subject class in the corpus registry. A malformed identity tuple uses its CBOR pointer before an owning semantic subject can be established. An identity claim otherwise uses the nearest owner: path for project content, Package Specification for package state, Font Container/face for font state, variant index for variant/trace/coverage state, extension key for extension state, and pack for Pack Identity. No other core rule code exists in Epoch 2 corpus revision 1; adapters place extra non-oracle detail beneath these classes. Human error text is never normative.

`cbor.type-mismatch` means a globally permitted CBOR major type appears in a field requiring another type. `cbor.value-range` means the correct scalar type is outside its allowed numeric or string bounds. `cbor.shape` means a map or array has the wrong arity or structural alternative. `cbor.unknown-enum` means an in-range unsigned value is unassigned in the field's closed enum. Globally forbidden CBOR types remain phase-2 `cbor.forbidden-type`.

### Trust And Limits

Trusted, Partially Trusted, and Hostile operations apply the same bytes, schema, identity, ceiling, and whole-Pack rules. The requested profile is selected before input-dependent interpretation, and the least-trusted admitted input controls the minimum profile. Profiles vary only admission, confinement, isolation, budgets, and interruption strength. A digest match never skips structural or semantic validation.

Hostile admission requires a verified deny-by-default operating-system or runtime boundary before Archive, CBOR, package, font, source, compiler, or exporter interpretation; hard admitted quotas; complete kill and reap; bounded parent verification; and parent-owned publication. If any property is unavailable, the operation is refused. There is no in-process fallback, post-parse promotion, or silent downgrade.

Epoch 2 representability ceilings are:

- every archive offset, archive length, compressed length, decoded object length, and aggregate decoded-byte count is at most `2^63 - 1`;
- archive entry count and every semantic collection count are at most `2^32 - 1`;
- Pack Control Record, semantic-extension payload, and annotation payload lengths are at most `2^32 - 1`;
- paths obey the byte and segment limits above;
- CBOR nesting is at most 32; and
- a Deflate object obeys the 2,048-to-1 ratio in addition to absolute ceilings.

Arithmetic and host-type conversions are checked before allocation, seeking, slicing, or multiplication. Declared sizes are charged before decoding and actual output is charged incrementally. Logical references and physical blobs are counted separately so deduplication cannot bypass semantic limits.

Crossing a format ceiling is invalid. Crossing only a stricter Operation Resource Limit is an operational resource outcome with validity unknown.

## Archive Encoding Identity

Archive Encoding Identity kind 12, schema 1 hashes this canonical payload:

```cddl
archive-encoding-payload = [
  1,                                  ; Pack Archive profile revision
  namespaced-id,                      ; writer producer
  namespaced-id,                      ; producer's recipe-schema ID
  1..4294967295,                      ; recipe-schema epoch
  [u32, u32, u32],                    ; writer implementation version
  bstr .size (0..255),                ; exact writer build fingerprint
  bstr,                               ; canonical recipe parameter bytes
]
```

The complete outer payload uses the same Section 4.2.1 core deterministic CBOR profile as other structured identity payloads. Its encoded length and recipe parameter length are each at most `2^32 - 1` bytes; writer version components are at most `2^32 - 1`; the build fingerprint is at most 255 bytes; and nesting is at most 32. Recipe parameters contain the producer schema's canonical bytes as one byte string; the outer identity implementation never guesses or rewrites them. The explicit recipe-schema ID and epoch bind those bytes to their interpretation.

Each producer registry publishes the complete parameter schema, deterministic Stored-versus-Deflate method-selection algorithm, Deflate implementation and parameters, and every remaining byte-affecting choice permitted by the narrow ZIP profile. The registry includes canonical payload, transcript, digest, and golden-archive vectors. A recipe cannot depend on ambient time, locale, process state, iteration order, destination, or resource policy. Successful Pack Archive Encoding requires a supported registered recipe; arbitrary unregistered payloads can be hashed but cannot be admitted for encoding or returned in a successful encoding receipt.

Epoch 2 registers one universal baseline recipe:

```text
producer:               org.typst-pack.epoch2
recipe schema:          org.typst-pack.archive.all-stored
recipe epoch:           1
implementation version: 1.0.0
build fingerprint:      empty byte string
recipe parameters:      empty byte string
```

It selects Stored method 0 for every blob. All other bytes follow the fixed narrow ZIP profile. Any implementation may claim this Archive Encoding Identity only when it reproduces its corpus archives byte-for-byte.

Archive Encoding Identity:

- is never stored in a Pack Control Record;
- never contributes to Pack Identity;
- does not include destination, timing, publication policy, or source provenance;
- cannot be inferred from ZIP bytes, methods, headers, or decompressed content;
- may differ even when two recipes coincidentally produce equal bytes;
- changes when the claimed recipe changes even if output bytes do not;
- is not publisher authentication; and
- is not a substitute for Archive Content Identity.

For a supported registered recipe, the exact determinism claim is:

```text
identical Pack Control Record bytes
+ identical canonical blob path/byte inventory
+ identical Archive Encoding Identity
=> identical Pack Archive bytes
```

Pack Identity plus Archive Encoding Identity is insufficient because non-identifying metadata, provenance, and annotations may alter `pack.cbor` while preserving Pack Identity.

Generic Archive ingress returns no Archive Encoding Identity. A reader MAY report an externally asserted identity only when it supports that exact recipe, re-encodes the exact control bytes and blob inventory, and compares the complete archive byte-for-byte. Otherwise it refuses the optional assertion-verification request rather than reporting an unverified identity.

## Receipt Semantics

Representation Receipt Contract version 1 is a separately versioned normative logical model, not a universal persisted wire protocol and not part of Pack Format Epoch dispatch. Rust, CLI JSON, Dagger, and service adapters may encode or extend it differently while preserving this required projection and terminal distinctions.

The implementation-neutral projection is:

```cddl
format-receipt = {
  0: 1,                               ; receipt contract version
  1: receipt-role,
  2: receipt-terminal,
  3: receipt-stage,
  4: receipt-counters,
  5: receipt-identities,
  6: verification-mode,
  7: bool / null,                     ; expected input Content Identity matched
  8: encoding-assertion-status,
  9: bool,                            ; Pack exposed
  10: bool,                           ; completed Stable Byte Value exists
  11: publication-status,
  12: cleanup-status,
  13: timing-status,
  14: namespaced-id,                  ; sanitized adapter class
  15: [* receipt-file] / null,
  16: receipt-controls,               ; requested
  17: receipt-controls / null,        ; admitted; null before admission
  18: failure-class,
  19: namespaced-id / null,           ; sanitized structured cause
  20: bool / null,                    ; expected Pack Identity matched
}

receipt-counters = {
  0: uint / null,                     ; input bytes
  1: uint / null,                     ; output bytes
  2: uint / null,                     ; control-record bytes
  3: uint / null,                     ; planned objects
  4: uint / null,                     ; verified objects
  5: uint / null,                     ; aggregate decoded bytes
  6: uint / null,                     ; file count
}

receipt-identities = {
  0: identity / null,                 ; input Archive Content Identity
  1: identity / null,                 ; Pack Control Record Content Identity
  2: identity / null,                 ; derived or source Pack Identity
  3: identity / null,                 ; expected Pack Identity
  4: identity / null,                 ; Archive Encoding Identity
  5: identity / null,                 ; output Archive Content Identity
  6: identity / null,                 ; Closure Export Tree Content Identity
  7: identity / null,                 ; expected input Content Identity
}

receipt-file = [nonempty-text, u63, identity]

receipt-controls = {
  0: trust-profile,
  1: bool,                            ; contractual offline operation
  2: namespaced-id,                   ; resource-profile identity
  3: bool,                            ; deadline present
  4: bool,                            ; cancellation source present
  5: publication-strength,
  6: cleanup-strength,
  7: [* receipt-limit],               ; sorted exact limits
  8: [* namespaced-id],               ; sorted enforcement facts
}

receipt-limit = [namespaced-id, uint]

publication-status = [
  requested: publication-strength,
  admitted: publication-strength / null,
  state: publication-state,
]

cleanup-status = [
  requested: cleanup-strength,
  admitted: cleanup-strength / null,
  state: cleanup-state,
]

receipt-role = 1 / 2 / 3 / 4 / 5 / 6
receipt-terminal = 0 / 1 / 2 / 3 / 4 / 5 / 6 / 7 / 8 / 9 / 10 / 11
receipt-stage = 0 / 1 / 2 / 3 / 4 / 5 / 6 / 7 / 8 / 9 / 10 / 11 / 12
verification-mode = 0 / 1 / 2
encoding-assertion-status = 0 / 1 / 2 / 3
trust-profile = 0 / 1 / 2
publication-strength = 0 / 1 / 2 / 3
publication-state = 0 / 1 / 2 / 3
cleanup-strength = 0 / 1 / 2 / 3
cleanup-state = 0 / 1 / 2 / 3 / 4
timing-status = 0 / 1 / 2 / 3
failure-class = 0 / 1 / 2 / 3 / 4 / 5 / 6 / 7
```

Registry values are:

| Registry | Values |
| --- | --- |
| receipt role | `1` archive encode, `2` archive read, `3` Closure Export project, `4` Closure Export import, `5` archive publish, `6` Closure Export publish |
| receipt terminal | `0` success, `1` invalid, `2` unsupported, `3` expected Pack Identity mismatch, `4` resource outcome, `5` cancelled, `6` deadline, `7` admission refused, `8` internal integrity, `9` transport failure, `10` Archive Encoding assertion mismatch, `11` input Content Identity mismatch |
| stage | `0` admission, `1` reference resolution, `2` acquisition, `3` spooling, `4` representation framing, `5` control record, `6` object verification, `7` construction, `8` encoding or projection, `9` transfer, `10` commit, `11` cleanup, `12` complete |
| verification mode | `0` not applicable, `1` derive, `2` verify |
| encoding assertion | `0` not applicable, `1` not asserted, `2` externally asserted and byte-verified, `3` externally asserted and mismatched |
| trust profile | `0` Trusted, `1` Partially Trusted, `2` Hostile |
| publication strength | `0` not applicable, `1` complete-collection atomic, `2` each-object atomic, `3` streaming |
| publication state | `0` not applicable, `1` not started, `2` committed, `3` failed |
| cleanup strength | `0` not applicable, `1` complete, `2` residual-locator permitted, `3` non-retractable streaming |
| cleanup state | `0` not applicable, `1` complete, `2` residual exists, `3` exposed bytes non-retractable, `4` failed |
| timing status | `0` not requested, `1` complete, `2` limited, `3` unavailable |
| failure class | `0` not applicable, `1` reference resolution, `2` acquisition, `3` spooling, `4` transfer, `5` commit, `6` cleanup, `7` adapter contract |

All map fields are mandatory; `null` or enum zero represents not established or not applicable. Requested and admitted limit arrays contain `(limit kind, exact value)` pairs sorted by kind and unique; enforcement facts are sorted unique namespaced capability identifiers. The admitted record contains every effective value after clamping and MUST NOT claim a stronger trust, publication, cleanup, or enforcement capability than admitted. Publication and cleanup strengths in status fields MUST equal their corresponding requested and admitted controls. Pre-admission refusal sets admitted controls to `null`.

Counter meanings are exact: input bytes are the complete admitted immutable input length; output bytes are bytes produced or transferred by the reached stage; control-record bytes are its exact known length; planned objects are the complete immutable plan count; verified objects count objects whose complete bytes and Exact Content Identity succeeded; aggregate decoded bytes sum only those verified objects; file count is the complete planned file count. A counter is `null` until knowable and never estimates future work.

Identities appear only after the reached stage establishes them. File inventory is mandatory for Closure Export projection and publication and `null` for other roles. Credentials, absolute host paths, URLs, object keys, provider messages, and residual locators appear only in separate capability-gated sensitive projections. Adapters may add controls outside this common projection, but the corpus compares only this closed projection.

The input-content match field is non-null only after an externally expected input identity is compared. The Pack match field is non-null only in Verify mode after complete internal Pack validation. Supplying both expectations therefore records two independent results.

The required success-role matrix is:

| Role | Required identities | Mode and status | Exposure and commit |
| --- | --- | --- | --- |
| archive encode | control record, source Pack, Archive Encoding, output Archive, Closure Export Tree Content | verification not applicable; encoding assertion not applicable | completed Stable Byte Value true; Pack exposed false; publication not applicable |
| archive read | input Archive, control record, derived Pack; expected Pack only for Verify; Archive Encoding only when externally verified | Derive or Verify; assertion not asserted or externally verified | Pack exposed true; Stable Byte Value false; publication not applicable |
| Closure Export project | control record, source Pack, Closure Export Tree Content | verification and assertion not applicable; file inventory required | Pack exposed false; publication not applicable |
| Closure Export import | control record, derived Pack, Closure Export Tree Content; expected Pack only for Verify | Derive or Verify; assertion not applicable; file inventory required | Pack exposed true; publication not applicable |
| archive publish | source/output Archive Content and Archive Encoding when known | verification and assertion not applicable | publication strength and commit required; Pack exposed false |
| Closure Export publish | source Pack and Closure Export Tree Content | verification and assertion not applicable; file inventory required | complete-collection atomic publication and commit required; Pack exposed false |

Counter applicability on success is fixed:

| Role | Input | Output | Control | Planned/verified/decoded objects | Files |
| --- | --- | --- | --- | --- | --- |
| archive encode | null | exact archive length | exact | all required objects | null |
| archive read | exact archive length | null | exact | all required objects | null |
| Closure Export project | null | aggregate tree file bytes | exact | all required objects | exact file count |
| Closure Export import | aggregate tree file bytes | null | exact | all required objects | exact file count |
| archive publish | exact archive length | transferred archive bytes | null | all null | null |
| Closure Export publish | aggregate tree file bytes | transferred tree bytes | exact | all null | exact file count |

An inapplicable counter is `null`, never zero. On failure, an applicable counter follows its exact reached-stage meaning and remains `null` until knowable.

Success always reports stage complete. Invalid and unsupported report the exact framing, control, object, or construction stage at which their normative phase terminated. Expected Pack Identity mismatch reports stage construction, complete internal validation, both Pack identities, and Pack exposed false. Archive Encoding assertion mismatch reports stage encoding or projection, assertion status mismatched, complete internal Pack validation, asserted Archive Encoding Identity, Archive Content Identity, and Pack exposed false. Input Content Identity mismatch reports stage representation framing, actual and expected input Content Identities, and no control-record or Pack facts. Resource, cancelled, and deadline report the stage where the outcome won; if a commit already occurred, committed success wins instead. Admission refusal reports stage admission, admitted controls `null`, publication-status and cleanup-status admitted strengths `null`, state not started, and no content-derived identity. Internal integrity reports the reached stage. Transport failure reports reference, acquisition, spooling, transfer, commit, or cleanup as appropriate, one nonzero failure class, and a sanitized cause ID; all other terminals use failure class zero and null cause.

On non-success, facts established before the reached stage remain populated and all later facts are `null`, false, or not applicable. Publication roles report `committed` only after their linearization point. A primary transfer or commit failure remains the terminal when cleanup also fails; cleanup state and the separately gated residual locator record the secondary cleanup fact.

### Pack Archive Encoding Receipt

A successful encoding receipt records:

- source Pack Identity;
- Pack Control Record Content Identity and length;
- Closure Export Tree Content Identity for the exact control-record and blob inventory;
- Archive Encoding Identity;
- Archive Content Identity and exact archive length;
- completed Stable Byte Value state; and
- publication state `not-applicable`.

Encoding commits when the complete immutable Stable Byte Value exists. Cancellation or deadline before that point wins; a signal after it cannot rewrite success. Publication is a separate operation and receipt.

A failed encoding may report selected Pack and recipe identities and partial produced-byte count, but cannot claim complete archive Content Identity or a completed Stable Byte Value.

### Pack Archive Read Receipt

A successful read receipt records:

- input Archive Content Identity and exact length;
- externally expected input Content Identity and match when one was supplied;
- Pack Control Record Content Identity and exact length;
- Pack Format Epoch 2;
- derived Pack Identity;
- verification mode `derive` or `verify`;
- expected Pack Identity and match when in Verify mode;
- complete validation and Pack commitment; and
- Archive Encoding Identity status `not-asserted` or `externally-asserted-and-byte-verified`.

There is no `derived Archive Encoding Identity` status. Expected Pack Identity mismatch carries both expected and derived identities, terminal `3`, `pack_exposed = false`, and complete validation. Derive mode makes no wholesale-substitution claim. A requested Archive Encoding assertion whose registered recipe is unsupported returns admission-refused before interpretation. A supported recipe that reproduces different bytes returns Archive Encoding assertion mismatch after full validation and comparison.

### Closure Export Projection Receipt

Before publication, projection records:

- projection kind `closure-export`;
- source Pack Identity;
- Pack Control Record Content Identity;
- Closure Export Tree Content Identity;
- exact file count and aggregate byte count;
- canonical `(path, size, Content Identity)` inventory; and
- publication state `not-applicable`.

Closure Export publication separately records the immutable tree identity, admitted complete-collection atomic strength, transferred path inventory and byte counts, commit point, and cleanup result. A composed adapter may return both receipts but cannot collapse projection success into publication success.

## Interoperability Corpus

The normative corpus is a versioned companion to this contract. It MUST be frozen before any public Epoch 2 writer prerelease.

The paths below organize the project-owned corpus and are not runtime Pack layout. Exact fixture bytes, identities, logical inputs, and expected outcomes are normative; generator language, test framework, and how an implementation loads them are not.

```text
epoch-2/
  corpus.json
  registries/
  identities/
  logical-packs/
  control-records/
  archives/
  closure-exports/
  fulfillments/
  extensions/
  receipts/
  invalid/
    cbor/
    zip/
    closure/
    invariants/
    mutations/
  boundaries/
```

Corpus operation values are closed:

```text
derive_identity
validate_canonical_cbor_profile
read_pack_archive
encode_pack_archive
import_closure_export
project_closure_export
publish_pack_archive
publish_closure_export
fulfill_package_requirement
fulfill_font_requirement
project_representation_receipt
```

`validate_canonical_cbor_profile` accepts one of two exact abstract probes and validates only the common permitted-type, definite-length, shortest-encoding, no-trailing-data, depth, and Section 4.2.1 ordering rules. No Pack Control Record schema applies and `null` is not part of either probe.

- Ordering probe: map `{24: 0, h'': 0}`. Canonical key order is unsigned key 24 then the empty byte-string key; Section 4.2.3 length-first order reverses them and is invalid.
- Depth probe `N`: exactly `N` nested one-element arrays ending in unsigned integer zero. The outer array is depth 1. `N = 32` is valid and `N = 33` is invalid.

Corpus authorship values are `independently-authored`, `deterministically-generated`, and `mutation-derived`. A generator record contains a namespaced generator ID, semantic version, exact source Content Identity, and seed; fields are null when not applicable.

Semantic-location subject kinds and canonical JSON keys are:

| Rank | Kind | Subject key JSON and comparator |
| ---: | --- | --- |
| `0` | `archive` | `null` |
| `1` | `cbor` | RFC 6901 JSON Pointer string, exact UTF-8 byte order |
| `2` | `object` | lowercase 64-digit digest string, decoded digest-byte order |
| `3` | `path` | exact UTF-8 path string, byte order |
| `4` | `variant` | zero-based declaration-index integer, numeric order |
| `5` | `package` | canonical `@namespace/name:major.minor.patch`, namespace/name byte order then numeric version |
| `6` | `font` | `[typed-container-identity-string, face-index-or-null]`, typed-identity order then null before numeric face index |
| `7` | `extension` | `[identifier, epoch]`, identifier byte order then numeric epoch |
| `8` | `pack` | `null` |
| `9` | `receipt` | receipt-role integer, numeric order |

The semantic location tuple is `[subject-kind, subject-key, zero-based-occurrence]`. Canonical issue ordering compares subject-kind rank, the field comparator above, rule-code ASCII bytes, then occurrence numerically. Subject keys never contain rendered error text.

Receipt vectors run under one implementation-neutral environment: adapter class `org.typst-pack.corpus.memory`, resource profile `org.typst-pack.corpus.format-ceilings`, and enforcement facts `org.typst-pack.corpus.immutable-input`, `org.typst-pack.corpus.memory-output`, and `org.typst-pack.corpus.no-ambient-io`. Each vector supplies every requested limit and expected admitted limit explicitly; independent implementations project these corpus identifiers rather than their production adapter names. Timing is not requested.

Corpus limit kinds and units are:

| Identifier | Unit |
| --- | --- |
| `org.typst-pack.limit.input-bytes` | bytes |
| `org.typst-pack.limit.output-bytes` | bytes |
| `org.typst-pack.limit.control-bytes` | bytes |
| `org.typst-pack.limit.object-count` | objects |
| `org.typst-pack.limit.decoded-bytes` | bytes |
| `org.typst-pack.limit.file-count` | files |
| `org.typst-pack.limit.cbor-depth` | aggregate nesting levels |
| `org.typst-pack.limit.temporary-bytes` | bytes |
| `org.typst-pack.limit.transfer-bytes` | bytes |

Corpus transport-failure cause identifiers are:

```text
org.typst-pack.corpus.failure.reference
org.typst-pack.corpus.failure.acquisition
org.typst-pack.corpus.failure.spooling
org.typst-pack.corpus.failure.transfer
org.typst-pack.corpus.failure.commit
org.typst-pack.corpus.failure.cleanup
org.typst-pack.corpus.failure.adapter-contract
```

Each maps to the same-named nonzero failure class.

Every exact byte artifact has a length and SHA-256 in `corpus.json`. `corpus.schema.json` is a closed JSON Schema with `additionalProperties: false` at every object. It enumerates the operation, terminal, validity, phase, receipt-role, and provenance values defined here; defines semantic location as the tuple `[subject-kind, canonical-subject-key, occurrence]`; and requires exact lowercase hexadecimal identities and artifact digests. Golden bytes are not silently regenerated. A corpus revision may add vectors without changing existing outcomes; changing accepted bytes or an outcome requires explicit compatibility review and, when the wire contract changes, a new Pack Format Epoch.

Every vector descriptor records:

```json
{
  "vector_id": "E2-ZIP-I-001",
  "epoch": 2,
  "operation": "read_pack_archive",
  "input": {
    "path": "invalid/zip/prefix.zip",
    "length": 1234,
    "sha256": "..."
  },
  "capabilities": {
    "pack_epochs": [2],
    "semantic_extensions": [],
    "package_metadata_profiles": ["org.typst.typst-0-15.package-metadata"],
    "font_metadata_profiles": ["org.typst.typst-0-15.font-metadata"],
    "unicode_versions": ["16.0.0"],
    "archive_encoding_recipes": ["typst-pack:archive-encoding:1:sha256:..."]
  },
  "limits": { "profile": "format-only" },
  "expected": {
    "terminal": "invalid",
    "validity": "invalid",
    "phase": 1,
    "code": "zip.prefix-forbidden",
    "issues_complete": true,
    "pack_exposed": false
  },
  "provenance": {
    "authorship": "independently-authored",
    "generator": null,
    "seed": null,
    "derived_from": null,
    "mutations": []
  }
}
```

Capability arrays are sorted unique sets by numeric epoch or exact ASCII bytes as applicable. A vector's unsupported outcome is evaluated against exactly these declared capabilities rather than the implementation's production configuration.

Error text is not an oracle. Terminal, validity, phase, primary code, semantic location, report completeness, identities, exact bytes, and exposure state are. Every required invalid vector selects one code from the complete registry above. Implementations may expose additional non-oracle detail. Multi-failure precedence is normative only for vectors explicitly marked as precedence cases.

### Required Valid Logical Packs

| ID | Case | Required proof |
| --- | --- | --- |
| `LP-000` | minimal Pack | one `main.typ`, one baseline read, no dependencies or extensions |
| `LP-010` | explicit inclusion | one unobserved baseline file included explicitly |
| `LP-020` | override-only observation | baseline blob present, replacement absent, commitment in request and trace |
| `LP-021` | override-only entrypoint | fixed entrypoint read only through a discovery override |
| `LP-022` | unread override target | target admitted by Explicit Conditional Inclusion |
| `LP-030` | external package | complete descriptors present and package blobs absent |
| `LP-031` | external font | container descriptor and face metadata present, container absent |
| `LP-032` | external package and font | ingress succeeds without authority acquisition |
| `LP-033` | disposition matrix | all four package/font embedded-external combinations |
| `LP-040` | blob deduplication | equal bytes across roles produce one physical blob |
| `LP-041` | incidental external digest | external disposition remains external despite equal embedded bytes |
| `LP-050` | multiple variants | labels and declaration order excluded from semantic identities |
| `LP-051` | ordered font catalog | reversed catalog changes Pack Identity |
| `LP-060` | extension and annotation | understood semantic extension and preserved opaque annotation |
| `LP-061` | metadata-only variation | Pack Identity stable, control and representation identities change |

Each logical fixture has:

- implementation-neutral semantic JSON;
- canonical `pack.cbor`;
- at least one Stored archive;
- canonical Closure Export;
- every intermediate identity payload, transcript, and digest;
- expected implementation-neutral logical Pack projection;
- Archive, control-record, and Closure Export content identities; and
- Derive and Verify ingress receipts.

At least one complete valid corpus is authored without the production writer, preferably in a second language. The production writer is tested against the corpus and is never the reader oracle.

### Required Identity Vectors

Each identity vector contains:

```text
abstract-value.json
payload.bin
transcript.bin
digest.hex
wire-tuple.cbor
expected.json
```

Vectors cover every registered kind and include:

- empty, ASCII, NUL-containing, and non-UTF-8 content;
- reordered tree inputs that produce equal identities;
- path, size, or content changes that produce unequal identities;
- equal package trees under different Package Specifications;
- commitment role and logical-key separation;
- empty and non-ASCII sensitive test values;
- variant label and declaration-order exclusions;
- extra unused input, feature, and override changing Discovery Variant Identity;
- access order and repeat count excluded from Discovery Trace Identity;
- either member changing Discovery Coverage Identity;
- every Pack included-versus-excluded field pair;
- font catalog order changing Pack Identity;
- metadata, provenance, annotation, and archive recipe preserving Pack Identity; and
- two Archive Encoding Identities for one exact Pack representation input.

Synthetic raw values used for commitment vectors are public test data so every implementation can reconstruct the digest.

### Required Canonical CBOR Invalids

The corpus includes one minimized vector for each:

- non-shortest unsigned integer and each non-shortest length form;
- indefinite byte string, text string, array, and map;
- wrong map-key order and duplicate map key;
- negative integer, every float width, tag, bignum, `undefined`, and arbitrary simple value;
- invalid UTF-8, truncation, trailing data, and CBOR sequence;
- permitted CBOR type in the wrong field, wrong map/array shape, scalar outside range, unknown core field, unknown core enum, missing mandatory field, omitted explicit absence, and unauthorized `null`;
- one standalone `validate_canonical_cbor_profile` map whose Section 4.2.1 bytewise order differs from Section 4.2.3 length-first order, with only the Section 4.2.1 form valid;
- unsorted semantic set, duplicate set member, and reordered intentional-list control case;
- depth 32 valid and depth 33 invalid; and
- NFC and NFD strings preserved as distinct exact text where their bytes differ.

### Required ZIP Invalids

Vectors cover:

- prefix, trailing bytes, comments, multiple EOCDs, and multidisk fields;
- encryption, data descriptor, signatures, archive extra records, central-directory encryption, and unknown records;
- directory, link, device, sparse, and unknown entries;
- wrong creator host, attributes, timestamp, flags, and version fields;
- missing, duplicate, renamed, compressed, or non-first `pack.cbor`;
- malformed, uppercase, duplicate, missing, extra, and unreferenced blob names;
- local or central order mismatch and local-central header disagreement;
- CRC, compressed-size, decoded-size, offset, and method mismatch;
- a valid entry whose literal CRC-32 is `0xffffffff`;
- out-of-bounds and overlapping records or payloads, central-directory crossing, arithmetic overflow, and truncation;
- incomplete Deflate, trailing Deflate input, nonzero final padding, expansion-limit breach, corrected CRC with wrong kind 1 Exact Content Identity, and corrected blob name with stale descriptor;
- oversubscribed, incomplete, legal single-symbol, missing end-of-block, empty-distance-without-length, and illegal empty-distance-with-length Huffman cases;
- unnecessary, missing, duplicate, reordered, malformed, or inconsistent ZIP64 fields; and
- ZIP32/ZIP64 boundaries immediately below, exactly at, and above `0xffff` and `0xffffffff`.

### Required Closure Export Invalids

Vectors cover:

- wrong or second root;
- missing, duplicate, or nested `pack.cbor`;
- extra file or directory;
- missing, uppercase, malformed, wrong-content, and unreferenced blob;
- symlink, hard-link alias, device, socket, FIFO, and other special node;
- file-directory conflict and wrapper case collision;
- inconsistent finite-tree snapshot;
- edited control record with stale identities; and
- valid content with arbitrary host metadata, which remains valid because host metadata is ignored.

### Required Whole-Pack Invalids

Vectors pass CBOR and representation framing and fail only at the authoritative whole-Pack seam:

- absent entrypoint;
- extra or omitted project binding;
- observed path without baseline bytes;
- override observation without commitment or matching request entry;
- baseline success for an overridden path, override success for an unoverridden path, and both success tags for one logical request;
- override replacement bytes serialized as a Pack role;
- override-only path without baseline bytes;
- unread override target without effective inclusion;
- empty variant set, duplicate variant identity, or stale trace/coverage identity;
- undeclared project, package, or font trace reference;
- missing probe contradicting a contained file;
- stale baseline descriptor;
- missing, extra, or inconsistent Package Requirement;
- package count, size, tree identity, manifest, or compiler-bound mismatch;
- invalid Version Bound shape and embedded metadata disagreeing with the declared supported profile;
- empty font face set, invalid face index, stale metadata, missing or duplicate catalog face;
- embedded content absent or external content treated as embedded by digest coincidence;
- missing and unreferenced semantic-extension objects;
- annotation affecting Pack Identity; and
- stale leaf, aggregate, coverage, engine, or claimed Pack Identity.

A real SHA-256 collision is not fabricated. Collision handling is a non-wire implementation test using a private injected digest oracle; it is not an archive interoperability vector.

### Required Unsupported Vectors

Vectors distinguish unsupported from invalid for a well-formed unknown Pack Format Epoch, semantic-extension identifier or epoch, package metadata profile, and font metadata profile. A wrong typed-identity kind, schema, or algorithm in any fixed Epoch 2 control-record field is instead invalid.

### External Fulfillment Vectors

Separate fixtures cover exact and mismatched Complete Package Trees and Font Containers, missing and extra package members, stale `typst.toml` summary, absent face indices, metadata mismatch, and every authority outcome. They prove that Pack ingress performs no external acquisition and that fulfillment independently validates exact requirements.

### Receipt And Commit Vectors

Vectors cover:

- successful Stored and Deflate encoding receipts;
- repeat encoding under one Archive Encoding Identity producing equal bytes;
- Derive ingress with no Archive Encoding Identity;
- Verify ingress match and mismatch;
- externally asserted and byte-verified Archive Encoding Identity;
- expected input Content Identity mismatch and supported Archive Encoding assertion mismatch;
- every adjacent pair in the external-check precedence order;
- invalid framing and validity-unknown resource outcomes;
- Closure Export projection and import convergence;
- `publish_pack_archive` complete-collection, each-object, and streaming strengths where applicable;
- `publish_closure_export` complete-collection atomic publication and weaker-sink admission refusal;
- cancellation immediately before commit winning;
- cancellation immediately after commit not rewriting success; and
- primary failure with complete cleanup or a separately gated residual locator.

Timing values are omitted from goldens or checked only for typed presence.

### Boundary Strategy

The following non-normative harness strategies avoid checking in huge artifacts; the exact boundary values and expected outcomes remain normative:

| Tier | Purpose |
| --- | --- |
| checked-in | small exact byte artifacts and path/nesting edges |
| generated | deterministic large archives with frozen generator version and seed |
| virtual planner | counting and sparse random-access fakes for `2^63` and `2^32` edges |

Every format and operation limit has `limit - 1`, `limit`, and `limit + 1` cases. The same otherwise-valid value succeeds under generous operation limits and returns validity unknown under a stricter operation limit. Exceeding a format ceiling is invalid under every adapter and trust profile.

### Mutation Corpus

Every derived invalid names one valid parent and an ordered mutation transcript. Mutations progress through:

1. surface framing corruption;
2. transport-consistent changes that repair ZIP CRC and lengths but leave semantic identity stale;
3. leaf-consistent changes that repair leaf descriptors but leave aggregate identity stale; and
4. semantically contradictory changes that preserve all local encodings.

Each mutation targets one primary rule unless explicitly marked as an error-precedence vector. Minimized fuzz regressions enter the corpus only after independent review assigns stable expected terminal, phase, and primary code.

## Compatibility And Change Control

Readers advertise a finite set of exact Pack Format Epochs and semantic-extension epochs. They never advertise a version range.

A writer emits one explicitly selected epoch only when every Pack value is representable without omission, synthesis, repair, reacquisition, or changed meaning. Exact archive copy, same-epoch semantic rewrite, and cross-epoch migration are distinct operations. A semantic rewrite preserves Pack Identity and every Pack Annotation but may change Archive Content Identity and Archive Encoding Identity.

Changes to any of these require a new Pack Format Epoch:

- core fields or enum meanings;
- canonical CBOR profile;
- path rules;
- ZIP acceptance or fixed metadata;
- identity transcript or projections;
- whole-Pack invariants;
- defaults or absence rules; or
- unknown-field policy.

Same-epoch evolution is limited to:

- newly registered Pack Semantic Extension epochs;
- opaque Pack Annotations;
- newly registered package and font metadata profiles whose identifiers bind complete immutable derivation contracts and vectors;
- Archive Encoding Identities and Deflate streams already permitted by the fixed profile; and
- corrections that reject bytes already invalid under this contract.

Migration fully validates the source epoch and writes the target from canonical logical state. It never reruns discovery, repairs evidence, reacquires missing dependencies, or invents defaults. Downgrade succeeds only when every field, extension, annotation, invariant, and meaning is losslessly representable. Explicit annotation dropping is a lossy metadata projection, not downgrade. If an identity schema changes, migration reports distinct old and new identities rather than asserting equality. Historical Epoch 1 Packs generally require fresh creation.

## Independent Implementation Gate

Epoch 2 is independently implementable when:

- two independent decoders agree on every valid implementation-neutral logical Pack projection, Pack Identity, and Representation Receipt projection;
- both reject every required corpus invalid vector with the same terminal class, validity state, phase, and primary code;
- Archive and Closure Export ingress converge on the same logical Pack;
- at least one complete corpus was not emitted by the production writer;
- every identity digest is reproducible from published payload and transcript bytes;
- resource vectors prove invalid representation versus validity unknown;
- every writer claiming an Archive Encoding Identity reproduces that recipe's golden bytes;
- unknown semantic extensions fail closed; and
- unknown annotations round-trip exactly without changing Pack Identity.

Materializing the corpus bytes and implementing these interfaces are implementation work. Their required contents and oracles are fixed here and leave no representation design choice to implementation planning.

## Recommendation Summary

Accept this direction with these explicit conclusions:

1. Epoch 2 is one closed canonical CBOR semantic record shared by a narrow ZIP Archive and exact Closure Export tree.
2. Identity transcripts use numeric kind-local schemas, fixed big-endian lengths, SHA-256, and canonical CBOR projections.
3. Project closure includes override-only observed paths while storing only their baseline bytes; private override bytes survive only as role/path-bound commitments.
4. Validation first proves safe framing, then collects canonical semantic issues, then converges on one private whole-Pack construction seam.
5. Archive Encoding Identity identifies a deterministic writer recipe and appears normally only in encoding receipts; archive bytes and Pack Identity do not reveal it.
6. Receipt semantics preserve validation, encoding, projection, publication, and cleanup as separate facts.
7. The independent corpus, not the production writer, is the compatibility oracle.

No additional wayfinding ticket is required by this recommendation. The existing Rust-interface and first-party-adapter tickets consume this contract, and the final approval ticket verifies the join.
