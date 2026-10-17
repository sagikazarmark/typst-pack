# ADR-0019: Create Packs by a Discovery Compile

## Status

Accepted

## Context

This ADR consolidates the creation decisions of ADR-0003, ADR-0004, ADR-0006,
ADR-0008, and ADR-0011, revised by the August 2026 architecture review and the
October 2026 simplification (`docs/research/2026-10-simplification.md`).
Earlier ADRs, including the Resource Slot and Pack Override designs, are in git
history; this record does not depend on them.

Typst offers no way to list the packages and fonts a document may use. The
only reliable source is the set it actually requests while compiling. Project
files, by contrast, can be enumerated without compiling.

## Decision

**Project membership is structural.** The project is every regular file beneath
the project root, minus what the root `.typkignore` excludes (gitignore
semantics; nested `.typkignore` files are ordinary files; `.typk` paths are
always excluded). The source reader selects membership; compilation never adds
or removes project files. The root and entrypoint follow Typst: the
entrypoint's directory by default, or an ancestor chosen by `--root` /
`TYPST_ROOT`.

**Dependencies come from a discovery compile.** Creation compiles the project
for one target (paged by default, or HTML) with the caller's inputs, document
time, and features. The packages and font containers that compile requests
become the pack's requirements. Discovery settings do not become pack state; a
pack compiles later for any target and request.

**Variants widen discovery.** A creation may name variants, each a set of input
overrides. Creation compiles the base options and then every variant, and the
pack requires the union of what any of them used. This covers documents whose
imports or fonts depend on inputs, without persisting any discovery trace.

**Creation is pure and resumable.** The core creation operation reads nothing:
the caller supplies the project snapshot, package trees, font containers, and
recorded package read failures. When discovery reaches a package that was not
supplied, creation returns the missing package specs; the caller fetches them
and calls creation again. Source-specific reading and the retry loop live in
assemblers (filesystem, OpenDAL), not in the core.

**Inputs replace overrides.** A pack's project files never change between
compilations. Per-recipient data is passed as `sys.inputs`: strings, or bytes
(`--input-file KEY=PATH` in the CLI) that Typst's `image`, data loaders, and
`str` accept directly. Inputs never add project paths or change requirements.

**Fonts are whole containers.** A required font container contributes every
one of its faces at compile time, so text the discovery compile did not render
falls back the same way it would in Typst.

**Embedded or external, fulfilled by digest.** Each package and font container
is either embedded in the pack or external. Packages are embedded by default;
fonts are external unless the creator embeds them. At compile time the caller
supplies external dependencies, and each requirement uses only the supplied
tree or container with the matching digest. A missing or mismatched one is an
unfulfilled requirement naming the package spec or the font families; it never
reaches Typst. Compilation is hermetic: once the request is fixed it reads no
ambient files, package caches, fonts, environment, clock, or network.

## Consequences

- Dependencies reached only under inputs or targets that creation did not try
  are not in the pack. Variants cover inputs; a document whose dependencies
  differ by target is created for the target it will be compiled for.
- Creation runs anywhere the core runs (including wasm), and any storage can
  supply its inputs.
- A recipient can verify an external dependency by digest, and a compilation
  identity names exactly what was compiled.
