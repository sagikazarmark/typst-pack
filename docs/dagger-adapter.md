# Dagger adapter contract

The first-party Dagger module is a typed transport adapter over the public
`typst-pack` CLI and Pack lifecycle. `create` stabilizes Dagger `Directory` and
`File` inputs in a container before Pack creation. `compile` invokes
`typst-pack compile`, which prepares a public `PackCompilationRequest` and
reaches the shared Pack Compilation Kernel. The module contains no compiler,
exporter, semantic default, diagnostic interpretation, or artifact
postprocessing.

Compilation inputs map as follows:

- `sysInputs`, `features`, `creationTimestamp`, page selection, PPI, PDF
  controls, and format map to their typed CLI controls;
- `packageDir`, `fontDir`, and their path controls fulfill exact Pack
  dependencies through the CLI authorities;
- `overrideDir` is an immutable project-shaped Dagger value, while
  `overridePaths` selects contained project paths whose same relative files are
  passed as Pack Overrides; and
- Resource Provider directories remain separate from Pack Overrides and can
  satisfy only declared Resource Slots.

The CLI completes compilation before the adapter returns the staged Dagger
`Directory`; `create` similarly completes Pack issuance before returning its
`File`. Dagger values and Compilation Results are immutable. Staging, later
queries, and exports cannot mutate semantic artifacts or status. A nonzero CLI
result is raised with its diagnostics as a compilation error. Failures that
occur while querying or exporting the returned `File` or `Directory` remain
later Dagger delivery errors and cannot be reported as compilation failures.

The adapter intentionally omits local output paths, stdout, terminal color,
viewer launch, timing files, dependency files, arbitrary environment defaults,
and Bundle output. It always stages Document Formats as `output.pdf` or
`output.html`, and Page Formats as `page-{0p}.png` or `page-{0p}.svg`. These are
transport differences after immutable Compilation Results. Native differential
tests remain the authority for compiler, diagnostic, and artifact parity; the
Dagger suite tests only typed mapping, artifact roles, immutability, and the
adapter failure boundary.
