mod support;

use std::num::NonZeroUsize;

use support::official_typst::{
    ArtifactRole, Fixture, ObservationStatus, OutputRequest, ReferenceRequest, Target, observe,
};
use typst::foundations::{Datetime, Smart};

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
fn oracle_is_structurally_independent_of_the_production_crate() {
    let test_sources = concat!(
        include_str!("official_typst_oracle.rs"),
        include_str!("support/mod.rs"),
        include_str!("support/official_typst.rs"),
    );
    let production_crate = ["typst", "pack"].join("_");

    assert!(!test_sources.contains(&production_crate));
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
