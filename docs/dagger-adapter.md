# Dagger adapter contract

The first-party Dagger module is a typed transport adapter over the public
`typst-pack` CLI and Pack lifecycle. `create` stabilizes Dagger `Directory` and
`File` inputs in a container before Pack creation. `compile` invokes
`typst-pack compile`, which prepares a public `PackCompilationRequest` and
reaches the single private Pack Compilation Kernel and Embedded Typst Adapter.
The module contains no compiler,
exporter, semantic default, diagnostic interpretation, or artifact
postprocessing.

Compilation inputs map as follows:

- `sysInputs`, `features`, `creationTimestamp`, page selection, PPI, PDF
  controls, and format map to semantic typed CLI controls; selecting HTML
  output derives the required HTML engine feature, so callers do not also
  select it;
- typed `packageDir` and mounted `fontDir` capabilities fulfill exact Pack
  dependencies through the CLI authorities;
- `overrideDir` is an immutable project-shaped Dagger value, while
  `overridePaths` selects contained project paths whose same relative files are
  passed as Pack Overrides.

Creation packs the supplied project `Directory` structurally. Its optional
singular target controls only the representative package/font selection run.

The CLI completes compilation before the adapter returns the staged Dagger
`Directory`; `create` similarly completes Pack issuance before returning its
`File`. Dagger values and Compilation Results are immutable. Staging, later
queries, and exports cannot mutate semantic artifacts or status. A nonzero CLI
result is raised with its diagnostics as a compilation error. Failures that
occur while querying or exporting the returned `File` or `Directory` remain
later Dagger delivery errors and cannot be reported as compilation failures.

The adapter intentionally omits diagnostic formatting, job counts, arbitrary
font path strings, local output paths, stdout, terminal color, viewer launch,
timing files, dependency files, arbitrary environment defaults, and Bundle
output. Its container image installs no system-font source, so the interface
also omits ineffective system-font controls. Callers can mount one typed font
directory, and can still control whether Typst's deliberately installed
embedded fonts participate.

The adapter always stages Document Formats as `output.pdf` or `output.html`,
and Page Formats as `page-{0p}.png` or `page-{0p}.svg`. These are transport
differences after immutable Compilation Results. Native differential tests
remain the authority for compiler, diagnostic, and artifact parity; the Dagger
suite tests typed mapping, schema absence, artifact roles, immutability, and the
adapter failure boundary.
