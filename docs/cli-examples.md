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

## Overrides

`--override PACK_PATH FILE` replaces an existing project path inside the pack
for one compilation. `PACK_PATH` must exactly name a contained project file;
`FILE` is the local replacement. The operation does not modify the pack, add a
new project path, or replace package and font authority.

Replace one contained image:

```console
typst-pack compile invoice.typk customer.pdf \
  --override assets/logo.png customer-logo.png
```

Repeat `--override` to replace several contained files:

```console
typst-pack compile invoice.typk customer.pdf \
  --override main.typ variants/customer-main.typ \
  --override data/customer.json customers/acme.json
```
