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

Every file beneath the project root is packed. List files that should stay
out, such as per-recipient data or a Git checkout's `.git/`, in a root
`.typkignore` (Gitignore syntax):

```text
.git/
recipients/
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

`inspect` shows the Typst version and tool that produced the pack, its
identity, its files, and the packages and fonts it embeds or requires; a
required font Typst ships is marked as provided by Typst. `inspect` and
`compile` warn when the pack was created with another Typst version than the
one typst-pack embeds, since its output may differ. `-` reads a pack from stdin for `inspect`, `extract` (which then needs
`--output`), and `compile`, and `create` writes one to stdout with `-`:

```console
typst-pack create project/main.typ - | typst-pack inspect -
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

Render selected pages to numbered PNG files (the output directory must exist):

```console
mkdir -p output
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

The discovery compile of `create` runs the document too, so a document that
requires an input needs it when the pack is created as well: pass a sample
with `create --input-file logo=sample-logo.png`, or give the document a
fallback such as `sys.inputs.at("logo", default: none)`.

Input files are dependencies: `--deps` lists them with the pack. One key may
come from `--input` or from `--input-file`, not from both.
