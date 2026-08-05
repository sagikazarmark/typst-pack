use proptest::prelude::*;
use typst_pack::pack_archive::{
    DecodeLimits, EncodeError, EncodeLimitError, EncodeLimits, EncodeLimitsError, EncodeResource,
    RepresentationError, decode, encode,
};
use typst_pack::{
    FontFaceIdentity, FontRequirement, Pack, PackFontCatalogFace, PackIdentity, PackMetadata,
    PackageRequirement,
};

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

#[test]
fn encoding_borrows_a_valid_pack_and_round_trips_its_semantics() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .file("notes/data.txt", b"Data".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let identity = pack.identity();

    let archive = encode(&pack, EncodeLimits::reference_v1()).unwrap();

    assert_eq!(pack.identity(), identity);
    assert_eq!(pack.file("main.typ"), Some(b"Hello".as_slice()));
    let decoded = decode(&archive, DecodeLimits::reference_v1()).unwrap();
    assert_eq!(decoded.identity(), identity);
    assert_eq!(
        decoded.files().collect::<Vec<_>>(),
        pack.files().collect::<Vec<_>>()
    );
}

#[test]
fn reference_v1_profile_has_every_required_encode_ceiling() {
    let limits = EncodeLimits::reference_v1();

    assert_eq!(limits.archive_bytes(), 512 * 1024 * 1024);
    assert_eq!(limits.members(), 100_000);
    assert_eq!(limits.generated_member_name_bytes(), 16 * 1024 * 1024);
    assert_eq!(limits.manifest_bytes(), 4 * 1024 * 1024);
    assert_eq!(limits.member_bytes(), 256 * 1024 * 1024);
    assert_eq!(limits.total_content_bytes(), 2 * 1024 * 1024 * 1024);
}

#[test]
fn encode_limits_reject_an_unprobeable_ceiling() {
    let resources = [
        EncodeResource::ArchiveBytes,
        EncodeResource::Members,
        EncodeResource::GeneratedMemberNameBytes,
        EncodeResource::ManifestBytes,
        EncodeResource::MemberBytes,
        EncodeResource::TotalContentBytes,
    ];

    for (index, resource) in resources.into_iter().enumerate() {
        let mut values = [1; 6];
        values[index] = u64::MAX;
        assert!(matches!(
            EncodeLimits::new(values[0], values[1], values[2], values[3], values[4], values[5]),
            Err(EncodeLimitsError::CannotProbe {
                resource: reported,
                ceiling: u64::MAX,
            }) if reported == resource
        ));
    }
}

#[test]
fn generated_boundaries_cover_every_pack_archive_encoding_resource() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"12".to_vec())
        .unwrap()
        .file("other.typ", b"345".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let baseline = encode(&pack, EncodeLimits::reference_v1()).unwrap();
    let (members, member_names, manifest_bytes) = {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(baseline.as_slice())).unwrap();
        let members = archive.len() as u64;
        let member_names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name_raw().len() as u64)
            .sum();
        let manifest_bytes = archive.by_name("typst-pack.toml").unwrap().size();
        (members, member_names, manifest_bytes)
    };
    let cases = [
        (EncodeResource::ArchiveBytes, baseline.len()),
        (EncodeResource::Members, members),
        (EncodeResource::GeneratedMemberNameBytes, member_names),
        (EncodeResource::ManifestBytes, manifest_bytes),
        (EncodeResource::MemberBytes, 3),
        (EncodeResource::TotalContentBytes, 5),
    ];

    for (resource, observed) in cases {
        for ceiling in [observed + 1, observed] {
            encode(&pack, encode_limits_for(resource, ceiling)).unwrap_or_else(|error| {
                panic!("{resource:?} rejected observed {observed} at ceiling {ceiling}: {error}")
            });
        }

        let ceiling = observed - 1;
        let error = encode(&pack, encode_limits_for(resource, ceiling)).unwrap_err();
        assert!(
            matches!(
                error,
                EncodeError::Limit(EncodeLimitError::Exceeded {
                    resource: reported,
                    ceiling: reported_ceiling,
                    observed_at_least,
                }) if reported == resource
                    && reported_ceiling == ceiling
                    && observed_at_least == observed
            ),
            "unexpected {resource:?} boundary failure: {error}"
        );
    }
}

fn encode_limits_for(resource: EncodeResource, ceiling: u64) -> EncodeLimits {
    let mut values = [10_000; 6];
    let index = match resource {
        EncodeResource::ArchiveBytes => 0,
        EncodeResource::Members => 1,
        EncodeResource::GeneratedMemberNameBytes => 2,
        EncodeResource::ManifestBytes => 3,
        EncodeResource::MemberBytes => 4,
        EncodeResource::TotalContentBytes => 5,
        _ => panic!("boundary fixture does not cover a future encode resource"),
    };
    values[index] = ceiling;
    EncodeLimits::new(
        values[0], values[1], values[2], values[3], values[4], values[5],
    )
    .unwrap()
}

#[test]
fn member_limit_accepts_the_boundary_and_rejects_one_over() {
    let pack = simple_pack();
    let at_boundary = EncodeLimits::new(10_000, 2, 1_000, 1_000, 1_000, 1_000).unwrap();
    let one_over = EncodeLimits::new(10_000, 1, 1_000, 1_000, 1_000, 1_000).unwrap();

    assert!(encode(&pack, at_boundary).is_ok());
    assert!(matches!(
        encode(&pack, one_over),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::Members,
            ceiling: 1,
            observed_at_least: 2,
        }))
    ));
}

#[test]
fn generated_member_name_limit_accepts_the_boundary_and_rejects_one_over() {
    let pack = simple_pack();
    let at_boundary = EncodeLimits::new(10_000, 10, 31, 1_000, 1_000, 1_000).unwrap();
    let one_over = EncodeLimits::new(10_000, 10, 30, 1_000, 1_000, 1_000).unwrap();

    assert!(encode(&pack, at_boundary).is_ok());
    assert!(matches!(
        encode(&pack, one_over),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::GeneratedMemberNameBytes,
            ceiling: 30,
            observed_at_least: 31,
        }))
    ));
}

#[test]
fn content_limits_accept_the_boundary_and_reject_one_over() {
    let pack = simple_pack();
    let identity = pack.identity();
    let at_boundary = EncodeLimits::new(10_000, 10, 1_000, 1_000, 5, 5).unwrap();
    let member_one_over = EncodeLimits::new(10_000, 10, 1_000, 1_000, 4, 1_000).unwrap();
    let total_one_over = EncodeLimits::new(10_000, 10, 1_000, 1_000, 1_000, 4).unwrap();

    assert!(encode(&pack, at_boundary).is_ok());
    assert!(matches!(
        encode(&pack, member_one_over),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::MemberBytes,
            ceiling: 4,
            observed_at_least: 5,
        }))
    ));
    assert!(matches!(
        encode(&pack, total_one_over),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::TotalContentBytes,
            ceiling: 4,
            observed_at_least: 5,
        }))
    ));
    assert_eq!(pack.identity(), identity);
}

#[test]
fn a_valid_pack_can_be_unrepresentable_in_version_one() {
    let path = "a".repeat(65_535 - "project/".len() + 1);
    let pack = Pack::builder(&path)
        .file(&path, b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let error = encode(&pack, EncodeLimits::reference_v1()).unwrap_err();

    assert!(matches!(
        error,
        EncodeError::Representation(RepresentationError::MemberNameTooLong {
            member_name,
            maximum: 65_535,
            observed: 65_536,
        }) if member_name == format!("project/{path}")
    ));
    assert_eq!(pack.file(&path), Some(b"Hello".as_slice()));

    let spec = "@local/example:1.0.0".parse().unwrap();
    let package_path = "b".repeat(65_535);
    let package = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .package_file(spec, &package_path, b"Package".to_vec())
        .unwrap()
        .build()
        .unwrap();

    assert!(matches!(
        encode(&package, EncodeLimits::reference_v1()),
        Err(EncodeError::Representation(
            RepresentationError::MemberNameTooLong { .. }
        ))
    ));
}

#[test]
fn generated_output_limits_accept_the_boundary_and_reject_one_over() {
    let pack = simple_pack();
    let baseline = encode(&pack, EncodeLimits::reference_v1()).unwrap();
    let manifest_bytes = {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(baseline.as_slice())).unwrap();
        archive.by_name("typst-pack.toml").unwrap().size()
    };
    let archive_bytes = baseline.len();
    let manifest_at_boundary =
        EncodeLimits::new(10_000, 10, 1_000, manifest_bytes, 1_000, 1_000).unwrap();
    let manifest_one_over =
        EncodeLimits::new(10_000, 10, 1_000, manifest_bytes - 1, 1_000, 1_000).unwrap();
    let archive_at_boundary =
        EncodeLimits::new(archive_bytes, 10, 1_000, 1_000, 1_000, 1_000).unwrap();
    let archive_one_over =
        EncodeLimits::new(archive_bytes - 1, 10, 1_000, 1_000, 1_000, 1_000).unwrap();

    assert!(encode(&pack, manifest_at_boundary).is_ok());
    assert!(matches!(
        encode(&pack, manifest_one_over),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::ManifestBytes,
            ceiling,
            observed_at_least,
        })) if ceiling == manifest_bytes - 1 && observed_at_least == manifest_bytes
    ));
    assert!(encode(&pack, archive_at_boundary).is_ok());
    let archive_error = encode(&pack, archive_one_over).unwrap_err();
    assert!(
        matches!(
            &archive_error,
            EncodeError::Limit(EncodeLimitError::Exceeded {
                resource: EncodeResource::ArchiveBytes,
                ceiling,
                observed_at_least,
            }) if *ceiling == archive_bytes - 1 && *observed_at_least == archive_bytes
        ),
        "{archive_error:?}"
    );
}

#[test]
fn manifest_encoding_is_incrementally_bounded_and_escapes_metadata() {
    let escaped = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .metadata(
            PackMetadata::new()
                .with_name("quote \" slash \\ newline\n null \0")
                .with_description("Unicode café")
                .with_author("First\tAuthor")
                .with_author("Second"),
        )
        .build()
        .unwrap();
    let archive = encode(&escaped, EncodeLimits::reference_v1()).unwrap();
    let decoded = decode(&archive, DecodeLimits::reference_v1()).unwrap();
    assert_eq!(semantic_projection(&decoded), semantic_projection(&escaped));

    let oversized = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .metadata(PackMetadata::new().with_name("a".repeat(100_000)))
        .build()
        .unwrap();
    let limits = EncodeLimits::new(1_000_000, 10, 1_000, 64, 1_000, 1_000).unwrap();
    assert!(matches!(
        encode(&oversized, limits),
        Err(EncodeError::Limit(EncodeLimitError::Exceeded {
            resource: EncodeResource::ManifestBytes,
            ceiling: 64,
            observed_at_least,
        })) if observed_at_least > 64
    ));
}

fn simple_pack() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap()
}

proptest! {
    #[test]
    fn encode_decode_preserves_the_semantic_projection(
        files in prop::collection::btree_map("[a-z]{1,8}\\.txt", prop::collection::vec(any::<u8>(), 0..64), 0..8),
        embedded_package_files in prop::collection::btree_map("[a-z]{1,8}\\.typ", prop::collection::vec(any::<u8>(), 0..64), 0..4),
        external_package_files in prop::collection::btree_map("[a-z]{1,8}\\.typ", prop::collection::vec(any::<u8>(), 0..64), 0..4),
        metadata_name in prop::option::of("[a-zA-Z ]{0,24}"),
        metadata_description in prop::option::of("[a-zA-Z ]{0,48}"),
        metadata_authors in prop::collection::vec("[a-zA-Z ]{0,24}", 0..4),
    ) {
        let mut builder = Pack::builder("main.typ")
            .file("main.typ", b"Property".to_vec())
            .unwrap();
        for (path, data) in files {
            builder = builder.file(path, data).unwrap();
        }
        let embedded_spec = "@local/embedded:1.0.0"
            .parse::<typst::syntax::package::PackageSpec>()
            .unwrap();
        for (path, data) in embedded_package_files {
            builder = builder.package_file(embedded_spec.clone(), path, data).unwrap();
        }
        let external_spec = "@local/external:1.0.0"
            .parse::<typst::syntax::package::PackageSpec>()
            .unwrap();
        for (path, data) in external_package_files {
            builder = builder.external_package_file(external_spec.clone(), path, data).unwrap();
        }
        if metadata_name.is_some() || metadata_description.is_some() || !metadata_authors.is_empty() {
            let mut metadata = PackMetadata::new();
            if let Some(name) = metadata_name {
                metadata = metadata.with_name(name);
            }
            if let Some(description) = metadata_description {
                metadata = metadata.with_description(description);
            }
            for author in metadata_authors {
                metadata = metadata.with_author(author);
            }
            builder = builder.metadata(metadata);
        }
        let pack = builder.build().unwrap();

        let archive = encode(&pack, EncodeLimits::reference_v1()).unwrap();
        let decoded = decode(&archive, DecodeLimits::reference_v1()).unwrap();

        prop_assert_eq!(semantic_projection(&decoded), semantic_projection(&pack));
    }
}

#[cfg(feature = "embedded-fonts")]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(4))]

    #[test]
    fn encode_decode_preserves_font_semantics(embedded in any::<bool>()) {
        let font = fonts::typst_container();
        let builder = Pack::builder("main.typ")
            .file("main.typ", b"Font property".to_vec())
            .unwrap();
        let builder = if embedded {
            builder.font(font, 0).unwrap()
        } else {
            builder.external_font(font, 0).unwrap()
        };
        let pack = builder.build().unwrap();

        let archive = encode(&pack, EncodeLimits::reference_v1()).unwrap();
        let decoded = decode(&archive, DecodeLimits::reference_v1()).unwrap();

        prop_assert_eq!(semantic_projection(&decoded), semantic_projection(&pack));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PackProjection {
    identity: PackIdentity,
    entrypoint: String,
    files: Vec<(String, Vec<u8>)>,
    package_requirements: Vec<PackageRequirement>,
    packages: Vec<(String, OwnedFiles)>,
    font_catalog: Vec<PackFontCatalogFace>,
    font_requirements: Vec<FontRequirement>,
    fonts: Vec<(FontFaceIdentity, Vec<u8>)>,
    metadata: Option<(Option<String>, Option<String>, Vec<String>)>,
}

type OwnedFiles = Vec<(String, Vec<u8>)>;

fn semantic_projection(pack: &Pack) -> PackProjection {
    PackProjection {
        identity: pack.identity(),
        entrypoint: pack.entrypoint().to_owned(),
        files: pack
            .files()
            .map(|(path, data)| (path.to_owned(), data.to_vec()))
            .collect(),
        package_requirements: pack.package_requirements().to_vec(),
        packages: pack
            .packages()
            .map(|(spec, files)| {
                (
                    spec.to_string(),
                    files
                        .map(|(path, data)| (path.to_owned(), data.to_vec()))
                        .collect(),
                )
            })
            .collect(),
        font_catalog: pack.font_catalog().to_vec(),
        font_requirements: pack.font_requirements().to_vec(),
        fonts: pack
            .fonts()
            .iter()
            .map(|font| (font.identity(), font.data().to_vec()))
            .collect(),
        metadata: pack.metadata().map(|metadata| {
            (
                metadata.name().map(str::to_owned),
                metadata.description().map(str::to_owned),
                metadata.authors().to_vec(),
            )
        }),
    }
}
