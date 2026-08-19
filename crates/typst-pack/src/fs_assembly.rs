//! The reference filesystem Pack Assembler.
//!
//! The adapter acquires and the core transforms. It lists and reads the
//! project, composes the Font Catalog out of the font sources the
//! host offers, obtains the Package Trees the core reports as
//! missing, and resolves Document Time; Pack Creation itself runs in
//! the core over those bytes.
//!
//! Each acquired value records the exact bytes observed by its source adapter;
//! the project gatherer does not reread the source solely to establish that all
//! values coexisted at one instant.

#![cfg(feature = "fs")]

use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use ecow::EcoVec;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::FontStore;

use crate::compile::{DocumentTime, TypstTarget};
use crate::creation::{
    DiscoverySpecification, PackCreationError, PackCreationInput, PackCreationOutcome, create,
};
use crate::font_catalog::FontDisposition;
use crate::fs_fonts::{FilesystemFontLimits, FilesystemFontSource, gather_filesystem_font_catalog};
use crate::fs_packages::{
    AcquiredPackages, FilesystemPackageAcquisitionError, FilesystemPackageAuthority,
    FilesystemPackageLimits,
};
use crate::fs_project;
use crate::manifest::PackMetadata;
use crate::pack::Pack;
use crate::package_catalog::{PackageCatalog, PackageCatalogError, PackageDisposition};
use crate::package_failure::PackageAcquisitionFailures;
use crate::project_snapshot::ProjectSnapshot;

/// Named finite resource policy for one filesystem Pack Assembly run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemPackAssemblyProfile {
    project: fs_project::FilesystemProjectLimits,
    packages: FilesystemPackageLimits,
    fonts: FilesystemFontLimits,
    #[cfg(feature = "egress")]
    package_expansion: crate::PackageExpansionLimits,
}

impl FilesystemPackAssemblyProfile {
    /// The first-party finite profile used by ordinary filesystem workflows.
    pub const fn reference_v1() -> Self {
        Self {
            project: fs_project::FilesystemProjectLimits::reference_v1(),
            packages: FilesystemPackageLimits::reference_v1(),
            fonts: FilesystemFontLimits::reference_v1(),
            #[cfg(feature = "egress")]
            package_expansion: crate::PackageExpansionLimits::reference_v1(),
        }
    }

    pub const fn project(&self) -> fs_project::FilesystemProjectLimits {
        self.project
    }

    pub const fn packages(&self) -> FilesystemPackageLimits {
        self.packages
    }

    pub const fn fonts(&self) -> FilesystemFontLimits {
        self.fonts
    }

    #[cfg(feature = "egress")]
    pub const fn package_expansion(&self) -> crate::PackageExpansionLimits {
        self.package_expansion
    }
}

/// Clock policy used when a run does not supply an exact Document Time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum FilesystemPackAssemblyClock {
    /// Resolve the current UNIX timestamp from the host clock for each run.
    #[default]
    System,
    /// Reuse one exact configured Document Time for every run.
    Fixed(DocumentTime),
}

/// Reusable host policy for the reference filesystem Pack Assembler.
#[derive(Debug)]
pub struct FilesystemPackAssemblerConfig {
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    typst_embedded_fonts: bool,
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
    offline: bool,
    #[cfg(feature = "egress")]
    certificate: Option<PathBuf>,
    clock: FilesystemPackAssemblyClock,
    profile: FilesystemPackAssemblyProfile,
}

impl FilesystemPackAssemblerConfig {
    /// Starts with ordinary first-party host policy and the reference-v1
    /// finite profile.
    pub fn new() -> Self {
        Self {
            font_paths: Vec::new(),
            system_fonts: true,
            typst_embedded_fonts: true,
            package_path: None,
            package_cache_path: None,
            offline: false,
            #[cfg(feature = "egress")]
            certificate: None,
            clock: FilesystemPackAssemblyClock::System,
            profile: FilesystemPackAssemblyProfile::reference_v1(),
        }
    }

    /// Selects the finite resource policy applied to every run.
    pub fn profile(mut self, profile: FilesystemPackAssemblyProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Selects how omitted per-run Document Time is resolved.
    pub fn clock(mut self, clock: FilesystemPackAssemblyClock) -> Self {
        self.clock = clock;
        self
    }

    /// Adds a directory to the configured Font Authority.
    pub fn font_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.font_paths.push(path.into());
        self
    }

    /// Whether the configured Font Authority scans host system fonts.
    pub fn system_fonts(mut self, system: bool) -> Self {
        self.system_fonts = system;
        self
    }

    /// Whether the configured Font Authority offers Typst's embedded fonts.
    pub fn typst_embedded_fonts(mut self, include: bool) -> Self {
        self.typst_embedded_fonts = include;
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
}

impl Default for FilesystemPackAssemblerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-run roots, Discovery Specification controls, embedding choices, and
/// Pack metadata.
pub struct FilesystemPackAssemblyRequest<'a> {
    root: &'a Path,
    entrypoint: &'a Path,
    vendor_packages: bool,
    embed_fonts: bool,
    include_typst_embedded_fonts: bool,
    inputs: Dict,
    features: Vec<Feature>,
    target: TypstTarget,
    document_time: Option<DocumentTime>,
    timings: Option<PathBuf>,
    metadata: Option<PackMetadata>,
}

impl<'a> FilesystemPackAssemblyRequest<'a> {
    /// Starts one run for an entrypoint that is absolute or relative to `root`.
    pub fn new(root: &'a Path, entrypoint: &'a Path) -> Self {
        Self {
            root,
            entrypoint,
            vendor_packages: true,
            embed_fonts: false,
            include_typst_embedded_fonts: false,
            inputs: Dict::new(),
            features: Vec::new(),
            target: TypstTarget::Paged,
            document_time: None,
            timings: None,
            metadata: None,
        }
    }

    /// Whether selected Package Trees are embedded in the Pack.
    pub fn vendor_packages(mut self, vendor: bool) -> Self {
        self.vendor_packages = vendor;
        self
    }

    /// Whether selected scanned and system Font Containers are embedded.
    pub fn embed_fonts(mut self, embed: bool) -> Self {
        self.embed_fonts = embed;
        self
    }

    /// Whether embedding includes selected Typst-embedded Font Containers.
    pub fn include_typst_embedded_fonts(mut self, include: bool) -> Self {
        self.include_typst_embedded_fonts = include;
        self
    }

    /// Values made available to document code as `sys.inputs` during discovery.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables one Typst engine feature during discovery.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Selects the Typst Target for discovery.
    pub fn target(mut self, target: TypstTarget) -> Self {
        self.target = target;
        self
    }

    /// Supplies an exact Document Time instead of resolving the host clock.
    pub fn document_time(mut self, document_time: DocumentTime) -> Self {
        self.document_time = Some(document_time);
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
}

/// Reusable filesystem Pack Assembly over configured concrete authorities.
pub struct FilesystemPackAssembler {
    authority: FilesystemPackageAuthority,
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    typst_embedded_fonts: bool,
    clock: FilesystemPackAssemblyClock,
    profile: FilesystemPackAssemblyProfile,
    #[cfg(test)]
    after_creation_hook: Option<Box<dyn Fn()>>,
}

impl FilesystemPackAssembler {
    /// Configures the concrete project, package, font, clock, and finite-profile
    /// policy reused by each assembly request.
    pub fn new(config: FilesystemPackAssemblerConfig) -> Self {
        let authority = FilesystemPackageAuthority::with_limits(
            config.package_path.as_deref(),
            config.package_cache_path.as_deref(),
            config.offline,
            config.profile.packages,
            #[cfg(feature = "egress")]
            config.profile.package_expansion,
        );
        #[cfg(feature = "egress")]
        let authority = authority.certificate(config.certificate);
        Self {
            authority,
            font_paths: config.font_paths,
            system_fonts: config.system_fonts,
            typst_embedded_fonts: config.typst_embedded_fonts,
            clock: config.clock,
            profile: config.profile,
            #[cfg(test)]
            after_creation_hook: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn after_creation_hook(mut self, hook: impl Fn() + 'static) -> Self {
        self.after_creation_hook = Some(Box::new(hook));
        self
    }

    /// Gathers one Project Snapshot and Font Catalog, then resolves exactly the
    /// packages reported between stateless Pack Creation invocations.
    pub fn assemble(
        &self,
        request: FilesystemPackAssemblyRequest<'_>,
    ) -> Result<PackAssemblyReport, FilesystemPackAssemblyError> {
        let (result, timing_error) = self.assemble_with_timing(request);
        timing_error.map_or(result, Err)
    }

    #[doc(hidden)]
    pub fn assemble_with_timing(
        &self,
        request: FilesystemPackAssemblyRequest<'_>,
    ) -> (
        Result<PackAssemblyReport, FilesystemPackAssemblyError>,
        Option<FilesystemPackAssemblyError>,
    ) {
        let mut timing_error = None;
        let result = self.assemble_inner(request, &mut timing_error);
        (result, timing_error)
    }

    fn assemble_inner(
        &self,
        request: FilesystemPackAssemblyRequest<'_>,
        timing_error: &mut Option<FilesystemPackAssemblyError>,
    ) -> Result<PackAssemblyReport, FilesystemPackAssemblyError> {
        let root = request.root.canonicalize().map_err(|err| {
            FilesystemPackAssemblyError::io("failed to resolve project root", err)
        })?;
        let entrypoint_abs = if request.entrypoint.is_absolute() {
            request.entrypoint.to_owned()
        } else {
            root.join(request.entrypoint)
        };
        let entrypoint_abs = entrypoint_abs
            .canonicalize()
            .map_err(|err| FilesystemPackAssemblyError::io("failed to resolve entrypoint", err))?;
        let entrypoint = VirtualPath::virtualize(&root, &entrypoint_abs)
            .map_err(|_| FilesystemPackAssemblyError::OutsideRoot(entrypoint_abs.clone()))?;
        let snapshot = Arc::new(fs_project::gather_filesystem_project(
            &root,
            entrypoint.get_without_slash(),
            self.profile.project,
        )?);

        let packages = Arc::new(AcquiredPackages::new());

        let scanned_disposition = FontDisposition::embedded_if(request.embed_fonts);
        let mut font_sources = Vec::new();
        if self.system_fonts {
            font_sources.push(FilesystemFontSource::system(scanned_disposition));
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded_fonts {
            font_sources.push(FilesystemFontSource::typst_embedded(
                FontDisposition::embedded_if(
                    request.embed_fonts && request.include_typst_embedded_fonts,
                ),
            ));
        }
        #[cfg(not(feature = "embedded-fonts"))]
        let _ = (
            self.typst_embedded_fonts,
            request.include_typst_embedded_fonts,
        );
        font_sources.extend(
            self.font_paths
                .iter()
                .map(|path| FilesystemFontSource::directory(path, scanned_disposition)),
        );
        let font_catalog = gather_filesystem_font_catalog(font_sources, self.profile.fonts)?;

        // The core consults no wall clock, so the adapter resolves the
        // representative request's Document Time from the host's.
        let document_time = request
            .document_time
            .unwrap_or_else(|| self.clock.document_time());

        let discovery = DiscoverySpecification::new(
            request.target,
            request.inputs,
            document_time,
            request.features,
        )
        .map_err(|source| {
            FilesystemPackAssemblyError::DiscoverySpecification(
                FilesystemPackAssemblyDiscoveryError { source },
            )
        })?;

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

        let disposition = if request.vendor_packages {
            PackageDisposition::Embedded
        } else {
            PackageDisposition::External
        };
        let mut timer = typst_kit::timer::Timer::new_or_placeholder(request.timings);
        let mut creation = None;
        let timings = timer.record(&mut world, |_| {
            creation = Some(resolve_and_create(
                &snapshot,
                &font_catalog,
                &discovery,
                request.metadata.as_ref(),
                &self.authority,
                &packages,
                disposition,
            ));
        });
        let Some(creation) = creation else {
            return Err(FilesystemPackAssemblyError::Timings(
                timings
                    .expect_err("timer did not execute creation")
                    .to_string(),
            ));
        };
        *timing_error = timings
            .err()
            .map(|error| FilesystemPackAssemblyError::Timings(error.to_string()));
        let (pack, warnings) = match creation {
            Ok(created) => created,
            Err(error) => return Err(error.into_assembly_error(world)),
        };

        #[cfg(test)]
        if let Some(hook) = &self.after_creation_hook {
            hook();
        }

        Ok(PackAssemblyReport {
            pack,
            warnings,
            #[cfg(feature = "diagnostics")]
            world,
        })
    }
}

impl FilesystemPackAssemblyClock {
    fn document_time(self) -> DocumentTime {
        match self {
            Self::System => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs() as i64);
                DocumentTime::UnixTimestamp(timestamp)
            }
            Self::Fixed(document_time) => document_time,
        }
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
    project: &ProjectSnapshot,
    fonts: &crate::font_catalog::FontCatalog,
    discovery: &DiscoverySpecification,
    metadata: Option<&PackMetadata>,
    authority: &FilesystemPackageAuthority,
    packages: &AcquiredPackages,
    disposition: PackageDisposition,
) -> Result<(Pack, EcoVec<SourceDiagnostic>), CreationFailure> {
    let mut attempted_specs: HashSet<String> = HashSet::new();
    let mut package_failures = Vec::new();
    let mut acquisition_failures = PackageAcquisitionFailures::new();
    let mut catalog = PackageCatalog::new();
    loop {
        let outcome = create(PackCreationInput {
            project,
            packages: &catalog,
            fonts,
            package_failures: &acquisition_failures,
            discovery,
            metadata,
        })
        .map_err(|error| CreationFailure::Core {
            error,
            package_failures: std::mem::take(&mut package_failures),
        })?;
        match outcome {
            PackCreationOutcome::Created { pack, warnings } => return Ok((pack, warnings)),
            PackCreationOutcome::MissingPackageSpecifications(missing) => {
                for spec in missing {
                    if !attempted_specs.insert(spec.to_string()) {
                        // Creation reports neither what a supplied tree covers
                        // nor what was declared unresolvable, so this cannot
                        // repeat; failing keeps that a diagnosis rather than a
                        // loop that never progresses.
                        return Err(CreationFailure::Adapter(
                            FilesystemPackAssemblyError::Package {
                                message: "the representative creation compile did not accept the \
                                      resolved package tree"
                                    .to_owned(),
                                spec,
                            },
                        ));
                    }
                    match authority.acquire(&spec) {
                        Ok(acquired) => {
                            let (tree, _) = acquired.into_parts();
                            packages.record(spec.clone(), tree.clone());
                            acquisition_failures.remove(&spec);
                            catalog
                                .insert(spec.clone(), tree, disposition)
                                .map_err(FilesystemPackAssemblyError::InvalidPackageCatalog)?;
                        }
                        Err(error) => {
                            acquisition_failures.insert(error.failure().clone());
                            package_failures.push(error);
                        }
                    }
                }
            }
        }
    }
}

/// A failure that ended one creation loop, before the adapter dressed it in
/// its own vocabulary.
enum CreationFailure {
    /// The core issued no Pack.
    Core {
        error: PackCreationError,
        package_failures: Vec<FilesystemPackageAcquisitionError>,
    },
    /// The adapter failed to acquire what the core reported as missing.
    Adapter(FilesystemPackAssemblyError),
}

impl CreationFailure {
    /// Reports the failure in the filesystem adapter's vocabulary, handing a
    /// failed representative compile the sources that render its diagnostics.
    fn into_assembly_error(self, world: AcquiredWorld) -> FilesystemPackAssemblyError {
        match self {
            Self::Adapter(error) => error,
            Self::Core {
                error,
                package_failures,
            } => FilesystemPackAssemblyError::Creation(FilesystemPackAssemblyCreationError {
                context: Box::new(PackAssemblyDiagnosticContext { world }),
                error,
                package_failures,
            }),
        }
    }
}

impl From<FilesystemPackAssemblyError> for CreationFailure {
    fn from(error: FilesystemPackAssemblyError) -> Self {
        Self::Adapter(error)
    }
}

/// The terminal report of a successful filesystem Pack Assembly run.
pub struct PackAssemblyReport {
    pack: Pack,
    warnings: EcoVec<SourceDiagnostic>,
    #[cfg(feature = "diagnostics")]
    pub(crate) world: AcquiredWorld,
}

impl PackAssemblyReport {
    /// The assembled, authoritatively validated Pack.
    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    /// Warnings emitted by the successful Dependency Discovery run.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }

    /// Recovers the Pack and discovery warnings.
    pub fn into_parts(self) -> (Pack, EcoVec<SourceDiagnostic>) {
        (self.pack, self.warnings)
    }
}

/// Opaque source context retained for first-party creation diagnostics.
///
/// This value intentionally does not implement Typst's [`World`] interface.
#[derive(Debug)]
pub struct PackAssemblyDiagnosticContext {
    #[cfg_attr(not(feature = "diagnostics"), allow(dead_code))]
    pub(crate) world: AcquiredWorld,
}

/// A Pack Creation failure retained by the filesystem Pack Assembler.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct FilesystemPackAssemblyCreationError {
    context: Box<PackAssemblyDiagnosticContext>,
    #[source]
    error: PackCreationError,
    package_failures: Vec<FilesystemPackageAcquisitionError>,
}

impl FilesystemPackAssemblyCreationError {
    /// Opaque source context for first-party diagnostic rendering.
    pub fn context(&self) -> &PackAssemblyDiagnosticContext {
        &self.context
    }

    /// The unchanged core Pack Creation failure.
    pub fn error(&self) -> &PackCreationError {
        &self.error
    }

    /// Package Authority failures from the same assembly attempt.
    pub fn package_failures(&self) -> &[FilesystemPackageAcquisitionError] {
        &self.package_failures
    }

    /// Recovers the diagnostic context, core error, and authority failures.
    pub fn into_parts(
        self,
    ) -> (
        Box<PackAssemblyDiagnosticContext>,
        PackCreationError,
        Vec<FilesystemPackageAcquisitionError>,
    ) {
        (self.context, self.error, self.package_failures)
    }
}

/// An invalid Discovery Specification retained by filesystem Pack Assembly.
#[derive(Debug, thiserror::Error)]
#[error("invalid Discovery Specification: {source}")]
pub struct FilesystemPackAssemblyDiscoveryError {
    #[source]
    source: crate::creation::DiscoverySpecificationError,
}

impl FilesystemPackAssemblyDiscoveryError {
    /// The unchanged Discovery Specification construction failure.
    pub fn source_error(&self) -> &crate::creation::DiscoverySpecificationError {
        &self.source
    }

    /// Recovers the Discovery Specification construction failure.
    pub fn into_source(self) -> crate::creation::DiscoverySpecificationError {
        self.source
    }
}

/// A failure while packing a project directory.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemPackAssemblyError {
    #[error("{message}: {source}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{0}` is outside the project root and cannot be packed")]
    OutsideRoot(PathBuf),
    #[error(transparent)]
    Creation(FilesystemPackAssemblyCreationError),
    #[error(transparent)]
    DiscoverySpecification(FilesystemPackAssemblyDiscoveryError),
    #[error("failed to write creation timings: {0}")]
    Timings(String),
    #[error("failed to load package {spec}: {message}")]
    Package { spec: PackageSpec, message: String },
    /// The acquired Package Trees do not form a valid Package Catalog.
    #[error(transparent)]
    InvalidPackageCatalog(PackageCatalogError),
    #[error(transparent)]
    ProjectGather(#[from] fs_project::FilesystemProjectGatherError),
    #[error(transparent)]
    FontGather(#[from] crate::fs_fonts::FilesystemFontGatherError),
}

impl FilesystemPackAssemblyError {
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
