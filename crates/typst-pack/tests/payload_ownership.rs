use typst_pack::pack_archive::{DecodeLimits, decode};
use typst_pack::{
    CompilationOutputSpecification, CreationOutcome, CreationRequest, Pack, PackArchiveBytes,
    PackCompilationRequest, PackageCatalog, PackageDisposition, PackageTree,
    ProjectSnapshotAssembly, SvgOutputSpecification, compile, create,
};

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

#[test]
fn project_snapshot_moves_and_shares_payload_bytes() {
    let source = b"= Shared payload".to_vec();
    let source_pointer = source.as_ptr();
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble([("main.typ", source)])
        .unwrap();
    let files: Vec<(&str, &[u8])> = snapshot.files().collect();

    assert_eq!(files[0].1.as_ptr(), source_pointer);

    let cloned = snapshot.clone();
    assert_eq!(
        cloned.file("main.typ").unwrap().as_ptr(),
        snapshot.file("main.typ").unwrap().as_ptr()
    );
}

#[test]
fn pack_archive_bytes_keep_unique_vector_ownership() {
    let archive = b"exact archive retry material".to_vec();
    let archive_pointer = archive.as_ptr();
    let archive = PackArchiveBytes::from(archive);

    assert_eq!(archive.as_slice().as_ptr(), archive_pointer);

    let archive = archive.into_vec();
    assert_eq!(archive.as_ptr(), archive_pointer);
}

#[test]
fn failed_pack_archive_decode_preserves_retry_ownership() {
    let archive = PackArchiveBytes::from(b"not a Pack Archive".to_vec());
    let archive_pointer = archive.as_slice().as_ptr();

    assert!(decode(&archive, DecodeLimits::reference_v1()).is_err());
    assert_eq!(archive.as_slice().as_ptr(), archive_pointer);
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn font_values_and_pack_creation_share_container_payloads() {
    use typst_pack::{FontCatalog, FontCatalogEntry, FontContainer, FontDisposition};

    let font = fonts::typst_container();
    let family = fonts::family_of(&font);
    let font_pointer = font.as_ptr();
    let container = FontContainer::new(font).unwrap();
    assert_eq!(container.data().as_ptr(), font_pointer);
    assert_eq!(container.clone().data().as_ptr(), container.data().as_ptr());

    let mut catalog = FontCatalog::new();
    catalog.push(FontCatalogEntry::new(
        container.clone(),
        FontDisposition::Embedded,
    ));
    let source = format!("#set text(font: \"{family}\")\nShared").into_bytes();
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble([("main.typ", source)])
        .unwrap();
    let request = CreationRequest::new(snapshot, 1_700_000_000).font_catalog(catalog);
    let CreationOutcome::Issued(issued) = create(&request).unwrap() else {
        panic!("the supplied font should issue a Pack");
    };

    assert_eq!(issued.pack.fonts()[0].data().as_ptr(), font_pointer);
    assert_eq!(
        issued.pack.clone().fonts()[0].data().as_ptr(),
        issued.pack.fonts()[0].data().as_ptr()
    );
}

#[test]
fn compilation_artifact_clones_share_payload_bytes() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 10pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let report = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
    ))
    .unwrap();
    let artifact = &report.result().unwrap().artifacts()[0];
    let artifact_pointer = artifact.bytes().as_ptr();

    assert_eq!(artifact.clone().bytes().as_ptr(), artifact_pointer);
    assert_eq!(
        report.result().unwrap().clone().artifacts()[0]
            .bytes()
            .as_ptr(),
        artifact_pointer
    );
    assert_eq!(
        report.clone().result().unwrap().artifacts()[0]
            .bytes()
            .as_ptr(),
        artifact_pointer
    );
}

#[test]
fn pack_creation_reuses_project_and_package_payloads() {
    let project_source = b"#import \"@local/example:1.0.0\": value\n#value".to_vec();
    let project_pointer = project_source.as_ptr();
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble([("main.typ", project_source)])
        .unwrap();

    let package_source = b"#let value = 42".to_vec();
    let package_pointer = package_source.as_ptr();
    let package: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let tree = PackageTree::from_owned_entries([
        (
            "typst.toml",
            b"[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n"
                .to_vec(),
        ),
        ("lib.typ", package_source),
    ])
    .unwrap();
    let tree_files: Vec<(&str, &[u8])> = tree.files().collect();
    assert_eq!(
        tree_files
            .iter()
            .find(|(path, _)| *path == "lib.typ")
            .unwrap()
            .1
            .as_ptr(),
        package_pointer
    );
    drop(tree_files);

    let tree_clone = tree.clone();
    let catalog =
        PackageCatalog::from_entries([(package.clone(), tree, PackageDisposition::Embedded)])
            .unwrap();
    let request = CreationRequest::new(snapshot.clone(), 1_700_000_000).package_catalog(catalog);
    let CreationOutcome::Issued(issued) = create(&request).unwrap() else {
        panic!("the supplied package tree should issue a Pack");
    };

    assert_eq!(
        issued.pack.file("main.typ").unwrap().as_ptr(),
        project_pointer
    );
    assert_eq!(
        issued
            .pack
            .package_file(&package, "lib.typ")
            .unwrap()
            .as_ptr(),
        tree_clone
            .files()
            .find(|(path, _)| *path == "lib.typ")
            .unwrap()
            .1
            .as_ptr()
    );

    let pack_clone = issued.pack.clone();
    assert_eq!(
        pack_clone.file("main.typ").unwrap().as_ptr(),
        issued.pack.file("main.typ").unwrap().as_ptr()
    );
    assert_eq!(
        pack_clone
            .package_file(&package, "lib.typ")
            .unwrap()
            .as_ptr(),
        issued
            .pack
            .package_file(&package, "lib.typ")
            .unwrap()
            .as_ptr()
    );
    let package_files: Vec<(&str, &[u8])> = pack_clone
        .packages()
        .find(|(spec, _)| *spec == &package)
        .unwrap()
        .1
        .collect();
    assert!(
        package_files
            .iter()
            .any(|(path, data)| { *path == "lib.typ" && *data == b"#let value = 42" })
    );
}
