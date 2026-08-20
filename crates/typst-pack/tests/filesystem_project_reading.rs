//! Project reading from the reference filesystem source.

#![cfg(feature = "fs")]

use std::fs;

use typst_pack::{
    FilesystemProjectIssue, FilesystemProjectLimitError, FilesystemProjectLimits,
    FilesystemProjectLimitsError, FilesystemProjectReadError, FilesystemProjectResource,
    read_filesystem_project,
};

fn limits(values: [u64; 5]) -> FilesystemProjectLimits {
    FilesystemProjectLimits::new(values[0], values[1], values[2], values[3], values[4]).unwrap()
}

const GENEROUS_LIMITS: FilesystemProjectLimits = FilesystemProjectLimits::reference_v1();

#[test]
fn the_reference_v1_profile_bounds_every_project_source_resource() {
    let limits = FilesystemProjectLimits::reference_v1();

    assert_eq!(limits.visited_entries(), 1_000_000);
    assert_eq!(limits.selected_files(), 100_000);
    assert_eq!(limits.root_policy_bytes(), 1024 * 1024);
    assert_eq!(limits.selected_file_bytes(), 256 * 1024 * 1024);
    assert_eq!(limits.total_selected_bytes(), 2 * 1024 * 1024 * 1024);
}

#[test]
fn every_project_source_ceiling_must_leave_room_for_a_plus_one_probe() {
    let resources = [
        FilesystemProjectResource::VisitedEntries,
        FilesystemProjectResource::SelectedFiles,
        FilesystemProjectResource::RootPolicyBytes,
        FilesystemProjectResource::SelectedFileBytes,
        FilesystemProjectResource::TotalSelectedBytes,
    ];

    for (index, resource) in resources.into_iter().enumerate() {
        let mut values = [1; 5];
        values[index] = u64::MAX;
        assert_eq!(
            FilesystemProjectLimits::new(values[0], values[1], values[2], values[3], values[4]),
            Err(FilesystemProjectLimitsError::CannotProbe {
                resource,
                ceiling: u64::MAX,
            })
        );
    }
}

#[test]
fn reading_applies_the_root_policy_once_and_preserves_its_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let policy = b"drop/**\n!drop/keep/\n!drop/keep/kept.txt\n*.secret\n";
    fs::create_dir_all(root.join("drop/keep")).unwrap();
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("main.typ"), b"main").unwrap();
    fs::write(root.join(".typkignore"), policy).unwrap();
    fs::write(root.join("drop/gone.txt"), b"gone").unwrap();
    fs::write(root.join("drop/keep/kept.txt"), b"kept").unwrap();
    fs::write(root.join("nested/.typkignore"), b"*.txt\n").unwrap();
    fs::write(root.join("nested/ordinary.txt"), b"ordinary").unwrap();
    fs::write(root.join("private.secret"), b"private").unwrap();

    let snapshot = read_filesystem_project(root, "main.typ", GENEROUS_LIMITS).unwrap();

    assert_eq!(snapshot.file(".typkignore"), Some(policy.as_slice()));
    assert_eq!(snapshot.file("drop/keep/kept.txt"), Some(&b"kept"[..]));
    assert_eq!(snapshot.file("nested/.typkignore"), Some(&b"*.txt\n"[..]));
    assert_eq!(snapshot.file("nested/ordinary.txt"), Some(&b"ordinary"[..]));
    assert_eq!(snapshot.file("drop/gone.txt"), None);
    assert_eq!(snapshot.file("private.secret"), None);
}

#[test]
fn malformed_root_policy_bytes_are_a_typed_reading_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.typ"), b"main").unwrap();
    fs::write(dir.path().join(".typkignore"), [0xff]).unwrap();

    let error = read_filesystem_project(dir.path(), "main.typ", GENEROUS_LIMITS).unwrap_err();

    assert!(matches!(
        error,
        FilesystemProjectReadError::InvalidPolicy { .. }
    ));
}

#[cfg(unix)]
#[test]
fn excluded_directories_are_pruned_before_aliases_beneath_them_are_surveyed() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("ignored")).unwrap();
    fs::write(root.join("main.typ"), b"main").unwrap();
    fs::write(root.join(".typkignore"), b"ignored/\n").unwrap();
    symlink(dir.path(), root.join("ignored/outside")).unwrap();

    let snapshot = read_filesystem_project(&root, "main.typ", GENEROUS_LIMITS).unwrap();

    assert_eq!(
        snapshot.files().map(|(path, _)| path).collect::<Vec<_>>(),
        [".typkignore", "main.typ"]
    );
}

#[cfg(unix)]
#[test]
fn aliases_and_unsupported_entries_are_aggregated_after_the_structural_survey() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("main.typ"), b"main").unwrap();
    symlink(root.join("main.typ"), root.join("alias.typ")).unwrap();
    let _socket = UnixListener::bind(root.join("project.sock")).unwrap();

    let error = read_filesystem_project(root, "main.typ", GENEROUS_LIMITS).unwrap_err();
    let FilesystemProjectReadError::Survey(survey) = error else {
        panic!("expected a structural survey error");
    };

    assert!(matches!(
        survey.issues(),
        [FilesystemProjectIssue::Alias { path: alias }, FilesystemProjectIssue::UnsupportedEntry { path: socket, .. }]
            if alias.ends_with("alias.typ") && socket.ends_with("project.sock")
    ));
}

#[cfg(unix)]
#[test]
fn unrepresentable_paths_are_reported_by_the_filesystem_survey() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.typ"), b"main").unwrap();
    fs::write(dir.path().join(OsString::from_vec(vec![0xff])), b"bytes").unwrap();

    let error = read_filesystem_project(dir.path(), "main.typ", GENEROUS_LIMITS).unwrap_err();
    let FilesystemProjectReadError::Survey(survey) = error else {
        panic!("expected a structural survey error");
    };
    assert!(matches!(
        survey.issues(),
        [FilesystemProjectIssue::UnrepresentablePath { .. }]
    ));
}

fn assert_limit(
    root: &std::path::Path,
    values: [u64; 5],
    resource: FilesystemProjectResource,
    ceiling: u64,
    observed: u64,
) {
    let error = read_filesystem_project(root, "main.typ", limits(values)).unwrap_err();
    assert!(
        matches!(
            error,
            FilesystemProjectReadError::Limit {
                source: FilesystemProjectLimitError::Exceeded {
                    resource: reported,
                    ceiling: reported_ceiling,
                    observed_at_least,
                },
                ..
            } if reported == resource
                && reported_ceiling == ceiling
                && observed_at_least == observed
        ),
        "unexpected {resource:?} limit error: {error}"
    );
}

#[test]
fn generated_boundaries_cover_every_project_source_resource() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("main.typ"), b"main").unwrap();
    fs::write(root.join(".typkignore"), b"x").unwrap();
    let cases = [
        (FilesystemProjectResource::VisitedEntries, 2),
        (FilesystemProjectResource::SelectedFiles, 2),
        (FilesystemProjectResource::RootPolicyBytes, 1),
        (FilesystemProjectResource::SelectedFileBytes, 4),
        (FilesystemProjectResource::TotalSelectedBytes, 5),
    ];
    let exact = [2, 2, 1, 4, 5];

    for (index, (resource, observed)) in cases.into_iter().enumerate() {
        for ceiling in [observed + 1, observed] {
            let mut values = exact;
            values[index] = ceiling;
            read_filesystem_project(root, "main.typ", limits(values)).unwrap();
        }
        let mut values = exact;
        values[index] -= 1;
        assert_limit(root, values, resource, values[index], observed);
    }
}
