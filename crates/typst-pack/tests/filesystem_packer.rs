//! Reusing a filesystem packer, and resolving a pack's external requirements
//! from the filesystem.

#![cfg(feature = "fs")]

use std::path::Path;

use typst_pack::{
    CompileError, CompileRequest, FilesystemPacker, Output, OutputFormat, Unfulfilled, compile,
};

fn write_package(packages: &Path, name: &str) {
    let package = packages.join(format!("local/{name}/0.1.0"));
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("typst.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n"),
    )
    .unwrap();
    std::fs::write(package.join("lib.typ"), "#let mark = rect(width: 1pt)").unwrap();
}

#[test]
fn one_packer_packs_several_projects() {
    let directory = tempfile::tempdir().unwrap();
    let packer = FilesystemPacker::new()
        .system_fonts(false)
        .typst_embedded_fonts(false);
    for name in ["first", "second"] {
        let project = directory.path().join(name);
        std::fs::create_dir(&project).unwrap();
        std::fs::write(project.join("main.typ"), name).unwrap();
        let packed = packer.pack(&project, "main.typ").unwrap();
        assert_eq!(packed.pack.file("main.typ"), Some(name.as_bytes()));
    }
}

#[test]
fn the_default_document_time_is_the_host_clock() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("main.typ"),
        "#assert(datetime.today().year() >= 2024)",
    )
    .unwrap();
    FilesystemPacker::new()
        .system_fonts(false)
        .pack(directory.path(), "main.typ")
        .unwrap();
}

#[test]
fn external_packages_resolve_from_the_package_path() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#import \"@local/kept:0.1.0\": mark\n#mark",
    )
    .unwrap();
    let packages = directory.path().join("packages");
    write_package(&packages, "kept");
    let packer = FilesystemPacker::new()
        .system_fonts(false)
        .package_path(&packages)
        .vendor_packages(false);
    let pack = packer.pack(&project, "main.typ").unwrap().pack;
    assert!(!pack.package_requirements()[0].embedded);

    let resolved = packer.resolve(&pack).unwrap();
    assert_eq!(resolved.packages.len(), 1);
    assert_eq!(
        resolved.package_dirs["@local/kept:0.1.0"],
        packages.join("local/kept/0.1.0")
    );
    let mut request = CompileRequest::new(pack.clone(), Output::new(OutputFormat::Svg));
    request.packages = resolved.packages;
    request.fonts = resolved.fonts;
    compile(&request).unwrap();

    // A package found nowhere is left for compilation to report.
    std::fs::remove_dir_all(&packages).unwrap();
    let resolved = packer.resolve(&pack).unwrap();
    assert!(resolved.packages.is_empty());
    request.packages = resolved.packages;
    assert!(matches!(
        compile(&request),
        Err(CompileError::Unfulfilled(issues))
            if matches!(issues.as_slice(), [Unfulfilled::MissingPackage { .. }])
    ));
}
