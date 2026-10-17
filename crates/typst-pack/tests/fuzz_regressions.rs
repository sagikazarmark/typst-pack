//! Deterministic replay for minimized lifecycle fuzz cases.

use std::num::NonZeroUsize;

use typst_pack::archive::{DecodeError, DecodeLimits, decode};
use typst_pack::compile::{Output, OutputFormat, PageSelection, RequestIssue};
#[cfg(feature = "fs")]
use typst_pack::fs;
use typst_pack::{Pack, PackIssue, ProjectSnapshot, compile, font, package};

/// Resolves a temporary directory to a destination the write preflight accepts.
///
/// macOS temp paths can start with `/var`, a symlink to `/private/var`.
#[cfg(feature = "fs")]
fn temp_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    std::fs::canonicalize(directory.path()).unwrap()
}

#[cfg(feature = "opendal")]
#[test]
fn compilation_artifact_key_regressions_store_exactly_at_the_given_key() {
    use std::future::Future;

    use typst_pack::opendal::{Error, Operator, WritePolicy, write_artifacts};

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#pagebreak()".to_vec())
        .build()
        .unwrap();
    let result = compile(&compile::Request::new(pack, Output::new(OutputFormat::Svg)))
        .unwrap()
        .artifacts;
    let write = |op: &Operator, keys: &[&str]| {
        let mut future = std::pin::pin!(write_artifacts(
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
        assert!(
            matches!(
                decode(bytes, DecodeLimits::default()),
                Err(DecodeError::Zip(_) | DecodeError::MissingManifest)
            ),
            "archive regression {name} changed phase"
        );
    }
}

#[cfg(feature = "package-reading")]
#[test]
fn package_archive_expansion_regressions_replay_through_the_public_expander() {
    let cases: &[(&str, &[u8])] = &[
        ("empty", b""),
        ("truncated-gzip", b"\x1f\x8b"),
        ("invalid-gzip-flags", b"\x1f\x8b\x08\xff\0\0\0\0\0\0"),
    ];

    for (name, bytes) in cases {
        assert!(
            matches!(
                typst_pack::package::registry::expand_archive(
                    bytes,
                    typst_pack::ReadLimits::default(),
                ),
                Err(typst_pack::package::registry::ExpandError::Malformed(_)),
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
            font::Container::new(bytes.to_vec()).is_none(),
            "font regression {name} was unexpectedly accepted"
        );
    }

    let duplicate_project = ProjectSnapshot::new(
        "main.typ",
        [
            ("main.typ", b"main".to_vec()),
            ("./same.typ", b"first".to_vec()),
            ("same.typ", b"second".to_vec()),
        ],
    )
    .unwrap_err();
    assert_eq!(
        duplicate_project.issues.as_slice(),
        [PackIssue::DuplicatePath {
            path: "same.typ".to_owned(),
        }]
    );
    let pack_path = "nested/archive.typk/file.typ";
    let invalid_project = ProjectSnapshot::new(
        "main.typ",
        [
            ("main.typ", b"main".to_vec()),
            (pack_path, b"archive".to_vec()),
        ],
    )
    .unwrap_err();
    assert!(matches!(
        invalid_project.issues.as_slice(),
        [PackIssue::InvalidPath { path, .. }] if path == pack_path
    ));

    let duplicate_package = package::Tree::new([
        ("./same.typ", b"first".to_vec()),
        ("same.typ", b"second".to_vec()),
    ])
    .unwrap_err();
    assert_eq!(
        duplicate_package.issues.as_slice(),
        [PackIssue::DuplicatePath {
            path: "same.typ".to_owned(),
        }]
    );
    let invalid_package = package::Tree::new([
        ("lib.typ", b"library".to_vec()),
        ("../escape.typ", b"escape".to_vec()),
    ])
    .unwrap_err();
    assert!(matches!(
        invalid_package.issues.as_slice(),
        [PackIssue::InvalidPath { path, .. }] if path == "../escape.typ"
    ));
}

#[test]
fn compilation_request_regressions_replay_both_acceptance_and_rejection() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"accepted request fuzz regression".to_vec())
        .build()
        .unwrap();
    compile(&compile::Request::new(
        pack.clone(),
        Output::new(OutputFormat::Svg),
    ))
    .unwrap();

    let rejected = compile(&compile::Request::new(
        pack,
        Output {
            pages: PageSelection {
                ranges: vec![
                    Some(NonZeroUsize::new(2).unwrap())..=Some(NonZeroUsize::new(1).unwrap()),
                ],
            },
            ..Output::new(OutputFormat::Png)
        },
    ));
    assert!(matches!(
        rejected,
        Err(compile::Error::InvalidRequest(issues))
            if matches!(issues.as_slice(), [RequestIssue::InvalidPageRange { start, end }]
                if start.get() == 2 && end.get() == 1)
    ));
}

#[cfg(feature = "fs")]
#[test]
fn typkignore_and_write_state_regressions_replay_natively() {
    use typst_pack::ReadLimits;

    let malformed = tempfile::tempdir().unwrap();
    std::fs::write(malformed.path().join("main.typ"), b"main").unwrap();
    std::fs::write(malformed.path().join(".typkignore"), [0xff]).unwrap();
    assert!(matches!(
        fs::read_project(malformed.path(), "main.typ", ReadLimits::default()),
        Err(fs::Error::InvalidIgnoreFile { .. })
    ));

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"published".to_vec())
        .build()
        .unwrap();
    let plan = typst_pack::extract::plan(&pack, Default::default()).unwrap();
    for policy in [fs::WritePolicy::CreateNew, fs::WritePolicy::Overwrite] {
        for existing_root in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let destination = temp_path(&directory).join("written");
            if existing_root {
                std::fs::create_dir(&destination).unwrap();
            }
            fs::write_extraction(&plan, &destination, policy)
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
    let error = fs::write_extraction(&plan, &destination, fs::WritePolicy::CreateNew).unwrap_err();
    assert!(matches!(error, fs::WriteError::Rejected { .. }));
    assert_eq!(
        std::fs::read(destination.join("main.typ")).unwrap(),
        b"existing"
    );
}
