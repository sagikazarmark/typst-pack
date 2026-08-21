//! Frozen Pack, Compilation, and Compilation Result identity contracts.

use proptest::prelude::*;
use typst::foundations::{Datetime, Dict, Smart, Value};
use typst_pack::{
    CanonicalIdentityRole, CompilationLimits, CompilationOutputSpecification,
    CompilationRequestRejection, CompilationResult, CreationTimestamp, DocumentTime,
    HtmlOutputSpecification, Pack, PackCompilationRequest, PackMetadata, PackOverrideSet,
    PackageDisposition, PdfOutputSpecification, PngOutputSpecification, SvgOutputSpecification,
    compile_with_limits as compile_to_report, parse_page_selection,
};

fn svg_output() -> CompilationOutputSpecification {
    CompilationOutputSpecification::Svg(SvgOutputSpecification::default())
}

fn compile(
    request: PackCompilationRequest,
) -> Result<CompilationResult, CompilationRequestRejection> {
    let report = compile_to_report(request, CompilationLimits::reference_v1())?;
    Ok(report
        .result()
        .expect("expected a semantic Compilation Result")
        .clone())
}

fn identity_pack() -> Pack {
    Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .file("unused.txt", b"baseline".to_vec())
        .unwrap()
        .build()
        .unwrap()
}

proptest! {
    #[test]
    fn pack_identity_binds_generated_project_bytes_and_excludes_metadata(
        project_bytes in prop::collection::vec(any::<u8>(), 1..128),
        mutation_index in any::<usize>(),
        metadata_name in "[a-zA-Z0-9 ]{0,32}",
        metadata_author in "[a-zA-Z0-9 ]{0,32}",
    ) {
        let plain = Pack::builder("main.typ")
            .file("main.typ", project_bytes.clone()).unwrap()
            .build().unwrap();
        let described = Pack::builder("main.typ")
            .file("main.typ", project_bytes.clone()).unwrap()
            .metadata(
                PackMetadata::new()
                    .with_name(metadata_name)
                    .with_author(metadata_author),
            )
            .build().unwrap();
        let mut changed_bytes = project_bytes;
        let mutation_index = mutation_index % changed_bytes.len();
        changed_bytes[mutation_index] ^= u8::MAX;
        let changed = Pack::builder("main.typ")
            .file("main.typ", changed_bytes).unwrap()
            .build().unwrap();

        prop_assert_eq!(plain.identity(), described.identity());
        prop_assert_ne!(plain.identity(), changed.identity());
    }
}

#[test]
fn frozen_pack_identity_vector() {
    let identity = identity_pack().identity();

    assert_eq!(identity.role(), CanonicalIdentityRole::Pack);
    assert_eq!(identity.schema(), "typst-pack-identity-v1");
    assert_eq!(identity.algorithm(), "typst-hash128-0.15");
    assert_eq!(
        identity.digest(),
        [
            0x50, 0x36, 0x3e, 0xc2, 0x23, 0x73, 0x08, 0x44, 0x57, 0xc2, 0x81, 0x9c, 0x12, 0x8e,
            0xd6, 0xe7,
        ]
    );
}

#[cfg(not(any(
    feature = "_test-package-download-probe",
    feature = "default",
    feature = "diagnostics",
    feature = "embedded-fonts",
    feature = "egress",
    feature = "fs",
    feature = "opendal",
    feature = "package-reading",
    feature = "parallel"
)))]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
#[test]
fn frozen_featureless_compilation_and_result_identity_vectors() {
    let result = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();
    let compilation = result.compilation_identity();
    let result_identity = result.result_identity();

    assert_eq!(compilation.role(), CanonicalIdentityRole::Compilation);
    assert_eq!(compilation.schema(), "typst-pack-compilation-v1");
    assert_eq!(compilation.algorithm(), "typst-hash128-0.15");
    assert_eq!(
        compilation.digest(),
        [
            0x04, 0x67, 0x5e, 0xcb, 0xc5, 0x89, 0x68, 0xd7, 0x63, 0xa8, 0xfe, 0xd3, 0x4c, 0xce,
            0xde, 0x67,
        ]
    );
    assert_eq!(
        result_identity.role(),
        CanonicalIdentityRole::CompilationResult
    );
    assert_eq!(result_identity.schema(), "typst-pack-compilation-result-v1");
    assert_eq!(result_identity.algorithm(), "typst-hash128-0.15");
    assert_eq!(
        result_identity.digest(),
        [
            0x4b, 0xce, 0xae, 0x83, 0x78, 0xcd, 0x40, 0x90, 0x72, 0x28, 0x91, 0x7b, 0x9e, 0x0a,
            0x74, 0xf6,
        ]
    );
}

#[cfg(all(
    feature = "opendal",
    not(any(
        feature = "_test-package-download-probe",
        feature = "default",
        feature = "diagnostics",
        feature = "embedded-fonts",
        feature = "egress",
        feature = "fs",
        feature = "package-reading",
        feature = "parallel"
    ))
))]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
#[test]
fn frozen_opendal_compilation_and_result_identity_vectors() {
    assert_implementation_attestation_vectors(
        "opendal",
        [
            0xc8, 0xcb, 0x29, 0x92, 0xf4, 0xcf, 0x8a, 0x1b, 0xdd, 0x5d, 0x4b, 0x92, 0xa7, 0xf6,
            0xcf, 0x6f,
        ],
        [
            0x37, 0xd3, 0x7c, 0x48, 0xdc, 0xa6, 0xab, 0xb3, 0x66, 0x0e, 0xa6, 0xe9, 0x13, 0x48,
            0x56, 0xf4,
        ],
    );
}

#[cfg(all(
    feature = "_test-package-download-probe",
    feature = "default",
    feature = "diagnostics",
    feature = "embedded-fonts",
    feature = "egress",
    feature = "fs",
    feature = "opendal",
    feature = "package-reading",
    feature = "parallel"
))]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
#[test]
fn frozen_all_feature_compilation_and_result_identity_vectors() {
    assert_implementation_attestation_vectors(
        "_test-package-download-probe,default,diagnostics,egress,embedded-fonts,fs,opendal,package-reading,parallel",
        [
            0xd0, 0x98, 0xec, 0x8b, 0xfb, 0x80, 0xf9, 0xdb, 0x26, 0xc8, 0x98, 0x71, 0x98, 0xe5,
            0x8d, 0xd1,
        ],
        [
            0x46, 0x75, 0xb7, 0x37, 0x63, 0x18, 0xe7, 0xfb, 0x71, 0xaa, 0x39, 0x7c, 0xe8, 0x9d,
            0xf3, 0x53,
        ],
    );
}

#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
fn assert_implementation_attestation_vectors(
    expected_features: &str,
    expected_compilation: [u8; 16],
    expected_result: [u8; 16],
) {
    let result = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();

    assert_eq!(result.engine_identity().feature_set(), expected_features);
    assert_eq!(result.exporter_identity().feature_set(), expected_features);
    assert_eq!(result.compilation_identity().digest(), expected_compilation);
    assert_eq!(result.result_identity().digest(), expected_result);
}

#[test]
fn pack_identity_binds_every_project_and_package_semantic() {
    #[derive(Clone)]
    struct PackCase<'a> {
        entrypoint: &'a str,
        project_path: &'a str,
        project_bytes: &'a [u8],
        package: typst::syntax::package::PackageSpec,
        package_path: &'a str,
        package_bytes: &'a [u8],
        disposition: PackageDisposition,
        extra_package_file: bool,
    }

    let package: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let other_package: typst::syntax::package::PackageSpec = "@local/other:1.0.0".parse().unwrap();
    let baseline = PackCase {
        entrypoint: "main.typ",
        project_path: "other.typ",
        project_bytes: b"other",
        package,
        package_path: "lib.typ",
        package_bytes: b"package",
        disposition: PackageDisposition::Embedded,
        extra_package_file: false,
    };
    let build = |case: PackCase<'_>| {
        let mut builder = Pack::builder(case.entrypoint)
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .file(case.project_path, case.project_bytes.to_vec())
            .unwrap();
        builder = if case.disposition.is_embedded() {
            builder
                .package_file(
                    case.package.clone(),
                    case.package_path,
                    case.package_bytes.to_vec(),
                )
                .unwrap()
        } else {
            builder
                .external_package_file(
                    case.package.clone(),
                    case.package_path,
                    case.package_bytes.to_vec(),
                )
                .unwrap()
        };
        if case.extra_package_file {
            builder = if case.disposition.is_embedded() {
                builder
                    .package_file(case.package, "extra.typ", b"extra".to_vec())
                    .unwrap()
            } else {
                builder
                    .external_package_file(case.package, "extra.typ", b"extra".to_vec())
                    .unwrap()
            };
        }
        builder.build().unwrap()
    };
    let baseline_identity = build(baseline.clone()).identity();

    let mutations = [
        PackCase {
            entrypoint: "other.typ",
            ..baseline.clone()
        },
        PackCase {
            project_path: "renamed.typ",
            ..baseline.clone()
        },
        PackCase {
            project_bytes: b"changed",
            ..baseline.clone()
        },
        PackCase {
            package: other_package,
            ..baseline.clone()
        },
        PackCase {
            package_path: "renamed.typ",
            ..baseline.clone()
        },
        PackCase {
            package_bytes: b"changed",
            ..baseline.clone()
        },
        PackCase {
            extra_package_file: true,
            ..baseline.clone()
        },
        PackCase {
            disposition: PackageDisposition::External,
            ..baseline
        },
    ];

    for mutation in mutations {
        assert_ne!(build(mutation).identity(), baseline_identity);
    }
}

#[test]
fn compilation_identity_binds_every_effective_output_control() {
    let compile_output = |output| {
        compile(PackCompilationRequest::new(identity_pack(), output))
            .unwrap()
            .compilation_identity()
    };
    let timestamp =
        typst_pdf::Timestamp::new_utc(Datetime::from_ymd_hms(2024, 2, 3, 4, 5, 6).unwrap());

    let pdf_baseline = compile_output(CompilationOutputSpecification::Pdf(
        PdfOutputSpecification::default(),
    ));
    let png_baseline = compile_output(CompilationOutputSpecification::Png(
        PngOutputSpecification::default(),
    ));
    let svg_baseline = compile_output(svg_output());
    let html_baseline = compile_output(CompilationOutputSpecification::Html(
        HtmlOutputSpecification::default(),
    ));
    assert_eq!(
        [pdf_baseline, png_baseline, svg_baseline, html_baseline]
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );

    let pdf_mutations = [
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            standards: vec![typst_pdf::PdfStandard::A_2b],
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            identifier: Smart::Custom("identity".to_owned()),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            creator: Smart::Custom(Some("identity".to_owned())),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            tags: Smart::Custom(false),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            creation_timestamp: CreationTimestamp::Explicit(timestamp),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            pretty: true,
            ..PdfOutputSpecification::default()
        }),
    ];
    for output in pdf_mutations {
        assert_ne!(compile_output(output), pdf_baseline);
    }

    let png_mutations = [
        CompilationOutputSpecification::Png(PngOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..PngOutputSpecification::default()
        }),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(216.0),
            ..PngOutputSpecification::default()
        }),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            render_bleed: true,
            ..PngOutputSpecification::default()
        }),
    ];
    for output in png_mutations {
        assert_ne!(compile_output(output), png_baseline);
    }

    let svg_mutations = [
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..SvgOutputSpecification::default()
        }),
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            render_bleed: true,
            ..SvgOutputSpecification::default()
        }),
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            pretty: true,
            ..SvgOutputSpecification::default()
        }),
    ];
    for output in svg_mutations {
        assert_ne!(compile_output(output), svg_baseline);
    }

    assert_ne!(
        compile_output(CompilationOutputSpecification::Html(
            HtmlOutputSpecification { pretty: true }
        )),
        html_baseline
    );
}

#[test]
fn compilation_identity_binds_pack_inputs_overrides_features_and_document_time() {
    let baseline_pack = identity_pack();
    let baseline = compile(PackCompilationRequest::new(
        baseline_pack.clone(),
        svg_output(),
    ))
    .unwrap();
    let baseline_identity = baseline.compilation_identity();
    let mut inputs = Dict::new();
    inputs.insert("unused".into(), Value::Str("value".into()));
    let main_override = PackOverrideSet::new(&baseline_pack)
        .replace("main.typ", b"aaaaaaaa".to_vec())
        .unwrap();
    let path_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"aaaaaaaa".to_vec())
        .unwrap();
    let length_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"short".to_vec())
        .unwrap();
    let byte_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"bbbbbbbb".to_vec())
        .unwrap();
    let changed_pack = Pack::builder("main.typ")
        .file("main.typ", b"changed pack".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let main_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(main_override),
    )
    .unwrap()
    .compilation_identity();
    let path_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(path_override),
    )
    .unwrap()
    .compilation_identity();
    let length_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(length_override),
    )
    .unwrap()
    .compilation_identity();
    let byte_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(byte_override),
    )
    .unwrap()
    .compilation_identity();
    assert_ne!(main_override, path_override);
    assert_ne!(path_override, length_override);
    assert_ne!(path_override, byte_override);

    let requests = [
        PackCompilationRequest::new(changed_pack, svg_output()),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).inputs(inputs),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output())
            .feature(typst::Feature::A11yExtras),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output())
            .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap())),
        PackCompilationRequest::new(baseline_pack, svg_output())
            .document_time(DocumentTime::UnixTimestamp(1_700_000_000)),
    ];

    for request in requests {
        assert_ne!(
            compile(request).unwrap().compilation_identity(),
            baseline_identity
        );
    }
}

#[test]
fn identities_exclude_metadata_and_normalize_equivalent_request_values() {
    let metadata = PackMetadata::new()
        .with_name("Named Pack")
        .with_description("Operational description")
        .with_author("Pack Author");
    let plain_pack = identity_pack();
    let metadata_pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .file("unused.txt", b"baseline".to_vec())
        .unwrap()
        .metadata(metadata)
        .build()
        .unwrap();
    assert_eq!(plain_pack.identity(), metadata_pack.identity());

    let mut inputs = Dict::new();
    inputs.insert("unused".into(), Value::Str("value".into()));
    let time = DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap());
    let caller = compile(
        PackCompilationRequest::new(plain_pack.clone(), svg_output())
            .inputs(inputs.clone())
            .feature(typst::Feature::A11yExtras)
            .document_time(time),
    )
    .unwrap();
    let described = compile(
        PackCompilationRequest::new(metadata_pack, svg_output())
            .inputs(inputs)
            .feature(typst::Feature::A11yExtras)
            .document_time(time),
    )
    .unwrap();
    assert_eq!(
        caller.compilation_identity(),
        described.compilation_identity()
    );
    assert_eq!(caller.result_identity(), described.result_identity());

    let default_png = compile(PackCompilationRequest::new(
        plain_pack.clone(),
        CompilationOutputSpecification::Png(PngOutputSpecification::default()),
    ))
    .unwrap();
    let explicit_png = compile(PackCompilationRequest::new(
        plain_pack.clone(),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(144.0),
            ..PngOutputSpecification::default()
        }),
    ))
    .unwrap();
    assert_eq!(
        default_png.compilation_identity(),
        explicit_png.compilation_identity()
    );
    assert_eq!(
        default_png.result_identity(),
        explicit_png.result_identity()
    );

    let caller_empty_overrides = compile(
        PackCompilationRequest::new(plain_pack.clone(), svg_output())
            .overrides(PackOverrideSet::new(&plain_pack)),
    )
    .unwrap();
    let default_empty_overrides =
        compile(PackCompilationRequest::new(plain_pack, svg_output())).unwrap();
    assert_eq!(
        default_empty_overrides.compilation_identity(),
        caller_empty_overrides.compilation_identity()
    );
    assert_eq!(
        default_empty_overrides.result_identity(),
        caller_empty_overrides.result_identity()
    );
}

#[test]
fn compilation_and_result_identities_exclude_destination_facts() {
    let first = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();
    let second = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_destination = directory.path().join("first.svg");
    let second_destination = directory.path().join("nested/second.svg");
    std::fs::create_dir(second_destination.parent().unwrap()).unwrap();
    std::fs::write(&first_destination, first.artifacts()[0].bytes()).unwrap();
    std::fs::write(&second_destination, second.artifacts()[0].bytes()).unwrap();

    assert_ne!(first_destination, second_destination);
    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.result_identity(), second.result_identity());
    assert_eq!(
        std::fs::read(first_destination).unwrap(),
        std::fs::read(second_destination).unwrap()
    );
}
