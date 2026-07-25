# ADR-0007: Separate the Library and CLI Crates

## Status

Accepted

## Context

The `typst-pack` package contained both the reusable library and the
`typst-pack` executable. The binary was gated by a `cli` feature, so Cargo
installation required an otherwise unnecessary feature selection and library
users shared one package manifest with process-only dependencies.

The CLI also needs first-party diagnostic source context, dependency
observations, and timed execution around the private Embedded Typst Adapter.
Moving the source file alone would either expose that adapter indiscriminately
or duplicate its compilation path.

## Decision

The workspace publishes two crates:

- `typst-pack` provides the Pack library;
- `typst-pack-cli` provides the `typst-pack` executable and depends on the exact
  matching `typst-pack` release.

The library has no `cli` feature. The CLI enables capability-oriented library
features instead: `diagnostics` retains first-party presentation context,
`parallel` parallelizes independent page exports, and the existing `fs` and
`embedded-fonts` features provide its host adapters.

The hidden `cli_support` module is the narrow seam used by the first-party CLI
to orchestrate timing and Typst-compatible diagnostics without introducing a
second semantic compilation path. It is not part of the documented general
library interface.

Both crates share one workspace version and release. The library must be
published before the CLI because crates.io resolves the CLI's exact library
dependency during packaging.

## Consequences

- Users install the command with `cargo install typst-pack-cli` and run
  `typst-pack` without selecting features.
- Library users no longer resolve Clap, Chrono, terminal, viewer, or JSON CLI
  dependencies.
- CLI process tests live with the binary crate, while semantic compilation
  tests remain with the library crate.
- CLI and library releases cannot be versioned or published independently.
