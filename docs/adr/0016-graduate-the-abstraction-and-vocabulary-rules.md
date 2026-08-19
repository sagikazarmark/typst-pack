# ADR-0016: Graduate the Abstraction and Vocabulary Rules

## Status

Accepted

Amends ADR-0013, ADR-0014, and ADR-0015.

## Context

ADR-0013 gave every operation its own failure vocabulary, ADR-0014 forbade
shared conformance machinery, and ADR-0015 forbade a universal storage
abstraction. Each rule was written to prevent a leaky god-abstraction, but
each was written as absolute, and the August 2026 architecture review
(docs/research/2026-08-architecture-review.md) measured the cost of that
absoluteness: ~120 error types (about six per callable operation), 85
hand-written `Debug`/`Display`/`Error` impls forced by resolver-generic error
types, internal error families existing only to be remapped arm by arm into
public ones, two identically named `workflow_evidence!` macros generating
incompatible same-named types in the filesystem and OpenDAL adapters, and
success-path evidence accessors returning compile-time constants. The
specification process compounded it: a normative spec containing full type
declarations grew larger than the implementation and required its own
ratification workflow.

The hazard the original rules target is real. A production trait that every
backend must implement leaks the weakest backend's semantics into every
caller's API. But shared data shapes have no implementors and no polymorphic
call sites; the rationale never reached them.

## Decision

The abstraction ban is graduated, not absolute:

- **Shared data shapes are allowed** across operations and adapters: limits
  and resource vocabularies, receipt/progress/entry shapes, error cores, and
  other types with no implementors and no polymorphic production call sites.
- **Shared behavioral traits remain forbidden** in production code: no
  universal storage, source, sink, gatherer, authority, scheduler, or
  conformance trait.
- **Test code is exempt from the ban entirely.** Test-only scaffolding cannot
  leak into the public API, so cross-adapter test helpers may be shared.
  ADR-0014's conformance shape — declarative scenario records, public-surface
  projections, and per-adapter runners with no production conformance trait —
  remains in force.

The failure vocabulary is reduced to the three words that carry real
distinctions: **Error** (a terminal typed failure), **Rejection** (a complete
deterministic refusal before semantic acceptance), and **Issue** (one
independently detectable fact, aggregated in canonical order). Receipt,
Outcome, and Report are no longer mandated categories; a successful operation
returns a plain value. Progress reporting survives only where a genuine
partial-effect story exists (multi-key publication), as one shared type.
Commit certainty is a field on the relevant error, not a concept family.

Adapter error types are not generic over caller-supplied resolver or
transport errors. The foreign error crosses the boundary as
`Box<dyn Error + Send + Sync>` with the `source()` chain intact; a caller
that needs its concrete type downcasts to the type it itself supplied.
Internal error types that exist only to be remapped into public ones are
removed; the surviving error carries the operation identity itself.

Specification process: specs describe behavior and guarantees, never type
declarations. Every spec-first implementation gets a code-level review pass
asking which of the resulting types earn their existence. A specification
growing larger than the code it specifies is itself a review trigger. No
specification receives a ratification workflow.

## Consequences

- One limits family, one receipt/progress shape, and one error core may be
  shared by the filesystem and OpenDAL adapters; the duplicate
  `workflow_evidence!` macro and the parallel per-adapter vocabularies are
  removed.
- `thiserror` derives replace hand-written error impls; typed matching on a
  caller-supplied resolver error becomes a downcast, which is the one API
  regression this ADR accepts.
- Per-operation error types remain only where they carry operation-specific
  facts, so the error surface shrinks toward one or two types per operation.
- ADR-0013's canonical-order aggregation of Issues and ADR-0015's
  crate-placement, feature, and dependency decisions are unchanged.
