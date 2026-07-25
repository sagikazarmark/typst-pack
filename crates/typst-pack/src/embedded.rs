//! Private adapter for the embedded Typst compiler and exporters.

use typst::World;
use typst::diag::{SourceResult, Warned};
use typst_layout::{Page, PagedDocument};

use crate::compile::{EngineIdentity, ExporterIdentity, OutputFormat};

const TYPST_ENGINE_VERSION: &str = env!("TYPST_PACK_ENGINE_VERSION");
const TYPST_ENGINE_CHECKSUM: &str = env!("TYPST_PACK_ENGINE_CHECKSUM");
const TYPST_PDF_VERSION: &str = env!("TYPST_PACK_PDF_VERSION");
const TYPST_PDF_CHECKSUM: &str = env!("TYPST_PACK_PDF_CHECKSUM");
const TYPST_RENDER_VERSION: &str = env!("TYPST_PACK_PNG_VERSION");
const TYPST_RENDER_CHECKSUM: &str = env!("TYPST_PACK_PNG_CHECKSUM");
const TYPST_SVG_VERSION: &str = env!("TYPST_PACK_SVG_VERSION");
const TYPST_SVG_CHECKSUM: &str = env!("TYPST_PACK_SVG_CHECKSUM");
const TYPST_HTML_VERSION: &str = env!("TYPST_PACK_HTML_VERSION");
const TYPST_HTML_CHECKSUM: &str = env!("TYPST_PACK_HTML_CHECKSUM");

pub(crate) struct EmbeddedTypst;

impl EmbeddedTypst {
    pub(crate) fn engine_identity() -> EngineIdentity {
        EngineIdentity::new("typst", TYPST_ENGINE_VERSION, TYPST_ENGINE_CHECKSUM)
    }

    pub(crate) fn exporter_identity(format: OutputFormat) -> ExporterIdentity {
        let (implementation, version, checksum) = match format {
            OutputFormat::Pdf => ("typst-pdf", TYPST_PDF_VERSION, TYPST_PDF_CHECKSUM),
            OutputFormat::Png => ("typst-render", TYPST_RENDER_VERSION, TYPST_RENDER_CHECKSUM),
            OutputFormat::Svg => ("typst-svg", TYPST_SVG_VERSION, TYPST_SVG_CHECKSUM),
            OutputFormat::Html => ("typst-html", TYPST_HTML_VERSION, TYPST_HTML_CHECKSUM),
        };
        ExporterIdentity::new(implementation, version, checksum)
    }

    pub(crate) fn compile_paged(world: &dyn World) -> Warned<SourceResult<PagedDocument>> {
        typst::compile::<PagedDocument>(world)
    }

    pub(crate) fn compile_html(
        world: &dyn World,
    ) -> Warned<SourceResult<typst_html::HtmlDocument>> {
        typst::compile::<typst_html::HtmlDocument>(world)
    }

    pub(crate) fn export_pdf(
        document: &PagedDocument,
        options: &typst_pdf::PdfOptions,
    ) -> SourceResult<Vec<u8>> {
        typst_pdf::pdf(document, options)
    }

    pub(crate) fn export_png(
        page: &Page,
        options: &typst_render::RenderOptions,
    ) -> Result<Vec<u8>, String> {
        typst_render::render(page, options)
            .encode_png()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn export_svg(page: &Page, options: &typst_svg::SvgOptions) -> Vec<u8> {
        typst_svg::svg(page, options).into_bytes()
    }

    pub(crate) fn export_html(
        document: &typst_html::HtmlDocument,
        options: &typst_html::HtmlOptions,
    ) -> SourceResult<Vec<u8>> {
        typst_html::html(document, options).map(String::into_bytes)
    }
}
