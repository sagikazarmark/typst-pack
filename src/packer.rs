//! Packing a project directory by discovering what a compile actually uses.

#![cfg(feature = "fs")]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ecow::EcoVec;
use typst::diag::{FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::layout::{Frame, FrameItem};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_html::{HtmlDocument, HtmlNode};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError};
use crate::resource::{DiscoveryResources, Provider};
use crate::world::system_packages;

#[cfg(test)]
type DiscoveryHook = Box<dyn Fn(&mut DiscoveryWorld)>;

/// Packs a Typst project directory into a [`Pack`].
///
/// The packer performs a discovery compile of the project and records every
/// file Typst actually reads. Source-project files are packed by default.
/// Non-source resources resolved by configured Resource Providers are declared
/// as Resource Slots instead of storing their bytes. Files that a compile with
/// different inputs or a different date would read are not discovered; add
/// packed files with [`include`](Self::include) or declare a Resource Slot with
/// [`resource_slot`](Self::resource_slot).
pub struct Packer {
    root: PathBuf,
    entrypoint: PathBuf,
    vendor_packages: bool,
    embed_fonts: bool,
    include_typst_embedded_fonts: bool,
    typst_embedded_fonts: bool,
    include: Vec<PathBuf>,
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    inputs: Dict,
    features: Vec<Feature>,
    targets: BTreeSet<DiscoveryTarget>,
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
    offline: bool,
    certificate: Option<PathBuf>,
    creation_timestamp: Option<i64>,
    timings: Option<PathBuf>,
    metadata: Option<PackMetadata>,
    resource_slots: BTreeSet<String>,
    resource_providers: Vec<Provider>,
    #[cfg(test)]
    discovery_hook: Option<DiscoveryHook>,
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
            include: Vec::new(),
            font_paths: Vec::new(),
            system_fonts: true,
            inputs: Dict::new(),
            features: Vec::new(),
            targets: BTreeSet::new(),
            package_path: None,
            package_cache_path: None,
            offline: false,
            certificate: None,
            creation_timestamp: None,
            timings: None,
            metadata: None,
            resource_slots: BTreeSet::new(),
            resource_providers: Vec::new(),
            #[cfg(test)]
            discovery_hook: None,
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

    /// Whether font embedding also stores fonts that are identical to Typst's
    /// embedded fonts. Defaults to `false`; consumers then need the
    /// `embedded-fonts` feature or another source for those fonts.
    pub fn include_typst_embedded_fonts(mut self, include: bool) -> Self {
        self.include_typst_embedded_fonts = include;
        self
    }

    /// Whether discovery may use fonts embedded into Typst. Defaults to `true`.
    pub fn typst_embedded_fonts(mut self, include: bool) -> Self {
        self.typst_embedded_fonts = include;
        self
    }

    /// Adds a file or directory (absolute, or relative to the project root)
    /// to the pack in addition to the discovered files. Paths must be inside
    /// the project root.
    pub fn include(mut self, path: impl Into<PathBuf>) -> Self {
        self.include.push(path.into());
        self
    }

    /// Adds a directory to scan for fonts during the discovery compile.
    pub fn font_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.font_paths.push(path.into());
        self
    }

    /// Whether the discovery compile may use system fonts. Defaults to
    /// `true`.
    pub fn system_fonts(mut self, system: bool) -> Self {
        self.system_fonts = system;
        self
    }

    /// Values made available to document code as `sys.inputs` during the
    /// discovery compile.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables an experimental Typst language feature during discovery.
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Adds a compilation target whose dependencies should be discovered.
    ///
    /// If no target is added, discovery defaults to [`DiscoveryTarget::Paged`].
    pub fn target(mut self, target: DiscoveryTarget) -> Self {
        self.targets.insert(target);
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

    /// Disallows network access during the discovery compile. Defaults to
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

    /// Uses a fixed UNIX timestamp during discovery.
    pub fn creation_timestamp(mut self, timestamp: Option<i64>) -> Self {
        self.creation_timestamp = timestamp;
        self
    }

    /// Writes discovery performance timings to a Perfetto-compatible JSON file.
    pub fn timings(mut self, path: Option<PathBuf>) -> Self {
        self.timings = path;
        self
    }

    /// Sets descriptive metadata recorded in the pack manifest.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Declares a Resource Slot whose representative bytes should be omitted from the Pack.
    ///
    /// ```compile_fail
    /// use typst_pack::Packer;
    ///
    /// let _ = Packer::new("project", "main.typ").external_resource("assets/logo.png");
    /// ```
    pub fn resource_slot(mut self, path: impl Into<String>) -> Self {
        self.resource_slots.insert(path.into());
        self
    }

    /// Adds a Resource Provider used during discovery.
    ///
    /// ```compile_fail
    /// use std::path::PathBuf;
    /// use typst::diag::{FileError, FileResult};
    /// use typst::foundations::Bytes;
    /// use typst::syntax::FileId;
    /// use typst_kit::files::FileLoader;
    /// use typst_pack::Packer;
    ///
    /// struct Missing;
    /// impl FileLoader for Missing {
    ///     fn load(&self, id: FileId) -> FileResult<Bytes> {
    ///         Err(FileError::NotFound(PathBuf::from(
    ///             id.vpath().get_without_slash(),
    ///         )))
    ///     }
    /// }
    ///
    /// let _ = Packer::new("project", "main.typ").external_resource_reference(Missing);
    /// ```
    pub fn resource_provider(mut self, provider: impl FileLoader + Send + Sync + 'static) -> Self {
        self.resource_providers.push(Box::new(provider));
        self
    }

    #[cfg(test)]
    pub(crate) fn discovery_hook(mut self, hook: impl Fn(&mut DiscoveryWorld) + 'static) -> Self {
        self.discovery_hook = Some(Box::new(hook));
        self
    }

    /// Runs the discovery compile and assembles the pack.
    pub fn pack(self) -> Result<PackOutcome, PackerError> {
        let (result, timing_error) = self.pack_with_timing();
        timing_error.map_or(result, Err)
    }

    pub(crate) fn pack_with_timing(
        self,
    ) -> (Result<PackOutcome, PackerError>, Option<PackerError>) {
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
        let mut builder = Pack::builder(entrypoint.get_without_slash());
        for path in &self.resource_slots {
            builder = builder.resource_slot(path)?;
        }
        builder.validate_declarations()?;
        let explicit_resource_slots = builder.resource_slot_paths();

        // Build the discovery world.
        let packages = system_packages(
            self.package_path.as_deref(),
            self.package_cache_path.as_deref(),
            self.offline,
            self.certificate.as_deref(),
        );

        let mut fonts = FontStore::new();
        if self.system_fonts {
            fonts.extend(typst_kit::fonts::system());
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded_fonts {
            fonts.extend(typst_kit::fonts::embedded());
        }
        for path in &self.font_paths {
            fonts.extend(typst_kit::fonts::scan(path));
        }

        let primary = Arc::new(PrimaryLoader {
            system: SystemFiles::new(FsRoot::new(root.clone()), packages),
            cache: Mutex::new(HashMap::new()),
        });
        let time = match self.creation_timestamp {
            Some(timestamp) => Time::fixed_timestamp(timestamp)
                .map_err(|error| PackerError::InvalidTimestamp(error.to_string()))?,
            None => Time::system(),
        };
        let mut world = DiscoveryWorld {
            root: root.clone(),
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
            files: FileStore::new(DiscoveryLoader {
                primary,
                resources: DiscoveryResources::new(
                    self.resource_providers,
                    explicit_resource_slots,
                ),
            }),
            fonts,
            time,
            #[cfg(test)]
            source_request_hook: None,
        };
        #[cfg(test)]
        if let Some(hook) = &self.discovery_hook {
            hook(&mut world);
        }

        // Discovery compile. File stores and Resource Slot provenance accumulate
        // dependencies across targets.
        let targets = if self.targets.is_empty() {
            vec![DiscoveryTarget::Paged]
        } else {
            self.targets.iter().copied().collect()
        };
        let mut timer = typst_kit::timer::Timer::new_or_placeholder(self.timings);
        let mut discovery = None;
        let timings = timer.record(&mut world, |world| {
            let mut paged_documents = Vec::new();
            let mut html_documents = Vec::new();
            let mut compile_warnings = EcoVec::new();
            let mut seen_warnings = HashSet::new();
            for target in targets {
                match target {
                    DiscoveryTarget::Paged => {
                        let Warned { output, warnings } = typst::compile::<PagedDocument>(world);
                        compile_warnings.extend(
                            warnings
                                .into_iter()
                                .filter(|warning| seen_warnings.insert(warning.clone())),
                        );
                        match output {
                            Ok(document) => paged_documents.push(document),
                            Err(errors) => {
                                discovery = Some(Err((errors, compile_warnings)));
                                return;
                            }
                        }
                    }
                    DiscoveryTarget::Html => {
                        let Warned { output, warnings } = typst::compile::<HtmlDocument>(world);
                        compile_warnings.extend(
                            warnings
                                .into_iter()
                                .filter(|warning| seen_warnings.insert(warning.clone())),
                        );
                        match output {
                            Ok(document) => html_documents.push(document),
                            Err(errors) => {
                                discovery = Some(Err((errors, compile_warnings)));
                                return;
                            }
                        }
                    }
                }
            }
            discovery = Some(Ok((paged_documents, html_documents, compile_warnings)));
        });
        let Some(discovery) = discovery else {
            return Err(PackerError::Timings(
                timings
                    .expect_err("timer did not execute discovery")
                    .to_string(),
            ));
        };
        *timing_error = timings
            .err()
            .map(|error| PackerError::Timings(error.to_string()));
        let (paged_documents, html_documents, compile_warnings) = match discovery {
            Ok(discovery) => discovery,
            Err((errors, warnings)) => {
                return discovery_compile_error(world, errors, warnings);
            }
        };
        if let Some(path) = world.unavailable_resource_slots().into_iter().next() {
            return Err(PackerError::ResourceSlotUnavailable { path });
        }

        let mut report = PackReport {
            files: Vec::new(),
            resource_slots: Vec::new(),
            packages_vendored: Vec::new(),
            packages_unvendored: Vec::new(),
            fonts: Vec::new(),
            warnings: Vec::new(),
            compile_warnings,
        };

        // Partition the observed dependencies.
        let source_dependencies: Vec<FileId> = {
            let (_, iter) = world.sources.dependencies();
            iter.collect()
        };
        let file_dependencies: Vec<FileId> = {
            let (_, iter) = world.files.dependencies();
            iter.collect()
        };
        enum ProjectFileOrigin {
            Source,
            File,
        }
        let mut project_files: Vec<(FileId, ProjectFileOrigin)> = Vec::new();
        let mut package_files: BTreeMap<String, (PackageSpec, FileId)> = BTreeMap::new();
        for id in source_dependencies {
            match id.root() {
                VirtualRoot::Project => project_files.push((id, ProjectFileOrigin::Source)),
                VirtualRoot::Package(spec) => {
                    package_files
                        .entry(spec.to_string())
                        .or_insert_with(|| (spec.clone(), id));
                }
            }
        }
        for id in file_dependencies {
            match id.root() {
                VirtualRoot::Project if world.files.loader().is_resource_slot(id) => {}
                VirtualRoot::Project if project_files.iter().any(|(source, _)| *source == id) => {}
                VirtualRoot::Project => project_files.push((id, ProjectFileOrigin::File)),
                VirtualRoot::Package(spec) => {
                    package_files
                        .entry(spec.to_string())
                        .or_insert_with(|| (spec.clone(), id));
                }
            }
        }

        for path in world.files.loader().resource_slots() {
            report.resource_slots.push(path.clone());
            builder = builder.resource_slot(path)?;
        }

        // Project files, from the compile's own cache.
        project_files.sort_by_key(|(id, _)| id.vpath().get_with_slash().to_owned());
        for (id, origin) in project_files {
            let path = id.vpath().get_without_slash();
            let data = match origin {
                ProjectFileOrigin::Source => world.sources.file(id),
                ProjectFileOrigin::File => world.files.file(id),
            };
            match data {
                Ok(data) => {
                    report.files.push(path.to_owned());
                    builder = builder.file(path, data.to_vec())?;
                }
                Err(_) => {
                    // Accessed but unreadable (e.g. probed and missing).
                    // The compile succeeded without it, so just skip it.
                }
            }
        }

        // A typst.toml next to the entrypoint travels along: it carries
        // template/package metadata that tooling may want after extraction.
        if !report.files.iter().any(|path| path == "typst.toml")
            && !report
                .resource_slots
                .iter()
                .any(|path| path == "typst.toml")
            && let Ok(data) = std::fs::read(root.join("typst.toml"))
        {
            report.files.push("typst.toml".to_owned());
            builder = builder.file("typst.toml", data)?;
        }

        // Explicitly included files and directories.
        for path in &self.include {
            let absolute = if path.is_absolute() {
                path.clone()
            } else {
                root.join(path)
            };
            let absolute = absolute.canonicalize().map_err(|err| {
                PackerError::io(
                    &format!("failed to resolve include `{}`", path.display()),
                    err,
                )
            })?;
            let mut selected: Vec<PathBuf> = Vec::new();
            if absolute.is_dir() {
                for entry in walkdir::WalkDir::new(&absolute).sort_by_file_name() {
                    let entry = entry.map_err(|err| PackerError::Walk(err.to_string()))?;
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    if entry.path().extension().is_some_and(|ext| ext == "typk") {
                        report.warnings.push(format!(
                            "skipped pack file `{}` inside included directory",
                            entry.path().display()
                        ));
                        continue;
                    }
                    selected.push(entry.path().to_owned());
                }
            } else {
                selected.push(absolute);
            }
            for file in selected {
                let vpath = VirtualPath::virtualize(&root, &file)
                    .map_err(|_| PackerError::OutsideRoot(file.clone()))?;
                let data = std::fs::read(&file).map_err(|err| {
                    PackerError::io(&format!("failed to read `{}`", file.display()), err)
                })?;
                let path = vpath.get_without_slash().to_owned();
                if !report.files.contains(&path) {
                    report.files.push(path.clone());
                }
                builder = builder.file(path, data)?;
            }
        }

        // Packages.
        for (spec, id) in package_files.values() {
            if self.vendor_packages {
                let package_root =
                    world
                        .files
                        .loader()
                        .root(*id)
                        .map_err(|err| PackerError::Package {
                            spec: spec.clone(),
                            message: err.to_string(),
                        })?;
                for entry in walkdir::WalkDir::new(package_root.path()).sort_by_file_name() {
                    let entry = entry.map_err(|err| PackerError::Walk(err.to_string()))?;
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let vpath = VirtualPath::virtualize(package_root.path(), entry.path())
                        .map_err(|_| PackerError::OutsideRoot(entry.path().to_owned()))?;
                    let data = std::fs::read(entry.path()).map_err(|err| {
                        PackerError::io(
                            &format!("failed to read `{}`", entry.path().display()),
                            err,
                        )
                    })?;
                    builder =
                        builder.package_file(spec.clone(), vpath.get_without_slash(), data)?;
                }
                report.packages_vendored.push(spec.clone());
            } else {
                builder = builder.unvendored_package(spec.clone());
                report.packages_unvendored.push(spec.clone());
            }
        }

        // Fonts actually used by the rendered document.
        if self.embed_fonts {
            let mut used: Vec<Font> = Vec::new();
            for document in &paged_documents {
                for page in document.pages() {
                    collect_fonts(&page.frame, &mut used);
                }
            }
            for document in &html_documents {
                collect_html_fonts(document.root_node(), &mut used);
            }
            for font in used {
                if !self.include_typst_embedded_fonts && is_typst_embedded_font(&font) {
                    continue;
                }
                builder = builder.font(font.data().to_vec(), font.index())?;
            }
        }

        if let Some(metadata) = self.metadata {
            builder = builder.metadata(metadata);
        }

        let pack = builder.build()?;
        report.fonts = pack
            .fonts()
            .iter()
            .map(|font| font.manifest().path().to_owned())
            .collect();

        Ok(PackOutcome {
            pack,
            report,
            world,
        })
    }
}

fn discovery_compile_error(
    world: DiscoveryWorld,
    errors: EcoVec<SourceDiagnostic>,
    warnings: EcoVec<SourceDiagnostic>,
) -> Result<PackOutcome, PackerError> {
    if let Some(path) = world.unavailable_resource_slots().into_iter().next() {
        return Err(PackerError::ResourceSlotUnavailable { path });
    }
    Err(PackerError::Compile {
        world: Box::new(world),
        errors,
        warnings,
    })
}

/// A document target used to discover a Pack's compilation dependencies.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiscoveryTarget {
    /// PDF and image formats.
    Paged,
    /// HTML.
    Html,
}

/// The result of a successful [`Packer::pack`] run.
pub struct PackOutcome {
    /// The assembled pack.
    pub pack: Pack,
    /// The contained and supplied parts of the compilation contract.
    pub report: PackReport,
    /// The world used for the discovery compile. Kept so that the compile
    /// warnings in the report can be rendered with source context.
    pub world: DiscoveryWorld,
}

/// A summary of the compilation contract discovered by a [`Packer`].
#[derive(Debug, Clone)]
pub struct PackReport {
    /// Root-relative paths of the packed project files.
    pub files: Vec<String>,
    /// Root-relative paths of observed or explicitly declared Resource Slots.
    pub resource_slots: Vec<String>,
    /// Packages stored inside the pack.
    pub packages_vendored: Vec<PackageSpec>,
    /// Observed dependencies that were not vendored.
    pub packages_unvendored: Vec<PackageSpec>,
    /// Archive paths of embedded fonts.
    pub fonts: Vec<String>,
    /// Non-fatal problems encountered while packing.
    pub warnings: Vec<String>,
    /// Warnings emitted by the discovery compile.
    pub compile_warnings: EcoVec<SourceDiagnostic>,
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
    #[error("the discovery compile failed with {} error(s)", errors.len())]
    Compile {
        /// The world the compile ran in, for rendering the diagnostics.
        world: Box<DiscoveryWorld>,
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    #[error(
        "requested Resource Slot `{path}` is unavailable for discovery; place representative bytes at `{path}` in the source project or supply them through a Resource Provider; representative bytes are not stored in the Pack"
    )]
    ResourceSlotUnavailable { path: String },
    #[error("invalid creation timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("failed to write discovery timings: {0}")]
    Timings(String),
    #[error("failed to load package {spec}: {message}")]
    Package { spec: PackageSpec, message: String },
    #[error("failed to walk directory: {0}")]
    Walk(String),
    #[error(transparent)]
    Build(#[from] PackBuildError),
}

impl PackerError {
    fn io(message: &str, source: std::io::Error) -> Self {
        Self::Io {
            message: message.to_owned(),
            source,
        }
    }
}

/// The file-system-backed world used for the discovery compile.
///
/// This is exposed so that its compile diagnostics can be rendered with
/// source context; it is not meant to be constructed directly.
pub struct DiscoveryWorld {
    root: PathBuf,
    workdir: Option<PathBuf>,
    library: LazyHash<Library>,
    main: FileId,
    sources: FileStore<Arc<PrimaryLoader>>,
    files: FileStore<DiscoveryLoader>,
    fonts: FontStore,
    time: Time,
    #[cfg(test)]
    source_request_hook: Option<Arc<dyn Fn(FileId) + Send + Sync>>,
}

impl DiscoveryWorld {
    /// The canonicalized project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    fn unavailable_resource_slots(&self) -> Vec<String> {
        self.files.loader().resources.unavailable_resource_slots()
    }

    #[cfg(test)]
    pub(crate) fn set_source_request_hook(
        &mut self,
        hook: impl Fn(FileId) + Send + Sync + 'static,
    ) {
        self.source_request_hook = Some(Arc::new(hook));
    }

    #[cfg(test)]
    pub(crate) fn resolve_resource(&self, id: FileId) -> FileResult<Bytes> {
        self.files.loader().load(id)
    }
}

impl fmt::Debug for DiscoveryWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscoveryWorld")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl World for DiscoveryWorld {
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
        self.files.loader().resources.source(id, || {
            #[cfg(test)]
            if let Some(hook) = &self.source_request_hook {
                hook(id);
            }
            self.sources.source(id)
        })
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.time.today(offset)
    }
}

struct PrimaryLoader {
    system: SystemFiles,
    cache: Mutex<HashMap<FileId, Arc<OnceLock<FileResult<Bytes>>>>>,
}

impl PrimaryLoader {
    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        self.system.root(id)
    }
}

impl FileLoader for PrimaryLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let entry = {
            let mut cache = self.cache.lock().expect("primary file cache lock poisoned");
            Arc::clone(cache.entry(id).or_default())
        };
        entry.get_or_init(|| self.system.load(id)).clone()
    }
}

struct DiscoveryLoader {
    primary: Arc<PrimaryLoader>,
    resources: DiscoveryResources,
}

impl DiscoveryLoader {
    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        self.primary.root(id)
    }

    fn is_resource_slot(&self, id: FileId) -> bool {
        self.resources.is_resource_slot(id)
    }

    fn resource_slots(&self) -> Vec<String> {
        self.resources.resource_slots()
    }
}

impl FileLoader for DiscoveryLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.resources.file(id, || self.primary.load(id))
    }
}

/// Collects the distinct fonts used in a frame tree.
fn collect_fonts(frame: &Frame, used: &mut Vec<Font>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => collect_fonts(&group.frame, used),
            FrameItem::Text(text) => {
                let font = text.font.font();
                if !used.contains(font) {
                    used.push(font.clone());
                }
            }
            _ => {}
        }
    }
}

/// Collects fonts from frames embedded anywhere in an HTML document.
fn collect_html_fonts(node: &HtmlNode, used: &mut Vec<Font>) {
    match node {
        HtmlNode::Element(element) => {
            for child in &element.children {
                collect_html_fonts(child, used);
            }
        }
        HtmlNode::Frame(frame) => collect_fonts(&frame.inner, used),
        _ => {}
    }
}

/// Whether the font is one of Typst's embedded fonts.
fn is_typst_embedded_font(font: &Font) -> bool {
    #[cfg(feature = "embedded-fonts")]
    {
        typst_kit::fonts::embedded()
            .any(|(default, _)| default.data().as_slice() == font.data().as_slice())
    }
    #[cfg(not(feature = "embedded-fonts"))]
    {
        let _ = font;
        false
    }
}
