# Typst Pack

Domain terms used across typst-pack. Decisions live in `docs/adr/`; exact
formats and guarantees live in the README and `docs/library-contract.md`.

**Pack**:
A validated, portable Typst project: one entrypoint, its project files, and the
packages and font containers it requires. A pack is self-contained when every
requirement is embedded.

**Pack archive (`.typk`)**:
The ZIP representation of a pack. Its layout and encoding do not affect the
pack's identity; decoded bytes become a pack only after verification.

**Manifest**:
`typst-pack.toml` inside the archive. Declares the format version, entrypoint,
every file with its digest, the packages, and the font containers with their
faces. Readers verify it against the archive's members.

**Project files and entrypoint**:
Every regular file beneath the project root that the ignore file does not
exclude, with the root-relative path of the main Typst file. Membership is
structural; compilation never selects project files.

**Ignore file (`.typkignore`)**:
Gitignore-style rules at the project root that exclude files from the pack.
Nested `.typkignore` files are ordinary project files.

**Package**:
A Typst package identified by its spec (`@namespace/name:version`). A pack
stores its complete file tree (the package tree) or requires it externally.

**Font container and face**:
A container is one font file or collection, identified by its digest; a face is
one font inside it (family, style, weight). Requirements are whole containers:
every face of a required container is available at compile time.

**Embedded and external**:
An embedded package or font is stored in the pack. An external one is recorded
by digest and supplied by the caller at compile time.

**Fulfillment**:
Supplying the external packages and fonts a pack requires. Only a supplied tree
or container with the required digest is used; anything missing is reported by
package spec or font family.

**Discovery compile**:
The compile that pack creation runs to learn which packages and fonts the
document uses. Its target, inputs, document time, and features do not become
pack state. Creation may stop with the missing package specs, to be resumed
once the caller supplies them.

**Variant**:
An extra set of inputs for discovery. Creation compiles each variant too and
requires the union of what any compile used.

**Input**:
A `sys.inputs` value supplied per compilation: a string, or bytes read from a
file. Inputs never change a pack's files or requirements.

**Target**:
Typst's document model, paged or HTML. Discovery uses one target; each output
format implies the target it needs.

**Output**:
A compiled artifact. PDF and HTML produce one document; PNG and SVG produce one
artifact per selected page, numbered by its page in the source document.

**Document time**:
The explicit value used for Typst's `datetime.today()`: absent, a fixed date,
or a Unix timestamp read in UTC or at a local UTC offset. Compilation never
reads the wall clock; the filesystem packer and the CLI capture the host's
local time when no timestamp is given, as Typst does.

**Digest, pack identity, compilation identity**:
A digest is SHA-256 written `sha256:<hex>`. The pack identity digests the
entrypoint, project files, and requirements (not metadata or archive layout);
the compilation identity adds the typst-pack and Typst versions, the request,
and the external dependencies used.

**Extraction**:
Writing a pack's project files, and optionally its embedded packages and fonts,
back into an editable directory tree. Each extracted file is a project file, a
package file, or a font container.
