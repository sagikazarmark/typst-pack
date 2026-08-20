# ADR-0013: Use Phase-Specific Lifecycle Failure Vocabulary

## Status

Accepted, amended by ADR-0016

## Context

The Pack lifecycle has several materially different non-success states: invalid
semantic construction, rejection before request acceptance, resumable creation,
post-acceptance operational outcomes, compiler rejection as a semantic result,
and destination failures with retry or partial-progress evidence. Representing
all of them as interchangeable errors would hide phase ordering, encourage
callers to retry the wrong values, and flatten authoritative lower-module
failures into duplicated outer enums.

The lifecycle also spans a featureless semantic core, the Pack Archive format,
concrete source readers and authorities, and destination adapters with
different atomicity guarantees. A generic storage or error hierarchy would make
each caller learn behavior that does not apply to its operation.

## Decision

Use phase-specific vocabulary:

- `Error` means construction, transformation, or adapter execution failed and no
  accepted semantic outcome exists.
- `Rejection` means a deterministic refusal before semantic acceptance.
- `Issue` means one independently detectable typed fact in an aggregate.
- `Outcome` means a normal state after input acceptance, including resumable or
  operational non-result states.
- `Report` means immutable terminal evidence for an accepted operation.
- `Receipt` and `Progress` are workflow-specific destination evidence.
- `Failure` is reserved for domain data describing an external failed attempt,
  such as `PackageReadFailure`, rather than used as a synonym for every
  Rust error.

Each module owns its public error names. Concise names such as `DecodeError` and
`EncodeError` remain scoped under `pack_archive`; no ambiguous root aliases or
lifecycle-wide error enum are introduced. Public failure and issue enums are
non-exhaustive. Genuinely closed success and status enums may remain exhaustive.
Pure semantic failures support `Clone`, `Eq`, and `Debug`; adapter errors that
retain native causes support `Debug`, `Display`, and `std::error::Error`.

An outer module preserves an authoritative lower-module failure as a typed nested
source instead of translating or duplicating its variants. In particular,
`pack_archive::DecodeError::InvalidPack` carries `PackInvariantError`, and a
concrete Pack Assembler carries `PackCreationError` unchanged. Expected content,
policy, codec, compiler, and adapter failures are explicit; public interfaces do
not include a generic `Internal` or `Unexpected` catchall for private invariant
bugs.

When validation can continue safely, an error, rejection, or operational outcome
owns a nonempty list of all independently detectable issues. Keyed issues are
ordered by domain role, canonical key, and documented issue-kind rank; unkeyed
request issues use interface field order. Traversal, hash-map, compiler callback,
and backend discovery order are never observable issue order. Parsing or
interpretation failures that prevent further checks remain singular typed
errors. Aggregate fields are private and exposed through borrowed accessors.

Every external-input or amplification seam owns a non-exhaustive resource enum
and limit error. These operation-specific types share the variants
`Exceeded { resource, ceiling, observed_at_least }` and
`AccountingOverflow { resource }`; there is no global resource enum or generic
public limit error.

Exact retry material has explicit ownership. Pack Archive decoding borrows
`PackArchiveBytes`. A convenience read-and-decode error returns the exact
read bytes after decode failure, and a convenience encode-and-write error
returns the exact encoded bytes after write failure. Semantic
construction, archive transformation, reading, expansion, and compilation
export return no partial semantic value.

Destination adapters validate requested policy before I/O and return a typed
`UnsupportedPolicy` error rather than weakening it. They aggregate safely
detectable preflight issues before writes. An application error retains
workflow-specific progress, failed phase and target, private staging residue,
the concrete adapter cause, and `CommitCertainty::{NotCommitted, Committed,
Indeterminate}` where the target's visibility may be uncertain. Receipts,
progress, and errors stay workflow-specific even when private implementation is
shared.

Use Pack Assembler, source-specific project reader, Package Authority, Font
Authority, adapter, Read, and Write for host-side roles and byte
movement. "Persistence" may group operations in prose but names no public
module, trait, or value. Capability appraisal is adapter implementation
vocabulary; unsupported public behavior is expressed as unsupported requested
policy. No generic capability, storage, repository, source, sink, or shared Pack
Assembly error/report interface is introduced before multiple real adapters
demonstrate identical behavior.

Public failure context is typed and recovery-oriented: canonical logical paths,
roles, specifications, identities, numeric limits, phases, and concrete adapter
destinations. Payload bytes and secrets are excluded. Source-derived strings are
untrusted presentation data and are escaped when rendered.

## Consequences

- Pack Creation keeps Missing Package Specifications as a resumable outcome;
  Dependency Discovery rejection is a `PackCreationError` retaining complete
  diagnostics and warnings.
- Compilation keeps request rejection outside `CompilationReport`, complete
  fulfillment and resource-limit outcomes inside the report, and compiler or
  exporter rejection inside `CompilationResult`.
- Whole-Pack validation can report complete canonical `PackInvariantIssue`
  evidence while Pack Archive decoding preserves the distinction between raw
  archive, manifest, limit, and semantic failures.
- Concrete Pack Assemblers and destination adapters keep native operational
  causes and evidence without contaminating semantic interfaces or identities.
- Callers branch on typed phase and context rather than rendered messages, while
  future compatible failure variants remain possible.
