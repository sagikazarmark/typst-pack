# ADR-0020: Library, CLI, and Adapters

## Status

Accepted

## Context

This ADR consolidates ADR-0005, ADR-0007, ADR-0008, and ADR-0015, and replaces
the filesystem confinement analysis in
`docs/research/platform-confinement-guarantees.md`, as revised by the October
2026 simplification (`docs/research/2026-10-simplification.md`). Earlier ADRs
are in git history (last at tag `v0.5.0`); this record does not depend on them.

typst-pack is used as a Rust library and as a CLI that should feel like
`typst`. Hosts differ in what they can read from: local files, object
storage, or bytes already in memory.

## Decision

**Two crates, one release.** `crates/typst-pack` is the library;
`crates/typst-pack-cli` is the `typst-pack` executable, depends on the same
workspace version, and uses only the public library API. The library has no
`cli` feature.

**An adapter-neutral core.** Pack construction, creation, compilation, archive
encoding and decoding, extraction planning, and package archive expansion work
on bytes and values, with no filesystem, network, or clock. With default
features off, the core builds for `wasm32-unknown-unknown`. Compilation and
creation are synchronous; adapters choose blocking or async I/O around them.

**Adapters live in the library, behind features.**

- `fs`: reads projects, package directories, and system fonts; writes packs,
  compiled artifacts, and extracted trees.
- `opendal`: a thin layer over a caller-supplied `&Operator` and paths, with
  one adapter error (which writes wrap with their recovery state) and, with
  `package-reading`, an assembler. Services, transports, runtimes, and retries
  stay the caller's choice.
- `egress`: downloads packages from Typst Universe. It is the only feature that
  links an HTTP client, so network isolation of a build is verifiable from its
  dependency graph.
- `package-reading` (transport-free package archive expansion), `parallel`,
  `diagnostics`, and `embedded-fonts` add capabilities without I/O.

Adapters share plain data shapes (limits, errors' common facts) but not
behavioural traits: there is no universal storage, source, or sink trait.

**Filesystem guarantees are proportionate.** The filesystem adapter is
validation, not confinement, at the level of cargo, uv, or git. Project and
package reading is rooted (cap-std): symbolic links resolving inside the root
are followed, links escaping it and special files are refused. Font
directories are host configuration: their links are followed wherever they
lead, and a font that cannot be read is skipped and reported rather than
failing the read. Writing goes
through a temporary sibling and a rename (overwrite) or a hard link with an
exclusive-create fallback (create-new). Writing an extraction plan first checks
portable names, case collisions, conflicts, and linked ancestors under the
destination, and writes nothing if any check fails. No claim is made against a
concurrent filesystem peer or about crash durability; a deployment that needs
isolation from hostile input provides it outside the process.

**The CLI tracks the embedded Typst release.** Shared flags, environment
variables, parsing, and defaults follow `typst compile` of the exact embedded
version, and help follows its wording, shortened in places;
`typst-pack --version` reports both versions. Pack-specific behaviour
(no compile-time `--root`, host configuration that only supplies exact
external requirements, `--offline`,
`--no-vendor-packages`, `--input-file`, `--variant`, `.typkignore`) is an
explicit, documented difference. The flag inventory and every difference live
in `docs/reference/cli-parity.md`; the release pin and the upgrade procedure
live in `crates/typst-pack/embedded-typst.toml` and
`docs/development/embedded-typst-upgrade.md`.
A differential suite runs the official Typst CLI of that release against
typst-pack.

## Consequences

- New storage backends are new adapters over the same core, not new
  implementations of a storage trait.
- A build without `egress` cannot download, whatever its runtime configuration.
- Typst users can rely on existing knowledge; each Typst upgrade includes a
  CLI parity review.
- Hardening against a hostile local filesystem is out of scope.
