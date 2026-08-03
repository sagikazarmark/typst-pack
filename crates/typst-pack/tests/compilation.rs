use proptest::prelude::*;
use typst_pack::{
    CompilationAccessKind, CompilationFulfillmentIssue, CompilationFulfillmentSet,
    CompilationFulfillmentSetIssue, CompilationOperationOutcome, CompilationOutputOrigins,
    CompilationOutputSpecification, CompilationReportOutcome, CompilationRequestIssue,
    CompilationRequestRejection, CompilationResult, CompilationStatus, CreationTimestamp,
    DiagnosticPhase, DiagnosticProducer, DocumentTime, HtmlOutputSpecification, OutputFormat, Pack,
    PackCompilationRequest, PackMetadata, PackOverrideSet, PackOverrideSetError, PackageTree,
    PackageTreeFulfillment, PdfOutputSpecification, PngOutputSpecification, RequestValueOrigin,
    SvgOutputSpecification, compile as compile_to_report,
};
#[cfg(feature = "embedded-fonts")]
use typst_pack::{FontContainer, FontContainerFulfillment};

fn compile(
    request: PackCompilationRequest,
) -> Result<CompilationResult, CompilationRequestRejection> {
    let report = compile_to_report(request)?;
    Ok(report
        .result()
        .expect("expected a semantic Compilation Result")
        .clone())
}

fn output(format: OutputFormat) -> CompilationOutputSpecification {
    match format {
        OutputFormat::Pdf => CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        OutputFormat::Png => CompilationOutputSpecification::Png(PngOutputSpecification::default()),
        OutputFormat::Svg => CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
        OutputFormat::Html => {
            CompilationOutputSpecification::Html(HtmlOutputSpecification::default())
        }
    }
}

fn page_output(
    format: OutputFormat,
    page_selection: typst_pack::PageSelection,
) -> CompilationOutputSpecification {
    match format {
        OutputFormat::Pdf => CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            page_selection,
            ..PdfOutputSpecification::default()
        }),
        OutputFormat::Png => CompilationOutputSpecification::Png(PngOutputSpecification {
            page_selection,
            ..PngOutputSpecification::default()
        }),
        OutputFormat::Svg => CompilationOutputSpecification::Svg(SvgOutputSpecification {
            page_selection,
            ..SvgOutputSpecification::default()
        }),
        OutputFormat::Html => panic!("HTML has no page selection"),
    }
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
fn compilation_fulfillment_set_rejects_duplicate_package_specifications() {
    let spec: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let first = PackageTreeFulfillment::new(
        spec.clone(),
        PackageTree::from_owned_entries([("lib.typ", b"first".to_vec())]).unwrap(),
    );
    let second = PackageTreeFulfillment::new(
        spec.clone(),
        PackageTree::from_owned_entries([("lib.typ", b"second".to_vec())]).unwrap(),
    );

    let error = CompilationFulfillmentSet::new([second, first], []).unwrap_err();

    assert!(matches!(
        error.issues(),
        [CompilationFulfillmentSetIssue::DuplicatePackageSpecification { spec: duplicate }]
            if duplicate == &spec
    ));
}

#[test]
fn package_fulfillment_verification_aggregates_every_exact_set_deviation() {
    let missing: typst::syntax::package::PackageSpec = "@local/a:1.0.0".parse().unwrap();
    let undeclared: typst::syntax::package::PackageSpec = "@local/b:1.0.0".parse().unwrap();
    let embedded: typst::syntax::package::PackageSpec = "@local/c:1.0.0".parse().unwrap();
    let mismatched: typst::syntax::package::PackageSpec = "@local/d:1.0.0".parse().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"unreached".to_vec())
        .unwrap()
        .external_package_file(missing.clone(), "lib.typ", b"missing".to_vec())
        .unwrap()
        .package_file(embedded.clone(), "lib.typ", b"embedded".to_vec())
        .unwrap()
        .external_package_file(mismatched.clone(), "lib.typ", b"expected".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let fulfillments = CompilationFulfillmentSet::new(
        [
            PackageTreeFulfillment::new(
                mismatched.clone(),
                PackageTree::from_owned_entries([("lib.typ", b"actual".to_vec())]).unwrap(),
            ),
            PackageTreeFulfillment::new(
                undeclared.clone(),
                PackageTree::from_owned_entries([("lib.typ", b"undeclared".to_vec())]).unwrap(),
            )
            .provenance("undeclared:package")
            .cache_hit(true),
            PackageTreeFulfillment::new(
                embedded.clone(),
                PackageTree::from_owned_entries([("lib.typ", b"wrong embedded".to_vec())]).unwrap(),
            ),
        ],
        [],
    )
    .unwrap();

    let report = compile_to_report(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg)).fulfillments(fulfillments),
    )
    .unwrap();
    let CompilationReportOutcome::Operation {
        outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
        compilation_identity,
        ..
    } = report.outcome()
    else {
        panic!("expected an invalid Compilation Fulfillment Set outcome");
    };

    assert_ne!(compilation_identity.digest(), [0; 16]);
    assert!(matches!(
        invalid.issues(),
        [
            CompilationFulfillmentIssue::MissingExternalPackage { spec: first },
            CompilationFulfillmentIssue::UndeclaredPackage { spec: second, .. },
            CompilationFulfillmentIssue::UnexpectedEmbeddedPackage { spec: third },
            CompilationFulfillmentIssue::MismatchedPackageTree { spec: fourth, .. },
            CompilationFulfillmentIssue::MismatchedPackageTree { spec: fifth, .. },
        ] if first == &missing
            && second == &undeclared
            && third == &embedded
            && fourth == &embedded
            && fifth == &mismatched
    ));
    assert!(report.result().is_none());
    let undeclared_report = report
        .fulfillments()
        .packages()
        .iter()
        .find(|fulfillment| fulfillment.spec() == &undeclared)
        .unwrap();
    assert!(!undeclared_report.declared());
    assert_eq!(undeclared_report.provenance(), Some("undeclared:package"));
    assert!(undeclared_report.cache_hit());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn compilation_fulfillment_set_rejects_duplicate_font_container_identities() {
    let data = typst_kit::fonts::embedded()
        .next()
        .unwrap()
        .0
        .data()
        .to_vec();
    let container = FontContainer::new(data).unwrap();
    let identity = container.identity();
    let first = FontContainerFulfillment::new(identity, container.clone());
    let second = FontContainerFulfillment::new(identity, container);

    let error = CompilationFulfillmentSet::new([], [second, first]).unwrap_err();

    assert!(matches!(
        error.issues(),
        [CompilationFulfillmentSetIssue::DuplicateFontContainerIdentity { identity: duplicate }]
            if *duplicate == identity
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn font_fulfillment_issues_follow_packages_and_canonical_identity_kind_order() {
    let package: typst::syntax::package::PackageSpec = "@local/a:1.0.0".parse().unwrap();
    let base = typst_kit::fonts::embedded().next().unwrap().0;
    let variant = |tag| {
        let mut data = base.data().to_vec();
        data.push(tag);
        data
    };
    let missing = variant(1);
    let embedded = variant(2);
    let mismatched = variant(3);
    let mismatched_identity = typst_pack::FontContainerIdentity::from_bytes(&mismatched);
    let embedded_actual = FontContainer::new(variant(4)).unwrap();
    let mismatched_actual = FontContainer::new(variant(5)).unwrap();
    let undeclared = FontContainer::new(variant(6)).unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"unreached".to_vec())
        .unwrap()
        .external_package_file(package.clone(), "lib.typ", b"missing".to_vec())
        .unwrap()
        .external_font(missing, base.index())
        .unwrap()
        .font(embedded, base.index())
        .unwrap()
        .external_font(mismatched, base.index())
        .unwrap()
        .build()
        .unwrap();
    let embedded_identity = pack
        .font_requirements()
        .iter()
        .find(|requirement| requirement.is_embedded())
        .unwrap()
        .container_identity();
    let fulfillments = CompilationFulfillmentSet::new(
        [],
        [
            FontContainerFulfillment::new(undeclared.identity(), undeclared)
                .provenance("undeclared:font")
                .licensing("advisory:undeclared"),
            FontContainerFulfillment::new(embedded_identity, embedded_actual),
            FontContainerFulfillment::new(mismatched_identity, mismatched_actual),
        ],
    )
    .unwrap();

    let report = compile_to_report(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg)).fulfillments(fulfillments),
    )
    .unwrap();
    let CompilationReportOutcome::Operation {
        outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
        ..
    } = report.outcome()
    else {
        panic!("expected an invalid Compilation Fulfillment Set outcome");
    };
    let issues = invalid.issues();

    assert!(matches!(
        issues[0],
        CompilationFulfillmentIssue::MissingExternalPackage { ref spec } if spec == &package
    ));
    let font_order = issues[1..]
        .iter()
        .map(|issue| match issue {
            CompilationFulfillmentIssue::MissingExternalFont { identity } => (*identity, 0),
            CompilationFulfillmentIssue::UndeclaredFont { identity, .. } => (*identity, 1),
            CompilationFulfillmentIssue::UnexpectedEmbeddedFont { identity } => (*identity, 2),
            CompilationFulfillmentIssue::MismatchedFontContainer { expected, .. } => (*expected, 3),
            other => panic!("unexpected non-font issue: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert!(font_order.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(
                issue,
                CompilationFulfillmentIssue::MissingExternalFont { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(issue, CompilationFulfillmentIssue::UndeclaredFont { .. }))
            .count(),
        1
    );
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(
                issue,
                CompilationFulfillmentIssue::UnexpectedEmbeddedFont { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(
                issue,
                CompilationFulfillmentIssue::MismatchedFontContainer { .. }
            ))
            .count(),
        2
    );
    let undeclared_report = report
        .fulfillments()
        .fonts()
        .iter()
        .find(|fulfillment| !fulfillment.declared())
        .unwrap();
    assert_eq!(undeclared_report.provenance(), Some("undeclared:font"));
    assert_eq!(undeclared_report.licensing(), Some("advisory:undeclared"));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn missing_declared_font_face_is_reported_before_world_materialization() {
    use std::io::Write;

    let data = typst_kit::fonts::embedded()
        .next()
        .unwrap()
        .0
        .data()
        .to_vec();
    let container = FontContainer::new(data).unwrap();
    let identity = container.identity();
    let digest = identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let missing_index = u32::MAX;
    let manifest = format!(
        "format-version = 1\n[project]\nentrypoint = \"main.typ\"\n\
         [[fonts]]\npath = \"fonts/external.ttf\"\nexternal = true\nindex = {missing_index}\n\
         container-digest = \"{digest}\"\n\
         container-identity-kind = \"font-container\"\n\
         container-identity-schema = \"typst-pack-font-container-identity-v1\"\n\
         container-identity-algorithm = \"typst-hash128-0.15\"\n\
         container-length = {}\n",
        container.data().len()
    );
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("typst-pack.toml", options).unwrap();
    archive.write_all(manifest.as_bytes()).unwrap();
    archive.start_file("project/main.typ", options).unwrap();
    archive.write_all(b"unreached").unwrap();
    let archive = typst_pack::PackArchiveBytes::from(archive.finish().unwrap().into_inner());
    let pack = typst_pack::pack_archive::decode(
        &archive,
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap();
    let fulfillments =
        CompilationFulfillmentSet::new([], [FontContainerFulfillment::new(identity, container)])
            .unwrap();

    let report = compile_to_report(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg)).fulfillments(fulfillments),
    )
    .unwrap();

    assert!(matches!(
        report.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
            ..
        } if matches!(
            invalid.issues(),
            [CompilationFulfillmentIssue::MissingFontFace { identity: issue_identity, index }]
                if *issue_identity == identity && *index == missing_index
        )
    ));
}

proptest! {
    #[test]
    fn equivalent_fulfillment_permutations_have_identical_inventories_and_issue_order(
        order in prop::collection::vec(any::<u8>(), 3),
    ) {
        let external: typst::syntax::package::PackageSpec = "@local/a:1.0.0".parse().unwrap();
        let embedded: typst::syntax::package::PackageSpec = "@local/b:1.0.0".parse().unwrap();
        let undeclared: typst::syntax::package::PackageSpec = "@local/c:1.0.0".parse().unwrap();
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"unreached".to_vec()).unwrap()
            .external_package_file(external.clone(), "lib.typ", b"expected".to_vec()).unwrap()
            .package_file(embedded.clone(), "lib.typ", b"embedded".to_vec()).unwrap()
            .build().unwrap();
        let entries = vec![
            PackageTreeFulfillment::new(
                undeclared,
                PackageTree::from_owned_entries([("lib.typ", b"extra".to_vec())]).unwrap(),
            ),
            PackageTreeFulfillment::new(
                embedded,
                PackageTree::from_owned_entries([("lib.typ", b"embedded".to_vec())]).unwrap(),
            ),
            PackageTreeFulfillment::new(
                external,
                PackageTree::from_owned_entries([("lib.typ", b"wrong".to_vec())]).unwrap(),
            ),
        ];
        let baseline = CompilationFulfillmentSet::new(entries.clone(), []).unwrap();
        let mut permuted = entries.into_iter().enumerate().collect::<Vec<_>>();
        permuted.sort_by_key(|(index, _)| (order[*index], *index));
        let permuted = CompilationFulfillmentSet::new(
            permuted.into_iter().map(|(_, fulfillment)| fulfillment),
            [],
        ).unwrap();
        let inventory = |set: &CompilationFulfillmentSet| {
            set.packages()
                .map(|fulfillment| (
                    fulfillment.spec().to_string(),
                    fulfillment.tree().identity(),
                ))
                .collect::<Vec<_>>()
        };
        prop_assert_eq!(inventory(&baseline), inventory(&permuted));

        let issues = |set| {
            let report = compile_to_report(
                PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg))
                    .fulfillments(set),
            ).unwrap();
            let CompilationReportOutcome::Operation {
                outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
                ..
            } = report.outcome() else {
                panic!("expected invalid fulfillments");
            };
            invalid.issues().to_vec()
        };
        prop_assert_eq!(issues(baseline), issues(permuted));
    }
}

#[cfg(feature = "embedded-fonts")]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn equivalent_font_fulfillment_permutations_have_identical_reports_and_issue_order(
        order in prop::collection::vec(any::<u8>(), 3),
    ) {
        let base = typst_kit::fonts::embedded().next().unwrap().0;
        let entries = (1..=3)
            .map(|tag| {
                let mut data = base.data().to_vec();
                data.push(tag);
                let container = FontContainer::new(data).unwrap();
                FontContainerFulfillment::new(container.identity(), container)
                    .provenance(format!("font:{tag}"))
                    .licensing(format!("license:{tag}"))
            })
            .collect::<Vec<_>>();
        let baseline = CompilationFulfillmentSet::new([], entries.clone()).unwrap();
        let mut permuted = entries.into_iter().enumerate().collect::<Vec<_>>();
        permuted.sort_by_key(|(index, _)| (order[*index], *index));
        let permuted = CompilationFulfillmentSet::new(
            [],
            permuted.into_iter().map(|(_, fulfillment)| fulfillment),
        ).unwrap();
        let inventory = |set: &CompilationFulfillmentSet| {
            set.fonts()
                .map(|fulfillment| (
                    fulfillment.expected_identity(),
                    fulfillment.container().identity(),
                ))
                .collect::<Vec<_>>()
        };
        prop_assert_eq!(inventory(&baseline), inventory(&permuted));

        let pack = Pack::builder("main.typ")
            .file("main.typ", b"unreached".to_vec()).unwrap()
            .build().unwrap();
        let observe = |set| {
            let report = compile_to_report(
                PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg))
                    .fulfillments(set),
            ).unwrap();
            let CompilationReportOutcome::Operation {
                outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
                ..
            } = report.outcome() else {
                panic!("expected invalid fulfillments");
            };
            (report.fulfillments().clone(), invalid.issues().to_vec())
        };
        prop_assert_eq!(observe(baseline), observe(permuted));
    }
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

    let result = compile(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Svg),
    ));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
}

#[test]
fn pack_bound_compilation_does_not_read_an_ambient_clock() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#datetime.today()".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack, output(OutputFormat::Svg)));

    assert_eq!(result.unwrap().status(), CompilationStatus::Rejected);
}

#[test]
fn source_access_trace_retains_exact_bom_prefixed_pack_bytes() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#include \"chapter.typ\"\n#read(\"chapter.typ\")".to_vec(),
        )
        .unwrap()
        .file("chapter.typ", b"\xef\xbb\xbfChapter".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let result = compile(PackCompilationRequest::new(pack, output(OutputFormat::Svg))).unwrap();
    let source = result
        .access_trace()
        .observations()
        .find(|observation| {
            observation.kind() == CompilationAccessKind::Source
                && observation.logical_path() == "project:chapter.typ"
        })
        .expect("chapter source access");
    let file = result
        .access_trace()
        .observations()
        .find(|observation| {
            observation.kind() == CompilationAccessKind::File
                && observation.logical_path() == "project:chapter.typ"
        })
        .expect("chapter file access");

    assert_eq!(source.outcome(), file.outcome());
}

#[test]
fn pack_override_preflight_rejects_paths_outside_the_bound_pack() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"baseline".to_vec())
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
    let baseline_result = compile(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Svg),
    ))
    .unwrap();
    let overrides = PackOverrideSet::new(&pack)
        .replace("main.typ", replacement)
        .unwrap()
        .replace("unused.txt", b"unused replacement".to_vec())
        .unwrap();

    let overridden = compile(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg)).overrides(overrides),
    )
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
    assert_eq!(pack.file("main.typ").unwrap(), baseline);
    assert_eq!(pack.file("unused.txt").unwrap(), b"unused baseline");

    let unused_override = PackOverrideSet::new(&pack)
        .replace("unused.txt", b"another unused value".to_vec())
        .unwrap();
    let unused_result = compile(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg))
            .overrides(unused_override),
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
    let accepted = compile(
        PackCompilationRequest::new(first, output(OutputFormat::Svg)).overrides(overrides.clone()),
    )
    .unwrap();
    let accepted_commitment = accepted
        .request_inventory()
        .overrides()
        .value()
        .iter()
        .next()
        .unwrap()
        .commitment();

    let result = compile(
        PackCompilationRequest::new(second, output(OutputFormat::Svg)).overrides(overrides),
    );

    let Err(rejection) = result else {
        panic!("expected a Pack Override Set binding rejection");
    };
    assert!(matches!(
        rejection.issues(),
        [CompilationRequestIssue::OverrideSetPackMismatch]
    ));
    assert_eq!(
        rejection
            .request_inventory()
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

    let png = compile(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Png),
    ))
    .unwrap();
    let CompilationOutputSpecification::Png(png_output) =
        png.request_inventory().output_specification().value()
    else {
        panic!("expected PNG output specification");
    };
    assert_eq!(png_output.pixels_per_inch, Some(144.0));
    assert_eq!(
        png.request_inventory().output_origins(),
        CompilationOutputOrigins::Png {
            pixels_per_inch: RequestValueOrigin::CoreDefaulted,
        }
    );
    assert!(!png_output.render_bleed);

    let explicit_png = compile(PackCompilationRequest::new(
        pack.clone(),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(144.0),
            ..PngOutputSpecification::default()
        }),
    ))
    .unwrap();
    assert_eq!(
        explicit_png.request_inventory().output_origins(),
        CompilationOutputOrigins::Png {
            pixels_per_inch: RequestValueOrigin::CallerSupplied,
        }
    );

    let pdf = compile(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Pdf),
    ))
    .unwrap();
    let CompilationOutputSpecification::Pdf(pdf_output) =
        pdf.request_inventory().output_specification().value()
    else {
        panic!("expected PDF output specification");
    };
    assert_eq!(pdf_output.tags, typst::foundations::Smart::Custom(true));
    assert_eq!(
        pdf.request_inventory().output_origins(),
        CompilationOutputOrigins::Pdf {
            tags: RequestValueOrigin::CoreDefaulted,
            creation_time: RequestValueOrigin::CoreDefaulted,
        }
    );
    assert!(matches!(
        pdf_output.creation_timestamp,
        CreationTimestamp::Omit
    ));

    let svg = compile(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Svg),
    ))
    .unwrap();
    assert_eq!(
        svg.request_inventory().output_origins(),
        CompilationOutputOrigins::Svg
    );

    let html = compile(PackCompilationRequest::new(
        pack,
        output(OutputFormat::Html),
    ))
    .unwrap();
    assert_eq!(
        html.request_inventory().output_origins(),
        CompilationOutputOrigins::Html
    );
}

#[test]
fn compilation_identity_ignores_pack_metadata() {
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
        output(OutputFormat::Svg),
    ))
    .unwrap();
    let second = compile(PackCompilationRequest::new(
        build("second"),
        output(OutputFormat::Svg),
    ))
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
    let first_ranges = SvgOutputSpecification {
        page_selection: typst_pack::parse_page_selection("1-2").unwrap(),
        ..SvgOutputSpecification::default()
    };
    let second_ranges = SvgOutputSpecification {
        page_selection: typst_pack::parse_page_selection("2,1").unwrap(),
        ..SvgOutputSpecification::default()
    };
    let first = compile(PackCompilationRequest::new(
        pack.clone(),
        CompilationOutputSpecification::Svg(first_ranges),
    ))
    .unwrap();
    let second = compile(PackCompilationRequest::new(
        pack.clone(),
        CompilationOutputSpecification::Svg(second_ranges),
    ))
    .unwrap();
    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());

    let first_standards = PdfOutputSpecification {
        standards: vec![typst_pdf::PdfStandard::A_2b, typst_pdf::PdfStandard::Ua_1],
        ..PdfOutputSpecification::default()
    };
    let second_standards = PdfOutputSpecification {
        standards: vec![typst_pdf::PdfStandard::Ua_1, typst_pdf::PdfStandard::A_2b],
        ..PdfOutputSpecification::default()
    };
    let first = compile(PackCompilationRequest::new(
        pack.clone(),
        CompilationOutputSpecification::Pdf(first_standards),
    ))
    .unwrap();
    let second = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(second_standards),
    ))
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
        PackCompilationRequest::new(pack, output(OutputFormat::Svg))
            .adapter_resolved_inputs(inputs)
            .adapter_resolved_feature(typst::Feature::A11yExtras)
            .adapter_resolved_document_time(DocumentTime::Fixed(
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
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg))
            .adapter_resolved_document_time(DocumentTime::UnixTimestamp(early)),
    )
    .unwrap();
    let second = compile(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg))
            .adapter_resolved_document_time(DocumentTime::UnixTimestamp(late)),
    )
    .unwrap();

    assert_eq!(
        first.request_inventory().document_time().value(),
        &DocumentTime::UnixTimestamp(early)
    );
    assert_eq!(
        first.request_inventory().document_time().origin(),
        RequestValueOrigin::AdapterResolved
    );
    assert_ne!(first.compilation_identity(), second.compilation_identity());
    assert_ne!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());
}

#[test]
fn invalid_document_timestamp_is_rejected_with_its_inventory() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"invalid time".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let request = PackCompilationRequest::new(pack, output(OutputFormat::Svg))
        .document_time(DocumentTime::UnixTimestamp(i64::MAX));

    let rejection = compile(request).unwrap_err();

    assert!(matches!(
        rejection.issues(),
        [CompilationRequestIssue::InvalidDocumentTimestamp]
    ));
    assert_eq!(
        rejection.request_inventory().document_time().value(),
        &DocumentTime::UnixTimestamp(i64::MAX)
    );
    assert_eq!(
        rejection.request_inventory().document_time().origin(),
        RequestValueOrigin::CallerSupplied
    );
}

#[cfg(not(any(
    feature = "_test-package-download-probe",
    feature = "default",
    feature = "diagnostics",
    feature = "embedded-fonts",
    feature = "fs",
    feature = "parallel"
)))]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
#[test]
fn document_time_refactor_preserves_the_existing_compilation_identity() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"identity baseline".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let result = compile(PackCompilationRequest::new(pack, output(OutputFormat::Svg))).unwrap();

    assert_eq!(
        result.compilation_identity().digest(),
        [
            0xa5, 0x94, 0x68, 0x70, 0x1d, 0x20, 0x4c, 0xf9, 0xaf, 0xca, 0x50, 0x39, 0x7b, 0x0a,
            0x0a, 0x67,
        ]
    );
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

    let report = compile_to_report(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Svg),
    ))
    .unwrap();

    assert!(matches!(
        report.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
            ..
        } if matches!(
            invalid.issues(),
            [CompilationFulfillmentIssue::MissingExternalPackage { .. }]
        )
    ));
    let report =
        compile_to_report(PackCompilationRequest::new(pack, output(OutputFormat::Svg))).unwrap();
    assert!(matches!(
        report.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(_),
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

    let mismatched = compile_to_report(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [PackageTreeFulfillment::new(
                    package.clone(),
                    PackageTree::from_owned_entries([
                        ("lib.typ", b"#let value = 7".to_vec()),
                        ("typst.toml", manifest.clone()),
                    ])
                    .unwrap(),
                )],
                [],
            )
            .unwrap(),
        ),
    );
    assert!(matches!(
        mismatched.unwrap().outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
            ..
        } if matches!(
            invalid.issues(),
            [CompilationFulfillmentIssue::MismatchedPackageTree { spec, .. }]
                if spec == &package
        )
    ));

    let baseline = compile(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [PackageTreeFulfillment::new(
                    package.clone(),
                    PackageTree::from_owned_entries([
                        ("lib.typ", source.clone()),
                        ("typst.toml", manifest.clone()),
                    ])
                    .unwrap(),
                )],
                [],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let with_telemetry = compile_to_report(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [PackageTreeFulfillment::new(
                    package,
                    PackageTree::from_owned_entries([
                        ("lib.typ", source),
                        ("typst.toml", manifest),
                    ])
                    .unwrap(),
                )
                .provenance("memory:test")
                .cache_hit(true)],
                [],
            )
            .unwrap(),
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

    let missing = compile_to_report(PackCompilationRequest::new(
        pack.clone(),
        output(OutputFormat::Svg),
    ))
    .unwrap();
    assert!(matches!(
        missing.outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
            ..
        } if matches!(
            invalid.issues(),
            [CompilationFulfillmentIssue::MissingExternalFont { identity }]
                if *identity == requirement.container_identity()
        )
    ));

    let mut wrong = data.clone();
    wrong.push(0);
    let mismatched = compile_to_report(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [],
                [FontContainerFulfillment::new(
                    requirement.container_identity(),
                    FontContainer::new(wrong).unwrap(),
                )],
            )
            .unwrap(),
        ),
    );
    assert!(matches!(
        mismatched.unwrap().outcome(),
        CompilationReportOutcome::Operation {
            outcome: CompilationOperationOutcome::InvalidFulfillmentSet(invalid),
            ..
        } if matches!(
            invalid.issues(),
            [CompilationFulfillmentIssue::MismatchedFontContainer { expected, .. }]
                if *expected == requirement.container_identity()
        )
    ));

    let baseline = compile(
        PackCompilationRequest::new(pack.clone(), output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [],
                [FontContainerFulfillment::new(
                    requirement.container_identity(),
                    FontContainer::new(data.clone()).unwrap(),
                )],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let with_metadata = compile_to_report(
        PackCompilationRequest::new(pack, output(OutputFormat::Svg)).fulfillments(
            CompilationFulfillmentSet::new(
                [],
                [FontContainerFulfillment::new(
                    requirement.container_identity(),
                    FontContainer::new(data).unwrap(),
                )
                .provenance("memory:test")
                .licensing("advisory:test")],
            )
            .unwrap(),
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
    let request = PackCompilationRequest::new(pack, output(OutputFormat::Svg))
        .feature(typst::Feature::Bundle);

    let rejection = compile(request).unwrap_err();
    assert!(matches!(
        rejection.issues(),
        [CompilationRequestIssue::UnsupportedBundleFeature]
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
        let specification = CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(ppi),
            ..PngOutputSpecification::default()
        });
        let rejection =
            compile(PackCompilationRequest::new(pack.clone(), specification)).unwrap_err();
        assert!(matches!(
            rejection.issues(),
            [CompilationRequestIssue::InvalidPpi]
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

    let output = compile(PackCompilationRequest::new(pack, output(OutputFormat::Svg))).unwrap();

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

    let result = compile(PackCompilationRequest::new(pack, output(OutputFormat::Pdf))).unwrap();

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
    let output = compile(PackCompilationRequest::new(pack, output(OutputFormat::Pdf))).unwrap();

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
        let page_selection = typst_pack::parse_page_selection(expression).unwrap();

        for format in [OutputFormat::Png, OutputFormat::Svg] {
            let output = compile(PackCompilationRequest::new(
                pack.clone(),
                page_output(format, page_selection.clone()),
            ))
            .unwrap();
            assert!(output.artifacts().is_empty());
        }
    }
}

#[test]
fn page_format_artifacts_preserve_source_page_identity() {
    let pack = five_page_pack();
    let specification = PngOutputSpecification {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        pixels_per_inch: Some(72.0),
        ..PngOutputSpecification::default()
    };

    let output = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Png(specification),
    ))
    .unwrap();

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
    let page_selection = typst_pack::parse_page_selection("5,2-4,2,3-5,1").unwrap();

    for format in [OutputFormat::Png, OutputFormat::Svg] {
        let output = compile(PackCompilationRequest::new(
            pack.clone(),
            page_output(format, page_selection.clone()),
        ))
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
        let page_selection = expression
            .map(typst_pack::parse_page_selection)
            .transpose()
            .unwrap()
            .unwrap_or_default();
        let output = compile(PackCompilationRequest::new(
            pack.clone(),
            page_output(OutputFormat::Svg, page_selection),
        ))
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
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("5,2").unwrap(),
        ..PdfOutputSpecification::default()
    };

    let output = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

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
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("9-").unwrap(),
        ..PdfOutputSpecification::default()
    };

    let output = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

    assert_eq!(output.artifacts().len(), 1);
    assert!(output.artifacts()[0].bytes().starts_with(b"%PDF"));
}

#[test]
fn pdf_page_selection_warns_that_accessibility_tags_are_disabled() {
    let pack = five_page_pack();
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        ..PdfOutputSpecification::default()
    };

    let output = compile(PackCompilationRequest::new(
        pack.clone(),
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

    assert!(
        output
            .pack_warnings()
            .iter()
            .any(|warning| warning.message().contains("--pages implies --no-pdf-tags"))
    );

    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("2,5").unwrap(),
        tags: typst::foundations::Smart::Custom(false),
        ..PdfOutputSpecification::default()
    };
    let output = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();
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
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        ..PdfOutputSpecification::default()
    };

    let result = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

    assert!(result.diagnostics().is_empty());
    assert_eq!(result.pack_warnings().len(), 1);
    assert!(
        result.pack_warnings()[0]
            .message()
            .contains("--pages implies --no-pdf-tags")
    );
}

#[test]
fn explicit_pdf_tags_with_page_selection_preserve_the_exporter_rejection() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"One page".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        tags: typst::foundations::Smart::Custom(true),
        ..PdfOutputSpecification::default()
    };

    let result = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

    assert_eq!(result.status(), CompilationStatus::Rejected);
    assert!(result.artifacts().is_empty());
    assert!(result.diagnostics().iter().any(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Export
            && diagnostic.producer() == DiagnosticProducer::Exporter(result.exporter_identity())
            && diagnostic.message().contains("tagged PDF")
    }));
}

#[test]
fn tag_required_pdf_standard_without_tags_preserves_the_exporter_rejection() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"One page".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let specification = PdfOutputSpecification {
        standards: vec![typst_pdf::PdfStandard::Ua_1],
        tags: typst::foundations::Smart::Custom(false),
        ..PdfOutputSpecification::default()
    };

    let result = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Pdf(specification),
    ))
    .unwrap();

    assert_eq!(result.status(), CompilationStatus::Rejected);
    assert!(result.artifacts().is_empty());
    assert!(result.diagnostics().iter().any(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Export
            && diagnostic.producer() == DiagnosticProducer::Exporter(result.exporter_identity())
            && diagnostic.message().contains("PDF/UA-1")
    }));
}

#[test]
fn pack_request_rejection_collects_independent_pdf_issues_in_stable_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#panic(\"must not compile\")".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let specification = PdfOutputSpecification {
        page_selection: typst_pack::parse_page_selection("1").unwrap(),
        standards: vec![
            typst_pdf::PdfStandard::V_2_0,
            typst_pdf::PdfStandard::A_1b,
            typst_pdf::PdfStandard::Ua_1,
        ],
        ..PdfOutputSpecification::default()
    };
    let request =
        PackCompilationRequest::new(pack, CompilationOutputSpecification::Pdf(specification))
            .feature(typst::Feature::Bundle);

    let Err(rejection) = compile(request) else {
        panic!("expected a Pack request rejection");
    };

    assert_eq!(rejection.issues().len(), 2);
    assert!(matches!(
        rejection.issues()[0],
        CompilationRequestIssue::UnsupportedBundleFeature
    ));
    assert!(matches!(
        rejection.issues()[1],
        CompilationRequestIssue::InvalidPdfStandards(_)
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
    let output = compile(PackCompilationRequest::new(
        pack,
        output(OutputFormat::Html),
    ))
    .unwrap();

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
fn pretty_affects_html_svg_and_pdf() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    for format in [OutputFormat::Html, OutputFormat::Svg, OutputFormat::Pdf] {
        let pretty_specification = match format {
            OutputFormat::Html => {
                CompilationOutputSpecification::Html(HtmlOutputSpecification { pretty: true })
            }
            OutputFormat::Svg => CompilationOutputSpecification::Svg(SvgOutputSpecification {
                pretty: true,
                ..SvgOutputSpecification::default()
            }),
            OutputFormat::Pdf => CompilationOutputSpecification::Pdf(PdfOutputSpecification {
                pretty: true,
                ..PdfOutputSpecification::default()
            }),
            OutputFormat::Png => unreachable!(),
        };
        let compact = compile(PackCompilationRequest::new(pack.clone(), output(format))).unwrap();
        let pretty = compile(PackCompilationRequest::new(
            pack.clone(),
            pretty_specification,
        ))
        .unwrap();
        assert_ne!(
            compact.artifacts()[0].bytes(),
            pretty.artifacts()[0].bytes()
        );
    }
}

#[test]
fn compilation_result_identity_binds_status_document_trace_and_artifacts() {
    let compile_source = |source: &[u8]| {
        let pack = Pack::builder("main.typ")
            .file("main.typ", source.to_vec())
            .unwrap()
            .build()
            .unwrap();
        compile(PackCompilationRequest::new(pack, output(OutputFormat::Svg))).unwrap()
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
    let specification = SvgOutputSpecification {
        page_selection: typst_pack::parse_page_selection("9").unwrap(),
        ..SvgOutputSpecification::default()
    };

    let output = compile(PackCompilationRequest::new(
        pack,
        CompilationOutputSpecification::Svg(specification),
    ))
    .unwrap();

    assert!(output.artifacts().is_empty());
    assert!(!output.diagnostics().is_empty());
}
