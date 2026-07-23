//! Private adapter for the embedded Typst compiler and exporters.

use typst::World;
use typst::diag::{SourceResult, Warned};
use typst_layout::{Page, PagedDocument};

use crate::compile::{EngineIdentity, ExporterIdentity, OutputFormat};

const TYPST_ENGINE_VERSION: &str = "0.15.0";
const TYPST_ENGINE_CHECKSUM: &str =
    "3c8bf2a5a9d58cc542764a88dd43c8a679b683db4ae23e36267219339ab36b01";
const TYPST_PDF_VERSION: &str = "0.15.0";
const TYPST_PDF_CHECKSUM: &str = "cd7b33bcabc3357480768f6c78dda99a838d621c71b4738b25e09ac30ac063c9";
const TYPST_RENDER_VERSION: &str = "0.15.0";
const TYPST_RENDER_CHECKSUM: &str =
    "040ab6e56e91099963ef69dd9f0d284adda66c066b742f1866a20b8295ebb2e3";
const TYPST_SVG_VERSION: &str = "0.15.0";
const TYPST_SVG_CHECKSUM: &str = "878b6e1293c2bea77a8be50670d1cbca4e676af96480313a105cba539da51d1c";
const TYPST_HTML_VERSION: &str = "0.15.0";
const TYPST_HTML_CHECKSUM: &str =
    "d442f92bae44087735efc8b83508b259c573bbccf027e098ac68c8f4ca879f76";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_identity_versions_match_exact_dependency_pins() {
        let manifest = toml::from_str::<toml::Value>(include_str!("../Cargo.toml")).unwrap();
        let dependencies = manifest["dependencies"].as_table().unwrap();
        let lockfile = toml::from_str::<toml::Value>(include_str!("../Cargo.lock")).unwrap();
        let packages = lockfile["package"].as_array().unwrap();
        for (dependency, version, checksum) in [
            ("typst", TYPST_ENGINE_VERSION, TYPST_ENGINE_CHECKSUM),
            ("typst-pdf", TYPST_PDF_VERSION, TYPST_PDF_CHECKSUM),
            ("typst-render", TYPST_RENDER_VERSION, TYPST_RENDER_CHECKSUM),
            ("typst-svg", TYPST_SVG_VERSION, TYPST_SVG_CHECKSUM),
            ("typst-html", TYPST_HTML_VERSION, TYPST_HTML_CHECKSUM),
        ] {
            assert_eq!(
                dependencies[dependency].as_str(),
                Some(format!("={version}").as_str()),
                "{dependency} identity is out of sync with Cargo.toml"
            );
            let package = packages
                .iter()
                .find(|package| {
                    package["name"].as_str() == Some(dependency)
                        && package["version"].as_str() == Some(version)
                })
                .unwrap();
            assert_eq!(package["checksum"].as_str(), Some(checksum));
        }
    }
}
