//! Compiling a pack into Compilation Output Artifacts.

use std::num::NonZeroUsize;

use ecow::EcoVec;
use typst::World;
use typst::diag::{SourceDiagnostic, Warned};
use typst::syntax::Span;
use typst_html::HtmlDocument;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

/// The Document Formats and Page Formats a pack can be compiled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pdf,
    Png,
    Svg,
    /// HTML export is experimental in Typst; compiling to it requires a world
    /// whose library has [`Feature::Html`](typst::Feature::Html) enabled
    /// (see [`PackWorldBuilder::feature`](crate::PackWorldBuilder::feature)),
    /// otherwise compilation errors.
    Html,
}

impl OutputFormat {
    /// The conventional file extension for this format.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Html => "html",
        }
    }
}

/// Options for [`compile`].
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Pixels per inch for PNG output. Defaults to 144.
    pub ppi: Option<f32>,
    /// PDF standards to enforce.
    pub pdf_standards: PdfStandards,
    /// The document creation datetime recorded in PDF metadata. When `None`,
    /// the date is derived from the world's `today`.
    pub creation_timestamp: Option<Timestamp>,
}

/// A one-indexed, inclusive page range with optional open ends.
pub type PageRange = std::ops::RangeInclusive<Option<NonZeroUsize>>;

/// A selection of one-indexed source page ranges.
///
/// An empty range collection selects all source pages. Ranges are inclusive
/// and may have open ends.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageSelection {
    ranges: Vec<PageRange>,
}

impl PageSelection {
    /// Selects all source pages.
    pub fn all() -> Self {
        Self::default()
    }

    /// Selects the union of the given source page ranges.
    ///
    /// An empty collection selects all source pages.
    pub fn new(ranges: Vec<PageRange>) -> Self {
        Self { ranges }
    }

    /// The selected source page ranges.
    pub fn ranges(&self) -> &[PageRange] {
        &self.ranges
    }

    fn typst_page_ranges(&self) -> Option<typst::layout::PageRanges> {
        (!self.ranges.is_empty()).then(|| typst::layout::PageRanges::new(self.ranges.clone()))
    }
}

/// Parses a textual page selection like `1,3-5,9-`.
pub fn parse_page_selection(text: &str) -> Result<PageSelection, String> {
    text.split(',')
        .map(|part| {
            let part = part.trim();
            let parse = |s: &str| -> Result<Option<NonZeroUsize>, String> {
                if s.is_empty() {
                    return Ok(None);
                }
                s.parse::<NonZeroUsize>()
                    .map(Some)
                    .map_err(|_| format!("invalid page number `{s}`"))
            };
            match part.split_once('-') {
                Some((start, end)) => Ok(parse(start)?..=parse(end)?),
                None => {
                    let page = parse(part)?.ok_or_else(|| "empty page range".to_owned())?;
                    Ok(Some(page)..=Some(page))
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(PageSelection::new)
}

/// One file produced by compiling a pack.
#[derive(Debug, Clone)]
pub struct CompilationArtifact {
    format: OutputFormat,
    bytes: Vec<u8>,
    source_page_number: Option<NonZeroUsize>,
}

impl CompilationArtifact {
    /// The format of this artifact.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// The one-based physical source page for a Page Format artifact.
    ///
    /// Document Format artifacts have no single Source Page Number.
    pub fn source_page_number(&self) -> Option<NonZeroUsize> {
        self.source_page_number
    }

    /// Borrows the artifact bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Extracts the owned artifact bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// The result of compiling a pack.
#[derive(Debug, Clone)]
pub struct CompilationOutput {
    /// The produced Compilation Output Artifacts.
    pub artifacts: Vec<CompilationArtifact>,
    /// Warnings emitted during compilation.
    pub warnings: EcoVec<SourceDiagnostic>,
}

/// A failed compilation.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// Compilation or export produced errors; warnings are included for
    /// complete reporting.
    #[error("compilation failed with {} error(s)", errors.len())]
    Diagnostics {
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    /// A paged compilation selected no source pages.
    #[error("page selection matched no source pages")]
    NoMatchingSourcePages {
        /// Warnings emitted before page selection was validated.
        warnings: EcoVec<SourceDiagnostic>,
    },
    /// PNG export failed after compilation completed.
    #[error("PNG export failed: {message}")]
    PngExport {
        message: String,
        /// Warnings emitted before PNG export failed.
        warnings: EcoVec<SourceDiagnostic>,
    },
}

/// Compiles the world's document and exports it in the requested format.
///
/// This works with any [`World`], but is intended for
/// [`PackWorld`](crate::PackWorld).
pub fn compile(
    world: &dyn World,
    format: OutputFormat,
    options: &CompileOptions,
) -> Result<CompilationOutput, CompileError> {
    if format == OutputFormat::Html {
        let Warned { output, warnings } = typst::compile::<HtmlDocument>(world);
        let document = output.map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
        })?;
        let html =
            typst_html::html(&document, &typst_html::HtmlOptions::default()).map_err(|errors| {
                CompileError::Diagnostics {
                    errors,
                    warnings: warnings.clone(),
                }
            })?;
        return Ok(CompilationOutput {
            artifacts: vec![CompilationArtifact {
                format,
                bytes: html.into_bytes(),
                source_page_number: None,
            }],
            warnings,
        });
    }

    let Warned {
        output,
        mut warnings,
    } = typst::compile::<PagedDocument>(world);
    let document = output.map_err(|errors| CompileError::Diagnostics {
        errors,
        warnings: warnings.clone(),
    })?;
    if format == OutputFormat::Pdf && !options.page_selection.ranges().is_empty() {
        warnings.push(SourceDiagnostic::warning(
            Span::detached(),
            "using page selection disables PDF tags",
        ));
    }
    if selected_pages(&document, options).next().is_none() {
        return Err(CompileError::NoMatchingSourcePages { warnings });
    }

    let artifacts = match format {
        OutputFormat::Pdf => {
            let timestamp = options
                .creation_timestamp
                .or_else(|| world.today(None).map(Timestamp::new_utc));
            let pdf_options = PdfOptions {
                timestamp,
                page_ranges: options.page_selection.typst_page_ranges(),
                standards: options.pdf_standards.clone(),
                tagged: options.page_selection.ranges().is_empty(),
                ..Default::default()
            };
            let pdf = typst_pdf::pdf(&document, &pdf_options).map_err(|errors| {
                CompileError::Diagnostics {
                    errors,
                    warnings: warnings.clone(),
                }
            })?;
            vec![CompilationArtifact {
                format,
                bytes: pdf,
                source_page_number: None,
            }]
        }
        OutputFormat::Png => {
            let ppi = options.ppi.unwrap_or(144.0);
            let render_options = typst_render::RenderOptions {
                pixel_per_pt: (f64::from(ppi) / 72.0).into(),
                ..Default::default()
            };
            selected_pages(&document, options)
                .map(|(source_page_number, page)| {
                    let bytes = typst_render::render(page, &render_options)
                        .encode_png()
                        .map_err(|err| CompileError::PngExport {
                            message: err.to_string(),
                            warnings: warnings.clone(),
                        })?;
                    Ok(CompilationArtifact {
                        format,
                        bytes,
                        source_page_number: Some(source_page_number),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        OutputFormat::Svg => {
            let svg_options = typst_svg::SvgOptions::default();
            selected_pages(&document, options)
                .map(|(source_page_number, page)| CompilationArtifact {
                    format,
                    bytes: typst_svg::svg(page, &svg_options).into_bytes(),
                    source_page_number: Some(source_page_number),
                })
                .collect()
        }
        OutputFormat::Html => unreachable!("handled above"),
    };

    Ok(CompilationOutput {
        artifacts,
        warnings,
    })
}

fn selected_pages<'a>(
    document: &'a PagedDocument,
    options: &'a CompileOptions,
) -> impl Iterator<Item = (NonZeroUsize, &'a typst_layout::Page)> {
    let ranges = options.page_selection.typst_page_ranges();
    document
        .pages()
        .iter()
        .enumerate()
        .filter(move |(index, _)| {
            ranges.as_ref().is_none_or(|ranges| {
                NonZeroUsize::new(index + 1).is_some_and(|number| ranges.includes_page(number))
            })
        })
        .map(|(index, page)| (NonZeroUsize::new(index + 1).unwrap(), page))
}
