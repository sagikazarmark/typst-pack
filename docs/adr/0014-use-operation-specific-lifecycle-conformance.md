# ADR-0014: Use Operation-Specific Lifecycle Conformance

## Status

Accepted, amended by ADR-0016

## Context

The Pack lifecycle spans semantic construction, a versioned archive format,
embedded compilation, source-specific gathering, and destination adapters with
different policies and guarantees. These modules need reusable conformance
evidence, including for future OpenDAL adapters, without introducing generic
storage or lifecycle interfaces solely to make tests shareable.

Testing only private stages would couple evidence to implementation structure.
Testing only end-to-end workflows would make exhaustive invariant, limit,
ordering, ownership, and failure-path coverage impractical. Golden snapshots
alone would also conflate semantic compatibility with exact representation and
rendered error text.

## Decision

Each deep module's public interface is its primary contract and test surface.
Public contract suites assert typed values, identities, ordered accessors,
errors, outcomes, reports, receipts, progress, retry material, exact promised
bytes, and destination state. Private stage tests are limited to exhaustive
invariants, allocation-sharing evidence, impossible public states, phase-entry
probes, and deterministic fault injection.

Cross-adapter reuse consists of declarative operation-specific scenario records,
public observation projections, and adapter-specific test runners. Test code does
not introduce a production conformance trait, generic storage interface, shared
policy implementation, or lifecycle-wide result model. Filesystem and OpenDAL
adapters run the scenarios applicable to their own contracts; they are not
required to share source-selection, path, atomicity, progress, or capability
semantics.

Evidence uses four complementary forms:

- checked-in golden fixtures for Pack Archive interoperability, malformed
  external inputs, fonts, and stable identity vectors;
- generated table cases for combinatorial rules, phase precedence, policies,
  aggregation, ordering, and every finite-limit boundary;
- property tests for canonicalization, permutation invariance, identities,
  determinism, and semantic encode/decode relations; and
- fuzz targets for every untrusted parser, decompressor, and structured semantic
  constructor, with minimized regressions replayed in CI.

The reference filesystem contract runs natively on Linux, Windows, and macOS.
The featureless core is compiled for `wasm32-unknown-unknown`. Official Typst
differential suites and bounded fuzz-regression replay are required CI evidence;
long-running mutation fuzzing remains outside the normal merge gate.

Failure injection is private and operation-specific. Scripted readers, writers,
filesystem stages, exporters, and OpenDAL services produce deterministic faults,
races, operation logs, and completion-order permutations. Tests do not depend on
sleep timing, ambient permissions, native error messages, process RSS, or random
chaos as their primary proof.

## Consequences

- Tests exercise the same small interfaces callers use and preserve module
  depth.
- A future adapter can demonstrate the shared semantic observations that apply
  to it without pretending filesystem and object-storage behavior are equal.
- Corpus records and observation projections are test assets, not new domain
  values or production interfaces.
- Pack equality is semantic; independently produced archives need not round-trip
  byte-for-byte. Selected canonical encodings may still have exact goldens.
- Native platform jobs are required for filesystem guarantees that lexical tests
  cannot prove.
- Allocation tests prove payload-sharing shape rather than allocator-wide counts
  or process-memory ceilings.
- Existing tests that encode superseded policies, representation-coupled Pack
  invariants, source revalidation, or fail-fast aggregates must be migrated or
  removed rather than preserved behind compatibility helpers.
