//! Packing a project directory by discovering what a compile actually uses.

#![cfg(feature = "fs")]

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use ecow::EcoVec;
use typst::diag::{FileResult, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::layout::{Frame, FrameItem};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::{FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::FontStore;
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};
use typst_layout::PagedDocument;

use crate::manifest::Metadata;
use crate::pack::{Pack, PackBuildError};

/// The user agent used when downloading packages from Typst Universe.
const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

/// Packs a Typst project directory into a [`Pack`].
///
/// The packer performs a discovery compile of the project and records every
/// file Typst actually reads: sources, images, data files, and package
/// files. Only those files end up in the pack. Files that a compile with
/// different inputs or a different date would read (e.g. behind conditional
/// imports) are not discovered; add them explicitly with
/// [`include`](Self::include).
pub struct Packer {
    root: PathBuf,
    entrypoint: PathBuf,
    vendor_packages: bool,
    embed_fonts: bool,
    include_default_fonts: bool,
    include: Vec<PathBuf>,
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    inputs: Dict,
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
    offline: bool,
    metadata: Option<Metadata>,
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
            include_default_fonts: false,
            include: Vec::new(),
            font_paths: Vec::new(),
            system_fonts: true,
            inputs: Dict::new(),
            package_path: None,
            package_cache_path: None,
            offline: false,
            metadata: None,
        }
    }

    /// Whether to store the files of all observed package dependencies inside
    /// the pack. Defaults to `true`; when disabled, dependencies are recorded
    /// as external and must be resolvable when the pack is compiled.
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
    /// embedded default fonts. Defaults to `false` since compiling a pack has
    /// those fonts available anyway.
    pub fn include_default_fonts(mut self, include: bool) -> Self {
        self.include_default_fonts = include;
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

    /// Sets descriptive metadata recorded in the pack manifest.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Runs the discovery compile and assembles the pack.
    pub fn pack(self) -> Result<PackOutcome, PackerError> {
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

        // Build the discovery world.
        let data = match &self.package_path {
            Some(path) => Some(FsPackages::new(path.clone())),
            None => FsPackages::system_data(),
        };
        let cache = match &self.package_cache_path {
            Some(path) => Some(FsPackages::new(path.clone())),
            None => FsPackages::system_cache(),
        };
        let universe = if self.offline {
            UniversePackages::new(crate::world::OfflineDownloader)
        } else {
            UniversePackages::new(SystemDownloader::new(USER_AGENT))
        };
        let packages = SystemPackages::from_parts(data, cache, universe);

        let mut fonts = FontStore::new();
        for path in &self.font_paths {
            fonts.extend(typst_kit::fonts::scan(path));
        }
        #[cfg(feature = "embedded-fonts")]
        fonts.extend(typst_kit::fonts::embedded());
        if self.system_fonts {
            fonts.extend(typst_kit::fonts::system());
        }

        let mut world = DiscoveryWorld {
            root: root.clone(),
            library: LazyHash::new(Library::builder().with_inputs(self.inputs.clone()).build()),
            main: RootedPath::new(VirtualRoot::Project, entrypoint.clone()).intern(),
            store: FileStore::new(SystemFiles::new(FsRoot::new(root.clone()), packages)),
            fonts,
            time: Time::system(),
        };

        // Discovery compile.
        let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
        let document = match output {
            Ok(document) => document,
            Err(errors) => {
                return Err(PackerError::Compile {
                    world: Box::new(world),
                    errors,
                    warnings,
                });
            }
        };

        let mut report = PackReport {
            files: Vec::new(),
            packages_vendored: Vec::new(),
            packages_external: Vec::new(),
            fonts: Vec::new(),
            warnings: Vec::new(),
            compile_warnings: warnings,
        };

        // Partition the observed dependencies.
        let dependencies: Vec<FileId> = {
            let (_, iter) = world.store.dependencies();
            iter.collect()
        };
        let mut project_files: Vec<FileId> = Vec::new();
        let mut package_files: BTreeMap<String, (PackageSpec, FileId)> = BTreeMap::new();
        for id in dependencies {
            match id.root() {
                VirtualRoot::Project => project_files.push(id),
                VirtualRoot::Package(spec) => {
                    package_files
                        .entry(spec.to_string())
                        .or_insert_with(|| (spec.clone(), id));
                }
            }
        }

        let mut builder = Pack::builder(entrypoint.get_without_slash());

        // Project files, from the compile's own cache.
        project_files.sort_by_key(|id| id.vpath().get_with_slash().to_owned());
        for id in project_files {
            let path = id.vpath().get_without_slash();
            match world.store.file(id) {
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
                        .store
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
                builder = builder.external_package(spec.clone());
                report.packages_external.push(spec.clone());
            }
        }

        // Fonts actually used by the rendered document.
        if self.embed_fonts {
            let mut used: Vec<Font> = Vec::new();
            for page in document.pages() {
                collect_fonts(&page.frame, &mut used);
            }
            for font in used {
                if !self.include_default_fonts && is_default_font(&font) {
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
            .map(|font| font.entry.path.clone())
            .collect();

        Ok(PackOutcome {
            pack,
            report,
            world,
        })
    }
}

/// The result of a successful [`Packer::pack`] run.
pub struct PackOutcome {
    /// The assembled pack.
    pub pack: Pack,
    /// What went into the pack.
    pub report: PackReport,
    /// The world used for the discovery compile. Kept so that the compile
    /// warnings in the report can be rendered with source context.
    pub world: DiscoveryWorld,
}

/// A summary of what a [`Packer`] put into a pack.
#[derive(Debug, Clone)]
pub struct PackReport {
    /// Root-relative paths of the packed project files.
    pub files: Vec<String>,
    /// Packages stored inside the pack.
    pub packages_vendored: Vec<PackageSpec>,
    /// Observed dependencies that were not vendored.
    pub packages_external: Vec<PackageSpec>,
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
    library: LazyHash<Library>,
    main: FileId,
    store: FileStore<SystemFiles>,
    fonts: FontStore,
    time: Time,
}

impl DiscoveryWorld {
    /// The canonicalized project root.
    pub fn root(&self) -> &Path {
        &self.root
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
        self.store.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.store.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.time.today(offset)
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

/// Whether the font is one of Typst's embedded default fonts.
fn is_default_font(font: &Font) -> bool {
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
