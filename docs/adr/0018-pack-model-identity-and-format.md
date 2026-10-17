# ADR-0018: Pack Model, Identity, and Archive Format

## Status

Accepted

## Context

This ADR consolidates the pack-model decisions of ADR-0002, ADR-0009, ADR-0010,
and the identity parts of ADR-0012, revised by the August 2026 architecture
review (`docs/research/2026-08-architecture-review.md`) and the October 2026
simplification (`docs/research/2026-10-simplification.md`). Earlier ADRs are
in git history; this record does not depend on them.

A pack must mean the same thing however it was produced (builder, creation, or
archive decoding) and however it is stored. Earlier designs let identity depend
on Rust `Hash` output and spread archive safety checks across a hand-rolled ZIP
layer, which made identities unportable and the format hard to describe.

## Decision

**The Pack owns whole-pack invariants.** A pack is one entrypoint, a set of
project files, package requirements, and font requirements. Every way of
obtaining a pack goes through one whole-pack validation: canonical paths, the
entrypoint is present, package trees and font containers agree with their
declarations, and every requirement is either embedded or external. Values
already validated by their own constructors (project snapshot, package tree,
font container) are not validated again.

**Identity is semantic, not representational.** Every identity is a SHA-256
digest written `sha256:<hex>` over a documented, domain-tagged,
length-prefixed preimage:

- file and font container: the exact bytes;
- package tree: every file path and file digest, in path order;
- pack: entrypoint, project files, packages (spec, tree digest, embedded) and
  font containers (digest, embedded), excluding metadata and every archive
  detail;
- compilation: `typst_pack::VERSION`, pack identity, the normalized request,
  and the digests of the external packages and fonts it was fulfilled with.

Limits, crate features, build profile, storage location, and host facts never
contribute. The preimages are specified in the README's "Pack format" section,
which is the normative reference; a preimage change bumps its domain tag.

**The archive is a self-verifying ZIP.** A `.typk` archive holds a
`typst-pack.toml` manifest, project files, embedded packages, and embedded
font containers. The manifest lists every file with its digest, one
`[[packages]]` list (spec, tree digest, `embedded`), and one `[[fonts]]` row
per container with its faces (family, style, weight) so a recipient can see
what an external font must provide. `typst-version`, `generator`, and
`[metadata]` are informational. Decoding verifies every member against the
manifest, refuses undeclared, missing, duplicate, unsafe, or special members,
bounds declared and inflated sizes, and then submits the result to the same
whole-pack validation. The ZIP library is trusted for container parsing; the
manifest digests, not ZIP-level cross-checks, establish integrity. Encoding is
deterministic for a given build.

**Versioning.** `format-version` changes with every incompatible format
change, and a reader rejects every version it does not implement. Unknown
manifest keys are ignored, so compatible additions need no new version. While
the crate is 0.x there is no legacy reader and no migration shim.

## Consequences

- Equal packs have equal identities across hosts, builds, and archive
  re-encodings; a compilation identity is a usable cache key across build
  configurations of one release.
- A pack's archive can be verified without compiling it.
- Supporting an older pack format means keeping an old typst-pack release; this
  is accepted while the format is pre-1.0.
