//! The Pack model: project snapshots, the archive format, and extraction
//! planning.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

use std::io::{self, Cursor, Read, Write};

use proptest::prelude::*;
use typst::syntax::package::PackageSpec;
use typst::text::{FontStretch, FontStyle, FontWeight};
use typst_pack::archive::{self, DecodeError, DecodeLimits, decode, encode};
use typst_pack::extract::{self, Conflict, Selection};
use typst_pack::font::Face;
use typst_pack::{Metadata, Pack, PackIssue, ProjectSnapshot};

fn spec(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

// --- Project snapshots -----------------------------------------------------

fn snapshot<P: AsRef<str>>(
    entries: impl IntoIterator<Item = (P, &'static str)>,
) -> ProjectSnapshot {
    let entries = entries
        .into_iter()
        .map(|(path, text)| (path.as_ref().to_owned(), text));
    ProjectSnapshot::new("main.typ", entries.collect::<Vec<_>>()).unwrap()
}

#[test]
fn a_snapshot_holds_canonical_paths_in_order() {
    let snapshot = snapshot([("./chapters/intro.typ", "= Intro"), ("main.typ", "Hello")]);

    assert_eq!(snapshot.entrypoint(), "main.typ");
    let paths = snapshot.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(paths, ["chapters/intro.typ", "main.typ"]);
    assert_eq!(snapshot.file("main.typ"), Some(b"Hello".as_slice()));
}

#[test]
fn snapshot_paths_must_name_root_relative_files_that_are_not_packs() {
    for path in [
        "",
        "/absolute.typ",
        "../escape.typ",
        "C:/drive.typ",
        "back\\slash.typ",
        ".typk",
        "old.typk",
        "nested/old.typk",
        "bundle.typk/main.typ",
    ] {
        let error =
            ProjectSnapshot::new("main.typ", [("main.typ", "Hello"), (path, "nope")]).unwrap_err();

        assert!(
            matches!(error.issues.as_slice(), [PackIssue::InvalidPath { path: reported, .. }] if reported == path),
            "`{path}`: {error}"
        );
    }

    // The builder applies the same rules, entrypoint included.
    let error = Pack::builder("main.typk")
        .file("main.typ", b"Hello".to_vec())
        .file("nested/old.typk", b"drop".to_vec())
        .build()
        .unwrap_err();
    assert!(
        error
            .issues
            .iter()
            .any(|issue| matches!(issue, PackIssue::InvalidEntrypoint { .. }))
    );
    assert!(
        error
            .issues
            .iter()
            .any(|issue| matches!(issue, PackIssue::InvalidPath { .. }))
    );
}

#[test]
fn snapshot_errors_list_every_independent_issue_escaped() {
    let error = ProjectSnapshot::new(
        "main.typ",
        [
            ("z\\escape.typ", "invalid"),
            ("./duplicate.typ", "first"),
            ("duplicate.typ", "second"),
            ("nested/../duplicate.typ", "third"),
            ("/forged\npath.typ", "invalid"),
        ],
    )
    .unwrap_err();

    let issues = &error.issues;
    assert_eq!(issues.len(), 4, "{error}");
    assert!(issues.contains(&PackIssue::DuplicatePath {
        path: "duplicate.typ".to_owned()
    }));
    assert!(issues.contains(&PackIssue::MissingEntrypoint {
        path: "main.typ".to_owned()
    }));
    let message = error.to_string();
    for issue in issues {
        assert!(message.contains(&issue.to_string()));
    }
    // Control characters from the source are escaped.
    assert!(message.contains(r"/forged\npath.typ"));
    assert!(!message.contains("forged\npath"));
}

proptest! {
    #[test]
    fn snapshots_canonicalize_generated_paths(
        stem in "[a-z][a-z0-9]{0,15}",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let canonical = format!("chapters/{stem}.typ");
        let supplied = format!("./{canonical}");
        let snapshot = ProjectSnapshot::new(&supplied, [(supplied.as_str(), bytes.clone())]).unwrap();

        prop_assert_eq!(snapshot.entrypoint(), canonical.as_str());
        prop_assert_eq!(snapshot.file(&canonical), Some(bytes.as_slice()));

        let error = ProjectSnapshot::new(&canonical, [
            (canonical.as_str(), b"first".to_vec()),
            (supplied.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();
        prop_assert_eq!(error.issues.as_slice(), [PackIssue::DuplicatePath { path: canonical }]);
    }

    #[test]
    fn snapshots_are_invariant_under_entry_permutation(
        generated in prop::collection::btree_map(
            "[a-z][a-z0-9]{0,15}",
            prop::collection::vec(any::<u8>(), 0..64),
            0..16,
        ),
    ) {
        let mut entries = vec![("main.typ".to_owned(), b"main".to_vec())];
        entries.extend(generated.into_iter().map(|(stem, bytes)| (format!("files/{stem}.bin"), bytes)));
        let mut reversed = entries.clone();
        reversed.reverse();

        prop_assert_eq!(
            ProjectSnapshot::new("main.typ", entries).unwrap(),
            ProjectSnapshot::new("main.typ", reversed).unwrap()
        );
    }
}

// --- The archive format ----------------------------------------------------

fn simple_pack() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .file("data/table.csv", b"a,b\n1,2\n".to_vec())
        .package_file(spec("@local/inside:1.0.0"), "lib.typ", b"inside".to_vec())
        .external_package_file(spec("@local/outside:1.0.0"), "lib.typ", b"outside".to_vec())
        .build()
        .unwrap()
}

type Members = Vec<(String, Vec<u8>)>;

/// Every member of an archive, in archive order.
fn members(archive: &[u8]) -> Members {
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

/// Re-zips an archive with the zip crate's defaults after editing its members.
fn rewrite(archive: &[u8], edit: impl FnOnce(&mut Members)) -> Vec<u8> {
    let mut members = members(archive);
    edit(&mut members);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in members {
        if let Some(directory) = name.strip_suffix('/') {
            writer.add_directory(directory, options).unwrap();
        } else {
            writer.start_file(name, options).unwrap();
            writer.write_all(&data).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

fn edit_manifest(archive: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    rewrite(archive, |members| {
        let manifest = members
            .iter_mut()
            .find(|(name, _)| name == "typst-pack.toml");
        let manifest = manifest.unwrap();
        manifest.1 = edit(String::from_utf8(manifest.1.clone()).unwrap()).into_bytes();
    })
}

fn decode_default(archive: &[u8]) -> Result<Pack, DecodeError> {
    decode(archive, DecodeLimits::default())
}

type Files = Vec<(String, Vec<u8>)>;

/// Everything a Pack exposes, as one comparable value.
#[derive(Debug, PartialEq, Eq)]
struct Projection {
    identity: typst_pack::Digest,
    entrypoint: String,
    files: Files,
    package_requirements: Vec<typst_pack::package::Requirement>,
    packages: Vec<(String, Files)>,
    font_requirements: Vec<typst_pack::font::Requirement>,
    font_data: Vec<Option<Vec<u8>>>,
    metadata: Option<Metadata>,
}

fn projection(pack: &Pack) -> Projection {
    let owned = |(path, data): (&str, &[u8])| (path.to_owned(), data.to_vec());
    let fonts = pack.font_requirements();
    Projection {
        identity: pack.identity(),
        entrypoint: pack.entrypoint().to_owned(),
        files: pack.files().map(owned).collect(),
        package_requirements: pack.package_requirements().to_vec(),
        packages: pack
            .packages()
            .map(|(spec, tree)| (spec.to_string(), tree.files().map(owned).collect()))
            .collect(),
        font_requirements: fonts.to_vec(),
        font_data: fonts
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
            builder = builder.metadata(Metadata { name, description, authors });
        }
        let pack = builder.build().unwrap();

        let decoded = decode_default(&encode(&pack)).unwrap();

        prop_assert_eq!(projection(&decoded), projection(&pack));
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn encode_decode_preserves_font_containers_and_verifies_their_digests() {
    let regular = fonts::typst_container();
    let collection = fonts::font_collection(&[regular.clone(), regular.clone()]);
    for embedded in [true, false] {
        let builder = Pack::builder("main.typ").file("main.typ", b"Fonts".to_vec());
        let pack = if embedded {
            builder.font(collection.clone()).font(regular.clone())
        } else {
            builder
                .external_font(collection.clone())
                .external_font(regular.clone())
        };
        let pack = pack.build().unwrap();
        assert_eq!(pack.font_requirements()[0].faces.len(), 2);

        let archive = encode(&pack);
        assert_eq!(
            projection(&decode_default(&archive).unwrap()),
            projection(&pack)
        );

        if embedded {
            let tampered = rewrite(&archive, |members| {
                let font = members
                    .iter_mut()
                    .find(|(name, _)| name.starts_with("fonts/"));
                font.unwrap().1.push(0);
            });
            assert!(matches!(
                decode_default(&tampered),
                Err(DecodeError::DigestMismatch { .. })
            ));
        }
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn an_embedded_font_container_has_exactly_one_member_name() {
    let font = fonts::typst_container();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Fonts".to_vec())
        .font(font.clone())
        .build()
        .unwrap();
    let archive = encode(&pack);
    let real = members(&archive)
        .into_iter()
        .map(|(name, _)| name)
        .find(|name| name.starts_with("fonts/"))
        .unwrap();
    assert!(real.ends_with(".ttf") || real.ends_with(".otf"), "{real}");
    let stem = real.rsplit_once('.').unwrap().0.to_owned();
    let other_extension = if real.ends_with(".ttf") { "otf" } else { "ttf" };

    // Extra members named after the declared digest.
    for (name, data) in [
        (format!("{stem}.{other_extension}"), b"junk".to_vec()),
        (format!("{stem}.{other_extension}"), font.clone()),
        (format!("{stem}.d/evil.typ"), b"junk".to_vec()),
        (stem.clone(), font.clone()),
    ] {
        let result = decode_default(&rewrite(&archive, |members| {
            members.push((name.clone(), data))
        }));
        assert!(
            matches!(
                &result,
                Err(DecodeError::UndeclaredMember(found) | DecodeError::DuplicateMember(found))
                    if *found == name || *found == real
            ),
            "{name}: {result:?}"
        );
    }
    // The container under a name that does not follow from its content.
    for name in [
        format!("{stem}.exe"),
        format!("{stem}.{other_extension}"),
        stem.clone(),
    ] {
        let result = decode_default(&rewrite(&archive, |members| {
            let member = members.iter_mut().find(|(found, _)| *found == real);
            member.unwrap().0 = name.clone();
        }));
        assert!(
            matches!(&result, Err(DecodeError::UndeclaredMember(found)) if *found == name),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn encoding_is_deterministic() {
    let pack = simple_pack();
    let archive = encode(&pack);
    assert_eq!(encode(&pack), archive);
    assert_eq!(encode(&decode_default(&archive).unwrap()), archive);
}

#[test]
fn the_manifest_declares_every_member_by_digest() {
    let archive = encode(&simple_pack());
    let members = members(&archive);
    let manifest = String::from_utf8(members[0].1.clone()).unwrap();

    assert!(manifest.starts_with("format-version = 2\n"), "{manifest}");
    let generator = concat!("generator = \"typst-pack ", env!("CARGO_PKG_VERSION"), "\"");
    assert!(manifest.contains(generator));
    assert!(manifest.contains("typst-version = \"0.15"));
    assert!(manifest.contains("entrypoint = \"main.typ\""));
    assert!(manifest.contains("[[files]]\npath = \"data/table.csv\"\ndigest = \"sha256:"));
    assert!(manifest.contains("spec = \"@local/outside:1.0.0\""));
    assert!(manifest.contains("embedded = false"));
    let names = members
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
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
    let archive = edit_manifest(&encode(&pack), |manifest| {
        let manifest = manifest
            .replacen("entrypoint", "future-key = [1, 2]\nentrypoint", 1)
            .replace("[[files]]\n", "[[files]]\nfuture-entry-key = true\n");
        format!("{manifest}\n[future-table]\nkey = \"value\"\n")
    });
    // Directory members, as other writers produce.
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
fn decoding_rejects_non_packs_and_other_format_versions() {
    for bytes in [
        &b"not a zip"[..],
        b"",
        b"PK",
        b"PK\x03\x04\x14\0",
        b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    ] {
        assert!(
            matches!(
                decode_default(bytes),
                Err(DecodeError::Zip(_) | DecodeError::MissingManifest)
            ),
            "{bytes:?}"
        );
    }
    let no_manifest = rewrite(&encode(&simple_pack()), |members| {
        members.remove(0);
    });
    assert!(matches!(
        decode_default(&no_manifest),
        Err(DecodeError::MissingManifest)
    ));

    let archive = encode(&simple_pack());
    for version in ["1", "3", "0"] {
        let changed = edit_manifest(&archive, |manifest| {
            manifest.replace("format-version = 2", &format!("format-version = {version}"))
        });
        assert!(matches!(
            decode_default(&changed),
            Err(DecodeError::UnsupportedVersion(found)) if found.to_string() == version
        ));
    }
    let missing = edit_manifest(&archive, |manifest| {
        manifest.replace("format-version = 2\n", "")
    });
    assert!(matches!(
        decode_default(&missing),
        Err(DecodeError::MissingFormatVersion)
    ));
    let malformed = edit_manifest(&archive, |manifest| {
        manifest.replace("@local/outside:1.0.0", "not a spec")
    });
    // The TOML parser's error is the source, not part of the message.
    let error = decode_default(&malformed).unwrap_err();
    assert!(matches!(error, DecodeError::InvalidManifest(_)));
    assert_eq!(
        error.to_string(),
        "failed to parse the typst-pack.toml manifest"
    );
    let source = std::error::Error::source(&error).unwrap().to_string();
    assert!(source.contains("not a spec"), "{source}");
}

#[test]
fn decoding_rejects_unsafe_non_canonical_and_duplicate_member_names() {
    let archive = encode(&simple_pack());
    for name in [
        "../escape.typ",
        "project/../../escape.typ",
        "/project/main.typ",
        "C:/evil.typ",
        "a\\b",
        // Aliases unzip would extract elsewhere than the canonical name.
        "project/./main.typ",
        "x/../project/./data.txt",
        "./typst-pack.toml",
        "project//main.typ",
        "project/./",
    ] {
        let unsafe_archive = rewrite(&archive, |members| {
            members.push((name.to_owned(), Vec::new()))
        });
        assert!(
            matches!(
                decode_default(&unsafe_archive),
                Err(DecodeError::UnsafeMemberName(found)) if found == name
            ),
            "{name}"
        );
    }
    // The zip writer refuses duplicate names, so rename a member in place.
    let mut duplicated = rewrite(&archive, |members| {
        members.push(("project/main.tyX".to_owned(), b"Other".to_vec()));
    });
    let needle = b"project/main.tyX";
    for index in 0..=duplicated.len() - needle.len() {
        if &duplicated[index..index + needle.len()] == needle {
            duplicated[index + needle.len() - 1] = b'p';
        }
    }
    assert!(matches!(
        decode_default(&duplicated),
        Err(DecodeError::DuplicateMember(name)) if name == "project/main.typ"
    ));

    // An end record that under-counts the central directory is malformed,
    // not a duplicate.
    let mut undercounted = archive.clone();
    let end = undercounted
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .unwrap();
    for field in [end + 8, end + 10] {
        let count = u16::from_le_bytes([undercounted[field], undercounted[field + 1]]);
        undercounted[field..field + 2].copy_from_slice(&(count - 1).to_le_bytes());
    }
    let error = decode_default(&undercounted).unwrap_err();
    assert!(matches!(error, DecodeError::Zip(_)));
    assert_eq!(error.to_string(), "not a valid ZIP archive");
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn decoding_rejects_manifests_with_more_than_one_reading() {
    let archive = encode(&simple_pack());
    let other = typst_pack::Digest::of(b"other");
    let ambiguous = |edit: &dyn Fn(String) -> String| {
        let result = decode_default(&edit_manifest(&archive, edit));
        assert!(
            matches!(result, Err(DecodeError::AmbiguousManifest(_))),
            "{result:?}"
        );
    };

    // One path, package, or font container declared twice.
    ambiguous(&|manifest| {
        format!("{manifest}\n[[files]]\npath = \"main.typ\"\ndigest = \"{other}\"\n")
    });
    ambiguous(&|manifest| {
        format!(
            "{manifest}\n[[packages]]\nspec = \"@local/outside:1.0.0\"\ndigest = \"{other}\"\nembedded = false\n"
        )
    });
    ambiguous(&|manifest| {
        format!(
            "{manifest}\n[[fonts]]\ndigest = \"{other}\"\nembedded = false\n[[fonts]]\ndigest = \"{other}\"\nembedded = false\n"
        )
    });
    // Paths that only normalize to a declared member.
    ambiguous(&|manifest| {
        manifest.replace(
            "entrypoint = \"main.typ\"",
            "entrypoint = \"x/../main.typ\"",
        )
    });
    ambiguous(&|manifest| {
        manifest.replace("path = \"data/table.csv\"", "path = \"./data/table.csv\"")
    });

    // A specification is written in canonical form only.
    let spec = edit_manifest(&archive, |manifest| {
        manifest.replace("@local/outside:1.0.0", "@local/outside:1.00.0")
    });
    assert!(matches!(
        decode_default(&spec),
        Err(DecodeError::InvalidManifest(error)) if error.to_string().contains("canonical")
    ));
}

#[test]
fn decoding_reports_the_recorded_producer() {
    let pack = simple_pack();
    assert_eq!(
        pack.generator(),
        Some(concat!("typst-pack ", env!("CARGO_PKG_VERSION")))
    );
    let engine = pack.typst_version().unwrap();
    assert!(typst_pack::VERSION.ends_with(&format!("(Typst {engine})")));

    let archive = edit_manifest(&encode(&pack), |manifest| {
        let manifest = manifest.replace(
            &format!("typst-version = \"{engine}\""),
            "typst-version = \"0.99.0\"",
        );
        let generator = concat!(
            "generator = \"typst-pack ",
            env!("CARGO_PKG_VERSION"),
            "\"\n"
        );
        manifest.replace(generator, "")
    });
    let decoded = decode_default(&archive).unwrap();
    assert_eq!(decoded.typst_version(), Some("0.99.0"));
    assert_eq!(decoded.generator(), None);
}

#[test]
fn manifest_faces_are_read_leniently() {
    let archive = edit_manifest(&encode(&simple_pack()), |manifest| {
        format!(
            "{manifest}\n[[fonts]]\ndigest = \"{}\"\nembedded = false\n\n\
             [[fonts.faces]]\nfamily = \"Future Sans\"\nstyle = \"slanted\"\nweight = 950\nstretch = 875\nfuture-axis = 3\n\n\
             [[fonts.faces]]\nindex = 1\nstyle = \"italic\"\n",
            typst_pack::Digest::of(b"font")
        )
    });
    let pack = decode_default(&archive).unwrap();

    let faces = &pack.font_requirements()[0].faces;
    let face = |index, family: &str, style, weight, stretch| Face {
        index,
        family: family.to_owned(),
        style,
        weight: FontWeight::from_number(weight),
        stretch,
    };
    assert_eq!(
        *faces,
        [
            face(
                0,
                "Future Sans",
                FontStyle::Normal,
                900,
                FontStretch::SEMI_CONDENSED
            ),
            face(1, "", FontStyle::Italic, 400, FontStretch::NORMAL),
        ]
    );
    let manifest = String::from_utf8(members(&encode(&pack))[0].1.clone()).unwrap();
    assert!(
        manifest.contains(
            "family = \"Future Sans\"\nstyle = \"normal\"\nweight = 900\nstretch = 875\n"
        ),
        "{manifest}"
    );
}

#[test]
fn decoding_accepts_unbounded_limits() {
    let archive = encode(&simple_pack());
    let unbounded = DecodeLimits {
        max_archive_bytes: u64::MAX,
        max_members: u64::MAX,
        max_member_bytes: u64::MAX,
        max_total_bytes: u64::MAX,
    };
    assert_eq!(
        decode(&archive, unbounded).unwrap().identity(),
        simple_pack().identity()
    );
    assert!(archive::read(Cursor::new(&archive), unbounded).is_ok());
}

#[test]
fn package_trees_must_declare_their_specification() {
    let declaration =
        "[package]\nname = \"inside\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n";
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .package_file(spec("@local/inside:1.0.0"), "typst.toml", declaration)
        .build()
        .unwrap();

    let wrong = declaration.replace("1.0.0", "2.0.0");
    let error = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .package_file(spec("@local/inside:1.0.0"), "typst.toml", wrong.clone())
        .build()
        .unwrap_err();
    assert!(
        matches!(error.issues.as_slice(), [PackIssue::InvalidDeclaration { spec: found, .. }] if *found == spec("@local/inside:1.0.0")),
        "{error}"
    );

    // An archive whose manifest declares the edited tree's digest is refused
    // the same way.
    let tree = typst_pack::package::Tree::new([("typst.toml", wrong.clone())]).unwrap();
    let original = pack.package_requirements()[0].digest.to_string();
    let archive = rewrite(&encode(&pack), |members| {
        for (name, data) in members.iter_mut() {
            if name == "typst-pack.toml" {
                let manifest = String::from_utf8(data.clone()).unwrap();
                *data = manifest
                    .replace(&original, &tree.digest().to_string())
                    .into_bytes();
            } else if name.ends_with("typst.toml") {
                *data = wrong.clone().into_bytes();
            }
        }
    });
    assert!(matches!(
        decode_default(&archive),
        Err(DecodeError::InvalidPack(error))
            if matches!(error.issues.as_slice(), [PackIssue::InvalidDeclaration { .. }])
    ));
}

#[test]
fn decoding_rejects_links() {
    let archive = encode(&simple_pack());
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in members(&archive) {
        writer.start_file(name, options).unwrap();
        writer.write_all(&data).unwrap();
    }
    writer
        .add_symlink("project/link.typ", "/etc/passwd", options)
        .unwrap();
    let linked = writer.finish().unwrap().into_inner();

    assert!(matches!(
        decode_default(&linked),
        Err(DecodeError::UnsupportedMemberKind(name)) if name == "project/link.typ"
    ));
}

#[test]
fn decoding_requires_exactly_the_declared_members_with_their_digests() {
    let archive = encode(&simple_pack());
    let decoded = |edit: &dyn Fn(&mut Members)| decode_default(&rewrite(&archive, edit));
    let push = |name: &'static str, data: &'static [u8]| {
        move |members: &mut Members| members.push((name.to_owned(), data.to_vec()))
    };

    for name in [
        "project/extra.typ",
        "README.md",
        "packages/local/outside/1.0.0/lib.typ",
    ] {
        assert!(
            matches!(decoded(&push(name, b"extra")), Err(DecodeError::UndeclaredMember(found)) if found == name),
            "{name}"
        );
    }
    let missing = decoded(&|members| members.retain(|(name, _)| name != "project/data/table.csv"));
    assert!(matches!(missing, Err(DecodeError::MissingMember(_))));
    let missing = decoded(&|members| members.retain(|(name, _)| !name.starts_with("packages/")));
    assert!(matches!(
        missing,
        Err(DecodeError::MissingMember(name)) if name.contains("@local/inside:1.0.0")
    ));

    for target in ["project/main.typ", "packages/local/inside/1.0.0/lib.typ"] {
        let tampered = decoded(&|members| {
            let member = members.iter_mut().find(|(name, _)| name == target);
            member.unwrap().1 = b"tampered".to_vec();
        });
        assert!(
            matches!(tampered, Err(DecodeError::DigestMismatch { .. })),
            "{target}"
        );
    }
    let extra = decoded(&push("packages/local/inside/1.0.0/extra.typ", b""));
    assert!(matches!(extra, Err(DecodeError::DigestMismatch { .. })));
}

#[test]
fn decoding_is_bounded_by_actual_inflation() {
    let archive = encode(&simple_pack());
    let base = DecodeLimits::default();
    let limited = |limits| matches!(decode(&archive, limits), Err(DecodeError::Limit(_)));
    let length = archive.len() as u64;
    assert!(limited(DecodeLimits {
        max_archive_bytes: length - 1,
        ..base
    }));
    assert!(limited(DecodeLimits {
        max_members: 3,
        ..base
    }));
    assert!(limited(DecodeLimits {
        max_member_bytes: 6,
        ..base
    }));
    assert!(limited(DecodeLimits {
        max_total_bytes: 20,
        ..base
    }));
    let exact = DecodeLimits {
        max_archive_bytes: length,
        max_members: 4,
        ..base
    };
    assert!(decode(&archive, exact).is_ok());

    // A member declaring one byte that inflates to a megabyte.
    let pack = Pack::builder("main.typ")
        .file("main.typ", vec![b'a'; 1 << 20])
        .build()
        .unwrap();
    let mut dishonest = encode(&pack);
    let name = b"project/main.typ";
    for index in 0..=dishonest.len() - name.len() {
        if &dishonest[index..index + name.len()] == name {
            // The size fields of the local and central headers.
            for (offset, signature, size) in [(30, b"PK\x03\x04", 22), (46, b"PK\x01\x02", 24)] {
                if index >= offset && dishonest[index - offset..].starts_with(signature) {
                    let field = index - offset + size;
                    dishonest[field..field + 4].copy_from_slice(&1u32.to_le_bytes());
                }
            }
        }
    }
    let limits = DecodeLimits {
        max_member_bytes: 1024,
        ..base
    };
    assert!(matches!(
        decode(&dishonest, limits),
        Err(DecodeError::Limit(_) | DecodeError::UnreadableMember { .. })
    ));
}

struct Failing;

impl Read for Failing {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted read failure"))
    }
}

impl Write for Failing {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted write failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streams_are_read_bounded_and_written_whole() {
    let pack = simple_pack();
    let archive = encode(&pack);
    let limits = |max_archive_bytes| DecodeLimits {
        max_archive_bytes,
        ..DecodeLimits::default()
    };
    let length = archive.len() as u64;
    let read = archive::read(Cursor::new(&archive), limits(length)).unwrap();
    assert_eq!(read.identity(), pack.identity());
    assert!(matches!(
        archive::read(Cursor::new(&archive), limits(length - 1)),
        Err(DecodeError::Limit(_))
    ));
    assert!(matches!(
        archive::read(Failing, limits(length)),
        Err(DecodeError::Io(_))
    ));

    let mut output = Vec::new();
    archive::write(&mut output, &pack).unwrap();
    assert_eq!(output, archive);
    assert!(archive::write(Failing, &pack).is_err());
}

#[cfg(feature = "fs")]
#[test]
fn files_are_saved_and_opened_bounded() {
    use typst_pack::fs::{self, WriteError, WritePolicy};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("archive.typk");
    let pack = simple_pack();
    fs::write_archive(&path, &pack, WritePolicy::CreateNew).unwrap();
    assert!(matches!(
        fs::write_archive(&path, &pack, WritePolicy::CreateNew),
        Err(WriteError::Io { .. })
    ));
    fs::write_archive(&path, &pack, WritePolicy::Overwrite).unwrap();
    let opened = fs::read_archive(&path, DecodeLimits::default()).unwrap();
    assert_eq!(opened.identity(), pack.identity());

    let length = encode(&pack).len() as u64 - 1;
    let limits = DecodeLimits {
        max_archive_bytes: length,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        fs::read_archive(&path, limits),
        Err(DecodeError::Limit(_))
    ));
    assert!(matches!(
        fs::read_archive(directory.path().join("missing.typk"), DecodeLimits::default()),
        Err(DecodeError::Io(error)) if error.kind() == io::ErrorKind::NotFound
    ));
}

// --- Extraction planning ---------------------------------------------------

const BASE: &str = "packages/local/example/1.0.0";

fn example() -> PackageSpec {
    spec("@local/example:1.0.0")
}

fn entries(pack: &Pack, packages: bool, fonts: bool) -> Vec<(String, Vec<u8>)> {
    let plan = extract::plan(pack, Selection { packages, fonts }).unwrap();
    let entries = plan.entries().iter();
    entries
        .map(|entry| (entry.path().to_owned(), entry.bytes().to_vec()))
        .collect()
}

fn conflicts(pack: &Pack) -> Vec<Conflict> {
    let selection = Selection {
        packages: true,
        fonts: false,
    };
    extract::plan(pack, selection).unwrap_err().issues
}

#[test]
fn plans_hold_the_project_and_selected_embedded_trees_in_path_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file("assets/logo.bin", b"logo".to_vec())
        .package_file(example(), "lib.typ", b"package".to_vec())
        .external_package_file(
            spec("@local/outside:1.0.0"),
            "lib.typ",
            b"external".to_vec(),
        )
        .build()
        .unwrap();
    let owned = |path: &str, data: &[u8]| (path.to_owned(), data.to_vec());

    // External trees are never planned.
    assert_eq!(
        entries(&pack, true, true),
        [
            owned("assets/logo.bin", b"logo"),
            owned("main.typ", b"main"),
            owned(&format!("{BASE}/lib.typ"), b"package"),
        ]
    );
    assert_eq!(entries(&pack, false, true), entries(&pack, true, true)[..2]);

    let plan = extract::plan(
        &pack,
        Selection {
            packages: true,
            fonts: true,
        },
    )
    .unwrap();
    let kinds = plan.entries().iter().map(|entry| entry.kind().to_string());
    assert_eq!(
        kinds.collect::<Vec<_>>(),
        ["project file", "project file", "package file"]
    );
}

#[test]
fn planning_aggregates_collisions_by_kind_then_path() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file(format!("{BASE}/z.typ"), b"project z".to_vec())
        .file(format!("{BASE}/a.typ"), b"project a".to_vec())
        .file(format!("{BASE}/b/descendant"), b"project b".to_vec())
        .package_file(example(), "z.typ", b"package z".to_vec())
        .package_file(example(), "a.typ", b"package a".to_vec())
        .package_file(example(), "b", b"package b".to_vec())
        .build()
        .unwrap();

    let error = extract::plan(
        &pack,
        Selection {
            packages: true,
            fonts: false,
        },
    )
    .unwrap_err();

    use typst_pack::extract::EntryKind::{PackageFile, ProjectFile};
    let found = error
        .issues
        .iter()
        .map(|conflict| (conflict.kind, conflict.path.as_str()));
    assert_eq!(
        found.collect::<Vec<_>>(),
        [
            (ProjectFile, "packages/local/example/1.0.0/a.typ"),
            (ProjectFile, "packages/local/example/1.0.0/z.typ"),
            (PackageFile, "packages/local/example/1.0.0/b"),
        ]
    );
    let message = error.to_string();
    assert!(
        message.contains("a.typ") && message.contains("z.typ"),
        "{message}"
    );
}

proptest! {
    #[test]
    fn planning_is_invariant_under_input_permutation(
        generated in prop::collection::btree_map(
            "[a-z][a-z0-9]{0,12}",
            prop::collection::vec(any::<u8>(), 0..64),
            0..16,
        ),
        packages in any::<bool>(),
        fonts in any::<bool>(),
    ) {
        let mut files = vec![("main.typ".to_owned(), b"main".to_vec())];
        files.extend(generated.into_iter().map(|(stem, bytes)| (format!("files/{stem}.bin"), bytes)));
        let build = |files: Vec<(String, Vec<u8>)>| {
            let mut builder = Pack::builder("main.typ");
            for (path, bytes) in files {
                builder = builder.file(path, bytes);
            }
            builder.package_file(example(), "lib.typ", b"package".to_vec()).build().unwrap()
        };
        let mut reversed = files.clone();
        reversed.reverse();

        prop_assert_eq!(
            entries(&build(files), packages, fonts),
            entries(&build(reversed), packages, fonts)
        );
    }

    #[test]
    fn exact_and_tree_collisions_conflict_but_segment_prefixes_do_not(
        stem in "[a-z][a-z0-9]{0,12}",
        kind in 0u8..4,
    ) {
        let package_path = format!("{BASE}/{stem}");
        let project_path = match kind {
            0 => package_path.clone(),
            1 => BASE.to_owned(),
            2 => format!("{package_path}/descendant"),
            _ => format!("{package_path}-sibling"),
        };
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file(project_path, b"project".to_vec())
            .package_file(example(), stem, b"package".to_vec())
            .build().unwrap();

        let plan = extract::plan(&pack, Selection { packages: true, fonts: false });
        match kind {
            3 => prop_assert_eq!(plan.unwrap().entries().len(), 3),
            _ => prop_assert_eq!(conflicts(&pack).len(), 1),
        }
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn embedded_font_containers_are_extracted_once_when_selected() {
    let face = fonts::typst_container();
    let collection = fonts::font_collection(&[face.clone(), face.clone()]);
    let build = |embedded: bool| {
        let builder = Pack::builder("main.typ").file("main.typ", b"main".to_vec());
        let builder = if embedded {
            builder.font(collection.clone()).font(face.clone())
        } else {
            builder.external_font(collection.clone())
        };
        builder.build().unwrap()
    };

    assert_eq!(entries(&build(true), false, false).len(), 1);
    let included = entries(&build(true), false, true);
    let fonts = included
        .iter()
        .filter(|(path, _)| path.starts_with("fonts/"));
    let mut fonts = fonts.map(|(_, data)| data.clone()).collect::<Vec<_>>();
    fonts.sort();
    let mut expected = vec![collection.clone(), face.clone()];
    expected.sort();
    assert_eq!(fonts, expected);
    assert_eq!(entries(&build(false), false, true).len(), 1);
}
