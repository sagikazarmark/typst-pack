# ADR-0021: API, Error, and Limits Conventions; Process Rules

## Status

Accepted

## Context

This ADR consolidates ADR-0012, ADR-0013, ADR-0014, ADR-0016, and ADR-0017. The
August 2026 architecture review (`docs/research/2026-08-architecture-review.md`)
traced most of the codebase's excess size to rules written as absolutes:
per-operation error vocabularies (about 120 error types), per-operation limits
families with no custom call sites, parallel per-adapter type hierarchies, and
specifications larger than the code they specified. The October 2026
simplification (`docs/research/2026-10-simplification.md`) applied the
conventions below. Earlier ADRs are in git history; this record does not
depend on them.

## Decision

**Public API.**

- Group the API in modules by concept (compile, archive, extract, fs, opendal,
  package, font) rather than prefixing type names.
- Adapters mirror each other: the same operation has the same name in `fs`
  and `opendal` (`read_archive`, `write_archive`, `write_extraction`,
  `write_artifacts`, `embed_packages`), takes its target first, then the
  data, then the policy, and takes `ReadLimits` by value. Core modules do no
  file or storage I/O.
- The library forbids `unsafe` code.
- A public type must be constructed or matched by a first-party caller or a
  doc example. Otherwise it is private or deleted.
- Plain data has public fields and `Default` where sensible; private fields
  with getters only protect an invariant.
- Successful operations return plain values, not reports wrapping results.

**Errors.**

- One error type per public operation or adapter, derived with `thiserror`.
  Variants carry operation-specific facts that a caller would branch on or a
  user needs to read; no label enums that only name a phase or an operation.
- An aggregate error's `Display` lists every issue, in a deterministic order.
- Foreign errors (I/O, storage, caller-supplied resolvers) cross the boundary
  as `#[source]`, boxed if the caller supplies the type, with the chain intact.
  A dependency's error type that is not part of the crate's chosen API (ZIP,
  TOML) is an opaque boxed source, never a public variant field or `From`
  impl.
- An error with a cause exposes it through `source()` and does not
  interpolate it in its `Display`; the `Display` says what failed. A wrapper
  that adds nothing is `#[error(transparent)]`. Whoever prints an error for a
  person prints the whole chain, `error: source: source`, as the CLI does; a
  library that records an error as text (a package failure reason) records
  the chain too.
- Error, issue, and outcome enums are `#[non_exhaustive]`, so adding a case
  is not a breaking change.

**Limits.**

- Explicit limits exist only where an operation reads bytes it does not
  control: archive decoding, package archive expansion, and source reading
  from a filesystem or storage. Compilation bounds its generated output.
- Limits are plain structs with public fields and a `Default`; an exceeded
  limit is one `LimitExceeded` naming the resource and the limit.
- Limits are operational policy: they never contribute to an identity, and the
  same input may succeed under larger limits. They bound documented retained
  or generated resources, not process memory or time.

**Abstractions.** Shared data shapes across operations and adapters are fine.
Shared behavioural traits for storage (source, sink, reader, writer,
conformance) are not, and no trait exists only for polymorphism between two or
three private callers. Test code may share helpers freely.

**Process.**

- Specs and ADRs describe behaviour and guarantees, never type declarations.
- A spec larger than the code it specifies is a review trigger, and every
  spec-first change gets a review asking which of its types earn their keep.
- When a decision changes, rewrite the ADR that holds it and keep it
  self-contained; do not add an amending ADR chain. Git history keeps the old
  text.
- Tests protect user-visible behaviour: round trips, refusals, error kinds a
  caller branches on, differential parity, and property and fuzz tests. Tests
  that pin `Debug` strings, pointer identity, auto traits, or test
  infrastructure are not written.
- Policy checks belong in tests or CI, not in `build.rs`; the build script of a
  published crate never reads workspace files.

## Consequences

- The error and limits surface stays proportional to the number of operations,
  and adding an adapter adds one error type, not a family.
- A typed match on a caller-supplied error is a downcast.
- The ADR set stays small enough to read in one sitting.
