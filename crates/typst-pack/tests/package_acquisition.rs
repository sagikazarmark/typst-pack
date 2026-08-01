//! The transport-free package acquisition helpers.
//!
//! Every test here drives the public library surface of a build that has the
//! `package-acquisition` feature and no HTTP client: a caller obtains the
//! registry URL for a reported package specification, fetches it with whatever
//! primitive its host provides, and expands the resulting archive bytes into a
//! Complete Package Tree the core accepts as a resolved tree.

#![cfg(feature = "package-acquisition")]

use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CreationOutcome, CreationRequest, PackageAcquisitionError, PackageDisposition,
    PackageExpansionCeiling, ProjectSnapshotAssembly, ResolvedPackageTree, create,
    expand_package_archive, package_archive_url,
};

fn spec(text: &str) -> PackageSpec {
    PackageSpec::from_str(text).unwrap()
}

/// 2023-11-14T22:13:20Z, the Document Time the representative request here is
/// fixed to.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// The declaration a tree for `@preview/example:1.0.0` carries.
const DECLARATION: &[u8] =
    b"[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n";

/// The archive a registry serves for one package: its Complete Package Tree,
/// gzip-compressed tar with the package files at the archive root, exactly as
/// Typst Universe serves it.
fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (path, data) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        builder.append_data(&mut header, path, *data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// A ceiling no archive in this suite reaches, for the tests that are about
/// something else.
const GENEROUS_CEILING: PackageExpansionCeiling = PackageExpansionCeiling { max_bytes: 1 << 20 };

#[test]
fn the_registry_url_of_a_specification_is_obtained_without_a_transport() {
    let url = package_archive_url(&spec("@preview/example:1.2.3")).unwrap();

    assert_eq!(
        url,
        "https://packages.typst.org/preview/example-1.2.3.tar.gz"
    );
}

#[test]
fn a_specification_the_registry_does_not_serve_has_no_url() {
    let unserved = spec("@local/example:1.2.3");

    let error = package_archive_url(&unserved).unwrap_err();

    assert!(
        matches!(&error, PackageAcquisitionError::UnservedNamespace { spec } if spec == &unserved),
        "{error}"
    );
}

#[test]
fn archive_bytes_expand_into_the_complete_package_tree_of_a_specification() {
    let example = spec("@preview/example:1.0.0");
    let bytes = archive(&[
        ("typst.toml", DECLARATION),
        ("lib.typ", b"#let value = 1"),
        ("assets/logo.svg", b"<svg/>"),
    ]);

    let tree = expand_package_archive(
        example.clone(),
        &bytes,
        PackageDisposition::External,
        GENEROUS_CEILING,
    )
    .unwrap();

    assert_eq!(tree.spec(), &example);
    assert_eq!(tree.disposition(), PackageDisposition::External);
    // The whole tree travels, in canonical package-relative path order.
    assert_eq!(
        tree.files().collect::<Vec<(&str, &[u8])>>(),
        [
            ("assets/logo.svg", &b"<svg/>"[..]),
            ("lib.typ", &b"#let value = 1"[..]),
            ("typst.toml", DECLARATION),
        ]
    );
}

#[test]
fn a_tree_expanding_to_exactly_the_ceiling_is_accepted() {
    let bytes = archive(&[("typst.toml", DECLARATION), ("lib.typ", b"#let value = 1")]);
    let ceiling = PackageExpansionCeiling {
        max_bytes: (DECLARATION.len() + b"#let value = 1".len()) as u64,
    };

    let tree = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        PackageDisposition::Embedded,
        ceiling,
    )
    .unwrap();

    assert_eq!(tree.files().count(), 2);
}

#[test]
fn package_expansion_ceiling_does_not_contribute_to_pack_identity() {
    let example = spec("@preview/example:1.0.0");
    let bytes = archive(&[("typst.toml", DECLARATION), ("lib.typ", b"#let value = 1")]);
    let exact = PackageExpansionCeiling {
        max_bytes: (DECLARATION.len() + b"#let value = 1".len()) as u64,
    };
    let exact_tree =
        expand_package_archive(example.clone(), &bytes, PackageDisposition::Embedded, exact)
            .unwrap();
    let generous_tree = expand_package_archive(
        example,
        &bytes,
        PackageDisposition::Embedded,
        GENEROUS_CEILING,
    )
    .unwrap();
    let project = ProjectSnapshotAssembly::new("main.typ")
        .assemble([(
            "main.typ",
            b"#import \"@preview/example:1.0.0\": value\n#rect(width: value * 1pt, height: 1pt)"
                .to_vec(),
        )])
        .unwrap();
    let issue = |tree| match create(
        &CreationRequest::new(project.clone(), CREATION_TIMESTAMP).package_tree(tree),
    )
    .unwrap()
    {
        CreationOutcome::Issued(issued) => issued.pack,
        CreationOutcome::MissingPackages(missing) => {
            panic!("the supplied tree did not cover {missing:?}")
        }
    };

    assert_eq!(
        issue(exact_tree).identity(),
        issue(generous_tree).identity()
    );
}

#[test]
fn an_archive_expanding_past_the_ceiling_is_not_expanded_at_all() {
    // A hundred and twenty-eight megabytes of zeros in a few kilobytes of
    // archive, against a four-kilobyte ceiling. Expansion reads no further than
    // the ceiling allows, so this costs the ceiling rather than the archive's
    // claim; an implementation that materialized the content before measuring
    // it would allocate the whole nominal size here.
    let nominal = 128 * 1024 * 1024;
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(nominal);
    header.set_mode(0o644);
    builder
        .append_data(
            &mut header,
            "lib.typ",
            std::io::Read::take(std::io::repeat(0), nominal),
        )
        .unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let ceiling = PackageExpansionCeiling { max_bytes: 4096 };

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        PackageDisposition::Embedded,
        ceiling,
    )
    .unwrap_err();

    assert!(
        matches!(
            &error,
            PackageAcquisitionError::ExpansionCeilingExceeded { spec: reported, ceiling: bound }
                if reported == &spec("@preview/example:1.0.0") && bound == &ceiling
        ),
        "{error}"
    );
}

#[test]
fn a_member_that_becomes_no_package_file_is_charged_against_the_ceiling_too() {
    // A member the tree would not contain still has to be expanded to reach the
    // one after it, so an archive that claims to expand past the ceiling in a
    // directory entry is over it just as one that claims it in a package file.
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut directory = tar::Header::new_gnu();
    directory.set_entry_type(tar::EntryType::Directory);
    directory.set_size(64 * 1024 * 1024);
    directory.set_cksum();
    builder.append(&directory, std::io::empty()).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        PackageDisposition::Embedded,
        GENEROUS_CEILING,
    )
    .unwrap_err();

    assert!(
        matches!(
            &error,
            PackageAcquisitionError::ExpansionCeilingExceeded { .. }
        ),
        "{error}"
    );
}

#[test]
fn bytes_that_are_not_the_archive_a_registry_serves_are_rejected() {
    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        b"<!doctype html><title>404</title>",
        PackageDisposition::Embedded,
        GENEROUS_CEILING,
    )
    .unwrap_err();

    assert!(
        matches!(&error, PackageAcquisitionError::MalformedArchive { .. }),
        "{error}"
    );
}

#[test]
fn an_archive_entry_that_cannot_name_a_package_file_is_rejected() {
    // Written through the raw header, because a well-behaved archive writer
    // refuses to name an entry that escapes the package root at all.
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(b"#let value = 1".len() as u64);
    header.set_mode(0o644);
    let escaping = b"../escape.typ";
    header.as_gnu_mut().unwrap().name[..escaping.len()].copy_from_slice(escaping);
    header.set_cksum();
    builder.append(&header, &b"#let value = 1"[..]).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        PackageDisposition::Embedded,
        GENEROUS_CEILING,
    )
    .unwrap_err();

    assert!(
        matches!(&error, PackageAcquisitionError::InvalidPackagePath { path, .. }
            if path == "../escape.typ"),
        "{error}"
    );
}

#[test]
fn only_addressable_regular_files_become_package_files() {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut directory = tar::Header::new_gnu();
    directory.set_entry_type(tar::EntryType::Directory);
    directory.set_size(0);
    builder
        .append_data(&mut directory, "assets", &[][..])
        .unwrap();
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_link_name("/etc/passwd").unwrap();
    builder.append_data(&mut link, "secrets", &[][..]).unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_size(DECLARATION.len() as u64);
    builder
        .append_data(&mut file, "typst.toml", DECLARATION)
        .unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();

    let tree = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        PackageDisposition::Embedded,
        GENEROUS_CEILING,
    )
    .unwrap();

    assert_eq!(
        tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["typst.toml"]
    );
}

/// The archive a stand-in registry serves at one URL, or nothing when it serves
/// no package there. A real adapter fetches these with whatever primitive its
/// host provides, including an asynchronous one; expansion itself needs no
/// transport, so this stands in for the whole of it.
fn fetch(url: &str) -> Option<Vec<u8>> {
    let declaration = |name: &str, version: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nentrypoint = \"lib.typ\"\n")
            .into_bytes()
    };
    match url {
        "https://packages.typst.org/preview/example-1.0.0.tar.gz" => Some(archive(&[
            ("typst.toml", &declaration("example", "1.0.0")),
            (
                "lib.typ",
                b"#import \"@preview/nested:2.0.0\": inner\n#let value = 1 + inner",
            ),
            ("README.md", b"Not read by the representative request."),
        ])),
        "https://packages.typst.org/preview/nested-2.0.0.tar.gz" => Some(archive(&[
            ("typst.toml", &declaration("nested", "2.0.0")),
            ("lib.typ", b"#let inner = 2"),
        ])),
        _ => None,
    }
}

#[test]
fn a_resume_loop_fetches_and_expands_what_creation_reported() {
    let project = ProjectSnapshotAssembly::new("main.typ")
        .assemble([(
            "main.typ",
            b"#import \"@preview/example:1.0.0\": value\n\
              #rect(width: value * 1pt, height: 1pt)"
                .to_vec(),
        )])
        .unwrap();

    let mut resolved: Vec<ResolvedPackageTree> = Vec::new();
    // Bounded so that a loop making no progress fails instead of hanging; the
    // number of rounds it actually takes is not asserted.
    let mut issued = None;
    for _ in 0..8 {
        let request = CreationRequest::new(project.clone(), CREATION_TIMESTAMP)
            .package_trees(resolved.iter().cloned());
        match create(&request).unwrap() {
            CreationOutcome::Issued(pack) => {
                issued = Some(*pack);
                break;
            }
            CreationOutcome::MissingPackages(missing) => {
                for spec in missing {
                    let url = package_archive_url(&spec).unwrap();
                    let bytes = fetch(&url).expect("the registry serves the reported package");
                    resolved.push(
                        expand_package_archive(
                            spec,
                            &bytes,
                            PackageDisposition::Embedded,
                            GENEROUS_CEILING,
                        )
                        .unwrap(),
                    );
                }
            }
        }
    }
    let issued = issued.expect("creation issued a Pack over the expanded trees");

    assert_eq!(
        issued
            .pack
            .package_requirements()
            .iter()
            .map(|requirement| requirement.spec().to_string())
            .collect::<Vec<_>>(),
        ["@preview/example:1.0.0", "@preview/nested:2.0.0"]
    );
    // The whole expanded tree travels, not only what the representative request
    // read.
    assert!(
        issued
            .pack
            .package_file(&spec("@preview/example:1.0.0"), "README.md")
            .is_some()
    );
}
