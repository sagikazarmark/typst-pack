# OpenDAL integration

The optional `opendal` feature reads projects, fonts, and packages, reads and
writes pack archives, and writes extraction plans and compiled artifacts
through caller-supplied OpenDAL [`Operator`]s. The application chooses the backend,
credentials, transport, layers, retry policy, and async runtime; every
function returns a caller-polled future and typst-pack owns no runtime.

```toml
[dependencies]
typst-pack = { version = "0.6", features = ["opendal", "package-reading"] }
opendal = { version = "0.59", default-features = false, features = ["services-s3"] }
```

OpenDAL's Memory service needs no extra feature, which makes it convenient
for tests.

## Paths

Functions take an `&Operator` plus plain string paths relative to the
operator's root. A *prefix* is `""` (the root) or a key ending in `/`; an
*object key* is nonempty, has no leading or trailing slash or whitespace, no
empty or `.`/`..` segments, and no backslashes or control characters. These rules keep a key
from aliasing another one after OpenDAL's path normalization. A bad prefix,
archive path, or listed key fails with [`Error::InvalidPath`]. Writes of many
keys ([`write_extraction`], [`write_artifacts`]) check every key first and
fail with [`Error::InvalidKeys`], listing each invalid or repeated key, before
anything is written; pack paths may end in whitespace, which OpenDAL would
trim into another key.

## Reading and assembling

[`Assembler`] (with `package-reading`) runs the whole creation loop: it reads
the project and fonts, calls [`create()`](crate::create()), reads each missing
package from the configured [`PackageSource`]s, inserts it, and resumes until a
pack is created. Its public `target`, `options`, and `variants` fields are the
discovery controls of [`create::Request`](crate::create::Request). A package
that is unavailable, over a read limit, or fails to expand or validate becomes
a package failure that creation reports at its import; storage failures end
assembly.

`create()` compiles the document synchronously, so polling `assemble()`
blocks the polling thread while it runs. On a shared async executor, poll it
from a blocking-capable task (for example Tokio's `spawn_blocking` with
`Handle::block_on`), or use the building blocks below and run `create()` on a
blocking thread yourself.

```rust,no_run
# #[cfg(feature = "package-reading")]
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use typst_pack::opendal::{Assembler, FontSource, Operator, PackageSource};

let operator = Operator::new(opendal::services::Memory::default())?;
let mut assembler = Assembler::new(operator.clone(), "projects/report/", "main.typ");
assembler.fonts.push(FontSource {
    prefix: "fonts/".into(),
    embedded: true,
});
assembler.packages = vec![
    PackageSource::Trees { operator: operator.clone(), prefix: "packages/".into() },
    PackageSource::ArchiveCache { operator, prefix: "cache/".into() },
];
let created = assembler.assemble().await?;
println!("{} warning(s)", created.warnings.len());
# Ok(())
# }
```

The building blocks are public for callers that need their own loop (for
example to embed only some packages):

| Function | Result |
| --- | --- |
| [`read_project`] | A [`ProjectSnapshot`](crate::ProjectSnapshot) of every file below a prefix except `.typk` paths. `.typkignore` is an ordinary file here; ignore rules belong to filesystem reading. |
| [`read_fonts`] | The [`font::Entry`](crate::font::Entry) values of `.ttf`/`.ttc`/`.otf`/`.otc` objects below each [`FontSource`], in source then path order. An object that is not a readable font fails the read with [`Error::Font`]: font prefixes are curated inputs here, unlike the filesystem adapter's font directories, which are host configuration and whose unreadable fonts are skipped and reported. |
| [`read_package`] | A [`PackageRead`]: a tree's files, cached or registry archive bytes, or `Unavailable`. Sources are tried in order; only definite absence advances. |
| [`insert_package`] | Builds or expands the tree, adds it to a [`create::Request`](crate::create::Request), and keeps its package failures (with their reasons) in step. |
| [`read_archive`] | The decoded [`Pack`](crate::Pack) of one archive object, as `fs::read_archive` reads a file; bytes that are not a valid pack fail with [`Error::Decode`]. |

[`write_archive`] encodes the pack it is given, so copying a decoded pack
writes this build's encoding, which may differ in bytes (never in identity)
from the original. To mirror archive objects byte for byte, copy them with
the `Operator` directly.

A listing is one observation, not a storage snapshot: objects may change
between listing and reading. An object that vanishes in between fails the
read. Each object is read whole in one request after its stat, and the bytes
must agree with that stat: where the operator supports conditional reads the
read is conditional on the stat's ETag, and a read whose length differs from
the stat's fails. A replaced, grown, or shrunk object therefore fails with a
temporary [`Error::Storage`] whose source has kind `ConditionNotMatch` (a
failed `If-Match` is marked temporary too); it is never returned as a
prefix, and retrying the operation is safe. Without ETag support, a same-length replacement
between stat and read cannot be detected.

## Limits

Reads take the shared [`ReadLimits`](crate::ReadLimits): every listed entry
counts toward `max_entries`, and each object's length is checked against
`max_file_bytes` and the remaining `max_total_bytes` before it is downloaded,
so an oversized object is never transferred. A raw package archive is bounded
by `max_file_bytes` and expanded under the same `ReadLimits`;
[`ReadLimits::PACKAGE`](crate::ReadLimits::PACKAGE) (256 MiB in total) is the
suggested limit for packages, and the [`Assembler`] uses it as its
`package_limits` default. [`read_archive`] bounds the object by
[`DecodeLimits::max_archive_bytes`](crate::archive::DecodeLimits::max_archive_bytes)
and decodes it under the same [`DecodeLimits`](crate::archive::DecodeLimits).
Exceeding a limit fails with [`Error::Limit`]; the [`Assembler`] records a
package over a limit as a package failure instead. An object that grows
after its stat is abandoned as soon as the read passes the stat's length.
Limits do not bound memory inside OpenDAL or its transport.

## Writing

| Function | Writes |
| --- | --- |
| [`write_archive`] | One [`Pack`](crate::Pack), encoded as an archive object, as `fs::write_archive` writes a file. |
| [`write_package_archive`] | Validated registry archive bytes into an archive cache (always `CreateOrVerify`). |
| [`write_extraction`] | Every extraction entry below a prefix. |
| [`write_artifacts`] | Each compiled artifact at `prefix + key`. |

[`WritePolicy::CreateOrVerify`] checks every key first: an existing object
with a different length is a conflict without being downloaded, and an
object of equal length is compared in chunks. A conflict anywhere fails with
[`Error::Conflict`] before anything is written. Absent keys are then created
with a conditional write (`if_not_exists`); if another writer wins the race,
its bytes must match. A conditional create refused although the object is
then absent (S3 answers `409 ConditionalRequestConflict` while another upload
of the key is in flight) is retried once; a second refusal is a temporary
[`Error::Storage`], not a conflict. Verification reads the whole object, so
an object that grew past the expected bytes never matches on its prefix.
[`WritePolicy::Overwrite`] writes every key
unconditionally. Required capabilities are checked up front and reported as
[`Error::Unsupported`].

Some S3-compatible endpoints ignore `If-None-Match: *`; on those a
conditional create may overwrite instead of failing. Choose and test backends
for the guarantees you need.

### Recovery

Writes are sequential and not atomic across keys. A [`WriteError`] carries
the failing key's `path` (`None` when the write was refused before any key
was tried), the `completed` keys and their outcomes, the underlying
[`Error`] as its `source`, and whether the failing key's effect is
`indeterminate` (the write was issued but its outcome is unknown). Every write borrows its input, so the recovery
path is to replay the whole write: with `CreateOrVerify`, keys that were
already written complete as [`WriteOutcome::AlreadyMatching`].

## Errors

[`Error`] is the one adapter error. Its `Display` names the path and the
storage operation but not the operator's endpoint or credentials; the native
OpenDAL error, which may carry backend context, is the error's `source`, as
is the [`LimitExceeded`](crate::LimitExceeded) of [`Error::Limit`]. As
everywhere in the crate, a cause is exposed through `source()` and not
repeated in `Display`; print the whole chain to show it.

Every function takes its target (operator and path or prefix) first, then
the data, then the policy, as the filesystem adapter does; limits are passed
by value.

## External compilation dependencies

There is no OpenDAL compilation API. Read external packages and fonts with
the same functions, put them in a [`compile::Request`](crate::compile::Request),
and call [`compile()`](crate::compile()), which matches every external
requirement by digest before running Typst:

```rust,no_run
use typst_pack::compile::{Compiled, Output, Request};
use typst_pack::opendal::{
    FontSource, Operator, PackageRead, PackageSource, read_fonts, read_package,
};
use typst_pack::{Pack, ReadLimits, package};

async fn compile_with_external_inputs(
    operator: &Operator,
    pack: &Pack,
    output: Output,
) -> Result<Compiled, Box<dyn std::error::Error>> {
    let sources = [PackageSource::Trees {
        operator: operator.clone(),
        prefix: "packages/".into(),
    }];
    let mut request = Request::new(pack.clone(), output);
    for requirement in pack.package_requirements().iter().filter(|r| !r.embedded) {
        let read = read_package(&sources, &requirement.spec, ReadLimits::PACKAGE).await?;
        let PackageRead::Tree(files) = read
        else {
            return Err(format!("no tree for {}", requirement.spec).into());
        };
        let tree = package::Tree::new(files)?;
        request.packages.push((requirement.spec.clone(), tree));
    }

    // Containers the pack does not require are ignored.
    let font_source = FontSource {
        prefix: "fonts/".into(),
        embedded: false,
    };
    let fonts = read_fonts(operator, &[font_source], ReadLimits::default()).await?;
    request.fonts = fonts.into_iter().map(|entry| entry.container).collect();
    Ok(typst_pack::compile(&request)?)
}
```
