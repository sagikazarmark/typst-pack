# ADR-0012: Share Payloads and Bound Resource Amplification

## Status

Accepted

## Context

Lifecycle values contain large immutable byte payloads, while archive parsing,
package expansion, source acquisition, and artifact export can amplify external
input. Adding an independent budget to every value and collection would spread
policy across semantic interfaces without preventing allocation that already
happened. Removing all limits would leave compressed and generated workloads able
to exhaust the host.

## Decision

Resource ceilings are operational policy, not semantic validity. They do not
contribute to canonical identities, and the same logical input may succeed under
a larger profile. Low-level operations require finite typed limits only where
they acquire external input or amplify work: source-specific traversal and
reads, Pack Archive acquisition, decoding and encoding, Package Archive
Expansion, and compilation artifact export. Already-materialized Project
Snapshot, Package Tree, catalog, fulfillment, override, and semantic plan
construction receives no additional resource profile.

Payload-bearing semantic values use private immutable shared allocations.
Moving an owned byte vector into the core does not copy its payload, accessors
borrow slices, and cloning a value never copies payload bytes. Pack Archive bytes
remain a uniquely owned, non-cloneable vector because exact retry material should
have explicit ownership.

Core transformations and compilation remain synchronous. Source and destination
adapters choose blocking or asynchronous I/O outside those seams. Internal
artifact-export parallelism is explicitly bounded per invocation and preserves
canonical result and error order.

Each low-level limits type has private validated fields and no `Default`, optional
ceiling, or unlimited value. High-level first-party workflows select one named
reference profile. Accounting uses checked `u64` arithmetic, rejects known
excess before expensive work, meters incremental work otherwise, and returns no
partial semantic value. A post-generation artifact-byte check is a retained
result limit, not a process-memory guarantee.

## Consequences

- Project Snapshot assembly no longer owns a separate budget; each source
  gatherer bounds traversal and reads before assembly.
- Peak-memory contracts describe payload allocations and bounded working memory,
  not allocator overhead, compiler internals, process RSS, elapsed time, or
  aggregate concurrency across independent invocations.
- Allocation tests remain limited to operation-specific ownership seams;
  a dedicated cross-type pointer-identity matrix is intentionally omitted
  because it couples unrelated values to their current representations.
- Hosts processing hostile Typst programs still need process isolation and
  external time or memory enforcement.
