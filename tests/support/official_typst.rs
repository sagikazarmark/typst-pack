use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroUsize;
use std::ops::Range;
use std::path::PathBuf;

use typst::diag::{
    FileError, FileResult, HintedString, Severity, SourceDiagnostic, Tracepoint, Warned,
};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Smart, Value};
use typst::layout::PageRanges;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryExt, World, WorldExt};
use typst_layout::{Page, PagedDocument};
use typst_pdf::{PdfStandard, PdfStandards, Timestamp};

const MAIN: &str = include_str!("../fixtures/official-oracle/main.typ");
const CHAPTER: &str = include_str!("../fixtures/official-oracle/chapter.typ");
const PACKAGE_MANIFEST: &str =
    include_str!("../fixtures/official-oracle/packages/local/oracle/1.0.0/typst.toml");
const PACKAGE_ENTRYPOINT: &str =
    include_str!("../fixtures/official-oracle/packages/local/oracle/1.0.0/lib.typ");
const ORACLE_PACKAGE_SPEC: &str = "@local/oracle:1.0.0";

pub struct Fixture {
    entrypoint: &'static str,
    project: &'static [(&'static str, &'static str)],
    packages: &'static [(&'static str, &'static str, &'static str)],
}

impl Fixture {
    pub fn official_oracle() -> Self {
        Self {
            entrypoint: "main.typ",
            project: &[("main.typ", MAIN), ("chapter.typ", CHAPTER)],
            packages: &[
                (ORACLE_PACKAGE_SPEC, "typst.toml", PACKAGE_MANIFEST),
                (ORACLE_PACKAGE_SPEC, "lib.typ", PACKAGE_ENTRYPOINT),
            ],
        }
    }
}

pub struct ReferenceRequest {
    pub inputs: Vec<(&'static str, &'static str)>,
    pub features: Vec<Feature>,
    pub document_time: Option<Datetime>,
    pub output: OutputRequest,
}

#[allow(dead_code)]
pub enum OutputRequest {
    Pdf {
        source_pages: Vec<NonZeroUsize>,
        ident: Smart<String>,
        creator: Smart<Option<String>>,
        creation_time: Option<Timestamp>,
        standards: Vec<PdfStandard>,
        tagged: bool,
        pretty: bool,
    },
    Png {
        source_pages: Vec<NonZeroUsize>,
        pixels_per_inch: f64,
        render_bleed: bool,
    },
    Svg {
        source_pages: Vec<NonZeroUsize>,
        render_bleed: bool,
        pretty: bool,
    },
    Html {
        pretty: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Paged,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalSpan {
    pub logical_path: Option<String>,
    pub byte_range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintObservation {
    pub message: String,
    pub span: LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticObservation {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: LogicalSpan,
    pub hints: Vec<HintObservation>,
    pub trace: Vec<TraceObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    Call,
    Show,
    Import,
    Include,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceObservation {
    pub kind: TraceKind,
    pub value: Option<String>,
    pub span: LogicalSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactRole {
    Pdf,
    Png { source_page_number: NonZeroUsize },
    Svg { source_page_number: NonZeroUsize },
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactObservation {
    pub role: ArtifactRole,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub status: ObservationStatus,
    pub target: Target,
    pub source_page_count: Option<usize>,
    pub diagnostics: Vec<DiagnosticObservation>,
    pub artifacts: Vec<ArtifactObservation>,
}

pub fn observe(fixture: &Fixture, request: &ReferenceRequest) -> Observation {
    let world = ReferenceWorld::new(fixture, request);
    match &request.output {
        OutputRequest::Html { pretty } => observe_html(&world, *pretty),
        output => observe_paged(&world, output),
    }
}

fn observe_html(world: &ReferenceWorld, pretty: bool) -> Observation {
    let Warned { output, warnings } = typst::compile::<typst_html::HtmlDocument>(world);
    let mut observation = Observation {
        status: ObservationStatus::Accepted,
        target: Target::Html,
        source_page_count: None,
        diagnostics: project_diagnostics(world, warnings),
        artifacts: vec![],
    };
    let document = match output {
        Ok(document) => document,
        Err(errors) => {
            observation.status = ObservationStatus::Rejected;
            observation
                .diagnostics
                .extend(project_diagnostics(world, errors));
            return observation;
        }
    };
    match typst_html::html(&document, &typst_html::HtmlOptions { pretty }) {
        Ok(html) => observation.artifacts.push(ArtifactObservation {
            role: ArtifactRole::Html,
            bytes: html.into_bytes(),
        }),
        Err(errors) => {
            observation.status = ObservationStatus::Rejected;
            observation
                .diagnostics
                .extend(project_diagnostics(world, errors));
        }
    }
    observation
}

fn observe_paged(world: &ReferenceWorld, output: &OutputRequest) -> Observation {
    let Warned {
        output: document,
        warnings,
    } = typst::compile::<PagedDocument>(world);
    let mut observation = Observation {
        status: ObservationStatus::Accepted,
        target: Target::Paged,
        source_page_count: None,
        diagnostics: project_diagnostics(world, warnings),
        artifacts: vec![],
    };
    let document = match document {
        Ok(document) => document,
        Err(errors) => {
            observation.status = ObservationStatus::Rejected;
            observation
                .diagnostics
                .extend(project_diagnostics(world, errors));
            return observation;
        }
    };
    observation.source_page_count = Some(document.pages().len());

    match output {
        OutputRequest::Pdf {
            source_pages,
            ident,
            creator,
            creation_time,
            standards,
            tagged,
            pretty,
        } => {
            let standards = match PdfStandards::new(standards) {
                Ok(standards) => standards,
                Err(error) => {
                    observation.status = ObservationStatus::Rejected;
                    observation.diagnostics.push(hinted_error(error));
                    return observation;
                }
            };
            let options = typst_pdf::PdfOptions {
                ident: ident.clone(),
                creator: creator.clone(),
                timestamp: *creation_time,
                page_ranges: page_ranges(source_pages),
                standards,
                tagged: *tagged,
                pretty: *pretty,
            };
            match typst_pdf::pdf(&document, &options) {
                Ok(bytes) => observation.artifacts.push(ArtifactObservation {
                    role: ArtifactRole::Pdf,
                    bytes,
                }),
                Err(errors) => {
                    observation.status = ObservationStatus::Rejected;
                    observation
                        .diagnostics
                        .extend(project_diagnostics(world, errors));
                }
            }
        }
        OutputRequest::Png {
            source_pages,
            pixels_per_inch,
            render_bleed,
        } => {
            let options = typst_render::RenderOptions {
                pixel_per_pt: (*pixels_per_inch / 72.0).into(),
                render_bleed: *render_bleed,
            };
            for (number, page) in selected_pages(&document, source_pages) {
                match typst_render::render(page, &options).encode_png() {
                    Ok(bytes) => observation.artifacts.push(ArtifactObservation {
                        role: ArtifactRole::Png {
                            source_page_number: number,
                        },
                        bytes,
                    }),
                    Err(error) => {
                        observation.status = ObservationStatus::Rejected;
                        observation
                            .diagnostics
                            .push(detached_error(error.to_string()));
                        observation.artifacts.clear();
                        break;
                    }
                }
            }
        }
        OutputRequest::Svg {
            source_pages,
            render_bleed,
            pretty,
        } => {
            let options = typst_svg::SvgOptions {
                render_bleed: *render_bleed,
                pretty: *pretty,
            };
            observation.artifacts = selected_pages(&document, source_pages)
                .map(|(number, page)| ArtifactObservation {
                    role: ArtifactRole::Svg {
                        source_page_number: number,
                    },
                    bytes: typst_svg::svg(page, &options).into_bytes(),
                })
                .collect();
        }
        OutputRequest::Html { .. } => unreachable!("HTML is handled before paged compilation"),
    }
    observation
}

fn page_ranges(source_pages: &[NonZeroUsize]) -> Option<PageRanges> {
    (!source_pages.is_empty()).then(|| {
        PageRanges::new(
            source_pages
                .iter()
                .copied()
                .map(|page| Some(page)..=Some(page))
                .collect(),
        )
    })
}

fn selected_pages<'a>(
    document: &'a PagedDocument,
    source_pages: &[NonZeroUsize],
) -> impl Iterator<Item = (NonZeroUsize, &'a Page)> {
    let selected = source_pages.iter().copied().collect::<BTreeSet<_>>();
    document
        .pages()
        .iter()
        .enumerate()
        .filter_map(move |(index, page)| {
            let number = NonZeroUsize::new(index + 1).unwrap();
            (selected.is_empty() || selected.contains(&number)).then_some((number, page))
        })
}

fn detached_error(message: String) -> DiagnosticObservation {
    DiagnosticObservation {
        severity: DiagnosticSeverity::Error,
        message,
        span: LogicalSpan {
            logical_path: None,
            byte_range: None,
        },
        hints: vec![],
        trace: vec![],
    }
}

fn hinted_error(error: HintedString) -> DiagnosticObservation {
    DiagnosticObservation {
        severity: DiagnosticSeverity::Error,
        message: error.message().to_string(),
        span: LogicalSpan {
            logical_path: None,
            byte_range: None,
        },
        hints: error
            .hints()
            .iter()
            .map(|hint| HintObservation {
                message: hint.to_string(),
                span: LogicalSpan {
                    logical_path: None,
                    byte_range: None,
                },
            })
            .collect(),
        trace: vec![],
    }
}

fn project_diagnostics(
    world: &ReferenceWorld,
    diagnostics: impl IntoIterator<Item = SourceDiagnostic>,
) -> Vec<DiagnosticObservation> {
    diagnostics
        .into_iter()
        .map(|diagnostic| DiagnosticObservation {
            severity: match diagnostic.severity {
                Severity::Error => DiagnosticSeverity::Error,
                Severity::Warning => DiagnosticSeverity::Warning,
            },
            message: diagnostic.message.to_string(),
            span: logical_span(world, diagnostic.span),
            hints: diagnostic
                .hints
                .into_iter()
                .map(|hint| HintObservation {
                    message: hint.v.to_string(),
                    span: logical_span(world, hint.span),
                })
                .collect(),
            trace: diagnostic
                .trace
                .into_iter()
                .map(|trace| {
                    let (kind, value) = match trace.v {
                        Tracepoint::Call(value) => (TraceKind::Call, value.map(String::from)),
                        Tracepoint::Show(value) => (TraceKind::Show, Some(value.into())),
                        Tracepoint::Import(value) => (TraceKind::Import, Some(value.into())),
                        Tracepoint::Include(value) => (TraceKind::Include, Some(value.into())),
                    };
                    TraceObservation {
                        kind,
                        value,
                        span: logical_span(world, trace.span.into()),
                    }
                })
                .collect(),
        })
        .collect()
}

fn logical_span(world: &ReferenceWorld, span: typst::syntax::DiagSpan) -> LogicalSpan {
    LogicalSpan {
        logical_path: span.id().map(logical_path),
        byte_range: world.range(span),
    }
}

fn logical_path(id: FileId) -> String {
    let path = id.vpath().get_without_slash();
    match id.root() {
        VirtualRoot::Project => format!("project:{path}"),
        VirtualRoot::Package(spec) => format!("package:{spec}/{path}"),
    }
}

struct ReferenceWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    main: FileId,
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
    document_time: Option<Datetime>,
}

impl ReferenceWorld {
    fn new(fixture: &Fixture, request: &ReferenceRequest) -> Self {
        let inputs = request
            .inputs
            .iter()
            .map(|(key, value)| ((*key).into(), Value::Str((*value).into())))
            .collect::<Dict>();
        let features = request.features.iter().copied().collect::<Features>();
        let library = LazyHash::new(
            Library::builder()
                .with_inputs(inputs)
                .with_features(features)
                .build(),
        );
        let mut sources = HashMap::new();
        let mut files = HashMap::new();

        for &(path, text) in fixture.project {
            insert_file(&mut sources, &mut files, VirtualRoot::Project, path, text);
        }
        for &(spec, path, text) in fixture.packages {
            let spec = spec
                .parse::<PackageSpec>()
                .expect("frozen package spec is valid");
            insert_file(
                &mut sources,
                &mut files,
                VirtualRoot::Package(spec),
                path,
                text,
            );
        }

        Self {
            library,
            book: LazyHash::new(FontBook::new()),
            main: file_id(VirtualRoot::Project, fixture.entrypoint),
            sources,
            files,
            document_time: request.document_time,
        }
    }
}

fn insert_file(
    sources: &mut HashMap<FileId, Source>,
    files: &mut HashMap<FileId, Bytes>,
    root: VirtualRoot,
    path: &str,
    text: &str,
) {
    let id = file_id(root, path);
    sources.insert(id, Source::new(id, text.to_owned()));
    files.insert(id, Bytes::from_string(text.to_owned()));
}

fn file_id(root: VirtualRoot, path: &str) -> FileId {
    RootedPath::new(root, VirtualPath::new(path).expect("frozen path is valid")).intern()
}

impl World for ReferenceWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.sources
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(logical_path(id))))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(logical_path(id))))
    }

    fn font(&self, _index: usize) -> Option<Font> {
        None
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        self.document_time
    }
}
