# ADR-0011: Let Source Readers Select Project Membership

## Status

Accepted, amended by ADR-0012

## Context

Project Snapshot assembly reapplied one core Project Ignore Policy so an
over-inclusive adapter could not change membership. Generalizing that policy
for filesystem and object-storage sources would create a shared policy and
reading interface before their selection and traversal behavior is known to
vary in the same way. Core could prevent over-inclusion but could never prove
that a source listing had not omitted an eligible file, and selection policy
does not survive in the Project Snapshot or contribute independently to Pack
Identity.

## Decision

Each source-specific project reader owns project membership selection,
traversal, pruning, listing, and content reads. The reference filesystem
reader applies the root `.typkignore`, prunes excluded directories before
descent, avoids reading excluded files, includes the exact root policy-file
bytes, and rejects unsupported eligible entries. An object-storage reader may
use different source-appropriate selection behavior.

No generic project-storage, reading, or ignore-policy interface is introduced.
Source readers submit their exact selected path-and-byte entries to
`ProjectSnapshotAssembly`, which remains the one shared core seam. Assembly owns
canonical root-relative paths, duplicate rejection, the non-overridable `.typk`
exclusion, entrypoint presence, exact owned bytes, and canonical ordering. It
does not receive or reapply source-selection policy.

Source readers should complete their structural survey and reject detectable
selection failures before ordinary content reads. Their storage operations,
pagination, concurrency, read limits, and operational errors remain
source-specific. A reader may cache bytes read while preparing selection,
such as root `.typkignore` bytes, and later move those exact bytes into the
selected entries without a second source read.

ADR-0012 removes the separate Project Snapshot assembly budget. Source-specific
readers still bound traversal and reads before handing already-owned entries to
assembly; those operational ceilings do not become Project Snapshot invariants.

This decision amends ADR-0008's assignment of Project Ignore Policy matching to
the core and its guarantee that Project Snapshot membership is independent of
adapter correctness. Adapter-neutral Pack Creation still consumes one completed
Project Snapshot and remains unchanged.

## Consequences

- A source reader can use its storage system's most efficient native listing
  and pruning behavior without conforming to a lowest-common-denominator source
  interface.
- A buggy over-inclusive reader can produce a larger but valid Project
  Snapshot when every supplied entry satisfies universal snapshot invariants.
- Equivalent physical source trees are not guaranteed to produce equal Project
  Snapshots across readers unless those readers separately promise the same
  selection semantics.
- Source-selection configuration is not retained and affects Pack Identity only
  through the selected canonical paths and exact bytes.
- A shared reading or policy seam may be introduced later if multiple real
  readers demonstrate identical reusable behavior.
