# typst-pack

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/typst-pack/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/typst-pack/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/typst-pack/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/typst-pack)
[![crates.io](https://img.shields.io/crates/v/typst-pack?style=flat-square)](https://crates.io/crates/typst-pack)
[![docs.rs](https://img.shields.io/docsrs/typst-pack?style=flat-square)](https://docs.rs/typst-pack)

**Portable single-file packs of Typst projects: sources, resources, packages, and fonts.**

A *pack* (`.typk`) captures the compilation contract of one Typst project:

- the packed project files: the entrypoint, other Typst sources, images, and
  data files,
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
- **Structural project closure**: include every eligible regular file beneath
  the selected project root, independently of compiler control flow.
- **Reproducible compilation**: compile without network or system font access,
  with support for fixed timestamps and vendored packages.
- **Pack Overrides**: replace any contained project file for one compilation
  without mutating the Pack.
- **Library and CLI interfaces**: create, inspect, compile, and extract packs in
  memory or on the file system.

## CLI

Install the command-line tool:

```console
cargo install typst-pack-cli
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

# Replace a contained placeholder for one compilation:
typst-pack compile invoice.typk customer.pdf --override assets/logo.png customer-logo.png

# PNG or SVG output, page selection, reproducible builds:
typst-pack compile project.typk "page-{0p}.png" --ppi 300 --pages 1-3
typst-pack compile project.typk reproducible.pdf --creation-timestamp 1700000000

# Guarantee no network access (fails instead of downloading packages):
typst-pack compile project.typk --offline

# Experimental HTML export (the output format enables its required feature):
typst-pack compile project.typk out.html

# An HTML representative creation compile still selects the feature explicitly:
typst-pack create project/main.typ --target html --features html

# Unpack a pack back into an editable project directory:
typst-pack extract project.typk -o project/
```

For Page Formats, `{p}` expands to the one-based Source Page Number, `{0p}` and
`{n}` are zero-padded aliases, and `{t}` is the total source-document page
count before page selection. Multi-page output requires an explicit `{p}`,
`{0p}`, or `{n}` template. All target paths are checked for duplicates before
writing. Document Format output paths are literal.

### Project files

`create` stabilizes every eligible regular file beneath the physical project
root before compiling. Project membership is independent of the representative
compile's target, inputs, date, features, and control flow. The root
`.typkignore` applies Gitignore-style ordered rules; it is always packed, nested
`.typkignore` files are ordinary project files, and every `.typk` path is always
excluded. Symlinks and other unignored non-regular entries are rejected.

The `.typk` exclusion is a canonical path invariant rather than a walker rule,
so it binds every route into a pack, including direct in-memory construction and
reading one back. A pack built in memory before that invariant existed, holding
a project path with a `.typk` segment, no longer reads.

Creation runs one representative compile from those stabilized bytes to select
exact package and font dependencies. `--target paged|html` is optional and
defaults to `paged`; it does not restrict later output formats. This concrete
evaluation is a temporary dependency-selection mechanism because Typst does not
report every package or font a different request might reach.

Every project path in a Pack has contained bytes. For per-document variation,
pack a valid placeholder and use compile-time `--override PACK_PATH FILE`.
Overrides may replace source, assets, data, or the entrypoint, but cannot add or
delete paths or authorize undeclared packages and fonts.

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

With `--embed-fonts`, selected containers are stored in the pack, except the
ones Typst itself ships. Pass `--include-typst-embedded-fonts` to store those
too. Embedding follows where a container came from, not what its bytes are: a
`--font-path` directory holding a copy of one of Typst's containers is embedded
like any other scanned container. Mind font licenses when redistributing
embedded containers; licensing and acquisition metadata do not change font
selection.

The Candidate Font Catalog creation selects from is one explicit ordered
sequence: `CandidateFontCatalog` holds `CandidateFontContainer`s, each carrying
its own embedded-or-external `FontDisposition`, so one pack can embed a
redistributable container and reference a restrictively licensed one. Faces are
expanded in container-local index order, catalog order decides which container
wins a family, and nothing joins a catalog implicitly:
`typst_embedded_font_containers` yields Typst's own containers for a caller to
splice in where it wants. `Packer` composes its catalog from system fonts,
Typst's embedded fonts, and `--font-path` directories, in that order.

### Output formats

PDF and HTML are Document Formats and produce one Compilation Output Artifact
without a Source Page Number. PNG and SVG are Page Formats and produce one
artifact per selected source page. Page artifacts retain their original Source
Page Number and are emitted once each in source-document order.

HTML export is experimental in Typst itself, and Typst emits a warning that its
behavior may change. Pack compilation derives the required engine feature from
`CompilationOutputSpecification::Html`; HTML creation still requires
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

Filesystem access and network egress are separately selectable, so the build
above reads projects, local package directories, and system fonts from disk with
no download capability compiled in at all: nothing in its dependency graph can
reach the network. Add the `egress` feature to let filesystem creation download
the packages a project imports.

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
let report = compile(request)?;
let output = report.result().expect("semantic compilation result");
assert_eq!(output.engine_identity().implementation(), "typst");
assert_eq!(output.exporter_identity().implementation(), "typst-pdf");
let artifact = output.artifacts().first().expect("PDF artifact");
assert_eq!(artifact.format(), OutputFormat::Pdf);
assert_eq!(artifact.source_page_number(), None);
let pdf = artifact.bytes();
```

Large immutable project, package, font, and compilation artifact payloads are
shared across semantic clones and projections. Their public accessors expose
borrowed byte slices without exposing the private sharing representation. Pack
Archive bytes are the exception: `Pack::to_bytes()` returns the distinct,
non-cloneable `PackArchiveBytes` value so exact retry material has explicit
unique ownership.

`PackOutcome::warnings` retains warnings from the representative creation
compile. Inspect `PackOutcome::pack` for authoritative project files, package
requirements and their embedding disposition, and the Pack Font Catalog; that
static inventory is not duplicated in the creation outcome.

`compile` always returns a `CompilationReport` after accepting the semantic
request. Its outcome contains either the immutable semantic result or an
operational dependency failure, and its fulfillment report retains
caller-supplied package and font provenance, cache disposition, and licensing
metadata without including those operational values in Compilation Identity or
Compilation Result Identity. Request rejection is the outer error and retains
the complete request inventory. Every semantic result also exposes its document
summary and canonical Compilation Access Trace.

For PNG and SVG, `source_page_number()` identifies each artifact independently
of its collection position. `bytes()` borrows the artifact bytes and
`into_bytes()` extracts an owned vector, reusing its buffer when the artifact is
uniquely owned and materializing a copy when another semantic clone shares it.

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

Building a pack by hand gives up the representative compile that discovers
dependencies. `create` keeps it and still needs no crate feature: it takes the
bytes a caller already holds, runs one representative Typst request, and issues
the pack it selected. It acquires nothing itself and consults no wall clock, so
the creation timestamp fixing that request's Document Time is required. Project
Snapshot assembly accepts entries already selected by the caller; source-specific
ignore policy and resource limits belong to the gatherer that obtains them:

```rust,ignore
use typst_pack::{
    create, CandidateFontCatalog, CandidateFontContainer, CreationRequest,
    ProjectSnapshotAssembly, ResolvedPackageTree,
};

let project = ProjectSnapshotAssembly::new("main.typ").assemble([
    ("main.typ", source_text.as_bytes().to_vec()),
    ("figure.png", image_bytes),
])?;

let request = CreationRequest::new(project, creation_timestamp)
    .font_catalog(CandidateFontCatalog::from_iter([
        CandidateFontContainer::embedded(font_bytes),
    ]))
    .package_tree(ResolvedPackageTree::embedded(spec, package_files));
let issued = create(&request)?.into_issued().expect("no package is missing");
let bytes = issued.pack.to_bytes()?;
```

Compiler observations select package and font requirements; project files come
from the Project Snapshot alone. Each supplied package tree and font container
carries its own embedded-or-external disposition, which is what the pack's
Package Requirements and Font Requirements record. `IssuedPack::warnings`
retains the representative compile's warnings, and a representative request that
does not compile fails creation instead of issuing an incomplete pack. The
request is an owned value the core retains nothing of, so it can be run again.
Obtaining its inputs is Creation Preparation, which belongs to the caller;
`Packer` is the reference filesystem Creation Adapter, implemented over
`create` like any other adapter.

Establishing that the acquired bytes represent one consistent source state is
the adapter's own obligation, and it is advisory: creation holds owned bytes and
has nothing to re-read, so an adapter acquiring from mutable storage without
revalidating still conforms, and may issue a pack describing a source state that
never existed simultaneously. `Packer` discharges it: it revalidates the
project, the package trees it acquired, and the font catalog before returning
the pack, and fails creation when any of them changed while it ran.

How far the adapter's own acquisition reaches is a build-time choice. With `fs`
alone it resolves reported specifications from the local package directories and
whichever package cache the host has, and fails creation with
`PackerError::Package` when a specification is in neither; with `egress` it
downloads the rest from the Typst Universe registry unless creation is offline.
`Packer::certificate` and `Packer::package_cache_path`
configure a download, so they need `egress`; `Packer::offline` does not, because
a build without egress already resolves that way and code that sets it keeps
compiling either way.

Package requirements can only be discovered by compiling, so creation resolves
package acquisition through a resumable protocol rather than a callback. A
request that read a package no supplied tree covers reports that exact
specification instead of issuing a pack, which is a normal outcome and not a
failure. The caller resolves it however its host allows and invokes creation
again with the same request values and the tree added:

```rust,ignore
use typst_pack::{create, CreationOutcome, CreationRequest, ResolvedPackageTree};

let mut resolved: Vec<ResolvedPackageTree> = Vec::new();
let issued = loop {
    let request = CreationRequest::new(project.clone(), creation_timestamp)
        .package_trees(resolved.iter().cloned());
    match create(&request)? {
        CreationOutcome::Issued(issued) => break issued,
        // Acquire each reported specification however this host allows: from a
        // cache, over an asynchronous transport, or in a later request.
        CreationOutcome::MissingPackages(missing) => {
            for spec in missing {
                resolved.push(acquire_tree(&spec)?);
            }
        }
    }
};
```

Reported specifications come from the package file requests the compiler made,
never from diagnostic text, and always carry an exact version, because a Typst
import specification always does. Because a failed import ends module
evaluation, one round reports what that round reached, and a project needing
several packages completes over repeated invocation. Nothing is retained
between invocations, so a resume step is valid across a host request boundary
and nothing in the core is `async`. A tree that does not declare the
specification it was supplied under is the distinct
`CreationError::MismatchedPackageTree` failure, so a loop that would otherwise
never progress gets a diagnosis instead.

A specification the caller cannot resolve ends the loop through
`CreationRequest::unresolvable_package`, which takes the caller's own
`PackageError`. Creation stops reporting that specification and fails the next
representative request at the import that needed it, carrying that error as its
diagnostic. This is how an acquisition failure keeps a source location: the
specifications creation reports name a package, never the file that imported
one, so only a failed representative request can point at the import. `Packer`
drives it, which is why an unresolvable package still fails with
`PackerError::Compile` and a spanned diagnostic rather than beside the source
that asked for it.

Resolving a reported specification against the Typst Universe registry needs
the registry layout and the archive encoding, not an HTTP client. The
`package-acquisition` feature supplies both halves, so the loop above keeps its
own transport — including one whose only network access is asynchronous:

```rust,ignore
use typst_pack::{
    expand_package_archive, package_archive_url, PackageDisposition, PackageExpansionCeiling,
};

let url = package_archive_url(&spec)?;
// Fetch the archive with whatever primitive this host provides.
let archive = fetch(&url)?;
let tree = expand_package_archive(
    spec,
    &archive,
    PackageDisposition::Embedded,
    // Required, so that the bound is always a deliberate choice.
    PackageExpansionCeiling { max_bytes: 32 * 1024 * 1024 },
)?;
```

Expansion stops at the ceiling and fails with
`PackageAcquisitionError::ExpansionCeilingExceeded` rather than materializing
what lies past it, so a caller-named package cannot exhaust the process. Every
archive member is charged against the ceiling, but only addressable regular
files become tree entries, and a member whose path cannot name a package file is
rejected. A specification in a namespace the registry does not serve has no URL
there and is reported as such.

Compilation-time Pack Overrides replace contained project-file bytes in memory:

```rust,ignore
let pack = Pack::builder("main.typ")
    .file("main.typ", source_text.as_bytes().to_vec())?
    .file("assets/logo.png", placeholder_png)?
    .build()?;
let overrides = PackOverrideSet::new(&pack)
    .replace("assets/logo.png", customer_png)?;
let request = PackCompilationRequest::new(
    pack,
    CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
).overrides(overrides);
let report = compile(request)?;
let output = report.result().expect("semantic compilation result");
```

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

Destinations remain adapter facts rather than semantic compilation request
values:

```compile_fail
use typst_pack::PackCompilationRequest;

fn add_destination(request: PackCompilationRequest) {
    let _ = request.destination("output.pdf");
}
```

Typst 0.15.1 owns language evaluation, layout, official diagnostics, document
structures, and PDF, PNG, SVG, and HTML export behavior. typst-pack owns Pack
creation and validity, the fixed set of contained project paths, exact package
and font verification, Pack Overrides, request identities and reports, and later
CLI or Dagger publication. Artifact bytes and official diagnostics are not
reinterpreted by destination, transport, cache, or presentation code.

Intentional differences from `typst compile` are Pack confinement, Pack input
instead of a source root, a fixed contained project namespace, exact dependency
fulfillment, Pack Overrides, unsupported Bundle output, and publication rules
for immutable artifacts. The complete version-bound inventory is in
[`docs/cli-parity.md`](docs/cli-parity.md).

### Migrating to 0.4

Version 0.4 makes clean naming and invariant-boundary breaks without retaining
compatibility aliases:

- Remove Resource Slot and Resource Provider APIs; pack valid baseline
  placeholders and replace contained files with Pack Overrides.
- Rename Dagger arguments: `source` -> `project`, `entrypoint` -> `input`,
  `inputs` -> `sysInputs`, `noPackages` -> `noVendorPackages`,
  `sourceDateEpoch` -> `creationTimestamp`, and `CreationTarget` ->
  `TypstTarget`. Removed resource and inclusion arguments have no replacements.
- Change creation from a directory plus `--entrypoint`/`--output` to
  `create <INPUT> [OUTPUT]`.
- Replace `compile_pack(request)` with `compile(request)`. The provisional
  arbitrary-`World` `compile` overload and public `PackWorld` builder are
  removed; configure semantic values on `PackCompilationRequest`.
- `compile` returns `CompilationReport`; inspect `report.outcome()` or
  `report.result()`. `compile_report`, `PackCompileError`, `CompilationAttempt`,
  and the empty `CompilationExecutionControls` are removed. Request rejection
  now owns its inventory and ordered `CompilationRequestIssue` values.
- Replace `CreationTarget` and `CompilationTarget` with `TypstTarget`.
- Configure document time with one `DocumentTime` value. `Absent`, `Fixed`, and
  `UnixTimestamp` replace the former date/timestamp fields and setters.
- Read representative-compile warnings from `PackOutcome::warnings`; the
  one-field `PackReport` is removed.
- Pack Manifest fields and `PackFont` fields are read-only. Use accessors such
  as `manifest.project()`, `project.entrypoint()`, `font.manifest()`, and
  `font.data()`. Package declarations are reached only through
  `manifest.packages().vendored()` and `.unvendored()`.
- Shared Pack consistency failures are available as `PackInvariantError`,
  wrapped by `PackBuildError::Invariant` or `PackReadError::Invariant`.
- Replace `OutputFormat` plus `CompileOptions` request construction with the
  corresponding `CompilationOutputSpecification` variant and format-specific
  structure. PDF creation time is configured through
  `PdfOutputSpecification::creation_timestamp`; use `CreationTimestamp::Omit`
  to suppress PDF creation datetime metadata.
- `ExtractError` adds `PlannedPathConflict` and `DestinationConflict`; exhaustive
  matches must handle both variants.

The unstable Pack format remains version 1, but discovery and Resource Slot
fields are removed in place. Old fields and aliases are not accepted.

### Feature flags

- `fs`: `Packer`, `extract`, local package directories and cache, system font
  scanning. Requires a file system, so disable this for wasm targets. It links
  no transport, so a build with this feature alone cannot download under any
  runtime configuration.
- `egress`: package downloading during filesystem creation, with the custom
  certificate and package cache directory only a download needs. Implies `fs`,
  builds on `package-acquisition`, and is what links an HTTP client and TLS
  roots, so the absence of network egress is verifiable from the dependency
  graph rather than from a runtime flag.
- `package-acquisition`: registry URL construction and bounded package archive
  expansion, for a caller that supplies its own transport. Pulls compression and
  archive support but no HTTP client.
- `embedded-fonts`: make Typst's bundled fonts available as intentional
  creation and external-fulfillment sources.
- `diagnostics`: retain source context for first-party diagnostic presentation
  adapters.
- `parallel`: export independent page artifacts in parallel.

All library crate features are opt-in. Pack Creation over supplied inputs
(`create`) and fixed timestamp conversion for `DocumentTime` are part of the
featureless core and remain available on wasm targets.

## Pack format

A pack is a Zip archive, conventionally named `*.typk`, with this semantic
layout. The encoder uses Deflate and emits the manifest first; readers also
accept interoperable ZIP encodings and member orderings that preserve the same
layout:

```text
typst-pack.toml                     manifest
project/<path>                      project files, root-relative
packages/<ns>/<name>/<version>/<path>   vendored package files
fonts/<file>                        embedded font files
```

The manifest looks like this:

```toml
format-version = 1

[project]
entrypoint = "main.typ"

[[packages.vendored]]
spec = "@preview/cetz:0.3.4"
tree-digest = "0123456789abcdef0123456789abcdef"
tree-identity-kind = "complete-package-tree"
tree-identity-schema = "typst-pack-complete-package-tree-v1"
tree-identity-algorithm = "typst-hash128-0.15"
file-count = 12
byte-length = 34567

[[packages.unvendored]]
spec = "@preview/tablex:0.0.9"
tree-digest = "fedcba9876543210fedcba9876543210"
tree-identity-kind = "complete-package-tree"
tree-identity-schema = "typst-pack-complete-package-tree-v1"
tree-identity-algorithm = "typst-hash128-0.15"
file-count = 8
byte-length = 23456

[[fonts]]
path = "fonts/ibm-plex-sans.ttf"
families = ["IBM Plex Sans"]

[metadata]
name = "Quarterly report"
authors = ["Jane Doe"]
```

Readers ignore safe unknown regular-file and directory entries and reject
manifests whose `format-version` is not the exact supported version. Raw member
names must be well-formed for their declared ZIP encoding, member paths must be
safe root-relative virtual paths, and non-directory members must be regular
files. Unknown entries are not Pack semantics and may disappear after decoding
and re-encoding. Pack Archive compatibility is semantic; exact ZIP bytes,
compression details, timestamps, and member ordering are not preserved.

Extraction rejects existing symlinked entries within the selected destination
before writing.

The format version remains 1 and is explicitly unstable: readers reject old
discovery, Resource Slot, `external-resources`, and `packages.external` fields
rather than retaining aliases.

## Development

Minimum verification:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`

Run CI's containerized checks with [Dagger](https://dagger.io):

- `dagger check`

The containerized suite includes the
[embedded Typst CLI differential gate](docs/cli-parity.md), pinned to the exact
official release used by the library. It also verifies the build configurations
the crate features promise: the featureless core for `wasm32-unknown-unknown`,
and a filesystem build whose resolved dependency graph contains no HTTP client.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
