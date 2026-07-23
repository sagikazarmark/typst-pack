//! Compiling a pack into Compilation Output Artifacts.

use std::num::NonZeroUsize;

use ecow::EcoVec;
#[cfg(feature = "cli")]
use rayon::prelude::*;
use typst::diag::{SourceDiagnostic, Warned};
use typst::foundations::{Datetime, Dict};
use typst::syntax::Span;
use typst::{Feature, World};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

use crate::embedded::EmbeddedTypst;
use crate::{Pack, PackWorld};

/// The exact embedded Typst compiler implementation that produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ImplementationIdentity {
    implementation: &'static str,
    version: &'static str,
    source_checksum: &'static str,
    target: &'static str,
    target_features: &'static str,
    feature_set: &'static str,
    debug_assertions: bool,
}

impl ImplementationIdentity {
    const fn new(
        implementation: &'static str,
        version: &'static str,
        source_checksum: &'static str,
    ) -> Self {
        Self {
            implementation,
            version,
            source_checksum,
            target: env!("TYPST_PACK_TARGET"),
            target_features: env!("TYPST_PACK_CARGO_CFG_TARGET_FEATURE"),
            feature_set: "cargo-default-features",
            debug_assertions: cfg!(debug_assertions),
        }
    }
}

/// The exact embedded Typst compiler implementation that produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EngineIdentity(ImplementationIdentity);

impl EngineIdentity {
    pub(crate) const fn new(
        implementation: &'static str,
        version: &'static str,
        source_checksum: &'static str,
    ) -> Self {
        Self(ImplementationIdentity::new(
            implementation,
            version,
            source_checksum,
        ))
    }

    /// The compiler crate used for semantic compilation.
    pub fn implementation(self) -> &'static str {
        self.0.implementation
    }

    /// The exact compiler crate version.
    pub fn version(self) -> &'static str {
        self.0.version
    }

    /// The checksum of the exact compiler crate source from the Cargo lockfile.
    pub fn source_checksum(self) -> &'static str {
        self.0.source_checksum
    }

    /// The complete Rust target triple of the compiler implementation.
    pub fn target(self) -> &'static str {
        self.0.target
    }

    /// The target CPU features enabled for the compiler implementation.
    pub fn target_features(self) -> &'static str {
        self.0.target_features
    }

    /// The Cargo feature configuration of the compiler crate.
    pub fn feature_set(self) -> &'static str {
        self.0.feature_set
    }

    /// Whether the compiler implementation was built with debug assertions.
    pub fn debug_assertions(self) -> bool {
        self.0.debug_assertions
    }
}

/// The exact official exporter implementation that produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExporterIdentity(ImplementationIdentity);

impl ExporterIdentity {
    pub(crate) const fn new(
        implementation: &'static str,
        version: &'static str,
        source_checksum: &'static str,
    ) -> Self {
        Self(ImplementationIdentity::new(
            implementation,
            version,
            source_checksum,
        ))
    }

    /// The exporter crate used to produce the artifacts.
    pub fn implementation(self) -> &'static str {
        self.0.implementation
    }

    /// The exact exporter crate version.
    pub fn version(self) -> &'static str {
        self.0.version
    }

    /// The checksum of the exact exporter crate source from the Cargo lockfile.
    pub fn source_checksum(self) -> &'static str {
        self.0.source_checksum
    }

    /// The complete Rust target triple of the exporter implementation.
    pub fn target(self) -> &'static str {
        self.0.target
    }

    /// The target CPU features enabled for the exporter implementation.
    pub fn target_features(self) -> &'static str {
        self.0.target_features
    }

    /// The Cargo feature configuration of the exporter crate.
    pub fn feature_set(self) -> &'static str {
        self.0.feature_set
    }

    /// Whether the exporter implementation was built with debug assertions.
    pub fn debug_assertions(self) -> bool {
        self.0.debug_assertions
    }
}

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

/// An explicit semantic compilation request bound to one validated [`Pack`].
///
/// Compilation through this request has no project, package, font, clock,
/// environment, cache, or network fallback beyond the Pack and these values.
#[derive(Debug, Clone)]
pub struct PackCompilationRequest {
    pack: Pack,
    format: OutputFormat,
    options: CompileOptions,
    inputs: Dict,
    features: Vec<Feature>,
    document_time: Option<Datetime>,
}

impl PackCompilationRequest {
    /// Binds a validated Pack to an output format and deterministic defaults.
    pub fn new(pack: Pack, format: OutputFormat) -> Self {
        Self {
            pack,
            format,
            options: CompileOptions::default(),
            inputs: Dict::new(),
            features: Vec::new(),
            document_time: None,
        }
    }

    /// Sets the official exporter controls for this request.
    pub fn options(mut self, options: CompileOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets the exact values exposed to document code as `sys.inputs`.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables one official Typst engine feature.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Sets the exact date returned by document-time requests.
    pub fn document_time(mut self, document_time: Datetime) -> Self {
        self.document_time = Some(document_time);
        self
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
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
}

impl CompilationOutput {
    /// Total pages in the source document before page selection.
    ///
    /// HTML output is unpaged and returns `None`.
    pub fn source_page_count(&self) -> Option<usize> {
        self.source_page_count
    }

    /// The embedded compiler implementation that produced this output.
    pub fn engine_identity(&self) -> EngineIdentity {
        self.engine_identity
    }

    /// The official exporter implementation that produced these artifacts.
    pub fn exporter_identity(&self) -> ExporterIdentity {
        self.exporter_identity
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

/// A failed Pack-bound compilation request or engine execution.
#[derive(Debug, thiserror::Error)]
pub enum PackCompileError {
    /// The Pack compilation contract intentionally excludes Typst Bundle.
    #[error("the Typst Bundle feature is not supported for Pack compilation")]
    UnsupportedBundleFeature,
    /// The embedded compiler or exporter rejected the request.
    #[error(transparent)]
    Compilation(#[from] CompileError),
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

/// Compiles a validated Pack through the private Pack Compilation Kernel.
pub fn compile_pack(
    request: PackCompilationRequest,
) -> Result<CompilationOutput, PackCompileError> {
    let PackCompilationRequest {
        pack,
        format,
        options,
        inputs,
        mut features,
        document_time,
    } = request;
    if features.contains(&Feature::Bundle) {
        return Err(PackCompileError::UnsupportedBundleFeature);
    }
    if format == OutputFormat::Html && !features.contains(&Feature::Html) {
        features.push(Feature::Html);
    }
    let mut world = PackWorld::builder(pack).inputs(inputs);
    #[cfg(feature = "embedded-fonts")]
    {
        world = world.embedded_fonts(false);
    }
    for feature in features {
        world = world.feature(feature);
    }
    if let Some(document_time) = document_time {
        world = world.fixed_date(document_time);
    }
    let world = world.build();

    Ok(compile_with_default_pdf_timestamp(
        &world,
        format,
        &options,
        || None,
    )?)
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
            engine_identity: EmbeddedTypst::engine_identity(),
            exporter_identity: EmbeddedTypst::exporter_identity(format),
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
        engine_identity: EmbeddedTypst::engine_identity(),
        exporter_identity: EmbeddedTypst::exporter_identity(format),
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
