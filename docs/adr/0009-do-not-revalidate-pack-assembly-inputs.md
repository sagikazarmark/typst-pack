# ADR-0009: Do Not Revalidate Pack Assembly Inputs

## Status

Accepted

## Context

The reference filesystem Pack Assembler re-read project, package, and font
inputs after Pack Creation to detect concurrent source mutation. This Creation
Evidence Fence doubled access to selected inputs but could not provide an atomic
snapshot or prove that all acquired values coexisted at one instant.

## Decision

A Pack represents the exact Project Snapshot, Package Catalog, and Font Catalog
values supplied to Pack Creation. Pack Assembly does not re-read those sources
solely to detect changes between acquisition steps. Mutation before a value is
acquired may affect that value; mutation afterward does not, and concurrent
source mutation is outside Pack Assembly's guarantees. Canonicalization,
selection, and whole-Pack validation of the acquired values remain unchanged.

This decision supersedes the Creation Evidence Fence and consistent-source-state
obligations in ADR-0006 and ADR-0008. Their adapter-neutral creation decisions
remain in force.

## Consequences

- The target filesystem Pack Assembler no longer performs project, package, or
  font revalidation solely for mutation detection.
- A valid Pack may combine values that did not coexist in its mutable source.
- Pack validity and Pack Identity continue to describe acquired values, never
  host filesystem history.
