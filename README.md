# typst-pack

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/typst-pack/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/typst-pack/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/typst-pack/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/typst-pack)
[![crates.io](https://img.shields.io/crates/v/typst-pack?style=flat-square)](https://crates.io/crates/typst-pack)
[![docs.rs](https://img.shields.io/docsrs/typst-pack?style=flat-square)](https://docs.rs/typst-pack)

**Portable single-file packs of [Typst](https://typst.app) projects.**

A *pack* (`.typk`) captures the compilation contract of one Typst project:

- the packed project files: the entrypoint, other Typst sources, images, and
  data files,
- optionally External Project Resource paths whose bytes are supplied when
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

## CLI

```console
# Pack a project directory (entrypoint main.typ), vendoring all packages:
typst-pack create path/to/project

# Pack a specific entrypoint, embedding the fonts the document uses:
typst-pack create letter.typ --embed-fonts

# See what a pack contains:
typst-pack inspect project.typk

# Compile a pack without network access:
typst-pack compile project.typk output.pdf

# Discover a resource outside the source project, then supply it when compiling:
typst-pack create invoice/ --resource-path representative-branding/ -o invoice.typk
typst-pack compile invoice.typk customer.pdf --resource-path customer-branding/

# PNG or SVG output, page selection, reproducible builds:
typst-pack compile project.typk "page-{0p}.png" --ppi 300 --pages 1-3
typst-pack compile project.typk --creation-timestamp 1700000000

# Guarantee no network access (fails instead of downloading packages):
typst-pack compile project.typk --offline

# Experimental HTML export, gated like in the Typst CLI:
typst-pack compile project.typk out.html --features html

# Unpack a pack back into an editable project directory:
typst-pack extract project.typk -o project/
```

### How files are discovered

`create` runs a *discovery compile* of the project and records every file
Typst actually reads, the same mechanism `typst compile --make-deps` uses.
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

### External Project Resources

An **External Project Resource** is a non-source resource addressed through
Typst's project root whose path belongs to a pack's compilation contract, but
whose bytes are supplied externally instead of being stored in the reusable
pack. This lets one invoice pack use the stable path `assets/logo.png` while a
different resource root supplies that path for each customer.

Discovery is strict by default: every requested project resource must exist in
the source project and is packed. Each `--resource-path <DIR>` enables external
resource discovery and acts as another virtual project root. The source project
is checked first, then resource roots are checked in command-line order. A file
loaded from a resource root is recorded under `[project].external-resources`
and its bytes are omitted from the archive.

Use `--external-resource <PATH>` during creation when a representative file is
present in the source project but should be omitted, or when a conditional
resource is not read by the representative discovery compile. Explicitly
declared paths are recorded even when discovery does not request them.

At compilation time, repeat `--resource-path` to provide the declared paths.
Packed project files always win, and undeclared missing paths never fall
through to these roots. External Project Resource loaders cannot supply Typst
imports/includes or package files. Loading is lazy: a requested resource that
is unavailable produces Typst's normal file diagnostic.

Typst 0.15 cannot check whether a path exists, so this version does not provide
optional-resource detection. Document code that requests an External Project
Resource must be compiled with that resource available. Inspection shows these
paths, but extraction does not create files for them.

### Packages

All observed package dependencies are vendored into the pack by default.
With `--no-packages`, they are instead recorded as *external* dependencies in
the manifest; compiling such a pack resolves them from the local package
directories or downloads them from Typst Universe, like the Typst CLI would.

`--offline` (on both `create` and `compile`) disables the download step
entirely: dependencies must come from the pack or the local package
directories, and anything else fails as not found. Use
`typst-pack compile --offline` to verify that a pack is truly
self-contained.

### Fonts

Fonts are *not* embedded by default. With `--embed-fonts`, the fonts used by
the rendered document are stored in the pack, except fonts identical to
Typst's embedded defaults (Libertinus Serif, New Computer Modern, Deja Vu
Sans Mono), which every consumer of this crate already has; pass
`--include-default-fonts` to store those too. Mind font licenses when
redistributing packs with embedded fonts.

When compiling a pack, fonts are used in this order of preference:
`--font-path` fonts, pack fonts, embedded default fonts, and system fonts
(disable the latter with `--ignore-system-fonts`).

### Output formats

`compile` produces PDF (default), PNG, and SVG. HTML export is also
supported but experimental in Typst itself; like the Typst CLI, it must be
enabled explicitly with `--features html` (or `TYPST_FEATURES=html`), and
Typst emits a warning that its behavior may change. In the library, enable
it with `PackWorldBuilder::feature(typst::Feature::Html)` and compile with
`OutputFormat::Html`.

## Library

```rust,ignore
use typst_pack::{compile, CompileOptions, OutputFormat, Pack, PackWorld, Packer};

// Pack a project directory (requires the `fs` feature).
let outcome = Packer::new("path/to/project", "main.typ")
    .embed_fonts(true)
    .pack()?;
let bytes = outcome.pack.to_bytes()?;

// ... ship the bytes somewhere, then compile without a file system:
let pack = Pack::from_bytes(bytes)?;
let world = PackWorld::builder(pack).build()?;
let pdf = compile(&world, OutputFormat::Pdf, &CompileOptions::default())?
    .outputs
    .remove(0);
```

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

External Project Resources can also be declared and supplied in memory. The
loader uses typst-kit's standard synchronous `FileLoader` interface, so callers
can adapt memory, object storage, or prefetched application data:

```rust,ignore
let pack = Pack::builder("main.typ")
    .file("main.typ", source_text.as_bytes().to_vec())?
    .external_resource("assets/logo.png")?
    .build()?;
let world = PackWorld::builder(pack)
    .external_resource_loader(resource_loader)
    .build()?;
```

For filesystem discovery, configure
`ProjectResourcePolicy::AllowExternalProjectResources` and add one or more
`Packer::external_resource_loader` implementations. Source-project resources
remain authoritative; successful fallback loads are inferred as External
Project Resources.

### Feature flags

- `fs` *(default)*: [`Packer`], [`extract`], package download and caching,
  system font scanning. Requires a file system, so disable this (and `cli`)
  for wasm targets.
- `cli` *(default)*: the `typst-pack` binary.
- `embedded-fonts` *(default)*: compile packs against Typst's default fonts.

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
external-resources = ["assets/logo.png"]

[packages]
vendored = ["@preview/cetz:0.3.4"]
external = ["@preview/tablex:0.0.9"]

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

`external-resources` is an ordered, deduplicated allowlist. Its entries have no
corresponding `project/` archive entry, and readers reject a path that is both
packed and declared external. Existing format-version-1 manifests that omit the
field remain valid and are read as having no External Project Resources.

The format version remains 1. Released older readers reject manifests that use
`external-resources` because they reject unknown fields; this intentional
compatibility break does not affect old packs read by newer versions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
