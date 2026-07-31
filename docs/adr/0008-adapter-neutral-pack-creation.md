# ADR-0008: Adapter-Neutral Pack Creation

## Status

Accepted, amended by ADR-0009 and ADR-0011

## Context

Pack Creation is only reachable on a host with a writable filesystem. A
consumer whose project lives in object storage, or who wants to create Packs
inside a wasm guest or any sandbox without a filesystem, cannot use the library
to create a Pack at all. Materializing the whole project into a temporary
directory first is not possible on those hosts, and assembling a Pack by hand
from already-resolved packages and fonts gives up the representative compile
that discovers dependencies in the first place.

The filesystem requirement also bundles capabilities a deployment may not want.
Filesystem access, network egress, and system font discovery arrive together
behind one crate feature, so a service that packs projects can suppress
downloading at runtime yet still link an HTTP stack into its binary. In a
container, "system fonts" means whatever the base image ships, so Pack contents
become a function of the image rather than of an explicit choice.

The featureless core already compiles for `wasm32-unknown-unknown`, and Pack
compilation already runs there. Creation is the only lifecycle stage excluded
from that target.

## Decision

Pack Creation is adapter-neutral. Given a stabilized Project Snapshot, an
ordered candidate font catalog, and resolved package trees, it runs one
representative Typst request and returns one Pack plus representative-compile
warnings. It acquires nothing itself, requires no crate feature, and runs
wherever the core runs.

Creation Preparation names the acquisition phase that obtains those inputs, and
a Creation Adapter is what performs it. An adapter supplies only listing,
reading, and fetching. Every transformation of bytes belongs to the core:
ignore-policy matching, Project Snapshot assembly, font container expansion,
package registry URL construction, and package archive expansion.

Because package requirements can only be discovered by compiling, acquisition
resolves through a resumable protocol rather than a callback. Creation reports
the exact package specifications it observed as missing, the caller obtains
those trees however its host allows, and invokes creation again with the same
owned Creation Request values. Reporting missing specifications is a normal
resumable outcome, not a failure, and those specifications are derived from the
package requests the compiler actually made rather than recovered from
diagnostic text. A supplied tree that does not satisfy the specification it was
supplied under is a distinct failure, so a resume loop cannot spin without
progress. Nothing is retained between invocations, so a caller doing
asynchronous I/O never has to hold library state across a suspension point,
which is what makes this work on hosts whose only network access is
asynchronous. The core requires an explicit creation timestamp and performs no
wall-clock or working-directory lookup.

Project membership stops depending on a filesystem. The Project Ignore Policy
is parsed from ignore-file bytes and matches against a path without any
filesystem access, so an adapter can filter a listing before paying to read
content. Project Snapshot assembly re-applies the policy to the path-and-bytes
entries it accepts, so the membership invariant does not depend on adapters
being well-behaved, and the non-overridable `.typk` exclusion binds every
caller including direct in-memory construction.

The candidate font catalog is one ordered sequence of Font Containers, and
resolved package trees are supplied per package specification. Each carries an
explicit embedded-or-external disposition, and the core appends nothing
implicitly, so Pack contents are not a function of which crate features a build
enabled.

Filesystem access and network egress become separately selectable capabilities,
so a build can read a project from disk with no download capability compiled in
at all. The existing filesystem path is reimplemented as the reference Creation
Adapter over the core, with its public interface unchanged. That adapter
resolves the creation timestamp, composes system fonts, Typst's embedded fonts,
and scanned font directories into the candidate catalog in their current
relative order, and continues to own the Creation Evidence Fence and its
filesystem failure vocabulary.

Establishing that acquired bytes represent one consistent source state is a
Creation Adapter responsibility, and it is advisory. An adapter that acquires
from mutable storage without revalidating still conforms.

This decision amends ADR-0006. The structural project closure and the Project
Ignore Policy it decided are unchanged; what changes is their binding to the
filesystem. It does depart from ADR-0006 on one point: creation may now run the
representative compile once per resume round rather than exactly once, because
that is the only way to discover packages the caller has not yet supplied.

## Consequences

- Pack Creation runs wherever Pack compilation already does, including
  `wasm32-unknown-unknown`, without selecting a crate feature.
- Non-filesystem adapters may produce Packs from inconsistent source states
  without detection, describing a source state that never existed
  simultaneously. The obligation to prevent it is documented, not enforced.
  The filesystem adapter keeps the Creation Evidence Fence and loses nothing.
- Package discovery costs one representative compile per resume round in the
  worst case, because a failed import ends module evaluation. Typst's
  memoization cache is not evicted between rounds, so round count affects
  memory as well as latency.
- Identical project bytes produce an identical Pack Identity regardless of
  which adapter obtained them, so adapter divergence is testable rather than
  discovered in production.
- Nothing joins the font catalog implicitly, so a caller that supplies the
  catalog itself gets Pack contents that do not depend on which crate features
  a build enabled or which font sources a container image ships. A caller that
  wants Typst's embedded fonts splices them in explicitly. The reference
  filesystem adapter still composes ambient sources, so it keeps today's
  dependence on the host.
- The absence of network egress becomes verifiable from the dependency graph
  rather than from a runtime flag.
- Existing filesystem callers are unaffected: the CLI, the Dagger adapter, and
  library users of the filesystem creation interface.
