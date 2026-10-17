//! Package trees and the package catalog.
//!
//! Every test here runs on a build with no crate feature enabled.

use proptest::prelude::*;
use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::PackIssue;
use typst_pack::package;

fn spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:1.0.0")).unwrap()
}

fn package_tree(name: &str, body: &[u8]) -> package::Tree {
    package::Tree::new([
        (
            "typst.toml",
            format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n"
            )
            .into_bytes(),
        ),
        ("lib.typ", body.to_vec()),
    ])
    .unwrap()
}

#[test]
fn package_tree_construction_rejects_invalid_paths() {
    for path in [
        "",
        "/absolute.typ",
        "../escape.typ",
        "C:/drive.typ",
        "back\\slash.typ",
    ] {
        let error = package::Tree::new([(path, b"content".to_vec())]).unwrap_err();

        assert!(
            error.issues.iter().any(
                |issue| matches!(issue, PackIssue::InvalidPath { path: reported, .. } if reported == path)
            ),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn package_tree_construction_rejects_duplicate_canonical_paths() {
    let error = package::Tree::new([
        ("lib.typ", b"first".to_vec()),
        ("./lib.typ", b"second".to_vec()),
    ])
    .unwrap_err();

    assert_eq!(
        error.issues,
        &[PackIssue::DuplicatePath {
            path: "lib.typ".to_owned(),
        }]
    );
}

#[test]
fn package_tree_construction_rejects_file_ancestor_conflicts() {
    let error = package::Tree::new([
        ("assets", b"file".to_vec()),
        ("assets/logo.svg", b"image".to_vec()),
    ])
    .unwrap_err();

    assert_eq!(
        error.issues,
        &[PackIssue::PathTreeConflict {
            ancestor: "assets".to_owned(),
            descendant: "assets/logo.svg".to_owned(),
        }]
    );
}

#[test]
fn package_tree_construction_aggregates_issues_in_canonical_order() {
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
    let reversed_error = package::Tree::new(reversed).unwrap_err();

    assert_eq!(error, reversed_error);
    assert_eq!(error.issues.len(), 4);
    assert_eq!(
        &error.issues[..2],
        &[
            PackIssue::PathTreeConflict {
                ancestor: "dir".to_owned(),
                descendant: "dir/first.typ".to_owned(),
            },
            PackIssue::PathTreeConflict {
                ancestor: "dir".to_owned(),
                descendant: "dir/second.typ".to_owned(),
            },
        ]
    );
    assert!(matches!(
        &error.issues[2],
        PackIssue::InvalidPath { path, .. } if path == "../escape.typ"
    ));
    assert_eq!(
        error.issues[3],
        PackIssue::DuplicatePath {
            path: "same.typ".to_owned(),
        }
    );
}

#[test]
fn package_tree_exposes_canonical_order_and_digest() {
    let tree = package::Tree::new([
        ("./src/lib.typ", b"library".to_vec()),
        ("README.md", b"read me".to_vec()),
    ])
    .unwrap();

    assert_eq!(
        tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["README.md", "src/lib.typ"]
    );
    assert_eq!(
        tree.digest(),
        package::Tree::new([
            ("README.md", b"read me".to_vec()),
            ("src/lib.typ", b"library".to_vec()),
        ])
        .unwrap()
        .digest()
    );
}

proptest! {
    #[test]
    fn package_tree_identity_and_order_are_invariant_under_input_permutation(
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
    fn package_tree_identity_binds_generated_paths_and_exact_bytes(
        stem in "[a-z][a-z0-9]{0,15}",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
        replacement in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        prop_assume!(bytes != replacement);
        let path = format!("files/{stem}.bin");
        let renamed = format!("renamed/{stem}.bin");
        let baseline = package::Tree::new([(path, bytes.clone())]).unwrap();
        let changed_bytes = package::Tree::new([(
            format!("files/{stem}.bin"),
            replacement,
        )])
        .unwrap();
        let changed_path = package::Tree::new([(renamed, bytes)]).unwrap();

        prop_assert_ne!(baseline.digest(), changed_bytes.digest());
        prop_assert_ne!(baseline.digest(), changed_path.digest());
    }

    #[test]
    fn package_tree_rejects_generated_canonical_duplicates(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let canonical = format!("files/{stem}.typ");
        let alias = format!("./{canonical}");
        let error = package::Tree::new([
            (canonical.as_str(), b"first".to_vec()),
            (alias.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();

        prop_assert_eq!(
            error.issues,
            &[PackIssue::DuplicatePath { path: canonical }],
        );
    }
}

#[test]
fn a_catalog_rejects_a_duplicate_exact_specification() {
    let duplicate = spec("example");
    let mut catalog = package::Catalog::new();
    catalog
        .insert(duplicate.clone(), package_tree("example", b"first"), true)
        .unwrap();
    let error = catalog
        .insert(duplicate.clone(), package_tree("example", b"second"), false)
        .unwrap_err();

    assert_eq!(
        error.issues,
        [PackIssue::DuplicatePackage { spec: duplicate }]
    );
}

/// The `typst.toml` problem the catalog reports for `tree` under `claimed`.
fn declaration_problem(tree: package::Tree) -> String {
    let error = package::Catalog::new()
        .insert(spec("claimed"), tree, true)
        .unwrap_err();
    match error.issues.as_slice() {
        [
            PackIssue::InvalidDeclaration {
                spec: reported,
                message,
            },
        ] if reported == &spec("claimed") => message.clone(),
        issues => panic!("unexpected issues {issues:?}"),
    }
}

#[test]
fn a_catalog_checks_the_declared_name_and_version() {
    let wrong_version = package::Tree::new([
        (
            "typst.toml",
            b"[package]\nname = \"claimed\"\nversion = \"2.0.0\"\nentrypoint = \"lib.typ\"\n"
                .to_vec(),
        ),
        ("lib.typ", b"body".to_vec()),
    ])
    .unwrap();
    let wrong_both = package::Tree::new([(
        "typst.toml",
        b"[package]\nname = \"other\"\nversion = \"2.0.0\"\n".to_vec(),
    )])
    .unwrap();

    assert_eq!(
        declaration_problem(package_tree("other", b"body")),
        "declares the name \"other\""
    );
    assert_eq!(
        declaration_problem(wrong_version),
        "declares the version 2.0.0"
    );
    assert_eq!(
        declaration_problem(wrong_both),
        "declares the name \"other\" and the version 2.0.0"
    );
}

#[test]
fn a_catalog_rejects_missing_or_malformed_declarations() {
    let missing = package::Tree::new([("lib.typ", b"body".to_vec())]).unwrap();
    let not_utf8 = package::Tree::new([("typst.toml", vec![0xff])]).unwrap();
    let malformed = package::Tree::new([("typst.toml", b"[package\nname =".to_vec())]).unwrap();

    assert_eq!(declaration_problem(missing), "holds no `typst.toml`");
    assert_eq!(
        declaration_problem(not_utf8),
        "has a non-UTF-8 `typst.toml`"
    );
    assert!(declaration_problem(malformed).starts_with("has malformed `typst.toml`"));
}
