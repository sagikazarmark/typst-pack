# ADR-0002: Make Pack the Owner of Whole-Pack Invariants

## Status

Accepted

## Context

Pack Manifest parsing, Zip reconstruction, in-memory building, filesystem
packing, and Pack-backed compilation previously enforced overlapping subsets of
Pack validity. A successfully created Pack could still contain an invalid font
or require downstream entrypoint validation. Canonical path and declaration
conflict behavior also differed by construction adapter.

The Pack Manifest is still a useful separate module: it owns the versioned TOML
schema, defaults, unknown-field policy, package-spec syntax, and deterministic
serialization. It cannot establish agreement with bytes that are not present in
the manifest.

## Decision

The private `Pack::construct` interface is the authoritative whole-Pack
construction seam. The in-memory builder and TOML/Zip reader are its two
adapters. Filesystem packing delegates to the in-memory builder.

The Pack module owns:

- canonical, portable paths and coherent project and package file trees;
- entrypoint presence;
- agreement between vendored/unvendored package declarations and package bytes;
- font path, byte, face-index, and declaration consistency;
- immutable canonical Pack state; and
- rejection of ambiguous archive identities before reconstruction.

The Pack Manifest module owns only versioned declarative syntax. Its public
records are read-only and expose accessors. Generic deserialization and
`PackManifest::from_toml` use the same version dispatch and manifest-local
validation.

`PackBuildError` and `PackReadError` wrap the same `PackInvariantError` for
shared invariant failures. Adapter-specific Zip, I/O, encoding, and ingestion
failures remain specific to archive reading.

A constructed Pack retains parsed fonts and a canonical contained entrypoint.
Consequently, the private Pack-backed World does not revalidate Pack content.

## Consequences

- Every Pack instance is canonical, immutable, writable, and usable by the
  private Pack compilation adapter.
- Writing performs no second invariant pass.
- The numeric Pack format version remains 1, while its unstable manifest schema
  intentionally changes incompatibly: old `external-resources` and
  `packages.external` fields are rejected.
- Safe unknown top-level archive entries remain ignorable, while unsafe or
  ambiguous entries are rejected before role interpretation.
- Deleting the private construction seam would redistribute canonicalization
  and declaration/content agreement into both construction adapters.

Pack Override remains a separate compilation concept: it replaces contained
project content for one compilation and never mutates the Pack.
