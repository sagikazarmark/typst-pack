use typst_pack::{
    CompilationAttempt, CompilationExecutionControls, CompilationOperationOutcome,
    CompilationReportOutcome, CompilationRequestRejection, CompilationStatus, CompilationTarget,
    CompileOptions, CreationTimestamp, DiagnosticPhase, DiagnosticProducer,
    FontContainerFulfillment, OutputFormat, Pack, PackCompilationRequest, PackCompileError,
    PackMetadata, PackOverrideSet, PackOverrideSetError, PackageTreeFulfillment,
    RequestValueOrigin, compile, compile_report,
};

struct RuntimeResource;

impl typst_kit::files::FileLoader for RuntimeResource {
    fn load(
        &self,
        id: typst::syntax::FileId,
    ) -> typst::diag::FileResult<typst::foundations::Bytes> {
        if id.vpath().get_without_slash() == "runtime.txt" {
            Ok(typst::foundations::Bytes::new(b"runtime value".to_vec()))
        } else {
            Err(typst::diag::FileError::NotFound(
                id.vpath().get_without_slash().into(),
            ))
        }
    }
}

#[test]
fn pack_bound_compilation_resolves_declared_resource_slots() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#read(\"runtime.txt\")".to_vec())
        .unwrap()
        .resource_slot("runtime.txt")
        .unwrap()
        .build()
        .unwrap();

    let result = compile(CompilationAttempt::new(
        PackCompilationRequest::new(pack, OutputFormat::Svg),
        CompilationExecutionControls::default().resource_provider(RuntimeResource),
    ))
    .unwrap();

    assert_eq!(result.status(), CompilationStatus::Succeeded);
    assert_eq!(result.artifacts().len(), 1);
    assert_eq!(result.document().target(), CompilationTarget::Paged);
    assert!(
        result
            .result_identity()
            .digest()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert!(
        result
            .access_trace()
            .observations()
            .any(|observation| { observation.logical_path() == "project:main.typ" })
    );
}

fn five_page_pack() -> Pack {
    let source = (1..=5)
        .map(|page| {
            format!(
                "#set page(width: {page}0pt, height: 10pt, margin: 0pt)\n\
                 #rect(width: 1pt, height: 1pt)\n"
            ) + if page < 5 { "#pagebreak()\n" } else { "" }
        })
        .collect::<String>();
    Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn pack_bound_compilation_does_not_read_ambient_project_files() {
    let ambient = "tests/fixtures/official-oracle/chapter.typ";
    assert!(std::path::Path::new(ambient).is_file());
    let pack = Pack::builder("main.typ")
        .file("main.typ", format!("#include \"{ambient}\"").into_bytes())
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
}

#[test]
fn pack_bound_compilation_does_not_read_an_ambient_clock() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#datetime.today()".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack, OutputFormat::Svg));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
}

#[test]
fn pack_override_preflight_rejects_paths_outside_the_bound_pack() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"baseline".to_vec())
        .unwrap()
        .resource_slot("runtime.txt")
        .unwrap()
        .package_file(
            "@local/example:1.0.0".parse().unwrap(),
            "lib.typ",
            b"package".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();

    for path in [
        "missing.typ",
        "runtime.txt",
        "packages/local/example/1.0.0/lib.typ",
    ] {
        let error = PackOverrideSet::new(&pack)
            .replace(path, b"replacement".to_vec())
            .unwrap_err();
        assert!(matches!(
            error,
            PackOverrideSetError::MissingProjectPath { path: rejected } if rejected == path
        ));
    }

    let error = PackOverrideSet::new(&pack)
        .replace("main.typ", b"first".to_vec())
        .unwrap()
        .replace("./main.typ", b"second".to_vec())
        .unwrap_err();
    assert!(matches!(
        error,
        PackOverrideSetError::DuplicateProjectPath { path } if path == "main.typ"
    ));
}

#[test]
fn pack_overrides_replace_contained_bytes_without_mutating_the_pack() {
    let baseline = b"#set page(width: 20pt, height: 10pt, margin: 0pt)\nbaseline".to_vec();
    let replacement = b"#set page(width: 40pt, height: 10pt, margin: 0pt)\nreplacement".to_vec();
    let pack = Pack::builder("main.typ")
        .file("main.typ", baseline.clone())
        .unwrap()
        .file("unused.txt", b"unused baseline".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let pack_identity = pack.identity();
    let baseline_result =
        compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg)).unwrap();
    let overrides = PackOverrideSet::new(&pack)
        .replace("main.typ", replacement)
        .unwrap()
        .replace("unused.txt", b"unused replacement".to_vec())
        .unwrap();

    let overridden =
        compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).overrides(overrides))
            .unwrap();

    assert_ne!(
        overridden.artifacts()[0].bytes(),
        baseline_result.artifacts()[0].bytes()
    );
    assert_ne!(
        overridden.compilation_identity(),
        baseline_result.compilation_identity()
    );
    assert_eq!(overridden.request_inventory().overrides().value().len(), 2);
    assert!(
        overridden
            .request_inventory()
            .overrides()
            .value()
            .iter()
            .all(|entry| entry.byte_len() > 0 && entry.commitment() != [0; 16])
    );
    assert_eq!(pack.identity(), pack_identity);
    assert_eq!(pack.file("main.typ").unwrap().as_slice(), baseline);
    assert_eq!(
        pack.file("unused.txt").unwrap().as_slice(),
        b"unused baseline"
    );

    let unused_override = PackOverrideSet::new(&pack)
        .replace("unused.txt", b"another unused value".to_vec())
        .unwrap();
    let unused_result = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).overrides(unused_override),
    )
    .unwrap();
    assert_eq!(
        unused_result.artifacts()[0].bytes(),
        baseline_result.artifacts()[0].bytes()
    );
    assert_ne!(
        unused_result.compilation_identity(),
        baseline_result.compilation_identity()
    );
}

#[test]
fn pack_override_set_cannot_be_applied_to_a_different_pack() {
    let first = Pack::builder("main.typ")
        .file("main.typ", b"first".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let second = Pack::builder("main.typ")
        .file("main.typ", b"second".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let overrides = PackOverrideSet::new(&first)
        .replace("main.typ", b"replacement".to_vec())
        .unwrap();
    let accepted =
        compile(PackCompilationRequest::new(first, OutputFormat::Svg).overrides(overrides.clone()))
            .unwrap();
    let accepted_commitment = accepted
        .request_inventory()
        .overrides()
        .value()
        .iter()
        .next()
        .unwrap()
        .commitment();

    let result =
        compile(PackCompilationRequest::new(second, OutputFormat::Svg).overrides(overrides));

    let Err(PackCompileError::RequestRejected {
        rejection: CompilationRequestRejection::OverrideSetPackMismatch,
        request_inventory,
    }) = result
    else {
        panic!("expected a Pack Override Set binding rejection");
    };
    assert_eq!(
        request_inventory
            .overrides()
            .value()
            .iter()
            .next()
            .unwrap()
            .commitment(),
        accepted_commitment
    );
}

#[test]
fn pack_compilation_resolves_exporter_defaults_before_execution() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"defaults".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let png = compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Png)).unwrap();
    assert_eq!(png.request_inventory().options().value().ppi, Some(144.0));
    assert_eq!(
        png.request_inventory().ppi_origin(),
        RequestValueOrigin::CoreDefaulted
    );
    assert!(!png.request_inventory().options().value().render_bleed);

    let pdf = compile(PackCompilationRequest::new(pack, OutputFormat::Pdf)).unwrap();
    let options = pdf.request_inventory().options().value();
    assert_eq!(options.pdf_tags, typst::foundations::Smart::Custom(true));
    assert_eq!(
        pdf.request_inventory().pdf_tags_origin(),
        RequestValueOrigin::CoreDefaulted
    );
    assert_eq!(
        pdf.request_inventory().pdf_creation_time_origin(),
        RequestValueOrigin::CoreDefaulted
    );
    assert!(matches!(
        options.creation_timestamp,
        CreationTimestamp::Omit
    ));
}

#[test]
fn compilation_identity_ignores_pack_metadata_and_irrelevant_output_controls() {
    let build = |name| {
        Pack::builder("main.typ")
            .file("main.typ", b"same semantics".to_vec())
            .unwrap()
            .metadata(PackMetadata::new().with_name(name))
            .build()
            .unwrap()
    };
    let first = compile(PackCompilationRequest::new(
        build("first"),
        OutputFormat::Svg,
    ))
    .unwrap();
    let options = CompileOptions {
        pdf_creator: typst::foundations::Smart::Custom(Some("irrelevant".to_owned())),
        ppi: Some(300.0),
        ..CompileOptions::default()
    };
    let second =
        compile(PackCompilationRequest::new(build("second"), OutputFormat::Svg).options(options))
            .unwrap();

    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());
    assert_eq!(first.compilation_identity().kind(), "compilation");
    assert_eq!(
        first.compilation_identity().algorithm(),
        "typst-hash128-0.15"
    );
}

#[test]
fn compilation_identity_canonicalizes_page_ranges_and_pdf_standard_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"canonical request".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let first_ranges = CompileOptions {
        page_selection: typst_pack::parse_page_selection("1-2").unwrap(),
        ..CompileOptions::default()
    };
    let second_ranges = CompileOptions {
        page_selection: typst_pack::parse_page_selection("2,1").unwrap(),
        ..CompileOptions::default()
    };
    let first =
        compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).options(first_ranges))
            .unwrap();
    let second = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).options(second_ranges),
    )
    .unwrap();
    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());

    let first_standards = CompileOptions {
        pdf_standards: vec![typst_pdf::PdfStandard::A_2b, typst_pdf::PdfStandard::Ua_1],
        ..CompileOptions::default()
    };
    let second_standards = CompileOptions {
        pdf_standards: vec![typst_pdf::PdfStandard::Ua_1, typst_pdf::PdfStandard::A_2b],
        ..CompileOptions::default()
    };
    let first = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Pdf).options(first_standards),
    )
    .unwrap();
    let second =
        compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(second_standards))
            .unwrap();
    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.status(), second.status());
    assert_eq!(
        first
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message())
            .collect::<Vec<_>>(),
        second
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message())
            .collect::<Vec<_>>()
    );
}

#[test]
fn adapter_resolved_shared_values_remain_distinguishable() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"adapter values".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let mut inputs = typst::foundations::Dict::new();
    inputs.insert(
        "unused".into(),
        typst::foundations::Value::Str("resolved".into()),
    );
    let result = compile(
        PackCompilationRequest::new(pack, OutputFormat::Svg)
            .adapter_resolved_inputs(inputs)
            .adapter_resolved_feature(typst::Feature::A11yExtras)
            .adapter_resolved_document_time(Some(
                typst::foundations::Datetime::from_ymd(2024, 2, 3).unwrap(),
            )),
    )
    .unwrap();
    let inventory = result.request_inventory();

    assert_eq!(
        inventory.inputs().origin(),
        RequestValueOrigin::AdapterResolved
    );
    assert_eq!(
        inventory.document_time().origin(),
        RequestValueOrigin::AdapterResolved
    );
    assert_eq!(
        inventory.features()[0].origin(),
        RequestValueOrigin::AdapterResolved
    );
}

#[cfg(feature = "cli")]
#[test]
fn offset_aware_document_timestamps_on_the_same_utc_date_have_distinct_identities() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: datetime.today(offset: 2).day() * 1pt, height: 10pt, margin: 0pt)"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let early = 1_704_069_000; // 2024-01-01 00:30:00 UTC.
    let late = 1_704_151_800; // 2024-01-01 23:30:00 UTC.
    assert_eq!(
        typst_kit::datetime::Time::fixed_timestamp(early)
            .unwrap()
            .today(None),
        typst_kit::datetime::Time::fixed_timestamp(late)
            .unwrap()
            .today(None)
    );

    let first = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg)
            .adapter_resolved_document_timestamp(early)
            .unwrap(),
    )
    .unwrap();
    let second = compile(
        PackCompilationRequest::new(pack, OutputFormat::Svg)
            .adapter_resolved_document_timestamp(late)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        first.request_inventory().document_timestamp().value(),
        &Some(early)
    );
    assert_eq!(
        first.request_inventory().document_timestamp().origin(),
        RequestValueOrigin::AdapterResolved
    );
    assert_ne!(first.compilation_identity(), second.compilation_identity());
    assert_ne!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());
}

#[test]
fn pack_bound_compilation_does_not_use_package_caches_or_network() {
    let package = "@preview/example:1.0.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@preview/example:1.0.0\": *".to_vec(),
        )
        .unwrap()
        .external_package_file(package, "lib.typ", b"#let value = 1".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg));

    assert!(matches!(
        result,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MissingExternalPackageFulfillment { packages },
            ..
        }) if packages.len() == 1
    ));
    let report = compile_report(PackCompilationRequest::new(pack, OutputFormat::Svg)).unwrap();
    assert!(matches!(
        report.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::MissingExternalPackageFulfillment { .. },
            ..
        }
    ));
    assert_eq!(report.fulfillments().packages().len(), 1);
    assert!(!report.fulfillments().packages()[0].embedded());
}

#[test]
fn external_package_fulfillment_is_verified_before_official_compilation() {
    let package: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let source = b"#let value = 42".to_vec();
    let manifest =
        b"[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"".to_vec();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/example:1.0.0\": value\n#value".to_vec(),
        )
        .unwrap()
        .external_package_file(package.clone(), "lib.typ", source.clone())
        .unwrap()
        .external_package_file(package.clone(), "typst.toml", manifest.clone())
        .unwrap()
        .build()
        .unwrap();

    let malformed = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).package_fulfillment(
            package.clone(),
            PackageTreeFulfillment::new([("../lib.typ", source.clone())]),
        ),
    );
    assert!(matches!(
        malformed,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MalformedExternalPackageTree { spec, .. },
            ..
        }) if spec == package
    ));

    let mismatched = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).package_fulfillment(
            package.clone(),
            PackageTreeFulfillment::new([
                ("lib.typ", b"#let value = 7".to_vec()),
                ("typst.toml", manifest.clone()),
            ]),
        ),
    );
    assert!(matches!(
        mismatched,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MismatchedExternalPackageTree { spec, .. },
            ..
        }) if spec == package
    ));

    let baseline = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).package_fulfillment(
            package.clone(),
            PackageTreeFulfillment::new([
                ("lib.typ", source.clone()),
                ("typst.toml", manifest.clone()),
            ]),
        ),
    )
    .unwrap();
    let with_telemetry = compile_report(
        PackCompilationRequest::new(pack, OutputFormat::Svg).package_fulfillment(
            package,
            PackageTreeFulfillment::new([("lib.typ", source), ("typst.toml", manifest)])
                .provenance("memory:test")
                .cache_hit(true),
        ),
    )
    .unwrap();
    let telemetry_result = with_telemetry.result().unwrap();

    assert_eq!(baseline.status(), CompilationStatus::Succeeded);
    assert_eq!(
        baseline.compilation_identity(),
        telemetry_result.compilation_identity()
    );
    assert_eq!(
        baseline.result_identity(),
        telemetry_result.result_identity()
    );
    assert_eq!(
        baseline.artifacts()[0].bytes(),
        telemetry_result.artifacts()[0].bytes()
    );
    assert_eq!(baseline.diagnostics(), telemetry_result.diagnostics());
    let fulfillment = &with_telemetry.fulfillments().packages()[0];
    assert_eq!(fulfillment.provenance(), Some("memory:test"));
    assert!(fulfillment.cache_hit());
    assert!(
        telemetry_result
            .access_trace()
            .observations()
            .any(|observation| {
                observation
                    .logical_path()
                    .contains("@local/example:1.0.0/lib.typ")
            })
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn external_font_fulfillment_is_verified_before_official_compilation() {
    let font = typst_kit::fonts::embedded().next().unwrap().0;
    let data = font.data().to_vec();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Exact font".to_vec())
        .unwrap()
        .external_font(data.clone(), font.index())
        .unwrap()
        .build()
        .unwrap();
    let requirement = pack.font_requirements()[0].clone();

    let missing = compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg));
    assert!(matches!(
        missing,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MissingExternalFontFulfillment { containers },
            ..
        }) if containers == vec![requirement.container_identity()]
    ));

    let mut wrong = data.clone();
    wrong.push(0);
    let mismatched = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).font_fulfillment(
            requirement.container_identity(),
            FontContainerFulfillment::new(wrong),
        ),
    );
    assert!(matches!(
        mismatched,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MismatchedExternalFontContainer { expected, .. },
            ..
        }) if expected == requirement.container_identity()
    ));

    let baseline = compile(
        PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).font_fulfillment(
            requirement.container_identity(),
            FontContainerFulfillment::new(data.clone()),
        ),
    )
    .unwrap();
    let with_metadata = compile_report(
        PackCompilationRequest::new(pack, OutputFormat::Svg).font_fulfillment(
            requirement.container_identity(),
            FontContainerFulfillment::new(data)
                .provenance("memory:test")
                .licensing("advisory:test"),
        ),
    )
    .unwrap();
    let metadata_result = with_metadata.result().unwrap();
    assert_eq!(metadata_result.status(), CompilationStatus::Succeeded);
    assert_eq!(
        baseline.compilation_identity(),
        metadata_result.compilation_identity()
    );
    assert_eq!(
        baseline.result_identity(),
        metadata_result.result_identity()
    );
    assert_eq!(
        baseline.artifacts()[0].bytes(),
        metadata_result.artifacts()[0].bytes()
    );
    assert_eq!(
        baseline.diagnostics().len(),
        metadata_result.diagnostics().len()
    );
    let fulfillment = &with_metadata.fulfillments().fonts()[0];
    assert_eq!(fulfillment.provenance(), Some("memory:test"));
    assert_eq!(fulfillment.licensing(), Some("advisory:test"));
}

#[test]
fn pack_bound_compilation_rejects_the_bundle_feature() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    let request =
        PackCompilationRequest::new(pack, OutputFormat::Svg).feature(typst::Feature::Bundle);

    assert!(matches!(
        compile(request),
        Err(PackCompileError::RequestRejected {
            rejection: CompilationRequestRejection::UnsupportedBundleFeature,
            ..
        })
    ));
}

#[test]
fn pack_bound_compilation_rejects_invalid_png_resolution() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"invalid resolution".to_vec())
        .unwrap()
        .build()
        .unwrap();

    for ppi in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let options = CompileOptions {
            ppi: Some(ppi),
            ..CompileOptions::default()
        };
        assert!(matches!(
            compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Png).options(options)),
            Err(PackCompileError::RequestRejected {
                rejection: CompilationRequestRejection::InvalidPpi,
                ..
            })
        ));
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_bound_compilation_does_not_use_unpacked_embedded_fonts() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Libertinus Serif\")\nHello".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();

    let output = compile(PackCompilationRequest::new(pack, OutputFormat::Svg)).unwrap();

    assert!(
        output
            .diagnostics()
            .iter()
            .any(|warning| warning.message().contains("unknown font family"))
    );
}

#[test]
fn official_exporter_rejection_is_a_scoped_compilation_result() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#pdf.attach(\"duplicate.txt\", bytes(\"first\"))\n\
              #pdf.attach(\"duplicate.txt\", bytes(\"second\"))"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack, OutputFormat::Pdf)).unwrap();

    assert_eq!(result.status(), CompilationStatus::Rejected);
    assert!(result.artifacts().is_empty());
    assert_eq!(result.source_page_count(), Some(1));
    assert!(result.diagnostics().iter().any(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Export
            && diagnostic.producer() == DiagnosticProducer::Exporter(result.exporter_identity())
            && diagnostic.message().contains("attempted to attach file")
    }));
}

#[test]
fn pdf_is_one_document_format_artifact() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 10pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let output = compile(PackCompilationRequest::new(pack, OutputFormat::Pdf)).unwrap();

    assert_eq!(output.artifacts().len(), 1);
    let artifact = output.artifacts()[0].clone();
    assert_eq!(artifact.format(), OutputFormat::Pdf);
    assert_eq!(artifact.source_page_number(), None);
    assert!(artifact.bytes().starts_with(b"%PDF"));
    assert!(artifact.into_bytes().starts_with(b"%PDF"));
}

#[test]
fn page_format_selection_matching_no_source_page_produces_no_artifacts() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#page(width: 10pt, height: 10pt, margin: 0pt)[#rect(width: 1pt, height: 1pt)]"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    for expression in ["9", "9-"] {
        let options = CompileOptions {
            page_selection: typst_pack::parse_page_selection(expression).unwrap(),
            ..CompileOptions::default()
        };

        for format in [OutputFormat::Png, OutputFormat::Svg] {
            let output =
                compile(PackCompilationRequest::new(pack.clone(), format).options(options.clone()))
                    .unwrap();
            assert!(output.artifacts().is_empty());
        }
    }
}

#[test]
fn page_format_artifacts_preserve_source_page_identity() {
    let pack = five_page_pack();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        ppi: Some(72.0),
        ..CompileOptions::default()
    };

    let output =
        compile(PackCompilationRequest::new(pack, OutputFormat::Png).options(options)).unwrap();

    let source_pages = output
        .artifacts()
        .iter()
        .map(|artifact| artifact.source_page_number().unwrap().get())
        .collect::<Vec<_>>();
    assert_eq!(source_pages, [2, 5]);
    assert!(
        output
            .artifacts()
            .iter()
            .all(|artifact| artifact.format() == OutputFormat::Png)
    );
    let widths = output
        .artifacts()
        .iter()
        .map(|artifact| {
            tiny_skia::Pixmap::decode_png(artifact.bytes())
                .unwrap()
                .width()
        })
        .collect::<Vec<_>>();
    assert_eq!(widths, [20, 50]);
}

#[test]
fn page_format_artifacts_are_ordered_and_deduplicated_by_source_page() {
    let pack = five_page_pack();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2-4,2,3-5,1").unwrap(),
        ..CompileOptions::default()
    };

    for format in [OutputFormat::Png, OutputFormat::Svg] {
        let output =
            compile(PackCompilationRequest::new(pack.clone(), format).options(options.clone()))
                .unwrap();
        let source_pages = output
            .artifacts()
            .iter()
            .map(|artifact| artifact.source_page_number().unwrap().get())
            .collect::<Vec<_>>();

        assert_eq!(source_pages, [1, 2, 3, 4, 5]);
        assert!(
            output
                .artifacts()
                .iter()
                .all(|artifact| artifact.format() == format)
        );
    }
}

#[test]
fn page_range_membership_preserves_typst_selection_semantics() {
    let pack = five_page_pack();
    let cases = [
        (None, vec![1, 2, 3, 4, 5]),
        (Some("-3"), vec![1, 2, 3]),
        (Some("4-"), vec![4, 5]),
        (Some("4-9"), vec![4, 5]),
        (Some("5,2"), vec![2, 5]),
    ];

    for (expression, expected) in cases {
        let options = CompileOptions {
            page_selection: expression
                .map(typst_pack::parse_page_selection)
                .transpose()
                .unwrap()
                .unwrap_or_default(),
            ..CompileOptions::default()
        };
        let output =
            compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Svg).options(options))
                .unwrap();
        let source_pages = output
            .artifacts()
            .iter()
            .map(|artifact| artifact.source_page_number().unwrap().get())
            .collect::<Vec<_>>();

        assert_eq!(source_pages, expected, "selection {expression:?}");
    }
}

#[test]
fn invalid_textual_page_expressions_fail_parsing() {
    for expression in ["", "0", "-", "5-3", "1,", ",1", "1--2", "nope"] {
        assert!(
            typst_pack::parse_page_selection(expression).is_err(),
            "expression {expression:?} parsed successfully"
        );
    }
}

#[test]
fn pdf_page_selection_produces_one_document_format_artifact() {
    let pack = five_page_pack();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        ..CompileOptions::default()
    };

    let output =
        compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options)).unwrap();

    assert_eq!(output.artifacts().len(), 1);
    assert_eq!(output.artifacts()[0].format(), OutputFormat::Pdf);
    assert_eq!(output.artifacts()[0].source_page_number(), None);
    assert!(output.artifacts()[0].bytes().starts_with(b"%PDF"));
    let pdf = hayro_syntax::Pdf::new(output.artifacts()[0].bytes().to_vec()).unwrap();
    let page_widths = pdf
        .pages()
        .iter()
        .map(|page| page.render_dimensions().0)
        .collect::<Vec<_>>();
    assert_eq!(page_widths, [20.0, 50.0]);
}

#[test]
fn pdf_page_selection_matching_no_source_page_still_produces_a_pdf() {
    let pack = five_page_pack();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("9-").unwrap(),
        ..CompileOptions::default()
    };

    let output =
        compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options)).unwrap();

    assert_eq!(output.artifacts().len(), 1);
    assert!(output.artifacts()[0].bytes().starts_with(b"%PDF"));
}

#[test]
fn pdf_page_selection_warns_that_accessibility_tags_are_disabled() {
    let pack = five_page_pack();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        ..CompileOptions::default()
    };

    let output =
        compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Pdf).options(options))
            .unwrap();

    assert!(
        output
            .pack_warnings()
            .iter()
            .any(|warning| warning.message().contains("--pages implies --no-pdf-tags"))
    );

    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        pdf_tags: typst::foundations::Smart::Custom(false),
        ..CompileOptions::default()
    };
    let output =
        compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options)).unwrap();
    assert!(
        output
            .pack_warnings()
            .iter()
            .all(|warning| !warning.message().contains("--pages implies --no-pdf-tags"))
    );
}

#[test]
fn pack_owned_pdf_warning_is_not_attributed_to_the_engine() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Pack warning".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        ..CompileOptions::default()
    };

    let result =
        compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options)).unwrap();

    assert!(result.diagnostics().is_empty());
    assert_eq!(result.pack_warnings().len(), 1);
    assert!(
        result.pack_warnings()[0]
            .message()
            .contains("--pages implies --no-pdf-tags")
    );
}

#[test]
fn explicit_pdf_tags_with_page_selection_are_rejected_before_compilation() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"One page".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        pdf_tags: typst::foundations::Smart::Custom(true),
        ..CompileOptions::default()
    };

    let rejection = compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options));

    assert!(matches!(
        rejection,
        Err(PackCompileError::RequestRejected {
            rejection: CompilationRequestRejection::PdfTagsWithPageSelection,
            ..
        })
    ));
}

#[test]
fn tag_required_pdf_standard_without_tags_is_rejected_before_compilation() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"One page".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let options = CompileOptions {
        pdf_standards: vec![typst_pdf::PdfStandard::Ua_1],
        pdf_tags: typst::foundations::Smart::Custom(false),
        ..CompileOptions::default()
    };

    let rejection = compile(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options));

    assert!(matches!(
        rejection,
        Err(PackCompileError::RequestRejected {
            rejection: CompilationRequestRejection::PdfStandardRequiresTags,
            ..
        })
    ));
}

#[test]
fn pack_request_rejection_collects_independent_pdf_issues_in_stable_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#panic(\"must not compile\")".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        pdf_standards: vec![
            typst_pdf::PdfStandard::V_2_0,
            typst_pdf::PdfStandard::A_1b,
            typst_pdf::PdfStandard::Ua_1,
        ],
        ..CompileOptions::default()
    };
    let request = PackCompilationRequest::new(pack, OutputFormat::Pdf)
        .feature(typst::Feature::Bundle)
        .options(options);

    let Err(PackCompileError::RequestRejected { rejection, .. }) = compile(request) else {
        panic!("expected a Pack request rejection");
    };

    assert_eq!(rejection.issues().len(), 3);
    assert!(matches!(
        rejection.issues()[0],
        CompilationRequestRejection::UnsupportedBundleFeature
    ));
    assert!(matches!(
        rejection.issues()[1],
        CompilationRequestRejection::InvalidPdfStandards(_)
    ));
    assert!(matches!(
        rejection.issues()[2],
        CompilationRequestRejection::PdfStandardRequiresTags
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn html_is_one_document_format_artifact() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello from HTML".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let output = compile(PackCompilationRequest::new(pack, OutputFormat::Html)).unwrap();

    assert_eq!(output.artifacts().len(), 1);
    assert_eq!(output.artifacts()[0].format(), OutputFormat::Html);
    assert_eq!(output.artifacts()[0].source_page_number(), None);
    assert!(
        std::str::from_utf8(output.artifacts()[0].bytes())
            .unwrap()
            .contains("Hello from HTML")
    );
    assert!(!output.diagnostics().is_empty());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pretty_affects_html_svg_and_pdf_but_not_png() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let compact = CompileOptions::default();
    let pretty = CompileOptions {
        pretty: true,
        ..CompileOptions::default()
    };

    for format in [OutputFormat::Html, OutputFormat::Svg, OutputFormat::Pdf] {
        let compact =
            compile(PackCompilationRequest::new(pack.clone(), format).options(compact.clone()))
                .unwrap();
        let pretty =
            compile(PackCompilationRequest::new(pack.clone(), format).options(pretty.clone()))
                .unwrap();
        assert_ne!(
            compact.artifacts()[0].bytes(),
            pretty.artifacts()[0].bytes()
        );
    }

    let compact =
        compile(PackCompilationRequest::new(pack.clone(), OutputFormat::Png).options(compact))
            .unwrap();
    let pretty =
        compile(PackCompilationRequest::new(pack, OutputFormat::Png).options(pretty)).unwrap();
    assert_eq!(
        compact.artifacts()[0].bytes(),
        pretty.artifacts()[0].bytes()
    );
}

#[test]
fn compilation_result_identity_binds_status_document_trace_and_artifacts() {
    let compile_source = |source: &[u8]| {
        let pack = Pack::builder("main.typ")
            .file("main.typ", source.to_vec())
            .unwrap()
            .build()
            .unwrap();
        compile(PackCompilationRequest::new(pack, OutputFormat::Svg)).unwrap()
    };
    let first = compile_source(
        b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)",
    );
    let changed = compile_source(
        b"#set page(width: 30pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)#pagebreak()#rect(width: 2pt, height: 2pt)",
    );
    let rejected = compile_source(b"#unknown-name");

    assert_eq!(first.status(), CompilationStatus::Succeeded);
    assert_eq!(rejected.status(), CompilationStatus::Rejected);
    assert_ne!(first.result_identity(), changed.result_identity());
    assert_ne!(first.result_identity(), rejected.result_identity());
    assert_ne!(first.document(), changed.document());
    assert_ne!(first.artifacts()[0].bytes(), changed.artifacts()[0].bytes());
    assert!(!rejected.diagnostics().is_empty());
    assert!(
        !first
            .access_trace()
            .observations()
            .eq(rejected.access_trace().observations())
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn empty_page_format_output_retains_compilation_warnings() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set text(font: \"Definitely Missing\")\nWarning".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("9").unwrap(),
        ..CompileOptions::default()
    };

    let output =
        compile(PackCompilationRequest::new(pack, OutputFormat::Svg).options(options)).unwrap();

    assert!(output.artifacts().is_empty());
    assert!(!output.diagnostics().is_empty());
}
