mod support;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

#[cfg(feature = "embedded-fonts")]
use support::official_typst::select_font;
use support::official_typst::{
    ArtifactRole, DiagnosticObservation, DiagnosticSeverity, Fixture, ObservationStatus,
    OutputRequest, ReferenceRequest, Target, TraceKind, observe,
};
use typst::WorldExt;
use typst::diag::{Severity, SourceDiagnostic, Tracepoint};
#[cfg(feature = "embedded-fonts")]
use typst::foundations::Bytes;
use typst::foundations::{Datetime, Dict, Smart, Value};
use typst::syntax::{FileId, VirtualRoot};
use typst_pack::compile::{
    self, Artifact, CreationTimestamp, DocumentTime, Input, Output, OutputFormat, RequestIssue,
};
use typst_pack::{Digest, Pack, Sources, package};
use typst_pdf::PdfStandard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompilationStatus {
    Succeeded,
    Rejected,
}

/// One compilation as the oracle observes it: a success or a failed compile,
/// with warnings before errors.
struct Observed {
    status: CompilationStatus,
    artifacts: Vec<Artifact>,
    diagnostics: Vec<SourceDiagnostic>,
    page_count: Option<usize>,
    identity: Option<Digest>,
    sources: Sources,
}

impl Observed {
    fn status(&self) -> CompilationStatus {
        self.status
    }

    fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    fn source_page_count(&self) -> Option<usize> {
        self.page_count
    }

    fn identity(&self) -> Digest {
        self.identity
            .expect("a succeeded compilation has an identity")
    }
}

fn compile(request: compile::Request) -> Result<Observed, compile::Error> {
    match typst_pack::compile(&request) {
        Ok(compiled) => Ok(Observed {
            status: CompilationStatus::Succeeded,
            artifacts: compiled.artifacts,
            diagnostics: compiled.warnings,
            page_count: compiled.page_count,
            identity: Some(compiled.identity),
            sources: compiled.sources,
        }),
        Err(compile::Error::Failed {
            errors,
            warnings,
            sources,
        }) => Ok(Observed {
            status: CompilationStatus::Rejected,
            artifacts: Vec::new(),
            diagnostics: warnings.into_iter().chain(errors).collect(),
            page_count: None,
            identity: None,
            sources,
        }),
        Err(error) => Err(error),
    }
}

/// Request setup in the shape the oracle requests use.
trait Requested {
    fn inputs(self, inputs: Dict) -> Self;
    fn document_time(self, time: DocumentTime) -> Self;
    fn feature(self, feature: typst::Feature) -> Self;
    fn with_packages(
        self,
        packages: Vec<(typst::syntax::package::PackageSpec, package::Tree)>,
    ) -> Self;
}

impl Requested for compile::Request {
    fn inputs(mut self, inputs: Dict) -> Self {
        self.options.inputs = inputs
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    Value::Str(text) => Input::Str(text.into()),
                    Value::Bytes(bytes) => Input::Bytes(bytes.to_vec()),
                    other => panic!("a pack input is a string or bytes, not {other:?}"),
                };
                (key.into(), value)
            })
            .collect();
        self
    }

    fn document_time(mut self, time: DocumentTime) -> Self {
        self.options.document_time = time;
        self
    }

    fn feature(mut self, feature: typst::Feature) -> Self {
        self.options.features.push(feature);
        self
    }

    fn with_packages(
        mut self,
        packages: Vec<(typst::syntax::package::PackageSpec, package::Tree)>,
    ) -> Self {
        self.packages = packages;
        self
    }
}

fn output(format: OutputFormat) -> Output {
    Output::new(format)
}

/// The logical name the oracle gives a file.
fn logical_path(id: FileId) -> String {
    let path = id.vpath().get_without_slash();
    match id.root() {
        VirtualRoot::Project => format!("project:{path}"),
        VirtualRoot::Package(spec) => format!("package:{spec}/{path}"),
    }
}

fn stabilized_pack(fixture: &Fixture) -> Pack {
    let mut builder = Pack::builder(fixture.entrypoint());
    for &(path, text) in fixture.project() {
        builder = builder.file(path, text.as_bytes().to_vec());
    }
    for &(spec, path, text) in fixture.packages() {
        builder = builder.package_file(spec.parse().unwrap(), path, text.as_bytes().to_vec());
    }
    for (data, _) in fixture.fonts() {
        builder = builder.font(data.clone());
    }
    let pack = builder.build().unwrap();
    typst_pack::archive::decode(&typst_pack::archive::encode(&pack), Default::default()).unwrap()
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
    let embedded = compile(
        compile::Request::new(stabilized_pack(&fixture), output(OutputFormat::Svg))
            .inputs(string_inputs([("width", "24")]))
            .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap())),
    )
    .unwrap();

    let mut builder = Pack::builder(fixture.entrypoint());
    for &(path, text) in fixture.project() {
        builder = builder.file(path, text.as_bytes().to_vec());
    }
    let spec: typst::syntax::package::PackageSpec = fixture.packages()[0].0.parse().unwrap();
    for &(_, path, text) in fixture.packages() {
        builder = builder.external_package_file(spec.clone(), path, text.as_bytes().to_vec());
    }
    let external_pack = builder.build().unwrap();
    let external = compile(
        compile::Request::new(external_pack, output(OutputFormat::Svg))
            .inputs(string_inputs([("width", "24")]))
            .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap()))
            .with_packages(vec![(
                spec,
                package::Tree::new(
                    fixture
                        .packages()
                        .iter()
                        .map(|&(_, path, text)| (path, text.as_bytes().to_vec())),
                )
                .unwrap(),
            )]),
    )
    .unwrap();

    assert_eq!(embedded.status(), CompilationStatus::Succeeded);
    assert_eq!(external.status(), embedded.status());
    assert_diagnostics_match(&embedded, &expected.diagnostics);
    assert_eq!(external.diagnostics, embedded.diagnostics);
    assert_eq!(external.artifacts().len(), expected.artifacts.len());
    assert!(
        external
            .artifacts()
            .iter()
            .zip(embedded.artifacts())
            .all(|(external, embedded)| external.bytes == embedded.bytes)
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
    let requirement = &embedded_pack.font_requirements()[0];
    assert_eq!(
        requirement.faces[0].family,
        expected.font_catalog[0].0.family
    );
    assert_eq!(
        embedded_pack.font_data(&requirement.digest),
        Some(expected.font_catalog[0].1.as_ref())
    );
    let embedded = compile(compile::Request::new(
        embedded_pack,
        output(OutputFormat::Svg),
    ))
    .unwrap();
    assert_eq!(embedded.artifacts()[0].bytes, expected.artifacts[0].bytes);

    let external_pack = Pack::builder("main.typ")
        .file("main.typ", fixture.project()[0].1.as_bytes().to_vec())
        .external_font(data.clone())
        .build()
        .unwrap();
    let mut request = compile::Request::new(external_pack, output(OutputFormat::Svg));
    request.fonts = vec![typst_pack::font::Container::new(data).unwrap()];
    let external = compile(request).unwrap();
    assert_eq!(external.artifacts()[0].bytes, expected.artifacts[0].bytes);
    assert_eq!(external.diagnostics.len(), expected.diagnostics.len());
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

        let expected_output = observe(
            &fixture,
            &ReferenceRequest {
                inputs: Dict::new(),
                features: vec![],
                document_time: None,
                output: OutputRequest::Svg {
                    source_pages: vec![],
                    render_bleed: false,
                    pretty: false,
                },
            },
        );
        let embedded = compile(compile::Request::new(
            stabilized_pack(&fixture),
            output(OutputFormat::Svg),
        ))
        .unwrap();
        assert_eq!(
            embedded.artifacts()[0].bytes,
            expected_output.artifacts[0].bytes
        );

        let external_pack = Pack::builder("main.typ")
            .file("main.typ", fixture.project()[0].1.as_bytes().to_vec())
            .external_font(ordered[0].clone())
            .external_font(ordered[1].clone())
            .build()
            .unwrap();
        let mut request = compile::Request::new(external_pack, output(OutputFormat::Svg));
        request.fonts = ordered
            .into_iter()
            .map(|data| typst_pack::font::Container::new(data).unwrap())
            .collect();
        let external = compile(request).unwrap();
        assert_eq!(
            external.artifacts()[0].bytes,
            expected_output.artifacts[0].bytes
        );
    }
}

fn string_inputs<const N: usize>(entries: [(&str, &str); N]) -> Dict {
    entries
        .into_iter()
        .map(|(key, value)| (key.into(), Value::Str(value.into())))
        .collect()
}

fn assert_diagnostics_match(actual: &Observed, expected: &[DiagnosticObservation]) {
    let sources = &actual.sources;
    let span = |span: typst::syntax::DiagSpan| (span.id().map(logical_path), sources.range(span));
    let expected_span = |span: &support::official_typst::LogicalSpan| {
        (span.logical_path.clone(), span.byte_range.clone())
    };
    assert_eq!(actual.diagnostics.len(), expected.len());
    for (actual, expected) in actual.diagnostics.iter().zip(expected) {
        assert_eq!(
            actual.severity,
            match expected.severity {
                DiagnosticSeverity::Error => Severity::Error,
                DiagnosticSeverity::Warning => Severity::Warning,
            }
        );
        assert_eq!(actual.message, expected.message);
        assert_eq!(span(actual.span), expected_span(&expected.span));
        assert_eq!(actual.hints.len(), expected.hints.len());
        for (actual, expected) in actual.hints.iter().zip(&expected.hints) {
            assert_eq!(actual.v, expected.message);
            assert_eq!(span(actual.span), expected_span(&expected.span));
        }
        assert_eq!(actual.trace.len(), expected.trace.len());
        for (actual, expected) in actual.trace.iter().zip(&expected.trace) {
            let (kind, value) = match &actual.v {
                Tracepoint::Call(value) => {
                    (TraceKind::Call, value.as_ref().map(ToString::to_string))
                }
                Tracepoint::Show(value) => (TraceKind::Show, Some(value.to_string())),
                Tracepoint::Import(value) => (TraceKind::Import, Some(value.to_string())),
                Tracepoint::Include(value) => (TraceKind::Include, Some(value.to_string())),
            };
            assert_eq!(kind, expected.kind);
            assert_eq!(value, expected.value);
            assert_eq!(span(actual.span.into()), expected_span(&expected.span));
        }
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
    let pack =
        typst_pack::archive::decode(&typst_pack::archive::encode(&created), Default::default())
            .unwrap();
    let output_specification = Output {
        pages: "2,1".parse::<compile::PageSelection>().unwrap(),
        pretty: true,
        ..Output::new(OutputFormat::Svg)
    };
    let request = compile::Request::new(pack, output_specification)
        .inputs(inputs)
        .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap()));

    let actual = compile(request).unwrap();
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    for (actual, expected) in actual.artifacts().iter().zip(&expected.artifacts) {
        let ArtifactRole::Svg { source_page_number } = expected.role else {
            panic!("oracle produced a non-SVG artifact");
        };
        assert_eq!(actual.format, OutputFormat::Svg);
        assert_eq!(actual.page, Some(source_page_number));
        assert_eq!(actual.bytes, expected.bytes);
    }
    let actual_dependencies = actual
        .sources
        .files()
        .iter()
        .map(|id| logical_path(*id))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_dependencies, expected.dependencies);
}

#[test]
fn string_and_byte_inputs_match_the_independent_oracle() {
    let fixture = Fixture::byte_input();
    let mut inputs = Dict::new();
    inputs.insert("label".into(), Value::Str("text".into()));
    inputs.insert(
        "data".into(),
        Value::Bytes(typst::foundations::Bytes::new(b"bytes".to_vec())),
    );
    let reference_request = ReferenceRequest {
        inputs: inputs.clone(),
        features: vec![],
        document_time: None,
        output: OutputRequest::Svg {
            source_pages: vec![],
            render_bleed: false,
            pretty: false,
        },
    };
    let expected = observe(&fixture, &reference_request);
    let actual = compile(
        compile::Request::new(stabilized_pack(&fixture), output(OutputFormat::Svg)).inputs(inputs),
    )
    .unwrap();

    assert_eq!(expected.status, ObservationStatus::Accepted);
    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn shared_semantic_values_drive_the_same_official_source_behavior() {
    let fixture = Fixture::semantic_request();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let inputs = string_inputs([("width", "24")]);
    let pack = stabilized_pack(&fixture);

    let actual = compile(
        compile::Request::new(pack, output(OutputFormat::Svg))
            .inputs(inputs.clone())
            .feature(typst::Feature::A11yExtras)
            .document_time(DocumentTime::Fixed(document_time)),
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
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn independent_oracle_and_pack_compilation_ignore_conflicting_ambient_environment() {
    const CHILD_ENV: &str = "TYPST_PACK_AMBIENT_ORACLE_CHILD";
    const TEST_NAME: &str =
        "independent_oracle_and_pack_compilation_ignore_conflicting_ambient_environment";

    if std::env::var_os(CHILD_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(CHILD_ENV, "1")
            .env("SOURCE_DATE_EPOCH", "0")
            .env("TYPST_FEATURES", "bundle")
            .env("TYPST_ROOT", "/ambient/root")
            .env("TYPST_PACKAGE_PATH", "/ambient/packages")
            .env("TYPST_FONT_PATHS", "/ambient/fonts")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "ambient-environment child failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let fixture = Fixture::semantic_request();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let inputs = string_inputs([("width", "24")]);
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: inputs.clone(),
            features: vec![typst::Feature::A11yExtras],
            document_time: Some(document_time),
            output: OutputRequest::Svg {
                source_pages: vec![],
                render_bleed: false,
                pretty: false,
            },
        },
    );
    let actual = compile(
        compile::Request::new(stabilized_pack(&fixture), output(OutputFormat::Svg))
            .inputs(inputs)
            .feature(typst::Feature::A11yExtras)
            .document_time(DocumentTime::Fixed(document_time)),
    )
    .unwrap();

    assert_eq!(expected.status, ObservationStatus::Accepted);
    assert_eq!(actual.status(), CompilationStatus::Succeeded);
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn html_feature_is_derived_and_bundle_is_rejected() {
    let fixture = Fixture::exporter_rejection();
    let pack = stabilized_pack(&fixture);
    let result = compile(compile::Request::new(pack, output(OutputFormat::Html))).unwrap();
    assert_eq!(result.status(), CompilationStatus::Succeeded);

    let explicit = compile(
        compile::Request::new(stabilized_pack(&fixture), output(OutputFormat::Html))
            .feature(typst::Feature::Html),
    )
    .unwrap();
    assert_eq!(result.identity(), explicit.identity());

    let rejected = compile(
        compile::Request::new(stabilized_pack(&fixture), output(OutputFormat::Html))
            .feature(typst::Feature::Bundle),
    );
    let Err(compile::Error::InvalidRequest(issues)) = rejected else {
        panic!("the unsupported Bundle feature must be rejected before compiling");
    };
    assert_eq!(issues, [RequestIssue::UnsupportedBundleFeature]);
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

    let first =
        compile(compile::Request::new(first_pack, output(OutputFormat::Svg)).inputs(first_inputs))
            .unwrap();
    let second = compile(
        compile::Request::new(second_pack, output(OutputFormat::Svg)).inputs(second_inputs),
    )
    .unwrap();
    let feature = compile(
        compile::Request::new(feature_pack, output(OutputFormat::Svg))
            .feature(typst::Feature::A11yExtras),
    )
    .unwrap();
    let document_time = compile(
        compile::Request::new(time_pack, output(OutputFormat::Svg))
            .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap())),
    )
    .unwrap();

    assert_ne!(first.identity(), second.identity());
    assert_ne!(first.identity(), feature.identity());
    assert_ne!(first.identity(), document_time.identity());
    assert_eq!(first.artifacts()[0].bytes, second.artifacts()[0].bytes);
    assert_eq!(first.artifacts()[0].bytes, feature.artifacts()[0].bytes);
    assert_eq!(
        first.artifacts()[0].bytes,
        document_time.artifacts()[0].bytes
    );
}

#[test]
fn pack_rejection_matches_official_diagnostics_and_remains_a_result() {
    let fixture = Fixture::official_oracle();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let pack = stabilized_pack(&fixture);

    let actual = compile(compile::Request::new(pack, output(OutputFormat::Svg)).inputs(inputs))
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
}

#[test]
fn pack_exporter_rejection_matches_official_diagnostics() {
    let fixture = Fixture::exporter_rejection();
    let pack = stabilized_pack(&fixture);

    let actual = compile(compile::Request::new(pack, output(OutputFormat::Pdf))).unwrap();
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
}

#[test]
fn pack_pdf_option_conflict_matches_official_exporter_rejection() {
    let fixture = Fixture::static_shape();
    let pack = stabilized_pack(&fixture);
    let source_page = NonZeroUsize::new(1).unwrap();
    let actual = compile(compile::Request::new(
        pack,
        Output {
            pages: "1".parse::<compile::PageSelection>().unwrap(),
            pdf_tags: Smart::Custom(true),
            ..Output::new(OutputFormat::Pdf)
        },
    ))
    .unwrap();
    let expected = observe(
        &fixture,
        &ReferenceRequest {
            inputs: Dict::new(),
            features: vec![],
            document_time: None,
            output: OutputRequest::Pdf {
                source_pages: vec![source_page],
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
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
    let output_specification = Output {
        pages: "2".parse::<compile::PageSelection>().unwrap(),
        pretty: true,
        pdf_identifier: Smart::Custom("official-oracle".to_owned()),
        pdf_creator: Smart::Custom(Some("typst-pack differential oracle".to_owned())),
        pdf_tags: Smart::Custom(false),
        pdf_standards: vec![PdfStandard::A_2b],
        pdf_timestamp: CreationTimestamp::Utc(1_706_933_106),
        ..Output::new(OutputFormat::Pdf)
    };

    let actual = compile(
        compile::Request::new(pack, output_specification)
            .inputs(inputs)
            .document_time(DocumentTime::Fixed(document_time)),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), 1);
    assert_eq!(expected.artifacts.len(), 1);
    assert_eq!(expected.artifacts[0].role, ArtifactRole::Pdf);
    assert_eq!(actual.artifacts()[0].format, OutputFormat::Pdf);
    assert_eq!(actual.artifacts()[0].page, None);
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn stabilized_pack_matches_official_pdf_exporter_defaults() {
    let fixture = Fixture::official_oracle();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let pack = stabilized_pack(&fixture);

    let actual = compile(
        compile::Request::new(pack, output(OutputFormat::Pdf))
            .inputs(inputs)
            .document_time(DocumentTime::Fixed(document_time)),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), 1);
    assert_eq!(actual.artifacts()[0].page, None);
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn stabilized_pack_matches_official_html_artifacts_and_pretty_control() {
    let fixture = Fixture::html_success();
    assert_eq!(
        Output::new(OutputFormat::Html).pretty,
        typst_html::HtmlOptions::default().pretty
    );
    let compact = compile(compile::Request::new(
        stabilized_pack(&fixture),
        output(OutputFormat::Html),
    ))
    .unwrap();
    let pretty_result = compile(compile::Request::new(
        stabilized_pack(&fixture),
        Output {
            pretty: true,
            ..Output::new(OutputFormat::Html)
        },
    ))
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

    for (actual, expected) in [
        (&compact, &expected_compact),
        (&pretty_result, &expected_pretty),
    ] {
        assert_eq!(expected.status, ObservationStatus::Accepted);
        assert_eq!(expected.target, Target::Html);
        assert_eq!(actual.status(), CompilationStatus::Succeeded);
        assert_eq!(actual.source_page_count(), None);
        assert_diagnostics_match(actual, &expected.diagnostics);
        assert_eq!(actual.artifacts().len(), 1);
        assert_eq!(expected.artifacts.len(), 1);
        assert_eq!(expected.artifacts[0].role, ArtifactRole::Html);
        assert_eq!(actual.artifacts()[0].format, OutputFormat::Html);
        assert_eq!(actual.artifacts()[0].page, None);
        assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
    }
    assert_ne!(
        compact.artifacts()[0].bytes,
        pretty_result.artifacts()[0].bytes
    );
    assert_ne!(compact.identity(), pretty_result.identity());
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
    let actual = compile(compile::Request::new(
        stabilized_pack(&fixture),
        output(OutputFormat::Html),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    let (error, warnings) = actual.diagnostics.split_last().unwrap();
    assert!(!warnings.is_empty());
    assert!(
        warnings
            .iter()
            .all(|diagnostic| diagnostic.severity == Severity::Warning)
    );
    assert_eq!(error.severity, Severity::Error);
}

#[test]
fn html_exporter_rejection_matches_official_diagnostics_and_order() {
    let fixture = Fixture::html_exporter_rejection();
    let actual = compile(compile::Request::new(
        stabilized_pack(&fixture),
        output(OutputFormat::Html),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    let (error, warnings) = actual.diagnostics.split_last().unwrap();
    assert!(!warnings.is_empty());
    assert!(
        warnings
            .iter()
            .all(|warning| warning.severity == Severity::Warning)
    );
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.hints.len(), 1);
}

#[test]
fn pack_png_defaults_match_the_pinned_official_exporter() {
    let fixture = Fixture::static_shape();
    let actual = compile(compile::Request::new(
        stabilized_pack(&fixture),
        output(OutputFormat::Png),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    assert_eq!(expected.artifacts.len(), 1);
    assert_eq!(
        expected.artifacts[0].role,
        ArtifactRole::Png {
            source_page_number: NonZeroUsize::new(1).unwrap(),
        }
    );
    assert_eq!(actual.artifacts()[0].format, OutputFormat::Png);
    assert_eq!(actual.artifacts()[0].page, NonZeroUsize::new(1));
    assert_eq!(actual.artifacts()[0].bytes, expected.artifacts[0].bytes);
}

#[test]
fn stabilized_pack_matches_complete_official_png_page_artifacts() {
    let fixture = Fixture::official_oracle();
    let document_time = Datetime::from_ymd(2024, 2, 3).unwrap();
    let mut inputs = Dict::new();
    inputs.insert("width".into(), Value::Str("24".into()));
    let output_specification = Output {
        pages: "2,1,2".parse::<compile::PageSelection>().unwrap(),
        ppi: Some(216.0),
        render_bleed: true,
        ..Output::new(OutputFormat::Png)
    };

    let actual = compile(
        compile::Request::new(stabilized_pack(&fixture), output_specification)
            .inputs(inputs)
            .document_time(DocumentTime::Fixed(document_time)),
    )
    .unwrap();
    let repeated = compile(
        compile::Request::new(
            stabilized_pack(&fixture),
            Output {
                pages: "1-2".parse::<compile::PageSelection>().unwrap(),
                ppi: Some(216.0),
                render_bleed: true,
                ..Output::new(OutputFormat::Png)
            },
        )
        .inputs(string_inputs([("width", "24")]))
        .document_time(DocumentTime::Fixed(document_time)),
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
    assert_eq!(actual.artifacts().len(), expected.artifacts.len());
    for (actual, expected) in actual.artifacts().iter().zip(&expected.artifacts) {
        let ArtifactRole::Png { source_page_number } = expected.role else {
            panic!("oracle produced a non-PNG artifact");
        };
        assert_eq!(actual.format, OutputFormat::Png);
        assert_eq!(actual.page, Some(source_page_number));
        assert_eq!(actual.bytes, expected.bytes);
    }
    assert_eq!(
        actual
            .artifacts()
            .iter()
            .map(|artifact| artifact.page.unwrap().get())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(actual.artifacts().len(), repeated.artifacts().len());
    assert!(
        actual
            .artifacts()
            .iter()
            .zip(repeated.artifacts())
            .all(|(actual, repeated)| actual.bytes == repeated.bytes)
    );
}

#[test]
fn valid_png_selection_matching_no_page_matches_the_official_oracle() {
    let fixture = Fixture::static_shape();
    let output_specification = Output {
        pages: "9".parse::<compile::PageSelection>().unwrap(),
        ..Output::new(OutputFormat::Png)
    };
    let actual = compile(compile::Request::new(
        stabilized_pack(&fixture),
        output_specification,
    ))
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
    assert_diagnostics_match(&actual, &expected.diagnostics);
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
