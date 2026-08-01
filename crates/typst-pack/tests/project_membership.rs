//! Project membership computed without a filesystem.
//!
//! Every test here runs on a build with no crate feature enabled.

use typst_pack::{
    IGNORE_FILE, Pack, PackBuildError, PackInvariantError, PackPathRole, ProjectIgnorePolicy,
    ProjectIgnorePolicyError, ProjectSnapshotAssembly, ProjectSnapshotBudget, ProjectSnapshotError,
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
    let policy = ProjectIgnorePolicy::built_in();

    let snapshot = ProjectSnapshotAssembly::new("main.typ", &policy)
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
fn assembly_re_applies_the_policy_to_the_entries_it_is_supplied() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"*.secret\n").unwrap();

    let snapshot = ProjectSnapshotAssembly::new("main.typ", &policy)
        .assemble([
            ("main.typ", b"Hello".to_vec()),
            ("private.secret", b"drop".to_vec()),
            ("old.typk", b"drop".to_vec()),
            ("bundle.typk/main.typ", b"drop".to_vec()),
        ])
        .unwrap();

    assert_eq!(
        snapshot.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["main.typ"]
    );
}

#[test]
fn assembly_rejects_entries_that_cannot_name_a_root_relative_project_file() {
    let policy = ProjectIgnorePolicy::built_in();

    for path in [
        "",
        "/absolute.typ",
        "../escape.typ",
        "C:/drive.typ",
        "back\\slash.typ",
    ] {
        let error = ProjectSnapshotAssembly::new("main.typ", &policy)
            .assemble([("main.typ", b"Hello".to_vec()), (path, b"nope".to_vec())])
            .unwrap_err();

        assert!(
            matches!(&error, ProjectSnapshotError::InvalidPath { path: reported, .. } if reported == path),
            "`{path}`: {error}"
        );
    }
}

#[test]
fn assembly_fails_when_the_entrypoint_does_not_survive_filtering() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"main.typ\n").unwrap();
    let error = ProjectSnapshotAssembly::new("main.typ", &policy)
        .assemble([("main.typ", b"Hello".to_vec())])
        .unwrap_err();
    assert_eq!(
        error,
        ProjectSnapshotError::ExcludedEntrypoint("main.typ".to_owned())
    );

    let policy = ProjectIgnorePolicy::built_in();
    let error = ProjectSnapshotAssembly::new("main.typ", &policy)
        .assemble([("other.typ", b"Hello".to_vec())])
        .unwrap_err();
    assert_eq!(
        error,
        ProjectSnapshotError::MissingEntrypoint("main.typ".to_owned())
    );
}

#[test]
fn a_budget_bounds_what_survives_exclusion() {
    let policy = ProjectIgnorePolicy::from_ignore_file(b"*.big\n").unwrap();
    let entries = || {
        [
            ("main.typ", b"Hello".to_vec()),
            ("huge.big", vec![0; 4096]),
            ("notes.txt", b"notes".to_vec()),
        ]
    };

    // Exclusion runs first, so the excluded entry counts against neither bound.
    let snapshot = ProjectSnapshotAssembly::new("main.typ", &policy)
        .budget(ProjectSnapshotBudget {
            max_files: Some(2),
            max_bytes: Some(10),
        })
        .assemble(entries())
        .unwrap();
    assert_eq!(
        snapshot.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["main.typ", "notes.txt"]
    );

    let error = ProjectSnapshotAssembly::new("main.typ", &policy)
        .budget(ProjectSnapshotBudget {
            max_files: Some(1),
            ..ProjectSnapshotBudget::default()
        })
        .assemble(entries())
        .unwrap_err();
    assert_eq!(error, ProjectSnapshotError::FileCountExceeded { limit: 1 });

    let error = ProjectSnapshotAssembly::new("main.typ", &policy)
        .budget(ProjectSnapshotBudget {
            max_bytes: Some(9),
            ..ProjectSnapshotBudget::default()
        })
        .assemble(entries())
        .unwrap_err();
    assert_eq!(error, ProjectSnapshotError::ByteSizeExceeded { limit: 9 });
}

#[test]
fn canonical_project_path_validation_rejects_pack_paths() {
    let error = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .file("nested/old.typk", b"drop".to_vec())
        .unwrap_err();
    assert!(
        matches!(
            error,
            PackBuildError::Invariant(PackInvariantError::InvalidPath {
                role: PackPathRole::ProjectFile,
                ..
            })
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
            PackBuildError::Invariant(PackInvariantError::InvalidPath {
                role: PackPathRole::Entrypoint,
                ..
            })
        ),
        "{error}"
    );
}
