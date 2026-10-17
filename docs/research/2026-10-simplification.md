# The October 2026 simplification

> Historical record (2026-10). Findings and outcome only; the decisions are in
> ADR-0018 to ADR-0021.

The [August 2026 review](2026-08-architecture-review.md) found the domain core
sound and the code around it oversized. A follow-up audit measured the
workspace at about 64.7K lines of Rust, about 330 public names, 92 error types,
45 limits names, per-operation by per-adapter type matrices, filesystem
hardening beyond the stated threat model, and about 37K lines of tests, a
quarter of them ceremony. The simplification kept every user-visible
capability and removed the rest. The crate is 0.x, so it made breaking changes
without compatibility shims.

## What changed

- **Identity:** SipHash over Rust `Hash` with role/schema/algorithm wrappers
  became SHA-256 `Digest`s over documented preimages.
- **Format:** pack format v2 with a self-verifying manifest (per-file digests,
  one package list, one row per font container with its faces). The decoder
  verifies members against the manifest; the hand-rolled ZIP
  central-directory checks were removed, except a minimal scan of the
  central directory's raw member names that detects duplicates the ZIP
  reader would merge.
- **Fonts:** requirements became whole containers, which fixed font fallback
  diverging from Typst.
- **Overrides:** Pack Overrides were replaced by byte inputs in `sys.inputs`
  and discovery variants.
- **Errors and limits:** one `thiserror` error type per operation or adapter;
  plain limits structs with defaults and one `LimitExceeded`.
- **Filesystem:** cap-std rooted reading and plain temp-file-and-rename
  writing replaced platform probes, `renameat2`/NT rename paths, staging-residue
  evidence, and commit-certainty machinery.
- **OpenDAL:** a thin adapter over `&Operator` and paths with one error type;
  the location and binding parser was removed.
- **Process:** sixteen ADRs with supersession chains up to four deep became
  four self-contained ones; the glossary went from 27 terms with synonym bans
  to 17 plain definitions; the embedded Typst pinning check moved from
  `build.rs` into a test.

## Measured outcome

Final, after the review-round fixes and the API consistency pass, from
`find crates fuzz -name '*.rs' | xargs cat | wc -l` and
`rg '^\s*pub (struct|enum|trait|type) ' crates/typst-pack/src`:

| Measure | Before | After |
| --- | --- | --- |
| Workspace Rust | ~64.7K lines | 22.6K lines |
| Library sources (`crates/typst-pack/src`) | — | 9.1K lines |
| CLI sources (`crates/typst-pack-cli/src`) | — | 1.5K lines |
| Tests (`crates/*/tests`) and fuzz targets | ~37K lines (tests) | 11.3K and 0.6K lines |
| Public names | ~330 | 63 public types |
| Public error types (library) | 92 | 18 |
| Public limits types (library) | 45 names | 3 structs and `LimitExceeded` |
| ADRs | 16 (7 superseded or amended) | 4 |
