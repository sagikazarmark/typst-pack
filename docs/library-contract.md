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
source reading, and OpenDAL reads. Limits bound documented retained or
generated resources, not process RSS, allocator overhead, compiler internals,
elapsed time, or aggregate concurrency between independent operations.

## Filesystem adapter

Project, package, and font readers share one bounded walker. It lists each
directory through the handle of its parent and takes `ReadLimits` (entries,
per-file bytes, total bytes); an exceeded limit is a `LimitExceeded` naming the
resource. All three report one error type, `FilesystemReadError`.

Projects and packages follow symbolic links whose targets resolve beneath the
root, including directory links, and refuse a link resolving outside it with
an error naming the link. A link that leads back into a directory on the
current descent is refused as a cycle. FIFOs, sockets, and devices are refused.
The root itself may be a link, as `@local` development packages commonly are.
Project membership comes from the root `.typkignore` policy, applied with
gitignore semantics before any refusal, so an excluded entry is never examined;
nested policy files are ordinary project files.

Font directories are host configuration. Links are followed wherever they
lead, only files with a font extension are considered, and an unreadable
directory or file, an unsupported entry, or an unparseable font is skipped and
reported in `FilesystemFonts::skipped` instead of failing the catalog. Only an
explicitly configured root that cannot be opened, or an exceeded limit, fails
the read.

These checks are validation, not confinement. The adapter uses the process's
ambient authority and claims nothing against a filesystem peer acting between
operations. One assembly's read values are each exact as observed by their
source adapter; the adapter does not establish that they coexisted at a single
instant.

## Writes and recovery

Write policy is explicit. Filesystem writes take `WritePolicy::CreateNew`
(fail when a target exists) or `WritePolicy::Overwrite` (replace or create).
Each file is written to a temporary sibling and then committed: `Overwrite`
renames it over the target, so readers see the old or the new content;
`CreateNew` hard-links it, which fails on an existing target, and falls back to
an exclusive create and write on filesystems without hard links, where an
interrupted write can leave a partial file. The temporary file is removed on
every path, and a failed cleanup after a successful commit is not an error.
Nothing is flushed to stable storage, so there is no crash-durability claim.

Before writing a Pack Extraction Plan, the adapter checks the plan and the
destination and writes nothing if any check fails, listing every issue:
paths must be canonical and portable (no Windows device names, trailing dots
or spaces, or control characters, on every platform), must not collide when
case is ignored, and must not conflict with existing destination entries. Under
`CreateNew` an existing target is an `ExistingTarget` issue. Directories
beneath the destination are created without following links, so a linked or
non-directory ancestor beneath the destination is refused; the destination
path itself may contain links.

OpenDAL provides `opendal::WritePolicy::CreateOrVerify` and
`opendal::WritePolicy::Overwrite`; it makes no transactional or multi-key
atomicity promise. An OpenDAL `WriteError` reports the completed keys and
whether the failing key's effect is indeterminate.

Write inputs are borrowed or returned on composed-operation failure so callers
retain exact replay material. A filesystem write failure names the failing path
and the files already committed; an OpenDAL `WriteError` names the completed
keys, the failing key, and the native cause.

Multi-entry writes may leave a completed prefix. There is no general rollback.
Inspect what was committed, reconcile destination state where needed, then
replay the complete immutable plan or result under a policy suitable for the
application. Cancellation can also leave effects; a dropped future produces no
receipt or error.

## Archive representation

`PackArchiveBytes` uniquely owns exact encoded or read bytes so retry material is
unambiguous. Archive decoding borrows those bytes. Composed read/decode and
encode/write errors preserve exact archive bytes where the operation owns them.

Pack Archive compatibility is semantic. Re-encoding may change compression,
timestamps, unknown safe entries, and member ordering without changing the Pack.
