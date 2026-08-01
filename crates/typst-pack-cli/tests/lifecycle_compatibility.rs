use std::path::Path;
use std::process::{Command, Output};

const MAIN_SOURCE: &str = "#let _ = read(\"data.txt\")\n#rect(width: 1pt, height: 1pt)";

fn command(current_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typst-pack"));
    command.current_dir(current_dir);
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
}

fn write_project(directory: &Path) {
    let project = directory.join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), MAIN_SOURCE).unwrap();
    std::fs::write(project.join("data.txt"), "payload").unwrap();
}

fn create_pack(directory: &Path) -> Output {
    command(directory)
        .args([
            "create",
            "project/main.typ",
            "project.typk",
            "--name",
            "Compatibility Fixture",
            "--description",
            "Ordinary CLI lifecycle",
            "--author",
            "Ada",
            "--author",
            "Linus",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap()
}

#[test]
fn ordinary_file_lifecycle_remains_compatible_through_the_cli_process() {
    let directory = tempfile::tempdir().unwrap();
    write_project(directory.path());

    let created = create_pack(directory.path());
    assert_eq!(created.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(created.stdout).unwrap(),
        "packed 2 project file(s), 0 package(s), 0 font(s) into `project.typk`\n"
    );
    assert!(created.stderr.is_empty(), "{:?}", created.stderr);
    assert!(directory.path().join("project.typk").is_file());

    let inspected = command(directory.path())
        .args(["inspect", "project.typk"])
        .output()
        .unwrap();
    assert_eq!(inspected.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(inspected.stdout).unwrap(),
        format!(
            concat!(
                "pack: project.typk\n",
                "format version: 1\n",
                "entrypoint: main.typ\n",
                "name: Compatibility Fixture\n",
                "description: Ordinary CLI lifecycle\n",
                "authors: Ada, Linus\n",
                "\n",
                "packed project files:\n",
                "  data.txt (7 B)\n",
                "  main.typ ({} B)\n",
            ),
            MAIN_SOURCE.len()
        )
    );
    assert!(inspected.stderr.is_empty(), "{:?}", inspected.stderr);

    let extracted = command(directory.path())
        .args(["extract", "project.typk", "--output", "extracted"])
        .output()
        .unwrap();
    assert_eq!(extracted.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(extracted.stdout).unwrap(),
        "extracted 2 file(s) into `extracted`\n"
    );
    assert!(extracted.stderr.is_empty(), "{:?}", extracted.stderr);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("extracted/main.typ")).unwrap(),
        MAIN_SOURCE
    );
    assert_eq!(
        std::fs::read(directory.path().join("extracted/data.txt")).unwrap(),
        b"payload"
    );

    let compiled = command(directory.path())
        .args([
            "compile",
            "project.typk",
            "output.svg",
            "--pages",
            "1",
            "--ignore-system-fonts",
            "--ignore-embedded-fonts",
        ])
        .output()
        .unwrap();
    assert_eq!(compiled.status.code(), Some(0));
    assert!(compiled.stdout.is_empty(), "{:?}", compiled.stdout);
    assert!(compiled.stderr.is_empty(), "{:?}", compiled.stderr);
    assert!(
        std::fs::read(directory.path().join("output.svg"))
            .unwrap()
            .starts_with(b"<svg")
    );
}

#[test]
fn extract_preflights_collisions_and_force_replaces_only_planned_files() {
    let directory = tempfile::tempdir().unwrap();
    write_project(directory.path());
    let created = create_pack(directory.path());
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let destination = directory.path().join("extracted");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), "keep existing").unwrap();
    std::fs::write(destination.join("unrelated.txt"), "keep unrelated").unwrap();

    let collided = command(directory.path())
        .args(["extract", "project.typk", "--output", "extracted"])
        .output()
        .unwrap();
    assert_eq!(collided.status.code(), Some(1));
    assert!(collided.stdout.is_empty(), "{:?}", collided.stdout);
    assert_eq!(
        String::from_utf8(collided.stderr).unwrap(),
        "error: `extracted/main.typ` already exists (pass force to overwrite)\n"
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("main.typ")).unwrap(),
        "keep existing"
    );
    assert!(!destination.join("data.txt").exists());

    let forced = command(directory.path())
        .args([
            "extract",
            "project.typk",
            "--output",
            "extracted",
            "--force",
        ])
        .output()
        .unwrap();
    assert_eq!(forced.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(forced.stdout).unwrap(),
        "extracted 2 file(s) into `extracted`\n"
    );
    assert!(forced.stderr.is_empty(), "{:?}", forced.stderr);
    assert_eq!(
        std::fs::read_to_string(destination.join("main.typ")).unwrap(),
        MAIN_SOURCE
    );
    assert_eq!(
        std::fs::read_to_string(destination.join("unrelated.txt")).unwrap(),
        "keep unrelated"
    );
}

#[test]
fn inspect_and_extract_help_remain_successful_process_outputs() {
    let directory = tempfile::tempdir().unwrap();

    for (subcommand, usage, expected) in [
        (
            "inspect",
            "Usage: typst-pack inspect <PACK>",
            ["Shows what is inside a pack", "The pack file to inspect"].as_slice(),
        ),
        (
            "extract",
            "Usage: typst-pack extract [OPTIONS] <PACK>",
            [
                "Extracts a pack into a directory",
                "--output <OUTPUT>",
                "--packages",
                "--fonts",
                "--all",
                "--force",
            ]
            .as_slice(),
        ),
    ] {
        let result = command(directory.path())
            .args([subcommand, "--help"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0), "{subcommand}");
        assert!(
            result.stderr.is_empty(),
            "{subcommand}: {:?}",
            result.stderr
        );
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert!(stdout.contains(usage), "{subcommand}:\n{stdout}");
        for text in expected {
            assert!(
                stdout.contains(text),
                "{subcommand}: missing {text}\n{stdout}"
            );
        }
    }
}
