# ADR-0017: Require Limits Only at Trust Boundaries

## Status

Accepted

Amends ADR-0012.

## Context

ADR-0012 made every resource ceiling a required parameter so that every bound
is a deliberate choice, and gave each operation its own limits, resource, and
limit-error family. The August 2026 architecture review
(docs/research/2026-08-architecture-review.md) measured the outcome: 13
`*Limits` structs, 13 `*Resource` enums, 26 error types distinguishing
misconfigured ceilings from exceeded ceilings, 25 `reference_v1()` profiles —
and zero call sites in the repository, first-party CLI included, that
construct anything other than the reference profile. The deliberate choice
the rule was designed to force is not being made in practice, while the
required parameter taxes every caller of every operation equally, whether or
not the operation faces untrusted input.

## Decision

Resource ceilings remain required parameters only at trust boundaries: any
operation whose input bytes originate outside the caller's control, such as
Pack Archive decoding, package archive expansion, and bounded acquisition
from storage. Operations over caller-controlled values, such as compilation
and encoding, resolve documented default ceilings; an explicit
`*_with_limits` variant remains for callers that need different bounds.

One parameterized limits family replaces the per-operation copies: a single
limits type, resource enum parameter, and exceeded-ceiling error shared by
the core and both adapters, with per-operation presets preserving the current
reference ceilings. This is a shared data shape under ADR-0016.

A misconfigured ceiling is a programmer error, not a runtime condition:
limits construction panics on an invalid configuration instead of returning a
dedicated error type. The plus-one-probe invariant (a ceiling must leave room
for observing one element past it) is kept as an internal implementation
detail rather than a public error taxonomy.

ADR-0012's payload-sharing decision and its rule that ceilings are
operational policy excluded from identities are unchanged.

## Consequences

- The `*LimitsError` class (misconfigured ceilings) is deleted; roughly 39
  limits-related types collapse into three.
- `compile(request)` and `encode(pack)` become callable without a limits
  argument; decode and expansion keep their required bound, so an
  attacker-supplied input can never run unbounded by omission.
- Reference ceilings stay behaviorally identical, so existing boundary and
  plus-one-probe tests translate to the shared family rather than being
  weakened.
- A future operation must classify its input as trusted or untrusted to know
  whether its limits parameter is required; that classification belongs in
  the operation's documentation.
