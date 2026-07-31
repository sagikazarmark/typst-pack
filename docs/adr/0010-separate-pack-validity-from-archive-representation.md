# ADR-0010: Separate Pack Validity from Archive Representation

## Status

Accepted

## Context

ADR-0002 made Pack construction authoritative for whole-Pack invariants, but it
also made every valid Pack writable and assigned rejection of ambiguous archive
identities to Pack construction. Those representation promises couple Pack
validity to the current Zip-based `.typk` encoding even though archive layout and
encoding do not contribute to Pack Identity.

Pack Archive Decoding must still reject malformed or ambiguous representations,
and every decoded value must still pass the same whole-Pack validation as an
in-memory value. The seam needs to preserve that single semantic authority
without making raw archive mechanics part of it.

## Decision

Pack construction remains the authoritative whole-Pack construction seam. It
owns canonical domain paths, coherent project and package trees, entrypoint
presence, declaration and content agreement, font consistency, identities, and
immutable canonical Pack state.

The Pack Archive format module owns the versioned Pack Manifest syntax, Zip
layout, malformed or ambiguous raw archive members, ingestion limits, and
encoding limits. Decoding interprets accepted archive members as domain
declarations and content, then submits them to Pack construction. Encoding
derives its representation records from canonical Pack state; Pack does not
retain a Pack Manifest as its canonical state.

A valid Pack is not guaranteed to satisfy every limit of the current archive
representation. Pack Archive Encoding reports representation-specific failures
without weakening Pack validity or repeating whole-Pack validation.

This decision amends ADR-0002's assignment of ambiguous archive identities to
Pack construction and its consequence that every Pack is writable. ADR-0002's
single whole-Pack construction seam and all semantic invariants remain in force.

## Consequences

- In-memory construction and archive decoding converge on one semantic
  validation path without duplicating decoded path or role-conflict rules.
- Raw archive safety and ambiguity failures remain distinct from Pack invariant
  failures.
- Pack inspection exposes domain values rather than versioned Manifest records.
- A future representation can encode the same Pack without changing Pack
  Identity or Pack validity.
- Pack Archive Encoding must expose representation failures even when its input
  is a valid Pack.
