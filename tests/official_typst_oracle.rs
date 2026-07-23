mod support;

use std::num::NonZeroUsize;

use support::official_typst::{
    ArtifactRole, DiagnosticObservation, DiagnosticSeverity, Fixture, ObservationStatus,
    OutputRequest, ReferenceRequest, Target, TraceKind, observe, observe_with_project_overrides,
    select_font,
};
use typst::World;
use typst::foundations::{Bytes, Datetime, Dict, Smart, Value};
use typst_pack::{
    CompilationDiagnostic, CompilationRequestRejection, CompilationStatus, CompileOptions,
    CreationTimestamp, DiagnosticPhase, DiagnosticProducer,
    DiagnosticSeverity as PackDiagnosticSeverity, OutputFormat, Pack, PackCompilationRequest,
    PackCompileError, PackOverrideSet, PackageTreeFulfillment, RequestValueOrigin, TracepointKind,
    compile_pack, parse_page_selection,
};
use typst_pdf::PdfStandard;

fn stabilized_pack(fixture: &Fixture) -> Pack {
    let mut builder = Pack::builder(fixture.entrypoint());
    for &(path, text) in fixture.project() {
        builder = builder.file(path, text.as_bytes().to_vec()).unwrap();
    }
    for &(spec, path, text) in fixture.packages() {
        builder = builder
            .package_file(spec.parse().unwrap(), path, text.as_bytes().to_vec())
            .unwrap();
    }
    for (data, index) in fixture.fonts() {
        builder = builder.font(data.clone(), *index).unwrap();
    }
    Pack::from_bytes(builder.build().unwrap().to_bytes().unwrap()).unwrap()
}

#[test]
fn embedded_and_external_complete_package_trees_match_the_independent_oracle() {
    let fixture = Fixture::official_oracle();
    let reference_request = ReferenceRequest {
        inputs: string_inputs([("width", "24")]),
        features: vec![],
        document_time: Some(Datetime::from_ymd(2024, 2, 3).unwrap()),
        output: OutputRequest::Svg {
            source_pages: vec![],
            render_bleed: false,
            pretty: false,
        },
    };
    let expected = observe(&fixture, &reference_request);
    let embedded = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Svg)
            .inputs(string_inputs([("width", "24")]))
            .document_time(Datetime::from_ymd(2024, 2, 3).unwrap()),
    )
    .unwrap();

    let mut builder = Pack::builder(fixture.entrypoint());
    for &(path, text) in fixture.project() {
        builder = builder.file(path, text.as_bytes().to_vec()).unwrap();
    }
    let spec: typst::syntax::package::PackageSpec = fixture.packages()[0].0.parse().unwrap();
    for &(_, path, text) in fixture.packages() {
        builder = builder
            .external_package_file(spec.clone(), path, text.as_bytes().to_vec())
            .unwrap();
    }
    let external_pack = builder.build().unwrap();
    let external = compile_pack(
        PackCompilationRequest::new(external_pack, OutputFormat::Svg)
            .inputs(string_inputs([("width", "24")]))
            .document_time(Datetime::from_ymd(2024, 2, 3).unwrap())
            .package_fulfillment(
                spec,
                PackageTreeFulfillment::new(
                    fixture
                        .packages()
                        .iter()
                        .map(|&(_, path, text)| (path, text.as_bytes().to_vec())),
                ),
            ),
    )
    .unwrap();

    assert_eq!(embedded.status(), CompilationStatus::Succeeded);
    assert_eq!(external.status(), embedded.status());
    assert_diagnostics_match(embedded.diagnostics(), &expected.diagnostics);
    assert_eq!(external.diagnostics(), embedded.diagnostics());
    assert_eq!(external.artifacts().len(), expected.artifacts.len());
    assert!(
        external
            .artifacts()
            .iter()
            .zip(embedded.artifacts())
            .all(|(external, embedded)| external.bytes() == embedded.bytes())
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn exact_pack_font_catalog_matches_the_independent_oracle_and_external_fulfillment() {
    let font = typst_kit::fonts::embedded()
        .find(|(_, info)| info.family == "Libertinus Serif")
        .unwrap()
        .0;
    let data = font.data().to_vec();
    let index = font.index();
    let fixture = Fixture::font_selection().font(data.clone(), index);
    let request = ReferenceRequest {
        inputs: Dict::new(),
        features: vec![],
        document_time: None,
        output: OutputRequest::Svg {
            source_pages: vec![],
            render_bleed: false,
            pretty: false,
        },
    };
    let expected = observe(&fixture, &request);
    assert_eq!(expected.font_catalog.len(), 1);
    assert_eq!(expected.font_catalog[0].1, data);
    assert_eq!(expected.font_catalog[0].2, index);

    let embedded_pack = stabilized_pack(&fixture);
    assert_eq!(embedded_pack.fonts()[0].info(), &expected.font_catalog[0].0);
    assert_eq!(
        embedded_pack.fonts()[0].data().as_slice(),
        expected.font_catalog[0].1
    );
    let embedded = compile_pack(PackCompilationRequest::new(
        embedded_pack,
        OutputFormat::Svg,
    ))
    .unwrap();
    assert_eq!(embedded.artifacts()[0].bytes(), expected.artifacts[0].bytes);

    let external_pack = Pack::builder("main.typ")
        .file("main.typ", fixture.project()[0].1.as_bytes().to_vec())
        .unwrap()
        .external_font(data.clone(), index)
        .unwrap()
        .build()
        .unwrap();
    let identity = external_pack.font_requirements()[0].container_identity();
    let external = compile_pack(
        PackCompilationRequest::new(external_pack, OutputFormat::Svg)
            .font_fulfillment(identity, typst_pack::FontContainerFulfillment::new(data)),
    )
    .unwrap();
    assert_eq!(external.artifacts()[0].bytes(), expected.artifacts[0].bytes);
    assert_eq!(external.diagnostics().len(), expected.diagnostics.len());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn catalog_order_drives_the_same_official_font_selection_on_every_path() {
    let base = typst_kit::fonts::embedded()
        .find(|(_, info)| info.family == "Libertinus Serif")
        .unwrap()
        .0;
    let mut first = base.data().to_vec();
    first.push(1);
    let mut second = base.data().to_vec();
    second.push(2);
    let parsed = typst::text::Font::new(Bytes::new(first.clone()), base.index()).unwrap();
    let family = parsed.info().family.to_lowercase();
    let variant = parsed.info().variant;

    for (expected, ordered) in [
        (first.as_slice(), [first.clone(), second.clone()]),
        (second.as_slice(), [second.clone(), first.clone()]),
    ] {
        let fixture = Fixture::font_selection()
            .font(ordered[0].clone(), base.index())
            .font(ordered[1].clone(), base.index());
        assert_eq!(select_font(&fixture, &family, variant).unwrap().0, expected);

        let embedded_pack = stabilized_pack(&fixture);
        let embedded_world = typst_pack::PackWorld::builder(embedded_pack)
            .build()
            .unwrap();
        let selected = embedded_world.book().select(&family, variant).unwrap();
        assert_eq!(
            embedded_world.font(selected).unwrap().data().as_slice(),
            expected
        );

        let external_pack = Pack::builder("main.typ")
            .file("main.typ", fixture.project()[0].1.as_bytes().to_vec())
            .unwrap()
            .external_font(ordered[0].clone(), base.index())
            .unwrap()
            .external_font(ordered[1].clone(), base.index())
            .unwrap()
            .build()
            .unwrap();
        let supplied = ordered.iter().map(|data| {
            let font = typst::text::Font::new(Bytes::new(data.clone()), base.index()).unwrap();
            let info = font.info().clone();
            (font, info)
        });
        let external_world = typst_pack::PackWorld::builder(external_pack)
            .extra_fonts(supplied)
            .build()
            .unwrap();
        let selected = external_world.book().select(&family, variant).unwrap();
        assert_eq!(
            external_world.font(selected).unwrap().data().as_slice(),
            expected
        );
    }
}

fn string_inputs<const N: usize>(entries: [(&str, &str); N]) -> Dict {
    entries
        .into_iter()
        .map(|(key, value)| (key.into(), Value::Str(value.into())))
        .collect()
}

fn assert_diagnostics_match(actual: &[CompilationDiagnostic], expected: &[DiagnosticObservation]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            actual.severity(),
            match expected.severity {
                DiagnosticSeverity::Error => PackDiagnosticSeverity::Error,
                DiagnosticSeverity::Warning => PackDiagnosticSeverity::Warning,
            }
        );
        assert_eq!(actual.message(), expected.message);
        assert_eq!(
            actual.span().logical_path(),
            expected.span.logical_path.as_deref()
        );
        assert_eq!(
            actual.span().byte_range(),
            expected.span.byte_range.as_ref()
        );
        assert_eq!(actual.hints().len(), expected.hints.len());
        for (actual, expected) in actual.hints().iter().zip(&expected.hints) {
            assert_eq!(actual.message(), expected.message);
            assert_eq!(
                actual.span().logical_path(),
                expected.span.logical_path.as_deref()
            );
            assert_eq!(
                actual.span().byte_range(),
                expected.span.byte_range.as_ref()
            );
        }
        assert_eq!(actual.trace().len(), expected.trace.len());
        for (actual, expected) in actual.trace().iter().zip(&expected.trace) {
            assert_eq!(
                actual.kind(),
                match expected.kind {
                    TraceKind::Call => TracepointKind::Call,
                    TraceKind::Show => TracepointKind::Show,
                    TraceKind::Import => TracepointKind::Import,
                    TraceKind::Include => TracepointKind::Include,
                }
            );
            assert_eq!(actual.value(), expected.value.as_deref());
            assert_eq!(
                actual.span().logical_path(),
                expected.span.logical_path.as_deref()
            );
            assert_eq!(
                actual.span().byte_range(),
                expected.span.byte_range.as_ref()
            );
        }
        assert_eq!(actual.source_page_number(), None);
    }
}

#[test]
fn frozen_fixture_and_request_produce_a_stable_official_observation() {
    let fixture = Fixture::official_oracle();
    let request = ReferenceRequest {
        inputs: string_inputs([("width", "24")]),
        features: vec![],
        document_time: Some(Datetime::from_ymd(2024, 2, 3).unwrap()),
        output: OutputRequest::Svg {
            source_pages: vec![NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(1).unwrap()],
            render_bleed: false,
            pretty: true,
        },
    };

    let first = observe(&fixture, &request);
    let second = observe(&fixture, &request);

    assert_eq!(first, second);
    assert_eq!(first.status, ObservationStatus::Accepted, "{first:#?}");
    assert_eq!(first.target, Target::Paged);
    assert_eq!(first.source_page_count, Some(2));
    assert_eq!(first.diagnostics.len(), 1);
    assert_eq!(first.diagnostics[0].severity.as_str(), "warning");
    assert_eq!(
        first.diagnostics[0].message,
        "creating a decimal using imprecise float literal"
    );
    assert_eq!(first.diagnostics[0].hints.len(), 1);
    assert_eq!(
        first.diagnostics[0].hints[0].message,
        "use a string in the decimal constructor to avoid loss of precision: `decimal(\"1.1\")`"
    );
    assert_eq!(first.diagnostics[0].hints[0].span.logical_path, None);
    assert_eq!(first.diagnostics[0].hints[0].span.byte_range, None);
    assert_eq!(
        first.diagnostics[0].span.logical_path.as_deref(),
        Some("project:main.typ")
    );
    assert!(first.diagnostics[0].span.byte_range.is_some());
    assert_eq!(
        first
            .artifacts
            .iter()
            .map(|artifact| artifact.role)
            .collect::<Vec<_>>(),
        [
            ArtifactRole::Svg {
                source_page_number: NonZeroUsize::new(1).unwrap(),
            },
            ArtifactRole::Svg {
                source_page_number: NonZeroUsize::new(2).unwrap(),
            },
        ]
    );
    assert!(first.artifacts.iter().all(|artifact| {
        artifact.bytes.starts_with(b"<svg") && artifact.bytes.ends_with(b"</svg>\n")
    }));
}

#[test]
fn stabilized_project_round_trips_and_matches_pack_svg_compilation() {
    let fixture = Fixture::official_oracle();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let created = stabilized_pack(&fixture);
    let pack = Pack::from_bytes(created.to_bytes().unwrap()).unwrap();
    let options = CompileOptions {
        page_selection: parse_page_selection("2,1").unwrap(),
        pretty: true,
        ..CompileOptions::default()
    };
    let request = PackCompilationRequest::new(pack, OutputFormat::Svg)
        .inputs(inputs)
        .document_time(Datetime::from_ymd(2024, 2, 3).unwrap())
        .options(options);

    let actual = compile_pack(request).unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: string_inputs([("width", "24")]),
            features: vec![],
            document_time: Some(Datetime::from_ymd(2024, 2, 3).unwrap()),
            output: OutputRequest::Svg {
                source_pages: vec![NonZeroUsize::new(2).unwrap(), NonZeroUsize::new(1).unwrap()],
                render_bleed: false,
                pretty: true,
            },
        },
    );

    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert!(actual.diagnostics().iter().all(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Compilation
            && diagnostic.producer() == DiagnosticProducer::Engine(actual.engine_identity())
    }));
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    for (actual, expected) in actual.artifacts().iter().zip(&expected.artifacts) {
        let ArtifactRole::Svg { source_page_number } = expected.role else {
            panic!("oracle produced a non-SVG artifact");
        };
        assert_eq!(actual.format(), OutputFormat::Svg);
        assert_eq!(actual.source_page_number(), Some(source_page_number));
        assert_eq!(actual.bytes(), expected.bytes);
    }
    assert_eq!(actual.engine_identity().implementation(), "typst");
    assert_eq!(actual.engine_identity().version(), "0.15.0");
    assert!(!actual.engine_identity().source_checksum().is_empty());
    assert!(!actual.engine_identity().target().is_empty());
    assert_eq!(actual.exporter_identity().implementation(), "typst-svg");
    assert_eq!(actual.exporter_identity().version(), "0.15.0");
    assert!(!actual.exporter_identity().source_checksum().is_empty());
    assert!(!actual.exporter_identity().target().is_empty());
}

#[test]
fn pack_source_and_non_source_overrides_match_the_independent_oracle() {
    let fixture = Fixture::override_behavior();
    let reference_request = ReferenceRequest {
        inputs: Dict::new(),
        features: vec![],
        document_time: None,
        output: OutputRequest::Svg {
            source_pages: vec![],
            render_bleed: false,
            pretty: false,
        },
    };
    let baseline = observe(&fixture, &reference_request);
    let source_only = observe_with_project_overrides(
        &fixture,
        &reference_request,
        &[("chapter.typ", "#let source-width = 20")],
    );
    let data_only =
        observe_with_project_overrides(&fixture, &reference_request, &[("data.txt", "20")]);
    let unused_only = observe_with_project_overrides(
        &fixture,
        &reference_request,
        &[("unused.txt", "replacement unused")],
    );
    assert_ne!(source_only.artifacts[0].bytes, baseline.artifacts[0].bytes);
    assert_ne!(data_only.artifacts[0].bytes, baseline.artifacts[0].bytes);
    assert_eq!(unused_only.artifacts[0].bytes, baseline.artifacts[0].bytes);

    let pack = stabilized_pack(&fixture);
    let overrides = PackOverrideSet::new(&pack)
        .replace("chapter.typ", b"#let source-width = 20".to_vec())
        .unwrap()
        .replace("data.txt", b"20".to_vec())
        .unwrap()
        .replace("unused.txt", b"replacement unused".to_vec())
        .unwrap();
    let actual =
        compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg).overrides(overrides))
            .unwrap();
    let expected = observe_with_project_overrides(
        &fixture,
        &reference_request,
        &[
            ("chapter.typ", "#let source-width = 20"),
            ("data.txt", "20"),
            ("unused.txt", "replacement unused"),
        ],
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
}

#[test]
fn shared_semantic_values_drive_the_same_official_source_behavior() {
    let fixture = Fixture::semantic_request();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Int(24));
    let pack = stabilized_pack(&fixture);

    let actual = compile_pack(
        PackCompilationRequest::new(pack, OutputFormat::Svg)
            .inputs(inputs.clone())
            .feature(typst::Feature::A11yExtras)
            .document_time(document_time),
    )
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs,
            features: vec![typst::Feature::A11yExtras],
            document_time: Some(document_time),
            output: OutputRequest::Svg {
                source_pages: vec![],
                render_bleed: false,
                pretty: false,
            },
        },
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
    let inventory = actual.request_inventory();
    assert_eq!(
        inventory.inputs().origin(),
        RequestValueOrigin::CallerSupplied
    );
    assert_eq!(inventory.inputs().value().entry_count(), 1);
    assert_eq!(inventory.inputs().value().total_key_bytes(), 5);
    assert!(inventory.inputs().value().total_value_repr_bytes() > 0);
    assert_ne!(inventory.inputs().value().commitment(), [0; 16]);
    assert_eq!(
        inventory.document_time().origin(),
        RequestValueOrigin::CallerSupplied
    );
    assert_eq!(inventory.document_time().value(), &Some(document_time));
    assert_eq!(inventory.features().len(), 1);
    assert_eq!(inventory.features()[0].value(), typst::Feature::A11yExtras);
    assert_eq!(
        inventory.features()[0].origin(),
        RequestValueOrigin::CallerSupplied
    );
}

#[test]
fn effective_defaults_and_required_features_keep_their_origins() {
    let fixture = Fixture::exporter_rejection();
    let pack = stabilized_pack(&fixture);
    let result = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Html)).unwrap();
    let inventory = result.request_inventory();

    assert_eq!(
        inventory.inputs().origin(),
        RequestValueOrigin::CoreDefaulted
    );
    assert_eq!(inventory.inputs().value().entry_count(), 0);
    assert_eq!(
        inventory.document_time().origin(),
        RequestValueOrigin::CoreDefaulted
    );
    assert_eq!(inventory.document_time().value(), &None);
    assert_eq!(inventory.features().len(), 1);
    assert_eq!(inventory.features()[0].value(), typst::Feature::Html);
    assert_eq!(
        inventory.features()[0].origin(),
        RequestValueOrigin::CoreDerived
    );
    assert!(inventory.selected_features().is_empty());

    let explicit = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Html)
            .feature(typst::Feature::Html),
    )
    .unwrap();
    assert_eq!(explicit.request_inventory().selected_features().len(), 1);
    assert_eq!(
        explicit.request_inventory().selected_features()[0].origin(),
        RequestValueOrigin::CallerSupplied
    );
    assert_eq!(
        explicit.request_inventory().features()[0].origin(),
        RequestValueOrigin::CoreDerived
    );

    let rejected = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Html)
            .feature(typst::Feature::Bundle),
    );
    let Err(PackCompileError::RequestRejected {
        rejection,
        request_inventory,
    }) = rejected
    else {
        panic!("the unsupported Bundle feature must be rejected by Pack preparation");
    };
    assert_eq!(*request_inventory.format().value(), OutputFormat::Html);
    assert_eq!(rejection.issues().len(), 1);
    assert!(matches!(
        rejection.issues()[0],
        CompilationRequestRejection::UnsupportedBundleFeature
    ));
}

#[test]
fn unobserved_shared_values_still_change_compilation_identity() {
    let fixture = Fixture::exporter_rejection();
    let first_pack = stabilized_pack(&fixture);
    let second_pack = stabilized_pack(&fixture);
    let feature_pack = stabilized_pack(&fixture);
    let time_pack = stabilized_pack(&fixture);
    let mut first_inputs = Dict::new();
    first_inputs.insert("unused".into(), Value::Str("first".into()));
    let mut second_inputs = Dict::new();
    second_inputs.insert("unused".into(), Value::Str("second".into()));

    let first = compile_pack(
        PackCompilationRequest::new(first_pack, OutputFormat::Svg).inputs(first_inputs),
    )
    .unwrap();
    let second = compile_pack(
        PackCompilationRequest::new(second_pack, OutputFormat::Svg).inputs(second_inputs),
    )
    .unwrap();
    let feature = compile_pack(
        PackCompilationRequest::new(feature_pack, OutputFormat::Svg)
            .feature(typst::Feature::A11yExtras),
    )
    .unwrap();
    let document_time = compile_pack(
        PackCompilationRequest::new(time_pack, OutputFormat::Svg)
            .document_time(Datetime::from_ymd(2024, 2, 3).unwrap()),
    )
    .unwrap();

    assert_ne!(first.compilation_identity(), second.compilation_identity());
    assert_ne!(first.compilation_identity(), feature.compilation_identity());
    assert_ne!(
        first.compilation_identity(),
        document_time.compilation_identity()
    );
    assert_eq!(first.artifacts()[0].bytes(), second.artifacts()[0].bytes());
    assert_eq!(first.artifacts()[0].bytes(), feature.artifacts()[0].bytes());
    assert_eq!(
        first.artifacts()[0].bytes(),
        document_time.artifacts()[0].bytes()
    );
}

#[test]
fn pack_rejection_matches_official_diagnostics_and_remains_a_result() {
    let fixture = Fixture::official_oracle();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let pack = stabilized_pack(&fixture);

    let actual = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Svg).inputs(inputs))
        .expect("the accepted Pack request must produce a Compilation Result");
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: string_inputs([("width", "24")]),
            features: vec![],
            document_time: None,
            output: OutputRequest::Svg {
                source_pages: vec![],
                render_bleed: false,
                pretty: false,
            },
        },
    );

    assert_eq!(expected.status, ObservationStatus::Rejected);
    assert_eq!(actual.status(), CompilationStatus::Rejected);
    assert!(actual.artifacts().is_empty());
    assert_eq!(actual.source_page_count(), None);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert!(actual.diagnostics().iter().all(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Compilation
            && diagnostic.producer() == DiagnosticProducer::Engine(actual.engine_identity())
    }));
}

#[test]
fn pack_exporter_rejection_matches_official_diagnostics() {
    let fixture = Fixture::exporter_rejection();
    let pack = stabilized_pack(&fixture);

    let actual = compile_pack(PackCompilationRequest::new(pack, OutputFormat::Pdf)).unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![],
            document_time: None,
            output: OutputRequest::Pdf {
                source_pages: vec![],
                ident: Smart::Auto,
                creator: Smart::Auto,
                creation_time: None,
                standards: vec![],
                tagged: true,
                pretty: false,
            },
        },
    );

    assert_eq!(expected.status, ObservationStatus::Rejected);
    assert_eq!(actual.status(), CompilationStatus::Rejected);
    assert!(actual.artifacts().is_empty());
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert!(actual.diagnostics().iter().all(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Export
            && diagnostic.producer() == DiagnosticProducer::Exporter(actual.exporter_identity())
    }));
}

#[test]
fn oracle_is_structurally_independent_of_the_production_crate() {
    let oracle_sources = concat!(
        include_str!("support/mod.rs"),
        include_str!("support/official_typst.rs"),
    );
    let production_crate = ["typst", "pack"].join("_");

    assert!(!oracle_sources.contains(&production_crate));
}

#[test]
fn explicit_pdf_controls_produce_stable_official_artifact_bytes() {
    let fixture = Fixture::official_oracle();
    let request = ReferenceRequest {
        inputs: string_inputs([("width", "24")]),
        features: vec![],
        document_time: Some(Datetime::from_ymd(2024, 2, 3).unwrap()),
        output: OutputRequest::Pdf {
            source_pages: vec![NonZeroUsize::new(2).unwrap()],
            ident: Smart::Custom("official-oracle".to_owned()),
            creator: Smart::Custom(Some("typst-pack differential oracle".to_owned())),
            creation_time: Some(typst_pdf::Timestamp::new_utc(
                Datetime::from_ymd_hms(2024, 2, 3, 4, 5, 6).unwrap(),
            )),
            standards: vec![],
            tagged: false,
            pretty: true,
        },
    };

    let first = observe(&fixture, &request);
    let second = observe(&fixture, &request);

    assert_eq!(first, second);
    assert_eq!(first.status, ObservationStatus::Accepted);
    assert_eq!(first.source_page_count, Some(2));
    assert_eq!(first.artifacts.len(), 1);
    assert_eq!(first.artifacts[0].role, ArtifactRole::Pdf);
    assert!(first.artifacts[0].bytes.starts_with(b"%PDF"));
}

#[test]
fn stabilized_pack_matches_official_pdf_export_with_explicit_controls() {
    let fixture = Fixture::official_oracle();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let creation_time =
        typst_pdf::Timestamp::new_utc(Datetime::from_ymd_hms(2024, 2, 3, 4, 5, 6).unwrap());
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let pack = stabilized_pack(&fixture);
    let options = CompileOptions {
        page_selection: parse_page_selection("2").unwrap(),
        pretty: true,
        pdf_identifier: Smart::Custom("official-oracle".to_owned()),
        pdf_creator: Smart::Custom(Some("typst-pack differential oracle".to_owned())),
        pdf_tags: Smart::Custom(false),
        pdf_standards: vec![PdfStandard::A_2b],
        creation_timestamp: CreationTimestamp::Explicit(creation_time),
        ..CompileOptions::default()
    };

    let actual = compile_pack(
        PackCompilationRequest::new(pack, OutputFormat::Pdf)
            .inputs(inputs)
            .document_time(document_time)
            .options(options),
    )
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: string_inputs([("width", "24")]),
            features: vec![],
            document_time: Some(document_time),
            output: OutputRequest::Pdf {
                source_pages: vec![NonZeroUsize::new(2).unwrap()],
                ident: Smart::Custom("official-oracle".to_owned()),
                creator: Smart::Custom(Some("typst-pack differential oracle".to_owned())),
                creation_time: Some(creation_time),
                standards: vec![PdfStandard::A_2b],
                tagged: false,
                pretty: true,
            },
        },
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert!(actual.pack_warnings().is_empty());
    assert_eq!(actual.artifacts().len(), 1);
    assert_eq!(expected.artifacts.len(), 1);
    assert_eq!(expected.artifacts[0].role, ArtifactRole::Pdf);
    assert_eq!(actual.artifacts()[0].format(), OutputFormat::Pdf);
    assert_eq!(actual.artifacts()[0].source_page_number(), None);
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
    let mut destination_bytes = actual.artifacts()[0].bytes().to_vec();
    destination_bytes.clear();
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
    assert_eq!(actual.exporter_identity().implementation(), "typst-pdf");
    assert_eq!(actual.exporter_identity().version(), "0.15.0");
}

#[test]
fn stabilized_pack_matches_official_pdf_exporter_defaults() {
    let fixture = Fixture::official_oracle();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let pack = stabilized_pack(&fixture);

    let actual = compile_pack(
        PackCompilationRequest::new(pack, OutputFormat::Pdf)
            .inputs(inputs)
            .document_time(document_time),
    )
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: string_inputs([("width", "24")]),
            features: vec![],
            document_time: Some(document_time),
            output: OutputRequest::Pdf {
                source_pages: vec![],
                ident: Smart::Auto,
                creator: Smart::Auto,
                creation_time: None,
                standards: vec![],
                tagged: true,
                pretty: false,
            },
        },
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), 1);
    assert_eq!(actual.artifacts()[0].source_page_number(), None);
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
}

#[test]
fn stabilized_pack_matches_official_html_artifacts_and_pretty_control() {
    let fixture = Fixture::html_success();
    assert_eq!(
        CompileOptions::default().pretty,
        typst_html::HtmlOptions::default().pretty
    );
    let compact = compile_pack(PackCompilationRequest::new(
        stabilized_pack(&fixture),
        OutputFormat::Html,
    ))
    .unwrap();
    let pretty_result = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Html).options(
            CompileOptions {
                pretty: true,
                ..CompileOptions::default()
            },
        ),
    )
    .unwrap();
    let expected_compact = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![typst::Feature::Html],
            document_time: None,
            output: OutputRequest::Html { pretty: false },
        },
    );
    let expected_pretty = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![typst::Feature::Html],
            document_time: None,
            output: OutputRequest::Html { pretty: true },
        },
    );

    for (actual, expected, pretty_enabled) in [
        (&compact, &expected_compact, false),
        (&pretty_result, &expected_pretty, true),
    ] {
        assert_eq!(expected.status, ObservationStatus::Accepted);
        assert_eq!(expected.target, Target::Html);
        assert_eq!(actual.status(), CompilationStatus::Succeeded);
        assert_eq!(actual.source_page_count(), None);
        assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
        assert_eq!(actual.artifacts().len(), 1);
        assert_eq!(expected.artifacts.len(), 1);
        assert_eq!(expected.artifacts[0].role, ArtifactRole::Html);
        assert_eq!(actual.artifacts()[0].format(), OutputFormat::Html);
        assert_eq!(actual.artifacts()[0].source_page_number(), None);
        assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
        assert_eq!(
            actual.request_inventory().options().value().pretty,
            pretty_enabled
        );
        assert_eq!(actual.request_inventory().features().len(), 1);
        assert_eq!(
            actual.request_inventory().features()[0].value(),
            typst::Feature::Html
        );
        assert_eq!(
            actual.request_inventory().features()[0].origin(),
            RequestValueOrigin::CoreDerived
        );
        assert!(actual.request_inventory().selected_features().is_empty());
        assert!(actual.diagnostics().iter().all(|diagnostic| {
            diagnostic.phase() == DiagnosticPhase::Compilation
                && diagnostic.producer() == DiagnosticProducer::Engine(actual.engine_identity())
        }));
        assert_eq!(actual.exporter_identity().implementation(), "typst-html");
        assert_eq!(actual.exporter_identity().version(), "0.15.0");
    }
    assert_ne!(
        compact.artifacts()[0].bytes(),
        pretty_result.artifacts()[0].bytes()
    );
    assert_ne!(
        compact.compilation_identity(),
        pretty_result.compilation_identity()
    );
}

#[test]
fn official_html_target_rejects_a_missing_required_feature() {
    let fixture = Fixture::html_success();
    let observation = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![],
            document_time: None,
            output: OutputRequest::Html { pretty: false },
        },
    );

    assert_eq!(observation.status, ObservationStatus::Rejected);
    assert_eq!(observation.target, Target::Html);
    assert_eq!(observation.source_page_count, None);
    assert!(observation.artifacts.is_empty());
    let (error, warnings) = observation.diagnostics.split_last().unwrap();
    assert!(
        warnings
            .iter()
            .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
    );
    assert_eq!(error.severity, DiagnosticSeverity::Error);
}

#[test]
fn html_compiler_rejection_matches_official_diagnostics_and_order() {
    let fixture = Fixture::html_compiler_rejection();
    let actual = compile_pack(PackCompilationRequest::new(
        stabilized_pack(&fixture),
        OutputFormat::Html,
    ))
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![typst::Feature::Html],
            document_time: None,
            output: OutputRequest::Html { pretty: false },
        },
    );

    assert_eq!(expected.status, ObservationStatus::Rejected);
    assert_eq!(expected.target, Target::Html);
    assert_eq!(actual.status(), CompilationStatus::Rejected);
    assert_eq!(actual.source_page_count(), None);
    assert!(actual.artifacts().is_empty());
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    let (error, warnings) = actual.diagnostics().split_last().unwrap();
    assert!(!warnings.is_empty());
    assert!(
        warnings
            .iter()
            .all(|diagnostic| diagnostic.severity() == PackDiagnosticSeverity::Warning)
    );
    assert_eq!(error.severity(), PackDiagnosticSeverity::Error);
    assert!(actual.diagnostics().iter().all(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Compilation
            && diagnostic.producer() == DiagnosticProducer::Engine(actual.engine_identity())
    }));
}

#[test]
fn html_exporter_rejection_matches_official_diagnostics_and_order() {
    let fixture = Fixture::html_exporter_rejection();
    let actual = compile_pack(PackCompilationRequest::new(
        stabilized_pack(&fixture),
        OutputFormat::Html,
    ))
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![typst::Feature::Html],
            document_time: None,
            output: OutputRequest::Html { pretty: false },
        },
    );

    assert_eq!(expected.status, ObservationStatus::Rejected);
    assert_eq!(expected.target, Target::Html);
    assert_eq!(actual.status(), CompilationStatus::Rejected);
    assert_eq!(actual.source_page_count(), None);
    assert!(actual.artifacts().is_empty());
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    let (error, warnings) = actual.diagnostics().split_last().unwrap();
    assert!(!warnings.is_empty());
    assert!(warnings.iter().all(|warning| {
        warning.severity() == PackDiagnosticSeverity::Warning
            && warning.phase() == DiagnosticPhase::Compilation
            && warning.producer() == DiagnosticProducer::Engine(actual.engine_identity())
    }));
    assert_eq!(error.severity(), PackDiagnosticSeverity::Error);
    assert_eq!(error.phase(), DiagnosticPhase::Export);
    assert_eq!(
        error.producer(),
        DiagnosticProducer::Exporter(actual.exporter_identity())
    );
    assert_eq!(error.hints().len(), 1);
}

#[test]
fn pack_png_defaults_match_the_pinned_official_exporter() {
    let fixture = Fixture::static_shape();
    let actual = compile_pack(PackCompilationRequest::new(
        stabilized_pack(&fixture),
        OutputFormat::Png,
    ))
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![],
            document_time: None,
            output: OutputRequest::Png {
                source_pages: vec![],
                pixels_per_inch: 144.0,
                render_bleed: false,
            },
        },
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    assert_eq!(expected.artifacts.len(), 1);
    assert_eq!(
        expected.artifacts[0].role,
        ArtifactRole::Png {
            source_page_number: NonZeroUsize::new(1).unwrap(),
        }
    );
    assert_eq!(actual.artifacts()[0].format(), OutputFormat::Png);
    assert_eq!(
        actual.artifacts()[0].source_page_number(),
        NonZeroUsize::new(1)
    );
    assert_eq!(actual.artifacts()[0].bytes(), expected.artifacts[0].bytes);
    assert_eq!(
        actual.request_inventory().options().value().ppi,
        Some(144.0)
    );
    assert!(!actual.request_inventory().options().value().render_bleed);
    assert_eq!(actual.exporter_identity().implementation(), "typst-render");
    assert_eq!(actual.exporter_identity().version(), "0.15.0");
}

#[test]
fn stabilized_pack_matches_complete_official_png_page_artifacts() {
    let fixture = Fixture::official_oracle();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let options = CompileOptions {
        page_selection: parse_page_selection("2,1,2").unwrap(),
        ppi: Some(216.0),
        render_bleed: true,
        ..CompileOptions::default()
    };

    let actual = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Png)
            .inputs(inputs)
            .document_time(document_time)
            .options(options),
    )
    .unwrap();
    let repeated = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Png)
            .inputs(string_inputs([("width", "24")]))
            .document_time(document_time)
            .options(CompileOptions {
                page_selection: parse_page_selection("1-2").unwrap(),
                ppi: Some(216.0),
                render_bleed: true,
                ..CompileOptions::default()
            }),
    )
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: string_inputs([("width", "24")]),
            features: vec![],
            document_time: Some(document_time),
            output: OutputRequest::Png {
                source_pages: vec![
                    NonZeroUsize::new(2).unwrap(),
                    NonZeroUsize::new(1).unwrap(),
                    NonZeroUsize::new(2).unwrap(),
                ],
                pixels_per_inch: 216.0,
                render_bleed: true,
            },
        },
    );

    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert!(actual.diagnostics().iter().all(|diagnostic| {
        diagnostic.phase() == DiagnosticPhase::Compilation
            && diagnostic.producer() == DiagnosticProducer::Engine(actual.engine_identity())
    }));
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    for (actual, expected) in actual.artifacts().iter().zip(&expected.artifacts) {
        let ArtifactRole::Png { source_page_number } = expected.role else {
            panic!("oracle produced a non-PNG artifact");
        };
        assert_eq!(actual.format(), OutputFormat::Png);
        assert_eq!(actual.source_page_number(), Some(source_page_number));
        assert_eq!(actual.bytes(), expected.bytes);
    }
    assert_eq!(
        actual
            .artifacts()
            .iter()
            .map(|artifact| artifact.source_page_number().unwrap().get())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(actual.pack_warnings().is_empty());
    assert_eq!(actual.exporter_identity().implementation(), "typst-render");
    assert_eq!(actual.exporter_identity().version(), "0.15.0");
    assert_eq!(actual.artifacts().len(), repeated.artifacts().len());
    assert!(
        actual
            .artifacts()
            .iter()
            .zip(repeated.artifacts())
            .all(|(actual, repeated)| actual.bytes() == repeated.bytes())
    );
}

#[test]
fn valid_png_selection_matching_no_page_matches_the_official_oracle() {
    let fixture = Fixture::static_shape();
    let options = CompileOptions {
        page_selection: parse_page_selection("9").unwrap(),
        ..CompileOptions::default()
    };
    let actual = compile_pack(
        PackCompilationRequest::new(stabilized_pack(&fixture), OutputFormat::Png).options(options),
    )
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![],
            document_time: None,
            output: OutputRequest::Png {
                source_pages: vec![NonZeroUsize::new(9).unwrap()],
                pixels_per_inch: 144.0,
                render_bleed: false,
            },
        },
    );

    assert_eq!(expected.status, ObservationStatus::Accepted);
    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_eq!(actual.source_page_count(), expected.source_page_count);
    assert_diagnostics_match(actual.diagnostics(), &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    assert!(actual.artifacts().is_empty());
}

#[test]
fn official_rejection_preserves_warning_and_error_order() {
    let fixture = Fixture::official_oracle();
    let request = ReferenceRequest {
        inputs: string_inputs([("width", "24")]),
        features: vec![],
        document_time: None,
        output: OutputRequest::Svg {
            source_pages: vec![],
            render_bleed: false,
            pretty: false,
        },
    };

    let observation = observe(&fixture, &request);

    assert_eq!(observation.status, ObservationStatus::Rejected);
    assert_eq!(observation.target, Target::Paged);
    assert_eq!(observation.source_page_count, None);
    assert!(observation.artifacts.is_empty());
    assert_eq!(observation.diagnostics.len(), 2);
    assert_eq!(observation.diagnostics[0].severity.as_str(), "warning");
    assert_eq!(observation.diagnostics[1].severity.as_str(), "error");
    assert_eq!(
        observation.diagnostics[1].message,
        "unable to get the current date"
    );
    assert_eq!(
        observation.diagnostics[1].span.logical_path.as_deref(),
        Some("package:@local/oracle:1.0.0/lib.typ")
    );
    assert_eq!(observation.diagnostics[1].trace.len(), 1);
    assert_eq!(
        observation.diagnostics[1].trace[0]
            .span
            .logical_path
            .as_deref(),
        Some("project:main.typ")
    );
}
