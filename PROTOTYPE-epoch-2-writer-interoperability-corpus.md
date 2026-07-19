# PROTOTYPE: Epoch 2 Writer And Interoperability Corpus

> Throwaway decision artifact for [Freeze the Epoch 2 writer and interoperability corpus](https://github.com/sagikazarmark/typst-pack/issues/64). It is not production documentation and must not be merged as-is.

## Status

**Recommended resolution: register only the universal all-Stored writer for the first Epoch 2 release. Keep the complete Deflate reader contract and test it with independently authored valid, invalid, boundary, and mixed-method archives.**

This artifact is a normative delta over the accepted [Pack Format Epoch 2 contract](https://github.com/sagikazarmark/typst-pack/blob/a490abc80af173422049ced1bf02585ddf7fc298/PROTOTYPE-pack-format-epoch-2.md). It resolves the writer-corpus contradiction found by [Approve the implementation-planning specification](https://github.com/sagikazarmark/typst-pack/issues/58) without reopening Pack validity, Archive Encoding Identity, Discovery Trace, Format Receipt, or Transport Receipt semantics.

All accepted Epoch 2 rules remain unchanged unless this artifact explicitly replaces one corpus requirement. In particular:

- Pack Archive readers still accept Stored method 0 and raw Deflate method 8 under the narrow ZIP profile.
- `typst-pack/pack.cbor` is always the first entry and always Stored.
- Generic ingress never derives Archive Encoding Identity from archive bytes.
- A claimed Archive Encoding Identity is reported by ingress only after supported-recipe re-encoding and complete byte equality.
- Representation Receipt Contract version 1 remains unchanged.
- Transport Receipt semantics remain outside this decision.

## Decision

### Initial writer set

The initial Epoch 2 Archive Encoding Recipe Registry contains exactly one production recipe: `epoch-2-all-stored-v1`.

No Deflate writer recipe is registered. An encoder MUST refuse every other recipe before encoding effects and MUST NOT return a successful archive-encode Format Receipt for an unregistered recipe.

Deflate remains a required reader capability. A conforming implementation can read a valid Deflate or mixed-method Pack Archive without knowing which software produced it, but it cannot attach an Archive Encoding Identity to that archive unless a supported recipe was externally asserted and exact re-encoding proves it.

Adding a deterministic Deflate recipe later is same-epoch registry evolution. It requires a new Archive Encoding Identity and the complete recipe and corpus evidence specified under [Future writer recipes](#future-writer-recipes); it does not change Pack Identity or weaken the existing reader profile.

### Why all-Stored first

RFC 1951 defines valid Deflate streams, not a canonical compressor. An honest deterministic Deflate recipe would have to freeze the compressor algorithm or exact normative implementation, source and build identity, method selection, match finding, block splitting, Huffman construction, tie-breaking, invocation, finalization, platform qualifications, and golden bytes. None is currently frozen.

Stored encoding already has every byte-affecting choice fixed by the narrow ZIP profile. It is portable across implementation languages, has predictable resource use, avoids a permanent compressor compatibility obligation, and is sufficient to issue interoperable Epoch 2 Packs. The cost is larger archives; compactness does not outrank portability and predictable operation in the accepted product priorities.

## Archive Encoding Recipe Registry

### Registry entry `epoch-2-all-stored-v1`

| Field | Exact value |
| --- | --- |
| Public selector | `epoch-2-all-stored-v1` |
| Constant-style selector | `EPOCH_2_ALL_STORED_V1` |
| Pack Archive profile revision | `1` |
| Producer | `org.typst-pack.epoch2` |
| Recipe schema | `org.typst-pack.archive.all-stored` |
| Recipe schema epoch | `1` |
| Writer implementation version | `1.0.0` |
| Build fingerprint | empty byte string |
| Recipe parameters | empty byte string |
| `pack.cbor` method | Stored, method `0` |
| Every blob method | Stored, method `0` |

Its canonical Archive Encoding payload is:

```cbor
[
  1,
  "org.typst-pack.epoch2",
  "org.typst-pack.archive.all-stored",
  1,
  [1, 0, 0],
  h'',
  h''
]
```

The canonical payload is 66 bytes:

```text
8701756f72672e74797073742d7061636b2e65706f63683278216f72672e74797073742d7061636b2e617263686976652e616c6c2d73746f72656401830100004040
```

Under identity kind `12`, schema `1`, and algorithm `1`, its Archive Encoding Identity is:

```text
typst-pack:archive-encoding:1:sha256:4e338d8a54d234ca28392ecf79386944757e0e4adf750192e21311d6b2491170
```

The complete 102-byte identity transcript is:

```text
74797073742d7061636b206964656e7469747900000000010000000c00000000000000428701756f72672e74797073742d7061636b2e65706f63683278216f72672e74797073742d7061636b2e617263686976652e616c6c2d73746f72656401830100004040
```

The materialized registry MUST publish the payload bytes, complete identity transcript bytes, and typed identity independently of any writer implementation.

### Writer behavior

For identical Pack Control Record bytes, canonical blob path and byte inventory, and the typed Archive Encoding Identity selected by `epoch-2-all-stored-v1`, every conforming writer MUST produce identical complete Pack Archive bytes.

The writer:

1. Accepts only a constructed Pack and its exact representation state.
2. Plans `pack.cbor` first and the required blobs in lowercase digest order.
3. Selects Stored method 0 for every entry.
4. Applies the accepted header values, CRC-32, ZIP32 and ZIP64 thresholds, attributes, timestamps, local and central ordering, and exact gap-free layout.
5. Produces one completed Stable Byte Value before archive encoding commits.
6. Returns the frozen archive-encode Format Receipt projection with the registered Archive Encoding Identity and exact output identities and counts.

It MUST NOT:

- emit a method-8 entry under this identity;
- vary bytes by platform, time, locale, process state, map iteration order, destination, resource pressure, or transport behavior;
- use Pack Identity as a substitute for the exact control-record bytes and blob inventory;
- claim the recipe merely because an archive appears all-Stored; or
- silently fall back to another recipe.

### Permanently unregistered corpus probe

Corpus revision 1 reserves one syntactically valid Archive Encoding payload solely for unsupported-recipe tests. It is never a production recipe and MUST NOT appear in a successful encode receipt or supported-recipe capability set.

| Field | Exact value |
| --- | --- |
| Producer | `org.typst-pack.test` |
| Recipe schema | `org.typst-pack.test.archive.unregistered` |
| Recipe schema epoch | `1` |
| Writer implementation version | `1.0.0` |
| Build fingerprint | empty byte string |
| Recipe parameters | empty byte string |

Its 71-byte canonical payload is:

```text
8701736f72672e74797073742d7061636b2e7465737478286f72672e74797073742d7061636b2e746573742e617263686976652e756e7265676973746572656401830100004040
```

Its typed identity is:

```text
typst-pack:archive-encoding:1:sha256:7e03af20e9fb79c04f206ceb9d69164cd68f6caaafcdb1a5ae8cb209e8438ead
```

The complete 107-byte identity transcript is:

```text
74797073742d7061636b206964656e7469747900000000010000000c00000000000000478701736f72672e74797073742d7061636b2e7465737478286f72672e74797073742d7061636b2e746573742e617263686976652e756e7265676973746572656401830100004040
```

This vector proves that a well-formed typed identity is not necessarily a supported recipe. The test namespace is permanently excluded from production registration.

### Future writer recipes

A future registered recipe MUST publish all of the following before any implementation advertises it:

- the complete canonical Archive Encoding payload, transcript, identity, selector, and parameter schema;
- deterministic Stored-versus-Deflate selection for every possible entry;
- every byte-affecting writer choice;
- exact output archives for the required logical fixtures;
- repeat-encoding vectors;
- cross-platform qualification for every target that claims the recipe; and
- evidence that the production writer is not the sole oracle.

A Deflate recipe additionally MUST freeze either one complete implementation-neutral compressor specification or one exact normative implementation artifact, including version and build fingerprint. It MUST freeze raw RFC 1951 invocation, window and matching behavior, block splitting, Stored/fixed/dynamic selection, Huffman construction and ties, dynamic-header encoding, bit packing, input chunking, flushing, finalization, empty and incompressible inputs, and every platform distinction that can affect bytes. Resource exhaustion fails the operation and MUST NOT select different bytes or silently fall back to another recipe.

Changing any byte-affecting choice creates a new Archive Encoding Identity. A finite golden set cannot replace the recipe definition.

## Corpus Revision 1

### Purpose

The Epoch 2 interoperability corpus is a versioned collection of immutable inputs and implementation-neutral oracles. It proves:

- canonical control-record and identity behavior;
- agreement of Pack Archive and Closure Export ingress;
- exact output from each registered writer recipe;
- acceptance of the complete valid Stored and Deflate reader profiles;
- rejection and precedence behavior for invalid or unsupported inputs;
- source and raw-file Discovery Trace semantics; and
- exact `format-receipt` projections of Representation Receipt Contract version 1.

The corpus does not prescribe parser libraries, private type layout, buffering strategy, test harness, or Transport Receipt serialization.

### Root layout

Corpus revision 1 has this declared root:

```text
epoch-2/
  corpus.json
  corpus.schema.json
  registries/
    archive-encoding-recipes.json
    identities.json
    operations.json
    outcomes.json
    request-kinds.json
    format-receipt.json
  identities/
  logical-packs/
  control-records/
  archives/
    writers/all-stored-v1/
    readers/stored/
    readers/deflate/
    readers/mixed/
  closure-exports/
  fulfillments/
  extensions/
  receipts/format/
  invalid/
    cbor/
    zip/
    closure/
    invariants/
    mutations/
  boundaries/
```

`corpus.json` declares Pack Format Epoch `2`, corpus revision `1`, every registry asset, and every vector. `corpus.schema.json` is closed with `additionalProperties: false` at every object.

Every exact byte artifact record contains:

- stable vector ID and role;
- relative path;
- exact byte length;
- SHA-256 content digest;
- authorship: `independently-authored`, `deterministically-generated`, or `mutation-derived`;
- source vector IDs; and
- for deterministically generated artifacts, a namespaced generator ID, semantic version, exact source Content Identity, and seed;
- for mutation-derived artifacts, the valid parent vector and ordered mutation transcript.

Golden bytes are immutable inputs. Tests fail when they disagree and MUST NOT silently regenerate them. Changing accepted bytes, registries, or outcomes requires compatibility review; a change to the Epoch 2 wire contract requires a new Pack Format Epoch rather than a corpus revision alone.

### Closed operation registry

Revision 1 retains the accepted operations:

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

Every vector declares the exact capabilities under which its expected result is evaluated. A capability-relative `unsupported` result is not inferred from a production build's incidental configuration.

### Oracle shape

Each vector records every applicable oracle from this closed set:

- terminal class;
- validity state;
- terminal phase;
- primary stable rule code;
- semantic location;
- issue-list completeness;
- exposed or withheld Pack state;
- exact implementation-neutral logical projection;
- exact typed identities and identity payloads;
- exact output bytes and inventories;
- exact Format Receipt version-1 projection; and
- exact capability-relative unsupported state.

Error text is never an oracle. Multiple-failure order is normative only for vectors explicitly marked as precedence vectors.

## Required Writer Vectors

Every valid logical Pack in the accepted `LP-000` through `LP-061` inventory and the request-kind additions below has:

- implementation-neutral semantic JSON;
- exact canonical `pack.cbor` bytes;
- every intermediate identity payload, transcript, and typed identity;
- one canonical Closure Export;
- one `epoch-2-all-stored-v1` Pack Archive;
- exact Archive and Closure Export content identities; and
- exact Derive and Verify Format Receipt projections.

At minimum, the corpus includes these writer vectors:

| Vector | Required oracle |
| --- | --- |
| `WR-STORED-000` | Minimal Pack encodes to exact all-Stored bytes and successful archive-encode Format Receipt. |
| `WR-STORED-010` | Multi-blob Pack uses canonical digest order and exact headers and directory order. |
| `WR-STORED-020` | Metadata-only control-record change preserves Pack Identity but changes archive bytes and Archive Content Identity. |
| `WR-STORED-030` | ZIP64 planning immediately below, at, and above every applicable sentinel threshold. |
| `WR-STORED-040` | Repeated encoding is byte-for-byte and complete-receipt-field identical under the fixed corpus environment. |
| `WR-RECIPE-INVALID-000` | The permanently unregistered corpus probe is refused before encoding effects. |

The former requirement for successful Deflate encoding is deleted. No revision-1 vector may expect a successful Deflate archive-encode receipt.

The former requirement for two registered Archive Encoding Identities over one representation input is replaced by:

1. the one registered all-Stored identity;
2. the permanently unregistered identity used only for admission-refusal and typed-identity derivation; and
3. Stored, Deflate, and mixed archives that decode to the same representation input while generic ingress reports no inferred recipe.

## Discovery Trace Request-Kind Vectors

The request-kind registry remains:

| Wire value | Canonical corpus name | Meaning |
| ---: | --- | --- |
| `0` | `typst-source` | Typst source request |
| `1` | `raw-file` | raw-file request |

`load`, `probe`, and other aliases are forbidden. Request kind says what Typst requested; baseline read, override read, and missing are separate observation outcomes.

The corpus adds these logical fixtures:

| Fixture | Required facts |
| --- | --- |
| `LP-062` | Project baseline source read, baseline raw-file read, one path read once under each kind, and source and raw-file missing observations. The fixed entrypoint is read as source. |
| `LP-063` | Distinct override-backed project observations under source and raw-file kinds; baseline bytes remain contained and private replacements remain absent. |
| `LP-064` | Package source read, raw-file read, one package path read once under each kind, and source and raw-file missing observations. |
| `LP-065` | Source member of an otherwise identical source/raw identity pair. |
| `LP-066` | Raw-file member of the pair; tree and blob inventory equal `LP-065`, while Trace, Coverage, Pack, control-record, and archive identities differ. |

`LP-062` includes at least this canonical path-first fact inventory:

```text
data.bin     baseline-read  raw-file
dual.typ     baseline-read  typst-source
dual.typ     baseline-read  raw-file
main.typ     baseline-read  typst-source
missing.bin  missing        raw-file
missing.typ  missing        typst-source
```

The valid and invalid request-kind vectors prove:

- at most one terminal per `(project path, request kind)` and per `(Package Requirement Identity, package path, request kind)`;
- the same path under both request kinds is valid and contributes one baseline project or package binding;
- exact project ordering by `(path UTF-8 bytes, request kind, observation tag)` and package ordering by `(Package Requirement Identity, path UTF-8 bytes, request kind, observation tag)`; source precedes raw-file only for the same path;
- duplicate terminals for one path and kind are invalid;
- success and missing for one path and kind are invalid;
- baseline and override success for one path and kind are invalid;
- unknown request kind `2` is invalid as `cbor.unknown-enum`;
- wrong observation order is invalid as `trace.order`;
- a missing observation naming contained content is invalid as `trace.outcome`;
- request kind contributes to Discovery Trace Identity; and
- an entrypoint observed only as raw-file is invalid.

## Deflate Reader Vectors

Revision 1 keeps every accepted Deflate invalid and boundary vector and adds explicit positive ingress coverage. Each positive archive is independently authored and is not emitted by the production writer.

| Vector | Required Deflate form |
| --- | --- |
| `RD-DEFLATE-000` | One raw non-compressed Deflate block (`BTYPE=00`). |
| `RD-DEFLATE-010` | Fixed-Huffman block with literals, valid back-reference, and end-of-block. |
| `RD-DEFLATE-020` | Dynamic-Huffman block with literals, valid back-reference, and end-of-block. |
| `RD-DEFLATE-030` | All declared distance-code lengths are zero and no length symbol is emitted. |
| `RD-DEFLATE-040A` | Dynamic block whose literal/length tree contains only end-of-block symbol 256 with code length 1. |
| `RD-DEFLATE-040B` | Dynamic block with one distance symbol of code length 1 and a valid emitted length/distance pair. |
| `RD-DEFLATE-050` | Multiple raw Deflate blocks ending in exactly one final block. |
| `RD-MIXED-000` | One Pack Archive containing both Stored and Deflate blob entries. |

Every positive vector:

- keeps `pack.cbor` Stored;
- decodes to an exact declared blob inventory;
- requires the final block's logical end to lie in the final compressed byte and verifies every remaining high bit is zero;
- verifies declared sizes, expansion ratio, CRC-32, Content Identities, and blob names;
- produces the same Pack Identity and Closure Export Tree Content Identity as its all-Stored equivalent;
- has a different Archive Content Identity when its bytes differ; and
- produces a successful archive-read Format Receipt with Archive Encoding Identity absent and assertion status `not-asserted`.

Required invalid vectors retain trailing compressed input, concatenated streams, incomplete streams, nonzero final padding, oversubscribed and impermissibly incomplete trees, missing end-of-block, forbidden symbols, truncated length, distance, and code-length-repeat extra-bit fields, code-length repeats exceeding the declared combined alphabet, distances invalid after decoding their extra bits, decoded-size mismatch, expansion-ratio breach, CRC mismatch, stale Content Identity, stale blob name, and every ZIP framing and ZIP64 error already frozen by the accepted contract.

Valid controls such as literal CRC-32 `0xffffffff`, the legal single-symbol tree, and the permitted empty distance tree are classified as valid controls rather than placed under an invalid heading.

## Format Receipt Vectors

### Scope

The corpus compares the already-frozen `format-receipt` projection of Representation Receipt Contract version 1. It does not add fields, change enum assignments, reinterpret nullability, collapse operation roles, or define a universal persisted receipt encoding.

Every expected receipt includes every mandatory field. Exact counters remain `null` until knowable; inapplicable values use the frozen null or not-applicable representation rather than a guessed zero.

The corpus environment is fixed as:

```text
adapter class:     org.typst-pack.corpus.memory
resource profile:  org.typst-pack.corpus.format-ceilings
enforcement facts:
  org.typst-pack.corpus.immutable-input
  org.typst-pack.corpus.memory-output
  org.typst-pack.corpus.no-ambient-io
timing: not requested
```

### Archive encode and read

| Vector | Expected Format Receipt result |
| --- | --- |
| `FR-AE-001` | Successful all-Stored encoding with source Pack Identity, control-record identity, registered Archive Encoding Identity, output Archive Content Identity, Closure Export Tree Content Identity, exact counts, and committed Stable Byte Value. |
| `FR-AE-002` | Repeat `FR-AE-001`; exact archive bytes, identities, and deterministic receipt fields agree. |
| `FR-AE-003` | Unregistered recipe admission refusal before effects. |
| `FR-AR-001` | All-Stored Derive ingress succeeds with Archive Encoding Identity absent and assertion status `not-asserted`. |
| `FR-AR-002` | All-Stored Verify ingress succeeds with expected Pack match. |
| `FR-AR-003` | Valid Pack reaches expected Pack mismatch after complete internal validation and exposes no Pack. |
| `FR-AR-004` | Independently authored Deflate Derive ingress succeeds with Archive Encoding Identity absent. |
| `FR-AR-005` | Independently authored Deflate Verify ingress succeeds with expected Pack match and Archive Encoding Identity absent. |
| `FR-AR-006` | All-Stored archive plus asserted all-Stored identity succeeds only after exact re-encoding and byte comparison, then reports that identity. |
| `FR-AR-007` | Valid Deflate archive plus asserted all-Stored identity reaches assertion mismatch after full Pack validation and exposes no Pack. |
| `FR-AR-008` | Unregistered asserted recipe is refused before archive interpretation. |
| `FR-AR-009` | Expected input Content Identity mismatch occurs before interpretation. |
| `FR-AR-010` | Explicit vectors cover every adjacent pair in the frozen external-check precedence order. |

There is no successful Deflate archive-encode receipt in revision 1.

### Closure Export and publication

Revision 1 retains the accepted Closure Export project and import, publication-strength, cancellation-before-and-after-commit, and cleanup vectors unchanged.

The vectors continue to prove:

- archive encoding commits when the completed Stable Byte Value exists;
- encoding success does not imply publication success;
- Closure Export projection and publication are separate roles;
- cancellation before commitment can win and cancellation after commitment cannot rewrite success;
- cleanup failure never replaces the primary failure; and
- expected input, internal validity, expected Pack Identity, and optional recipe assertion follow the frozen precedence.

### Transport boundary

Format Receipt is the format-owned required projection used by this corpus. It composes with, but does not replace, a Transport Receipt.

This decision and corpus do not define, change, or compare:

- Transport Receipt schemas or serialization;
- locator resolution, acquisition, spooling, retry, or backpressure;
- Transport Operation admission or transfer lifecycle;
- Publication Commit Strength or Transport Cleanup Strength semantics;
- residual Transport Locator representation;
- transport disclosure capabilities; or
- public Rust or first-party adapter receipt composition.

Publication vectors may drive a scripted sink and compare only the frozen format-receipt role projection. [Complete the Rust lifecycle and receipt interfaces](https://github.com/sagikazarmark/typst-pack/issues/65) owns the public Rust composition, and [Reconcile the first-party adapter schemas and profiles](https://github.com/sagikazarmark/typst-pack/issues/63) owns first-party serialization.

## Independence And Gates

### Independent authorship

At least one complete valid Pack, including semantic JSON, control record, identity transcripts, all-Stored archive, Closure Export, and expected Format Receipts, is authored without calling the production writer.

At least `LP-062` and every positive Deflate and mixed-method archive are independently authored. Valid Deflate streams do not use the production ZIP or Deflate stack as their oracle.

Expected identities are derived from checked-in payload and transcript bytes. Expected Format Receipts are authored from the frozen role matrix. A production implementation passing its own snapshots is insufficient.

### Decoder agreement

Before a public Epoch 2 writer prerelease, two independent decoders MUST:

- agree on every valid logical Pack projection, identity, object inventory, and Format Receipt projection;
- agree on invalid terminal, validity, phase, primary rule code, issue completeness, and exposure state;
- converge from Pack Archive and Closure Export on the same logical Pack;
- reproduce every identity from published payload and transcript bytes;
- distinguish representation-invalid from resource-unknown outcomes;
- fail closed on unknown Pack Semantic Extensions;
- preserve unknown Pack Annotations without changing Pack Identity; and
- pass the complete Stored and Deflate positive, invalid, and boundary reader corpus.

Every implementation claiming `epoch-2-all-stored-v1` MUST reproduce every registered writer golden archive byte-for-byte on every target where it advertises that recipe.

### Release sequencing

The scenario and registry contract is frozen by this decision. Materializing and independently reviewing the exact corpus bytes remains implementation and release-gate work, not additional wayfinding.

No public Epoch 2 writer prerelease may occur until:

1. corpus revision 1 bytes, closed schema, registries, per-artifact lengths and SHA-256 digests, and an immutable publication revision are published;
2. the independent-authoring requirements are met;
3. two independent decoders pass the required agreement gate; and
4. the first-party writer reproduces every `epoch-2-all-stored-v1` golden.

No implementation may advertise a Deflate writer recipe until a later registry addition satisfies [Future writer recipes](#future-writer-recipes) and extends the corpus with exact successful encode receipts and writer goldens for that new identity.

## Contract Corrections

This decision makes only these corrections to the accepted corpus requirements:

| Previous requirement | Revision-1 requirement |
| --- | --- |
| Successful Stored and Deflate encoding receipts | Successful all-Stored encoding receipts plus successful Stored, Deflate, and mixed-method archive-read receipts |
| Two Archive Encoding Identities for one representation input | One registered all-Stored identity, one permanently unregistered negative identity, and multiple reader-valid representations with no inferred recipe |
| Deflate positive behavior implied primarily through writer scenarios | Explicit independently authored positive Deflate and mixed-method ingress vectors |
| Request-kind semantics present in schema but not decisive in corpus | Explicit source/raw-file logical, identity, ordering, duplicate, entrypoint, and unknown-enum vectors |
| `corpus.schema.json` required but absent from the declared root | `corpus.schema.json` explicitly present and closed |
| Valid Deflate and ZIP controls grouped ambiguously with invalid cases | Every vector explicitly classified by its own terminal and validity oracle |

Everything else in the accepted Epoch 2 format, validation, identity, representation, receipt, and corpus contracts remains in force.

## Map Effect

This decision surfaces no new wayfinding ticket and graduates no fog. It unblocks the format inputs needed by [Reconcile the first-party adapter schemas and profiles](https://github.com/sagikazarmark/typst-pack/issues/63); the remaining Rust and adapter tickets own their respective interface and serialization corrections.
