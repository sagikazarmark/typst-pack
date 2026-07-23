#![cfg(feature = "cli")]

#[path = "support/official_typst_cli.rs"]
mod official_typst_cli;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use official_typst_cli::OfficialTypstCli;
use typst_pack::{OutputFormat, Pack, PackCompilationRequest, compile};

const PAGED_SOURCE: &str =
    "#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 4pt, height: 4pt)";
const HTML_SOURCE: &str = "#html.div[CLI parity]";
const TIMESTAMP: &str = "946684800";
const TEST_BINARY_ENV: &str = "TYPST_PACK_TEST_BINARY";

fn write_project(root: &Path, source: &str) -> PathBuf {
    let project = root.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), source).unwrap();
    project
}

fn write_pack(path: &Path, source: &str) -> Pack {
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
        .unwrap()
        .build()
        .unwrap();
    std::fs::write(path, pack.to_bytes().unwrap()).unwrap();
    pack
}

fn pack_command(current_dir: &Path, arguments: &[&str]) -> Output {
    pack_run(
        current_dir,
        std::iter::once("compile").chain(arguments.iter().copied()),
        &[],
    )
}

fn pack_run<I, S>(current_dir: &Path, arguments: I, environment: &[(&str, &str)]) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let executable = std::env::var_os(TEST_BINARY_ENV)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_typst-pack").into());
    let mut command = Command::new(executable);
    for variable in [
        "SOURCE_DATE_EPOCH",
        "TYPST_CERT",
        "TYPST_FEATURES",
        "TYPST_FONT_PATHS",
        "TYPST_IGNORE_EMBEDDED_FONTS",
        "TYPST_IGNORE_SYSTEM_FONTS",
        "TYPST_PACKAGE_CACHE_PATH",
        "TYPST_PACKAGE_PATH",
        "TYPST_ROOT",
    ] {
        command.env_remove(variable);
    }
    command
        .current_dir(current_dir)
        .args(arguments)
        .envs(environment.iter().copied())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn read_eventually(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(value) = std::fs::read_to_string(path) {
            return value;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn official_typst_compile_is_the_process_level_parity_baseline() {
    let Some(official) = OfficialTypstCli::from_environment() else {
        eprintln!("skipping official CLI parity outside its pinned Dagger gate");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let project = write_project(directory.path(), PAGED_SOURCE);
    let pack_path = directory.path().join("project.typk");
    let pack = write_pack(&pack_path, PAGED_SOURCE);

    let public_result = compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg))
        .expect("public Pack compilation must establish the embedded Engine baseline");
    official.require_version(public_result.engine_identity().version());
    let pack_version = pack_run(directory.path(), ["--version"], &[]);
    assert_success(&pack_version);
    assert!(
        String::from_utf8_lossy(&pack_version.stdout).contains(&format!(
            "(Typst {})",
            public_result.engine_identity().version()
        )),
        "{TEST_BINARY_ENV} must report the exact embedded Engine baseline"
    );

    for extension in ["pdf", "png", "svg"] {
        let official_output = directory.path().join(format!("official.{extension}"));
        let pack_output = directory.path().join(format!("pack.{extension}"));
        let official_result = official.compile(
            &project,
            [
                "main.typ",
                official_output.to_str().unwrap(),
                "--ignore-system-fonts",
                "--creation-timestamp",
                TIMESTAMP,
            ],
        );
        let pack_result = pack_command(
            directory.path(),
            &[
                pack_path.to_str().unwrap(),
                pack_output.to_str().unwrap(),
                "--ignore-system-fonts",
                "--creation-timestamp",
                TIMESTAMP,
            ],
        );
        assert_success(&official_result);
        assert_success(&pack_result);
        assert_eq!(
            std::fs::read(pack_output).unwrap(),
            std::fs::read(official_output).unwrap(),
            "{extension} artifact differs from official Typst"
        );
    }

    let html_project = directory.path().join("html-project");
    std::fs::create_dir(&html_project).unwrap();
    std::fs::write(html_project.join("main.typ"), HTML_SOURCE).unwrap();
    let html_pack_path = directory.path().join("html.typk");
    write_pack(&html_pack_path, HTML_SOURCE);
    let official_html = directory.path().join("official.html");
    let pack_html = directory.path().join("pack.html");
    let official_result = official.compile(
        &html_project,
        [
            "main.typ",
            official_html.to_str().unwrap(),
            "--features",
            "html",
            "--ignore-system-fonts",
        ],
    );
    let pack_result = pack_command(
        directory.path(),
        &[
            html_pack_path.to_str().unwrap(),
            pack_html.to_str().unwrap(),
            "--features",
            "html",
            "--ignore-system-fonts",
        ],
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    assert_eq!(
        std::fs::read(pack_html).unwrap(),
        std::fs::read(official_html).unwrap(),
        "HTML artifact differs from official Typst"
    );

    let public_svg = compile(PackCompilationRequest::new(pack, OutputFormat::Svg)).unwrap();
    assert_eq!(
        public_svg.artifacts()[0].bytes(),
        std::fs::read(directory.path().join("official.svg")).unwrap(),
        "CLI parity fixture must also traverse the proven public Pack compilation seam"
    );
}

#[test]
fn official_typst_compile_gates_shared_environment_diagnostics_and_exit_behavior() {
    let Some(official) = OfficialTypstCli::from_environment() else {
        eprintln!("skipping official CLI parity outside its pinned Dagger gate");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let source = "#warning(\"parity warning\")\n#unknown-name";
    let project = write_project(directory.path(), source);
    let pack_path = directory.path().join("rejected.typk");
    write_pack(&pack_path, source);
    let official_output = directory.path().join("official.pdf");
    let pack_output = directory.path().join("pack.pdf");

    let official_result = official.compile(
        &project,
        [
            "main.typ",
            official_output.to_str().unwrap(),
            "--diagnostic-format",
            "short",
            "--ignore-system-fonts",
        ],
    );
    let pack_result = pack_command(
        directory.path(),
        &[
            pack_path.to_str().unwrap(),
            pack_output.to_str().unwrap(),
            "--diagnostic-format",
            "short",
            "--ignore-system-fonts",
        ],
    );

    assert_eq!(pack_result.status.code(), official_result.status.code());
    assert_eq!(pack_result.stdout, official_result.stdout);
    assert_eq!(pack_result.stderr, official_result.stderr);
    assert!(!official_output.exists());
    assert!(!pack_output.exists());

    let official_color = official.run(
        &project,
        [
            "--color",
            "always",
            "compile",
            "main.typ",
            official_output.to_str().unwrap(),
            "--diagnostic-format",
            "human",
            "--ignore-system-fonts",
        ],
        &[],
    );
    let pack_color = pack_run(
        directory.path(),
        [
            "--color",
            "always",
            "compile",
            pack_path.to_str().unwrap(),
            pack_output.to_str().unwrap(),
            "--diagnostic-format",
            "human",
            "--ignore-system-fonts",
        ],
        &[],
    );
    assert_eq!(pack_color.status.code(), official_color.status.code());
    assert_eq!(pack_color.stderr, official_color.stderr);
    assert!(
        official_color
            .stderr
            .windows(2)
            .any(|bytes| bytes == b"\x1b[")
    );

    let environment_source = "#set page(width: int(sys.inputs.width) * 1pt, height: datetime.today().day() * 1pt, margin: 0pt)";
    std::fs::write(project.join("main.typ"), environment_source).unwrap();
    let environment_pack = directory.path().join("environment.typk");
    write_pack(&environment_pack, environment_source);
    let official_svg = directory.path().join("environment-official.svg");
    let pack_svg = directory.path().join("environment-pack.svg");
    let official_result = official.run(
        &project,
        [
            "compile",
            "main.typ",
            official_svg.to_str().unwrap(),
            "--input",
            "width=23",
            "--ignore-system-fonts",
        ],
        &[("SOURCE_DATE_EPOCH", TIMESTAMP)],
    );
    let pack_result = pack_run(
        directory.path(),
        [
            "compile",
            environment_pack.to_str().unwrap(),
            pack_svg.to_str().unwrap(),
            "--input",
            "width=23",
            "--ignore-system-fonts",
        ],
        &[("SOURCE_DATE_EPOCH", TIMESTAMP)],
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    assert_eq!(
        std::fs::read(pack_svg).unwrap(),
        std::fs::read(official_svg).unwrap()
    );
}

#[test]
fn official_typst_compile_gates_shared_spelling_parsing_and_conflicts() {
    let Some(official) = OfficialTypstCli::from_environment() else {
        eprintln!("skipping official CLI parity outside its pinned Dagger gate");
        return;
    };
    let directory = tempfile::tempdir().unwrap();

    let official_help = official.run(directory.path(), ["compile", "--help"], &[]);
    let pack_help = pack_run(directory.path(), ["compile", "--help"], &[]);
    assert_success(&official_help);
    assert_success(&pack_help);
    let official_help = String::from_utf8(official_help.stdout).unwrap();
    let pack_help = String::from_utf8(pack_help.stdout).unwrap();
    for spelling in [
        "--format <FORMAT>",
        "--input <key=value>",
        "--features <FEATURES>",
        "--pages <PAGES>",
        "--ppi <PPI>",
        "--pdf-standard <PDF_STANDARD>",
        "--no-pdf-tags",
        "--creation-timestamp <UNIX_TIMESTAMP>",
        "--diagnostic-format <DIAGNOSTIC_FORMAT>",
        "--deps <PATH>",
        "--deps-format <DEPS_FORMAT>",
        "--timings <OUTPUT_JSON>",
    ] {
        assert!(
            official_help.contains(spelling),
            "official help lost {spelling}"
        );
        assert!(
            pack_help.contains(spelling),
            "Pack help drifted from {spelling}"
        );
    }

    let official_range = official.compile(
        directory.path(),
        ["missing.typ", "out.pdf", "--pages", "3-1"],
    );
    let pack_range = pack_command(
        directory.path(),
        &["missing.typk", "out.pdf", "--pages", "3-1"],
    );
    assert_eq!(pack_range.status.code(), official_range.status.code());
    assert_eq!(pack_range.stderr, official_range.stderr);

    let official_conflict = official.run(
        directory.path(),
        [
            "--color",
            "never",
            "compile",
            "missing.typ",
            "out.pdf",
            "--pdf-standard",
            "a-2a",
            "--pages",
            "1",
            "--diagnostic-format",
            "short",
        ],
        &[],
    );
    let pack_conflict = pack_run(
        directory.path(),
        [
            "--color",
            "never",
            "compile",
            "missing.typk",
            "out.pdf",
            "--pdf-standard",
            "a-2a",
            "--pages",
            "1",
            "--diagnostic-format",
            "short",
        ],
        &[],
    );
    assert_eq!(pack_conflict.status.code(), official_conflict.status.code());
    assert_eq!(pack_conflict.stderr, official_conflict.stderr);
}

#[test]
fn official_typst_compile_gates_package_font_and_offline_request_routing() {
    let Some(official) = OfficialTypstCli::from_environment() else {
        eprintln!("skipping official CLI parity outside its pinned Dagger gate");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("dependencies");
    std::fs::create_dir(&project).unwrap();
    let font = typst_kit::fonts::embedded().next().unwrap().0;
    let font_directory = directory.path().join("fonts");
    std::fs::create_dir(&font_directory).unwrap();
    std::fs::write(font_directory.join("exact.ttf"), font.data()).unwrap();
    let family = font.info().family.to_string();
    std::fs::write(
        project.join("main.typ"),
        format!(
            "#import \"@local/oracle:1.0.0\": oracle-box\n\
         #set text(font: \"{family}\")\n\
         #set page(width: 40pt, height: 20pt, margin: 0pt)\n\
         #oracle-box(width: 8pt)\n\
         #text(\"Parity\")"
        ),
    )
    .unwrap();
    let packages =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/official-oracle/packages");
    let pack_path = directory.path().join("dependencies.typk");
    let created = pack_run(
        &project,
        [
            "create",
            "main.typ",
            pack_path.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
            "--font-path",
            font_directory.to_str().unwrap(),
        ],
        &[],
    );
    assert_success(&created);

    let official_pdf = directory.path().join("dependencies-official.pdf");
    let pack_pdf = directory.path().join("dependencies-pack.pdf");
    let font_environment = [
        ("TYPST_FONT_PATHS", font_directory.to_str().unwrap()),
        ("TYPST_IGNORE_EMBEDDED_FONTS", "true"),
    ];
    let official_result = official.run(
        &project,
        [
            "compile",
            "main.typ",
            official_pdf.to_str().unwrap(),
            "--package-path",
            packages.to_str().unwrap(),
            "--ignore-system-fonts",
            "--creation-timestamp",
            TIMESTAMP,
        ],
        &font_environment,
    );
    let pack_result = pack_run(
        directory.path(),
        [
            "compile",
            pack_path.to_str().unwrap(),
            pack_pdf.to_str().unwrap(),
            "--offline",
            "--ignore-system-fonts",
            "--creation-timestamp",
            TIMESTAMP,
        ],
        &font_environment,
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    assert_eq!(
        std::fs::read(pack_pdf).unwrap(),
        std::fs::read(official_pdf).unwrap(),
        "verified package and font fulfillment drifted from official Typst"
    );
}

#[cfg(unix)]
#[test]
fn official_typst_compile_gates_templates_automation_dependencies_and_viewing() {
    use std::os::unix::fs::PermissionsExt;

    let Some(official) = OfficialTypstCli::from_environment() else {
        eprintln!("skipping official CLI parity outside its pinned Dagger gate");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let source = "#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 2pt, height: 2pt)#pagebreak()#rect(width: 3pt, height: 3pt)";
    let project = write_project(directory.path(), source);
    let pack_path = directory.path().join("automation.typk");
    write_pack(&pack_path, source);

    let official_template = directory.path().join("official-{0p}.svg");
    let pack_template = directory.path().join("pack-{0p}.svg");
    let official_result = official.compile(
        &project,
        [
            "main.typ",
            official_template.to_str().unwrap(),
            "-j",
            "1",
            "--ignore-system-fonts",
        ],
    );
    let pack_result = pack_command(
        directory.path(),
        &[
            pack_path.to_str().unwrap(),
            pack_template.to_str().unwrap(),
            "-j",
            "1",
            "--ignore-system-fonts",
        ],
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    for page in ["1", "2"] {
        assert_eq!(
            std::fs::read(directory.path().join(format!("pack-{page}.svg"))).unwrap(),
            std::fs::read(directory.path().join(format!("official-{page}.svg"))).unwrap(),
        );
    }

    let official_pdf = directory.path().join("automation-official.pdf");
    let pack_pdf = directory.path().join("automation-pack.pdf");
    let official_timings = directory.path().join("official-timings.json");
    let pack_timings = directory.path().join("pack-timings.json");
    let official_deps = directory.path().join("official-deps.json");
    let pack_deps = directory.path().join("pack-deps.json");
    let official_result = official.compile(
        &project,
        [
            "main.typ",
            official_pdf.to_str().unwrap(),
            "--creation-timestamp",
            TIMESTAMP,
            "--timings",
            official_timings.to_str().unwrap(),
            "--deps",
            official_deps.to_str().unwrap(),
            "--deps-format",
            "json",
            "--ignore-system-fonts",
        ],
    );
    let pack_result = pack_command(
        directory.path(),
        &[
            pack_path.to_str().unwrap(),
            pack_pdf.to_str().unwrap(),
            "--creation-timestamp",
            TIMESTAMP,
            "--timings",
            pack_timings.to_str().unwrap(),
            "--deps",
            pack_deps.to_str().unwrap(),
            "--deps-format",
            "json",
            "--ignore-system-fonts",
        ],
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    assert_eq!(
        std::fs::read(pack_pdf).unwrap(),
        std::fs::read(official_pdf).unwrap()
    );
    for path in [&official_timings, &pack_timings, &official_deps, &pack_deps] {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(
            !value.as_array().is_some_and(Vec::is_empty),
            "{} was empty",
            path.display()
        );
    }
    assert!(
        String::from_utf8(std::fs::read(&official_deps).unwrap())
            .unwrap()
            .contains("main.typ")
    );
    assert!(
        String::from_utf8(std::fs::read(&pack_deps).unwrap())
            .unwrap()
            .contains("automation.typk")
    );

    let viewer = directory.path().join("viewer.sh");
    std::fs::write(&viewer, "#!/bin/sh\nprintf '%s' \"$1\" > \"$VIEWER_LOG\"\n").unwrap();
    std::fs::set_permissions(&viewer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let official_log = directory.path().join("official-viewer.log");
    let pack_log = directory.path().join("pack-viewer.log");
    let viewer_project = directory.path().join("viewer-project");
    std::fs::create_dir(&viewer_project).unwrap();
    std::fs::write(viewer_project.join("main.typ"), PAGED_SOURCE).unwrap();
    let viewer_pack = directory.path().join("viewer.typk");
    write_pack(&viewer_pack, PAGED_SOURCE);
    let official_result = official.run(
        &viewer_project,
        [
            "compile",
            "main.typ",
            "view-official.svg",
            "--open",
            viewer.to_str().unwrap(),
            "--ignore-system-fonts",
        ],
        &[("VIEWER_LOG", official_log.to_str().unwrap())],
    );
    let pack_result = pack_run(
        directory.path(),
        [
            "compile",
            viewer_pack.to_str().unwrap(),
            "view-pack.svg",
            "--open",
            viewer.to_str().unwrap(),
            "--ignore-system-fonts",
        ],
        &[("VIEWER_LOG", pack_log.to_str().unwrap())],
    );
    assert_success(&official_result);
    assert_success(&pack_result);
    assert!(read_eventually(&official_log).ends_with("view-official.svg"));
    assert!(read_eventually(&pack_log).ends_with("view-pack.svg"));
}
