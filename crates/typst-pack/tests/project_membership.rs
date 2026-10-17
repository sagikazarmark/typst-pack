//! Project membership computed without a filesystem.
//!
//! Every test here runs on a build with no crate feature enabled.

use proptest::prelude::*;
use typst_pack::{Pack, PackInvariantIssue, ProjectSnapshot};

#[test]
fn a_snapshot_is_assembled_from_path_and_bytes_entries() {
    let snapshot = ProjectSnapshot::new(
        "main.typ",
        [
            ("./chapters/intro.typ", b"= Intro".to_vec()),
            ("main.typ", b"Hello".to_vec()),
        ],
    )
    .unwrap();

    assert_eq!(snapshot.entrypoint(), "main.typ");
    assert_eq!(
        snapshot.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["chapters/intro.typ", "main.typ"]
    );
    assert_eq!(snapshot.file("main.typ"), Some(b"Hello".as_slice()));
}

#[test]
fn assembly_preserves_entries_selected_by_the_source() {
    let snapshot = ProjectSnapshot::new(
        "main.typ",
        [
            ("main.typ", b"Hello".to_vec()),
            ("private.secret", b"selected".to_vec()),
        ],
    )
    .unwrap();

    assert_eq!(
        snapshot.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["main.typ", "private.secret"]
    );
    assert_eq!(
        snapshot.file("private.secret"),
        Some(b"selected".as_slice())
    );
}

#[test]
fn assembly_rejects_entries_that_cannot_name_a_root_relative_project_file() {
    for path in [
        "",
        "/absolute.typ",
        "../escape.typ",
        "C:/drive.typ",
        "back\\slash.typ",
    ] {
        let error = ProjectSnapshot::new(
            "main.typ",
            [("main.typ", b"Hello".to_vec()), (path, b"nope".to_vec())],
        )
        .unwrap_err();

        assert!(
            matches!(error.issues(), [PackInvariantIssue::InvalidPath { path: reported, .. }] if reported == path),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn assembly_requires_the_selected_entrypoint() {
    let error = ProjectSnapshot::new("main.typ", [("other.typ", b"Hello".to_vec())]).unwrap_err();
    assert_eq!(
        error.issues(),
        [PackInvariantIssue::MissingEntrypoint {
            path: "main.typ".to_owned(),
        }]
    );
}

#[test]
fn assembly_rejects_every_pack_path() {
    for path in [
        ".typk",
        "old.typk",
        "nested/old.typk",
        "bundle.typk/main.typ",
    ] {
        let error = ProjectSnapshot::new(
            "main.typ",
            [("main.typ", b"Hello".to_vec()), (path, b"pack".to_vec())],
        )
        .unwrap_err();

        assert!(
            matches!(error.issues(), [PackInvariantIssue::InvalidPath { path: reported, .. }] if reported == path),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn snapshots_report_every_independent_issue() {
    let error = ProjectSnapshot::new(
        "main.typ",
        [
            ("z\\escape.typ", b"invalid".to_vec()),
            ("./duplicate.typ", b"first".to_vec()),
            ("duplicate.typ", b"second".to_vec()),
            ("nested/../duplicate.typ", b"third".to_vec()),
            ("/absolute.typ", b"invalid".to_vec()),
        ],
    )
    .unwrap_err();

    let issues = error.issues();
    assert_eq!(issues.len(), 4, "{error}");
    for path in ["/absolute.typ", "z\\escape.typ"] {
        assert!(issues.iter().any(
            |issue| matches!(issue, PackInvariantIssue::InvalidPath { path: reported, .. } if reported == path)
        ));
    }
    assert!(issues.contains(&PackInvariantIssue::DuplicateProjectPath {
        path: "duplicate.typ".to_owned()
    }));
    assert!(issues.contains(&PackInvariantIssue::MissingEntrypoint {
        path: "main.typ".to_owned()
    }));
    // The error lists every issue.
    for issue in issues {
        assert!(error.to_string().contains(&issue.to_string()));
    }
}

#[test]
fn snapshot_issue_display_escapes_source_control_characters() {
    let error =
        ProjectSnapshot::new("main.typ", [("/forged\npath.typ", b"invalid".to_vec())]).unwrap_err();

    assert!(error.to_string().contains(r#"/forged\npath.typ"#));
    assert!(!error.to_string().contains("forged\npath"));
}

proptest! {
    #[test]
    fn assembly_canonicalizes_generated_project_paths(
        stem in "[a-z][a-z0-9]{0,15}",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let canonical = format!("chapters/{stem}.typ");
        let supplied = format!("./{canonical}");
        let snapshot = ProjectSnapshot::new(&supplied, [(supplied.as_str(), bytes.clone())])
            .unwrap();

        prop_assert_eq!(snapshot.entrypoint(), canonical.as_str());
        prop_assert_eq!(snapshot.file(&canonical), Some(bytes.as_slice()));
    }

    #[test]
    fn assembly_is_invariant_under_entry_permutation(
        generated in prop::collection::btree_map(
            "[a-z][a-z0-9]{0,15}",
            prop::collection::vec(any::<u8>(), 0..64),
            0..16,
        ),
    ) {
        let mut entries = vec![("main.typ".to_owned(), b"main".to_vec())];
        entries.extend(
            generated
                .into_iter()
                .map(|(stem, bytes)| (format!("files/{stem}.bin"), bytes)),
        );
        let mut reversed = entries.clone();
        reversed.reverse();

        let forward = ProjectSnapshot::new("main.typ", entries)
            .unwrap();
        let backward = ProjectSnapshot::new("main.typ", reversed)
            .unwrap();

        prop_assert_eq!(forward, backward);
    }

    #[test]
    fn assembly_rejects_generated_canonical_duplicates(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let canonical = format!("files/{stem}.typ");
        let alias = format!("./{canonical}");
        let error = ProjectSnapshot::new("main.typ", [
                ("main.typ", b"main".to_vec()),
                (canonical.as_str(), b"first".to_vec()),
                (alias.as_str(), b"second".to_vec()),
            ])
            .unwrap_err();

        prop_assert_eq!(
            error.issues(),
            [PackInvariantIssue::DuplicateProjectPath { path: canonical }],
        );
    }

    #[test]
    fn assembly_rejects_generated_snapshots_missing_the_entrypoint(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let other = format!("files/{stem}.typ");
        let error = ProjectSnapshot::new("main.typ", [(other, b"other".to_vec())])
            .unwrap_err();

        prop_assert_eq!(
            error.issues(),
            [PackInvariantIssue::MissingEntrypoint {
                path: "main.typ".to_owned(),
            }],
        );
    }

    #[test]
    fn assembly_rejects_generated_pack_paths(
        stem in "[a-z][a-z0-9]{0,15}",
        nested in any::<bool>(),
    ) {
        let path = if nested {
            format!("archives/{stem}.typk/main.typ")
        } else {
            format!("archives/{stem}.typk")
        };
        let error = ProjectSnapshot::new("main.typ", [("main.typ", b"main".to_vec()), (path.as_str(), b"pack".to_vec())])
            .unwrap_err();

        let rejected = matches!(
            error.issues(),
            [PackInvariantIssue::InvalidPath { path: reported, .. }] if reported == path.as_str()
        );
        prop_assert!(rejected);
    }
}

#[test]
fn canonical_project_path_validation_rejects_pack_paths() {
    let error = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .file("nested/old.typk", b"drop".to_vec())
        .build()
        .unwrap_err();
    assert!(
        matches!(error.issues(), [PackInvariantIssue::InvalidPath { .. }]),
        "{error}"
    );

    let error = Pack::builder("main.typk")
        .file("main.typ", b"Hello".to_vec())
        .build()
        .unwrap_err();
    assert!(
        matches!(
            error.issues(),
            [PackInvariantIssue::InvalidEntrypoint { .. }]
        ),
        "{error}"
    );
}
