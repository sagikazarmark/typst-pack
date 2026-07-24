# Embedded Typst upgrade procedure

The embedded Typst release is an implementation authority, not a loose
compatibility range. `embedded-typst.toml` is the machine-readable approved
release set and differential inventory. `build.rs` rejects an unapproved,
duplicated, non-exact, or checksum-mismatched Typst crate before typst-pack can
build.

## Upgrade inventory

Before changing any Typst dependency, compare the old and new official release
and record the result for every item below in the upgrade change or its linked
issue:

- public compiler, `World`, `Library`, document, layout, and diagnostic APIs;
- all compiler options, defaults, feature flags, and target requirements;
- diagnostic messages, spans, hints, traces, ordering, rendering, and exit
  behavior;
- package resolution, package manifests, file requests, and offline behavior;
- font discovery, metadata, selection, embedded fonts, and fallback behavior;
- PDF, PNG, SVG, and HTML exporter APIs, defaults, output bytes, warnings, and
  rejection behavior;
- official CLI commands, flags, aliases, parsers, environment variables, help,
  defaults, conflicts, output planning, dependencies, timings, and viewing;
- every intentional Pack difference in ADR-0005 and `docs/cli-parity.md`;
- the official release artifact platform and the cargo-dist artifact used by
  release verification.

## Required changes

1. Update every direct Typst dependency in `Cargo.toml` to an exact `=VERSION`
   pin. Run `cargo update` so `Cargo.lock` contains one compatible release of
   every `typst` and `typst-*` crate.
2. Update every `[[crate]]` entry in `embedded-typst.toml`, including transitive
   crates, checksums, the Engine baseline, and all Exporter Identity sources.
   Do not add an exception implicitly; explain intentional patch-version splits
   such as `typst-timing` in the upgrade change.
3. Update `[official-cli]` in `embedded-typst.toml`, the one authoritative
   declaration of the official binary version, URL, and SHA-256 digest. Verify
   the published digest independently before changing this pin. Dagger and the
   release workflow both consume it through `scripts/install-official-typst.sh`;
   do not update either consumer separately.
4. Review every `[[matrix]]` and `[[semantic]]` entry. New custom behavior must
   be added with coverage and exactly one classification:
   `upstream-behavior`, `pack-invariant`, `adapter-concern`,
   `intentional-pack-difference`, or `unavoidable-mirror`. Represent
   differential matrix coverage with a typed entry in
   `tests/support/differential.rs`; the embedded gate requires every classified
   category to name at least one executable differential suite.
5. Update frozen assertions or differential expectations only with a recorded
   classification. The change description must state whether each changed
   observation is upstream behavior, a Pack invariant, an adapter concern, an
   intentional Pack-specific difference, or an unavoidable mirror. An
   unexplained expectation update is not an approved baseline change. Git
   records source and test changes; the review gate intentionally does not
   maintain duplicate whole-file or whole-tree content digests.
6. Update `docs/cli-parity.md` and ADR-0005 when the shared behavior inventory
   or an intentional difference changes.

## Verification

Run focused checks while upgrading:

```console
cargo check --locked --all-features --all-targets
cargo test --locked --all-features --test embedded_typst_gate
cargo test --locked --all-features --test official_typst_oracle
```

Run the process gate with the exact official binary and the first-party binary
being released:

```console
TYPST_PACK_REQUIRE_OFFICIAL_TYPST=1 \
TYPST_PACK_OFFICIAL_TYPST=/path/to/official/typst \
TYPST_PACK_TEST_BINARY=/path/to/packaged/typst-pack \
cargo test --locked --all-features --test official_typst_cli
```

Finish with `dagger check`. The embedded gate verifies that Dagger and release
verification consume the canonical pin without copied literals. Dagger's typed
tests exercise the download, digest, extraction, and version checks while
verifying representative adapter delegation, and deliberately leave native
semantic coverage to the Rust differential matrix. The release workflow
extracts the cargo-dist Linux archive and reruns the same mandatory process gate
against that exact binary before `host` can publish any artifact. A missing or
replaced reference binary, digest or version mismatch, artifact difference,
diagnostic difference, unclassified semantic, or incomplete matrix fails the
release.
