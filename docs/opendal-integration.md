# OpenDAL integration

The optional `opendal` feature lets an application read Pack Assembly inputs,
read and write Pack Archives, and write extraction or compilation results
through caller-supplied OpenDAL `Operator` values. The application chooses the
backend, credentials, transport, TLS, layers, retry policy, executor, and
runtime. typst-pack owns no runtime and provides no blocking facade.

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["opendal", "package-reading"] }
opendal = { version = "0.58", default-features = false, features = ["services-s3"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The `opendal` feature selects no backend or runtime feature. OpenDAL's Memory
service is available without enabling a `services-memory` feature.

## Locations and operators

`Location` addresses a caller-defined `OperatorBinding` as
`binding:/root-relative/path`. Exact objects are non-root paths without a
trailing slash; prefixes are the root or paths with a trailing slash. Parsing
rejects aliases, dot segments, repeated separators, authorities, query strings,
fragments, backslashes, and noncanonical escaping.

Use `OperatorBindings` for an immutable map, or implement `OperatorResolver` to
resolve bindings dynamically:

```rust
use typst_pack::opendal::{OperatorBinding, OperatorBindings};

let operator = opendal::Operator::new(opendal::services::Memory::default())?;
let binding = OperatorBinding::new("documents")?;
let bindings = OperatorBindings::new([(binding, operator)])?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Each operation resolves and appraises every reached binding once. Resolver
errors cross the public boundary as `Box<dyn Error + Send + Sync>` and remain in
the error source chain. Operation `Display` and `Debug` output omit native
resolver and OpenDAL messages; rendering the complete source chain may expose
backend paths, endpoints, or other untrusted context.

## Current API

The crate root exports `opendal::Operator`, locations, bindings, and the resolver.
Workflow APIs stay in operation-specific modules.

### Reads

| Purpose | Request and operation | Result | Required limits | Synchronous handoff |
| --- | --- | --- | --- | --- |
| Project prefix | `ProjectReadRequest`, `read_project` | `ProjectRead` | `ProjectReadLimits` | `ProjectSnapshotAssembly::assemble` |
| Font prefixes | `FontReadRequest`, `read_fonts` | `FontRead` | `FontReadLimits` | `FontContainer::new`, then `FontCatalogEntry` |
| Package fallback | `PackageReadRequest`, `read_package` | `PackageRead` | `PackageReadLimits` | `insert_read_package`, then resume `create` |
| Pack Archive object | `PackArchiveReadRequest`, `read_pack_archive` | `PackArchiveBytes` | `pack_archive::ReadLimits` | `pack_archive::decode` |

Project, font, and package APIs are in `typst_pack::opendal::pack_assembly`.
Pack Archive reading is in `typst_pack::opendal::pack_archive`.

`read_package` checks caller-ordered Package Tree prefixes, then an optional raw
archive cache, then an optional official registry prefix. Only definite absence
advances to the next candidate. It returns raw values and does not expand an
archive or write a cache. With `package-reading`, `insert_read_package` builds or
expands the tree, validates and inserts it, updates `PackageReadFailures`, and
returns `RegistryArchiveResidue` only when registry bytes are safe to cache.

### Writes

All write APIs are in `typst_pack::opendal::write`.

| Purpose | Request and operation | Success evidence |
| --- | --- | --- |
| Pack Archive object | `PackArchiveWriteRequest`, `write_pack_archive` | `PackArchiveWriteReceipt` |
| Package cache object | `PackageCacheArchiveWriteRequest`, `write_package_cache_archive` | `PackageCacheArchiveWriteReceipt` |
| Pack Extraction Plan | `PackExtractionWriteRequest`, `write_pack_extraction_plan` | shared `PackExtractionWriteReceipt` |
| Compilation artifacts | `CompilationArtifactWriteRequest`, `write_compilation_artifacts` | shared `CompilationArtifactWriteReceipt` |

`WritePolicy::CreateOrVerify` creates an absent object or accepts an existing
byte-identical object. `WritePolicy::OverwriteExactKeys` writes every key without
distinguishing creation from replacement. Package-cache writes always use
`CreateOrVerify`.

`WriteKeyOutcome` reports `Created`, `AlreadyMatching`, or `Written`. Pack
Extraction and Compilation Output Artifact progress, entries, and receipts are
shared with the filesystem adapter and exported from the crate root. Single-key
OpenDAL operations use their module-specific progress and receipt types.

### Supporting types and errors

Each read operation exposes its exact values rather than a common storage
result. Project and font reads provide `*ReadEntry`, `*ReadIssue`,
`*ReadSurveyError`, `*ReadError`, and `*ReadErrorCause` types. Package reading
adds `PackageTreeRead`, `CachedPackageArchiveRead`,
`RegistryPackageArchiveRead`, `UnavailablePackageRead`, and the
`ReadPackageInsertion*` family. Request construction reports either a singular
`*RequestError` or an aggregated `*RequestRejection`, according to whether the
request has one or several independently invalid locations.

Each limits preset has matching `*Ceilings`, `*Resource`, `*Limits`, and
`*LimitError` names. These are operation-specific aliases of the shared generic
limits implementation, not interchangeable resource profiles. Invalid limit
construction is a programmer error and panics.

Writes expose `OpenDalWritePhase`, operation-specific `*WriteError` and
`*WriteErrorCause` types, and request validation errors. Compilation artifact
request validation uses `CompilationArtifactWriteRequestRejection`,
`CompilationArtifactWriteRequestIssue`, and `CompilationArtifactKeyIssue`.
Single-key operations expose `PackArchiveWriteEntry/Progress/Receipt` and
`PackageCacheArchiveWriteEntry/Progress/Receipt`; multi-key operations use the
shared crate-root evidence types named above.

## Capability appraisal

Appraisal uses `Operator::info().capability()` before storage work. It rejects
only incompatibilities visible in advertised capabilities. A backend can still
fail an operation after appraisal, and a service that advertises a capability
but implements it incorrectly can violate the advertised behavior.

| Operation | Location role | Required advertised capabilities |
| --- | --- | --- |
| Project, font, or Package Tree read | Prefix | `list`, `list_with_recursive`, `read` |
| Pack Archive or raw package archive read | Exact object | `read` |
| `CreateOrVerify` write | Exact object(s) | `read`, `write`, `write_with_if_not_exists`; `write_can_empty` for empty values; advertised size support |
| `OverwriteExactKeys` write | Exact object(s) | `write`; `write_can_empty` for empty values; advertised size support |

OpenDAL advertises `write_with_if_not_exists` statically for S3-compatible
services. Appraisal cannot detect an endpoint that ignores
`If-None-Match: *`; such an endpoint may overwrite instead of rejecting the
conditional create. Choose and test backends according to the guarantees your
application needs.

Listing is one completed observation, not a snapshot. A successful read covers
the entries yielded by that observation; it does not prove that all existing
objects were listed, that an object did not change between listing and reading,
or that all returned values coexisted.

## Limits

Read limits are aliases of the shared `Limits<ResourceKind>` family with an
operation-specific `reference_v1()` preset. Constructors validate finite
ceilings. Narrow a profile by copying its named ceilings and changing only the
application-specific values:

```rust
use typst_pack::opendal::pack_assembly::{ProjectReadCeilings, ProjectReadLimits};

let _limits = ProjectReadLimits::new(ProjectReadCeilings {
    object_bytes: 16 * 1024 * 1024,
    total_bytes: 64 * 1024 * 1024,
    ..ProjectReadCeilings::reference_v1()
});
```

| Operation | `reference_v1()` ceilings |
| --- | --- |
| Project read | 1,000,000 listed entries; 64 KiB one path; 256 MiB retained paths; 100,000 files; 256 MiB one object; 2 GiB total bytes |
| Font read | 100,000 listed entries; 64 KiB one path; 64 MiB retained paths; 16,384 containers; 256 MiB one container; 2 GiB total bytes |
| Package Tree read | 100,000 listed entries; 64 KiB one path; 64 MiB retained paths; 50,000 files; 64 MiB one object; 512 MiB total bytes |
| Raw package archive read | 128 MiB archive bytes per cache or registry candidate |
| Composite package read | Package Tree profile shared across tree candidates, plus the raw archive profile applied independently to cache and registry candidates |
| Pack Archive read | 512 MiB archive bytes |

For object families, one-object bytes cannot exceed total bytes. Count and path
limits are charged before payload limits; per-object limits are charged before
aggregate payload limits. Limit failures expose the operation-specific resource,
ceiling, and at least the first value beyond it.

These are typst-pack retention and observation limits. They do not bound memory
inside an OpenDAL backend, transport buffering, one yielded buffer, allocator
overhead, compiler memory, process RSS, elapsed time, or concurrent operations.
For example, the Memory service may yield an entire object as one buffer even
though typst-pack retains only the permitted prefix plus its overage probe.

Writes take caller-owned, already-materialized bytes and have no write limits.
The caller remains responsible for bounding those bytes before constructing the
semantic input.

## Lifecycle composition

OpenDAL performs storage I/O; synchronous core types remain authoritative for
paths, fonts, package trees, Pack Creation, archive encoding/decoding,
extraction planning, and compilation.

For Pack Assembly:

1. `read_project`, then assemble its entries with `ProjectSnapshotAssembly`.
2. `read_fonts`, validate each `FontContainer`, and build a `FontCatalog`.
3. Call `create` with those values and an initially empty `PackageCatalog`.
4. On `PackCreationOutcome::MissingPackageSpecifications`, call `read_package` for each exact specification.
5. Call `insert_read_package`; if it returns registry residue, optionally write those bytes with `write_package_cache_archive`.
6. Call `create` again until it returns `Created` or fails.

Cache writing is deliberately after expansion, validation, and catalog
insertion. Writing registry bytes earlier can make a malformed archive a
terminal cache hit. A cache-write failure does not invalidate a successfully
inserted Package Tree; retain the residue and report or replay it separately.

## External compilation fulfillments

There is no OpenDAL compilation-read API. Read external package archives and
font objects directly with the application's `Operator` values, enforce
application limits, construct core fulfillment values, and then call `compile`:

```rust,ignore
use typst_pack::{
    CompilationFulfillmentSet, FontContainerFulfillment, PackageExpansionLimits,
    PackageTreeFulfillment, expand_package_archive, resolve_external_font_requirements,
};

let package_bytes = package_operator.read(package_key).await?.to_vec();
check_package_download_limit(package_bytes.len())?;
let tree = expand_package_archive(
    package_spec.clone(),
    &package_bytes,
    PackageExpansionLimits::reference_v1(),
)?;
let package = PackageTreeFulfillment::new(package_spec, tree);

let font_bytes = font_operator.read(font_key).await?.to_vec();
check_font_download_limit(font_bytes.len())?;
let fonts = resolve_external_font_requirements(&pack, [font_bytes])?;
let fulfillments = CompilationFulfillmentSet::new([package], fonts)?;
let request = request.fulfillments(fulfillments);
let report = typst_pack::compile(request)?;
```

The core verifies complete package-tree identities, font-container identities,
and required font faces before invoking Typst. Direct reads are not implicitly
bounded by typst-pack; enforce transport and retained-byte ceilings at that
trust boundary.

## Recovery and partial effects

Every write borrows its input, so the caller retains exact replay material
without an extra copy. Full replay is the recovery contract. There is no
transaction, rollback, staging protocol, resume token, sub-plan API, portable
multipart guarantee, or multi-key atomicity.

`CreateOrVerify` makes replay useful: already matching objects complete as
`AlreadyMatching` without another destination effect. `OverwriteExactKeys`
replays writes but can replace values changed by another actor.

Multi-key operations are sequential. Their caller-owned progress is cleared
synchronously before the future is returned and then records the contiguous
completed prefix. If a polled future is cancelled or dropped, already-issued
effects may remain and the progress value is the surviving evidence. A returned
error contains the same progress plus `CommitCertainty` for the failed effect:
`NotCommitted`, `Committed`, or `Indeterminate`. A dropped future returns no
error or receipt, so commit certainty for its in-flight effect is unknown.

Successful progress and receipts describe observed outcomes, not a linearization
point or durable atomic commit. Mutable destinations can change immediately
after any observation. Read back and compare exact bytes when the application
needs post-write verification.

Filesystem and OpenDAL policies are related but not equivalent:

| Filesystem policy | Closest OpenDAL policy | Important difference |
| --- | --- | --- |
| `WriteNewTree` | None | OpenDAL does not commit a whole new prefix atomically. |
| `MergeCreateOnly` | `CreateOrVerify` | Filesystem writing rejects every existing target; OpenDAL accepts byte-identical targets. |
| `MergeReplaceExactFiles` | `OverwriteExactKeys` | Filesystem replacement is atomic per file where promised; OpenDAL makes no atomicity claim. |

## Migrating to 0.5

Version 0.5 introduced the optional OpenDAL integration.

- Builds that do not enable `opendal` keep their existing feature graph and behavior.
- Enable `opendal` only for operation-level asynchronous reads and writes; there is no universal storage abstraction or end-to-end OpenDAL workflow object.
- Add a compatible direct `opendal = "0.58"` dependency and select backend/runtime features there. typst-pack uses a caret requirement because `Operator` is a public parameter type.
- Compose storage operations manually with synchronous core construction, validation, creation, planning, encoding, decoding, and compilation.
- Use direct `Operator` reads for external compilation fulfillments; no compilation-read facade is provided.
- Linux, Windows, and macOS are supported native targets. The first OpenDAL release makes no OpenDAL-on-wasm promise and provides no blocking API.
- Enabling `opendal` changes implementation-attested compilation and result identities, just as enabling `fs`, `parallel`, or `diagnostics` does. Include the enabled feature set in any identity-keyed cache namespace.
- Backend configuration, credentials, retries, and exact service guarantees remain application policy; typst-pack neither installs nor weakens them.

Version 0.6 renamed the original acquisition/publication surface to read/write. See the complete
[0.6 rename table](../README.md#migrating-to-06); no old names remain as
compatibility aliases.
