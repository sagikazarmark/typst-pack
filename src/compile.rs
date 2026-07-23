//! Compiling a pack into Compilation Output Artifacts.

use std::num::NonZeroUsize;
use std::ops::Range;

use ecow::EcoVec;
#[cfg(feature = "cli")]
use rayon::prelude::*;
use typst::diag::{Severity, SourceDiagnostic, Tracepoint, Warned};
use typst::foundations::{Datetime, Dict, Smart};
use typst::syntax::package::PackageSpec;
use typst::syntax::{DiagSpan, FileId, Span, VirtualRoot};
use typst::{Feature, World, WorldExt};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

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
    /// PDF standards to enforce through the official exporter.
    pub pdf_standards: Vec<PdfStandard>,
    /// The PDF file identifier, using the official exporter's automatic mode by default.
    pub pdf_identifier: Smart<String>,
    /// The PDF creator metadata, using the official exporter's automatic mode by default.
    pub pdf_creator: Smart<Option<String>>,
    /// Whether PDF accessibility tags should be emitted.
    ///
    /// Automatic tagging is disabled with a warning for a page subset, matching
    /// Typst's CLI. Explicit tagging is passed through to the exporter.
    pub pdf_tags: Smart<bool>,
    /// How the document creation datetime is recorded in PDF metadata.
    pub creation_timestamp: CreationTimestamp,
}

impl Default for CompileOptions {
    fn default() -> Self {
        let pdf = PdfOptions::default();
        Self {
            page_selection: PageSelection::default(),
            ppi: None,
            pretty: pdf.pretty,
            pdf_standards: vec![],
            pdf_identifier: pdf.ident,
            pdf_creator: pdf.creator,
            pdf_tags: Smart::Auto,
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

/// Whether the official compiler and exporter accepted the compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationStatus {
    Succeeded,
    Rejected,
}

/// The official phase that emitted a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Compilation,
    Export,
}

/// The exact embedded implementation that emitted a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticProducer {
    Engine(EngineIdentity),
    Exporter(ExporterIdentity),
}

/// Official Typst diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A source location expressed in the Pack's logical namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalSpan {
    logical_path: Option<String>,
    byte_range: Option<Range<usize>>,
}

impl LogicalSpan {
    /// The logical project or package path, independent of transport location.
    pub fn logical_path(&self) -> Option<&str> {
        self.logical_path.as_deref()
    }

    /// The exact source byte range when Typst attached one.
    pub fn byte_range(&self) -> Option<&Range<usize>> {
        self.byte_range.as_ref()
    }
}

/// A structured hint attached to an official diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticHint {
    message: String,
    span: LogicalSpan,
}

impl DiagnosticHint {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> &LogicalSpan {
        &self.span
    }
}

/// The kind of one official diagnostic tracepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracepointKind {
    Call,
    Show,
    Import,
    Include,
}

/// One structured tracepoint attached to an official diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTracepoint {
    kind: TracepointKind,
    value: Option<String>,
    span: LogicalSpan,
}

impl DiagnosticTracepoint {
    pub fn kind(&self) -> TracepointKind {
        self.kind
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn span(&self) -> &LogicalSpan {
        &self.span
    }
}

/// A lossless projection of the exposed fields of an official Typst diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationDiagnostic {
    severity: DiagnosticSeverity,
    message: String,
    span: LogicalSpan,
    hints: Vec<DiagnosticHint>,
    trace: Vec<DiagnosticTracepoint>,
    phase: DiagnosticPhase,
    producer: DiagnosticProducer,
}

impl CompilationDiagnostic {
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn span(&self) -> &LogicalSpan {
        &self.span
    }

    pub fn hints(&self) -> &[DiagnosticHint] {
        &self.hints
    }

    pub fn trace(&self) -> &[DiagnosticTracepoint] {
        &self.trace
    }

    pub fn phase(&self) -> DiagnosticPhase {
        self.phase
    }

    pub fn producer(&self) -> DiagnosticProducer {
        self.producer
    }
}

/// The semantic result of an accepted Pack compilation request.
#[derive(Debug, Clone)]
pub struct CompilationResult {
    status: CompilationStatus,
    artifacts: Vec<CompilationArtifact>,
    diagnostics: Vec<CompilationDiagnostic>,
    pack_warnings: Vec<PackCompilationWarning>,
    source_page_count: Option<usize>,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
}

impl CompilationResult {
    pub fn status(&self) -> CompilationStatus {
        self.status
    }

    pub fn artifacts(&self) -> &[CompilationArtifact] {
        &self.artifacts
    }

    pub fn diagnostics(&self) -> &[CompilationDiagnostic] {
        &self.diagnostics
    }

    /// Pack-owned warnings kept separate from official diagnostics.
    pub fn pack_warnings(&self) -> &[PackCompilationWarning] {
        &self.pack_warnings
    }

    pub fn source_page_count(&self) -> Option<usize> {
        self.source_page_count
    }

    pub fn engine_identity(&self) -> EngineIdentity {
        self.engine_identity
    }

    pub fn exporter_identity(&self) -> ExporterIdentity {
        self.exporter_identity
    }
}

/// A Pack-owned semantic request warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackCompilationWarning {
    message: String,
    hints: Vec<String>,
}

impl PackCompilationWarning {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn hints(&self) -> &[String] {
        &self.hints
    }
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
    pack_warnings: EcoVec<SourceDiagnostic>,
    source_page_count: Option<usize>,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
}

impl CompilationOutput {
    /// Pack-owned warnings kept separate from official Typst warnings.
    pub fn pack_warnings(&self) -> &[SourceDiagnostic] {
        &self.pack_warnings
    }

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
    /// The official PDF standards validator rejected the requested set.
    #[error(transparent)]
    InvalidPdfStandards(#[from] PdfStandardsValidationError),
    /// Compilation or export produced errors; warnings are included for
    /// complete reporting.
    #[error("compilation failed with {} error(s)", errors.len())]
    Diagnostics {
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
        phase: DiagnosticPhase,
        source_page_count: Option<usize>,
    },
    /// PNG export failed after compilation completed.
    #[error("PNG export failed: {message}")]
    PngExport {
        message: String,
        /// Warnings emitted before PNG export failed.
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
        source_page_count: usize,
    },
}

/// A lossless projection of an official PDF standards validation error.
#[derive(Debug, thiserror::Error)]
#[error("invalid PDF standards: {message}")]
pub struct PdfStandardsValidationError {
    message: String,
    hints: Vec<String>,
}

impl PdfStandardsValidationError {
    /// The official validation message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The official validation hints in their original order.
    pub fn hints(&self) -> &[String] {
        &self.hints
    }

    /// Consumes the error into its official message and ordered hints.
    pub fn into_parts(self) -> (String, Vec<String>) {
        (self.message, self.hints)
    }
}

/// A Pack-owned semantic request rejection.
#[derive(Debug, thiserror::Error)]
pub enum CompilationRequestRejection {
    /// The Pack compilation contract intentionally excludes Typst Bundle.
    #[error("the Typst Bundle feature is not supported for Pack compilation")]
    UnsupportedBundleFeature,
    /// The official PDF standards validator rejected the requested set.
    #[error(transparent)]
    InvalidPdfStandards(PdfStandardsValidationError),
    /// Explicit tagging cannot be combined with a PDF page subset.
    #[error("cannot enable tagged PDF and export a page range")]
    PdfTagsWithPageSelection,
    /// A selected PDF standard requires accessibility tags.
    #[error("cannot disable PDF tags for a standard that requires them")]
    PdfStandardRequiresTags,
    /// Multiple independently detectable request issues in stable order.
    #[error("the compilation request contains multiple invalid values")]
    Multiple { issues: Vec<Self> },
}

impl CompilationRequestRejection {
    /// The independently detectable request issues in stable order.
    pub fn issues(&self) -> &[Self] {
        match self {
            Self::Multiple { issues } => issues,
            issue => std::slice::from_ref(issue),
        }
    }

    fn from_issues(mut issues: Vec<Self>) -> Option<Self> {
        match issues.len() {
            0 => None,
            1 => issues.pop(),
            _ => Some(Self::Multiple { issues }),
        }
    }
}

/// A Pack-owned operational outcome before official compilation begins.
#[derive(Debug, thiserror::Error)]
pub enum CompilationOperationOutcome {
    /// The request supplied no authority for declared external packages.
    #[error("external package fulfillment is unavailable for {packages:?}")]
    MissingExternalPackageFulfillment { packages: Vec<PackageSpec> },
}

/// A failed Pack-bound request or operation, never an official rejection.
#[derive(Debug, thiserror::Error)]
pub enum PackCompileError {
    #[error(transparent)]
    RequestRejected(#[from] CompilationRequestRejection),
    #[error(transparent)]
    Operation(#[from] CompilationOperationOutcome),
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
) -> Result<CompilationResult, PackCompileError> {
    let PackCompilationRequest {
        pack,
        format,
        options,
        inputs,
        mut features,
        document_time,
    } = request;
    let mut request_issues = vec![];
    if features.contains(&Feature::Bundle) {
        request_issues.push(CompilationRequestRejection::UnsupportedBundleFeature);
    }
    if format == OutputFormat::Pdf {
        if let Err(error) = validate_pdf_standards(&options.pdf_standards) {
            request_issues.push(CompilationRequestRejection::InvalidPdfStandards(error));
        }
        let has_page_selection = !options.page_selection.ranges().is_empty();
        let tagged = match options.pdf_tags {
            Smart::Auto => PdfOptions::default().tagged && !has_page_selection,
            Smart::Custom(tagged) => tagged,
        };
        if has_page_selection && matches!(options.pdf_tags, Smart::Custom(true)) {
            request_issues.push(CompilationRequestRejection::PdfTagsWithPageSelection);
        }
        if !tagged && pdf_standard_requiring_tags(&options.pdf_standards).is_some() {
            request_issues.push(CompilationRequestRejection::PdfStandardRequiresTags);
        }
    }
    if let Some(rejection) = CompilationRequestRejection::from_issues(request_issues) {
        return Err(rejection.into());
    }
    let external_packages = pack.manifest().packages().unvendored().to_vec();
    if !external_packages.is_empty() {
        return Err(
            CompilationOperationOutcome::MissingExternalPackageFulfillment {
                packages: external_packages,
            }
            .into(),
        );
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

    let engine_identity = EmbeddedTypst::engine_identity();
    let exporter_identity = EmbeddedTypst::exporter_identity(format);
    let result = match compile_with_default_pdf_timestamp(&world, format, &options, || None) {
        Ok(output) => {
            let diagnostics = project_diagnostics(
                &world,
                output.warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(engine_identity),
            );
            let pack_warnings = project_pack_warnings(output.pack_warnings);
            CompilationResult {
                status: CompilationStatus::Succeeded,
                artifacts: output.artifacts,
                diagnostics,
                pack_warnings,
                source_page_count: output.source_page_count,
                engine_identity,
                exporter_identity,
            }
        }
        Err(CompileError::Diagnostics {
            errors,
            warnings,
            pack_warnings,
            phase,
            source_page_count,
        }) => {
            let mut diagnostics = project_diagnostics(
                &world,
                warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(engine_identity),
            );
            let producer = match phase {
                DiagnosticPhase::Compilation => DiagnosticProducer::Engine(engine_identity),
                DiagnosticPhase::Export => DiagnosticProducer::Exporter(exporter_identity),
            };
            diagnostics.extend(project_diagnostics(&world, errors, phase, producer));
            CompilationResult {
                status: CompilationStatus::Rejected,
                artifacts: vec![],
                diagnostics,
                pack_warnings: project_pack_warnings(pack_warnings),
                source_page_count,
                engine_identity,
                exporter_identity,
            }
        }
        Err(CompileError::PngExport {
            message,
            warnings,
            pack_warnings,
            source_page_count,
        }) => {
            let mut diagnostics = project_diagnostics(
                &world,
                warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(engine_identity),
            );
            diagnostics.push(CompilationDiagnostic {
                severity: DiagnosticSeverity::Error,
                message,
                span: LogicalSpan {
                    logical_path: None,
                    byte_range: None,
                },
                hints: vec![],
                trace: vec![],
                phase: DiagnosticPhase::Export,
                producer: DiagnosticProducer::Exporter(exporter_identity),
            });
            CompilationResult {
                status: CompilationStatus::Rejected,
                artifacts: vec![],
                diagnostics,
                pack_warnings: project_pack_warnings(pack_warnings),
                source_page_count: Some(source_page_count),
                engine_identity,
                exporter_identity,
            }
        }
        Err(CompileError::InvalidPdfStandards(error)) => {
            return Err(CompilationRequestRejection::InvalidPdfStandards(error).into());
        }
    };
    Ok(result)
}

pub(crate) fn compile_with_default_pdf_timestamp(
    world: &dyn World,
    format: OutputFormat,
    options: &CompileOptions,
    default_pdf_timestamp: impl FnOnce() -> Option<Timestamp>,
) -> Result<CompilationOutput, CompileError> {
    let _compilation_timing = typst_timing::TimingScope::new("typst-pack compilation");
    let pdf_standards = (format == OutputFormat::Pdf)
        .then(|| validate_pdf_standards(&options.pdf_standards))
        .transpose()
        .map_err(CompileError::InvalidPdfStandards)?;
    if format == OutputFormat::Html {
        let pack_warnings = EcoVec::new();
        let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
        let document = output.map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
            pack_warnings: pack_warnings.clone(),
            phase: DiagnosticPhase::Compilation,
            source_page_count: None,
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
            pack_warnings: pack_warnings.clone(),
            phase: DiagnosticPhase::Export,
            source_page_count: None,
        })?;
        return Ok(CompilationOutput {
            artifacts: vec![CompilationArtifact {
                format,
                bytes,
                source_page_number: None,
            }],
            warnings,
            pack_warnings,
            source_page_count: None,
            engine_identity: EmbeddedTypst::engine_identity(),
            exporter_identity: EmbeddedTypst::exporter_identity(format),
        });
    }

    let Warned {
        output,
        warnings: compile_warnings,
    } = EmbeddedTypst::compile_paged(world);
    let warnings = compile_warnings;
    let mut pack_warnings = EcoVec::new();
    if format == OutputFormat::Pdf
        && !options.page_selection.ranges().is_empty()
        && options.pdf_tags.is_auto()
        && PdfOptions::default().tagged
    {
        pack_warnings.push(
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
        pack_warnings: pack_warnings.clone(),
        phase: DiagnosticPhase::Compilation,
        source_page_count: None,
    })?;
    let source_page_count = document.pages().len();
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
                    ident: options.pdf_identifier.clone(),
                    creator: options.pdf_creator.clone(),
                    timestamp,
                    page_ranges: options.page_selection.typst_page_ranges(),
                    standards: pdf_standards
                        .clone()
                        .expect("PDF standards are prepared for PDF export"),
                    tagged: match options.pdf_tags {
                        Smart::Auto => {
                            PdfOptions::default().tagged
                                && options.page_selection.ranges().is_empty()
                        }
                        Smart::Custom(tagged) => tagged,
                    },
                    pretty: options.pretty,
                };
                let pdf = EmbeddedTypst::export_pdf(&document, &pdf_options).map_err(|errors| {
                    CompileError::Diagnostics {
                        errors,
                        warnings: warnings.clone(),
                        pack_warnings: pack_warnings.clone(),
                        phase: DiagnosticPhase::Export,
                        source_page_count: Some(source_page_count),
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
                                pack_warnings: pack_warnings.clone(),
                                source_page_count,
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
        pack_warnings,
        source_page_count: Some(source_page_count),
        engine_identity: EmbeddedTypst::engine_identity(),
        exporter_identity: EmbeddedTypst::exporter_identity(format),
    })
}

pub(crate) fn validate_pdf_standards(
    standards: &[PdfStandard],
) -> Result<PdfStandards, PdfStandardsValidationError> {
    PdfStandards::new(standards).map_err(|error| PdfStandardsValidationError {
        message: error.message().to_string(),
        hints: error.hints().iter().map(ToString::to_string).collect(),
    })
}

pub(crate) fn pdf_standard_requiring_tags(standards: &[PdfStandard]) -> Option<&'static str> {
    standards.iter().find_map(|standard| match standard {
        PdfStandard::A_1a => Some("PDF/A-1a"),
        PdfStandard::A_2a => Some("PDF/A-2a"),
        PdfStandard::A_3a => Some("PDF/A-3a"),
        PdfStandard::Ua_1 => Some("PDF/UA-1"),
        _ => None,
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

fn project_diagnostics(
    world: &dyn World,
    diagnostics: impl IntoIterator<Item = SourceDiagnostic>,
    phase: DiagnosticPhase,
    producer: DiagnosticProducer,
) -> Vec<CompilationDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| CompilationDiagnostic {
            severity: match diagnostic.severity {
                Severity::Error => DiagnosticSeverity::Error,
                Severity::Warning => DiagnosticSeverity::Warning,
            },
            message: diagnostic.message.into(),
            span: logical_span(world, diagnostic.span),
            hints: diagnostic
                .hints
                .into_iter()
                .map(|hint| DiagnosticHint {
                    message: hint.v.into(),
                    span: logical_span(world, hint.span),
                })
                .collect(),
            trace: diagnostic
                .trace
                .into_iter()
                .map(|trace| {
                    let (kind, value) = match trace.v {
                        Tracepoint::Call(value) => (TracepointKind::Call, value.map(String::from)),
                        Tracepoint::Show(value) => (TracepointKind::Show, Some(value.into())),
                        Tracepoint::Import(value) => (TracepointKind::Import, Some(value.into())),
                        Tracepoint::Include(value) => (TracepointKind::Include, Some(value.into())),
                    };
                    DiagnosticTracepoint {
                        kind,
                        value,
                        span: logical_span(world, trace.span.into()),
                    }
                })
                .collect(),
            phase,
            producer,
        })
        .collect()
}

fn project_pack_warnings(
    warnings: impl IntoIterator<Item = SourceDiagnostic>,
) -> Vec<PackCompilationWarning> {
    warnings
        .into_iter()
        .map(|warning| PackCompilationWarning {
            message: warning.message.into(),
            hints: warning
                .hints
                .into_iter()
                .map(|hint| hint.v.into())
                .collect(),
        })
        .collect()
}

fn logical_span(world: &dyn World, span: DiagSpan) -> LogicalSpan {
    LogicalSpan {
        logical_path: span.id().map(logical_path),
        byte_range: world.range(span),
    }
}

fn logical_path(id: FileId) -> String {
    let path = id.vpath().get_without_slash();
    match id.root() {
        VirtualRoot::Project => format!("project:{path}"),
        VirtualRoot::Package(spec) => format!("package:{spec}/{path}"),
    }
}
