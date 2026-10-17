//! Deterministic replay for minimized lifecycle fuzz cases.

use std::num::NonZeroUsize;

use typst_pack::pack_archive::{DecodeError, DecodeLimits, decode};
use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationRequestIssue, FontContainer,
    FontContainerError, Pack, PackArchiveBytes, PackCompilationRequest, PackageTree,
    PackageTreeIssue, PageSelection, PngOutputSpecification, ProjectSnapshotAssembly,
    ProjectSnapshotIssue, compile_with_limits,
};

/// Resolves a temporary directory to a destination the write preflight accepts.
///
/// macOS temp paths can start with `/var`, a symlink to `/private/var`.
fn temp_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    std::fs::canonicalize(directory.path()).unwrap()
}

#[cfg(feature = "opendal")]
#[test]
fn compilation_artifact_key_regressions_store_exactly_at_the_given_key() {
    use std::future::Future;

    use typst_pack::opendal::{Error, Operator, WritePolicy, write_compilation_artifacts};

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
    let write = |op: &Operator, keys: &[&str]| {
        let mut future = std::pin::pin!(write_compilation_artifacts(
            op,
            "artifacts/",
            &result,
            keys,
            WritePolicy::CreateOrVerify,
        ));
        let waker = std::task::Waker::noop();
        match future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(waker))
        {
            std::task::Poll::Ready(result) => result,
            std::task::Poll::Pending => panic!("the Memory service never pends"),
        }
    };

    // Percent signs and invisible characters are literal key bytes.
    for keys in [["tree%", "tree%/page%2F.svg"], ["a\u{feff}", "a\u{200b}"]] {
        let op = Operator::new(opendal::services::Memory::default()).unwrap();
        write(&op, &keys).unwrap();
        for key in keys {
            let path = format!("artifacts/{key}");
            let mut stat = std::pin::pin!(op.stat(&path));
            let ready = stat
                .as_mut()
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
            assert!(matches!(ready, std::task::Poll::Ready(Ok(_))), "{key:?}");
        }
    }

    // Keys OpenDAL would normalize into another key are refused.
    let op = Operator::new(opendal::services::Memory::default()).unwrap();
    for key in ["", " a", "a ", "a\u{00a0}", "a/./b", "a//b"] {
        let error = write(&op, &[key, "valid.svg"]).unwrap_err();
        assert!(matches!(error.error, Error::InvalidPath { .. }), "{key:?}");
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
    use typst_pack::{
        FilesystemReadError, FilesystemWriteError, PackExtractionSelection, ReadLimits,
        WritePolicy, plan_pack_extraction, read_filesystem_project,
        write_pack_extraction_plan_to_filesystem,
    };

    let malformed = tempfile::tempdir().unwrap();
    std::fs::write(malformed.path().join("main.typ"), b"main").unwrap();
    std::fs::write(malformed.path().join(".typkignore"), [0xff]).unwrap();
    assert!(matches!(
        read_filesystem_project(malformed.path(), "main.typ", ReadLimits::default()),
        Err(FilesystemReadError::InvalidIgnoreFile { .. })
    ));

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"published".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    for policy in [WritePolicy::CreateNew, WritePolicy::Overwrite] {
        for existing_root in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let destination = temp_path(&directory).join("written");
            if existing_root {
                std::fs::create_dir(&destination).unwrap();
            }
            write_pack_extraction_plan_to_filesystem(&plan, &destination, policy)
                .unwrap_or_else(|error| panic!("write regression {policy:?} failed: {error}"));
            assert_eq!(
                std::fs::read(destination.join("main.typ")).unwrap(),
                b"published"
            );
        }
    }

    let directory = tempfile::tempdir().unwrap();
    let destination = temp_path(&directory).join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    let error =
        write_pack_extraction_plan_to_filesystem(&plan, &destination, WritePolicy::CreateNew)
            .unwrap_err();
    assert!(matches!(error, FilesystemWriteError::Rejected { .. }));
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"existing"
    );
}
