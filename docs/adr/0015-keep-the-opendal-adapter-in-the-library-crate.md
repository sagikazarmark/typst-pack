# ADR-0015: Keep the OpenDAL Adapter in the Library Crate

## Status

Accepted, amended by ADR-0016

## Context

The first-party OpenDAL integration adds asynchronous storage adapters for Pack
Assembly, Pack Archive reading and writing, compilation input reading, Pack
Extraction writing, package-cache writing, and Compilation Output Artifact
writing. These workflows hand exact bytes to or
from existing core constructors and plans; OpenDAL does not replace those core
semantic authorities.

The integration needs crate-private access to those authorities. In particular,
the shared read work in #216 and #223 uses core path validation and
construction seams, the write execution in #197 applies existing plans
without redefining them, and the package and font workflows use the centralized
layout authority from #191. Making `typst-pack-opendal` a companion crate would
therefore require exposing that machinery as public API or duplicating semantic
rules across crates.

OpenDAL's `Operator` is part of the adapter's public API. Dependency selection
must consequently allow a downstream caller to supply its own compatible
`Operator` without forcing an exact OpenDAL release or creating an incompatible
duplicate type.

## Decision

Ship the OpenDAL adapter in the `typst-pack` library crate behind the non-default
`opendal` feature, under the `typst_pack::opendal` module. OpenDAL modules may
depend on existing core constructors, validators, path and layout authorities,
and write plans. Core semantic modules never depend on OpenDAL.

Shared read, write, and scheduling mechanics remain crate-private.
Public interfaces are operation-specific requests, errors, outcomes, receipts,
progress, and other evidence. The integration does not expose a universal
storage abstraction, source, sink, plan, scheduler, error, or conformance trait.
Capability appraisal is likewise private. In accordance with ADR-0013, a public
operation that cannot honor a requested write policy reports
`UnsupportedPolicy` rather than exposing backend capabilities or weakening the
policy.

The alternative of a companion `typst-pack-opendal` crate is rejected. It would
turn core path validation, package and font layout, construction, write
planning, and related seams into public cross-crate API solely for one adapter,
or force the adapter to duplicate them. In-crate placement preserves those
authorities as implementation details and keeps dependency direction toward the
core.

This placement does not conflict with ADR-0007. That decision separates the
reusable library from CLI process concerns and dependencies; it does not require
adapters to live in separate crates. The `fs`, `egress`, and
`package-reading` features already ship first-party adapter and reading
capabilities in the library crate.

Callers construct and control Operators and own backend features, credentials,
transport, executors, runtimes, TLS configuration, layers, retry policy, and
deployment-level concurrency. The library installs none of these implicitly and
does not own a runtime. Typst-pack owns bounded in-flight read fan-out
inside one operation because its retained-memory contract requires it; that local
scheduling bound does not become deployment concurrency policy.
`OperatorBindings` may retain cheap Operator clones supplied by the caller
without taking over backend policy.

Declare OpenDAL as a caret requirement with no exact pin:

```toml
opendal = { version = "0.58", default-features = false, optional = true }
```

Do not depend directly on `opendal-core`. An exact `=0.58.x` requirement in a
published library whose public API accepts caller-supplied `Operator` values can
make the graph unsatisfiable for a downstream pinned to another patch. A
downstream using a later OpenDAL minor can instead resolve two copies, but its
`Operator` is then a different Rust type from typst-pack's `Operator`. Neither
problem has a consumer-side conversion or dependency override that preserves
the public call. The caret requirement admits compatible 0.58 patch releases
while an OpenDAL minor upgrade remains an explicit typst-pack compatibility
decision.

Production code relies on no `opendal::raw` item. The complete raw-API inventory
is test-only:

- `opendal::raw::normalize_path` is the oracle for #207's differential test of
  the crate-private vendored normalization predicate.
- `opendal::raw::Access` is implemented by #209's deterministic listing/read
  test service and #217's deterministic write test service.

These test couplings may require maintenance when OpenDAL changes, but cannot
break a typst-pack consumer or force unstable raw types into production or
public API. Keeping normalization vendored and all raw use in tests is what
makes the `0.58` caret requirement liftable across compatible patches.

Enabling `opendal` changes the engine and exporter `ImplementationIdentity`
values because
`build.rs::enabled_feature_set()` includes every enabled crate feature in the
implementation attestation. Those changes flow into Compilation Identity and
Compilation Result Identity. This is accepted even when storage selection has
no semantic effect on compilation or output bytes. It is pre-existing behavior
of the all-crate-feature attestation contract, not a new identity rule introduced
by OpenDAL: `fs`, `parallel`, and `diagnostics` already change these identities
under the same rule. Identity schemas and featureless frozen vectors do not
change.

`Location`, `OperatorBinding`, `OperatorBindings`, `OperatorResolver`,
`WritePolicy`, operation-specific `...Source` entries, and the read, progress,
receipt, and outcome families are adapter vocabulary. They describe
how bytes are addressed and moved, not what a Pack is. Their contracts belong in
the OpenDAL integration guide rather than
`CONTEXT.md`. This work leaves the domain glossary unchanged because it neither
introduces a domain concept nor changes an existing domain term's contract.

## Consequences

- OpenDAL workflows reuse private core authority without widening the core's
  public API or duplicating semantic rules.
- The optional adapter adds no backend, transport, TLS, runtime, retry, layer, or
  deployment-concurrency choice on the caller's behalf.
- Public workflow types remain specific enough to expose their actual policy,
  evidence, and recovery contracts while private mechanics can be shared.
- Consumers can select compatible OpenDAL 0.58 patch releases and pass their
  Operators directly; moving to another minor requires a typst-pack review.
- OpenDAL raw-API drift can break development tests but not production code or
  downstream type compatibility.
- Builds that enable `opendal` intentionally receive different implementation,
  compilation, and result identities under the existing feature attestation
  contract.
