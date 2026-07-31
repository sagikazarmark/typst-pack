# Typst Pack

This context defines the semantic values and guarantees shipped by typst-pack.
Future transport, caching, asynchronous execution, isolation, and session designs
belong in issues and ADRs until they become part of the implementation.

## Packs

**Pack**:
A validated, portable Typst compilation closure with one fixed entrypoint and a
fixed set of contained project paths and bytes. A Pack identifies its exact
package and font requirements and records whether each is embedded or must be
fulfilled externally. A value exists as a Pack only after whole-Pack canonical
and content-integrity validation. In-memory construction may choose project files
directly, subject to the built-in exclusion in the Project Ignore Policy; Pack
Creation uses the complete structural project tree described below.

**Pack Creation**:
The adapter-neutral operation that takes one Project Snapshot, one Font Catalog,
and one Package Catalog, performs Dependency Discovery to select package and font
requirements, and returns one Pack plus discovery warnings. It acquires nothing
itself; obtaining its inputs and coordinating repeated invocations belong to
Pack Assembly. Compiler observations select package and font requirements but
never select project files. The Discovery Specification fixes the Typst Target,
inputs, Document Time, and engine features for that run only; these values do
not become Pack state or restrict
later output formats. Creation fails when Dependency Discovery does not compile,
and when a supplied tree does not declare the specification it was supplied
under. When Dependency Discovery needs a package tree it was not given, creation
reports those exact package specifications instead of issuing a Pack,
so the caller can resolve them and invoke creation again. Those specifications
come from observed package requests rather than diagnostic text, creation
retains nothing between invocations, and how many invocations a project takes
is not semantic. A specification the caller declares unresolvable is no longer
reported, and the representative request fails at the file request that needed
it, carrying the caller's own failure; that is where an acquisition failure and
the source location it belongs to meet, because a reported specification names
a package and never the file that imported one.

**Dependency Discovery**:
The representative Typst compilation phase within Pack Creation that selects
package and font requirements from compiler observations. It never selects
project membership and may run once per resumable Pack Creation invocation.
_Avoid_: Creation Discovery, Precompilation

**Discovery Specification**:
The Typst Target, inputs, Document Time, and engine features used for one
Dependency Discovery run. It is a Pack Creation request value and never becomes
Pack state.

**Pack Creation Outcome**:
The normal result of one Pack Creation invocation. It is either `Created`,
containing one Pack and discovery warnings, or `Missing Package
Specifications`, containing the exact package specifications Pack Assembly must
add to the Package Catalog before invoking creation again.

**Pack Assembly**:
The host-side workflow that provisions one Project Snapshot, one Font Catalog,
and a Package Catalog containing the Package Trees requested by Pack Creation,
invokes Pack Creation until it issues a Pack or reaches a terminal failure, and
applies host-specific acquisition policy. Source-specific project gathering,
listing, reading, and fetching belong to configured gatherers, authorities, and
adapters; canonicalization, archive expansion, dependency selection, and
whole-Pack validation remain core transformations.
_Avoid_: Creation Preparation

**Pack Assembler**:
The workflow responsibility that performs Pack Assembly by coordinating
configured authorities and adapters with core transformations and Pack Creation.
The reference filesystem Pack Assembler additionally defaults the Discovery
Specification's Document Time to the host clock. A Pack Assembler does not
re-read acquired inputs solely to detect concurrent source mutation: a Pack
represents the exact values acquired, with no guarantee that every source value
coexisted at one instant.
_Avoid_: Creation Adapter

**Project Ignore Policy**:
The reference filesystem project gatherer's root-scoped exclusion policy.
Ordered rules come from the root `.typkignore` and use Gitignore-style matching,
including negation and last-match precedence. The gatherer applies it while
traversing so excluded files are not read and excluded directories are not
entered. The root policy file itself is included; nested `.typkignore` files are
ordinary project files. Other source gatherers may select project membership by
different source-specific means.

**Project Snapshot**:
One stabilized, source-selected set of project files with one entrypoint among
them, represented by canonical root-relative path and exact bytes. Project
Snapshot assembly rejects invalid or duplicate paths, excludes every `.typk`
path by a non-overridable built-in rule, requires the entrypoint, and may bound
the result by file count and total byte size. Source selection policy is not
retained and does not contribute to Pack Identity except through the selected
paths and bytes.

**Pack Identity**:
The content identity of a Pack's canonical logical compilation state: fixed
entrypoint, contained project files, ordered Pack Font Catalog, exact dependency
requirements, and each requirement's embedded or external disposition. Creation
request values, metadata, provenance, archive encoding, and host filesystem facts
do not contribute.

**Canonical Identity**:
The shared convention used by concrete Pack, package-tree, font-container,
compilation, and result identities. Equality includes the identity role, schema,
algorithm, and digest; a bare digest is insufficient.
_Avoid_: bare hash, checksum

**Self-Contained Pack**:
A Pack that embeds every package and font dependency. It can compile with only
the Pack, the exact embedded Typst implementation, and explicit compilation
request values. Other Packs remain portable but require exact declared external
fulfillment.

**Pack Manifest**:
The versioned declarative record stored in a Pack archive. It describes the
entrypoint, package requirements, font catalog, and optional metadata. Pack
validation derives identities and verifies agreement between these declarations
and contained bytes rather than trusting the manifest alone. The current reader
supports only its exact version-1 representation schema; version 1 remains
unstable and does not identify schemas from earlier releases.

**Pack Inspection**:
The side-effect-free static inventory exposed by validated Pack accessors and the
`inspect` command. It does not acquire dependencies, apply Pack Overrides, or
compile the Pack.

**Pack Extraction**:
The non-invertible workflow that projects contained project files into an
editable project tree. Packages and fonts are separate extraction choices; Pack
metadata and Pack Overrides are excluded. Its semantic portion selects and plans
the projection, while a destination adapter applies that plan and reports writes
or partial outcomes. Extraction is not Pack Archive Decoding, and using its
result for new Pack Creation does not preserve the original Pack Identity.

**Pack Extraction Plan**:
The collision-checked, destination-relative projection of one Pack under explicit
project, package, and font extraction choices. It contains no destination or
write policy; destination I/O failures belong to the adapter and may produce a
partial outcome.

**Pack Extraction Request**:
The workflow envelope that combines a Pack and extraction choices with a
destination and conflict policy. Planning yields a Pack Extraction Plan before a
destination adapter performs writes.

**Pack Archive**:
The interoperable byte representation of one Pack. A Pack Archive carries a Pack
Manifest and the bytes required by that representation, but archive layout and
encoding do not contribute to Pack Identity. A decoded value becomes a Pack only
after whole-Pack validation. Safe unknown entries may be ignored during decoding
and need not survive re-encoding.

**Pack Archive Encoding**:
The representation operation that converts a validated Pack into Pack Archive
bytes. It does not publish those bytes or repeat whole-Pack validation. Encoding
enforces representation-specific limits, so a valid Pack may still fail to encode
in the current representation.

**Pack Archive Decoding**:
The representation operation that parses Pack Archive bytes and submits their
declarations and content to whole-Pack validation. Decoding rejects malformed,
unsafe, or ambiguous raw archive members before interpreting them as domain
content. It is not Pack Extraction.

**Pack Archive Publication**:
The storage operation that writes exact Pack Archive bytes to a destination. It
does not define their encoding.

**Pack Archive Acquisition**:
The storage operation that obtains exact Pack Archive bytes from a source. It
does not decode or validate them.

## Packages

**Package Catalog**:
The Pack Creation input keyed by exact package specification. Each entry pairs a
Package Tree with its intended embedded-or-external disposition; Pack Creation
validates every claimed specification and Package Tree relationship before
Dependency Discovery, whether or not discovery selects that entry.

**Package Acquisition Failure**:
An operational failure from attempting to acquire one specification returned by
`Missing Package Specifications`. Pack Assembly carries it separately from the
Package Catalog so Dependency Discovery can attach it to the importing source.

**Package Requirement**:
One dependency identified by an exact Typst package specification and the
Canonical Identity of its Package Tree. Source location and acquisition
metadata do not contribute. The requirement records whether that exact tree is
embedded or externally fulfilled.

**Package Tree**:
Every addressable regular file beneath one acquired package root, represented by
canonical package-relative path and exact bytes. It includes package metadata and
files not read by Dependency Discovery, but excludes empty directories and
host filesystem metadata. Completeness is an invariant: a partial set of package
files is not a Package Tree.
_Avoid_: Complete Package Tree

**Package Authority**:
The explicitly configured responsibility that resolves package specifications
during Pack Assembly or obtains exact trees for compilation. In the core,
compilation receives already acquired Package Tree Fulfillments rather than a
public authority interface. Configured offline behavior disables package
downloading; it may still use explicit or local package sources.

**Package Registry**:
The remote source a Pack Assembler fetches package archives from, addressed by
one URL per exact package specification. Only the official Typst Universe
namespace is served; a specification in any other namespace has no registry URL.
The core owns URL construction, and fetching stays with the adapter, so no
transport is implied.

**Package Archive Expansion**:
The core transformation from the archive bytes served for one specification into
a Package Tree. Only addressable regular files become entries, and a
member whose path cannot name a package file is rejected. It takes a required
expansion ceiling, charges every archive member against it, and fails past it
rather than materializing what lies beyond, so a caller-named package cannot
exhaust the process. The ceiling is required rather than defaulted so that the
bound is always a deliberate choice.

**Package Tree Fulfillment**:
Supplying one non-embedded Package Requirement as a Package Tree. The
core canonicalizes and verifies the entire supplied tree against the requirement
before Typst compilation. Optional provenance and cache-hit fields are
caller-provided operational report metadata; they are not authenticated,
identity-bearing, or semantic inputs.

**Package Embedding**:
The Pack Creation choice to embed every selected Package Tree or record
selected packages for external fulfillment. The choice affects Pack Identity and
self-containment.

## Fonts

**Font Container**:
The exact bytes of one standalone font file or multi-face font collection. Its
Canonical Identity is independent of source location, and every face in a
collection travels in the same container.

**Font Catalog**:
The ordered sequence of Font Containers Pack Creation may select faces from,
each carrying its own embedded-or-external disposition. Faces are expanded in
container-local index order, catalog order decides which container offers a
family, and nothing joins a supplied catalog implicitly. A Pack Assembler
composes it during Pack Assembly; the reference filesystem Pack Assembler
composes system fonts, Typst's embedded fonts, and scanned font directories, in
that order.
_Avoid_: Candidate Font Catalog

**Font Face Identity**:
One exact face within a Font Container, identified by container identity and
container-local face index. Family, style, coverage, and other metadata are
derived from the verified container.

**Font Requirement**:
One required Font Container and the nonempty set of Font Face Identities selected
during Pack Creation. The requirement records whether the container is embedded
or externally fulfilled.

**Font Authority**:
The explicitly configured responsibility that supplies the Font Catalog during
Pack Assembly or obtains exact Font Containers for
compilation. In the core, compilation receives already acquired Font Container
Fulfillments rather than a public authority interface.

**Pack Font Catalog**:
The ordered projection of the Font Catalog containing exactly the Font
Face Identities available to Pack compilation. Relative selection order is
preserved. Other faces physically present in a required Font Container remain
unavailable unless declared.

**Font Container Fulfillment**:
Supplying one non-embedded Font Requirement as exact Font Container bytes. The
core verifies container identity and every declared face before Typst
compilation. Optional provenance and licensing fields are caller-provided
operational report metadata; they do not establish authenticity or legal
permission and do not contribute to identities.

**Font Embedding**:
The Pack Creation choice to embed selected Font Containers or record them for
external fulfillment. It is declared per container in the Font Catalog and never
inferred from container bytes, so one Pack may embed one
container and reference another, and Typst-embedded-font handling is an
explicit creation option. The choice affects Pack Identity and
self-containment; typst-pack does not derive a per-font legal policy from
licensing metadata.

## Compilation Requests

**Pack Compilation Request**:
The workflow envelope that combines one Pack, caller-supplied semantic
compilation controls, one Compilation Fulfillment Set, and optional operational
report metadata. It is not itself a validated domain value.

**Compilation Fulfillment Set**:
The Package Tree Fulfillments and Font Container Fulfillments supplied for one
Pack compilation, with at most one entry per exact package specification or Font
Container Identity. Construction rejects duplicate entries. After semantic
request acceptance, compilation verifies that the set corresponds exactly to
the Pack's external requirements; missing entries and entries for embedded or
undeclared dependencies are invalid. Fulfillment bytes and their operational
report metadata do not contribute to semantic request values.

**Typst Target**:
The selected Typst document model: Paged or HTML. A Discovery Specification
selects one target for Dependency Discovery. A Compilation Output Specification
derives the target required for that compilation.

**Document Format**:
A format that produces one Compilation Output Artifact for the selected document.
PDF and HTML are Document Formats.

**Page Format**:
A format that produces one Compilation Output Artifact for each selected source
page. PNG and SVG are Page Formats. Page selection is canonical and artifacts are
ordered by Source Page Number.

**Source Page Number**:
The one-based physical position of a page in the source document before page
selection, distinct from emission order and printed page labels.

**Compilation Output Artifact**:
One exact owned byte value produced by compilation. Its role contains the output
format and, for a Page Format, one Source Page Number. Filenames, destinations,
and transport metadata are not part of the artifact.

**Document Time**:
The exact value used to answer Typst document-time requests such as
`datetime.today()`: explicitly absent, one fixed Typst date/datetime, or one Unix
timestamp interpreted under each requested timezone offset. The core default is
absent. It contributes to Compilation Identity whether or not document code
observes it.

**PDF Creation Timestamp**:
The PDF-specific timestamp control offered to PDF metadata. It is explicitly
omitted or fixed after defaults are resolved, is independent of Document Time,
and contributes to Compilation Identity. An adapter may intentionally resolve one
external timestamp into both values.

**Compilation Output Specification**:
The required tagged output request. PDF, PNG, SVG, and HTML variants expose only
their applicable controls and derive the required Typst Target and format feature.
The core resolves deterministic defaults, canonicalizes page selection and PDF
standards, and rejects invalid values before compilation.

**Compilation Request Inventory**:
The canonical description of every effective semantic request value: output
controls and origins, safe Typst-input and Pack-override commitments, selected
and derived features, and Document Time. Values that affect only acquisition,
presentation, or destination do not contribute.

**Compilation Request Commitment**:
A role-separated digest that binds sensitive Typst inputs or Pack Override bytes
without exposing them. Safe inventories retain commitments and exact sizes rather
than raw values. Commitments do not provide confidentiality against guessing.

**Pack Override**:
A compilation-scoped byte replacement for one project file contained in a Pack,
including Typst source or the entrypoint. It cannot add or delete paths or replace
package or font content.

**Pack Override Set**:
The immutable set of Pack Overrides bound to one Pack Identity. Each contained
path appears at most once and declaration order is not semantic. Every member
contributes to Compilation Identity through path, size, and commitment whether or
not the engine reads it.

## Compilation Results

**Engine Identity**:
The exact embedded Typst compiler identity attested by typst-pack: implementation,
version, source checksum, Rust target, target features, crate feature set, and
debug-assertion mode.

**Exporter Identity**:
The exact official format exporter identity attested with the same implementation
fields as Engine Identity.

**Compilation Request Rejection**:
The deterministic, side-effect-free rejection of an invalid semantic request
before Compilation Identity or dependency verification. It owns the complete
Compilation Request Inventory and every independently detectable ordered
Compilation Request Issue.

**Compilation Identity**:
The pre-execution identity of one fully specified semantic compilation. It binds
Pack Identity, output specification, input and override commitments, features,
Document Time, Engine Identity, and Exporter Identity. Destinations, timing,
provenance, and fulfillment cache metadata do not contribute.

**Compilation Operation Outcome**:
A typed operational failure after request acceptance but before a semantic
Compilation Result. Shipped outcomes aggregate every independently detectable
missing, unexpected, or identity-mismatched external package and font
fulfillment in canonical order. Fulfillments carry validated Package Trees and
Font Containers, so malformed raw dependency bytes fail before compilation when
those values are constructed. Outcomes retain request inventory, Compilation
Identity, and fulfillment reporting through Compilation Report.

**Canonical Compilation Diagnostic**:
One ordered compiler or exporter diagnostic containing severity, message, logical
span, hints, tracepoints, phase, producer, and an optional Source Page Number.
Messages and source-derived values remain untrusted presentation data.

**Pack Compilation Warning**:
One Pack-owned warning produced while preparing or exporting an accepted
compilation. It is distinct from official compiler or exporter diagnostics and
from discovery warnings returned by Pack Creation.

**Compilation Access Trace**:
The canonical set of project, package, and font requests observed by Typst. Each
observation records request kind, logical path, optional font index, and a read,
missing, or failed outcome. Successful reads include exact byte length and
content digest. Access order and repeat counts are not semantic.

**Compilation Document Summary**:
The Typst Target reached by semantic compilation and, for paged documents, the
total Source Page Number count reached before complete export.

**Compilation Result**:
The immutable semantic value produced after request acceptance and exact
dependency verification. It records succeeded or rejected status, ordered
artifacts, complete canonical diagnostics, Pack Compilation Warnings, document
summary, Compilation Access Trace, implementation identities, request inventory,
Compilation Identity, and Compilation Result Identity. Compiler or exporter
rejection is a result with no artifacts, not an operational failure.

**Compilation Result Identity**:
The post-execution identity binding Compilation Identity to result status,
document summary, diagnostics, Pack Compilation Warnings, access trace, and every
ordered artifact role, length, and content identity. Fulfillment provenance and
cache metadata do not contribute.

**Compilation Report**:
The immutable terminal returned for every accepted compilation request. Its
outcome is either one Compilation Result or one Compilation Operation Outcome.
It also retains package and font fulfillment reporting. Invalid semantic requests
return Compilation Request Rejection before a report exists.

**Environment-Independent Compilation**:
The confinement guarantee that, once Pack, semantic request, implementation
identities, and exact dependency bytes are fixed, compilation does not consult
ambient project files, package paths, fonts, environment variables, wall-clock
time, or network. Availability of explicitly external dependencies may still
produce a Compilation Operation Outcome before a result exists.
