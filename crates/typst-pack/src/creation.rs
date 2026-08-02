//! Pack Creation: one representative Typst request over supplied inputs.
//!
//! Creation acquires nothing. The caller supplies a [`ProjectSnapshot`], a
//! [`FontCatalog`], and the Package Trees resolved for the
//! document, all as bytes it already holds, so the operation runs wherever the
//! core runs — including a host with no filesystem and no clock. Obtaining
//! those inputs belongs to Pack Assembly and a Pack Assembler.

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
use crate::font_catalog::{CatalogFonts, FontCatalog, FontDisposition};
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError};
use crate::package_catalog::PackageCatalog;
use crate::payload::SharedBytes;
use crate::project_snapshot::ProjectSnapshot;

/// The owned values one Pack Creation runs over.
///
/// The caller constructs it and keeps it; creation borrows it and retains
/// nothing after returning, so the same request may be run again.
#[derive(Clone, Debug)]
pub struct CreationRequest {
    project: ProjectSnapshot,
    creation_timestamp: i64,
    fonts: FontCatalog,
    packages: PackageCatalog,
    unresolvable: BTreeMap<String, PackageError>,
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
            fonts: FontCatalog::new(),
            packages: PackageCatalog::new(),
            unresolvable: BTreeMap::new(),
            target: TypstTarget::Paged,
            inputs: Dict::new(),
            features: Vec::new(),
            metadata: None,
        }
    }

    /// Offers the Font Catalog creation may select faces from.
    /// Defaults to an empty catalog, which offers no face at all.
    pub fn font_catalog(mut self, catalog: FontCatalog) -> Self {
        self.fonts = catalog;
        self
    }

    /// Supplies the validated Package Catalog creation may select from.
    pub fn package_catalog(mut self, catalog: PackageCatalog) -> Self {
        self.packages = catalog;
        self
    }

    /// Declares that Pack Assembly could not resolve one reported
    /// specification, and the failure it met doing so.
    ///
    /// Creation stops reporting that specification as missing, and the
    /// representative request fails at the file request that needed it,
    /// carrying `failure` as its diagnostic. An acquisition failure is
    /// therefore reported at the import that asked for the package rather than
    /// beside it, which is the only place the caller's own reason and the
    /// source location it belongs to can meet: the specifications creation
    /// reports name a package, never the file that imported it.
    ///
    /// A tree supplied for the same specification takes precedence, so a
    /// caller that resolves it after all simply supplies it.
    pub fn unresolvable_package(mut self, spec: PackageSpec, failure: PackageError) -> Self {
        self.unresolvable.insert(spec.to_string(), failure);
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

/// What one Pack Creation invocation produced.
///
/// Package requirements can only be discovered by compiling, so creation
/// resolves acquisition through a resumable protocol rather than a callback: a
/// representative request that read a package no supplied tree covers reports
/// that specification instead of issuing a Pack. The caller obtains the tree
/// however its host allows and invokes creation again with the same Creation
/// Request values and the reported trees added. Creation retains nothing
/// between invocations, so a resume step is valid across a host request
/// boundary and needs no asynchronous library interface.
#[derive(Debug)]
pub enum CreationOutcome {
    /// The representative request ran over trees that covered every package it
    /// read, so creation selected every requirement and issued a Pack.
    Issued(Box<IssuedPack>),
    /// The representative request read packages no supplied tree covers, in
    /// canonical specification order. This is a normal, resumable outcome and
    /// not a failure.
    ///
    /// Every specification comes from a package file request the compiler
    /// actually made, so a caller never parses diagnostic text to drive its
    /// loop, and every one carries an exact version, because a Typst import
    /// specification always does.
    ///
    /// A caller that cannot resolve one declares it through
    /// [`CreationRequest::unresolvable_package`] rather than abandoning the
    /// loop, so that its own failure is reported at the import that needed the
    /// package.
    MissingPackages(Vec<PackageSpec>),
}

impl CreationOutcome {
    /// Takes the issued Pack, or `None` when creation reported missing
    /// packages, for a caller that supplied every tree the document needs.
    /// A caller driving the resume protocol matches the outcome instead.
    pub fn into_issued(self) -> Option<IssuedPack> {
        match self {
            Self::Issued(issued) => Some(*issued),
            Self::MissingPackages(_) => None,
        }
    }
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
    /// The selected inputs do not assemble into a valid Pack.
    #[error(transparent)]
    Build(#[from] PackBuildError),
}

/// Runs one representative Typst request over the supplied inputs and issues
/// the Pack it selected, or reports the packages it needed and was not given.
///
/// Compiler observations select package and font requirements; project files
/// come from the Project Snapshot alone. Creation fails rather than issuing an
/// incomplete Pack when the representative request does not compile.
///
/// A request that read a package no supplied tree covers returns
/// [`CreationOutcome::MissingPackages`] instead: resolve those specifications,
/// add their trees to the same request, and invoke creation again. Because a
/// failed import ends module evaluation, one round reports what that round
/// reached, and a project needing several packages completes over repeated
/// invocation. A specification the caller cannot resolve ends the loop through
/// [`CreationRequest::unresolvable_package`], which fails the next round's
/// representative request at the import that needed it.
///
/// Creation holds owned bytes and has nothing to re-read. A Pack represents the
/// exact values its source adapters acquired, without guaranteeing that values
/// from mutable sources all coexisted at one instant.
pub fn create(request: &CreationRequest) -> Result<CreationOutcome, CreationError> {
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
            packages: &request.packages,
            unresolvable: &request.unresolvable,
        }),
        fonts: request.fonts.expand(),
        used_font_indices: Mutex::new(BTreeSet::new()),
        time,
    };

    let Warned { output, warnings } = compile_creation_target(&world, request.target);
    let observed = world.observed_packages();

    // Reported before the compile outcome is inspected, because the import that
    // needed a tree is exactly what failed the compile. The caller resolves
    // these and invokes creation again rather than reading diagnostics.
    if !observed.missing.is_empty() {
        return Ok(CreationOutcome::MissingPackages(observed.missing));
    }

    if let Err(errors) = output {
        return Err(CreationError::Compile { errors, warnings });
    }

    let mut builder = Pack::builder(request.project.entrypoint());
    for (path, data) in request.project.shared_files() {
        builder = builder.shared_file(path, data.clone())?;
    }

    // Packages, in canonical specification order. The whole Complete Package
    // Tree travels, not only the files the representative request read.
    let loader = world.files.loader();
    for spec in observed.supplied {
        let entry = loader
            .packages
            .get(&spec)
            .expect("observed package was partitioned as supplied");
        for (path, data) in entry.tree().shared_files() {
            builder = if entry.disposition().is_embedded() {
                builder.shared_package_file(spec.clone(), path, data.clone())?
            } else {
                builder.shared_external_package_file(spec.clone(), path, data.clone())?
            };
        }
    }

    // Selected faces in candidate catalog order, each under the disposition
    // its container carries.
    for (font, disposition) in world.used_fonts() {
        builder = if disposition.is_embedded() {
            builder.shared_font(SharedBytes::from_typst(font.data().clone()), font.index())?
        } else {
            builder
                .shared_external_font(SharedBytes::from_typst(font.data().clone()), font.index())?
        };
    }

    if let Some(metadata) = &request.metadata {
        builder = builder.metadata(metadata.clone());
    }

    Ok(CreationOutcome::Issued(Box::new(IssuedPack {
        pack: builder.build()?,
        warnings,
    })))
}

/// The package specifications one representative request asked for, split by
/// whether the supplied trees covered them.
#[derive(Default)]
struct ObservedPackages {
    /// Specifications a supplied tree covers, which become Package
    /// Requirements.
    supplied: Vec<PackageSpec>,
    /// Specifications no supplied tree covers, which creation reports so the
    /// caller can resolve them and invoke creation again.
    missing: Vec<PackageSpec>,
}

/// The world the representative request compiles against: supplied bytes and
/// nothing else.
struct SuppliedWorld<'a> {
    library: LazyHash<Library>,
    main: FileId,
    files: FileStore<SuppliedLoader<'a>>,
    fonts: CatalogFonts,
    used_font_indices: Mutex<BTreeSet<usize>>,
    time: Time,
}

impl SuppliedWorld<'_> {
    /// The packages the representative request asked for a file of, split by
    /// whether a supplied tree covers them, each in canonical specification
    /// order.
    ///
    /// Both halves come from the requests the compiler made, not from what its
    /// diagnostics said about them.
    fn observed_packages(&mut self) -> ObservedPackages {
        let mut specs: BTreeMap<String, PackageSpec> = BTreeMap::new();
        let (loader, dependencies) = self.files.dependencies();
        for id in dependencies {
            if let VirtualRoot::Package(spec) = id.root() {
                specs.insert(spec.to_string(), spec.clone());
            }
        }

        let mut observed = ObservedPackages::default();
        for (key, spec) in specs {
            if loader.packages.get(&spec).is_some() {
                observed.supplied.push(spec);
            } else if !loader.unresolvable.contains_key(&key) {
                observed.missing.push(spec);
            }
            // A specification the caller declared unresolvable is neither. The
            // representative request already failed at it, carrying the
            // caller's own reason, and reporting it again would ask for what
            // the caller said it cannot supply.
        }
        observed
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
    packages: &'a PackageCatalog,
    /// The specifications the caller declared it could not resolve, and the
    /// failure it met, which the request that needs one fails with.
    unresolvable: &'a BTreeMap<String, PackageError>,
}

impl FileLoader for SuppliedLoader<'_> {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => self
                .project
                .shared_file(path)
                .map(|data| data.to_typst())
                .ok_or_else(|| FileError::NotFound(path.into())),
            VirtualRoot::Package(spec) => {
                let key = spec.to_string();
                let Some(entry) = self.packages.get(spec) else {
                    // A supplied tree first, so a caller that resolved a
                    // specification it had declared unresolvable is served it.
                    return Err(FileError::Package(
                        self.unresolvable
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| PackageError::NotFound(spec.clone())),
                    ));
                };
                entry
                    .tree()
                    .shared_file(path)
                    .map(SharedBytes::to_typst)
                    .ok_or_else(|| FileError::NotFound(path.into()))
            }
        }
    }
}

/// Runs the representative request for the selected Typst Target, keeping only
/// what selects requirements: whether it compiled, and its warnings.
fn compile_creation_target(
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
