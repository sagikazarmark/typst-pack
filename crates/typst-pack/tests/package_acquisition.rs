//! The transport-free package acquisition helpers.
//!
//! Every test here drives the public library surface of a build that has the
//! `package-acquisition` feature and no HTTP client: a caller obtains the
//! registry URL for a reported package specification, fetches it with whatever
//! primitive its host provides, and expands the resulting archive bytes into a
//! Package Tree the core accepts as a resolved tree.

#![cfg(feature = "package-acquisition")]

use std::cell::Cell;
use std::io::{self, Read};
use std::rc::Rc;
use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CreationOutcome, CreationRequest, PackageAcquisitionError, PackageArchiveAcquisitionError,
    PackageCatalog, PackageDisposition, PackageExpansionLimitError, PackageExpansionLimits,
    PackageExpansionLimitsError, PackageExpansionResource, PackageTree, PackageTreeIssue,
    ProjectSnapshotAssembly, acquire_package_archive, create, expand_package_archive,
    package_archive_url,
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

/// A profile no archive in this suite reaches, for the tests that are about
/// something else.
const GENEROUS_LIMITS: PackageExpansionLimits = PackageExpansionLimits::reference_v1();

fn limits(
    compressed_archive_bytes: u64,
    members: u64,
    member_name_bytes: u64,
    member_bytes: u64,
    total_expanded_bytes: u64,
) -> PackageExpansionLimits {
    PackageExpansionLimits::new(
        compressed_archive_bytes,
        members,
        member_name_bytes,
        member_bytes,
        total_expanded_bytes,
    )
    .unwrap()
}

fn limits_for(resource: PackageExpansionResource, ceiling: u64) -> PackageExpansionLimits {
    let mut values = [16 * 1024 * 1024, 100, 1024 * 1024, 1024 * 1024, 1024 * 1024];
    values[match resource {
        PackageExpansionResource::CompressedArchiveBytes => 0,
        PackageExpansionResource::Members => 1,
        PackageExpansionResource::MemberNameBytes => 2,
        PackageExpansionResource::MemberBytes => 3,
        PackageExpansionResource::TotalExpandedBytes => 4,
        _ => unreachable!("the test covers every current expansion resource"),
    }] = ceiling;
    limits(values[0], values[1], values[2], values[3], values[4])
}

#[test]
fn the_reference_v1_profile_bounds_every_package_expansion_resource() {
    let limits = PackageExpansionLimits::reference_v1();

    assert_eq!(limits.compressed_archive_bytes(), 128 * 1024 * 1024);
    assert_eq!(limits.members(), 50_000);
    assert_eq!(limits.member_name_bytes(), 8 * 1024 * 1024);
    assert_eq!(limits.member_bytes(), 64 * 1024 * 1024);
    assert_eq!(limits.total_expanded_bytes(), 512 * 1024 * 1024);
}

#[test]
fn every_package_expansion_ceiling_must_leave_room_for_a_plus_one_probe() {
    let ceilings = [
        PackageExpansionResource::CompressedArchiveBytes,
        PackageExpansionResource::Members,
        PackageExpansionResource::MemberNameBytes,
        PackageExpansionResource::MemberBytes,
        PackageExpansionResource::TotalExpandedBytes,
    ];

    for (index, resource) in ceilings.into_iter().enumerate() {
        let mut values = [1; 5];
        values[index] = u64::MAX;
        assert_eq!(
            PackageExpansionLimits::new(values[0], values[1], values[2], values[3], values[4]),
            Err(PackageExpansionLimitsError::CannotProbe {
                resource,
                ceiling: u64::MAX,
            })
        );
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
fn known_oversized_package_archive_is_rejected_before_reading() {
    let reads = Rc::new(Cell::new(0));
    let reader = ObservedReader {
        bytes: io::Cursor::new(vec![0; 5]),
        reads: Rc::clone(&reads),
    };

    let error = acquire_package_archive(reader, Some(5), limits(4, 10, 100, 100, 100)).unwrap_err();

    assert_eq!(reads.get(), 0);
    assert!(matches!(
        error,
        PackageArchiveAcquisitionError::Limit(PackageExpansionLimitError::Exceeded {
            resource: PackageExpansionResource::CompressedArchiveBytes,
            ceiling: 4,
            observed_at_least: 5,
        })
    ));
}

#[test]
fn unknown_package_archive_size_is_incrementally_metered_with_a_plus_one_probe() {
    let exact =
        acquire_package_archive(io::Cursor::new(b"1234"), None, limits(4, 10, 100, 100, 100))
            .unwrap();
    assert_eq!(exact, b"1234");

    let error = acquire_package_archive(
        io::Cursor::new(b"12345-extra"),
        None,
        limits(4, 10, 100, 100, 100),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PackageArchiveAcquisitionError::Limit(PackageExpansionLimitError::Exceeded {
            resource: PackageExpansionResource::CompressedArchiveBytes,
            ceiling: 4,
            observed_at_least: 5,
        })
    ));
}

#[test]
fn generated_boundaries_cover_every_package_expansion_resource() {
    let compressed = archive(&[("a", b"x")]);
    let cases = [
        (
            PackageExpansionResource::CompressedArchiveBytes,
            compressed,
            None,
        ),
        (
            PackageExpansionResource::Members,
            archive(&[("a", b"x"), ("b", b"y")]),
            Some(2),
        ),
        (
            PackageExpansionResource::MemberNameBytes,
            archive(&[("a", b"x"), ("bb", b"y")]),
            Some(3),
        ),
        (
            PackageExpansionResource::MemberBytes,
            archive(&[("a", b"xyz")]),
            Some(3),
        ),
        (
            PackageExpansionResource::TotalExpandedBytes,
            archive(&[("a", b"xx"), ("b", b"yyy")]),
            Some(5),
        ),
    ];

    for (resource, bytes, known_observed) in cases {
        let observed = known_observed.unwrap_or(bytes.len() as u64);
        for ceiling in [observed + 1, observed] {
            expand_package_archive(
                spec("@preview/example:1.0.0"),
                &bytes,
                limits_for(resource, ceiling),
            )
            .unwrap_or_else(|error| {
                panic!("{resource:?} rejected observed {observed} at ceiling {ceiling}: {error}")
            });
        }

        let ceiling = observed - 1;
        let error = expand_package_archive(
            spec("@preview/example:1.0.0"),
            &bytes,
            limits_for(resource, ceiling),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                PackageAcquisitionError::ExpansionLimit {
                    source: PackageExpansionLimitError::Exceeded {
                        resource: reported,
                        ceiling: reported_ceiling,
                        observed_at_least,
                    },
                    ..
                } if reported == resource
                    && reported_ceiling == ceiling
                    && observed_at_least == observed
            ),
            "unexpected {resource:?} boundary failure: {error}"
        );
    }
}

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

    let tree = expand_package_archive(example, &bytes, GENEROUS_LIMITS).unwrap();

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
fn archive_expansion_preserves_duplicate_entries_for_package_tree_rejection() {
    let bytes = archive(&[
        ("typst.toml", DECLARATION),
        ("lib.typ", b"first"),
        ("./lib.typ", b"second"),
    ]);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    let PackageAcquisitionError::InvalidPackageTree { source, .. } = &error else {
        panic!("{error}");
    };
    assert!(source.issues().iter().any(
        |issue| matches!(issue, PackageTreeIssue::DuplicatePath { path } if path == "lib.typ")
    ));
}

#[test]
fn archive_expansion_preserves_ancestor_conflicts_for_package_tree_rejection() {
    let bytes = archive(&[("assets", b"file"), ("assets/logo.svg", b"<svg/>")]);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    let PackageAcquisitionError::InvalidPackageTree { source, .. } = &error else {
        panic!("{error}");
    };
    assert!(source.issues().iter().any(|issue| matches!(
        issue,
        PackageTreeIssue::PathTreeConflict {
            ancestor,
            descendant,
        } if ancestor == "assets" && descendant == "assets/logo.svg"
    )));
}

#[test]
fn a_gnu_long_name_can_address_a_package_file() {
    let path = format!("assets/{}.typ", "a".repeat(120));
    let bytes = archive(&[(&path, b"content")]);

    let tree =
        expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS).unwrap();

    assert_eq!(tree.file(&path), Some(&b"content"[..]));
}

#[test]
fn a_local_pax_path_can_address_a_package_file() {
    let mut builder = archive_builder();
    builder
        .append_pax_extensions([("path", &b"actual.typ"[..])])
        .unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(7);
    header.set_mode(0o644);
    builder
        .append_data(&mut header, "placeholder", &b"content"[..])
        .unwrap();
    let bytes = finish(builder);

    let tree =
        expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS).unwrap();

    assert_eq!(tree.file("actual.typ"), Some(&b"content"[..]));
    assert_eq!(tree.file("placeholder"), None);
}

#[test]
fn a_pax_path_is_bounded_before_its_name_allocation() {
    let mut builder = archive_builder();
    let path = vec![b'a'; 2048];
    builder
        .append_pax_extensions([("path", path.as_slice())])
        .unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    builder.append_data(&mut header, "a", &b"x"[..]).unwrap();
    let bytes = finish(builder);

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        limits(1 << 20, 10, 100, 1 << 20, 1 << 20),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageAcquisitionError::ExpansionLimit {
            source: PackageExpansionLimitError::Exceeded {
                resource: PackageExpansionResource::MemberNameBytes,
                ..
            },
            ..
        }
    ));
}

#[test]
fn unrelated_pax_metadata_is_not_charged_as_member_name_bytes() {
    let mut builder = archive_builder();
    let comment = vec![b'x'; 256];
    builder
        .append_pax_extensions([("comment", comment.as_slice())])
        .unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    builder.append_data(&mut header, "a", &b"x"[..]).unwrap();
    let bytes = finish(builder);

    let tree = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        limits(1 << 20, 10, 1, 1 << 20, 1 << 20),
    )
    .unwrap();

    assert_eq!(tree.file("a"), Some(&b"x"[..]));
}

#[test]
fn an_oversized_pax_size_is_rejected_before_the_described_member_is_read() {
    let mut builder = archive_builder();
    builder
        .append_pax_extensions([("size", &b"1048576"[..])])
        .unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    builder.append_data(&mut header, "a", &b"x"[..]).unwrap();
    let bytes = finish(builder);

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        limits(1 << 20, 10, 1 << 20, 1024, 1 << 20),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageAcquisitionError::ExpansionLimit {
            source: PackageExpansionLimitError::Exceeded {
                resource: PackageExpansionResource::MemberBytes,
                ceiling: 1024,
                observed_at_least: 1048576,
            },
            ..
        }
    ));
}

#[test]
fn nonzero_bytes_past_a_member_declaration_are_rejected() {
    use std::io::{Read, Write};

    let bytes = archive(&[("a", b"x")]);
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut tar)
        .unwrap();
    tar[512 + 1] = b'y';
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&tar).unwrap();
    let bytes = encoder.finish().unwrap();

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    assert!(
        matches!(error, PackageAcquisitionError::MalformedArchive { .. }),
        "{error}"
    );
}

#[test]
fn competing_gnu_and_pax_names_are_rejected_as_ambiguous() {
    let mut builder = archive_builder();
    let mut long_name = tar::Header::new_gnu();
    long_name.set_entry_type(tar::EntryType::GNULongName);
    long_name.set_size(9);
    long_name.set_mode(0o644);
    long_name.set_cksum();
    builder.append(&long_name, &b"gnu.typ\0"[..]).unwrap();
    builder
        .append_pax_extensions([("path", &b"pax.typ"[..])])
        .unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_size(7);
    file.set_mode(0o644);
    builder
        .append_data(&mut file, "placeholder", &b"content"[..])
        .unwrap();
    let bytes = finish(builder);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    assert!(
        matches!(error, PackageAcquisitionError::MalformedArchive { .. }),
        "{error}"
    );
}

#[test]
fn long_name_payloads_are_charged_before_they_are_materialized_as_names() {
    let path = format!("{}.typ", "a".repeat(120));
    let bytes = archive(&[(&path, b"content")]);

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        limits(1 << 20, 10, 100, 1 << 20, 1 << 20),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageAcquisitionError::ExpansionLimit {
            source: PackageExpansionLimitError::Exceeded {
                resource: PackageExpansionResource::MemberNameBytes,
                ..
            },
            ..
        }
    ));
}

#[test]
fn a_tree_expanding_to_exactly_the_ceiling_is_accepted() {
    let bytes = archive(&[("typst.toml", DECLARATION), ("lib.typ", b"#let value = 1")]);
    let total = (DECLARATION.len() + b"#let value = 1".len()) as u64;
    let limits = limits(1 << 20, 10, 1 << 20, 1 << 20, total);

    let tree = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, limits).unwrap();

    assert_eq!(tree.files().count(), 2);
}

#[test]
fn package_expansion_ceiling_does_not_contribute_to_pack_identity() {
    let example = spec("@preview/example:1.0.0");
    let bytes = archive(&[("typst.toml", DECLARATION), ("lib.typ", b"#let value = 1")]);
    let total = (DECLARATION.len() + b"#let value = 1".len()) as u64;
    let exact = limits(1 << 20, 10, 1 << 20, 1 << 20, total);
    let exact_tree = expand_package_archive(example.clone(), &bytes, exact).unwrap();
    let generous_tree = expand_package_archive(example.clone(), &bytes, GENEROUS_LIMITS).unwrap();
    let project = ProjectSnapshotAssembly::new("main.typ")
        .assemble([(
            "main.typ",
            b"#import \"@preview/example:1.0.0\": value\n#rect(width: value * 1pt, height: 1pt)"
                .to_vec(),
        )])
        .unwrap();
    let issue = |tree| {
        let catalog =
            PackageCatalog::from_entries([(example.clone(), tree, PackageDisposition::Embedded)])
                .unwrap();
        match create(
            &CreationRequest::new(project.clone(), CREATION_TIMESTAMP).package_catalog(catalog),
        )
        .unwrap()
        {
            CreationOutcome::Issued(issued) => issued.pack,
            CreationOutcome::MissingPackages(missing) => {
                panic!("the supplied tree did not cover {missing:?}")
            }
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
    let limits = limits(2 << 20, 10, 1 << 20, 4096, 4096);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, limits).unwrap_err();

    assert!(
        matches!(
            &error,
            PackageAcquisitionError::ExpansionLimit {
                spec: reported,
                source: PackageExpansionLimitError::Exceeded {
                    resource: PackageExpansionResource::MemberBytes,
                    ceiling: 4096,
                    observed_at_least,
                },
            } if reported == &spec("@preview/example:1.0.0")
                && *observed_at_least == nominal
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
        limits(1 << 20, 10, 1 << 20, 128 * 1024 * 1024, 1 << 20),
    )
    .unwrap_err();

    assert!(
        matches!(
            &error,
            PackageAcquisitionError::ExpansionLimit {
                source: PackageExpansionLimitError::Exceeded {
                    resource: PackageExpansionResource::TotalExpandedBytes,
                    ..
                },
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn omitted_member_payloads_are_charged_cumulatively() {
    let mut builder = archive_builder();
    for path in ["first", "second"] {
        let mut directory = tar::Header::new_gnu();
        directory.set_entry_type(tar::EntryType::Directory);
        directory.set_size(3);
        directory.set_mode(0o755);
        builder
            .append_data(&mut directory, path, &b"abc"[..])
            .unwrap();
    }
    let bytes = finish(builder);

    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        &bytes,
        limits(1 << 20, 10, 1 << 20, 3, 5),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PackageAcquisitionError::ExpansionLimit {
            source: PackageExpansionLimitError::Exceeded {
                resource: PackageExpansionResource::TotalExpandedBytes,
                ceiling: 5,
                observed_at_least: 6,
            },
            ..
        }
    ));
}

#[test]
fn bytes_that_are_not_the_archive_a_registry_serves_are_rejected() {
    let error = expand_package_archive(
        spec("@preview/example:1.0.0"),
        b"<!doctype html><title>404</title>",
        GENEROUS_LIMITS,
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

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    let PackageAcquisitionError::InvalidPackageTree { source, .. } = &error else {
        panic!("{error}");
    };
    assert!(source.issues().iter().any(
        |issue| matches!(issue, PackageTreeIssue::InvalidPath { path, .. } if path == "../escape.typ")
    ));
}

#[test]
fn an_omitted_archive_entry_must_still_have_a_safe_package_path() {
    let mut builder = archive_builder();
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    let escaping = b"../escape";
    header.as_gnu_mut().unwrap().name[..escaping.len()].copy_from_slice(escaping);
    header.set_cksum();
    builder.append(&header, std::io::empty()).unwrap();
    let bytes = finish(builder);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    assert!(
        matches!(error, PackageAcquisitionError::MalformedArchive { .. }),
        "{error}"
    );
}

#[test]
fn a_non_utf8_archive_member_name_is_not_addressable() {
    let mut builder = archive_builder();
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    header.as_gnu_mut().unwrap().name[0] = 0xff;
    header.set_cksum();
    builder.append(&header, &b"x"[..]).unwrap();
    let bytes = finish(builder);

    let error = expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS)
        .unwrap_err();

    assert!(
        matches!(error, PackageAcquisitionError::MalformedArchive { .. }),
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

    let tree =
        expand_package_archive(spec("@preview/example:1.0.0"), &bytes, GENEROUS_LIMITS).unwrap();

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

    let mut resolved: Vec<(PackageSpec, PackageTree, PackageDisposition)> = Vec::new();
    // Bounded so that a loop making no progress fails instead of hanging; the
    // number of rounds it actually takes is not asserted.
    let mut issued = None;
    for _ in 0..8 {
        let catalog = PackageCatalog::from_entries(resolved.iter().cloned()).unwrap();
        let request =
            CreationRequest::new(project.clone(), CREATION_TIMESTAMP).package_catalog(catalog);
        match create(&request).unwrap() {
            CreationOutcome::Issued(pack) => {
                issued = Some(*pack);
                break;
            }
            CreationOutcome::MissingPackages(missing) => {
                for spec in missing {
                    let url = package_archive_url(&spec).unwrap();
                    let bytes = fetch(&url).expect("the registry serves the reported package");
                    let tree =
                        expand_package_archive(spec.clone(), &bytes, GENEROUS_LIMITS).unwrap();
                    resolved.push((spec, tree, PackageDisposition::Embedded));
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
