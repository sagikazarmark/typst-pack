//! Pack archive encoding, decoding, and stream and file I/O.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

use std::io::{self, Cursor, Read, Write};

use proptest::prelude::*;
use typst::syntax::package::PackageSpec;
use typst_pack::pack_archive::{
    DecodeError, DecodeLimits, ManifestError, ReadError, ReadPackError, WritePackError, decode,
    encode, read, read_pack, write_pack,
};
use typst_pack::{LimitExceeded, Pack, PackMetadata};

fn spec(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

fn simple_pack() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .file("data/table.csv", b"a,b\n1,2\n".to_vec())
        .package_file(spec("@local/inside:1.0.0"), "lib.typ", b"inside".to_vec())
        .external_package_file(spec("@local/outside:1.0.0"), "lib.typ", b"outside".to_vec())
        .build()
        .unwrap()
}

/// Every member of an archive, in archive order.
fn members(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    (0..zip.len())
        .map(|index| {
            let mut entry = zip.by_index(index).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (entry.name().to_owned(), data)
        })
        .collect()
}

/// Writes members with the zip crate's defaults.
fn zip(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in members {
        if let Some(directory) = name.strip_suffix('/') {
            writer
                .add_directory(directory, zip::write::SimpleFileOptions::default())
                .unwrap();
        } else {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

/// Re-zips an archive after editing its members.
fn rewrite(archive: &[u8], edit: impl FnOnce(&mut Vec<(String, Vec<u8>)>)) -> Vec<u8> {
    let mut members = members(archive);
    edit(&mut members);
    zip(&members)
}

fn edit_manifest(archive: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    rewrite(archive, |members| {
        let manifest = members
            .iter_mut()
            .find(|(name, _)| name == "typst-pack.toml")
            .unwrap();
        manifest.1 = edit(String::from_utf8(manifest.1.clone()).unwrap()).into_bytes();
    })
}

fn manifest(archive: &[u8]) -> String {
    let (_, data) = members(archive)
        .into_iter()
        .find(|(name, _)| name == "typst-pack.toml")
        .unwrap();
    String::from_utf8(data).unwrap()
}

fn decode_default(archive: &[u8]) -> Result<Pack, DecodeError> {
    decode(archive, DecodeLimits::DEFAULT)
}

type Files = Vec<(String, Vec<u8>)>;

#[derive(Debug, PartialEq, Eq)]
struct Projection {
    identity: typst_pack::Digest,
    entrypoint: String,
    files: Files,
    package_requirements: Vec<typst_pack::PackageRequirement>,
    packages: Vec<(String, Files)>,
    font_requirements: Vec<typst_pack::FontRequirement>,
    font_data: Vec<Option<Vec<u8>>>,
    metadata: Option<PackMetadata>,
}

fn projection(pack: &Pack) -> Projection {
    Projection {
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
        font_requirements: pack.font_requirements().to_vec(),
        font_data: pack
            .font_requirements()
            .iter()
            .map(|requirement| pack.font_data(&requirement.digest).map(<[u8]>::to_vec))
            .collect(),
        metadata: pack.metadata().cloned(),
    }
}

proptest! {
    #[test]
    fn encode_decode_preserves_the_semantic_projection(
        files in prop::collection::btree_map("[a-z]{1,8}\\.txt", prop::collection::vec(any::<u8>(), 0..64), 0..8),
        embedded_package_files in prop::collection::btree_map("[a-z]{1,8}\\.typ", prop::collection::vec(any::<u8>(), 0..64), 0..4),
        external_package_files in prop::collection::btree_map("[a-z]{1,8}\\.typ", prop::collection::vec(any::<u8>(), 0..64), 0..4),
        name in prop::option::of("[a-zA-Z \"\\\\\n\t]{0,24}"),
        description in prop::option::of("[a-zA-Z ]{0,48}"),
        authors in prop::collection::vec("[a-zA-Z ]{0,24}", 0..4),
    ) {
        let mut builder = Pack::builder("main.typ").file("main.typ", b"Property".to_vec());
        for (path, data) in files {
            builder = builder.file(path, data);
        }
        for (path, data) in embedded_package_files {
            builder = builder.package_file(spec("@local/embedded:1.0.0"), path, data);
        }
        for (path, data) in external_package_files {
            builder = builder.external_package_file(spec("@local/external:1.0.0"), path, data);
        }
        if name.is_some() || description.is_some() || !authors.is_empty() {
            builder = builder.metadata(PackMetadata { name, description, authors });
        }
        let pack = builder.build().unwrap();

        let decoded = decode_default(&encode(&pack).unwrap()).unwrap();

        prop_assert_eq!(projection(&decoded), projection(&pack));
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn encode_decode_preserves_font_containers() {
    let regular = fonts::typst_container();
    let collection = fonts::font_collection(&[regular.clone(), regular.clone()]);
    for embedded in [true, false] {
        let builder = Pack::builder("main.typ").file("main.typ", b"Fonts".to_vec());
        let builder = if embedded {
            builder.font(collection.clone()).font(regular.clone())
        } else {
            builder
                .external_font(collection.clone())
                .external_font(regular.clone())
        };
        let pack = builder.build().unwrap();
        assert_eq!(pack.font_requirements()[0].faces.len(), 2);

        let decoded = decode_default(&encode(&pack).unwrap()).unwrap();

        assert_eq!(projection(&decoded), projection(&pack));
    }
}

#[test]
fn encoding_is_deterministic() {
    let pack = simple_pack();
    let archive = encode(&pack).unwrap();
    assert_eq!(encode(&pack).unwrap(), archive);
    assert_eq!(encode(&decode_default(&archive).unwrap()).unwrap(), archive);
}

#[test]
fn manifest_declares_every_member_by_digest() {
    let archive = encode(&simple_pack()).unwrap();
    let manifest = manifest(&archive);

    assert!(manifest.starts_with("format-version = 2\n"), "{manifest}");
    assert!(manifest.contains(concat!(
        "generator = \"typst-pack ",
        env!("CARGO_PKG_VERSION"),
        "\""
    )));
    assert!(manifest.contains("typst-version = \"0.15"));
    assert!(manifest.contains("entrypoint = \"main.typ\""));
    assert!(manifest.contains("[[files]]\npath = \"data/table.csv\"\ndigest = \"sha256:"));
    assert!(manifest.contains("spec = \"@local/outside:1.0.0\""));
    assert!(manifest.contains("embedded = false"));
    assert_eq!(
        members(&archive)
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [
            "typst-pack.toml",
            "project/data/table.csv",
            "project/main.typ",
            "packages/local/inside/1.0.0/lib.typ",
        ]
    );
}

#[test]
fn decoding_tolerates_unknown_manifest_keys_and_other_zip_writers() {
    let pack = simple_pack();
    let archive = edit_manifest(&encode(&pack).unwrap(), |manifest| {
        let manifest = manifest
            .replacen("entrypoint", "future-key = [1, 2]\nentrypoint", 1)
            .replace("[[files]]\n", "[[files]]\nfuture-entry-key = true\n");
        format!("{manifest}\n[future-table]\nkey = \"value\"\n")
    });
    // Directory members and stored members, as other writers produce.
    let archive = rewrite(&archive, |members| {
        members.insert(1, ("project/".to_owned(), Vec::new()));
        members.insert(2, ("project/data/".to_owned(), Vec::new()));
    });

    assert_eq!(
        decode_default(&archive).unwrap().identity(),
        pack.identity()
    );
}

#[test]
fn decoding_rejects_other_format_versions() {
    let archive = encode(&simple_pack()).unwrap();
    for version in ["1", "3", "0"] {
        let changed = edit_manifest(&archive, |manifest| {
            manifest.replace("format-version = 2", &format!("format-version = {version}"))
        });
        assert!(matches!(
            decode_default(&changed),
            Err(DecodeError::Manifest(ManifestError::UnsupportedVersion(found)))
                if found.to_string() == version
        ));
    }
    let missing = edit_manifest(&archive, |manifest| {
        manifest.replace("format-version = 2\n", "")
    });
    assert!(matches!(
        decode_default(&missing),
        Err(DecodeError::Manifest(ManifestError::MissingFormatVersion))
    ));
    let malformed = edit_manifest(&archive, |manifest| {
        manifest.replace("@local/outside:1.0.0", "not a spec")
    });
    assert!(matches!(
        decode_default(&malformed),
        Err(DecodeError::Manifest(ManifestError::Parse(_)))
    ));
}

#[test]
fn decoding_rejects_non_packs() {
    assert!(matches!(
        decode_default(b"not a zip"),
        Err(DecodeError::Zip(_))
    ));
    let no_manifest = zip(&[("project/main.typ".to_owned(), b"Hello".to_vec())]);
    assert!(matches!(
        decode_default(&no_manifest),
        Err(DecodeError::MissingManifest)
    ));
}

#[test]
fn decoding_rejects_unsafe_and_duplicate_member_names() {
    let archive = encode(&simple_pack()).unwrap();
    for name in [
        "../escape.typ",
        "project/../../escape.typ",
        "/project/main.typ",
        "C:/evil.typ",
        "a\\b",
    ] {
        let unsafe_archive = rewrite(&archive, |members| {
            members.push((name.to_owned(), Vec::new()));
        });
        assert!(
            matches!(
                decode_default(&unsafe_archive),
                Err(DecodeError::UnsafeMemberName(found)) if found == name
            ),
            "{name}"
        );
    }

    let aliased = rewrite(&archive, |members| {
        members.push(("project/./main.typ".to_owned(), b"Other".to_vec()));
    });
    assert!(matches!(
        decode_default(&aliased),
        Err(DecodeError::DuplicateMember(name)) if name == "project/main.typ"
    ));

    // The zip writer refuses duplicate names, so rename a member in place.
    let renamed = rewrite(&archive, |members| {
        members.push(("project/main.tyX".to_owned(), b"Other".to_vec()));
    });
    let duplicated = replace_all(&renamed, b"project/main.tyX", b"project/main.typ");
    assert!(matches!(
        decode_default(&duplicated),
        Err(DecodeError::DuplicateMember(name)) if name == "project/main.typ"
    ));
}

#[test]
fn decoding_rejects_links_and_other_member_kinds() {
    let archive = encode(&simple_pack()).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in members(&archive) {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&data).unwrap();
    }
    writer
        .add_symlink(
            "project/link.typ",
            "/etc/passwd",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let linked = writer.finish().unwrap().into_inner();

    assert!(matches!(
        decode_default(&linked),
        Err(DecodeError::UnsupportedMemberKind(name)) if name == "project/link.typ"
    ));
}

#[test]
fn decoding_requires_exactly_the_declared_members() {
    let archive = encode(&simple_pack()).unwrap();

    let undeclared = rewrite(&archive, |members| {
        members.push(("project/extra.typ".to_owned(), b"extra".to_vec()));
    });
    assert!(matches!(
        decode_default(&undeclared),
        Err(DecodeError::UndeclaredMember(name)) if name == "project/extra.typ"
    ));
    let unknown_root = rewrite(&archive, |members| {
        members.push(("README.md".to_owned(), b"extra".to_vec()));
    });
    assert!(matches!(
        decode_default(&unknown_root),
        Err(DecodeError::UndeclaredMember(name)) if name == "README.md"
    ));
    let undeclared_package = rewrite(&archive, |members| {
        members.push((
            "packages/local/outside/1.0.0/lib.typ".to_owned(),
            b"outside".to_vec(),
        ));
    });
    assert!(matches!(
        decode_default(&undeclared_package),
        Err(DecodeError::UndeclaredMember(_))
    ));

    let missing = rewrite(&archive, |members| {
        members.retain(|(name, _)| name != "project/data/table.csv");
    });
    assert!(matches!(
        decode_default(&missing),
        Err(DecodeError::MissingMember(_))
    ));
    let missing_package = rewrite(&archive, |members| {
        members.retain(|(name, _)| !name.starts_with("packages/"));
    });
    assert!(matches!(
        decode_default(&missing_package),
        Err(DecodeError::MissingMember(name)) if name.contains("@local/inside:1.0.0")
    ));
}

#[test]
fn decoding_verifies_every_member_digest() {
    let archive = encode(&simple_pack()).unwrap();
    for target in ["project/main.typ", "packages/local/inside/1.0.0/lib.typ"] {
        let tampered = rewrite(&archive, |members| {
            members
                .iter_mut()
                .find(|(name, _)| name == target)
                .unwrap()
                .1 = b"tampered".to_vec();
        });
        assert!(
            matches!(
                decode_default(&tampered),
                Err(DecodeError::DigestMismatch { .. })
            ),
            "{target}"
        );
    }
    let extra_package_file = rewrite(&archive, |members| {
        members.push((
            "packages/local/inside/1.0.0/extra.typ".to_owned(),
            Vec::new(),
        ));
    });
    assert!(matches!(
        decode_default(&extra_package_file),
        Err(DecodeError::DigestMismatch { .. })
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn decoding_verifies_embedded_font_digests() {
    let font = fonts::typst_container();
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .font(font.clone())
        .build()
        .unwrap();
    let archive = encode(&pack).unwrap();
    let mut other = font;
    other.push(0);
    let tampered = rewrite(&archive, |members| {
        members
            .iter_mut()
            .find(|(name, _)| name.starts_with("fonts/"))
            .unwrap()
            .1 = other;
    });
    assert!(matches!(
        decode_default(&tampered),
        Err(DecodeError::DigestMismatch { .. })
    ));
}

#[test]
fn decoding_is_bounded() {
    let archive = encode(&simple_pack()).unwrap();
    let limited = |limits: DecodeLimits| match decode(&archive, limits) {
        Err(DecodeError::Limit(LimitExceeded { resource, .. })) => resource,
        other => panic!("expected a limit, got {other:?}"),
    };
    let base = DecodeLimits::DEFAULT;
    limited(DecodeLimits {
        max_archive_bytes: archive.len() as u64 - 1,
        ..base
    });
    limited(DecodeLimits {
        max_members: 3,
        ..base
    });
    limited(DecodeLimits {
        max_member_bytes: 6,
        ..base
    });
    limited(DecodeLimits {
        max_total_bytes: 20,
        ..base
    });
    assert!(
        decode(
            &archive,
            DecodeLimits {
                max_archive_bytes: archive.len() as u64,
                max_members: 4,
                ..base
            }
        )
        .is_ok()
    );
}

#[test]
fn decoding_meters_actual_inflation_not_declared_sizes() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", vec![b'a'; 1 << 20])
        .build()
        .unwrap();
    let archive = encode(&pack).unwrap();
    // Declare one byte for a member that inflates to a megabyte.
    let dishonest = with_declared_size(archive, b"project/main.typ", 1);
    let limits = DecodeLimits {
        max_member_bytes: 1024,
        ..DecodeLimits::DEFAULT
    };

    assert!(matches!(
        decode(&dishonest, limits),
        Err(DecodeError::Limit(_) | DecodeError::UnreadableMember { .. })
    ));
}

#[test]
fn stream_read_is_bounded() {
    let archive = encode(&simple_pack()).unwrap();
    let length = archive.len() as u64;
    assert_eq!(read(Cursor::new(&archive), length).unwrap(), archive);
    assert!(matches!(
        read(Cursor::new(&archive), length - 1),
        Err(ReadError::Limit(_))
    ));
    assert!(matches!(read(FailingReader, length), Err(ReadError::Io(_))));

    let pack = read_pack(Cursor::new(&archive), DecodeLimits::DEFAULT).unwrap();
    assert_eq!(pack.identity(), simple_pack().identity());
    assert!(matches!(
        read_pack(Cursor::new(b"not a zip"), DecodeLimits::DEFAULT),
        Err(ReadPackError::Decode(DecodeError::Zip(_)))
    ));
}

#[test]
fn stream_write_writes_the_encoded_archive() {
    let pack = simple_pack();
    let mut output = Vec::new();
    write_pack(&mut output, &pack).unwrap();
    assert_eq!(output, encode(&pack).unwrap());
    assert!(matches!(
        write_pack(FailingWriter, &pack),
        Err(WritePackError::Io(_))
    ));
}

#[cfg(feature = "fs")]
#[test]
fn files_are_written_without_replacing_unless_asked() {
    use typst_pack::pack_archive::{SavePackError, open_pack, save_pack, write_file};
    use typst_pack::{FilesystemWriteError, WritePolicy};

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    write_file(&destination, b"first", WritePolicy::CreateNew).unwrap();
    let error = write_file(&destination, b"second", WritePolicy::CreateNew).unwrap_err();
    assert!(matches!(
        error,
        FilesystemWriteError::Io { ref source, .. } if source.kind() == io::ErrorKind::AlreadyExists
    ));
    assert_eq!(std::fs::read(&destination).unwrap(), b"first");

    let pack = simple_pack();
    save_pack(&destination, &pack, WritePolicy::Overwrite).unwrap();
    assert_eq!(
        open_pack(&destination, DecodeLimits::DEFAULT)
            .unwrap()
            .identity(),
        pack.identity()
    );
    assert!(matches!(
        save_pack(&destination, &pack, WritePolicy::CreateNew),
        Err(SavePackError::Write(_))
    ));
    let names = std::fs::read_dir(directory.path()).unwrap().count();
    assert_eq!(names, 1, "no temporary file is left behind");
}

#[cfg(feature = "fs")]
#[test]
fn opening_a_file_is_bounded_and_names_the_path() {
    use typst_pack::pack_archive::open_pack;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("archive.typk");
    let archive = encode(&simple_pack()).unwrap();
    std::fs::write(&path, &archive).unwrap();

    let error = open_pack(
        &path,
        DecodeLimits {
            max_archive_bytes: archive.len() as u64 - 1,
            ..DecodeLimits::DEFAULT
        },
    )
    .unwrap_err();
    assert_eq!(error.path, path);
    assert!(matches!(
        error.source,
        ReadPackError::Read(ReadError::Limit(_))
    ));
    assert!(error.to_string().contains("archive.typk"));

    let missing = open_pack(directory.path().join("missing.typk"), DecodeLimits::DEFAULT);
    assert!(matches!(
        missing,
        Err(error) if matches!(error.source, ReadPackError::Read(ReadError::Io(_)))
    ));
}

fn replace_all(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    assert_eq!(from.len(), to.len());
    let mut output = haystack.to_vec();
    let mut index = 0;
    while index + from.len() <= output.len() {
        if &output[index..index + from.len()] == from {
            output[index..index + from.len()].copy_from_slice(to);
            index += from.len();
        } else {
            index += 1;
        }
    }
    output
}

/// Rewrites the uncompressed size a member's local and central headers declare.
fn with_declared_size(mut archive: Vec<u8>, name: &[u8], size: u32) -> Vec<u8> {
    let mut index = 0;
    while index + name.len() <= archive.len() {
        if &archive[index..index + name.len()] == name {
            if index >= 30 && archive[index - 30..].starts_with(b"PK\x03\x04") {
                archive[index - 30 + 22..index - 30 + 26].copy_from_slice(&size.to_le_bytes());
            }
            if index >= 46 && archive[index - 46..].starts_with(b"PK\x01\x02") {
                archive[index - 46 + 24..index - 46 + 28].copy_from_slice(&size.to_le_bytes());
            }
        }
        index += 1;
    }
    archive
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted read failure"))
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted write failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
