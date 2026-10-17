//! Running the Typst compiler and exporters over a prepared world.

use std::num::NonZeroUsize;

use ecow::EcoVec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use typst::World;
use typst::diag::{SourceDiagnostic, Warned};
use typst::foundations::Smart;
use typst::syntax::Span;
use typst_layout::{Page, PagedDocument};
use typst_pdf::PdfOptions;

use super::output::{Normalized, Output, OutputFormat, PageSelection};
use super::{Artifact, CompilationLimits};
use crate::LimitExceeded;

/// What a successful run produced.
pub(super) struct Exported {
    pub(super) artifacts: Vec<Artifact>,
    pub(super) page_count: Option<usize>,
    pub(super) warnings: EcoVec<SourceDiagnostic>,
}

/// Why a run produced nothing.
pub(super) enum Failure {
    Limit(LimitExceeded),
    Diagnostics {
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    Png {
        page: NonZeroUsize,
        message: String,
        warnings: EcoVec<SourceDiagnostic>,
    },
}

impl From<LimitExceeded> for Failure {
    fn from(error: LimitExceeded) -> Self {
        Self::Limit(error)
    }
}

/// Compiles the world's document and exports the requested output.
pub(super) fn export(
    world: &dyn World,
    normalized: &Normalized,
    limits: CompilationLimits,
) -> Result<Exported, Failure> {
    if let Output::Html(html) = &normalized.output {
        let Warned { output, warnings } = typst::compile::<typst_html::HtmlDocument>(world);
        let failed = |errors| Failure::Diagnostics {
            errors,
            warnings: warnings.clone(),
        };
        let document = output.map_err(failed)?;
        check_count(limits, 1)?;
        let _timing = typst_timing::TimingScope::new("export");
        let options = typst_html::HtmlOptions {
            pretty: html.pretty,
        };
        let bytes = typst_html::html(&document, &options).map_err(failed)?;
        let artifacts = vec![artifact(OutputFormat::Html, None, bytes.into_bytes())];
        check_bytes(limits, &artifacts)?;
        return Ok(Exported {
            artifacts,
            page_count: None,
            warnings,
        });
    }

    let Warned {
        output,
        mut warnings,
    } = typst::compile::<PagedDocument>(world);
    if normalized.untagged_by_selection {
        warnings.push(
            SourceDiagnostic::warning(Span::detached(), "using --pages implies --no-pdf-tags")
                .with_hints([
                    "the resulting PDF will be inaccessible".into(),
                    "add --no-pdf-tags to silence this warning".into(),
                ]),
        );
    }
    let document = output.map_err(|errors| Failure::Diagnostics {
        errors,
        warnings: warnings.clone(),
    })?;
    let page_count = document.pages().len();
    LimitExceeded::check(page_count as u64, limits.max_pages, "pages")?;
    let _timing = typst_timing::TimingScope::new("export");
    let artifacts = match &normalized.output {
        Output::Pdf(pdf) => {
            check_count(limits, 1)?;
            let options = PdfOptions {
                ident: pdf.identifier.clone(),
                creator: pdf.creator.clone(),
                timestamp: normalized.pdf_timestamp,
                page_ranges: pdf.page_selection.typst_page_ranges(),
                standards: normalized
                    .pdf_standards
                    .clone()
                    .expect("validated PDF standards"),
                tagged: matches!(pdf.tags, Smart::Custom(true)),
                pretty: pdf.pretty,
            };
            let bytes =
                typst_pdf::pdf(&document, &options).map_err(|errors| Failure::Diagnostics {
                    errors,
                    warnings: warnings.clone(),
                })?;
            vec![artifact(OutputFormat::Pdf, None, bytes)]
        }
        Output::Png(png) => {
            let options = typst_render::RenderOptions {
                pixel_per_pt: (png.pixels_per_inch.expect("normalized PPI") / 72.0).into(),
                render_bleed: png.render_bleed,
            };
            let pages = selected_pages(&document, &png.page_selection);
            check_count(limits, pages.len())?;
            check_pixels(limits, &pages, &options)?;
            export_pages(pages, limits, |(page, content)| {
                let bytes = typst_render::render(content, &options)
                    .encode_png()
                    .map_err(|error| Failure::Png {
                        page,
                        message: error.to_string(),
                        warnings: warnings.clone(),
                    })?;
                Ok(artifact(OutputFormat::Png, Some(page), bytes))
            })?
        }
        Output::Svg(svg) => {
            let options = typst_svg::SvgOptions {
                render_bleed: svg.render_bleed,
                pretty: svg.pretty,
            };
            let pages = selected_pages(&document, &svg.page_selection);
            check_count(limits, pages.len())?;
            export_pages(pages, limits, |(page, content)| {
                let bytes = typst_svg::svg(content, &options).into_bytes();
                Ok(artifact(OutputFormat::Svg, Some(page), bytes))
            })?
        }
        Output::Html(_) => unreachable!("HTML is exported above"),
    };
    check_bytes(limits, &artifacts)?;
    Ok(Exported {
        artifacts,
        page_count: Some(page_count),
        warnings,
    })
}

fn artifact(format: OutputFormat, page: Option<NonZeroUsize>, bytes: Vec<u8>) -> Artifact {
    Artifact {
        format,
        page,
        bytes,
    }
}

fn selected_pages<'a>(
    document: &'a PagedDocument,
    selection: &PageSelection,
) -> Vec<(NonZeroUsize, &'a Page)> {
    let ranges = selection.typst_page_ranges();
    document
        .pages()
        .iter()
        .enumerate()
        .map(|(index, page)| (NonZeroUsize::new(index + 1).expect("one-based"), page))
        .filter(|(number, _)| {
            ranges
                .as_ref()
                .is_none_or(|ranges| ranges.includes_page(*number))
        })
        .collect()
}

fn check_count(limits: CompilationLimits, count: usize) -> Result<(), LimitExceeded> {
    LimitExceeded::check(count as u64, limits.max_artifacts, "artifacts")
}

fn check_pixels(
    limits: CompilationLimits,
    pages: &[(NonZeroUsize, &Page)],
    options: &typst_render::RenderOptions,
) -> Result<(), LimitExceeded> {
    let mut total = 0u64;
    for (_, page) in pages {
        let size = if options.render_bleed {
            page.frame.size() + page.bleed.sum_by_axis()
        } else {
            page.frame.size()
        };
        let pixel_per_pt = options.pixel_per_pt.get() as f32;
        let width = (pixel_per_pt * size.x.to_pt() as f32).round().max(1.0) as u32;
        let height = (pixel_per_pt * size.y.to_pt() as f32).round().max(1.0) as u32;
        let pixels = u64::from(width) * u64::from(height);
        LimitExceeded::check(
            pixels,
            limits.max_pixels_per_artifact,
            "pixels per artifact",
        )?;
        LimitExceeded::add(&mut total, pixels, limits.max_total_pixels, "total pixels")?;
    }
    Ok(())
}

fn check_bytes(limits: CompilationLimits, artifacts: &[Artifact]) -> Result<(), LimitExceeded> {
    let mut total = 0;
    artifacts
        .iter()
        .try_for_each(|artifact| retain(limits, &mut total, artifact))
}

fn retain(
    limits: CompilationLimits,
    total: &mut u64,
    artifact: &Artifact,
) -> Result<(), LimitExceeded> {
    let bytes = artifact.bytes.len() as u64;
    LimitExceeded::check(bytes, limits.max_artifact_bytes, "artifact bytes")?;
    LimitExceeded::add(
        total,
        bytes,
        limits.max_total_artifact_bytes,
        "total artifact bytes",
    )
}

/// Exports pages in order, in batches of at most `export_workers`, stopping
/// at the first failure or exceeded byte limit.
#[cfg(feature = "parallel")]
pub(super) fn export_pages<T: Send>(
    items: Vec<T>,
    limits: CompilationLimits,
    export: impl Fn(T) -> Result<Artifact, Failure> + Sync + Send,
) -> Result<Vec<Artifact>, Failure> {
    let workers = usize::try_from(limits.export_workers.max(1))
        .unwrap_or(usize::MAX)
        .min(items.len().max(1));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .ok();
    let mut items = items.into_iter();
    let mut artifacts = Vec::new();
    let mut total = 0;
    loop {
        let batch = items.by_ref().take(workers).collect::<Vec<_>>();
        if batch.is_empty() {
            return Ok(artifacts);
        }
        let batch: Vec<_> = match &pool {
            Some(pool) => pool.install(|| batch.into_par_iter().map(&export).collect()),
            None => batch.into_iter().map(&export).collect(),
        };
        for artifact in batch {
            let artifact = artifact?;
            retain(limits, &mut total, &artifact)?;
            artifacts.push(artifact);
        }
    }
}

#[cfg(not(feature = "parallel"))]
pub(super) fn export_pages<T>(
    items: Vec<T>,
    limits: CompilationLimits,
    export: impl Fn(T) -> Result<Artifact, Failure>,
) -> Result<Vec<Artifact>, Failure> {
    let mut artifacts = Vec::new();
    let mut total = 0;
    for item in items {
        let artifact = export(item)?;
        retain(limits, &mut total, &artifact)?;
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

#[cfg(all(test, feature = "parallel"))]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    use super::*;

    #[test]
    fn parallel_export_obeys_the_worker_limit_and_preserves_page_order() {
        for demand in [3u8, 4, 5] {
            let expected_peak = usize::from(demand.min(4));
            let active = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));
            let barrier = Arc::new(Barrier::new(expected_peak));
            let output = export_pages(
                (0u8..demand).collect(),
                CompilationLimits::default(),
                |item| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    if usize::from(item) < expected_peak {
                        barrier.wait();
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(artifact(OutputFormat::Svg, None, vec![item]))
                },
            )
            .unwrap_or_else(|_| panic!("export failed"));

            assert_eq!(peak.load(Ordering::SeqCst), expected_peak);
            assert_eq!(
                output.iter().map(|a| a.bytes[0]).collect::<Vec<_>>(),
                (0u8..demand).collect::<Vec<_>>()
            );
        }
    }
}
