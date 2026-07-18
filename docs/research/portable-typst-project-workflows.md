# Portable Typst project workflows

Research date: 2026-07-18

Research ticket: [#30](https://github.com/sagikazarmark/typst-pack/issues/30) for wayfinder map [#27](https://github.com/sagikazarmark/typst-pack/issues/27)

## Question and method

Which concrete local CLI, reusable-library, web or WASM, remote-service, CI,
template, reproducible-build, and per-compilation customization workflows are
evidenced by primary sources, and what must a portable Typst project system
guarantee for them?

This survey uses only sources that own the behavior they describe:

- Typst's product documentation and compiler/CLI source;
- `typst-pack` documentation, source, and explicitly marked maintainer
  requirements in issues;
- source from an actual public `typst-pack` integration;
- source and documentation from the tools and services used in evidenced
  workflows; and
- public workflow definitions that actually compile Typst projects.

No personas are inferred. A workflow below is an observed operation or an
explicit maintainer requirement, not a claim about market size.

### Evidence grades

- **Strong**: shipped first-party product behavior, first-party source, or a
  concrete public workflow definition.
- **Moderate**: a real but early or single public integration, or an accepted
  maintainer requirement for software that is not yet the destination system.
- **Limited**: negative evidence, such as a dependency-index result, or a gap
  for which no shipped guarantee was found.

### Adoption evidence

The crates.io API reports [no published reverse dependencies for
`typst-pack`](https://crates.io/api/v1/crates/typst-pack/reverse_dependencies)
as of the research date. The concrete public integration found is
[`sagikazarmark/diotypst`](https://github.com/sagikazarmark/diotypst/tree/6afc56735103b3f525c95a5773f8bb99864895e6): its `typst-project` crate has an
optional, default-feature-free dependency on
[`typst-pack = 0.2.0`](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/Cargo.toml),
and its browser demo reads and writes `.typk` files. This is strong evidence
that the archive can be an interchange boundary and moderate evidence for the
specific browser workflow; it is not evidence of broad adoption.

## Workflow summary

| Workflow | Concrete journey | Required portable-project guarantee | Main scale or transport constraint | Evidence |
| --- | --- | --- | --- | --- |
| Local CLI | Discover and create a pack, inspect or extract it, then compile it locally or offline | Safe contained paths, stable entrypoint, complete declared closure, CLI-compatible controls and diagnostics | File and stdio transport; current reads and outputs ultimately materialize bytes in memory | Strong |
| Reusable Rust library | Build or read a pack in memory, construct a `World`, compile, and consume typed artifacts | Featureless in-memory core, enforced invariants, explicit packages/fonts/date/inputs, typed multi-artifact results | Typst's `World` loading is synchronous; current pack and artifact models own complete byte buffers | Strong, with one moderate public integration |
| Web/WASM | Import browser files, build/download/load a `.typk`, render offline, and re-render edited source | No host filesystem dependency, explicit virtual paths, preloaded packages/fonts, browser-safe archive code | Browser file APIs and the observed integration buffer whole files; package fetch must happen outside synchronous compilation | Moderate |
| Remote service | Resolve a pack and resources by opaque references, compile durably, persist outputs, return a small inventory | Reference-based I/O, async outer adapters, deterministic retries, structured diagnostics, bounded and classifiable provider failures | Pack/resource/artifact bytes must not cross request, response, or journal boundaries | Moderate maintainer requirement, supported by first-party service constraints |
| CI | Pin Typst, restore packages/fonts, compile on checkout, and upload PDFs or page outputs | Noninteractive CLI, explicit engine/environment controls, dependency visibility, deterministic artifact naming | Repository checkout, caches, downloads, and CI artifact transport are separate from compilation | Strong |
| Template | Publish a versioned package with a template directory, initialize a project by copying it, then edit and compile | Preserve `typst.toml`, template tree, package identity/version, and editable files beyond one representative render | Package download/cache plus directory copy; package authors can exclude large support files | Strong |
| Reproducible build | Recompile the same project with fixed date, packages, fonts, engine, inputs, and outputs | Record or constrain every observable input and reject ambient fallbacks when reproducibility is requested | A pack currently does not identify the compiler, and fonts are not embedded by default | Strong for individual controls; limited for an end-to-end byte guarantee |
| Per-compilation customization | Vary string inputs, selected resource bytes, date, output options, or trusted project files for one render | Distinct trust levels for values, non-source resources, and source overrides; discovery must cover conditional paths | Dynamic resources can be lazy but the loader is synchronous; source overrides change dependency closure | Strong for inputs/resource slots; moderate and contradictory for source overrides |

## 1. Local CLI workflows

**Evidence strength: strong.** Typst ships local `compile`, `watch`, and `init`
workflows. The [Typst README](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/README.md#usage)
documents one-shot compile, incremental watch, custom font paths, and local use.
The CLI source exposes the concrete input/output, root, string input, font,
package, timestamp, page, dependency, diagnostics, parallelism, and timing
controls in
[`CompileArgs` and `WorldArgs`](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst-cli/src/args.rs).

`typst-pack` adds a concrete project lifecycle: create a `.typk` from a named
source file, inspect it, compile it, and extract it. Its current README shows
the [CLI journey](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/README.md#cli)
and explains that creation runs a real compilation, captures files actually
read, accepts explicit conditional inclusions, vendors observed packages by
default, optionally embeds used fonts, and can verify package self-containment
with `--offline`.

### Guarantees this journey requires

- A pack must carry one normalized entrypoint and a contained project tree that
  cannot escape its virtual root. Current construction validates the
  entrypoint, project/resource disjointness, package declarations, font data,
  and path-tree conflicts in
  [`Pack::construct`](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/pack.rs#L119-L315).
- Creation must say what its closure means. Current `Packer` explicitly says
  that it observes one compilation and will miss files selected by different
  inputs or dates unless the author uses `include` or declares a Resource Slot
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/packer.rs#L33-L41)).
- Package and font authority must be inspectable. Current behavior vendors the
  whole tree of each observed package, while font embedding is opt-in and only
  selects fonts used by rendered output
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/packer.rs#L564-L616)).
- Multi-output identity must survive transport. PDF and HTML yield one artifact;
  PNG and SVG can yield one artifact per selected source page. The current
  typed artifact stores its format, bytes, and optional original source-page
  number
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/compile.rs#L162-L210)).
- Automation needs dependencies and diagnostics, not only bytes. Upstream Typst
  exposes dependency output in JSON, NUL-delimited, or Make form plus timing
  output; accepted maintainer requirements demand Pack-aware provenance rather
  than fictional archive-member paths
  ([issue #20](https://github.com/sagikazarmark/typst-pack/issues/20)).

### Constraints and contradictions

- `Pack::read` requires `Read + Seek`, then reads every retained project,
  package, and font entry to a complete in-memory buffer
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/pack.rs#L376-L568)).
  The CLI can read stdin, but does so by buffering it before `Pack::from_bytes`.
  The single-file archive is convenient transport, not a streaming or bounded-
  memory guarantee.
- Upstream watch updates the watcher from the most recent compilation's
  dependencies, resets the world, and recompiles
  ([source](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst-cli/src/watch.rs)).
  In contrast, accepted `typst-pack` requirements explicitly defer watch until
  provider and host dependency provenance can invalidate correctly
  ([issue #20](https://github.com/sagikazarmark/typst-pack/issues/20)). A portable
  watch command therefore needs stable dependency identities for the pack,
  mutable overrides/resources, package sources, and fonts; merely watching the
  `.typk` file is insufficient.
- Extraction creates an editable project, while discovery captures a
  representative compilation closure. Editing an extracted file can make a
  previously unobserved conditional file necessary. "Compiles as packed" and
  "complete editable source distribution" are different guarantees.

## 2. Reusable-library workflow

**Evidence strength: strong for the APIs, moderate for adoption.** The current
featureless API can build a `Pack` from in-memory files, serialize it, parse it
elsewhere, construct a `PackWorld`, and compile to typed artifacts
([documented example](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/README.md#library)).
Filesystem packing, CLI behavior, and embedded fonts are opt-in features; the
crate's [`Cargo.toml`](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/Cargo.toml#L18-L46)
keeps the base feature set empty.

This shape follows Typst's embedding seam. `typst::compile` accepts a borrowed
`dyn World` and returns a paged or HTML document plus diagnostics
([API](https://docs.rs/typst/0.15.0/typst/fn.compile.html)). `World` synchronously
supplies the library, font book, entrypoint, source bytes, raw file bytes,
fonts, and date, and its documentation puts cache ownership on the integrator
([API](https://docs.rs/typst/0.15.0/typst/trait.World.html)).

The public `diotypst` integration translates `.typk` bytes into its own explicit
project, package-bundle, font, and render-environment model and back
([implementation](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/src/pack.rs)).
That is direct evidence for a reusable interchange format rather than only a
CLI file.

### Guarantees this journey requires

- Pack invariants must be established at every construction/read boundary, so
  callers cannot create a world whose entrypoint or declared content is
  inconsistent.
- The core must accept and return owned in-memory data without requiring paths,
  environment variables, a network client, Tokio, or a process-global cache.
- Compiler inputs must be explicit: packages, ordered fonts, `sys.inputs`,
  language features, and date. Current `PackWorldBuilder` has seams for each
  and makes package fallback optional
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/world.rs#L217-L392)).
- Warnings, errors, output cardinality, format, and source-page identity must be
  typed independently of terminal rendering.
- Format/version errors must be explicit. A byte archive is an external
  protocol once another crate parses it.

### Constraints and contradictions

- Current pack parsing owns the archive bytes and expands every relevant entry;
  current compilation owns each output as `Vec<u8>`. This is a coherent small-
  to-medium in-memory API but provides no bounded-memory behavior for large
  packs or many raster pages.
- Typst's source/file loading seam is synchronous. An asynchronous object store
  cannot be placed directly behind it without prefetching, blocking, or a
  bridge owned by an adapter.
- The public integration is pinned to `typst-pack 0.2.0` and its conversion
  model does not represent the later Resource Slot model. Meanwhile the
  current `typst-pack` README says format version 1 is explicitly unstable and
  old field aliases are rejected
  ([format notes](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/README.md#pack-format)).
  Clean breaks are allowed by map #27, but a migration/version-negotiation
  story is now a concrete integration concern rather than theoretical
  compatibility work.

## 3. Web and WASM workflow

**Evidence strength: moderate.** Typst's official web app establishes the
product workflow: a project has one or more Typst, text, image, and font files,
supports collaborative live preview, and can select any Typst file as the
preview/export entrypoint
([project concepts](https://typst.app/docs/web-app/concepts/),
[preview and export](https://typst.app/docs/web-app/export-and-preview/)). Git
sync transports unpacked project files between the web app and GitHub or GitLab
for users who also work locally
([documentation](https://typst.app/docs/web-app/git-sync/)). These docs prove the
workflow but do not disclose the web app's compiler transport or WASM
architecture.

The public `diotypst` source supplies the concrete WASM evidence:

- its `pack` feature is explicitly default-feature-free and WASM-safe
  ([crate manifest](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/Cargo.toml));
- browser file or directory selection becomes an explicit project tree and
  font set before render
  ([demo source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/demo/src/examples/import.rs));
- a browser can build and download a `.typk`, read one back, install its
  vendored packages and fonts, and render without a package source or network
  ([demo source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/demo/src/examples/pack.rs)); and
- editor changes drive debounced re-rendering while the last good artifact can
  remain visible on a failed compile
  ([demo source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/demo/src/examples/editor.rs)).

### Guarantees this journey requires

- Project identity must be virtual: one explicit root entrypoint plus normalized
  root-relative files, with no ambient host filesystem.
- Archive parsing, project construction, package bundle parsing, font loading,
  compilation, and chosen exporters must compile without native filesystem or
  system downloader features.
- Network package acquisition must be an asynchronous preparation phase that
  yields in-memory bundles before synchronous Typst world lookup. The observed
  integration's `SandboxedWorld` has no implicit filesystem or network read
  and resolves files from explicit workspace/package data
  ([source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/src/world.rs)).
- Diagnostics and artifacts must be usable as application state, not only
  printed to stderr or written to a host path.
- A project must round-trip to a transport a browser can upload/download, while
  Git remains a valid transport for the unpacked editable tree.

### Constraints and contradictions

- The observed browser path calls `read_bytes`, `ProjectPack::from_bytes`, and
  `to_bytes`; it buffers the selected archive and generated archive. No primary
  evidence establishes browser memory, CPU, page-count, cancellation, or
  latency limits.
- Closed-world offline rendering favors prefetching all packages and fonts,
  while live editing favors incremental mutable source caches. These are
  compatible only if immutable pack content and compilation/session state have
  separate lifetimes.
- The same integration implements `WorldOverlay`, which can replace source
  files and the main entrypoint for one render
  ([source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/src/world.rs)).
  That is concrete demand for broader trusted customization than current
  Resource Slots, but it is not evidence that every pack consumer should allow
  source replacement.

## 4. Remote-service workflow

**Evidence strength: moderate maintainer requirement.** The Restate service PRD
in [issue #12](https://github.com/sagikazarmark/typst-pack/issues/12) is explicit
requirements evidence, not destination scope. Its concrete journey is:

1. receive opaque references for a pack, ordered Resource Providers, and an
   output destination plus small compile options;
2. load the pack asynchronously;
3. compile on a blocking worker while lazily resolving declared resources;
4. persist every output asynchronously under deterministic names; and
5. return only structured diagnostics or a small persisted-artifact inventory.

The requirement that bytes stay out of Restate messages and journals matches
first-party Restate constraints: durable execution records operations and
results in a journal
([concepts](https://docs.restate.dev/concepts/durable_execution)), and Restate
documents failures for messages over `worker.invoker.message_size_limit` and
for oversized service-provider payloads
([RT0003 and RT0019](https://docs.restate.dev/references/errors#rt0003)).

### Guarantees this journey requires

- Pack, resource, and output references must be opaque to the core and typed by
  role; storage schemes and credentials belong to adapters.
- Not-found, temporary, permanent, and deterministic compiler failures must
  remain distinguishable. Only not-found advances an ordered resource fallback
  chain; only temporary infrastructure errors should be retried.
- Large bytes must stay inside the compile/persist operation. A successful
  response means all reported artifacts were persisted and includes stable
  names, media types, byte lengths, and source-page identities.
- Retry convergence requires a fixed timestamp, deterministic artifact names,
  and overwrite/idempotent persistence. Partial multi-object output is not a
  transaction; the returned inventory is the completion record.
- Core compilation must remain independent of Restate, Tokio, OpenDAL, and any
  server. The PRD explicitly places those dependencies in a separate service
  repository.

### Constraints and contradictions

- Current `FileLoader` and Typst `World` reads are synchronous, while pack and
  object I/O should be asynchronous. The PRD's blocking-worker bridge is one
  adapter design, not a reason to make the in-memory core async.
- Current `Pack::read` is seek-based and expands entries, and exporters return
  complete byte buffers. Remote reference transport avoids message amplification
  but does not by itself make compilation streaming or bounded-memory.
- The PRD explicitly leaves compilation concurrency, archive/resource/page/
  output byte limits, hard timeouts, and force-cancellation out of scope. Map
  #27 asks for scale and performance decisions, so those remain unanswered.
- The service requires fully vendored packages, embedded/default fonts, no
  system fonts, and no Pack Overrides. Those are valid constraints for that
  service's trust and retry boundary, not evidence that the portable core must
  ban unvendored packages, host fonts, or overrides everywhere.

## 5. CI workflows

**Evidence strength: strong.** The first-party-to-the-tool
[`setup-typst` action](https://github.com/typst-community/setup-typst/blob/bb238cfa1f00bd6711328d2d149e9b224c0c176e/README.md)
installs a selected Typst version, caches package dependencies, supports local
ZIP packages, and documents compiling then uploading an artifact.

Concrete public workflows show three common variants:

- OI-wiki pins Typst 0.13.1, caches declared package imports, installs explicit
  fonts, compiles, and uploads the PDF
  ([workflow](https://github.com/OI-wiki/OI-wiki/blob/1e04537ab249066c5de28231df13a49b61857429/.github/workflows/build-pdf-typst.yml)).
- `better-thesis` compiles a template in pull requests with explicit project
  root and font directory
  ([workflow](https://github.com/sysu/better-thesis/blob/205dc285c1355424650288b716cc95229b55ffdd/.github/workflows/check.yml)).
- `hei-synd-thesis` uses a reusable matrix workflow to compile language/type
  variants through `--input` and uploads each distinctly named PDF
  ([workflow](https://github.com/hei-templates/hei-synd-thesis/blob/c377a1fa93eaf683f183529cf05f2be6c953c57c/.github/workflows/_reusable-build.yml)).

`typst-pack` itself exposes Dagger functions that mount project, package, font,
and ordered resource directories, return a pack as a `File`, and compile to a
`Directory` for uniform one-or-many artifact transport
([module source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/dagger.dang#L124-L224),
[compile function](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/dagger.dang#L346-L501)).

### Guarantees this journey requires

- Compiler version selection must be explicit and visible in logs; a cache hit
  must not silently select another engine.
- Projects need deterministic root/entrypoint and output naming, including
  matrix variants and page-per-file formats.
- Package and font acquisition must be cacheable but separable from authority:
  a cache accelerates a build, while a pack or lock-like declaration determines
  what may satisfy it.
- Machine-readable diagnostics and exact dependency reporting are needed for
  checks, cache invalidation, and warnings-as-policy.
- Container/build-system adapters should map first-class files and directories
  to core byte/project interfaces rather than put container handles in the
  library.

### Constraints and contradictions

- CI examples commonly pin Typst and fonts but do not set
  `SOURCE_DATE_EPOCH`, disable all system fonts, or vendor every package. They
  prove repeatable automation, not byte-for-byte reproducibility.
- Setup, cache, compile, and artifact upload are distinct transports. A `.typk`
  can reduce compile-time network and environment dependencies, but it does
  not replace CI artifact retention or package/template publication.
- Dagger returns a directory even for one document so its type handles all
  formats. The CLI naturally returns a file for PDF/HTML and a filename
  template for page formats. The core must preserve artifact cardinality and
  identity without forcing either adapter's transport shape on the other.

## 6. Template workflows

**Evidence strength: strong.** Typst packages are versioned collections of
Typst files and assets with a root `typst.toml`; full versions are required for
published imports and packages download on demand into a cache
([official package repository](https://github.com/typst/packages/blob/dec229b6a887127d37c804c0cab7e2289266e947/README.md)).

A template package adds a directory and entrypoint. `typst init` obtains the
package, validates its manifest, and copies that template directory into a new
editable project
([CLI source](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst-cli/src/init.rs)).
The official manifest documentation recommends keeping reusable styling in the
package library while the copied template entrypoint imports and configures it;
it also requires the template path and entrypoint and documents the local
init-then-compile test workflow
([manifest documentation](https://github.com/typst/packages/blob/dec229b6a887127d37c804c0cab7e2289266e947/docs/manifest.md#templates)).

The web app creates projects from Universe or private templates, and private
template directories are copied as-is. Its documentation warns that relative
imports to files outside the copied directory will break
([private templates](https://typst.app/docs/web-app/private-packages/#private-templates)).

### Guarantees this journey requires

- `typst.toml` is project/package metadata even when compilation does not read
  it. Current `Packer` deliberately includes a root manifest next to the
  entrypoint
  ([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/packer.rs#L505-L516)).
- Package identity is namespace, name, and exact version; template identity
  additionally includes a copied directory and its relative entrypoint.
- A template distribution must preserve files intended for later editing, not
  just files read by its initial preview.
- Reusable package code and copied project files must retain separate
  authorities: updating the document project is not the same as mutating its
  imported package version.

### Constraints and contradictions

- Package manifests can exclude large support files from published bundles,
  and template thumbnails have an explicit 3 MiB limit. A portable project
  must not assume the source repository tree equals the downloaded package
  tree.
- Representative compile discovery is narrower than template distribution.
  The initialized project is expected to be edited, so branches, examples, or
  assets unused by the initial template compile can become necessary later.
  Packaging a reusable template therefore needs explicit whole-template
  inclusion or a declared variant matrix, not closure observation alone.
- A pack and a template package have different jobs. A package template
  scaffolds an editable project and retains an imported reusable library; a
  pack transports a chosen project's compilation contract. Treating either as
  an alias for the other loses required semantics.

## 7. Reproducible-build workflow

**Evidence strength: strong for controls, limited for a full guarantee.** Typst
documents that `datetime.today()` uses the current date and can be overridden
by `--creation-timestamp` or `SOURCE_DATE_EPOCH`
([documentation](https://typst.app/docs/reference/foundations/datetime/#definitions-today)).
The CLI applies an explicit timestamp to PDF metadata, but otherwise uses the
current local time and timezone
([source](https://github.com/typst/typst/blob/89495172a5b6a697cbf2d6c560efb2da9dfd173b/crates/typst-cli/src/compile.rs)).

`typst-pack` provides relevant controls: package vendoring and offline mode,
used-font embedding, system-font exclusion, fixed discovery/compile dates,
explicit inputs, and an ordered contained project. Its archive writer emits
manifest, project, package, and font entries in ordered collections
([source](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/pack.rs#L576-L623)).
The documented manifest records project, package, font, and descriptive
metadata but no Typst compiler/exporter version
([schema](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/README.md#pack-format)).

### Required guarantees by input dimension

| Dimension | Required guarantee | Current evidence or gap |
| --- | --- | --- |
| Engine | Exact compiler/exporter compatibility is known and enforced or reported | The crate pins exact Typst crates, but the Pack Manifest does not record the compiler version |
| Project | Entrypoint and every eligible project byte are fixed and path-safe | Strong current invariant; representative discovery still needs explicit conditional coverage |
| Packages | Exact specs and bytes are contained, or resolution is explicitly allowed and reported | Vendoring is default; unvendored packages intentionally reintroduce local/cache/network state |
| Fonts | Every selected font face and precedence rule is fixed | Embedding is opt-in; Typst embedded fonts and host fonts can remain external |
| Date/time | Document date and PDF metadata use one fixed instant/policy | Supported through explicit timestamp; defaults are ambient |
| Values/resources | `sys.inputs`, Resource Slot bytes, and trusted overrides are fixed for the run and included in its identity | APIs accept them, but no current compilation fingerprint binds them together |
| Export | Format, pages, PPI, PDF standards/tags, pretty mode, and exporter version are fixed | Typed/CLI controls exist; engine version remains external |
| Serialization | Archive semantics and reader compatibility are versioned | Format version 1 is explicitly unstable and has changed in place |

### What can and cannot be claimed

- A self-contained pack compiled with a fixed engine, fixed timestamp, no
  ambient package access, and a fixed font policy has the well-evidenced input
  controls needed for a reproducible workflow. No reviewed source states an
  unconditional byte-for-byte guarantee.
- `SOURCE_DATE_EPOCH` alone is not reproducibility. It controls date-sensitive
  document behavior and PDF metadata, not compiler, package, font, source,
  input, resource, or exporter identity.
- `--offline` alone is not self-containment: local package directories can
  still satisfy unvendored packages, and fonts can still come from the host.
- A current pack alone is not a cross-version byte-for-byte promise because it
  does not identify the Typst engine/exporter and its own format is unstable.
  No primary source reviewed makes that stronger guarantee.

The map therefore needs to define reproducibility levels rather than one
boolean. At minimum, distinguish "no network", "contained dependencies",
"environment-independent compile", and "same-engine byte-reproducible output".

## 8. Per-compilation customization workflows

**Evidence strength: strong for values and non-source bytes; moderate and
contradictory for source replacement.** Four concrete layers are evidenced.

### String values

Typst's `--input key=value` exposes strings through `sys.inputs`; more complex
values can be encoded and parsed by document code
([official documentation](https://typst.app/docs/reference/foundations/sys/)).
The public browser demo changes a `sys.inputs` value and re-renders after a
debounce
([source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/demo/src/examples/sys_inputs.rs)).
This is the lowest-trust customization layer because it cannot directly supply
source or bytes; the document decides how strings affect behavior.

### Declared non-source bytes

Typst documents can read text or raw bytes by project-relative path
([`read`](https://typst.app/docs/reference/data-loading/read/)). Current
Resource Slots declare exact project-relative non-source paths whose bytes are
provided lazily for a compilation. Contained files win, only declared missing
paths reach providers, source requests are blocked, providers are ordered, and
only not-found falls through
([implementation](https://github.com/sagikazarmark/typst-pack/blob/caeb89003affb39a56e63c89a1c19c3d1fc0c353/src/resource.rs#L19-L69)).

This supports one reusable invoice/report pack with per-run images or data
without allowing the provider to expand the code graph. Required guarantees
are exact path identity, lazy access, missing-only fallback, typed provider
failure, and an explicit distinction between unfilled and invalid requested
slots.

### Compile and export options

Page selection, PNG PPI, PDF standards and tags, HTML feature enablement,
pretty output, date, jobs, diagnostics, and format are per-compilation values in
the Typst and `typst-pack` APIs. They can alter output cardinality and bytes
without changing project files. They belong in the compilation request and
result identity, not in immutable Pack content unless a product deliberately
stores defaults.

### Trusted project-file replacement

Map #27 presents Pack Overrides as a leading hypothesis: a compilation-scoped
replacement for any contained project file, potentially including source and
the entrypoint. The current glossary limits the term to contained project files
and excludes packages/fonts, but the current pack implementation does not
provide it. The public `diotypst` integration independently implements exact
project-file and main-entrypoint overlays
([source](https://github.com/sagikazarmark/diotypst/blob/6afc56735103b3f525c95a5773f8bb99864895e6/crates/typst-project/src/world.rs)).

Evidence conflicts by boundary:

- the browser/editor integration permits source and entrypoint replacement;
- Resource Slots deliberately prohibit source and contained-file replacement
  ([issue #20](https://github.com/sagikazarmark/typst-pack/issues/20)); and
- the Restate service PRD deliberately excludes Pack Overrides
  ([issue #12](https://github.com/sagikazarmark/typst-pack/issues/12)).

This contradiction should be resolved as capability policy, not by weakening
Resource Slots. Values, non-source resources, and trusted source overrides have
different authority and dependency consequences. A consumer must be able to
support only the lower-trust layers.

### Conditional-closure constraint

Inputs, date, target, and source overrides can change which files Typst reads.
Current discovery unions paged and HTML targets when requested, but it uses one
input dictionary and one date. Therefore a portable system must choose and make
visible one of these guarantees:

- discovery covers an explicit matrix of representative compilations;
- authors explicitly include every conditional project file and declare every
  conditional Resource Slot; or
- a compilation may report that the requested variant is outside the contained
  contract.

Silently treating one successful discovery compile as proof for every later
customization is contradicted by the current `Packer` documentation.

## Cross-workflow contradictions

1. **Closure versus editability.** An observed compilation closure is suitable
   for replaying represented variants. Templates, extracted projects, editors,
   and source overrides can require files outside it.
2. **Portability versus ambient fallback.** Unvendored packages, system fonts,
   local dates, and host paths are useful local conveniences but weaken
   environment-independent output. The API must expose the selected policy.
3. **Synchronous compilation versus asynchronous transport.** Typst's `World`
   is synchronous; browser and object-store acquisition are asynchronous.
   Preparation/prefetch or an adapter-owned blocking bridge is required.
4. **Whole-buffer APIs versus large values.** Current archives and artifacts
   materialize bytes. Remote workflows require references and may benefit from
   storage streaming, but there is no evidence that Typst compilation/export is
   end-to-end streaming. Limits and buffering must be explicit.
5. **Resource Slots versus source overlays.** Resource Slots are constrained
   non-source inputs; an actual public integration has trusted source and
   entrypoint overlays; one service explicitly bans them. These should remain
   separate capabilities.
6. **Unstable format versus real interchange.** A public integration already
   pins an older `typst-pack` and translates the archive into its own model.
   Clean breaks remain possible, but version negotiation and migration are
   destination requirements.
7. **Watch versus opaque providers.** Correct invalidation requires dependency
   identities and change notification. An opaque provider that only answers
   synchronous reads is enough to compile, not enough to watch.
8. **Reproducibility label versus engine provenance.** Current controls can
   close source/package/font/date inputs, but the pack does not record the
   compiler/exporter version. Reproducibility needs graded, testable language.
9. **Single document versus artifact collection.** PDF/HTML and PNG/SVG have
   different cardinality. CLI paths, Dagger directories, browser downloads,
   and object-store prefixes are adapter transports over the same typed
   artifact inventory.

## Guarantees the destination should make decision-complete

The evidence supports requiring the wayfinder destination to decide, rather
than assume, all of the following:

1. **Content contract:** whether a Pack is only a represented compilation
   closure, a complete editable project, or one explicit mode of each; how
   multi-input/date/target discovery and explicit inclusions are recorded.
2. **Authority contract:** precedence and eligibility for contained project
   files, Resource Slots, trusted Pack Overrides, vendored/unvendored packages,
   pack/default/host fonts, and clocks.
3. **Compatibility contract:** Pack format negotiation/migration and how exact
   Typst compiler/exporter compatibility is recorded or checked.
4. **Reproducibility contract:** named levels with testable requirements for
   network, dependency containment, ambient environment, engine, and output
   bytes.
5. **I/O contract:** a featureless in-memory core plus explicit native,
   browser, CI/container, and remote-reference adapters; sync/async boundaries
   must follow Typst's synchronous `World` rather than hide blocking.
6. **Scale contract:** maximum archive entries and expanded bytes, individual
   resource size, total fonts/packages, page count, artifact bytes, memory,
   concurrency, duration, and cancellation semantics. No reviewed source
   provides a sufficient default.
7. **Output contract:** typed format/cardinality/source-page identity,
   deterministic naming at adapters, diagnostics/warnings, and persisted or
   returned inventory semantics.
8. **Incremental contract:** dependency identities, cache ownership,
   invalidation, provider change notification, and stale-artifact behavior for
   watch/editor sessions.

The Restate PRD is one demanding consumer of these guarantees. Its reference
transport, retries, and persistence policy should validate the library seams,
but Restate, OpenDAL, its exact request schema, and its narrower no-override/no-
unvendored-package policy are not the destination scope.
