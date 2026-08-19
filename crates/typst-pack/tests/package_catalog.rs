//! Validated Package Trees and Package Catalogs.
//!
//! Every test here runs on a build with no crate feature enabled.

use proptest::prelude::*;
use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    PackageCatalog, PackageCatalogIssue, PackageDisposition, PackageTree, PackageTreeIssue,
};

fn spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:1.0.0")).unwrap()
}

fn package_tree(name: &str, body: &[u8]) -> PackageTree {
    PackageTree::from_owned_entries([
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
        let error = PackageTree::from_owned_entries([(path, b"content".to_vec())]).unwrap_err();

        assert!(
            error.issues().iter().any(
                |issue| matches!(issue, PackageTreeIssue::InvalidPath { path: reported, .. } if reported == path)
            ),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn package_tree_construction_rejects_duplicate_canonical_paths() {
    let error = PackageTree::from_owned_entries([
        ("lib.typ", b"first".to_vec()),
        ("./lib.typ", b"second".to_vec()),
    ])
    .unwrap_err();

    assert_eq!(
        error.issues(),
        &[PackageTreeIssue::DuplicatePath {
            path: "lib.typ".to_owned(),
        }]
    );
}

#[test]
fn package_tree_construction_rejects_file_ancestor_conflicts() {
    let error = PackageTree::from_owned_entries([
        ("assets", b"file".to_vec()),
        ("assets/logo.svg", b"image".to_vec()),
    ])
    .unwrap_err();

    assert_eq!(
        error.issues(),
        &[PackageTreeIssue::PathTreeConflict {
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
    let error = PackageTree::from_owned_entries(entries).unwrap_err();
    let reversed_error = PackageTree::from_owned_entries(reversed).unwrap_err();

    assert_eq!(error, reversed_error);
    assert_eq!(error.issues().len(), 4);
    assert!(matches!(
        &error.issues()[0],
        PackageTreeIssue::InvalidPath { path, .. } if path == "../escape.typ"
    ));
    assert_eq!(
        &error.issues()[1..],
        &[
            PackageTreeIssue::PathTreeConflict {
                ancestor: "dir".to_owned(),
                descendant: "dir/first.typ".to_owned(),
            },
            PackageTreeIssue::PathTreeConflict {
                ancestor: "dir".to_owned(),
                descendant: "dir/second.typ".to_owned(),
            },
            PackageTreeIssue::DuplicatePath {
                path: "same.typ".to_owned(),
            },
        ]
    );
}

#[test]
fn package_tree_construction_explicitly_moves_or_copies_payloads() {
    let owned = b"owned package bytes".to_vec();
    let owned_pointer = owned.as_ptr();
    let tree = PackageTree::from_owned_entries([("lib.typ", owned)]).unwrap();

    assert_eq!(tree.file("lib.typ").unwrap().as_ptr(), owned_pointer);
    assert_eq!(
        tree.clone().file("lib.typ").unwrap().as_ptr(),
        owned_pointer
    );

    let borrowed = b"borrowed package bytes".to_vec();
    let copied = PackageTree::copy_from_entries([("lib.typ", borrowed.as_slice())]).unwrap();
    assert_eq!(copied.file("lib.typ"), Some(borrowed.as_slice()));
    assert_ne!(copied.file("lib.typ").unwrap().as_ptr(), borrowed.as_ptr());
}

#[test]
fn package_tree_exposes_canonical_identity_order_and_totals() {
    let tree = PackageTree::from_owned_entries([
        ("./src/lib.typ", b"library".to_vec()),
        ("README.md", b"read me".to_vec()),
    ])
    .unwrap();

    assert_eq!(
        tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["README.md", "src/lib.typ"]
    );
    assert_eq!(tree.file_count(), 2);
    assert_eq!(tree.byte_length(), 14);
    assert_eq!(
        tree.identity().role(),
        typst_pack::CanonicalIdentityRole::PackageTree
    );
    assert_eq!(
        tree.identity().schema(),
        "typst-pack-complete-package-tree-v1"
    );
    assert_eq!(tree.identity().algorithm(), "typst-hash128-0.15");
    assert_eq!(
        tree.identity().digest(),
        [
            0xb3, 0x7d, 0xc1, 0xe5, 0x59, 0x5c, 0x2a, 0xd4, 0x60, 0x3a, 0x17, 0xc7, 0xac, 0x1b,
            0x0f, 0x44,
        ]
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

        let forward = PackageTree::from_owned_entries(entries).unwrap();
        let backward = PackageTree::from_owned_entries(reversed).unwrap();

        prop_assert_eq!(forward.identity(), backward.identity());
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
        let baseline = PackageTree::from_owned_entries([(path, bytes.clone())]).unwrap();
        let changed_bytes = PackageTree::from_owned_entries([(
            format!("files/{stem}.bin"),
            replacement,
        )])
        .unwrap();
        let changed_path = PackageTree::from_owned_entries([(renamed, bytes)]).unwrap();

        prop_assert_ne!(baseline.identity(), changed_bytes.identity());
        prop_assert_ne!(baseline.identity(), changed_path.identity());
    }

    #[test]
    fn package_tree_rejects_generated_canonical_duplicates(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let canonical = format!("files/{stem}.typ");
        let alias = format!("./{canonical}");
        let error = PackageTree::from_owned_entries([
            (canonical.as_str(), b"first".to_vec()),
            (alias.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();

        prop_assert_eq!(
            error.issues(),
            &[PackageTreeIssue::DuplicatePath { path: canonical }],
        );
    }
}

#[test]
fn package_catalog_rejects_duplicate_exact_specifications() {
    let duplicate = spec("example");
    let error = PackageCatalog::from_entries([
        (
            duplicate.clone(),
            package_tree("example", b"first"),
            PackageDisposition::Embedded,
        ),
        (
            duplicate.clone(),
            package_tree("example", b"second"),
            PackageDisposition::External,
        ),
    ])
    .unwrap_err();

    assert_eq!(
        error.issues(),
        &[PackageCatalogIssue::DuplicateSpecification { spec: duplicate }]
    );
}

#[test]
fn package_catalog_eagerly_checks_claimed_name_and_version() {
    let claimed = spec("claimed");
    let wrong_name = package_tree("other", b"body");
    let wrong_version = PackageTree::from_owned_entries([
        (
            "typst.toml",
            b"[package]\nname = \"claimed\"\nversion = \"2.0.0\"\nentrypoint = \"lib.typ\"\n"
                .to_vec(),
        ),
        ("lib.typ", b"body".to_vec()),
    ])
    .unwrap();

    for tree in [wrong_name, wrong_version] {
        let error =
            PackageCatalog::from_entries([(claimed.clone(), tree, PackageDisposition::Embedded)])
                .unwrap_err();

        assert!(
            error.issues().iter().any(|issue| matches!(
                issue,
                PackageCatalogIssue::MismatchedName { spec, .. }
                    | PackageCatalogIssue::MismatchedVersion { spec, .. }
                    if spec == &claimed
            )),
            "{error}"
        );
    }
}

#[test]
fn package_catalog_rejects_missing_or_malformed_metadata() {
    let claimed = spec("claimed");
    let cases = [
        PackageTree::from_owned_entries([("lib.typ", b"body".to_vec())]).unwrap(),
        PackageTree::from_owned_entries([("typst.toml", vec![0xff])]).unwrap(),
        PackageTree::from_owned_entries([("typst.toml", b"[package\nname =".to_vec())]).unwrap(),
    ];

    for tree in cases {
        let error =
            PackageCatalog::from_entries([(claimed.clone(), tree, PackageDisposition::Embedded)])
                .unwrap_err();

        assert!(
            error.issues().iter().any(|issue| matches!(
                issue,
                PackageCatalogIssue::MissingDeclaration { spec }
                    | PackageCatalogIssue::DeclarationNotUtf8 { spec }
                    | PackageCatalogIssue::MalformedDeclaration { spec, .. }
                    if spec == &claimed
            )),
            "{error}"
        );
    }
}

#[test]
fn package_catalog_aggregates_issues_in_canonical_order() {
    let alpha = spec("alpha");
    let zeta = spec("zeta");
    let wrong_alpha = PackageTree::from_owned_entries([(
        "typst.toml",
        b"[package]\nname = \"other\"\nversion = \"2.0.0\"\n".to_vec(),
    )])
    .unwrap();
    let missing_zeta = PackageTree::from_owned_entries([("lib.typ", b"body".to_vec())]).unwrap();

    let error = PackageCatalog::from_entries([
        (zeta.clone(), missing_zeta, PackageDisposition::External),
        (alpha.clone(), wrong_alpha, PackageDisposition::Embedded),
        (
            zeta.clone(),
            package_tree("zeta", b"valid"),
            PackageDisposition::Embedded,
        ),
    ])
    .unwrap_err();

    assert!(matches!(
        &error.issues()[0],
        PackageCatalogIssue::MismatchedName { spec, .. } if spec == &alpha
    ));
    assert!(matches!(
        &error.issues()[1],
        PackageCatalogIssue::MismatchedVersion { spec, .. } if spec == &alpha
    ));
    assert!(matches!(
        &error.issues()[2],
        PackageCatalogIssue::DuplicateSpecification { spec } if spec == &zeta
    ));
    assert!(matches!(
        &error.issues()[3],
        PackageCatalogIssue::MissingDeclaration { spec } if spec == &zeta
    ));
}

#[test]
fn package_catalog_issue_order_is_invariant_under_input_permutation() {
    let claimed = spec("claimed");
    let wrong_tree = |declared: &str| {
        PackageTree::from_owned_entries([(
            "typst.toml",
            format!("[package]\nname = \"{declared}\"\nversion = \"1.0.0\"\n").into_bytes(),
        )])
        .unwrap()
    };
    let entries = vec![
        (
            claimed.clone(),
            wrong_tree("zeta"),
            PackageDisposition::Embedded,
        ),
        (claimed, wrong_tree("alpha"), PackageDisposition::External),
    ];
    let mut reversed = entries.clone();
    reversed.reverse();

    let forward = PackageCatalog::from_entries(entries).unwrap_err();
    let backward = PackageCatalog::from_entries(reversed).unwrap_err();

    assert_eq!(forward, backward);
}

#[test]
fn package_catalog_entries_are_canonical_and_retain_dispositions() {
    let alpha = spec("alpha");
    let zeta = spec("zeta");
    let catalog = PackageCatalog::from_entries([
        (
            zeta.clone(),
            package_tree("zeta", b"zeta"),
            PackageDisposition::External,
        ),
        (
            alpha.clone(),
            package_tree("alpha", b"alpha"),
            PackageDisposition::Embedded,
        ),
    ])
    .unwrap();
    let entries = catalog.entries().collect::<Vec<_>>();

    assert_eq!(entries[0].spec(), &alpha);
    assert_eq!(entries[0].disposition(), PackageDisposition::Embedded);
    assert_eq!(entries[0].tree().file("lib.typ"), Some(b"alpha".as_slice()));
    assert_eq!(entries[1].spec(), &zeta);
    assert_eq!(entries[1].disposition(), PackageDisposition::External);
    assert_eq!(catalog.get(&zeta), Some(entries[1]));
}

proptest! {
    #[test]
    fn package_catalog_is_invariant_under_input_permutation(
        names in prop::collection::btree_set("[a-z][a-z0-9]{0,15}", 0..16),
    ) {
        let entries = names
            .into_iter()
            .map(|name| {
                (
                    spec(&name),
                    package_tree(&name, name.as_bytes()),
                    PackageDisposition::Embedded,
                )
            })
            .collect::<Vec<_>>();
        let mut reversed = entries.clone();
        reversed.reverse();

        let forward = PackageCatalog::from_entries(entries).unwrap();
        let backward = PackageCatalog::from_entries(reversed).unwrap();

        prop_assert_eq!(&forward, &backward);
        let specs = backward
            .entries()
            .map(|entry| entry.spec().to_string())
            .collect::<Vec<_>>();
        prop_assert!(specs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn package_catalog_rejects_generated_duplicate_specifications(
        name in "[a-z][a-z0-9]{0,15}",
    ) {
        let duplicate = spec(&name);
        let error = PackageCatalog::from_entries([
            (
                duplicate.clone(),
                package_tree(&name, b"first"),
                PackageDisposition::Embedded,
            ),
            (
                duplicate.clone(),
                package_tree(&name, b"second"),
                PackageDisposition::External,
            ),
        ])
        .unwrap_err();

        prop_assert_eq!(
            error.issues(),
            &[PackageCatalogIssue::DuplicateSpecification { spec: duplicate }],
        );
    }
}
