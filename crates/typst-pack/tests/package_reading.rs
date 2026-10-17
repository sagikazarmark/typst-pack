//! The transport-free package reading helpers.
//!
//! Every test here drives the public library surface of a build that has the
//! `package-reading` feature and no HTTP client: a caller obtains the
//! registry URL for a reported package specification, fetches it with whatever
//! primitive its host provides, and expands the resulting archive bytes into a
//! Package Tree the core accepts as a resolved tree.

#![cfg(feature = "package-reading")]

use std::cell::Cell;
use std::io::{self, Read};
use std::rc::Rc;
use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    DiscoverySpecification, DocumentTime, FontCatalog, LimitExceeded, PackCreationInput,
    PackCreationOutcome, PackageArchiveReadError, PackageCatalog, PackageDisposition,
    PackageReadError, PackageReadFailures, PackageTree, PackageTreeIssue, ProjectSnapshotAssembly,
    ReadLimits, TypstTarget, create, expand_package_archive, package_archive_url,
    read_package_archive,
};

fn spec(text: &str) -> PackageSpec {
    PackageSpec::from_str(text).unwrap()
}

/// 2023-11-14T22:13:20Z, the Document Time the representative request here is
/// fixed to.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

fn discovery() -> DiscoverySpecification {
    DiscoverySpecification::new(
        TypstTarget::Paged,
        typst::foundations::Dict::new(),
        DocumentTime::UnixTimestamp(CREATION_TIMESTAMP),
        [],
    )
    .unwrap()
}

/// The declaration a tree for `@preview/example:1.0.0` carries.
const DECLARATION: &[u8] =
    b"[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n";

/// The archive a registry serves for one package: its Package Tree,
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

fn finish(builder: tar::Builder<flate2::write::GzEncoder<Vec<u8>>>) -> Vec<u8> {
    builder.into_inner().unwrap().finish().unwrap()
}

fn archive_builder() -> tar::Builder<flate2::write::GzEncoder<Vec<u8>>> {
    tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ))
}

const LIMITS: ReadLimits = ReadLimits::DEFAULT;

fn expand(bytes: &[u8]) -> Result<PackageTree, PackageReadError> {
    expand_package_archive(spec("@preview/example:1.0.0"), bytes, LIMITS)
}

fn expand_under(bytes: &[u8], limits: ReadLimits) -> Result<PackageTree, PackageReadError> {
    expand_package_archive(spec("@preview/example:1.0.0"), bytes, limits)
}

/// An archive with one member whose raw header the test shapes.
fn raw_member(shape: impl FnOnce(&mut tar::Header), data: &[u8]) -> Vec<u8> {
    let mut builder = archive_builder();
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    shape(&mut header);
    header.set_cksum();
    builder.append(&header, data).unwrap();
    finish(builder)
}

fn limit_resource(error: PackageReadError) -> &'static str {
    match error {
        PackageReadError::Limit {
            source: LimitExceeded { resource, .. },
            ..
        } => resource,
        other => panic!("expected a limit, got {other}"),
    }
}

struct ObservedReader {
    bytes: io::Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
}

impl Read for ObservedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.bytes.read(buffer)
    }
}

#[test]
fn package_archive_reads_are_bounded() {
    assert_eq!(
        read_package_archive(io::Cursor::new(b"1234"), None, 4).unwrap(),
        b"1234"
    );
    for known_size in [None, Some(4)] {
        assert!(matches!(
            read_package_archive(io::Cursor::new(b"12345-extra"), known_size, 4),
            Err(PackageArchiveReadError::Limit(LimitExceeded {
                limit: 4,
                ..
            }))
        ));
    }

    // A known oversized archive is refused before the reader is touched.
    let reads = Rc::new(Cell::new(0));
    let reader = ObservedReader {
        bytes: io::Cursor::new(vec![0; 5]),
        reads: Rc::clone(&reads),
    };
    assert!(read_package_archive(reader, Some(5), 4).is_err());
    assert_eq!(reads.get(), 0);
}

#[test]
fn the_registry_url_of_a_specification_is_obtained_without_a_transport() {
    let layouts = [
        (
            "@preview/example:1.2.3",
            "https://packages.typst.org/preview/example-1.2.3.tar.gz",
        ),
        (
            "@preview/example:0.1.0",
            "https://packages.typst.org/preview/example-0.1.0.tar.gz",
        ),
        (
            "@preview/nested-name:10.0.1",
            "https://packages.typst.org/preview/nested-name-10.0.1.tar.gz",
        ),
    ];

    for (text, expected) in layouts {
        let url = package_archive_url(&spec(text)).unwrap();

        assert_eq!(url, expected, "{text}");
    }
}

#[test]
fn a_specification_the_registry_does_not_serve_has_no_url() {
    for text in [
        "@local/example:1.2.3",
        "@custom/example:1.2.3",
        "@Preview/example:1.2.3",
    ] {
        let unserved = spec(text);

        let error = package_archive_url(&unserved).unwrap_err();

        assert!(
            matches!(&error, PackageReadError::UnservedNamespace { spec } if spec == &unserved),
            "{text}: {error}"
        );
    }
}

#[test]
fn archive_bytes_expand_into_the_complete_package_tree_of_a_specification() {
    let bytes = archive(&[
        ("typst.toml", DECLARATION),
        ("./lib.typ", b"#let value = 1"),
        ("assets/logo.svg", b"<svg/>"),
    ]);

    // The whole tree travels, in canonical package-relative path order.
    assert_eq!(
        expand(&bytes)
            .unwrap()
            .files()
            .collect::<Vec<(&str, &[u8])>>(),
        [
            ("assets/logo.svg", &b"<svg/>"[..]),
            ("lib.typ", &b"#let value = 1"[..]),
            ("typst.toml", DECLARATION),
        ]
    );
}

#[test]
fn long_names_address_package_files() {
    let long = format!("{}/lib.typ", "nested".repeat(30));
    let mut builder = archive_builder();
    let mut gnu = tar::Header::new_gnu();
    gnu.set_size(1);
    builder.append_data(&mut gnu, &long, &b"g"[..]).unwrap();
    let mut pax = archive_builder();
    pax.append_pax_extensions([("path", long.as_bytes())])
        .unwrap();
    let mut header = tar::Header::new_ustar();
    header.set_size(1);
    pax.append_data(&mut header, "short", &b"p"[..]).unwrap();

    assert_eq!(
        expand(&finish(builder)).unwrap().file(&long),
        Some(&b"g"[..])
    );
    assert_eq!(expand(&finish(pax)).unwrap().file(&long), Some(&b"p"[..]));
}

#[test]
fn nonzero_padding_between_members_is_accepted() {
    let mut bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut bytes);
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        builder
            .append_data(&mut header, "lib.typ", &b"x"[..])
            .unwrap();
        builder.finish().unwrap();
    }
    // The member's data block is padded to 512 bytes after its one byte.
    bytes[513] = 0xff;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    io::Write::write_all(&mut encoder, &bytes).unwrap();

    let tree = expand(&encoder.finish().unwrap()).unwrap();
    assert_eq!(tree.file("lib.typ"), Some(&b"x"[..]));
}

#[test]
fn duplicate_and_conflicting_entries_are_rejected_by_package_tree_validation() {
    for files in [
        &[("lib.typ", &b"first"[..]), ("./lib.typ", b"second")][..],
        &[("lib", &b"file"[..]), ("lib/nested.typ", b"descendant")][..],
    ] {
        let error = expand(&archive(files)).unwrap_err();
        assert!(
            matches!(error, PackageReadError::InvalidPackageTree { .. }),
            "{error}"
        );
    }
}

#[test]
fn an_archive_entry_that_cannot_name_a_package_file_is_rejected() {
    for escaping in [&b"../escape.typ"[..], b"/absolute.typ"] {
        let bytes = raw_member(
            |header| header.as_gnu_mut().unwrap().name[..escaping.len()].copy_from_slice(escaping),
            b"#let value = 1",
        );
        let error = expand(&bytes).unwrap_err();
        let PackageReadError::InvalidPackageTree { source, .. } = &error else {
            panic!("{error}");
        };
        assert!(
            source
                .issues()
                .iter()
                .any(|issue| matches!(issue, PackageTreeIssue::InvalidPath { .. }))
        );
    }
}

#[test]
fn links_and_other_member_kinds_are_rejected() {
    for kind in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Fifo,
        tar::EntryType::Char,
    ] {
        let mut builder = archive_builder();
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(kind);
        header.set_size(0);
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name("/etc/passwd").unwrap();
        }
        builder
            .append_data(&mut header, "secrets", &[][..])
            .unwrap();
        let error = expand(&finish(builder)).unwrap_err();
        assert!(
            matches!(&error, PackageReadError::MalformedArchive { message, .. } if message.contains("secrets")),
            "{kind:?}: {error}"
        );
    }
}

#[test]
fn directories_are_not_package_files() {
    let mut builder = archive_builder();
    let mut directory = tar::Header::new_gnu();
    directory.set_entry_type(tar::EntryType::Directory);
    directory.set_size(0);
    builder
        .append_data(&mut directory, "assets", &[][..])
        .unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_size(DECLARATION.len() as u64);
    builder
        .append_data(&mut file, "typst.toml", DECLARATION)
        .unwrap();

    let tree = expand(&finish(builder)).unwrap();
    assert_eq!(
        tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["typst.toml"]
    );
}

#[test]
fn malformed_bytes_and_names_are_rejected() {
    assert!(matches!(
        expand(b"<!doctype html><title>404</title>"),
        Err(PackageReadError::MalformedArchive { .. })
    ));
    let non_utf8 = raw_member(|header| header.as_gnu_mut().unwrap().name[0] = 0xff, b"x");
    assert!(matches!(
        expand(&non_utf8),
        Err(PackageReadError::MalformedArchive { .. })
    ));
}

#[test]
fn expansion_is_bounded() {
    let bytes = archive(&[("a.typ", b"12345"), ("b.typ", b"12345"), ("c.typ", b"1")]);
    assert!(
        expand_under(
            &bytes,
            ReadLimits {
                max_entries: 3,
                max_file_bytes: 5,
                max_total_bytes: 11,
            }
        )
        .is_ok()
    );
    let limited = |limits| limit_resource(expand_under(&bytes, limits).unwrap_err());
    limited(ReadLimits {
        max_entries: 2,
        ..LIMITS
    });
    limited(ReadLimits {
        max_file_bytes: 4,
        ..LIMITS
    });
    limited(ReadLimits {
        max_total_bytes: 10,
        ..LIMITS
    });
}

#[test]
fn a_compression_bomb_stops_at_the_limit() {
    let limits = ReadLimits {
        max_entries: 10,
        max_file_bytes: 1024,
        max_total_bytes: 4096,
    };
    // A member that inflates far past the limit.
    let bomb = archive(&[("bomb.typ", &vec![0; 64 << 20])]);
    assert!(bomb.len() < 1 << 20);
    limit_resource(expand_under(&bomb, limits).unwrap_err());

    // A long-name extension inflating far past the limit is cut off before
    // it is materialized.
    let name = "n".repeat(16 << 20);
    let mut builder = archive_builder();
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    builder.append_data(&mut header, &name, &b"x"[..]).unwrap();
    assert!(matches!(
        expand_under(&finish(builder), limits),
        Err(PackageReadError::MalformedArchive { .. })
    ));
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

    let mut resolved: Vec<(PackageSpec, PackageTree, PackageDisposition)> = Vec::new();
    let fonts = FontCatalog::new();
    let package_failures = PackageReadFailures::new();
    let discovery = discovery();
    // Bounded so that a loop making no progress fails instead of hanging; the
    // number of rounds it actually takes is not asserted.
    let mut issued = None;
    for _ in 0..8 {
        let catalog = PackageCatalog::from_entries(resolved.iter().cloned()).unwrap();
        match create(PackCreationInput {
            project: &project,
            packages: &catalog,
            fonts: &fonts,
            package_failures: &package_failures,
            discovery: &discovery,
            metadata: None,
        })
        .unwrap()
        {
            PackCreationOutcome::Created { pack, .. } => {
                issued = Some(pack);
                break;
            }
            PackCreationOutcome::MissingPackageSpecifications(missing) => {
                for spec in missing {
                    let url = package_archive_url(&spec).unwrap();
                    let bytes = fetch(&url).expect("the registry serves the reported package");
                    let tree = expand_package_archive(spec.clone(), &bytes, LIMITS).unwrap();
                    resolved.push((spec, tree, PackageDisposition::Embedded));
                }
            }
        }
    }
    let pack = issued.expect("creation issued a Pack over the expanded trees");

    assert_eq!(
        pack.package_requirements()
            .iter()
            .map(|requirement| requirement.spec.to_string())
            .collect::<Vec<_>>(),
        ["@preview/example:1.0.0", "@preview/nested:2.0.0"]
    );
    // The whole expanded tree travels, not only what the representative request
    // read.
    assert!(
        pack.package_file(&spec("@preview/example:1.0.0"), "README.md")
            .is_some()
    );
}
