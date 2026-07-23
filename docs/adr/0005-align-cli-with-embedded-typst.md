# ADR-0005: Align the CLI with Embedded Typst

## Status

Accepted

## Context

`typst-pack compile` invokes the Typst compiler but had accumulated different
spellings, parsers, environment variables, help text, and exporter behavior from
`typst compile`. Some differences express the Pack compilation contract, while
others are accidental and make existing Typst knowledge unreliable.

## Decision

The CLI parity baseline is the exact Typst engine version embedded by
typst-pack. Shared flags track that version's spelling, values, parsing,
environment variables, semantics, and help text. A Typst engine upgrade includes
an explicit CLI parity review. `typst-pack --version` reports both versions.
The Pack-backed `World`, Typst compiler call, and official exporter calls remain
private to the Embedded Typst Adapter; the CLI supplies only Pack-bound request
and operational values.

The command shapes are:

```text
typst-pack create [OPTIONS] <INPUT> [OUTPUT]
typst-pack compile [OPTIONS] <PACK> [OUTPUT]
```

`create` requires an input Typst file and removes directory-as-input and
`--entrypoint`. Its `--root` behavior, `TYPST_ROOT` environment support, and
default root match Typst. The default Pack output replaces the input extension
with `.typk`; `OUTPUT=-` writes the Pack to stdout. Source input from stdin is
not supported because a Pack must retain a stable project-relative entrypoint.
The Pack Manifest continues to call that persisted role `entrypoint`.

`compile` retains `<PACK>` rather than calling it `<INPUT>`. `PACK=-` reads a
Pack from stdin and then requires an explicit output. There is no compile-time
`--root`: the Pack owns the virtual project tree, and Resource Providers can
fill only declared Resource Slots. `OUTPUT=-` writes stdout when exactly one
output file is emitted and is rejected for multi-file output.

Shared Typst behavior includes:

- `--input` parsing, including whitespace trimming and empty-key rejection;
- case-insensitive output-format inference and Typst's supported extensions;
- Page Format templates, total-page semantics, aliases, and page-range grammar;
- `--pretty`, `--pages`, `--ppi`, `--pdf-standard`, and `--no-pdf-tags`;
- `--creation-timestamp` and `SOURCE_DATE_EPOCH` validation and timestamp
  behavior;
- `--font-path`, embedded/system font controls, path-list parsing, and Typst's
  font environment variables;
- `--package-path`, `--package-cache-path`, and Typst's package environment
  variables;
- `--features` for every applicable embedded feature, including `html` and
  `a11y-extras`;
- `-j/--jobs`, `--diagnostic-format`, `--timings`, and `--open [VIEWER]`;
- `--deps` and `--deps-format`; and
- global `--color` and `--cert`/`TYPST_CERT` controls.

`compile` exposes the visible `c` alias. Typst source diagnostics are not
followed by an extra generic `error: compilation failed` line.

Page Format output requires an indexable page-number template when more than
one page is emitted; typst-pack does not invent a default template. Document
Format output paths are literal. Valid multi-output paths may still be
preflighted before writing to avoid partial output or duplicate destinations.

Dependency output has Pack-aware host-rebuild semantics. It reports the Pack
path once for contained content, plus actual filesystem-backed Resource Slot
and unvendored package files read during compilation. It does not invent host
paths for archive members or opaque providers.

Pack creation performs one strict discovery compile for each selected
`--target <paged|html>` and unions their dependencies. The option is repeatable
and comma-delimited and defaults to `paged`. `create` reuses every
target-independent compilation control that affects discovery, including
inputs, timestamps, features, jobs, diagnostics, timings, fonts, packages,
certificates, and color. Export-, page-, and PDF-specific options remain
compile-only.

Pack-specific additions remain explicit:

- `--resource-path <DIR>` configures ordered filesystem Resource Providers as
  established by ADR-0004;
- `--offline` prevents package downloads;
- `create --resource-slot <PATH>` declares Resource Slots;
- `create --no-vendor-packages` controls package storage; and
- `compile --override PACK_PATH FILE` supplies one Pack Override; and
- creation retains project inclusion, font embedding, and Pack metadata
  controls.

Routine CLI help uses Typst's plain output-file, output-format, and page-number
wording. Formal terms such as Compilation Output Artifact, Document Format,
Page Format, and Source Page Number remain available to library and domain
documentation where their distinctions matter.

Help is grouped by user task rather than copied as Typst's flat list.
`compile` uses Compilation, Output, PDF, Resource Slots, Fonts, Packages, and
Diagnostics & Automation sections. `create` uses Project, Discovery, Pack
Contents, Resource Slots, Fonts, Packages, Metadata, and Diagnostics &
Automation sections.

The Dagger API follows semantic rather than transport parity. It exposes
compiler behavior that has a meaningful typed representation, but not terminal
viewing, stdout, color, or arbitrary local output paths. Its creation inputs use
`project`, `input`, and `sysInputs`; ordered `resourceDirs` are mounted and
registered automatically, and explicit declarations are `resourceSlots`.

## Intentional Differences

- Typst Bundle output and its feature are not supported.
- A `watch` command is deferred because correctly watching Packs, selected
  Resource Provider files, packages, and font paths requires a separate
  dependency/provenance design.
- `create` does not accept source input from stdin.
- `compile` has no `--root` and consumes a Pack rather than a Typst source file.
- Pack-contained fonts and vendored packages remain authoritative layers ahead
  of host configuration.
- Pack Overrides replace only contained project files for one compilation and
  never mutate the Pack.
- The hidden deprecated `--make-deps` compatibility option is not adopted.

## Consequences

- Existing Typst knowledge applies by default; each divergence is a Pack
  contract decision rather than accidental drift.
- Shared argument definitions and parsers should be reused by `create` and
  `compile`, while help headings remain task-specific.
- CLI tests need to cover upstream-compatible parsing, environment variables,
  help text, stdout conflicts, output templates, and every intentional
  difference.
- New Typst compile options are not adopted blindly: each engine upgrade checks
  whether the option applies to Pack compilation and records exclusions.
- The machine-enforced baseline and maintainer checklist are defined by
  `embedded-typst.toml` and `docs/embedded-typst-upgrade.md`.
