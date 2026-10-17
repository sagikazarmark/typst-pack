//! Package trees, the package catalog, and the transport-free registry
//! helpers.

use proptest::prelude::*;
use typst::syntax::package::PackageSpec;
use typst_pack::{PackIssue, package};

fn spec(text: &str) -> PackageSpec {
    text.parse().unwrap()
}

fn manifest(name: &str, version: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nentrypoint = \"lib.typ\"\n")
}

fn tree(files: &[(&str, &str)]) -> package::Tree {
    package::Tree::new(
        files
            .iter()
            .map(|(path, text)| (*path, text.as_bytes().to_vec())),
    )
    .unwrap()
}

#[test]
fn tree_paths_must_name_package_files() {
    for path in [
        "",
        "/absolute.typ",
        "../escape.typ",
        "C:/drive.typ",
        "back\\slash.typ",
    ] {
        let error = package::Tree::new([(path, b"content".to_vec())]).unwrap_err();
        assert!(
            matches!(error.issues.as_slice(), [PackIssue::InvalidPath { path: reported, .. }] if reported == path),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn tree_issues_are_aggregated_in_canonical_order() {
    let entries = vec![
        ("dir/second.typ", b"second".to_vec()),
        ("same.typ", b"first".to_vec()),
        ("../escape.typ", b"invalid".to_vec()),
        ("./same.typ", b"duplicate".to_vec()),
        ("dir", b"ancestor".to_vec()),
        ("dir/first.typ", b"first".to_vec()),
    ];
    let mut reversed = entries.clone();
    reversed.reverse();
    let error = package::Tree::new(entries).unwrap_err();

    assert_eq!(error, package::Tree::new(reversed).unwrap_err());
    let conflict = |descendant: &str| PackIssue::PathTreeConflict {
        ancestor: "dir".to_owned(),
        descendant: descendant.to_owned(),
    };
    assert_eq!(
        error.issues[..2],
        [conflict("dir/first.typ"), conflict("dir/second.typ")]
    );
    assert!(
        matches!(&error.issues[2], PackIssue::InvalidPath { path, .. } if path == "../escape.typ")
    );
    assert_eq!(
        error.issues[3],
        PackIssue::DuplicatePath {
            path: "same.typ".to_owned()
        }
    );
    assert_eq!(error.issues.len(), 4);
}

#[test]
fn trees_are_canonical() {
    let tree = package::Tree::new([
        ("./src/lib.typ", b"library".to_vec()),
        ("README.md", b"read me".to_vec()),
    ])
    .unwrap();

    let files = tree.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(files, ["README.md", "src/lib.typ"]);
    let same = [("README.md", "read me"), ("src/lib.typ", "library")];
    assert_eq!(tree.digest(), self::tree(&same).digest());
}

proptest! {
    #[test]
    fn tree_identity_and_order_are_invariant_under_input_permutation(
        generated in prop::collection::btree_map(
            "[a-z][a-z0-9]{0,15}",
            prop::collection::vec(any::<u8>(), 0..64),
            0..16,
        ),
    ) {
        let entries = generated
            .into_iter()
            .map(|(stem, bytes)| (format!("files/{stem}.bin"), bytes))
            .collect::<Vec<_>>();
        let mut reversed = entries.clone();
        reversed.reverse();

        let forward = package::Tree::new(entries).unwrap();
        let backward = package::Tree::new(reversed).unwrap();

        prop_assert_eq!(forward.digest(), backward.digest());
        prop_assert_eq!(forward.files().collect::<Vec<_>>(), backward.files().collect::<Vec<_>>());
        let paths = forward.files().map(|(path, _)| path).collect::<Vec<_>>();
        prop_assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn tree_identity_binds_generated_paths_and_exact_bytes(
        stem in "[a-z][a-z0-9]{0,15}",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
        replacement in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        prop_assume!(bytes != replacement);
        let path = format!("files/{stem}.bin");
        let baseline = package::Tree::new([(path.clone(), bytes.clone())]).unwrap();
        let changed_bytes = package::Tree::new([(path, replacement)]).unwrap();
        let changed_path = package::Tree::new([(format!("renamed/{stem}.bin"), bytes)]).unwrap();

        prop_assert_ne!(baseline.digest(), changed_bytes.digest());
        prop_assert_ne!(baseline.digest(), changed_path.digest());
    }

    #[test]
    fn tree_rejects_generated_canonical_duplicates(stem in "[a-z][a-z0-9]{0,15}") {
        let canonical = format!("files/{stem}.typ");
        let alias = format!("./{canonical}");
        let error = package::Tree::new([
            (canonical.as_str(), b"first".to_vec()),
            (alias.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();

        prop_assert_eq!(error.issues, &[PackIssue::DuplicatePath { path: canonical }]);
    }
}

#[test]
fn a_catalog_rejects_a_duplicate_exact_specification() {
    let duplicate = spec("@local/example:1.0.0");
    let example = || tree(&[("typst.toml", &manifest("example", "1.0.0"))]);
    let mut catalog = package::Catalog::new();
    catalog.insert(duplicate.clone(), example(), true).unwrap();

    let error = catalog
        .insert(duplicate.clone(), example(), false)
        .unwrap_err();

    assert_eq!(
        error.issues,
        [PackIssue::DuplicatePackage { spec: duplicate }]
    );
}

/// A tree that does not declare its specification is a distinct failure,
/// not a missing package, whether or not any compile reads it.
#[test]
fn a_catalog_checks_each_tree_declares_its_specification() {
    let problem = |manifest: &str| {
        let files = [("typst.toml", manifest), ("lib.typ", "body")];
        let files = if manifest.is_empty() {
            &files[1..]
        } else {
            &files[..]
        };
        let error = package::Catalog::new()
            .insert(spec("@local/claimed:1.0.0"), tree(files), true)
            .unwrap_err();
        match error.issues.as_slice() {
            [
                PackIssue::InvalidDeclaration {
                    spec: reported,
                    message,
                },
            ] if reported == &spec("@local/claimed:1.0.0") => message.clone(),
            issues => panic!("unexpected issues {issues:?}"),
        }
    };

    assert_eq!(
        problem(&manifest("other", "1.0.0")),
        "declares the name \"other\""
    );
    assert_eq!(
        problem(&manifest("claimed", "2.0.0")),
        "declares the version 2.0.0"
    );
    assert_eq!(
        problem(&manifest("other", "2.0.0")),
        "declares the name \"other\" and the version 2.0.0"
    );
    problem("");
    problem("[package\nname =");
    let not_utf8 = package::Tree::new([("typst.toml", vec![0xff])]).unwrap();
    assert!(
        package::Catalog::new()
            .insert(spec("@local/claimed:1.0.0"), not_utf8, true)
            .is_err()
    );

    // Whatever else a declaration holds, a matching one is accepted.
    let full = format!(
        "{}authors = [\"Author\"]\nexclude = [\"tests/**\"]\n\n\
         [template]\npath = \"template\"\nentrypoint = \"main.typ\"\n\n\
         [tool.some-tool]\nkey = \"value\"\n",
        manifest("claimed", "1.0.0")
    );
    let accepted = tree(&[("typst.toml", &full)]);
    package::Catalog::new()
        .insert(spec("@local/claimed:1.0.0"), accepted, true)
        .unwrap();
}

#[cfg(feature = "package-reading")]
mod registry {
    use std::io::Write;

    use typst_pack::package::registry::{ExpandError, archive_url, expand_archive};
    use typst_pack::{PackIssue, ProjectSnapshot, ReadLimits, create};

    use super::{manifest, spec};

    type Builder = tar::Builder<flate2::write::GzEncoder<Vec<u8>>>;

    fn builder() -> Builder {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        tar::Builder::new(encoder)
    }

    fn finish(builder: Builder) -> Vec<u8> {
        builder.into_inner().unwrap().finish().unwrap()
    }

    /// The gzip-compressed tar a registry serves, files at the archive root.
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = builder();
        for (path, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            builder.append_data(&mut header, path, *data).unwrap();
        }
        finish(builder)
    }

    /// An archive with one member whose raw header the test shapes.
    fn raw_member(shape: impl FnOnce(&mut tar::Header), data: &[u8]) -> Vec<u8> {
        let mut builder = builder();
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        shape(&mut header);
        header.set_cksum();
        builder.append(&header, data).unwrap();
        finish(builder)
    }

    fn expand(bytes: &[u8]) -> Result<typst_pack::package::Tree, ExpandError> {
        expand_archive(bytes, ReadLimits::default())
    }

    #[test]
    fn only_preview_specifications_have_a_registry_url() {
        assert_eq!(
            archive_url(&spec("@preview/nested-name:10.0.1")).unwrap(),
            "https://packages.typst.org/preview/nested-name-10.0.1.tar.gz"
        );
        for text in [
            "@local/example:1.2.3",
            "@custom/example:1.2.3",
            "@Preview/example:1.2.3",
        ] {
            assert_eq!(archive_url(&spec(text)), None, "{text}");
        }
    }

    #[test]
    fn an_archive_expands_into_the_complete_tree() {
        let declaration = manifest("example", "1.0.0");
        let bytes = archive(&[
            ("typst.toml", declaration.as_bytes()),
            ("./lib.typ", b"#let value = 1"),
            ("assets/logo.svg", b"<svg/>"),
        ]);

        let tree = expand(&bytes).unwrap();

        assert_eq!(
            tree.files().collect::<Vec<_>>(),
            [
                ("assets/logo.svg", &b"<svg/>"[..]),
                ("lib.typ", &b"#let value = 1"[..]),
                ("typst.toml", declaration.as_bytes()),
            ]
        );
    }

    #[test]
    fn long_names_padding_and_directories_are_understood() {
        let long = format!("{}/lib.typ", "nested".repeat(30));
        let mut gnu = builder();
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        gnu.append_data(&mut header, &long, &b"g"[..]).unwrap();
        // Directories are not package files.
        let mut directory = tar::Header::new_gnu();
        directory.set_entry_type(tar::EntryType::Directory);
        directory.set_size(0);
        gnu.append_data(&mut directory, "assets", &[][..]).unwrap();
        let mut pax = builder();
        pax.append_pax_extensions([("path", long.as_bytes())])
            .unwrap();
        let mut header = tar::Header::new_ustar();
        header.set_size(1);
        pax.append_data(&mut header, "short", &b"p"[..]).unwrap();

        let tree = expand(&finish(gnu)).unwrap();
        assert_eq!(
            tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
            [long.as_str()]
        );
        assert_eq!(expand(&finish(pax)).unwrap().file(&long), Some(&b"p"[..]));

        // Nonzero padding after a member's data is accepted.
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
        bytes[513] = 0xff;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&bytes).unwrap();
        let tree = expand(&encoder.finish().unwrap()).unwrap();
        assert_eq!(tree.file("lib.typ"), Some(&b"x"[..]));
    }

    #[test]
    fn invalid_trees_escaping_names_and_other_member_kinds_are_rejected() {
        for files in [
            &[("lib.typ", &b"first"[..]), ("./lib.typ", b"second")][..],
            &[("lib", &b"file"[..]), ("lib/nested.typ", b"descendant")][..],
        ] {
            assert!(matches!(
                expand(&archive(files)),
                Err(ExpandError::Invalid(_))
            ));
        }
        for escaping in [&b"../escape.typ"[..], b"/absolute.typ"] {
            let name = |header: &mut tar::Header| {
                header.as_gnu_mut().unwrap().name[..escaping.len()].copy_from_slice(escaping)
            };
            let error = expand(&raw_member(name, b"#let value = 1")).unwrap_err();
            assert!(matches!(
                &error,
                ExpandError::Invalid(source)
                    if matches!(source.issues.as_slice(), [PackIssue::InvalidPath { .. }])
            ));
        }
        for kind in [
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Fifo,
            tar::EntryType::Char,
        ] {
            let mut builder = builder();
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
                matches!(&error, ExpandError::Malformed(message) if message.contains("secrets")),
                "{kind:?}: {error}"
            );
        }
    }

    #[test]
    fn malformed_bytes_and_names_are_rejected() {
        let non_utf8 = raw_member(|header| header.as_gnu_mut().unwrap().name[0] = 0xff, b"x");
        for bytes in [
            &b"<!doctype html><title>404</title>"[..],
            b"",
            b"\x1f\x8b",
            b"\x1f\x8b\x08\xff\0\0\0\0\0\0",
            &non_utf8,
        ] {
            assert!(
                matches!(expand(bytes), Err(ExpandError::Malformed(_))),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn expansion_is_bounded_even_against_bombs() {
        let bytes = archive(&[("a.typ", b"12345"), ("b.typ", b"12345"), ("c.typ", b"1")]);
        let fits = ReadLimits {
            max_entries: 3,
            max_file_bytes: 5,
            max_total_bytes: 11,
        };
        assert!(expand_archive(&bytes, fits).is_ok());
        let exceeded = |change: fn(&mut ReadLimits)| {
            let mut limits = fits;
            change(&mut limits);
            let result = expand_archive(&bytes, limits);
            assert!(matches!(result, Err(ExpandError::Limit(_))), "{result:?}");
        };
        exceeded(|l| l.max_entries = 2);
        exceeded(|l| l.max_file_bytes = 4);
        exceeded(|l| l.max_total_bytes = 10);

        let limits = ReadLimits {
            max_entries: 10,
            max_file_bytes: 1024,
            max_total_bytes: 4096,
        };
        let bomb = archive(&[("bomb.typ", &vec![0; 64 << 20])]);
        assert!(bomb.len() < 1 << 20);
        assert!(matches!(
            expand_archive(&bomb, limits),
            Err(ExpandError::Limit(_))
        ));
        // A long-name extension is cut off before it is materialized.
        let mut builder = builder();
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        builder
            .append_data(&mut header, "n".repeat(16 << 20), &b"x"[..])
            .unwrap();
        assert!(matches!(
            expand_archive(&finish(builder), limits),
            Err(ExpandError::Malformed(_))
        ));
    }

    #[test]
    fn a_resume_loop_fetches_and_expands_what_creation_reported() {
        // A stand-in for whatever transport the caller's host provides.
        let fetch = |url: &str| match url {
            "https://packages.typst.org/preview/example-1.0.0.tar.gz" => archive(&[
                ("typst.toml", manifest("example", "1.0.0").as_bytes()),
                (
                    "lib.typ",
                    b"#import \"@preview/nested:2.0.0\": inner\n#let value = 1 + inner",
                ),
                ("README.md", b"Not read by the representative request."),
            ]),
            "https://packages.typst.org/preview/nested-2.0.0.tar.gz" => archive(&[
                ("typst.toml", manifest("nested", "2.0.0").as_bytes()),
                ("lib.typ", b"#let inner = 2"),
            ]),
            url => panic!("the registry serves nothing at {url}"),
        };
        let source = "#import \"@preview/example:1.0.0\": value\n#rect(width: value * 1pt)";
        let project = ProjectSnapshot::new("main.typ", [("main.typ", source)]).unwrap();
        let mut request = create::Request::new(project);

        let pack = loop {
            match create(&request).unwrap() {
                create::Outcome::Created(created) => break created.pack,
                create::Outcome::MissingPackages(missing) => {
                    for spec in missing {
                        let tree = expand(&fetch(&archive_url(&spec).unwrap())).unwrap();
                        request.packages.insert(spec, tree, true).unwrap();
                    }
                }
            }
        };

        let specs = pack
            .package_requirements()
            .iter()
            .map(|r| r.spec.to_string());
        assert_eq!(
            specs.collect::<Vec<_>>(),
            ["@preview/example:1.0.0", "@preview/nested:2.0.0"]
        );
        let (_, example) = pack.packages().next().unwrap();
        assert!(example.file("README.md").is_some());
    }
}
