//! Compiling a Pack produces the same artifacts and diagnostics as a plain
//! Typst `World` over the same files, so typst-pack's World and exporter
//! plumbing adds nothing of its own.
//!
//! The process-level differential gate against the official `typst` binary is
//! `crates/typst-pack-cli/tests/official_typst_cli.rs`.

use std::collections::HashMap;
use std::num::NonZeroUsize;

use typst::diag::{FileError, FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Smart, Value};
use typst::layout::PageRanges;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World, WorldExt};
use typst_layout::PagedDocument;
use typst_pack::compile::{self, CreationTimestamp, DocumentTime, Input, Output, OutputFormat};
use typst_pack::{Pack, archive};
use typst_pdf::{PdfStandard, PdfStandards, Timestamp};

const PACKAGE: &str = "@local/oracle:1.0.0";
const PACKAGE_FILES: &[(&str, &str)] = &[
    (
        "typst.toml",
        include_str!("fixtures/official-oracle/packages/local/oracle/1.0.0/typst.toml"),
    ),
    (
        "lib.typ",
        include_str!("fixtures/official-oracle/packages/local/oracle/1.0.0/lib.typ"),
    ),
];
/// Two pages, a project include, a package import, a string input, the
/// document date, and a warning with a hint.
const PROJECT: &[(&str, &str)] = &[
    (
        "main.typ",
        include_str!("fixtures/official-oracle/main.typ"),
    ),
    (
        "chapter.typ",
        include_str!("fixtures/official-oracle/chapter.typ"),
    ),
];

/// A plain Typst World over in-memory files, without fonts or a package cache.
struct PlainWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    files: HashMap<FileId, &'static str>,
    today: Option<Datetime>,
}

fn file_id(root: VirtualRoot, path: &str) -> FileId {
    RootedPath::new(root, VirtualPath::new(path).unwrap()).intern()
}

impl PlainWorld {
    fn new(project: &[(&str, &'static str)], request: &Request) -> Self {
        let mut files = HashMap::new();
        for &(path, text) in project {
            files.insert(file_id(VirtualRoot::Project, path), text);
        }
        for &(path, text) in PACKAGE_FILES {
            files.insert(
                file_id(VirtualRoot::Package(PACKAGE.parse().unwrap()), path),
                text,
            );
        }
        let inputs = request
            .inputs
            .iter()
            .map(|(key, value)| (key.as_str().into(), value.clone()))
            .collect::<Dict>();
        let library = Library::builder()
            .with_inputs(inputs)
            .with_features(request.features.iter().copied().collect())
            .build();
        Self {
            library: LazyHash::new(library),
            book: LazyHash::new(FontBook::new()),
            files,
            today: request.today,
        }
    }

    fn get(&self, id: FileId) -> FileResult<&'static str> {
        let path = || FileError::NotFound(id.vpath().get_without_slash().into());
        self.files.get(&id).copied().ok_or_else(path)
    }
}

impl World for PlainWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        file_id(VirtualRoot::Project, "main.typ")
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        Ok(Source::new(id, self.get(id)?.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Ok(Bytes::from_string(self.get(id)?))
    }

    fn font(&self, _: usize) -> Option<Font> {
        None
    }

    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        self.today
    }
}

#[derive(Default)]
struct Request {
    inputs: Vec<(String, Value)>,
    features: Vec<Feature>,
    today: Option<Datetime>,
}

/// The fixture's usual request: `width=24` on 2024-02-03.
fn dated() -> Request {
    Request {
        inputs: vec![("width".into(), Value::Str("24".into()))],
        today: Some(Datetime::from_ymd(2024, 2, 3).unwrap()),
        ..Request::default()
    }
}

/// Compiles the project as an archived Pack with the request's values.
fn pack_compile(
    project: &[(&str, &str)],
    request: &Request,
    output: Output,
) -> Result<compile::Compiled, compile::Error> {
    let mut builder = Pack::builder("main.typ");
    for &(path, text) in project {
        builder = builder.file(path, text.as_bytes().to_vec());
    }
    for &(path, text) in PACKAGE_FILES {
        builder = builder.package_file(PACKAGE.parse().unwrap(), path, text.as_bytes().to_vec());
    }
    let pack = builder.build().unwrap();
    let pack = archive::decode(&archive::encode(&pack), Default::default()).unwrap();
    let mut compile_request = compile::Request::new(pack, output);
    compile_request.options.inputs = request
        .inputs
        .iter()
        .map(|(key, value)| {
            let input = match value {
                Value::Str(text) => Input::Str(text.to_string()),
                Value::Bytes(bytes) => Input::Bytes(bytes.to_vec()),
                other => panic!("unsupported input {other:?}"),
            };
            (key.clone(), input)
        })
        .collect();
    compile_request.options.features = request.features.clone();
    if let Some(today) = request.today {
        compile_request.options.document_time = DocumentTime::Fixed(today);
    }
    typst_pack::compile(&compile_request)
}

/// Every diagnostic fact a caller can observe, with spans resolved.
fn facts(world: &dyn World, diagnostics: &[SourceDiagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let hints = diagnostic.hints.iter().map(|hint| {
                let range = world.range(hint.span);
                format!("{} {:?} {range:?}", hint.v, hint.span.id())
            });
            let trace = diagnostic.trace.iter().map(|point| {
                let range = world.range(point.span);
                format!("{} {range:?}", point.v)
            });
            format!(
                "{:?} {} {:?} {:?} hints={:?} trace={:?}",
                diagnostic.severity,
                diagnostic.message,
                diagnostic.span.id(),
                world.range(diagnostic.span),
                hints.collect::<Vec<_>>(),
                trace.collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn paged(world: &PlainWorld) -> (PagedDocument, Vec<SourceDiagnostic>) {
    let Warned { output, warnings } = typst::compile::<PagedDocument>(world);
    (output.unwrap(), warnings.to_vec())
}

fn page(number: usize) -> NonZeroUsize {
    NonZeroUsize::new(number).unwrap()
}

#[test]
fn svg_with_pages_inputs_and_document_time_matches() {
    let request = dated();
    let output = Output {
        pages: "2,1".parse().unwrap(),
        pretty: true,
        ..Output::new(OutputFormat::Svg)
    };
    let actual = pack_compile(PROJECT, &request, output).unwrap();

    let world = PlainWorld::new(PROJECT, &request);
    let (document, warnings) = paged(&world);
    let options = typst_svg::SvgOptions {
        render_bleed: false,
        pretty: true,
    };
    let expected = document
        .pages()
        .iter()
        .map(|page| typst_svg::svg(page, &options).into_bytes());

    assert_eq!(actual.page_count, Some(2));
    assert_eq!(
        facts(&actual.sources, &actual.warnings),
        facts(&world, &warnings)
    );
    assert!(!warnings.is_empty());
    let pages = actual.artifacts.iter().map(|artifact| artifact.page);
    assert_eq!(pages.collect::<Vec<_>>(), [Some(page(1)), Some(page(2))]);
    let bytes = actual.artifacts.into_iter().map(|artifact| artifact.bytes);
    assert!(bytes.eq(expected));
}

#[test]
fn pdf_with_default_and_explicit_controls_matches() {
    let request = dated();
    let world = PlainWorld::new(PROJECT, &request);
    let (document, _) = paged(&world);

    let defaults = pack_compile(PROJECT, &request, Output::new(OutputFormat::Pdf)).unwrap();
    let expected = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).unwrap();
    assert_eq!(defaults.artifacts[0].bytes, expected);

    let explicit = Output {
        pages: "2".parse().unwrap(),
        pretty: true,
        pdf_identifier: Smart::Custom("parity".into()),
        pdf_creator: Smart::Custom(Some("typst-pack parity".into())),
        pdf_tags: Smart::Custom(false),
        pdf_standards: vec![PdfStandard::A_2b],
        pdf_timestamp: CreationTimestamp::Utc(1_706_933_106),
        ..Output::new(OutputFormat::Pdf)
    };
    let actual = pack_compile(PROJECT, &request, explicit).unwrap();
    let options = typst_pdf::PdfOptions {
        ident: Smart::Custom("parity".into()),
        creator: Smart::Custom(Some("typst-pack parity".into())),
        timestamp: Some(Timestamp::new_utc(
            Datetime::from_ymd_hms(2024, 2, 3, 4, 5, 6).unwrap(),
        )),
        page_ranges: Some(PageRanges::new(vec![Some(page(2))..=Some(page(2))])),
        standards: PdfStandards::new(&[PdfStandard::A_2b]).unwrap(),
        tagged: false,
        pretty: true,
    };
    let expected = typst_pdf::pdf(&document, &options).unwrap();
    assert_eq!(actual.artifacts.len(), 1);
    assert_eq!(actual.artifacts[0].page, None);
    assert_eq!(actual.artifacts[0].bytes, expected);
}

#[test]
fn png_with_ppi_bleed_and_repeated_pages_matches() {
    let request = dated();
    let output = Output {
        pages: "2,1,2".parse().unwrap(),
        ppi: Some(216.0),
        render_bleed: true,
        ..Output::new(OutputFormat::Png)
    };
    let actual = pack_compile(PROJECT, &request, output).unwrap();

    let (document, _) = paged(&PlainWorld::new(PROJECT, &request));
    let options = typst_render::RenderOptions {
        pixel_per_pt: (216.0 / 72.0).into(),
        render_bleed: true,
    };
    let expected = document
        .pages()
        .iter()
        .map(|page| typst_render::render(page, &options).encode_png().unwrap());

    let pages = actual.artifacts.iter().map(|artifact| artifact.page);
    assert_eq!(pages.collect::<Vec<_>>(), [Some(page(1)), Some(page(2))]);
    let bytes = actual.artifacts.into_iter().map(|artifact| artifact.bytes);
    assert!(bytes.eq(expected));
}

#[test]
fn html_compact_and_pretty_match() {
    let project = &[
        (
            "main.typ",
            "#let _ = decimal(1.1)\n#html.div[HTML artifact #include \"fragment.typ\"]",
        ),
        ("fragment.typ", "from a discovered dependency"),
    ];
    let request = Request {
        features: vec![Feature::Html],
        ..Request::default()
    };
    let world = PlainWorld::new(project, &request);
    let Warned { output, warnings } = typst::compile::<typst_html::HtmlDocument>(&world);
    let document = output.unwrap();

    for pretty in [false, true] {
        let output = Output {
            pretty,
            ..Output::new(OutputFormat::Html)
        };
        // The HTML output derives the HTML feature itself.
        let actual = pack_compile(project, &Request::default(), output).unwrap();
        let expected = typst_html::html(&document, &typst_html::HtmlOptions { pretty }).unwrap();
        assert_eq!(actual.page_count, None);
        assert_eq!(
            facts(&actual.sources, &actual.warnings),
            facts(&world, &warnings)
        );
        assert_eq!(actual.artifacts[0].bytes, expected.into_bytes());
    }
}

#[test]
fn byte_inputs_and_features_match() {
    let project = &[(
        "main.typ",
        "#set page(width: 100pt, height: 20pt, margin: 0pt)\n\
         #assert.eq(type(sys.inputs.data), bytes)\n\
         #rect(width: (str(sys.inputs.data).len() + sys.inputs.label.len()) * 1pt)\n\
         #pdf.table-summary(table(columns: 1, [feature enabled]))",
    )];
    let request = Request {
        inputs: vec![
            ("label".into(), Value::Str("text".into())),
            ("data".into(), Value::Bytes(Bytes::new(b"bytes".to_vec()))),
        ],
        features: vec![Feature::A11yExtras],
        today: None,
    };
    let actual = pack_compile(project, &request, Output::new(OutputFormat::Svg)).unwrap();
    let (document, _) = paged(&PlainWorld::new(project, &request));
    let options = typst_svg::SvgOptions::default();
    let expected = typst_svg::svg(&document.pages()[0], &options);
    assert_eq!(actual.artifacts[0].bytes, expected.into_bytes());
}

#[test]
fn failed_compilation_diagnostics_match() {
    // Without a document time the package's `datetime.today()` fails, after a
    // warning, with a trace back into the project.
    let request = Request {
        today: None,
        ..dated()
    };
    let Err(compile::Error::Failed {
        errors,
        warnings,
        sources,
    }) = pack_compile(PROJECT, &request, Output::new(OutputFormat::Svg))
    else {
        panic!("expected a failed compilation");
    };

    let world = PlainWorld::new(PROJECT, &request);
    let Warned {
        output,
        warnings: expected_warnings,
    } = typst::compile::<PagedDocument>(&world);
    let expected_errors = output.err().unwrap();

    assert_eq!(
        facts(&sources, &warnings),
        facts(&world, &expected_warnings)
    );
    assert_eq!(facts(&sources, &errors), facts(&world, &expected_errors));
    assert_eq!(errors[0].trace.len(), 1);
}
