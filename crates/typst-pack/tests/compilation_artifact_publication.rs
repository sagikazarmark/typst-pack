//! Destination-independent Compilation Output Artifact publication planning.

use typst_pack::{
    ArtifactPublicationPlanIssue, CompilationLimits, CompilationOperationOutcome,
    CompilationOutputSpecification, CompilationReportOutcome, CompilationResource,
    CompilationStatus, HtmlOutputSpecification, OutputFormat, Pack, PackCompilationRequest,
    PdfOutputSpecification, PngOutputSpecification, SvgOutputSpecification, compile,
    parse_page_selection, plan_compilation_artifact_publication,
};

fn output(format: OutputFormat) -> CompilationOutputSpecification {
    match format {
        OutputFormat::Pdf => CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        OutputFormat::Html => {
            CompilationOutputSpecification::Html(HtmlOutputSpecification::default())
        }
        _ => panic!("expected a Document Format"),
    }
}

fn page_output(
    format: OutputFormat,
    selection: typst_pack::PageSelection,
) -> CompilationOutputSpecification {
    match format {
        OutputFormat::Png => CompilationOutputSpecification::Png(PngOutputSpecification {
            page_selection: selection,
            pixels_per_inch: Some(72.0),
            ..PngOutputSpecification::default()
        }),
        OutputFormat::Svg => CompilationOutputSpecification::Svg(SvgOutputSpecification {
            page_selection: selection,
            ..SvgOutputSpecification::default()
        }),
        _ => panic!("expected a Page Format"),
    }
}

fn page_pack(page_count: usize) -> Pack {
    let source = (1..=page_count)
        .map(|page| {
            "#set page(width: 10pt, height: 10pt, margin: 0pt)\n\
             #rect(width: 1pt, height: 1pt)\n"
                .to_owned()
                + if page < page_count {
                    "#pagebreak()\n"
                } else {
                    ""
                }
        })
        .collect::<String>();
    Pack::builder("main.typ")
        .file("main.typ", source.into_bytes())
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn document_artifacts_have_deterministic_names_and_shared_owned_payloads() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"= Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();

    for (format, expected_path) in [
        (OutputFormat::Pdf, "output.pdf"),
        (OutputFormat::Html, "output.html"),
    ] {
        let report = compile(
            PackCompilationRequest::new(pack.clone(), output(format)),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let result = report.result().unwrap();
        let result_identity = result.result_identity();
        let artifact_pointer = result.artifacts()[0].bytes().as_ptr();
        let plan = plan_compilation_artifact_publication(result).unwrap();
        let cloned = plan.clone();

        assert_eq!(plan.result_identity(), &result_identity);
        assert_eq!(plan.entries().len(), 1);
        let entry = &plan.entries()[0];
        assert_eq!(entry.relative_path(), expected_path);
        assert_eq!(entry.format(), format);
        assert_eq!(entry.source_page_number(), None);
        assert_eq!(entry.len(), entry.bytes().len() as u64);
        assert_eq!(entry.bytes().as_ptr(), artifact_pointer);
        assert_eq!(cloned.entries()[0].bytes().as_ptr(), artifact_pointer);

        drop(report);
        assert!(!plan.entries()[0].bytes().is_empty());
    }
}

#[test]
fn page_artifact_names_preserve_source_pages_and_use_complete_page_count_padding() {
    let pack = page_pack(10);

    for format in [OutputFormat::Png, OutputFormat::Svg] {
        let report = compile(
            PackCompilationRequest::new(
                pack.clone(),
                page_output(format, parse_page_selection("10,2").unwrap()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let plan = plan_compilation_artifact_publication(report.result().unwrap()).unwrap();

        assert_eq!(
            plan.entries()
                .iter()
                .map(|entry| {
                    (
                        entry.relative_path().to_owned(),
                        entry.format(),
                        entry.source_page_number().unwrap().get(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                (format!("page-02.{}", format.extension()), format, 2),
                (format!("page-10.{}", format.extension()), format, 10),
            ]
        );
    }
}

#[test]
fn empty_page_selection_produces_a_valid_empty_plan() {
    let pack = page_pack(2);

    for format in [OutputFormat::Png, OutputFormat::Svg] {
        let report = compile(
            PackCompilationRequest::new(
                pack.clone(),
                page_output(format, parse_page_selection("9").unwrap()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let result = report.result().unwrap();
        let plan = plan_compilation_artifact_publication(result).unwrap();

        assert_eq!(plan.result_identity(), &result.result_identity());
        assert!(plan.entries().is_empty());
    }
}

#[test]
fn rejected_compilation_result_cannot_produce_a_plan() {
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
    let report = compile(
        PackCompilationRequest::new(pack, output(OutputFormat::Pdf)),
        CompilationLimits::reference_v1(),
    )
    .unwrap();
    let result = report.result().unwrap();

    assert_eq!(result.status(), CompilationStatus::Rejected);
    assert_eq!(
        plan_compilation_artifact_publication(result)
            .unwrap_err()
            .issues(),
        [ArtifactPublicationPlanIssue::RejectedCompilationResult]
    );
}

#[test]
fn compilation_operation_outcome_exposes_no_result_to_plan() {
    let reference = CompilationLimits::reference_v1();
    let limits = CompilationLimits::new(
        1,
        reference.artifacts(),
        reference.pixels_per_artifact(),
        reference.total_pixels(),
        reference.artifact_bytes(),
        reference.retained_artifact_bytes(),
        reference.export_workers(),
    )
    .unwrap();
    let report = compile(
        PackCompilationRequest::new(page_pack(2), output(OutputFormat::Pdf)),
        limits,
    )
    .unwrap();

    assert!(report.result().is_none());
    assert!(matches!(
        report.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::ResourceLimit(
                typst_pack::CompilationLimitError::Exceeded {
                    resource: CompilationResource::SourcePages,
                    ..
                }
            ),
            ..
        }
    ));
}
