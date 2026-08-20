#![cfg(feature = "fs")]

use typst_pack::pack_archive::{CommitCertainty, StagingResidueStatus};
use typst_pack::{
    CompilationArtifactWriteIssue, CompilationLimits, CompilationOutputSpecification,
    FilesystemMergePolicy, FilesystemWritePreflightIssue, Pack, PackCompilationRequest,
    PackExtractionSelection, PdfOutputSpecification, SvgOutputSpecification, WriteKeyOutcome,
    compile_with_limits, plan_pack_extraction, resolve_filesystem_write_paths,
    write_compilation_artifacts_to_filesystem_paths, write_pack_extraction_plan_to_filesystem,
};

fn extraction_plan() -> typst_pack::PackExtractionPlan {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"new main".to_vec())
        .unwrap()
        .file("assets/data.txt", b"new data".to_vec())
        .unwrap()
        .build()
        .unwrap();
    plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap()
}

fn temp_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    std::fs::canonicalize(directory.path()).unwrap()
}

fn two_page_compilation_result() -> typst_pack::CompilationResult {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"first#pagebreak()second".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    report.result().unwrap().clone()
}

#[test]
fn write_paths_resolve_a_common_destination_and_relative_targets() {
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    std::fs::create_dir_all(root.join("first")).unwrap();
    std::fs::create_dir_all(root.join("second")).unwrap();
    let targets = [
        root.join("first/output-1.svg"),
        root.join("second/output-2.svg"),
    ];

    let (destination, relative_paths) = resolve_filesystem_write_paths(&targets).unwrap();

    assert_eq!(destination, root);
    assert_eq!(
        relative_paths,
        [
            std::path::PathBuf::from("first/output-1.svg"),
            std::path::PathBuf::from("second/output-2.svg"),
        ]
    );
}

#[test]
fn write_path_resolution_retains_the_unresolved_output_directory() {
    let directory = tempfile::tempdir().unwrap();
    let unresolved = directory.path().join("missing");
    let target = unresolved.join("output.svg");

    let error = resolve_filesystem_write_paths(&[target]).unwrap_err();

    assert!(matches!(
        error,
        typst_pack::FilesystemWritePathError::OutputDirectory { path, .. }
            if path == unresolved
    ));
}

#[cfg(unix)]
#[test]
fn write_path_resolution_rejects_a_target_without_a_file_name() {
    let target = std::path::PathBuf::from("/");

    let error = resolve_filesystem_write_paths(std::slice::from_ref(&target)).unwrap_err();

    assert!(matches!(
        error,
        typst_pack::FilesystemWritePathError::OutputPathDoesNotNameFile { path }
            if path == target
    ));
}

#[test]
fn write_new_tree_exposes_the_complete_plan_through_one_root_commit() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    let plan = extraction_plan();

    let receipt = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::WriteNewTree,
    )
    .unwrap();

    assert_eq!(receipt.pack_identity(), *plan.pack_identity());
    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| (entry.relative_path(), entry.outcome()))
            .collect::<Vec<_>>(),
        [
            ("assets/data.txt", WriteKeyOutcome::Created),
            ("main.typ", WriteKeyOutcome::Created),
        ]
    );
    assert_eq!(
        std::fs::read(destination.join("assets/data.txt")).unwrap(),
        b"new data"
    );
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"new main"
    );
    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        1,
        "the sibling staging directory must be consumed by the root commit"
    );
}

#[test]
fn write_new_tree_preflight_aggregates_an_existing_root_with_other_issues() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    let plan = extraction_plan();

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::WriteNewTree,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[0],
        FilesystemWritePreflightIssue::ExistingDestinationRoot { path }
            if path == &destination
    ));
    assert!(matches!(
        &issues[1],
        FilesystemWritePreflightIssue::ExistingTarget { relative_path }
            if relative_path == "main.typ"
    ));
    assert_eq!(error.phase(), typst_pack::FilesystemWritePhase::Preflight);
    assert_eq!(error.failed_target(), None);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
    assert_eq!(
        std::fs::read_dir(directory.path()).unwrap().count(),
        1,
        "preflight must not create sibling staging"
    );
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"existing"
    );
}

#[test]
fn write_new_tree_preflight_validates_destination_components_before_staging() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("x".repeat(256));
    let plan = extraction_plan();

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::WriteNewTree,
    )
    .unwrap_err();

    assert!(error.preflight_issues().unwrap().iter().any(|issue| {
        matches!(
            issue,
            FilesystemWritePreflightIssue::DestinationComponentTooLong { path, .. }
                if path == &destination
        )
    }));
    assert_eq!(error.phase(), typst_pack::FilesystemWritePhase::Preflight);
    assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn merge_policies_remain_distinct_and_preserve_unrelated_content() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir_all(destination.join("assets")).unwrap();
    std::fs::write(destination.join("main.typ"), b"old main").unwrap();
    std::fs::write(destination.join("assets/data.txt"), b"old data").unwrap();
    std::fs::write(destination.join("unrelated.txt"), b"keep me").unwrap();
    let plan = extraction_plan();

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();

    let existing = error
        .preflight_issues()
        .unwrap()
        .iter()
        .filter_map(|issue| match issue {
            FilesystemWritePreflightIssue::ExistingTarget { relative_path } => {
                Some(relative_path.as_path())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        existing,
        [
            std::path::Path::new("assets/data.txt"),
            std::path::Path::new("main.typ"),
        ]
    );
    assert_eq!(error.phase(), typst_pack::FilesystemWritePhase::Preflight);
    assert_eq!(error.failed_target(), None);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
    assert_eq!(error.staging_residue(), None);
    assert!(error.progress().completed().is_empty());
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"old main"
    );
    assert_eq!(
        std::fs::read(destination.join("assets/data.txt")).unwrap(),
        b"old data"
    );
    std::fs::remove_file(destination.join("assets/data.txt")).unwrap();

    let receipt = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap();

    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| (entry.relative_path(), entry.outcome()))
            .collect::<Vec<_>>(),
        [
            ("assets/data.txt", WriteKeyOutcome::Written),
            ("main.typ", WriteKeyOutcome::Written),
        ]
    );
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"new main"
    );
    assert_eq!(
        std::fs::read(destination.join("assets/data.txt")).unwrap(),
        b"new data"
    );
    assert_eq!(
        std::fs::read(destination.join("unrelated.txt")).unwrap(),
        b"keep me"
    );
}

#[test]
fn preflight_aggregates_detectable_issues_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("existing.txt"), b"old").unwrap();
    std::fs::write(destination.join("blocked"), b"not a directory").unwrap();
    let oversized_component = "x".repeat(256);
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("existing.txt", b"new".to_vec())
        .unwrap()
        .file("blocked/child.txt", b"blocked".to_vec())
        .unwrap()
        .file(
            format!("{oversized_component}/child.txt"),
            b"large".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemWritePreflightIssue::ExistingTarget { relative_path }
            if relative_path == "existing.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemWritePreflightIssue::ConflictingAncestor { relative_path, .. }
            if relative_path == "blocked/child.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemWritePreflightIssue::ComponentTooLong { relative_path, .. }
            if relative_path.ends_with("child.txt")
    )));
    assert!(error.progress().completed().is_empty());
    assert!(!destination.join("main.typ").exists());
    assert_eq!(
        std::fs::read(destination.join("existing.txt")).unwrap(),
        b"old"
    );
}

#[test]
fn compilation_artifacts_write_as_a_new_tree_through_caller_selected_platform_paths() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Written".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let result = report.result().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");
    let paths = vec![std::path::PathBuf::from("reports/custom-name.pdf")];

    let receipt = write_compilation_artifacts_to_filesystem_paths(
        result,
        &destination,
        &paths,
        FilesystemMergePolicy::WriteNewTree,
    )
    .unwrap();

    assert_eq!(
        receipt.compilation_result_identity(),
        result.result_identity()
    );
    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| (entry.artifact_index(), entry.outcome()))
            .collect::<Vec<_>>(),
        [(0, WriteKeyOutcome::Created)]
    );
    assert_eq!(
        std::fs::read(destination.join(&paths[0])).unwrap(),
        result.artifacts()[0].bytes()
    );
}

#[test]
fn caller_selected_artifact_paths_reject_count_and_tree_conflicts_before_writes() {
    let result = two_page_compilation_result();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");

    let mismatch = write_compilation_artifacts_to_filesystem_paths(
        &result,
        &destination,
        &[std::path::PathBuf::from("one.svg")],
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();
    assert_eq!(
        mismatch.issues(),
        Some(
            [CompilationArtifactWriteIssue::PathCountMismatch {
                artifact_count: 2,
                path_count: 1,
            }]
            .as_slice()
        )
    );

    let count_and_conflict = write_compilation_artifacts_to_filesystem_paths(
        &result,
        &destination,
        &[
            std::path::PathBuf::from("same.svg"),
            std::path::PathBuf::from("same.svg"),
            std::path::PathBuf::from("third.svg"),
        ],
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();
    assert_eq!(
        count_and_conflict.issues(),
        Some(
            [
                CompilationArtifactWriteIssue::PathCountMismatch {
                    artifact_count: 2,
                    path_count: 3,
                },
                CompilationArtifactWriteIssue::PathConflict {
                    first_path: std::path::PathBuf::from("same.svg"),
                    second_path: std::path::PathBuf::from("same.svg"),
                },
            ]
            .as_slice()
        )
    );

    for paths in [
        vec![
            std::path::PathBuf::from("same.svg"),
            std::path::PathBuf::from("same.svg"),
        ],
        vec![
            std::path::PathBuf::from("tree"),
            std::path::PathBuf::from("tree/page.svg"),
        ],
    ] {
        let error = write_compilation_artifacts_to_filesystem_paths(
            &result,
            &destination,
            &paths,
            FilesystemMergePolicy::MergeReplaceExactFiles,
        )
        .unwrap_err();
        assert!(matches!(
            error.issues(),
            Some([CompilationArtifactWriteIssue::PathConflict { .. }])
        ));
        assert!(!destination.exists());
    }

    for invalid in [
        std::path::PathBuf::from("/absolute.svg"),
        std::path::PathBuf::from("../parent.svg"),
    ] {
        let error = write_compilation_artifacts_to_filesystem_paths(
            &result,
            &destination,
            &[invalid.clone(), std::path::PathBuf::from("valid.svg")],
            FilesystemMergePolicy::MergeReplaceExactFiles,
        )
        .unwrap_err();
        let error = error
            .write_error()
            .expect("path count matches and selected paths do not conflict");
        assert!(error.preflight_issues().unwrap().iter().any(|issue| {
            matches!(
                issue,
                FilesystemWritePreflightIssue::InvalidRelativePath { relative_path }
                    if relative_path.as_path() == invalid.as_path()
            )
        }));
        assert!(error.progress().completed().is_empty());
        assert!(!destination.exists());
    }
}

#[test]
fn rejected_compilation_results_are_not_written() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#unknown-function()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");

    let error = write_compilation_artifacts_to_filesystem_paths(
        report.result().unwrap(),
        &destination,
        &[std::path::PathBuf::from("output.pdf")],
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();

    assert_eq!(
        error.issues(),
        Some(
            [
                CompilationArtifactWriteIssue::RejectedCompilationResult,
                CompilationArtifactWriteIssue::PathCountMismatch {
                    artifact_count: 0,
                    path_count: 1,
                },
            ]
            .as_slice()
        )
    );
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn caller_selected_artifact_write_supports_non_unicode_platform_paths() {
    use std::os::unix::ffi::OsStringExt as _;

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Written".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let result = report.result().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");
    let relative =
        std::path::PathBuf::from(std::ffi::OsString::from_vec(b"report-\xff.pdf".to_vec()));

    let receipt = write_compilation_artifacts_to_filesystem_paths(
        result,
        &destination,
        std::slice::from_ref(&relative),
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap();

    assert_eq!(receipt.completed()[0].artifact_index(), 0);
    assert_eq!(
        std::fs::read(destination.join(relative)).unwrap(),
        result.artifacts()[0].bytes()
    );
}

#[cfg(unix)]
#[test]
fn preflight_rejects_symlinked_targets_and_ancestors_without_writes() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let destination = root.join("written");
    let outside = root.join("outside");
    std::fs::create_dir(&destination).unwrap();
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, destination.join("linked")).unwrap();
    symlink(outside.join("target.txt"), destination.join("target.txt")).unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("linked/child.txt", b"child".to_vec())
        .unwrap()
        .file("target.txt", b"target".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemWritePreflightIssue::ConflictingAncestor { relative_path, .. }
            if relative_path == "linked/child.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemWritePreflightIssue::ConflictingTarget { relative_path, .. }
            if relative_path == "target.txt"
    )));
    assert!(error.progress().completed().is_empty());
    assert!(!destination.join("main.typ").exists());
    assert!(!outside.join("child.txt").exists());
    assert!(!outside.join("target.txt").exists());
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn native_case_aliases_are_aggregated_before_writes() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("Case.txt", b"upper".to_vec())
        .unwrap()
        .file("case.txt", b"lower".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let destination = root.join("written");
    let probe = root.join("case-probe");
    std::fs::write(&probe, b"probe").unwrap();
    let case_insensitive = root.join("CASE-PROBE").exists();
    std::fs::remove_file(probe).unwrap();

    let result = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    );

    if case_insensitive {
        let error = result.unwrap_err();
        assert!(
            error
                .preflight_issues()
                .unwrap()
                .iter()
                .any(|issue| { matches!(issue, FilesystemWritePreflightIssue::PathAlias { .. }) })
        );
        assert!(!destination.exists());
    } else {
        let receipt = result.unwrap();
        assert_eq!(
            receipt
                .completed()
                .iter()
                .map(|entry| entry.relative_path())
                .collect::<Vec<_>>(),
            ["Case.txt", "case.txt", "main.typ"]
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_reserved_names_are_aggregated_before_writes() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("CON.txt", b"reserved".to_vec())
        .unwrap()
        .file("nested/trailing. ", b"reserved".to_vec())
        .unwrap()
        .file("COM¹", b"reserved".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();
    let reserved_count = error
        .preflight_issues()
        .unwrap()
        .iter()
        .filter(|issue| matches!(issue, FilesystemWritePreflightIssue::ReservedName { .. }))
        .count();

    assert_eq!(reserved_count, 3);
    assert!(!destination.exists());
}

#[cfg(windows)]
#[test]
fn windows_reserved_destination_root_is_rejected_before_staging() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("CON");

    let error = write_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        FilesystemMergePolicy::WriteNewTree,
    )
    .unwrap_err();

    assert!(error.preflight_issues().unwrap().iter().any(|issue| {
        matches!(
            issue,
            FilesystemWritePreflightIssue::DestinationReservedName { path, component }
                if path == &destination && component == "CON"
        )
    }));
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_normalization_aliases_are_aggregated_before_writes() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("é.txt", b"composed".to_vec())
        .unwrap()
        .file("e\u{301}.txt", b"decomposed".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");

    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();

    assert!(
        error
            .preflight_issues()
            .unwrap()
            .iter()
            .any(|issue| { matches!(issue, FilesystemWritePreflightIssue::PathAlias { .. }) })
    );
    assert!(!destination.exists());
}
