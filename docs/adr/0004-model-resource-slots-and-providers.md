# ADR-0004: Model Resource Slots and Resource Providers

## Status

Superseded by ADR-0006

## Context

The terms External Project Resource and External Resource Reference exposed an
abstract implementation model without explaining the user workflow. In
particular, `--source-reference` suggested Typst source lookup even though the
mechanism deliberately rejects source requests. The underlying Pack contract is
simpler: it declares an exact project location whose non-source bytes are left
for a compilation to supply.

This decision supersedes ADR-0003. Its centralized resolution seam and most of
its authority and fallback rules remain, but its public terminology and
discovery policy do not.

## Decision

A **Resource Slot** is an exact, normalized, project-root-relative location
declared by a Pack for non-source bytes supplied to a compilation rather than
stored in the Pack. A slot may remain unfilled when a compilation does not
request it. Slot declarations carry no required or optional status; an actual
request determines whether bytes are required for that compilation.

A **Resource Provider** is an opaque, path-addressable compilation input that
may supply bytes for one or more Resource Slots. Providers form one ordered
fallback chain. Only a missing resource advances to the next provider; success
or any other error terminates lookup. Providers cannot supply Typst source,
package files, undeclared project paths, or replacements for contained Pack
files. The source restriction follows the Typst request kind rather than the
path extension.

The filesystem CLI names the concrete value it accepts. Repeated
`--resource-path <DIR>` options add project-shaped resource roots in command-line
order, so slot `branding/logo.svg` is looked up as
`<DIR>/branding/logo.svg`. The explicit declaration vocabulary is
`--resource-slot <PATH>`, `[project].resource-slots`, and `resource_slot` in
APIs. Generic APIs accept Resource Providers rather than source references.

Pack creation remains strict and compile-driven. It does not gain partial,
dummy, or non-discovery modes:

- a same-path source-project file is the first representative value for an
  explicitly declared slot, but its bytes are omitted from the Pack;
- Resource Providers are consulted after the source project;
- registering a provider enables missing non-source requests that it satisfies
  to be inferred as Resource Slots;
- explicit declarations cover slots not exercised by representative discovery
  and present project files that must be omitted;
- an unrequested slot needs no representative value; and
- a requested slot that no provider supplies fails creation with a diagnostic
  explaining how to add a representative project file or resource path.

Representative bytes must be valid for the document and sufficiently
representative of its dependency behavior. Arbitrary placeholders cannot be
synthesized safely because images, structured data, and raw files may affect
layout, control flow, and further dependency discovery.

Resource Providers only fill Resource Slots. A compilation-scoped replacement
for a contained project file remains a separate future **Pack Override**
concept; vendored package files and embedded fonts are not override targets.

## Considered Options

- **External Project Resource** was technically neutral but did not identify
  what was external or explain that the Pack carries a path without bytes.
- **Late-bound**, **deferred**, and **compile-time resource** emphasized timing
  without distinguishing slots from ordinary project resources.
- **Runtime resource**, **include path**, **linked resource**, **mount**, and
  **overlay** implied unsupported execution, source, reference, or replacement
  semantics.
- **File-valued parameter** explained variation but conflicted with Typst's
  `--input` vocabulary and suggested eager argument binding.
- **Externalized project file** explained storage but implied prior bytes and
  understated the path's role as a compilation variation point.
- Partial discovery and generated dummy values were rejected because they can
  silently omit dependencies. Non-discovery creation was rejected because it
  gives up successful representative compilation and automatic package and font
  discovery.

## Consequences

- Resource Slot declarations and Pack files remain disjoint Pack roles.
- A Pack with Resource Slots is portable but not self-contained; reproducing a
  compilation also requires equivalent provider contents and ordering.
- Availability validation is demand-driven. Structural slot validation remains
  eager, but unrequested slots are neither resolved nor warned about.
- Extraction does not materialize empty placeholder files for Resource Slots;
  it lists the omitted slot paths instead.
- The filesystem CLI aligns with Typst's `--font-path` and `--package-path`
  vocabulary without leaking the generic provider abstraction.
- The manifest, CLI, Rust API, Dagger API, documentation, diagnostics, and tests
  require a coordinated breaking terminology migration.
- Pack Manifest format version 1 is still unstable. Its schema changes in place
  to use `resource-slots` and `packages.unvendored`; Packs written with the old
  version-1 field names need not remain readable, and no field aliases are kept.
- The private centralized resolver retained from ADR-0003 continues to own Pack
  authority, slot eligibility, source/package exclusion, provider ordering,
  missing-only fallback, canonical missing identity, and discovery provenance.
