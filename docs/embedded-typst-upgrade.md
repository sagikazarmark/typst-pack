# Embedded Typst upgrade procedure

typst-pack embeds one exact Typst release. Typst crates share internal types,
so every `typst` and `typst-*` crate must resolve to that release.
`crates/typst-pack/embedded-typst.toml` declares it (`[typst] version`) and the
official CLI used as the differential oracle (`[official-cli]`). `build.rs`
only reads the version to report it in `typst_pack::VERSION`; the pinning
policy is checked by `crates/typst-pack/tests/embedded_typst_gate.rs`, which
runs with the workspace tests.

## Upgrade inventory

Before changing any Typst dependency, compare the old and new official release
and note in the upgrade change what changed in:

- compiler, `World`, `Library`, document, and diagnostic APIs;
- compiler options, defaults, feature flags, and target requirements;
- package resolution and font discovery, selection, and fallback;
- PDF, PNG, SVG, and HTML exporter APIs, defaults, and output;
- official CLI commands, flags, defaults, and environment variables
  (see `docs/cli-parity.md`).

## Required changes

1. Pin every direct Typst dependency (workspace `Cargo.toml`,
   `crates/typst-pack/Cargo.toml`, `crates/typst-pack-cli/Cargo.toml`) to
   `=VERSION` and run `cargo update` so `Cargo.lock` resolves each `typst` and
   `typst-*` crate exactly once at `VERSION`. Do the same in
   `fuzz/Cargo.toml` and `fuzz/Cargo.lock`, which the gate does not check.
2. Set `[typst] version` in `embedded-typst.toml`. The engine version is part of
   `typst_pack::VERSION`, so every compilation identity changes with it.
3. Update `[official-cli]` (version, platform, URL, SHA-256) in
   `embedded-typst.toml`.
   Verify the published digest independently. `just test-cli-parity` and the
   release workflow both install the oracle through
   `scripts/install-official-typst.sh`; do not copy the pin elsewhere.
4. Update differential expectations only with a stated reason in the change:
   upstream behavior, a pack invariant, or an intentional difference recorded
   in `docs/cli-parity.md`.
5. Update the prose that names the release: `docs/cli-parity.md`, the module
   doc of `crates/typst-pack-cli/src/cli.rs`, the README's example manifest and
   `implementation` example, and the advisory reasons in `deny.toml` and
   `.cargo/audit.toml`.

## Verification

```console
cargo test --locked -p typst-pack --test embedded_typst_gate
cargo test --locked -p typst-pack --all-features --test typst_parity
TYPST_PACK_REQUIRE_OFFICIAL_TYPST=1 \
TYPST_PACK_OFFICIAL_TYPST=/path/to/official/typst \
TYPST_PACK_TEST_BINARY=/path/to/packaged/typst-pack \
cargo test --locked -p typst-pack-cli --all-features --test official_typst_cli
```

Finish with `just ci` and `just test-cli-parity` (x86_64 Linux). Before a
release, the release workflow runs the pin gate, extracts the cargo-dist
Linux binary, installs the pinned official CLI, and runs the workspace tests,
whose official CLI differential runs against that exact binary.
