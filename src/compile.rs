//! Compiling a pack into Compilation Output Artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::ops::Range;

use ecow::EcoVec;
#[cfg(feature = "cli")]
use rayon::prelude::*;
use typst::diag::{Severity, SourceDiagnostic, Tracepoint, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Repr, Smart};
use typst::syntax::package::PackageSpec;
use typst::syntax::{DiagSpan, FileId, Span, VirtualRoot};
use typst::{Feature, World, WorldExt};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use crate::embedded::EmbeddedTypst;
use crate::pack::{FontCatalogError, PackageTreeError};
#[cfg(feature = "cli")]
use crate::resource::Provider;
use crate::{FontContainerIdentity, Pack, PackWorld, PackageTreeIdentity};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Whether Page Format exporters render into the page bleed region.
    pub render_bleed: bool,
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
            render_bleed: false,
            pretty: pdf.pretty,
            pdf_standards: vec![],
            pdf_identifier: pdf.ident,
            pdf_creator: pdf.creator,
            pdf_tags: Smart::Auto,
            creation_timestamp: CreationTimestamp::Automatic,
        }
    }
}

/// How an effective compilation request value was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestValueOrigin {
    /// The library caller supplied the value directly.
    CallerSupplied,
    /// The semantic core supplied its deterministic default.
    CoreDefaulted,
    /// The semantic core derived the value from another request value.
    CoreDerived,
    /// An adapter resolved an ambient or adapter-level default before execution.
    AdapterResolved,
}

/// One effective request value together with its provenance.
#[derive(Debug, Clone)]
pub struct EffectiveRequestValue<T> {
    value: T,
    origin: RequestValueOrigin,
}

impl<T> EffectiveRequestValue<T> {
    fn new(value: T, origin: RequestValueOrigin) -> Self {
        Self { value, origin }
    }

    /// The exact effective value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// How the effective value was established.
    pub fn origin(&self) -> RequestValueOrigin {
        self.origin
    }
}

/// One enabled Typst engine feature and why it is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectiveEngineFeature {
    value: Feature,
    origin: RequestValueOrigin,
}

impl EffectiveEngineFeature {
    /// The exact feature from the pinned Typst feature set.
    pub fn value(self) -> Feature {
        self.value
    }

    /// How the feature became enabled.
    pub fn origin(self) -> RequestValueOrigin {
        self.origin
    }
}

/// Every effective shared semantic value passed to the embedded Typst engine.
#[derive(Debug, Clone)]
pub struct CompilationRequestInventory {
    format: EffectiveRequestValue<OutputFormat>,
    options: EffectiveRequestValue<CompileOptions>,
    ppi_origin: RequestValueOrigin,
    pdf_tags_origin: RequestValueOrigin,
    pdf_creation_time_origin: RequestValueOrigin,
    inputs: EffectiveRequestValue<TypstInputsInventory>,
    overrides: EffectiveRequestValue<PackOverridesInventory>,
    selected_features: Vec<EffectiveEngineFeature>,
    features: Vec<EffectiveEngineFeature>,
    document_time: EffectiveRequestValue<Option<Datetime>>,
    document_timestamp: EffectiveRequestValue<Option<i64>>,
}

impl CompilationRequestInventory {
    /// The selected output format.
    pub fn format(&self) -> &EffectiveRequestValue<OutputFormat> {
        &self.format
    }

    /// The output controls, including their deterministic defaults.
    pub fn options(&self) -> &EffectiveRequestValue<CompileOptions> {
        &self.options
    }

    /// Safe evidence for the exact `sys.inputs` dictionary.
    pub fn inputs(&self) -> &EffectiveRequestValue<TypstInputsInventory> {
        &self.inputs
    }

    /// Safe evidence for the exact Pack Override Set.
    pub fn overrides(&self) -> &EffectiveRequestValue<PackOverridesInventory> {
        &self.overrides
    }

    /// How the effective PNG PPI was established.
    pub fn ppi_origin(&self) -> RequestValueOrigin {
        self.ppi_origin
    }

    /// How the effective PDF tagging value was established.
    pub fn pdf_tags_origin(&self) -> RequestValueOrigin {
        self.pdf_tags_origin
    }

    /// How the effective PDF creation time was established.
    pub fn pdf_creation_time_origin(&self) -> RequestValueOrigin {
        self.pdf_creation_time_origin
    }

    /// The exact effective pinned Typst feature set.
    pub fn features(&self) -> &[EffectiveEngineFeature] {
        &self.features
    }

    /// Features explicitly selected by a caller or adapter before derivation.
    pub fn selected_features(&self) -> &[EffectiveEngineFeature] {
        &self.selected_features
    }

    /// The exact Compilation Document Time, including explicit absence.
    pub fn document_time(&self) -> &EffectiveRequestValue<Option<Datetime>> {
        &self.document_time
    }

    /// The exact effective Unix timestamp used for offset-aware document-time requests.
    ///
    /// This is `None` when document time is absent or represented by [`Self::document_time`].
    pub fn document_timestamp(&self) -> &EffectiveRequestValue<Option<i64>> {
        &self.document_timestamp
    }
}

/// Safe, role-bound evidence for potentially sensitive Typst inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypstInputsInventory {
    commitment: u128,
    entry_count: usize,
    total_key_bytes: usize,
    total_value_repr_bytes: usize,
}

impl TypstInputsInventory {
    /// The commitment schema, including the pinned value-hash implementation.
    pub fn schema(self) -> &'static str {
        "typst-pack-inputs-v1+typst-0.15"
    }

    /// The role-bound commitment digest in big-endian order.
    pub fn commitment(self) -> [u8; 16] {
        self.commitment.to_be_bytes()
    }

    /// The number of input keys represented by the commitment.
    pub fn entry_count(self) -> usize {
        self.entry_count
    }

    /// The exact total UTF-8 byte length of all input keys.
    pub fn total_key_bytes(self) -> usize {
        self.total_key_bytes
    }

    /// The exact total UTF-8 byte length of the pinned Typst representations.
    pub fn total_value_repr_bytes(self) -> usize {
        self.total_value_repr_bytes
    }
}

/// Safe, role-bound evidence for one replacement in a Pack Override Set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackOverrideInventoryEntry {
    path: String,
    byte_len: usize,
    commitment: u128,
}

impl PackOverrideInventoryEntry {
    /// The commitment schema, including the pinned value-hash implementation.
    pub fn schema(&self) -> &'static str {
        "typst-pack-override-v1+typst-0.15"
    }

    /// The canonical contained project path being replaced.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The exact replacement byte length.
    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// The role-bound commitment digest in big-endian order.
    pub fn commitment(&self) -> [u8; 16] {
        self.commitment.to_be_bytes()
    }
}

/// Safe evidence for an immutable Pack Override Set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackOverridesInventory(Vec<PackOverrideInventoryEntry>);

impl PackOverridesInventory {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PackOverrideInventoryEntry> {
        self.0.iter()
    }
}

/// An immutable set of contained project-file replacements bound to one Pack.
#[derive(Debug, Clone)]
pub struct PackOverrideSet {
    pack_identity: crate::PackIdentity,
    project_paths: BTreeSet<String>,
    replacements: BTreeMap<String, Bytes>,
}

impl PackOverrideSet {
    /// Starts an empty override set bound to `pack`.
    pub fn new(pack: &Pack) -> Self {
        Self {
            pack_identity: pack.identity(),
            project_paths: pack.files().map(|(path, _)| path.to_owned()).collect(),
            replacements: BTreeMap::new(),
        }
    }

    /// Adds one replacement after strict Pack-owned preflight.
    pub fn replace(
        mut self,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackOverrideSetError> {
        let supplied = path.as_ref();
        let path = Pack::canonical_project_path(supplied).map_err(|message| {
            PackOverrideSetError::InvalidProjectPath {
                path: supplied.to_owned(),
                message,
            }
        })?;
        if self.replacements.contains_key(&path) {
            return Err(PackOverrideSetError::DuplicateProjectPath { path });
        }
        if !self.project_paths.contains(&path) {
            return Err(PackOverrideSetError::MissingProjectPath { path });
        }
        self.replacements.insert(path, Bytes::new(data.into()));
        Ok(self)
    }
}

/// A Pack-owned Pack Override preflight rejection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackOverrideSetError {
    #[error("invalid Pack Override project path `{path}`: {message}")]
    InvalidProjectPath { path: String, message: String },
    #[error("Pack Override path `{path}` is declared more than once")]
    DuplicateProjectPath { path: String },
    #[error("Pack Override path `{path}` is not a contained project file")]
    MissingProjectPath { path: String },
}

/// The pre-execution identity of a fully specified semantic compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationIdentity(u128);

impl CompilationIdentity {
    /// The canonical identity kind.
    pub fn kind(self) -> &'static str {
        "compilation"
    }

    /// The identity schema used by this release.
    pub fn schema(self) -> &'static str {
        "typst-pack-compilation-v1"
    }

    /// The deterministic digest algorithm used by this identity schema.
    pub fn algorithm(self) -> &'static str {
        "typst-hash128-0.15"
    }

    /// The deterministic 128-bit identity digest in big-endian order.
    pub fn digest(self) -> [u8; 16] {
        self.0.to_be_bytes()
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
    options: EffectiveRequestValue<CompileOptions>,
    inputs: EffectiveRequestValue<Dict>,
    overrides: EffectiveRequestValue<PackOverrideSet>,
    features: Vec<EffectiveEngineFeature>,
    document_time: EffectiveRequestValue<Option<Datetime>>,
    document_timestamp: EffectiveRequestValue<Option<i64>>,
    package_fulfillments: BTreeMap<String, PackageTreeFulfillment>,
    font_fulfillments: BTreeMap<FontContainerIdentity, FontContainerFulfillment>,
}

/// One externally acquired Complete Package Tree and operational metadata.
#[derive(Debug, Clone)]
pub struct PackageTreeFulfillment {
    files: Vec<(String, Bytes)>,
    provenance: Option<String>,
    cache_hit: bool,
}

impl PackageTreeFulfillment {
    pub fn new<I, P, D>(files: I) -> Self
    where
        I: IntoIterator<Item = (P, D)>,
        P: Into<String>,
        D: Into<Vec<u8>>,
    {
        Self {
            files: files
                .into_iter()
                .map(|(path, data)| (path.into(), Bytes::new(data.into())))
                .collect(),
            provenance: None,
            cache_hit: false,
        }
    }

    pub fn provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub fn cache_hit(mut self, cache_hit: bool) -> Self {
        self.cache_hit = cache_hit;
        self
    }
}

/// Exact externally supplied Font Container bytes and non-semantic metadata.
#[derive(Debug, Clone)]
pub struct FontContainerFulfillment {
    data: Bytes,
    provenance: Option<String>,
    licensing: Option<String>,
}

impl FontContainerFulfillment {
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: Bytes::new(data.into()),
            provenance: None,
            licensing: None,
        }
    }

    pub fn provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub fn licensing(mut self, licensing: impl Into<String>) -> Self {
        self.licensing = Some(licensing.into());
        self
    }
}

impl PackCompilationRequest {
    /// Binds a validated Pack to an output format and deterministic defaults.
    pub fn new(pack: Pack, format: OutputFormat) -> Self {
        let overrides = PackOverrideSet::new(&pack);
        Self {
            pack,
            format,
            options: EffectiveRequestValue::new(
                CompileOptions::default(),
                RequestValueOrigin::CoreDefaulted,
            ),
            inputs: EffectiveRequestValue::new(Dict::new(), RequestValueOrigin::CoreDefaulted),
            overrides: EffectiveRequestValue::new(overrides, RequestValueOrigin::CoreDefaulted),
            features: Vec::new(),
            document_time: EffectiveRequestValue::new(None, RequestValueOrigin::CoreDefaulted),
            document_timestamp: EffectiveRequestValue::new(None, RequestValueOrigin::CoreDefaulted),
            package_fulfillments: BTreeMap::new(),
            font_fulfillments: BTreeMap::new(),
        }
    }

    /// Sets the official exporter controls for this request.
    pub fn options(mut self, options: CompileOptions) -> Self {
        self.options = EffectiveRequestValue::new(options, RequestValueOrigin::CallerSupplied);
        self
    }

    /// Sets exporter controls after an adapter has resolved its external defaults.
    pub fn adapter_resolved_options(mut self, options: CompileOptions) -> Self {
        self.options = EffectiveRequestValue::new(options, RequestValueOrigin::AdapterResolved);
        self
    }

    /// Sets the exact values exposed to document code as `sys.inputs`.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = EffectiveRequestValue::new(inputs, RequestValueOrigin::CallerSupplied);
        self
    }

    /// Sets `sys.inputs` after an adapter has resolved its external defaults.
    pub fn adapter_resolved_inputs(mut self, inputs: Dict) -> Self {
        self.inputs = EffectiveRequestValue::new(inputs, RequestValueOrigin::AdapterResolved);
        self
    }

    /// Applies one immutable Pack-bound Pack Override Set.
    pub fn overrides(mut self, overrides: PackOverrideSet) -> Self {
        self.overrides = EffectiveRequestValue::new(overrides, RequestValueOrigin::CallerSupplied);
        self
    }

    /// Enables one official Typst engine feature.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(EffectiveEngineFeature {
            value: feature,
            origin: RequestValueOrigin::CallerSupplied,
        });
        self
    }

    /// Enables a feature selected by an adapter before semantic preparation.
    pub fn adapter_resolved_feature(mut self, feature: Feature) -> Self {
        self.features.push(EffectiveEngineFeature {
            value: feature,
            origin: RequestValueOrigin::AdapterResolved,
        });
        self
    }

    /// Sets the exact date returned by document-time requests.
    pub fn document_time(mut self, document_time: Datetime) -> Self {
        self.document_time =
            EffectiveRequestValue::new(Some(document_time), RequestValueOrigin::CallerSupplied);
        self.document_timestamp =
            EffectiveRequestValue::new(None, RequestValueOrigin::CoreDefaulted);
        self
    }

    /// Sets the exact document time resolved by an adapter.
    pub fn adapter_resolved_document_time(mut self, document_time: Option<Datetime>) -> Self {
        self.document_time =
            EffectiveRequestValue::new(document_time, RequestValueOrigin::AdapterResolved);
        self.document_timestamp =
            EffectiveRequestValue::new(None, RequestValueOrigin::CoreDefaulted);
        self
    }

    /// Sets the exact Unix timestamp resolved by an adapter for document-time requests.
    #[cfg(feature = "cli")]
    pub fn adapter_resolved_document_timestamp(
        mut self,
        timestamp: i64,
    ) -> typst::diag::StrResult<Self> {
        typst_kit::datetime::Time::fixed_timestamp(timestamp)?;
        self.document_time = EffectiveRequestValue::new(None, RequestValueOrigin::CoreDefaulted);
        self.document_timestamp =
            EffectiveRequestValue::new(Some(timestamp), RequestValueOrigin::AdapterResolved);
        Ok(self)
    }

    /// Supplies bytes for one exact external Font Container requirement.
    pub fn font_fulfillment(
        mut self,
        expected: FontContainerIdentity,
        fulfillment: FontContainerFulfillment,
    ) -> Self {
        self.font_fulfillments.insert(expected, fulfillment);
        self
    }

    /// Supplies one Complete Package Tree under its exact Typst package specification.
    pub fn package_fulfillment(
        mut self,
        spec: PackageSpec,
        fulfillment: PackageTreeFulfillment,
    ) -> Self {
        self.package_fulfillments
            .insert(spec.to_string(), fulfillment);
        self
    }
}

/// The source of the document creation datetime recorded in PDF metadata.
#[derive(Debug, Clone, Copy, Default, Hash)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
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

/// A structured compiler or exporter diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationDiagnostic {
    severity: DiagnosticSeverity,
    message: String,
    span: LogicalSpan,
    hints: Vec<DiagnosticHint>,
    trace: Vec<DiagnosticTracepoint>,
    phase: DiagnosticPhase,
    producer: DiagnosticProducer,
    source_page_number: Option<NonZeroUsize>,
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

    /// The Source Page Number whose Page Format export failed, when known.
    pub fn source_page_number(&self) -> Option<NonZeroUsize> {
        self.source_page_number
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
    request_inventory: CompilationRequestInventory,
    compilation_identity: CompilationIdentity,
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

    /// The complete effective semantic request prepared before execution.
    pub fn request_inventory(&self) -> &CompilationRequestInventory {
        &self.request_inventory
    }

    /// The identity of the complete prepared request and implementation.
    pub fn compilation_identity(&self) -> CompilationIdentity {
        self.compilation_identity
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
    #[error("PNG export failed for source page {source_page_number}: {message}")]
    PngExport {
        message: String,
        /// Warnings emitted before PNG export failed.
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
        source_page_count: usize,
        /// The page whose PNG encoding failed.
        source_page_number: NonZeroUsize,
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
    /// PNG resolution must be finite and greater than zero.
    #[error("PNG pixels per inch must be finite and greater than zero")]
    InvalidPpi,
    /// The official PDF standards validator rejected the requested set.
    #[error(transparent)]
    InvalidPdfStandards(PdfStandardsValidationError),
    /// Explicit tagging cannot be combined with a PDF page subset.
    #[error("cannot enable tagged PDF and export a page range")]
    PdfTagsWithPageSelection,
    /// A selected PDF standard requires accessibility tags.
    #[error("cannot disable PDF tags for a standard that requires them")]
    PdfStandardRequiresTags,
    /// The Pack Override Set was preflighted against a different Pack.
    #[error("the Pack Override Set is bound to a different Pack")]
    OverrideSetPackMismatch,
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
    /// An explicit Package Authority could not acquire one declared requirement.
    #[error("external package fulfillment for {spec} is unavailable: {message}")]
    UnavailableExternalPackageFulfillment { spec: PackageSpec, message: String },
    /// Supplied files do not match the exact required Complete Package Tree.
    #[error("external package fulfillment for {spec} supplied {actual:?}, expected {expected:?}")]
    MismatchedExternalPackageTree {
        spec: PackageSpec,
        expected: PackageTreeIdentity,
        actual: PackageTreeIdentity,
        expected_file_count: u64,
        actual_file_count: u64,
        expected_byte_length: u64,
        actual_byte_length: u64,
    },
    /// Supplied package files do not form a valid Complete Package Tree.
    #[error("external package fulfillment for {spec} is malformed at `{path}`: {message}")]
    MalformedExternalPackageTree {
        spec: PackageSpec,
        path: String,
        message: String,
    },
    /// No fulfillment was supplied for declared external Font Containers.
    #[error("external font fulfillment is unavailable for {containers:?}")]
    MissingExternalFontFulfillment {
        containers: Vec<FontContainerIdentity>,
    },
    /// Supplied bytes do not match the exact required Font Container.
    #[error("external font fulfillment for {expected:?} supplied {actual:?}")]
    MismatchedExternalFontContainer {
        expected: FontContainerIdentity,
        actual: FontContainerIdentity,
        expected_length: u64,
        actual_length: u64,
    },
    /// Verified container bytes do not contain one declared face.
    #[error("external font container {container:?} has no valid face at index {index}")]
    MalformedExternalFontContainer {
        container: FontContainerIdentity,
        index: u32,
    },
}

/// A failed Pack-bound request or operation, never an official rejection.
#[derive(Debug, thiserror::Error)]
pub enum PackCompileError {
    #[error("{rejection}")]
    RequestRejected {
        rejection: CompilationRequestRejection,
        request_inventory: Box<CompilationRequestInventory>,
    },
    #[error("{outcome}")]
    Operation {
        outcome: CompilationOperationOutcome,
        request_inventory: Box<CompilationRequestInventory>,
        compilation_identity: CompilationIdentity,
    },
}

impl PackCompileError {
    /// The request inventory retained by either terminal branch.
    pub fn request_inventory(&self) -> &CompilationRequestInventory {
        match self {
            Self::RequestRejected {
                request_inventory, ..
            }
            | Self::Operation {
                request_inventory, ..
            } => request_inventory.as_ref(),
        }
    }

    /// The identity prepared before an operational outcome, if one exists.
    pub fn compilation_identity(&self) -> Option<CompilationIdentity> {
        match self {
            Self::RequestRejected { .. } => None,
            Self::Operation {
                compilation_identity,
                ..
            } => Some(*compilation_identity),
        }
    }
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
    let prepared = prepare_pack_compilation(request, PackCompilationContext::default())?;
    let (world, kernel) = prepared.into_parts();
    Ok(compile_pack_kernel(&world, kernel).result)
}

#[derive(Default)]
pub(crate) struct PackCompilationContext {
    #[cfg(feature = "cli")]
    resource_providers: Vec<Provider>,
}

impl PackCompilationContext {
    #[cfg(feature = "cli")]
    pub(crate) fn resource_provider(
        mut self,
        provider: impl typst_kit::files::FileLoader + Send + Sync + 'static,
    ) -> Self {
        self.resource_providers.push(Box::new(provider));
        self
    }
}

pub(crate) struct PreparedPackCompilation {
    world: PackWorld,
    kernel: PreparedPackCompilationKernel,
}

impl PreparedPackCompilation {
    pub(crate) fn into_parts(self) -> (PackWorld, PreparedPackCompilationKernel) {
        (self.world, self.kernel)
    }
}

pub(crate) struct PreparedPackCompilationKernel {
    format: OutputFormat,
    request_inventory: CompilationRequestInventory,
    compilation_identity: CompilationIdentity,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
    page_selection_implies_untagged_pdf: bool,
}

pub(crate) struct PackCompilationExecution {
    pub(crate) result: CompilationResult,
    pub(crate) presentation: PackCompilationPresentation,
}

pub(crate) enum PackCompilationPresentation {
    Succeeded {
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
    },
    Diagnostics {
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
    },
    PngExport {
        error: String,
        warnings: EcoVec<SourceDiagnostic>,
        pack_warnings: EcoVec<SourceDiagnostic>,
    },
}

pub(crate) fn prepare_pack_compilation(
    request: PackCompilationRequest,
    context: PackCompilationContext,
) -> Result<PreparedPackCompilation, PackCompileError> {
    let PackCompilationRequest {
        pack,
        format,
        mut options,
        inputs,
        overrides,
        features,
        document_time,
        document_timestamp,
        package_fulfillments,
        font_fulfillments,
    } = request;
    let options_value = options.value();
    let mut request_issues = vec![];
    if overrides.value.pack_identity != pack.identity() {
        request_issues.push(CompilationRequestRejection::OverrideSetPackMismatch);
    }
    if features
        .iter()
        .any(|feature| feature.value == Feature::Bundle)
    {
        request_issues.push(CompilationRequestRejection::UnsupportedBundleFeature);
    }
    if format == OutputFormat::Png
        && options_value
            .ppi
            .is_some_and(|ppi| !ppi.is_finite() || ppi <= 0.0)
    {
        request_issues.push(CompilationRequestRejection::InvalidPpi);
    }
    if format == OutputFormat::Pdf {
        if let Err(error) = validate_pdf_standards(&options_value.pdf_standards) {
            request_issues.push(CompilationRequestRejection::InvalidPdfStandards(error));
        }
        let has_page_selection = !options_value.page_selection.ranges().is_empty();
        let tagged = match options_value.pdf_tags {
            Smart::Auto => PdfOptions::default().tagged && !has_page_selection,
            Smart::Custom(tagged) => tagged,
        };
        if has_page_selection && matches!(options_value.pdf_tags, Smart::Custom(true)) {
            request_issues.push(CompilationRequestRejection::PdfTagsWithPageSelection);
        }
        if !tagged && pdf_standard_requiring_tags(&options_value.pdf_standards).is_some() {
            request_issues.push(CompilationRequestRejection::PdfStandardRequiresTags);
        }
    }
    let page_selection_implies_untagged_pdf = format == OutputFormat::Pdf
        && !options.value.page_selection.ranges().is_empty()
        && options.value.pdf_tags.is_auto()
        && PdfOptions::default().tagged;
    let mut ppi_origin = options.origin;
    let mut pdf_tags_origin = options.origin;
    let mut pdf_creation_time_origin = options.origin;
    match format {
        OutputFormat::Png => {
            if options.value.ppi.is_none() {
                options.value.ppi = Some(default_png_ppi());
                ppi_origin = RequestValueOrigin::CoreDefaulted;
            }
        }
        OutputFormat::Pdf => {
            if options.value.pdf_tags.is_auto() {
                options.value.pdf_tags = Smart::Custom(
                    PdfOptions::default().tagged
                        && options.value.page_selection.ranges().is_empty(),
                );
                pdf_tags_origin = if options.value.page_selection.ranges().is_empty() {
                    RequestValueOrigin::CoreDefaulted
                } else {
                    RequestValueOrigin::CoreDerived
                };
            }
            if matches!(
                options.value.creation_timestamp,
                CreationTimestamp::Automatic
            ) {
                options.value.creation_timestamp = CreationTimestamp::Omit;
                pdf_creation_time_origin = RequestValueOrigin::CoreDefaulted;
            }
        }
        OutputFormat::Svg | OutputFormat::Html => {}
    }
    let selected_features = [Feature::Html, Feature::Bundle, Feature::A11yExtras]
        .into_iter()
        .filter_map(|value| {
            features
                .iter()
                .find(|feature| feature.value == value)
                .copied()
        })
        .collect::<Vec<_>>();
    let mut effective_features = selected_features.clone();
    if format == OutputFormat::Html {
        effective_features.retain(|feature| feature.value != Feature::Html);
        effective_features.push(EffectiveEngineFeature {
            value: Feature::Html,
            origin: RequestValueOrigin::CoreDerived,
        });
    }
    let engine_identity = EmbeddedTypst::engine_identity();
    let exporter_identity = EmbeddedTypst::exporter_identity(format);
    let raw_inputs = inputs.value;
    let total_key_bytes = raw_inputs.iter().map(|(key, _)| key.len()).sum();
    let total_value_repr_bytes = raw_inputs.iter().map(|(_, value)| value.repr().len()).sum();
    let inputs_commitment = typst::utils::hash128(&(
        "typst-pack-inputs-v1",
        total_key_bytes,
        total_value_repr_bytes,
        &raw_inputs,
    ));
    let override_inventory = PackOverridesInventory(
        overrides
            .value
            .replacements
            .iter()
            .map(|(path, data)| PackOverrideInventoryEntry {
                path: path.clone(),
                byte_len: data.len(),
                commitment: typst::utils::hash128(&(
                    "typst-pack-override-v1+typst-0.15",
                    "project-file",
                    overrides.value.pack_identity,
                    path,
                    data.len(),
                    data,
                )),
            })
            .collect(),
    );
    let request_inventory = CompilationRequestInventory {
        format: EffectiveRequestValue::new(format, RequestValueOrigin::CallerSupplied),
        options,
        ppi_origin,
        pdf_tags_origin,
        pdf_creation_time_origin,
        inputs: EffectiveRequestValue::new(
            TypstInputsInventory {
                commitment: inputs_commitment,
                entry_count: raw_inputs.len(),
                total_key_bytes,
                total_value_repr_bytes,
            },
            inputs.origin,
        ),
        overrides: EffectiveRequestValue::new(override_inventory, overrides.origin),
        selected_features,
        features: effective_features,
        document_time,
        document_timestamp,
    };
    if let Some(rejection) = CompilationRequestRejection::from_issues(request_issues) {
        return Err(PackCompileError::RequestRejected {
            rejection,
            request_inventory: Box::new(request_inventory),
        });
    }
    let compilation_identity = compilation_identity(
        &pack,
        &request_inventory,
        engine_identity,
        exporter_identity,
    );
    let package_files = package_fulfillments
        .into_iter()
        .map(|(spec, fulfillment)| (spec, fulfillment.files))
        .collect();
    let exact_packages = pack
        .materialize_package_trees(package_files)
        .map_err(|error| PackCompileError::Operation {
            outcome: package_tree_outcome(error),
            request_inventory: Box::new(request_inventory.clone()),
            compilation_identity,
        })?;

    let fulfillment_bytes = font_fulfillments
        .into_iter()
        .map(|(identity, fulfillment)| (identity, fulfillment.data))
        .collect();
    let catalog_fonts = pack
        .materialize_font_catalog(&fulfillment_bytes)
        .map_err(|error| {
            let outcome = match error {
                FontCatalogError::Missing { containers } => {
                    CompilationOperationOutcome::MissingExternalFontFulfillment { containers }
                }
                FontCatalogError::Mismatched {
                    expected,
                    actual,
                    expected_length,
                    actual_length,
                } => CompilationOperationOutcome::MismatchedExternalFontContainer {
                    expected,
                    actual,
                    expected_length,
                    actual_length,
                },
                FontCatalogError::Malformed { container, index } => {
                    CompilationOperationOutcome::MalformedExternalFontContainer { container, index }
                }
            };
            PackCompileError::Operation {
                outcome,
                request_inventory: Box::new(request_inventory.clone()),
                compilation_identity,
            }
        })?;

    let mut world = PackWorld::builder(pack)
        .inputs(raw_inputs)
        .exact_project_overrides(overrides.value.replacements)
        .exact_packages(exact_packages)
        .exact_font_catalog(catalog_fonts);
    #[cfg(feature = "embedded-fonts")]
    {
        world = world.embedded_fonts(false);
    }
    for feature in &request_inventory.features {
        world = world.feature(feature.value);
    }
    #[cfg(feature = "fs")]
    if let Some(timestamp) = request_inventory.document_timestamp.value {
        world = world
            .fixed_timestamp(timestamp)
            .expect("validated adapter-resolved timestamp must remain valid");
    } else if let Some(document_time) = request_inventory.document_time.value {
        world = world.fixed_date(document_time);
    }
    #[cfg(not(feature = "fs"))]
    if let Some(document_time) = request_inventory.document_time.value {
        world = world.fixed_date(document_time);
    }
    #[cfg(feature = "cli")]
    let world = world.resource_providers(context.resource_providers);
    let world = world
        .build()
        .expect("verified Pack dependency snapshots must build a World");

    Ok(PreparedPackCompilation {
        world,
        kernel: PreparedPackCompilationKernel {
            format,
            request_inventory,
            compilation_identity,
            engine_identity,
            exporter_identity,
            page_selection_implies_untagged_pdf,
        },
    })
}

pub(crate) fn compile_pack_kernel(
    world: &PackWorld,
    kernel: PreparedPackCompilationKernel,
) -> PackCompilationExecution {
    let PreparedPackCompilationKernel {
        format,
        request_inventory,
        compilation_identity,
        engine_identity,
        exporter_identity,
        page_selection_implies_untagged_pdf,
    } = kernel;
    let result = match compile_with_default_pdf_timestamp(
        world,
        format,
        request_inventory.options.value(),
        || None,
    ) {
        Ok(output) => {
            let warnings = output.warnings.clone();
            let mut presentation_pack_warnings = output.pack_warnings.clone();
            if page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            let diagnostics = project_diagnostics(
                world,
                output.warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(engine_identity),
            );
            let pack_warnings =
                project_pack_warnings(output.pack_warnings, page_selection_implies_untagged_pdf);
            PackCompilationExecution {
                result: CompilationResult {
                    status: CompilationStatus::Succeeded,
                    artifacts: output.artifacts,
                    diagnostics,
                    pack_warnings,
                    source_page_count: output.source_page_count,
                    request_inventory: request_inventory.clone(),
                    compilation_identity,
                    engine_identity,
                    exporter_identity,
                },
                presentation: PackCompilationPresentation::Succeeded {
                    warnings,
                    pack_warnings: presentation_pack_warnings,
                },
            }
        }
        Err(CompileError::Diagnostics {
            errors,
            warnings,
            pack_warnings,
            phase,
            source_page_count,
        }) => {
            let mut presentation_pack_warnings = pack_warnings.clone();
            if page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            let presentation = PackCompilationPresentation::Diagnostics {
                errors: errors.clone(),
                warnings: warnings.clone(),
                pack_warnings: presentation_pack_warnings,
            };
            let mut diagnostics = project_diagnostics(
                world,
                warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(engine_identity),
            );
            let producer = match phase {
                DiagnosticPhase::Compilation => DiagnosticProducer::Engine(engine_identity),
                DiagnosticPhase::Export => DiagnosticProducer::Exporter(exporter_identity),
            };
            diagnostics.extend(project_diagnostics(world, errors, phase, producer));
            PackCompilationExecution {
                result: CompilationResult {
                    status: CompilationStatus::Rejected,
                    artifacts: vec![],
                    diagnostics,
                    pack_warnings: project_pack_warnings(
                        pack_warnings,
                        page_selection_implies_untagged_pdf,
                    ),
                    source_page_count,
                    request_inventory: request_inventory.clone(),
                    compilation_identity,
                    engine_identity,
                    exporter_identity,
                },
                presentation,
            }
        }
        Err(CompileError::PngExport {
            message,
            warnings,
            pack_warnings,
            source_page_count,
            source_page_number,
        }) => {
            let mut presentation_pack_warnings = pack_warnings.clone();
            if page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            let presentation = PackCompilationPresentation::PngExport {
                error: format!("PNG export failed for source page {source_page_number}: {message}"),
                warnings: warnings.clone(),
                pack_warnings: presentation_pack_warnings,
            };
            let mut diagnostics = project_diagnostics(
                world,
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
                source_page_number: Some(source_page_number),
            });
            PackCompilationExecution {
                result: CompilationResult {
                    status: CompilationStatus::Rejected,
                    artifacts: vec![],
                    diagnostics,
                    pack_warnings: project_pack_warnings(
                        pack_warnings,
                        page_selection_implies_untagged_pdf,
                    ),
                    source_page_count: Some(source_page_count),
                    request_inventory: request_inventory.clone(),
                    compilation_identity,
                    engine_identity,
                    exporter_identity,
                },
                presentation,
            }
        }
        Err(CompileError::InvalidPdfStandards(error)) => {
            unreachable!("PDF standards are validated during request preparation: {error}");
        }
    };
    result
}

pub(crate) fn package_tree_outcome(error: PackageTreeError) -> CompilationOperationOutcome {
    match error {
        PackageTreeError::Missing { packages } => {
            CompilationOperationOutcome::MissingExternalPackageFulfillment { packages }
        }
        PackageTreeError::Mismatched {
            spec,
            expected,
            actual,
            expected_file_count,
            actual_file_count,
            expected_byte_length,
            actual_byte_length,
        } => CompilationOperationOutcome::MismatchedExternalPackageTree {
            spec,
            expected,
            actual,
            expected_file_count,
            actual_file_count,
            expected_byte_length,
            actual_byte_length,
        },
        PackageTreeError::Malformed {
            spec,
            path,
            message,
        } => CompilationOperationOutcome::MalformedExternalPackageTree {
            spec,
            path,
            message,
        },
    }
}

fn compilation_identity(
    pack: &Pack,
    inventory: &CompilationRequestInventory,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
) -> CompilationIdentity {
    let options = inventory.options.value();
    let page_selection = canonical_page_selection(&options.page_selection);
    let output_digest = match inventory.format.value {
        OutputFormat::Pdf => {
            let mut standards = options
                .pdf_standards
                .iter()
                .map(pdf_standard_identity)
                .collect::<Vec<_>>();
            standards.sort_unstable();
            typst::utils::hash128(&(
                "pdf",
                &page_selection,
                &options.pdf_identifier,
                &options.pdf_creator,
                options.pdf_tags,
                options.creation_timestamp,
                standards,
                options.pretty,
            ))
        }
        OutputFormat::Png => typst::utils::hash128(&(
            "png",
            &page_selection,
            options.ppi.map(f64::to_bits),
            options.render_bleed,
        )),
        OutputFormat::Svg => {
            typst::utils::hash128(&("svg", &page_selection, options.render_bleed, options.pretty))
        }
        OutputFormat::Html => typst::utils::hash128(&("html", options.pretty)),
    };
    let feature_values = inventory
        .features
        .iter()
        .map(|feature| feature.value)
        .collect::<Vec<_>>();
    CompilationIdentity(typst::utils::hash128(&(
        "typst-pack-compilation-v1",
        pack.identity(),
        inventory.format.value,
        output_digest,
        inventory.inputs.value.commitment,
        inventory
            .overrides
            .value
            .iter()
            .map(|entry| (&entry.path, entry.byte_len, entry.commitment))
            .collect::<Vec<_>>(),
        feature_values,
        inventory.document_time.value,
        inventory.document_timestamp.value,
        engine_identity,
        exporter_identity,
    )))
}

fn canonical_page_selection(selection: &PageSelection) -> (bool, Vec<(usize, usize)>) {
    let selects_all = selection.ranges.is_empty();
    let mut ranges = selection
        .ranges
        .iter()
        .filter_map(|range| {
            let start = range.start().map_or(1, NonZeroUsize::get);
            let end = range.end().map_or(usize::MAX, NonZeroUsize::get);
            (start <= end).then_some((start, end))
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    let mut canonical: Vec<(usize, usize)> = vec![];
    for (start, end) in ranges {
        if let Some(last) = canonical.last_mut()
            && start <= last.1.saturating_add(1)
        {
            last.1 = last.1.max(end);
        } else {
            canonical.push((start, end));
        }
    }
    (selects_all, canonical)
}

fn pdf_standard_identity(standard: &PdfStandard) -> &'static str {
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
        _ => unreachable!("all standards in pinned typst-pdf are represented"),
    }
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
        pack_warnings.push(page_selection_pdf_tags_warning());
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
                let ppi = options.ppi.unwrap_or_else(default_png_ppi);
                let render_options = typst_render::RenderOptions {
                    pixel_per_pt: (ppi / 72.0).into(),
                    render_bleed: options.render_bleed,
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
                                source_page_number,
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
                    render_bleed: options.render_bleed,
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

fn default_png_ppi() -> f64 {
    typst_render::RenderOptions::default().pixel_per_pt.get() * 72.0
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
            source_page_number: None,
        })
        .collect()
}

fn project_pack_warnings(
    warnings: impl IntoIterator<Item = SourceDiagnostic>,
    page_selection_implies_untagged_pdf: bool,
) -> Vec<PackCompilationWarning> {
    warnings
        .into_iter()
        .chain(page_selection_implies_untagged_pdf.then(page_selection_pdf_tags_warning))
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

fn page_selection_pdf_tags_warning() -> SourceDiagnostic {
    SourceDiagnostic::warning(Span::detached(), "using --pages implies --no-pdf-tags").with_hints([
        "the resulting PDF will be inaccessible".into(),
        "add --no-pdf-tags to silence this warning".into(),
    ])
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
