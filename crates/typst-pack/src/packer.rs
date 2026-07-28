//! Packing the structural closure of a project directory.

#![cfg(feature = "fs")]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
#[cfg(feature = "diagnostics")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use ecow::EcoVec;
use typst::diag::{FileError, FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::FontPath;

use crate::compile::TypstTarget;
use crate::creation::compile_creation_target;
#[cfg(feature = "embedded-fonts")]
use crate::font_catalog::typst_embedded_font_containers;
use crate::font_catalog::{
    CandidateFontCatalog, CandidateFontContainer, CandidateFonts, FontDisposition,
};
use crate::fs_project;
use crate::ignore_policy::ProjectIgnorePolicyError;
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError};
use crate::project_snapshot::{ProjectSnapshot, ProjectSnapshotError};
use crate::world::system_packages;

type PackageEvidence = (PackageSpec, PathBuf, Vec<(String, Bytes)>);

/// Packs a Typst project directory into a [`Pack`].
///
/// The packer snapshots every eligible regular file beneath the project root,
/// then performs one representative compile to select package and font
/// dependencies. Compiler observations never select project files.
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
    pub fn package_cache_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.package_cache_path = Some(path.into());
        self
    }

    /// Disallows network access during creation. Defaults to
    /// `false`.
    ///
    /// When enabled, package dependencies must already exist in the local
    /// package directories; anything that would need to be downloaded fails
    /// the compile as not found.
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Configures a custom CA certificate for package downloads.
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
        let snapshot = Arc::new(fs_project::acquire_snapshot(
            &root,
            entrypoint.get_without_slash(),
        )?);
        let mut builder = Pack::builder(snapshot.entrypoint());

        let packages = system_packages(
            self.package_path.as_deref(),
            self.package_cache_path.as_deref(),
            self.offline,
            self.certificate.as_deref(),
        );

        let font_sources = FontSources {
            system: self.system_fonts,
            typst_embedded: self.typst_embedded_fonts,
            paths: &self.font_paths,
            embed: self.embed_fonts,
            include_typst_embedded: self.include_typst_embedded_fonts,
        };
        let font_catalog = font_sources.compose();
        let fonts = font_catalog.expand();

        let primary = Arc::new(PrimaryLoader {
            system: SystemFiles::new(FsRoot::new(root.clone()), packages),
            project: Arc::clone(&snapshot),
            cache: Mutex::new(HashMap::new()),
        });
        let creation_timestamp = self.creation_timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs() as i64)
        });
        let time = Time::fixed_timestamp(creation_timestamp)
            .map_err(|error| PackerError::InvalidTimestamp(error.to_string()))?;
        let mut world = CreationWorld {
            root: root.clone(),
            #[cfg(feature = "diagnostics")]
            workdir: std::env::current_dir()
                .ok()
                .map(|path| path.canonicalize().unwrap_or(path)),
            library: LazyHash::new(
                Library::builder()
                    .with_inputs(self.inputs.clone())
                    .with_features(self.features.iter().copied().collect())
                    .build(),
            ),
            main: RootedPath::new(VirtualRoot::Project, entrypoint.clone()).intern(),
            sources: FileStore::new(Arc::clone(&primary)),
            files: FileStore::new(primary),
            fonts,
            used_font_indices: Mutex::new(BTreeSet::new()),
            time,
        };

        let target = self.target;
        let mut timer = typst_kit::timer::Timer::new_or_placeholder(self.timings);
        let mut compilation = None;
        let timings = timer.record(&mut world, |world| {
            compilation = Some(compile_creation_target(world, target));
        });
        let Some(Warned { output, warnings }) = compilation else {
            return Err(PackerError::Timings(
                timings
                    .expect_err("timer did not execute creation compilation")
                    .to_string(),
            ));
        };
        *timing_error = timings
            .err()
            .map(|error| PackerError::Timings(error.to_string()));
        if let Err(errors) = output {
            return creation_compile_error(world, errors, warnings);
        }
        #[cfg(test)]
        if let Some(hook) = &self.after_creation_hook {
            hook();
        }

        let mut package_evidence = Vec::new();
        let mut package_files: BTreeMap<String, (PackageSpec, FileId)> = BTreeMap::new();
        let (source_dependencies, file_dependencies, _) = world.take_dependency_observations();
        for id in source_dependencies.into_iter().chain(file_dependencies) {
            if let VirtualRoot::Package(spec) = id.root() {
                package_files
                    .entry(spec.to_string())
                    .or_insert_with(|| (spec.clone(), id));
            }
        }

        for (path, data) in snapshot.files() {
            builder = builder.file(path, data.to_vec())?;
        }

        // Packages.
        for (spec, id) in package_files.values() {
            let package_root =
                world
                    .files
                    .loader()
                    .root(*id)
                    .map_err(|err| PackerError::Package {
                        spec: spec.clone(),
                        message: err.to_string(),
                    })?;
            let tree = crate::world::read_complete_package_tree(package_root.path()).map_err(
                |message| PackerError::Package {
                    spec: spec.clone(),
                    message,
                },
            )?;
            package_evidence.push((spec.clone(), package_root.path().to_owned(), tree.clone()));
            for (path, data) in tree {
                builder = if self.vendor_packages {
                    builder.package_file(spec.clone(), path, data.to_vec())?
                } else {
                    builder.external_package_file(spec.clone(), path, data.to_vec())?
                };
            }
        }

        // Project selected faces back into the original candidate catalog
        // order, each under the disposition its container carries.
        for (font, disposition) in world.used_fonts() {
            builder = if disposition.is_embedded() {
                builder.font(font.data().to_vec(), font.index())?
            } else {
                builder.external_font(font.data().to_vec(), font.index())?
            };
        }

        if let Some(metadata) = self.metadata {
            builder = builder.metadata(metadata);
        }

        fs_project::revalidate(&snapshot, &root)?;
        revalidate_package_evidence(&package_evidence)?;
        font_sources.revalidate(&font_catalog)?;
        let pack = builder.build()?;

        Ok(PackOutcome {
            pack,
            warnings,
            #[cfg(feature = "diagnostics")]
            world,
        })
    }
}

fn creation_compile_error(
    world: CreationWorld,
    errors: EcoVec<SourceDiagnostic>,
    warnings: EcoVec<SourceDiagnostic>,
) -> Result<PackOutcome, PackerError> {
    Err(PackerError::Compile {
        world: Box::new(CreationDiagnosticContext { world }),
        errors,
        warnings,
    })
}

/// The result of a successful [`Packer::pack`] run.
pub struct PackOutcome {
    /// The assembled pack.
    pub pack: Pack,
    /// Warnings emitted by the representative creation compile.
    pub warnings: EcoVec<SourceDiagnostic>,
    #[cfg(feature = "diagnostics")]
    pub(crate) world: CreationWorld,
}

/// Opaque source context retained for first-party creation diagnostics.
///
/// This value intentionally does not implement Typst's [`World`] interface.
#[derive(Debug)]
pub struct CreationDiagnosticContext {
    #[cfg_attr(not(feature = "diagnostics"), allow(dead_code))]
    pub(crate) world: CreationWorld,
}

/// A failure while packing a project directory.
#[derive(Debug, thiserror::Error)]
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
    },
    #[error("invalid creation timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("failed to write creation timings: {0}")]
    Timings(String),
    #[error("failed to load package {spec}: {message}")]
    Package { spec: PackageSpec, message: String },
    #[error("failed to walk directory: {0}")]
    Walk(String),
    #[error(transparent)]
    InvalidIgnorePolicy(#[from] ProjectIgnorePolicyError),
    #[error("project path `{path}` cannot be represented: {message}")]
    InvalidProjectPath { path: String, message: String },
    #[error("entrypoint `{0}` is excluded by the Project Ignore Policy")]
    IgnoredEntrypoint(String),
    /// A Project Snapshot assembly failure with no filesystem vocabulary of
    /// its own.
    #[error(transparent)]
    Snapshot(ProjectSnapshotError),
    #[error("project path `{}` is not valid UTF-8", path.display())]
    UnrepresentablePath { path: PathBuf },
    #[error("unsupported filesystem entry `{}` in the project", path.display())]
    UnsupportedProjectEntry { path: PathBuf },
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

impl From<ProjectSnapshotError> for PackerError {
    /// Reports snapshot assembly in the filesystem adapter's own vocabulary.
    fn from(error: ProjectSnapshotError) -> Self {
        match error {
            ProjectSnapshotError::InvalidPath { path, message } => {
                Self::InvalidProjectPath { path, message }
            }
            // A walked entrypoint that no longer survives assembly was either
            // excluded by the policy or is not a regular file; the adapter has
            // reported both as an excluded entrypoint since it walked trees.
            ProjectSnapshotError::ExcludedEntrypoint(path)
            | ProjectSnapshotError::MissingEntrypoint(path) => Self::IgnoredEntrypoint(path),
            error @ (ProjectSnapshotError::DuplicatePath { .. }
            | ProjectSnapshotError::FileCountExceeded { .. }
            | ProjectSnapshotError::ByteSizeExceeded { .. }) => Self::Snapshot(error),
        }
    }
}

/// The private snapshot-backed world used for creation compilation.
pub(crate) struct CreationWorld {
    root: PathBuf,
    #[cfg(feature = "diagnostics")]
    workdir: Option<PathBuf>,
    library: LazyHash<Library>,
    main: FileId,
    sources: FileStore<Arc<PrimaryLoader>>,
    files: FileStore<Arc<PrimaryLoader>>,
    fonts: CandidateFonts,
    used_font_indices: Mutex<BTreeSet<usize>>,
    time: Time,
}

impl CreationWorld {
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
    /// The canonicalized project root.
    #[cfg(feature = "diagnostics")]
    fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(feature = "diagnostics")]
    fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    fn take_dependency_observations(&mut self) -> (Vec<FileId>, Vec<FileId>, BTreeSet<usize>) {
        let (_, sources) = self.sources.dependencies();
        let sources = sources.collect();
        let (_, files) = self.files.dependencies();
        let files = files.collect();
        let fonts = self
            .used_font_indices
            .lock()
            .expect("used font index lock poisoned")
            .clone();
        (sources, files, fonts)
    }
}

impl fmt::Debug for CreationWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreationWorld")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl World for CreationWorld {
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
        self.sources.source(id)
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

#[cfg(feature = "diagnostics")]
impl typst_kit::diagnostics::DiagnosticWorld for CreationWorld {
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

struct PrimaryLoader {
    system: SystemFiles,
    project: Arc<ProjectSnapshot>,
    cache: Mutex<HashMap<FileId, Arc<OnceLock<FileResult<Bytes>>>>>,
}

impl PrimaryLoader {
    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        self.system.root(id)
    }
}

impl FileLoader for PrimaryLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        if matches!(id.root(), VirtualRoot::Project) {
            let path = id.vpath().get_without_slash();
            return self
                .project
                .file(path)
                .cloned()
                .ok_or_else(|| FileError::NotFound(PathBuf::from(path)));
        }
        let entry = {
            let mut cache = self.cache.lock().expect("primary file cache lock poisoned");
            Arc::clone(cache.entry(id).or_default())
        };
        entry.get_or_init(|| self.system.load(id)).clone()
    }
}

fn revalidate_package_evidence(packages: &[PackageEvidence]) -> Result<(), PackerError> {
    for (spec, root, expected) in packages {
        if crate::world::read_complete_package_tree(root).as_ref().ok() != Some(expected) {
            return Err(PackerError::CreationEvidenceChanged {
                path: spec.to_string(),
            });
        }
    }
    Ok(())
}

/// The font half of Creation Preparation for the filesystem adapter: the
/// ambient sources it acquires candidate Font Containers from, and the
/// disposition each source's containers carry.
struct FontSources<'a> {
    system: bool,
    typst_embedded: bool,
    paths: &'a [PathBuf],
    embed: bool,
    include_typst_embedded: bool,
}

impl FontSources<'_> {
    /// Composes the candidate font catalog: system fonts, then Typst's
    /// embedded fonts, then each scanned directory in the order it was added.
    fn compose(&self) -> CandidateFontCatalog {
        let mut catalog = CandidateFontCatalog::new();
        let scanned = FontDisposition::embedded_if(self.embed);
        if self.system {
            catalog.extend(read_containers(typst_kit::fonts::system(), scanned));
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded {
            catalog.extend(typst_embedded_font_containers(
                FontDisposition::embedded_if(self.embed && self.include_typst_embedded),
            ));
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
    fn revalidate(&self, catalog: &CandidateFontCatalog) -> Result<(), PackerError> {
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
) -> Vec<CandidateFontContainer> {
    let mut seen = HashSet::new();
    let mut containers = Vec::new();
    for (source, _) in faces {
        if !seen.insert(source.path.clone()) {
            continue;
        }
        let Ok(data) = std::fs::read(&source.path) else {
            continue;
        };
        containers.push(CandidateFontContainer::new(data, disposition));
    }
    containers
}
