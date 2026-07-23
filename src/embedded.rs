//! Private adapter for the embedded Typst compiler and exporters.

use typst::World;
use typst::diag::{SourceResult, Warned};
use typst_layout::{Page, PagedDocument};

pub(crate) struct EmbeddedTypst;

impl EmbeddedTypst {
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
