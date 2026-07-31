# ADR-0011: Let Source Gatherers Select Project Membership

## Status

Accepted

## Context

Project Snapshot assembly reapplied one core Project Ignore Policy so an
over-inclusive adapter could not change membership. Generalizing that policy
for filesystem and object-storage sources would create a shared policy and
gathering interface before their selection and traversal behavior is known to
vary in the same way. Core could prevent over-inclusion but could never prove
that a source listing had not omitted an eligible file, and selection policy
does not survive in the Project Snapshot or contribute independently to Pack
Identity.

## Decision

Each source-specific project gatherer owns project membership selection,
traversal, pruning, listing, and content reads. The reference filesystem
gatherer applies the root `.typkignore`, prunes excluded directories before
descent, avoids reading excluded files, includes the exact root policy-file
bytes, and rejects unsupported eligible entries. An object-storage gatherer may
use different source-appropriate selection behavior.

No generic project-storage, gathering, or ignore-policy interface is introduced.
Source gatherers submit their exact selected path-and-byte entries to
`ProjectSnapshotAssembly`, which remains the one shared core seam. Assembly owns
canonical root-relative paths, duplicate rejection, the non-overridable `.typk`
exclusion, entrypoint presence, exact owned bytes, canonical ordering, and
snapshot budgets. It does not receive or reapply source-selection policy.

Source gatherers should complete their structural survey and reject detectable
selection failures before ordinary content reads. Their storage operations,
pagination, concurrency, acquisition limits, and operational errors remain
source-specific. A gatherer may cache bytes acquired while preparing selection,
such as root `.typkignore` bytes, and later move those exact bytes into the
selected entries without a second source read.

This decision amends ADR-0008's assignment of Project Ignore Policy matching to
the core and its guarantee that Project Snapshot membership is independent of
adapter correctness. Adapter-neutral Pack Creation still consumes one completed
Project Snapshot and remains unchanged.

## Consequences

- A source gatherer can use its storage system's most efficient native listing
  and pruning behavior without conforming to a lowest-common-denominator source
  interface.
- A buggy over-inclusive gatherer can produce a larger but valid Project
  Snapshot when every supplied entry satisfies universal snapshot invariants.
- Equivalent physical source trees are not guaranteed to produce equal Project
  Snapshots across gatherers unless those gatherers separately promise the same
  selection semantics.
- Source-selection configuration is not retained and affects Pack Identity only
  through the selected canonical paths and exact bytes.
- A shared gathering or policy seam may be introduced later if multiple real
  gatherers demonstrate identical reusable behavior.
