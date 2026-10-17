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
- Pass strings and file bytes to the document through `sys.inputs` at compile time.
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

# Pass a recipient's file to the document as bytes, e.g. for `image(sys.inputs.logo)`:
typst-pack compile invoice.typk customer.pdf --input-file logo=customer-logo.png

# PNG or SVG output, page selection, reproducible builds:
typst-pack compile project.typk "page-{0p}.png" --ppi 300 --pages 1-3
typst-pack compile project.typk reproducible.pdf --creation-timestamp 1700000000

# Guarantee no network access (fails instead of downloading packages):
typst-pack compile project.typk --offline

# Experimental HTML export:
typst-pack compile project.typk out.html
typst-pack create project/main.typ --target html

# Also discover packages and fonts reached only under other inputs:
typst-pack create report.typ --variant edition=print --variant edition=web,lang=de

# Unpack a pack back into an editable project directory:
typst-pack extract project.typk -o project/
```

For PNG and SVG output, `{p}` is the one-based source page number, `{0p}` and
`{n}` are zero-padded aliases, and `{t}` is the source-document page count.
Multi-page output needs a page placeholder. All output paths are checked for
duplicates before anything is written.

See [CLI examples](docs/cli-examples.md) for additional create, compile,
extraction, input, and discovery-variant commands.

### Project files

`create` includes every eligible regular file beneath the project root, not
only files reached by the representative compile. A root `.typkignore` uses
Gitignore-style ordered rules. It is included in the pack; nested
`.typkignore` files are ordinary files. Symbolic links are followed when they
resolve inside the project root. A link leading outside it, a FIFO, socket, or
device is rejected unless ignored, and any path containing a `.typk` component
is left out.

The representative compile selects package and font requirements. Its target,
inputs, date, features, and control flow do not change which project files are
included. Each `--variant KEY=VALUE[,KEY=VALUE...]` runs one more discovery
compile with those inputs replacing or adding to `--input`, so packages and
fonts reached only under other inputs are required too. Per-recipient data
reaches a compile through `--input KEY=VALUE` (a string) and
`--input-file KEY=PATH` (the file's bytes, which `image`, `read`-style
decoders such as `json`, and `str` accept).

### Packages and fonts

Observed packages are embedded by default. `--no-vendor-packages` records each
exact package and its tree digest instead; compilation then requires the
application's configured package sources to provide a matching tree. `--offline`
prevents downloads during both creation and compilation.

Fonts are external by default and are matched by digest at compile time;
`typst-pack inspect` names the families a pack expects. `--embed-fonts` stores
the used font containers except those shipped by Typst; `--include-typst-embedded-fonts` stores those as
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
dependency fulfillment, environment independence, write policies, and partial
effects. The [OpenDAL guide](crates/typst-pack/docs/opendal-integration.md)
documents asynchronous storage integration.

### Common workflows

Each entry links to a compile-checked example on docs.rs, so the code shown
there is verified against the release you are reading about.

| Task | Start here |
| --- | --- |
| Pack a project directory from disk | [`fs::Packer`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html) |
| Pack in-memory bytes with no filesystem | [`Pack::builder`](https://docs.rs/typst-pack/latest/typst_pack/struct.Pack.html#method.builder) |
| Supply packages yourself and resume creation | [`create`](https://docs.rs/typst-pack/latest/typst_pack/create/fn.create.html) |
| Compile a pack with inputs | [`compile`](https://docs.rs/typst-pack/latest/typst_pack/compile/fn.compile.html) |
| Find a pack's external packages and fonts on disk | [`fs::Packer::resolve`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html#method.resolve) |
| Read and write packs through object storage | [OpenDAL guide](crates/typst-pack/docs/opendal-integration.md) |

The shortest complete example — build a pack in memory and encode it:

```rust
use typst_pack::{Pack, archive};

let pack = Pack::builder("main.typ")
    .file("main.typ", b"= Report\n".to_vec())
    .build()
    .expect("the entrypoint is contained");

let bytes = archive::encode(&pack);
assert!(!bytes.is_empty());
```

`Pack::builder` does not discover dependencies: the pack contains exactly the
files added to it. Use `create` when the library should discover them by
compiling the project over values the caller already holds, and `fs::Packer`
or `opendal::Assembler` when it should also read those values from a source.

Pack creation is stateless and resumable. If the representative compile reaches
a package that is not in the supplied catalog, creation reports that exact
specification instead of failing; add its tree and call `create` again. The
`package-reading` feature provides official registry URL construction and
bounded `.tar.gz` expansion without choosing an HTTP client, and
`opendal::Assembler` runs the same loop over OpenDAL storage.

Compilation takes one `compile::Request`: the pack, the output and its
controls, `sys.inputs` strings and bytes, document time, features, the packages
and font containers the pack requires externally, and limits. It returns
`compile::Compiled` artifacts and warnings, or a `compile::Error` whose message
names every invalid value or unsatisfied requirement. Diagnostics come with the
`Sources` they point into:

```rust
use typst_pack::compile::{self, Input, Output, OutputFormat, Request};
use typst_pack::Pack;

let pack = Pack::builder("main.typ")
    .file("main.typ", b"#str(sys.inputs.greeting)".to_vec())
    .build()?;
let mut request = Request::new(pack, Output::new(OutputFormat::Pdf));
request.options.inputs.push(("greeting".into(), Input::Bytes(b"Hello".to_vec())));

match compile::compile(&request) {
    Ok(compiled) => assert!(compiled.artifacts[0].bytes.starts_with(b"%PDF")),
    Err(compile::Error::Failed { errors, sources, .. }) => {
        for error in &errors {
            let file = error.span.id().map(|id| sources.name(id));
            eprintln!("{file:?}: {}", error.message);
        }
    }
    Err(error) => return Err(error.into()),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Feature flags

- `fs`: Read projects, local packages, caches, and system fonts from the filesystem; unavailable on wasm targets.
- `egress`: Download missing packages during filesystem assembly; implies `fs` and `package-reading` and links HTTP/TLS dependencies.
- `package-reading`: Construct registry URLs and expand bounded package archives without choosing a transport (`package::registry`).
- `opendal`: Read, assemble, and write packs through caller-supplied OpenDAL operators, with caller-polled futures and no bundled runtime.
- `embedded-fonts`: Make Typst's bundled fonts available to assembly and external fulfillment.
- `diagnostics`: Implement `typst-kit`'s `DiagnosticWorld` for `Sources`, so `typst_kit::diagnostics::emit` renders diagnostics with source context.
- `parallel`: Export independent page artifacts in parallel.

All features are opt-in. Featureless creation and compilation remain available
on `wasm32-unknown-unknown`.

## Pack format

A pack is a ZIP archive, conventionally named `*.typk`, with this layout:

```text
typst-pack.toml                         manifest
project/<path>                          every project file, root-relative
packages/<ns>/<name>/<version>/<path>   files of embedded packages
fonts/<hex digest>.<ext>                embedded font containers
```

Example manifest:

```toml
format-version = 2
typst-version = "0.15.1"
generator = "typst-pack 0.6.0"
entrypoint = "main.typ"

[[files]]
path = "main.typ"
digest = "sha256:8d5f78795df7052f5d6004f5d268ca4175c86ccdeaaf5a30d1c759c78e34fe60"

[[packages]]
spec = "@preview/cetz:0.3.4"
digest = "sha256:59a7210040a1150688b50619e101f628b72234a06c7568ca5e556283642281fb"
embedded = true

[[fonts]]
digest = "sha256:fcf06307a77367394fcb0ccb241e59eea70dba3d732be309647611224679c733"
embedded = false

[[fonts.faces]]
index = 0
family = "Libertinus Serif"
style = "normal"
weight = 400
stretch = 1000

[metadata]
name = "Quarterly report"
authors = ["Jane Doe"]
```

`[[files]]` lists every project file. `[[packages]]` lists every package the
document needs, embedded or to be supplied at compile time. `[[fonts]]` lists
every font container the document needs, in font book order; every face of a
listed container is available to the document. Faces are informational: they
let a recipient see which fonts an external container provides. `typst-version`
and `generator` are informational, and `[metadata]` is descriptive only.

### Digests

Every digest is SHA-256, written `sha256:` followed by 64 lowercase
hexadecimal digits.

- **File digest:** SHA-256 of the file's bytes.
- **Font container digest:** SHA-256 of the font file or collection bytes.
- **Package tree digest:** SHA-256 of a preimage over every file of the
  package, in byte-wise path order.
- **Pack identity:** SHA-256 of a preimage over the entrypoint, the project
  files, the packages, and the font containers. Metadata, informational fields,
  member order, compression, and every other ZIP detail are excluded.

Preimages are built from these fields: `str(s)` is the UTF-8 byte length of
`s` as a big-endian 64-bit integer followed by the bytes; `count(n)` is `n` as a
big-endian 64-bit integer; `digest(d)` is the 32 raw digest bytes; `bool(b)` is
one byte, `0` or `1`. Each preimage starts with a domain tag.

```text
package tree = str("typst-pack/package-tree/v1") count(files)
               { str(path) digest(file digest) }      per file, by path

pack         = str("typst-pack/pack/v1") str(entrypoint)
               count(files) { str(path) digest(file digest) }      by path
               count(packages) { str(spec) digest(tree digest) bool(embedded) }
                                                                  by spec
               count(fonts) { digest(container digest) bool(embedded) }
                                                       in manifest order
```

### Compilation identity

`compile::Compiled::identity` is SHA-256 over this preimage, in the same encoding, plus
`int(n)`, a big-endian two's-complement 64-bit integer:

```text
compilation = str("typst-pack/compilation/v1") str(implementation)
              digest(pack identity) output
              count(inputs) { str(key) input }        in effective order
              time
              count(features) { str(feature) }        "html", "a11y-extras"
              count(packages) { str(spec) digest(tree digest) }
                                       externally supplied, by spec
              count(fonts) { digest(container digest) }
                                       externally supplied, by digest
input       = str("str") str(value) | str("bytes") digest(SHA-256 of the bytes)
time        = str("absent") | str("unix") int(seconds)
            | str("fixed") { bool(present) int(value) }
                           for year, month, day, hour, minute, second
output      = str("pdf") pages count(standards) { str(standard) }
                identifier creator bool(tagged) timestamp bool(pretty)
            | str("png") pages int(bits of the f64 PPI) bool(render bleed)
            | str("svg") pages bool(render bleed) bool(pretty)
            | str("html") bool(pretty)
pages       = count(ranges) { count(first) int(last or -1 when open) }
identifier  = str("auto") | str("custom") str(identifier)
creator     = str("auto") | str("none") | str("custom") str(creator)
timestamp   = str("omit") | str("utc") int(seconds)
            | str("local") int(seconds) int(offset minutes)
```

`implementation` is `typst_pack::VERSION`, for example `0.6.0 (Typst 0.15.1)`.
Values are normalized first: a repeated input key keeps its first position and
last value, HTML output adds the `html` feature, page ranges are sorted and
merged, PDF standards are sorted by name, automatic PDF tagging is resolved,
and the default PPI is filled in. Metadata, limits, export workers, and
dependencies the pack does not require do not contribute.

### Verification

A reader decodes a pack only if every member matches the manifest: each
project file and embedded font container matches its digest, each embedded
package's files match its tree digest, and no member is missing or undeclared.
Member names must be safe relative paths without duplicates; links and other
special entries are refused; directory entries are ignored. Declared sizes and
actual inflated bytes are both bounded by `archive::DecodeLimits`. External packages and
fonts are matched by digest when the pack is compiled.

The encoder is deterministic for a given build: fixed member order, timestamps,
permissions, and compression level.

### Compatibility

`format-version` changes with every incompatible format change, and readers
reject every version they do not implement. Readers ignore unknown manifest
keys, so a producer can add compatible fields without a new version.

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
