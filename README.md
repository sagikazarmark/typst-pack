# typst-pack

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/typst-pack/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/typst-pack/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/typst-pack/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/typst-pack)
[![crates.io](https://img.shields.io/crates/v/typst-pack?style=flat-square)](https://crates.io/crates/typst-pack)
[![docs.rs](https://img.shields.io/docsrs/typst-pack?style=flat-square)](https://docs.rs/typst-pack)

**Portable single-file packs of Typst projects: sources, resources, packages, and fonts.**

A *pack* (`.typk`) captures the compilation contract of one Typst project:

- the packed project files: the entrypoint, other Typst sources, images, and
  data files,
- optionally Resource Slot paths whose bytes are supplied when
  requested instead of being stored in the reusable pack,
- optionally the files of the [Typst Universe](https://typst.app/universe)
  packages the project imports, so compiling needs no network access,
- optionally the fonts the document uses, so compiling produces identical
  output on machines without those fonts.

Use it as a CLI to distribute finished Typst projects, or as a library to
produce and consume packs programmatically (e.g. offering a "download
project" pack in a web-based Typst editor).

Note: this is unrelated to Typst's own *bundle export* (the `typst-bundle`
crate), which is a multi-file **output** target. A pack is an **input**
archive: a portable form of a project's sources and resources.

## Features

- **Portable project archives**: bundle Typst sources, resources, packages, and
  fonts into one `.typk` file.
- **Automatic discovery**: observe the files a real Typst compilation reads and
  include additional conditional files explicitly.
- **Reproducible compilation**: compile without network or system font access,
  with support for fixed timestamps and vendored packages.
- **Resource Slots**: keep declared non-source project bytes outside a
  reusable pack and supply them for each compilation.
- **Library and CLI APIs**: create, inspect, compile, and extract packs in memory
  or on the file system.

## CLI

Install the command-line tool with its opt-in `cli` feature:

```console
cargo install typst-pack --features cli
```

```console
# Pack a named source file, vendoring all observed packages:
typst-pack create path/to/project/main.typ

# Pack a specific entrypoint, embedding the fonts the document uses:
typst-pack create letter.typ --embed-fonts

# See what a pack contains:
typst-pack inspect project.typk

# Compile a pack without network access:
typst-pack compile project.typk output.pdf

# Discover a resource outside the source project, then supply it when compiling:
typst-pack create invoice/main.typ invoice.typk --resource-path representative-branding/
typst-pack compile invoice.typk customer.pdf --resource-path customer-branding/

# PNG or SVG output, page selection, reproducible builds:
typst-pack compile project.typk "page-{0p}.png" --ppi 300 --pages 1-3
typst-pack compile project.typk reproducible.pdf --creation-timestamp 1700000000

# Guarantee no network access (fails instead of downloading packages):
typst-pack compile project.typk --offline

# Experimental HTML export (the output format enables its required feature):
typst-pack compile project.typk out.html

# HTML discovery still selects the feature explicitly:
typst-pack create project/main.typ --target html --features html

# Unpack a pack back into an editable project directory:
typst-pack extract project.typk -o project/
```

For Page Formats, `{p}` expands to the one-based Source Page Number, `{0p}` and
`{n}` are zero-padded aliases, and `{t}` is the total source-document page
count before page selection. Multi-page output requires an explicit `{p}`,
`{0p}`, or `{n}` template. All target paths are checked for duplicates before
writing. Document Format output paths are literal.

### How files are discovered

`create` runs a *discovery compile* of the project and records every file Typst
actually reads. Select paged or HTML discovery with repeatable,
comma-delimited `--target`; paged is the default.
Sources, images, data files, and package files are picked up automatically,
including files accessed dynamically. Because discovery observes one
concrete compile, files that would only be read under different
`--input` values or on a different date are not seen; add those with
`--include <path>` (a file or directory inside the project root).

A `typst.toml` next to the entrypoint (Typst's own package/template
manifest, not to be confused with the pack's `typst-pack.toml`) is always
packed as a regular project file, even though compiles don't read it. That
way template and package metadata survives the round trip through
`create` and `extract`.

### Resource Slots

A **Resource Slot** is one exact normalized project-relative location whose
non-source bytes are supplied to a compilation instead of stored in the Pack.
For example, one invoice Pack can declare `assets/logo.png` while a Resource
Provider supplies different valid bytes for each compilation.

The filesystem CLI registers ordered Resource Providers with repeated
`--resource-path <DIR>`. During creation, the source project is checked first;
only a missing resource falls through to providers in command-line order. A
successful provider load is inferred as a Resource Slot and serialized under
`[project].resource-slots` without storing its representative bytes.

Use `create --resource-slot <PATH>` for an unexercised slot or for a present
representative file whose bytes must be omitted. An unrequested slot may remain
unfilled. If discovery requests an unavailable explicit slot, creation explains
that representative bytes can be placed in the source project or supplied with
`--resource-path`; those bytes are not stored in the Pack.

During Pack compilation, providers are consulted only for declared Resource
Slots. Contained project files remain authoritative, and providers cannot
supply Typst source, packages, undeclared paths, or replacements. Only a missing
provider result falls through; success and non-missing errors stop resolution.
Inspection and extraction list Resource Slots, but extraction does not create
empty files for them.

### Packages

All observed package dependencies are vendored into the pack by default.
With `--no-vendor-packages`, each dependency is instead recorded as an exact
package specification and Complete Package Tree identity. Compilation acquires
the whole tree from the configured package directory, cache, or Typst Universe,
verifies it before invoking Typst, and exposes only the verified paths and bytes.
Undeclared package locations and ambient caches cannot satisfy imports.

`--offline` (on both `create` and `compile`) disables the download step
entirely: dependencies must come from the pack or the local package
directories, and anything else fails as not found. Use
`typst-pack compile --offline` to verify that a pack is truly
self-contained.

### Fonts

Every selected face is recorded in the ordered Pack Font Catalog with its exact
container identity. Fonts are *not* embedded by default: compilation must find
the declared exact containers among the configured system, Typst-embedded, or
`--font-path` sources. Other available fonts are not exposed to Typst.

With `--embed-fonts`, selected containers are stored in the pack, except those
identical to Typst's embedded fonts. Pass `--include-typst-embedded-fonts` to
store those too. Mind font licenses when redistributing embedded containers;
licensing and acquisition metadata do not change font selection.

### Output formats

PDF and HTML are Document Formats and produce one Compilation Output Artifact
without a Source Page Number. PNG and SVG are Page Formats and produce one
artifact per selected source page. Page artifacts retain their original Source
Page Number and are emitted once each in source-document order.

HTML export is experimental in Typst itself, and Typst emits a warning that its
behavior may change. Pack compilation derives the required engine feature from
`CompilationOutputSpecification::Html`; HTML discovery still requires
`--features html` (or `TYPST_FEATURES=html`).

The Dagger `compile` function returns a directory for every format. Document
Formats use `output.pdf` or `output.html`; Page Formats use deterministic names
such as `page-2.png`, derived from Source Page Numbers. Its typed mapping,
staging, failure boundary, and intentional transport omissions are documented
in the [Dagger adapter contract](docs/dagger-adapter.md).

Maintainers changing the embedded compiler must follow the
[embedded Typst upgrade procedure](docs/embedded-typst-upgrade.md). CI enforces
the approved crate graph, classified differential matrix, official CLI oracle,
and the packaged release binary.

## Library

Add the crate with filesystem-backed packing support and Typst's embedded
fonts:

```toml
[dependencies]
typst-pack = { version = "0.4", features = ["embedded-fonts", "fs"] }
```

The core in-memory packing and compilation APIs require no crate features.

```rust,ignore
use typst_pack::{
    compile, CompilationOutputSpecification, OutputFormat, Pack,
    PackCompilationRequest, Packer, PdfOutputSpecification,
};

// Pack a project directory (requires the `fs` feature).
let outcome = Packer::new("path/to/project", "main.typ")
    .embed_fonts(true)
    .pack()?;
let bytes = outcome.pack.to_bytes()?;

// ... ship the bytes somewhere, then compile without a file system:
let pack = Pack::from_bytes(bytes)?;
let request = PackCompilationRequest::new(
    pack,
    CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
);
let output = compile(request)?;
assert_eq!(output.engine_identity().implementation(), "typst");
assert_eq!(output.exporter_identity().implementation(), "typst-pdf");
let artifact = output.artifacts().first().expect("PDF artifact");
assert_eq!(artifact.format(), OutputFormat::Pdf);
assert_eq!(artifact.source_page_number(), None);
let pdf = artifact.bytes();
```

Use `compile_report` when operational dependency evidence is needed alongside
the immutable semantic result. Its fulfillment report retains caller-supplied
package and font provenance, cache disposition, and licensing metadata without
including those operational values in Compilation Identity or Compilation
Result Identity. Every result also exposes its document summary and canonical
Compilation Access Trace.

For PNG and SVG, `source_page_number()` identifies each artifact independently
of its collection position. `bytes()` borrows the artifact bytes and
`into_bytes()` extracts them without cloning.

Packs can also be assembled fully in memory, with no file system involved, which
is what a web editor wants:

```rust,ignore
use typst_pack::Pack;

let pack = Pack::builder("main.typ")
    .file("main.typ", source_text.as_bytes().to_vec())?
    .file("figure.png", image_bytes)?
    .build()?;
let bytes = pack.to_bytes()?;
```

Packs issued by `Packer` additionally persist each successful Discovery
Variant's safe request commitments and canonical Discovery Trace. This evidence
survives archive round trips and contributes to Pack Identity. Manually assembled
Packs carry no discovery claim.

Resource Slots can also be declared and supplied in memory. A Resource Provider
uses typst-kit's standard synchronous `FileLoader` interface, so callers can
adapt memory, object storage, or prefetched application data:

```rust,ignore
let pack = Pack::builder("main.typ")
    .file("main.typ", source_text.as_bytes().to_vec())?
    .resource_slot("assets/logo.png")?
    .build()?;
let request = PackCompilationRequest::new(
    pack,
    CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
);
let controls = CompilationExecutionControls::default()
    .resource_provider(resource_provider);
let output = compile(CompilationAttempt::new(request, controls))?;
```

For filesystem discovery, add one or more `Packer::resource_provider`
implementations. Registering a provider enables successful missing-resource
loads to be inferred as Resource Slots; no separate fallback policy is needed.

### Compilation authority

The public compilation boundary accepts only a validated `Pack` bound into a
`PackCompilationRequest`. The Pack-backed Typst `World`, compilation kernel,
and embedded compiler and exporter adapter are private. In particular, callers
cannot substitute a `typst::World`, language library, compiler, or exporter:

```compile_fail
use typst_pack::PackWorld;
```

```compile_fail
use typst_pack::compile_pack;
```

```compile_fail
use typst_pack::compile;

fn arbitrary_world(world: &dyn typst::World) {
    let _ = compile(world);
}
```

Typst 0.15.0 owns language evaluation, layout, official diagnostics, document
structures, and PDF, PNG, SVG, and HTML export behavior. typst-pack owns Pack
creation and validity, the fixed project namespace, exact package and font
verification, Resource Slots, Pack Overrides, request identities and reports,
and later CLI or Dagger publication. Artifact bytes and official diagnostics
are not reinterpreted by destination, transport, cache, or presentation code.

Intentional differences from `typst compile` are Pack confinement, Pack input
instead of a source root, declared Resource Slots, exact dependency
fulfillment, Pack Overrides, unsupported Bundle output, and publication rules
for immutable artifacts. The complete version-bound inventory is in
[`docs/cli-parity.md`](docs/cli-parity.md).

### Migrating to 0.4

Version 0.4 makes clean naming and invariant-boundary breaks without retaining
compatibility aliases:

- Replace Resource APIs using `external_resource` or
  `external_resource_reference` with `resource_slot` and `resource_provider`.
- Replace CLI `--source-reference <DIR>` with `--resource-path <DIR>` and
  `--external-resource <PATH>` with `--resource-slot <PATH>`.
- Rename Dagger arguments: `source` -> `project`, `entrypoint` -> `input`,
  `inputs` -> `sysInputs`, `noPackages` -> `noVendorPackages`,
  `sourceReferenceDir`/`sourceReferences` -> `resourceDirs`,
  `externalResources` -> `resourceSlots`, and `sourceDateEpoch` ->
  `creationTimestamp`.
- Change creation from a directory plus `--entrypoint`/`--output` to
  `create <INPUT> [OUTPUT]`.
- Replace `compile_pack(request)` with `compile(request)`. The provisional
  arbitrary-`World` `compile` overload and public `PackWorld` builder are
  removed; configure semantic values on `PackCompilationRequest` and operational
  values on `CompilationExecutionControls`.
- Pack Manifest fields and `PackFont` fields are read-only. Use accessors such
  as `manifest.project()`, `project.entrypoint()`, `font.manifest()`, and
  `font.data()`.
- Shared Pack consistency failures are available as `PackInvariantError`,
  wrapped by `PackBuildError::Invariant` or `PackReadError::Invariant`.
- Replace `OutputFormat` plus `CompileOptions` request construction with the
  corresponding `CompilationOutputSpecification` variant and format-specific
  structure. PDF creation time is configured through
  `PdfOutputSpecification::creation_timestamp`; use `CreationTimestamp::Omit`
  to suppress PDF creation datetime metadata.
- `ExtractError` adds `PlannedPathConflict` and `DestinationConflict`; exhaustive
  matches must handle both variants.

The unstable Pack format remains version 1, but its fields change in place to
`project.resource-slots` and `packages.unvendored`; old field aliases are not
accepted. Resource Providers remain compilation inputs rather than serialized
Pack content.

### Feature flags

- `fs`: `Packer`, `extract`, package download and caching,
  system font scanning. Requires a file system, so disable this (and `cli`)
  for wasm targets.
- `cli`: the `typst-pack` binary.
- `embedded-fonts`: make Typst's bundled fonts available as intentional
  discovery and external-fulfillment sources.

All crate features are opt-in.

## Pack format

A pack is a Zip archive (Deflate), conventionally named `*.typk`, with this
layout:

```text
typst-pack.toml                     manifest (always first)
project/<path>                      project files, root-relative
packages/<ns>/<name>/<version>/<path>   vendored package files
fonts/<file>                        embedded font files
```

The manifest looks like this:

```toml
format-version = 1

[project]
entrypoint = "main.typ"
resource-slots = ["assets/logo.png"]

[packages]
vendored = ["@preview/cetz:0.3.4"]
unvendored = ["@preview/tablex:0.0.9"]

[[fonts]]
path = "fonts/ibm-plex-sans.ttf"
families = ["IBM Plex Sans"]

[metadata]
name = "Quarterly report"
authors = ["Jane Doe"]
```

Readers ignore unknown top-level archive entries and reject manifests with a
`format-version` greater than they support. Paths inside the archive are
validated, root-relative virtual paths, so a pack can never read or write
outside its own tree.

`resource-slots` is an ordered, deduplicated allowlist. Its entries have no
corresponding `project/` archive entry, and readers reject a path that is both
packed and declared as a Resource Slot. The format version remains 1 and is
explicitly unstable: readers reject the old `external-resources` and
`packages.external` version-1 fields rather than retaining aliases.

## Development

Minimum verification:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`

Run CI's containerized checks with [Dagger](https://dagger.io):

- `dagger check`

The containerized suite includes the
[embedded Typst CLI differential gate](docs/cli-parity.md), pinned to the exact
official release used by the library.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
