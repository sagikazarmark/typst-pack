//! Compiling a pack into Compilation Output Artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::ops::Range;

use ecow::EcoVec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use typst::diag::{Severity, SourceDiagnostic, Tracepoint, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Repr, Smart};
use typst::syntax::package::PackageSpec;
use typst::syntax::{DiagSpan, Span};
use typst::{Feature, World, WorldExt};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use crate::embedded::EmbeddedTypst;
use crate::payload::SharedBytes;
use crate::world::PackWorld;
use crate::world_trace::{WorldTrace, logical_path};
use crate::{FontContainer, FontContainerIdentity, Pack, PackageTree, PackageTreeIdentity};

/// A resource bounded during compilation artifact export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompilationResource {
    SourcePages,
    Artifacts,
    PixelsPerArtifact,
    TotalPixels,
    ArtifactBytes,
    RetainedArtifactBytes,
    ExportWorkers,
}

/// A supplied compilation ceiling that cannot support bounded accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: CompilationResource,
        ceiling: u64,
    },
    #[error("the ExportWorkers ceiling must be greater than zero")]
    ZeroWorkers,
}

/// A mandatory compilation export ceiling was exceeded or could not be accounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationLimitError {
    #[error(
        "compilation {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: CompilationResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("compilation {resource:?} accounting overflowed")]
    AccountingOverflow { resource: CompilationResource },
}

/// Mandatory finite resource ceilings for compilation artifact export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilationLimits {
    source_pages: u64,
    artifacts: u64,
    pixels_per_artifact: u64,
    total_pixels: u64,
    artifact_bytes: u64,
    retained_artifact_bytes: u64,
    export_workers: u64,
}

impl CompilationLimits {
    /// Constructs validated mandatory finite compilation ceilings.
    pub fn new(
        source_pages: u64,
        artifacts: u64,
        pixels_per_artifact: u64,
        total_pixels: u64,
        artifact_bytes: u64,
        retained_artifact_bytes: u64,
        export_workers: u64,
    ) -> Result<Self, CompilationLimitsError> {
        let ceilings = [
            (CompilationResource::SourcePages, source_pages),
            (CompilationResource::Artifacts, artifacts),
            (CompilationResource::PixelsPerArtifact, pixels_per_artifact),
            (CompilationResource::TotalPixels, total_pixels),
            (CompilationResource::ArtifactBytes, artifact_bytes),
            (
                CompilationResource::RetainedArtifactBytes,
                retained_artifact_bytes,
            ),
            (CompilationResource::ExportWorkers, export_workers),
        ];
        if let Some((resource, ceiling)) = ceilings
            .into_iter()
            .find(|(_, ceiling)| *ceiling == u64::MAX)
        {
            return Err(CompilationLimitsError::CannotProbe { resource, ceiling });
        }
        if export_workers == 0 {
            return Err(CompilationLimitsError::ZeroWorkers);
        }
        Ok(Self {
            source_pages,
            artifacts,
            pixels_per_artifact,
            total_pixels,
            artifact_bytes,
            retained_artifact_bytes,
            export_workers,
        })
    }

    /// The first-party bounded compilation export profile.
    pub const fn reference_v1() -> Self {
        Self {
            source_pages: 10_000,
            artifacts: 10_000,
            pixels_per_artifact: 100_000_000,
            total_pixels: 1_000_000_000,
            artifact_bytes: 512 * 1024 * 1024,
            retained_artifact_bytes: 2 * 1024 * 1024 * 1024,
            export_workers: 4,
        }
    }

    /// Replaces the export-worker ceiling while preserving every other limit.
    pub fn with_export_workers(self, export_workers: u64) -> Result<Self, CompilationLimitsError> {
        Self::new(
            self.source_pages,
            self.artifacts,
            self.pixels_per_artifact,
            self.total_pixels,
            self.artifact_bytes,
            self.retained_artifact_bytes,
            export_workers,
        )
    }

    pub const fn source_pages(self) -> u64 {
        self.source_pages
    }

    pub const fn artifacts(self) -> u64 {
        self.artifacts
    }

    pub const fn pixels_per_artifact(self) -> u64 {
        self.pixels_per_artifact
    }

    pub const fn total_pixels(self) -> u64 {
        self.total_pixels
    }

    pub const fn artifact_bytes(self) -> u64 {
        self.artifact_bytes
    }

    pub const fn retained_artifact_bytes(self) -> u64 {
        self.retained_artifact_bytes
    }

    pub const fn export_workers(self) -> u64 {
        self.export_workers
    }
}

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
            feature_set: env!("TYPST_PACK_FEATURE_SET"),
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
    /// HTML export is experimental in Typst. Pack-bound compilation derives the
    /// required [`Feature::Html`](typst::Feature::Html) engine feature.
    Html,
}

/// The Typst document model selected for creation or compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypstTarget {
    /// A paged document used by PDF and image formats.
    Paged,
    /// An HTML document.
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

/// Semantic controls for PDF output.
#[derive(Debug, Clone)]
pub struct PdfOutputSpecification {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// PDF standards to enforce through the official exporter.
    pub standards: Vec<PdfStandard>,
    /// The PDF file identifier, using the official exporter's automatic mode by default.
    pub identifier: Smart<String>,
    /// The PDF creator metadata, using the official exporter's automatic mode by default.
    pub creator: Smart<Option<String>>,
    /// Whether PDF accessibility tags should be emitted.
    ///
    /// Automatic tagging is disabled with a warning for a page subset, matching
    /// Typst's CLI. Explicit tagging is passed through to the exporter.
    pub tags: Smart<bool>,
    /// How the document creation datetime is recorded in PDF metadata.
    pub creation_timestamp: CreationTimestamp,
    /// Whether to pretty-print PDF output.
    pub pretty: bool,
}

impl Default for PdfOutputSpecification {
    fn default() -> Self {
        let pdf = PdfOptions::default();
        Self {
            page_selection: PageSelection::default(),
            standards: vec![],
            identifier: pdf.ident,
            creator: pdf.creator,
            tags: Smart::Auto,
            creation_timestamp: CreationTimestamp::Automatic,
            pretty: pdf.pretty,
        }
    }
}

/// Semantic controls for PNG output.
#[derive(Debug, Clone, Default)]
pub struct PngOutputSpecification {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Pixels per inch. `None` selects the core default of 144.
    pub pixels_per_inch: Option<f64>,
    /// Whether to render into the page bleed region.
    pub render_bleed: bool,
}

/// Semantic controls for SVG output.
#[derive(Debug, Clone, Default)]
pub struct SvgOutputSpecification {
    /// Which source pages to export.
    pub page_selection: PageSelection,
    /// Whether to render into the page bleed region.
    pub render_bleed: bool,
    /// Whether to pretty-print SVG output.
    pub pretty: bool,
}

/// Semantic controls for HTML output.
#[derive(Debug, Clone, Default)]
pub struct HtmlOutputSpecification {
    /// Whether to pretty-print HTML output.
    pub pretty: bool,
}

/// The required tagged semantic output request.
#[derive(Debug, Clone)]
pub enum CompilationOutputSpecification {
    Pdf(PdfOutputSpecification),
    Png(PngOutputSpecification),
    Svg(SvgOutputSpecification),
    Html(HtmlOutputSpecification),
}

impl CompilationOutputSpecification {
    /// The output format represented by this specification.
    pub fn format(&self) -> OutputFormat {
        match self {
            Self::Pdf(_) => OutputFormat::Pdf,
            Self::Png(_) => OutputFormat::Png,
            Self::Svg(_) => OutputFormat::Svg,
            Self::Html(_) => OutputFormat::Html,
        }
    }

    fn target(&self) -> TypstTarget {
        match self {
            Self::Html(_) => TypstTarget::Html,
            Self::Pdf(_) | Self::Png(_) | Self::Svg(_) => TypstTarget::Paged,
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

/// The exact or explicitly absent time used by Typst document-time requests.
#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub enum DocumentTime {
    /// Document-time requests have no value.
    Absent,
    /// Return one fixed Typst date or datetime.
    Fixed(Datetime),
    /// Resolve one fixed UTC instant under each requested timezone offset.
    UnixTimestamp(i64),
}

impl DocumentTime {
    fn identity_projection(self) -> (Option<Datetime>, Option<i64>) {
        match self {
            Self::Absent => (None, None),
            Self::Fixed(datetime) => (Some(datetime), None),
            Self::UnixTimestamp(timestamp) => (None, Some(timestamp)),
        }
    }
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

struct RequestValueCandidates<T> {
    caller: Option<T>,
    adapter: Option<T>,
    core_default: Option<T>,
}

impl<T> RequestValueCandidates<T> {
    fn caller(value: T) -> Self {
        Self {
            caller: Some(value),
            adapter: None,
            core_default: None,
        }
    }

    fn adapter(value: T) -> Self {
        Self {
            caller: None,
            adapter: Some(value),
            core_default: None,
        }
    }

    fn core_default(value: T) -> Self {
        Self {
            caller: None,
            adapter: None,
            core_default: Some(value),
        }
    }

    fn set_caller(&mut self, value: T) {
        self.caller = Some(value);
    }

    fn set_adapter(&mut self, value: T) {
        self.adapter = Some(value);
    }

    fn resolve(self) -> EffectiveRequestValue<T> {
        if let Some(value) = self.caller {
            EffectiveRequestValue::new(value, RequestValueOrigin::CallerSupplied)
        } else if let Some(value) = self.adapter {
            EffectiveRequestValue::new(value, RequestValueOrigin::AdapterResolved)
        } else {
            EffectiveRequestValue::new(
                self.core_default
                    .expect("request value must have a default or supplied candidate"),
                RequestValueOrigin::CoreDefaulted,
            )
        }
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
    output_specification: EffectiveRequestValue<CompilationOutputSpecification>,
    output_origins: CompilationOutputOrigins,
    inputs: EffectiveRequestValue<TypstInputsInventory>,
    overrides: EffectiveRequestValue<PackOverridesInventory>,
    selected_features: Vec<EffectiveEngineFeature>,
    features: Vec<EffectiveEngineFeature>,
    document_time: EffectiveRequestValue<DocumentTime>,
}

impl CompilationRequestInventory {
    /// The tagged output controls, including their deterministic defaults.
    pub fn output_specification(&self) -> &EffectiveRequestValue<CompilationOutputSpecification> {
        &self.output_specification
    }

    /// Safe evidence for the exact `sys.inputs` dictionary.
    pub fn inputs(&self) -> &EffectiveRequestValue<TypstInputsInventory> {
        &self.inputs
    }

    /// Safe evidence for the exact Pack Override Set.
    pub fn overrides(&self) -> &EffectiveRequestValue<PackOverridesInventory> {
        &self.overrides
    }

    /// Format-specific provenance for effective output controls.
    pub fn output_origins(&self) -> CompilationOutputOrigins {
        self.output_origins
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
    pub fn document_time(&self) -> &EffectiveRequestValue<DocumentTime> {
        &self.document_time
    }
}

/// Format-specific provenance for output controls resolved during preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationOutputOrigins {
    Pdf {
        tags: RequestValueOrigin,
        creation_time: RequestValueOrigin,
    },
    Png {
        pixels_per_inch: RequestValueOrigin,
    },
    Svg,
    Html,
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
pub struct PackCompilationRequest {
    pack: Pack,
    output_specification: RequestValueCandidates<CompilationOutputSpecification>,
    inputs: RequestValueCandidates<Dict>,
    overrides: RequestValueCandidates<PackOverrideSet>,
    features: Vec<EffectiveEngineFeature>,
    document_time: RequestValueCandidates<DocumentTime>,
    fulfillments: CompilationFulfillmentSet,
}

/// One externally acquired Package Tree and operational metadata.
#[derive(Debug, Clone)]
pub struct PackageTreeFulfillment {
    spec: PackageSpec,
    tree: PackageTree,
    provenance: Option<String>,
    cache_hit: bool,
}

impl PackageTreeFulfillment {
    /// Pairs one exact package specification with its validated tree.
    pub fn new(spec: PackageSpec, tree: PackageTree) -> Self {
        Self {
            spec,
            tree,
            provenance: None,
            cache_hit: false,
        }
    }

    /// The exact package specification this fulfillment claims to satisfy.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The already validated Package Tree.
    pub fn tree(&self) -> &PackageTree {
        &self.tree
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

/// One externally acquired Font Container and operational metadata.
#[derive(Debug, Clone)]
pub struct FontContainerFulfillment {
    expected_identity: FontContainerIdentity,
    container: FontContainer,
    provenance: Option<String>,
    licensing: Option<String>,
}

impl FontContainerFulfillment {
    /// Pairs one required Font Container Identity with a validated container.
    pub fn new(expected_identity: FontContainerIdentity, container: FontContainer) -> Self {
        Self {
            expected_identity,
            container,
            provenance: None,
            licensing: None,
        }
    }

    /// The Font Container Identity this fulfillment claims to satisfy.
    pub fn expected_identity(&self) -> FontContainerIdentity {
        self.expected_identity
    }

    /// The already validated Font Container.
    pub fn container(&self) -> &FontContainer {
        &self.container
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

/// One construction issue in a Compilation Fulfillment Set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationFulfillmentSetIssue {
    /// An exact package specification was supplied more than once.
    #[error("package specification {spec} is fulfilled more than once")]
    DuplicatePackageSpecification { spec: PackageSpec },
    /// A required Font Container Identity was supplied more than once.
    #[error("Font Container Identity {identity:?} is fulfilled more than once")]
    DuplicateFontContainerIdentity { identity: FontContainerIdentity },
}

/// A failure to construct a duplicate-free Compilation Fulfillment Set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Compilation Fulfillment Set construction failed with {} issue(s)", .issues.len())]
pub struct CompilationFulfillmentSetError {
    issues: Vec<CompilationFulfillmentSetIssue>,
}

impl CompilationFulfillmentSetError {
    /// Every duplicate in canonical package-then-font key order.
    pub fn issues(&self) -> &[CompilationFulfillmentSetIssue] {
        &self.issues
    }
}

/// The duplicate-free external dependencies supplied for one compilation.
#[derive(Debug, Clone, Default)]
pub struct CompilationFulfillmentSet {
    packages: BTreeMap<String, PackageTreeFulfillment>,
    fonts: BTreeMap<FontContainerIdentity, FontContainerFulfillment>,
}

impl CompilationFulfillmentSet {
    /// Constructs one canonical duplicate-free fulfillment set.
    pub fn new(
        packages: impl IntoIterator<Item = PackageTreeFulfillment>,
        fonts: impl IntoIterator<Item = FontContainerFulfillment>,
    ) -> Result<Self, CompilationFulfillmentSetError> {
        let packages = packages.into_iter().collect::<Vec<_>>();
        let fonts = fonts.into_iter().collect::<Vec<_>>();
        let mut package_keys = BTreeSet::new();
        let mut duplicate_packages = BTreeSet::new();
        for fulfillment in &packages {
            let key = fulfillment.spec.to_string();
            if !package_keys.insert(key.clone()) {
                duplicate_packages.insert(key);
            }
        }
        let mut font_keys = BTreeSet::new();
        let mut duplicate_fonts = BTreeSet::new();
        for fulfillment in &fonts {
            if !font_keys.insert(fulfillment.expected_identity) {
                duplicate_fonts.insert(fulfillment.expected_identity);
            }
        }
        let mut issues = duplicate_packages
            .into_iter()
            .map(|key| {
                let spec = packages
                    .iter()
                    .find(|fulfillment| fulfillment.spec.to_string() == key)
                    .expect("duplicate key came from a package fulfillment")
                    .spec
                    .clone();
                CompilationFulfillmentSetIssue::DuplicatePackageSpecification { spec }
            })
            .collect::<Vec<_>>();
        issues.extend(duplicate_fonts.into_iter().map(|identity| {
            CompilationFulfillmentSetIssue::DuplicateFontContainerIdentity { identity }
        }));
        if !issues.is_empty() {
            return Err(CompilationFulfillmentSetError { issues });
        }
        Ok(Self {
            packages: packages
                .into_iter()
                .map(|fulfillment| (fulfillment.spec.to_string(), fulfillment))
                .collect(),
            fonts: fonts
                .into_iter()
                .map(|fulfillment| (fulfillment.expected_identity, fulfillment))
                .collect(),
        })
    }

    /// An empty fulfillment set for a self-contained Pack.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Package Tree fulfillments in canonical exact-specification order.
    pub fn packages(&self) -> impl ExactSizeIterator<Item = &PackageTreeFulfillment> {
        self.packages.values()
    }

    /// Font Container fulfillments in canonical identity order.
    pub fn fonts(&self) -> impl ExactSizeIterator<Item = &FontContainerFulfillment> {
        self.fonts.values()
    }
}

impl PackCompilationRequest {
    fn with_output_candidate(
        pack: Pack,
        output_specification: RequestValueCandidates<CompilationOutputSpecification>,
    ) -> Self {
        let overrides = PackOverrideSet::new(&pack);
        Self {
            pack,
            output_specification,
            inputs: RequestValueCandidates::core_default(Dict::new()),
            overrides: RequestValueCandidates::core_default(overrides),
            features: Vec::new(),
            document_time: RequestValueCandidates::core_default(DocumentTime::Absent),
            fulfillments: CompilationFulfillmentSet::empty(),
        }
    }

    /// Binds a validated Pack to a tagged semantic output specification.
    pub fn new(pack: Pack, output_specification: CompilationOutputSpecification) -> Self {
        Self::with_output_candidate(pack, RequestValueCandidates::caller(output_specification))
    }

    /// Starts a request whose initial output controls were resolved by an adapter.
    pub fn new_with_adapter_resolved_output(
        pack: Pack,
        output_specification: CompilationOutputSpecification,
    ) -> Self {
        Self::with_output_candidate(pack, RequestValueCandidates::adapter(output_specification))
    }

    /// Sets caller-supplied output controls.
    pub fn output(mut self, output_specification: CompilationOutputSpecification) -> Self {
        self.output_specification.set_caller(output_specification);
        self
    }

    /// Sets output controls resolved by an adapter.
    pub fn adapter_resolved_output(
        mut self,
        output_specification: CompilationOutputSpecification,
    ) -> Self {
        self.output_specification.set_adapter(output_specification);
        self
    }

    /// Sets the exact values exposed to document code as `sys.inputs`.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs.set_caller(inputs);
        self
    }

    /// Sets `sys.inputs` after an adapter has resolved its external defaults.
    pub fn adapter_resolved_inputs(mut self, inputs: Dict) -> Self {
        self.inputs.set_adapter(inputs);
        self
    }

    /// Applies one immutable Pack-bound Pack Override Set.
    pub fn overrides(mut self, overrides: PackOverrideSet) -> Self {
        self.overrides.set_caller(overrides);
        self
    }

    /// Applies a Pack Override Set resolved by an adapter.
    pub fn adapter_resolved_overrides(mut self, overrides: PackOverrideSet) -> Self {
        self.overrides.set_adapter(overrides);
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

    /// Sets the exact value returned by document-time requests.
    pub fn document_time(mut self, document_time: DocumentTime) -> Self {
        self.document_time.set_caller(document_time);
        self
    }

    /// Sets the exact document time resolved by an adapter.
    pub fn adapter_resolved_document_time(mut self, document_time: DocumentTime) -> Self {
        self.document_time.set_adapter(document_time);
        self
    }

    /// Supplies the complete duplicate-free external fulfillment set.
    pub fn fulfillments(mut self, fulfillments: CompilationFulfillmentSet) -> Self {
        self.fulfillments = fulfillments;
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
    bytes: SharedBytes,
    source_page_number: Option<NonZeroUsize>,
}

/// Whether the official compiler and exporter accepted the compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilationStatus {
    Succeeded,
    Rejected,
}

/// The official phase that emitted a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticPhase {
    Compilation,
    Export,
}

/// The exact embedded implementation that emitted a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticProducer {
    Engine(EngineIdentity),
    Exporter(ExporterIdentity),
}

/// Official Typst diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A source location expressed in the Pack's logical namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TracepointKind {
    Call,
    Show,
    Import,
    Include,
}

/// One structured tracepoint attached to an official diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// The stable document facts reached before complete export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationDocumentSummary {
    target: TypstTarget,
    source_page_count: Option<usize>,
}

impl CompilationDocumentSummary {
    pub fn target(self) -> TypstTarget {
        self.target
    }

    pub fn source_page_count(self) -> Option<usize> {
        self.source_page_count
    }
}

/// The kind of dependency request made by the embedded engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilationAccessKind {
    Source,
    File,
    Font,
}

/// The stable outcome of one dependency request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilationAccessOutcome {
    Read {
        byte_length: usize,
        digest: [u8; 16],
    },
    Missing,
    Failed,
}

/// One canonical dependency observation made by the embedded engine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompilationAccessObservation {
    kind: CompilationAccessKind,
    logical_path: String,
    font_index: Option<usize>,
    outcome: CompilationAccessOutcome,
}

impl CompilationAccessObservation {
    pub(crate) fn new(
        kind: CompilationAccessKind,
        logical_path: String,
        font_index: Option<usize>,
        outcome: CompilationAccessOutcome,
    ) -> Self {
        Self {
            kind,
            logical_path,
            font_index,
            outcome,
        }
    }

    pub fn kind(&self) -> CompilationAccessKind {
        self.kind
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub fn font_index(&self) -> Option<usize> {
        self.font_index
    }

    pub fn outcome(&self) -> &CompilationAccessOutcome {
        &self.outcome
    }
}

/// Canonical accesses retained by a semantic compilation result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CompilationAccessTrace {
    observations: BTreeSet<CompilationAccessObservation>,
}

impl CompilationAccessTrace {
    pub fn observations(&self) -> impl Iterator<Item = &CompilationAccessObservation> {
        self.observations.iter()
    }

    pub(crate) fn from_observations(observations: BTreeSet<CompilationAccessObservation>) -> Self {
        Self { observations }
    }
}

/// The identity of one complete compiler and exporter result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationResultIdentity(u128);

impl CompilationResultIdentity {
    pub fn kind(self) -> &'static str {
        "compilation-result"
    }

    pub fn schema(self) -> &'static str {
        "typst-pack-compilation-result-v1"
    }

    pub fn algorithm(self) -> &'static str {
        "typst-hash128-0.15"
    }

    pub fn digest(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

/// Operational evidence retained for one exact package fulfillment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFulfillmentReport {
    spec: PackageSpec,
    required_tree_identity: Option<PackageTreeIdentity>,
    supplied_tree_identity: Option<PackageTreeIdentity>,
    declared: bool,
    embedded: bool,
    provenance: Option<String>,
    cache_hit: bool,
}

impl PackageFulfillmentReport {
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }
    /// The declared required tree, or `None` when the specification is undeclared.
    pub fn required_tree_identity(&self) -> Option<PackageTreeIdentity> {
        self.required_tree_identity
    }
    pub fn supplied_tree_identity(&self) -> Option<PackageTreeIdentity> {
        self.supplied_tree_identity
    }
    pub fn declared(&self) -> bool {
        self.declared
    }
    pub fn embedded(&self) -> bool {
        self.embedded
    }
    pub fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }
    pub fn cache_hit(&self) -> bool {
        self.cache_hit
    }
}

/// Operational evidence retained for one exact font fulfillment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFulfillmentReport {
    container_identity: FontContainerIdentity,
    supplied_container_identity: Option<FontContainerIdentity>,
    declared: bool,
    embedded: bool,
    provenance: Option<String>,
    licensing: Option<String>,
}

impl FontFulfillmentReport {
    /// The declared or claimed identity used as the fulfillment-set key.
    pub fn container_identity(&self) -> FontContainerIdentity {
        self.container_identity
    }
    pub fn supplied_container_identity(&self) -> Option<FontContainerIdentity> {
        self.supplied_container_identity
    }
    pub fn declared(&self) -> bool {
        self.declared
    }
    pub fn embedded(&self) -> bool {
        self.embedded
    }
    pub fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }
    pub fn licensing(&self) -> Option<&str> {
        self.licensing.as_deref()
    }
}

/// Operational dependency evidence surrounding one official semantic result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompilationFulfillmentReport {
    packages: Vec<PackageFulfillmentReport>,
    fonts: Vec<FontFulfillmentReport>,
}

impl CompilationFulfillmentReport {
    pub fn packages(&self) -> &[PackageFulfillmentReport] {
        &self.packages
    }
    pub fn fonts(&self) -> &[FontFulfillmentReport] {
        &self.fonts
    }
}

/// The immutable account of an accepted compilation through complete export.
#[derive(Debug, Clone)]
pub struct CompilationReport {
    outcome: CompilationReportOutcome,
    fulfillments: CompilationFulfillmentReport,
}

#[derive(Debug, Clone)]
pub enum CompilationReportOutcome {
    Result(Box<CompilationResult>),
    Operation {
        outcome: CompilationOperationOutcome,
        request_inventory: Box<CompilationRequestInventory>,
        compilation_identity: CompilationIdentity,
    },
}

impl CompilationReport {
    pub fn outcome(&self) -> &CompilationReportOutcome {
        &self.outcome
    }

    pub fn result(&self) -> Option<&CompilationResult> {
        match &self.outcome {
            CompilationReportOutcome::Result(result) => Some(result.as_ref()),
            CompilationReportOutcome::Operation { .. } => None,
        }
    }
    pub fn fulfillments(&self) -> &CompilationFulfillmentReport {
        &self.fulfillments
    }
}

/// The semantic result of an accepted Pack compilation request.
#[derive(Debug, Clone)]
pub struct CompilationResult {
    status: CompilationStatus,
    artifacts: Vec<CompilationArtifact>,
    diagnostics: Vec<CompilationDiagnostic>,
    pack_warnings: Vec<PackCompilationWarning>,
    document: CompilationDocumentSummary,
    access_trace: CompilationAccessTrace,
    result_identity: CompilationResultIdentity,
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
        self.document.source_page_count
    }

    pub fn document(&self) -> CompilationDocumentSummary {
        self.document
    }

    pub fn access_trace(&self) -> &CompilationAccessTrace {
        &self.access_trace
    }

    pub fn result_identity(&self) -> CompilationResultIdentity {
        self.result_identity
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        self.bytes.as_slice()
    }

    pub(crate) fn shared_bytes(&self) -> &SharedBytes {
        &self.bytes
    }

    /// Extracts the owned artifact bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes.into_vec()
    }
}

/// The result of compiling a pack.
#[derive(Debug, Clone)]
pub(crate) struct CompilationOutput {
    /// The produced Compilation Output Artifacts.
    pub artifacts: Vec<CompilationArtifact>,
    /// Warnings emitted during compilation.
    pub warnings: EcoVec<SourceDiagnostic>,
    pack_warnings: EcoVec<SourceDiagnostic>,
    source_page_count: Option<usize>,
}

/// A failed compilation.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CompileError {
    /// Compilation artifact export exceeded a mandatory operational ceiling.
    #[error(transparent)]
    Limit(#[from] CompilationLimitError),
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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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

/// One independently detectable issue in a rejected semantic request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationRequestIssue {
    /// A page range ends before it starts.
    #[error("page selection range {start}-{end} ends before it starts")]
    InvalidPageRange {
        start: NonZeroUsize,
        end: NonZeroUsize,
    },
    /// The Pack compilation contract intentionally excludes Typst Bundle.
    #[error("the Typst Bundle feature is not supported for Pack compilation")]
    UnsupportedBundleFeature,
    /// PNG resolution must be finite and greater than zero.
    #[error("PNG pixels per inch must be finite and greater than zero")]
    InvalidPpi,
    /// The official PDF standards validator rejected the requested set.
    #[error(transparent)]
    InvalidPdfStandards(PdfStandardsValidationError),
    /// The Pack Override Set was preflighted against a different Pack.
    #[error("the Pack Override Set is bound to a different Pack")]
    OverrideSetPackMismatch,
    /// The document-time Unix timestamp cannot be represented.
    #[error("the document-time UNIX timestamp is out of range")]
    InvalidDocumentTimestamp,
}

/// A rejected semantic request and its complete effective inventory.
#[derive(Debug)]
pub struct CompilationRequestRejection {
    issues: Vec<CompilationRequestIssue>,
    request_inventory: Box<CompilationRequestInventory>,
}

impl CompilationRequestRejection {
    /// The independently detectable request issues in stable order.
    pub fn issues(&self) -> &[CompilationRequestIssue] {
        &self.issues
    }

    /// Every effective semantic request value retained before rejection.
    pub fn request_inventory(&self) -> &CompilationRequestInventory {
        &self.request_inventory
    }

    fn new(
        issues: Vec<CompilationRequestIssue>,
        request_inventory: CompilationRequestInventory,
    ) -> Self {
        debug_assert!(!issues.is_empty());
        Self {
            issues,
            request_inventory: Box::new(request_inventory),
        }
    }
}

impl std::fmt::Display for CompilationRequestRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let [issue] = self.issues.as_slice() {
            issue.fmt(formatter)
        } else {
            formatter.write_str("the compilation request contains multiple invalid values")
        }
    }
}

impl std::error::Error for CompilationRequestRejection {}

/// One exact-set deviation detected before private World materialization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationFulfillmentIssue {
    #[error("external package fulfillment for {spec} is missing")]
    MissingExternalPackage { spec: PackageSpec },
    #[error("package fulfillment for undeclared specification {spec} supplied {actual:?}")]
    UndeclaredPackage {
        spec: PackageSpec,
        actual: PackageTreeIdentity,
    },
    #[error("embedded package {spec} was unexpectedly fulfilled externally")]
    UnexpectedEmbeddedPackage { spec: PackageSpec },
    #[error("package fulfillment for {spec} supplied {actual:?}, expected {expected:?}")]
    MismatchedPackageTree {
        spec: PackageSpec,
        expected: PackageTreeIdentity,
        actual: PackageTreeIdentity,
        expected_file_count: u64,
        actual_file_count: u64,
        expected_byte_length: u64,
        actual_byte_length: u64,
    },
    #[error("external Font Container fulfillment for {identity:?} is missing")]
    MissingExternalFont { identity: FontContainerIdentity },
    #[error("Font Container fulfillment for undeclared identity {identity:?} supplied {actual:?}")]
    UndeclaredFont {
        identity: FontContainerIdentity,
        actual: FontContainerIdentity,
    },
    #[error("embedded Font Container {identity:?} was unexpectedly fulfilled externally")]
    UnexpectedEmbeddedFont { identity: FontContainerIdentity },
    #[error("Font Container fulfillment for {expected:?} supplied {actual:?}")]
    MismatchedFontContainer {
        expected: FontContainerIdentity,
        actual: FontContainerIdentity,
        expected_length: u64,
        actual_length: u64,
    },
    #[error("Font Container {identity:?} has no required face at index {index}")]
    MissingFontFace {
        identity: FontContainerIdentity,
        index: u32,
    },
}

/// Complete canonical evidence that a fulfillment set is not exact.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Compilation Fulfillment Set has {} deviation(s)", .issues.len())]
pub struct InvalidCompilationFulfillmentSet {
    issues: Vec<CompilationFulfillmentIssue>,
}

impl InvalidCompilationFulfillmentSet {
    /// Every deviation in domain-role, canonical-key, and issue-kind order.
    pub fn issues(&self) -> &[CompilationFulfillmentIssue] {
        &self.issues
    }
}

/// A Pack-owned operational outcome after request acceptance and before a semantic result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompilationOperationOutcome {
    #[error(transparent)]
    InvalidFulfillmentSet(InvalidCompilationFulfillmentSet),
    #[error(transparent)]
    ResourceLimit(CompilationLimitError),
}

pub(crate) enum PackCompilationPreparation {
    Execute {
        world: PackWorld,
        kernel: PreparedPackCompilationKernel,
    },
    Report(CompilationReport),
    Rejected(CompilationRequestRejection),
}

/// Compiles one private adapter world through the embedded Typst implementation.
#[cfg(test)]
pub(crate) fn compile_world(
    world: &dyn World,
    output: &CompilationOutputSpecification,
) -> Result<CompilationOutput, CompileError> {
    compile_with_default_pdf_timestamp(world, output, CompilationLimits::reference_v1(), || {
        world.today(None).map(Timestamp::new_utc)
    })
}

/// Compiles a validated Pack and retains operational fulfillment evidence.
#[allow(clippy::result_large_err)]
pub fn compile(
    request: PackCompilationRequest,
    limits: CompilationLimits,
) -> Result<CompilationReport, CompilationRequestRejection> {
    let (world, kernel) = match prepare_pack_compilation(request, limits) {
        PackCompilationPreparation::Execute { world, kernel } => (world, kernel),
        PackCompilationPreparation::Report(report) => return Ok(report),
        PackCompilationPreparation::Rejected(rejection) => return Err(rejection),
    };
    Ok(match compile_pack_kernel(&world, kernel) {
        PackCompilationKernelOutcome::Execution(execution) => CompilationReport {
            outcome: CompilationReportOutcome::Result(Box::new(execution.result)),
            fulfillments: execution.fulfillments,
        },
        PackCompilationKernelOutcome::Operation(report) => report,
    })
}

pub(crate) struct PreparedPackCompilationKernel {
    request_inventory: CompilationRequestInventory,
    compilation_identity: CompilationIdentity,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
    page_selection_implies_untagged_pdf: bool,
    fulfillments: CompilationFulfillmentReport,
    limits: CompilationLimits,
}

pub(crate) struct PackCompilationExecution {
    pub(crate) result: CompilationResult,
    #[cfg(feature = "diagnostics")]
    pub(crate) presentation: PackCompilationPresentation,
    pub(crate) fulfillments: CompilationFulfillmentReport,
}

pub(crate) enum PackCompilationKernelOutcome {
    Execution(PackCompilationExecution),
    Operation(CompilationReport),
}

#[cfg(feature = "diagnostics")]
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
    limits: CompilationLimits,
) -> PackCompilationPreparation {
    let PackCompilationRequest {
        pack,
        output_specification,
        inputs,
        overrides,
        features,
        document_time,
        fulfillments,
    } = request;
    let mut output = output_specification.resolve();
    let inputs = inputs.resolve();
    let overrides = overrides.resolve();
    let document_time = document_time.resolve();
    let mut request_issues = vec![];
    let page_selection_implies_untagged_pdf;
    let output_origins = match &mut output.value {
        CompilationOutputSpecification::Png(specification) => {
            canonicalize_page_selection(&mut specification.page_selection, &mut request_issues);
            let mut pixels_per_inch = output.origin;
            if specification
                .pixels_per_inch
                .is_some_and(|ppi| !ppi.is_finite() || ppi <= 0.0)
            {
                request_issues.push(CompilationRequestIssue::InvalidPpi);
            }
            if specification.pixels_per_inch.is_none() {
                specification.pixels_per_inch = Some(default_png_ppi());
                pixels_per_inch = RequestValueOrigin::CoreDefaulted;
            }
            page_selection_implies_untagged_pdf = false;
            CompilationOutputOrigins::Png { pixels_per_inch }
        }
        CompilationOutputSpecification::Pdf(specification) => {
            canonicalize_page_selection(&mut specification.page_selection, &mut request_issues);
            specification.standards.sort_by_key(pdf_standard_identity);
            let mut tags = output.origin;
            let mut creation_time = output.origin;
            if let Err(error) = validate_pdf_standards(&specification.standards) {
                request_issues.push(CompilationRequestIssue::InvalidPdfStandards(error));
            }
            page_selection_implies_untagged_pdf = !specification.page_selection.ranges().is_empty()
                && specification.tags.is_auto()
                && PdfOptions::default().tagged;
            if specification.tags.is_auto() {
                specification.tags = Smart::Custom(
                    PdfOptions::default().tagged
                        && specification.page_selection.ranges().is_empty(),
                );
                tags = if specification.page_selection.ranges().is_empty() {
                    RequestValueOrigin::CoreDefaulted
                } else {
                    RequestValueOrigin::CoreDerived
                };
            }
            if matches!(
                specification.creation_timestamp,
                CreationTimestamp::Automatic
            ) {
                specification.creation_timestamp = CreationTimestamp::Omit;
                creation_time = RequestValueOrigin::CoreDefaulted;
            }
            CompilationOutputOrigins::Pdf {
                tags,
                creation_time,
            }
        }
        CompilationOutputSpecification::Svg(specification) => {
            canonicalize_page_selection(&mut specification.page_selection, &mut request_issues);
            page_selection_implies_untagged_pdf = false;
            CompilationOutputOrigins::Svg
        }
        CompilationOutputSpecification::Html(_) => {
            page_selection_implies_untagged_pdf = false;
            CompilationOutputOrigins::Html
        }
    };
    if overrides.value.pack_identity != pack.identity() {
        request_issues.push(CompilationRequestIssue::OverrideSetPackMismatch);
    }
    if features
        .iter()
        .any(|feature| feature.value == Feature::Bundle)
    {
        request_issues.push(CompilationRequestIssue::UnsupportedBundleFeature);
    }
    if let DocumentTime::UnixTimestamp(timestamp) = document_time.value
        && typst_kit::datetime::Time::fixed_timestamp(timestamp).is_err()
    {
        request_issues.push(CompilationRequestIssue::InvalidDocumentTimestamp);
    }
    let selected_features = [Feature::Html, Feature::Bundle, Feature::A11yExtras]
        .into_iter()
        .filter_map(|value| {
            features
                .iter()
                .filter(|feature| feature.value == value)
                .max_by_key(|feature| request_origin_rank(feature.origin))
                .copied()
        })
        .collect::<Vec<_>>();
    let derives_html = matches!(&output.value, CompilationOutputSpecification::Html(_));
    let effective_features = [Feature::Html, Feature::Bundle, Feature::A11yExtras]
        .into_iter()
        .filter_map(|value| {
            selected_features
                .iter()
                .find(|feature| feature.value == value)
                .copied()
                .or_else(|| {
                    (value == Feature::Html && derives_html).then_some(EffectiveEngineFeature {
                        value,
                        origin: RequestValueOrigin::CoreDerived,
                    })
                })
        })
        .collect::<Vec<_>>();
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
        output_specification: output,
        output_origins,
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
    };
    if !request_issues.is_empty() {
        return PackCompilationPreparation::Rejected(CompilationRequestRejection::new(
            request_issues,
            request_inventory,
        ));
    }

    // Implementation identities exist only for an accepted semantic request.
    let engine_identity = EmbeddedTypst::engine_identity();
    let exporter_identity =
        EmbeddedTypst::exporter_identity(request_inventory.output_specification.value().format());
    let compilation_identity = compilation_identity(
        &pack,
        &request_inventory,
        engine_identity,
        exporter_identity,
    );
    let CompilationFulfillmentSet {
        packages: package_fulfillments,
        fonts: font_fulfillments,
    } = fulfillments;
    let package_requirements = pack
        .package_requirements()
        .iter()
        .map(|requirement| (requirement.spec().to_string(), requirement))
        .collect::<BTreeMap<_, _>>();
    let package_report_keys = package_requirements
        .keys()
        .chain(package_fulfillments.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let font_requirements = pack
        .font_requirements()
        .iter()
        .map(|requirement| (requirement.container_identity(), requirement))
        .collect::<BTreeMap<_, _>>();
    let font_report_keys = font_requirements
        .keys()
        .chain(font_fulfillments.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let fulfillments = CompilationFulfillmentReport {
        packages: package_report_keys
            .into_iter()
            .map(|key| {
                let requirement = package_requirements.get(&key).copied();
                let supplied = package_fulfillments.get(&key);
                PackageFulfillmentReport {
                    spec: requirement
                        .map(|value| value.spec().clone())
                        .unwrap_or_else(|| {
                            supplied
                                .expect("report key came from a requirement or fulfillment")
                                .spec
                                .clone()
                        }),
                    required_tree_identity: requirement.map(|value| value.tree_identity()),
                    supplied_tree_identity: supplied.map(|value| value.tree.identity()),
                    declared: requirement.is_some(),
                    embedded: requirement.is_some_and(|value| value.is_embedded()),
                    provenance: supplied.and_then(|value| value.provenance.clone()),
                    cache_hit: supplied.is_some_and(|value| value.cache_hit),
                }
            })
            .collect(),
        fonts: font_report_keys
            .into_iter()
            .map(|identity| {
                let requirement = font_requirements.get(&identity).copied();
                let supplied = font_fulfillments.get(&identity);
                FontFulfillmentReport {
                    container_identity: identity,
                    supplied_container_identity: supplied.map(|value| value.container.identity()),
                    declared: requirement.is_some(),
                    embedded: requirement.is_some_and(|value| value.is_embedded()),
                    provenance: supplied.and_then(|value| value.provenance.clone()),
                    licensing: supplied.and_then(|value| value.licensing.clone()),
                }
            })
            .collect(),
    };
    let fulfillment_issues =
        verify_compilation_fulfillment_set(&pack, &package_fulfillments, &font_fulfillments);
    if !fulfillment_issues.is_empty() {
        return PackCompilationPreparation::Report(CompilationReport {
            outcome: CompilationReportOutcome::Operation {
                outcome: CompilationOperationOutcome::InvalidFulfillmentSet(
                    InvalidCompilationFulfillmentSet {
                        issues: fulfillment_issues,
                    },
                ),
                request_inventory: Box::new(request_inventory),
                compilation_identity,
            },
            fulfillments,
        });
    }
    let package_trees = package_fulfillments
        .into_iter()
        .map(|(spec, fulfillment)| (spec, fulfillment.tree))
        .collect();
    let font_containers = font_fulfillments
        .into_iter()
        .map(|(identity, fulfillment)| (identity, fulfillment.container))
        .collect();
    let dependencies =
        pack.materialize_compilation_dependency_snapshot(package_trees, font_containers);

    let world = PackWorld::new(
        pack,
        dependencies,
        overrides.value.replacements,
        raw_inputs,
        request_inventory
            .features
            .iter()
            .map(|feature| feature.value)
            .collect(),
        request_inventory.document_time.value,
    )
    .expect("preflighted Pack World inputs must remain valid");

    PackCompilationPreparation::Execute {
        world,
        kernel: PreparedPackCompilationKernel {
            request_inventory,
            compilation_identity,
            engine_identity,
            exporter_identity,
            page_selection_implies_untagged_pdf,
            fulfillments,
            limits,
        },
    }
}

pub(crate) fn compile_pack_kernel(
    world: &PackWorld,
    kernel: PreparedPackCompilationKernel,
) -> PackCompilationKernelOutcome {
    let traced = WorldTrace::new(world);
    let compiled = compile_with_default_pdf_timestamp(
        &traced,
        kernel.request_inventory.output_specification.value(),
        kernel.limits,
        || None,
    );
    let access_trace = traced.snapshot();
    match compiled {
        Ok(output) => {
            #[cfg(feature = "diagnostics")]
            let warnings = output.warnings.clone();
            #[cfg(feature = "diagnostics")]
            let mut presentation_pack_warnings = output.pack_warnings.clone();
            #[cfg(feature = "diagnostics")]
            if kernel.page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            let diagnostics = project_diagnostics(
                &traced,
                output.warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(kernel.engine_identity),
            );
            let pack_warnings = project_pack_warnings(
                output.pack_warnings,
                kernel.page_selection_implies_untagged_pdf,
            );
            PackCompilationKernelOutcome::Execution(PackCompilationExecution {
                result: assemble_compilation_result(
                    &kernel,
                    CompilationStatus::Succeeded,
                    output.artifacts,
                    diagnostics,
                    pack_warnings,
                    output.source_page_count,
                    access_trace,
                ),
                #[cfg(feature = "diagnostics")]
                presentation: PackCompilationPresentation::Succeeded {
                    warnings,
                    pack_warnings: presentation_pack_warnings,
                },
                fulfillments: kernel.fulfillments,
            })
        }
        Err(CompileError::Diagnostics {
            errors,
            warnings,
            pack_warnings,
            phase,
            source_page_count,
        }) => {
            #[cfg(feature = "diagnostics")]
            let mut presentation_pack_warnings = pack_warnings.clone();
            #[cfg(feature = "diagnostics")]
            if kernel.page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            #[cfg(feature = "diagnostics")]
            let presentation = PackCompilationPresentation::Diagnostics {
                errors: errors.clone(),
                warnings: warnings.clone(),
                pack_warnings: presentation_pack_warnings,
            };
            let mut diagnostics = project_diagnostics(
                &traced,
                warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(kernel.engine_identity),
            );
            let producer = match phase {
                DiagnosticPhase::Compilation => DiagnosticProducer::Engine(kernel.engine_identity),
                DiagnosticPhase::Export => DiagnosticProducer::Exporter(kernel.exporter_identity),
            };
            diagnostics.extend(project_diagnostics(&traced, errors, phase, producer));
            PackCompilationKernelOutcome::Execution(PackCompilationExecution {
                result: assemble_compilation_result(
                    &kernel,
                    CompilationStatus::Rejected,
                    vec![],
                    diagnostics,
                    project_pack_warnings(
                        pack_warnings,
                        kernel.page_selection_implies_untagged_pdf,
                    ),
                    source_page_count,
                    access_trace,
                ),
                #[cfg(feature = "diagnostics")]
                presentation,
                fulfillments: kernel.fulfillments,
            })
        }
        Err(CompileError::PngExport {
            message,
            warnings,
            pack_warnings,
            source_page_count,
            source_page_number,
        }) => {
            #[cfg(feature = "diagnostics")]
            let mut presentation_pack_warnings = pack_warnings.clone();
            #[cfg(feature = "diagnostics")]
            if kernel.page_selection_implies_untagged_pdf {
                presentation_pack_warnings.push(page_selection_pdf_tags_warning());
            }
            #[cfg(feature = "diagnostics")]
            let presentation = PackCompilationPresentation::PngExport {
                error: format!("PNG export failed for source page {source_page_number}: {message}"),
                warnings: warnings.clone(),
                pack_warnings: presentation_pack_warnings,
            };
            let mut diagnostics = project_diagnostics(
                &traced,
                warnings,
                DiagnosticPhase::Compilation,
                DiagnosticProducer::Engine(kernel.engine_identity),
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
                producer: DiagnosticProducer::Exporter(kernel.exporter_identity),
                source_page_number: Some(source_page_number),
            });
            PackCompilationKernelOutcome::Execution(PackCompilationExecution {
                result: assemble_compilation_result(
                    &kernel,
                    CompilationStatus::Rejected,
                    vec![],
                    diagnostics,
                    project_pack_warnings(
                        pack_warnings,
                        kernel.page_selection_implies_untagged_pdf,
                    ),
                    Some(source_page_count),
                    access_trace,
                ),
                #[cfg(feature = "diagnostics")]
                presentation,
                fulfillments: kernel.fulfillments,
            })
        }
        Err(CompileError::InvalidPdfStandards(error)) => {
            unreachable!("PDF standards are validated during request preparation: {error}");
        }
        Err(CompileError::Limit(error)) => {
            PackCompilationKernelOutcome::Operation(CompilationReport {
                outcome: CompilationReportOutcome::Operation {
                    outcome: CompilationOperationOutcome::ResourceLimit(error),
                    request_inventory: Box::new(kernel.request_inventory),
                    compilation_identity: kernel.compilation_identity,
                },
                fulfillments: kernel.fulfillments,
            })
        }
    }
}

fn verify_compilation_fulfillment_set(
    pack: &Pack,
    package_fulfillments: &BTreeMap<String, PackageTreeFulfillment>,
    font_fulfillments: &BTreeMap<FontContainerIdentity, FontContainerFulfillment>,
) -> Vec<CompilationFulfillmentIssue> {
    let package_requirements = pack
        .package_requirements()
        .iter()
        .map(|requirement| (requirement.spec().to_string(), requirement))
        .collect::<BTreeMap<_, _>>();
    let package_keys = package_requirements
        .keys()
        .chain(package_fulfillments.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    for key in package_keys {
        match (
            package_requirements.get(&key),
            package_fulfillments.get(&key),
        ) {
            (Some(requirement), None) if !requirement.is_embedded() => {
                issues.push(CompilationFulfillmentIssue::MissingExternalPackage {
                    spec: requirement.spec().clone(),
                });
            }
            (None, Some(fulfillment)) => {
                issues.push(CompilationFulfillmentIssue::UndeclaredPackage {
                    spec: fulfillment.spec.clone(),
                    actual: fulfillment.tree.identity(),
                });
            }
            (Some(requirement), Some(fulfillment)) => {
                if requirement.is_embedded() {
                    issues.push(CompilationFulfillmentIssue::UnexpectedEmbeddedPackage {
                        spec: requirement.spec().clone(),
                    });
                }
                if fulfillment.tree.identity() != requirement.tree_identity()
                    || fulfillment.tree.file_count() != requirement.file_count()
                    || fulfillment.tree.byte_length() != requirement.byte_length()
                {
                    issues.push(CompilationFulfillmentIssue::MismatchedPackageTree {
                        spec: requirement.spec().clone(),
                        expected: requirement.tree_identity(),
                        actual: fulfillment.tree.identity(),
                        expected_file_count: requirement.file_count(),
                        actual_file_count: fulfillment.tree.file_count(),
                        expected_byte_length: requirement.byte_length(),
                        actual_byte_length: fulfillment.tree.byte_length(),
                    });
                }
            }
            _ => {}
        }
    }

    let font_requirements = pack
        .font_requirements()
        .iter()
        .map(|requirement| (requirement.container_identity(), requirement))
        .collect::<BTreeMap<_, _>>();
    let font_keys = font_requirements
        .keys()
        .chain(font_fulfillments.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for identity in font_keys {
        match (
            font_requirements.get(&identity),
            font_fulfillments.get(&identity),
        ) {
            (Some(requirement), None) if !requirement.is_embedded() => {
                issues.push(CompilationFulfillmentIssue::MissingExternalFont { identity });
            }
            (None, Some(fulfillment)) => {
                issues.push(CompilationFulfillmentIssue::UndeclaredFont {
                    identity,
                    actual: fulfillment.container.identity(),
                });
            }
            (Some(requirement), Some(fulfillment)) => {
                if requirement.is_embedded() {
                    issues.push(CompilationFulfillmentIssue::UnexpectedEmbeddedFont { identity });
                }
                let actual = fulfillment.container.identity();
                let actual_length = fulfillment.container.data().len() as u64;
                if actual != identity || actual_length != requirement.container_length() {
                    issues.push(CompilationFulfillmentIssue::MismatchedFontContainer {
                        expected: identity,
                        actual,
                        expected_length: requirement.container_length(),
                        actual_length,
                    });
                }
                for index in requirement
                    .face_indices()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                {
                    if !fulfillment
                        .container
                        .faces()
                        .iter()
                        .any(|face| face.identity().index() == index)
                    {
                        issues
                            .push(CompilationFulfillmentIssue::MissingFontFace { identity, index });
                    }
                }
            }
            _ => {}
        }
    }
    issues
}

fn assemble_compilation_result(
    kernel: &PreparedPackCompilationKernel,
    status: CompilationStatus,
    artifacts: Vec<CompilationArtifact>,
    diagnostics: Vec<CompilationDiagnostic>,
    pack_warnings: Vec<PackCompilationWarning>,
    source_page_count: Option<usize>,
    access_trace: CompilationAccessTrace,
) -> CompilationResult {
    finalize_result(CompilationResult {
        status,
        artifacts,
        diagnostics,
        pack_warnings,
        document: document_summary(
            kernel.request_inventory.output_specification.value(),
            source_page_count,
        ),
        access_trace,
        result_identity: CompilationResultIdentity(0),
        request_inventory: kernel.request_inventory.clone(),
        compilation_identity: kernel.compilation_identity,
        engine_identity: kernel.engine_identity,
        exporter_identity: kernel.exporter_identity,
    })
}

fn document_summary(
    output: &CompilationOutputSpecification,
    source_page_count: Option<usize>,
) -> CompilationDocumentSummary {
    CompilationDocumentSummary {
        target: output.target(),
        source_page_count,
    }
}

fn finalize_result(mut result: CompilationResult) -> CompilationResult {
    let artifacts = result
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.format,
                artifact.source_page_number,
                artifact.bytes.len(),
                typst::utils::hash128(artifact.bytes.as_slice()),
            )
        })
        .collect::<Vec<_>>();
    result.result_identity = CompilationResultIdentity(typst::utils::hash128(&(
        "typst-pack-compilation-result-v1",
        result.compilation_identity,
        result.status,
        result.document,
        &result.diagnostics,
        &result.pack_warnings,
        &result.access_trace,
        artifacts,
    )));
    result
}

fn compilation_identity(
    pack: &Pack,
    inventory: &CompilationRequestInventory,
    engine_identity: EngineIdentity,
    exporter_identity: ExporterIdentity,
) -> CompilationIdentity {
    let output_digest = match inventory.output_specification.value() {
        CompilationOutputSpecification::Pdf(specification) => {
            let page_selection = canonical_page_selection(&specification.page_selection);
            let mut standards = specification
                .standards
                .iter()
                .map(pdf_standard_identity)
                .collect::<Vec<_>>();
            standards.sort_unstable();
            typst::utils::hash128(&(
                "pdf",
                &page_selection,
                &specification.identifier,
                &specification.creator,
                specification.tags,
                specification.creation_timestamp,
                standards,
                specification.pretty,
            ))
        }
        CompilationOutputSpecification::Png(specification) => {
            let page_selection = canonical_page_selection(&specification.page_selection);
            typst::utils::hash128(&(
                "png",
                &page_selection,
                specification.pixels_per_inch.map(f64::to_bits),
                specification.render_bleed,
            ))
        }
        CompilationOutputSpecification::Svg(specification) => {
            let page_selection = canonical_page_selection(&specification.page_selection);
            typst::utils::hash128(&(
                "svg",
                &page_selection,
                specification.render_bleed,
                specification.pretty,
            ))
        }
        CompilationOutputSpecification::Html(specification) => {
            typst::utils::hash128(&("html", specification.pretty))
        }
    };
    let feature_values = inventory
        .features
        .iter()
        .map(|feature| feature.value)
        .collect::<Vec<_>>();
    let (document_time, document_timestamp) = inventory.document_time.value.identity_projection();
    CompilationIdentity(typst::utils::hash128(&(
        "typst-pack-compilation-v1",
        pack.identity(),
        inventory.output_specification.value().format(),
        output_digest,
        inventory.inputs.value.commitment,
        inventory
            .overrides
            .value
            .iter()
            .map(|entry| (&entry.path, entry.byte_len, entry.commitment))
            .collect::<Vec<_>>(),
        feature_values,
        document_time,
        document_timestamp,
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

fn canonicalize_page_selection(
    selection: &mut PageSelection,
    issues: &mut Vec<CompilationRequestIssue>,
) {
    let invalid = selection
        .ranges
        .iter()
        .filter_map(|range| {
            let (Some(start), Some(end)) = (*range.start(), *range.end()) else {
                return None;
            };
            (start > end).then_some((start, end))
        })
        .collect::<BTreeSet<_>>();
    issues.extend(
        invalid
            .iter()
            .map(|(start, end)| CompilationRequestIssue::InvalidPageRange {
                start: *start,
                end: *end,
            }),
    );
    if selection.ranges.is_empty() {
        return;
    }
    let (_, ranges) = canonical_page_selection(selection);
    let mut ranges = ranges
        .into_iter()
        .map(|(start, end)| {
            let start = Some(NonZeroUsize::new(start).expect("canonical page starts at one"));
            let end = (end != usize::MAX)
                .then(|| NonZeroUsize::new(end).expect("canonical page ends at one or later"));
            start..=end
        })
        .chain(
            invalid
                .into_iter()
                .map(|(start, end)| Some(start)..=Some(end)),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| {
        (
            range.start().map_or(1, NonZeroUsize::get),
            range.end().map_or(usize::MAX, NonZeroUsize::get),
        )
    });
    selection.ranges = ranges;
}

fn request_origin_rank(origin: RequestValueOrigin) -> u8 {
    match origin {
        RequestValueOrigin::CoreDefaulted => 0,
        RequestValueOrigin::CoreDerived => 1,
        RequestValueOrigin::AdapterResolved => 2,
        RequestValueOrigin::CallerSupplied => 3,
    }
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
    specification: &CompilationOutputSpecification,
    limits: CompilationLimits,
    default_pdf_timestamp: impl FnOnce() -> Option<Timestamp>,
) -> Result<CompilationOutput, CompileError> {
    let _compilation_timing = typst_timing::TimingScope::new("typst-pack compilation");
    if let CompilationOutputSpecification::Html(specification) = specification {
        let pack_warnings = EcoVec::new();
        let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
        let document = output.map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
            pack_warnings: pack_warnings.clone(),
            phase: DiagnosticPhase::Compilation,
            source_page_count: None,
        })?;
        check_artifact_count(limits, 1)?;
        let _export_timing = typst_timing::TimingScope::new("export");
        let bytes = EmbeddedTypst::export_html(
            &document,
            &typst_html::HtmlOptions {
                pretty: specification.pretty,
            },
        )
        .map_err(|errors| CompileError::Diagnostics {
            errors,
            warnings: warnings.clone(),
            pack_warnings: pack_warnings.clone(),
            phase: DiagnosticPhase::Export,
            source_page_count: None,
        })?;
        let artifacts = vec![CompilationArtifact {
            format: OutputFormat::Html,
            bytes: SharedBytes::new(bytes),
            source_page_number: None,
        }];
        check_artifact_bytes(limits, &artifacts)?;
        return Ok(CompilationOutput {
            artifacts,
            warnings,
            pack_warnings,
            source_page_count: None,
        });
    }

    let Warned {
        output,
        warnings: compile_warnings,
    } = EmbeddedTypst::compile_paged(world);
    let warnings = compile_warnings;
    let mut pack_warnings = EcoVec::new();
    if let CompilationOutputSpecification::Pdf(specification) = specification
        && !specification.page_selection.ranges().is_empty()
        && specification.tags.is_auto()
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
    check_compilation_limit(
        CompilationResource::SourcePages,
        limits.source_pages,
        u64::try_from(source_page_count).map_err(|_| {
            CompilationLimitError::AccountingOverflow {
                resource: CompilationResource::SourcePages,
            }
        })?,
    )?;
    let artifacts = {
        let _export_timing = typst_timing::TimingScope::new("export");
        match specification {
            CompilationOutputSpecification::Pdf(specification) => {
                check_artifact_count(limits, 1)?;
                let standards = validate_pdf_standards(&specification.standards)
                    .map_err(CompileError::InvalidPdfStandards)?;
                let timestamp = match specification.creation_timestamp {
                    CreationTimestamp::Automatic => default_pdf_timestamp(),
                    CreationTimestamp::Explicit(timestamp) => Some(timestamp),
                    CreationTimestamp::Omit => None,
                };
                let pdf_options = PdfOptions {
                    ident: specification.identifier.clone(),
                    creator: specification.creator.clone(),
                    timestamp,
                    page_ranges: specification.page_selection.typst_page_ranges(),
                    standards,
                    tagged: match specification.tags {
                        Smart::Auto => {
                            PdfOptions::default().tagged
                                && specification.page_selection.ranges().is_empty()
                        }
                        Smart::Custom(tagged) => tagged,
                    },
                    pretty: specification.pretty,
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
                    format: OutputFormat::Pdf,
                    bytes: SharedBytes::new(pdf),
                    source_page_number: None,
                }]
            }
            CompilationOutputSpecification::Png(specification) => {
                let pixels_per_inch = specification
                    .pixels_per_inch
                    .unwrap_or_else(default_png_ppi);
                let render_options = typst_render::RenderOptions {
                    pixel_per_pt: (pixels_per_inch / 72.0).into(),
                    render_bleed: specification.render_bleed,
                };
                let pages =
                    selected_pages(&document, &specification.page_selection).collect::<Vec<_>>();
                check_artifact_count(limits, pages.len())?;
                check_png_pixels(limits, &pages, &render_options)?;
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
                        format: OutputFormat::Png,
                        bytes: SharedBytes::new(bytes),
                        source_page_number: Some(source_page_number),
                    })
                };
                export_artifacts_bounded(pages, limits, export)?
            }
            CompilationOutputSpecification::Svg(specification) => {
                let svg_options = typst_svg::SvgOptions {
                    render_bleed: specification.render_bleed,
                    pretty: specification.pretty,
                };
                let pages =
                    selected_pages(&document, &specification.page_selection).collect::<Vec<_>>();
                check_artifact_count(limits, pages.len())?;
                let export = |(source_page_number, page)| {
                    Ok(CompilationArtifact {
                        format: OutputFormat::Svg,
                        bytes: SharedBytes::new(EmbeddedTypst::export_svg(page, &svg_options)),
                        source_page_number: Some(source_page_number),
                    })
                };
                export_artifacts_bounded(pages, limits, export)?
            }
            CompilationOutputSpecification::Html(_) => unreachable!("handled above"),
        }
    };
    check_artifact_bytes(limits, &artifacts)?;
    Ok(CompilationOutput {
        artifacts,
        warnings,
        pack_warnings,
        source_page_count: Some(source_page_count),
    })
}

fn check_compilation_limit(
    resource: CompilationResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), CompilationLimitError> {
    if observed > ceiling {
        Err(CompilationLimitError::Exceeded {
            resource,
            ceiling,
            observed_at_least: observed,
        })
    } else {
        Ok(())
    }
}

fn check_artifact_count(
    limits: CompilationLimits,
    count: usize,
) -> Result<(), CompilationLimitError> {
    let observed = u64::try_from(count).map_err(|_| CompilationLimitError::AccountingOverflow {
        resource: CompilationResource::Artifacts,
    })?;
    check_compilation_limit(CompilationResource::Artifacts, limits.artifacts, observed)
}

fn check_png_pixels(
    limits: CompilationLimits,
    pages: &[(NonZeroUsize, &typst_layout::Page)],
    options: &typst_render::RenderOptions,
) -> Result<(), CompilationLimitError> {
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
        let pixels = u64::from(width).checked_mul(u64::from(height)).ok_or(
            CompilationLimitError::AccountingOverflow {
                resource: CompilationResource::PixelsPerArtifact,
            },
        )?;
        check_compilation_limit(
            CompilationResource::PixelsPerArtifact,
            limits.pixels_per_artifact,
            pixels,
        )?;
        total = total
            .checked_add(pixels)
            .ok_or(CompilationLimitError::AccountingOverflow {
                resource: CompilationResource::TotalPixels,
            })?;
    }
    check_compilation_limit(CompilationResource::TotalPixels, limits.total_pixels, total)
}

fn check_artifact_bytes(
    limits: CompilationLimits,
    artifacts: &[CompilationArtifact],
) -> Result<(), CompilationLimitError> {
    let mut retained = 0u64;
    for artifact in artifacts {
        retain_artifact_bytes(limits, &mut retained, artifact)?;
    }
    Ok(())
}

fn retain_artifact_bytes(
    limits: CompilationLimits,
    retained: &mut u64,
    artifact: &CompilationArtifact,
) -> Result<(), CompilationLimitError> {
    let bytes = u64::try_from(artifact.bytes.len()).map_err(|_| {
        CompilationLimitError::AccountingOverflow {
            resource: CompilationResource::ArtifactBytes,
        }
    })?;
    check_compilation_limit(
        CompilationResource::ArtifactBytes,
        limits.artifact_bytes,
        bytes,
    )?;
    *retained = retained
        .checked_add(bytes)
        .ok_or(CompilationLimitError::AccountingOverflow {
            resource: CompilationResource::RetainedArtifactBytes,
        })?;
    check_compilation_limit(
        CompilationResource::RetainedArtifactBytes,
        limits.retained_artifact_bytes,
        *retained,
    )
}

pub(crate) fn validate_pdf_standards(
    standards: &[PdfStandard],
) -> Result<PdfStandards, PdfStandardsValidationError> {
    PdfStandards::new(standards).map_err(|error| PdfStandardsValidationError {
        message: error.message().to_string(),
        hints: error.hints().iter().map(ToString::to_string).collect(),
    })
}

#[cfg(feature = "diagnostics")]
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
    page_selection: &'a PageSelection,
) -> impl Iterator<Item = (NonZeroUsize, &'a typst_layout::Page)> {
    let ranges = page_selection.typst_page_ranges();
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

#[cfg(feature = "parallel")]
fn export_artifacts_bounded<T>(
    items: Vec<T>,
    limits: CompilationLimits,
    export: impl Fn(T) -> Result<CompilationArtifact, CompileError> + Sync + Send,
) -> Result<Vec<CompilationArtifact>, CompileError>
where
    T: Send,
{
    if items.is_empty() {
        return Ok(vec![]);
    }
    let workers = usize::try_from(limits.export_workers)
        .unwrap_or(usize::MAX)
        .min(items.len());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .ok();
    let mut items = items.into_iter();
    let mut artifacts = Vec::new();
    let mut retained = 0;
    loop {
        let batch = items.by_ref().take(workers).collect::<Vec<_>>();
        if batch.is_empty() {
            break;
        }
        let batch: Vec<Result<CompilationArtifact, CompileError>> = match &pool {
            Some(pool) => pool.install(|| batch.into_par_iter().map(&export).collect()),
            None => batch.into_iter().map(&export).collect(),
        };
        for artifact in batch {
            let artifact = artifact?;
            retain_artifact_bytes(limits, &mut retained, &artifact)?;
            artifacts.push(artifact);
        }
    }
    Ok(artifacts)
}

#[cfg(not(feature = "parallel"))]
fn export_artifacts_bounded<T>(
    items: Vec<T>,
    limits: CompilationLimits,
    export: impl Fn(T) -> Result<CompilationArtifact, CompileError>,
) -> Result<Vec<CompilationArtifact>, CompileError> {
    let mut artifacts = Vec::new();
    let mut retained = 0;
    for item in items {
        let artifact = export(item)?;
        retain_artifact_bytes(limits, &mut retained, &artifact)?;
        artifacts.push(artifact);
    }
    Ok(artifacts)
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

#[cfg(test)]
mod artifact_publication_tests {
    use super::*;

    #[test]
    fn artifact_publication_planning_aggregates_collisions_in_canonical_order() {
        let pack = Pack::builder("main.typ")
            .file(
                "main.typ",
                b"collision one\n#pagebreak()\ncollision two".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap();
        let report = compile(
            PackCompilationRequest::new(
                pack,
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let mut result = report.result().unwrap().clone();
        result.artifacts.push(result.artifacts[1].clone());
        result.artifacts.push(result.artifacts[0].clone());

        assert_eq!(
            crate::plan_compilation_artifact_publication(&result)
                .unwrap_err()
                .issues(),
            [
                crate::ArtifactPublicationPlanIssue::NamingCollision {
                    relative_path: "page-1.svg".to_owned(),
                },
                crate::ArtifactPublicationPlanIssue::NamingCollision {
                    relative_path: "page-2.svg".to_owned(),
                },
            ]
        );
    }
}

#[cfg(test)]
mod result_identity_tests {
    use super::*;

    #[test]
    fn compilation_trace_retains_missing_font_requests() {
        let trace = CompilationAccessTrace::from_observations(BTreeSet::from([
            CompilationAccessObservation::new(
                CompilationAccessKind::Font,
                "font-index:7".to_owned(),
                Some(7),
                CompilationAccessOutcome::Missing,
            ),
        ]));

        let observation = trace.observations().next().unwrap();
        assert_eq!(observation.kind(), CompilationAccessKind::Font);
        assert_eq!(observation.logical_path(), "font-index:7");
        assert_eq!(observation.font_index(), Some(7));
        assert_eq!(observation.outcome(), &CompilationAccessOutcome::Missing);
    }

    #[test]
    fn result_identity_binds_each_post_execution_projection() {
        let pack = Pack::builder("main.typ")
            .file(
                "main.typ",
                b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap();
        let report = compile(
            PackCompilationRequest::new(
                pack,
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let base = report.result().unwrap().clone();
        let identity = base.result_identity;

        let mut compilation = base.clone();
        compilation.compilation_identity = CompilationIdentity(base.compilation_identity.0 ^ 1);
        assert_ne!(finalize_result(compilation).result_identity, identity);

        let mut status = base.clone();
        status.status = CompilationStatus::Rejected;
        assert_ne!(finalize_result(status).result_identity, identity);

        let mut target = base.clone();
        target.document.target = TypstTarget::Html;
        assert_ne!(finalize_result(target).result_identity, identity);

        let mut document = base.clone();
        document.document.source_page_count = Some(2);
        assert_ne!(finalize_result(document).result_identity, identity);

        let diagnostic = CompilationDiagnostic {
            severity: DiagnosticSeverity::Warning,
            message: "identity warning".to_owned(),
            span: LogicalSpan {
                logical_path: Some("project:main.typ".to_owned()),
                byte_range: Some(1..2),
            },
            hints: vec![DiagnosticHint {
                message: "identity hint".to_owned(),
                span: LogicalSpan {
                    logical_path: Some("project:hint.typ".to_owned()),
                    byte_range: Some(2..3),
                },
            }],
            trace: vec![DiagnosticTracepoint {
                kind: TracepointKind::Call,
                value: Some("identity trace".to_owned()),
                span: LogicalSpan {
                    logical_path: Some("project:trace.typ".to_owned()),
                    byte_range: Some(3..4),
                },
            }],
            phase: DiagnosticPhase::Compilation,
            producer: DiagnosticProducer::Engine(base.engine_identity),
            source_page_number: NonZeroUsize::new(1),
        };
        let mut diagnostics = base.clone();
        diagnostics.diagnostics.push(diagnostic.clone());
        let diagnostic_identity = finalize_result(diagnostics).result_identity;
        assert_ne!(diagnostic_identity, identity);
        let diagnostic_mutations = [
            CompilationDiagnostic {
                severity: DiagnosticSeverity::Error,
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                message: "changed warning".to_owned(),
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                span: LogicalSpan {
                    logical_path: Some("project:changed.typ".to_owned()),
                    ..diagnostic.span.clone()
                },
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                span: LogicalSpan {
                    byte_range: Some(4..5),
                    ..diagnostic.span.clone()
                },
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                hints: vec![DiagnosticHint {
                    message: "changed hint".to_owned(),
                    ..diagnostic.hints[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                hints: vec![DiagnosticHint {
                    span: LogicalSpan {
                        logical_path: Some("project:changed-hint.typ".to_owned()),
                        ..diagnostic.hints[0].span.clone()
                    },
                    ..diagnostic.hints[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                hints: vec![DiagnosticHint {
                    span: LogicalSpan {
                        byte_range: Some(5..6),
                        ..diagnostic.hints[0].span.clone()
                    },
                    ..diagnostic.hints[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                trace: vec![DiagnosticTracepoint {
                    kind: TracepointKind::Include,
                    ..diagnostic.trace[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                trace: vec![DiagnosticTracepoint {
                    value: Some("changed trace".to_owned()),
                    ..diagnostic.trace[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                trace: vec![DiagnosticTracepoint {
                    span: LogicalSpan {
                        logical_path: Some("project:changed-trace.typ".to_owned()),
                        ..diagnostic.trace[0].span.clone()
                    },
                    ..diagnostic.trace[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                trace: vec![DiagnosticTracepoint {
                    span: LogicalSpan {
                        byte_range: Some(6..7),
                        ..diagnostic.trace[0].span.clone()
                    },
                    ..diagnostic.trace[0].clone()
                }],
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                phase: DiagnosticPhase::Export,
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                producer: DiagnosticProducer::Exporter(base.exporter_identity),
                ..diagnostic.clone()
            },
            CompilationDiagnostic {
                source_page_number: NonZeroUsize::new(2),
                ..diagnostic
            },
        ];
        for diagnostic in &diagnostic_mutations {
            let mut result = base.clone();
            result.diagnostics.push(diagnostic.clone());
            assert_ne!(finalize_result(result).result_identity, diagnostic_identity);
        }

        let mut warning = base.clone();
        warning.pack_warnings.push(PackCompilationWarning {
            message: "identity warning".to_owned(),
            hints: vec!["identity hint".to_owned()],
        });
        let warning_identity = finalize_result(warning).result_identity;
        assert_ne!(warning_identity, identity);
        for warning in [
            PackCompilationWarning {
                message: "changed warning".to_owned(),
                hints: vec!["identity hint".to_owned()],
            },
            PackCompilationWarning {
                message: "identity warning".to_owned(),
                hints: vec!["changed hint".to_owned()],
            },
        ] {
            let mut result = base.clone();
            result.pack_warnings.push(warning);
            assert_ne!(finalize_result(result).result_identity, warning_identity);
        }

        let observation = CompilationAccessObservation {
            kind: CompilationAccessKind::File,
            logical_path: "project:identity.txt".to_owned(),
            font_index: Some(1),
            outcome: CompilationAccessOutcome::Read {
                byte_length: 8,
                digest: [1; 16],
            },
        };
        let mut access = base.clone();
        access.access_trace.observations.insert(observation.clone());
        let access_identity = finalize_result(access).result_identity;
        assert_ne!(access_identity, identity);
        let observation_mutations = [
            CompilationAccessObservation {
                kind: CompilationAccessKind::Source,
                ..observation.clone()
            },
            CompilationAccessObservation {
                logical_path: "project:changed.txt".to_owned(),
                ..observation.clone()
            },
            CompilationAccessObservation {
                font_index: Some(2),
                ..observation.clone()
            },
            CompilationAccessObservation {
                outcome: CompilationAccessOutcome::Read {
                    byte_length: 9,
                    digest: [1; 16],
                },
                ..observation.clone()
            },
            CompilationAccessObservation {
                outcome: CompilationAccessOutcome::Read {
                    byte_length: 8,
                    digest: [2; 16],
                },
                ..observation.clone()
            },
            CompilationAccessObservation {
                outcome: CompilationAccessOutcome::Missing,
                ..observation.clone()
            },
            CompilationAccessObservation {
                outcome: CompilationAccessOutcome::Failed,
                ..observation
            },
        ];
        for observation in observation_mutations {
            let mut result = base.clone();
            result.access_trace.observations.insert(observation);
            assert_ne!(finalize_result(result).result_identity, access_identity);
        }

        let mut artifact_format = base.clone();
        artifact_format.artifacts[0].format = OutputFormat::Png;
        assert_ne!(finalize_result(artifact_format).result_identity, identity);

        let mut artifact_page = base.clone();
        artifact_page.artifacts[0].source_page_number = NonZeroUsize::new(2);
        assert_ne!(finalize_result(artifact_page).result_identity, identity);

        let mut artifact = base.clone();
        let mut artifact_bytes = artifact.artifacts[0].bytes.as_slice().to_vec();
        artifact_bytes.push(0);
        artifact.artifacts[0].bytes = SharedBytes::new(artifact_bytes);
        assert_ne!(finalize_result(artifact).result_identity, identity);

        let mut ordered = base.clone();
        let mut second = ordered.artifacts[0].clone();
        let mut second_bytes = second.bytes.as_slice().to_vec();
        second_bytes.push(0);
        second.bytes = SharedBytes::new(second_bytes);
        ordered.artifacts.push(second);
        let ordered_identity = finalize_result(ordered.clone()).result_identity;
        ordered.artifacts.reverse();
        assert_ne!(finalize_result(ordered).result_identity, ordered_identity);

        let mut ordered_diagnostics = base.clone();
        let mut first_diagnostic = diagnostic_mutations[0].clone();
        first_diagnostic.message = "first diagnostic".to_owned();
        let mut second_diagnostic = diagnostic_mutations[1].clone();
        second_diagnostic.message = "second diagnostic".to_owned();
        ordered_diagnostics.diagnostics = vec![first_diagnostic, second_diagnostic];
        let ordered_diagnostics_identity =
            finalize_result(ordered_diagnostics.clone()).result_identity;
        ordered_diagnostics.diagnostics.reverse();
        assert_ne!(
            finalize_result(ordered_diagnostics).result_identity,
            ordered_diagnostics_identity
        );

        let mut ordered_warnings = base;
        ordered_warnings.pack_warnings = vec![
            PackCompilationWarning {
                message: "first warning".to_owned(),
                hints: vec![],
            },
            PackCompilationWarning {
                message: "second warning".to_owned(),
                hints: vec![],
            },
        ];
        let ordered_warnings_identity = finalize_result(ordered_warnings.clone()).result_identity;
        ordered_warnings.pack_warnings.reverse();
        assert_ne!(
            finalize_result(ordered_warnings).result_identity,
            ordered_warnings_identity
        );
    }

    #[test]
    fn compilation_identity_binds_every_implementation_identity_field() {
        fn mutations(
            identity: ImplementationIdentity,
            implementation: &'static str,
        ) -> [ImplementationIdentity; 7] {
            [
                ImplementationIdentity {
                    implementation,
                    ..identity
                },
                ImplementationIdentity {
                    version: "changed-version",
                    ..identity
                },
                ImplementationIdentity {
                    source_checksum: "changed-checksum",
                    ..identity
                },
                ImplementationIdentity {
                    target: "changed-target",
                    ..identity
                },
                ImplementationIdentity {
                    target_features: "changed-target-features",
                    ..identity
                },
                ImplementationIdentity {
                    feature_set: "changed-feature-set",
                    ..identity
                },
                ImplementationIdentity {
                    debug_assertions: !identity.debug_assertions,
                    ..identity
                },
            ]
        }

        let pack = Pack::builder("main.typ")
            .file("main.typ", b"implementation identity".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let report = compile(
            PackCompilationRequest::new(
                pack.clone(),
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap();
        let result = report.result().unwrap();
        let inventory = result.request_inventory();
        let engine = result.engine_identity();
        let exporter = result.exporter_identity();
        let baseline = result.compilation_identity();
        for engine in mutations(engine.0, "changed-engine").map(EngineIdentity) {
            assert_ne!(
                compilation_identity(&pack, inventory, engine, exporter),
                baseline
            );
        }

        for exporter in mutations(exporter.0, "changed-exporter").map(ExporterIdentity) {
            assert_ne!(
                compilation_identity(&pack, inventory, engine, exporter),
                baseline
            );
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_export_scheduler_obeys_worker_limit_and_preserves_input_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(4));
        let output =
            export_artifacts_bounded((0u8..8).collect(), CompilationLimits::reference_v1(), {
                let active = Arc::clone(&active);
                let peak = Arc::clone(&peak);
                let barrier = Arc::clone(&barrier);
                move |item| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    barrier.wait();
                    let workers = rayon::current_num_threads();
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(CompilationArtifact {
                        format: OutputFormat::Svg,
                        bytes: SharedBytes::new(vec![item, workers as u8]),
                        source_page_number: None,
                    })
                }
            })
            .unwrap();

        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert_eq!(
            output
                .iter()
                .map(|artifact| artifact.bytes().to_vec())
                .collect::<Vec<_>>(),
            (0u8..8).map(|item| vec![item, 4]).collect::<Vec<_>>()
        );
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn worker_count_does_not_change_exporter_and_limit_outcome_order() {
        let execute = |workers| {
            export_artifacts_bounded(
                vec![0u8, 1],
                CompilationLimits::new(2, 2, 1, 1, 0, 2, workers).unwrap(),
                |item| {
                    if item == 1 {
                        Err(CompileError::PngExport {
                            message: "later exporter failure".to_owned(),
                            warnings: EcoVec::new(),
                            pack_warnings: EcoVec::new(),
                            source_page_count: 2,
                            source_page_number: NonZeroUsize::new(2).unwrap(),
                        })
                    } else {
                        Ok(CompilationArtifact {
                            format: OutputFormat::Png,
                            bytes: SharedBytes::new(vec![item]),
                            source_page_number: NonZeroUsize::new(1),
                        })
                    }
                },
            )
            .unwrap_err()
        };

        for error in [execute(1), execute(2)] {
            assert!(matches!(
                error,
                CompileError::Limit(CompilationLimitError::Exceeded {
                    resource: CompilationResource::ArtifactBytes,
                    ceiling: 0,
                    observed_at_least: 1,
                })
            ));
        }
    }
}
