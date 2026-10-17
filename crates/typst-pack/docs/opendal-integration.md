# OpenDAL integration

The optional `opendal` feature reads Pack Assembly inputs, reads and writes
Pack Archives, and writes extraction plans and compilation artifacts through
caller-supplied OpenDAL [`Operator`]s. The application chooses the backend,
credentials, transport, layers, retry policy, and async runtime; every
function returns a caller-polled future and typst-pack owns no runtime.

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["opendal", "package-reading"] }
opendal = { version = "0.58", default-features = false, features = ["services-s3"] }
```

OpenDAL's Memory service needs no extra feature, which makes it convenient
for tests.

## Paths

Functions take an `&Operator` plus plain string paths relative to the
operator's root. A *prefix* is `""` (the root) or a key ending in `/`; an
*object key* is nonempty, has no leading or trailing slash or whitespace, no
empty or `.`/`..` segments, and no backslashes or control characters. These rules keep a key
from aliasing another one after OpenDAL's path normalization. Violations fail
with [`Error::InvalidPath`].

## Reading and assembling

[`OpenDalPackAssembler`] (with `package-reading`) runs the whole Pack creation
loop: it reads the project and fonts, calls [`create`](crate::create), reads
each missing package from the configured [`PackageSource`]s, inserts it, and
resumes until a Pack is created. Its public `target`, `options`, and
`variants` fields are the discovery controls of
[`CreateRequest`](crate::CreateRequest).

```rust,no_run
# #[cfg(feature = "package-reading")]
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use typst_pack::opendal::{FontSource, OpenDalPackAssembler, Operator, PackageSource};
use typst_pack::FontDisposition;

let operator = Operator::new(opendal::services::Memory::default())?;
let mut assembler = OpenDalPackAssembler::new(operator.clone(), "projects/report/", "main.typ");
assembler.fonts.push(FontSource {
    prefix: "fonts/".into(),
    disposition: FontDisposition::Embedded,
});
assembler.packages = vec![
    PackageSource::Trees { operator: operator.clone(), prefix: "packages/".into() },
    PackageSource::ArchiveCache { operator, prefix: "cache/".into() },
];
let assembled = assembler.assemble().await?;
println!("{} warning(s)", assembled.warnings.len());
# Ok(())
# }
```

The building blocks are public for callers that need their own loop (for
example per-package dispositions):

| Function | Result |
| --- | --- |
| [`read_project`] | A [`ProjectSnapshot`](crate::ProjectSnapshot) of every file below a prefix. `.typkignore` is an ordinary file here; ignore rules belong to filesystem reading. |
| [`read_fonts`] | A [`FontCatalog`](crate::FontCatalog) of `.ttf`/`.ttc`/`.otf`/`.otc` objects below each [`FontSource`], in source then path order. |
| [`read_package`] | A [`PackageRead`]: a tree's files, cached or registry archive bytes, or `Unavailable`. Sources are tried in order; only definite absence advances. |
| [`insert_package`] | Builds or expands the tree, inserts it, and keeps [`PackageReadFailures`](crate::PackageReadFailures) in step. |
| [`read_pack_archive`] | The exact archive bytes; decode with [`pack_archive::decode`](crate::pack_archive::decode). |

A listing is one observation, not a storage snapshot: objects may change
between listing and reading. An object that vanishes in between fails the
read.

## Limits

Reads take the shared [`ReadLimits`](crate::ReadLimits): every listed entry
counts toward `max_entries`, and each object's length is checked against
`max_file_bytes` and the remaining `max_total_bytes` before it is downloaded,
so an oversized object is never transferred. A raw package archive is bounded
by `max_file_bytes` and expanded under the same `ReadLimits`;
[`read_pack_archive`] takes an explicit byte ceiling.
Exceeding a limit fails with [`Error::Limit`]. Limits do not bound memory
inside OpenDAL or its transport.

## Writing

| Function | Writes |
| --- | --- |
| [`write_pack_archive`] | One Pack Archive object. |
| [`write_package_archive`] | Validated registry archive bytes into an archive cache (always `CreateOrVerify`). |
| [`write_pack_extraction_plan`] | Every extraction entry below a prefix. |
| [`write_compilation_artifacts`] | Each compiled artifact at `prefix + key`. |

[`WritePolicy::CreateOrVerify`] checks every key first: an existing object
with a different length is a conflict without being downloaded, and an
object of equal length is compared in chunks. A conflict anywhere fails with
[`Error::Conflict`] before anything is written. Absent keys are then created
with a conditional write (`if_not_exists`); if another writer wins the race,
its bytes must match. [`WritePolicy::Overwrite`] writes every key
unconditionally. Required capabilities are checked up front and reported as
[`Error::Unsupported`].

Some S3-compatible endpoints ignore `If-None-Match: *`; on those a
conditional create may overwrite instead of failing. Choose and test backends
for the guarantees you need.

### Recovery

Writes are sequential and not atomic across keys. A [`WriteError`] carries
the `completed` keys and their outcomes, the underlying [`Error`], and
whether the failing key's effect is `indeterminate` (the write was issued
but its outcome is unknown). Every write borrows its input, so the recovery
path is to replay the whole write: with `CreateOrVerify`, keys that were
already written complete as [`WriteOutcome::AlreadyMatching`].

## Errors

[`Error`] is the one adapter error. Its `Display` names the path and the
storage operation but not the operator's endpoint or credentials; the native
OpenDAL error, which may carry backend context, is the error's `source`.

## External compilation dependencies

There is no OpenDAL compilation API. Read external packages and fonts with
the same functions, put them in a [`CompileRequest`](crate::CompileRequest),
and call [`compile`](crate::compile), which matches every external
requirement by digest before running Typst:

```rust,no_run
use typst_pack::opendal::{
    FontSource, Operator, PackageRead, PackageSource, read_fonts, read_package,
};
use typst_pack::{
    CompileRequest, Compiled, FontDisposition, Output, Pack, PackageTree, ReadLimits,
};

async fn compile_with_external_inputs(
    operator: &Operator,
    pack: &Pack,
    output: Output,
) -> Result<Compiled, Box<dyn std::error::Error>> {
    let limits = ReadLimits::default();
    let sources = [PackageSource::Trees {
        operator: operator.clone(),
        prefix: "packages/".into(),
    }];
    let mut request = CompileRequest::new(pack.clone(), output);
    for requirement in pack.package_requirements().iter().filter(|r| !r.embedded) {
        let PackageRead::Tree(files) = read_package(&sources, &requirement.spec, &limits).await?
        else {
            return Err(format!("no tree for {}", requirement.spec).into());
        };
        let tree = PackageTree::from_owned_entries(files)?;
        request.packages.push((requirement.spec.clone(), tree));
    }

    // Containers the pack does not require are ignored.
    let font_source = FontSource {
        prefix: "fonts/".into(),
        disposition: FontDisposition::External,
    };
    let catalog = read_fonts(operator, &[font_source], &limits).await?;
    request.fonts = catalog
        .entries()
        .iter()
        .map(|entry| entry.container().clone())
        .collect();
    Ok(typst_pack::compile(&request)?)
}
```
