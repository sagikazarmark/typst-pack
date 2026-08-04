#![cfg(feature = "fs")]

use typst_pack::pack_archive::{CommitCertainty, StagingResidueStatus};
use typst_pack::{
    CompilationArtifactPathIssue, CompilationLimits, CompilationOutputSpecification,
    FilesystemMergePolicy, FilesystemPublicationPreflightIssue, Pack, PackCompilationRequest,
    PackExtractionSelection, PdfOutputSpecification, SvgOutputSpecification, compile,
    plan_compilation_artifact_publication, plan_pack_extraction,
    publish_compilation_artifact_plan_to_filesystem,
    publish_compilation_artifact_plan_to_filesystem_paths,
    publish_pack_extraction_plan_to_filesystem,
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

fn two_page_artifact_plan() -> typst_pack::CompilationArtifactPublicationPlan {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"first#pagebreak()second".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    plan_compilation_artifact_publication(report.result().unwrap()).unwrap()
}

#[test]
fn publish_new_tree_exposes_the_complete_plan_through_one_root_commit() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("published");
    let plan = extraction_plan();

    let receipt = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::PublishNewTree,
    )
    .unwrap();

    assert_eq!(receipt.commit_certainty(), CommitCertainty::Committed);
    assert_eq!(
        receipt.staging_residue_status(),
        StagingResidueStatus::Absent
    );
    assert_eq!(
        receipt.progress().committed_files(),
        [
            std::path::PathBuf::from("assets/data.txt"),
            std::path::PathBuf::from("main.typ"),
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
fn publish_new_tree_preflight_aggregates_an_existing_root_with_other_issues() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("published");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    let plan = extraction_plan();

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::PublishNewTree,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[0],
        FilesystemPublicationPreflightIssue::ExistingDestinationRoot { path }
            if path == &destination
    ));
    assert!(matches!(
        &issues[1],
        FilesystemPublicationPreflightIssue::ExistingTarget { relative_path }
            if relative_path == "main.typ"
    ));
    assert_eq!(
        error.phase(),
        typst_pack::FilesystemPlanPublicationPhase::Preflight
    );
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
fn publish_new_tree_preflight_validates_destination_components_before_staging() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("x".repeat(256));
    let plan = extraction_plan();

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::PublishNewTree,
    )
    .unwrap_err();

    assert!(error.preflight_issues().unwrap().iter().any(|issue| {
        matches!(
            issue,
            FilesystemPublicationPreflightIssue::DestinationComponentTooLong { path, .. }
                if path == &destination
        )
    }));
    assert_eq!(
        error.phase(),
        typst_pack::FilesystemPlanPublicationPhase::Preflight
    );
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
    let destination = temp_path(&directory).join("published");
    std::fs::create_dir_all(destination.join("assets")).unwrap();
    std::fs::write(destination.join("main.typ"), b"old main").unwrap();
    std::fs::write(destination.join("assets/data.txt"), b"old data").unwrap();
    std::fs::write(destination.join("unrelated.txt"), b"keep me").unwrap();
    let plan = extraction_plan();

    let error = publish_pack_extraction_plan_to_filesystem(
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
            FilesystemPublicationPreflightIssue::ExistingTarget { relative_path } => {
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
    assert_eq!(
        error.phase(),
        typst_pack::FilesystemPlanPublicationPhase::Preflight
    );
    assert_eq!(error.failed_target(), None);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
    assert_eq!(error.staging_residue(), None);
    assert!(error.progress().committed_files().is_empty());
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"old main"
    );
    assert_eq!(
        std::fs::read(destination.join("assets/data.txt")).unwrap(),
        b"old data"
    );

    let receipt = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap();

    assert_eq!(
        receipt.progress().committed_files(),
        [
            std::path::PathBuf::from("assets/data.txt"),
            std::path::PathBuf::from("main.typ"),
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
    let destination = temp_path(&directory).join("published");
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

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemPublicationPreflightIssue::ExistingTarget { relative_path }
            if relative_path == "existing.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemPublicationPreflightIssue::ConflictingAncestor { relative_path, .. }
            if relative_path == "blocked/child.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemPublicationPreflightIssue::ComponentTooLong { relative_path, .. }
            if relative_path.ends_with("child.txt")
    )));
    assert!(error.progress().committed_files().is_empty());
    assert!(!destination.join("main.typ").exists());
    assert_eq!(
        std::fs::read(destination.join("existing.txt")).unwrap(),
        b"old"
    );
}

#[test]
fn artifact_plans_publish_with_workflow_specific_progress() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Published".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let plan = plan_compilation_artifact_publication(report.result().unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");

    let receipt = publish_compilation_artifact_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::PublishNewTree,
    )
    .unwrap();

    assert_eq!(
        receipt.progress().committed_files(),
        [std::path::PathBuf::from("output.pdf")]
    );
    assert_eq!(
        std::fs::read(destination.join("output.pdf")).unwrap(),
        plan.entries()[0].bytes()
    );
}

#[test]
fn artifact_plans_publish_through_caller_selected_platform_paths() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Published".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let plan = plan_compilation_artifact_publication(report.result().unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");
    let paths = vec![std::path::PathBuf::from("reports/custom-name.pdf")];

    let receipt = publish_compilation_artifact_plan_to_filesystem_paths(
        &plan,
        &destination,
        &paths,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap();

    assert_eq!(
        receipt.progress().committed_files(),
        [std::path::PathBuf::from("reports/custom-name.pdf")]
    );
    assert_eq!(
        std::fs::read(destination.join(&paths[0])).unwrap(),
        plan.entries()[0].bytes()
    );
}

#[test]
fn caller_selected_artifact_paths_reject_count_and_tree_conflicts_before_writes() {
    let plan = two_page_artifact_plan();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");

    let mismatch = publish_compilation_artifact_plan_to_filesystem_paths(
        &plan,
        &destination,
        &[std::path::PathBuf::from("one.svg")],
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();
    assert_eq!(mismatch.path_count_mismatch(), Some((2, 1)));

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
        let error = publish_compilation_artifact_plan_to_filesystem_paths(
            &plan,
            &destination,
            &paths,
            FilesystemMergePolicy::MergeReplaceExactFiles,
        )
        .unwrap_err();
        assert!(matches!(
            error.issues(),
            Some([CompilationArtifactPathIssue::PathConflict { .. }])
        ));
        assert!(!destination.exists());
    }

    for invalid in [
        std::path::PathBuf::from("/absolute.svg"),
        std::path::PathBuf::from("../parent.svg"),
    ] {
        let error = publish_compilation_artifact_plan_to_filesystem_paths(
            &plan,
            &destination,
            &[invalid.clone(), std::path::PathBuf::from("valid.svg")],
            FilesystemMergePolicy::MergeReplaceExactFiles,
        )
        .unwrap_err();
        let error = error
            .publication_error()
            .expect("path count matches and selected paths do not conflict");
        assert!(error.preflight_issues().unwrap().iter().any(|issue| {
            matches!(
                issue,
                FilesystemPublicationPreflightIssue::InvalidRelativePath { relative_path }
                    if relative_path == &invalid
            )
        }));
        assert!(error.progress().committed_files().is_empty());
        assert!(!destination.exists());
    }
}

#[cfg(unix)]
#[test]
fn caller_selected_artifact_progress_preserves_non_unicode_platform_paths() {
    use std::os::unix::ffi::OsStringExt as _;

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Published".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let report = compile(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let plan = plan_compilation_artifact_publication(report.result().unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("artifacts");
    let relative =
        std::path::PathBuf::from(std::ffi::OsString::from_vec(b"report-\xff.pdf".to_vec()));

    let receipt = publish_compilation_artifact_plan_to_filesystem_paths(
        &plan,
        &destination,
        std::slice::from_ref(&relative),
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap();

    assert_eq!(
        receipt.progress().committed_files(),
        std::slice::from_ref(&relative)
    );
    assert_eq!(
        std::fs::read(destination.join(relative)).unwrap(),
        plan.entries()[0].bytes()
    );
}

#[cfg(unix)]
#[test]
fn preflight_rejects_symlinked_targets_and_ancestors_without_writes() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let root = temp_path(&directory);
    let destination = root.join("published");
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

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    )
    .unwrap_err();
    let issues = error.preflight_issues().unwrap();

    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemPublicationPreflightIssue::ConflictingAncestor { relative_path, .. }
            if relative_path == "linked/child.txt"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        FilesystemPublicationPreflightIssue::ConflictingTarget { relative_path, .. }
            if relative_path == "target.txt"
    )));
    assert!(error.progress().committed_files().is_empty());
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
    let destination = root.join("published");
    let probe = root.join("case-probe");
    std::fs::write(&probe, b"probe").unwrap();
    let case_insensitive = root.join("CASE-PROBE").exists();
    std::fs::remove_file(probe).unwrap();

    let result = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeReplaceExactFiles,
    );

    if case_insensitive {
        let error = result.unwrap_err();
        assert!(error.preflight_issues().unwrap().iter().any(|issue| {
            matches!(issue, FilesystemPublicationPreflightIssue::PathAlias { .. })
        }));
        assert!(!destination.exists());
    } else {
        let receipt = result.unwrap();
        assert_eq!(
            receipt.progress().committed_files(),
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
    let destination = temp_path(&directory).join("published");

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();
    let reserved_count = error
        .preflight_issues()
        .unwrap()
        .iter()
        .filter(|issue| {
            matches!(
                issue,
                FilesystemPublicationPreflightIssue::ReservedName { .. }
            )
        })
        .count();

    assert_eq!(reserved_count, 3);
    assert!(!destination.exists());
}

#[cfg(windows)]
#[test]
fn windows_reserved_destination_root_is_rejected_before_staging() {
    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("CON");

    let error = publish_pack_extraction_plan_to_filesystem(
        &extraction_plan(),
        &destination,
        FilesystemMergePolicy::PublishNewTree,
    )
    .unwrap_err();

    assert!(error.preflight_issues().unwrap().iter().any(|issue| {
        matches!(
            issue,
            FilesystemPublicationPreflightIssue::DestinationReservedName { path, component }
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
    let destination = temp_path(&directory).join("published");

    let error = publish_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();

    assert!(
        error.preflight_issues().unwrap().iter().any(|issue| {
            matches!(issue, FilesystemPublicationPreflightIssue::PathAlias { .. })
        })
    );
    assert!(!destination.exists());
}
