use typst_pack::{
    CompilationOperationOutcome, CompilationRequestRejection, CompilationStatus, CompileOptions,
    DiagnosticPhase, DiagnosticProducer, OutputFormat, Pack, PackCompilationRequest,
    PackCompileError, PackWorld, compile, compile_pack,
};

fn five_page_world() -> PackWorld {
    let source = (1..=5)
        .map(|page| {
            format!(
                "#set page(width: {page}0pt, height: 10pt, margin: 0pt)\n\
                 #rect(width: 1pt, height: 1pt)\n"
            ) + if page < 5 { "#pagebreak()\n" } else { "" }
        })
        .collect::<String>();
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap()
        .build()
        .unwrap();

    PackWorld::builder(pack).build()
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

    let result = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
}

#[test]
fn pack_bound_compilation_does_not_read_an_ambient_clock() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#datetime.today()".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let result = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
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
        .unvendored_package(package)
        .build()
        .unwrap();

    let result = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg));

    assert!(matches!(
        result,
        Err(PackCompileError::Operation(
            CompilationOperationOutcome::MissingExternalPackageFulfillment { packages }
        )) if packages.len() == 1
    ));
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
        compile_pack(request),
        Err(PackCompileError::RequestRejected(
            CompilationRequestRejection::UnsupportedBundleFeature
        ))
    ));
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

    let output = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg)).unwrap();

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

    let result = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Pdf)).unwrap();

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
    let world = PackWorld::builder(pack).build();

    let output = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();

    assert_eq!(output.artifacts.len(), 1);
    let artifact = output.artifacts.into_iter().next().unwrap();
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
    let world = PackWorld::builder(pack).build();
    for expression in ["9", "9-"] {
        let options = CompileOptions {
            page_selection: typst_pack::parse_page_selection(expression).unwrap(),
            ..CompileOptions::default()
        };

        for format in [OutputFormat::Png, OutputFormat::Svg] {
            let output = compile(&world, format, &options).unwrap();
            assert!(output.artifacts.is_empty());
        }
    }
}

#[test]
fn page_format_artifacts_preserve_source_page_identity() {
    let world = five_page_world();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        ppi: Some(72.0),
        ..CompileOptions::default()
    };

    let output = compile(&world, OutputFormat::Png, &options).unwrap();

    let source_pages = output
        .artifacts
        .iter()
        .map(|artifact| artifact.source_page_number().unwrap().get())
        .collect::<Vec<_>>();
    assert_eq!(source_pages, [2, 5]);
    assert!(
        output
            .artifacts
            .iter()
            .all(|artifact| artifact.format() == OutputFormat::Png)
    );
    let widths = output
        .artifacts
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
    let world = five_page_world();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2-4,2,3-5,1").unwrap(),
        ..CompileOptions::default()
    };

    for format in [OutputFormat::Png, OutputFormat::Svg] {
        let output = compile(&world, format, &options).unwrap();
        let source_pages = output
            .artifacts
            .iter()
            .map(|artifact| artifact.source_page_number().unwrap().get())
            .collect::<Vec<_>>();

        assert_eq!(source_pages, [1, 2, 3, 4, 5]);
        assert!(
            output
                .artifacts
                .iter()
                .all(|artifact| artifact.format() == format)
        );
    }
}

#[test]
fn page_range_membership_preserves_typst_selection_semantics() {
    let world = five_page_world();
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
        let output = compile(&world, OutputFormat::Svg, &options).unwrap();
        let source_pages = output
            .artifacts
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
    let world = five_page_world();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        ..CompileOptions::default()
    };

    let output = compile(&world, OutputFormat::Pdf, &options).unwrap();

    assert_eq!(output.artifacts.len(), 1);
    assert_eq!(output.artifacts[0].format(), OutputFormat::Pdf);
    assert_eq!(output.artifacts[0].source_page_number(), None);
    assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
    let pdf = hayro_syntax::Pdf::new(output.artifacts[0].bytes().to_vec()).unwrap();
    let page_widths = pdf
        .pages()
        .iter()
        .map(|page| page.render_dimensions().0)
        .collect::<Vec<_>>();
    assert_eq!(page_widths, [20.0, 50.0]);
}

#[test]
fn pdf_page_selection_matching_no_source_page_still_produces_a_pdf() {
    let world = five_page_world();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("9-").unwrap(),
        ..CompileOptions::default()
    };

    let output = compile(&world, OutputFormat::Pdf, &options).unwrap();

    assert_eq!(output.artifacts.len(), 1);
    assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
}

#[test]
fn pdf_page_selection_warns_that_accessibility_tags_are_disabled() {
    let world = five_page_world();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        ..CompileOptions::default()
    };

    let output = compile(&world, OutputFormat::Pdf, &options).unwrap();

    assert!(
        output
            .pack_warnings()
            .iter()
            .any(|warning| warning.message.contains("--pages implies --no-pdf-tags"))
    );

    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        pdf_tags: false,
        ..CompileOptions::default()
    };
    let output = compile(&world, OutputFormat::Pdf, &options).unwrap();
    assert!(
        output
            .pack_warnings()
            .iter()
            .all(|warning| !warning.message.contains("--pages implies --no-pdf-tags"))
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
        compile_pack(PackCompilationRequest::new(pack, OutputFormat::Pdf).options(options))
            .unwrap();

    assert!(result.diagnostics().is_empty());
    assert_eq!(result.pack_warnings().len(), 1);
    assert!(
        result.pack_warnings()[0]
            .message()
            .contains("--pages implies --no-pdf-tags")
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn html_is_one_document_format_artifact() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello from HTML".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack)
        .feature(typst::Feature::Html)
        .build();

    let output = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();

    assert_eq!(output.artifacts.len(), 1);
    assert_eq!(output.artifacts[0].format(), OutputFormat::Html);
    assert_eq!(output.artifacts[0].source_page_number(), None);
    assert!(
        std::str::from_utf8(output.artifacts[0].bytes())
            .unwrap()
            .contains("Hello from HTML")
    );
    assert!(!output.warnings.is_empty());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pretty_affects_html_svg_and_pdf_but_not_png() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack)
        .feature(typst::Feature::Html)
        .build();
    let compact = CompileOptions::default();
    let pretty = CompileOptions {
        pretty: true,
        ..CompileOptions::default()
    };

    for format in [OutputFormat::Html, OutputFormat::Svg, OutputFormat::Pdf] {
        let compact = compile(&world, format, &compact).unwrap();
        let pretty = compile(&world, format, &pretty).unwrap();
        assert_ne!(compact.artifacts[0].bytes(), pretty.artifacts[0].bytes());
    }

    let compact = compile(&world, OutputFormat::Png, &compact).unwrap();
    let pretty = compile(&world, OutputFormat::Png, &pretty).unwrap();
    assert_eq!(compact.artifacts[0].bytes(), pretty.artifacts[0].bytes());
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
    let world = PackWorld::builder(pack).build();
    let options = CompileOptions {
        page_selection: typst_pack::parse_page_selection("9").unwrap(),
        ..CompileOptions::default()
    };

    let output = compile(&world, OutputFormat::Svg, &options).unwrap();

    assert!(output.artifacts.is_empty());
    assert!(!output.warnings.is_empty());
}
