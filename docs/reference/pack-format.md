# Pack format

This is the normative description of the `.typk` archive, its manifest, and
the identities typst-pack computes, as of format version 2. The
[library contract](library-contract.md) describes what the identities
guarantee.

## Layout

A pack is a ZIP archive, conventionally named `*.typk`, with this layout:

```text
typst-pack.toml                         manifest
project/<path>                          every project file, root-relative
packages/<ns>/<name>/<version>/<path>   files of embedded packages
fonts/<hex digest>.<ext>                embedded font containers
```

## Manifest

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
listed container is available to the document. `typst-version` and
`generator` name the producer and are informational (`Pack::typst_version`,
`Pack::generator`); `[metadata]` is descriptive only.

Faces are informational too: they let a recipient see which fonts an external
container provides. `index` is the face's index in the container, `family`
its typographic family name, `style` one of `normal`, `italic`, or `oblique`,
`weight` the CSS weight from 100 to 900 (400 is normal), and `stretch` the
width in per mille of the normal width, from 500 to 2000 (1000 is normal).
Readers are lenient with faces: `faces` reads as the entries that are tables,
every field is optional (defaulting to index 0, an empty family, `normal`,
400, and 1000), a value of the wrong type reads as the default, an unknown
style reads as `normal`, and out-of-range numbers are clamped. A producer that
does not know its Typst version omits `typst-version`.

Paths, specifications, and digests are written in canonical form, each
declared once: a path is root-relative with `/` separators and no `.`, `..`,
or empty segments, and a specification is written exactly as Typst prints it
(`@preview/cetz:0.3.4`, never `@preview/cetz:0.03.4`). An embedded font
container is stored as `fonts/<hex digest>.<ext>`, where `<ext>` follows from
its leading bytes: `otf` (`OTTO`), `ttc` (`ttcf`), `woff` (`wOFF`), `woff2`
(`wOF2`), and `ttf` otherwise.

## Digests

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
one byte, `0` or `1`. Each preimage starts with a domain tag. "By path" and "by
spec" order records byte-wise by the path or by the specification's text, so
`@preview/a:0.10.0` sorts before `@preview/a:0.9.0`.

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

## Compilation identity

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
            | str("local") int(seconds) int(UTC offset seconds)
            | str("date") int(year) int(month) int(day)
output      = str("pdf") pages count(standards) { str(standard) }
                identifier creator bool(tagged) timestamp bool(pretty)
            | str("png") pages int(bits of the f64 PPI) bool(render bleed)
            | str("svg") pages bool(render bleed) bool(pretty)
            | str("html") bool(pretty)
pages       = count(ranges) { count(first) int(last or -1 when open) }
identifier  = str("auto") | str("custom") str(identifier)
creator     = str("auto") | str("none") | str("custom") str(creator)
timestamp   = str("omit") | str("utc") int(seconds)
            | str("local") int(seconds) int(UTC offset seconds)
```

`implementation` is `typst_pack::VERSION`, for example `0.6.0 (Typst 0.15.1)`.
Values are normalized first: a repeated input key keeps its first position and
last value, HTML output adds the `html` feature, page ranges are sorted and
merged, PDF standards are sorted by name, automatic PDF tagging is resolved,
a fixed document time is reduced to its date, and the default PPI is filled
in. Metadata, limits, export workers, and
dependencies the pack does not require do not contribute.

## Verification

A reader decodes a pack only if every member matches the manifest: each
project file and embedded font container matches its digest, each embedded
package's files match its tree digest, and no member is missing or undeclared.
Every member name must be exactly the canonical name the encoder writes, so an
archive has one reading for every extractor: names that only normalize to a
member (`project/./main.typ`, `./typst-pack.toml`), duplicates, and a font
container under any name but the one its content implies are refused. A
manifest that declares a path, package, or font container twice, or a path or
specification in non-canonical form, is refused too. An embedded package tree
that holds a `typst.toml` must declare its specification's name and version.
Links and other special entries are refused; directory entries are ignored.
Declared sizes and actual inflated bytes are both bounded by
`archive::DecodeLimits`. External packages and fonts are matched by digest
when the pack is compiled.

The encoder is deterministic for a given build: fixed member order, timestamps,
permissions, and compression level.

## Compatibility

`format-version` changes with every incompatible format change, and readers
reject every version they do not implement. Readers ignore unknown manifest
keys, so a producer can add compatible fields without a new version.
