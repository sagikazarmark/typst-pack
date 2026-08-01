//! Project membership computed without a filesystem.
//!
//! Every test here runs on a build with no crate feature enabled.

use proptest::prelude::*;
use typst_pack::{
    IGNORE_FILE, Pack, PackBuildError, PackInvariantIssue, PackPathRole, ProjectIgnorePolicy,
    ProjectIgnorePolicyError, ProjectSnapshotAssembly, ProjectSnapshotError,
};

#[test]
fn an_ignore_policy_is_parsed_from_bytes_and_matched_against_a_path() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"*.secret\n").unwrap();

    assert!(policy.excludes_file("private.secret"));
    assert!(!policy.excludes_file("main.typ"));
}

#[test]
fn an_excluded_directory_excludes_every_path_beneath_it() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"ignored/\n!ignored/keep.txt\n").unwrap();

    assert!(policy.excludes_directory("ignored"));
    assert!(policy.excludes_file("ignored/keep.txt"));
    assert!(!policy.excludes_file("keep.txt"));
}

#[test]
fn negation_and_last_match_precedence_decide_membership() {
    let policy = ProjectIgnorePolicy::from_ignore_file(
        b"ignored/**\n!ignored/reincluded/\n!ignored/reincluded/keep.txt\n",
    )
    .unwrap();

    assert!(policy.excludes_file("ignored/drop.txt"));
    assert!(!policy.excludes_directory("ignored/reincluded"));
    assert!(!policy.excludes_file("ignored/reincluded/keep.txt"));
}

#[test]
fn the_built_in_pack_exclusion_cannot_be_overridden() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"!*.typk\n").unwrap();

    assert!(policy.excludes_file(".typk"));
    assert!(policy.excludes_file("old.typk"));
    assert!(policy.excludes_file("nested/inner.typk"));
    assert!(policy.excludes_directory("bundle.typk"));
    assert!(policy.excludes_file("bundle.typk/main.typ"));
}

#[test]
fn a_project_without_an_ignore_file_still_gets_the_built_in_exclusion() {
    let policy = ProjectIgnorePolicy::built_in();

    assert!(policy.excludes_file("old.typk"));
    assert!(!policy.excludes_file("main.typ"));
}

#[test]
fn a_malformed_policy_is_rejected() {
    let error = ProjectIgnorePolicy::from_ignore_file(b"*.typ\ntrailing\\\n").unwrap_err();
    assert!(
        matches!(error, ProjectIgnorePolicyError::InvalidRule { line: 2, .. }),
        "{error}"
    );

    let error = ProjectIgnorePolicy::from_ignore_file(&[0xff]).unwrap_err();
    assert_eq!(error, ProjectIgnorePolicyError::NotUtf8);
}

#[test]
fn the_root_policy_file_is_included_and_nested_ones_are_ordinary_project_files() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"*ignore\n").unwrap();

    assert!(!policy.excludes_file(IGNORE_FILE));
    assert!(policy.excludes_file("nested/.typkignore"));
}

#[test]
fn a_snapshot_is_assembled_from_path_and_bytes_entries() {
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("./chapters/intro.typ", b"= Intro".to_vec()),
            ("main.typ", b"Hello".to_vec()),
        ])
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
    let policy = ProjectIgnorePolicy::from_ignore_file(b"*.secret\n").unwrap();
    assert!(policy.excludes_file("private.secret"));

    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"Hello".to_vec()),
            ("private.secret", b"selected".to_vec()),
        ])
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
        let error = ProjectSnapshotAssembly::new("main.typ")
            .assemble([("main.typ", b"Hello".to_vec()), (path, b"nope".to_vec())])
            .unwrap_err();

        assert!(
            matches!(&error, ProjectSnapshotError::InvalidPath { path: reported, .. } if reported == path),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn assembly_requires_the_selected_entrypoint() {
    let error = ProjectSnapshotAssembly::new("main.typ")
        .assemble([("other.typ", b"Hello".to_vec())])
        .unwrap_err();
    assert_eq!(
        error,
        ProjectSnapshotError::MissingEntrypoint("main.typ".to_owned())
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
        let error = ProjectSnapshotAssembly::new("main.typ")
            .assemble([("main.typ", b"Hello".to_vec()), (path, b"pack".to_vec())])
            .unwrap_err();

        assert!(
            matches!(&error, ProjectSnapshotError::InvalidPath { path: reported, .. } if reported == path),
            "`{path}`: {error}"
        );
    }
}

proptest! {
    #[test]
    fn assembly_canonicalizes_generated_project_paths(
        stem in "[a-z][a-z0-9]{0,15}",
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let canonical = format!("chapters/{stem}.typ");
        let supplied = format!("./{canonical}");
        let snapshot = ProjectSnapshotAssembly::new(&supplied)
            .assemble([(supplied.as_str(), bytes.clone())])
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

        let forward = ProjectSnapshotAssembly::new("main.typ")
            .assemble(entries)
            .unwrap();
        let backward = ProjectSnapshotAssembly::new("main.typ")
            .assemble(reversed)
            .unwrap();

        prop_assert_eq!(forward, backward);
    }

    #[test]
    fn assembly_rejects_generated_canonical_duplicates(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let canonical = format!("files/{stem}.typ");
        let alias = format!("./{canonical}");
        let error = ProjectSnapshotAssembly::new("main.typ")
            .assemble([
                ("main.typ", b"main".to_vec()),
                (canonical.as_str(), b"first".to_vec()),
                (alias.as_str(), b"second".to_vec()),
            ])
            .unwrap_err();

        prop_assert_eq!(error, ProjectSnapshotError::DuplicatePath { path: canonical });
    }

    #[test]
    fn assembly_rejects_generated_snapshots_missing_the_entrypoint(
        stem in "[a-z][a-z0-9]{0,15}",
    ) {
        let other = format!("files/{stem}.typ");
        let error = ProjectSnapshotAssembly::new("main.typ")
            .assemble([(other, b"other".to_vec())])
            .unwrap_err();

        prop_assert_eq!(
            error,
            ProjectSnapshotError::MissingEntrypoint("main.typ".to_owned()),
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
        let error = ProjectSnapshotAssembly::new("main.typ")
            .assemble([("main.typ", b"main".to_vec()), (path.as_str(), b"pack".to_vec())])
            .unwrap_err();

        let rejected =
            matches!(error, ProjectSnapshotError::InvalidPath { path: reported, .. } if reported == path);
        prop_assert!(rejected);
    }
}

#[test]
fn canonical_project_path_validation_rejects_pack_paths() {
    let error = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .file("nested/old.typk", b"drop".to_vec())
        .unwrap()
        .build()
        .unwrap_err();
    assert!(
        matches!(
            error,
            PackBuildError::Invariant(ref error)
                if matches!(error.issues(), [PackInvariantIssue::InvalidPath {
                    role: PackPathRole::ProjectFile,
                    ..
                }])
        ),
        "{error}"
    );

    let error = Pack::builder("main.typk")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap_err();
    assert!(
        matches!(
            error,
            PackBuildError::Invariant(ref error)
                if matches!(error.issues(), [PackInvariantIssue::InvalidPath {
                    role: PackPathRole::Entrypoint,
                    ..
                }])
        ),
        "{error}"
    );
}
