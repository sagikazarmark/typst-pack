# typst-pack CLI

Bundle a Typst project and its fonts and packages into one `.typk` file that
can compile on another machine. This is the command-line tool for the
[`typst-pack`](https://crates.io/crates/typst-pack) library.

```console
cargo install typst-pack-cli
```

```console
typst-pack create project/main.typ project.typk
typst-pack inspect project.typk
typst-pack compile project.typk report.pdf
typst-pack extract project.typk -o copy/
```

See the [CLI guide](https://github.com/sagikazarmark/typst-pack/blob/main/docs/guides/cli.md)
for every task, or run `typst-pack <command> --help`.

The package is licensed under either the MIT License or the Apache License 2.0.
The license texts are available in the
[source repository](https://github.com/sagikazarmark/typst-pack).
