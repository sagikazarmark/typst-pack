# Library contract

This page collects guarantees that are useful when integrating the Rust library
but too detailed for the README.

## Pack and identities

A `Pack` is valid only when its entrypoint, project paths, packages, and font
containers agree. The builder, creation, and archive decoding all construct a
pack through the same validation.

Every identity is a SHA-256 `Digest` over a documented preimage (see the README's
"Pack format"). The pack identity covers the entrypoint, project paths and bytes,
each package's tree digest and whether it is embedded, and each font container's
digest and whether it is embedded, in font book order. Metadata, informational
manifest fields, archive compression and member order, storage locations, and
host configuration do not contribute.

Font requirements are whole containers: compilation exposes every face of every
required container to Typst, so text the representative compile did not render
uses the same fonts. External containers are matched by exact digest, and
fulfillment failures name the families the pack expects.

A compilation identity covers the pack identity, the normalized output and
its controls, the `sys.inputs` strings and byte digests, document time,
features, the digests of the externally supplied packages and font containers
the pack requires, and the implementation string `typst_pack::VERSION`. Its
preimage is documented in the README's "Compilation identity". Crate features,
target, build profile, limits, and export workers do not contribute, so the
identity is a cache key across build configurations of one release.

## Creation and fulfillment

`create` reads one `create::Request`: a validated `ProjectSnapshot`, a
`package::Catalog` and a list of `font::Entry` values that each say whether
the dependency is embedded or external, failed package reads, the discovery
target, options (inputs, document time, features), and input variants. It reads no files, uses
no network, and consults no wall clock. It compiles the base options and then
each variant with its inputs replacing or adding to the base inputs; the
packages and font containers any of those compiles used become requirements.
Project membership comes only from the snapshot, which the pack carries without
validating it again. The HTML target enables Typst's HTML feature itself.

Creation may return `create::Outcome::MissingPackages`. This is a normal
resumable state: add exact trees to the package catalog (or record a failed
read) and invoke the stateless operation again; a package any variant reached
is reported. A creation whose compiles succeeded returns the pack, the warnings
of all of them without repeats, and the `Sources` they read. `fs::Packer::pack`
and `opendal::Assembler` own the source-specific reading and retry loop.

Compilation receives external dependencies as `compile::Request::packages`
(trees under their specifications) and `compile::Request::fonts` (containers).
Each external requirement uses the supplied tree or container whose digest
matches; others are ignored and never reach Typst. A requirement with no
match, or whose specification was supplied only with other bytes, is a
`compile::Error::Unfulfilled` naming the package specification or the font
families. `fs::Packer::resolve` finds them in the configured package
directories and font sources.

## Environment independence

Once a `compile::Request` is fixed, compilation does not fall back to ambient
project files, package caches, fonts, environment variables, wall-clock time, or
the network. `DocumentTime`, `sys.inputs`, features, and output controls are
explicit request values. A project file of the pack never changes between
compilations; per-recipient data is an input instead. `Input::Str` values
appear in `sys.inputs` as Typst strings and `Input::Bytes` values as Typst
`bytes`, which `image`, the data loaders, and `str` accept directly. Inputs do
not add project paths or change package or font requirements.

## Limits

Operations over caller-controlled values need no read limits;
`compile::Request::limits` bounds compiled output with `compile::Limits`
(pages, artifacts, pixels, and bytes), and an exceeded one is
`compile::Error::Limit`. Operations that cross a byte trust boundary take
explicit limits: `archive::DecodeLimits` for archive reading and decoding,
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
resource. Reading, packing, and resolving report one error type, `fs::Error`.

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
reported in `fs::Packed::skipped_fonts` (or `fs::Resolved::skipped_fonts`)
instead of failing the read. Only an explicitly configured font path that
cannot be opened, or an exceeded limit, fails the read.

These checks are validation, not confinement. The adapter uses the process's
ambient authority and claims nothing against a filesystem peer acting between
operations. One assembly's read values are each exact as observed by their
source adapter; the adapter does not establish that they coexisted at a single
instant.

## Writes and recovery

Write policy is explicit. Filesystem writes take `fs::WritePolicy::CreateNew`
(fail when a target exists) or `fs::WritePolicy::Overwrite` (replace or create).
Each file is written to a temporary sibling and then committed: `Overwrite`
renames it over the target, so readers see the old or the new content;
`CreateNew` hard-links it, which fails on an existing target, and falls back to
an exclusive create and write on filesystems without hard links, where an
interrupted write can leave a partial file. The temporary file is removed on
every path, and a failed cleanup after a successful commit is not an error.
Nothing is flushed to stable storage, so there is no crash-durability claim.

Before writing an extraction plan, the adapter checks the plan and the
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

Archive bytes are plain `Vec<u8>` values: `archive::encode` returns them,
`archive::decode` borrows them, and `archive::read`, `archive::write`,
`archive::open`, and `archive::save` compose them with streams and files.

Decoding verifies every member against the digest the manifest declares, so a
decoded pack is exactly the content its producer encoded, whatever ZIP writer
produced the archive. Re-encoding a decoded pack yields the canonical encoding,
which may differ from the original archive in compression, timestamps, and
member order, but never in its identity.
