#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};

use typst_pack::{
    Artifact, CompileRequest, FilesystemWriteError, Output, OutputFormat, Pack,
    PackExtractionSelection, WriteIssue, WritePolicy, compile, plan_pack_extraction,
    write_compilation_artifacts_to_filesystem_paths, write_pack_extraction_plan_to_filesystem,
};

fn plan_of(files: &[(&str, &[u8])]) -> typst_pack::PackExtractionPlan {
    let mut builder = Pack::builder("main.typ").file("main.typ", b"new main".to_vec());
    for (path, bytes) in files {
        builder = builder.file(*path, bytes.to_vec());
    }
    plan_pack_extraction(
        &builder.build().unwrap(),
        PackExtractionSelection::default(),
    )
    .unwrap()
}

fn extraction_plan() -> typst_pack::PackExtractionPlan {
    plan_of(&[("assets/data.txt", b"new data")])
}

fn temp_path(directory: &tempfile::TempDir) -> PathBuf {
    std::fs::canonicalize(directory.path()).unwrap()
}

/// Every name in a directory tree, relative to it and sorted.
fn tree(root: &Path) -> Vec<String> {
    fn walk(root: &Path, directory: &Path, names: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            names.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
            if path.is_dir() && !path.is_symlink() {
                walk(root, &path, names);
            }
        }
    }
    let mut names = Vec::new();
    walk(root, root, &mut names);
    names.sort();
    names
}

fn rejected(result: Result<(), FilesystemWriteError>) -> Vec<WriteIssue> {
    match result {
        Err(FilesystemWriteError::Rejected { issues }) => issues,
        other => panic!("expected a rejected write, got {other:?}"),
    }
}

fn compilation_result(source: &[u8], svg: bool) -> Vec<Artifact> {
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.to_vec())
        .build()
        .unwrap();
    let format = if svg {
        OutputFormat::Svg
    } else {
        OutputFormat::Pdf
    };
    compile(&CompileRequest::new(pack, Output::new(format)))
        .unwrap()
        .artifacts
}

#[test]
fn extraction_creates_the_destination_and_writes_exact_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("new/nested/written");

    write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        WritePolicy::CreateNew,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"new main"
    );
    assert_eq!(
        std::fs::read(destination.join("assets/data.txt")).unwrap(),
        b"new data"
    );
    assert_eq!(
        tree(&destination),
        ["assets", "assets/data.txt", "main.typ"]
    );
}

#[test]
fn create_new_refuses_every_existing_target_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir_all(destination.join("assets")).unwrap();
    std::fs::write(destination.join("main.typ"), b"old main").unwrap();
    std::fs::write(destination.join("assets/data.txt"), b"old data").unwrap();

    let issues = rejected(write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        WritePolicy::CreateNew,
    ));

    assert_eq!(
        issues,
        [
            WriteIssue::ExistingTarget {
                path: PathBuf::from("assets/data.txt")
            },
            WriteIssue::ExistingTarget {
                path: PathBuf::from("main.typ")
            },
        ]
    );
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"old main"
    );
}

#[test]
fn overwrite_replaces_planned_files_and_keeps_unrelated_content() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), b"old main").unwrap();
    std::fs::write(destination.join("unrelated.txt"), b"unrelated").unwrap();

    write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        WritePolicy::Overwrite,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"new main"
    );
    assert_eq!(
        std::fs::read(destination.join("unrelated.txt")).unwrap(),
        b"unrelated"
    );
    assert_eq!(
        tree(&destination),
        ["assets", "assets/data.txt", "main.typ", "unrelated.txt"]
    );
}

#[test]
fn conflicting_files_and_directories_are_refused_under_every_policy() {
    for policy in [WritePolicy::CreateNew, WritePolicy::Overwrite] {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        std::fs::create_dir_all(destination.join("main.typ")).unwrap();
        std::fs::write(destination.join("assets"), b"a file").unwrap();

        let issues = rejected(write_pack_extraction_plan_to_filesystem(
            &extraction_plan(),
            &destination,
            policy,
        ));

        assert_eq!(
            issues,
            [
                WriteIssue::ConflictingAncestor {
                    path: PathBuf::from("assets/data.txt"),
                    ancestor: destination.join("assets"),
                },
                WriteIssue::ConflictingTarget {
                    path: PathBuf::from("main.typ")
                },
            ]
        );
        assert_eq!(tree(&destination), ["assets", "main.typ"]);
    }

    let directory = tempfile::tempdir().unwrap();
    let file = temp_path(&directory).join("file");
    std::fs::write(&file, b"a file").unwrap();
    let issues = rejected(write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &file,
        WritePolicy::Overwrite,
    ));
    assert!(matches!(
        issues.as_slice(),
        [WriteIssue::ConflictingAncestor { ancestor, .. }] if ancestor == &file
    ));
}

#[test]
fn unportable_and_case_colliding_plan_paths_are_refused_everywhere() {
    let plan = plan_of(&[
        ("CON.txt", b"device"),
        ("notes.", b"trailing dot"),
        ("Data.txt", b"upper"),
        ("data.txt", b"lower"),
    ]);
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");

    let issues = rejected(write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        WritePolicy::Overwrite,
    ));

    assert!(issues.contains(&WriteIssue::ReservedName {
        path: PathBuf::from("CON.txt"),
        component: "CON.txt".to_owned(),
    }));
    assert!(issues.contains(&WriteIssue::ReservedName {
        path: PathBuf::from("notes."),
        component: "notes.".to_owned(),
    }));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        WriteIssue::PathConflict { first, second }
            if first == Path::new("Data.txt") && second == Path::new("data.txt")
    )));
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn symlinked_ancestors_and_targets_beneath_the_destination_are_refused() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let destination = root.join("written");
    let outside = root.join("outside");
    std::fs::create_dir(&destination).unwrap();
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, destination.join("assets")).unwrap();
    symlink(outside.join("main.typ"), destination.join("main.typ")).unwrap();

    let issues = rejected(write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        WritePolicy::Overwrite,
    ));

    assert_eq!(
        issues,
        [
            WriteIssue::ConflictingAncestor {
                path: PathBuf::from("assets/data.txt"),
                ancestor: destination.join("assets"),
            },
            WriteIssue::ConflictingTarget {
                path: PathBuf::from("main.typ")
            },
        ]
    );
    assert!(tree(&outside).is_empty());
}

#[cfg(unix)]
#[test]
fn a_destination_reached_through_symbolic_links_is_written() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    std::fs::create_dir(root.join("real")).unwrap();
    symlink(root.join("real"), root.join("linked")).unwrap();

    let plan = extraction_plan();
    // The destination itself is a link, then lies beneath one.
    write_pack_extraction_plan_to_filesystem(&plan, root.join("linked"), WritePolicy::CreateNew)
        .unwrap();
    write_pack_extraction_plan_to_filesystem(
        &plan,
        root.join("linked/nested"),
        WritePolicy::CreateNew,
    )
    .unwrap();
    write_pack_extraction_plan_to_filesystem(&plan, root.join("linked"), WritePolicy::Overwrite)
        .unwrap();

    assert_eq!(
        std::fs::read(root.join("real/assets/data.txt")).unwrap(),
        b"new data"
    );
    assert_eq!(
        std::fs::read(root.join("real/nested/main.typ")).unwrap(),
        b"new main"
    );
}

#[test]
fn artifacts_are_written_to_their_paths_and_replaced_on_overwrite() {
    let result = compilation_result(b"first#pagebreak()second", true);
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    std::fs::create_dir(root.join("pages")).unwrap();
    let paths = [root.join("page-1.svg"), root.join("pages/page-2.svg")];
    std::fs::write(&paths[0], b"stale").unwrap();

    let error =
        write_compilation_artifacts_to_filesystem_paths(&result, &paths, WritePolicy::CreateNew)
            .unwrap_err();
    assert!(matches!(
        error,
        FilesystemWriteError::Io { ref path, ref completed, .. }
            if path == &paths[0] && completed.is_empty()
    ));
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"stale");

    write_compilation_artifacts_to_filesystem_paths(&result, &paths, WritePolicy::Overwrite)
        .unwrap();
    for (artifact, path) in result.iter().zip(&paths) {
        assert_eq!(std::fs::read(path).unwrap(), artifact.bytes);
    }
    assert_eq!(tree(&root), ["page-1.svg", "pages", "pages/page-2.svg"]);
}

#[test]
fn artifact_writes_reject_count_conflicts_and_failed_results_before_writing() {
    let result = compilation_result(b"first#pagebreak()second", true);
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);

    let issues = rejected(write_compilation_artifacts_to_filesystem_paths(
        &result,
        &[
            root.join("same.svg"),
            root.join("same.svg"),
            root.join("third.svg"),
        ],
        WritePolicy::Overwrite,
    ));
    assert_eq!(
        issues,
        [
            WriteIssue::PathCountMismatch {
                artifacts: 2,
                paths: 3
            },
            WriteIssue::PathConflict {
                first: root.join("same.svg"),
                second: root.join("same.svg"),
            },
        ]
    );

    assert!(tree(&root).is_empty());
}

#[test]
fn a_failed_artifact_write_leaves_no_temporary_file() {
    let result = compilation_result(b"= Written", false);
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let output = root.join("output.pdf");
    std::fs::create_dir(&output).unwrap();

    let error = write_compilation_artifacts_to_filesystem_paths(
        &result,
        std::slice::from_ref(&output),
        WritePolicy::Overwrite,
    )
    .unwrap_err();

    assert!(matches!(error, FilesystemWriteError::Io { ref path, .. } if path == &output));
    assert_eq!(tree(&root), ["output.pdf"]);
}

#[cfg(unix)]
#[test]
fn artifact_paths_resolve_symlinks_and_parent_components_natively() {
    use std::os::unix::fs::symlink;

    let result = compilation_result(b"= Written", false);
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    std::fs::create_dir_all(root.join("actual/nested")).unwrap();
    symlink(root.join("actual/nested"), root.join("linked")).unwrap();

    write_compilation_artifacts_to_filesystem_paths(
        &result,
        &[root.join("linked/../output.pdf")],
        WritePolicy::Overwrite,
    )
    .unwrap();

    assert_eq!(
        std::fs::read(root.join("actual/output.pdf")).unwrap(),
        result[0].bytes
    );
}

#[cfg(unix)]
#[test]
fn artifact_paths_need_not_be_unicode() {
    use std::os::unix::ffi::OsStringExt as _;

    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let path = root.join(std::ffi::OsString::from_vec(b"report-\xff.pdf".to_vec()));
    // APFS and HFS+ refuse names that are not UTF-8.
    if std::fs::write(&path, b"probe").is_err() {
        eprintln!("skipped: the temporary filesystem rejects non-UTF-8 names");
        return;
    }
    let result = compilation_result(b"= Written", false);

    write_compilation_artifacts_to_filesystem_paths(
        &result,
        std::slice::from_ref(&path),
        WritePolicy::Overwrite,
    )
    .unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), result[0].bytes);
}
