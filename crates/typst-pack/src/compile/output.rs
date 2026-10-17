//! What a compilation produces: the output format and its controls.

use std::num::NonZeroUsize;
use std::str::FromStr;

use typst::foundations::{Datetime, Smart};
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use super::RequestIssue;
use crate::digest::Preimage;

/// The formats a pack can be compiled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// One PDF document.
    Pdf,
    /// One PNG image per page.
    Png,
    /// One SVG image per page.
    Svg,
    /// One HTML document. HTML export is experimental in Typst and enables
    /// [`Feature::Html`](typst::Feature::Html).
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

/// The output a compilation produces, and its controls.
///
/// Controls that do not apply to the format are ignored.
#[derive(Debug, Clone)]
pub struct Output {
    /// The format to export.
    pub format: OutputFormat,
    /// Which source pages to export (PDF, PNG, SVG).
    pub pages: PageSelection,
    /// Whether to pretty-print the output (PDF, SVG, HTML).
    pub pretty: bool,
    /// Pixels per inch (PNG); `None` selects Typst's default of 144.
    pub ppi: Option<f64>,
    /// Whether to render into the page bleed region (PNG, SVG).
    pub render_bleed: bool,
    /// The PDF standards the exporter enforces.
    pub pdf_standards: Vec<PdfStandard>,
    /// The PDF file identifier; automatic by default.
    pub pdf_identifier: Smart<String>,
    /// The PDF creator metadata; automatic by default.
    pub pdf_creator: Smart<Option<String>>,
    /// Whether PDF accessibility tags are written.
    ///
    /// Automatic tagging is disabled with a warning for a page subset, as in
    /// Typst's CLI. Explicit tagging is passed to the exporter.
    pub pdf_tags: Smart<bool>,
    /// The creation datetime recorded in PDF metadata.
    pub pdf_timestamp: CreationTimestamp,
}

impl Output {
    /// The format with default controls.
    pub fn new(format: OutputFormat) -> Self {
        let pdf = PdfOptions::default();
        Self {
            format,
            pages: PageSelection::default(),
            pretty: false,
            ppi: None,
            render_bleed: false,
            pdf_standards: Vec::new(),
            pdf_identifier: pdf.ident,
            pdf_creator: pdf.creator,
            pdf_tags: Smart::Auto,
            pdf_timestamp: CreationTimestamp::Omit,
        }
    }
}

/// The creation datetime recorded in PDF metadata.
///
/// As in Typst's CLI, an instant whose datetime Typst cannot represent (such
/// as one past the year 9999) records nothing. A UTC offset of a day or more
/// is an invalid request, as for [`DocumentTime`](super::DocumentTime).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CreationTimestamp {
    /// Record no creation datetime.
    #[default]
    Omit,
    /// The instant this many seconds after the UNIX epoch, recorded in UTC.
    Utc(i64),
    /// The instant `seconds` after the UNIX epoch, recorded as local time at
    /// a UTC offset of `utc_offset_seconds`, as
    /// [`DocumentTime::Local`](super::DocumentTime::Local) describes it. PDF
    /// records the offset in whole minutes, so seconds beyond a minute are
    /// dropped from the recorded offset (not from the local time), as in
    /// Typst's CLI.
    Local {
        seconds: i64,
        utc_offset_seconds: i32,
    },
}

impl CreationTimestamp {
    /// The exporter timestamp, if it can be represented.
    fn to_pdf(self) -> Option<Timestamp> {
        match self {
            Self::Omit => None,
            Self::Utc(unix) => datetime_at(unix, 0).map(Timestamp::new_utc),
            Self::Local {
                seconds,
                utc_offset_seconds,
            } => datetime_at(seconds, i64::from(utc_offset_seconds))
                .and_then(|at| Timestamp::new_local(at, utc_offset_seconds / 60)),
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

/// The source pages to export: the union of its ranges, or every page when
/// there are none.
///
/// Parses from text like `1,3-5,9-`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PageSelection {
    /// The selected ranges.
    pub ranges: Vec<PageRange>,
}

impl PageSelection {
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

impl FromStr for PageSelection {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let page = |value: &str| -> Result<NonZeroUsize, String> {
            if value == "0" {
                return Err("page numbers start at one".to_owned());
            }
            value
                .parse::<NonZeroUsize>()
                .map_err(|_| format!("`{value}` is not a valid page number"))
        };
        let ranges = text
            .split(',')
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
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { ranges })
    }
}

/// Resource ceilings for compilation output.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Limits {
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
    /// Number of pages exported concurrently; zero is treated as one. Pages
    /// export on the current rayon pool, or on a dedicated pool when this
    /// exceeds the current pool's size.
    pub export_workers: u64,
}

impl Default for Limits {
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
            output: Output::new(OutputFormat::Html),
            pdf_timestamp: None,
            pdf_standards: None,
            untagged_by_selection: false,
        };
        match output.format {
            OutputFormat::Pdf => {
                output.pages.normalize(issues);
                output.pdf_standards.sort_by_key(standard_name);
                match PdfStandards::new(&output.pdf_standards) {
                    Ok(standards) => normalized.pdf_standards = Some(standards),
                    Err(error) => issues.push(RequestIssue::InvalidPdfStandards {
                        message: error.message().to_string(),
                        hints: error.hints().iter().map(ToString::to_string).collect(),
                    }),
                }
                let default_tagged = PdfOptions::default().tagged;
                let selected = !output.pages.ranges.is_empty();
                normalized.untagged_by_selection =
                    selected && output.pdf_tags.is_auto() && default_tagged;
                if output.pdf_tags.is_auto() {
                    output.pdf_tags = Smart::Custom(default_tagged && !selected);
                }
                if let CreationTimestamp::Local {
                    utc_offset_seconds, ..
                } = output.pdf_timestamp
                    && utc_offset_seconds.unsigned_abs() >= 86_400
                {
                    issues.push(RequestIssue::InvalidUtcOffset);
                }
                normalized.pdf_timestamp = output.pdf_timestamp.to_pdf();
            }
            OutputFormat::Png => {
                output.pages.normalize(issues);
                let ppi = output.ppi.get_or_insert_with(|| {
                    typst_render::RenderOptions::default().pixel_per_pt.get() * 72.0
                });
                if !ppi.is_finite() || *ppi <= 0.0 {
                    issues.push(RequestIssue::InvalidPpi);
                }
            }
            OutputFormat::Svg => output.pages.normalize(issues),
            OutputFormat::Html => {}
        }
        normalized.output = output;
        normalized
    }

    /// Writes a normalized output to the compilation identity preimage.
    pub(crate) fn write(&self, preimage: &mut Preimage) {
        match self.format {
            OutputFormat::Pdf => {
                preimage.str("pdf");
                self.pages.write(preimage);
                preimage.count(self.pdf_standards.len());
                for standard in &self.pdf_standards {
                    preimage.str(standard_name(standard));
                }
                match &self.pdf_identifier {
                    Smart::Auto => preimage.str("auto"),
                    Smart::Custom(identifier) => preimage.str("custom").str(identifier),
                };
                match &self.pdf_creator {
                    Smart::Auto => preimage.str("auto"),
                    Smart::Custom(None) => preimage.str("none"),
                    Smart::Custom(Some(creator)) => preimage.str("custom").str(creator),
                };
                preimage.bool(matches!(self.pdf_tags, Smart::Custom(true)));
                match self.pdf_timestamp {
                    CreationTimestamp::Omit => preimage.str("omit"),
                    CreationTimestamp::Utc(unix) => preimage.str("utc").int(unix),
                    CreationTimestamp::Local {
                        seconds,
                        utc_offset_seconds,
                    } => preimage
                        .str("local")
                        .int(seconds)
                        .int(i64::from(utc_offset_seconds)),
                };
                preimage.bool(self.pretty);
            }
            OutputFormat::Png => {
                preimage.str("png");
                self.pages.write(preimage);
                preimage
                    .int(self.ppi.unwrap_or_default().to_bits() as i64)
                    .bool(self.render_bleed);
            }
            OutputFormat::Svg => {
                preimage.str("svg");
                self.pages.write(preimage);
                preimage.bool(self.render_bleed).bool(self.pretty);
            }
            OutputFormat::Html => {
                preimage.str("html").bool(self.pretty);
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
