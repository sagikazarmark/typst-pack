//! Compiling a pack into output documents.

use std::num::NonZeroUsize;

use ecow::EcoVec;
use typst::World;
use typst::diag::{SourceDiagnostic, Warned};
use typst_html::HtmlDocument;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

/// The output formats a pack can be compiled to.
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
    /// Which pages to export. All pages if empty.
    ///
    /// Ranges are one-indexed and inclusive, with open ends allowed, matching
    /// `typst compile --pages`.
    pub pages: Vec<PageRange>,
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

/// Parses a `--pages`-style page selection like `1,3-5,9-`.
pub fn parse_pages(text: &str) -> Result<Vec<PageRange>, String> {
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
        .collect()
}

/// The result of compiling a pack.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// The produced format.
    pub format: OutputFormat,
    /// The output documents: exactly one buffer for PDF, one buffer per
    /// exported page for PNG and SVG.
    pub outputs: Vec<Vec<u8>>,
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
    #[error("PNG encoding failed: {0}")]
    PngEncoding(String),
}

/// Compiles the world's document and exports it in the requested format.
///
/// This works with any [`World`], but is intended for
/// [`PackWorld`](crate::PackWorld).
pub fn compile(
    world: &dyn World,
    format: OutputFormat,
    options: &CompileOptions,
) -> Result<CompileOutput, CompileError> {
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
        return Ok(CompileOutput {
            format,
            outputs: vec![html.into_bytes()],
            warnings,
        });
    }

    let Warned { output, warnings } = typst::compile::<PagedDocument>(world);
    let document = output.map_err(|errors| CompileError::Diagnostics {
        errors,
        warnings: warnings.clone(),
    })?;

    let outputs = match format {
        OutputFormat::Pdf => {
            let timestamp = options
                .creation_timestamp
                .or_else(|| world.today(None).map(Timestamp::new_utc));
            let pdf_options = PdfOptions {
                timestamp,
                page_ranges: page_ranges(options),
                standards: options.pdf_standards.clone(),
                ..Default::default()
            };
            let pdf = typst_pdf::pdf(&document, &pdf_options).map_err(|errors| {
                CompileError::Diagnostics {
                    errors,
                    warnings: warnings.clone(),
                }
            })?;
            vec![pdf]
        }
        OutputFormat::Png => {
            let ppi = options.ppi.unwrap_or(144.0);
            let render_options = typst_render::RenderOptions {
                pixel_per_pt: (f64::from(ppi) / 72.0).into(),
                ..Default::default()
            };
            selected_pages(&document, options)
                .map(|page| {
                    typst_render::render(page, &render_options)
                        .encode_png()
                        .map_err(|err| CompileError::PngEncoding(err.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        OutputFormat::Svg => {
            let svg_options = typst_svg::SvgOptions::default();
            selected_pages(&document, options)
                .map(|page| typst_svg::svg(page, &svg_options).into_bytes())
                .collect()
        }
        OutputFormat::Html => unreachable!("handled above"),
    };

    Ok(CompileOutput {
        format,
        outputs,
        warnings,
    })
}

fn page_ranges(options: &CompileOptions) -> Option<typst::layout::PageRanges> {
    (!options.pages.is_empty()).then(|| typst::layout::PageRanges::new(options.pages.clone()))
}

fn selected_pages<'a>(
    document: &'a PagedDocument,
    options: &'a CompileOptions,
) -> impl Iterator<Item = &'a typst_layout::Page> {
    let ranges = page_ranges(options);
    document
        .pages()
        .iter()
        .enumerate()
        .filter(move |(index, _)| {
            ranges.as_ref().is_none_or(|ranges| {
                NonZeroUsize::new(index + 1).is_some_and(|number| ranges.includes_page(number))
            })
        })
        .map(|(_, page)| page)
}
