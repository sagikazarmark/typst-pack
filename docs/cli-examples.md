# CLI examples

The CLI works with filesystem projects and local `.typk` files. OpenDAL is a
library integration and does not add OpenDAL locations to the CLI.

Install the command-line tool:

```console
cargo install typst-pack-cli
```

## Create and inspect

Create a pack. Packages observed during the representative compilation are
vendored by default:

```console
typst-pack create project/main.typ project.typk
```

Embed the fonts used by the document:

```console
typst-pack create project/main.typ project.typk --embed-fonts
```

Run additional discovery compiles for inputs that reach other packages or
fonts. Each `--variant` takes comma-separated `KEY=VALUE` pairs that replace or
add to the `--input` values for one more compile; the pack requires everything
any compile used. Values cannot contain commas:

```console
typst-pack create report.typ report.typk \
  --input edition=print \
  --variant edition=web \
  --variant edition=web,lang=de
```

Inspect the resulting pack:

```console
typst-pack inspect project.typk
```

## Compile

Compile to PDF without allowing package downloads:

```console
typst-pack compile project.typk output.pdf --offline
```

Pass string values through Typst's `sys.inputs`:

```console
typst-pack compile project.typk output.pdf \
  --input customer=Acme \
  --input locale=en
```

Render selected pages to numbered PNG files:

```console
typst-pack compile project.typk "output/page-{0p}-of-{t}.png" \
  --pages 1-3 \
  --ppi 300
```

Extract the project, vendored packages, and embedded fonts:

```console
typst-pack extract project.typk --all --output extracted/
```

## Inputs

`--input KEY=VALUE` passes a string and `--input-file KEY=PATH` passes the
bytes of a local file through `sys.inputs`. The pack is not modified: a
document reads per-recipient data from its inputs instead of from a replaced
project file. Typst functions that take data accept bytes directly:

```typ
#image(sys.inputs.logo, width: 2cm)
#let customer = json(sys.inputs.customer)
#str(sys.inputs.note)
```

```console
typst-pack compile invoice.typk customer.pdf \
  --input-file logo=customer-logo.png \
  --input-file customer=customers/acme.json \
  --input locale=en
```

Input files are dependencies: `--deps` lists them with the pack. `create`
accepts `--input` and `--input-file` too, for discovery compiles that read
them.
