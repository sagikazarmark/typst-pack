# Typst Pack

This glossary names the domain concepts exposed by typst-pack. Operational and
implementation details belong in API documentation and architectural decisions.

## Packs

**Pack**:
A validated, portable Typst project with one entrypoint and a fixed set of
project files, package requirements, and font requirements. A Pack is
self-contained when every requirement is embedded. Its accessors and `inspect`
show its static contents without resolving dependencies or compiling it.

**Pack Creation**:
The adapter-neutral operation that combines a Project Snapshot, Package Catalog,
Font Catalog, and Dependency Discovery into a Pack. It may report package
specifications that the caller must add before trying creation again.

**Dependency Discovery**:
The representative Typst compilation that selects package and font requirements
for Pack Creation. Its Discovery Specification fixes the Typst Target, inputs,
Document Time, and engine features for that run; those values do not become Pack
state. Project membership always comes from the Project Snapshot.
_Avoid_: Creation Discovery, Precompilation

**Pack Assembly and Pack Assembler**:
Pack Assembly obtains a Project Snapshot, fonts, and requested Package Trees,
then repeats Pack Creation until it produces a Pack or fails. A Pack Assembler
performs that workflow for configured sources and policies.
_Avoid_: acquisition, gathering, gatherer, Creation Preparation, Creation Adapter

**Project Snapshot**:
One source-selected set of canonical root-relative project paths and exact bytes,
including its entrypoint. Snapshot assembly rejects invalid or duplicate paths
and every path containing a `.typk` component.

**Project Ignore Policy**:
The filesystem reader's root `.typkignore` rules for selecting project files.
Rules use Gitignore-style ordering, negation, and last-match precedence. The root
policy file is included; nested `.typkignore` files are ordinary project files.

**Pack Identity**:
The content identity of a Pack's entrypoint, project files, ordered fonts, exact
dependency requirements, and each requirement's embedded or external state.
Request values, metadata, archive encoding, and host facts do not contribute.

**Canonical Identity**:
The shared identity convention for Packs, package trees, font containers,
compilations, and results. Equality includes role, schema, algorithm, and digest;
a bare digest is insufficient.
_Avoid_: bare hash, checksum

**Pack Manifest**:
The versioned declaration inside a Pack Archive. It records the entrypoint,
requirements, and optional metadata; Pack validation verifies those declarations
against the contained bytes instead of trusting the manifest alone.

**Pack Extraction**:
The planned projection of selected Pack project files, packages, and fonts into
an editable tree. Planning checks paths and collisions before a destination
adapter writes the plan. Extraction is not archive decoding and does not preserve
the original Pack Identity.

**Pack Archive**:
The interoperable byte representation of a Pack, conventionally stored as a
`.typk` file. Archive layout and encoding do not contribute to Pack Identity; a
decoded archive becomes a Pack only after whole-Pack validation.

**Pack Archive Reading**:
The bounded operation that reads exact Pack Archive bytes from a stream, file, or
storage service. Reading does not itself decode or validate those bytes; composed
helpers such as `read_pack` and `open_pack` do both operations.
_Avoid_: Pack Archive Acquisition, acquire

**Pack Archive Writing**:
The operation that writes exact Pack Archive bytes to a stream, file, or storage
service under an explicit policy. Writing does not define archive encoding;
composed helpers such as `write_pack` and `save_pack` do both operations.
_Avoid_: Pack Archive Publication, publish

## Packages And Fonts

**Package Catalog, Package Tree, and Package Requirement**:
A Package Catalog maps exact package specifications to complete Package Trees and
their embedded or external state. Pack Creation selects Package Requirements that
bind a specification, tree identity, and state. A Package Tree contains every
addressable regular file beneath one package root.

**Package Registry**:
The remote source addressed by one URL per exact package specification. The core
constructs URLs for the official Typst Universe namespace; adapters choose and
perform any transport.

**Package Archive Expansion**:
The bounded transformation from package archive bytes into a complete Package
Tree. It rejects unsafe or malformed members and requires explicit limits because
the input crosses a trust boundary.

**Fulfillment**:
Supplying exact Package Trees and Font Containers for a Pack's external
requirements. Compilation verifies complete identities and declared font faces
before exposing fulfilled values to Typst.

**Font Container, Font Catalog, and Font Requirement**:
A Font Container is one exact font file or multi-face collection. A Font Catalog
is an ordered sequence of containers and their embedded or external state. Pack
Creation selects Font Requirements that bind container identity and used faces.

## Compilation

**Pack Override**:
A compilation-scoped replacement for one project file already contained in a
Pack. Overrides cannot add or delete paths or replace package or font content.

**Typst Target**:
The Typst document model, Paged or HTML. Dependency Discovery selects one target;
each compilation output request implies the target it needs.

**Output Formats**:
PDF and HTML produce one document artifact. PNG and SVG produce one artifact per
selected source page, ordered by its one-based Source Page Number. Page numbers
identify positions in the source document, not output collection indexes.

**Document Time**:
The explicit value used for Typst document-time requests: absent, a fixed Typst
date or datetime, or a Unix timestamp interpreted for requested timezone offsets.
It contributes to Compilation Identity even when document code does not read it.

**Implementation Identity**:
The exact embedded Typst engine or exporter implementation attested by typst-pack,
including its role, version, source checksum, target, crate features, and build
mode. Engine and exporter roles are distinct identities.

**Compilation Result and Output Artifact**:
A Compilation Result records accepted compilation status, diagnostics, warnings,
identities, and ordered artifacts. Each output artifact owns exact bytes and a
format role; page-format artifacts also carry their Source Page Number.

**Environment-Independent Compilation**:
Once the Pack, compilation request, implementation identities, and fulfilled
dependency bytes are fixed, compilation does not consult ambient files, package
paths, fonts, environment variables, wall-clock time, or the network.

## Conventions

Public failures use three suffixes: **Error** for a terminal typed failure,
**Rejection** for a complete deterministic refusal before semantic acceptance,
and **Issue** for one independently detectable fact in an ordered aggregate.
These are API naming conventions, not domain concepts.
