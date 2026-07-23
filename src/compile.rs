//! Compiling a pack into Compilation Output Artifacts.

use std::num::NonZeroUsize;

use ecow::EcoVec;
#[cfg(feature = "cli")]
use rayon::prelude::*;
use typst::World;
use typst::diag::{SourceDiagnostic, Warned};
use typst::syntax::Span;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

use crate::embedded::EmbeddedTypst;

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
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Pixels per inch for PNG output. Defaults to 144.
    pub ppi: Option<f64>,
    /// Whether to pretty-print HTML, SVG, and PDF output.
    pub pretty: bool,
    /// PDF standards to enforce.
    pub pdf_standards: PdfStandards,
    /// Whether PDF accessibility tags should be emitted when possible.
    pub pdf_tags: bool,
    /// How the document creation datetime is recorded in PDF metadata.
    pub creation_timestamp: CreationTimestamp,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            page_selection: PageSelection::default(),
            ppi: None,
            pretty: false,
            pdf_standards: PdfStandards::default(),
            pdf_tags: true,
            creation_timestamp: CreationTimestamp::Automatic,
        }
    }
}

/// The source of the document creation datetime recorded in PDF metadata.
#[derive(Debug, Clone, Copy, Default)]
pub enum CreationTimestamp {
    /// Derive the timestamp from the world's `today`.
    #[default]
    Automatic,
    /// Record an explicit UTC timestamp.
    Explicit(Timestamp),
    /// Omit creation datetime metadata without falling back to the world.
    Omit,
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
            let parse = |value: &str| -> Result<NonZeroUsize, String> {
                if value == "0" {
                    Err("page numbers start at one".to_owned())
                } else {
                    value
                        .parse::<NonZeroUsize>()
                        .map_err(|_| format!("`{value}` is not a valid page number"))
                }
            };
            match part
                .split('-')
                .map(str::trim)
                .collect::<Vec<_>>()
                .as_slice()
            {
                [] | [""] => Err("page export range must not be empty".to_owned()),
                [single] => {
                    let page = parse(single)?;
                    Ok(Some(page)..=Some(page))
                }
                ["", ""] => Err("page export range must have start or end".to_owned()),
                [start, ""] => Ok(Some(parse(start)?)..=None),
                ["", end] => Ok(None..=Some(parse(end)?)),
                [start, end] => {
                    let start = parse(start)?;
                    let end = parse(end)?;
                    if start > end {
                        Err("page export range must end at a page after the start".to_owned())
                    } else {
                        Ok(Some(start)..=Some(end))
                    }
                }
                _ => Err("page export range must have a single hyphen".to_owned()),
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
    source_page_count: Option<usize>,
}

impl CompilationOutput {
    /// Total pages in the source document before page selection.
    ///
    /// HTML output is unpaged and returns `None`.
    pub fn source_page_count(&self) -> Option<usize> {
        self.source_page_count
    }
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
    compile_with_default_pdf_timestamp(world, format, options, || {
        world.today(None).map(Timestamp::new_utc)
    })
}

pub(crate) fn compile_with_default_pdf_timestamp(
    world: &dyn World,
    format: OutputFormat,
    options: &CompileOptions,
    default_pdf_timestamp: impl FnOnce() -> Option<Timestamp>,
) -> Result<CompilationOutput, CompileError> {
    let _compilation_timing = typst_timing::TimingScope::new("typst-pack compilation");
    if format == OutputFormat::Html {
        let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
        let document = output.map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
        })?;
        let _export_timing = typst_timing::TimingScope::new("export");
        let bytes = EmbeddedTypst::export_html(
            &document,
            &typst_html::HtmlOptions {
                pretty: options.pretty,
            },
        )
        .map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
        })?;
        return Ok(CompilationOutput {
            artifacts: vec![CompilationArtifact {
                format,
                bytes,
                source_page_number: None,
            }],
            warnings,
            source_page_count: None,
        });
    }

    let Warned {
        output,
        warnings: compile_warnings,
    } = EmbeddedTypst::compile_paged(world);
    let mut warnings = compile_warnings;
    if format == OutputFormat::Pdf
        && !options.page_selection.ranges().is_empty()
        && options.pdf_tags
    {
        warnings.push(
            SourceDiagnostic::warning(Span::detached(), "using --pages implies --no-pdf-tags")
                .with_hints([
                    "the resulting PDF will be inaccessible".into(),
                    "add --no-pdf-tags to silence this warning".into(),
                ]),
        );
    }
    let document = output.map_err(|errors| CompileError::Diagnostics {
        errors,
        warnings: warnings.clone(),
    })?;
    let artifacts = {
        let _export_timing = typst_timing::TimingScope::new("export");
        match format {
            OutputFormat::Pdf => {
                let timestamp = match options.creation_timestamp {
                    CreationTimestamp::Automatic => default_pdf_timestamp(),
                    CreationTimestamp::Explicit(timestamp) => Some(timestamp),
                    CreationTimestamp::Omit => None,
                };
                let pdf_options = PdfOptions {
                    timestamp,
                    page_ranges: options.page_selection.typst_page_ranges(),
                    standards: options.pdf_standards.clone(),
                    tagged: options.pdf_tags && options.page_selection.ranges().is_empty(),
                    pretty: options.pretty,
                    ..Default::default()
                };
                let pdf = EmbeddedTypst::export_pdf(&document, &pdf_options).map_err(|errors| {
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
                    pixel_per_pt: (ppi / 72.0).into(),
                    ..Default::default()
                };
                let pages = selected_pages(&document, options).collect::<Vec<_>>();
                let export = |(source_page_number, page)| {
                    let bytes =
                        EmbeddedTypst::export_png(page, &render_options).map_err(|message| {
                            CompileError::PngExport {
                                message,
                                warnings: warnings.clone(),
                            }
                        })?;
                    Ok::<_, CompileError>(CompilationArtifact {
                        format,
                        bytes,
                        source_page_number: Some(source_page_number),
                    })
                };
                #[cfg(feature = "cli")]
                let artifacts = pages
                    .into_par_iter()
                    .map(export)
                    .collect::<Result<Vec<_>, _>>()?;
                #[cfg(not(feature = "cli"))]
                let artifacts = pages
                    .into_iter()
                    .map(export)
                    .collect::<Result<Vec<_>, _>>()?;
                artifacts
            }
            OutputFormat::Svg => {
                let svg_options = typst_svg::SvgOptions {
                    render_bleed: false,
                    pretty: options.pretty,
                };
                let pages = selected_pages(&document, options).collect::<Vec<_>>();
                let export = |(source_page_number, page)| CompilationArtifact {
                    format,
                    bytes: EmbeddedTypst::export_svg(page, &svg_options),
                    source_page_number: Some(source_page_number),
                };
                #[cfg(feature = "cli")]
                let artifacts = pages.into_par_iter().map(export).collect();
                #[cfg(not(feature = "cli"))]
                let artifacts = pages.into_iter().map(export).collect();
                artifacts
            }
            OutputFormat::Html => unreachable!("handled above"),
        }
    };
    Ok(CompilationOutput {
        artifacts,
        warnings,
        source_page_count: Some(document.pages().len()),
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
