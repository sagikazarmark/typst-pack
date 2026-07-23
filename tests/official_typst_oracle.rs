mod support;

use std::num::NonZeroUsize;

use support::official_typst::{
    ArtifactRole, Fixture, ObservationStatus, OutputRequest, ReferenceRequest, Target, observe,
};
use typst::foundations::{Datetime, Dict, Smart, Value};
use typst_pack::{
    CompileOptions, OutputFormat, Pack, PackCompilationRequest, compile_pack, parse_page_selection,
};

#[test]
fn frozen_fixture_and_request_produce_a_stable_official_observation() {
    let fixture = Fixture::official_oracle();
    let request = ReferenceRequest {
        inputs: vec![("width", "24")],
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
    let mut builder = Pack::builder(fixture.entrypoint());
    for &(path, text) in fixture.project() {
        builder = builder.file(path, text.as_bytes().to_vec()).unwrap();
    }
    for &(spec, path, text) in fixture.packages() {
        builder = builder
            .package_file(spec.parse().unwrap(), path, text.as_bytes().to_vec())
            .unwrap();
    }
    let created = builder.build().unwrap();
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
            inputs: vec![("width", "24")],
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
    assert_eq!(actual.warnings.len(), expected.diagnostics.len());
    for (actual, expected) in actual.warnings.iter().zip(&expected.diagnostics) {
        assert_eq!(actual.message.as_str(), expected.message);
        assert_eq!(
            actual
                .hints
                .iter()
                .map(|hint| hint.v.as_str())
                .collect::<Vec<_>>(),
            expected
                .hints
                .iter()
                .map(|hint| hint.message.as_str())
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(actual.artifacts.len(), expected.artifacts.len());
    for (actual, expected) in actual.artifacts.iter().zip(&expected.artifacts) {
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
        inputs: vec![("width", "24")],
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
fn official_rejection_preserves_warning_and_error_order() {
    let fixture = Fixture::official_oracle();
    let request = ReferenceRequest {
        inputs: vec![("width", "24")],
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
