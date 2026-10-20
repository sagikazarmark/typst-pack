# Embedded Typst CLI parity inventory

This inventory is tied to Typst 0.15.1. It supplements
[ADR-0020](../adr/0020-library-cli-and-adapters.md) and must be reviewed before
the embedded Typst release changes. Follow the complete
[embedded Typst upgrade procedure](../development/embedded-typst-upgrade.md); the pinned Typst
release and the official CLI artifact live in
`crates/typst-pack/embedded-typst.toml`.

The process differential gate is `crates/typst-pack-cli/tests/official_typst_cli.rs`. `just test-cli-parity` downloads
the official Typst 0.15.1 release artifact, verifies its published SHA-256
digest, and exposes it through `TYPST_PACK_OFFICIAL_TYPST`. The test then checks
the binary's version against the embedded Typst release that
`typst_pack::VERSION` names. Missing oracle and packaged binaries are permitted only outside
that dedicated gate; missing, replaced, or version-mismatched tooling fails it.

## Shared behavior

| Behavior | Upstream authority or unavoidable mirror | Differential or focused coverage |
| --- | --- | --- |
| PDF, PNG, SVG, and HTML compilation output | Public Typst compiler and exporter crates. SVG and HTML bytes, PDF content, and PNG pixels are identical; PDF and PNG bytes also depend on the compression backend build, and match the pinned official release for the gate fixture (this build's `miniz_oxide` differs from the version in Typst's lockfile, so other pages, such as text pages, may compress differently) | `official_typst_compile_is_the_process_level_parity_baseline`; `crates/typst-pack/tests/typst_parity.rs` |
| Typst inputs, features, document time, and PDF creation time | Public `Library`, `Features`, `World::today`, and exporter controls; CLI environment resolution is mirrored: a creation timestamp is read in UTC, the current time in the local timezone | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; `official_typst_compile_gates_the_local_date_without_a_timestamp`; `crates/typst-pack/tests/typst_parity.rs`; `source_date_epoch_and_the_creation_timestamp_fix_dates` and `without_a_timestamp_today_is_the_local_date` in `crates/typst-pack-cli/tests/cli.rs` |
| Complete packages, exact fonts, byte inputs, and offline dependency authority | Public Typst `World` requests wrapped by Pack verification; `bytes` values in `sys.inputs` are Typst library behavior; offline is Pack policy | `official_typst_compile_gates_package_font_and_offline_request_routing`; `crates/typst-pack/tests/typst_parity.rs`; `crates/typst-pack/tests/compilation.rs`; `font_and_package_environment_is_honored_by_compile`, `external_fonts_are_named_when_missing_and_used_when_supplied`, `string_and_file_inputs_reach_sys_inputs_and_dependencies`, and `unvendored_packages_are_recorded_and_resolved_at_compile_time` in `crates/typst-pack-cli/tests/cli.rs` |
| Diagnostic content, ordering (errors, then warnings), rendering, and process exit status | `SourceDiagnostic` and `typst-kit::diagnostics`; process exit policy is mirrored | `official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior`; `official_typst_compile_gates_errors_before_warnings`; `failed_compilation_diagnostics_match` in `crates/typst-pack/tests/typst_parity.rs`; `diagnostics_render_source_names_in_order_without_a_trailer` and `color_follows_the_global_control` in `crates/typst-pack-cli/tests/cli.rs` |
| Option spelling, aliases, value parsing, defaults, and conflicts | Clap adapter mirrors Typst 0.15.1 because Typst exposes no stable reusable CLI parser | `official_typst_compile_gates_shared_spelling_parsing_and_conflicts`; `features_accept_lists_and_reject_bundle`, `pdf_standards_and_tags_are_validated_with_every_hint`, `pretty_changes_svg_and_html_bytes`, `ppi_scales_png_pixels`, and `create_root_flag_and_environment_define_the_project_tree` in `crates/typst-pack-cli/tests/cli.rs` |
| Output format inference and page-template expansion | Mirrors Typst 0.15.1 because no public CLI planning API exists | format, page-range, template, and collision cases in `crates/typst-pack-cli/tests/cli.rs` |
| Dependency JSON, zero, and Make serialization | Mirrors Typst 0.15.1 with Pack-aware input provenance | `dependencies_follow_typst_formats_and_transport_rules` in `crates/typst-pack-cli/tests/cli.rs` |
| Font, package, certificate, timestamp, jobs, timing, and viewer environment resolution | `typst-kit` where public; remaining process policy mirrors Typst 0.15.1; `--jobs` sizes the pool pages export on and does not change artifacts | `official_typst_compile_gates_templates_automation_dependencies_and_viewing`; `font_and_package_environment_is_honored_by_compile`, `certificates_reach_the_package_downloader_unless_offline`, `source_date_epoch_and_the_creation_timestamp_fix_dates`, `jobs_do_not_change_artifacts`, `timings_are_perfetto_json_and_their_failure_is_reported_last`, and `a_viewer_opens_the_first_emitted_page_unless_output_is_stdout` in `crates/typst-pack-cli/tests/cli.rs` |
| Writing an output through a symbolic link at its final path component | Typst writes through the link and keeps the target's permissions; `fs::WritePolicy::Overwrite` resolves the link and commits at its target with the target's permissions | symbolic-link target cases in `crates/typst-pack/tests/fs.rs` |

`World`, `Library`, the synchronous Typst compiler call, and all official
exporter calls in this table are private mechanisms of the embedded adapter.
They are not typst-pack extension interfaces. Public Rust and CLI
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
  resolution (UTC with a timestamp, the local timezone without one);
- feature parsing and the `TYPST_FEATURES` environment;
- font path-list parsing and the three `TYPST_*FONT*` environments;
- package path/cache and certificate environments;
- jobs (default: available parallelism; also the page export workers),
  diagnostic format, color, timings, dependency, stdout, and viewer process
  policy;
- error-then-warning ordering, short/human terminal rendering, and exit
  status;
- overwriting an output through a symbolic link at its final path component;
  and
- the spelling of every shared option.

Each item is owned by the process differential gate where an equivalent direct
Typst command exists and by the named focused CLI tests for Pack adapter cases.
Adding, removing, or changing one requires updating this list and its owning
test in the same Typst upgrade.

## Pack differences

| Intentional difference | Pack contract reason | Positive and negative coverage |
| --- | --- | --- |
| Compile consumes a Pack and has no `--root` | The Pack owns its fixed virtual project tree | Pack file and stdin cases in `crates/typst-pack-cli/tests/cli.rs` |
| `--input-file KEY=PATH` passes file bytes through `sys.inputs` | Typst's CLI has string inputs only; per-recipient files must not change the Pack | `string_and_file_inputs_reach_sys_inputs_and_dependencies` and `create_inputs_reach_discovery_and_conflicting_keys_are_rejected` in `crates/typst-pack-cli/tests/cli.rs`; `string_and_byte_inputs_reach_sys_inputs` in `crates/typst-pack/tests/compilation.rs`; `byte_inputs_and_features_match` in `crates/typst-pack/tests/typst_parity.rs` |
| A key given by both `--input` and `--input-file` is rejected | Neither flag should silently win over the other | `create_inputs_reach_discovery_and_conflicting_keys_are_rejected` in `crates/typst-pack-cli/tests/cli.rs` |
| `create --variant KEY=VALUE[,KEY=VALUE...]` adds discovery compiles | Packages and fonts reached only under other inputs must become requirements | `create_variants_discover_packages_of_every_input_branch` and `a_failing_variant_is_named` in `crates/typst-pack-cli/tests/cli.rs`; variant cases in `crates/typst-pack/tests/creation.rs` |
| `--offline` is explicit | Exact Package Requirements must not fall through to an undeclared network source | `certificates_reach_the_package_downloader_unless_offline` and `unvendored_packages_are_recorded_and_resolved_at_compile_time` in `crates/typst-pack-cli/tests/cli.rs` |
| Host configuration only supplies exact external requirements | Pack fonts and vendored packages are used as contained; host fonts and packages only supply the exact font digests and package specifications the pack requires externally, and never take part in font selection otherwise | `pack_dependencies_take_precedence_over_host_configuration` in `crates/typst-pack-cli/tests/cli.rs`; `pack_fonts_are_the_only_fonts` and `vendored_packages_compile_from_the_pack` in `crates/typst-pack/tests/compilation.rs` |
| Bundle output and the Bundle feature are rejected | Bundle is outside the Pack output contract | feature acceptance/rejection cases in `crates/typst-pack-cli/tests/cli.rs` and `crates/typst-pack/tests/compilation.rs` |
| HTML output and the HTML creation target derive the HTML feature | The output or target fully determines the official Typst target and its required feature | `html_output_enables_the_html_feature_itself` in `crates/typst-pack/tests/compilation.rs`; `the_html_target_enables_the_html_feature_itself` in `crates/typst-pack/tests/creation.rs`; `create_derives_the_html_feature_from_a_single_target` in `crates/typst-pack-cli/tests/cli.rs` |
| Watch and deprecated `--make-deps` are absent | Watch needs Pack-aware provenance; deprecated compatibility is not adopted | not offered by the CLI parser |
| `inspect` and `compile` warn when a pack records another Typst version | The same pack can render differently under another Typst release; the warning names both versions | `inspect_and_compile_name_the_producer_and_warn_on_another_typst` in `crates/typst-pack-cli/tests/cli.rs` |
| Creation structurally packs the selected root and uses one optional representative target | Pack issuance needs a stable entrypoint, complete project snapshot, and selected dependency closure | create stdin, root, ignore-policy, target, and vendoring cases in `crates/typst-pack-cli/tests/cli.rs` |
| Multi-output paths are collision-preflighted and stdout requires one artifact | Writing must not expose ambiguous or partial output | template collision, stdout, and empty/single/multiple artifact cases in `crates/typst-pack-cli/tests/cli.rs` |
| Outputs are replaced through a temporary sibling: a read-only (`0444`) existing output is replaced where Typst fails, and a writable output in a directory that admits no new entry is rewritten in place, as Typst does | Readers see the old or the new output; only a directory that cannot hold the temporary file forces the in-place write | `overwriting_replaces_read_only_files_and_rewrites_in_locked_directories` in `crates/typst-pack/tests/fs.rs` |
| A successful `create` warns about each font file it could not read; `compile` does so when a required font is missing | Skipped host fonts are otherwise invisible when a font requirement goes unfulfilled | `external_fonts_are_named_when_missing_and_used_when_supplied` in `crates/typst-pack-cli/tests/cli.rs` |
| Help and error texts are typst-pack's own: shared options keep Typst's spelling and defaults with shortened help, an uninferable output format is reported in other words, and a pack read from stdin without an output fails at run time (exit status 1) instead of as a usage error (exit status 2) | The pack CLI documents pack-specific behavior and checks the stdin output after parsing, with the other pack inputs | `stdout_takes_exactly_one_artifact_and_stdin_an_explicit_output` and `the_output_format_comes_from_the_flag_or_the_extension` in `crates/typst-pack-cli/tests/cli.rs` |

## Adapter boundary

Destination paths, templates, collision checks, stdout constraints, dependency
files, terminal color and rendering, timings, viewer launch, and writing are
performed only after compilation has produced immutable artifact bytes and
diagnostics. These values do not enter the compilation identity or alter the
`Compiled` value. Timings record the library's compile through `typst-timing`
and resolve spans against the `Sources` the compile returned. The process differential gate checks the shared observable
behavior, while Pack-specific positive and negative tests protect the stricter
adapter contract.
