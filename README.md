# typst-pack

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/typst-pack/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/typst-pack/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/typst-pack/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/typst-pack)
[![crates.io](https://img.shields.io/crates/v/typst-pack?style=flat-square)](https://crates.io/crates/typst-pack)
[![docs.rs](https://img.shields.io/docsrs/typst-pack?style=flat-square)](https://docs.rs/typst-pack)

**Bundle a Typst project and its fonts and packages into one file that can
compile on another machine.**

A pack (`.typk`) contains an entrypoint, the project's source and data files,
and the exact package and font requirements found during a representative
compile. Packages and fonts can be embedded for offline, portable compilation
or recorded as external requirements for the receiving application to supply.

Use the CLI to create, inspect, compile, and extract packs. Use the Rust library
to build the same workflows in editors, web services, object-storage systems,
and other applications.

This is unrelated to Typst's bundle output (`typst-bundle`). A pack is portable
**input** for later compilation, not a collection of rendered output files.

## Features

- Put a whole Typst project in one `.typk` file, including images and data.
- Vendor imported Typst Universe packages so compilation works offline.
- Embed selected fonts so output does not depend on fonts installed elsewhere.
- Compile to PDF, PNG, SVG, or experimental HTML without reading ambient project files.
- Replace a contained project file for one compile without changing the pack.
- Inspect or extract a pack before using it.
- Build packs from the filesystem, entirely in memory, or with caller-supplied OpenDAL storage.
- Keep filesystem access and network download support as separate build choices.

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

# Experimental HTML export:
typst-pack compile project.typk out.html
typst-pack create project/main.typ --target html --features html

# Unpack a pack back into an editable project directory:
typst-pack extract project.typk -o project/
```

For PNG and SVG output, `{p}` is the one-based source page number, `{0p}` and
`{n}` are zero-padded aliases, and `{t}` is the source-document page count.
Multi-page output needs a page placeholder. All output paths are checked for
duplicates before anything is written.

See [CLI examples](docs/cli-examples.md) for additional create, compile,
extraction, and Pack Override commands.

### Project files

`create` includes every eligible regular file beneath the project root, not
only files reached by the representative compile. A root `.typkignore` uses
Gitignore-style ordered rules. It is included in the pack; nested
`.typkignore` files are ordinary files. Symlinks, unsupported entries, and any
path containing a `.typk` component are rejected.

The representative compile selects package and font requirements. Its target,
inputs, date, features, and control flow do not change which project files are
included. Pack a valid placeholder when a document needs per-recipient data,
then replace it with `--override PACK_PATH FILE` at compile time.

### Packages and fonts

Observed packages are embedded by default. `--no-vendor-packages` records each
exact package and complete tree identity instead; compilation then requires the
application's configured package sources to provide a matching tree. `--offline`
prevents downloads during both creation and compilation.

Fonts are external by default. `--embed-fonts` stores selected font containers
except those shipped by Typst; `--include-typst-embedded-fonts` stores those as
well. Mind font licenses when redistributing embedded files.

### Output formats

PDF and HTML produce one artifact. PNG and SVG produce one artifact for each
selected source page. HTML is experimental in Typst. The complete intentional
differences from `typst compile` are listed in the
[CLI parity inventory](docs/cli-parity.md).

## Library

The core in-memory packing, creation, and compilation APIs need no crate
features. Add filesystem support when the library should read local projects,
package directories, and system fonts:

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["embedded-fonts", "fs"] }
```

The `fs` feature links no network client. Add `egress` only when filesystem
assembly should download missing Typst Universe packages.

The [library contract](docs/library-contract.md) describes identity,
dependency fulfillment, environment independence, write policies, retry
material, and partial effects. The [OpenDAL guide](crates/typst-pack/docs/opendal-integration.md)
documents asynchronous storage integration.

### Common workflows

Each entry links to a compile-checked example on docs.rs, so the code shown
there is verified against the release you are reading about.

| Task | Start here |
| --- | --- |
| Pack a project directory from disk | [`FilesystemPackAssembler`](https://docs.rs/typst-pack/latest/typst_pack/struct.FilesystemPackAssembler.html) |
| Pack in-memory bytes with no filesystem | [`Pack::builder`](https://docs.rs/typst-pack/latest/typst_pack/struct.Pack.html#method.builder) |
| Supply packages yourself and resume creation | [`create`](https://docs.rs/typst-pack/latest/typst_pack/fn.create.html) |
| Swap a contained file for one compile | [`PackOverrideSet`](https://docs.rs/typst-pack/latest/typst_pack/struct.PackOverrideSet.html) |
| Read and write packs through object storage | [OpenDAL guide](crates/typst-pack/docs/opendal-integration.md) |

The shortest complete example — build a pack in memory and encode it:

```rust
use typst_pack::Pack;
use typst_pack::pack_archive::encode;

let pack = Pack::builder("main.typ")
    .file("main.typ", b"= Report\n".to_vec())
    .expect("main.typ is a valid project path")
    .build()
    .expect("the entrypoint is contained");

let archive = encode(&pack).expect("the pack fits the reference encode limits");
assert!(!archive.as_slice().is_empty());
```

`Pack::builder` does not discover dependencies: the pack contains exactly the
files added to it. Use `create` when the library should run dependency
discovery over values the caller already holds, and a Pack Assembler when it
should also read those values from a source.

Pack creation is stateless and resumable. If the representative compile reaches
a package that is not in the supplied catalog, creation reports that exact
specification instead of failing; add its tree and call `create` again. The
`package-reading` feature provides official registry URL construction and
bounded `.tar.gz` expansion without choosing an HTTP client, and OpenDAL
provides `read_package` and `insert_read_package` for the same lifecycle over
configured operators.

A Pack Override can replace only a project path already contained in the pack.
It cannot add a path or change package or font requirements.

### Feature flags

- `fs`: Read projects, local packages, caches, and system fonts from the filesystem; unavailable on wasm targets.
- `egress`: Download missing packages during filesystem assembly; implies `fs` and `package-reading` and links HTTP/TLS dependencies.
- `package-reading`: Construct registry URLs, read bounded package archives, and expand them without choosing a transport.
- `opendal`: Use caller-polled OpenDAL reads and writes with caller-supplied operators and runtime support.
- `embedded-fonts`: Make Typst's bundled fonts available to assembly and external fulfillment.
- `diagnostics`: Retain source context for first-party diagnostic presentation adapters.
- `parallel`: Export independent page artifacts in parallel.

All features are opt-in. Featureless creation and compilation remain available
on `wasm32-unknown-unknown`.

## Migrating

### Migrating to 0.6

Version 0.6 standardizes storage vocabulary on **read** and **write**. The
rename was generated from `git diff fb610cb..HEAD`; there are no compatibility
aliases.

| Before 0.6 | 0.6 |
| --- | --- |
| Feature `package-acquisition` | `package-reading` |
| Module `typst_pack::opendal::publication` | `typst_pack::opendal::write` |
| `gather_filesystem_project` | `read_filesystem_project` |
| `gather_filesystem_font_catalog` | `read_filesystem_fonts` |
| `gather_filesystem_package` | `read_filesystem_package` |
| `FilesystemPackageAuthority::acquire` | `FilesystemPackageAuthority::read` |
| `acquire_package_archive` | `read_package_archive` |
| `opendal::pack_assembly::acquire_project` | `read_project` |
| `opendal::pack_assembly::acquire_fonts` | `read_fonts` |
| `opendal::pack_assembly::acquire_package` | `read_package` |
| `opendal::pack_archive::acquire_pack_archive` | `read_pack_archive` |
| `opendal::publication::publish_pack_archive` | `opendal::write::write_pack_archive` |
| `publish_package_cache_archive` | `write_package_cache_archive` |
| `publish_pack_extraction_plan` | `write_pack_extraction_plan` |
| `publish_compilation_artifacts` | `write_compilation_artifacts` |
| `publish_pack_extraction_plan_to_filesystem` | `write_pack_extraction_plan_to_filesystem` |
| `publish_pack_extraction_plan_to_filesystem_with_fault_probe` | `write_pack_extraction_plan_to_filesystem_with_fault_probe` |
| `publish_compilation_artifacts_to_filesystem_paths` | `write_compilation_artifacts_to_filesystem_paths` |
| `resolve_filesystem_publication_paths` | `resolve_filesystem_write_paths` |
| `CompilationArtifactPathPublicationError::publication_error` | `CompilationArtifactPathWriteError::write_error` |
| `insert_acquired_package` | `insert_read_package` |
| `pack_archive::acquire` / `acquire_file` | `pack_archive::read` / `read_file` |
| `pack_archive::publish` / `publish_file` | `pack_archive::write` / `write_file` |

Type families follow the same mechanical rules:

| Before 0.6 | 0.6 |
| --- | --- |
| `*Acquisition*` | `*Read*` |
| `*Publication*` | `*Write*` |
| `*GatherError` | `*ReadError` |
| `Acquired*` | `Read*` |
| `PublicationPolicy` | `WritePolicy` |
| `PublicationKeyOutcome` | `WriteKeyOutcome` |
| `PackArchiveAcquisitionError` | `PackArchiveReadError` |
| `ProjectAcquisitionRequest` | `ProjectReadRequest` |
| `PackageAcquisitionLimits` | `PackageReadLimits` |
| `AcquiredPackageInsertionError` | `ReadPackageInsertionError` |
| `FilesystemPackageAcquisitionError` | `FilesystemPackageAuthorityReadError` |
| `PackExtractionPublicationProgress` | `PackExtractionWriteProgress` |
| `CompilationArtifactPublicationReceipt` | `CompilationArtifactWriteReceipt` |
| `FilePublicationPolicy` | `FileWritePolicy` |

OpenDAL operation errors are no longer generic over an `OperatorResolver` error
type. Resolver failures are retained as boxed sources. Match the operation's
typed cause, then downcast the source to the concrete error supplied by your
resolver:

```rust,ignore
use typst_pack::opendal::pack_assembly::ProjectReadErrorCause;

if let ProjectReadErrorCause::ResolveOperator(source) = error.cause() {
    if let Some(resolver_error) = source.downcast_ref::<MyResolverError>() {
        handle_resolver_error(resolver_error);
    }
}
```

Compilation and encoding now have convenient reference limits. Use
`compile(request)` and `pack_archive::encode(pack)` for the built-in profiles;
use `compile_with_limits`, `encode_with_limits`, `write_pack_with_limits`, or
`save_pack_with_limits` to narrow them. Limits remain required at trust
boundaries, including archive decoding, package expansion, stream/file reads,
and filesystem or OpenDAL read requests. Invalid custom limit configurations
are programmer errors and panic during construction.

Pack Extraction and Compilation Output Artifact writes now share the crate-root
`*WriteEntry`, `*WriteProgress`, and `*WriteReceipt` types across filesystem and
OpenDAL adapters. Write errors retain progress where relevant and expose
`CommitCertainty`; successful receipts do not make an atomicity claim.

The request-origin and inventory wrappers were removed:
`CompilationRequestInventory`, `TypstInputsInventory`, `PackOverridesInventory`,
`PackOverrideInventoryEntry`, `EffectiveRequestValue`, `RequestValueOrigin`, and
`CompilationOutputOrigins`. Configure values directly on
`PackCompilationRequest`. Fulfillment provenance remains available through
`PackageTreeFulfillment`, `FontContainerFulfillment`, and the fulfillment report;
`CompilationAccessTrace` also remains available on a result.

`CanonicalIdentity` and `CanonicalIdentityRole` now implement `Display`,
rendering as `role:digest`. Equality is unchanged and still covers role, schema,
and algorithm, so compare whole identity values rather than the rendered string.

Dependency-fulfillment failures report what is missing. Every
`CompilationFulfillmentIssue` message names the package or font container
involved instead of dumping a `Debug` projection, and
`InvalidCompilationFulfillmentSet` no longer summarizes a single issue as a
count. Present these failures through `issues()`: the aggregate `Display` is a
summary, not the detail.

`typst-pack inspect` gained a `required fonts` section listing the external Font
Requirements a recipient must supply, and its `embedded fonts` lines now use the
shorter `role:digest` identity form. `typst-pack compile` reports each
unfulfilled dependency as a hint with the recovery to try.

The `egress` feature no longer links `rustls-pemfile`. Custom `--cert` PEM
parsing moved to `rustls-pki-types`, which absorbed it; behavior is unchanged,
including that a file with no PEM section adds no trust anchor.

### Migrating to 0.5

Version 0.5 added the optional OpenDAL adapter. Existing builds that do not
enable `opendal` are unaffected. Applications that enable it must select their
own backend, transport, runtime, credentials, and retry behavior. See
[Migrating to 0.5](crates/typst-pack/docs/opendal-integration.md#migrating-to-05) for dependency,
composition, target, identity, and cache guidance.

### Migrating to 0.4

Version 0.4 made clean breaks without compatibility aliases:

- Remove Resource Slot and Resource Provider APIs; pack placeholders and replace them with Pack Overrides.
- Rename Dagger arguments: `source` -> `project`, `entrypoint` -> `input`, `inputs` -> `sysInputs`, `noPackages` -> `noVendorPackages`, `sourceDateEpoch` -> `creationTimestamp`, and `CreationTarget` -> `TypstTarget`.
- Change creation from a directory plus `--entrypoint`/`--output` to `create <INPUT> [OUTPUT]`.
- Replace `compile_pack(request)` with `compile(request)`; the arbitrary-`World` overload and public `PackWorld` builder are removed.
- Read accepted compilation results from `CompilationReport::outcome()` or `result()`; `compile_report`, `PackCompileError`, `CompilationAttempt`, and `CompilationExecutionControls` are removed.
- Replace `CreationTarget` and `CompilationTarget` with `TypstTarget`, and configure time with one `DocumentTime`.
- Replace `Packer` with `FilesystemPackAssemblerConfig`, `FilesystemPackAssembler`, and `FilesystemPackAssemblyRequest`.
- Read representative-compile warnings from `PackAssemblyReport::warnings`; `PackReport` is removed.
- Inspect domain values through `Pack` accessors rather than Pack Manifest records.
- Handle shared Pack consistency failures through `PackInvariantError::issues()`.
- Replace `OutputFormat` plus `CompileOptions` request construction with a `CompilationOutputSpecification` variant.
- Replace `extract` with `plan_pack_extraction` followed by `write_pack_extraction_plan_to_filesystem` and an explicit `FilesystemMergePolicy`.

The unstable Pack format remains version 1, but discovery and Resource Slot
fields were removed in place. Old fields and aliases are not accepted.

## Pack format

A pack is a Zip archive, conventionally named `*.typk`, with this layout:

```text
typst-pack.toml                         manifest
project/<path>                          project files, root-relative
packages/<ns>/<name>/<version>/<path>   embedded package files
fonts/<file>                            embedded font files
```

Example manifest:

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

[[fonts]]
path = "fonts/ibm-plex-sans.ttf"
families = ["IBM Plex Sans"]

[metadata]
name = "Quarterly report"
authors = ["Jane Doe"]
```

The encoder writes Deflate-compressed version-1 archives. Readers accept safe
interoperable ZIP encodings and member orderings, ignore safe unknown entries,
and reject unknown format versions, unsafe paths, unsupported entry kinds, and
inconsistent manifests. Decoding and re-encoding preserves Pack semantics, not
exact ZIP bytes, compression settings, timestamps, unknown entries, or member
order. Format version 1 is explicitly unstable.

## Development

Minimum verification:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `dagger check`

Maintainers changing the embedded compiler must follow the
[embedded Typst upgrade procedure](docs/embedded-typst-upgrade.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
