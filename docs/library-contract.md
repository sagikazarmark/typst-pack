# Library contract

This page collects guarantees that are useful when integrating the Rust library
but too detailed for the README.

## Pack and identities

A `Pack` is valid only when its entrypoint, project paths, package requirements,
font requirements, embedded values, and declared identities agree. Construction
and archive decoding run the same whole-Pack validation.

Pack Identity covers the entrypoint, project paths and bytes, ordered fonts,
exact package and font requirements, and whether each requirement is embedded.
Metadata, archive compression and ordering, storage locations, read/write
evidence, and host configuration do not contribute. Canonical identities include
their role, schema, algorithm, and digest; compare the whole value, not a bare
digest.

Compilation and result identities also include the semantic request and exact
embedded implementation identities. Every enabled typst-pack crate feature is
part of implementation attestation, even when that feature affects only an
adapter. Feature-set changes therefore require a different identity-keyed cache
namespace.

## Creation and fulfillment

`create` borrows a completed `ProjectSnapshot`, `PackageCatalog`, `FontCatalog`,
failed package reads, and a `DiscoverySpecification`. It reads no files, uses no
network, and consults no wall clock. Compiler observations select package and
font requirements; project membership comes only from the snapshot.

Creation may return `MissingPackageSpecifications`. This is a normal resumable
state: add exact trees to the package catalog and invoke the stateless operation
again. A Pack Assembler owns the source-specific reading and retry loop.

Compilation accepts external dependencies only through a
`CompilationFulfillmentSet`. Before Typst runs, the library rejects missing,
undeclared, unexpectedly external, or identity-mismatched package trees and font
containers, and verifies required font faces. Fulfillment provenance, cache-hit,
and licensing fields are operational report data; they do not change compilation
or result identity.

## Environment independence

Once a Pack, compilation request, implementation identities, and verified
external fulfillment bytes are fixed, compilation does not fall back to ambient
project files, package caches, fonts, environment variables, wall-clock time, or
the network. `DocumentTime`, Typst inputs, features, output controls, and Pack
Overrides are explicit request values.

`PackOverrideSet` is bound to one Pack Identity. It can replace bytes only at an
existing project path and cannot add paths, remove paths, or change package or
font authority.

## Limits

Operations over caller-controlled semantic values provide first-party limits:
`compile(request)` and `pack_archive::encode(pack)`. Their `*_with_limits`
variants accept narrower or larger validated profiles.

Operations that cross a byte trust boundary require explicit limits. These
include Pack Archive reading and decoding, package archive expansion, filesystem
source reading, and OpenDAL read requests. Limits bound documented retained or
generated resources, not process RSS, allocator overhead, compiler internals,
elapsed time, or aggregate concurrency between independent operations.

## Writes and recovery

Write policy is explicit. Filesystem APIs provide policies with their documented
staging and per-file or whole-tree guarantees. OpenDAL provides
`WritePolicy::CreateOrVerify` and `WritePolicy::OverwriteExactKeys`; it makes no
transactional or multi-key atomicity promise.

Write inputs are borrowed or returned on composed-operation failure so callers
retain exact replay material. Successful receipts report completed work. Relevant
errors retain progress, the failed phase or path, the native cause, and
`CommitCertainty` when a destination effect may be uncertain.

Multi-entry writes may leave a completed prefix. There is no general rollback.
Inspect progress and commit certainty, reconcile destination state where needed,
then replay the complete immutable plan or result under a policy suitable for
the application. Cancellation can also leave effects; a dropped future produces
no receipt or error.

## Archive representation

`PackArchiveBytes` uniquely owns exact encoded or read bytes so retry material is
unambiguous. Archive decoding borrows those bytes. Composed read/decode and
encode/write errors preserve exact archive bytes where the operation owns them.

Pack Archive compatibility is semantic. Re-encoding may change compression,
timestamps, unknown safe entries, and member ordering without changing the Pack.
