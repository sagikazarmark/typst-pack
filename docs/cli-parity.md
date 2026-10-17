# Embedded Typst CLI parity inventory

This inventory is tied to Typst 0.15.1. It supplements
[ADR-0020](adr/0020-library-cli-and-adapters.md) and must be reviewed before
the embedded Typst release changes. Follow the complete
[embedded Typst upgrade procedure](embedded-typst-upgrade.md); the pinned Typst
release and the official CLI artifact live in
`crates/typst-pack/embedded-typst.toml`.

The process differential gate is `crates/typst-pack-cli/tests/official_typst_cli.rs`. Dagger downloads
the official Typst 0.15.1 release artifact, verifies its published SHA-256
digest, and exposes it through `TYPST_PACK_OFFICIAL_TYPST`. The test then checks
the binary's version against the embedded Typst release that
`typst_pack::VERSION` names. Missing oracle and packaged binaries are permitted only outside
that dedicated gate; missing, replaced, or version-mismatched tooling fails it.

## Shared behavior

| Behavior | Upstream authority or unavoidable mirror | Differential or focused coverage |
| --- | --- | --- |
| PDF, PNG, SVG, and HTML compilation and artifact bytes | Public Typst compiler and exporter crates | `official_typst_compile_is_the_process_level_parity_baseline`; `crates/typst-pack/tests/official_typst_oracle.rs` |
| Typst inputs, features, document time, and PDF creation time | Public `Library`, `Features`, `World::today`, and exporter controls; CLI environment resolution is mirrored | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; `crates/typst-pack/tests/official_typst_oracle.rs`; `crates/typst-pack-cli/tests/cli.rs` |
| Complete packages, exact fonts, byte inputs, and offline dependency authority | Public Typst `World` requests wrapped by Pack verification; `bytes` values in `sys.inputs` are Typst library behavior; offline is Pack policy | `official_typst_compile_gates_package_font_and_offline_request_routing`; `crates/typst-pack/tests/official_typst_oracle.rs`; `crates/typst-pack/tests/compilation.rs`; package, font, input-file, and offline cases in `crates/typst-pack-cli/tests/cli.rs` |
| Diagnostic content, ordering, rendering, and process exit status | `SourceDiagnostic` and `typst-kit::diagnostics`; process exit policy is mirrored | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; diagnostic cases in `crates/typst-pack/tests/official_typst_oracle.rs` and `crates/typst-pack-cli/tests/cli.rs` |
| Option spelling, aliases, value parsing, defaults, and conflicts | Clap adapter mirrors Typst 0.15.1 because Typst exposes no stable reusable CLI parser | `official_typst_compile_gates_shared_spelling_parsing_and_conflicts`; parsing, conflict, and environment cases in `crates/typst-pack-cli/tests/cli.rs` |
| Output format inference and page-template expansion | Mirrors Typst 0.15.1 because no public CLI planning API exists | format, page-range, template, and collision cases in `crates/typst-pack-cli/tests/cli.rs` |
| Dependency JSON, zero, and Make serialization | Mirrors Typst 0.15.1 with Pack-aware input provenance | dependency cases in `crates/typst-pack-cli/tests/cli.rs` |
| Font, package, certificate, timestamp, jobs, timing, and viewer environment resolution | `typst-kit` where public; remaining process policy mirrors Typst 0.15.1 | corresponding environment and automation cases in `crates/typst-pack-cli/tests/cli.rs` |

`World`, `Library`, the synchronous Typst compiler call, and all official
exporter calls in this table are private mechanisms of the embedded adapter.
They are not typst-pack extension interfaces. Public Rust, CLI, and Dagger
callers provide validated Pack-bound lifecycle values and all converge on that
adapter.

The mirrored rows are the complete parity-review list. They must not be moved
behind private or unstable Typst CLI internals. An embedded Typst upgrade must
compare each row against the new official command before updating expectations.

The individual Typst 0.15.1 mirrors in those rows are:

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
- the spelling of every shared option.

Each item is owned by the process differential gate where an equivalent direct
Typst command exists and by the named focused CLI tests for Pack adapter cases.
Adding, removing, or changing one requires updating this list and its owning
test in the same Typst upgrade.

## Pack differences

| Intentional difference | Pack contract reason | Positive and negative coverage |
| --- | --- | --- |
| Compile consumes a Pack and has no `--root` | The Pack owns its fixed virtual project tree | Pack file and stdin cases in `crates/typst-pack-cli/tests/cli.rs` |
| `--input-file KEY=PATH` passes file bytes through `sys.inputs` | Typst's CLI has string inputs only; per-recipient files must not change the Pack | byte-input cases in `crates/typst-pack-cli/tests/cli.rs`, `crates/typst-pack/tests/compilation.rs`, and `crates/typst-pack/tests/official_typst_oracle.rs` |
| `create --variant KEY=VALUE[,KEY=VALUE...]` adds discovery compiles | Packages and fonts reached only under other inputs must become requirements | variant cases in `crates/typst-pack-cli/tests/cli.rs` and `crates/typst-pack/tests/creation.rs` |
| `--offline` is explicit | Exact Package Requirements must not fall through to an undeclared network source | offline package cases in `crates/typst-pack-cli/tests/cli.rs` and `crates/typst-pack/tests/compilation.rs` |
| Pack fonts and vendored packages precede host configuration | Contained exact dependencies remain authoritative | package and font authority cases in `crates/typst-pack-cli/tests/cli.rs`, `crates/typst-pack/tests/compilation.rs`, and `crates/typst-pack/tests/official_typst_oracle.rs` |
| Bundle output and the Bundle feature are rejected | Bundle is outside the Pack output contract | feature acceptance/rejection cases in `crates/typst-pack-cli/tests/cli.rs` and `crates/typst-pack/tests/compilation.rs` |
| HTML output and the HTML creation target derive the HTML feature | The output or target fully determines the official Typst target and its required feature | derivation and direct-official rejection cases in `crates/typst-pack/tests/official_typst_oracle.rs`, `crates/typst-pack/tests/creation.rs`, and `crates/typst-pack-cli/tests/cli.rs` |
| Watch and deprecated `--make-deps` are absent | Watch needs Pack-aware provenance; deprecated compatibility is not adopted | not offered by the CLI parser |
| Creation structurally packs the selected root and uses one optional representative target | Pack issuance needs a stable entrypoint, complete project snapshot, and selected dependency closure | create stdin, root, ignore-policy, target, and vendoring cases in `crates/typst-pack-cli/tests/cli.rs` |
| Multi-output paths are collision-preflighted and stdout requires one artifact | Writing must not expose ambiguous or partial output | template collision, stdout, and empty/single/multiple artifact cases in `crates/typst-pack-cli/tests/cli.rs` |

## Adapter boundary

Destination paths, templates, collision checks, stdout constraints, dependency
files, terminal color and rendering, timings, viewer launch, and writing are
performed only after compilation has produced immutable artifact bytes and
diagnostics. These values do not enter the compilation identity or alter the
`Compiled` value. Timings record the library's compile through `typst-timing`
and resolve spans against the `Sources` the compile returned. The process differential gate checks the shared observable
behavior, while Pack-specific positive and negative tests protect the stricter
adapter contract.
