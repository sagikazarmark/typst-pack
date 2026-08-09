<a id="opendal-integration-specification"></a>
# OpenDAL Integration Specification

<a id="authority-and-status"></a>
## Authority and status

This document is the normative OpenDAL integration specification for
typst-pack 0.5.0. It carries the complete intended public API and behavioral
contract. Issue #190 is its normative source. Earlier planning and API
inventories, including issue #140, are historical and non-normative. If an
implementation ticket disagrees with this document, update #190 and this
document before changing the implementation; do not invent a public API in an
implementation ticket.

This document specifies an API that does not exist yet. It introduces no
production API by itself. Ratification and cross-ticket repointing belong to
#225.

All Rust declarations in this document describe public signatures. Fields
shown as `/* private */` are deliberately opaque and their representation is
not contractual. Every public enum variant and every public field is shown.

<a id="scope"></a>
## Scope

The crate ships a complete first-party OpenDAL integration behind a non-default
`opendal` feature. It provides operation-specific asynchronous storage I/O for:

- Project and font acquisition for Pack Assembly.
- Package Tree, cache archive, and official registry archive acquisition.
- Exact Pack Archive acquisition and publication.
- Pack-bound compilation input acquisition.
- Pack Extraction Plan publication.
- Exact package-cache archive publication.
- Compilation Output Artifact publication.

OpenDAL owns only caller-polled storage I/O. Existing synchronous typst-pack
operations remain authoritative for Project Snapshot assembly, Font Catalog and
Package Catalog construction, Package Archive Expansion, Pack Creation, Pack
Archive encode/decode, raw compilation-bundle conversion, Pack Extraction
planning, and compilation.

The integration does not provide a blocking facade, library-owned runtime,
backend construction, credential handling, retry policy, transport or TLS
selection, CLI integration, universal storage abstraction, transaction,
rollback, staging, stale-prefix deletion, automatic retry, resume token, or
snapshot/coexistence guarantee.

<a id="adapter-vocabulary"></a>
## Adapter vocabulary

The integration introduces adapter vocabulary: `Location`, `OperatorBinding`,
`OperatorBindings`, `OperatorResolver`, `PublicationPolicy`, the `...Source`
request entries, and operation-specific acquisition, progress, receipt, and
outcome families. These names describe how bytes are addressed and moved. They
are not domain terms and do not change `CONTEXT.md`.

`acquire_*` is the OpenDAL equivalent of filesystem `gather_*`. OpenDAL
`ListedEntries`, `SelectedContainers`, and `TotalBytes` correspond broadly to
filesystem `VisitedEntries`, `AcceptedContainers`, and `TotalAcceptedBytes`,
but each adapter retains its operation-specific vocabulary and semantics.

| OpenDAL term | Filesystem/core term | Relationship |
|---|---|---|
| `acquire_project` | `gather_filesystem_project` | Different source membership policy; same Project Snapshot handoff. |
| `acquire_fonts` | `gather_filesystem_font_catalog` | OpenDAL returns raw containers; the caller performs the core catalog handoff. |
| `acquire_package` | `gather_filesystem_package` | OpenDAL exposes tree/cache/registry precedence and raw archive ownership. |
| `ListedEntries` | `VisitedEntries` | Every yielded listing entry versus every visited filesystem entry. |
| `SelectedContainers` | `AcceptedContainers` | Selected suffix-matching objects versus accepted filesystem containers. |
| `TotalBytes` | `TotalAcceptedBytes` or role-specific total | Adapter-specific cumulative retained payload. |

<a id="task-operation-handoff-matrix"></a>
## Task, operation, and handoff matrix

| Task | Request | Async operation | Async result/evidence | Synchronous handoff |
|---|---|---|---|---|
| Acquire project | `ProjectAcquisitionRequest` | `acquire_project` | `ProjectAcquisition` | `ProjectSnapshotAssembly::assemble` |
| Acquire Pack Assembly fonts | `FontAcquisitionRequest` | `acquire_fonts` | `FontAcquisition` | `FontContainer::new`, `FontCatalogEntry::new`, `FontCatalog::push` |
| Acquire one package | `PackageAcquisitionRequest` | `acquire_package` | `PackageAcquisition` | `insert_acquired_package`, then resumed `create` |
| Acquire Pack Archive | `PackArchiveAcquisitionRequest` | `acquire_pack_archive` | `PackArchiveBytes` | `pack_archive::decode` |
| Acquire compilation inputs | `CompilationAcquisitionRequest` | `acquire_compilation_bundle` | `CompilationAcquisitionBundle` | `into_values`, then `compile` |
| Publish Pack Archive | `PackArchivePublicationRequest` | `publish_pack_archive` | operation-specific receipt/error | None; archive was already encoded |
| Publish extraction | `PackExtractionPublicationRequest` | `publish_pack_extraction_plan` | receipt/error plus caller progress | Plan was already built by `plan_pack_extraction` |
| Publish package cache | `PackageCacheArchivePublicationRequest` | `publish_package_cache_archive` | operation-specific receipt/error | Must follow expand/validate/insert |
| Publish artifacts | `CompilationArtifactPublicationRequest` | `publish_compilation_artifacts` | receipt/error plus caller progress | Result was already produced by `compile` |

<a id="module-map"></a>
## Module map

```rust
#[cfg(feature = "opendal")]
pub mod opendal {
    pub mod location;
    pub mod pack_assembly;
    pub mod pack_archive;
    pub mod compilation;
    pub mod publication;

    pub use ::opendal::Operator;
    pub use location::{
        Location, LocationError, LocationRoleError, OperatorBinding,
        OperatorBindingError, OperatorBindings, OperatorBindingsError,
        OperatorBindingsResolveError, OperatorResolver,
    };
}
```

The crate root does not re-export OpenDAL workflow APIs. The integration does
not define a universal source, storage, sink, authority, gatherer, conformance,
error, receipt, progress, limits, or plan abstraction.

Public requests, locations, source entries, limits, and ceilings have safe
`Debug`. Small immutable values implement `Clone`, `Eq`, and `PartialEq` where
their fields permit it. Limits and ceilings implement `Copy`. Adapter-defined
raw values owning `Vec<u8>` are not `Clone`; their `Debug` implementations show
structure and lengths but never bytes. Public error, issue, cause, resource,
phase, entry-kind, policy, target, and outcome enums are `#[non_exhaustive]`
unless this document explicitly says otherwise.

<a id="features-and-dependencies"></a>
## Features and dependencies

The manifest contract is:

```toml
[features]
opendal = ["dep:opendal", "dep:futures-util"]

[dependencies]
opendal = { version = "0.58", default-features = false, optional = true }
futures-util = { version = "0.3.31", default-features = false, features = ["alloc", "async-await"], optional = true }
```

`"0.58"` is a caret requirement. An exact OpenDAL patch pin is rejected because
the public API accepts a caller-supplied `opendal::Operator`: exact pinning can
make a downstream graph unsatisfiable or produce a duplicate-crate `Operator`
type mismatch. A packaged downstream-resolution audit must build a consumer
against the packaged crate and a compatible independently declared OpenDAL
0.58.x dependency.

There is no direct `opendal-core` dependency. Production normalization is
vendored crate-privately. A test-only differential test compares it with
`opendal::raw::normalize_path`; production code does not import
`opendal::raw`. Stable `opendal::Capability` and `opendal::ErrorKind` are the
remaining OpenDAL compatibility inputs.

The `opendal` feature does not imply `package-acquisition`. OpenDAL-only builds
can acquire raw package archives. Expansion additionally requires the existing
`package-acquisition` feature. Do not assert `futures-util/std` absent: Cargo
feature unification enables it through `opendal-core`, while typst-pack's direct
declaration still records only what typst-pack requires.

<a id="feature-absence-list"></a>
### Exact feature-absence list

Dependency gates inspect normal and build edges and assert that none of these
OpenDAL features is enabled:

```text
auto-register-services
blocking
executors-tokio
internal-tokio-rt
tests

http-transport-reqwest
http-transport-reqwest-native-tls
http-transport-reqwest-rustls
http-transport-reqwest-rustls-no-provider
reqwest-rustls-no-provider-tls
reqwest-rustls-tls

layers-async-backtrace
layers-await-tree
layers-capability-check
layers-chaos
layers-concurrent-limit
layers-dtrace
layers-fastmetrics
layers-fastrace
layers-foyer
layers-hotpath
layers-immutable-index
layers-logging
layers-metrics
layers-mime-guess
layers-otel-metrics
layers-otel-trace
layers-prometheus
layers-prometheus-client
layers-retry
layers-route
layers-tail-cut
layers-throttle
layers-timeout
layers-tracing

services-aliyun-drive
services-alluxio
services-azblob
services-azdls
services-azfile
services-b2
services-cacache
services-cloudflare-kv
services-compfs
services-cos
services-d1
services-dashmap
services-dbfs
services-dropbox
services-etcd
services-foundationdb
services-foyer
services-fs
services-ftp
services-gcs
services-gdrive
services-ghac
services-github
services-goosefs
services-gridfs
services-hdfs
services-hdfs-native
services-hf
services-http
services-huggingface
services-ipfs
services-ipmfs
services-koofr
services-lakefs
services-memcached
services-memory
services-mini-moka
services-moka
services-mongodb
services-monoiofs
services-mysql
services-obs
services-onedrive
services-opfs
services-oss
services-pcloud
services-persy
services-postgresql
services-redb
services-redis
services-redis-native-tls
services-rocksdb
services-s3
services-seafile
services-sftp
services-sled
services-sqlite
services-surrealdb
services-swift
services-tikv
services-tos
services-upyun
services-vercel-artifacts
services-vercel-blob
services-webdav
services-webhdfs
services-yandex-disk
```

The resolved `opendal-core` graph must not enable `blocking`,
`executors-tokio`, `internal-tokio-rt`, `reqsign`, or `services-memory`.
`services-memory` is an empty no-op and `opendal::services::Memory` compiles
unconditionally, so Memory tests require no service feature.

`tokio` itself is a mandatory non-optional `opendal-core` dependency with
`macros` and `io-util`. The graph must not enable `tokio/rt` or
`tokio/rt-multi-thread`. Mandatory Tokio does not install or drive an executor;
the streaming read path does not require one. A gate claiming "no Tokio" would
be false.

Behavioral tests necessarily run under a superset graph because Cargo unifies
dev and test features. OpenDAL-only gates prove compile-time API and normal/build
dependency isolation, not behavioral feature isolation.

<a id="targets-and-compatibility"></a>
## Targets and compatibility

The OpenDAL feature supports Rust 1.92 on native Linux, Windows, and macOS. Its
behavioral-contract targets are:

```text
x86_64-unknown-linux-gnu
x86_64-pc-windows-msvc
aarch64-apple-darwin
```

There is no 32-bit promise. Featureless `wasm32-unknown-unknown` compilation
remains supported, but the first release makes no OpenDAL-on-Wasm promise.
OpenDAL 0.58.1, opendal-core 0.58.1, and opendal-service-s3 0.58.1 declare Rust
1.91 and edition 2024, compatible with workspace MSRV 1.92 and resolver 3.

Moving to OpenDAL 0.59 requires explicit dependency/API review, the full
conformance matrix, MSRV review, documentation updates, and a new typst-pack
minor release. Ordinary 0.58.x patch movement is admitted by the caret
requirement.

`deny.toml` needs no change. The existing license allow list covers OpenDAL's
Apache-2.0 and its transitive licenses; `multiple-versions` remains `warn`.

<a id="async-ownership-and-cancellation"></a>
## Async, ownership, and cancellation

All storage I/O is async and caller-polled. Callers construct Operators and own
backend, credential, transport, TLS, layer, executor, runtime, and retry policy.
Typst-pack owns bounded in-flight acquisition fan-out because retained-memory
bounds are part of its safety contract.

`OperatorResolver` and workflow traits are not required to be `Send` or `Sync`,
and object safety is not promised. Every public async operation has a compile-only
assertion proving that use with `OperatorBindings` returns a `Send` future on
supported native targets when borrowed semantic inputs are `Sync`. A custom
resolver's future behavior follows that resolver's own `Sync` properties.

Publication borrows `PackArchiveBytes`, `PackExtractionPlan`,
`CompilationResult`, and cache bytes. Callers retain replay material without an
extra copy. Adapter-defined acquisition results uniquely own raw vectors and
provide borrowed access plus consuming `into_parts` methods.

Dropping an acquisition or publication future returns no receipt. Already-issued
storage work may have occurred. A dropped multi-key publication future leaves
the caller-owned progress value containing the contiguous completed prefix.
Full-operation replay from retained semantic input or exact bytes is the only
library recovery contract. There is no rollback, retry, staging, resume token,
transaction, or sub-plan API.

<a id="locations-and-bindings"></a>
## Locations and Operator bindings

```rust
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperatorBinding { /* private */ }

impl OperatorBinding {
    pub fn new(value: impl AsRef<str>) -> Result<Self, OperatorBindingError>;
    pub fn as_str(&self) -> &str;
}

impl std::str::FromStr for OperatorBinding {
    type Err = OperatorBindingError;
}

impl std::fmt::Display for OperatorBinding;
impl std::fmt::Debug for OperatorBinding;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingError {
    Empty,
    InvalidInitialCharacter { index: usize, character: char },
    InvalidCharacter { index: usize, character: char },
    NonLowercaseCharacter { index: usize, character: char },
}
```

```rust
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Location { /* private */ }

impl Location {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, LocationError>;
    pub fn from_operation_path(
        binding: OperatorBinding,
        operation_path: impl AsRef<str>,
    ) -> Result<Self, LocationError>;
    pub fn binding(&self) -> &OperatorBinding;
    pub fn operation_path(&self) -> &str;
    pub fn is_root(&self) -> bool;
    pub fn has_trailing_slash(&self) -> bool;
}

impl std::str::FromStr for Location {
    type Err = LocationError;
}

impl std::fmt::Display for Location;
impl std::fmt::Debug for Location;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum LocationError {
    MissingBindingSeparator,
    InvalidBinding { source: OperatorBindingError },
    MissingAbsolutePath { index: usize },
    AuthorityNotAllowed { index: usize },
    UserInfoNotAllowed { index: usize },
    QueryNotAllowed { index: usize },
    FragmentNotAllowed { index: usize },
    RawNonAscii { index: usize },
    ControlCharacter { index: usize },
    Backslash { index: usize },
    MalformedPercentEscape { index: usize },
    NoncanonicalPercentEscape { index: usize },
    EncodedPchar { index: usize },
    EncodedSeparator { index: usize },
    EncodedBackslash { index: usize },
    InvalidUtf8 { index: usize },
    RepeatedSeparator { index: usize },
    DotSegment { index: usize },
    NormalizationAlias { index: usize },
    NoncanonicalPathCharacter { index: usize, character: char },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum LocationRoleError {
    ObjectAtRoot,
    ObjectHasTrailingSlash,
    PrefixMissingTrailingSlash,
}
```

The canonical form is `binding:/decoded/root-relative/path`. Binding grammar is
`[a-z][a-z0-9+.-]*`; uppercase is rejected, not normalized. Root is
`binding:/`, stores `operation_path() == ""`, reports a trailing slash, and is
dispatched to OpenDAL as `/`. Every non-root path is a fixed point under the
vendored normalization predicate. Non-root trailing separators are preserved.

RFC 3986 `pchar` characters are literal. Other decoded UTF-8 scalars are encoded
bytewise using uppercase `%HH`. Parsing rejects encoded pchar, `/`, or `\`,
lowercase hex, malformed escapes, raw non-ASCII, raw controls, raw backslashes,
raw characters that must be encoded, invalid UTF-8, authority, userinfo, query,
fragment, repeated separators, and dot segments. Error indexes are zero-based
input byte offsets. `char::is_whitespace` defines normalization trimming;
U+FEFF and U+200B are not whitespace aliases. Normalization does not resolve dot
segments, so both checks are independent.

Exact-object roles require non-root locations without a trailing separator.
Prefix roles permit root or require a trailing separator. Prefix confinement is
segment-aware and byte-exact after root dispatch projection.

```rust
pub trait OperatorResolver {
    type Error: std::error::Error + 'static;
    fn resolve(
        &self,
        binding: &OperatorBinding,
    ) -> Result<::opendal::Operator, Self::Error>;
}

#[derive(Clone, Debug)]
pub struct OperatorBindings { /* private immutable lexical map */ }

impl OperatorBindings {
    pub fn new(
        entries: impl IntoIterator<Item = (OperatorBinding, ::opendal::Operator)>,
    ) -> Result<Self, OperatorBindingsError>;
    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &OperatorBinding>;
    pub fn operator(&self, binding: &OperatorBinding) -> Option<::opendal::Operator>;
}

impl OperatorResolver for OperatorBindings {
    type Error = OperatorBindingsResolveError;
    fn resolve(
        &self,
        binding: &OperatorBinding,
    ) -> Result<::opendal::Operator, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingsError {
    DuplicateBinding { binding: OperatorBinding },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingsResolveError {
    UnknownBinding { binding: OperatorBinding },
}
```

`OperatorBindings` owns Operators, is immutable, returns cheap clones, and
permits distinct aliases for clones of one Operator. Ordinary workflows resolve
each distinct binding once in first canonical-target occurrence order and reuse
it. Package fallback resolves candidates lazily in source-precedence order.

<a id="limits-and-accounting"></a>
## Limits and resource accounting

Limits are finite, private, validated, and have no `Default`, unlimited, or
optional state. Zero is valid. Every primitive family has an exhaustive public
ceilings struct, `reference_v1()`, a limits constructor accepting that struct,
and same-named accessors. Struct-update syntax is the required narrowing idiom:

```rust
let limits = ProjectAcquisitionLimits::new(ProjectAcquisitionCeilings {
    total_bytes: 256 * 1024 * 1024,
    ..ProjectAcquisitionCeilings::reference_v1()
})?;
```

Adding a ceiling is a semver-major change because silently defaulting a new
security bound would weaken existing callers' explicit bounds.

Payload byte ceilings that require a plus-one probe reject `u64::MAX` with
`CannotProbe`. Count ceilings and both listed-path ceilings accept `u64::MAX`.
Constructors reject `object_bytes > total_bytes` and
`container_bytes > total_bytes`. Accounting uses checked `u64`. Metadata can
reject early but actual yielded paths and bytes are authoritative.

<a id="limit-declarations"></a>
### Limit declarations

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_files: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_containers: u64,
    pub container_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTreeAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub selected_files: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageArchiveAcquisitionCeilings {
    pub archive_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackOverrideAcquisitionCeilings {
    pub objects: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationPackageAcquisitionCeilings {
    pub listed_entries: u64,
    pub listed_path_bytes: u64,
    pub total_listed_path_bytes: u64,
    pub file_objects: u64,
    pub object_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationFontAcquisitionCeilings {
    pub containers: u64,
    pub container_bytes: u64,
    pub total_bytes: u64,
}
```

Each ceilings type has `pub const fn reference_v1() -> Self`. Corresponding
opaque limits types are:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectAcquisitionLimits { /* private */ }
impl ProjectAcquisitionLimits {
    pub fn new(ceilings: ProjectAcquisitionCeilings) -> Result<Self, ProjectAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn listed_entries(&self) -> u64;
    pub const fn listed_path_bytes(&self) -> u64;
    pub const fn total_listed_path_bytes(&self) -> u64;
    pub const fn selected_files(&self) -> u64;
    pub const fn object_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontAcquisitionLimits { /* private */ }
impl FontAcquisitionLimits {
    pub fn new(ceilings: FontAcquisitionCeilings) -> Result<Self, FontAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn listed_entries(&self) -> u64;
    pub const fn listed_path_bytes(&self) -> u64;
    pub const fn total_listed_path_bytes(&self) -> u64;
    pub const fn selected_containers(&self) -> u64;
    pub const fn container_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTreeAcquisitionLimits { /* private */ }
impl PackageTreeAcquisitionLimits {
    pub fn new(ceilings: PackageTreeAcquisitionCeilings) -> Result<Self, PackageTreeAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn listed_entries(&self) -> u64;
    pub const fn listed_path_bytes(&self) -> u64;
    pub const fn total_listed_path_bytes(&self) -> u64;
    pub const fn selected_files(&self) -> u64;
    pub const fn object_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageArchiveAcquisitionLimits { /* private */ }
impl PackageArchiveAcquisitionLimits {
    pub fn new(ceilings: PackageArchiveAcquisitionCeilings) -> Result<Self, PackageArchiveAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn archive_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackOverrideAcquisitionLimits { /* private */ }
impl PackOverrideAcquisitionLimits {
    pub fn new(ceilings: PackOverrideAcquisitionCeilings) -> Result<Self, PackOverrideAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn objects(&self) -> u64;
    pub const fn object_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationPackageAcquisitionLimits { /* private */ }
impl CompilationPackageAcquisitionLimits {
    pub fn new(ceilings: CompilationPackageAcquisitionCeilings) -> Result<Self, CompilationPackageAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn listed_entries(&self) -> u64;
    pub const fn listed_path_bytes(&self) -> u64;
    pub const fn total_listed_path_bytes(&self) -> u64;
    pub const fn file_objects(&self) -> u64;
    pub const fn object_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationFontAcquisitionLimits { /* private */ }
impl CompilationFontAcquisitionLimits {
    pub fn new(ceilings: CompilationFontAcquisitionCeilings) -> Result<Self, CompilationFontAcquisitionLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn containers(&self) -> u64;
    pub const fn container_bytes(&self) -> u64;
    pub const fn total_bytes(&self) -> u64;
}
```

Composite families also use named ceilings structs; no public limits constructor
takes multiple adjacent ceiling arguments:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageAcquisitionCeilings {
    pub trees: PackageTreeAcquisitionCeilings,
    pub archives: PackageArchiveAcquisitionCeilings,
}
impl PackageAcquisitionCeilings {
    pub const fn reference_v1() -> Self;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageAcquisitionLimits { /* private */ }
impl PackageAcquisitionLimits {
    pub fn new(ceilings: PackageAcquisitionCeilings) -> Result<Self, PackageAcquisitionLimitsError>;
    pub const fn trees(&self) -> PackageTreeAcquisitionLimits;
    pub const fn archives(&self) -> PackageArchiveAcquisitionLimits;
    pub const fn reference_v1() -> Self;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationAcquisitionCeilings {
    pub pack_overrides: PackOverrideAcquisitionCeilings,
    pub packages: CompilationPackageAcquisitionCeilings,
    pub fonts: CompilationFontAcquisitionCeilings,
    pub max_in_flight: std::num::NonZeroUsize,
}
impl CompilationAcquisitionCeilings {
    pub const fn reference_v1() -> Self;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilationAcquisitionLimits { /* private */ }
impl CompilationAcquisitionLimits {
    pub fn new(ceilings: CompilationAcquisitionCeilings) -> Result<Self, CompilationAcquisitionLimitsError>;
    pub const fn pack_overrides(&self) -> PackOverrideAcquisitionLimits;
    pub const fn packages(&self) -> CompilationPackageAcquisitionLimits;
    pub const fn fonts(&self) -> CompilationFontAcquisitionLimits;
    pub const fn max_in_flight(&self) -> std::num::NonZeroUsize;
    pub const fn reference_v1() -> Self;
}
```

Resource enums contain exactly these variants:

```rust
#[non_exhaustive] pub enum ProjectAcquisitionResource { ListedEntries, ListedPathBytes, TotalListedPathBytes, SelectedFiles, ObjectBytes, TotalBytes }
#[non_exhaustive] pub enum FontAcquisitionResource { ListedEntries, ListedPathBytes, TotalListedPathBytes, SelectedContainers, ContainerBytes, TotalBytes }
#[non_exhaustive] pub enum PackageTreeAcquisitionResource { ListedEntries, ListedPathBytes, TotalListedPathBytes, SelectedFiles, ObjectBytes, TotalBytes }
#[non_exhaustive] pub enum PackageArchiveAcquisitionResource { ArchiveBytes }
#[non_exhaustive] pub enum PackOverrideAcquisitionResource { Objects, ObjectBytes, TotalBytes }
#[non_exhaustive] pub enum CompilationPackageAcquisitionResource { ListedEntries, ListedPathBytes, TotalListedPathBytes, FileObjects, ObjectBytes, TotalBytes }
#[non_exhaustive] pub enum CompilationFontAcquisitionResource { Containers, ContainerBytes, TotalBytes }
```

Each `*LimitsError` has `CannotProbe { resource, ceiling }`; object families
also have `ObjectBytesExceedTotalBytes { object_bytes, total_bytes }`, and font
families have `ContainerBytesExceedTotalBytes { container_bytes, total_bytes }`.
Each runtime `*LimitError` has exactly:

```rust
Exceeded { resource: ResourceType, ceiling: u64, observed_at_least: u64 }
AccountingOverflow { resource: ResourceType }
```

The concrete names are `ProjectAcquisitionLimitsError`,
`FontAcquisitionLimitsError`, `PackageTreeAcquisitionLimitsError`,
`PackageArchiveAcquisitionLimitsError`, `PackOverrideAcquisitionLimitsError`,
`CompilationPackageAcquisitionLimitsError`,
`CompilationFontAcquisitionLimitsError`, and the same names with singular
`LimitError` for runtime failures.

The exact construction declarations are:

```rust
#[non_exhaustive]
pub enum ProjectAcquisitionLimitsError {
    CannotProbe { resource: ProjectAcquisitionResource, ceiling: u64 },
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}
#[non_exhaustive]
pub enum FontAcquisitionLimitsError {
    CannotProbe { resource: FontAcquisitionResource, ceiling: u64 },
    ContainerBytesExceedTotalBytes { container_bytes: u64, total_bytes: u64 },
}
#[non_exhaustive]
pub enum PackageTreeAcquisitionLimitsError {
    CannotProbe { resource: PackageTreeAcquisitionResource, ceiling: u64 },
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}
#[non_exhaustive]
pub enum PackageArchiveAcquisitionLimitsError {
    CannotProbe { resource: PackageArchiveAcquisitionResource, ceiling: u64 },
}
#[non_exhaustive]
pub enum PackOverrideAcquisitionLimitsError {
    CannotProbe { resource: PackOverrideAcquisitionResource, ceiling: u64 },
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}
#[non_exhaustive]
pub enum CompilationPackageAcquisitionLimitsError {
    CannotProbe { resource: CompilationPackageAcquisitionResource, ceiling: u64 },
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}
#[non_exhaustive]
pub enum CompilationFontAcquisitionLimitsError {
    CannotProbe { resource: CompilationFontAcquisitionResource, ceiling: u64 },
    ContainerBytesExceedTotalBytes { container_bytes: u64, total_bytes: u64 },
}

#[non_exhaustive]
pub enum PackageAcquisitionLimitsError {
    Trees(PackageTreeAcquisitionLimitsError),
    Archives(PackageArchiveAcquisitionLimitsError),
}

#[non_exhaustive]
pub enum CompilationAcquisitionLimitsError {
    PackOverrides(PackOverrideAcquisitionLimitsError),
    Packages(CompilationPackageAcquisitionLimitsError),
    Fonts(CompilationFontAcquisitionLimitsError),
}
```

The exact runtime declarations are:

```rust
#[non_exhaustive]
pub enum ProjectAcquisitionLimitError {
    Exceeded { resource: ProjectAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: ProjectAcquisitionResource },
}
#[non_exhaustive]
pub enum FontAcquisitionLimitError {
    Exceeded { resource: FontAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: FontAcquisitionResource },
}
#[non_exhaustive]
pub enum PackageTreeAcquisitionLimitError {
    Exceeded { resource: PackageTreeAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: PackageTreeAcquisitionResource },
}
#[non_exhaustive]
pub enum PackageArchiveAcquisitionLimitError {
    Exceeded { resource: PackageArchiveAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: PackageArchiveAcquisitionResource },
}
#[non_exhaustive]
pub enum PackOverrideAcquisitionLimitError {
    Exceeded { resource: PackOverrideAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: PackOverrideAcquisitionResource },
}
#[non_exhaustive]
pub enum CompilationPackageAcquisitionLimitError {
    Exceeded { resource: CompilationPackageAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: CompilationPackageAcquisitionResource },
}
#[non_exhaustive]
pub enum CompilationFontAcquisitionLimitError {
    Exceeded { resource: CompilationFontAcquisitionResource, ceiling: u64, observed_at_least: u64 },
    AccountingOverflow { resource: CompilationFontAcquisitionResource },
}
```

<a id="reference-profiles"></a>
### Reference profiles

| Family | Listed entries | One path | Retained paths | Selected/file objects | One payload | Total payload |
|---|---:|---:|---:|---:|---:|---:|
| Project | 1,000,000 | 64 KiB | 256 MiB | 100,000 | 256 MiB | 2 GiB |
| Fonts | 100,000 | 64 KiB | 64 MiB across all sources | 16,384 | 256 MiB | 2 GiB |
| Pack Assembly Package Tree | 100,000 | 64 KiB | 64 MiB across attempted tree sources/spec | 50,000 | 64 MiB | 512 MiB |
| Compilation packages | 1,000,000 | 64 KiB | 256 MiB across all package targets | 500,000 | 64 MiB | 2 GiB |

Raw cache and registry archive reads are each 128 MiB. Compilation Pack
Overrides are 100,000 objects, 256 MiB each, 2 GiB total. Compilation fonts are
16,384 containers, 256 MiB each, 2 GiB total. Compilation concurrency is 16.
Pack Override exact paths need no separate listed-path limit because Pack-bound
request values already bound them.

The approximate maximum retained typst-pack payload/path exposure is 2.25 GiB
for projects, 2 GiB plus 64 MiB for fonts, 576 MiB for one Package Tree attempt,
and 6.25 GiB plus at most 16 probe bytes for compilation. These limits do not
bound allocations inside an OpenDAL service before it yields data or the size of
one chunk a service yields.

<a id="limit-precedence"></a>
### Limit precedence

Competing violations use this fixed public precedence:

```text
ListedEntries
ListedPathBytes
TotalListedPathBytes
SelectedFiles / SelectedContainers / FileObjects / Objects / Containers
ObjectBytes / ContainerBytes / ArchiveBytes
TotalBytes
```

This is survey before payload, count before bytes, and per-item before aggregate.
Backend yield order and detection order cannot change the reported resource.
When retaining exact overage evidence would exceed a bound,
`observed_at_least` is deterministically `ceiling + 1`.

<a id="listing-observation"></a>
## Listing observation contract

A complete survey means every entry yielded by one listing observation that
itself completed successfully was counted and structurally considered. Every
yielded entry is counted before filtering. Structural surveys finish before
ordinary payload reads.

This is not a snapshot guarantee. It does not assert that every object existing
at any instant was yielded, that yielded entries coexisted, that an object did
not change between listing and reading, or that all acquired values coexisted at
one instant. A listing error after yielding entries fails the operation rather
than returning a partial survey.

`listed_path_bytes` is one yielded UTF-8 operation path's byte length.
`total_listed_path_bytes` charges each retained path, structural-issue path,
sort key, or path copy once per retained allocation and is shared across all
sources or targets governed by one limits value. Once count/path retention would
exceed its limit, return the operation-specific limit error rather than retaining
unbounded further evidence.

Recursive surveys require effective `list` and `list_with_recursive`; payloads
require `read`. Bounded reads stream incrementally from offset zero and retain at
most the effective ceiling plus one byte. They must not issue a finite
`0..ceiling + 1` range: OpenDAL Memory returns `RangeNotSatisfied` when that end
exceeds a shorter object, including a `0..1` read of an empty object.

<a id="project-acquisition"></a>
## Project acquisition

All names are under `typst_pack::opendal::pack_assembly`.

```rust
#[derive(Clone, Debug)]
pub struct ProjectAcquisitionRequest { /* private */ }
impl ProjectAcquisitionRequest {
    pub fn new(source: Location, limits: ProjectAcquisitionLimits) -> Result<Self, ProjectAcquisitionRequestError>;
    pub fn source(&self) -> &Location;
    pub const fn limits(&self) -> ProjectAcquisitionLimits;
}
#[non_exhaustive]
pub enum ProjectAcquisitionRequestError {
    InvalidSourceRole { location: Location, source: LocationRoleError },
}

pub async fn acquire_project<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &ProjectAcquisitionRequest,
) -> Result<ProjectAcquisition, ProjectAcquisitionError<R::Error>>;

pub struct ProjectAcquisitionEntry { /* private String, Vec<u8> */ }
impl ProjectAcquisitionEntry {
    pub fn relative_path(&self) -> &str;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (String, Vec<u8>);
}

pub struct ProjectAcquisition { /* private */ }
impl ProjectAcquisition {
    pub fn source(&self) -> &Location;
    pub fn entries(&self) -> &[ProjectAcquisitionEntry];
    pub fn into_parts(self) -> (Location, Vec<ProjectAcquisitionEntry>);
}

#[non_exhaustive]
pub enum ProjectAcquisitionEntryKind { Unknown }

#[non_exhaustive]
pub enum ProjectAcquisitionIssue {
    ListedPathOutsidePrefix { operation_path: String },
    PrefixMarkerWhereFileRequired { operation_path: String },
    EmptyRelativeOperationPath { operation_path: String },
    InvalidRelativeOperationPath { operation_path: String },
    DuplicateListedObject { operation_path: String },
    UnsupportedEntryKind { operation_path: String, kind: ProjectAcquisitionEntryKind },
}

pub struct ProjectAcquisitionSurveyError { /* nonempty private issues */ }
impl ProjectAcquisitionSurveyError {
    pub fn issues(&self) -> &[ProjectAcquisitionIssue];
}

pub struct ProjectAcquisitionError<E> { /* private */ }
impl<E> ProjectAcquisitionError<E> {
    pub fn source_location(&self) -> &Location;
    pub fn failed_path(&self) -> Option<&str>;
    pub fn cause(&self) -> &ProjectAcquisitionErrorCause<E>;
}

#[non_exhaustive]
pub enum ProjectAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedCapabilities { list: bool, list_with_recursive: bool, read: bool },
    List(::opendal::Error),
    Read(::opendal::Error),
    ListedObjectAbsent(::opendal::Error),
    Structural(ProjectAcquisitionSurveyError),
    Limit(ProjectAcquisitionLimitError),
}
```

The operation acquires every file object under one prefix, including
`.typkignore` as an ordinary file. It ignores directory markers, rejects unknown
or unsupported kinds, and returns exact entries sorted by relative operation
path. `ProjectSnapshotAssembly` remains authoritative for canonical paths,
duplicates, built-in `.typk` exclusion, entrypoint presence, bytes, and ordering.

<a id="font-acquisition"></a>
## Font acquisition

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontSource { /* private */ }
impl FontSource {
    pub fn new(source: Location, disposition: FontDisposition) -> Self;
    pub fn source(&self) -> &Location;
    pub const fn disposition(&self) -> FontDisposition;
}

#[derive(Clone, Debug)]
pub struct FontAcquisitionRequest { /* private */ }
impl FontAcquisitionRequest {
    pub fn new(
        sources: impl IntoIterator<Item = FontSource>,
        limits: FontAcquisitionLimits,
    ) -> Result<Self, FontAcquisitionRequestRejection>;
    pub fn sources(&self) -> &[FontSource];
    pub const fn limits(&self) -> FontAcquisitionLimits;
}

pub struct FontAcquisitionRequestRejection { /* nonempty private issues */ }
impl FontAcquisitionRequestRejection {
    pub fn issues(&self) -> &[FontAcquisitionRequestIssue];
}
#[non_exhaustive]
pub enum FontAcquisitionRequestIssue {
    InvalidSourceRole {
        source_index: usize,
        location: Location,
        source: LocationRoleError,
    },
}

pub async fn acquire_fonts<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &FontAcquisitionRequest,
) -> Result<FontAcquisition, FontAcquisitionError<R::Error>>;

pub struct FontAcquisitionEntry { /* private */ }
impl FontAcquisitionEntry {
    pub fn source_index(&self) -> usize;
    pub fn source(&self) -> &Location;
    pub fn relative_path(&self) -> &str;
    pub const fn disposition(&self) -> FontDisposition;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (usize, Location, String, FontDisposition, Vec<u8>);
}

pub struct FontAcquisition { /* private */ }
impl FontAcquisition {
    pub fn sources(&self) -> &[FontSource];
    pub fn entries(&self) -> &[FontAcquisitionEntry];
    pub fn into_parts(self) -> (Vec<FontSource>, Vec<FontAcquisitionEntry>);
}

#[non_exhaustive]
pub enum FontAcquisitionEntryKind { Unknown }

#[non_exhaustive]
pub enum FontAcquisitionIssue {
    ListedPathOutsidePrefix { source_index: usize, operation_path: String },
    PrefixMarkerWhereFileRequired { source_index: usize, operation_path: String },
    EmptyRelativeOperationPath { source_index: usize, operation_path: String },
    InvalidRelativeOperationPath { source_index: usize, operation_path: String },
    DuplicateListedObject { source_index: usize, operation_path: String },
    UnsupportedEntryKind { source_index: usize, operation_path: String, kind: FontAcquisitionEntryKind },
}

pub struct FontAcquisitionSurveyError { /* nonempty private issues */ }
impl FontAcquisitionSurveyError {
    pub fn issues(&self) -> &[FontAcquisitionIssue];
}

pub struct FontAcquisitionError<E> { /* private */ }
impl<E> FontAcquisitionError<E> {
    pub fn source_index(&self) -> usize;
    pub fn source_location(&self) -> &Location;
    pub fn failed_path(&self) -> Option<&str>;
    pub fn cause(&self) -> &FontAcquisitionErrorCause<E>;
}

#[non_exhaustive]
pub enum FontAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedCapabilities { list: bool, list_with_recursive: bool, read: bool },
    List(::opendal::Error),
    Read(::opendal::Error),
    ListedObjectAbsent(::opendal::Error),
    Structural(FontAcquisitionSurveyError),
    Limit(FontAcquisitionLimitError),
}
```

Sources retain caller order. Within each source, selected `.ttf`, `.ttc`,
`.otf`, and `.otc` files sort by relative path using ASCII-case-insensitive
suffix matching. Non-font files and directory markers are ignored. Duplicate
containers and overlapping prefixes remain distinct. `FontContainer::new` and
`FontCatalogEntry::new` remain authoritative. The suffix set is one ungated
crate-private authority shared with filesystem gathering.

<a id="package-acquisition"></a>
## Package acquisition

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTreeSource { /* private */ }
impl PackageTreeSource {
    pub fn new(source: Location) -> Self;
    pub fn source(&self) -> &Location;
}

#[derive(Clone, Debug)]
pub struct PackageAcquisitionRequest { /* private */ }
impl PackageAcquisitionRequest {
    pub fn new(
        spec: typst::syntax::package::PackageSpec,
        tree_sources: impl IntoIterator<Item = PackageTreeSource>,
        archive_cache: Option<Location>,
        registry: Option<Location>,
        limits: PackageAcquisitionLimits,
    ) -> Result<Self, PackageAcquisitionRequestRejection>;
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn tree_sources(&self) -> &[PackageTreeSource];
    pub fn archive_cache(&self) -> Option<&Location>;
    pub fn registry(&self) -> Option<&Location>;
    pub const fn limits(&self) -> PackageAcquisitionLimits;
}

pub struct PackageAcquisitionRequestRejection { /* private */ }
impl PackageAcquisitionRequestRejection {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn issues(&self) -> &[PackageAcquisitionRequestIssue];
}

#[non_exhaustive]
pub enum PackageAcquisitionRequestIssue {
    InvalidTreeSourceRole {
        source_index: usize,
        location: Location,
        source: LocationRoleError,
    },
    InvalidArchiveCacheRole { location: Location, source: LocationRoleError },
    InvalidRegistryRole { location: Location, source: LocationRoleError },
}

pub async fn acquire_package<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageAcquisitionRequest,
) -> Result<PackageAcquisition, PackageAcquisitionError<R::Error>>;

#[non_exhaustive]
pub enum PackageAcquisition {
    Tree(PackageTreeAcquisition),
    CachedArchive(CachedPackageArchiveAcquisition),
    RegistryArchive(RegistryPackageArchiveAcquisition),
    Unavailable(UnavailablePackageAcquisition),
}
impl PackageAcquisition {
    pub fn configured_source(&self) -> Option<&Location>;
    pub fn candidate_location(&self) -> Option<&Location>;
}
```

Candidate precedence is caller-ordered Package Tree prefixes, optional raw
archive cache, then optional official registry prefix. Candidates are derived as
`{namespace}/{name}/{version}/`, `{namespace}/{name}/{version}.tar.gz`, and
`{namespace}/{name}-{version}.tar.gz`. Registry lookup is skipped for unserved
namespaces. Shared ungated crate-private layout authority serves both OpenDAL and
existing package acquisition while preserving public feature gates.

Resolution/appraisal is lazy. Fallback advances only on definite absence. The
first present candidate terminates fallback even if later core construction,
expansion, declaration, or insertion fails. An empty successfully surveyed tree
prefix is absence.

```rust
pub struct PackageTreeAcquisitionEntry { /* private String, Vec<u8> */ }
impl PackageTreeAcquisitionEntry {
    pub fn relative_path(&self) -> &str;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (String, Vec<u8>);
}

pub struct PackageTreeAcquisition { /* private */ }
impl PackageTreeAcquisition {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn source_index(&self) -> usize;
    pub fn configured_source(&self) -> &Location;
    pub fn candidate_location(&self) -> &Location;
    pub fn entries(&self) -> &[PackageTreeAcquisitionEntry];
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, usize, Location, Location, Vec<PackageTreeAcquisitionEntry>);
}

pub struct CachedPackageArchiveAcquisition { /* private */ }
impl CachedPackageArchiveAcquisition {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn configured_source(&self) -> &Location;
    pub fn candidate_location(&self) -> &Location;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, Location, Location, Vec<u8>);
}

pub struct RegistryPackageArchiveAcquisition { /* private */ }
impl RegistryPackageArchiveAcquisition {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn configured_source(&self) -> &Location;
    pub fn candidate_location(&self) -> &Location;
    pub fn cache_destination(&self) -> Option<&Location>;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, Location, Location, Option<Location>, Vec<u8>);
}

pub struct UnavailablePackageAcquisition { /* private */ }
impl UnavailablePackageAcquisition {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn failure(&self) -> &PackageAcquisitionFailure;
    pub fn reason(&self) -> &PackageAcquisitionFailureReason;
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, PackageAcquisitionFailure);
}
```

<a id="core-package-path-preflight"></a>
### Core Package Tree path preflight

Envelope survey issues are:

```rust
#[non_exhaustive]
pub enum PackageTreeAcquisitionEntryKind { Unknown }

#[non_exhaustive]
pub enum PackageTreeAcquisitionIssue {
    ListedPathOutsidePrefix { operation_path: String },
    PrefixMarkerWhereFileRequired { operation_path: String },
    EmptyRelativeOperationPath { operation_path: String },
    UnsupportedEntryKind { operation_path: String, kind: PackageTreeAcquisitionEntryKind },
}

pub struct PackageTreeAcquisitionSurveyError { /* nonempty issues */ }
impl PackageTreeAcquisitionSurveyError {
    pub fn issues(&self) -> &[PackageTreeAcquisitionIssue];
}
```

Canonical relative Package Tree paths, canonical duplicates, and
file/descendant conflicts are checked by one crate-private core authority reused
by OpenDAL surveys and final `PackageTree::from_owned_entries`. Its authoritative
`PackageTreeError` is preserved as a typed cause; adapters do not re-spell those
core issue variants.

```rust
pub struct PackageAcquisitionError<E> { /* private */ }
impl<E> PackageAcquisitionError<E> {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn source_index(&self) -> Option<usize>;
    pub fn configured_source(&self) -> Option<&Location>;
    pub fn candidate_location(&self) -> Option<&Location>;
    pub fn failed_path(&self) -> Option<&str>;
    pub fn failure(&self) -> &PackageAcquisitionFailure;
    pub fn reason(&self) -> &PackageAcquisitionFailureReason;
    pub fn cause(&self) -> &PackageAcquisitionErrorCause<E>;
}

#[non_exhaustive]
pub enum PackageAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedTreeCapabilities { list: bool, list_with_recursive: bool, read: bool },
    UnsupportedArchiveRead,
    TreeList(::opendal::Error),
    TreeRead(::opendal::Error),
    ListedTreeObjectAbsent(::opendal::Error),
    CacheRead(::opendal::Error),
    RegistryRead(::opendal::Error),
    TreeStructural(PackageTreeAcquisitionSurveyError),
    InvalidPackageTree(PackageTreeError),
    TreeLimit(PackageTreeAcquisitionLimitError),
    ArchiveLimit(PackageArchiveAcquisitionLimitError),
}
```

`Unavailable` owns `PackageAcquisitionFailure::new(spec, NotFound)`. Every
non-`NotFound` registry OpenDAL error maps to `Other { detail: None }`, preserving
the OpenDAL error as the typed adapter cause. `NetworkFailed` is not emitted in
0.5 because OpenDAL has no portable network error kind; `is_temporary()` is a
retry signal, not a network classifier. No latest-version lookup is made merely
to emit `VersionNotFound`.

<a id="package-insertion-and-cache-safety"></a>
## Package insertion and cache safety

```rust
pub fn insert_acquired_package(
    catalog: &mut PackageCatalog,
    failures: &mut PackageAcquisitionFailures,
    acquisition: PackageAcquisition,
    disposition: PackageDisposition,
    expansion_limits: PackageExpansionLimits,
) -> Result<Option<RegistryArchiveResidue>, AcquiredPackageInsertionError>;

pub struct RegistryArchiveResidue { /* private spec, destination, Vec<u8> */ }
impl RegistryArchiveResidue {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn destination(&self) -> &Location;
    pub fn bytes(&self) -> &[u8];
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, Location, Vec<u8>);
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AcquiredPackageInsertionTarget {
    PackageTree,
    CachedArchive,
    RegistryArchive,
    PackageCatalog,
}

pub struct AcquiredPackageInsertionError { /* private */ }
impl AcquiredPackageInsertionError {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn failure(&self) -> &PackageAcquisitionFailure;
    pub fn reason(&self) -> &PackageAcquisitionFailureReason;
    pub fn target(&self) -> &AcquiredPackageInsertionTarget;
    pub fn cause(&self) -> &AcquiredPackageInsertionErrorCause;
}

#[non_exhaustive]
pub enum AcquiredPackageInsertionErrorCause {
    PackageTree(PackageTreeError),
    ArchiveExpansion(crate::PackageAcquisitionError),
    PackageCatalog(PackageCatalogError),
}
```

Tree and catalog failures map to `Other`. Cache and registry expansion map
`MalformedArchive` and `InvalidPackageTree` to `MalformedArchive`, and expansion
limits/insertion failures to `Other`. `Unavailable` records its failure and
returns `Ok(None)`. Successful insertion removes an older failure. Successful
registry expansion/validation/insertion returns exact original registry bytes
and the derived cache destination as residue.

The cache publication API below is deliberately low-level: a byte slice and a
destination cannot prove prior semantic validation. The required security
sequence is acquire, expand, validate Package Tree, insert into Package Catalog,
then publish the exact original registry bytes. Calling the low-level function
directly with unvalidated bytes can poison a cache. Cache publication failure is
separate evidence, does not invalidate acquisition/insertion, and does not
become a Package Acquisition Failure.

<a id="pack-archive-acquisition"></a>
## Pack Archive acquisition

All names are under `typst_pack::opendal::pack_archive`.

```rust
#[derive(Clone, Debug)]
pub struct PackArchiveAcquisitionRequest { /* private */ }
impl PackArchiveAcquisitionRequest {
    pub fn new(
        source: Location,
        limits: typst_pack::pack_archive::AcquisitionLimits,
    ) -> Result<Self, PackArchiveAcquisitionRequestError>;
    pub fn source(&self) -> &Location;
    pub const fn limits(&self) -> typst_pack::pack_archive::AcquisitionLimits;
}
#[non_exhaustive]
pub enum PackArchiveAcquisitionRequestError {
    InvalidSourceRole { location: Location, source: LocationRoleError },
}

pub async fn acquire_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchiveAcquisitionRequest,
) -> Result<PackArchiveBytes, PackArchiveAcquisitionError<R::Error>>;

pub struct PackArchiveAcquisitionError<E> { /* private */ }
impl<E> PackArchiveAcquisitionError<E> {
    pub fn source_location(&self) -> &Location;
    pub fn cause(&self) -> &PackArchiveAcquisitionErrorCause<E>;
}
#[non_exhaustive]
pub enum PackArchiveAcquisitionErrorCause<E> {
    ResolveOperator(E),
    ReadUnsupported,
    ObjectAbsent(::opendal::Error),
    Read(::opendal::Error),
    Limit(typst_pack::pack_archive::AcquisitionLimitError),
}
```

Success is the existing uniquely owned, non-`Clone` `PackArchiveBytes`. The async
operation neither decodes nor validates the Pack.

<a id="compilation-acquisition"></a>
## Compilation acquisition

All names are under `typst_pack::opendal::compilation`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackOverrideSource { /* private */ }
impl PackOverrideSource {
    pub fn new(path: impl Into<String>, source: Location) -> Self;
    pub fn path(&self) -> &str;
    pub fn source(&self) -> &Location;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTreeSource { /* private */ }
impl PackageTreeSource {
    pub fn new(spec: typst::syntax::package::PackageSpec, source: Location) -> Self;
    pub fn with_provenance(self, provenance: impl Into<String>) -> Self;
    pub fn with_cache_hit(self, cache_hit: bool) -> Self;
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn source(&self) -> &Location;
    pub fn provenance(&self) -> Option<&str>;
    pub fn cache_hit(&self) -> bool;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontContainerSource { /* private */ }
impl FontContainerSource {
    pub fn new(expected_identity: FontContainerIdentity, source: Location) -> Self;
    pub fn with_provenance(self, provenance: impl Into<String>) -> Self;
    pub fn with_licensing(self, licensing: impl Into<String>) -> Self;
    pub fn expected_identity(&self) -> FontContainerIdentity;
    pub fn source(&self) -> &Location;
    pub fn provenance(&self) -> Option<&str>;
    pub fn licensing(&self) -> Option<&str>;
}

#[derive(Clone, Debug)]
pub struct CompilationAcquisitionRequest { /* private */ }
impl CompilationAcquisitionRequest {
    pub fn new(
        pack: &Pack,
        pack_overrides: impl IntoIterator<Item = PackOverrideSource>,
        packages: impl IntoIterator<Item = PackageTreeSource>,
        fonts: impl IntoIterator<Item = FontContainerSource>,
        limits: CompilationAcquisitionLimits,
    ) -> Result<Self, CompilationAcquisitionRequestRejection>;
    pub fn pack_identity(&self) -> PackIdentity;
    pub fn pack_overrides(&self) -> &[PackOverrideSource];
    pub fn packages(&self) -> &[PackageTreeSource];
    pub fn fonts(&self) -> &[FontContainerSource];
    pub const fn limits(&self) -> CompilationAcquisitionLimits;
}

pub struct CompilationAcquisitionRequestRejection { /* nonempty private issues */ }
impl CompilationAcquisitionRequestRejection {
    pub fn pack_identity(&self) -> PackIdentity;
    pub fn issues(&self) -> &[CompilationAcquisitionRequestIssue];
}
```

```rust
#[non_exhaustive]
pub enum CompilationAcquisitionRequestIssue {
    InvalidPackOverridePath { supplied: String, source: PackOverrideSetError },
    MissingPackOverrideTarget { path: String },
    DuplicatePackOverrideTarget { path: String },
    InvalidPackOverrideSourceRole { path: String, location: Location, source: LocationRoleError },
    MissingPackageSource { spec: typst::syntax::package::PackageSpec },
    DuplicatePackageSource { spec: typst::syntax::package::PackageSpec },
    EmbeddedPackageSource { spec: typst::syntax::package::PackageSpec },
    UndeclaredPackageSource { spec: typst::syntax::package::PackageSpec },
    InvalidPackageSourceRole { spec: typst::syntax::package::PackageSpec, location: Location, source: LocationRoleError },
    MissingFontSource { identity: FontContainerIdentity },
    DuplicateFontSource { identity: FontContainerIdentity },
    EmbeddedFontSource { identity: FontContainerIdentity },
    UndeclaredFontSource { identity: FontContainerIdentity },
    InvalidFontSourceRole { identity: FontContainerIdentity, location: Location, source: LocationRoleError },
    PackOverrideObjectLimitExceeded { ceiling: u64, observed_at_least: u64 },
    PackageListedEntryLimitExceeded { ceiling: u64, declared_at_least: u64 },
    PackageFileObjectLimitExceeded { ceiling: u64, declared: u64 },
    PackageTotalByteLimitExceeded { ceiling: u64, declared: u64 },
    FontContainerLimitExceeded { ceiling: u64, declared: u64 },
    FontContainerByteLimitExceeded { identity: FontContainerIdentity, ceiling: u64, declared: u64 },
    FontTotalByteLimitExceeded { ceiling: u64, declared: u64 },
}
```

Construction aggregates all detectable issues before resolution/I/O. Pack
Overrides are optional and can only replace canonical project paths already in
the Pack. Every external package/font requirement has exactly one source;
embedded and undeclared sources reject. Pack Overrides and Font Containers are
exact objects; Package Trees are recursive prefixes. No raw archives, cache or
registry fallback, ambient package source, system font, embedded font, or font
suffix filtering participates.

```rust
pub async fn acquire_compilation_bundle<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &CompilationAcquisitionRequest,
) -> Result<CompilationAcquisitionBundle, CompilationAcquisitionError<R::Error>>;

pub struct PackOverrideAcquisitionEntry { /* private */ }
impl PackOverrideAcquisitionEntry {
    pub fn path(&self) -> &str;
    pub fn source(&self) -> &Location;
    pub fn bytes(&self) -> &[u8];
    pub fn into_parts(self) -> (String, Location, Vec<u8>);
}

pub struct CompilationPackageAcquisitionEntry { /* private */ }
impl CompilationPackageAcquisitionEntry {
    pub fn relative_path(&self) -> &str;
    pub fn bytes(&self) -> &[u8];
    pub fn into_parts(self) -> (String, Vec<u8>);
}

pub struct CompilationPackageAcquisition { /* private */ }
impl CompilationPackageAcquisition {
    pub fn spec(&self) -> &typst::syntax::package::PackageSpec;
    pub fn source(&self) -> &Location;
    pub fn provenance(&self) -> Option<&str>;
    pub fn cache_hit(&self) -> bool;
    pub fn entries(&self) -> &[CompilationPackageAcquisitionEntry];
    pub fn into_parts(self) -> (typst::syntax::package::PackageSpec, Location, Option<String>, bool, Vec<CompilationPackageAcquisitionEntry>);
}

pub struct FontContainerAcquisitionEntry { /* private */ }
impl FontContainerAcquisitionEntry {
    pub fn expected_identity(&self) -> FontContainerIdentity;
    pub fn source(&self) -> &Location;
    pub fn provenance(&self) -> Option<&str>;
    pub fn licensing(&self) -> Option<&str>;
    pub fn bytes(&self) -> &[u8];
    pub fn into_parts(self) -> (FontContainerIdentity, Location, Option<String>, Option<String>, Vec<u8>);
}

pub struct CompilationAcquisitionBundle { /* private */ }
impl CompilationAcquisitionBundle {
    pub fn pack_identity(&self) -> PackIdentity;
    pub fn pack_overrides(&self) -> &[PackOverrideAcquisitionEntry];
    pub fn packages(&self) -> &[CompilationPackageAcquisition];
    pub fn fonts(&self) -> &[FontContainerAcquisitionEntry];
    pub fn into_parts(self) -> (PackIdentity, Vec<PackOverrideAcquisitionEntry>, Vec<CompilationPackageAcquisition>, Vec<FontContainerAcquisitionEntry>);
    pub fn into_values(self, pack: &Pack) -> Result<CompilationAcquisitionValues, CompilationAcquisitionConversionError>;
}

pub struct CompilationAcquisitionValues { /* private */ }
impl CompilationAcquisitionValues {
    pub fn pack_overrides(&self) -> &PackOverrideSet;
    pub fn fulfillments(&self) -> &CompilationFulfillmentSet;
    pub fn into_parts(self) -> (PackOverrideSet, CompilationFulfillmentSet);
}
```

```rust
#[non_exhaustive]
pub enum CompilationAcquisitionConversionTarget {
    Pack,
    PackOverride { path: String },
    PackageTree { spec: typst::syntax::package::PackageSpec },
    FontContainer { identity: FontContainerIdentity },
    FulfillmentSet,
}

pub struct CompilationAcquisitionConversionError { /* private */ }
impl CompilationAcquisitionConversionError {
    pub fn target(&self) -> &CompilationAcquisitionConversionTarget;
    pub fn cause(&self) -> &CompilationAcquisitionConversionErrorCause;
}

#[non_exhaustive]
pub enum CompilationAcquisitionConversionErrorCause {
    PackMismatch { expected: PackIdentity, actual: PackIdentity },
    PackOverride(PackOverrideSetError),
    PackageTree(PackageTreeError),
    FontContainer(FontContainerError),
    FulfillmentSet(CompilationFulfillmentSetError),
}

#[non_exhaustive]
pub enum CompilationAcquisitionTarget {
    PackOverride { path: String },
    PackageTree { spec: typst::syntax::package::PackageSpec },
    PackageFile { spec: typst::syntax::package::PackageSpec, relative_path: String },
    FontContainer { identity: FontContainerIdentity },
}

#[non_exhaustive]
pub enum CompilationPackageAcquisitionEntryKind { Unknown }

#[non_exhaustive]
pub enum CompilationPackageAcquisitionIssue {
    ListedPathOutsidePrefix { spec: typst::syntax::package::PackageSpec, operation_path: String },
    PrefixMarkerWhereFileRequired { spec: typst::syntax::package::PackageSpec, operation_path: String },
    EmptyRelativeOperationPath { spec: typst::syntax::package::PackageSpec, operation_path: String },
    UnsupportedEntryKind { spec: typst::syntax::package::PackageSpec, operation_path: String, kind: CompilationPackageAcquisitionEntryKind },
}

pub struct CompilationPackageAcquisitionSurveyError { /* nonempty issues */ }
impl CompilationPackageAcquisitionSurveyError {
    pub fn issues(&self) -> &[CompilationPackageAcquisitionIssue];
}

pub struct CompilationAcquisitionError<E> { /* private */ }
impl<E> CompilationAcquisitionError<E> {
    pub fn pack_identity(&self) -> PackIdentity;
    pub fn target(&self) -> &CompilationAcquisitionTarget;
    pub fn source_location(&self) -> &Location;
    pub fn cause(&self) -> &CompilationAcquisitionErrorCause<E>;
}

#[non_exhaustive]
pub enum CompilationAcquisitionErrorCause<E> {
    ResolveOperator(E),
    UnsupportedCapabilities { list: bool, list_with_recursive: bool, read: bool },
    List(::opendal::Error),
    Read(::opendal::Error),
    ObjectAbsent(::opendal::Error),
    PrefixAbsent,
    ListedObjectAbsent(::opendal::Error),
    PackageStructural(CompilationPackageAcquisitionSurveyError),
    InvalidPackageTree(PackageTreeError),
    PackOverrideLimit(PackOverrideAcquisitionLimitError),
    PackageLimit(CompilationPackageAcquisitionLimitError),
    FontLimit(CompilationFontAcquisitionLimitError),
}
```

Conversion verifies Pack Identity and uses authoritative `PackOverrideSet`,
`PackageTree`, `FontContainer`, fulfillment, and
`CompilationFulfillmentSet` constructors. It returns no partial semantic values.

<a id="compilation-reservations"></a>
### Compilation scheduling and reservations

Canonical scheduling order is Pack Override path, exact Package Specification,
then Font Container Identity. Each role tracks `reserved_in_flight`,
`retained_success`, and probe bytes. A read starts only after reserving its
effective per-object allowance from
`role_total - retained_success - reserved_in_flight`. Actual successful bytes
move to retained success; only unused reservation is refunded. Failed/dropped
work releases reservation and probe. Buffered bundle bytes remain charged
regardless of completion order.

A target unable to reserve blocks until in-flight reservations resolve; it does
not fail based on timing-dependent refunds. If no reservation remains in flight
and the remaining total cannot satisfy the effective per-object allowance, fail
the earliest canonical unlaunched target with `Exceeded { resource: TotalBytes,
observed_at_least: ceiling + 1 }`. Package listing/path budgets are shared across
targets. Peak payload is each role total plus at most one probe byte per in-flight
read. After failure, launch nothing else and return the earliest canonical
failure with no partial bundle.

<a id="compilation-builder-decision"></a>
### Compilation request builder decision

`CompilationAcquisitionRequest` does not gain a builder. The direct walkthrough
below explicitly names six OpenDAL adapter types. A builder cannot hide the three
independently useful source types or their metadata and would add another public
type while duplicating the same aggregate validation. The direct constructor is
the single validation seam.

<a id="publication-common"></a>
## Publication common contract

All names are under `typst_pack::opendal::publication`.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublicationPolicy {
    CreateOrVerify,
    OverwriteExactKeys,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublicationKeyOutcome {
    Created,
    AlreadyMatching,
    Written,
}
impl PublicationKeyOutcome {
    pub const fn commit_certainty(self) -> Option<CommitCertainty>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OpenDalPublicationPhase {
    ResultValidation,
    DestinationValidation,
    ResolveOperator,
    CapabilityAppraisal,
    PreflightRead,
    ConditionalCreate,
    RaceVerification,
    DirectWrite,
    Complete,
}
```

`PublicationPolicy` has no `Default` and there is no weakened fallback.
Package-cache publication is the one fixed-policy operation: its request always
uses `CreateOrVerify`, and `PackageCacheArchivePublicationRequest::policy()`
always returns that variant. `OverwriteExactKeys`
creates absent requested objects and overwrites present requested objects using
one direct write per key, without read, list, delete, staging, rollback,
post-write verification, or unrelated-key effects. `CreateOrVerify` appraises
read, write, conditional create, empty write, and advertised size as applicable;
it incrementally compares all planned keys before mutation, rejects known
conflicts, conditionally creates absent keys in deterministic order, and verifies
one conditional race.

Public causes report `UnsupportedPolicy { policy }`, not separate read/write/
empty/conditional capability variants. Capability details remain private.
`UnsupportedObjectSize` remains typed. OpenDAL exposes no portable one-shot
write maximum distinct from total/multipart limits, so an unadvertised one-shot
backend limit can fail during execution with conservative certainty. The
integration uses one buffer and one write and claims no multipart publication.

<a id="capability-table"></a>
### Capability table

All requirements refer only to effective advertised capabilities.

| Operation | Required advertised capabilities |
|---|---|
| Project survey/acquisition | `list`, `list_with_recursive`, `read` |
| Font survey/acquisition | `list`, `list_with_recursive`, `read` |
| Package Tree candidate | `list`, `list_with_recursive`, `read` |
| Cache/registry archive candidate | `read` |
| Pack Archive acquisition | `read` |
| Compilation Pack Override/font exact object | `read` |
| Compilation Package Tree | `list`, `list_with_recursive`, `read` |
| `CreateOrVerify` publication | `read`, `write`, `write_with_if_not_exists`; `write_can_empty` for empty payload; advertised total-size support |
| `OverwriteExactKeys` publication | `write`; `write_can_empty` for empty payload; advertised total-size support |

No publication operation requires or infers support from `write_can_multi`,
`write_multi_min_size`, or `write_multi_max_size`. Capability appraisal cannot
detect a backend-specific unadvertised one-shot maximum or an endpoint that lies
about conditional creation.

Multi-key publication is sequential. Create-or-verify preflights all keys before
writes; overwrite does not read. Empty operations succeed without resolving an
Operator. Prefix publication never lists, deletes stale objects, creates
directory markers, touches unrelated objects, or imposes filesystem ancestor
conflicts.

<a id="publication-requests"></a>
### Publication requests and operations

```rust
#[derive(Clone, Debug)]
pub struct PackArchivePublicationRequest { /* private */ }
impl PackArchivePublicationRequest {
    pub fn new(destination: Location, policy: PublicationPolicy) -> Result<Self, PackArchivePublicationRequestError>;
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
}
#[non_exhaustive]
pub enum PackArchivePublicationRequestError {
    InvalidDestinationRole { location: Location, source: LocationRoleError },
}

#[derive(Clone, Debug)]
pub struct PackExtractionPublicationRequest { /* private */ }
impl PackExtractionPublicationRequest {
    pub fn new(destination: Location, policy: PublicationPolicy) -> Result<Self, PackExtractionPublicationRequestError>;
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
}
#[non_exhaustive]
pub enum PackExtractionPublicationRequestError {
    InvalidDestinationRole { location: Location, source: LocationRoleError },
}

#[derive(Clone, Debug)]
pub struct PackageCacheArchivePublicationRequest { /* private */ }
impl PackageCacheArchivePublicationRequest {
    pub fn new(destination: Location) -> Result<Self, PackageCacheArchivePublicationRequestError>;
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
}
#[non_exhaustive]
pub enum PackageCacheArchivePublicationRequestError {
    InvalidDestinationRole { location: Location, source: LocationRoleError },
}
```

```rust
#[derive(Clone, Debug)]
pub struct CompilationArtifactPublicationRequest { /* private */ }
impl CompilationArtifactPublicationRequest {
    pub fn new(
        result: &CompilationResult,
        destination: Location,
        artifact_keys: impl IntoIterator<Item = impl Into<String>>,
        policy: PublicationPolicy,
    ) -> Result<Self, CompilationArtifactPublicationRequestRejection>;
    pub fn compilation_result_identity(&self) -> CompilationResultIdentity;
    pub fn destination(&self) -> &Location;
    pub fn artifact_keys(&self) -> &[String];
    pub const fn policy(&self) -> PublicationPolicy;
}

pub struct CompilationArtifactPublicationRequestRejection { /* nonempty */ }
impl CompilationArtifactPublicationRequestRejection {
    pub fn compilation_result_identity(&self) -> CompilationResultIdentity;
    pub fn issues(&self) -> &[CompilationArtifactPublicationRequestIssue];
}

#[non_exhaustive]
pub enum CompilationArtifactPublicationRequestIssue {
    ResultNotSucceeded,
    InvalidDestinationRole { location: Location, source: LocationRoleError },
    ArtifactKeyCountMismatch { expected: usize, actual: usize },
    InvalidArtifactKey { artifact_index: usize, key: String, reason: CompilationArtifactKeyIssue },
    DuplicateArtifactKey { key: String, first_artifact_index: usize, duplicate_artifact_index: usize },
}

#[non_exhaustive]
pub enum CompilationArtifactKeyIssue {
    Empty,
    LeadingSlash,
    TrailingSlash,
    RepeatedSeparator,
    DotSegment,
    Backslash,
    ControlCharacter,
    NormalizationAlias { index: usize },
}
```

Artifact keys are decoded UTF-8 relative operation keys, not URI text. Literal
percent is allowed and no percent decoding occurs. Empty segments, leading/
trailing/repeated separators, dot segments, backslashes, controls, and
normalization aliases reject. Exact duplicates reject; ancestor/descendant pairs
are allowed. Issues sort by result status, destination, count, artifact index,
then duplicate rank.

```rust
pub async fn publish_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchivePublicationRequest,
    archive: &PackArchiveBytes,
) -> Result<PackArchivePublicationReceipt, PackArchivePublicationError<R::Error>>;

pub async fn publish_pack_extraction_plan<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackExtractionPublicationRequest,
    plan: &PackExtractionPlan,
    progress: &mut PackExtractionPublicationProgress,
) -> Result<PackExtractionPublicationReceipt, PackExtractionPublicationError<R::Error>>;

pub async fn publish_package_cache_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageCacheArchivePublicationRequest,
    archive: &[u8],
) -> Result<PackageCacheArchivePublicationReceipt, PackageCacheArchivePublicationError<R::Error>>;

pub async fn publish_compilation_artifacts<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &CompilationArtifactPublicationRequest,
    result: &CompilationResult,
    progress: &mut CompilationArtifactPublicationProgress,
) -> Result<CompilationArtifactPublicationReceipt, CompilationArtifactPublicationError<R::Error>>;
```

The two mutable progress arguments are mandatory; no convenience overload
exists. On first poll, the operation clears the supplied progress before
validation/I/O. Callers normally pass a new value of the corresponding
operation-specific progress type.

<a id="publication-evidence"></a>
### Publication evidence

```rust
pub struct PackArchivePublicationEntry { /* private */ }
impl PackArchivePublicationEntry {
    pub fn destination_path(&self) -> &str;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub const fn commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct PackExtractionPublicationEntry { /* private */ }
impl PackExtractionPublicationEntry {
    pub fn relative_path(&self) -> &str;
    pub fn destination_path(&self) -> &str;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub const fn commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct CompilationArtifactPublicationEntry { /* private */ }
impl CompilationArtifactPublicationEntry {
    pub fn artifact_index(&self) -> usize;
    pub fn key(&self) -> &str;
    pub fn destination_path(&self) -> &str;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub const fn commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct PackageCacheArchivePublicationEntry { /* private */ }
impl PackageCacheArchivePublicationEntry {
    pub fn destination_path(&self) -> &str;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub const fn commit_certainty(&self) -> Option<CommitCertainty>;
}
```

The exact progress declarations are:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackArchivePublicationProgress { /* private */ }
impl PackArchivePublicationProgress {
    pub fn new() -> Self;
    pub fn completed(&self) -> Option<&PackArchivePublicationEntry>;
    pub fn outcome(&self) -> Option<PublicationKeyOutcome>;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageCacheArchivePublicationProgress { /* private */ }
impl PackageCacheArchivePublicationProgress {
    pub fn new() -> Self;
    pub fn completed(&self) -> Option<&PackageCacheArchivePublicationEntry>;
    pub fn outcome(&self) -> Option<PublicationKeyOutcome>;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackExtractionPublicationProgress { /* private */ }
impl PackExtractionPublicationProgress {
    pub fn new() -> Self;
    pub fn completed(&self) -> &[PackExtractionPublicationEntry];
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationArtifactPublicationProgress { /* private */ }
impl CompilationArtifactPublicationProgress {
    pub fn new() -> Self;
    pub fn completed(&self) -> &[CompilationArtifactPublicationEntry];
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}
```

Receipt types and exact accessors are:

```rust
pub struct PackArchivePublicationReceipt { /* private */ }
impl PackArchivePublicationReceipt {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn completed(&self) -> &PackArchivePublicationEntry;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub fn progress(&self) -> &PackArchivePublicationProgress;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct PackExtractionPublicationReceipt { /* private */ }
impl PackExtractionPublicationReceipt {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub fn pack_identity(&self) -> PackIdentity;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn completed(&self) -> &[PackExtractionPublicationEntry];
    pub fn progress(&self) -> &PackExtractionPublicationProgress;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct CompilationArtifactPublicationReceipt { /* private */ }
impl CompilationArtifactPublicationReceipt {
    pub fn compilation_result_identity(&self) -> CompilationResultIdentity;
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn completed(&self) -> &[CompilationArtifactPublicationEntry];
    pub fn progress(&self) -> &CompilationArtifactPublicationProgress;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}

pub struct PackageCacheArchivePublicationReceipt { /* private */ }
impl PackageCacheArchivePublicationReceipt {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn completed(&self) -> &PackageCacheArchivePublicationEntry;
    pub const fn outcome(&self) -> PublicationKeyOutcome;
    pub fn progress(&self) -> &PackageCacheArchivePublicationProgress;
    pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty>;
}
```

Every successful receipt reports phase `Complete`. `Created` and `Written`
carry `Some(Committed)`. `AlreadyMatching` attempted no destination effect and
carries `None`. Receipt `attempted_effects_commit_certainty()` is `None` when
every entry was `AlreadyMatching`; otherwise successful receipts return
`Some(Committed)`. There is no unconditional receipt `commit_certainty()`.
`CONTEXT.md` remains the authority for the domain term Commit Certainty and is
not redefined here.

`Created`, `Written`, `AlreadyMatching`, and `Committed` describe evidence at
the relevant read/write observation. They do not promise mutable destination
state remains unchanged when the call returns or that multi-key publication is
atomic. A streamed match proves one successful byte stream matched expected
bytes; OpenDAL provides no portable snapshot/version-bound read, so it does not
prove the complete object existed in that state at one instant.

<a id="publication-errors"></a>
### Publication errors

Every publication error has private fields, implements `std::error::Error`, and
exposes non-optional terminal `commit_certainty() -> CommitCertainty`.

```rust
pub struct PackArchivePublicationError<E> { /* private */ }
impl<E> PackArchivePublicationError<E> {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub fn failed_path(&self) -> Option<&str>;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn progress(&self) -> &PackArchivePublicationProgress;
    pub const fn commit_certainty(&self) -> CommitCertainty;
    pub fn cause(&self) -> &PackArchivePublicationErrorCause<E>;
}

pub struct PackExtractionPublicationError<E> { /* private */ }
impl<E> PackExtractionPublicationError<E> {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub fn failed_relative_path(&self) -> Option<&str>;
    pub fn failed_destination_path(&self) -> Option<&str>;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn progress(&self) -> &PackExtractionPublicationProgress;
    pub const fn commit_certainty(&self) -> CommitCertainty;
    pub fn cause(&self) -> &PackExtractionPublicationErrorCause<E>;
}

pub struct CompilationArtifactPublicationError<E> { /* private */ }
impl<E> CompilationArtifactPublicationError<E> {
    pub fn compilation_result_identity(&self) -> CompilationResultIdentity;
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub fn failed_artifact_index(&self) -> Option<usize>;
    pub fn failed_key(&self) -> Option<&str>;
    pub fn failed_destination_path(&self) -> Option<&str>;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn progress(&self) -> &CompilationArtifactPublicationProgress;
    pub const fn commit_certainty(&self) -> CommitCertainty;
    pub fn cause(&self) -> &CompilationArtifactPublicationErrorCause<E>;
}

pub struct PackageCacheArchivePublicationError<E> { /* private */ }
impl<E> PackageCacheArchivePublicationError<E> {
    pub fn destination(&self) -> &Location;
    pub const fn policy(&self) -> PublicationPolicy;
    pub fn failed_path(&self) -> Option<&str>;
    pub const fn phase(&self) -> OpenDalPublicationPhase;
    pub fn progress(&self) -> &PackageCacheArchivePublicationProgress;
    pub const fn commit_certainty(&self) -> CommitCertainty;
    pub fn cause(&self) -> &PackageCacheArchivePublicationErrorCause<E>;
}
```

```rust
#[non_exhaustive]
pub enum PackArchivePublicationErrorCause<E> {
    ResolveOperator(E),
    UnsupportedPolicy { policy: PublicationPolicy },
    UnsupportedObjectSize { byte_length: u64 },
    PreflightRead(::opendal::Error),
    ByteConflict { expected_byte_length: u64, observed_byte_length_at_least: u64 },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
    DirectWrite(::opendal::Error),
}

#[non_exhaustive]
pub enum PackExtractionPublicationErrorCause<E> {
    InvalidDestinationPath { relative_path: String },
    ResolveOperator(E),
    UnsupportedPolicy { policy: PublicationPolicy },
    UnsupportedObjectSize { byte_length: u64 },
    PreflightRead(::opendal::Error),
    ByteConflict { expected_byte_length: u64, observed_byte_length_at_least: u64 },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
    DirectWrite(::opendal::Error),
}

#[non_exhaustive]
pub enum CompilationArtifactPublicationErrorCause<E> {
    CompilationResultMismatch { expected: CompilationResultIdentity, actual: CompilationResultIdentity },
    InvalidDestinationPath { artifact_index: usize, key: String },
    ResolveOperator(E),
    UnsupportedPolicy { policy: PublicationPolicy },
    UnsupportedObjectSize { artifact_index: usize, byte_length: u64 },
    PreflightRead(::opendal::Error),
    ByteConflict { expected_byte_length: u64, observed_byte_length_at_least: u64 },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
    DirectWrite(::opendal::Error),
}

#[non_exhaustive]
pub enum PackageCacheArchivePublicationErrorCause<E> {
    ResolveOperator(E),
    UnsupportedPolicy { policy: PublicationPolicy },
    UnsupportedObjectSize { byte_length: u64 },
    PreflightRead(::opendal::Error),
    ByteConflict { expected_byte_length: u64, observed_byte_length_at_least: u64 },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
}
```

Because cause enums are non-exhaustive, callers match every variant they need
and retain a wildcard arm. For example:

```rust
fn pack_archive_publication_failure_kind<E>(
    cause: &PackArchivePublicationErrorCause<E>,
) -> &'static str {
    match cause {
        PackArchivePublicationErrorCause::ResolveOperator(_) => "resolver",
        PackArchivePublicationErrorCause::UnsupportedPolicy { .. } => "policy",
        PackArchivePublicationErrorCause::UnsupportedObjectSize { .. } => "size",
        PackArchivePublicationErrorCause::PreflightRead(_) => "read",
        PackArchivePublicationErrorCause::ByteConflict { .. } => "conflict",
        PackArchivePublicationErrorCause::ConditionalCreate(_) => "create",
        PackArchivePublicationErrorCause::RaceVerification(_) => "race",
        PackArchivePublicationErrorCause::DirectWrite(_) => "write",
        _ => "future",
    }
}
```

Both prefix operations validate every composed path before resolution/I/O.
Composed-path failures use `DestinationValidation`, empty progress, and
`NotCommitted`. Known pre-write conflicts and failed race comparisons are
`NotCommitted`. Other direct/conditional write errors are conservatively
`Indeterminate`.

<a id="capability-honesty"></a>
### Capability honesty

Capability appraisal rejects only incompatibilities knowable from advertised
capabilities. OpenDAL 0.58.1 hardcodes `write_with_if_not_exists` for every
S3-compatible service and implements it by adding `If-None-Match: *` to PUT. It
does not probe the endpoint. A nonconforming endpoint can advertise support and
silently overwrite, so create-or-verify cannot detect such an endpoint during
appraisal.

<a id="filesystem-policy-mapping"></a>
### Filesystem policy mapping

| Filesystem policy | OpenDAL relationship |
|---|---|
| `PublishNewTree` | No OpenDAL equivalent; OpenDAL does not root-commit a new tree. |
| `MergeCreateOnly` | Not `CreateOrVerify`; filesystem rejects every existing target, while OpenDAL accepts an exact match. |
| `MergeReplaceExactFiles` | Closest to `OverwriteExactKeys`, but filesystem-specific ancestor checks and replacement mechanics remain different. |
| No filesystem policy | `CreateOrVerify` has no exact filesystem equivalent. |

<a id="diagnostics-and-determinism"></a>
## Diagnostics and determinism

Request and survey issues aggregate independently detectable failures and sort
by semantic role, canonical target/path, and documented variant order. Backend
yield order, pagination, hash iteration, nonsemantic declaration order, and
completion timing are unobservable.

Outer `Display` and `Debug` render binding, typed role, and operation path.
Paths are not redacted because the grammar excludes authority, userinfo, query,
and fragment. Payload bytes, secrets, native resolver text, and native OpenDAL
text are excluded. Typed cause access and `Error::source()` preserve native
errors. Documentation must warn that full source-chain renderers such as
`anyhow` alternate display, `eyre`, and tracing error fields may disclose
backend endpoints or bucket names.

<a id="filesystem-limits-migration"></a>
## Filesystem limits migration

Typst-pack 0.5 migrates all three shipped filesystem limits constructors that
currently take adjacent positional `u64` values. No compatibility overload is
kept.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemProjectCeilings {
    pub visited_entries: u64,
    pub selected_files: u64,
    pub root_policy_bytes: u64,
    pub selected_file_bytes: u64,
    pub total_selected_bytes: u64,
}
impl FilesystemProjectCeilings { pub const fn reference_v1() -> Self; }
impl FilesystemProjectLimits {
    pub fn new(ceilings: FilesystemProjectCeilings) -> Result<Self, FilesystemProjectLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn visited_entries(&self) -> u64;
    pub const fn selected_files(&self) -> u64;
    pub const fn root_policy_bytes(&self) -> u64;
    pub const fn selected_file_bytes(&self) -> u64;
    pub const fn total_selected_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemFontCeilings {
    pub visited_entries: u64,
    pub accepted_containers: u64,
    pub container_bytes: u64,
    pub total_accepted_bytes: u64,
}
impl FilesystemFontCeilings { pub const fn reference_v1() -> Self; }
impl FilesystemFontLimits {
    pub fn new(ceilings: FilesystemFontCeilings) -> Result<Self, FilesystemFontLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn visited_entries(&self) -> u64;
    pub const fn accepted_containers(&self) -> u64;
    pub const fn container_bytes(&self) -> u64;
    pub const fn total_accepted_bytes(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemPackageCeilings {
    pub visited_entries: u64,
    pub selected_files: u64,
    pub selected_file_bytes: u64,
    pub package_tree_bytes: u64,
}
impl FilesystemPackageCeilings { pub const fn reference_v1() -> Self; }
impl FilesystemPackageLimits {
    pub fn new(ceilings: FilesystemPackageCeilings) -> Result<Self, FilesystemPackageLimitsError>;
    pub const fn reference_v1() -> Self;
    pub const fn visited_entries(&self) -> u64;
    pub const fn selected_files(&self) -> u64;
    pub const fn selected_file_bytes(&self) -> u64;
    pub const fn package_tree_bytes(&self) -> u64;
}
```

Reference values are unchanged. The migration removes the same
swapped-adjacent-limit hazard that OpenDAL avoids and keeps adapters visibly
consistent. Version 0.5 is the appropriate source-breaking boundary. Existing
non-filesystem Pack Archive encode/decode and package expansion limits are not
part of this migration.

<a id="identity-and-migration"></a>
## Identity and migration

Locations, bindings, limits, concurrency, source order, provenance, licensing,
cache metadata, artifact keys, destinations, policies, receipts, progress,
native errors, and Commit Certainty are operational and do not contribute to
Pack Identity, Compilation Identity, or Compilation Result Identity.

The existing all-feature implementation attestation does apply: enabling
`opendal` adds the feature name to Engine Identity and Exporter Identity and
therefore changes Compilation Identity and Compilation Result Identity without
changing their schemas. This is the same existing behavior used by `fs`,
`parallel`, and `diagnostics`. Featureless frozen vectors remain byte-for-byte
unchanged.

There is no earlier OpenDAL API to migrate. Feature-disabled and filesystem-only
users need no OpenDAL migration. Filesystem users invoking the three explicit
limits constructors migrate to named ceilings as described above.

<a id="walkthrough-validation"></a>
## Walkthrough validation

The three walkthroughs were compile-checked before implementation against a
temporary signature facade generated outside the repository. The facade declared
the exact OpenDAL signatures exercised by these walkthroughs, re-exported current
typst-pack core types, and used behaviorless `unimplemented!()` or trivial
placeholder bodies. Each checked-in Rust block was compiled verbatim in a
separate temporary binary with only an empty `main` appended, using rustc and
cargo 1.92.0. This checks imports, ownership, generic bounds, moves, borrows,
wildcard matching, and sync/async composition without claiming that the current
production crate exports these OpenDAL APIs. The facade and consumer were
throwaway research artifacts and are not production code or compatibility
fixtures.

Named-type counts below count distinct OpenDAL adapter types explicitly named in
imports, signatures, annotations, or qualified expression paths. Existing core
domain types and error types inferred through `?` are excluded.

<a id="walkthrough-pack-assembly"></a>
## Walkthrough: object-storage Pack Assembly

Named OpenDAL adapter types: **13**.

```rust
use std::collections::HashSet;
use std::error::Error;

use typst_pack::{
    DiscoverySpecification, FontCatalog, FontCatalogEntry, FontContainer, Pack,
    PackCreationInput, PackCreationOutcome, PackMetadata,
    PackageAcquisitionFailures, PackageCatalog, PackageDisposition,
    PackageExpansionLimits, ProjectSnapshotAssembly, create,
};
use typst_pack::opendal::{Location, OperatorBindings};
use typst_pack::opendal::pack_assembly as object_assembly;
use typst_pack::opendal::publication as object_publication;

pub async fn assemble_from_object_storage(
    bindings: &OperatorBindings,
    entrypoint: &str,
    project_source: Location,
    font_sources: Vec<object_assembly::FontSource>,
    package_tree_sources: Vec<object_assembly::PackageTreeSource>,
    archive_cache: Option<Location>,
    registry: Option<Location>,
    discovery: &DiscoverySpecification,
    metadata: Option<&PackMetadata>,
    package_disposition: PackageDisposition,
) -> Result<Pack, Box<dyn Error>> {
    let project_request = object_assembly::ProjectAcquisitionRequest::new(
        project_source,
        object_assembly::ProjectAcquisitionLimits::reference_v1(),
    )?;
    let (_, project_entries) =
        object_assembly::acquire_project(bindings, &project_request)
            .await?
            .into_parts();
    let project = ProjectSnapshotAssembly::new(entrypoint).assemble(
        project_entries
            .into_iter()
            .map(object_assembly::ProjectAcquisitionEntry::into_parts),
    )?;

    let font_request = object_assembly::FontAcquisitionRequest::new(
        font_sources,
        object_assembly::FontAcquisitionLimits::reference_v1(),
    )?;
    let (_, acquired_fonts) =
        object_assembly::acquire_fonts(bindings, &font_request)
            .await?
            .into_parts();
    let mut fonts = FontCatalog::new();
    for acquired in acquired_fonts {
        let (_, _, _, disposition, bytes) = acquired.into_parts();
        fonts.push(FontCatalogEntry::new(FontContainer::new(bytes)?, disposition));
    }

    let mut packages = PackageCatalog::new();
    let mut failures = PackageAcquisitionFailures::new();
    let mut attempted = HashSet::new();

    loop {
        match create(PackCreationInput {
            project: &project,
            packages: &packages,
            fonts: &fonts,
            package_failures: &failures,
            discovery,
            metadata,
        })? {
            PackCreationOutcome::Created { pack, warnings: _ } => return Ok(pack),
            PackCreationOutcome::MissingPackageSpecifications(missing) => {
                for spec in missing {
                    if !attempted.insert(spec.to_string()) {
                        return Err("Pack Creation repeated an attempted specification".into());
                    }
                    let request = object_assembly::PackageAcquisitionRequest::new(
                        spec,
                        package_tree_sources.clone(),
                        archive_cache.clone(),
                        registry.clone(),
                        object_assembly::PackageAcquisitionLimits::reference_v1(),
                    )?;
                    let acquisition =
                        object_assembly::acquire_package(bindings, &request).await?;

                    match &acquisition {
                        object_assembly::PackageAcquisition::Tree(_) => {}
                        object_assembly::PackageAcquisition::CachedArchive(_) => {}
                        object_assembly::PackageAcquisition::RegistryArchive(_) => {}
                        object_assembly::PackageAcquisition::Unavailable(_) => {}
                        _ => return Err("unsupported future PackageAcquisition variant".into()),
                    }

                    let residue = match object_assembly::insert_acquired_package(
                        &mut packages,
                        &mut failures,
                        acquisition,
                        package_disposition,
                        PackageExpansionLimits::reference_v1(),
                    ) {
                        Ok(residue) => residue,
                        Err(_typed_error) => {
                            // The mapped Package Acquisition Failure was retained;
                            // the next creation round reports it at the import.
                            continue;
                        }
                    };

                    if let Some(residue) = residue {
                        let cache_request =
                            object_publication::PackageCacheArchivePublicationRequest::new(
                                residue.destination().clone(),
                            )?;
                        if let Err(_cache_error) =
                            object_publication::publish_package_cache_archive(
                                bindings,
                                &cache_request,
                                residue.bytes(),
                            )
                            .await
                        {
                            // Cache failure is separate evidence and does not
                            // invalidate the inserted Package Tree.
                        }
                    }
                }
            }
        }
    }
}
```

<a id="walkthrough-pack-archive"></a>
## Walkthrough: publish then acquire Pack Archive

Named OpenDAL adapter types: **6**.

```rust
use std::error::Error;

use typst_pack::{Pack, PackArchiveBytes};
use typst_pack::pack_archive::{
    AcquisitionLimits, DecodeError, DecodeLimits, EncodeLimits, decode, encode,
};
use typst_pack::opendal::{Location, OperatorBindings};
use typst_pack::opendal::pack_archive::{
    PackArchiveAcquisitionRequest, acquire_pack_archive,
};
use typst_pack::opendal::publication::{
    PackArchivePublicationRequest, PublicationKeyOutcome, PublicationPolicy,
    publish_pack_archive,
};

pub async fn publish_then_acquire(
    bindings: &OperatorBindings,
    pack: &Pack,
    destination: Location,
) -> Result<(PackArchiveBytes, Result<Pack, DecodeError>), Box<dyn Error>> {
    let archive = encode(pack, EncodeLimits::reference_v1())?;
    let publish_request = PackArchivePublicationRequest::new(
        destination.clone(),
        PublicationPolicy::CreateOrVerify,
    )?;
    let receipt = publish_pack_archive(bindings, &publish_request, &archive).await?;
    match receipt.outcome() {
        PublicationKeyOutcome::Created => {}
        PublicationKeyOutcome::AlreadyMatching => {}
        PublicationKeyOutcome::Written => {}
        _ => return Err("unsupported future PublicationKeyOutcome variant".into()),
    }
    let _attempted_effects = receipt.attempted_effects_commit_certainty();

    let acquire_request = PackArchiveAcquisitionRequest::new(
        destination,
        AcquisitionLimits::reference_v1(),
    )?;
    let acquired = acquire_pack_archive(bindings, &acquire_request).await?;
    assert_eq!(archive.as_slice(), acquired.as_slice());

    // Decode borrows acquired bytes, so the caller retains exact bytes even
    // when semantic decoding fails.
    let decoded = decode(&acquired, DecodeLimits::reference_v1());
    Ok((acquired, decoded))
}
```

<a id="walkthrough-compilation"></a>
## Walkthrough: OpenDAL-fulfilled compilation

Named OpenDAL adapter types: **6**. This is the required non-builder form and is
the evidence for not adding a builder.

```rust
use std::error::Error;

use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationReport,
    DocumentTime, Pack, PackCompilationRequest, compile,
};
use typst_pack::opendal::OperatorBindings;
use typst_pack::opendal::compilation::{
    CompilationAcquisitionLimits, CompilationAcquisitionRequest,
    FontContainerSource, PackOverrideSource, PackageTreeSource,
    acquire_compilation_bundle,
};

pub async fn compile_with_object_storage_inputs(
    bindings: &OperatorBindings,
    pack: Pack,
    pack_overrides: Vec<PackOverrideSource>,
    packages: Vec<PackageTreeSource>,
    fonts: Vec<FontContainerSource>,
    output: CompilationOutputSpecification,
    document_time: DocumentTime,
) -> Result<CompilationReport, Box<dyn Error>> {
    let acquisition_request = CompilationAcquisitionRequest::new(
        &pack,
        pack_overrides,
        packages,
        fonts,
        CompilationAcquisitionLimits::reference_v1(),
    )?;
    let acquired = acquire_compilation_bundle(bindings, &acquisition_request).await?;
    let values = acquired.into_values(&pack)?;
    let (pack_overrides, fulfillments) = values.into_parts();

    let request = PackCompilationRequest::new(pack, output)
        .adapter_resolved_overrides(pack_overrides)
        .fulfillments(fulfillments)
        .document_time(document_time);
    Ok(compile(request, CompilationLimits::reference_v1())?)
}
```

<a id="minio-fixture"></a>
## MinIO fixture

Registry lookup at specification authoring time selected these manifest-list
pins:

```text
docker.io/minio/minio:RELEASE.2025-04-22T22-12-26Z
docker.io/minio/minio@sha256:a1ea29fa28355559ef137d71fc570e508a214ec84ff8083e39bc5428980b015e

docker.io/minio/mc:RELEASE.2025-08-13T08-35-41Z
docker.io/minio/mc@sha256:a7fe349ef4bd8521fb8497f55c6042871b2ae640607cf99d9bede5e9bdf11727
```

Linux amd64 child manifests are:

```text
minio/minio sha256:3f97c5651cb6662b880c787a232b6b34fec8d8922e08d6617b25d241a21164bb
minio/mc    sha256:eb4ea9884b77704230e2423e9004d2fa738dc272876b9cc41a297d29443b8780
```

The server/client fixture is dedicated Linux CI with deterministic bootstrap,
cleanup, and a no-silent-skip guard. MinIO failures block release, not ordinary
merges; binaries are absent from the default PR matrix. The fixture exercises
listing, conditional create, representative one-shot/advertised-size boundaries,
multipart constraints without claiming multipart publication, and write-failure
behavior.

The pinned server must honor `If-None-Match: *` on PUT. #213 proves this
behaviorally by creating one value, issuing a conditional PUT with different
bytes, requiring precondition failure, and verifying the original bytes remain.
The tag/digest is not accepted as semantic proof.

<a id="test-matrix"></a>
## Test and CI matrix

Public behavior is asserted through typed values/causes, ordering, exact bytes,
identities, semantic outcomes, rejections, receipts, progress, Commit Certainty,
and observable destination state. Tests do not assert private stage structure,
native message text, sleeps, ambient credentials, random chaos, or behavior
outside appraised capabilities.

Required evidence:

- Location/binding parsing, display, byte indexes, error precedence, roles,
  lexical bindings, duplicates, unknown bindings, and one-resolution reuse.
- ASCII/Unicode whitespace aliases; U+FEFF/U+200B non-aliases; root/non-root,
  siblings, prefix markers, and composed-path normalization.
- Vendored normalization differential tests including `a/./b` and trailing
  U+FEFF.
- Every acquisition capability combination, complete survey-before-read,
  entry kind, path safety, canonical order, disappearance race, exact bytes,
  fallback, and authoritative core handoff.
- Every zero, exact, plus-one, overflow, incompatible-construction, and
  simultaneous-overage limit case with fixed precedence.
- Package Tree/cache/registry/unavailable paths; lazy resolution; no
  `NetworkFailed`; insertion mapping; validated cache residue; cache failure
  independence.
- Pack Archive bounded reads, absence, matching/conflicting publication,
  conditional races, retry material, and every certainty branch.
- Compilation aggregate preflight, exact coverage, canonical scheduling,
  reservation blocking/exhaustion, completion permutations, earliest failure,
  cancellation, peak retained bytes, conversion, mismatch, and no partial
  bundle.
- Prefix publication composed-path validation before mutation, exact mapping,
  sequential effects, unrelated-key preservation, cancellation, dropped-future
  progress, replay, overwrite, and `AlreadyMatching` without certainty.
- Deterministic private `opendal::raw::Access` services for capabilities,
  operation logs, injected failures, races, completion order, cancellation, and
  indeterminate writes. This is dev-only non-semver-stable coupling.
- All public workflow contracts against Memory on Linux, Windows, and macOS.
- MinIO S3-compatible evidence on Linux.
- Cross-adapter Pack Creation conformance for source-neutral fixtures, excluding
  filesystem-specific ignore-policy scenarios.
- Publish-then-acquire Pack Archive exact-byte evidence before decode.
- OpenDAL compilation equivalence for request inventory, Compilation Identity,
  Compilation Result, and Compilation Result Identity.
- Pack Extraction and artifact publication exact keys/bytes, receipts,
  progress, certainty, retry evidence, and unrelated-object preservation.
- OpenDAL-only build/test on all three behavioral targets, Linux all-features,
  and MSRV 1.92.
- Existing featureless, filesystem, egress, native filesystem, all-features,
  featureless-Wasm, identity, differential, and fuzz-regression gates.
- API absence or compile-fail evidence when `opendal` is disabled.
- Normal/build dependency-graph feature assertions from the exact list above.
- Featureless identity vectors unchanged, all-feature vectors updated, and one
  OpenDAL-enabled attestation vector.
- docs.rs build with `opendal` enabled.

<a id="documentation-requirements"></a>
## Documentation requirements

User documentation must cover feature enablement, dependency rationale,
caller-owned Operator/runtime/backend setup, advertised-only capability
boundaries, exact-object and prefix roles, reference limits and cumulative
retained-memory exposure, native source-chain disclosure risk, cancellation,
replay, partial effects, Commit Certainty, representative rather than universal
backend evidence, sync/async handoffs, package insertion/cache safety, and
identity attestation.

Cause-enum examples match declared variants and include a future-compatible
wildcard. Archive examples retain acquired bytes across decode failure. Imports
are explicit and use module aliases where same-named source types could collide.

<a id="out-of-scope"></a>
## Out of scope

The initial release does not certify every OpenDAL backend, detect endpoints
that misreport capabilities, promise Wasm/32-bit OpenDAL support, provide
portable transactions or atomic multi-key publication, guarantee all acquired
values coexisted, revalidate sources, use ETags as consistency guarantees,
reinterpret local-only system-font/viewer/CA/timing/path behavior, alter Pack or
identity schemas, or provide durable execution orchestration.
