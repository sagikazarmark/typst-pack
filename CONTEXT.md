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
The adapter-neutral operation that takes one Project Snapshot, one Candidate
Font Catalog, and the Complete Package Trees resolved for it, runs one
representative Typst request to select package and font requirements, and
returns one Pack plus representative-compile warnings. It acquires nothing
itself; obtaining its inputs is Creation Preparation. Compiler observations
select package and font requirements but never select project files. The
representative request fixes the Typst Target, inputs, Document Time, and engine
features for that run only; these values do not become Pack state or restrict
later output formats. Creation fails when the representative request does not
compile, and when a supplied tree does not declare the specification it was
supplied under. When that request needs a package tree it was not given,
creation reports those exact package specifications instead of issuing a Pack,
so the caller can resolve them and invoke creation again. Those specifications
come from observed package requests rather than diagnostic text, creation
retains nothing between invocations, and how many invocations a project takes
is not semantic.

**Creation Preparation**:
The acquisition phase that obtains Pack Creation's inputs: the project files,
the Candidate Font Catalog, and the Complete Package Trees for the
specifications creation reported as missing. It acquires bytes and never
transforms them; every transformation belongs to the core.

**Creation Adapter**:
The host-specific responsibility that performs Creation Preparation for one kind
of host, supplying listing, reading, and fetching only. The reference adapter is
the filesystem one, which additionally defaults the representative request's
Document Time to the host clock and revalidates project and selected dependency
evidence before the Pack is returned, failing creation when that evidence
changed (the Creation Evidence Fence). Establishing that acquired bytes
represent one consistent source state is the adapter's own obligation and is
advisory: an adapter acquiring from mutable storage without revalidating still
conforms, and may produce a Pack describing a source state that never existed
simultaneously.

**Project Ignore Policy**:
The root-scoped exclusion policy that decides project membership. Every `.typk`
path is excluded by a non-overridable built-in rule that binds every caller,
including in-memory construction. Additional ordered rules come from the root
`.typkignore` and use Gitignore-style matching, including negation and last-match
precedence. The policy is determined by ignore-file bytes alone and matches a
path without consulting a host, so a Creation Adapter can apply it to a listing
before reading content. The root policy file itself is included; nested
`.typkignore` files are ordinary project files. Malformed rules prevent creation,
as do the unsupported unignored entries, unreadable files, invalid paths, and
traversal failures a Creation Adapter reports.

**Project Snapshot**:
One stabilized set of project files with one entrypoint among them, represented
by canonical root-relative path and exact bytes. It is assembled from
path-and-bytes entries under one Project Ignore Policy, which assembly
re-applies so that membership does not depend on a Creation Adapter being
well-behaved. Assembly rejects entries that cannot name a root-relative project
file and fails when the entrypoint does not survive filtering. A caller may
bound the result by file count and total byte size; exclusion is applied before
those bounds are measured.

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
Projection of contained project files into an editable project tree. Packages and
fonts are separate extraction choices; Pack metadata and Pack Overrides are
excluded. Using the result for new Pack Creation does not preserve the original
Pack Identity.

## Packages

**Package Requirement**:
One dependency identified by an exact Typst package specification and the
Canonical Identity of its Complete Package Tree. Source location and acquisition
metadata do not contribute. The requirement records whether that exact tree is
embedded or externally fulfilled.

**Complete Package Tree**:
Every addressable regular file beneath one acquired package root, represented by
canonical package-relative path and exact bytes. It includes package metadata and
files not read by the representative request, but excludes empty directories and
host filesystem metadata.

**Package Authority**:
The explicitly configured responsibility that resolves package specifications
during Creation Preparation or obtains exact trees for compilation. In the core,
compilation receives already acquired Package Tree Fulfillments rather than a
public authority interface. Configured offline behavior disables package
downloading; it may still use explicit or local package sources.

**External Package Fulfillment**:
Supplying one non-embedded Package Requirement as a Complete Package Tree. The
core canonicalizes and verifies the entire supplied tree against the requirement
before Typst compilation. Optional provenance and cache-hit fields are
caller-provided operational report metadata; they are not authenticated,
identity-bearing, or semantic inputs.

**Package Embedding**:
The Pack Creation choice to embed every selected Complete Package Tree or record
selected packages for external fulfillment. The choice affects Pack Identity and
self-containment.

## Fonts

**Font Container**:
The exact bytes of one standalone font file or multi-face font collection. Its
Canonical Identity is independent of source location, and every face in a
collection travels in the same container.

**Candidate Font Catalog**:
The ordered sequence of Font Containers Pack Creation may select faces from,
each carrying its own embedded-or-external disposition. Faces are expanded in
container-local index order, catalog order decides which container offers a
family, and nothing joins a supplied catalog implicitly. A Creation Adapter
composes it during Creation Preparation; the reference filesystem adapter
composes system fonts, Typst's embedded fonts, and scanned font directories, in
that order.

**Font Face Identity**:
One exact face within a Font Container, identified by container identity and
container-local face index. Family, style, coverage, and other metadata are
derived from the verified container.

**Font Requirement**:
One required Font Container and the nonempty set of Font Face Identities selected
during Pack Creation. The requirement records whether the container is embedded
or externally fulfilled.

**Font Authority**:
The explicitly configured responsibility that supplies the Candidate Font
Catalog during Creation Preparation or obtains exact Font Containers for
compilation. In the core, compilation receives already acquired Font Container
Fulfillments rather than a public authority interface.

**Pack Font Catalog**:
The ordered projection of the Candidate Font Catalog containing exactly the Font
Face Identities available to Pack compilation. Relative selection order is
preserved. Other faces physically present in a required Font Container remain
unavailable unless declared.

**External Font Fulfillment**:
Supplying one non-embedded Font Requirement as exact Font Container bytes. The
core verifies container identity and every declared face before Typst
compilation. Optional provenance and licensing fields are caller-provided
operational report metadata; they do not establish authenticity or legal
permission and do not contribute to identities.

**Font Embedding**:
The Pack Creation choice to embed selected Font Containers or record them for
external fulfillment. It is declared per container in the Candidate Font
Catalog and never inferred from container bytes, so one Pack may embed one
container and reference another, and Typst-embedded-font handling is an
explicit creation option. The choice affects Pack Identity and
self-containment; typst-pack does not derive a per-font legal policy from
licensing metadata.

## Compilation Requests

**Typst Target**:
The selected Typst document model: Paged or HTML. Pack Creation selects one target
for its representative package/font run. A Compilation Output Specification
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
Compilation Result. Shipped outcomes cover missing, malformed, or identity-
mismatched external package and font fulfillment. They retain request inventory,
Compilation Identity, and fulfillment reporting through Compilation Report.

**Canonical Compilation Diagnostic**:
One ordered compiler or exporter diagnostic containing severity, message, logical
span, hints, tracepoints, phase, producer, and an optional Source Page Number.
Messages and source-derived values remain untrusted presentation data.

**Pack Compilation Warning**:
One Pack-owned warning produced while preparing or exporting an accepted
compilation. It is distinct from official compiler or exporter diagnostics and
from representative-compile warnings returned by Pack Creation.

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
