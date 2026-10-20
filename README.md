# typst-pack

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/typst-pack/ci.yaml?style=flat-square)](https://github.com/sagikazarmark/typst-pack/actions/workflows/ci.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/typst-pack/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/typst-pack)
[![crates.io](https://img.shields.io/crates/v/typst-pack?style=flat-square)](https://crates.io/crates/typst-pack)
[![docs.rs](https://img.shields.io/docsrs/typst-pack?style=flat-square)](https://docs.rs/typst-pack)

**Bundle a Typst project and its fonts and packages into one file that can
compile on another machine.**

A pack (`.typk`) contains an entrypoint, the project's source and data files,
and the exact packages and fonts the document uses. Packages and fonts can be
embedded for offline, portable compilation or recorded as requirements for the
compiling machine to supply.

This is unrelated to Typst's bundle output (`typst-bundle`). A pack is portable
**input** for later compilation, not a collection of rendered output files.

## Features

- 📦 Put a whole Typst project in one `.typk` file, including images and data.
- ✈️ Vendor imported Typst Universe packages so compilation works offline.
- 🔤 Embed selected fonts so output does not depend on fonts installed elsewhere.
- 🖨️ Compile to PDF, PNG, SVG, or experimental HTML without reading ambient project files.
- 🎛️ Pass strings and file bytes to the document through `sys.inputs` at compile time.
- 🔍 Inspect or extract a pack before using it.
- 🏗️ Build packs from the filesystem, entirely in memory, or with caller-supplied OpenDAL storage.
- 🔒 Keep filesystem access and network download support as separate build choices.

## CLI or library?

typst-pack comes as a command-line tool and as a Rust library. Both read and
write the same `.typk` files.

- **The CLI** (`typst-pack-cli`) packs projects on disk and compiles packs,
  from a terminal, a script, or CI. Its flags follow `typst compile`.
- **The library** (`typst-pack`) builds the same workflows into applications:
  a web service that renders a document per customer, an editor, a system
  that keeps projects in object storage, or a WebAssembly module. Its core
  does no I/O, so the application decides where projects, packages, fonts,
  and packs come from.

## CLI

Download a prebuilt binary from the
[latest release](https://github.com/sagikazarmark/typst-pack/releases/latest),
or build from source:

```console
cargo install typst-pack-cli
```

Pack a project, look inside, compile it, and get the editable project back:

```console
typst-pack create project/main.typ project.typk
typst-pack inspect project.typk
typst-pack compile project.typk report.pdf
typst-pack extract project.typk -o copy/
```

`create` packs every file beneath the entrypoint's directory and stores every
package the document imports. Fonts are recorded but not stored unless you
ask, so the compiling machine needs the same fonts; Typst's own fonts are
always available.

### Common tasks

```console
# Make the pack fully self-contained by embedding the fonts it uses:
typst-pack create project/main.typ project.typk --embed-fonts

# Keep files out of the pack with Gitignore rules:
echo ".git/" >> project/.typkignore

# Compile one document per recipient with `sys.inputs` (strings or file bytes):
typst-pack compile invoice.typk acme.pdf --input locale=de --input-file logo=acme-logo.png

# Render pages to images:
typst-pack compile project.typk "page-{0p}.png" --ppi 300 --pages 1-3

# Make sure nothing is downloaded:
typst-pack compile project.typk --offline
```

The [CLI guide](https://github.com/sagikazarmark/typst-pack/blob/main/docs/guides/cli.md)
covers every command: choosing project files, vendoring packages, embedding
fonts, discovery variants, inputs, output formats, extraction, and pipes.

## Library

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["embedded-fonts", "fs"] }
```

The core needs no features. `fs` reads projects, packages, and fonts from the
filesystem and writes packs and outputs; it links no network client unless
`egress` is enabled too.

Build a pack in memory, encode it, and compile it with an input:

```rust
use typst_pack::archive::{self, DecodeLimits};
use typst_pack::compile::{self, Input, Output, OutputFormat, Request};
use typst_pack::Pack;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello, #sys.inputs.name!".to_vec())
        .build()?;

    // Store or send the `.typk` bytes anywhere...
    let typk: Vec<u8> = archive::encode(&pack);

    // ...and compile them later.
    let pack = archive::decode(&typk, DecodeLimits::default())?;
    let mut request = Request::new(pack, Output::new(OutputFormat::Pdf));
    request.options.inputs.push(("name".into(), Input::from("Jane")));
    let compiled = compile::compile(&request)?;

    assert!(compiled.artifacts[0].bytes.starts_with(b"%PDF"));
    Ok(())
}
```

`Pack::builder` packs exactly what it is given. To discover the packages and
fonts a document uses, start from one of these:

| Task | Start here |
| --- | --- |
| Pack a project directory from disk | [`fs::Packer`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html) |
| Pack in-memory files, supplying packages and fonts yourself | [`create`](https://docs.rs/typst-pack/latest/typst_pack/create/fn.create.html) |
| Find a pack's external packages and fonts on disk | [`fs::Packer::resolve`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html#method.resolve) |
| Compile a pack | [`compile`](https://docs.rs/typst-pack/latest/typst_pack/compile/fn.compile.html) |
| Read and write packs through object storage | [OpenDAL guide](https://github.com/sagikazarmark/typst-pack/blob/main/crates/typst-pack/docs/opendal-integration.md) |

The [library guide](https://github.com/sagikazarmark/typst-pack/blob/main/docs/guides/library.md)
walks through these with examples and lists every crate feature.

## Documentation

Guides:

- [CLI guide](https://github.com/sagikazarmark/typst-pack/blob/main/docs/guides/cli.md)
- [Library guide](https://github.com/sagikazarmark/typst-pack/blob/main/docs/guides/library.md)
- [OpenDAL guide](https://github.com/sagikazarmark/typst-pack/blob/main/crates/typst-pack/docs/opendal-integration.md)

Reference:

- [API reference](https://docs.rs/typst-pack) on docs.rs
- [Pack format](https://github.com/sagikazarmark/typst-pack/blob/main/docs/reference/pack-format.md):
  the archive, manifest, digests, and identities
- [Library contract](https://github.com/sagikazarmark/typst-pack/blob/main/docs/reference/library-contract.md):
  guarantees about identity, environment independence, limits, writes, and errors
- [CLI parity inventory](https://github.com/sagikazarmark/typst-pack/blob/main/docs/reference/cli-parity.md):
  how the CLI follows and differs from `typst compile`

The [documentation index](https://github.com/sagikazarmark/typst-pack/blob/main/docs/README.md)
also lists the development docs and architecture decisions.

## Development

The development environment is defined with [devenv](https://devenv.sh), and
CI runs the [just](https://just.systems) recipes in it. Minimum verification:

- `just ci`
- `just test-cli-parity` (x86_64 Linux only)

`just` lists every recipe. Maintainers changing the embedded compiler must
follow the
[embedded Typst upgrade procedure](https://github.com/sagikazarmark/typst-pack/blob/main/docs/development/embedded-typst-upgrade.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/sagikazarmark/typst-pack/blob/main/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](https://github.com/sagikazarmark/typst-pack/blob/main/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
