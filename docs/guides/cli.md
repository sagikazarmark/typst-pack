# CLI guide

The `typst-pack` command packs Typst projects on the filesystem into `.typk`
files, and inspects, compiles, and extracts them. This guide covers each task;
`typst-pack <command> --help` lists every flag.

Flags that `typst compile` also has keep its spelling, defaults, and
environment variables. The deliberate differences are listed in the
[CLI parity inventory](../reference/cli-parity.md#pack-differences).

## Install

Download a prebuilt binary from the
[latest release](https://github.com/sagikazarmark/typst-pack/releases/latest),
or build from source:

```console
cargo install typst-pack-cli
```

`typst-pack --version` prints the typst-pack version and the Typst release it
embeds.

## Create a pack

```console
typst-pack create project/main.typ
```

This writes `project/main.typk`. Name a different output path as the second
argument, or `-` for stdout:

```console
typst-pack create project/main.typ report.typk
```

`create` runs a _discovery compile_ of the document to find the packages and
fonts it uses, then packs:

- every file beneath the project root (by default the entrypoint's directory;
  set it with `--root`), whether or not the document reads it;
- every package the document imports, stored in the pack ("vendored");
- the fonts the document uses, recorded by digest but not stored, unless you
  ask to embed them.

### Choose the project files

Every regular file beneath the root is packed. List files that should stay
out, such as a Git checkout's `.git/` or per-recipient data, in a
`.typkignore` file at the project root. It uses Gitignore syntax and is packed
itself; nested `.typkignore` files are ordinary files.

```text
.git/
recipients/
```

Symbolic links are followed when they resolve inside the project root. A link
leading outside it, a FIFO, a socket, or a device is an error unless ignored.
Any path with a component named `.typk` or ending in `.typk`, such as an
earlier pack, is left out.

### Packages

Packages are vendored by default, so the pack compiles offline. They are read
from the local package directory and the package cache, and downloaded from
Typst Universe when missing. `--package-path` and `--package-cache-path` (or
`TYPST_PACKAGE_PATH` and `TYPST_PACKAGE_CACHE_PATH`) select other directories,
and `--offline` forbids the download.

To keep packages out of the pack, record them instead:

```console
typst-pack create project/main.typ --no-vendor-packages
```

The pack then names each exact package version and the digest of its files.
Compiling it requires a matching package, which `compile` looks up the same
way (and downloads unless `--offline`).

### Fonts

Fonts are recorded by digest and not stored by default. Compiling the pack
then needs the same font files on the compiling machine. `typst-pack inspect`
lists the font families a pack expects.

Embed the fonts the document uses to make the pack self-contained:

```console
typst-pack create project/main.typ --embed-fonts
```

This stores every used font file except those Typst itself ships, which every
typst-pack build provides; add `--include-typst-embedded-fonts` to store those
as well. Mind font licenses before you redistribute a pack with embedded
fonts.

Fonts are found the same way as in `typst compile`: system fonts, Typst's own
fonts, and `--font-path` directories, controlled by `--ignore-system-fonts`,
`--ignore-embedded-fonts`, and the matching `TYPST_*` variables. A whole font
file is required, so every face in it is available when the pack is compiled.
Text that needs a font file the discovery compile never loaded can fall back
only among the pack's fonts and may show missing glyphs; use a
[variant](#discover-dependencies-under-other-inputs) whose inputs reach that
text.

### Discover dependencies under other inputs

Only packages and fonts the discovery compile actually used are recorded. If
the document imports other packages or uses other fonts under other inputs,
add a `--variant` for each such case. Each variant runs one more discovery
compile with its comma-separated `KEY=VALUE` pairs replacing or adding to the
`--input` and `--input-file` values; the pack requires everything any compile
used. Values cannot contain commas.

```console
typst-pack create report.typ \
  --input edition=print \
  --variant edition=web \
  --variant edition=web,lang=de
```

The discovery compile's inputs, date, and features never change which project
files are packed.

### HTML documents

For a document written for HTML export, discover with the HTML target:

```console
typst-pack create project/main.typ --target html
```

HTML export is experimental in Typst.

### Metadata

`--name`, `--description`, and `--author` (repeatable) record descriptive
metadata, which `inspect` shows. Metadata is not part of the pack's identity.

## Inspect a pack

```console
typst-pack inspect project.typk
```

`inspect` shows the Typst version and tool that produced the pack, its
identity, its files, and the packages and fonts it embeds or requires. A
required font that Typst ships is marked as provided by Typst.

`inspect` and `compile` warn when the pack was created with another Typst
version than the one typst-pack embeds, since its output may differ.

## Compile a pack

```console
typst-pack compile project.typk
```

This writes `project.pdf`. The output format follows the output path's
extension, or `--format`:

```console
typst-pack compile project.typk report.pdf
typst-pack compile project.typk report.html
typst-pack compile project.typk --format html
```

HTML export is experimental in Typst; HTML output enables Typst's `html`
feature itself.

Compilation reads nothing but the pack, its inputs, and the external packages
and fonts it requires; it never reads other files from the project directory.
`--offline` guarantees that no required package is downloaded.

### PNG and SVG

PNG and SVG produce one file per selected page. With several pages, the
output path needs a page placeholder: `{p}` is the one-based page number in
the document, `{0p}` (or `{n}`) the same zero-padded, and `{t}` the
document's page count.

```console
mkdir -p output
typst-pack compile project.typk "output/page-{0p}-of-{t}.png" \
  --pages 1-3 \
  --ppi 300
```

Every output path is checked for collisions before anything is written. The
output directory must exist.

### Reproducible output

Fix the document date and the PDF creation date with a Unix timestamp (or
`SOURCE_DATE_EPOCH`):

```console
typst-pack compile project.typk report.pdf --creation-timestamp 1700000000
```

Without a timestamp, `datetime.today()` is the local date, as in Typst.

### Other compile flags

`--pdf-standard`, `--no-pdf-tags`, `--pretty`, `--features`, `--jobs`,
`--diagnostic-format`, `--timings`, `--deps`, and `--open` behave as in
`typst compile`. `--deps` lists the pack and any input files as
dependencies.

## Fill a document with inputs

A pack never changes between compilations. Per-recipient data reaches the
document through `sys.inputs` instead:

- `--input KEY=VALUE` passes a string;
- `--input-file KEY=PATH` passes a file's bytes, which `image`, the data
  loaders such as `json`, and `str` accept directly.

One key may come from `--input` or `--input-file`, not both.

An `invoice.typ` that reads a logo, a customer record, and a locale:

```typ
#set text(lang: sys.inputs.at("locale", default: "en"))
#let customer = json(sys.inputs.customer)
#image(sys.inputs.logo, width: 2cm)

Invoice for #customer.name
```

`create` runs the document too, so pass sample values for inputs the document
requires. If the samples live beneath the project root, list them in
`.typkignore` so the pack does not contain them:

```console
typst-pack create invoice.typ \
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

Alternatively, give the document a fallback for an input it can do without,
so no sample is needed:

```typ
#let logo = sys.inputs.at("logo", default: none)
#if logo != none { image(logo, width: 2cm) }
```

## Extract a pack

Turn a pack back into an editable project directory:

```console
typst-pack extract project.typk
```

This writes the project files to `project/`; `-o` names another directory.
`--packages` also writes the vendored packages to `packages/`, `--fonts` the
embedded fonts to `fonts/`, and `--all` both. `extract` refuses to replace
existing files unless `--force` is given, and writes nothing when any path
conflicts.

## Use pipes

`-` reads a pack from stdin for `inspect`, `extract` (which then needs
`--output`), and `compile` (which then needs an output path). `create` and
`compile` write to stdout with `-` as the output path; `compile` does so only
for a single artifact.

```console
typst-pack create project/main.typ - | typst-pack inspect -
```
