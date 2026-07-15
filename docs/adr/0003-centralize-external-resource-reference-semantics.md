# ADR-0003: Centralize External Resource Reference Semantics

## Status

Accepted

## Context

Compilation and discovery had separate implementations of External Resource
Reference behavior. A small shared fallback loop preserved registration order,
but authority, declaration and policy gating, source/package exclusion,
canonical missing identity, and discovery provenance remained distributed.
Changes could therefore weaken one adapter without changing the other.

Pack and Pack Manifest structural rules are a different concern. They establish
which External Project Resources are declared and ensure contained data does not
conflict with those declarations.

## Decision

The private `external_resource` module owns runtime and discovery semantics. It
exposes two crate-private configured contexts:

- `Compilation`, used by the Pack-backed `FileLoader` adapter; and
- `Discovery`, used by the source-project `FileLoader` adapter.

Both contexts consume ordered opaque `FileLoader` adapters as External Resource
References. No public trait or public resolution module is introduced.

The shared implementation owns these invariants:

- only project-root raw-file requests are eligible;
- authoritative Pack or source-project content is consulted first;
- compilation fallback requires an accepted Pack declaration;
- discovery fallback requires explicit policy enablement;
- Typst source and package requests never consult a reference;
- references are lazy and tried in registration order;
- only `FileError::NotFound` advances the fallback chain;
- the first success or any non-missing error stops resolution;
- all-missing resolution reports the canonical requested project path; and
- non-missing adapter errors are propagated unchanged.

Discovery provenance starts with explicit declarations. Inferred provenance is
added only after successful fallback, is stored in deterministic set order, and
is updated under synchronization. No provenance lock is held while invoking an
External Resource Reference. Source and raw-file stores remain isolated while
sharing the authoritative primary cache.

The public Rust registration method is
`external_resource_reference`. The CLI accepts repeated `--source-reference`
options. The former loader-oriented Rust names and `--resource-path` option are
removed without aliases as part of the 0.4 break.

## Consequences

- Compilation and discovery retain distinct Typst adapters while sharing one
  deep policy implementation.
- Pack and Pack Manifest retain structural declaration ownership.
- The module stays usable without filesystem support; filesystem references are
  CLI adapters, not dependencies of the core implementation.
- External Resource References are compilation inputs and are not serialized.
- Compilation Output Artifact behavior established by ADR-0001 is unchanged.
- Deleting this module would redistribute authority, gating, ordering, error,
  path-identity, source/package exclusion, and provenance rules across both
  adapters.

Pack Override remains conceptually separate. It deliberately replaces a file
contained in a Pack for one compilation; an External Resource Reference may
only supply a declared External Project Resource.
