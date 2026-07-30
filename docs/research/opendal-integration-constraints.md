# OpenDAL Integration Constraints

Research for [Establish the OpenDAL integration constraints](https://github.com/sagikazarmark/typst-pack/issues/132), 2026-07-30.

## Answer

OpenDAL can provide the storage I/O behind a first-party `typst-pack` integration, but it must remain an adapter outside the existing synchronous, byte-oriented Pack operations. The stable seam is an OpenDAL `Operator` plus a root-relative object path. Acquisition adapters asynchronously list and read objects into the existing Pack-owned input types; publication adapters serialize complete bytes first and then write them. They must not move path canonicalization, Pack validation, or compilation fallback into storage code.

Later design tickets must treat these facts as fixed:

- An OpenDAL operation accepts a path relative to its configured operator root. It does not resolve a full storage URI supplied as an operation path.
- `Operator::from_uri` constructs only an `Operator`. The URI path becomes that operator's root; the API does not return a separate object or prefix path. Full-location UX therefore needs a `typst-pack` resolver that produces the equivalent of `(Operator, relative_path)`.
- OpenDAL is async-first. Its blocking facade drives async operations through Tokio; it is not a native synchronous storage path and must not be called from an async context.
- Backend availability is selected at compile time through `services-*` Cargo features and registration. Runtime URI parsing cannot load a backend that was not compiled and registered.
- Backend and composed-layer capabilities vary. Every workflow must inspect the effective `OperatorInfo::capability()` and either use a defined fallback or report that the workflow is unsupported.
- No workflow may assume recursive listing, directories, rename or copy, conditional writes, versions or ETags, multipart behavior, or atomic publication across all operators.
- OpenDAL's target accommodations do not make every backend portable to every target. Backend features must be checked on each supported target, especially `wasm32-unknown-unknown`.
- Apache OpenDAL has no profile-name configuration abstraction. `ProfileOperatorFactory` is supplied by the separate third-party `opendal-util` crate.
- `typst-pack` keeps default features empty, keeps its featureless core available on `wasm32-unknown-unknown`, and keeps `fs` transport-free. An OpenDAL integration must be separately opt-in and must not alter those guarantees.

## Release Baseline

This research examines Apache OpenDAL `v0.58.0`, released on 2026-07-16 and marked as the latest GitHub release on 2026-07-30. The release changed operator construction and runtime composition and removed the old native/full capability split, so designs should use the `0.58` APIs documented here rather than older examples.

There is one dependency-selection caveat. On 2026-07-30 docs.rs marks `opendal 0.58.0` as yanked, and `cargo info opendal@0.58.0` cannot select it; unversioned `cargo info opendal` resolves `0.57.0`. The report still records the latest released API direction, but implementation must revalidate its API, MSRV, features, and backend matrix against the non-yanked version Cargo can publish at implementation time. It must not add a Git dependency merely to force `0.58.0`.

## Operator Construction

The released construction surfaces serve two different cases:

| API | Selection time | Constraint |
| --- | --- | --- |
| `Operator::new(builder)` | Backend type chosen statically | Builds a ready-to-use operator and installs OpenDAL's correctness/completion layers |
| `Operator::from_config(config)` | Backend config type chosen statically | Converts the typed config to its builder |
| `Operator::from_iter::<B>(values)` | Backend type chosen statically | Parses generic key/value configuration for known builder `B` |
| `Operator::via_iter(scheme, values)` | Scheme chosen at runtime | Delegates to the global registry |
| `Operator::from_uri(uri)` | Scheme chosen at runtime | Parses the URI and delegates to the global registry |
| `OperatorRegistry::{register,load}` | Registration at startup or by the facade | Maps a scheme to a compiled builder factory |

`OperatorRegistry` always registers the memory service. Other facade services are optional crates behind `services-*` features, and their automatic registration additionally depends on `auto-register-services`. An unknown or unregistered scheme returns `ErrorKind::Unsupported`. Dynamic construction is therefore dynamic only among factories already present in the binary.

For caller-supplied locations, accepting an already configured `Operator` is the least-assumption API. It preserves caller-owned credentials, HTTP transport, executor, layers, root, and backend feature selection. URI construction is a separate convenience with a larger compile-time and configuration contract; it is not required to use an operator-relative path.

## Paths and Full URIs

OpenDAL normalizes operation paths as root-relative object paths:

- leading slashes and surrounding whitespace are removed;
- repeated internal slashes are collapsed;
- an empty path becomes `/`;
- a trailing slash distinguishes a directory path from a file path.

The configured service root is normalized separately and prepended by the backend. Consequently, `operator.read("s3://bucket/key")` does not select S3 or `bucket`; it passes a string resembling `s3:/bucket/key` to the operator's existing service after ordinary path normalization.

`OperatorUri` parses a URI into scheme, authority/name, credentials, query options, and `root`. It percent-decodes the URI path, trims its leading and trailing slashes, and exposes that value as the root. `OperatorRegistry::load` passes the parsed value to a service configurator and returns only the constructed `Operator`. There is no returned residual object path.

This creates a hard distinction for the planned location forms:

- A scoped location is naturally represented as a caller-supplied `Operator` and one validated relative path or prefix.
- A full URI naming an object or prefix cannot be passed directly to an operation and cannot be recovered from `Operator::from_uri` after construction. A `typst-pack` location resolver must parse the complete location, construct or obtain the operator at the intended scope, retain the residual object/prefix path, and reject ambiguous forms.

The resolver must define the split per supported URI contract. It must not guess that OpenDAL's URI `root` is the residual object name: OpenDAL treats it as operator configuration. Credentials and query values also belong to operator construction and must not leak into diagnostics.

OpenDAL path normalization is not a substitute for Pack path validation. `ProjectSnapshotAssembly` and `Pack` continue to canonicalize project and package paths and reject duplicates, invalid trees, and ambiguous archive identities.

## Async, Runtime, and Blocking

`opendal::Operator` is the public asynchronous entry point. It is `Clone + Send + Sync`, and its operations borrow `&self`. Runtime resources are carried by `OperationContext`; `Operator::with_context` can replace the HTTP transport or executor while preserving layer composition.

The default facade enables a Tokio executor. OpenDAL also exposes an `Executor` wrapper over a user-provided `Execute` implementation, so Tokio is not an unavoidable requirement for all async use. Without an enabled or supplied executor, background/concurrent tasks fail when invoked. HTTP-backed services likewise need a configured HTTP transport; the default facade installs Reqwest with Rustls.

The optional `blocking` feature exposes `opendal::blocking::Operator`. Its own documentation fixes the constraints:

- it wraps an async `Operator` and invokes an async runtime's `block_on` path;
- construction captures the current Tokio runtime handle;
- pure blocking callers must create and enter a runtime first;
- calls must run in blocking context, not directly inside async context.

This facade does not justify adding OpenDAL I/O inside `typst-pack`'s synchronous Pack Creation or Compilation Kernel. The normal integration shape is:

1. Await OpenDAL acquisition in an async adapter.
2. Hand owned paths and bytes to `ProjectSnapshotAssembly`, `CreationRequest`, `Pack::from_bytes`, `PackOverrideSet`, `PackageTreeFulfillment`, or `FontContainerFulfillment`.
3. Run the existing synchronous semantic operation.
4. Serialize a complete Pack or Compilation Output Artifact to bytes.
5. Await OpenDAL publication.

A synchronous convenience may be designed separately, but it must expose its Tokio/runtime restrictions and must not be the foundational API.

## Cargo and Backend Features

OpenDAL `0.58.0` declares MSRV 1.91; this workspace declares 1.92, so the inspected release does not raise the current workspace MSRV.

The facade's default feature set includes:

- automatic service registration;
- Tokio execution;
- Reqwest HTTP transport with Rustls;
- concurrent-limit, logging, retry, and timeout layers.

Backend implementations are separate optional dependencies selected with `services-*` features. Memory is always available; `services-memory` is a deprecated no-op compatibility feature. Enabling `default-features = false` avoids the default runtime, transport, and layers, but then the integration or downstream application must deliberately provide all required registration, executor, transport, and service features.

These facts constrain the later Cargo design:

- `typst-pack`'s OpenDAL dependency must be optional and absent from default features.
- The exact enabled OpenDAL defaults must be deliberate. Enabling defaults imports a Tokio/Reqwest/Rustls execution stack even when the caller supplies a local operator; disabling defaults shifts registration and runtime responsibility to the application.
- `typst-pack` cannot promise that arbitrary URI schemes work merely because it accepts URI strings. Its documentation and errors must distinguish an invalid URI from a valid but uncompiled or unregistered service.
- A backend forwarding policy, if offered, must be explicit Cargo features. A single `opendal` feature cannot imply every backend without a very large and target-incompatible dependency graph.
- The workspace's all-features checks will compile every forwarded backend together. Any backend feature exposed by `typst-pack` must satisfy the repository's license/source policy and all advertised target checks.

## Target Constraints

OpenDAL core has explicit wasm accommodations: boxed futures become `LocalBoxFuture` and its `MaybeSend` marker drops the `Send` requirement on `wasm32`. The OPFS service is linked only under `cfg(target_arch = "wasm32")` and uses browser APIs. This demonstrates intentional wasm support, not universal backend support.

Backend crates have independent native libraries, runtimes, transports, and target-specific dependencies. The facade also makes OPFS itself target-specific. Therefore no `services-*` feature should be described as portable without compiling and testing that exact feature/target combination.

For `typst-pack`, the fixed target rules are:

- The featureless library must continue to compile for `wasm32-unknown-unknown`.
- The existing `fs` feature remains unavailable on wasm and transport-free on native targets.
- The OpenDAL integration is a separate optional capability. If it claims wasm support, tests must cover its selected OpenDAL features on `wasm32-unknown-unknown`; native success is insufficient.
- The `blocking` facade is not the wasm integration strategy. Wasm-facing adapters should remain async and pass owned bytes across the existing adapter-neutral seam.

## Capability Variance

`OperatorInfo::capability()` is the public contract for the current composed operator, including capabilities supplied by layers. OpenDAL `0.58.0` removed the previous native-versus-full capability APIs. Designs should inspect this effective value, not infer behavior from the service scheme.

The capability record independently describes basic operations and variants, including:

- `read`, `write`, `stat`, `list`, `delete`, `copy`, `rename`, and `create_dir`;
- recursive/versioned/deleted listing and recursive deletion;
- conditional read, stat, write, copy, and rename behavior;
- multipart and segmented-copy support and size limits;
- total write size, empty-write, append, metadata, and presign support.

Workflow implications are:

| Workflow need | Required appraisal |
| --- | --- |
| Read one Pack archive, Pack Override, font, or artifact | `read`; optionally supported conditions/version when consistency evidence requires them |
| Acquire a project or Complete Package Tree from a prefix | `list` and `read`; recursively traverse when `list_with_recursive` is absent or report unsupported when traversal cannot preserve semantics |
| Write a Pack or Compilation Output Artifact | `write`, `write_total_max_size`, empty-write support, and multipart limits for the chosen strategy |
| Extract a Pack to a prefix | `write`; directory creation only when the operator actually models directories; collision/publication policy cannot assume conditional writes |
| Publish without exposing partial output | A backend-specific staging and commit strategy; `rename`, `copy`, and conditional destination creation are optional and have distinct semantics |
| Revalidate an acquired mutable source | Metadata actually returned plus supported conditional/version operations; ETag or version availability cannot be presumed |

Capability presence alone does not establish cross-object snapshot consistency, transactional multi-object publication, strong listing consistency, or atomic rename semantics. Those are workflow/backend contracts that later tickets must either establish with evidence or explicitly decline. An adapter must never silently weaken a Pack-owned all-or-nothing or consistency guarantee because a backend lacks a convenient operation.

## Fit With Existing `typst-pack`

The repository already supplies the correct semantic boundary:

- `crates/typst-pack/Cargo.toml` keeps `default = []`, separates `fs` from `egress`, and states that `fs` links no transport and is unavailable on wasm.
- `docs/adr/0008-adapter-neutral-pack-creation.md` fixes Pack Creation as synchronous and acquisition-free. Creation Preparation and Creation Adapters own listing, reading, and fetching; transformations and validation stay in the core.
- `crates/typst-pack/src/project_snapshot.rs` makes `ProjectSnapshotAssembly` reapply ignore policy, canonicalize paths, reject duplicates, enforce budgets, and require the entrypoint.
- `crates/typst-pack/src/creation.rs` accepts owned Project Snapshot, package, and font bytes and documents consistency of acquired bytes as the Creation Adapter's evidence obligation.
- `crates/typst-pack/src/pack.rs` exposes `Pack::read<R: Read + Seek>`, `Pack::from_bytes`, `Pack::write<W: Write + Seek>`, and `Pack::to_bytes`. Whole-Pack validation remains behind `Pack` construction.
- `crates/typst-pack/src/compile.rs` accepts in-memory `PackOverrideSet`, `PackageTreeFulfillment`, and `FontContainerFulfillment` values and has no project, package, font, cache, or network fallback.
- `docs/adr/0002-make-pack-owner-of-whole-pack-invariants.md` assigns canonical paths, coherent trees, entrypoint presence, package/font agreement, and ambiguous-archive rejection to `Pack`.
- `docs/adr/0007-separate-library-and-cli-crates.md` places reusable integrations in the published library and keeps process-only configuration in `typst-pack-cli`. The current map also excludes OpenDAL CLI configuration.

OpenDAL adapters should compose these surfaces rather than create parallel semantic models. In particular, they should collect storage paths and bytes and then call the same assemblies/builders used by filesystem and in-memory callers. They should serialize complete byte values before publication unless a later ticket proves a streaming design preserves the same all-or-nothing behavior.

## Third-Party `opendal-util`

`opendal-util 0.7.0`, released by `sagikazarmark/opendal-util` on 2026-07-23, is not part of Apache OpenDAL. Its `ProfileOperatorFactory` interprets a URI scheme as a profile name, reads a profile map containing a `type`, rewrites the URI scheme, merges profile values into `OperatorUri`, and then calls `Operator::from_uri`.

That utility may inform a future application-level configuration adapter, but profile names are not OpenDAL URI semantics and must not be documented as such. The planned first-party `typst-pack` library integration has no requirement to adopt profiles, particularly because CLI and deployment configuration are out of scope.

## Requirements for Later Tickets

1. Define one validated location value that always resolves to an `Operator` and a relative file or prefix path. Specify URI splitting, percent-decoding, slash handling, credential redaction, root locations, and file-versus-prefix rules.
2. Keep async I/O orchestration outside synchronous Pack Creation and compilation. Do not base the public integration on `blocking::Operator`.
3. Decide the Cargo ownership model for OpenDAL defaults, automatic registration, service forwarding, executor, and HTTP transport. State exactly which URI schemes a given feature set can construct.
4. Define a capability appraisal per storage role and workflow. Errors must identify the missing capability and location role before partial mutation.
5. Define consistency evidence for multi-object acquisition independently of OpenDAL's generic API. Conditional reads, ETags, versions, and list consistency are optional backend facts.
6. Define publication guarantees per capability set. Direct write, staged copy, staged rename, and conditional creation are not equivalent; call the result atomic only when the active backend contract proves it.
7. Preserve Pack-owned canonicalization and validation by routing acquired bytes through existing `typst-pack` types. Never trust OpenDAL-normalized names as canonical Pack paths.
8. Test each promised backend/target/feature combination. Include an operator with deliberately missing capabilities so fallback and refusal behavior is exercised independently of a named service.
9. Recheck the non-yanked OpenDAL release before implementation. `v0.58.0` establishes direction but was not Cargo-selectable on the research date.

## Primary Sources

All online sources below were accessed on 2026-07-30. Versioned links pin the inspected release or tag.

### Apache OpenDAL

- [OpenDAL `v0.58.0` release, 2026-07-16](https://github.com/apache/opendal/releases/tag/v0.58.0)
- [OpenDAL `0.58.0` facade Cargo manifest and feature definitions](https://github.com/apache/opendal/blob/v0.58.0/core/Cargo.toml)
- [OpenDAL `0.58.0` core Cargo manifest, runtime features, and target dependencies](https://github.com/apache/opendal/blob/v0.58.0/core/core/Cargo.toml)
- [OpenDAL `0.58.0` operator API](https://docs.rs/opendal/0.58.0/opendal/struct.Operator.html)
- [OpenDAL `0.58.0` operator constructors](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/types/operator/builder.rs)
- [OpenDAL `0.58.0` `OperatorUri`](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/types/operator/uri.rs)
- [OpenDAL `0.58.0` `OperatorRegistry`](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/types/operator/registry.rs)
- [OpenDAL `0.58.0` path normalization](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/raw/path.rs)
- [OpenDAL `0.58.0` `OperationContext`](https://docs.rs/opendal/0.58.0/opendal/struct.OperationContext.html)
- [OpenDAL `0.58.0` executor implementation](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/types/execute/executor.rs)
- [OpenDAL `0.58.0` Tokio executor](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/types/execute/executors/tokio_executor.rs)
- [OpenDAL `0.58.0` blocking operator](https://docs.rs/opendal/0.58.0/opendal/blocking/struct.Operator.html)
- [OpenDAL `0.58.0` capability contract](https://docs.rs/opendal/0.58.0/opendal/struct.Capability.html)
- [OpenDAL `0.58.0` wasm future and `MaybeSend` adaptation](https://github.com/apache/opendal/blob/v0.58.0/core/core/src/raw/futures_util.rs)
- [OpenDAL `0.58.0` OPFS target dependencies](https://github.com/apache/opendal/blob/v0.58.0/core/services/opfs/Cargo.toml)
- [docs.rs OpenDAL `0.58.0` feature inventory and yanked status](https://docs.rs/crate/opendal/0.58.0/features)
- [crates.io OpenDAL `0.57.0`, the selectable release on the research date](https://crates.io/crates/opendal/0.57.0)

### Third-Party Utility

- [`opendal-util v0.7.0` release, 2026-07-23](https://github.com/sagikazarmark/opendal-util/releases/tag/v0.7.0)
- [`opendal-util v0.7.0` operator factories and `ProfileOperatorFactory`](https://github.com/sagikazarmark/opendal-util/blob/v0.7.0/src/factory.rs)
