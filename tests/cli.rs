#![cfg(feature = "cli")]

use std::process::Command;

use typst_pack::Pack;

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
    assert!(directory.path().join("page-2-2-of-2.svg").is_file());
    assert!(directory.path().join("page-5-5-of-2.svg").is_file());
    assert!(!directory.path().join("page-1-1-of-2.svg").exists());
}

#[test]
fn duplicate_expanded_paths_fail_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());
    let output = directory.path().join("collision-{t}.svg");
    let collided = directory.path().join("collision-2.svg");
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
        String::from_utf8_lossy(&result.stderr).contains("same output path"),
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
fn document_formats_reject_source_page_placeholders() {
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

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("Source Page Number"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
    assert!(stderr.contains("accessibility tags"), "{stderr}");
    assert!(!output.exists());
}

#[test]
fn no_matching_source_pages_reports_retained_compilation_warnings() {
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

    assert!(!result.status.success());
    assert!(stderr.contains("Definitely Missing"), "{stderr}");
    assert!(stderr.contains("matched no source pages"), "{stderr}");
}

#[test]
fn default_multi_page_names_use_source_page_numbers() {
    let directory = tempfile::tempdir().unwrap();
    let pack = write_five_page_pack(directory.path());

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack.to_str().unwrap(),
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
    assert!(directory.path().join("selection-2.svg").is_file());
    assert!(directory.path().join("selection-5.svg").is_file());
    assert!(!directory.path().join("selection-1.svg").exists());
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
fn document_format_total_placeholder_resolves_to_one() {
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
    assert!(directory.path().join("document-1.pdf").is_file());
}

#[test]
fn padded_page_placeholder_uses_artifact_count_width() {
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
    let output = directory.path().join("page-{0p}.svg");

    let result = Command::new(env!("CARGO_BIN_EXE_typst-pack"))
        .current_dir(directory.path())
        .args([
            "compile",
            pack_path.to_str().unwrap(),
            output.to_str().unwrap(),
            "--format",
            "svg",
            "--pages",
            "2-11",
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
    assert!(directory.path().join("page-02.svg").is_file());
    assert!(directory.path().join("page-11.svg").is_file());
    assert!(!directory.path().join("page-01.svg").exists());
}
