use typst_pack::PackArchiveBytes;
use typst_pack::PackInvariantIssue;
use typst_pack::pack_archive::{
    ArchiveError, DecodeError, DecodeLimitError, DecodeLimits, DecodeResource, ManifestError,
    decode,
};

const ACCEPTED: &[u8] = include_bytes!("fixtures/pack-archive-v1/accepted-python.typk");

#[test]
fn reference_v1_profile_has_every_required_ceiling() {
    let limits = DecodeLimits::reference_v1();

    assert_eq!(limits.archive_bytes(), 512 * 1024 * 1024);
    assert_eq!(limits.members(), 100_000);
    assert_eq!(limits.raw_member_name_bytes(), 16 * 1024 * 1024);
    assert_eq!(limits.manifest_bytes(), 4 * 1024 * 1024);
    assert_eq!(limits.member_bytes(), 256 * 1024 * 1024);
    assert_eq!(limits.total_content_bytes(), 2 * 1024 * 1024 * 1024);
}

#[test]
fn decode_limits_reject_an_unprobeable_ceiling() {
    let resources = [
        DecodeResource::ArchiveBytes,
        DecodeResource::Members,
        DecodeResource::RawMemberNameBytes,
        DecodeResource::ManifestBytes,
        DecodeResource::MemberBytes,
        DecodeResource::TotalContentBytes,
    ];

    for (index, _resource) in resources.into_iter().enumerate() {
        let mut values = [1; 6];
        values[index] = u64::MAX;
        assert!(
            std::panic::catch_unwind(|| {
                DecodeLimits::new(
                    values[0], values[1], values[2], values[3], values[4], values[5],
                )
            })
            .is_err()
        );
    }
}

#[test]
fn generated_boundaries_cover_every_pack_archive_decoding_resource() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let cases = [
        (DecodeResource::ArchiveBytes, archive.len()),
        (DecodeResource::Members, 6),
        (DecodeResource::RawMemberNameBytes, 84),
        (DecodeResource::ManifestBytes, 415),
        (DecodeResource::MemberBytes, 18),
        (DecodeResource::TotalContentBytes, 31),
    ];

    for (resource, observed) in cases {
        for ceiling in [observed + 1, observed] {
            decode(&archive, decode_limits_for(resource, ceiling)).unwrap_or_else(|error| {
                panic!("{resource:?} rejected observed {observed} at ceiling {ceiling}: {error}")
            });
        }

        let ceiling = observed - 1;
        let error = decode(&archive, decode_limits_for(resource, ceiling)).unwrap_err();
        assert!(
            matches!(
                error,
                DecodeError::Limit(DecodeLimitError::Exceeded {
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

fn decode_limits_for(resource: DecodeResource, ceiling: u64) -> DecodeLimits {
    let mut values = [10_000; 6];
    let index = match resource {
        DecodeResource::ArchiveBytes => 0,
        DecodeResource::Members => 1,
        DecodeResource::RawMemberNameBytes => 2,
        DecodeResource::ManifestBytes => 3,
        DecodeResource::MemberBytes => 4,
        DecodeResource::TotalContentBytes => 5,
        _ => panic!("boundary fixture does not cover a future decode resource"),
    };
    values[index] = ceiling;
    DecodeLimits::new(
        values[0], values[1], values[2], values[3], values[4], values[5],
    )
}

#[test]
fn reference_v1_profile_decodes_borrowed_exact_archive_bytes() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let archive_pointer = archive.as_slice().as_ptr();

    let pack = decode(&archive, DecodeLimits::reference_v1()).unwrap();

    assert_eq!(pack.entrypoint(), "main.typ");
    assert_eq!(
        pack.file("main.typ"),
        Some(b"Hello from Python\n".as_slice())
    );
    assert_eq!(archive.as_slice().as_ptr(), archive_pointer);
    drop(archive);
    assert_eq!(
        pack.file("notes/café.typ"),
        Some(b"Unicode path\n".as_slice())
    );
}

#[test]
fn archive_byte_limit_rejects_before_zip_parsing() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let ceiling = archive.len() - 1;
    let at_boundary = DecodeLimits::new(archive.len(), 100, 10_000, 10_000, 10_000, 100_000);
    let limits = DecodeLimits::new(ceiling, 100, 10_000, 10_000, 10_000, 100_000);

    assert!(decode(&archive, at_boundary).is_ok());
    let error = decode(&archive, limits).unwrap_err();

    assert!(matches!(
        error,
        DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::ArchiveBytes,
            ceiling: actual_ceiling,
            observed_at_least,
        }) if actual_ceiling == ceiling && observed_at_least == archive.len()
    ));
}

#[test]
fn member_limit_accepts_the_boundary_and_rejects_one_over() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let at_boundary = DecodeLimits::new(10_000, 6, 10_000, 10_000, 10_000, 100_000);
    let one_over = DecodeLimits::new(10_000, 5, 10_000, 10_000, 10_000, 100_000);

    assert!(decode(&archive, at_boundary).is_ok());
    assert!(matches!(
        decode(&archive, one_over),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::Members,
            ceiling: 5,
            observed_at_least: 6,
        }))
    ));
}

#[test]
fn raw_member_name_limit_accepts_the_boundary_and_rejects_one_over() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let at_boundary = DecodeLimits::new(10_000, 10, 84, 10_000, 10_000, 100_000);
    let one_over = DecodeLimits::new(10_000, 10, 83, 10_000, 10_000, 100_000);

    assert!(decode(&archive, at_boundary).is_ok());
    match decode(&archive, one_over).unwrap_err() {
        DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::RawMemberNameBytes,
            ceiling,
            observed_at_least,
        }) => assert_eq!((ceiling, observed_at_least), (83, 84)),
        error => panic!("unexpected error: {error:?}"),
    }
}

#[test]
fn manifest_limit_accepts_the_boundary_and_rejects_one_over() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let at_boundary = DecodeLimits::new(10_000, 10, 1_000, 415, 1_000, 10_000);
    let one_over = DecodeLimits::new(10_000, 10, 1_000, 414, 1_000, 10_000);

    assert!(decode(&archive, at_boundary).is_ok());
    assert!(matches!(
        decode(&archive, one_over),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::ManifestBytes,
            ceiling: 414,
            observed_at_least: 415,
        }))
    ));
}

#[test]
fn content_member_limit_accepts_the_boundary_and_rejects_one_over() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let at_boundary = DecodeLimits::new(10_000, 10, 1_000, 1_000, 18, 10_000);
    let one_over = DecodeLimits::new(10_000, 10, 1_000, 1_000, 17, 10_000);

    assert!(decode(&archive, at_boundary).is_ok());
    assert!(matches!(
        decode(&archive, one_over),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::MemberBytes,
            ceiling: 17,
            observed_at_least: 18,
        }))
    ));
}

#[test]
fn total_content_limit_accepts_the_boundary_and_rejects_one_over() {
    let archive = PackArchiveBytes::from_vec(ACCEPTED.to_vec());
    let at_boundary = DecodeLimits::new(10_000, 10, 1_000, 1_000, 1_000, 31);
    let one_over = DecodeLimits::new(10_000, 10, 1_000, 1_000, 1_000, 30);

    assert!(decode(&archive, at_boundary).is_ok());
    assert!(matches!(
        decode(&archive, one_over),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::TotalContentBytes,
            ceiling: 30,
            observed_at_least: 31,
        }))
    ));
}

#[test]
fn decode_failures_preserve_their_phase() {
    let malformed_archive = PackArchiveBytes::from_vec(b"not a ZIP archive".to_vec());
    assert!(matches!(
        decode(&malformed_archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::Zip(_)))
    ));

    let malformed_manifest = PackArchiveBytes::from_vec(
        include_bytes!("fixtures/pack-archive-v1/malformed-manifest.typk").to_vec(),
    );
    assert!(matches!(
        decode(&malformed_manifest, DecodeLimits::reference_v1()),
        Err(DecodeError::Manifest(ManifestError::Parse(_)))
    ));

    let invalid_pack = PackArchiveBytes::from_vec(
        include_bytes!("fixtures/pack-archive-v1/invalid-pack.typk").to_vec(),
    );
    let DecodeError::InvalidPack(error) =
        decode(&invalid_pack, DecodeLimits::reference_v1()).unwrap_err()
    else {
        panic!("invalid semantic content must remain an invalid-Pack failure");
    };
    assert!(matches!(
        error.issues(),
        [PackInvariantIssue::MissingEntrypoint { path }] if path == "missing.typ"
    ));
}

#[test]
fn raw_member_safety_precedes_manifest_interpretation() {
    let archive = PackArchiveBytes::from_vec(raw_stored_zip(&[
        ("typst-pack.toml", b"not valid TOML = ["),
        ("future/data", b"first"),
        ("future/data", b"second"),
    ]));

    assert!(matches!(
        decode(&archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::DuplicateMember(name)))
            if name == b"future/data"
    ));
}

#[test]
fn every_raw_member_is_checked_before_package_role_interpretation() {
    let archive = PackArchiveBytes::from_vec(raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/main.typ", b"Hello"),
        ("packages/not-a-package", b"invalid role layout"),
        ("../escape", b"unsafe later member"),
    ]));

    assert!(matches!(
        decode(&archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::UnsafeMemberName(name)))
            if name == "../escape"
    ));
}

#[test]
fn local_and_central_member_names_must_agree() {
    let mut bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/main.typ", b"Hello"),
    ]);
    let local_name = bytes
        .windows(b"project/main.typ".len())
        .position(|window| window == b"project/main.typ")
        .unwrap();
    bytes[local_name] = b'x';
    let archive = PackArchiveBytes::from_vec(bytes);

    assert!(matches!(
        decode(&archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::AmbiguousMemberNames))
    ));
}

#[test]
fn local_unicode_name_cannot_disagree_with_the_central_name() {
    let mut bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/main.typ", b"Hello"),
    ]);
    let raw_name = b"project/main.typ";
    let local_name = bytes
        .windows(raw_name.len())
        .position(|window| window == raw_name)
        .unwrap();
    let local_header = local_name - 30;
    let alternate = b"project/other.typ";
    let mut unicode_extra = Vec::new();
    unicode_extra.extend_from_slice(&0x7075u16.to_le_bytes());
    unicode_extra.extend_from_slice(&u16::try_from(5 + alternate.len()).unwrap().to_le_bytes());
    unicode_extra.push(1);
    unicode_extra.extend_from_slice(&test_crc32(raw_name).to_le_bytes());
    unicode_extra.extend_from_slice(alternate);
    bytes[local_header + 28..local_header + 30]
        .copy_from_slice(&u16::try_from(unicode_extra.len()).unwrap().to_le_bytes());
    let data_start = local_name + raw_name.len();
    bytes.splice(data_start..data_start, unicode_extra.iter().copied());
    let eocd = bytes.len() - 22;
    let central_offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
    bytes[eocd + 16..eocd + 20].copy_from_slice(
        &(central_offset + u32::try_from(unicode_extra.len()).unwrap()).to_le_bytes(),
    );
    let archive = PackArchiveBytes::from_vec(bytes);

    assert!(matches!(
        decode(&archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::AmbiguousMemberNames))
    ));
}

#[test]
fn one_valid_central_unicode_name_is_authoritative() {
    let bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/legacy.typ", b"Hello"),
    ]);
    let archive = PackArchiveBytes::from_vec(with_central_unicode_paths(
        bytes,
        "project/legacy.typ",
        &["project/main.typ"],
    ));

    let pack = decode(&archive, DecodeLimits::reference_v1()).unwrap();

    assert_eq!(pack.file("main.typ"), Some(b"Hello".as_slice()));
}

#[test]
fn duplicate_unicode_names_are_ambiguous() {
    let bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/legacy.typ", b"Hello"),
    ]);
    let archive = PackArchiveBytes::from_vec(with_central_unicode_paths(
        bytes,
        "project/legacy.typ",
        &["project/legacy.typ", "project/main.typ"],
    ));

    assert!(matches!(
        decode(&archive, DecodeLimits::reference_v1()),
        Err(DecodeError::Archive(ArchiveError::AmbiguousMemberNames))
    ));
}

fn with_central_unicode_paths(
    mut archive: Vec<u8>,
    target: &str,
    unicode_paths: &[&str],
) -> Vec<u8> {
    let eocd = archive.len() - 22;
    let count = usize::from(u16::from_le_bytes(
        archive[eocd + 10..eocd + 12].try_into().unwrap(),
    ));
    let mut cursor = usize::try_from(u32::from_le_bytes(
        archive[eocd + 16..eocd + 20].try_into().unwrap(),
    ))
    .unwrap();
    for _ in 0..count {
        let name_len = usize::from(u16::from_le_bytes(
            archive[cursor + 28..cursor + 30].try_into().unwrap(),
        ));
        let extra_len = usize::from(u16::from_le_bytes(
            archive[cursor + 30..cursor + 32].try_into().unwrap(),
        ));
        let comment_len = usize::from(u16::from_le_bytes(
            archive[cursor + 32..cursor + 34].try_into().unwrap(),
        ));
        let raw_name = archive[cursor + 46..cursor + 46 + name_len].to_vec();
        if raw_name == target.as_bytes() {
            let mut extra = Vec::new();
            for path in unicode_paths {
                extra.extend_from_slice(&0x7075u16.to_le_bytes());
                extra.extend_from_slice(&u16::try_from(5 + path.len()).unwrap().to_le_bytes());
                extra.push(1);
                extra.extend_from_slice(&test_crc32(&raw_name).to_le_bytes());
                extra.extend_from_slice(path.as_bytes());
            }
            archive[cursor + 30..cursor + 32].copy_from_slice(
                &u16::try_from(extra_len + extra.len())
                    .unwrap()
                    .to_le_bytes(),
            );
            let insert = cursor + 46 + name_len + extra_len;
            archive.splice(insert..insert, extra.iter().copied());
            let eocd = archive.len() - 22;
            let central_size =
                u32::from_le_bytes(archive[eocd + 12..eocd + 16].try_into().unwrap());
            archive[eocd + 12..eocd + 16].copy_from_slice(
                &(central_size + u32::try_from(extra.len()).unwrap()).to_le_bytes(),
            );
            return archive;
        }
        cursor += 46 + name_len + extra_len + comment_len;
    }
    panic!("missing ZIP member {target}");
}

fn test_crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[test]
fn invalid_manifest_package_spec_is_an_invalid_pack() {
    let manifest = br#"
        format-version = 1
        [project]
        entrypoint = "main.typ"
        [packages]
        unvendored = [{ spec = "not-a-package", tree-digest = "00000000000000000000000000000001", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 }]
    "#;
    let archive = PackArchiveBytes::from_vec(raw_stored_zip(&[
        ("typst-pack.toml", manifest),
        ("project/main.typ", b"Hello"),
    ]));

    let DecodeError::InvalidPack(error) =
        decode(&archive, DecodeLimits::reference_v1()).unwrap_err()
    else {
        panic!("invalid declaration semantics must reach Pack construction");
    };
    assert!(matches!(
        error.issues(),
        [PackInvariantIssue::InvalidPackageSpec { spec, .. }] if spec == "not-a-package"
    ));
}

#[test]
fn incremental_metering_rejects_a_dishonest_member_size() {
    let bytes = with_declared_uncompressed_size(ACCEPTED.to_vec(), "project/main.typ", 17);
    let archive = PackArchiveBytes::from_vec(bytes);
    let limits = DecodeLimits::new(10_000, 10, 1_000, 1_000, 17, 10_000);

    assert!(matches!(
        decode(&archive, limits),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::MemberBytes,
            ceiling: 17,
            observed_at_least: 18,
        }))
    ));
}

#[test]
fn incremental_metering_rejects_a_dishonest_total_size() {
    let bytes = with_declared_uncompressed_size(ACCEPTED.to_vec(), "project/main.typ", 17);
    let archive = PackArchiveBytes::from_vec(bytes);
    let limits = DecodeLimits::new(10_000, 10, 1_000, 1_000, 100, 30);

    assert!(matches!(
        decode(&archive, limits),
        Err(DecodeError::Limit(DecodeLimitError::Exceeded {
            resource: DecodeResource::TotalContentBytes,
            ceiling: 30,
            observed_at_least: 31,
        }))
    ));
}

#[test]
fn archive_decoder_fuzz_regressions_are_ordinary_contract_cases() {
    let cases: &[&[u8]] = &[
        b"",
        b"PK",
        b"PK\x03\x04\x14\0",
        b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    ];

    for &bytes in cases {
        let archive = PackArchiveBytes::from_vec(bytes.to_vec());
        assert!(matches!(
            decode(&archive, DecodeLimits::reference_v1()),
            Err(DecodeError::Archive(_))
        ));
    }
}

#[test]
fn zip64_end_records_are_accepted_before_bounded_raw_scanning() {
    let bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/main.typ", b"Hello"),
    ]);
    let archive = PackArchiveBytes::from_vec(with_zip64_end_records(bytes));

    let pack = decode(&archive, DecodeLimits::reference_v1()).unwrap();

    assert_eq!(pack.file("main.typ"), Some(b"Hello".as_slice()));
}

#[test]
fn zip64_central_local_header_offsets_are_accepted() {
    let bytes = raw_stored_zip(&[
        (
            "typst-pack.toml",
            b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("project/main.typ", b"Hello"),
    ]);
    let archive = PackArchiveBytes::from_vec(with_zip64_local_offset(bytes, "project/main.typ"));

    let pack = decode(&archive, DecodeLimits::reference_v1()).unwrap();

    assert_eq!(pack.file("main.typ"), Some(b"Hello".as_slice()));
}

fn with_zip64_local_offset(mut archive: Vec<u8>, target: &str) -> Vec<u8> {
    let eocd = archive.len() - 22;
    let count = usize::from(u16::from_le_bytes(
        archive[eocd + 10..eocd + 12].try_into().unwrap(),
    ));
    let mut cursor = usize::try_from(u32::from_le_bytes(
        archive[eocd + 16..eocd + 20].try_into().unwrap(),
    ))
    .unwrap();
    for _ in 0..count {
        let name_len = usize::from(u16::from_le_bytes(
            archive[cursor + 28..cursor + 30].try_into().unwrap(),
        ));
        let extra_len = usize::from(u16::from_le_bytes(
            archive[cursor + 30..cursor + 32].try_into().unwrap(),
        ));
        let comment_len = usize::from(u16::from_le_bytes(
            archive[cursor + 32..cursor + 34].try_into().unwrap(),
        ));
        if &archive[cursor + 46..cursor + 46 + name_len] == target.as_bytes() {
            let local_offset =
                u32::from_le_bytes(archive[cursor + 42..cursor + 46].try_into().unwrap());
            let mut extra = Vec::new();
            extra.extend_from_slice(&0x0001u16.to_le_bytes());
            extra.extend_from_slice(&8u16.to_le_bytes());
            extra.extend_from_slice(&u64::from(local_offset).to_le_bytes());
            archive[cursor + 30..cursor + 32].copy_from_slice(
                &u16::try_from(extra_len + extra.len())
                    .unwrap()
                    .to_le_bytes(),
            );
            archive[cursor + 42..cursor + 46].copy_from_slice(&u32::MAX.to_le_bytes());
            let insert = cursor + 46 + name_len + extra_len;
            archive.splice(insert..insert, extra.iter().copied());
            let eocd = archive.len() - 22;
            let central_size =
                u32::from_le_bytes(archive[eocd + 12..eocd + 16].try_into().unwrap());
            archive[eocd + 12..eocd + 16].copy_from_slice(
                &(central_size + u32::try_from(extra.len()).unwrap()).to_le_bytes(),
            );
            return archive;
        }
        cursor += 46 + name_len + extra_len + comment_len;
    }
    panic!("missing ZIP member {target}");
}

fn with_zip64_end_records(mut archive: Vec<u8>) -> Vec<u8> {
    let eocd = archive.len() - 22;
    let entries = u16::from_le_bytes(archive[eocd + 10..eocd + 12].try_into().unwrap());
    let central_size = u32::from_le_bytes(archive[eocd + 12..eocd + 16].try_into().unwrap());
    let central_offset = u32::from_le_bytes(archive[eocd + 16..eocd + 20].try_into().unwrap());
    archive.truncate(eocd);
    let zip64_eocd_offset = archive.len() as u64;

    archive.extend_from_slice(b"PK\x06\x06");
    archive.extend_from_slice(&44u64.to_le_bytes());
    archive.extend_from_slice(&45u16.to_le_bytes());
    archive.extend_from_slice(&45u16.to_le_bytes());
    archive.extend_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&u64::from(entries).to_le_bytes());
    archive.extend_from_slice(&u64::from(entries).to_le_bytes());
    archive.extend_from_slice(&u64::from(central_size).to_le_bytes());
    archive.extend_from_slice(&u64::from(central_offset).to_le_bytes());
    archive.extend_from_slice(b"PK\x06\x07");
    archive.extend_from_slice(&0u32.to_le_bytes());
    archive.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
    archive.extend_from_slice(&1u32.to_le_bytes());
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&u16::MAX.to_le_bytes());
    archive.extend_from_slice(&u16::MAX.to_le_bytes());
    archive.extend_from_slice(&u32::MAX.to_le_bytes());
    archive.extend_from_slice(&u32::MAX.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive
}

fn with_declared_uncompressed_size(mut archive: Vec<u8>, target: &str, size: u32) -> Vec<u8> {
    let eocd = archive
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .unwrap();
    let count = usize::from(u16::from_le_bytes(
        archive[eocd + 10..eocd + 12].try_into().unwrap(),
    ));
    let mut cursor = usize::try_from(u32::from_le_bytes(
        archive[eocd + 16..eocd + 20].try_into().unwrap(),
    ))
    .unwrap();
    for _ in 0..count {
        let name_len = usize::from(u16::from_le_bytes(
            archive[cursor + 28..cursor + 30].try_into().unwrap(),
        ));
        let extra_len = usize::from(u16::from_le_bytes(
            archive[cursor + 30..cursor + 32].try_into().unwrap(),
        ));
        let comment_len = usize::from(u16::from_le_bytes(
            archive[cursor + 32..cursor + 34].try_into().unwrap(),
        ));
        let name = &archive[cursor + 46..cursor + 46 + name_len];
        if name == target.as_bytes() {
            let local = usize::try_from(u32::from_le_bytes(
                archive[cursor + 42..cursor + 46].try_into().unwrap(),
            ))
            .unwrap();
            archive[cursor + 24..cursor + 28].copy_from_slice(&size.to_le_bytes());
            archive[local + 22..local + 26].copy_from_slice(&size.to_le_bytes());
            return archive;
        }
        cursor += 46 + name_len + extra_len + comment_len;
    }
    panic!("missing ZIP member {target}");
}

fn raw_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    fn u16_bytes(value: usize) -> [u8; 2] {
        u16::try_from(value).unwrap().to_le_bytes()
    }

    fn u32_bytes(value: usize) -> [u8; 4] {
        u32::try_from(value).unwrap().to_le_bytes()
    }

    let mut archive = Vec::new();
    let mut central_entries = Vec::new();
    for &(name, data) in entries {
        let name = name.as_bytes();
        let offset = archive.len();
        let crc = crc32(data);
        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(data.len()));
        archive.extend_from_slice(&u32_bytes(data.len()));
        archive.extend_from_slice(&u16_bytes(name.len()));
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name);
        archive.extend_from_slice(data);
        central_entries.push((name, data.len(), crc, offset));
    }

    let central_start = archive.len();
    for (name, size, crc, offset) in central_entries {
        archive.extend_from_slice(b"PK\x01\x02");
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(size));
        archive.extend_from_slice(&u32_bytes(size));
        archive.extend_from_slice(&u16_bytes(name.len()));
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u32.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(offset));
        archive.extend_from_slice(name);
    }
    let central_size = archive.len() - central_start;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&u16_bytes(entries.len()));
    archive.extend_from_slice(&u16_bytes(entries.len()));
    archive.extend_from_slice(&u32_bytes(central_size));
    archive.extend_from_slice(&u32_bytes(central_start));
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive
}
