//! The reference Pack Assembler: Pack Assembly over a project
//! directory.
//!
//! The adapter acquires and the core transforms. It lists and reads the
//! project, composes the Font Catalog out of the font sources the
//! host offers, obtains the Package Trees the core reports as
//! missing, and resolves the creation timestamp; Pack Creation itself runs in
//! the core over those bytes.
//!
//! Each acquired value records the exact bytes observed by its source adapter;
//! the project gatherer does not reread the source solely to establish that all
//! values coexisted at one instant.

#![cfg(feature = "fs")]

use std::collections::HashSet;
use std::fmt;
#[cfg(feature = "diagnostics")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use ecow::EcoVec;
use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{FontPath, FontStore};

use crate::compile::TypstTarget;
use crate::creation::{CreationError, CreationOutcome, CreationRequest, IssuedPack, create};
#[cfg(feature = "embedded-fonts")]
use crate::font_catalog::typst_embedded_font_containers;
use crate::font_catalog::{FontCatalog, FontCatalogEntry, FontContainer, FontDisposition};
use crate::fs_packages::{
    AcquiredPackages, FilesystemPackageAcquisitionError, FilesystemPackageAuthority,
};
use crate::fs_project;
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError};
use crate::package_catalog::{PackageCatalog, PackageCatalogError, PackageDisposition};
use crate::package_failure::{PackageAcquisitionFailure, PackageAcquisitionFailureReason};
use crate::project_snapshot::ProjectSnapshot;

/// Packs a Typst project directory into a [`Pack`].
///
/// The packer snapshots every eligible regular file beneath the project root,
/// then performs one representative compile to select package and font
/// dependencies. Compiler observations never select project files.
///
/// It is the reference Pack Assembler: it acquires the project, the
/// Font Catalog, and the package trees creation reports as missing,
/// and [`create`](crate::create) selects requirements over those bytes. How far
/// its acquisition reaches is a build-time choice: with the `fs` feature alone
/// it resolves reported specifications from local package directories and the
/// host's package cache, and with `egress` it downloads the rest unless
/// creation is [offline](Self::offline).
///
/// The returned Pack represents the exact values acquired. Project files
/// are not reread solely to detect concurrent source mutation.
pub struct Packer {
    root: PathBuf,
    entrypoint: PathBuf,
    vendor_packages: bool,
    embed_fonts: bool,
    include_typst_embedded_fonts: bool,
    typst_embedded_fonts: bool,
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    inputs: Dict,
    features: Vec<Feature>,
    target: TypstTarget,
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
    offline: bool,
    #[cfg(feature = "egress")]
    certificate: Option<PathBuf>,
    creation_timestamp: Option<i64>,
    timings: Option<PathBuf>,
    metadata: Option<PackMetadata>,
    #[cfg(test)]
    after_creation_hook: Option<Box<dyn Fn()>>,
}

impl Packer {
    /// Creates a packer for the project in `root` with the given entrypoint
    /// (absolute, or relative to `root`).
    pub fn new(root: impl Into<PathBuf>, entrypoint: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            entrypoint: entrypoint.into(),
            vendor_packages: true,
            embed_fonts: false,
            include_typst_embedded_fonts: false,
            typst_embedded_fonts: true,
            font_paths: Vec::new(),
            system_fonts: true,
            inputs: Dict::new(),
            features: Vec::new(),
            target: TypstTarget::Paged,
            package_path: None,
            package_cache_path: None,
            offline: false,
            #[cfg(feature = "egress")]
            certificate: None,
            creation_timestamp: None,
            timings: None,
            metadata: None,
            #[cfg(test)]
            after_creation_hook: None,
        }
    }

    /// Whether to store the files of all observed package dependencies inside
    /// the pack. Defaults to `true`; when disabled, dependencies are recorded
    /// as unvendored and must be resolvable when the pack is compiled.
    pub fn vendor_packages(mut self, vendor: bool) -> Self {
        self.vendor_packages = vendor;
        self
    }

    /// Whether to embed the fonts used by the document. Defaults to `false`.
    ///
    /// Note that font licenses differ; make sure you may redistribute the
    /// fonts you embed.
    pub fn embed_fonts(mut self, embed: bool) -> Self {
        self.embed_fonts = embed;
        self
    }

    /// Whether font embedding also stores the containers Typst embeds.
    /// Defaults to `false`; consumers then need the `embedded-fonts` feature
    /// or another source for those containers.
    ///
    /// This follows where a container came from, not what its bytes are: a
    /// scanned directory holding a copy of one of Typst's containers is
    /// embedded like any other scanned container.
    pub fn include_typst_embedded_fonts(mut self, include: bool) -> Self {
        self.include_typst_embedded_fonts = include;
        self
    }

    /// Whether creation may use fonts embedded into Typst. Defaults to `true`.
    pub fn typst_embedded_fonts(mut self, include: bool) -> Self {
        self.typst_embedded_fonts = include;
        self
    }

    /// Adds a directory to scan for fonts during creation.
    pub fn font_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.font_paths.push(path.into());
        self
    }

    /// Whether the creation compile may use system fonts. Defaults to
    /// `true`.
    pub fn system_fonts(mut self, system: bool) -> Self {
        self.system_fonts = system;
        self
    }

    /// Values made available to document code as `sys.inputs` during the
    /// creation compile.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables an experimental Typst language feature during creation.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Selects the target for the representative creation compilation.
    pub fn target(mut self, target: TypstTarget) -> Self {
        self.target = target;
        self
    }

    /// Overrides the directory in which locally installed packages are
    /// searched (namespace/name/version layout).
    pub fn package_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.package_path = Some(path.into());
        self
    }

    /// Overrides the directory in which downloaded packages are cached.
    ///
    /// This configures both cache reads and the destination of successful
    /// downloads. A build without egress can still read the selected cache.
    pub fn package_cache_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.package_cache_path = Some(path.into());
        self
    }

    /// Disallows network access during creation. Defaults to
    /// `false`.
    ///
    /// When enabled, package dependencies must already exist in the local
    /// package directories or package cache; anything that would need to be
    /// downloaded fails the compile as not found. A build without the `egress`
    /// feature behaves this way regardless, having no transport to reach the
    /// network with, so setting this stays available either way.
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Configures a custom CA certificate for package downloads.
    ///
    /// Only a download presents a certificate to verify, so this needs the
    /// `egress` feature.
    #[cfg(feature = "egress")]
    pub fn certificate(mut self, path: Option<PathBuf>) -> Self {
        self.certificate = path;
        self
    }

    /// Uses a fixed UNIX timestamp during creation.
    pub fn creation_timestamp(mut self, timestamp: Option<i64>) -> Self {
        self.creation_timestamp = timestamp;
        self
    }

    /// Writes creation performance timings to a Perfetto-compatible JSON file.
    pub fn timings(mut self, path: Option<PathBuf>) -> Self {
        self.timings = path;
        self
    }

    /// Sets descriptive metadata recorded in the pack manifest.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[cfg(test)]
    pub(crate) fn after_creation_hook(mut self, hook: impl Fn() + 'static) -> Self {
        self.after_creation_hook = Some(Box::new(hook));
        self
    }

    /// The Package Authority this creation resolves reported specifications
    /// through, which is as far as the host's capabilities reach.
    fn package_authority(&self) -> FilesystemPackageAuthority {
        let authority = FilesystemPackageAuthority::new(
            self.package_path.as_deref(),
            self.package_cache_path.as_deref(),
            self.offline,
        );
        #[cfg(feature = "egress")]
        let authority = authority.certificate(self.certificate.clone());
        authority
    }

    /// Snapshots the project, runs the representative compile, and assembles the Pack.
    pub fn pack(self) -> Result<PackOutcome, PackerError> {
        let (result, timing_error) = self.pack_with_timing();
        timing_error.map_or(result, Err)
    }

    #[doc(hidden)]
    pub fn pack_with_timing(self) -> (Result<PackOutcome, PackerError>, Option<PackerError>) {
        let mut timing_error = None;
        let result = self.pack_inner(&mut timing_error);
        (result, timing_error)
    }

    fn pack_inner(
        self,
        timing_error: &mut Option<PackerError>,
    ) -> Result<PackOutcome, PackerError> {
        let root = self
            .root
            .canonicalize()
            .map_err(|err| PackerError::io("failed to resolve project root", err))?;
        let entrypoint_abs = if self.entrypoint.is_absolute() {
            self.entrypoint.clone()
        } else {
            root.join(&self.entrypoint)
        };
        let entrypoint_abs = entrypoint_abs
            .canonicalize()
            .map_err(|err| PackerError::io("failed to resolve entrypoint", err))?;
        let entrypoint = VirtualPath::virtualize(&root, &entrypoint_abs)
            .map_err(|_| PackerError::OutsideRoot(entrypoint_abs.clone()))?;
        let snapshot = Arc::new(fs_project::gather_filesystem_project(
            &root,
            entrypoint.get_without_slash(),
            fs_project::FilesystemProjectLimits::reference_v1(),
        )?);

        let authority = self.package_authority();
        let packages = Arc::new(AcquiredPackages::new());

        let font_sources = FontSources {
            system: self.system_fonts,
            typst_embedded: self.typst_embedded_fonts,
            paths: &self.font_paths,
            embed: self.embed_fonts,
            include_typst_embedded: self.include_typst_embedded_fonts,
        };
        let font_catalog = font_sources.compose();

        // The core consults no wall clock, so the adapter resolves the
        // representative request's Document Time from the host's.
        let creation_timestamp = self.creation_timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs() as i64)
        });

        let mut request =
            CreationRequest::new(ProjectSnapshot::clone(&snapshot), creation_timestamp)
                .font_catalog(font_catalog.clone())
                .target(self.target)
                .inputs(self.inputs.clone());
        for feature in &self.features {
            request = request.feature(*feature);
        }
        if let Some(metadata) = self.metadata.clone() {
            request = request.metadata(metadata);
        }

        let mut world = AcquiredWorld {
            root: root.clone(),
            #[cfg(feature = "diagnostics")]
            workdir: std::env::current_dir()
                .ok()
                .map(|path| path.canonicalize().unwrap_or(path)),
            library: LazyHash::new(Library::builder().build()),
            main: RootedPath::new(VirtualRoot::Project, entrypoint).intern(),
            files: FileStore::new(AcquiredLoader {
                project: Arc::clone(&snapshot),
                packages: Arc::clone(&packages),
            }),
            fonts: FontStore::new(),
        };

        let disposition = if self.vendor_packages {
            PackageDisposition::Embedded
        } else {
            PackageDisposition::External
        };
        let mut timer = typst_kit::timer::Timer::new_or_placeholder(self.timings);
        let mut creation = None;
        let timings = timer.record(&mut world, |_| {
            creation = Some(resolve_and_create(
                request,
                &authority,
                &packages,
                disposition,
            ));
        });
        let Some(creation) = creation else {
            return Err(PackerError::Timings(
                timings
                    .expect_err("timer did not execute creation")
                    .to_string(),
            ));
        };
        *timing_error = timings
            .err()
            .map(|error| PackerError::Timings(error.to_string()));
        let issued = match creation {
            Ok(issued) => issued,
            Err(error) => return Err(error.into_packer_error(world)),
        };

        #[cfg(test)]
        if let Some(hook) = &self.after_creation_hook {
            hook();
        }

        font_sources.revalidate(&font_catalog)?;

        Ok(PackOutcome {
            pack: issued.pack,
            warnings: issued.warnings,
            #[cfg(feature = "diagnostics")]
            world,
        })
    }
}

/// Runs creation over the acquired inputs, resolving what it reports as
/// missing, until it issues a Pack.
///
/// Package requirements can only be discovered by compiling, so each round
/// reports the exact specifications no supplied tree covers, the adapter
/// obtains them through the Package Authority, and creation runs again over
/// the larger set. Every round therefore either issues a Pack, adds a tree the
/// request did not have, declares one the Package Authority could not resolve,
/// or fails.
///
/// A specification the authority cannot resolve is declared rather than
/// returned. The next round's representative request then fails at the import
/// that needed it, carrying the authority's own reason, so an unresolvable
/// package is reported where the document asked for it exactly as it was
/// before package resolution moved out of the representative compile.
fn resolve_and_create(
    mut request: CreationRequest,
    authority: &FilesystemPackageAuthority,
    packages: &AcquiredPackages,
    disposition: PackageDisposition,
) -> Result<IssuedPack, CreationFailure> {
    let mut acquired: HashSet<String> = HashSet::new();
    let mut package_failures = Vec::new();
    let mut catalog = PackageCatalog::new();
    loop {
        let outcome = create(&request).map_err(|error| CreationFailure::Core {
            error,
            package_failures: std::mem::take(&mut package_failures),
        })?;
        match outcome {
            CreationOutcome::Issued(issued) => return Ok(*issued),
            CreationOutcome::MissingPackages(missing) => {
                for spec in missing {
                    if !acquired.insert(spec.to_string()) {
                        // Creation reports neither what a supplied tree covers
                        // nor what was declared unresolvable, so this cannot
                        // repeat; failing keeps that a diagnosis rather than a
                        // loop that never progresses.
                        return Err(CreationFailure::Adapter(PackerError::Package {
                            message: "the representative creation compile did not accept the \
                                      resolved package tree"
                                .to_owned(),
                            spec,
                        }));
                    }
                    request = match authority.acquire(&spec) {
                        Ok(acquired) => {
                            let (tree, _) = acquired.into_parts();
                            packages.record(spec.clone(), tree.clone());
                            catalog
                                .insert(spec.clone(), tree, disposition)
                                .map_err(PackerError::InvalidPackageCatalog)?;
                            request.package_catalog(catalog.clone())
                        }
                        Err(error) => {
                            let failure = package_failure_for_creation(error.failure());
                            package_failures.push(error);
                            request.unresolvable_package(spec, failure)
                        }
                    };
                }
            }
        }
    }
}

/// Transitional bridge until Pack Creation consumes Package Acquisition
/// Failures directly rather than Typst's package error vocabulary.
fn package_failure_for_creation(failure: &PackageAcquisitionFailure) -> PackageError {
    let spec = failure.spec().clone();
    match failure.reason() {
        PackageAcquisitionFailureReason::NotFound => PackageError::NotFound(spec),
        PackageAcquisitionFailureReason::VersionNotFound { latest } => {
            PackageError::VersionNotFound(spec, *latest)
        }
        PackageAcquisitionFailureReason::NetworkFailed { detail } => {
            PackageError::NetworkFailed(detail.clone().map(Into::into))
        }
        PackageAcquisitionFailureReason::MalformedArchive { detail } => {
            PackageError::MalformedArchive(detail.clone().map(Into::into))
        }
        PackageAcquisitionFailureReason::Other { detail } => {
            PackageError::Other(detail.clone().map(Into::into))
        }
    }
}

/// A failure that ended one creation loop, before the adapter dressed it in
/// its own vocabulary.
enum CreationFailure {
    /// The core issued no Pack.
    Core {
        error: CreationError,
        package_failures: Vec<FilesystemPackageAcquisitionError>,
    },
    /// The adapter failed to acquire what the core reported as missing.
    Adapter(PackerError),
}

impl CreationFailure {
    /// Reports the failure in the filesystem adapter's vocabulary, handing a
    /// failed representative compile the sources that render its diagnostics.
    fn into_packer_error(self, world: AcquiredWorld) -> PackerError {
        match self {
            Self::Adapter(error) => error,
            Self::Core {
                error: CreationError::Compile { errors, warnings },
                package_failures,
            } => PackerError::Compile {
                world: Box::new(CreationDiagnosticContext { world }),
                errors,
                warnings,
                package_failures,
            },
            Self::Core {
                error: CreationError::InvalidTimestamp(message),
                ..
            } => PackerError::InvalidTimestamp(message),
            Self::Core {
                error: CreationError::Build(error),
                ..
            } => PackerError::Build(error),
        }
    }
}

impl From<PackerError> for CreationFailure {
    fn from(error: PackerError) -> Self {
        Self::Adapter(error)
    }
}

/// The result of a successful [`Packer::pack`] run.
pub struct PackOutcome {
    /// The assembled pack.
    pub pack: Pack,
    /// Warnings emitted by the representative creation compile.
    pub warnings: EcoVec<SourceDiagnostic>,
    #[cfg(feature = "diagnostics")]
    pub(crate) world: AcquiredWorld,
}

/// Opaque source context retained for first-party creation diagnostics.
///
/// This value intentionally does not implement Typst's [`World`] interface.
#[derive(Debug)]
pub struct CreationDiagnosticContext {
    #[cfg_attr(not(feature = "diagnostics"), allow(dead_code))]
    pub(crate) world: AcquiredWorld,
}

/// A failure while packing a project directory.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackerError {
    #[error("{message}: {source}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{0}` is outside the project root and cannot be packed")]
    OutsideRoot(PathBuf),
    #[error("the representative creation compile failed with {} error(s)", errors.len())]
    Compile {
        /// Opaque source context retained for first-party diagnostic rendering.
        world: Box<CreationDiagnosticContext>,
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
        /// Typed Package Authority failures attached to this representative
        /// compile through the transitional creation bridge.
        package_failures: Vec<FilesystemPackageAcquisitionError>,
    },
    #[error("invalid creation timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("failed to write creation timings: {0}")]
    Timings(String),
    #[error("failed to load package {spec}: {message}")]
    Package { spec: PackageSpec, message: String },
    /// The acquired Package Trees do not form a valid Package Catalog.
    #[error(transparent)]
    InvalidPackageCatalog(PackageCatalogError),
    #[error(transparent)]
    ProjectGather(#[from] fs_project::FilesystemProjectGatherError),
    #[error("creation evidence changed before Pack issuance: `{path}`")]
    CreationEvidenceChanged { path: String },
    #[error(transparent)]
    Build(#[from] PackBuildError),
}

impl PackerError {
    pub(crate) fn io(message: &str, source: std::io::Error) -> Self {
        Self::Io {
            message: message.to_owned(),
            source,
        }
    }
}

/// The bytes one creation acquired, as a world.
///
/// It compiles nothing: the representative request runs in the core, over the
/// same bytes. This world exists so that what the adapter acquired can still be
/// presented afterwards — creation diagnostics render their source context and
/// timing spans resolve their file and line from here, without reading the
/// project or a package tree a second time.
pub(crate) struct AcquiredWorld {
    root: PathBuf,
    #[cfg(feature = "diagnostics")]
    workdir: Option<PathBuf>,
    library: LazyHash<Library>,
    main: FileId,
    files: FileStore<AcquiredLoader>,
    fonts: FontStore,
}

impl AcquiredWorld {
    /// The canonicalized project root.
    #[cfg(feature = "diagnostics")]
    fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(feature = "diagnostics")]
    fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }
}

impl fmt::Debug for AcquiredWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcquiredWorld")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl World for AcquiredWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    /// No face at all: presentation resolves file requests, never fonts.
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

    fn font(&self, _index: usize) -> Option<Font> {
        None
    }

    /// Absent: the representative request's Document Time is the core's, and
    /// presentation evaluates no document code.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(feature = "diagnostics")]
impl typst_kit::diagnostics::DiagnosticWorld for AcquiredWorld {
    fn name(&self, id: FileId) -> String {
        match id.root() {
            VirtualRoot::Project => id
                .vpath()
                .realize(self.root())
                .ok()
                .and_then(|path| relative_path(&path, self.workdir()?))
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| display_file_id(id)),
            VirtualRoot::Package(_) => display_file_id(id),
        }
    }
}

#[cfg(feature = "diagnostics")]
fn display_file_id(id: FileId) -> String {
    match id.root() {
        VirtualRoot::Project => id.vpath().get_without_slash().to_owned(),
        VirtualRoot::Package(spec) => format!("{spec}{}", id.vpath().get_with_slash()),
    }
}

#[cfg(feature = "diagnostics")]
fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
    if path.is_absolute() != base.is_absolute() {
        return path.is_absolute().then(|| path.to_path_buf());
    }

    let mut path_components = path.components();
    let mut base_components = base.components();
    let mut relative = Vec::new();
    loop {
        match (path_components.next(), base_components.next()) {
            (None, None) => break,
            (Some(component), None) => {
                relative.push(component);
                relative.extend(path_components.by_ref());
                break;
            }
            (None, Some(_)) => relative.push(std::path::Component::ParentDir),
            (Some(path), Some(base)) if relative.is_empty() && path == base => {}
            (Some(path), Some(std::path::Component::CurDir)) => relative.push(path),
            (Some(_), Some(std::path::Component::ParentDir)) => return None,
            (Some(std::path::Component::Prefix(_) | std::path::Component::RootDir), Some(_))
            | (Some(_), Some(std::path::Component::Prefix(_) | std::path::Component::RootDir)) => {
                return path.is_absolute().then(|| path.to_path_buf());
            }
            (Some(path), Some(_)) => {
                relative.push(std::path::Component::ParentDir);
                relative.extend(base_components.map(|_| std::path::Component::ParentDir));
                relative.push(path);
                relative.extend(path_components.by_ref());
                break;
            }
        }
    }

    Some(relative.iter().map(|part| part.as_os_str()).collect())
}

/// Serves file requests from the bytes the adapter acquired, and from nothing
/// else: no request reaches the filesystem a second time.
struct AcquiredLoader {
    project: Arc<ProjectSnapshot>,
    packages: Arc<AcquiredPackages>,
}

impl FileLoader for AcquiredLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => self.project.shared_file(path).map(|data| data.to_typst()),
            VirtualRoot::Package(spec) => self.packages.file(spec, path),
        }
        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
    }
}

/// The font acquisition half of Pack Assembly for the filesystem adapter: the
/// ambient sources it acquires Font Containers from, and the
/// disposition each source's containers carry.
struct FontSources<'a> {
    system: bool,
    typst_embedded: bool,
    paths: &'a [PathBuf],
    embed: bool,
    include_typst_embedded: bool,
}

impl FontSources<'_> {
    /// Composes the Font Catalog: system fonts, then Typst's
    /// embedded fonts, then each scanned directory in the order it was added.
    fn compose(&self) -> FontCatalog {
        let mut catalog = FontCatalog::new();
        let scanned = FontDisposition::embedded_if(self.embed);
        if self.system {
            catalog.extend(read_containers(typst_kit::fonts::system(), scanned));
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded {
            let disposition =
                FontDisposition::embedded_if(self.embed && self.include_typst_embedded);
            catalog.extend(
                typst_embedded_font_containers()
                    .map(|container| FontCatalogEntry::new(container, disposition)),
            );
        }
        #[cfg(not(feature = "embedded-fonts"))]
        let _ = (self.typst_embedded, self.include_typst_embedded);
        for path in self.paths {
            catalog.extend(read_containers(typst_kit::fonts::scan(path), scanned));
        }
        catalog
    }

    /// Fails when the fonts backing `catalog` no longer agree with the
    /// filesystem, which is the font half of the Creation Evidence Fence.
    fn revalidate(&self, catalog: &FontCatalog) -> Result<(), PackerError> {
        if &self.compose() != catalog {
            return Err(PackerError::CreationEvidenceChanged {
                path: "font catalog".to_owned(),
            });
        }
        Ok(())
    }
}

/// Reads the containers behind scanned faces, one container per font file, in
/// the order the scan first reported them.
///
/// A listed file that cannot be read offers no candidate face at all, so
/// selection never reaches a container the adapter cannot supply.
fn read_containers(
    faces: impl Iterator<Item = (FontPath, FontInfo)>,
    disposition: FontDisposition,
) -> Vec<FontCatalogEntry> {
    let mut seen = HashSet::new();
    let mut containers = Vec::new();
    for (source, _) in faces {
        if !seen.insert(source.path.clone()) {
            continue;
        }
        let Ok(data) = std::fs::read(&source.path) else {
            continue;
        };
        if let Ok(container) = FontContainer::new(data) {
            containers.push(FontCatalogEntry::new(container, disposition));
        }
    }
    containers
}
