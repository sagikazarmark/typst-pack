//! Deterministic replay for minimized lifecycle fuzz cases.

use std::num::NonZeroUsize;

use typst_pack::pack_archive::{DecodeError, DecodeLimits, decode};
use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationRequestIssue, FontContainer,
    FontContainerError, Pack, PackArchiveBytes, PackCompilationRequest, PackageTree,
    PackageTreeIssue, PageSelection, PngOutputSpecification, ProjectSnapshotAssembly,
    ProjectSnapshotIssue, compile_with_limits,
};

#[cfg(feature = "opendal")]
#[test]
fn opendal_location_regressions_preserve_canonical_objects_and_alias_rejections() {
    use typst_pack::opendal::{Location, LocationError, OperatorBinding};

    let binding = OperatorBinding::new("fuzz").unwrap();
    for path in ["", "/", "a/./b", "a\u{feff}", "a\u{200b}", "café/%"] {
        let result = Location::from_operation_path(binding.clone(), path);
        if path == "a/./b" {
            assert_eq!(result, Err(LocationError::DotSegment { index: 2 }));
        } else {
            let location = result.unwrap();
            assert_eq!(Location::parse(location.to_string()).unwrap(), location);
        }
    }

    for (path, index) in [(" a", 0), ("a ", 1), ("a\u{00a0}", 1)] {
        assert_eq!(
            Location::from_operation_path(binding.clone(), path),
            Err(LocationError::NormalizationAlias { index })
        );
    }
}

#[cfg(feature = "opendal")]
#[test]
fn compilation_artifact_key_regressions_replay_through_request_construction() {
    use typst_pack::opendal::Location;
    use typst_pack::opendal::write::{
        CompilationArtifactKeyIssue, CompilationArtifactWriteRequest,
        CompilationArtifactWriteRequestIssue, WritePolicy,
    };

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#pagebreak()".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let result = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Svg(Default::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap()
    .result()
    .unwrap()
    .clone();
    let destination: Location = "fuzz:/artifacts/".parse().unwrap();

    for keys in [["tree%", "tree%/page%2F.svg"], ["a\u{feff}", "a\u{200b}"]] {
        let request = CompilationArtifactWriteRequest::new(
            &result,
            destination.clone(),
            keys,
            WritePolicy::CreateOrVerify,
        )
        .unwrap();
        assert_eq!(request.artifact_keys(), keys);
    }

    for (key, reason) in [
        ("", CompilationArtifactKeyIssue::Empty),
        (
            " a",
            CompilationArtifactKeyIssue::NormalizationAlias { index: 0 },
        ),
        (
            "a\u{00a0}",
            CompilationArtifactKeyIssue::NormalizationAlias { index: 1 },
        ),
    ] {
        let rejection = CompilationArtifactWriteRequest::new(
            &result,
            destination.clone(),
            [key, "valid.svg"],
            WritePolicy::CreateOrVerify,
        )
        .unwrap_err();
        assert!(matches!(
            rejection.issues(),
            [CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 0,
                reason: actual,
                ..
            }] if *actual == reason
        ));
    }
}

#[test]
fn pack_archive_decoding_regressions_replay_through_the_public_decoder() {
    let cases: &[(&str, &[u8])] = &[
        ("empty", b""),
        ("truncated-signature", b"PK"),
        ("truncated-local-header", b"PK\x03\x04\x14\0"),
        (
            "empty-end-record",
            b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        ),
    ];

    for (name, bytes) in cases {
        let archive = PackArchiveBytes::from_vec(bytes.to_vec());
        assert!(
            matches!(
                decode(&archive, DecodeLimits::reference_v1()),
                Err(DecodeError::Archive(_))
            ),
            "archive regression {name} changed phase"
        );
    }
}

#[cfg(feature = "package-reading")]
#[test]
fn package_archive_expansion_regressions_replay_through_the_public_expander() {
    let spec: typst::syntax::package::PackageSpec = "@preview/fuzz:1.0.0".parse().unwrap();
    let cases: &[(&str, &[u8])] = &[
        ("empty", b""),
        ("truncated-gzip", b"\x1f\x8b"),
        ("invalid-gzip-flags", b"\x1f\x8b\x08\xff\0\0\0\0\0\0"),
    ];

    for (name, bytes) in cases {
        assert!(
            matches!(
                typst_pack::expand_package_archive(
                    spec.clone(),
                    bytes,
                    typst_pack::PackageExpansionLimits::reference_v1(),
                ),
                Err(typst_pack::PackageReadError::MalformedArchive { .. }),
            ),
            "package archive regression {name} was unexpectedly accepted"
        );
    }
}

#[test]
fn font_and_semantic_constructor_regressions_replay_without_partial_values() {
    for (name, bytes) in [
        ("empty-font", b"".as_slice()),
        ("truncated-opentype", b"OTTO".as_slice()),
        ("truncated-collection", b"ttcf\0\x01".as_slice()),
    ] {
        assert!(
            matches!(
                FontContainer::new(bytes.to_vec()),
                Err(FontContainerError::NoReadableFace)
            ),
            "font regression {name} was unexpectedly accepted"
        );
    }

    let duplicate_project = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"main".to_vec()),
            ("./same.typ", b"first".to_vec()),
            ("same.typ", b"second".to_vec()),
        ])
        .unwrap_err();
    assert_eq!(
        duplicate_project.issues(),
        [ProjectSnapshotIssue::DuplicatePath {
            path: "same.typ".to_owned(),
        }]
    );
    let pack_path = "nested/archive.typk/file.typ";
    let invalid_project = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"main".to_vec()),
            (pack_path, b"archive".to_vec()),
        ])
        .unwrap_err();
    assert!(matches!(
        invalid_project.issues(),
        [ProjectSnapshotIssue::InvalidPath { path, .. }] if path == pack_path
    ));

    let duplicate_package = PackageTree::from_owned_entries([
        ("./same.typ", b"first".to_vec()),
        ("same.typ", b"second".to_vec()),
    ])
    .unwrap_err();
    assert_eq!(
        duplicate_package.issues(),
        [PackageTreeIssue::DuplicatePath {
            path: "same.typ".to_owned(),
        }]
    );
    let invalid_package = PackageTree::from_owned_entries([
        ("lib.typ", b"library".to_vec()),
        ("../escape.typ", b"escape".to_vec()),
    ])
    .unwrap_err();
    assert!(matches!(
        invalid_package.issues(),
        [PackageTreeIssue::InvalidPath { path, .. }] if path == "../escape.typ"
    ));
}

#[test]
fn compilation_request_regressions_replay_both_acceptance_and_rejection() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"accepted request fuzz regression".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let accepted = compile_with_limits(
        PackCompilationRequest::new(
            pack.clone(),
            CompilationOutputSpecification::Svg(Default::default()),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    assert!(accepted.result().is_some());

    let rejected = compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Png(PngOutputSpecification {
                page_selection: PageSelection::new(vec![
                    Some(NonZeroUsize::new(2).unwrap())..=Some(NonZeroUsize::new(1).unwrap()),
                ]),
                ..PngOutputSpecification::default()
            }),
        ),
        CompilationLimits::reference_v1(),
    );
    let rejection = rejected.unwrap_err();
    assert!(matches!(
        rejection.issues(),
        [CompilationRequestIssue::InvalidPageRange { start, end }]
            if start.get() == 2 && end.get() == 1
    ));
}

#[cfg(feature = "fs")]
#[test]
fn typkignore_and_write_state_regressions_replay_natively() {
    use typst_pack::pack_archive::CommitCertainty;
    use typst_pack::{
        FilesystemMergePolicy, FilesystemProjectLimits, FilesystemProjectReadError,
        PackExtractionSelection, plan_pack_extraction, read_filesystem_project,
        write_pack_extraction_plan_to_filesystem,
    };

    let malformed = tempfile::tempdir().unwrap();
    std::fs::write(malformed.path().join("main.typ"), b"main").unwrap();
    std::fs::write(malformed.path().join(".typkignore"), [0xff]).unwrap();
    assert!(matches!(
        read_filesystem_project(
            malformed.path(),
            "main.typ",
            FilesystemProjectLimits::reference_v1(),
        ),
        Err(FilesystemProjectReadError::InvalidPolicy { .. })
    ));

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"published".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    for policy in [
        FilesystemMergePolicy::MergeCreateOnly,
        FilesystemMergePolicy::MergeReplaceExactFiles,
        FilesystemMergePolicy::WriteNewTree,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("written");
        if policy != FilesystemMergePolicy::WriteNewTree {
            std::fs::create_dir(&destination).unwrap();
        }
        let receipt = write_pack_extraction_plan_to_filesystem(&plan, &destination, policy)
            .unwrap_or_else(|error| panic!("write regression {policy:?} failed: {error}"));
        assert_eq!(receipt.completed().len(), 1);
        assert_eq!(
            std::fs::read(destination.join("main.typ")).unwrap(),
            b"published"
        );
    }

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    let error = write_pack_extraction_plan_to_filesystem(
        &plan,
        &destination,
        FilesystemMergePolicy::MergeCreateOnly,
    )
    .unwrap_err();
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"existing"
    );
}
