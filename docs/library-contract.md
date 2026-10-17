# Library contract

This page collects guarantees that are useful when integrating the Rust library
but too detailed for the README.

## Pack and identities

A `Pack` is valid only when its entrypoint, project paths, packages, and font
containers agree. The builder, creation, and archive decoding all construct a
Pack through the same whole-Pack validation.

Every identity is a SHA-256 `Digest` over a documented preimage (see the README's
"Pack format"). Pack Identity covers the entrypoint, project paths and bytes,
each package's tree digest and whether it is embedded, and each font container's
digest and whether it is embedded, in font book order. Metadata, informational
manifest fields, archive compression and member order, storage locations, and
host configuration do not contribute.

Font requirements are whole containers: compilation exposes every face of every
required container to Typst, so text the representative compile did not render
uses the same fonts. External containers are matched by exact digest, and
fulfillment failures name the families the pack expects.

Compilation and result identities also include the semantic request and exact
embedded implementation identities, including the enabled typst-pack crate
features. Their preimages are not a documented format; use them only as cache
keys within one build configuration.

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
undeclared, unexpectedly external, or digest-mismatched package trees and font
containers. Fulfillment provenance, cache-hit,
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

Operations over caller-controlled values, `compile(request)` and
`pack_archive::encode(pack)`, need no limits; `compile_with_limits` accepts a
`CompilationLimits` value. Operations that cross a byte trust boundary take
explicit limits: `pack_archive::DecodeLimits` for archive reading and decoding,
and `ReadLimits` for package archive expansion, filesystem source reading, and
OpenDAL reads. Every limits type is a plain struct with public fields and a
`Default`; an exceeded limit is a `LimitExceeded` naming the resource and the
limit. Limits bound documented retained or generated resources, not process
RSS, allocator overhead, compiler internals, elapsed time, or aggregate
concurrency between independent operations.

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

Write inputs are borrowed, so callers keep them for a retry. A filesystem write
failure names the failing path
and the files already committed; an OpenDAL `WriteError` names the completed
keys, the failing key, and the native cause.

Multi-entry writes may leave a completed prefix. There is no general rollback.
Inspect what was committed, reconcile destination state where needed, then
replay the complete immutable plan or result under a policy suitable for the
application. Cancellation can also leave effects; a dropped future produces no
receipt or error.

## Archive representation

Archive bytes are plain `Vec<u8>` values: `pack_archive::encode` returns them,
`pack_archive::decode` borrows them, and `read`, `read_pack`, `write_pack`,
`open_pack`, and `save_pack` compose them with streams and files.

Decoding verifies every member against the digest the manifest declares, so a
decoded Pack is exactly the content its producer encoded, whatever ZIP writer
produced the archive. Re-encoding a decoded Pack yields the canonical encoding,
which may differ from the original archive in compression, timestamps, and
member order, but never in Pack Identity.
