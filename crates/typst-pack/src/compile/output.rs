//! What a compilation produces: output formats and their controls.

use std::num::NonZeroUsize;

use typst::foundations::{Datetime, Smart};
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use super::RequestIssue;
use crate::identity::Preimage;

/// The formats a pack can be compiled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Pdf,
    Png,
    Svg,
    /// HTML export is experimental in Typst. Compiling to HTML enables the
    /// required [`Feature::Html`](typst::Feature::Html).
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

    /// The format a file extension names, compared case-insensitively.
    pub fn from_extension(extension: &str) -> Option<Self> {
        [Self::Pdf, Self::Png, Self::Svg, Self::Html]
            .into_iter()
            .find(|format| extension.eq_ignore_ascii_case(format.extension()))
    }
}

/// The output a compilation produces, with its format's controls.
#[derive(Debug, Clone)]
pub enum Output {
    Pdf(PdfOutput),
    Png(PngOutput),
    Svg(SvgOutput),
    Html(HtmlOutput),
}

impl Output {
    /// The output format with default controls.
    pub fn new(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Pdf => Self::Pdf(PdfOutput::default()),
            OutputFormat::Png => Self::Png(PngOutput::default()),
            OutputFormat::Svg => Self::Svg(SvgOutput::default()),
            OutputFormat::Html => Self::Html(HtmlOutput::default()),
        }
    }

    /// The format this output produces.
    pub fn format(&self) -> OutputFormat {
        match self {
            Self::Pdf(_) => OutputFormat::Pdf,
            Self::Png(_) => OutputFormat::Png,
            Self::Svg(_) => OutputFormat::Svg,
            Self::Html(_) => OutputFormat::Html,
        }
    }
}

/// Controls for PDF output.
#[derive(Debug, Clone)]
pub struct PdfOutput {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// PDF standards the exporter enforces.
    pub standards: Vec<PdfStandard>,
    /// The PDF file identifier; automatic by default.
    pub identifier: Smart<String>,
    /// The PDF creator metadata; automatic by default.
    pub creator: Smart<Option<String>>,
    /// Whether accessibility tags are written.
    ///
    /// Automatic tagging is disabled with a warning for a page subset, as in
    /// Typst's CLI. Explicit tagging is passed to the exporter.
    pub tags: Smart<bool>,
    /// The creation datetime recorded in PDF metadata.
    pub creation_timestamp: CreationTimestamp,
    /// Whether to pretty-print the PDF.
    pub pretty: bool,
}

impl Default for PdfOutput {
    fn default() -> Self {
        let pdf = PdfOptions::default();
        Self {
            page_selection: PageSelection::default(),
            standards: vec![],
            identifier: pdf.ident,
            creator: pdf.creator,
            tags: Smart::Auto,
            creation_timestamp: CreationTimestamp::Omit,
            pretty: pdf.pretty,
        }
    }
}

/// Controls for PNG output, one artifact per selected page.
#[derive(Debug, Clone, Default)]
pub struct PngOutput {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Pixels per inch; `None` selects Typst's default of 144.
    pub pixels_per_inch: Option<f64>,
    /// Whether to render into the page bleed region.
    pub render_bleed: bool,
}

/// Controls for SVG output, one artifact per selected page.
#[derive(Debug, Clone, Default)]
pub struct SvgOutput {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Whether to render into the page bleed region.
    pub render_bleed: bool,
    /// Whether to pretty-print the SVG.
    pub pretty: bool,
}

/// Controls for HTML output.
#[derive(Debug, Clone, Default)]
pub struct HtmlOutput {
    /// Whether to pretty-print the HTML.
    pub pretty: bool,
}

/// The creation datetime recorded in PDF metadata.
///
/// As in Typst's CLI, an instant whose datetime Typst cannot represent (such
/// as one past the year 9999) or an offset beyond ±23:59 records nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CreationTimestamp {
    /// Record no creation datetime.
    #[default]
    Omit,
    /// The instant `unix` seconds after the UNIX epoch, recorded in UTC.
    Utc(i64),
    /// The instant `unix` seconds after the UNIX epoch, recorded as local time
    /// at a UTC offset of `offset_minutes`.
    Local { unix: i64, offset_minutes: i32 },
}

impl CreationTimestamp {
    /// The exporter timestamp, if it can be represented.
    pub(crate) fn to_pdf(self) -> Option<Timestamp> {
        match self {
            Self::Omit => None,
            Self::Utc(unix) => datetime_at(unix, 0).map(Timestamp::new_utc),
            Self::Local {
                unix,
                offset_minutes,
            } => datetime_at(unix, i64::from(offset_minutes) * 60)
                .and_then(|at| Timestamp::new_local(at, offset_minutes)),
        }
    }
}

/// The civil datetime `unix + offset` seconds after the epoch.
fn datetime_at(unix: i64, offset: i64) -> Option<Datetime> {
    let seconds = unix.checked_add(offset)?;
    let (days, time) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Howard Hinnant's days-to-civil conversion.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    Datetime::from_ymd_hms(
        i32::try_from(year).ok()?,
        month as u8,
        day as u8,
        (time / 3600) as u8,
        (time % 3600 / 60) as u8,
        (time % 60) as u8,
    )
}

/// A one-indexed, inclusive page range with optional open ends.
pub type PageRange = std::ops::RangeInclusive<Option<NonZeroUsize>>;

/// A selection of one-indexed source page ranges.
///
/// An empty range collection selects all source pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PageSelection {
    ranges: Vec<PageRange>,
}

impl PageSelection {
    /// Selects all source pages.
    pub fn all() -> Self {
        Self::default()
    }

    /// Selects the union of the given ranges; none selects all pages.
    pub fn new(ranges: Vec<PageRange>) -> Self {
        Self { ranges }
    }

    /// The selected source page ranges.
    pub fn ranges(&self) -> &[PageRange] {
        &self.ranges
    }

    pub(crate) fn typst_page_ranges(&self) -> Option<typst::layout::PageRanges> {
        (!self.ranges.is_empty()).then(|| typst::layout::PageRanges::new(self.ranges.clone()))
    }

    /// Sorts and merges the ranges, reporting any that end before they start.
    fn normalize(&mut self, issues: &mut Vec<RequestIssue>) {
        let mut ranges = Vec::<(usize, usize)>::new();
        for range in &self.ranges {
            let start = range.start().map_or(1, NonZeroUsize::get);
            let end = range.end().map_or(usize::MAX, NonZeroUsize::get);
            if start > end {
                issues.push(RequestIssue::InvalidPageRange {
                    start: range.start().expect("a reversed range has a start"),
                    end: range.end().expect("a reversed range has an end"),
                });
            } else {
                ranges.push((start, end));
            }
        }
        ranges.sort_unstable();
        let mut merged = Vec::<(usize, usize)>::new();
        for (start, end) in ranges {
            match merged.last_mut() {
                Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        if !self.ranges.is_empty() {
            self.ranges = merged
                .into_iter()
                .map(|(start, end)| {
                    let end = (end != usize::MAX)
                        .then(|| NonZeroUsize::new(end))
                        .flatten();
                    NonZeroUsize::new(start)..=end
                })
                .collect();
        }
    }

    fn write(&self, preimage: &mut Preimage) {
        preimage.count(self.ranges.len());
        for range in &self.ranges {
            preimage
                .count(range.start().map_or(1, NonZeroUsize::get))
                .int(range.end().map_or(-1, |end| end.get() as i64));
        }
    }
}

/// Parses a textual page selection like `1,3-5,9-`.
pub fn parse_page_selection(text: &str) -> Result<PageSelection, String> {
    let page = |value: &str| -> Result<NonZeroUsize, String> {
        if value == "0" {
            return Err("page numbers start at one".to_owned());
        }
        value
            .parse::<NonZeroUsize>()
            .map_err(|_| format!("`{value}` is not a valid page number"))
    };
    text.split(',')
        .map(
            |part| match part.trim().split('-').map(str::trim).collect::<Vec<_>>()[..] {
                [""] => Err("page export range must not be empty".to_owned()),
                [single] => page(single).map(|page| Some(page)..=Some(page)),
                ["", ""] => Err("page export range must have start or end".to_owned()),
                [start, ""] => Ok(Some(page(start)?)..=None),
                ["", end] => Ok(None..=Some(page(end)?)),
                [start, end] => {
                    let (start, end) = (page(start)?, page(end)?);
                    if start > end {
                        Err("page export range must end at a page after the start".to_owned())
                    } else {
                        Ok(Some(start)..=Some(end))
                    }
                }
                _ => Err("page export range must have a single hyphen".to_owned()),
            },
        )
        .collect::<Result<Vec<_>, _>>()
        .map(PageSelection::new)
}

/// Resource ceilings for compilation output.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompilationLimits {
    /// Maximum number of pages in the compiled document.
    pub max_pages: u64,
    /// Maximum number of exported artifacts.
    pub max_artifacts: u64,
    /// Maximum number of pixels in one PNG artifact.
    pub max_pixels_per_artifact: u64,
    /// Maximum number of pixels across all PNG artifacts.
    pub max_total_pixels: u64,
    /// Maximum size of one artifact.
    pub max_artifact_bytes: u64,
    /// Maximum combined size of all artifacts.
    pub max_total_artifact_bytes: u64,
    /// Number of pages exported concurrently; zero is treated as one.
    pub export_workers: u64,
}

impl Default for CompilationLimits {
    fn default() -> Self {
        Self {
            max_pages: 10_000,
            max_artifacts: 10_000,
            max_pixels_per_artifact: 100_000_000,
            max_total_pixels: 1_000_000_000,
            max_artifact_bytes: 512 * 1024 * 1024,
            max_total_artifact_bytes: 2 * 1024 * 1024 * 1024,
            export_workers: 4,
        }
    }
}

/// An output whose defaults are resolved and whose values are validated.
pub(crate) struct Normalized {
    pub(crate) output: Output,
    /// The exporter's PDF creation timestamp.
    pub(crate) pdf_timestamp: Option<Timestamp>,
    /// The validated PDF standards.
    pub(crate) pdf_standards: Option<PdfStandards>,
    /// Whether automatic PDF tagging was disabled because pages are selected.
    pub(crate) untagged_by_selection: bool,
}

impl Output {
    /// Resolves defaults and canonical forms, reporting invalid values.
    pub(crate) fn normalize(&self, issues: &mut Vec<RequestIssue>) -> Normalized {
        let mut output = self.clone();
        let mut normalized = Normalized {
            output: Output::Html(HtmlOutput::default()),
            pdf_timestamp: None,
            pdf_standards: None,
            untagged_by_selection: false,
        };
        match &mut output {
            Output::Pdf(pdf) => {
                pdf.page_selection.normalize(issues);
                pdf.standards.sort_by_key(standard_name);
                match PdfStandards::new(&pdf.standards) {
                    Ok(standards) => normalized.pdf_standards = Some(standards),
                    Err(error) => issues.push(RequestIssue::InvalidPdfStandards {
                        message: error.message().to_string(),
                        hints: error.hints().iter().map(ToString::to_string).collect(),
                    }),
                }
                let default_tagged = PdfOptions::default().tagged;
                let selected = !pdf.page_selection.ranges().is_empty();
                normalized.untagged_by_selection = selected && pdf.tags.is_auto() && default_tagged;
                if pdf.tags.is_auto() {
                    pdf.tags = Smart::Custom(default_tagged && !selected);
                }
                normalized.pdf_timestamp = pdf.creation_timestamp.to_pdf();
            }
            Output::Png(png) => {
                png.page_selection.normalize(issues);
                let ppi = png.pixels_per_inch.get_or_insert_with(|| {
                    typst_render::RenderOptions::default().pixel_per_pt.get() * 72.0
                });
                if !ppi.is_finite() || *ppi <= 0.0 {
                    issues.push(RequestIssue::InvalidPpi);
                }
            }
            Output::Svg(svg) => svg.page_selection.normalize(issues),
            Output::Html(_) => {}
        }
        normalized.output = output;
        normalized
    }

    /// Writes a normalized output to the compilation identity preimage.
    pub(crate) fn write(&self, preimage: &mut Preimage) {
        match self {
            Output::Pdf(pdf) => {
                preimage.str("pdf");
                pdf.page_selection.write(preimage);
                preimage.count(pdf.standards.len());
                for standard in &pdf.standards {
                    preimage.str(standard_name(standard));
                }
                match &pdf.identifier {
                    Smart::Auto => preimage.str("auto"),
                    Smart::Custom(identifier) => preimage.str("custom").str(identifier),
                };
                match &pdf.creator {
                    Smart::Auto => preimage.str("auto"),
                    Smart::Custom(None) => preimage.str("none"),
                    Smart::Custom(Some(creator)) => preimage.str("custom").str(creator),
                };
                preimage.bool(matches!(pdf.tags, Smart::Custom(true)));
                match pdf.creation_timestamp {
                    CreationTimestamp::Omit => preimage.str("omit"),
                    CreationTimestamp::Utc(unix) => preimage.str("utc").int(unix),
                    CreationTimestamp::Local {
                        unix,
                        offset_minutes,
                    } => preimage
                        .str("local")
                        .int(unix)
                        .int(i64::from(offset_minutes)),
                };
                preimage.bool(pdf.pretty);
            }
            Output::Png(png) => {
                preimage.str("png");
                png.page_selection.write(preimage);
                preimage
                    .int(png.pixels_per_inch.unwrap_or_default().to_bits() as i64)
                    .bool(png.render_bleed);
            }
            Output::Svg(svg) => {
                preimage.str("svg");
                svg.page_selection.write(preimage);
                preimage.bool(svg.render_bleed).bool(svg.pretty);
            }
            Output::Html(html) => {
                preimage.str("html").bool(html.pretty);
            }
        }
    }
}

/// The canonical name of a PDF standard, as Typst's CLI spells it.
fn standard_name(standard: &PdfStandard) -> &'static str {
    match standard {
        PdfStandard::V_1_4 => "1.4",
        PdfStandard::V_1_5 => "1.5",
        PdfStandard::V_1_6 => "1.6",
        PdfStandard::V_1_7 => "1.7",
        PdfStandard::V_2_0 => "2.0",
        PdfStandard::A_1b => "a-1b",
        PdfStandard::A_1a => "a-1a",
        PdfStandard::A_2b => "a-2b",
        PdfStandard::A_2u => "a-2u",
        PdfStandard::A_2a => "a-2a",
        PdfStandard::A_3b => "a-3b",
        PdfStandard::A_3u => "a-3u",
        PdfStandard::A_3a => "a-3a",
        PdfStandard::A_4 => "a-4",
        PdfStandard::A_4f => "a-4f",
        PdfStandard::A_4e => "a-4e",
        PdfStandard::Ua_1 => "ua-1",
        _ => unreachable!("every standard of the pinned typst-pdf is named"),
    }
}
