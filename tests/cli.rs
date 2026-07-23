#![cfg(feature = "cli")]

use std::io::Write as _;
use std::process::{Command, Stdio};

use hayro_syntax::object::{DateTime, Dict, Name};
use typst_pack::Pack;

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

fn write_minimal_project(directory: &std::path::Path) -> std::path::PathBuf {
    let project = directory.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();
    project
}

fn pdf_has_accessibility_tags(bytes: Vec<u8>) -> bool {
    let pdf = hayro_syntax::Pdf::new(bytes).unwrap();
    let xref = pdf.xref();
    let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) else {
        return false;
    };
    let Some(structure) = catalog.get::<Dict<'_>>(b"StructTreeRoot") else {
        return false;
    };
    let Some(mark_info) = catalog.get::<Dict<'_>>(b"MarkInfo") else {
        return false;
    };

    structure
        .get::<Name<'_>>(b"Type")
        .is_some_and(|name| name.as_str() == "StructTreeRoot")
        && structure.contains_key(b"K")
        && mark_info.get::<bool>(b"Marked") == Some(true)
}

fn help_section<'a>(help: &'a str, heading: &str, next_heading: Option<&str>) -> &'a str {
    let marker = format!("\n{heading}\n");
    let section = help
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing help heading {heading}\n{help}"))
        .1;
    match next_heading {
        Some(next) => {
            section
                .split_once(&format!("\n{next}\n"))
                .unwrap_or_else(|| panic!("missing help heading {next}\n{help}"))
                .0
        }
        None => section,
    }
}

fn write_distinct_embedded_fonts(directory: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut selected: Vec<(String, Vec<u8>)> = Vec::new();
    for (font, info) in typst_kit::fonts::embedded() {
        let data = font.data().to_vec();
        if selected.iter().any(|(family, _)| family == &info.family)
            || selected
                .iter()
                .any(|(_, existing)| existing.as_slice() == data.as_slice())
        {
            continue;
        }
        selected.push((info.family, data));
        if selected.len() == 2 {
            break;
        }
    }
    assert_eq!(
        selected.len(),
        2,
        "Typst must provide two distinct font files"
    );

    selected
        .into_iter()
        .enumerate()
        .map(|(index, (family, data))| {
            let path = directory.join(format!("font-{index}"));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("font.ttf"), data).unwrap();
            (path, family)
        })
        .collect()
}

fn write_cached_package(cache: &std::path::Path, name: &str) -> std::path::PathBuf {
    let package = cache.join(format!("preview/{name}/0.1.0"));
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("typst.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n"),
    )
    .unwrap();
    std::fs::write(
        package.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();
    package
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
fn assert_probe_recorded_certificate(
    result: std::process::Output,
    probe: &std::path::Path,
    certificate: &std::path::Path,
) {
    assert!(!result.status.success());
    assert_eq!(
        std::fs::read_to_string(probe).unwrap(),
        certificate.to_string_lossy()
    );
}

#[test]
fn create_uses_source_input_and_derives_default_output() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");
    let expected = project.join("main.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args(["create", input.to_str().unwrap(), "--ignore-system-fonts"])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pack = Pack::from_bytes(std::fs::read(expected).unwrap()).unwrap();
    assert_eq!(pack.entrypoint(), "main.typ");
}

#[test]
fn create_writes_pack_to_stdout() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            "-",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pack = Pack::from_bytes(result.stdout).unwrap();
    assert_eq!(pack.entrypoint(), "main.typ");
}

#[test]
fn create_rejects_stdin_input() {
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .args(["create", "-", "output.typk"])
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "error: create input must be a named Typst source file, not stdin\n"
    );
}

#[test]
fn create_validates_creation_timestamp_before_input_and_root_io() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");
    let missing_input = directory.path().join("missing.typ");
    let missing_root = directory.path().join("missing-root");

    for arguments in [
        vec![
            "create",
            missing_input.to_str().unwrap(),
            "missing-input.typk",
            "--creation-timestamp",
            "9223372036854775807",
        ],
        vec![
            "create",
            input.to_str().unwrap(),
            "missing-root.typk",
            "--root",
            missing_root.to_str().unwrap(),
            "--creation-timestamp",
            "9223372036854775807",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .args(arguments)
            .output()
            .unwrap();

        assert!(!result.status.success());
        assert_eq!(
            String::from_utf8_lossy(&result.stderr),
            "error: creation timestamp is out of range\n"
        );
    }
}

#[test]
fn standalone_html_create_requires_and_accepts_the_html_feature() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");
    let rejected_output = directory.path().join("rejected.typk");

    let rejected = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env_remove("TYPST_FEATURES")
        .args([
            "create",
            input.to_str().unwrap(),
            rejected_output.to_str().unwrap(),
            "--target",
            "html",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(!rejected_output.exists());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("html"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );

    let accepted_output = directory.path().join("accepted.typk");
    let accepted = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env_remove("TYPST_FEATURES")
        .args([
            "create",
            input.to_str().unwrap(),
            accepted_output.to_str().unwrap(),
            "--target",
            "html",
            "--features",
            "html",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(Pack::from_bytes(std::fs::read(accepted_output).unwrap()).is_ok());
}

#[cfg(unix)]
#[test]
fn stdout_uses_typst_sigpipe_behavior() {
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::ExitStatusExt as _;

    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let writer: std::os::fd::OwnedFd = writer.into();
    let mut child = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            "-",
            "--ignore-system-fonts",
        ])
        .stdout(Stdio::from(writer))
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(13), "{status:?}");
}

#[test]
fn create_unions_repeated_and_comma_delimited_discovery_targets() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#context if target() == \"html\" { read(\"html.txt\") } else { read(\"paged.txt\") }",
    )
    .unwrap();
    std::fs::write(project.join("paged.txt"), "paged").unwrap();
    std::fs::write(project.join("html.txt"), "html").unwrap();
    let input = project.join("main.typ");
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--target",
            "paged,html",
            "--features",
            "html",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pack = Pack::from_bytes(std::fs::read(output).unwrap()).unwrap();
    assert!(pack.file("paged.txt").is_some());
    assert!(pack.file("html.txt").is_some());
}

#[test]
fn create_rejects_legacy_input_and_option_shapes() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");

    let directory_input = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            project.to_str().unwrap(),
            "directory.typk",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(!directory_input.status.success());
    assert!(String::from_utf8_lossy(&directory_input.stderr).contains("Typst source file"));

    for arguments in [
        vec![
            "create",
            input.to_str().unwrap(),
            "entrypoint.typk",
            "--entrypoint",
            "main.typ",
        ],
        vec![
            "create",
            input.to_str().unwrap(),
            "packages.typk",
            "--no-packages",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .args(arguments)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("unexpected argument"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn create_root_and_typst_root_define_the_pack_project_tree() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("src/main.typ"), "#include \"../shared.typ\"").unwrap();
    std::fs::write(project.join("shared.typ"), "Shared").unwrap();
    let input = project.join("src/main.typ");

    for (name, use_environment) in [("flag", false), ("environment", true)] {
        let output = directory.path().join(format!("{name}.typk"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command.current_dir(directory.path()).args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
        ]);
        if use_environment {
            command.env("TYPST_ROOT", &project);
        } else {
            command.args(["--root", project.to_str().unwrap()]);
        }
        let result = command.output().unwrap();

        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let pack = Pack::from_bytes(std::fs::read(output).unwrap()).unwrap();
        assert_eq!(pack.entrypoint(), "src/main.typ");
        assert!(pack.file("shared.typ").is_some());
    }
}

#[test]
fn legacy_resource_options_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());

    for (option, value) in [
        ("--source-reference", "resources"),
        ("--external-resource", "resource.bin"),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .args([
                "create",
                project.to_str().unwrap(),
                "output.typk",
                option,
                value,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);

        assert!(!result.status.success(), "{option} was accepted");
        assert!(stderr.contains("unexpected argument"), "{stderr}");
    }
}

#[test]
fn resource_paths_preserve_order_for_create_and_compile() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    std::fs::write(
        project.join("main.typ"),
        "#assert(read(\"resource.bin\") == \"first\")\n#rect(width: 1pt, height: 1pt)",
    )
    .unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::write(first.join("resource.bin"), "first").unwrap();
    std::fs::write(second.join("resource.bin"), "second").unwrap();
    let pack_path = directory.path().join("project.typk");
    let input = project.join("main.typ");

    let created = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            pack_path.to_str().unwrap(),
            "--resource-path",
            first.to_str().unwrap(),
            "--resource-path",
            second.to_str().unwrap(),
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let pack = Pack::from_bytes(std::fs::read(&pack_path).unwrap()).unwrap();
    assert_eq!(pack.resource_slots().collect::<Vec<_>>(), ["resource.bin"]);

    let output = directory.path().join("output.svg");
    let compiled = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--resource-path",
            first.to_str().unwrap(),
            "--resource-path",
            second.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn unavailable_resource_slot_creation_reports_the_complete_remedy() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(&input, "#read(\"branding/logo.bin\")").unwrap();
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--resource-slot",
            "branding/logo.bin",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(!output.exists());
    assert_eq!(
        stderr,
        "error: requested Resource Slot `branding/logo.bin` is unavailable for discovery; place representative bytes at `branding/logo.bin` in the source project or supply them via `--resource-path`; representative bytes are not stored in the Pack\n"
    );
}

#[test]
fn create_reports_source_diagnostics_before_a_timing_failure() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        "#set text(font: \"Definitely Missing\")\n#unknown-function()",
    )
    .unwrap();
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let source_error = stderr.find("unknown-function").unwrap();
    let warning = stderr.find("Definitely Missing").unwrap();
    let timing = stderr.find("failed to create file").unwrap();
    assert!(source_error < warning, "{stderr}");
    assert!(warning < timing, "{stderr}");
    assert!(!output.exists());
}

#[test]
fn create_reports_resource_slot_guidance_before_a_timing_failure() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(&input, "#read(\"branding/logo.bin\")").unwrap();
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--resource-slot",
            "branding/logo.bin",
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let guidance = stderr.find("place representative bytes").unwrap();
    let timing = stderr.find("failed to create file").unwrap();
    assert!(guidance < timing, "{stderr}");
    assert!(stderr.contains("--resource-path"), "{stderr}");
    assert!(!output.exists());
}

#[test]
fn create_reports_successful_discovery_warnings_before_a_timing_failure() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    let included = project.join("included");
    std::fs::create_dir_all(&included).unwrap();
    let input = project.join("main.typ");
    std::fs::write(&input, "#set text(font: \"Definitely Missing\")\nWarning").unwrap();
    std::fs::write(included.join("nested.typk"), "not a pack").unwrap();
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--include",
            included.to_str().unwrap(),
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let compile_warning = stderr.find("Definitely Missing").unwrap();
    let pack_warning = stderr.find("skipped pack file").unwrap();
    let timing = stderr.find("failed to create file").unwrap();
    assert!(compile_warning < pack_warning, "{stderr}");
    assert!(pack_warning < timing, "{stderr}");
    assert!(!output.exists());
}

#[test]
fn inspect_lists_resource_slots() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args(["inspect", pack_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);

    assert!(result.status.success(), "{stdout}");
    assert!(
        stdout.contains("\nResource Slots:\n  assets/logo.png\n"),
        "{stdout}"
    );
}

#[test]
fn inspect_treats_a_dash_as_a_pack_file_path() {
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .args(["inspect", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("cannot open `-`"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn extract_preserves_prefilled_resource_slots_without_materializing_them() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("ordinary.txt", b"ordinary".to_vec())
        .unwrap()
        .resource_slot("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    let output = directory.path().join("extracted");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    std::fs::create_dir_all(output.join("assets")).unwrap();
    std::fs::write(output.join("assets/logo.png"), b"prefilled").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "extract",
            pack_path.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(stdout.contains("extracted 2 file(s)"), "{stdout}");
    assert!(
        stdout.contains("\nResource Slots (not extracted):\n  assets/logo.png\n"),
        "{stdout}"
    );
    assert_eq!(std::fs::read(output.join("main.typ")).unwrap(), b"main");
    assert_eq!(
        std::fs::read(output.join("ordinary.txt")).unwrap(),
        b"ordinary"
    );
    assert_eq!(
        std::fs::read(output.join("assets/logo.png")).unwrap(),
        b"prefilled"
    );
}

fn write_five_page_pack(directory: &std::path::Path) -> std::path::PathBuf {
    let source = (1..=5)
        .map(|page| {
            format!(
                "#set page(width: {page}0pt, height: 10pt, margin: 0pt)\n\
                 #rect(width: 1pt, height: 1pt)\n"
            ) + if page < 5 { "#pagebreak()\n" } else { "" }
        })
        .collect::<String>();
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap()
        .build()
        .unwrap();
    let path = directory.join("selection.typk");
    std::fs::write(&path, pack.to_bytes().unwrap()).unwrap();

    path
}

#[test]
fn compile_reads_pack_from_stdin_and_writes_one_artifact_to_stdout() {
    let directory = tempfile::tempdir().unwrap();
    let pack_path = write_five_page_pack(directory.path());
    let pack_bytes = std::fs::read(pack_path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            "-",
            "-",
            "--format",
            "svg",
            "--pages",
            "2",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&pack_bytes).unwrap();
    let result = child.wait_with_output().unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.starts_with(b"<svg"));
}

#[test]
fn compile_validates_input_independent_options_before_pack_io_in_typst_order() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.typk");
    let invalid_extension = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            missing.to_str().unwrap(),
            "output.unknown",
            "--pdf-standard",
            "a-4,ua-1",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&invalid_extension.stderr);
    assert!(!invalid_extension.status.success());
    assert!(stderr.contains("cannot infer output format"), "{stderr}");
    assert!(!stderr.contains("cannot open"), "{stderr}");

    let corrupt = directory.path().join("corrupt.typk");
    std::fs::write(&corrupt, b"not a Pack").unwrap();
    let inaccessible = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            corrupt.to_str().unwrap(),
            "output.pdf",
            "--pages",
            "1",
            "--pdf-standard",
            "ua-1",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&inaccessible.stderr);
    assert!(!inaccessible.status.success());
    assert!(stderr.contains("cannot disable PDF tags"), "{stderr}");
    assert!(!stderr.contains("zip"), "{stderr}");

    let incompatible = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            "-",
            "-",
            "--pdf-standard",
            "a-4,ua-1",
            "--deps",
            "-",
            "--creation-timestamp",
            "9223372036854775807",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&incompatible.stderr);
    assert!(!incompatible.status.success());
    assert!(stderr.contains("mutually incompatible"), "{stderr}");
    assert!(!stderr.contains("both output and dependencies"), "{stderr}");
    assert!(!stderr.contains("timestamp"), "{stderr}");

    let stdout_conflict = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            "-",
            "-",
            "--deps",
            "-",
            "--creation-timestamp",
            "9223372036854775807",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&stdout_conflict.stderr);
    assert!(!stdout_conflict.status.success());
    assert!(
        stderr.contains("cannot write both output and dependencies to stdout"),
        "{stderr}"
    );
    assert!(!stderr.contains("timestamp"), "{stderr}");

    let invalid_timestamp = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            "-",
            "output.pdf",
            "--creation-timestamp",
            "9223372036854775807",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&invalid_timestamp.stderr);
    assert!(!invalid_timestamp.status.success());
    assert!(stderr.contains("timestamp"), "{stderr}");
    assert!(stderr.contains("out of range"), "{stderr}");
    assert!(!stderr.contains("zip"), "{stderr}");
}

#[test]
fn every_output_format_can_write_one_artifact_to_stdout() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    for (format, signature) in [
        ("pdf", b"%PDF".as_slice()),
        ("html", b"<!DOCTYPE html>".as_slice()),
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        ("svg", b"<svg".as_slice()),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command.current_dir(directory.path()).args([
            "compile",
            pack_path.to_str().unwrap(),
            "-",
            "--format",
            format,
            "--ignore-system-fonts",
        ]);
        if format == "html" {
            command.args(["--features", "html"]);
        }
        if matches!(format, "png" | "svg") {
            command.args(["--pages", "1"]);
        }
        let result = command.output().unwrap();

        assert!(
            result.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.starts_with(signature), "{format}");
    }
}

#[test]
fn compile_rejects_ambiguous_stdin_and_stdout_destinations() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());

    let missing_output = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args(["compile", "-"])
        .output()
        .unwrap();
    assert!(!missing_output.status.success());
    assert!(
        String::from_utf8_lossy(&missing_output.stderr).contains("explicit output"),
        "{}",
        String::from_utf8_lossy(&missing_output.stderr)
    );

    let multiple_pages = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "-",
            "--format",
            "png",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(!multiple_pages.status.success());
    assert!(multiple_pages.stdout.is_empty());

    let no_pages = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "-",
            "--format",
            "svg",
            "--pages",
            "9",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(!no_pages.status.success());
    assert!(no_pages.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&no_pages.stderr).contains("exactly one file"),
        "{}",
        String::from_utf8_lossy(&no_pages.stderr)
    );
}

#[test]
fn compile_infers_typst_formats_case_insensitively_without_htm_alias() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#rect(width: 1pt, height: 1pt)".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    for (filename, extra) in [
        ("document.PDF", vec![]),
        ("page.PNG", vec![]),
        ("page.SVG", vec![]),
        ("document.HTML", vec!["--features", "html"]),
    ] {
        let output = directory.path().join(filename);
        let mut arguments = vec![
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ];
        arguments.extend(extra);
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{filename}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(output.is_file(), "{filename}");
    }

    let htm = directory.path().join("document.htm");
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            htm.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!htm.exists());
}

#[test]
fn explicit_format_overrides_a_conflicting_output_extension() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#rect(width: 1pt, height: 1pt)".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    let output = directory.path().join("actually-svg.pdf");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(std::fs::read(output).unwrap().starts_with(b"<svg"));
}

#[test]
fn compile_derives_default_output_next_to_the_pack() {
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#rect(width: 1pt, height: 1pt)".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = nested.join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(nested.join("project.pdf").is_file());
    assert!(!directory.path().join("project.pdf").exists());
}

#[test]
fn successful_file_compilation_keeps_stdout_empty() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("page.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pages",
            "2",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(output.is_file());
}

#[test]
fn compile_requires_format_for_an_extensionless_output() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#rect(width: 1pt, height: 1pt)".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let output = directory.path().join("document");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("cannot infer output format"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
}

#[test]
fn compile_accepts_applicable_features_and_rejects_bundle() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("document.pdf");

    let accepted = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--features",
            "a11y-extras",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    let rejected = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "bundle.pdf",
            "--features",
            "bundle",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
}

#[test]
fn compile_accepts_multiple_features_in_one_comma_delimited_value() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#pdf.table-summary(summary: \"One cell\", table(columns: 1, [Visible marker]))"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("features.typk");
    let output = directory.path().join("features.html");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--features",
            "html,a11y-extras",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::fs::read_to_string(output)
            .unwrap()
            .contains("Visible marker")
    );
}

#[test]
fn compile_parses_typed_comma_delimited_pdf_controls() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set document(title: \"Test\", author: \"Tester\")\n#set text(lang: \"en\")\nHello"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let output = directory.path().join("document.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pdf-standard",
            "a-2b,ua-1",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());

    let untagged = directory.path().join("untagged.pdf");
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            untagged.to_str().unwrap(),
            "--no-pdf-tags",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--pdf-standard",
            "invalid",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(!invalid.status.success());
    assert!(stderr.contains("possible values"), "{stderr}");
}

#[test]
fn incompatible_pdf_standards_render_all_validation_hints() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--pdf-standard",
            "a-4,ua-1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(stderr.contains("mutually incompatible"), "{stderr}");
    assert!(
        stderr.contains("hint: PDF/A-4 requires version PDF 2.0"),
        "{stderr}"
    );
    assert!(
        stderr.contains("hint: PDF/UA-1 requires a version between PDF 1.4 and PDF 1.7"),
        "{stderr}"
    );
    assert_eq!(stderr.matches("hint: ").count(), 2, "{stderr}");
}

#[test]
fn application_validation_hints_are_styled_separately_when_color_is_always() {
    let cases = [
        (
            vec![
                "--color",
                "always",
                "compile",
                "missing.typk",
                "output.pdf",
                "--pdf-standard",
                "a-4,ua-1",
            ],
            2,
        ),
        (
            vec![
                "--color",
                "always",
                "compile",
                "missing.typk",
                "output.pdf",
                "--pages",
                "1",
                "--pdf-standard",
                "ua-1",
            ],
            1,
        ),
    ];

    for (arguments, expected_hints) in cases {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .args(arguments)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&result.stderr);
        let hint_lines = stderr
            .lines()
            .filter(|line| line.contains("hint"))
            .collect::<Vec<_>>();

        assert!(!result.status.success());
        assert_eq!(hint_lines.len(), expected_hints, "{stderr}");
        assert!(
            hint_lines.iter().all(|line| line.contains("\x1b[")),
            "{stderr}"
        );
    }
}

#[test]
fn cli_pdf_tags_are_present_by_default_and_absent_when_disabled() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Accessible heading\nBody text".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    for (name, no_tags, expected) in [("tagged.pdf", false, true), ("untagged.pdf", true, false)] {
        let output = directory.path().join(name);
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command.current_dir(directory.path()).args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--creation-timestamp",
            "946684800",
            "--ignore-system-fonts",
        ]);
        if no_tags {
            command.arg("--no-pdf-tags");
        }
        let result = command.output().unwrap();

        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            pdf_has_accessibility_tags(std::fs::read(output).unwrap()),
            expected,
            "{name}"
        );
    }
}

#[test]
fn cli_pretty_changes_html_svg_and_pdf_but_not_png() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    for (format, differs) in [("html", true), ("svg", true), ("pdf", true), ("png", false)] {
        let compact = directory.path().join(format!("compact.{format}"));
        let pretty = directory.path().join(format!("pretty.{format}"));
        for (output, pretty) in [(&compact, false), (&pretty, true)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
            command.current_dir(directory.path()).args([
                "compile",
                pack_path.to_str().unwrap(),
                output.to_str().unwrap(),
                "--format",
                format,
                "--creation-timestamp",
                "946684800",
                "--ignore-system-fonts",
            ]);
            if format == "html" {
                command.args(["--features", "html"]);
            }
            if pretty {
                command.arg("--pretty");
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{format}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }

        let compact = std::fs::read(compact).unwrap();
        let pretty = std::fs::read(pretty).unwrap();
        assert_eq!(compact != pretty, differs, "{format}");
    }
}

#[test]
fn source_diagnostics_have_no_redundant_compilation_failed_trailer() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(stderr.contains("unknown-function"), "{stderr}");
    assert!(!stderr.contains("error: compilation failed"), "{stderr}");
}

#[test]
fn create_short_diagnostics_name_nested_source_relative_to_invocation_directory() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), "#unknown-function()").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "--color",
            "never",
            "create",
            "project/main.typ",
            "project.typk",
            "--diagnostic-format",
            "short",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "project/main.typ:1:1: error: unknown variable: unknown-function\n"
    );
}

#[test]
fn compile_short_diagnostics_use_pack_virtual_source_name() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "--color",
            "never",
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--diagnostic-format",
            "short",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "main.typ:1:1: error: unknown variable: unknown-function\n"
    );
}

#[test]
fn failed_pdf_compilation_retains_the_pages_warning() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Definitely Missing\")\nWarning\n#unknown-function()".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    let output = directory.path().join("invalid.pdf");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pages",
            "1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let error = stderr.find("unknown-function").unwrap();
    let compiler_warning = stderr.find("Definitely Missing").unwrap();
    let static_warning = stderr.find("using --pages implies --no-pdf-tags").unwrap();
    assert!(compiler_warning < static_warning, "{stderr}");
    assert!(static_warning < error, "{stderr}");
    assert!(!output.exists());
}

#[test]
fn input_pairs_trim_values_and_reject_empty_keys() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#assert(sys.inputs.at(\"key\") == \"value\")\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("input.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let accepted = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "input.pdf",
            "--input",
            " key = value ",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    let rejected = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--input",
            " = value",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("key was missing or empty"));
}

#[test]
fn malformed_source_date_epoch_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("SOURCE_DATE_EPOCH", "not-a-timestamp")
        .args([
            "compile",
            pack.to_str().unwrap(),
            "timestamp.pdf",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("invalid value"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn valid_source_date_epoch_is_available_to_typst_code() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#assert(datetime.today().year() == 2000)\n#rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("timestamp.typk");
    let output = directory.path().join("timestamp.svg");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("SOURCE_DATE_EPOCH", "946684800")
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn out_of_range_creation_timestamp_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let pack = directory.path().join("missing.typk");
    let output = directory.path().join("timestamp.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--creation-timestamp",
            "9223372036854775807",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert_eq!(stderr, "error: creation timestamp is out of range\n");
    assert!(!output.exists());
}

#[test]
fn creation_timestamp_preserves_typst_year_boundary_metadata_states() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("timestamp.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    for (name, timestamp, expected_year) in [
        ("year-9999.pdf", "253402300799", Some(9999)),
        ("year-10000.pdf", "253402300800", None),
    ] {
        let output = directory.path().join(name);
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .args([
                "compile",
                pack_path.to_str().unwrap(),
                output.to_str().unwrap(),
                "--creation-timestamp",
                timestamp,
                "--ignore-system-fonts",
                "--ignore-embedded-fonts",
            ])
            .output()
            .unwrap();

        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let creation_date = hayro_syntax::Pdf::new(std::fs::read(output).unwrap())
            .unwrap()
            .metadata()
            .creation_date;
        assert_eq!(creation_date.map(|date| date.year), expected_year, "{name}");
    }

    let output = directory.path().join("automatic.pdf");
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        hayro_syntax::Pdf::new(std::fs::read(output).unwrap())
            .unwrap()
            .metadata()
            .creation_date
            .is_some()
    );
}

#[test]
fn source_date_epoch_produces_deterministic_pdf_creation_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("timestamp.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let outputs = [
        directory.path().join("first.pdf"),
        directory.path().join("second.pdf"),
    ];

    for output in &outputs {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .current_dir(directory.path())
            .env("SOURCE_DATE_EPOCH", "946684800")
            .args([
                "compile",
                pack_path.to_str().unwrap(),
                output.to_str().unwrap(),
                "--ignore-system-fonts",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let first = std::fs::read(&outputs[0]).unwrap();
    let second = std::fs::read(&outputs[1]).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        hayro_syntax::Pdf::new(first)
            .unwrap()
            .metadata()
            .creation_date,
        Some(DateTime {
            year: 2000,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            utc_offset_hour: 0,
            utc_offset_minute: 0,
        })
    );
}

#[test]
fn creation_timestamp_respects_datetime_today_timezone_offsets() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            br#"#let date = datetime.today(offset: 2)
#assert(date.year() == 2000)
#assert(date.month() == 1)
#assert(date.day() == 1)
#rect(width: 1pt, height: 1pt)"#
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("timestamp.typk");
    let output = directory.path().join("timestamp.svg");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--creation-timestamp",
            "946684799",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn compile_alias_and_version_report_typst_parity_baseline() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("alias.pdf");

    let alias = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "c",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        alias.status.success(),
        "{}",
        String::from_utf8_lossy(&alias.stderr)
    );

    let version = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .arg("--version")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&version.stdout);
    assert!(version.status.success());
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
    assert!(stdout.contains("Typst 0.15.0"), "{stdout}");
}

#[test]
fn typst_font_and_package_environment_variables_are_honored() {
    let directory = tempfile::tempdir().unwrap();
    let text_pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Libertinus Serif\")\nHello".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let text_pack_path = directory.path().join("text.typk");
    std::fs::write(&text_pack_path, text_pack.to_bytes().unwrap()).unwrap();

    let no_fonts = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_IGNORE_SYSTEM_FONTS", "true")
        .env("TYPST_IGNORE_EMBEDDED_FONTS", "true")
        .args(["compile", text_pack_path.to_str().unwrap(), "no-fonts.pdf"])
        .output()
        .unwrap();
    assert!(no_fonts.status.success());
    assert!(
        String::from_utf8_lossy(&no_fonts.stderr).contains("font"),
        "{}",
        String::from_utf8_lossy(&no_fonts.stderr)
    );

    let package_root = directory.path().join("packages");
    let package = package_root.join("local/shapes/0.1.0");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("typst.toml"),
        "[package]\nname = \"shapes\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    std::fs::write(
        package.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();
    let spec = "@local/shapes:0.1.0".parse().unwrap();
    let package_pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/shapes:0.1.0\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .unvendored_package(spec)
        .build()
        .unwrap();
    let package_pack_path = directory.path().join("package.typk");
    std::fs::write(&package_pack_path, package_pack.to_bytes().unwrap()).unwrap();
    let package_deps = directory.path().join("package-deps.json");

    let package_result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_PACKAGE_PATH", &package_root)
        .args([
            "compile",
            package_pack_path.to_str().unwrap(),
            "package.pdf",
            "--deps",
            package_deps.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        package_result.status.success(),
        "{}",
        String::from_utf8_lossy(&package_result.stderr)
    );
    let deps: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package_deps).unwrap()).unwrap();
    assert!(
        deps["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == &serde_json::json!(package.join("lib.typ")))
    );
}

#[test]
fn platform_delimited_typst_font_paths_are_used_by_compile() {
    let directory = tempfile::tempdir().unwrap();
    let fonts = write_distinct_embedded_fonts(directory.path());
    let source = format!(
        "#text(font: \"{}\")[First]\n#text(font: \"{}\")[Second]",
        fonts[0].1, fonts[1].1
    );
    let mut builder = Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap();
    for (path, _) in &fonts {
        builder = builder
            .external_font(std::fs::read(path.join("font.ttf")).unwrap(), 0)
            .unwrap();
    }
    let pack = builder.build().unwrap();
    let pack_path = directory.path().join("fonts.typk");
    let output = directory.path().join("fonts.pdf");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let font_paths = std::env::join_paths(fonts.iter().map(|(path, _)| path)).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_FONT_PATHS", font_paths)
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(result.status.success(), "{stderr}");
    assert!(!stderr.contains("unknown font family"), "{stderr}");
    assert!(output.is_file());
}

#[test]
fn typst_package_cache_path_resolves_unvendored_packages_during_compile() {
    let directory = tempfile::tempdir().unwrap();
    let cache = directory.path().join("cache");
    write_cached_package(&cache, "compile-cache");
    let empty_packages = directory.path().join("empty-packages");
    std::fs::create_dir(&empty_packages).unwrap();
    let spec = "@preview/compile-cache:0.1.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@preview/compile-cache:0.1.0\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .unvendored_package(spec)
        .build()
        .unwrap();
    let pack_path = directory.path().join("package.typk");
    let output = directory.path().join("package.svg");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_PACKAGE_PATH", &empty_packages)
        .env("TYPST_PACKAGE_CACHE_PATH", &cache)
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn create_uses_shared_font_and_package_environment() {
    let directory = tempfile::tempdir().unwrap();
    let fonts = write_distinct_embedded_fonts(directory.path());
    let cache = directory.path().join("cache");
    write_cached_package(&cache, "create-cache");
    let empty_packages = directory.path().join("empty-packages");
    std::fs::create_dir(&empty_packages).unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        format!(
            "#import \"@preview/create-cache:0.1.0\": mark\n#set text(font: \"{}\")\n#mark\nCustom font",
            fonts[0].1
        ),
    )
    .unwrap();
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_FONT_PATHS", &fonts[0].0)
        .env("TYPST_PACKAGE_PATH", &empty_packages)
        .env("TYPST_PACKAGE_CACHE_PATH", &cache)
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--embed-fonts",
            "--include-typst-embedded-fonts",
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let pack = Pack::from_bytes(std::fs::read(output).unwrap()).unwrap();
    let spec = "@preview/create-cache:0.1.0".parse().unwrap();
    assert!(pack.package_file(&spec, "lib.typ").is_some());
    assert!(!pack.fonts().is_empty());
}

#[test]
fn no_vendor_packages_records_dependency_and_compiles_with_package_path() {
    let directory = tempfile::tempdir().unwrap();
    let packages = directory.path().join("packages");
    write_cached_package(&packages, "unvendored");
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(&input, "#import \"@preview/unvendored:0.1.0\": mark\n#mark").unwrap();
    let pack_path = directory.path().join("project.typk");

    let created = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            pack_path.to_str().unwrap(),
            "--no-vendor-packages",
            "--package-path",
            packages.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let pack = Pack::from_bytes(std::fs::read(&pack_path).unwrap()).unwrap();
    let spec = "@preview/unvendored:0.1.0".parse().unwrap();
    assert!(!pack.has_package(&spec));
    assert_eq!(pack.manifest().packages().unvendored(), [spec]);

    let output = directory.path().join("project.svg");
    let compiled = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn create_and_compile_accept_jobs_control() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let input = project.join("main.typ");
    let pack = directory.path().join("project.typk");

    let created = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            pack.to_str().unwrap(),
            "-j",
            "1",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let compiled = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "project.pdf",
            "-j",
            "1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}

#[test]
fn compile_writes_valid_perfetto_timings_json() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#read(\"data/value.txt\")\n#rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .unwrap()
        .resource_slot("data/value.txt")
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let resources = directory.path().join("resources");
    std::fs::create_dir_all(resources.join("data")).unwrap();
    std::fs::write(resources.join("data/value.txt"), "provided").unwrap();
    let timings = directory.path().join("timings.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "timed.pdf",
            "--resource-path",
            resources.to_str().unwrap(),
            "--timings",
            timings.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json = std::fs::read_to_string(timings).unwrap();
    let timings: serde_json::Value = serde_json::from_str(&json).unwrap();
    let names = timings
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for name in [
        "typst-pack compilation",
        "Pack",
        "Resource Provider",
        "export",
    ] {
        assert!(
            names.contains(name),
            "missing `{name}` timing span: {names:?}"
        );
    }
}

#[test]
fn timing_export_errors_are_reported_after_compilation_errors() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(stderr.contains("unknown-function"), "{stderr}");
    assert!(stderr.contains("failed to create file"), "{stderr}");
}

#[test]
fn timing_export_errors_happen_after_output_and_dependencies_are_written() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("timed.pdf");
    let dependencies = directory.path().join("timed.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--deps",
            dependencies.to_str().unwrap(),
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(stderr.contains("failed to create file"), "{stderr}");
    assert!(output.is_file());
    assert!(dependencies.is_file());
}

#[test]
fn successful_export_warnings_precede_dependency_errors_when_timings_succeed() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Definitely Missing\")\nWarning".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("warning.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let output = directory.path().join("output.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--deps",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let warning = stderr.find("Definitely Missing").unwrap();
    let dependencies = stderr.find("cannot write dependencies").unwrap();
    assert!(warning < dependencies, "{stderr}");
    assert!(output.is_file());
}

#[test]
fn timing_failure_suppresses_a_saved_dependency_error_after_preflight_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "pages.svg",
            "--deps",
            directory.path().to_str().unwrap(),
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let primary = stderr.find("page number template").unwrap();
    let timings = stderr.find("failed to create file").unwrap();
    assert!(primary < timings, "{stderr}");
    assert!(!stderr.contains("cannot write dependencies"), "{stderr}");
}

#[test]
fn filesystem_export_error_and_warnings_precede_timing_while_saved_errors_are_suppressed() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Definitely Missing\")\nWarning".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("warning.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let output = directory.path().join("output.pdf");
    std::fs::create_dir(&output).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--deps",
            directory.path().to_str().unwrap(),
            "--timings",
            directory.path().to_str().unwrap(),
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    let primary = stderr.find("cannot write `").unwrap();
    let warning = stderr.find("Definitely Missing").unwrap();
    let timings = stderr.find("failed to create file").unwrap();
    assert!(primary < warning, "{stderr}");
    assert!(warning < timings, "{stderr}");
    assert!(!stderr.contains("cannot write dependencies"), "{stderr}");
}

#[test]
fn dependency_json_reports_pack_and_consumed_resource_slot_files() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#assert(read(\"data/value.txt\") == \"provided\")\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .resource_slot("data/value.txt")
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let resources = directory.path().join("resources");
    std::fs::create_dir_all(resources.join("data")).unwrap();
    let resource = resources.join("data/value.txt");
    std::fs::write(&resource, "provided").unwrap();
    let output = directory.path().join("document.svg");
    let deps = directory.path().join("deps.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--resource-path",
            resources.to_str().unwrap(),
            "--deps",
            deps.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let deps: serde_json::Value = serde_json::from_slice(&std::fs::read(deps).unwrap()).unwrap();
    let inputs = deps["inputs"].as_array().unwrap();
    assert!(
        inputs
            .iter()
            .any(|value| value == &serde_json::json!(pack_path))
    );
    assert!(
        inputs
            .iter()
            .any(|value| value == &serde_json::json!(resource))
    );
    assert!(
        inputs
            .iter()
            .all(|value| !value.as_str().unwrap().ends_with("main.typ"))
    );
    assert_eq!(deps["outputs"], serde_json::json!([output]));
}

#[test]
fn dependency_formats_and_stdout_follow_typst_transport_rules() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#rect(width: 1pt, height: 1pt)".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let zero = directory.path().join("deps.zero");
    let zero_result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "zero.svg",
            "--deps",
            zero.to_str().unwrap(),
            "--deps-format",
            "zero",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(zero_result.status.success());
    let mut expected = pack_path.as_os_str().as_encoded_bytes().to_vec();
    expected.push(0);
    assert_eq!(std::fs::read(zero).unwrap(), expected);

    let make = directory.path().join("deps.mk");
    let output = directory.path().join("make.svg");
    let make_result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--deps",
            make.to_str().unwrap(),
            "--deps-format",
            "make",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(make_result.status.success());
    assert_eq!(
        std::fs::read_to_string(make).unwrap(),
        format!("{}: {}\n", output.display(), pack_path.display())
    );

    let stdout_output = directory.path().join("stdout.svg");
    let stdout_result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            stdout_output.to_str().unwrap(),
            "--deps",
            "-",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let deps: serde_json::Value = serde_json::from_slice(&stdout_result.stdout).unwrap();
    assert!(stdout_result.status.success());
    assert_eq!(deps["outputs"], serde_json::json!([stdout_output]));

    let conflict = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args(["compile", pack_path.to_str().unwrap(), "-", "--deps", "-"])
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    assert!(conflict.stdout.is_empty());
}

#[test]
fn json_dependencies_are_written_when_compilation_fails() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let deps = directory.path().join("deps.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
            "--deps",
            deps.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    let deps: serde_json::Value = serde_json::from_slice(&std::fs::read(deps).unwrap()).unwrap();
    assert_eq!(deps["inputs"], serde_json::json!([pack_path]));
    assert_eq!(deps["outputs"], serde_json::Value::Null);
}

#[test]
fn json_dependencies_are_written_when_output_export_fails() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let deps = directory.path().join("deps.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "pages.svg",
            "--deps",
            deps.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    let deps: serde_json::Value = serde_json::from_slice(&std::fs::read(deps).unwrap()).unwrap();
    assert_eq!(deps["inputs"], serde_json::json!([pack]));
    assert_eq!(deps["outputs"], serde_json::Value::Null);
}

#[test]
fn open_accepts_a_viewer_and_ignores_stdout_output() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "-",
            "--format",
            "svg",
            "--pages",
            "2",
            "--open",
            "viewer-that-must-not-run",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.starts_with(b"<svg"));
}

#[test]
fn open_is_processed_when_dependencies_are_written_to_stdout() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("document.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--deps",
            "-",
            "--open",
            "viewer-that-does-not-exist",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(output.is_file());
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn named_viewer_receives_the_first_emitted_page() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = std::path::PathBuf::from("page-{p}.svg");
    let viewer = directory.path().join("viewer");
    let marker = directory.path().join("opened.txt");
    std::fs::write(
        &viewer,
        "#!/bin/sh\nprintf '%s' \"$1\" > \"$VIEWER_MARKER\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&viewer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&viewer, permissions).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("VIEWER_MARKER", &marker)
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pages",
            "5,2",
            "--open",
            viewer.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for _ in 0..100 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read_to_string(marker).unwrap(),
        directory
            .path()
            .canonicalize()
            .unwrap()
            .join("page-2.svg")
            .to_string_lossy()
    );
}

#[test]
fn global_color_controls_owned_and_source_diagnostics() {
    let directory = tempfile::tempdir().unwrap();

    let owned = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args(["--color", "always", "compile", "missing.typk"])
        .output()
        .unwrap();
    assert!(!owned.status.success());
    assert!(owned.stderr.contains(&0x1b), "{:?}", owned.stderr);

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("invalid.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let source = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "--color",
            "never",
            "compile",
            pack_path.to_str().unwrap(),
            "invalid.pdf",
        ])
        .output()
        .unwrap();
    assert!(!source.status.success());
    assert!(!source.stderr.contains(&0x1b), "{:?}", source.stderr);
}

#[test]
fn auto_color_disables_ansi_when_stderr_is_captured() {
    for arguments in [
        vec!["compile", "missing.typk"],
        vec!["--color", "auto", "compile", "missing.typk"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .args(arguments)
            .output()
            .unwrap();

        assert!(!result.status.success());
        assert!(!result.stderr.contains(&0x1b), "{:?}", result.stderr);
    }
}

#[test]
fn global_color_controls_create_report_warnings() {
    let directory = tempfile::tempdir().unwrap();
    let project = write_minimal_project(directory.path());
    let included = project.join("included");
    std::fs::create_dir(&included).unwrap();
    std::fs::write(included.join("nested.typk"), b"ignored").unwrap();
    let input = project.join("main.typ");
    let output = directory.path().join("project.typk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "--color",
            "always",
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--include",
            included.to_str().unwrap(),
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("skipped pack file"));
    assert!(result.stderr.contains(&0x1b), "{:?}", result.stderr);
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
#[test]
fn create_forwards_argument_and_environment_certificates_to_package_downloader() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        "#import \"@preview/certificate-probe:0.1.0\": value\n#value",
    )
    .unwrap();
    let packages = directory.path().join("packages");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&packages).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let certificate = directory.path().join("ca.pem");
    std::fs::write(&certificate, "controlled by package download probe").unwrap();

    for (name, use_environment) in [("argument", false), ("environment", true)] {
        let output = directory.path().join(format!("{name}.typk"));
        let probe = directory.path().join(format!("create-{name}.txt"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command
            .current_dir(directory.path())
            .env_remove("TYPST_CERT")
            .env(PACKAGE_DOWNLOAD_PROBE_ENV, &probe);
        if use_environment {
            command.env("TYPST_CERT", &certificate);
        } else {
            command.args(["--cert", certificate.to_str().unwrap()]);
        }
        let result = command
            .args([
                "create",
                input.to_str().unwrap(),
                output.to_str().unwrap(),
                "--package-path",
                packages.to_str().unwrap(),
                "--package-cache-path",
                cache.to_str().unwrap(),
                "--ignore-system-fonts",
                "--ignore-embedded-fonts",
            ])
            .output()
            .unwrap();

        assert_probe_recorded_certificate(result, &probe, &certificate);
        assert!(!output.exists());
    }
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
#[test]
fn compile_forwards_argument_and_environment_certificates_to_package_downloader() {
    let directory = tempfile::tempdir().unwrap();
    let spec = "@preview/certificate-probe:0.1.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@preview/certificate-probe:0.1.0\": value\n#value".to_vec(),
        )
        .unwrap()
        .unvendored_package(spec)
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let packages = directory.path().join("packages");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&packages).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let certificate = directory.path().join("ca.pem");
    std::fs::write(&certificate, "controlled by package download probe").unwrap();

    for (name, use_environment) in [("argument", false), ("environment", true)] {
        let output = directory.path().join(format!("{name}.pdf"));
        let probe = directory.path().join(format!("compile-{name}.txt"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command
            .current_dir(directory.path())
            .env_remove("TYPST_CERT")
            .env(PACKAGE_DOWNLOAD_PROBE_ENV, &probe);
        if use_environment {
            command.env("TYPST_CERT", &certificate);
        } else {
            command.args(["--cert", certificate.to_str().unwrap()]);
        }
        let result = command
            .args([
                "compile",
                pack_path.to_str().unwrap(),
                output.to_str().unwrap(),
                "--package-path",
                packages.to_str().unwrap(),
                "--package-cache-path",
                cache.to_str().unwrap(),
                "--ignore-system-fonts",
                "--ignore-embedded-fonts",
            ])
            .output()
            .unwrap();

        assert_probe_recorded_certificate(result, &probe, &certificate);
        assert!(!output.exists());
    }
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
#[test]
fn create_offline_missing_package_does_not_activate_download_probe() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        "#import \"@preview/offline-probe:0.1.0\": value\n#value",
    )
    .unwrap();
    let packages = directory.path().join("packages");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&packages).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let output = directory.path().join("project.typk");
    let probe = directory.path().join("probe.txt");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env(PACKAGE_DOWNLOAD_PROBE_ENV, &probe)
        .args([
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--package-cache-path",
            cache.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(stderr.contains("package not found"), "{stderr}");
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(!probe.exists());
    assert!(!output.exists());
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
#[test]
fn compile_offline_missing_package_does_not_activate_download_probe() {
    let directory = tempfile::tempdir().unwrap();
    let spec = "@preview/offline-probe:0.1.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@preview/offline-probe:0.1.0\": value\n#value".to_vec(),
        )
        .unwrap()
        .unvendored_package(spec)
        .build()
        .unwrap();
    let pack_path = directory.path().join("project.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let packages = directory.path().join("packages");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&packages).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let output = directory.path().join("output.pdf");
    let probe = directory.path().join("probe.txt");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env(PACKAGE_DOWNLOAD_PROBE_ENV, &probe)
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--package-cache-path",
            cache.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(stderr.contains("package not found"), "{stderr}");
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(!probe.exists());
    assert!(!output.exists());
}

#[cfg(all(not(feature = "_test-package-download-probe"), debug_assertions))]
#[test]
fn ordinary_debug_build_ignores_package_download_probe_environment() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        "#import \"@preview/inert-probe:0.1.0\": value\n#value",
    )
    .unwrap();
    let packages = directory.path().join("packages");
    let cache = directory.path().join("cache");
    std::fs::create_dir(&packages).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let output = directory.path().join("project.typk");
    let probe = directory.path().join("probe.txt");
    let missing_certificate = directory.path().join("missing-ca.pem");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .env("TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE", &probe)
        .args([
            "--cert",
            missing_certificate.to_str().unwrap(),
            "create",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--package-cache-path",
            cache.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
    assert!(!probe.exists());
    assert!(!output.exists());
}

#[test]
fn command_help_is_task_grouped_and_documents_intentional_differences() {
    let global = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .arg("--help")
        .output()
        .unwrap();
    let global = String::from_utf8_lossy(&global.stdout);
    let global_flat = global.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(global_flat.contains(
        "--color [<COLOR>] Whether to use color. When set to `auto` if the terminal to supports it [default:"
    ));
    assert!(global_flat.contains(
        "--cert <PATH> Path to a custom CA certificate to use when making network requests [env:"
    ));
    assert!(
        !global.contains("Whether to use color in diagnostics."),
        "{global}"
    );
    assert!(!global.contains("used for package downloads"), "{global}");

    for (command, headings) in [
        (
            "compile",
            [
                "Compilation:",
                "Output:",
                "PDF:",
                "Resource Slots:",
                "Fonts:",
                "Packages:",
                "Diagnostics & Automation:",
                "",
            ],
        ),
        (
            "create",
            [
                "Project:",
                "Discovery:",
                "Pack Contents:",
                "Resource Slots:",
                "Fonts:",
                "Packages:",
                "Metadata:",
                "Diagnostics & Automation:",
            ],
        ),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .args([command, "--help"])
            .output()
            .unwrap();
        let help = String::from_utf8_lossy(&result.stdout);
        assert!(result.status.success());
        let mut position = 0;
        for heading in headings {
            if heading.is_empty() {
                continue;
            }
            let needle = format!("\n{heading}\n");
            let Some(offset) = help[position..].find(&needle) else {
                panic!("{command}: missing or out-of-order {heading}\n{help}");
            };
            position += offset + needle.len();
        }
        assert!(!help.contains("--source-reference"), "{help}");
        assert!(!help.contains("--external-resource"), "{help}");
    }

    let compile = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .args(["compile", "--help"])
        .output()
        .unwrap();
    let compile = String::from_utf8_lossy(&compile.stdout);
    let compile_flat = compile.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(compile.contains("Usage: typst-pack compile [OPTIONS] <PACK> [OUTPUT]"));
    assert!(!compile.contains("--root"));
    assert!(!compile.contains("bundle"));
    assert!(compile.contains("{p}"));
    assert!(compile.contains("{0p}"));
    assert!(compile.contains("{t}"));
    assert!(compile_flat.contains(
        "--format <FORMAT> The format of the output file, inferred from the extension by default [possible values:"
    ));
    assert!(
        !compile.contains("HTML is experimental and additionally requires"),
        "{compile}"
    );
    for wording in [
        "Path to output file (PDF, PNG, SVG, or HTML).",
        "page-01-of-10.png",
        "This formats the output in a more human-readable",
        "Which pages to export. When unspecified, all pages are exported.",
        "page 8 and any pages after it",
        "The PPI (pixels per inch) to use for PNG export",
        "Add a string key-value pair visible through `sys.inputs`",
        "Number of parallel jobs spawned during compilation.",
        "Enables in-development features that may be changed or removed",
        "Adds additional directories that are recursively searched for fonts",
        "Ensures system fonts won't be searched",
        "Ensures fonts embedded into Typst won't be considered",
        "Use `-` to write to stdout",
        "Ignored if output is stdout",
        "Produces performance timings of the compilation process. (experimental)",
        "trying to reduce the size of a document",
    ] {
        assert!(compile.contains(wording), "missing `{wording}`\n{compile}");
    }
    for (heading, next, options) in [
        (
            "Compilation:",
            Some("Output:"),
            ["<PACK>", "--input", "--features"].as_slice(),
        ),
        (
            "Output:",
            Some("PDF:"),
            ["[OUTPUT]", "--format", "--pretty", "--pages", "--ppi"].as_slice(),
        ),
        (
            "Fonts:",
            Some("Packages:"),
            [
                "--font-path",
                "--ignore-system-fonts",
                "--ignore-embedded-fonts",
            ]
            .as_slice(),
        ),
        (
            "Packages:",
            Some("Diagnostics & Automation:"),
            ["--package-path", "--package-cache-path", "--offline"].as_slice(),
        ),
        (
            "Diagnostics & Automation:",
            None,
            [
                "--jobs",
                "--creation-timestamp",
                "--diagnostic-format",
                "--timings",
                "--deps",
                "--deps-format",
                "--open",
            ]
            .as_slice(),
        ),
    ] {
        let section = help_section(&compile, heading, next);
        for option in options {
            assert!(
                section.contains(option),
                "{option} is not under {heading}\n{compile}"
            );
        }
    }

    let create = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .args(["create", "--help"])
        .output()
        .unwrap();
    let create = String::from_utf8_lossy(&create.stdout);
    assert!(create.contains("Usage: typst-pack create [OPTIONS] <INPUT> [OUTPUT]"));
    assert!(!create.contains("--entrypoint"));
    for (heading, next, options) in [
        (
            "Project:",
            Some("Discovery:"),
            ["<INPUT>", "--root"].as_slice(),
        ),
        (
            "Discovery:",
            Some("Pack Contents:"),
            ["--target", "--input", "--features"].as_slice(),
        ),
        (
            "Pack Contents:",
            Some("Resource Slots:"),
            ["[OUTPUT]", "--embed-fonts", "--include"].as_slice(),
        ),
        (
            "Fonts:",
            Some("Packages:"),
            [
                "--font-path",
                "--ignore-system-fonts",
                "--ignore-embedded-fonts",
            ]
            .as_slice(),
        ),
        (
            "Packages:",
            Some("Metadata:"),
            [
                "--no-vendor-packages",
                "--package-path",
                "--package-cache-path",
                "--offline",
            ]
            .as_slice(),
        ),
        (
            "Diagnostics & Automation:",
            None,
            [
                "--jobs",
                "--creation-timestamp",
                "--diagnostic-format",
                "--timings",
            ]
            .as_slice(),
        ),
    ] {
        let section = help_section(&create, heading, next);
        for option in options {
            assert!(
                section.contains(option),
                "{option} is not under {heading}\n{create}"
            );
        }
    }
}

#[test]
fn typst_015_value_help_uses_expected_metavars_and_descriptions() {
    let help = |command| {
        let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(result.status.success());
        String::from_utf8(result.stdout)
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };

    let create = help("create");
    assert!(create.contains("--features <FEATURES>"), "{create}");

    let compile = help("compile");
    assert!(compile.contains("--features <FEATURES>"), "{compile}");
    assert!(
        compile.contains("--pdf-standard <PDF_STANDARD>"),
        "{compile}"
    );

    for (value, description) in [
        ("json", "Encodes as JSON, failing for non-Unicode paths"),
        (
            "zero",
            "Separates paths with NULL bytes and can express all paths",
        ),
        ("make", "Emits in Make format, omitting inexpressible paths"),
    ] {
        let expected = format!("- {value}: {description}");
        assert!(
            compile.contains(&expected),
            "missing `{expected}`\n{compile}"
        );
    }

    for (value, description) in [
        ("1.4", "PDF 1.4"),
        ("1.5", "PDF 1.5"),
        ("1.6", "PDF 1.6"),
        ("1.7", "PDF 1.7"),
        ("2.0", "PDF 2.0"),
        ("a-1b", "PDF/A-1b"),
        ("a-1a", "PDF/A-1a"),
        ("a-2b", "PDF/A-2b"),
        ("a-2u", "PDF/A-2u"),
        ("a-2a", "PDF/A-2a"),
        ("a-3b", "PDF/A-3b"),
        ("a-3u", "PDF/A-3u"),
        ("a-3a", "PDF/A-3a"),
        ("a-4", "PDF/A-4"),
        ("a-4f", "PDF/A-4f"),
        ("a-4e", "PDF/A-4e"),
        ("ua-1", "PDF/UA-1"),
    ] {
        let expected = format!("- {value}: {description}");
        assert!(
            compile.contains(&expected),
            "missing `{expected}`\n{compile}"
        );
    }
}

#[test]
fn create_shares_timestamp_diagnostics_and_timings_controls() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let input = project.join("main.typ");
    std::fs::write(
        &input,
        "#assert(datetime.today().year() == 2000)\n#rect(width: 1pt, height: 1pt)",
    )
    .unwrap();
    let pack = directory.path().join("project.typk");
    let timings = directory.path().join("create-timings.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "create",
            input.to_str().unwrap(),
            pack.to_str().unwrap(),
            "--creation-timestamp",
            "946684800",
            "--diagnostic-format",
            "short",
            "--timings",
            timings.to_str().unwrap(),
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&std::fs::read(timings).unwrap()).unwrap();
}

#[test]
fn page_placeholders_use_source_page_numbers() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("page-{p}-{0p}-of-{t}.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "5,2",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(directory.path().join("page-2-2-of-5.svg").is_file());
    assert!(directory.path().join("page-5-5-of-5.svg").is_file());
    assert!(!directory.path().join("page-1-1-of-5.svg").exists());
}

#[test]
fn page_ranges_accept_repeated_and_comma_delimited_values() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("page-{p}.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pages",
            "5",
            "--pages",
            "2-3",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(directory.path().join("page-2.svg").is_file());
    assert!(directory.path().join("page-3.svg").is_file());
    assert!(directory.path().join("page-5.svg").is_file());
    assert!(!directory.path().join("page-1.svg").exists());
    assert!(!directory.path().join("page-4.svg").exists());
}

#[test]
fn total_page_placeholder_alone_is_not_indexable() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("collision-{t}.svg");
    let collided = directory.path().join("collision-5.svg");
    std::fs::write(&collided, b"keep me").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "2,5",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("page number template"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(std::fs::read(collided).unwrap(), b"keep me");
}

#[test]
fn lexically_aliased_paths_fail_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    std::fs::create_dir(directory.path().join("page-2")).unwrap();
    std::fs::create_dir(directory.path().join("page-5")).unwrap();
    let output = directory.path().join("page-{p}/../same.svg");
    let collided = directory.path().join("same.svg");
    std::fs::write(&collided, b"keep me").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "2,5",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert_eq!(std::fs::read(collided).unwrap(), b"keep me");
}

#[test]
fn document_format_output_paths_are_literal() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("document-{p}.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "pdf",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
    assert!(!directory.path().join("document-1.pdf").exists());
}

#[test]
fn accessible_pdf_standard_rejects_page_selection() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("accessible.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "pdf",
            "--pages",
            "2,5",
            "--pdf-standard",
            "a-1a",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(
        stderr.contains("cannot disable PDF tags when exporting a PDF/A-1a document"),
        "{stderr}"
    );
    assert!(
        stderr.contains("hint: using --pages implies --no-pdf-tags"),
        "{stderr}"
    );
    assert!(!output.exists());
}

#[test]
fn accessible_pdf_standard_rejects_explicitly_disabled_tags_with_the_standard_name() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("accessible.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--no-pdf-tags",
            "--pdf-standard",
            "ua-1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(!result.status.success());
    assert!(
        stderr.contains("cannot disable PDF tags when exporting a PDF/UA-1 document"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    assert!(!output.exists());
}

#[test]
fn accessible_pdf_standard_validation_is_independent_of_output_format() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("page.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--pages",
            "2",
            "--pdf-standard",
            "ua-1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("cannot disable PDF tags when exporting a PDF/UA-1 document"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
}

#[test]
fn empty_page_format_output_succeeds_and_reports_retained_compilation_warnings() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Definitely Missing\")\nWarning".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("warning.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "9",
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(result.status.success(), "{stderr}");
    assert!(stderr.contains("Definitely Missing"), "{stderr}");
    assert!(!pack_path.with_extension("svg").exists());
    assert!(result.stdout.is_empty(), "{:?}", result.stdout);
}

#[cfg(unix)]
#[test]
fn non_unicode_page_outputs_preserve_singletons_and_are_not_indexable() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory
        .path()
        .join(OsString::from_vec(b"single-\xff-{p}.svg".to_vec()));
    let deps = directory.path().join("deps.mk");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .arg("compile")
        .arg(&pack)
        .arg(&output)
        .args([
            "--pages",
            "2",
            "--deps",
            deps.to_str().unwrap(),
            "--deps-format",
            "make",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    #[cfg(target_os = "macos")]
    {
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("Illegal byte sequence"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let lossy_output = std::path::PathBuf::from(output.to_string_lossy().replace("{p}", "2"));
        assert!(!lossy_output.exists());
    }
    #[cfg(not(target_os = "macos"))]
    {
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(output.is_file());
        assert_eq!(
            std::fs::read(deps).unwrap(),
            format!(": {}\n", pack.display()).into_bytes()
        );
    }

    let multi = directory
        .path()
        .join(OsString::from_vec(b"multi-\xff-{p}.svg".to_vec()));
    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .arg("compile")
        .arg(&pack)
        .arg(&multi)
        .args([
            "--pages",
            "2,5",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("page number template"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!multi.exists());
}

#[test]
fn multi_page_output_requires_an_explicit_indexable_template() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let timings = directory.path().join("timings.json");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "5,2",
            "--timings",
            timings.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("page number template"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!directory.path().join("selection.svg").exists());
    let timings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(timings).unwrap()).unwrap();
    assert!(
        timings
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "export"),
        "{timings}"
    );
}

#[test]
fn singleton_page_artifact_accepts_literal_output_path() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("only.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "5",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
}

#[test]
fn omitted_page_output_expands_inherited_templates_but_document_outputs_are_literal() {
    let directory = tempfile::tempdir().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("document-{p}-{0p}-{n}.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let deps = directory.path().join("deps.json");
    let expanded = directory.path().join("document-1-1-1.svg");

    let svg = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            "--format",
            "svg",
            "--deps",
            deps.to_str().unwrap(),
            "--ignore-system-fonts",
        ])
        .output()
        .unwrap();
    assert!(
        svg.status.success(),
        "{}",
        String::from_utf8_lossy(&svg.stderr)
    );
    assert!(expanded.is_file());
    let dependencies: serde_json::Value =
        serde_json::from_slice(&std::fs::read(deps).unwrap()).unwrap();
    assert_eq!(dependencies["outputs"], serde_json::json!([expanded]));
    assert!(!pack_path.with_extension("svg").exists());

    for (format, feature) in [("pdf", false), ("html", true)] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
        command.current_dir(directory.path()).args([
            "compile",
            pack_path.to_str().unwrap(),
            "--format",
            format,
            "--ignore-system-fonts",
        ]);
        if feature {
            command.args(["--features", "html"]);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{format}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(pack_path.with_extension(format).is_file(), "{format}");
    }
}

#[test]
fn singleton_page_artifact_keeps_total_placeholder_literal() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("only-{t}.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "5",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
    assert!(!directory.path().join("only-5.svg").exists());
}

#[test]
fn document_format_total_placeholder_is_literal() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("document-{t}.pdf");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "pdf",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
    assert!(!directory.path().join("document-1.pdf").exists());
}

#[test]
fn padded_page_placeholders_use_total_source_page_width() {
    let directory = tempfile::tempdir().unwrap();
    let source = (1..=12)
        .map(|page| {
            format!(
                "#set page(width: {page}pt, height: 10pt, margin: 0pt)\n\
                 #rect(width: 1pt, height: 1pt)\n"
            ) + if page < 12 { "#pagebreak()\n" } else { "" }
        })
        .collect::<String>();
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap()
        .build()
        .unwrap();
    let pack_path = directory.path().join("padded.typk");
    std::fs::write(&pack_path, pack.to_bytes().unwrap()).unwrap();
    let output = directory.path().join("page-{0p}-{n}.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "2,11",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(directory.path().join("page-02-02.svg").is_file());
    assert!(directory.path().join("page-11-11.svg").is_file());
    assert!(!directory.path().join("page-01.svg").exists());
}
