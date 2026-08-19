use std::io::Read;

use flate2::read::DeflateDecoder;
#[path = "support/archive.rs"]
mod archive_support;
use archive_support::{decode_reference, encode_reference};
use typst_pack::pack_archive::{ArchiveError, DecodeError, ManifestError};
use typst_pack::{Pack, PackInvariantIssue};

const ACCEPTED: &[u8] = include_bytes!("fixtures/pack-archive-v1/accepted-python.typk");

#[derive(Debug, Eq, PartialEq)]
struct PackObservation {
    entrypoint: String,
    files: Vec<(String, Vec<u8>)>,
    packages: Vec<(String, IdentityObservation, u64, u64, bool)>,
    metadata: Option<(String, Vec<String>)>,
}

#[derive(Debug, Eq, PartialEq)]
struct IdentityObservation {
    kind: String,
    schema: String,
    algorithm: String,
    digest: [u8; 16],
}

fn observe_pack(pack: &Pack) -> PackObservation {
    PackObservation {
        entrypoint: pack.entrypoint().to_owned(),
        files: pack
            .files()
            .map(|(path, data)| (path.to_owned(), data.to_vec()))
            .collect(),
        packages: pack
            .package_requirements()
            .iter()
            .map(|requirement| {
                let identity = requirement.tree_identity();
                (
                    requirement.spec().to_string(),
                    IdentityObservation {
                        kind: identity.role().as_str().into(),
                        schema: identity.schema().into(),
                        algorithm: identity.algorithm().into(),
                        digest: identity.digest(),
                    },
                    requirement.file_count(),
                    requirement.byte_length(),
                    requirement.is_embedded(),
                )
            })
            .collect(),
        metadata: pack.metadata().map(|metadata| {
            (
                metadata.name().unwrap_or_default().to_owned(),
                metadata.authors().to_vec(),
            )
        }),
    }
}

#[test]
fn independently_produced_version_one_archive_decodes_semantically() {
    let pack = decode_reference(ACCEPTED.to_vec()).unwrap();

    assert_eq!(
        observe_pack(&pack),
        PackObservation {
            entrypoint: "main.typ".into(),
            files: vec![
                ("main.typ".into(), b"Hello from Python\n".to_vec()),
                ("notes/caf\u{e9}.typ".into(), b"Unicode path\n".to_vec()),
            ],
            packages: vec![(
                "@preview/example:1.0.0".into(),
                IdentityObservation {
                    kind: "complete-package-tree".into(),
                    schema: "typst-pack-complete-package-tree-v1".into(),
                    algorithm: "typst-hash128-0.15".into(),
                    digest: 1u128.to_be_bytes(),
                },
                1,
                7,
                false,
            )],
            metadata: Some(("Python fixture".into(), vec!["Independent producer".into()],)),
        }
    );
}

#[test]
fn safe_unknown_entries_may_disappear_without_changing_pack_semantics() {
    let pack = decode_reference(ACCEPTED.to_vec()).unwrap();
    let original = consume_zip_independently(ACCEPTED);
    assert!(original.iter().any(|entry| entry.name == "future/data.bin"));

    let rewritten = encode_reference(&pack).unwrap();
    let rewritten_entries = consume_zip_independently(&rewritten);
    assert!(
        rewritten_entries
            .iter()
            .all(|entry| entry.name != "future/data.bin")
    );

    let reread = decode_reference(rewritten).unwrap();
    assert_eq!(observe_pack(&reread), observe_pack(&pack));
}

#[derive(Debug, Eq, PartialEq)]
enum DecodeObservation {
    MissingManifest,
    DuplicateMember(Vec<u8>),
    AmbiguousMember,
    MalformedRawName(Vec<u8>),
    UnsafePath(String),
    UnsupportedMemberKind(String),
    ManifestNotUtf8,
    MalformedManifest,
    UnsupportedVersion(u32),
    InvalidPackMissingEntrypoint(String),
}

fn observe_decode(bytes: &[u8]) -> DecodeObservation {
    match decode_reference(bytes.to_vec()).unwrap_err() {
        DecodeError::Archive(ArchiveError::MissingManifest) => DecodeObservation::MissingManifest,
        DecodeError::Archive(ArchiveError::DuplicateMember(name)) => {
            DecodeObservation::DuplicateMember(name)
        }
        DecodeError::Archive(ArchiveError::AmbiguousMemberNames) => {
            DecodeObservation::AmbiguousMember
        }
        DecodeError::Archive(ArchiveError::InvalidUtf8MemberName(name)) => {
            DecodeObservation::MalformedRawName(name)
        }
        DecodeError::Archive(ArchiveError::UnsafeMemberName(path)) => {
            DecodeObservation::UnsafePath(path)
        }
        DecodeError::Archive(ArchiveError::UnsupportedMemberKind(path)) => {
            DecodeObservation::UnsupportedMemberKind(path)
        }
        DecodeError::Manifest(ManifestError::NotUtf8(_)) => DecodeObservation::ManifestNotUtf8,
        DecodeError::Manifest(ManifestError::Parse(_)) => DecodeObservation::MalformedManifest,
        DecodeError::Manifest(ManifestError::UnsupportedVersion(version)) => {
            DecodeObservation::UnsupportedVersion(version)
        }
        DecodeError::InvalidPack(error) => match error.issues() {
            [PackInvariantIssue::MissingEntrypoint { path }] => {
                DecodeObservation::InvalidPackMissingEntrypoint(path.clone())
            }
            issues => panic!("unexpected Pack invariant issues: {issues:?}"),
        },
        error => panic!("unexpected public decode observation: {error:?}"),
    }
}

#[test]
fn malformed_external_fixtures_have_stable_public_observations() {
    let cases: &[(&str, &[u8], DecodeObservation)] = &[
        (
            "missing manifest",
            include_bytes!("fixtures/pack-archive-v1/missing-manifest.typk"),
            DecodeObservation::MissingManifest,
        ),
        (
            "duplicate member",
            include_bytes!("fixtures/pack-archive-v1/duplicate-member.typk"),
            DecodeObservation::DuplicateMember(b"future/data".to_vec()),
        ),
        (
            "canonical collision",
            include_bytes!("fixtures/pack-archive-v1/canonical-collision.typk"),
            DecodeObservation::AmbiguousMember,
        ),
        (
            "malformed raw name",
            include_bytes!("fixtures/pack-archive-v1/invalid-utf8-name.typk"),
            DecodeObservation::MalformedRawName(b"future/\xff.bin".to_vec()),
        ),
        (
            "unsafe path",
            include_bytes!("fixtures/pack-archive-v1/unsafe-path.typk"),
            DecodeObservation::UnsafePath("../escape".into()),
        ),
        (
            "unsupported member kind",
            include_bytes!("fixtures/pack-archive-v1/unsupported-kind.typk"),
            DecodeObservation::UnsupportedMemberKind("future/link".into()),
        ),
        (
            "unsupported directory member kind",
            include_bytes!("fixtures/pack-archive-v1/unsupported-directory-kind.typk"),
            DecodeObservation::UnsupportedMemberKind("future/link/".into()),
        ),
        (
            "manifest UTF-8",
            include_bytes!("fixtures/pack-archive-v1/invalid-manifest-utf8.typk"),
            DecodeObservation::ManifestNotUtf8,
        ),
        (
            "manifest syntax",
            include_bytes!("fixtures/pack-archive-v1/malformed-manifest.typk"),
            DecodeObservation::MalformedManifest,
        ),
        (
            "manifest version",
            include_bytes!("fixtures/pack-archive-v1/unsupported-version.typk"),
            DecodeObservation::UnsupportedVersion(99),
        ),
        (
            "whole-Pack validation",
            include_bytes!("fixtures/pack-archive-v1/invalid-pack.typk"),
            DecodeObservation::InvalidPackMissingEntrypoint("missing.typ".into()),
        ),
    ];

    for (name, bytes, expected) in cases {
        assert_eq!(observe_decode(bytes), *expected, "fixture: {name}");
    }
}

#[test]
fn independent_zip_consumer_accepts_version_one_encoder_layout() {
    let spec = "@local/example:1.0.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .file("assets/data.txt", b"Data".to_vec())
        .unwrap()
        .package_file(spec, "lib.typ", b"Package".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let encoded = encode_reference(&pack).unwrap();
    let entries = consume_zip_independently(&encoded);
    assert_eq!(entries[0].name, "typst-pack.toml");
    assert!(entries.iter().all(|entry| entry.compression == 8));
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        [
            "typst-pack.toml",
            "project/assets/data.txt",
            "project/main.typ",
            "packages/local/example/1.0.0/lib.typ",
        ]
    );
    assert_eq!(entry(&entries, "project/main.typ").data, b"Hello".to_vec());
    assert_eq!(
        entry(&entries, "packages/local/example/1.0.0/lib.typ").data,
        b"Package".to_vec()
    );

    let manifest = std::str::from_utf8(&entry(&entries, "typst-pack.toml").data).unwrap();
    let manifest: toml::Value = toml::from_str(manifest).unwrap();
    assert_eq!(manifest["format-version"].as_integer(), Some(1));
    assert_eq!(manifest["project"]["entrypoint"].as_str(), Some("main.typ"));
    assert_eq!(
        manifest["packages"]["vendored"][0]["spec"].as_str(),
        Some("@local/example:1.0.0")
    );
}

#[derive(Debug)]
struct IndependentZipEntry {
    name: String,
    compression: u16,
    data: Vec<u8>,
}

fn entry<'a>(entries: &'a [IndependentZipEntry], name: &str) -> &'a IndependentZipEntry {
    entries.iter().find(|entry| entry.name == name).unwrap()
}

fn consume_zip_independently(bytes: &[u8]) -> Vec<IndependentZipEntry> {
    let eocd = bytes
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .expect("end of central directory");
    let count = usize::from(read_u16(bytes, eocd + 10));
    let mut cursor = usize::try_from(read_u32(bytes, eocd + 16)).unwrap();
    let mut entries = Vec::with_capacity(count);

    for _ in 0..count {
        assert_eq!(&bytes[cursor..cursor + 4], b"PK\x01\x02");
        let flags = read_u16(bytes, cursor + 8);
        assert_eq!(
            flags & 1,
            0,
            "encrypted members are unsupported by this oracle"
        );
        let compression = read_u16(bytes, cursor + 10);
        let compressed_size = usize::try_from(read_u32(bytes, cursor + 20)).unwrap();
        let name_len = usize::from(read_u16(bytes, cursor + 28));
        let extra_len = usize::from(read_u16(bytes, cursor + 30));
        let comment_len = usize::from(read_u16(bytes, cursor + 32));
        let local_offset = usize::try_from(read_u32(bytes, cursor + 42)).unwrap();
        let name = std::str::from_utf8(&bytes[cursor + 46..cursor + 46 + name_len])
            .unwrap()
            .to_owned();

        assert_eq!(&bytes[local_offset..local_offset + 4], b"PK\x03\x04");
        let local_name_len = usize::from(read_u16(bytes, local_offset + 26));
        let local_extra_len = usize::from(read_u16(bytes, local_offset + 28));
        let data_start = local_offset + 30 + local_name_len + local_extra_len;
        let compressed = &bytes[data_start..data_start + compressed_size];
        let data = match compression {
            0 => compressed.to_vec(),
            8 => {
                let mut data = Vec::new();
                DeflateDecoder::new(compressed)
                    .read_to_end(&mut data)
                    .unwrap();
                data
            }
            method => panic!("unsupported compression method in oracle: {method}"),
        };
        entries.push(IndependentZipEntry {
            name,
            compression,
            data,
        });
        cursor += 46 + name_len + extra_len + comment_len;
    }

    entries
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
