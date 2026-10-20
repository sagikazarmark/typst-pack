# Library guide

The `typst-pack` crate does everything the CLI does, and lets an application
decide where projects, packages, fonts, and packs come from. Its core reads
nothing and needs no crate features: callers hand it bytes, and it creates,
encodes, decodes, and compiles packs. Adapters read and write the bytes:
`fs` for the local filesystem and `opendal` for object storage.

The [API reference](https://docs.rs/typst-pack) documents every type. The
examples on this page are compile-checked against the current source.

## Add the dependency

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["embedded-fonts", "fs"] }
```

All features are opt-in:

| Feature | Adds |
| --- | --- |
| `fs` | Reading projects, local packages, the package cache, and system fonts from the filesystem; reading and writing packs, extractions, and artifacts. Links no network client. Unavailable on wasm targets. |
| `egress` | Downloading missing packages from Typst Universe during filesystem packing. Implies `fs` and `package-reading`, and links HTTP and TLS dependencies. |
| `embedded-fonts` | Typst's own fonts (Libertinus Serif, New Computer Modern, ...) for packing and compiling. |
| `package-reading` | Bounded expansion of registry package archives (`package::registry::expand_archive`), without choosing an HTTP client. |
| `opendal` | Reading and writing through caller-supplied OpenDAL operators, with caller-polled futures and no bundled runtime. With `package-reading`, also packing (`opendal::Assembler`). |
| `diagnostics` | `typst-kit`'s `DiagnosticWorld` for `Sources`, so `typst_kit::diagnostics::emit` renders diagnostics with source context. |
| `parallel` | Exporting independent pages in parallel. |

Without features, creation and compilation still work, including on
`wasm32-unknown-unknown`.

## Pack a project directory

[`fs::Packer`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html)
does what `typst-pack create` does. Its fields mirror the CLI's flags:
`embed_fonts`, `embed_packages`, `download_packages` (with `egress`), `options`
for the discovery compile's inputs, `variants`, and more.

```rust,no_run
use typst_pack::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packer = fs::Packer {
        embed_fonts: true,
        ..fs::Packer::default()
    };
    let packed = packer.pack("path/to/project", "main.typ")?;
    for warning in &packed.warnings {
        eprintln!("warning: {}", warning.message);
    }
    fs::write_archive("project.typk", &packed.pack, fs::WritePolicy::CreateNew)?;
    Ok(())
}
```

## Compile a pack

[`compile`](https://docs.rs/typst-pack/latest/typst_pack/compile/fn.compile.html)
takes one `compile::Request`: the pack, the output and its controls, the
`sys.inputs` values, the document time, Typst features, the packages and fonts
the pack requires but does not embed, and output limits. It reads nothing
else: no project files, package caches, fonts, environment variables, clock,
or network.

[`fs::Packer::resolve`](https://docs.rs/typst-pack/latest/typst_pack/fs/struct.Packer.html#method.resolve)
finds a pack's external packages and fonts on the host:

```rust,no_run
use typst_pack::archive::DecodeLimits;
use typst_pack::compile::{self, DocumentTime, Input, Output, OutputFormat, Request};
use typst_pack::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = fs::read_archive("invoice.typk", DecodeLimits::default())?;
    let resolved = fs::Packer::default().resolve(&pack)?;

    let mut request = Request::new(pack, Output::new(OutputFormat::Pdf));
    request.packages = resolved.packages;
    request.fonts = resolved.fonts;
    request.options.inputs = vec![
        ("customer".into(), Input::from(std::fs::read("acme.json")?)),
        ("locale".into(), Input::from("de")),
    ];
    request.options.document_time = DocumentTime::UnixTimestamp(1_700_000_000);

    let compiled = compile::compile(&request)?;
    fs::write_file("acme.pdf", &compiled.artifacts[0].bytes, fs::WritePolicy::Overwrite)?;
    Ok(())
}
```

Without a document time, `datetime.today()` returns `none`. PNG and SVG
output produce one artifact per selected page, each with its page number;
`fs::write_artifacts` writes them together:

```rust,no_run
use typst_pack::archive::DecodeLimits;
use typst_pack::compile::{self, Output, OutputFormat, Request};
use typst_pack::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = fs::read_archive("project.typk", DecodeLimits::default())?;
    let mut output = Output::new(OutputFormat::Png);
    output.ppi = Some(300.0);

    let compiled = compile::compile(&Request::new(pack, output))?;
    let paths: Vec<String> = compiled
        .artifacts
        .iter()
        .map(|artifact| format!("page-{}.png", artifact.page.expect("PNG artifacts have pages")))
        .collect();
    fs::write_artifacts(&paths, &compiled.artifacts, fs::WritePolicy::CreateNew)?;
    Ok(())
}
```

### Report compile errors

A failed compilation returns a `compile::Error` whose message names every
invalid value or unfulfilled requirement. Typst errors come with the
`Sources` their spans point into:

```rust
use typst_pack::compile::{self, Input, Output, OutputFormat, Request};
use typst_pack::Pack;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#str(sys.inputs.greeting)".to_vec())
        .build()?;
    let mut request = Request::new(pack, Output::new(OutputFormat::Pdf));
    request.options.inputs.push(("greeting".into(), Input::from(b"Hello".to_vec())));

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
    Ok(())
}
```

With the `diagnostics` feature, `typst_kit::diagnostics::emit` renders them
with source context, as the CLI does.

## Extract a pack

`extract::plan` lists the files an extraction writes; an adapter writes them,
checking every path before it writes anything:

```rust,no_run
use typst_pack::archive::DecodeLimits;
use typst_pack::{extract, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = fs::read_archive("project.typk", DecodeLimits::default())?;
    let selection = extract::Selection { packages: true, fonts: true };
    let plan = extract::plan(&pack, selection)?;
    fs::write_extraction("project", &plan, fs::WritePolicy::CreateNew)?;
    Ok(())
}
```

## Work without a filesystem

The core accepts values the application already holds, so it runs in web
services, editors, and WebAssembly. Pick the entry point by how much the
library should discover:

- [`Pack::builder`](https://docs.rs/typst-pack/latest/typst_pack/struct.Pack.html#method.builder)
  packs exactly the files, packages, and fonts it is given. It compiles
  nothing.
- [`create`](https://docs.rs/typst-pack/latest/typst_pack/create/fn.create.html)
  compiles a `ProjectSnapshot` over the packages and fonts you supply, and
  requires what the document used.
- `fs::Packer` and `opendal::Assembler` also read the project, packages, and
  fonts from their source.

`archive::encode` and `archive::decode` convert between a `Pack` and `.typk`
bytes, which can be stored anywhere.

Creation is stateless and resumable. When a discovery compile imports a
package that is not in the supplied catalog, `create` returns
`Outcome::MissingPackages` with the exact specifications instead of failing.
Add the trees and call it again:

```rust,no_run
use typst::syntax::package::PackageSpec;
use typst_pack::create::{Outcome, Request};
use typst_pack::{Pack, ProjectSnapshot, create, package};

fn assemble(
    project: ProjectSnapshot,
    mut read_tree: impl FnMut(&PackageSpec) -> Result<package::Tree, Box<dyn std::error::Error>>,
) -> Result<Pack, Box<dyn std::error::Error>> {
    let mut request = Request::new(project);
    loop {
        match create(&request)? {
            Outcome::Created(created) => return Ok(created.pack),
            Outcome::MissingPackages(missing) => {
                for spec in missing {
                    let tree = read_tree(&spec)?;
                    request.packages.insert(spec, tree, true)?;
                }
            }
            outcome => return Err(format!("unexpected outcome: {outcome:?}").into()),
        }
    }
}
```

To fetch packages with your own HTTP client, `package::registry` names the
official registry URL of each specification, and the `package-reading`
feature expands the downloaded `.tar.gz` into a `package::Tree`.

## Use object storage

The `opendal` feature reads projects, packages, fonts, and packs from
caller-supplied OpenDAL operators and writes packs, extractions, and
artifacts back. See the
[OpenDAL guide](https://github.com/sagikazarmark/typst-pack/blob/main/crates/typst-pack/docs/opendal-integration.md).

## Guarantees

The [library contract](../reference/library-contract.md) describes identity,
dependency fulfillment, environment independence, limits, write policies,
partial effects, and errors. The [pack format](../reference/pack-format.md)
specifies the archive and every identity.
