# Upstream Typst Compilation and Artifact Constraints

This note answers [issue #29](https://github.com/sagikazarmark/typst-pack/issues/29): what upstream Typst permits and requires around compilation, `World` access, diagnostics, timing, cancellation, concurrency, incremental/watch behavior, document lifetimes, export formats, and streaming.

## Scope and versions

typst-pack pins all Typst crates to `=0.15.0`; its accepted CLI parity ADR therefore treats that exact embedded version as the behavioral baseline ([Cargo.toml](../../Cargo.toml#L59-L67), [ADR-0005](../adr/0005-align-cli-with-embedded-typst.md#L14-L19)). The corresponding upstream tag is [`v0.15.0` at `3ae52774b48987fc78a72ff483068cacc28e46c2`](https://github.com/typst/typst/releases/tag/v0.15.0).

As of 2026-07-18, the latest upstream release is [`v0.15.1` at `9dfd3a08500b7896045f907433cf7b4b02434fad`](https://github.com/typst/typst/releases/tag/v0.15.1), and the inspected `main` snapshot is [`89495172a5b6a697cbf2d6c560efb2da9dfd173b`](https://github.com/typst/typst/commit/89495172a5b6a697cbf2d6c560efb2da9dfd173b). The 0.15.1 changelog contains correctness fixes, including SVG and experimental Bundle fixes, but no change to the compilation, `World`, document, timing, or exporter return-type boundaries described below ([0.15.1 changelog](https://typst.app/docs/changelog/0.15.1/), [0.15.0..0.15.1 comparison](https://github.com/typst/typst/compare/3ae52774b48987fc78a72ff483068cacc28e46c2...9dfd3a08500b7896045f907433cf7b4b02434fad)). Those key signatures also remain present at the inspected `main` snapshot ([current `compile`](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst/src/lib.rs#L63-L82), [current `World`](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst-library/src/lib.rs#L45-L98)).

In this note:

- **Public interface** means an exported Rust API in the pinned Typst release. It does not claim a compatibility guarantee beyond that release.
- **Public helper** means an exported API from a supporting crate such as `typst-kit`, not a requirement of the core compiler.
- **Implementation fact** means observed compiler or CLI behavior. typst-pack can learn from it, but should not treat it as a stable contract.

## Findings at a glance

| Concern | Upstream constraint | Evidence class |
| --- | --- | --- |
| Compilation | `typst::compile` is a synchronous function returning an owned output-or-errors value plus warnings. | Public interface |
| Input access | Every `World` method is synchronous. Source and binary file reads return complete `Source` and `Bytes` values. | Public interface |
| Async I/O | There is no future-, stream-, or callback-based compiler input interface. Async work must end at an adapter before or during a blocking `World` call. | Public interface implication |
| Cancellation | `compile` and `World` expose no cancellation token, deadline, or progress callback. | Public interface absence |
| Concurrency | A `World` must be `Send + Sync`; the compiler and exporters can use Rayon internally. | Public interface and implementation fact |
| Incrementality | The `World` owns input caching and invalidation. Reusing and editing `Source` values enables memoized work to be retained. | Public interface guidance |
| Watch | The CLI compiles, waits for dependency events, resets its world, and compiles again sequentially. It does not preempt an active compilation. | CLI implementation fact |
| Diagnostics | Diagnostics are owned, but rich rendering looks source text and paths up through a `World`. | Public interface and public helper |
| Timing | Timing collection is process-global, and the helper exports Chrome trace JSON after the measured call. | Public interface and public helper |
| Documents | Compilation returns a complete owned `PagedDocument`, `HtmlDocument`, or experimental `Bundle`; none borrows the `World`. | Public interface |
| Export | PDF, HTML, SVG, PNG rendering, and Bundle export materialize complete return values. | Public interface |
| Streaming | Inputs can be loaded lazily per file and page artifacts can be forwarded individually after compilation, but upstream has no byte-streaming compile or artifact-export API. | Public interface implication |

## Compilation and `World`

### Public boundary

The core entry point is:

```rust
pub fn compile<T>(world: &dyn World) -> Warned<SourceResult<T>>
where
    T: Output;
```

For this signature, `Warned` contains `output: SourceResult<T>` and an owned vector of warnings. Compilation therefore reports warnings independently from fatal errors and returns its document by value ([compiler entry point](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L63-L82), [`Warned` and `SourceDiagnostic`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/diag.rs#L281-L320)).

`World` is `Send + Sync`. All methods take `&self`, and all are ordinary synchronous methods. `library` and `book` return shared references; `main` returns a `FileId`; `source`, `file`, and `font` return owned reference-counted values; and `today` returns an optional date ([`World` definition](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L45-L98)). In particular, the input methods are:

```rust
fn source(&self, id: FileId) -> FileResult<Source>;
fn file(&self, id: FileId) -> FileResult<Bytes>;
fn font(&self, index: usize) -> Option<Font>;
```

There is no reader, stream, future, or borrowed byte range in those signatures. A `World` may load lazily when Typst requests a `FileId`, but each successful call must return the complete requested value synchronously ([`World::source` and `World::file`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L69-L88)).

Typst explicitly assigns caching to the `World`: repeated loading should be cheap, immutable fonts may be cached across compilations, source entries may be cleared between compilations, and advanced clients may retain and edit `Source` values in place for better incremental performance ([`World` caching guidance](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L45-L58)).

`typst-kit::FileStore` is an optional public helper for this contract. Its `FileLoader` is also synchronous and returns a complete `Bytes`; `FileStore` caches file/source conversions and can report which `FileId`s were accessed ([`FileStore` contract and accessors](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L19-L99), [`FileLoader`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L246-L266)). This helper is useful precedent, not a requirement to use `FileStore`.

### Completion boundary

Compilation first evaluates the main source, then repeatedly creates a document until introspection stabilizes or the iteration limit is reached. It promotes delayed errors before returning the final document ([compilation loop](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L116-L193)). A page or partial DOM is not exposed while this process runs; the public result appears only after the whole target document has completed.

## Async execution and cancellation

### Async I/O

Neither `compile` nor `World` has an async variant, and the supporting `FileLoader` is synchronous as well ([`compile`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L63-L82), [`World`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L59-L98), [`FileLoader`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L246-L266)). Consequently:

- An async caller can run the synchronous compiler on a blocking worker, but wrapping it in an async task does not make `World` access asynchronous.
- Remote or otherwise async-backed inputs must be prefetched/materialized, synchronously blocked on inside the adapter, or surfaced as a failed lookup for an outer retry. Typst cannot suspend a compilation at a missing file request and later resume it through a public API.
- Lazy loading remains possible at file granularity, but not as incremental chunks of one file.

These are consequences of the public signatures, not a typst-pack policy choice.

### Cancellation and progress

The public compile call accepts only `&dyn World`, and `World` has no cancellation, deadline, or progress method. There is therefore no public cooperative cancellation or progress-reporting hook at the compile boundary ([`compile` and `World`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L63-L82)). Dropping or cancelling an outer async future does not itself signal the already-running synchronous compiler.

The CLI watch loop likewise calls `compile_once` to completion before returning to `watcher.wait`; changes arriving during a compile cannot preempt that compile through this implementation ([watch loop](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/watch.rs#L59-L83)). Hard deadlines or preemption therefore require an isolation or termination mechanism outside the public in-process compiler API; upstream does not supply one.

## Concurrency

The `Send + Sync` bound and shared `&self` access mean a `World` implementation must tolerate calls from parallel compiler work. Implementations that cache, record dependencies, or lazily fetch values need internal synchronization ([`World` bound](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L59-L98)).

Parallelism is not merely theoretical:

- The compiler's `Engine::parallelize` clones the tracked compilation context and executes work through Rayon while preserving result order ([engine parallelization](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/engine.rs#L52-L102)).
- Experimental Bundle compilation compiles child documents in parallel, and Bundle export processes files in parallel ([Bundle compilation](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/lib.rs#L116-L120), [parallel child compilation](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/lib.rs#L177-L195), [parallel Bundle export](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/export.rs#L19-L40)).
- The CLI exports selected PNG/SVG pages in parallel and configures a process-global Rayon pool for `--jobs` ([parallel image export](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L461-L533), [CLI pool setup](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/world.rs#L40-L54)).

The Rust types permit separate threads to call `compile` when their `World` access is valid, but the public API has no per-compilation executor, concurrency limit, or fairness control. Service-level parallel compilations and Typst's internal Rayon work can therefore multiply runnable work unless the surrounding application controls both layers. Upstream also does not document isolation guarantees for concurrently compiling against the same mutable-cache `World`; that case needs validation rather than assumption.

## Diagnostics

`SourceDiagnostic` is an owned, cloneable value containing severity, a source span, a message, a trace, and hints. Fatal diagnostics are the `Err` side of `SourceResult`; warnings remain in the adjacent `Warned::warnings` vector ([diagnostic data model](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/diag.rs#L281-L329)). The diagnostic value has no borrow from the compilation `World`.

Human-readable rendering is a separate concern. `typst-kit::diagnostics::emit` accepts a `DiagnosticWorld`, and resolves diagnostic spans, line metadata, source text, and display names through that world while rendering ([diagnostic rendering API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/diagnostics.rs#L22-L47), [source lookups during rendering](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/diagnostics.rs#L148-L183)). The CLI renders diagnostics before its next watch reset ([CLI diagnostic handling](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L267-L305)).

An integration can retain structured diagnostics without retaining the world, but exact rich rendering requires access to the matching source snapshot and file-name mapping. Rendering after changing or discarding that snapshot can lose or mismatch source context even though the diagnostic itself remains valid as structured data.

## Timing

Compiler timing is instrumented with `typst_timing`. Its enabled flag and event vector are process-global statics; events carry thread IDs, and `export_json` drains all recorded events into Chrome trace JSON ([global timing state](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-timing/src/lib.rs#L46-L104), [event recording](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-timing/src/lib.rs#L152-L204)). Timing is not scoped to a compiler object or request.

`typst-kit::Timer` enables that global collector, clears it before a measured closure, runs the closure synchronously, then writes JSON and resolves spans through the still-available `World` ([timer helper](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/timer.rs#L16-L95), [timing span resolution](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/timer.rs#L98-L116)). The CLI labels `--timings` experimental ([CLI timing option](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/args.rs#L387-L393)).

Overlapping timed compilations can clear, combine, or drain the same global event storage. Upstream's timing collector therefore cannot provide request-isolated traces for concurrent compilations without external serialization or a different measurement layer. The one upstream streaming writer in this area is for timing JSON, not compilation artifacts.

## Incremental and watch behavior

### Reuse and invalidation

The public compiler has no explicit session or incremental-compilation handle. Reuse comes from Typst's memoized implementation together with stable tracked inputs supplied by the `World`; upstream's public guidance specifically calls out retaining and editing `Source` values for better incremental performance ([`World` caching guidance](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L45-L58), [tracked compile entry](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L74-L81)). Cache identity and invalidation are therefore affected by how a `World` preserves or replaces `Source`, `Bytes`, fonts, library configuration, and time values.

`FileStore::reset` marks loaded entries stale, clears the accessed dependency set, and later edits a retained `Source` in place when new bytes arrive. Its documentation identifies this as an incremental-performance optimization ([reset contract](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L81-L116), [stale source replacement](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L128-L159), [reload implementation](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/files.rs#L189-L236)). This is a reusable helper behavior, not a full watch protocol.

### CLI watch loop

The 0.15.0 CLI watch implementation:

1. Creates one `SystemWorld` and performs an initial compilation.
2. Watches dependencies accessed by the most recent compilation.
3. Waits for a relevant event.
4. Resets file and time state.
5. Recompiles synchronously.
6. Evicts older memoized work after the compilation.

This sequence is explicit in the watch loop and `SystemWorld::reset` ([watch loop](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/watch.rs#L59-L83), [world reset and dependencies](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/world.rs#L97-L107)). The watcher replaces its watched set with dependencies from the latest compilation, batches rapid events for 100 ms with a 500 ms starvation bound, and polls missing paths every 300 ms ([watcher behavior](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/watcher.rs#L35-L45), [dependency-set update](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/watcher.rs#L72-L118), [event batching](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-kit/src/watcher.rs#L120-L192)).

For Page Formats, the CLI additionally hashes each finished page and skips rewriting an unchanged image during watch mode if the destination still exists. This is an exporter cache layered after compilation, not partial compilation ([image export cache](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L640-L670), [cache use](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L496-L533)).

The CLI is useful precedent for dependency-driven invalidation, but it supplies no general watch session API, no change-set API, no cancellation of superseded work, and no provenance model beyond the latest accessed filesystem paths.

## Documents, artifacts, and lifetimes

### Compiled documents

`Output` is an owned compilation target with a target selector, constructor, and introspector; `compile` returns the selected `Output` by value ([`Output` trait](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/foundations/target.rs#L10-L30)). The concrete outputs contain owned data:

- `PagedDocument` contains finished pages, document metadata, and an `Arc`-backed introspector ([`PagedDocument`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-layout/src/document.rs#L15-L45)).
- `HtmlDocument` contains the HTML output, metadata, and an `Arc`-backed introspector. In 0.15.0, compiling this target requires the in-development `html` feature and emits a warning that behavior may change ([`HtmlDocument`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-html/src/dom.rs#L20-L73), [HTML feature warning](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L246-L265)).
- Upstream `Bundle` contains an `Arc`-backed map of document/assets and an introspector; it is a multi-file compilation target, not a typst-pack Pack archive ([upstream `Bundle`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/lib.rs#L37-L92)). Bundle compilation is explicitly experimental and feature-gated in 0.15.0 ([Bundle feature warning](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L268-L285)).

None of these return types carries a lifetime tied to `&dyn World`. The compiled document can outlive the world at the Rust type level. Exporters borrow the document only for the duration of export and return another owned value.

### Export matrix

| Format/target | Required compiled value | Export result | Granularity and materialization |
| --- | --- | --- | --- |
| PDF | `&PagedDocument` | `SourceResult<Vec<u8>>` | One complete byte vector for the document. Page ranges are exporter options, not partial compilation. ([PDF API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-pdf/src/lib.rs#L32-L53), [page-range option](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-pdf/src/lib.rs#L76-L81)) |
| HTML | `&HtmlDocument` | `SourceResult<String>` | One complete string backed by an internal output buffer. ([HTML API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-html/src/encode.rs#L15-L50)) |
| SVG | `&Page` | `String` | One complete string per page; a separate API merges a complete `PagedDocument` into one string. ([SVG APIs](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-svg/src/lib.rs#L30-L43), [merged SVG](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-svg/src/lib.rs#L125-L155)) |
| PNG | `&Page` | `tiny_skia::Pixmap`, then encoded bytes | One complete pixel buffer per page; the CLI then creates a complete PNG byte vector before writing. ([raster API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-render/src/lib.rs#L16-L48), [CLI PNG encoding](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L565-L590)) |
| Upstream Bundle | `&Bundle` | `SourceResult<VirtualFs>` | One complete `IndexMap<VirtualPath, Bytes>`; all files are collected before return. ([Bundle export API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/export.rs#L19-L40)) |

PDF and HTML need their full document targets. SVG and PNG can export pages independently, and the CLI demonstrates parallel per-page export, but only after `compile::<PagedDocument>` has produced the complete paged document ([CLI compile/export split](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L316-L375), [parallel page export](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-cli/src/compile.rs#L461-L533)).

## Streaming feasibility

“Streaming” has different answers at each boundary:

| Boundary | What upstream permits | What upstream does not provide |
| --- | --- | --- |
| Pack archive transport | A typst-pack adapter may receive, index, spool, or materialize its own archive by any mechanism before exposing files through `World`. | Typst has no Pack/archive input API and cannot consume an archive byte stream directly. Its only relevant boundary is synchronous `FileId -> Source/Bytes` lookup. ([`World`](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L59-L98)) |
| Individual input files | The world may load only files Typst requests, so whole files can be lazy and demand-driven. | A requested source or resource cannot be returned incrementally; the call must finish with a complete `Source` or `Bytes`. ([input methods](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-library/src/lib.rs#L69-L88)) |
| Compilation result | The owned document can be retained, queried, cloned according to its type, and exported later. | No public callback yields pages, DOM nodes, diagnostics, or other partial output while compilation is running. ([compile API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L63-L82), [compilation loop](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst/src/lib.rs#L116-L193)) |
| Document Format artifact | A materialized PDF byte vector or HTML string can be streamed onward by the caller after export. | PDF and HTML exporters do not accept `Write`/async sinks and do not yield output chunks; their complete artifact is allocated before return. ([PDF API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-pdf/src/lib.rs#L32-L53), [HTML API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-html/src/encode.rs#L22-L50)) |
| Page Format artifacts | After full paged compilation, a caller can export and forward one page artifact at a time, or export pages concurrently. This can avoid retaining every encoded artifact simultaneously. | The full `PagedDocument` remains required, and each SVG string, raster buffer, and PNG encoding is itself materialized. ([paged document](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-layout/src/document.rs#L15-L45), [SVG API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-svg/src/lib.rs#L30-L43), [raster API](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-render/src/lib.rs#L16-L48)) |
| Upstream Bundle output | The returned `VirtualFs` can be iterated and each already-materialized `Bytes` forwarded afterward. | Export collects the complete path-to-bytes map before return; there is no per-file sink callback. ([Bundle export](https://github.com/typst/typst/blob/3ae52774b48987fc78a72ff483068cacc28e46c2/crates/typst-bundle/src/export.rs#L19-L40)) |
| typst-pack Pack creation/writing | This is outside Typst's compiler/exporter interfaces and can be designed independently, subject to supplying any discovery compilation through `World`. | Upstream provides no Pack archive serializer or streaming guarantee. |

Thus “streaming compilation” is not available through upstream 0.15.x. Transport streaming, spooling, lazy whole-file lookup, and post-materialization artifact streaming are possible adapter behaviors, but they do not remove Typst's complete-file, complete-document, or complete-export-value boundaries.

## Implications for the redesign map

These are decision inputs, not decisions:

- An async typst-pack API must define where synchronous compilation runs and how an async Resource Provider crosses the synchronous `World` boundary; upstream does not resolve that seam.
- A cancellation contract must distinguish cancelling queued/outer work from stopping an active Typst call. The latter has no cooperative upstream hook.
- A concurrency budget must account for both independent compilations and internal Rayon work. `World` implementations must remain thread-safe even if typst-pack serializes top-level requests.
- Request-isolated timing cannot directly rely on the global upstream timing collector during concurrent work.
- Structured diagnostics can outlive compilation, but exact rich rendering needs the matching source/name snapshot.
- Watch behavior requires typst-pack-owned dependency identity, provenance, invalidation, supersession, and Resource Provider rules; the CLI's latest-filesystem-dependency loop does not define them.
- A streaming output contract must state whether it means transport streaming after materialization or bounded-memory generation. Upstream supports the former, not general bounded-memory artifact generation.
- Page Format artifacts can be emitted one at a time only after full paged compilation; PDF and HTML are one materialized artifact per export.
- Upstream Bundle and a typst-pack Pack are distinct concepts and must not share an ambiguous API name or streaming assumption.

## Unresolved facts

The upstream interfaces do not answer the following. They need benchmarks, prototypes, or an explicit typst-pack contract rather than further source interpretation:

- Peak memory and latency for representative Pack sizes, source files, page counts, fonts, and each exporter.
- Cache-hit behavior and retained memory across long-running compilations with typst-pack's eventual `World` identity and invalidation model.
- Throughput and oversubscription when multiple compilations run alongside Typst's internal Rayon work.
- Behavior and usefulness of compiling concurrently against one shared typst-pack `World` versus isolated worlds.
- Practical cancellation latency and resource reclamation for any chosen out-of-process isolation mechanism.
- Whether a given remote Resource Provider can prefetch all potentially requested values or must support blocking/retry behavior during dynamic dependency discovery.
- How much source data must be retained to guarantee delayed rich diagnostic rendering under typst-pack's eventual diagnostic schema.
- Whether future Typst releases add async I/O, cooperative cancellation, request-scoped timing, or streaming exporters; these findings must be rechecked as part of an engine upgrade.
