//! Packing a project directory by discovering what a compile actually uses.

#![cfg(feature = "fs")]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
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
use typst_kit::fonts::FontStore;

use crate::embedded::EmbeddedTypst;
use crate::manifest::{
    DiscoveryEvidence, DiscoveryObservationEvidence, DiscoveryOverrideEvidence, PackMetadata,
};
use crate::pack::{Pack, PackBuildError, PackageFiles};
use crate::resource::{DiscoveryResources, Provider};
use crate::world::system_packages;
use crate::world_trace::{
    CapturedAccessKind, CapturedAccessOutcome, CapturedObservation, WorldTrace,
};

#[cfg(test)]
type DiscoveryHook = Box<dyn Fn(&mut DiscoveryWorld)>;
type PackageEvidence = (PackageSpec, PathBuf, Vec<(String, Bytes)>);

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
    discovery_overrides: BTreeMap<String, Bytes>,
    features: Vec<Feature>,
    targets: Vec<DiscoveryTarget>,
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
    #[cfg(test)]
    after_discovery_hook: Option<Box<dyn Fn()>>,
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
            discovery_overrides: BTreeMap::new(),
            features: Vec::new(),
            targets: Vec::new(),
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
            #[cfg(test)]
            after_discovery_hook: None,
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

    /// Replaces one existing project file only while discovering and replaying closure.
    pub fn discovery_override(mut self, path: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        self.discovery_overrides
            .insert(path.into(), Bytes::new(data.into()));
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
        if !self.targets.contains(&target) {
            self.targets.push(target);
        }
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

    #[cfg(test)]
    pub(crate) fn after_discovery_hook(mut self, hook: impl Fn() + 'static) -> Self {
        self.after_discovery_hook = Some(Box::new(hook));
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
        let mut discovery_overrides = BTreeMap::new();
        for (supplied, data) in self.discovery_overrides {
            let path = Pack::canonical_project_path(&supplied).map_err(|message| {
                PackerError::InvalidDiscoveryOverride {
                    path: supplied.clone(),
                    message,
                }
            })?;
            if !root.join(&path).is_file() {
                return Err(PackerError::InvalidDiscoveryOverride {
                    path,
                    message: "the baseline project file does not exist".to_owned(),
                });
            }
            discovery_overrides.insert(path, data);
        }
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

        let fonts = font_store(
            self.system_fonts,
            self.typst_embedded_fonts,
            &self.font_paths,
        );

        let primary = Arc::new(PrimaryLoader {
            system: SystemFiles::new(FsRoot::new(root.clone()), packages),
            cache: Mutex::new(HashMap::new()),
        });
        let creation_timestamp = self.creation_timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs() as i64)
        });
        let time = Time::fixed_timestamp(creation_timestamp)
            .map_err(|error| PackerError::InvalidTimestamp(error.to_string()))?;
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
            used_font_indices: Mutex::new(BTreeSet::new()),
            discovery_overrides: discovery_overrides.clone(),
            time,
            #[cfg(test)]
            source_request_hook: None,
        };
        #[cfg(test)]
        if let Some(hook) = &self.discovery_hook {
            hook(&mut world);
        }

        // Discovery compile. Each target receives its own diagnostics and trace;
        // the underlying stable stores may still share immutable acquisitions.
        let targets = if self.targets.is_empty() {
            vec![DiscoveryTarget::Paged]
        } else {
            self.targets.clone()
        };
        let mut timer = typst_kit::timer::Timer::new_or_placeholder(self.timings);
        let mut discovery = None;
        let mut discovered_sources = HashSet::new();
        let mut discovered_files = HashSet::new();
        let mut discovered_font_indices = BTreeSet::new();
        let timings = timer.record(&mut world, |world| {
            let mut variants = Vec::new();
            for target in targets {
                // The first pass stabilizes lazy package and font acquisitions.
                // Its partial observations and diagnostics are intentionally discarded.
                let _ = compile_discovery_target(world, target);
                world.discard_dependency_observations();
                let traced = WorldTrace::new(world);
                let Warned { output, warnings } = compile_discovery_target(&traced, target);
                let trace = DiscoveryTrace::from_captured(traced.snapshot());
                let (sources, files, fonts) = world.take_dependency_observations();
                discovered_sources.extend(sources);
                discovered_files.extend(files);
                discovered_font_indices.extend(fonts);
                if let Err(errors) = output {
                    discovery = Some(Err((errors, warnings)));
                    return;
                }
                variants.push(DiscoveryVariantReport {
                    request: DiscoveryRequest {
                        target,
                        inputs: DiscoveryInputsInventory::from_inputs(&self.inputs),
                        overrides: DiscoveryOverridesInventory::from_overrides(
                            &discovery_overrides,
                        ),
                        features: self.features.clone(),
                        document_timestamp: creation_timestamp,
                    },
                    replay_inputs: self.inputs.clone(),
                    replay_overrides: discovery_overrides.clone(),
                    warnings,
                    trace,
                    replay_warnings: EcoVec::new(),
                    replay_trace: DiscoveryTrace::default(),
                });
            }
            discovery = Some(Ok(variants));
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
        let mut discovery_variants = match discovery {
            Ok(discovery) => discovery,
            Err((errors, warnings)) => {
                return discovery_compile_error(world, errors, warnings);
            }
        };
        #[cfg(test)]
        if let Some(hook) = &self.after_discovery_hook {
            hook();
        }
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
            compile_warnings: discovery_variants
                .iter()
                .flat_map(|variant| variant.warnings.iter().cloned())
                .collect(),
            discovery_variants: Vec::new(),
        };

        let mut project_evidence = Vec::new();
        let mut project_absence_evidence = Vec::new();
        let mut package_evidence = Vec::new();
        let mut directory_evidence = Vec::new();
        let mut external_package_trees = BTreeMap::new();
        let mut resource_snapshot = BTreeMap::new();
        let mut resource_evidence = Vec::new();

        // Partition the observed dependencies.
        let source_dependencies: Vec<FileId> = discovered_sources.into_iter().collect();
        let file_dependencies: Vec<FileId> = discovered_files.into_iter().collect();
        *world
            .used_font_indices
            .lock()
            .expect("used font index lock poisoned") = discovered_font_indices;
        let font_evidence = FontEvidence {
            catalog: font_catalog_evidence(&world.fonts),
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
                VirtualRoot::Project if world.files.loader().is_resource_slot(id) => {
                    if let Ok(data) = world.files.file(id) {
                        let path = id.vpath().get_without_slash().to_owned();
                        resource_evidence.push(ResourceEvidence {
                            path: path.clone(),
                            data: data.clone(),
                            provider: world.files.loader().resources.selected_provider(&path),
                        });
                        resource_snapshot.insert(path, data);
                    }
                }
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
                    project_evidence.push((root.join(path), data.clone()));
                    builder = builder.file(path, data.to_vec())?;
                }
                Err(FileError::NotFound(_)) => {
                    // Accessed but unreadable (e.g. probed and missing).
                    // The compile succeeded without it, so just skip it.
                    project_absence_evidence.push(root.join(path));
                }
                Err(_) => {}
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
            project_evidence.push((root.join("typst.toml"), Bytes::new(data.clone())));
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
                directory_evidence.push((absolute.clone(), selected.clone()));
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
                project_evidence.push((file.clone(), Bytes::new(data.clone())));
                builder = builder.file(path, data)?;
            }
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
            if !self.vendor_packages {
                external_package_trees.insert(spec.to_string(), tree.clone());
            }
            for (path, data) in tree {
                builder = if self.vendor_packages {
                    builder.package_file(spec.clone(), path, data.to_vec())?
                } else {
                    builder.external_package_file(spec.clone(), path, data.to_vec())?
                };
            }
            if self.vendor_packages {
                report.packages_vendored.push(spec.clone());
            } else {
                report.packages_unvendored.push(spec.clone());
            }
        }

        // Project selected faces back into the original candidate catalog order.
        for font in world.used_fonts() {
            let embed = self.embed_fonts
                && (self.include_typst_embedded_fonts || !is_typst_embedded_font(&font));
            builder = if embed {
                builder.font(font.data().to_vec(), font.index())?
            } else {
                builder.external_font(font.data().to_vec(), font.index())?
            };
        }

        if let Some(metadata) = self.metadata {
            builder = builder.metadata(metadata);
        }

        revalidate_creation_evidence(
            &project_evidence,
            &project_absence_evidence,
            &package_evidence,
            &directory_evidence,
        )?;
        revalidate_resource_evidence(&root, &world.files.loader().resources, &resource_evidence)?;
        revalidate_font_evidence(
            self.system_fonts,
            self.typst_embedded_fonts,
            &self.font_paths,
            &font_evidence,
        )?;
        let mut pack = builder.build()?;
        report.fonts = pack
            .manifest()
            .fonts()
            .iter()
            .map(|font| font.path().to_owned())
            .collect();

        let exact_packages = pack
            .materialize_package_trees(external_package_trees)
            .expect("creation retained every exact external package tree");
        let font_fulfillments = world
            .used_fonts()
            .into_iter()
            .map(|font| {
                (
                    crate::FontContainerIdentity::from_bytes(font.data().as_slice()),
                    font.data().clone(),
                )
            })
            .collect();
        let exact_fonts = pack
            .materialize_font_catalog(&font_fulfillments)
            .expect("creation retained every exact font container");
        replay_variants(
            &pack,
            &mut discovery_variants,
            exact_packages,
            exact_fonts,
            resource_snapshot,
        )?;
        revalidate_creation_evidence(
            &project_evidence,
            &project_absence_evidence,
            &package_evidence,
            &directory_evidence,
        )?;
        revalidate_resource_evidence(&root, &world.files.loader().resources, &resource_evidence)?;
        revalidate_font_evidence(
            self.system_fonts,
            self.typst_embedded_fonts,
            &self.font_paths,
            &font_evidence,
        )?;
        pack.set_discovery(
            discovery_variants
                .iter()
                .map(|variant| variant.persisted_evidence(&pack))
                .collect(),
        );
        report.discovery_variants = discovery_variants;

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
        world: Box::new(CreationDiagnosticContext { world }),
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
    pub(crate) world: DiscoveryWorld,
}

/// Opaque source context retained for first-party creation diagnostics.
///
/// This value intentionally does not implement Typst's [`World`] interface.
#[derive(Debug)]
pub struct CreationDiagnosticContext {
    world: DiscoveryWorld,
}

impl CreationDiagnosticContext {
    pub(crate) fn world(&self) -> &DiscoveryWorld {
        &self.world
    }
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
    /// Independent requests, diagnostics, and traces for every ordered variant.
    pub discovery_variants: Vec<DiscoveryVariantReport>,
}

/// The exact source-evaluation request used by one Discovery Variant.
#[derive(Debug, Clone)]
pub struct DiscoveryRequest {
    target: DiscoveryTarget,
    inputs: DiscoveryInputsInventory,
    overrides: DiscoveryOverridesInventory,
    features: Vec<Feature>,
    document_timestamp: i64,
}

impl DiscoveryRequest {
    pub fn target(&self) -> DiscoveryTarget {
        self.target
    }

    pub fn inputs(&self) -> DiscoveryInputsInventory {
        self.inputs
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn overrides(&self) -> &DiscoveryOverridesInventory {
        &self.overrides
    }

    pub fn document_timestamp(&self) -> i64 {
        self.document_timestamp
    }
}

/// Safe commitment evidence for discovery-only project replacements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryOverridesInventory(Vec<(String, usize, [u8; 16])>);

impl DiscoveryOverridesInventory {
    fn from_overrides(overrides: &BTreeMap<String, Bytes>) -> Self {
        Self(
            overrides
                .iter()
                .map(|(path, data)| {
                    (
                        path.clone(),
                        data.len(),
                        typst::utils::hash128(&(
                            "typst-pack-discovery-override-v1+typst-0.15",
                            path,
                            data,
                        ))
                        .to_be_bytes(),
                    )
                })
                .collect(),
        )
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, usize, [u8; 16])> + '_ {
        self.0
            .iter()
            .map(|(path, length, commitment)| (path.as_str(), *length, *commitment))
    }
}

/// Safe commitment evidence for the exact Typst inputs used during discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryInputsInventory {
    commitment: [u8; 16],
    entry_count: usize,
}

impl DiscoveryInputsInventory {
    fn from_inputs(inputs: &Dict) -> Self {
        Self {
            commitment: typst::utils::hash128(&(
                "typst-pack-discovery-inputs-v1+typst-0.15",
                inputs,
            ))
            .to_be_bytes(),
            entry_count: inputs.len(),
        }
    }

    pub fn schema(self) -> &'static str {
        "typst-pack-discovery-inputs-v1+typst-0.15"
    }

    pub fn commitment(self) -> [u8; 16] {
        self.commitment
    }

    pub fn entry_count(self) -> usize {
        self.entry_count
    }
}

/// Independent creation and replay evidence for one Discovery Variant.
#[derive(Clone)]
pub struct DiscoveryVariantReport {
    request: DiscoveryRequest,
    replay_inputs: Dict,
    replay_overrides: BTreeMap<String, Bytes>,
    warnings: EcoVec<SourceDiagnostic>,
    trace: DiscoveryTrace,
    replay_warnings: EcoVec<SourceDiagnostic>,
    replay_trace: DiscoveryTrace,
}

impl fmt::Debug for DiscoveryVariantReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscoveryVariantReport")
            .field("request", &self.request)
            .field("warnings", &self.warnings)
            .field("trace", &self.trace)
            .field("replay_warnings", &self.replay_warnings)
            .field("replay_trace", &self.replay_trace)
            .finish()
    }
}

impl DiscoveryVariantReport {
    pub fn request(&self) -> &DiscoveryRequest {
        &self.request
    }

    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }

    pub fn trace(&self) -> &DiscoveryTrace {
        &self.trace
    }

    pub fn replay_warnings(&self) -> &[SourceDiagnostic] {
        &self.replay_warnings
    }

    pub fn replay_trace(&self) -> &DiscoveryTrace {
        &self.replay_trace
    }

    fn persisted_evidence(&self, pack: &Pack) -> DiscoveryEvidence {
        let observations = self
            .trace
            .observations()
            .map(|observation| {
                let kind = match observation.kind() {
                    DiscoveryAccessKind::Source => "source",
                    DiscoveryAccessKind::File => "file",
                    DiscoveryAccessKind::Font => "font",
                };
                let project_path = observation.logical_path().strip_prefix("project:");
                let replacement = project_path.and_then(|path| {
                    self.request
                        .overrides
                        .iter()
                        .find(|(candidate, _, _)| *candidate == path)
                });
                let (outcome, mut byte_length, mut digest) = match observation.outcome() {
                    DiscoveryAccessOutcome::Read {
                        byte_length,
                        digest,
                    } => ("read", Some(*byte_length as u64), Some(hex_digest(*digest))),
                    DiscoveryAccessOutcome::Missing => ("missing", None, None),
                    DiscoveryAccessOutcome::Failed => ("failed", None, None),
                };
                let (authority, project_provenance, commitment) =
                    if let Some((_, length, value)) = replacement {
                        byte_length = Some(length as u64);
                        digest = None;
                        ("project", Some("override"), Some(hex_digest(value)))
                    } else if project_path.is_some_and(|path| pack.is_resource_slot(path)) {
                        ("resource-provider", Some("resource-slot"), None)
                    } else if project_path.is_some() {
                        ("project", Some("baseline"), None)
                    } else if observation.logical_path().starts_with("package:") {
                        ("package", None, None)
                    } else {
                        ("font-catalog", None, None)
                    };
                DiscoveryObservationEvidence::new(
                    kind.to_owned(),
                    observation.logical_path().to_owned(),
                    authority.to_owned(),
                    project_provenance.map(str::to_owned),
                    observation.font_index().map(|index| index as u64),
                    outcome.to_owned(),
                    byte_length,
                    digest,
                    commitment,
                )
            })
            .collect();
        DiscoveryEvidence::new(
            match self.request.target {
                DiscoveryTarget::Paged => "paged",
                DiscoveryTarget::Html => "html",
            }
            .to_owned(),
            hex_digest(self.request.inputs.commitment()),
            self.request.inputs.entry_count() as u64,
            self.request
                .overrides
                .iter()
                .map(|(path, length, commitment)| {
                    DiscoveryOverrideEvidence::new(
                        path.to_owned(),
                        length as u64,
                        hex_digest(commitment),
                    )
                })
                .collect(),
            self.request
                .features
                .iter()
                .map(|feature| format!("{feature:?}"))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            self.request.document_timestamp,
            observations,
        )
    }
}

fn hex_digest(digest: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(digest))
}

/// The canonical causal observations made by one engine execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryTrace {
    observations: BTreeSet<DiscoveryObservation>,
}

impl DiscoveryTrace {
    pub fn observations(&self) -> impl Iterator<Item = &DiscoveryObservation> {
        self.observations.iter()
    }

    fn from_captured(observations: BTreeSet<CapturedObservation>) -> Self {
        Self {
            observations: observations
                .into_iter()
                .filter(|observation| {
                    !(observation.kind == CapturedAccessKind::Font
                        && observation.outcome == CapturedAccessOutcome::Missing)
                })
                .map(DiscoveryObservation::from_captured)
                .collect(),
        }
    }
}

/// The kind of dependency access observed by Typst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiscoveryAccessKind {
    Source,
    File,
    Font,
}

/// The stable outcome of one dependency access.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiscoveryAccessOutcome {
    Read {
        byte_length: usize,
        digest: [u8; 16],
    },
    Missing,
    Failed,
}

/// One deduplicated logical dependency observation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiscoveryObservation {
    kind: DiscoveryAccessKind,
    logical_path: String,
    font_index: Option<usize>,
    outcome: DiscoveryAccessOutcome,
}

impl DiscoveryObservation {
    fn from_captured(observation: CapturedObservation) -> Self {
        Self {
            kind: match observation.kind {
                CapturedAccessKind::Source => DiscoveryAccessKind::Source,
                CapturedAccessKind::File => DiscoveryAccessKind::File,
                CapturedAccessKind::Font => DiscoveryAccessKind::Font,
            },
            logical_path: observation.logical_path,
            font_index: observation.font_index,
            outcome: match observation.outcome {
                CapturedAccessOutcome::Read {
                    byte_length,
                    digest,
                } => DiscoveryAccessOutcome::Read {
                    byte_length,
                    digest,
                },
                CapturedAccessOutcome::Missing => DiscoveryAccessOutcome::Missing,
                CapturedAccessOutcome::Failed => DiscoveryAccessOutcome::Failed,
            },
        }
    }

    pub fn kind(&self) -> DiscoveryAccessKind {
        self.kind
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub fn font_index(&self) -> Option<usize> {
        self.font_index
    }

    pub fn outcome(&self) -> &DiscoveryAccessOutcome {
        &self.outcome
    }
}

#[cfg(test)]
mod trace_projection_tests {
    use super::*;

    #[test]
    fn discovery_filters_missing_fonts_but_retains_other_misses() {
        let trace = DiscoveryTrace::from_captured(BTreeSet::from([
            CapturedObservation {
                kind: CapturedAccessKind::File,
                logical_path: "project:missing.bin".to_owned(),
                font_index: None,
                outcome: CapturedAccessOutcome::Missing,
            },
            CapturedObservation {
                kind: CapturedAccessKind::Font,
                logical_path: "font-index:7".to_owned(),
                font_index: Some(7),
                outcome: CapturedAccessOutcome::Missing,
            },
        ]));

        assert_eq!(trace.observations().count(), 1);
        let observation = trace.observations().next().unwrap();
        assert_eq!(observation.kind(), DiscoveryAccessKind::File);
        assert_eq!(observation.outcome(), &DiscoveryAccessOutcome::Missing);
    }
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
        /// Opaque source context retained for first-party diagnostic rendering.
        world: Box<CreationDiagnosticContext>,
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    #[error(
        "requested Resource Slot `{path}` is unavailable for discovery; place representative bytes at `{path}` in the source project or supply them through a Resource Provider; representative bytes are not stored in the Pack"
    )]
    ResourceSlotUnavailable { path: String },
    #[error("invalid creation timestamp: {0}")]
    InvalidTimestamp(String),
    #[error("invalid discovery-only override `{path}`: {message}")]
    InvalidDiscoveryOverride { path: String, message: String },
    #[error("failed to write discovery timings: {0}")]
    Timings(String),
    #[error("failed to load package {spec}: {message}")]
    Package { spec: PackageSpec, message: String },
    #[error("failed to walk directory: {0}")]
    Walk(String),
    #[error("creation evidence changed before Pack issuance: `{path}`")]
    CreationEvidenceChanged { path: String },
    #[error("assembled Pack replay for {target:?} failed with {} error(s)", errors.len())]
    ReplayCompile {
        target: DiscoveryTarget,
        errors: EcoVec<SourceDiagnostic>,
        warnings: EcoVec<SourceDiagnostic>,
    },
    #[error("assembled Pack replay for {target:?} did not reproduce its Discovery Trace")]
    ReplayTraceMismatch { target: DiscoveryTarget },
    #[error("assembled Pack replay for {target:?} did not reproduce its official diagnostics")]
    ReplayDiagnosticsMismatch { target: DiscoveryTarget },
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

/// The private file-system-backed world used for discovery compilation.
pub(crate) struct DiscoveryWorld {
    root: PathBuf,
    workdir: Option<PathBuf>,
    library: LazyHash<Library>,
    main: FileId,
    sources: FileStore<Arc<PrimaryLoader>>,
    files: FileStore<DiscoveryLoader>,
    fonts: FontStore,
    used_font_indices: Mutex<BTreeSet<usize>>,
    time: Time,
    discovery_overrides: BTreeMap<String, Bytes>,
    #[cfg(test)]
    source_request_hook: Option<Arc<dyn Fn(FileId) + Send + Sync>>,
}

impl DiscoveryWorld {
    fn used_fonts(&self) -> Vec<Font> {
        self.used_font_indices
            .lock()
            .expect("used font index lock poisoned")
            .iter()
            .filter_map(|index| self.fonts.font(*index))
            .collect()
    }
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

    fn discard_dependency_observations(&mut self) {
        let (_, sources) = self.sources.dependencies();
        sources.for_each(drop);
        let (_, files) = self.files.dependencies();
        files.for_each(drop);
        self.used_font_indices
            .lock()
            .expect("used font index lock poisoned")
            .clear();
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
        if matches!(id.root(), VirtualRoot::Project)
            && let Some(data) = self.discovery_overrides.get(id.vpath().get_without_slash())
        {
            let _ = self.sources.source(id);
            let text = std::str::from_utf8(data.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
            return Ok(Source::new(id, text.to_owned()));
        }
        self.files.loader().resources.source(id, || {
            #[cfg(test)]
            if let Some(hook) = &self.source_request_hook {
                hook(id);
            }
            self.sources.source(id)
        })
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if matches!(id.root(), VirtualRoot::Project)
            && let Some(data) = self.discovery_overrides.get(id.vpath().get_without_slash())
        {
            let _ = self.files.file(id);
            return Ok(data.clone());
        }
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

fn compile_discovery_target(
    world: &dyn World,
    target: DiscoveryTarget,
) -> Warned<Result<(), EcoVec<SourceDiagnostic>>> {
    match target {
        DiscoveryTarget::Paged => {
            let Warned { output, warnings } = EmbeddedTypst::compile_paged(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
        DiscoveryTarget::Html => {
            let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
    }
}

fn revalidate_creation_evidence(
    project: &[(PathBuf, Bytes)],
    project_absences: &[PathBuf],
    packages: &[PackageEvidence],
    directories: &[(PathBuf, Vec<PathBuf>)],
) -> Result<(), PackerError> {
    for (path, expected) in project {
        if std::fs::read(path).ok().as_deref() != Some(expected.as_slice()) {
            return Err(PackerError::CreationEvidenceChanged {
                path: path.display().to_string(),
            });
        }
    }
    for path in project_absences {
        if path.try_exists().unwrap_or(true) {
            return Err(PackerError::CreationEvidenceChanged {
                path: path.display().to_string(),
            });
        }
    }
    for (spec, root, expected) in packages {
        if crate::world::read_complete_package_tree(root).as_ref().ok() != Some(expected) {
            return Err(PackerError::CreationEvidenceChanged {
                path: spec.to_string(),
            });
        }
    }
    for (root, expected) in directories {
        let actual = walkdir::WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "typk")
            })
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        if &actual != expected {
            return Err(PackerError::CreationEvidenceChanged {
                path: root.display().to_string(),
            });
        }
    }
    Ok(())
}

struct ResourceEvidence {
    path: String,
    data: Bytes,
    provider: Option<usize>,
}

fn revalidate_resource_evidence(
    root: &Path,
    resources: &DiscoveryResources,
    evidence: &[ResourceEvidence],
) -> Result<(), PackerError> {
    for expected in evidence {
        let actual = if expected.provider.is_none() {
            std::fs::read(root.join(&expected.path))
                .ok()
                .map(Bytes::new)
        } else {
            if root.join(&expected.path).try_exists().unwrap_or(true) {
                return Err(PackerError::CreationEvidenceChanged {
                    path: expected.path.clone(),
                });
            }
            let id = RootedPath::new(
                VirtualRoot::Project,
                VirtualPath::new(&expected.path)
                    .expect("recorded Resource Slot path remains canonical"),
            )
            .intern();
            resources
                .resolve_provider(id)
                .ok()
                .and_then(|(provider, data)| (Some(provider) == expected.provider).then_some(data))
        };
        if actual.as_ref() != Some(&expected.data) {
            return Err(PackerError::CreationEvidenceChanged {
                path: expected.path.clone(),
            });
        }
    }
    Ok(())
}

fn font_store(system_fonts: bool, typst_embedded_fonts: bool, font_paths: &[PathBuf]) -> FontStore {
    let mut fonts = FontStore::new();
    if system_fonts {
        fonts.extend(typst_kit::fonts::system());
    }
    #[cfg(feature = "embedded-fonts")]
    if typst_embedded_fonts {
        fonts.extend(typst_kit::fonts::embedded());
    }
    #[cfg(not(feature = "embedded-fonts"))]
    let _ = typst_embedded_fonts;
    for path in font_paths {
        fonts.extend(typst_kit::fonts::scan(path));
    }
    fonts
}

fn revalidate_font_evidence(
    system_fonts: bool,
    typst_embedded_fonts: bool,
    font_paths: &[PathBuf],
    evidence: &FontEvidence,
) -> Result<(), PackerError> {
    let fonts = font_store(system_fonts, typst_embedded_fonts, font_paths);
    if font_catalog_evidence(&fonts) != evidence.catalog {
        return Err(PackerError::CreationEvidenceChanged {
            path: "font catalog".to_owned(),
        });
    }
    Ok(())
}

struct FontEvidence {
    catalog: Vec<(FontInfo, Option<Bytes>)>,
}

fn font_book_infos(book: &FontBook) -> Vec<FontInfo> {
    let count = book
        .families()
        .flat_map(|(_, indices)| indices)
        .max()
        .map_or(0, |index| index + 1);
    (0..count)
        .filter_map(|index| book.info(index).cloned())
        .collect()
}

fn font_catalog_evidence(fonts: &FontStore) -> Vec<(FontInfo, Option<Bytes>)> {
    font_book_infos(fonts.book())
        .into_iter()
        .enumerate()
        .map(|(index, info)| (info, fonts.font(index).map(|font| font.data().clone())))
        .collect()
}

#[derive(Clone)]
struct SnapshotResources(Arc<BTreeMap<String, Bytes>>);

impl FileLoader for SnapshotResources {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
    }
}

fn replay_variants(
    pack: &Pack,
    variants: &mut [DiscoveryVariantReport],
    exact_packages: BTreeMap<String, PackageFiles>,
    exact_fonts: Vec<Font>,
    resources: BTreeMap<String, Bytes>,
) -> Result<(), PackerError> {
    let resources = SnapshotResources(Arc::new(resources));
    for variant in variants {
        let mut world = crate::world::PackWorld::builder(pack.clone())
            .inputs(std::mem::take(&mut variant.replay_inputs))
            .exact_project_overrides(std::mem::take(&mut variant.replay_overrides))
            .exact_packages(exact_packages.clone())
            .exact_font_catalog(exact_fonts.clone())
            .resource_provider(resources.clone())
            .fixed_timestamp(variant.request.document_timestamp)
            .map_err(|error| PackerError::InvalidTimestamp(error.to_string()))?;
        for feature in &variant.request.features {
            world = world.feature(*feature);
        }
        let world = world
            .build()
            .expect("frozen creation dependencies must build a Pack World");
        let traced = WorldTrace::new(&world);
        let Warned { output, warnings } = compile_discovery_target(&traced, variant.request.target);
        let replay_trace = DiscoveryTrace::from_captured(traced.snapshot());
        if let Err(errors) = output {
            return Err(PackerError::ReplayCompile {
                target: variant.request.target,
                errors,
                warnings,
            });
        }
        if replay_trace != variant.trace {
            return Err(PackerError::ReplayTraceMismatch {
                target: variant.request.target,
            });
        }
        if warnings != variant.warnings {
            return Err(PackerError::ReplayDiagnosticsMismatch {
                target: variant.request.target,
            });
        }
        variant.replay_warnings = warnings;
        variant.replay_trace = replay_trace;
    }
    Ok(())
}
