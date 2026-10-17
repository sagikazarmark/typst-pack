//! The typst-pack command line: Pack-specific flows and one focused test per
//! adapter behavior in `docs/cli-parity.md`. Shared Typst behavior is gated
//! against the official binary in `official_typst_cli.rs`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use typst_pack::Pack;
use typst_pack::archive::{DecodeLimits, decode, encode};

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
const PROBE: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

/// `typst-pack` in `dir`, isolated from ambient Typst configuration and
/// offering no system fonts.
fn typst_pack(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
    command.current_dir(dir);
    for variable in [
        "SOURCE_DATE_EPOCH",
        "TYPST_CERT",
        "TYPST_FEATURES",
        "TYPST_FONT_PATHS",
        "TYPST_IGNORE_EMBEDDED_FONTS",
        "TYPST_PACKAGE_CACHE_PATH",
        "TYPST_PACKAGE_PATH",
        "TYPST_ROOT",
    ] {
        command.env_remove(variable);
    }
    command.env("TYPST_IGNORE_SYSTEM_FONTS", "true");
    command
}

fn run(dir: &Path, args: &[impl AsRef<OsStr>]) -> Output {
    typst_pack(dir).args(args).output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[track_caller]
fn ok(output: Output) -> Output {
    assert!(output.status.success(), "{}", stderr(&output));
    output
}

/// The stderr of a run that must fail.
#[track_caller]
fn failed(output: Output) -> String {
    assert!(!output.status.success(), "unexpected success");
    stderr(&output)
}

/// Writes `files` beneath `dir`, creating parents.
fn write(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn save(dir: &Path, name: &str, pack: &Pack) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, encode(pack)).unwrap();
    path
}

/// Writes a pack of `main.typ` holding `source` as `name`.
fn pack(dir: &Path, name: &str, source: &str) -> PathBuf {
    let pack = Pack::builder("main.typ").file("main.typ", source.as_bytes().to_vec());
    save(dir, name, &pack.build().unwrap())
}

/// Writes `pages.typk`, whose page `n` is `n` × 10pt wide.
fn pages(dir: &Path, count: usize) -> PathBuf {
    let pages = (1..=count).map(|page| format!("#page(width: {page}0pt, height: 10pt)[]"));
    pack(dir, "pages.typk", &pages.collect::<Vec<_>>().join("\n"))
}

fn open(path: impl AsRef<Path>) -> Pack {
    decode(&std::fs::read(path).unwrap(), DecodeLimits::default()).unwrap()
}

fn manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n")
}

/// Writes `@preview/{name}:0.1.0` beneath a package path or cache.
fn write_package(root: &Path, name: &str) {
    write(
        &root.join(format!("preview/{name}/0.1.0")),
        &[
            ("typst.toml", &manifest(name)),
            ("lib.typ", "#let mark = rect(width: 1pt, height: 1pt)"),
        ],
    );
}

/// A pack requiring `@preview/{name}:0.1.0` without containing it.
fn requiring(dir: &Path, name: &str) -> PathBuf {
    let spec: typst::syntax::package::PackageSpec =
        format!("@preview/{name}:0.1.0").parse().unwrap();
    let source = format!("#import \"@preview/{name}:0.1.0\": mark\n#mark");
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .external_package_file(spec.clone(), "typst.toml", manifest(name).into_bytes())
        .external_package_file(
            spec,
            "lib.typ",
            b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .build()
        .unwrap();
    save(dir, "package.typk", &pack)
}

fn json(path: impl AsRef<Path>) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

const WARNING: &str = "#set text(font: \"Definitely Missing\")\nWarning";

// --- create ---------------------------------------------------------------

#[test]
fn create_writes_next_to_the_source_or_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[("project/main.typ", "#rect(width: 1pt, height: 1pt)")],
    );

    ok(run(dir, &["create", "project/main.typ"]));
    assert_eq!(open(dir.join("project/main.typk")).entrypoint(), "main.typ");

    let stdout = ok(run(dir, &["create", "project/main.typ", "-"])).stdout;
    let pack = decode(&stdout, DecodeLimits::default()).unwrap();
    assert_eq!(pack.entrypoint(), "main.typ");

    // The input is a named source file, never stdin or a directory.
    let stdin = typst_pack(dir)
        .args(["create", "-", "out.typk"])
        .stdin(Stdio::null())
        .output();
    assert!(failed(stdin.unwrap()).contains("stdin"));
    assert!(failed(run(dir, &["create", "project", "out.typk"])).contains("Typst source file"));
}

#[cfg(unix)]
#[test]
fn stdout_uses_typst_sigpipe_behavior() {
    use std::os::unix::process::ExitStatusExt as _;

    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), &[("main.typ", "Hello")]);
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    drop(reader);
    let writer: std::os::fd::OwnedFd = writer.into();

    let status = typst_pack(dir.path())
        .args(["create", "main.typ", "-"])
        .stdout(Stdio::from(writer))
        .status()
        .unwrap();

    assert_eq!(status.signal(), Some(13), "{status:?}");
}

#[test]
fn create_root_flag_and_environment_define_the_project_tree() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[
            ("project/src/main.typ", "#include \"../shared.typ\""),
            ("project/shared.typ", "Shared"),
        ],
    );

    for (name, environment) in [("flag.typk", false), ("environment.typk", true)] {
        let mut command = typst_pack(dir);
        command.args(["create", "project/src/main.typ", name]);
        if environment {
            command.env("TYPST_ROOT", "project");
        } else {
            command.args(["--root", "project"]);
        }
        ok(command.output().unwrap());

        let pack = open(dir.join(name));
        assert_eq!(pack.entrypoint(), "src/main.typ");
        assert!(pack.file("shared.typ").is_some());
    }
}

#[test]
fn create_applies_the_root_ignore_policy_and_reports_a_summary() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[
            ("project/main.typ", "Hello"),
            ("project/unused.txt", "packed"),
            (
                "project/.typkignore",
                "ignored/**\n!ignored/keep/\n!ignored/keep/keep.txt\n*.secret\n",
            ),
            ("project/ignored/drop.txt", "drop"),
            ("project/ignored/keep/keep.txt", "keep"),
            ("project/private.secret", "drop"),
            ("project/old.typk", "drop"),
        ],
    );

    let output = ok(run(dir, &["create", "project/main.typ", "out.typk"]));

    let summary = String::from_utf8_lossy(&output.stdout);
    assert!(
        summary.contains("packed 4 project file(s), 0 package(s), 1 font(s)"),
        "{summary}"
    );
    let pack = open(dir.join("out.typk"));
    assert_eq!(
        pack.files().map(|(path, _)| path).collect::<Vec<_>>(),
        [
            ".typkignore",
            "ignored/keep/keep.txt",
            "main.typ",
            "unused.txt"
        ]
    );
}

#[test]
fn create_refuses_ignored_entrypoints_bad_policies_and_escaping_links() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    for (policy, expected) in [("main.typ\n", "main.typ"), ("trailing\\\n", ".typkignore")] {
        write(
            dir,
            &[
                ("project/main.typ", "Hello"),
                ("project/.typkignore", policy),
            ],
        );
        let stderr = failed(run(dir, &["create", "project/main.typ", "out.typk"]));
        assert!(stderr.contains(expected), "{stderr}");
    }

    #[cfg(unix)]
    {
        write(
            dir,
            &[("linked/main.typ", "Hello"), ("outside.txt", "outside")],
        );
        std::os::unix::fs::symlink(dir.join("outside.txt"), dir.join("linked/leak.txt")).unwrap();
        let stderr = failed(run(dir, &["create", "linked/main.typ", "out.typk"]));
        assert!(stderr.contains("leak.txt"), "{stderr}");
    }
    assert!(!dir.join("out.typk").exists());
}

#[test]
fn create_derives_the_html_feature_from_a_single_target() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(dir, &[("main.typ", "#html.elem(\"p\")[Paragraph]")]);

    ok(run(
        dir,
        &["create", "main.typ", "html.typk", "--target", "html"],
    ));
    open(dir.join("html.typk"));

    let one = ["create", "main.typ", "two.typk", "--target", "paged,html"];
    failed(run(dir, &one));
    let two = [
        "create", "main.typ", "two.typk", "--target", "paged", "--target", "html",
    ];
    failed(run(dir, &two));
}

#[test]
fn create_variants_discover_packages_of_every_input_branch() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[(
            "project/main.typ",
            "#if sys.inputs.at(\"edition\", default: \"a\") == \"a\" {\n\
               import \"@preview/alpha:0.1.0\": mark; mark\n\
             } else {\n\
               import \"@preview/beta:0.1.0\": mark; mark\n\
             }",
        )],
    );
    write_package(&dir.join("packages"), "alpha");
    write_package(&dir.join("packages"), "beta");

    let required = |output: &str, variant: &[&str]| {
        let args = [
            "create",
            "project/main.typ",
            output,
            "--package-path",
            "packages",
        ];
        ok(typst_pack(dir)
            .args(args)
            .arg("--offline")
            .args(variant)
            .output()
            .unwrap());
        let pack = open(dir.join(output));
        let specs = pack.package_requirements().iter();
        specs
            .map(|requirement| requirement.spec.name.to_string())
            .collect::<Vec<_>>()
    };

    assert_eq!(required("base.typk", &[]), ["alpha"]);
    assert_eq!(
        required("variants.typk", &["--variant", "edition=b,unused=1"]),
        ["alpha", "beta"]
    );
}

#[test]
fn unvendored_packages_are_recorded_and_resolved_at_compile_time() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write_package(&dir.join("packages"), "unvendored");
    let source = "#import \"@preview/unvendored:0.1.0\": mark\n#mark";
    write(dir, &[("project/main.typ", source)]);
    let paths = ["--package-path", "packages", "--offline"];

    let created = ok(typst_pack(dir)
        .args([
            "create",
            "project/main.typ",
            "out.typk",
            "--no-vendor-packages",
        ])
        .args(paths)
        .output()
        .unwrap());

    let summary = String::from_utf8_lossy(&created.stdout);
    assert!(
        summary.contains("packed 1 project file(s), 0 package(s)"),
        "{summary}"
    );
    assert!(
        summary.contains("1 package(s) were not vendored"),
        "{summary}"
    );
    assert!(summary.contains("@preview/unvendored:0.1.0"), "{summary}");
    let pack = open(dir.join("out.typk"));
    assert_eq!(pack.packages().count(), 0);
    assert_eq!(pack.package_requirements()[0].spec.name, "unvendored");

    ok(typst_pack(dir)
        .args(["compile", "out.typk", "out.svg"])
        .args(paths)
        .output()
        .unwrap());
    assert!(dir.join("out.svg").is_file());
}

#[test]
fn create_shares_font_package_time_and_automation_controls() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let (font, family) = distinct_fonts(dir).remove(0);
    write_package(&dir.join("cache"), "cached");
    std::fs::create_dir(dir.join("empty")).unwrap();
    write(
        dir,
        &[(
            "project/main.typ",
            &format!(
                "#assert(datetime.today().year() == 2000)\n\
                 #import \"@preview/cached:0.1.0\": mark\n#set text(font: \"{family}\")\n#mark\nText"
            ),
        )],
    );

    ok(typst_pack(dir)
        .env("TYPST_FONT_PATHS", &font)
        .env("TYPST_PACKAGE_PATH", "empty")
        .env("TYPST_PACKAGE_CACHE_PATH", "cache")
        .args([
            "create",
            "project/main.typ",
            "out.typk",
            "--embed-fonts",
            "--offline",
        ])
        .args([
            "--ignore-embedded-fonts",
            "--creation-timestamp",
            "946684800",
            "-j",
            "1",
        ])
        .args(["--diagnostic-format", "short", "--timings", "timings.json"])
        .output()
        .unwrap());

    let pack = open(dir.join("out.typk"));
    assert!(pack.packages().any(|(spec, _)| spec.name == "cached"));
    assert!(
        pack.font_requirements()
            .iter()
            .any(|requirement| requirement.embedded)
    );
    assert!(json(dir.join("timings.json")).is_array());
}

// --- compile: inputs and outputs -----------------------------------------

#[test]
fn compile_reads_stdin_and_writes_each_format_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let bytes = std::fs::read(pages(dir, 5)).unwrap();
    let mut child = typst_pack(dir)
        .args(["compile", "-", "-", "--format", "svg", "--pages", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), &bytes).unwrap();
    assert!(
        ok(child.wait_with_output().unwrap())
            .stdout
            .starts_with(b"<svg")
    );

    pack(dir, "hello.typk", "Hello");
    for (format, signature) in [
        ("pdf", &b"%PDF"[..]),
        ("html", b"<!DOCTYPE html>"),
        ("png", b"\x89PNG\r\n\x1a\n"),
        ("svg", b"<svg"),
    ] {
        let stdout = ok(run(
            dir,
            &["compile", "hello.typk", "-", "--format", format],
        ))
        .stdout;
        assert!(stdout.starts_with(signature), "{format}");
    }
}

#[test]
fn stdout_takes_exactly_one_artifact_and_stdin_an_explicit_output() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pages(dir, 5);

    assert!(failed(run(dir, &["compile", "-"])).contains("explicit output"));
    let many = run(dir, &["compile", "pages.typk", "-", "--format", "png"]);
    assert!(many.stdout.is_empty());
    failed(many);
    let none = run(
        dir,
        &[
            "compile",
            "pages.typk",
            "-",
            "--format",
            "svg",
            "--pages",
            "9",
        ],
    );
    assert!(none.stdout.is_empty());
    assert!(failed(none).contains("exactly one file"));
}

#[test]
fn the_output_format_comes_from_the_flag_or_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    std::fs::create_dir(dir.join("nested")).unwrap();
    pack(
        &dir.join("nested"),
        "doc.typk",
        "#rect(width: 1pt, height: 1pt)",
    );
    let compile = |args: &[&str]| run(dir, &[&["compile", "nested/doc.typk"], args].concat());

    for name in ["doc.PDF", "page.PNG", "page.SVG", "doc.HTML"] {
        let output = compile(&[name]);
        assert!(ok(output).stdout.is_empty(), "{name}");
        assert!(dir.join(name).is_file(), "{name}");
    }
    // Typst has no `.htm` alias, and an extensionless output needs a format.
    failed(compile(&["doc.htm"]));
    assert!(failed(compile(&["document"])).contains("cannot infer output format"));
    // An explicit format wins over the extension.
    ok(compile(&["svg.pdf", "--format", "svg"]));
    assert!(
        std::fs::read(dir.join("svg.pdf"))
            .unwrap()
            .starts_with(b"<svg")
    );
    // Without an output, it lands next to the pack.
    ok(compile(&[]));
    assert!(dir.join("nested/doc.pdf").is_file());
}

#[test]
fn string_and_file_inputs_reach_sys_inputs_and_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(
        dir,
        "logo.typk",
        "#set page(width: auto, height: auto, margin: 0pt)\n\
         #assert(type(sys.inputs.logo) == bytes)\n\
         #assert.eq(sys.inputs.key, \"value\")\n\
         #image(sys.inputs.logo, width: int(sys.inputs.width) * 1pt)",
    );
    write(
        dir,
        &[(
            "logo.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#,
        )],
    );
    let compile = |output: &str, width: &str| {
        let args = ["compile", "logo.typk", output, "--input-file=logo=logo.svg"];
        let inputs = [
            "--input",
            " key = value ",
            "--input",
            &format!("width={width}"),
        ];
        typst_pack(dir)
            .args(args)
            .args(inputs)
            .args(["--deps", "deps.json"])
            .output()
            .unwrap()
    };

    ok(compile("narrow.svg", "4"));
    ok(compile("wide.svg", "8"));

    let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
    assert_ne!(read("narrow.svg"), read("wide.svg"));
    let inputs = json(dir.join("deps.json"))["inputs"].to_string();
    assert!(
        inputs.contains("logo.svg") && inputs.contains("logo.typk"),
        "{inputs}"
    );

    let empty_key = ["compile", "logo.typk", "out.svg", "--input", " = value"];
    assert!(failed(run(dir, &empty_key)).contains("key was missing or empty"));
    let missing = [
        "compile",
        "logo.typk",
        "out.svg",
        "--input-file",
        "logo=missing.bin",
    ];
    assert!(failed(run(dir, &missing)).contains("missing.bin"));
    assert!(!dir.join("out.svg").exists());
}

#[cfg(all(unix, not(target_vendor = "apple")))]
#[test]
fn an_input_file_path_need_not_be_unicode() {
    use std::os::unix::ffi::OsStringExt as _;

    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(dir, "data.typk", "#str(sys.inputs.data)");
    let data = std::ffi::OsString::from_vec(b"data-\xff.txt".to_vec());
    std::fs::write(dir.join(&data), "text").unwrap();
    let mut argument = std::ffi::OsString::from("--input-file=data=");
    argument.push(&data);

    ok(typst_pack(dir)
        .args(["compile", "data.typk", "out.svg"])
        .arg(argument)
        .output()
        .unwrap());
    assert!(dir.join("out.svg").is_file());
}

#[test]
fn write_failures_are_reported_with_the_native_error() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(dir, "doc.typk", "#rect(width: 1pt, height: 1pt)");
    let native = std::fs::read_dir(dir.join("missing")).unwrap_err();

    let stderr = failed(run(dir, &["compile", "doc.typk", "missing/out.svg"]));

    assert_eq!(
        stderr,
        format!("error: cannot write `missing/out.svg`: {native}\n")
    );
    assert!(!dir.join("missing").exists());
}

#[test]
fn inspect_treats_a_dash_as_a_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let inspect = typst_pack(dir.path())
        .args(["inspect", "-"])
        .stdin(Stdio::null())
        .output();
    assert!(failed(inspect.unwrap()).contains("cannot open pack `-`"));
}

// --- compile: controls ----------------------------------------------------

#[test]
fn features_accept_lists_and_reject_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(
        dir,
        "doc.typk",
        "#pdf.table-summary(summary: \"One cell\", table([Visible marker]))",
    );

    ok(run(
        dir,
        &[
            "compile",
            "doc.typk",
            "doc.html",
            "--features",
            "html,a11y-extras",
        ],
    ));
    let html = std::fs::read_to_string(dir.join("doc.html")).unwrap();
    assert!(html.contains("Visible marker"));
    failed(run(
        dir,
        &["compile", "doc.typk", "bundle.pdf", "--features", "bundle"],
    ));
}

fn pdf_has_accessibility_tags(bytes: Vec<u8>) -> bool {
    use hayro_syntax::object::{Dict, Name};

    let pdf = hayro_syntax::Pdf::new(bytes).unwrap();
    let xref = pdf.xref();
    let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) else {
        return false;
    };
    let structure = catalog.get::<Dict<'_>>(b"StructTreeRoot");
    let marked = catalog.get::<Dict<'_>>(b"MarkInfo");
    structure.is_some_and(|structure| {
        structure
            .get::<Name<'_>>(b"Type")
            .is_some_and(|name| name.as_str() == "StructTreeRoot")
            && structure.contains_key(b"K")
    }) && marked.is_some_and(|info| info.get::<bool>(b"Marked") == Some(true))
}

#[test]
fn pdf_standards_and_tags_are_validated_with_every_hint() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(
        dir,
        "doc.typk",
        "#set document(title: \"Test\", author: \"Tester\")\n#set text(lang: \"en\")\n= Heading\nBody",
    );
    pages(dir, 2);
    let compile = |args: &[&str]| run(dir, &[&["compile", "doc.typk"], args].concat());

    ok(compile(&["standards.pdf", "--pdf-standard", "a-2b,ua-1"]));
    ok(compile(&["tagged.pdf"]));
    ok(compile(&["untagged.pdf", "--no-pdf-tags"]));
    let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
    assert!(pdf_has_accessibility_tags(read("tagged.pdf")));
    assert!(!pdf_has_accessibility_tags(read("untagged.pdf")));

    assert!(failed(compile(&["x.pdf", "--pdf-standard", "invalid"])).contains("possible values"));
    let stderr = failed(compile(&["x.pdf", "--pdf-standard", "a-4,ua-1"]));
    assert!(stderr.contains("mutually incompatible"), "{stderr}");
    assert_eq!(stderr.matches("hint: ").count(), 2, "{stderr}");

    // Page selection disables tags, which accessible standards need,
    // whatever the output format.
    for output in ["x.pdf", "x.svg"] {
        let args = [
            "compile",
            "pages.typk",
            output,
            "--pages",
            "2",
            "--pdf-standard",
            "ua-1",
        ];
        let stderr = failed(run(dir, &args));
        assert!(stderr.contains("cannot disable PDF tags"), "{stderr}");
        assert!(
            stderr.contains("hint: using --pages implies --no-pdf-tags"),
            "{stderr}"
        );
    }
    assert!(!dir.join("x.pdf").exists());
}

#[test]
fn source_date_epoch_and_the_creation_timestamp_fix_dates() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(
        dir,
        "dated.typk",
        "#assert.eq(datetime.today(offset: 2).year(), 2000)\nDated",
    );
    let compile = |output: &str, epoch: &str| {
        let args = ["compile", "dated.typk", output];
        typst_pack(dir)
            .env("SOURCE_DATE_EPOCH", epoch)
            .args(args)
            .output()
            .unwrap()
    };

    assert!(failed(compile("x.pdf", "not-a-timestamp")).contains("invalid value"));
    ok(compile("first.pdf", "946684799"));
    ok(compile("second.pdf", "946684799"));
    let first = std::fs::read(dir.join("first.pdf")).unwrap();
    assert_eq!(first, std::fs::read(dir.join("second.pdf")).unwrap());
    let date = hayro_syntax::Pdf::new(first)
        .unwrap()
        .metadata()
        .creation_date
        .unwrap();
    assert_eq!(
        (date.year, date.month, date.day, date.hour),
        (1999, 12, 31, 23)
    );

    let out_of_range = [
        "compile",
        "dated.typk",
        "x.pdf",
        "--creation-timestamp",
        "9223372036854775807",
    ];
    assert!(failed(run(dir, &out_of_range)).contains("out of range"));
}

#[test]
fn the_compile_alias_and_version_name_the_typst_baseline() {
    let dir = tempfile::tempdir().unwrap();
    pages(dir.path(), 1);
    ok(run(dir.path(), &["c", "pages.typk", "alias.pdf"]));

    let version = ok(run(dir.path(), &["--version"])).stdout;
    let version = String::from_utf8_lossy(&version);
    assert!(version.contains(env!("CARGO_PKG_VERSION")), "{version}");
    assert!(version.contains("Typst 0.15.1"), "{version}");
}

/// Two directories, each holding one distinct font Typst ships.
fn distinct_fonts(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut selected: Vec<(String, Vec<u8>)> = Vec::new();
    for (font, info) in typst_kit::fonts::embedded() {
        let data = font.data().to_vec();
        if selected
            .iter()
            .all(|(family, existing)| *family != info.family && *existing != data)
        {
            selected.push((info.family, data));
        }
    }
    selected.truncate(2);
    let fonts = selected
        .into_iter()
        .enumerate()
        .map(|(index, (family, data))| {
            let path = dir.join(format!("font-{index}"));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("font.ttf"), data).unwrap();
            (path, family)
        });
    fonts.collect()
}

#[test]
fn font_and_package_environment_is_honored_by_compile() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(
        dir,
        "text.typk",
        "#set text(font: \"Libertinus Serif\")\nHello",
    );
    let no_fonts = typst_pack(dir)
        .env("TYPST_IGNORE_EMBEDDED_FONTS", "true")
        .args(["compile", "text.typk", "text.pdf"])
        .output();
    assert!(stderr(&ok(no_fonts.unwrap())).contains("font"));

    // Font paths are platform delimited.
    let fonts = distinct_fonts(dir);
    let source = format!(
        "#text(font: \"{}\")[A]\n#text(font: \"{}\")[B]",
        fonts[0].1, fonts[1].1
    );
    let mut builder = Pack::builder("main.typ").file("main.typ", source.into_bytes());
    for (path, _) in &fonts {
        builder = builder.external_font(std::fs::read(path.join("font.ttf")).unwrap());
    }
    save(dir, "fonts.typk", &builder.build().unwrap());
    let font_paths = std::env::join_paths(fonts.iter().map(|(path, _)| path)).unwrap();
    let fonts = typst_pack(dir)
        .env("TYPST_FONT_PATHS", font_paths)
        .args([
            "compile",
            "fonts.typk",
            "fonts.pdf",
            "--ignore-embedded-fonts",
        ])
        .output();
    assert!(!stderr(&ok(fonts.unwrap())).contains("unknown font family"));

    // The package path, then the cache, supply unvendored packages.
    requiring(dir, "shapes");
    write_package(&dir.join("packages"), "shapes");
    write_package(&dir.join("cache"), "shapes");
    std::fs::create_dir(dir.join("empty")).unwrap();
    for (path, cache, used) in [
        ("packages", "empty", "packages"),
        ("empty", "cache", "cache"),
    ] {
        let args = [
            "compile",
            "package.typk",
            "package.pdf",
            "--offline",
            "--deps",
            "deps.json",
        ];
        ok(typst_pack(dir)
            .env("TYPST_PACKAGE_PATH", dir.join(path))
            .env("TYPST_PACKAGE_CACHE_PATH", dir.join(cache))
            .args(args)
            .output()
            .unwrap());
        let library = dir.join(used).join("preview/shapes/0.1.0/lib.typ");
        let deps = json(dir.join("deps.json"));
        assert!(
            deps["inputs"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(library))
        );
    }
}

#[test]
fn external_fonts_are_named_when_missing_and_used_when_supplied() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let font = typst_kit::fonts::embedded().next().unwrap().0;
    let family = font.info().family.clone();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            format!("#set text(font: \"{family}\")\nExternal").into_bytes(),
        )
        .external_font(font.data().to_vec())
        .build()
        .unwrap();
    save(dir, "fonts.typk", &pack);
    std::fs::create_dir(dir.join("fonts")).unwrap();
    std::fs::write(dir.join("fonts/font.ttf"), font.data()).unwrap();
    let compile = [
        "compile",
        "fonts.typk",
        "out.pdf",
        "--ignore-embedded-fonts",
    ];

    let stderr = failed(run(dir, &compile));
    assert!(
        stderr.starts_with("error: required font container sha256:"),
        "{stderr}"
    );
    assert!(stderr.contains(&family), "{stderr}");
    assert!(stderr.contains("hint: `typst-pack inspect"), "{stderr}");
    assert!(!dir.join("out.pdf").exists());

    ok(typst_pack(dir)
        .args(compile)
        .args(["--font-path", "fonts"])
        .output()
        .unwrap());
    assert!(dir.join("out.pdf").is_file());
}

// --- diagnostics, dependencies, timings, viewing ------------------------

#[test]
fn diagnostics_render_source_names_in_order_without_a_trailer() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(dir, &[("project/main.typ", "#unknown-function()")]);
    pack(dir, "invalid.typk", "#unknown-function()");
    let short = ["--color", "never"];

    // Creation names the source relative to the invocation; compilation uses
    // the pack's virtual name.
    let create = [
        "create",
        "project/main.typ",
        "x.typk",
        "--diagnostic-format",
        "short",
    ];
    let stderr = failed(typst_pack(dir).args(short).args(create).output().unwrap());
    assert_eq!(
        stderr,
        "project/main.typ:1:1: error: unknown variable: unknown-function\n"
    );
    let compile = [
        "compile",
        "invalid.typk",
        "x.pdf",
        "--diagnostic-format",
        "short",
    ];
    let stderr = failed(typst_pack(dir).args(short).args(compile).output().unwrap());
    assert_eq!(
        stderr,
        "main.typ:1:1: error: unknown variable: unknown-function\n"
    );

    // Compiler warnings, then the CLI's own, then errors.
    pack(
        dir,
        "warned.typk",
        &format!("{WARNING}\n#unknown-function()"),
    );
    let output = run(dir, &["compile", "warned.typk", "x.pdf", "--pages", "1"]);
    assert!(output.stdout.is_empty());
    let stderr = failed(output);
    ordered(
        &stderr,
        &[
            "Definitely Missing",
            "using --pages implies --no-pdf-tags",
            "unknown-function",
        ],
    );
    assert!(!stderr.contains("compilation failed"), "{stderr}");
    assert!(!dir.join("x.pdf").exists());
}

#[test]
fn color_follows_the_global_control() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pack(dir, "invalid.typk", "#unknown-function()");
    write(dir, &[("main.typ", WARNING)]);
    let ansi = |output: &Output| output.stderr.contains(&0x1b);

    let owned = run(dir, &["--color", "always", "compile", "missing.typk"]);
    assert!(ansi(&owned));
    let warned = ok(run(
        dir,
        &["--color", "always", "create", "main.typ", "warned.typk"],
    ));
    assert!(ansi(&warned) && stderr(&warned).contains("Definitely Missing"));
    // Hints of the CLI's own validation are styled separately.
    let hinted = [
        "--color",
        "always",
        "compile",
        "missing.typk",
        "x.pdf",
        "--pdf-standard",
        "a-4,ua-1",
    ];
    let stderr = failed(run(dir, &hinted));
    let hints = stderr
        .lines()
        .filter(|line| line.contains("hint"))
        .collect::<Vec<_>>();
    assert_eq!(hints.len(), 2, "{stderr}");
    assert!(hints.iter().all(|line| line.contains("\x1b[")), "{stderr}");

    assert!(!ansi(&run(
        dir,
        &["--color", "never", "compile", "invalid.typk", "x.pdf"]
    )));
    assert!(!ansi(&run(dir, &["compile", "missing.typk"])));
    assert!(!ansi(&run(
        dir,
        &["--color", "auto", "compile", "missing.typk"]
    )));
}

#[test]
fn dependencies_follow_typst_formats_and_transport_rules() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let pack_path = pack(dir, "doc.typk", "#rect(width: 1pt, height: 1pt)");
    let deps = |output: &str, args: &[&str]| {
        let mut command = typst_pack(dir);
        command.arg("compile").arg(&pack_path).arg(output);
        command.arg("--deps").args(args).output().unwrap()
    };

    ok(deps("zero.svg", &["deps.zero", "--deps-format", "zero"]));
    let mut zero = pack_path.as_os_str().as_encoded_bytes().to_vec();
    zero.push(0);
    assert_eq!(std::fs::read(dir.join("deps.zero")).unwrap(), zero);
    ok(deps("make.svg", &["deps.mk", "--deps-format", "make"]));
    assert_eq!(
        std::fs::read_to_string(dir.join("deps.mk")).unwrap(),
        format!("make.svg: {}\n", pack_path.display())
    );
    let stdout = ok(deps("json.svg", &["-"])).stdout;
    let written: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(written["outputs"], serde_json::json!(["json.svg"]));
    let conflict = deps("-", &["-"]);
    assert!(conflict.stdout.is_empty());
    failed(conflict);

    // Dependencies are written even when compilation or export fails.
    pack(dir, "invalid.typk", "#unknown-function()");
    pages(dir, 2);
    for (pack, output) in [("invalid.typk", "x.pdf"), ("pages.typk", "pages.svg")] {
        let pack = dir.join(pack);
        let mut compile = typst_pack(dir);
        compile.arg("compile").arg(&pack).arg(output);
        failed(compile.args(["--deps", "deps.json"]).output().unwrap());
        let deps = json(dir.join("deps.json"));
        assert_eq!(deps["inputs"], serde_json::json!([pack]));
        assert_eq!(deps["outputs"], serde_json::Value::Null);
    }

    // Export warnings precede a dependency write error, after the output.
    pack(dir, "warned.typk", WARNING);
    let stderr = failed(run(
        dir,
        &["compile", "warned.typk", "warned.pdf", "--deps", "."],
    ));
    ordered(
        &stderr,
        &["Definitely Missing", "cannot write dependencies"],
    );
    assert!(dir.join("warned.pdf").is_file());
}

#[test]
fn timings_are_perfetto_json_and_their_failure_is_reported_last() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#read(\"value.txt\")".to_vec())
        .file("value.txt", b"provided".to_vec())
        .build()
        .unwrap();
    save(dir, "doc.typk", &pack);

    ok(run(
        dir,
        &[
            "compile",
            "doc.typk",
            "doc.pdf",
            "--timings",
            "timings.json",
        ],
    ));
    let timings = json(dir.join("timings.json"));
    let names = timings
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["name"].as_str());
    let names = names.collect::<std::collections::BTreeSet<_>>();
    for name in ["typst-pack compilation", "load file", "export"] {
        assert!(names.contains(name), "missing `{name}`: {names:?}");
    }

    // A directory is no timings file: that failure follows everything else.
    let timed = |args: &[&str]| failed(run(dir, &[args, &["--timings", "."]].concat()));
    pack_warned(dir);
    let stderr = timed(&["compile", "invalid.typk", "x.pdf"]);
    ordered(&stderr, &["unknown-function", "failed to create file"]);
    let stderr = timed(&["compile", "doc.typk", "out.pdf", "--deps", "deps.json"]);
    assert!(stderr.contains("failed to create file"), "{stderr}");
    assert!(dir.join("out.pdf").is_file() && dir.join("deps.json").is_file());
    let stderr = timed(&["create", "error.typ", "x.typk"]);
    ordered(
        &stderr,
        &[
            "unknown-function",
            "Definitely Missing",
            "failed to create file",
        ],
    );
    let stderr = timed(&["create", "warning.typ", "x.typk"]);
    ordered(&stderr, &["Definitely Missing", "failed to create file"]);
    assert!(!dir.join("x.typk").exists());

    // A saved dependency error is suppressed behind the primary failure.
    std::fs::create_dir(dir.join("blocked.pdf")).unwrap();
    let stderr = timed(&["compile", "warned.typk", "blocked.pdf", "--deps", "."]);
    ordered(
        &stderr,
        &[
            "cannot write `",
            "Definitely Missing",
            "failed to create file",
        ],
    );
    assert!(!stderr.contains("cannot write dependencies"), "{stderr}");
}

/// Asserts that `text` holds every part, in order.
#[track_caller]
fn ordered(text: &str, parts: &[&str]) {
    let positions = parts
        .iter()
        .map(|part| text.find(part).unwrap_or_else(|| panic!("{part}: {text}")));
    let positions = positions.collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
}

/// Writes the failing and warning sources the timing test packs and creates.
fn pack_warned(dir: &Path) {
    pack(dir, "invalid.typk", "#unknown-function()");
    pack(dir, "warned.typk", WARNING);
    write(
        dir,
        &[
            ("error.typ", &format!("{WARNING}\n#unknown-function()")),
            ("warning.typ", WARNING),
        ],
    );
}

#[test]
fn a_viewer_opens_the_first_emitted_page_unless_output_is_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pages(dir, 5);

    let stdout = [
        "compile",
        "pages.typk",
        "-",
        "--format",
        "svg",
        "--pages",
        "2",
    ];
    let output = ok(typst_pack(dir)
        .args(stdout)
        .args(["--open", "must-not-run"])
        .output()
        .unwrap());
    assert!(output.stdout.starts_with(b"<svg"));
    // With dependencies on stdout, the missing viewer is still launched.
    let deps = [
        "compile",
        "pages.typk",
        "doc.pdf",
        "--deps",
        "-",
        "--open",
        "does-not-exist",
    ];
    failed(run(dir, &deps));
    assert!(dir.join("doc.pdf").is_file());

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let viewer = dir.join("viewer");
        std::fs::write(
            &viewer,
            "#!/bin/sh\nprintf '%s' \"$1\" > \"$VIEWER_MARKER\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&viewer, std::fs::Permissions::from_mode(0o755)).unwrap();
        let marker = dir.join("opened.txt");
        let args = [
            "compile",
            "pages.typk",
            "page-{p}.svg",
            "--pages",
            "5,2",
            "--open",
        ];
        ok(typst_pack(dir)
            .env("VIEWER_MARKER", &marker)
            .args(args)
            .arg(&viewer)
            .output()
            .unwrap());
        for _ in 0..100 {
            if marker.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let page = dir.canonicalize().unwrap().join("page-2.svg");
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            page.to_string_lossy()
        );
    }
}

// --- page templates -------------------------------------------------------

#[test]
fn page_templates_expand_source_page_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pages(dir, 12);
    let exists = |name: &str| dir.join(name).is_file();

    let padded = [
        "compile",
        "pages.typk",
        "page-{p}-{0p}-{n}-of-{t}.svg",
        "--pages",
        "11,2",
    ];
    ok(run(dir, &padded));
    assert!(exists("page-2-02-02-of-12.svg") && exists("page-11-11-11-of-12.svg"));
    assert!(!exists("page-1-01-01-of-12.svg"));

    let repeated = [
        "compile",
        "pages.typk",
        "p{p}.svg",
        "--pages",
        "5",
        "--pages",
        "2-3",
    ];
    ok(run(dir, &repeated));
    assert!(exists("p2.svg") && exists("p3.svg") && exists("p5.svg"));
    assert!(!exists("p1.svg") && !exists("p4.svg"));
}

#[test]
fn multiple_page_outputs_need_an_indexable_template_and_never_collide() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pages(dir, 5);
    write(
        dir,
        &[("collision-5.svg", "keep me"), ("same.svg", "keep me")],
    );
    std::fs::create_dir(dir.join("page-2")).unwrap();
    std::fs::create_dir(dir.join("page-5")).unwrap();
    let compile = |output: &str| {
        run(
            dir,
            &[
                "compile",
                "pages.typk",
                output,
                "--format",
                "svg",
                "--pages",
                "2,5",
            ],
        )
    };

    assert!(failed(compile("collision-{t}.svg")).contains("page number template"));
    failed(compile("page-{p}/../same.svg"));
    for name in ["collision-5.svg", "same.svg"] {
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"keep me");
    }

    // Without an output the pack's name is no template; timings still record
    // the export.
    let omitted = ["compile", "pages.typk", "--format", "svg", "--pages", "5,2"];
    let stderr = failed(
        typst_pack(dir)
            .args(omitted)
            .args(["--timings", "t.json"])
            .output()
            .unwrap(),
    );
    assert!(stderr.contains("page number template"), "{stderr}");
    assert!(!dir.join("pages.svg").exists());
    let timings = json(dir.join("t.json")).to_string();
    assert!(timings.contains("\"export\""), "{timings}");

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;

        let multi = std::ffi::OsString::from_vec(b"multi-\xff-{p}.svg".to_vec());
        let output = typst_pack(dir)
            .args(["compile", "pages.typk"])
            .arg(&multi)
            .args(["--pages", "2,5"])
            .output();
        assert!(failed(output.unwrap()).contains("page number template"));
    }
}

#[test]
fn document_and_single_page_outputs_are_literal() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    pages(dir, 5);
    let literal = |output: &str, format: &str, pages: &[&str]| {
        let args = ["compile", "pages.typk", output, "--format", format];
        ok(typst_pack(dir).args(args).args(pages).output().unwrap());
        assert!(dir.join(output).is_file(), "{output}");
    };

    literal("document-{p}.pdf", "pdf", &[]);
    literal("document-{t}.pdf", "pdf", &[]);
    literal("only.svg", "svg", &["--pages", "5"]);
    literal("only-{t}.svg", "svg", &["--pages", "5"]);
    assert!(!dir.join("document-1.pdf").exists() && !dir.join("only-5.svg").exists());

    // An omitted output inherits the pack's name, expanded only for pages.
    let pack_path = pack(dir, "doc-{p}-{0p}-{n}.typk", "Hello");
    let omitted = |format: &str| {
        let output = typst_pack(dir)
            .arg("compile")
            .arg(&pack_path)
            .args(["--format", format, "--deps", "deps.json"])
            .output();
        ok(output.unwrap());
    };
    omitted("svg");
    let outputs = &json(dir.join("deps.json"))["outputs"];
    assert_eq!(*outputs, serde_json::json!([dir.join("doc-1-1-1.svg")]));
    for format in ["pdf", "html"] {
        omitted(format);
        assert!(pack_path.with_extension(format).is_file(), "{format}");
    }

    // A selection matching no page writes nothing and still warns.
    pack(dir, "warned.typk", WARNING);
    let output = ok(run(
        dir,
        &["compile", "warned.typk", "--format", "svg", "--pages", "9"],
    ));
    assert!(stderr(&output).contains("Definitely Missing") && output.stdout.is_empty());
    assert!(!dir.join("warned.svg").exists());

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;

        let single = std::ffi::OsString::from_vec(b"single-\xff-{p}.svg".to_vec());
        let output = typst_pack(dir)
            .args(["compile", "pages.typk"])
            .arg(&single)
            .args(["--pages", "2", "--deps", "deps.mk", "--deps-format", "make"])
            .output()
            .unwrap();
        if cfg!(target_os = "macos") {
            assert!(failed(output).contains("Illegal byte sequence"));
        } else {
            ok(output);
            assert!(dir.join(&single).is_file());
            let deps = std::fs::read(dir.join("deps.mk")).unwrap();
            assert_eq!(deps, b": pages.typk\n");
        }
    }
}

// --- package downloads ----------------------------------------------------

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
#[test]
fn certificates_reach_the_package_downloader_unless_offline() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[
            ("main.typ", "#import \"@preview/probe:0.1.0\": mark\n#mark"),
            ("ca.pem", "probe"),
        ],
    );
    requiring(dir, "probe");
    std::fs::create_dir(dir.join("empty")).unwrap();
    let paths = ["--package-path", "empty", "--package-cache-path", "empty"];

    for command in [
        ["create", "main.typ", "out.typk"],
        ["compile", "package.typk", "out.pdf"],
    ] {
        for environment in [false, true] {
            let probe = dir.join("probe.txt");
            let mut process = typst_pack(dir);
            process.env(PROBE, &probe);
            if environment {
                process.env("TYPST_CERT", "ca.pem");
            } else {
                process.args(["--cert", "ca.pem"]);
            }
            let stderr = failed(process.args(command).args(paths).output().unwrap());
            let recorded = std::fs::read_to_string(&probe);
            assert_eq!(recorded.ok().as_deref(), Some("ca.pem"), "{stderr}");
            std::fs::remove_file(probe).unwrap();
        }

        // Offline, a missing package fails at its import without a download.
        let output = typst_pack(dir)
            .env(PROBE, dir.join("probe.txt"))
            .args(command)
            .args(paths)
            .arg("--offline")
            .output()
            .unwrap();
        assert!(output.stdout.is_empty());
        let stderr = failed(output);
        if command[0] == "create" {
            assert!(
                stderr.contains("package not found") && stderr.contains("main.typ:1:8"),
                "{stderr}"
            );
        } else {
            assert!(
                stderr.contains("@preview/probe:0.1.0 was not supplied"),
                "{stderr}"
            );
        }
        assert!(!dir.join("probe.txt").exists() && !dir.join(command[2]).exists());
    }
}

#[cfg(all(not(feature = "_test-package-download-probe"), debug_assertions))]
#[test]
fn an_ordinary_build_ignores_the_download_probe() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    write(
        dir,
        &[("main.typ", "#import \"@preview/inert:0.1.0\": mark\n#mark")],
    );
    std::fs::create_dir(dir.join("empty")).unwrap();

    let output = typst_pack(dir)
        .env(
            "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE",
            dir.join("probe.txt"),
        )
        .args(["--cert", "missing.pem", "create", "main.typ", "out.typk"])
        .args(["--package-path", "empty", "--package-cache-path", "empty"])
        .output()
        .unwrap();

    assert!(output.stdout.is_empty());
    failed(output);
    assert!(!dir.join("probe.txt").exists() && !dir.join("out.typk").exists());
}
