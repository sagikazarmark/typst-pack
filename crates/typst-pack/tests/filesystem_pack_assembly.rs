#![cfg(feature = "fs")]

use std::path::Path;

use typst_pack::{
    DocumentTime, FilesystemPackAssembler, FilesystemPackAssemblerConfig,
    FilesystemPackAssemblyClock, FilesystemPackAssemblyRequest,
};

#[test]
fn configured_assembler_is_reused_with_separate_run_requests() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::write(first.join("main.typ"), "first").unwrap();
    std::fs::write(second.join("main.typ"), "second").unwrap();

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .system_fonts(false)
            .typst_embedded_fonts(false),
    );
    let first_report = assembler
        .assemble(
            FilesystemPackAssemblyRequest::new(&first, Path::new("main.typ"))
                .document_time(DocumentTime::UnixTimestamp(1_704_067_200)),
        )
        .unwrap();
    let second_report = assembler
        .assemble(
            FilesystemPackAssemblyRequest::new(&second, Path::new("main.typ"))
                .document_time(DocumentTime::UnixTimestamp(1_704_067_200)),
        )
        .unwrap();

    assert_eq!(first_report.pack().file("main.typ"), Some(&b"first"[..]));
    assert_eq!(second_report.pack().file("main.typ"), Some(&b"second"[..]));
}

#[test]
fn configured_clock_supplies_the_default_discovery_document_time() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#if datetime.today().year() != 2024 { panic(\"wrong configured clock\") }",
    )
    .unwrap();

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .system_fonts(false)
            .typst_embedded_fonts(false)
            .clock(FilesystemPackAssemblyClock::Fixed(
                DocumentTime::UnixTimestamp(1_704_067_200),
            )),
    );

    assembler
        .assemble(FilesystemPackAssemblyRequest::new(
            &project,
            Path::new("main.typ"),
        ))
        .unwrap();
}
