use std::num::NonZeroUsize;

use typst::foundations::Smart;
use typst::syntax::package::PackageSpec;
use typst_pack::{
    CompilationLimits, CompileError, CompileRequest, Compiled, CreationTimestamp, DocumentTime,
    HtmlOutput, Input, LimitExceeded, Output, OutputFormat, Pack, PackMetadata, PackageTree,
    PageSelection, PdfOutput, PngOutput, RequestIssue, SvgOutput, Unfulfilled, compile,
    parse_page_selection,
};

fn pack(source: &str) -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
        .build()
        .unwrap()
}

fn compiled(request: &CompileRequest) -> Compiled {
    compile(request).unwrap_or_else(|error| panic!("compilation failed: {error}"))
}

fn compile_to(pack: Pack, output: Output) -> Compiled {
    compiled(&CompileRequest::new(pack, output))
}

fn pages(compiled: &Compiled) -> Vec<usize> {
    let pages = compiled.artifacts.iter().map(|artifact| artifact.page);
    pages.map(|page| page.unwrap().get()).collect()
}

fn selection(text: &str) -> PageSelection {
    parse_page_selection(text).unwrap()
}

fn page_pack(widths: &[u32]) -> Pack {
    let pages = widths.iter().map(|width| {
        format!(
            "#page(width: {width}pt, height: 10pt, margin: 0pt)[#rect(width: 1pt, height: 1pt)]"
        )
    });
    pack(&pages.collect::<Vec<_>>().join("\n"))
}

fn five_page_pack() -> Pack {
    page_pack(&[10, 20, 30, 40, 50])
}

fn svg(page_selection: PageSelection) -> Output {
    Output::Svg(SvgOutput {
        page_selection,
        ..SvgOutput::default()
    })
}

fn png(ppi: f64) -> Output {
    Output::Png(PngOutput {
        pixels_per_inch: Some(ppi),
        ..PngOutput::default()
    })
}

fn limited(
    pack: Pack,
    output: Output,
    limits: CompilationLimits,
) -> Result<Compiled, CompileError> {
    let mut request = CompileRequest::new(pack, output);
    request.limits = limits;
    compile(&request)
}

fn limit_error(result: Result<Compiled, CompileError>) -> (&'static str, u64) {
    match result {
        Err(CompileError::Limit(LimitExceeded { resource, limit })) => (resource, limit),
        other => panic!("expected an exceeded limit, got {other:?}"),
    }
}

#[test]
fn every_output_format_produces_its_artifacts() {
    let pdf = compile_to(five_page_pack(), Output::new(OutputFormat::Pdf));
    assert_eq!(pdf.artifacts.len(), 1);
    assert_eq!(pdf.artifacts[0].format, OutputFormat::Pdf);
    assert_eq!(pdf.artifacts[0].page, None);
    assert!(pdf.artifacts[0].bytes.starts_with(b"%PDF"));
    assert_eq!(pdf.page_count, Some(5));

    let pngs = compile_to(five_page_pack(), png(72.0));
    assert_eq!(pages(&pngs), [1, 2, 3, 4, 5]);
    let widths = pngs.artifacts.iter().map(|artifact| {
        tiny_skia::Pixmap::decode_png(&artifact.bytes)
            .unwrap()
            .width()
    });
    assert_eq!(widths.collect::<Vec<_>>(), [10, 20, 30, 40, 50]);
    let doubled = compile_to(page_pack(&[10]), png(144.0));
    assert_eq!(
        tiny_skia::Pixmap::decode_png(&doubled.artifacts[0].bytes)
            .unwrap()
            .width(),
        20
    );

    let svg = compile_to(five_page_pack(), Output::new(OutputFormat::Svg));
    assert_eq!(pages(&svg), [1, 2, 3, 4, 5]);
    assert!(svg.artifacts[0].bytes.starts_with(b"<svg"));

    let html = compile_to(pack("Hello from HTML"), Output::new(OutputFormat::Html));
    assert_eq!(html.artifacts.len(), 1);
    assert_eq!(html.page_count, None);
    let text = std::str::from_utf8(&html.artifacts[0].bytes).unwrap();
    assert!(text.contains("Hello from HTML"), "{text}");
    // Typst warns that HTML export is experimental.
    assert!(!html.warnings.is_empty());
}

#[test]
fn page_selection_follows_typst_semantics() {
    for (expression, expected) in [
        ("-3", vec![1, 2, 3]),
        ("4-", vec![4, 5]),
        ("4-9", vec![4, 5]),
        ("5,2", vec![2, 5]),
        ("5,2-4,2,3-5,1", vec![1, 2, 3, 4, 5]),
        ("9", vec![]),
        ("9-", vec![]),
    ] {
        let compiled = compile_to(five_page_pack(), svg(selection(expression)));
        assert_eq!(pages(&compiled), expected, "{expression}");
    }

    let pdf = compile_to(
        five_page_pack(),
        Output::Pdf(PdfOutput {
            page_selection: selection("5,2"),
            ..PdfOutput::default()
        }),
    );
    let document = hayro_syntax::Pdf::new(pdf.artifacts[0].bytes.clone()).unwrap();
    let widths = document
        .pages()
        .iter()
        .map(|page| page.render_dimensions().0);
    assert_eq!(widths.collect::<Vec<_>>(), [20.0, 50.0]);

    for expression in ["", "0", "-", "5-3", "1,", ",1", "1--2", "nope"] {
        assert!(parse_page_selection(expression).is_err(), "{expression:?}");
    }
}

#[test]
fn pdf_page_selection_disables_automatic_tags_with_a_warning() {
    let warned = |tags| {
        let compiled = compile_to(
            pack("Tagged"),
            Output::Pdf(PdfOutput {
                page_selection: selection("1"),
                tags,
                ..PdfOutput::default()
            }),
        );
        compiled
            .warnings
            .iter()
            .any(|warning| warning.message.contains("--pages implies --no-pdf-tags"))
    };
    assert!(warned(Smart::Auto));
    assert!(!warned(Smart::Custom(false)));
}

#[test]
fn exporter_rejections_are_failed_compilations() {
    let rejected = |source: &str, pdf: PdfOutput| match compile(&CompileRequest::new(
        pack(source),
        Output::Pdf(pdf),
    )) {
        Err(CompileError::Failed { errors, .. }) => errors,
        other => panic!("expected an exporter rejection, got {other:?}"),
    };

    let errors = rejected(
        "#pdf.attach(\"twice.txt\", bytes(\"a\"))\n#pdf.attach(\"twice.txt\", bytes(\"b\"))",
        PdfOutput::default(),
    );
    assert!(errors[0].message.contains("attempted to attach file"));

    let errors = rejected(
        "One page",
        PdfOutput {
            page_selection: selection("1"),
            tags: Smart::Custom(true),
            ..PdfOutput::default()
        },
    );
    assert!(errors[0].message.contains("tagged PDF"), "{errors:?}");

    let errors = rejected(
        "One page",
        PdfOutput {
            standards: vec![typst_pdf::PdfStandard::Ua_1],
            tags: Smart::Custom(false),
            ..PdfOutput::default()
        },
    );
    assert!(errors[0].message.contains("PDF/UA-1"), "{errors:?}");
}

#[test]
fn pdf_creation_timestamps_are_recorded_as_requested() {
    let creation = |timestamp| {
        let compiled = compile_to(
            pack("Dated"),
            Output::Pdf(PdfOutput {
                creation_timestamp: timestamp,
                ..PdfOutput::default()
            }),
        );
        let document = hayro_syntax::Pdf::new(compiled.artifacts[0].bytes.clone()).unwrap();
        document.metadata().creation_date.map(|date| {
            (
                (date.year, date.month, date.day),
                (date.hour, date.minute, date.second),
                date.utc_offset_hour,
            )
        })
    };
    assert_eq!(creation(CreationTimestamp::Omit), None);
    assert_eq!(
        creation(CreationTimestamp::Utc(946_684_800)),
        Some(((2000, 1, 1), (0, 0, 0), 0))
    );
    assert_eq!(
        creation(CreationTimestamp::Local {
            unix: 946_684_800,
            offset_minutes: -90,
        }),
        Some(((1999, 12, 31), (22, 30, 0), -1))
    );
    assert_eq!(
        creation(CreationTimestamp::Utc(-86_400)),
        Some(((1969, 12, 31), (0, 0, 0), 0))
    );
    // Past the year 9999, as in Typst's CLI, nothing is recorded.
    assert_eq!(creation(CreationTimestamp::Utc(253_402_300_800)), None);
}

#[test]
fn pretty_affects_html_svg_and_pdf() {
    let outputs = [
        (
            Output::new(OutputFormat::Html),
            Output::Html(HtmlOutput { pretty: true }),
        ),
        (
            Output::new(OutputFormat::Svg),
            Output::Svg(SvgOutput {
                pretty: true,
                ..SvgOutput::default()
            }),
        ),
        (
            Output::new(OutputFormat::Pdf),
            Output::Pdf(PdfOutput {
                pretty: true,
                ..PdfOutput::default()
            }),
        ),
    ];
    for (compact, pretty) in outputs {
        let compact = compile_to(pack("Hello"), compact);
        let pretty = compile_to(pack("Hello"), pretty);
        assert_ne!(compact.artifacts[0].bytes, pretty.artifacts[0].bytes);
    }
}

#[test]
fn string_and_byte_inputs_reach_sys_inputs() {
    let logo = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#;
    let mut request = CompileRequest::new(
        pack(
            "#set page(width: auto, height: auto, margin: 0pt)\n\
             #assert.eq(sys.inputs.name, \"Acme\")\n\
             #assert.eq(type(sys.inputs.logo), bytes)\n\
             #image(sys.inputs.logo, width: int(sys.inputs.width) * 1pt)",
        ),
        Output::new(OutputFormat::Svg),
    );
    request.options.inputs = vec![
        ("name".into(), Input::from("Acme")),
        ("width".into(), Input::from("4")),
        ("logo".into(), Input::Bytes(logo.to_vec())),
    ];
    let narrow = compiled(&request);
    request
        .options
        .inputs
        .push(("width".into(), Input::from("8")));
    let wide = compiled(&request);

    assert_ne!(narrow.artifacts[0].bytes, wide.artifacts[0].bytes);
    assert_ne!(narrow.identity, wide.identity);
}

#[test]
fn document_time_is_explicit() {
    let source = "#datetime.today().display()";
    let mut request = CompileRequest::new(pack(source), Output::new(OutputFormat::Svg));
    assert!(matches!(
        compile(&request),
        Err(CompileError::Failed { .. })
    ));

    request.options.document_time = DocumentTime::UnixTimestamp(946_684_800);
    let fixed = compiled(&request);
    request.options.document_time =
        DocumentTime::Fixed(typst::foundations::Datetime::from_ymd(2000, 1, 1).unwrap());
    assert_eq!(compiled(&request).artifacts, fixed.artifacts);

    // A timezone offset can move the date of an instant.
    request.options.document_time = DocumentTime::UnixTimestamp(946_684_799);
    request.pack = pack("#assert.eq(datetime.today(offset: 2).year(), 2000)");
    compiled(&request);
}

#[test]
fn invalid_requests_list_every_issue() {
    let mut request = CompileRequest::new(
        pack("#panic(\"must not compile\")"),
        Output::Pdf(PdfOutput {
            page_selection: PageSelection::new(vec![NonZeroUsize::new(5)..=NonZeroUsize::new(2)]),
            standards: vec![typst_pdf::PdfStandard::A_1b, typst_pdf::PdfStandard::V_2_0],
            ..PdfOutput::default()
        }),
    );
    request.options.features.push(typst::Feature::Bundle);
    request.options.document_time = DocumentTime::UnixTimestamp(i64::MAX);

    let Err(error @ CompileError::InvalidRequest(_)) = compile(&request) else {
        panic!("expected an invalid request");
    };
    let CompileError::InvalidRequest(issues) = &error else {
        unreachable!()
    };
    assert_eq!(issues.len(), 4, "{issues:?}");
    assert!(issues.contains(&RequestIssue::InvalidPageRange {
        start: NonZeroUsize::new(5).unwrap(),
        end: NonZeroUsize::new(2).unwrap(),
    }));
    assert!(issues.contains(&RequestIssue::UnsupportedBundleFeature));
    assert!(issues.contains(&RequestIssue::InvalidDocumentTimestamp));
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, RequestIssue::InvalidPdfStandards { .. }))
    );
    let message = error.to_string();
    for issue in issues {
        assert!(message.contains(&issue.to_string()), "{message}");
    }
    assert!(!error.hints().is_empty());

    for ppi in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            compile(&CompileRequest::new(pack("Hi"), png(ppi))),
            Err(CompileError::InvalidRequest(issues)) if issues == [RequestIssue::InvalidPpi]
        ));
    }
}

#[test]
fn html_output_enables_the_html_feature_itself() {
    let request = CompileRequest::new(pack("#html.div[Hi]"), Output::new(OutputFormat::Html));
    assert!(request.options.features.is_empty());
    compiled(&request);
}

#[test]
fn failed_compilations_keep_diagnostics_and_sources() {
    let Err(CompileError::Failed {
        errors, sources, ..
    }) = compile(&CompileRequest::new(
        pack("#unknown-name"),
        Output::new(OutputFormat::Svg),
    ))
    else {
        panic!("expected a failed compilation");
    };
    assert!(errors[0].message.contains("unknown variable"));
    let id = errors[0].span.id().unwrap();
    assert_eq!(sources.name(id), "main.typ");
    assert!(typst::WorldExt::range(&sources, errors[0].span).is_some());
    assert!(sources.files().contains(&id));
}

#[test]
fn empty_page_output_keeps_warnings() {
    let compiled = compile_to(
        pack("#set text(font: \"Definitely Missing\")\nWarning"),
        svg(selection("9")),
    );
    assert!(compiled.artifacts.is_empty());
    assert!(!compiled.warnings.is_empty());
}

#[test]
fn compilation_reads_nothing_but_the_pack() {
    // No project file outside the pack, no package cache, and no system or
    // Typst-embedded font is reachable.
    let Err(CompileError::Failed { errors, .. }) = compile(&CompileRequest::new(
        pack("#read(\"Cargo.toml\")"),
        Output::new(OutputFormat::Svg),
    )) else {
        panic!("an ambient file was read");
    };
    assert!(errors[0].message.contains("not found"), "{errors:?}");

    let Err(CompileError::Failed { errors, .. }) = compile(&CompileRequest::new(
        pack("#import \"@preview/cetz:0.3.4\": *"),
        Output::new(OutputFormat::Svg),
    )) else {
        panic!("an undeclared package was reachable");
    };
    assert!(
        errors[0].message.contains("package not found"),
        "{errors:?}"
    );

    let compiled = compile_to(
        pack("#set text(font: \"Libertinus Serif\")\nHello"),
        Output::new(OutputFormat::Svg),
    );
    assert!(
        compiled
            .warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family"))
    );
}

fn spec(text: &str) -> PackageSpec {
    text.parse().unwrap()
}

#[test]
fn external_packages_must_be_supplied_exactly() {
    let package = spec("@local/example:1.0.0");
    let manifest = "[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"";
    let tree = |value: &str| {
        PackageTree::from_owned_entries([
            ("lib.typ", format!("#let value = {value}").into_bytes()),
            ("typst.toml", manifest.as_bytes().to_vec()),
        ])
        .unwrap()
    };
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/example:1.0.0\": value\n#value".to_vec(),
        )
        .external_package_file(package.clone(), "lib.typ", b"#let value = 42".to_vec())
        .external_package_file(package.clone(), "typst.toml", manifest.as_bytes().to_vec())
        .build()
        .unwrap();
    let mut request = CompileRequest::new(pack, Output::new(OutputFormat::Svg));

    let missing = compile(&request).unwrap_err();
    assert!(matches!(
        &missing,
        CompileError::Unfulfilled(issues)
            if issues == &[Unfulfilled::MissingPackage { spec: package.clone() }]
    ));
    assert!(
        missing.to_string().contains("@local/example:1.0.0"),
        "{missing}"
    );
    assert!(!missing.hints().is_empty());

    request.packages = vec![(package.clone(), tree("7"))];
    let mismatched = compile(&request).unwrap_err();
    assert!(matches!(
        &mismatched,
        CompileError::Unfulfilled(issues)
            if matches!(issues.as_slice(), [Unfulfilled::MismatchedPackage { spec, .. }] if *spec == package)
    ));
    assert!(mismatched.to_string().contains("@local/example:1.0.0"));

    // Unrelated and wrong candidates are ignored once the exact tree is
    // supplied.
    request.packages = vec![
        (spec("@local/unrelated:1.0.0"), tree("1")),
        (package.clone(), tree("7")),
        (package.clone(), tree("42")),
    ];
    let compiled = compiled(&request);
    assert_eq!(compiled.artifacts.len(), 1);
    assert!(
        compiled
            .sources
            .files()
            .iter()
            .any(|id| { compiled.sources.name(*id) == "@local/example:1.0.0/lib.typ" })
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn external_fonts_are_matched_by_digest_and_named_when_missing() {
    let font = typst_kit::fonts::embedded().next().unwrap().0;
    let family = font.info().family.clone();
    let data = font.data().to_vec();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            format!("#set text(font: \"{family}\")\nExact").into_bytes(),
        )
        .external_font(data.clone())
        .build()
        .unwrap();
    let mut request = CompileRequest::new(pack, Output::new(OutputFormat::Svg));

    let missing = compile(&request).unwrap_err();
    assert!(matches!(
        &missing,
        CompileError::Unfulfilled(issues)
            if matches!(issues.as_slice(), [Unfulfilled::MissingFont { .. }])
    ));
    assert!(missing.to_string().contains(&family), "{missing}");

    let mut other = data.clone();
    other.push(0);
    request.fonts = vec![typst_pack::FontContainer::new(other).unwrap()];
    assert!(matches!(
        compile(&request),
        Err(CompileError::Unfulfilled(_))
    ));

    request
        .fonts
        .push(typst_pack::FontContainer::new(data).unwrap());
    let compiled = compiled(&request);
    assert!(
        compiled.warnings.is_empty(),
        "the exact font is used: {:?}",
        compiled.warnings
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_fonts_are_the_only_fonts() {
    let embedded = typst_kit::fonts::embedded().next().unwrap().0;
    let mut data = embedded.data().to_vec();
    data.push(0);
    let font = typst::text::Font::new(typst::foundations::Bytes::new(data.clone()), 0).unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            format!("#set text(font: \"{}\")\nHello", font.info().family).into_bytes(),
        )
        .font(data)
        .build()
        .unwrap();
    let compiled = compile_to(pack, Output::new(OutputFormat::Svg));
    assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
}

#[test]
fn vendored_packages_compile_from_the_pack() {
    let build = |entrypoint: &str| {
        Pack::builder("main.typ")
            .file(
                "main.typ",
                b"#import \"@local/inside:1.0.0\": mark\n#mark".to_vec(),
            )
            .package_file(
                spec("@local/inside:1.0.0"),
                "typst.toml",
                format!(
                    "[package]\nname = \"inside\"\nversion = \"1.0.0\"\nentrypoint = \"{entrypoint}\"\n"
                ),
            )
            .package_file(
                spec("@local/inside:1.0.0"),
                "lib.typ",
                b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
            )
            .build()
            .unwrap()
    };
    let svg = Output::new(OutputFormat::Svg);
    assert!(compile(&CompileRequest::new(build("lib.typ"), svg.clone())).is_ok());
    assert!(compile(&CompileRequest::new(build("missing.typ"), svg)).is_err());
}

#[test]
fn compilation_identity_binds_the_request_but_not_metadata() {
    let identity = |pack: Pack, output: Output, inputs: Vec<(String, Input)>| {
        let mut request = CompileRequest::new(pack, output);
        request.options.inputs = inputs;
        compiled(&request).identity
    };
    let source = "#rect(width: 1pt, height: 1pt)";
    let svg = Output::new(OutputFormat::Svg);
    let baseline = identity(pack(source), svg.clone(), vec![]);

    let described = Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
        .metadata(PackMetadata {
            name: Some("described".into()),
            ..PackMetadata::default()
        })
        .build()
        .unwrap();
    assert_eq!(identity(described, svg.clone(), vec![]), baseline);
    assert_eq!(
        identity(pack(source), svg.clone(), vec![]),
        baseline,
        "identities are stable"
    );

    for changed in [
        identity(pack("#rect(width: 2pt, height: 1pt)"), svg.clone(), vec![]),
        identity(pack(source), Output::new(OutputFormat::Pdf), vec![]),
        identity(
            pack(source),
            Output::Svg(SvgOutput {
                pretty: true,
                ..SvgOutput::default()
            }),
            vec![],
        ),
        identity(
            pack(source),
            svg.clone(),
            vec![("a".into(), Input::from("1"))],
        ),
        identity(
            pack(source),
            svg.clone(),
            vec![("a".into(), Input::Bytes(b"1".to_vec()))],
        ),
    ] {
        assert_ne!(changed, baseline);
    }

    // Equivalent page selections are one request.
    let selected = |text: &str| identity(five_page_pack(), svg_selection(text), vec![]);
    assert_eq!(selected("2-3,1"), selected("1-3"));
    assert_ne!(selected("1-3"), selected("1-2"));
}

fn svg_selection(text: &str) -> Output {
    svg(selection(text))
}

#[test]
fn limits_bound_pages_artifacts_pixels_and_bytes() {
    let tight = |limits: CompilationLimits| CompilationLimits {
        export_workers: 1,
        ..limits
    };
    let defaults = CompilationLimits::default();

    assert!(
        limited(
            five_page_pack(),
            Output::new(OutputFormat::Svg),
            tight(CompilationLimits {
                max_pages: 5,
                ..defaults
            })
        )
        .is_ok()
    );
    assert_eq!(
        limit_error(limited(
            five_page_pack(),
            Output::new(OutputFormat::Svg),
            CompilationLimits {
                max_pages: 4,
                ..defaults
            }
        )),
        ("pages", 4)
    );
    assert_eq!(
        limit_error(limited(
            five_page_pack(),
            Output::new(OutputFormat::Svg),
            CompilationLimits {
                max_artifacts: 4,
                ..defaults
            }
        )),
        ("artifacts", 4)
    );
    // One 10pt square page at 72 PPI is 100 pixels.
    let square = || page_pack(&[10]);
    assert!(
        limited(
            square(),
            png(72.0),
            CompilationLimits {
                max_pixels_per_artifact: 100,
                ..defaults
            }
        )
        .is_ok()
    );
    assert_eq!(
        limit_error(limited(
            square(),
            png(72.0),
            CompilationLimits {
                max_pixels_per_artifact: 99,
                ..defaults
            }
        )),
        ("pixels per artifact", 99)
    );
    assert_eq!(
        limit_error(limited(
            page_pack(&[10, 10]),
            png(72.0),
            CompilationLimits {
                max_total_pixels: 199,
                ..defaults
            }
        )),
        ("total pixels", 199)
    );

    let bytes = compile_to(square(), Output::new(OutputFormat::Svg)).artifacts[0]
        .bytes
        .len() as u64;
    assert!(
        limited(
            square(),
            Output::new(OutputFormat::Svg),
            CompilationLimits {
                max_artifact_bytes: bytes,
                ..defaults
            }
        )
        .is_ok()
    );
    assert_eq!(
        limit_error(limited(
            square(),
            Output::new(OutputFormat::Svg),
            CompilationLimits {
                max_artifact_bytes: bytes - 1,
                ..defaults
            }
        )),
        ("artifact bytes", bytes - 1)
    );
    let total = compile_to(page_pack(&[10, 10]), Output::new(OutputFormat::Svg))
        .artifacts
        .iter()
        .map(|artifact| artifact.bytes.len() as u64)
        .sum::<u64>();
    assert_eq!(
        limit_error(limited(
            page_pack(&[10, 10]),
            Output::new(OutputFormat::Svg),
            CompilationLimits {
                max_total_artifact_bytes: total - 1,
                ..defaults
            }
        )),
        ("total artifact bytes", total - 1)
    );
}

#[cfg(feature = "parallel")]
#[test]
fn export_workers_do_not_change_results() {
    let with_workers = |export_workers| {
        let compiled = limited(
            five_page_pack(),
            png(72.0),
            CompilationLimits {
                export_workers,
                ..CompilationLimits::default()
            },
        )
        .unwrap();
        (compiled.identity, compiled.artifacts)
    };
    let sequential = with_workers(1);
    for workers in [2, 3, 4] {
        assert_eq!(with_workers(workers), sequential);
    }
}
