//! Pack Creation: one representative Typst request over supplied inputs.
//!
//! Creation acquires nothing. The caller supplies a [`ProjectSnapshot`], a
//! [`CandidateFontCatalog`], and the Complete Package Trees resolved for the
//! document, all as bytes it already holds, so the operation runs wherever the
//! core runs — including a host with no filesystem and no clock. Obtaining
//! those inputs is Creation Preparation and belongs to a Creation Adapter.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use ecow::EcoVec;
use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore};

use crate::compile::TypstTarget;
use crate::embedded::EmbeddedTypst;
use crate::font_catalog::{CandidateFontCatalog, CandidateFonts, FontDisposition};
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError};
use crate::project_snapshot::ProjectSnapshot;

/// Whether a Complete Package Tree's bytes travel inside the Pack or must be
/// fulfilled externally when the Pack is compiled.
///
/// A caller declares the disposition of every tree it supplies; creation never
/// derives it from a global choice, so one Pack may embed a small helper
/// package and reference a large template package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackageDisposition {
    /// The tree's exact bytes are stored in the Pack.
    Embedded,
    /// The tree is declared and must be supplied at compilation.
    External,
}

impl PackageDisposition {
    /// Whether the tree's exact bytes are stored in the Pack.
    pub fn is_embedded(self) -> bool {
        matches!(self, Self::Embedded)
    }
}

/// One Complete Package Tree resolved for an exact package specification, and
/// the disposition it carries into the Pack.
#[derive(Clone, Debug)]
pub struct ResolvedPackageTree {
    spec: PackageSpec,
    files: Vec<(String, Bytes)>,
    disposition: PackageDisposition,
}

impl ResolvedPackageTree {
    /// Supplies the tree resolved for `spec` under the given disposition.
    ///
    /// Paths are canonicalized before the representative request runs, and two
    /// entries naming one canonical package file keep the bytes supplied last,
    /// exactly as repeated [`PackBuilder`](crate::PackBuilder) calls do.
    pub fn new<I, P, D>(spec: PackageSpec, files: I, disposition: PackageDisposition) -> Self
    where
        I: IntoIterator<Item = (P, D)>,
        P: Into<String>,
        D: Into<Vec<u8>>,
    {
        Self {
            spec,
            files: files
                .into_iter()
                .map(|(path, data)| (path.into(), Bytes::new(data.into())))
                .collect(),
            disposition,
        }
    }

    /// Supplies a tree whose bytes are stored in the Pack.
    pub fn embedded<I, P, D>(spec: PackageSpec, files: I) -> Self
    where
        I: IntoIterator<Item = (P, D)>,
        P: Into<String>,
        D: Into<Vec<u8>>,
    {
        Self::new(spec, files, PackageDisposition::Embedded)
    }

    /// Supplies a tree that must be fulfilled externally.
    pub fn external<I, P, D>(spec: PackageSpec, files: I) -> Self
    where
        I: IntoIterator<Item = (P, D)>,
        P: Into<String>,
        D: Into<Vec<u8>>,
    {
        Self::new(spec, files, PackageDisposition::External)
    }

    /// The exact specification the tree was resolved for.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The supplied files, as package-relative paths and exact bytes.
    pub fn files(&self) -> impl Iterator<Item = (&str, &Bytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    /// Whether the tree travels inside the Pack.
    pub fn disposition(&self) -> PackageDisposition {
        self.disposition
    }
}

/// The owned values one Pack Creation runs over.
///
/// The caller constructs it and keeps it; creation borrows it and retains
/// nothing after returning, so the same request may be run again.
#[derive(Clone, Debug)]
pub struct CreationRequest {
    project: ProjectSnapshot,
    creation_timestamp: i64,
    fonts: CandidateFontCatalog,
    packages: BTreeMap<String, ResolvedPackageTree>,
    target: TypstTarget,
    inputs: Dict,
    features: Vec<Feature>,
    metadata: Option<PackMetadata>,
}

impl CreationRequest {
    /// Creates a request over one Project Snapshot.
    ///
    /// `creation_timestamp` is required: it fixes the representative request's
    /// Document Time, because creation consults no wall clock of its own.
    pub fn new(project: ProjectSnapshot, creation_timestamp: i64) -> Self {
        Self {
            project,
            creation_timestamp,
            fonts: CandidateFontCatalog::new(),
            packages: BTreeMap::new(),
            target: TypstTarget::Paged,
            inputs: Dict::new(),
            features: Vec::new(),
            metadata: None,
        }
    }

    /// Offers the Candidate Font Catalog creation may select faces from.
    /// Defaults to an empty catalog, which offers no face at all.
    pub fn font_catalog(mut self, catalog: CandidateFontCatalog) -> Self {
        self.fonts = catalog;
        self
    }

    /// Supplies one resolved Complete Package Tree, replacing any tree already
    /// supplied for the same specification.
    pub fn package_tree(mut self, tree: ResolvedPackageTree) -> Self {
        self.packages.insert(tree.spec.to_string(), tree);
        self
    }

    /// Supplies several resolved Complete Package Trees.
    pub fn package_trees(mut self, trees: impl IntoIterator<Item = ResolvedPackageTree>) -> Self {
        for tree in trees {
            self = self.package_tree(tree);
        }
        self
    }

    /// Selects the Typst Target of the representative request. Defaults to
    /// [`TypstTarget::Paged`].
    pub fn target(mut self, target: TypstTarget) -> Self {
        self.target = target;
        self
    }

    /// Values made available to document code as `sys.inputs` during the
    /// representative request.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables an experimental Typst language feature for the representative
    /// request.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Sets descriptive metadata recorded in the Pack Manifest.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// One Pack issued by creation, and the warnings its representative compile
/// produced.
#[derive(Debug)]
pub struct IssuedPack {
    /// The issued Pack.
    pub pack: Pack,
    /// Warnings emitted by the representative compile.
    pub warnings: EcoVec<SourceDiagnostic>,
}

/// A failure that issues no Pack.
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    /// The representative request did not compile, so no Pack describes a
    /// document that compiled.
    #[error("the representative creation compile failed with {} error(s)", errors.len())]
    Compile {
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    /// The creation timestamp does not name a representable instant.
    #[error("invalid creation timestamp: {0}")]
    InvalidTimestamp(String),
    /// The representative request read a file of a package no supplied tree
    /// covers, so the Pack would omit a requirement the document needs.
    ///
    /// An unsatisfied import fails the representative compile, so reaching
    /// this means the request compiled around a package it read; creation
    /// fails rather than issuing an incomplete Pack.
    #[error("no supplied tree satisfies package {spec}, which the representative request read")]
    UnsuppliedPackage { spec: PackageSpec },
    /// A supplied tree holds a path that cannot name a package file.
    #[error("package {spec} file `{path}` cannot be represented: {message}")]
    InvalidPackagePath {
        spec: PackageSpec,
        path: String,
        message: String,
    },
    /// The selected inputs do not assemble into a valid Pack.
    #[error(transparent)]
    Build(#[from] PackBuildError),
}

/// Runs one representative Typst request over the supplied inputs and issues
/// the Pack it selected.
///
/// Compiler observations select package and font requirements; project files
/// come from the Project Snapshot alone. Creation fails rather than issuing an
/// incomplete Pack when the representative request does not compile.
pub fn create(request: &CreationRequest) -> Result<IssuedPack, CreationError> {
    let packages = canonical_package_trees(request)?;
    let time = Time::fixed_timestamp(request.creation_timestamp)
        .map_err(|error| CreationError::InvalidTimestamp(error.to_string()))?;
    let entrypoint = VirtualPath::new(request.project.entrypoint())
        .expect("Project Snapshot entrypoint invariant violated");

    let mut world = SuppliedWorld {
        library: LazyHash::new(
            Library::builder()
                .with_inputs(request.inputs.clone())
                .with_features(request.features.iter().copied().collect())
                .build(),
        ),
        main: RootedPath::new(VirtualRoot::Project, entrypoint).intern(),
        files: FileStore::new(SuppliedLoader {
            project: &request.project,
            packages,
        }),
        fonts: request.fonts.expand(),
        used_font_indices: Mutex::new(BTreeSet::new()),
        time,
    };

    let Warned { output, warnings } = compile_creation_target(&world, request.target);
    if let Err(errors) = output {
        return Err(CreationError::Compile { errors, warnings });
    }

    let mut builder = Pack::builder(request.project.entrypoint());
    for (path, data) in request.project.files() {
        builder = builder.file(path, data.to_vec())?;
    }

    // Packages, in canonical specification order. The whole Complete Package
    // Tree travels, not only the files the representative request read.
    let observed = world.observed_packages();
    let loader = world.files.loader();
    for spec in observed {
        let Some(tree) = loader.packages.get(&spec.to_string()) else {
            return Err(CreationError::UnsuppliedPackage { spec });
        };
        for (path, data) in &tree.files {
            builder = if tree.disposition.is_embedded() {
                builder.package_file(spec.clone(), path, data.to_vec())?
            } else {
                builder.external_package_file(spec.clone(), path, data.to_vec())?
            };
        }
    }

    // Selected faces in candidate catalog order, each under the disposition
    // its container carries.
    for (font, disposition) in world.used_fonts() {
        builder = if disposition.is_embedded() {
            builder.font(font.data().to_vec(), font.index())?
        } else {
            builder.external_font(font.data().to_vec(), font.index())?
        };
    }

    if let Some(metadata) = &request.metadata {
        builder = builder.metadata(metadata.clone());
    }

    Ok(IssuedPack {
        pack: builder.build()?,
        warnings,
    })
}

/// Canonicalizes every supplied tree before the representative request runs,
/// so that a tree is looked up and contained under the same path.
fn canonical_package_trees(
    request: &CreationRequest,
) -> Result<BTreeMap<String, CanonicalPackageTree>, CreationError> {
    let mut trees = BTreeMap::new();
    for (key, tree) in &request.packages {
        let mut files = BTreeMap::new();
        for (path, data) in &tree.files {
            let canonical = Pack::canonical_package_path(path).map_err(|message| {
                CreationError::InvalidPackagePath {
                    spec: tree.spec.clone(),
                    path: path.clone(),
                    message,
                }
            })?;
            files.insert(canonical, data.clone());
        }
        trees.insert(
            key.clone(),
            CanonicalPackageTree {
                files,
                disposition: tree.disposition,
            },
        );
    }
    Ok(trees)
}

/// One supplied Complete Package Tree, keyed by canonical package-relative
/// path.
struct CanonicalPackageTree {
    files: BTreeMap<String, Bytes>,
    disposition: PackageDisposition,
}

/// The world the representative request compiles against: supplied bytes and
/// nothing else.
struct SuppliedWorld<'a> {
    library: LazyHash<Library>,
    main: FileId,
    files: FileStore<SuppliedLoader<'a>>,
    fonts: CandidateFonts,
    used_font_indices: Mutex<BTreeSet<usize>>,
    time: Time,
}

impl SuppliedWorld<'_> {
    /// The packages the representative request read a file of, in canonical
    /// specification order.
    fn observed_packages(&mut self) -> Vec<PackageSpec> {
        let mut specs: BTreeMap<String, PackageSpec> = BTreeMap::new();
        let (_, dependencies) = self.files.dependencies();
        for id in dependencies {
            if let VirtualRoot::Package(spec) = id.root() {
                specs.insert(spec.to_string(), spec.clone());
            }
        }
        specs.into_values().collect()
    }

    /// The selected faces in candidate catalog order, each with the
    /// disposition its container carries.
    fn used_fonts(&self) -> Vec<(Font, FontDisposition)> {
        self.used_font_indices
            .lock()
            .expect("used font index lock poisoned")
            .iter()
            .filter_map(|index| Some((self.fonts.font(*index)?, self.fonts.disposition(*index)?)))
            .collect()
    }
}

impl World for SuppliedWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        let font = self.fonts.font(index);
        if font.is_some() {
            self.used_font_indices
                .lock()
                .expect("used font index lock poisoned")
                .insert(index);
        }
        font
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.time.today(offset)
    }
}

/// Serves file requests from the supplied Project Snapshot and package trees.
struct SuppliedLoader<'a> {
    project: &'a ProjectSnapshot,
    packages: BTreeMap<String, CanonicalPackageTree>,
}

impl FileLoader for SuppliedLoader<'_> {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => self
                .project
                .file(path)
                .cloned()
                .ok_or_else(|| FileError::NotFound(path.into())),
            VirtualRoot::Package(spec) => {
                let tree = self
                    .packages
                    .get(&spec.to_string())
                    .ok_or_else(|| FileError::Package(PackageError::NotFound(spec.clone())))?;
                tree.files
                    .get(path)
                    .cloned()
                    .ok_or_else(|| FileError::NotFound(path.into()))
            }
        }
    }
}

/// Runs the representative request for the selected Typst Target, keeping only
/// what selects requirements: whether it compiled, and its warnings.
pub(crate) fn compile_creation_target(
    world: &dyn World,
    target: TypstTarget,
) -> Warned<Result<(), EcoVec<SourceDiagnostic>>> {
    match target {
        TypstTarget::Paged => {
            let Warned { output, warnings } = EmbeddedTypst::compile_paged(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
        TypstTarget::Html => {
            let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
    }
}
