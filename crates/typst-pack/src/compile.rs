//! Compiling a Pack into output artifacts.

mod export;
mod output;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use typst::Feature;
use typst::diag::SourceDiagnostic;
use typst::foundations::{Bytes, Dict, IntoValue};
use typst::syntax::package::PackageSpec;
use typst::text::Font;
use typst::{Library, LibraryExt};

pub use self::output::{
    CompilationLimits, CreationTimestamp, HtmlOutput, Output, OutputFormat, PageRange,
    PageSelection, PdfOutput, PngOutput, SvgOutput, parse_page_selection,
};
use crate::domain::DocumentTime;
use crate::identity::Preimage;
use crate::world::{Clock, Loader, SourceWorld, Sources};
use crate::{Digest, FontContainer, LimitExceeded, Pack, PackageTree, VERSION};

/// A value exposed to document code through `sys.inputs`.
///
/// Strings appear as Typst `str` values and bytes as Typst `bytes` values, so
/// a document can, for example, decode an image with `image(sys.inputs.logo)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Str(String),
    Bytes(Vec<u8>),
}

impl From<&str> for Input {
    fn from(value: &str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<String> for Input {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl From<Vec<u8>> for Input {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

/// The document-facing controls shared by creation and compilation.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Values exposed as `sys.inputs`, in order; a repeated key replaces the
    /// earlier value in place.
    pub inputs: Vec<(String, Input)>,
    /// What `datetime.today()` returns.
    pub document_time: DocumentTime,
    /// Typst engine features. HTML output enables [`Feature::Html`] itself;
    /// [`Feature::Bundle`] is not supported.
    pub features: Vec<Feature>,
}

/// One compilation of a Pack.
///
/// Compilation reads nothing but these values: no project files, package
/// caches, fonts, environment variables, clock, or network.
#[derive(Debug, Clone)]
pub struct CompileRequest {
    /// The pack to compile.
    pub pack: Pack,
    /// The output to produce.
    pub output: Output,
    /// Inputs, document time, and features.
    pub options: CompileOptions,
    /// Package trees for the pack's external package requirements. Each
    /// requirement uses the tree supplied under its specification; trees the
    /// pack does not require externally are ignored.
    pub packages: Vec<(PackageSpec, PackageTree)>,
    /// Font containers for the pack's external font requirements, matched by
    /// digest. Containers the pack does not require externally are ignored.
    pub fonts: Vec<FontContainer>,
    /// Resource ceilings for the produced output.
    pub limits: CompilationLimits,
}

impl CompileRequest {
    /// Compiles `pack` to `output` with default options, no external
    /// dependencies, and default limits.
    pub fn new(pack: Pack, output: Output) -> Self {
        Self {
            pack,
            output,
            options: CompileOptions::default(),
            packages: Vec::new(),
            fonts: Vec::new(),
            limits: CompilationLimits::default(),
        }
    }
}

/// A successful compilation.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// One artifact for PDF and HTML; one per selected page for PNG and SVG.
    pub artifacts: Vec<Artifact>,
    /// The number of pages in the compiled document; `None` for HTML.
    pub page_count: Option<usize>,
    /// Compiler and exporter warnings, in Typst's order.
    pub warnings: Vec<SourceDiagnostic>,
    /// The compilation identity; see the README's "Compilation identity".
    pub identity: Digest,
    /// The sources the compilation read.
    pub sources: Sources,
}

/// One file produced by compiling a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub format: OutputFormat,
    /// The one-based source page of a PNG or SVG artifact.
    pub page: Option<NonZeroUsize>,
    pub bytes: Vec<u8>,
}

/// A compilation that produced no artifacts.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// The request holds invalid values; nothing was compiled.
    #[error("{}", list("invalid compile request", .0))]
    InvalidRequest(Vec<RequestIssue>),
    /// The pack's external requirements were not supplied; nothing was
    /// compiled.
    #[error("{}", list("the pack's external dependencies are not satisfied", .0))]
    Unfulfilled(Vec<Unfulfilled>),
    /// The output exceeded a [`CompilationLimits`] ceiling.
    #[error(transparent)]
    Limit(#[from] LimitExceeded),
    /// Typst reported errors while compiling or exporting.
    #[error("{}", list("compilation failed", &.errors.iter().map(|error| &error.message).collect::<Vec<_>>()))]
    Failed {
        errors: Vec<SourceDiagnostic>,
        warnings: Vec<SourceDiagnostic>,
        sources: Sources,
    },
    /// A rendered page could not be encoded as PNG.
    #[error("PNG export failed for source page {page}: {message}")]
    Export {
        page: NonZeroUsize,
        message: String,
        warnings: Vec<SourceDiagnostic>,
        sources: Sources,
    },
}

impl CompileError {
    /// Advice for resolving the error, for presentation after its message.
    pub fn hints(&self) -> Vec<String> {
        match self {
            Self::InvalidRequest(issues) => issues
                .iter()
                .flat_map(|issue| match issue {
                    RequestIssue::InvalidPdfStandards { hints, .. } => hints.clone(),
                    _ => Vec::new(),
                })
                .collect(),
            Self::Unfulfilled(issues) => {
                let any = |kind: fn(&Unfulfilled) -> bool| issues.iter().any(kind);
                let mut hints = Vec::new();
                if any(|issue| matches!(issue, Unfulfilled::MissingPackage { .. })) {
                    hints.push(
                        "supply the packages the pack lists as external, or recreate the pack \
                         with its packages vendored"
                            .to_owned(),
                    );
                }
                if any(|issue| matches!(issue, Unfulfilled::MismatchedPackage { .. })) {
                    hints.push(
                        "a supplied package must be identical to the one the pack was created with"
                            .to_owned(),
                    );
                }
                if any(|issue| matches!(issue, Unfulfilled::MissingFont { .. })) {
                    hints.push(
                        "supply the font files the pack lists as external, or recreate the pack \
                         with its fonts embedded"
                            .to_owned(),
                    );
                }
                hints
            }
            _ => Vec::new(),
        }
    }
}

/// Formats an aggregate as its only issue, or a summary listing every issue.
pub(crate) fn list(summary: &str, issues: &[impl std::fmt::Display]) -> String {
    match issues {
        [issue] => issue.to_string(),
        issues => {
            let issues = issues.iter().map(ToString::to_string).collect::<Vec<_>>();
            format!("{summary}: {}", issues.join("; "))
        }
    }
}

/// One invalid value in a compile or create request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestIssue {
    #[error("page selection range {start}-{end} ends before it starts")]
    InvalidPageRange {
        start: NonZeroUsize,
        end: NonZeroUsize,
    },
    #[error("PNG pixels per inch must be finite and greater than zero")]
    InvalidPpi,
    /// Typst's PDF exporter rejects the combination of standards.
    #[error("{message}")]
    InvalidPdfStandards { message: String, hints: Vec<String> },
    #[error("the document-time UNIX timestamp is out of range")]
    InvalidDocumentTimestamp,
    #[error("the Typst Bundle feature is not supported")]
    UnsupportedBundleFeature,
}

/// One external requirement of a pack that the request does not satisfy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unfulfilled {
    #[error("required package {spec} was not supplied")]
    MissingPackage { spec: PackageSpec },
    #[error("package {spec} was supplied with tree {actual}, but the pack requires {expected}")]
    MismatchedPackage {
        spec: PackageSpec,
        expected: Digest,
        actual: Digest,
    },
    #[error("required font container {digest} ({families}) was not supplied")]
    MissingFont { digest: Digest, families: String },
}

/// Validated options turned into what a world is built from.
pub(crate) struct Environment {
    pub(crate) library: Library,
    pub(crate) clock: Clock,
    inputs: Vec<(String, Input)>,
    features: Vec<Feature>,
    document_time: DocumentTime,
}

impl Environment {
    /// Validates `options`, enabling HTML when the target requires it.
    pub(crate) fn new(
        options: &CompileOptions,
        html: bool,
        issues: &mut Vec<RequestIssue>,
    ) -> Option<Self> {
        let clock = Clock::new(options.document_time);
        if clock.is_none() {
            issues.push(RequestIssue::InvalidDocumentTimestamp);
        }
        if options.features.contains(&Feature::Bundle) {
            issues.push(RequestIssue::UnsupportedBundleFeature);
        }
        let features = [Feature::Html, Feature::A11yExtras]
            .into_iter()
            .filter(|feature| {
                options.features.contains(feature) || (html && *feature == Feature::Html)
            })
            .collect::<Vec<_>>();
        let mut inputs = Vec::<(String, Input)>::new();
        for (key, value) in &options.inputs {
            match inputs.iter_mut().find(|(existing, _)| existing == key) {
                Some((_, existing)) => *existing = value.clone(),
                None => inputs.push((key.clone(), value.clone())),
            }
        }
        let mut dict = Dict::new();
        for (key, value) in &inputs {
            let value = match value {
                Input::Str(text) => text.as_str().into_value(),
                Input::Bytes(data) => Bytes::new(data.clone()).into_value(),
            };
            dict.insert(key.as_str().into(), value);
        }
        let library = Library::builder()
            .with_inputs(dict)
            .with_features(features.iter().copied().collect())
            .build();
        Some(Self {
            library,
            clock: clock?,
            inputs,
            features,
            document_time: options.document_time,
        })
    }

    fn write(&self, preimage: &mut Preimage) {
        preimage.count(self.inputs.len());
        for (key, value) in &self.inputs {
            preimage.str(key);
            match value {
                Input::Str(text) => preimage.str("str").str(text),
                Input::Bytes(data) => preimage.str("bytes").digest(&Digest::of(data)),
            };
        }
        match self.document_time {
            DocumentTime::Absent => {
                preimage.str("absent");
            }
            DocumentTime::UnixTimestamp(seconds) => {
                preimage.str("unix").int(seconds);
            }
            DocumentTime::Fixed(datetime) => {
                preimage.str("fixed");
                for part in [
                    datetime.year().map(i64::from),
                    datetime.month().map(i64::from),
                    datetime.day().map(i64::from),
                    datetime.hour().map(i64::from),
                    datetime.minute().map(i64::from),
                    datetime.second().map(i64::from),
                ] {
                    preimage.bool(part.is_some()).int(part.unwrap_or_default());
                }
            }
        }
        preimage.count(self.features.len());
        for feature in &self.features {
            preimage.str(match feature {
                Feature::Html => "html",
                _ => "a11y-extras",
            });
        }
    }
}

/// Compiles a pack.
///
/// The request is validated, the pack's external requirements are matched
/// against the supplied packages and fonts, and then Typst compiles the
/// document and exports the output.
///
/// ```
/// use typst_pack::{CompileRequest, Input, Output, OutputFormat, Pack, compile};
///
/// let pack = Pack::builder("main.typ")
///     .file("main.typ", b"Hello, #sys.inputs.name!".to_vec())
///     .build()?;
/// let mut request = CompileRequest::new(pack, Output::new(OutputFormat::Svg));
/// request.options.inputs.push(("name".into(), Input::from("World")));
///
/// let compiled = compile(&request)?;
/// assert_eq!(compiled.artifacts.len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compile(request: &CompileRequest) -> Result<Compiled, CompileError> {
    let _timing = typst_timing::TimingScope::new("typst-pack compilation");
    let mut issues = Vec::new();
    let normalized = request.output.normalize(&mut issues);
    let html = matches!(normalized.output, Output::Html(_));
    let environment = Environment::new(&request.options, html, &mut issues);
    let Some(environment) = environment.filter(|_| issues.is_empty()) else {
        return Err(CompileError::InvalidRequest(issues));
    };

    let pack = &request.pack;
    let (packages, fonts) = fulfill(pack, &request.packages, &request.fonts)?;

    let mut preimage = Preimage::new("typst-pack/compilation/v1");
    preimage.str(VERSION).digest(&pack.identity());
    normalized.output.write(&mut preimage);
    environment.write(&mut preimage);
    preimage.count(packages.len());
    for (spec, tree) in &packages {
        preimage.str(spec).digest(&tree.digest());
    }
    preimage.count(fonts.len());
    for container in fonts.values() {
        preimage.digest(&container.digest());
    }
    let identity = preimage.finish();

    let containers = pack
        .font_requirements()
        .iter()
        .map(
            |requirement| match pack.shared_font_data(&requirement.digest) {
                Some(data) => Font::iter(data.to_typst()).collect(),
                None => fonts[&requirement.digest].fonts().to_vec(),
            },
        )
        .collect::<Vec<_>>();
    let mut trees = pack.embedded_packages().clone();
    trees.extend(packages);
    let loader = Loader {
        project: pack.project().clone(),
        packages: trees,
        failures: Default::default(),
    };
    let world = SourceWorld::new(loader, containers, environment.library, environment.clock);
    let exported = export::export(&world, &normalized, request.limits);
    let sources = world.into_sources();
    match exported {
        Ok(exported) => Ok(Compiled {
            artifacts: exported.artifacts,
            page_count: exported.page_count,
            warnings: exported.warnings.into_iter().collect(),
            identity,
            sources,
        }),
        Err(export::Failure::Limit(error)) => Err(CompileError::Limit(error)),
        Err(export::Failure::Diagnostics { errors, warnings }) => Err(CompileError::Failed {
            errors: errors.into_iter().collect(),
            warnings: warnings.into_iter().collect(),
            sources,
        }),
        Err(export::Failure::Png {
            page,
            message,
            warnings,
        }) => Err(CompileError::Export {
            page,
            message,
            warnings: warnings.into_iter().collect(),
            sources,
        }),
    }
}

type Fulfilled = (
    BTreeMap<String, PackageTree>,
    BTreeMap<Digest, FontContainer>,
);

/// Selects the supplied tree or container for every external requirement.
fn fulfill(
    pack: &Pack,
    packages: &[(PackageSpec, PackageTree)],
    fonts: &[FontContainer],
) -> Result<Fulfilled, CompileError> {
    let mut issues = Vec::new();
    let mut trees = BTreeMap::new();
    for requirement in pack.package_requirements().iter().filter(|r| !r.embedded) {
        let mut supplied = packages
            .iter()
            .filter(|(spec, _)| *spec == requirement.spec)
            .map(|(_, tree)| tree);
        let first = supplied.clone().next();
        match supplied.find(|tree| tree.digest() == requirement.digest) {
            Some(tree) => {
                trees.insert(requirement.spec.to_string(), tree.clone());
            }
            None => issues.push(match first {
                Some(tree) => Unfulfilled::MismatchedPackage {
                    spec: requirement.spec.clone(),
                    expected: requirement.digest,
                    actual: tree.digest(),
                },
                None => Unfulfilled::MissingPackage {
                    spec: requirement.spec.clone(),
                },
            }),
        }
    }
    let mut containers = BTreeMap::new();
    for requirement in pack.font_requirements().iter().filter(|r| !r.embedded) {
        match fonts
            .iter()
            .find(|font| font.digest() == requirement.digest)
        {
            Some(font) => {
                containers.insert(requirement.digest, font.clone());
            }
            None => issues.push(Unfulfilled::MissingFont {
                digest: requirement.digest,
                families: requirement.families(),
            }),
        }
    }
    if issues.is_empty() {
        Ok((trees, containers))
    } else {
        Err(CompileError::Unfulfilled(issues))
    }
}
