//! The OpenDAL adapter against the real Memory service, plus a small
//! fault-injecting wrapper for failures Memory cannot produce.
#![cfg(feature = "opendal")]

#[path = "support/opendal.rs"]
mod faults;
#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

use faults::{Faults, block_on};
use opendal::{ErrorKind, Operator};
use typst::syntax::package::PackageSpec;
use typst_pack::opendal::{
    Error, FontSource, PackageRead, PackageSource, WriteOutcome, WritePolicy, read_fonts,
    read_pack_archive, read_package, read_project, write_compilation_artifacts, write_pack_archive,
    write_pack_extraction_plan, write_package_archive,
};
use typst_pack::pack_archive::{DecodeLimits, decode, encode};
use typst_pack::{
    Artifact, CompileRequest, FontDisposition, Output, OutputFormat, Pack, PackExtractionSelection,
    ReadLimits, compile, plan_pack_extraction,
};

fn memory() -> Operator {
    Operator::new(opendal::services::Memory::default()).unwrap()
}

fn put(op: &Operator, path: &str, bytes: impl Into<Vec<u8>>) {
    block_on(op.write(path, bytes.into())).unwrap();
}

fn get(op: &Operator, path: &str) -> Vec<u8> {
    block_on(op.read(path)).unwrap().to_vec()
}

fn spec(text: &str) -> PackageSpec {
    text.parse().unwrap()
}

fn package_manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n")
}

fn sample_pack() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"= Sample".to_vec())
        .file("assets/logo.bin", b"logo".to_vec())
        .build()
        .unwrap()
}

// --- Reading -------------------------------------------------------------

#[test]
fn project_read_selects_every_file_below_the_prefix() {
    let op = memory();
    put(&op, "doc/main.typ", "= Hello");
    put(&op, "doc/chapters/one.typ", "One");
    put(&op, "doc/.typkignore", "chapters/");
    put(&op, "doc/empty.txt", "");
    put(&op, "doc-sibling/stray.typ", "not part of the project");
    put(&op, "other/main.typ", "not part of the project");

    let project = block_on(read_project(
        &op,
        "doc/",
        "main.typ",
        &ReadLimits::default(),
    ))
    .unwrap();

    let files = project.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(
        files,
        [".typkignore", "chapters/one.typ", "empty.txt", "main.typ"]
    );
    assert_eq!(project.file("chapters/one.typ"), Some(b"One".as_slice()));
    assert_eq!(project.file("empty.txt"), Some(b"".as_slice()));
}

#[test]
fn project_read_from_the_root_and_snapshot_validation() {
    let op = memory();
    put(&op, "main.typ", "= Root");
    let project = block_on(read_project(&op, "", "main.typ", &ReadLimits::default())).unwrap();
    assert_eq!(project.file("main.typ"), Some(b"= Root".as_slice()));

    let missing = block_on(read_project(&op, "", "absent.typ", &ReadLimits::default()));
    assert!(matches!(missing, Err(Error::Project(_))), "{missing:?}");
}

#[test]
fn malformed_prefixes_and_keys_are_rejected_before_storage_access() {
    let op = memory();
    for prefix in ["doc", "/doc/", "doc//", "../doc/", "doc/./"] {
        let result = block_on(read_project(
            &op,
            prefix,
            "main.typ",
            &ReadLimits::default(),
        ));
        assert!(
            matches!(result, Err(Error::InvalidPath { .. })),
            "{prefix:?}"
        );
    }
    for key in ["", "pack.typk/", "/pack.typk", "a/../pack.typk"] {
        let result = block_on(read_pack_archive(&op, key, 1024));
        assert!(matches!(result, Err(Error::InvalidPath { .. })), "{key:?}");
    }
}

#[test]
fn every_read_limit_is_enforced() {
    let op = memory();
    put(&op, "doc/main.typ", vec![b'a'; 10]);
    put(&op, "doc/b.typ", vec![b'b'; 10]);
    let read = |limits| block_on(read_project(&op, "doc/", "main.typ", &limits));
    let exceeded = |result: Result<_, Error>| match result {
        Err(Error::Limit { exceeded, .. }) => (exceeded.resource, exceeded.limit),
        other => panic!("expected a limit error, got {other:?}"),
    };

    let fits = ReadLimits {
        max_entries: 2,
        max_file_bytes: 10,
        max_total_bytes: 20,
    };
    assert!(read(fits).is_ok());
    assert_eq!(
        exceeded(read(ReadLimits {
            max_entries: 1,
            ..fits
        })),
        ("entries", 1)
    );
    assert_eq!(
        exceeded(read(ReadLimits {
            max_file_bytes: 9,
            ..fits
        })),
        ("file bytes", 9)
    );
    assert_eq!(
        exceeded(read(ReadLimits {
            max_total_bytes: 19,
            ..fits
        })),
        ("total bytes", 19)
    );

    put(&op, "pack.typk", vec![0; 11]);
    assert!(matches!(
        block_on(read_pack_archive(&op, "pack.typk", 10)),
        Err(Error::Limit { .. })
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn font_read_selects_containers_case_insensitively_in_source_order() {
    let op = memory();
    let font = fonts::typst_container();
    put(&op, "fonts/a/one.TTF", font.clone());
    put(&op, "fonts/a/nested/two.otf", font.clone());
    put(&op, "fonts/a/readme.txt", "not a font");
    put(&op, "fonts/b/three.ttc", font.clone());
    let sources = [
        FontSource {
            prefix: "fonts/b/".into(),
            disposition: FontDisposition::Embedded,
        },
        FontSource {
            prefix: "fonts/a/".into(),
            disposition: FontDisposition::External,
        },
    ];

    let catalog = block_on(read_fonts(&op, &sources, &ReadLimits::default())).unwrap();

    let dispositions = catalog
        .entries()
        .iter()
        .map(|entry| entry.disposition())
        .collect::<Vec<_>>();
    assert_eq!(
        dispositions,
        [
            FontDisposition::Embedded,
            FontDisposition::External,
            FontDisposition::External
        ]
    );

    put(&op, "fonts/a/broken.ttf", "not a font");
    let error = block_on(read_fonts(&op, &sources, &ReadLimits::default())).unwrap_err();
    assert!(
        matches!(&error, Error::Font { path, .. } if path == "fonts/a/broken.ttf"),
        "{error:?}"
    );
}

#[test]
fn package_read_tries_sources_in_order_and_reports_each_outcome() {
    let op = memory();
    let sources = [
        PackageSource::Trees {
            operator: op.clone(),
            prefix: "empty-trees/".into(),
        },
        PackageSource::Trees {
            operator: op.clone(),
            prefix: "trees/".into(),
        },
        PackageSource::ArchiveCache {
            operator: op.clone(),
            prefix: "cache/".into(),
        },
        PackageSource::Registry {
            operator: op.clone(),
            prefix: "registry/".into(),
        },
    ];
    let read =
        |text| block_on(read_package(&sources, &spec(text), &ReadLimits::default())).unwrap();

    put(
        &op,
        "trees/local/tree/1.0.0/typst.toml",
        package_manifest("tree"),
    );
    put(&op, "trees/local/tree/1.0.0/src/lib.typ", "#let x = 1");
    put(&op, "cache/local/cached/1.0.0.tar.gz", "cached bytes");
    put(
        &op,
        "registry/preview/served-1.0.0.tar.gz",
        "registry bytes",
    );
    put(&op, "registry/local/unserved-1.0.0.tar.gz", "never read");

    assert_eq!(
        read("@local/tree:1.0.0"),
        PackageRead::Tree(vec![
            ("src/lib.typ".into(), b"#let x = 1".to_vec()),
            ("typst.toml".into(), package_manifest("tree").into_bytes()),
        ])
    );
    assert_eq!(
        read("@local/cached:1.0.0"),
        PackageRead::CachedArchive(b"cached bytes".to_vec())
    );
    assert_eq!(
        read("@preview/served:1.0.0"),
        PackageRead::RegistryArchive(b"registry bytes".to_vec())
    );
    // The registry serves only `preview`, so a `local` spec never reaches it.
    assert_eq!(read("@local/unserved:1.0.0"), PackageRead::Unavailable);
}

#[test]
fn a_failing_source_stops_package_fallback() {
    let op = memory();
    let (faulty, faults) = Faults::wrap(&op);
    put(&op, "cache/local/pkg/1.0.0.tar.gz", "cached");
    put(&op, "later/local/pkg/1.0.0.tar.gz", "later");
    faults.fail_read("cache/local/pkg/1.0.0.tar.gz");
    let sources = [
        PackageSource::ArchiveCache {
            operator: faulty,
            prefix: "cache/".into(),
        },
        PackageSource::ArchiveCache {
            operator: op,
            prefix: "later/".into(),
        },
    ];

    let error = block_on(read_package(
        &sources,
        &spec("@local/pkg:1.0.0"),
        &ReadLimits::default(),
    ))
    .unwrap_err();

    assert!(
        matches!(
            error,
            Error::Storage {
                operation: "read",
                ..
            }
        ),
        "{error:?}"
    );
}

#[cfg(feature = "package-reading")]
#[test]
fn inserting_a_read_package_updates_catalog_and_failures() {
    use typst_pack::opendal::insert_package;
    use typst_pack::{
        PackageCatalog, PackageDisposition, PackageReadFailure, PackageReadFailureReason,
        PackageReadFailures, ReadLimits,
    };

    let mut catalog = PackageCatalog::new();
    let mut failures = PackageReadFailures::new();
    let tree_spec = spec("@local/tree:1.0.0");
    let mut insert = |spec: &PackageSpec, read| {
        insert_package(
            &mut catalog,
            &mut failures,
            spec,
            read,
            PackageDisposition::Embedded,
            ReadLimits::DEFAULT,
        )
    };

    insert(&spec("@local/gone:1.0.0"), PackageRead::Unavailable).unwrap();
    let malformed = insert(
        &spec("@preview/bad:1.0.0"),
        PackageRead::RegistryArchive(b"not gzip".to_vec()),
    );
    assert!(
        matches!(malformed, Err(Error::PackageArchive(_))),
        "{malformed:?}"
    );
    let invalid_tree = insert(
        &spec("@local/no-manifest:1.0.0"),
        PackageRead::Tree(vec![("lib.typ".into(), b"x".to_vec())]),
    );
    assert!(invalid_tree.is_err());

    failures.insert(PackageReadFailure::new(
        tree_spec.clone(),
        PackageReadFailureReason::NotFound,
    ));
    insert_package(
        &mut catalog,
        &mut failures,
        &tree_spec,
        PackageRead::Tree(vec![
            ("typst.toml".into(), package_manifest("tree").into_bytes()),
            ("lib.typ".into(), b"#let x = 1".to_vec()),
        ]),
        PackageDisposition::External,
        ReadLimits::DEFAULT,
    )
    .unwrap();

    assert!(catalog.get(&tree_spec).is_some());
    let reasons = failures
        .entries()
        .map(|failure| (failure.spec().to_string(), failure.reason().clone()))
        .collect::<Vec<_>>();
    assert!(reasons.contains(&(
        "@local/gone:1.0.0".into(),
        PackageReadFailureReason::NotFound
    )));
    assert!(reasons.contains(&(
        "@preview/bad:1.0.0".into(),
        PackageReadFailureReason::MalformedArchive { detail: None }
    )));
    assert!(
        reasons
            .iter()
            .any(|(spec, _)| spec == "@local/no-manifest:1.0.0")
    );
    assert!(
        failures.get(&tree_spec).is_none(),
        "success clears a stale failure"
    );
}

#[test]
fn an_object_vanishing_between_list_and_read_fails_the_read() {
    let op = memory();
    let (faulty, faults) = Faults::wrap(&op);
    put(&op, "doc/main.typ", "= Hello");
    put(&op, "doc/gone.typ", "vanishes");
    faults.vanish("doc/gone.typ");

    let error = block_on(read_project(
        &faulty,
        "doc/",
        "main.typ",
        &ReadLimits::default(),
    ))
    .unwrap_err();

    match error {
        Error::Storage { path, source, .. } => {
            assert_eq!(path, "doc/gone.typ");
            assert_eq!(source.kind(), ErrorKind::NotFound);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_failed_object_read_names_the_object() {
    let op = memory();
    let (faulty, faults) = Faults::wrap(&op);
    put(&op, "doc/main.typ", "= Hello");
    faults.fail_read("doc/main.typ");

    let error = block_on(read_project(
        &faulty,
        "doc/",
        "main.typ",
        &ReadLimits::default(),
    ))
    .unwrap_err();

    assert!(
        matches!(&error, Error::Storage { path, operation: "read", .. } if path == "doc/main.typ"),
        "{error:?}"
    );
    // The native error stays in the source chain; Display omits it.
    assert!(!error.to_string().contains("injected"));
    let source = std::error::Error::source(&error).unwrap().to_string();
    assert!(source.contains("injected"), "{source}");
}

// --- Writing -------------------------------------------------------------

#[test]
fn pack_archives_round_trip_under_both_policies() {
    let op = memory();
    let pack = sample_pack();
    let archive = encode(&pack).unwrap();
    let write = |policy| block_on(write_pack_archive(&op, "out/sample.typk", &archive, policy));

    assert_eq!(
        write(WritePolicy::CreateOrVerify).unwrap(),
        WriteOutcome::Created
    );
    assert_eq!(
        write(WritePolicy::CreateOrVerify).unwrap(),
        WriteOutcome::AlreadyMatching
    );
    assert_eq!(
        write(WritePolicy::Overwrite).unwrap(),
        WriteOutcome::Written
    );

    let read = block_on(read_pack_archive(&op, "out/sample.typk", 1 << 20)).unwrap();
    assert_eq!(read.as_slice(), archive.as_slice());
    let decoded = decode(&read, DecodeLimits::DEFAULT).unwrap();
    assert_eq!(decoded.identity(), pack.identity());

    let absent = block_on(read_pack_archive(&op, "out/absent.typk", 1 << 20));
    assert!(
        matches!(&absent, Err(Error::Storage { source, .. }) if source.kind() == ErrorKind::NotFound),
        "{absent:?}"
    );
}

#[test]
fn create_or_verify_refuses_different_bytes_without_writing() {
    let op = memory();
    let pack = encode(&sample_pack()).unwrap();
    // Same length with different bytes needs a comparison; a different
    // length conflicts on the advertised size alone.
    for existing in [vec![0; pack.as_slice().len()], b"short".to_vec()] {
        put(&op, "out.typk", existing.clone());
        let error = block_on(write_pack_archive(
            &op,
            "out.typk",
            &pack,
            WritePolicy::CreateOrVerify,
        ))
        .unwrap_err();
        assert!(matches!(error.error, Error::Conflict { .. }), "{error:?}");
        assert!(!error.indeterminate);
        assert_eq!(get(&op, "out.typk"), existing);
    }
}

#[test]
fn empty_objects_are_created_verified_and_read() {
    let op = memory();
    let empty = Vec::<u8>::new();
    for expected in [WriteOutcome::Created, WriteOutcome::AlreadyMatching] {
        let outcome = block_on(write_pack_archive(
            &op,
            "empty.typk",
            &empty,
            WritePolicy::CreateOrVerify,
        ));
        assert_eq!(outcome.unwrap(), expected);
    }
    let read = block_on(read_pack_archive(&op, "empty.typk", 0)).unwrap();
    assert!(read.as_slice().is_empty());
}

#[test]
fn create_or_verify_checks_every_key_before_creating_any() {
    let op = memory();
    let plan = plan_pack_extraction(&sample_pack(), PackExtractionSelection::default()).unwrap();
    put(&op, "out/main.typ", "conflicting");

    let error = block_on(write_pack_extraction_plan(
        &op,
        "out/",
        &plan,
        WritePolicy::CreateOrVerify,
    ))
    .unwrap_err();

    assert!(matches!(&error.error, Error::Conflict { path } if path == "out/main.typ"));
    assert!(error.completed.is_empty());
    assert!(!block_on(op.exists("out/assets/logo.bin")).unwrap());
}

#[test]
fn extraction_writes_replay_as_already_matching() {
    let op = memory();
    let plan = plan_pack_extraction(&sample_pack(), PackExtractionSelection::default()).unwrap();
    let write = |policy| block_on(write_pack_extraction_plan(&op, "out/", &plan, policy)).unwrap();

    let created = write(WritePolicy::CreateOrVerify);
    assert_eq!(
        created,
        [
            ("out/assets/logo.bin".into(), WriteOutcome::Created),
            ("out/main.typ".into(), WriteOutcome::Created),
        ]
    );
    assert_eq!(get(&op, "out/assets/logo.bin"), b"logo");
    assert!(
        write(WritePolicy::CreateOrVerify)
            .iter()
            .all(|(_, outcome)| *outcome == WriteOutcome::AlreadyMatching)
    );
    assert!(
        write(WritePolicy::Overwrite)
            .iter()
            .all(|(_, outcome)| *outcome == WriteOutcome::Written)
    );
}

#[test]
fn a_write_failure_reports_the_completed_prefix_and_indeterminate_effect() {
    let plan = plan_pack_extraction(&sample_pack(), PackExtractionSelection::default()).unwrap();
    for policy in [WritePolicy::Overwrite, WritePolicy::CreateOrVerify] {
        let op = memory();
        let (faulty, faults) = Faults::wrap(&op);
        faults.fail_write("out/main.typ");

        let error =
            block_on(write_pack_extraction_plan(&faulty, "out/", &plan, policy)).unwrap_err();

        assert_eq!(error.completed.len(), 1, "{policy:?}");
        assert_eq!(error.completed[0].0, "out/assets/logo.bin");
        assert!(error.indeterminate);
        assert!(matches!(&error.error, Error::Storage { path, .. } if path == "out/main.typ"));

        // Replaying with CreateOrVerify after the fault clears completes the
        // already written prefix as matching.
        let replay = block_on(write_pack_extraction_plan(
            &op,
            "out/",
            &plan,
            WritePolicy::CreateOrVerify,
        ))
        .unwrap();
        assert_eq!(replay[1].1, WriteOutcome::Created);
    }
}

#[test]
fn a_lost_conditional_create_race_is_verified() {
    let op = memory();
    let (faulty, faults) = Faults::wrap(&op);
    let archive = encode(&sample_pack()).unwrap();

    // Another writer created matching bytes after the preflight check.
    put(&op, "matching.typk", archive.as_slice().to_vec());
    faults.stale_stat_once("matching.typk");
    let outcome = block_on(write_pack_archive(
        &faulty,
        "matching.typk",
        &archive,
        WritePolicy::CreateOrVerify,
    ));
    assert_eq!(outcome.unwrap(), WriteOutcome::AlreadyMatching);

    // ... or different bytes, which are kept.
    put(&op, "different.typk", "theirs");
    faults.stale_stat_once("different.typk");
    let error = block_on(write_pack_archive(
        &faulty,
        "different.typk",
        &archive,
        WritePolicy::CreateOrVerify,
    ))
    .unwrap_err();
    assert!(matches!(error.error, Error::Conflict { .. }));
    assert!(!error.indeterminate);
    assert_eq!(get(&op, "different.typk"), b"theirs");
}

#[test]
fn create_or_verify_requires_conditional_create() {
    let (faulty, faults) = Faults::wrap(&memory());
    faults.hide_conditional_create();
    let archive = encode(&sample_pack()).unwrap();

    let error = block_on(write_pack_archive(
        &faulty,
        "out.typk",
        &archive,
        WritePolicy::CreateOrVerify,
    ))
    .unwrap_err();
    assert!(
        matches!(error.error, Error::Unsupported { .. }),
        "{error:?}"
    );

    let written = block_on(write_pack_archive(
        &faulty,
        "out.typk",
        &archive,
        WritePolicy::Overwrite,
    ));
    assert_eq!(written.unwrap(), WriteOutcome::Written);
}

#[test]
fn package_archives_are_cached_in_the_layout_the_cache_source_reads() {
    let op = memory();
    let spec = spec("@preview/cached:1.0.0");
    let outcome = block_on(write_package_archive(&op, "cache/", &spec, b"archive")).unwrap();
    assert_eq!(outcome, WriteOutcome::Created);
    let replay = block_on(write_package_archive(&op, "cache/", &spec, b"archive")).unwrap();
    assert_eq!(replay, WriteOutcome::AlreadyMatching);

    let read = block_on(read_package(
        &[PackageSource::ArchiveCache {
            operator: op,
            prefix: "cache/".into(),
        }],
        &spec,
        &ReadLimits::default(),
    ))
    .unwrap();
    assert_eq!(read, PackageRead::CachedArchive(b"archive".to_vec()));
}

#[test]
fn compilation_artifacts_are_written_at_validated_keys() {
    let op = memory();
    let result = two_page_result();
    let write = |keys: &[&str]| {
        block_on(write_compilation_artifacts(
            &op,
            "pages/",
            &result,
            keys,
            WritePolicy::CreateOrVerify,
        ))
    };

    let written = write(&["one.svg", "nested/two.svg"]).unwrap();
    assert_eq!(written[1].0, "pages/nested/two.svg");
    assert_eq!(get(&op, "pages/one.svg"), result[0].bytes);

    let error = |keys: &[&str]| write(keys).unwrap_err().error;
    assert!(matches!(
        error(&["one.svg"]),
        Error::ArtifactKeyCount {
            expected: 2,
            actual: 1
        }
    ));
    assert!(matches!(
        error(&["same.svg", "same.svg"]),
        Error::InvalidPath { .. }
    ));
    for bad in ["../escape.svg", "/abs.svg", "dir/", "a//b.svg", "a\\b.svg"] {
        assert!(
            matches!(error(&["ok.svg", bad]), Error::InvalidPath { .. }),
            "{bad:?}"
        );
    }
}

fn two_page_result() -> Vec<Artifact> {
    compilation_result(
        "#set page(width: 10pt, height: 10pt, margin: 0pt)\n\
         #rect(width: 1pt, height: 1pt)\n\
         #pagebreak()\n\
         #rect(width: 2pt, height: 2pt)",
    )
}

fn compilation_result(source: &str) -> Vec<Artifact> {
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
        .build()
        .unwrap();
    compile(&CompileRequest::new(pack, Output::new(OutputFormat::Svg)))
        .unwrap()
        .artifacts
}

// --- Assembly ------------------------------------------------------------

#[cfg(feature = "package-reading")]
mod assembly {
    use typst_pack::CreateError;
    use typst_pack::opendal::{Error, OpenDalPackAssembler, Operator, PackageSource};

    use super::{block_on, memory, package_manifest, put};

    fn assembler(op: &Operator) -> OpenDalPackAssembler {
        let mut assembler = OpenDalPackAssembler::new(op.clone(), "doc/", "main.typ");
        assembler.packages = vec![
            PackageSource::Trees {
                operator: op.clone(),
                prefix: "packages/".into(),
            },
            PackageSource::Registry {
                operator: op.clone(),
                prefix: "registry/".into(),
            },
        ];
        assembler
    }

    fn registry_archive(name: &str) -> Vec<u8> {
        use std::io::Write;

        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (path, data) in [
            ("typst.toml", package_manifest(name).into_bytes()),
            ("lib.typ", b"#let shout() = [LOUD]".to_vec()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(&mut header, path, data.as_slice())
                .unwrap();
        }
        let mut encoder = builder.into_inner().unwrap();
        encoder.flush().unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn assembly_resumes_through_nested_package_reads() {
        let op = memory();
        put(
            &op,
            "doc/main.typ",
            "#import \"@local/greet:1.0.0\": hello\n#hello()",
        );
        put(
            &op,
            "packages/local/greet/1.0.0/typst.toml",
            package_manifest("greet"),
        );
        put(
            &op,
            "packages/local/greet/1.0.0/lib.typ",
            "#import \"@preview/loud:1.0.0\": shout\n#let hello() = [Hi #shout()]",
        );
        put(
            &op,
            "registry/preview/loud-1.0.0.tar.gz",
            registry_archive("loud"),
        );

        let assembled = block_on(assembler(&op).assemble()).unwrap();

        let specs = assembled
            .pack
            .package_requirements()
            .iter()
            .map(|requirement| requirement.spec.to_string())
            .collect::<Vec<_>>();
        assert_eq!(specs, ["@local/greet:1.0.0", "@preview/loud:1.0.0"]);
        assert_eq!(
            assembled.pack.file("main.typ"),
            Some(b"#import \"@local/greet:1.0.0\": hello\n#hello()".as_slice())
        );
    }

    #[test]
    fn an_unavailable_package_fails_creation_at_its_import() {
        let op = memory();
        put(&op, "doc/main.typ", "#import \"@local/missing:1.0.0\": x");

        let error = block_on(assembler(&op).assemble()).unwrap_err();

        match error {
            Error::Creation(error) => match *error {
                CreateError::Failed { errors, .. } => assert!(!errors.is_empty()),
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
    }
}
