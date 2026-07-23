# ADR-0006: Pack the Structural Project Closure

## Status

Accepted

## Context

Pack creation selected project files by tracing one or more representative
Typst compilations. That made ordinary project membership depend on concrete
targets, inputs, features, and control flow, then required explicit inclusions,
persisted coverage evidence, and assembled-Pack replay to explain and verify the
selected closure. Resource Slots added a second project-file role whose bytes
could be absent during later compilation.

Typst already gives a project one explicit root. Treating that root as the
project boundary makes membership predictable across branches and compilation
requests, at the cost of larger Packs and requiring users to exclude unrelated
or sensitive files deliberately.

## Decision

Pack creation stabilizes the complete eligible regular-file tree beneath the
physical project root. Every file enters the Pack except paths excluded by the
Project Ignore Policy. Compiler observations never select project files.

The filesystem adapter reads one root `.typkignore`. Nested `.typkignore` files
are ordinary project files. Rules are Gitignore-style, with comments, negation
and last-match precedence, root anchoring, directory-only matching, basename
matching, and recursive wildcards. The root policy file is always included.
Every `.typk` path is excluded by a non-overridable built-in rule. No other
ignore file is consulted.

Only regular files beneath the physical root are eligible. Symlinks and other
filesystem entry kinds are rejected unless conclusively ignored, as are
unreadable eligible files, unrepresentable paths, malformed ignore rules, and
traversal failures. Mutable filesystem adapters revalidate eligible membership
and bytes before Pack Issuance; changes confined to conclusively ignored
subtrees are irrelevant.

Creation performs one representative compile from the stabilized Project
Snapshot. The optional `--target <paged|html>` defaults to `paged` and selects
only that run; it does not restrict Pack outputs. Typst exposes packages and
fonts reached by one concrete evaluation rather than every potential
dependency, so this target-specific run is a temporary workaround until a
better package and font closure mechanism exists. Inputs, Compilation Document
Time, features, package and font authorities, and embedding policy still affect
the run or resulting dependency closure.

The Creation Request and its access observations are transient. Discovery
Variants, coverage identities, persisted Discovery Traces, explicit project
inclusions, discovery-only Pack Overrides, and assembled-Pack replay are
removed. Whole-Pack validation and the Creation Evidence Fence remain.

Resource Slots and Resource Providers are removed. Every contained project path
has baseline bytes, and the representative Creation Request must compile from
those bytes. Variable assets use contained placeholders plus compilation-time
Pack Overrides. An override may replace any contained project file, including
Typst source and the entrypoint, but cannot add or delete paths, replace package
or font content, or authorize undeclared dependencies. Override-driven requests
for absent package or font requirements fail normally.

Package and font embedding and exact external fulfillment remain unchanged.
Project-root resolution continues to follow Typst: the canonical entrypoint's
parent is the default, `--root` and `TYPST_ROOT` may select an ancestor, and the
entrypoint must be beneath the physical root. `typst.toml` has no special case;
every eligible instance is included structurally.

The Pack format remains version 1. Its unstable schema changes incompatibly in
place, with no legacy reader, compatibility aliases, or retained interpretation
of removed discovery and Resource Slot fields.

## Consequences

- Pack contents no longer vary with which project paths one concrete compile
  happens to read.
- Packs may be larger and may include unrelated or sensitive files unless the
  Project Ignore Policy excludes them.
- A Pack is project-file-complete, while its package and font completeness is
  still limited to one representative creation evaluation.
- Later compilation remains output-independent and may override any contained
  project file, but cannot expand the fixed project namespace or dependency
  authority.
- Creation, compilation, inspection, extraction, manifest, CLI, Rust, Dagger,
  documentation, and tests lose all Resource Slot and discovery-coverage
  surfaces.

This decision supersedes ADR-0004 and the creation-discovery portions of
ADR-0005.
