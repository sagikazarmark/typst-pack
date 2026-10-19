# CLI examples

The CLI works with filesystem projects and local `.typk` files. OpenDAL is a
library integration and does not add OpenDAL locations to the CLI.

Install the command-line tool:

```console
cargo install typst-pack-cli
```

## Create and inspect

Create a pack. Packages observed during the discovery compile are
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
add to the `--input` and `--input-file` values for one more compile; the pack
requires everything any compile used. Values cannot contain commas:

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
one typst-pack embeds, since its output may differ. `-` reads a pack from
stdin for `inspect`, `extract` (which then needs `--output`), and `compile`
(which then needs an output path), and `create` writes one to stdout with `-`:

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
project file. Typst functions that take data accept bytes directly. An
`invoice.typ` that reads a logo, a customer record, and a locale:

```typ
#set text(lang: sys.inputs.at("locale", default: "en"))
#let customer = json(sys.inputs.customer)
#image(sys.inputs.logo, width: 2cm)

Invoice for #customer.name
```

The discovery compile of `create` runs the document too, so inputs the
document requires are needed when the pack is created as well; pass samples
(and list the input files in `.typkignore` if they live beneath the project
root, so the pack does not contain them):

```console
typst-pack create invoice.typ invoice.typk \
  --input-file logo=sample-logo.png \
  --input-file customer=customers/sample.json
```

Then compile the pack once per recipient:

```console
typst-pack compile invoice.typk acme.pdf \
  --input-file logo=acme-logo.png \
  --input-file customer=customers/acme.json \
  --input locale=de
```

Alternatively, give the document a fallback for an input it can do without:

```typ
#let logo = sys.inputs.at("logo", default: none)
#if logo != none { image(logo, width: 2cm) }
```

Input files are dependencies: `--deps` lists them with the pack. One key may
come from `--input` or from `--input-file`, not from both.
