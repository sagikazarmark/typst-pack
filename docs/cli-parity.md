# Embedded Typst CLI parity inventory

This inventory is tied to Typst 0.15.0. It supplements
[ADR-0005](adr/0005-align-cli-with-embedded-typst.md) and must be reviewed before
the embedded Typst release changes. Follow the complete
[embedded Typst upgrade procedure](embedded-typst-upgrade.md); the approved
crate set and classified differential matrix live in `embedded-typst.toml`.

The process differential gate is `tests/official_typst_cli.rs`. Dagger downloads
the official Typst 0.15.0 release artifact, verifies its published SHA-256
digest, and exposes it through `TYPST_PACK_OFFICIAL_TYPST`. The test then checks
the binary's version against the `EngineIdentity` produced by public Pack
compilation. A missing oracle is permitted only outside that dedicated gate; a
missing, replaced, or version-mismatched oracle fails the gate.

## Shared behavior

| Behavior | Upstream authority or unavoidable mirror | Differential or focused coverage |
| --- | --- | --- |
| PDF, PNG, SVG, and HTML compilation and artifact bytes | Public Typst compiler and exporter crates | `official_typst_compile_is_the_process_level_parity_baseline`; `tests/official_typst_oracle.rs` |
| Typst inputs, features, document time, and PDF creation time | Public `Library`, `Features`, `World::today`, and exporter controls; CLI environment resolution is mirrored | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; `tests/official_typst_oracle.rs`; `tests/cli.rs` |
| Complete packages, exact fonts, Pack Overrides, and offline dependency authority | Public Typst `World` requests wrapped by Pack verification; offline is Pack policy | `tests/official_typst_oracle.rs`; `tests/compilation.rs`; package, font, override, and offline cases in `tests/cli.rs` |
| Diagnostic content, ordering, rendering, and process exit status | `SourceDiagnostic` and `typst-kit::diagnostics`; process exit policy is mirrored | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; diagnostic cases in `tests/official_typst_oracle.rs` and `tests/cli.rs` |
| Option spelling, aliases, value parsing, defaults, conflicts, and help wording | Clap adapter mirrors Typst 0.15.0 because Typst exposes no stable reusable CLI parser | parsing, conflict, help, and environment cases in `tests/cli.rs` |
| Output format inference and page-template expansion | Mirrors Typst 0.15.0 because no public CLI planning API exists | format, page-range, template, and collision cases in `tests/cli.rs` |
| Dependency JSON, zero, and Make serialization | Mirrors Typst 0.15.0 with Pack-aware input provenance | dependency cases in `tests/cli.rs` |
| Font, package, certificate, timestamp, jobs, timing, and viewer environment resolution | `typst-kit` where public; remaining process policy mirrors Typst 0.15.0 | corresponding environment and automation cases in `tests/cli.rs` |

`World`, `Library`, the synchronous Typst compiler call, and all official
exporter calls in this table are private mechanisms of the embedded adapter.
They are not typst-pack extension interfaces. Public Rust, CLI, and Dagger
callers provide validated Pack-bound lifecycle values and all converge on that
adapter.

The mirrored rows are the complete parity-review list. They must not be moved
behind private or unstable Typst CLI internals. An embedded Typst upgrade must
compare each row against the new official command before updating expectations.

The individual Typst 0.15.0 mirrors in those rows are:

- `--input` trimming and empty-key rejection;
- output-format inference and the `pdf`, `png`, `svg`, and `html` values;
- page-range parsing and `{p}`, `{0p}`, `{n}`, and `{t}` expansion;
- PNG PPI, PDF standard, PDF tag, pretty-printing, and page-selection defaults
  and conflicts;
- `SOURCE_DATE_EPOCH`, local PDF timestamp, and offset-aware document-time
  resolution;
- feature parsing and the `TYPST_FEATURES` environment;
- font path-list parsing and the three `TYPST_*FONT*` environments;
- package path/cache and certificate environments;
- jobs, diagnostic format, color, timings, dependency, stdout, and viewer
  process policy;
- warning/error ordering, short/human terminal rendering, and exit status; and
- task-grouped help wording for every shared option.

Each item is owned by the process differential gate where an equivalent direct
Typst command exists and by the named focused CLI tests for Pack adapter cases.
Adding, removing, or changing one requires updating this list and its owning
test in the same Typst upgrade.

## Pack differences

| Intentional difference | Pack contract reason | Positive and negative coverage |
| --- | --- | --- |
| Compile consumes a Pack and has no `--root` | The Pack owns its fixed virtual project tree | Pack file/stdin and help omission cases in `tests/cli.rs` |
| Resource Slots use ordered `--resource-path` providers | Only declared non-source paths may receive compilation-specific bytes | Resource Provider order, missing, authority, and help cases in `tests/cli.rs` and `src/tests.rs` |
| Pack Overrides use `--override PACK_PATH FILE` | A compilation may replace only contained project files without mutating the Pack | Pack Override cases in `tests/cli.rs`, `tests/compilation.rs`, and `tests/official_typst_oracle.rs` |
| `--offline` is explicit | Exact Package Requirements must not fall through to an undeclared network source | offline package cases in `tests/cli.rs` and `tests/compilation.rs` |
| Pack fonts and vendored packages precede host configuration | Contained exact dependencies remain authoritative | package and font authority cases in `tests/cli.rs`, `tests/compilation.rs`, and `tests/official_typst_oracle.rs` |
| Bundle output and the Bundle feature are rejected | Bundle is outside the Pack output contract | feature acceptance/rejection and help omission cases in `tests/cli.rs` and `tests/compilation.rs` |
| Watch and deprecated `--make-deps` are absent | Watch needs Pack-aware provenance; deprecated compatibility is not adopted | command and help omission cases in `tests/cli.rs` |
| Creation requires a named source and supports Pack-specific discovery controls | Pack issuance needs a stable entrypoint and dependency closure | create stdin, target, Resource Slot, inclusion, and vendoring cases in `tests/cli.rs` |
| Multi-output paths are collision-preflighted and stdout requires one artifact | Publication must not expose ambiguous or partial output | template collision, stdout, and empty/single/multiple artifact cases in `tests/cli.rs` |

## Adapter boundary

Destination paths, templates, collision checks, stdout constraints, dependency
files, terminal color and rendering, timings, viewer launch, and publication are
performed only after compilation has produced immutable artifact bytes and
diagnostics. These values do not enter `CompilationIdentity` or alter a
`CompilationResult`. The process differential gate checks the shared observable
behavior, while Pack-specific positive and negative tests protect the stricter
adapter contract.
