//! Filesystem packing, and resolving a pack's external requirements from the
//! filesystem.
//!
//! The adapter reads and the core transforms: it reads the project, composes
//! the font catalog from the host's font sources, and obtains the packages
//! creation reports missing; creation and compilation run in the core over
//! those bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use typst::diag::SourceDiagnostic;
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;

use crate::compile::{CompileOptions, Input};
use crate::creation::{CreateError, CreateOutcome, CreateRequest, create};
use crate::domain::{DocumentTime, TypstTarget};
use crate::font_catalog::{FontContainer, FontDisposition};
use crate::fs_fonts::{FilesystemFontSource, read_filesystem_fonts};
use crate::fs_packages::{PackageDirs, Registry};
use crate::fs_project::{IGNORE_FILE, read_filesystem_project};
use crate::fs_read::FilesystemReadError;
use crate::limits::ReadLimits;
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackInvariantIssue};
use crate::package_catalog::{PackageDisposition, PackageTree};
use crate::package_failure::{PackageReadFailure, PackageReadFailureReason};
use crate::world::Sources;

/// Read limits applied to each filesystem read.
///
/// Font roots get larger defaults than a project because system font
/// directories routinely hold gigabytes of collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemReadLimits {
    /// Limits for reading the project directory.
    pub project: ReadLimits,
    /// Limits for reading each package tree.
    pub packages: ReadLimits,
    /// Limits shared by every font root of one read.
    pub fonts: ReadLimits,
}

impl Default for FilesystemReadLimits {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self {
            project: ReadLimits {
                max_entries: 1_000_000,
                max_file_bytes: 256 * MIB,
                max_total_bytes: 2048 * MIB,
            },
            packages: ReadLimits {
                max_entries: 100_000,
                max_file_bytes: 64 * MIB,
                max_total_bytes: 512 * MIB,
            },
            fonts: ReadLimits {
                max_entries: 100_000,
                max_file_bytes: 256 * MIB,
                max_total_bytes: 2048 * MIB,
            },
        }
    }
}

/// Creates packs from project directories, and resolves the external
/// requirements of packs, using the host's fonts and package directories.
///
/// Fonts come from the system font directories, Typst's embedded fonts (with
/// the `embedded-fonts` feature), and added font paths, in that order.
/// Packages come from the local package directory, then the package cache,
/// then, only after [`download_packages`](Self::download_packages) (with the
/// `egress` feature), the Typst Universe registry.
///
/// ```no_run
/// use typst_pack::FilesystemPacker;
/// use typst_pack::pack_archive::encode;
///
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let packed = FilesystemPacker::new()
///     .embed_fonts(true)
///     .pack("path/to/project", "main.typ")?;
/// for warning in &packed.warnings {
///     eprintln!("warning: {}", warning.message);
/// }
/// std::fs::write("project.typk", encode(&packed.pack)?)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct FilesystemPacker {
    font_paths: Vec<PathBuf>,
    system_fonts: bool,
    typst_embedded_fonts: bool,
    package_path: Option<PathBuf>,
    package_cache_path: Option<PathBuf>,
    registry: Option<Registry>,
    limits: FilesystemReadLimits,
    vendor_packages: bool,
    embed_fonts: bool,
    include_typst_embedded_fonts: bool,
    target: TypstTarget,
    options: CompileOptions,
    variants: Vec<Vec<(String, Input)>>,
    metadata: Option<PackMetadata>,
}

/// A pack created from a project directory.
#[derive(Debug)]
pub struct Packed {
    pub pack: Pack,
    /// Warnings of the discovery compiles.
    pub warnings: Vec<SourceDiagnostic>,
    /// The sources the discovery compiles read; project files are named
    /// relative to the current directory.
    pub sources: Sources,
    /// Font files and directories that could not be read and were left out.
    pub skipped_fonts: Vec<FilesystemReadError>,
}

/// The external requirements of a pack found on the filesystem, ready for
/// [`CompileRequest::packages`](crate::CompileRequest::packages) and
/// [`CompileRequest::fonts`](crate::CompileRequest::fonts).
#[derive(Debug, Default)]
pub struct Resolved {
    pub packages: Vec<(PackageSpec, PackageTree)>,
    pub fonts: Vec<FontContainer>,
    /// The directory each package was read from, for dependency reporting.
    pub package_dirs: BTreeMap<String, PathBuf>,
    /// Font files and directories that could not be read and were left out.
    pub skipped_fonts: Vec<FilesystemReadError>,
}

/// A failure of filesystem packing or resolution.
#[derive(Debug, thiserror::Error)]
pub enum FilesystemPackError {
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`{}` is outside the project root and cannot be packed", .0.display())]
    OutsideRoot(PathBuf),
    #[error("entrypoint `{0}` is excluded by the project's {IGNORE_FILE}")]
    EntrypointIgnored(String),
    #[error("failed to read the project: {0}")]
    Project(#[source] FilesystemReadError),
    #[error("failed to read fonts: {0}")]
    Fonts(#[source] FilesystemReadError),
    /// A required package could not be read; resolution only.
    #[error(transparent)]
    Package(PackageReadFailure),
    #[error("creation requested package {0} again after it was supplied")]
    RepeatedPackage(PackageSpec),
    #[error(transparent)]
    Create(#[from] CreateError),
}

impl Default for FilesystemPacker {
    fn default() -> Self {
        Self::new()
    }
}

impl FilesystemPacker {
    /// System fonts and Typst's embedded fonts, the host's package
    /// directories without downloads, vendored packages, external fonts, the
    /// paged target, and the host's current time as document time.
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i64);
        Self {
            font_paths: Vec::new(),
            system_fonts: true,
            typst_embedded_fonts: true,
            package_path: None,
            package_cache_path: None,
            registry: None,
            limits: FilesystemReadLimits::default(),
            vendor_packages: true,
            embed_fonts: false,
            include_typst_embedded_fonts: false,
            target: TypstTarget::Paged,
            options: CompileOptions {
                document_time: DocumentTime::UnixTimestamp(now),
                ..CompileOptions::default()
            },
            variants: Vec::new(),
            metadata: None,
        }
    }

    /// Adds a directory searched recursively for fonts.
    pub fn font_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.font_paths.push(path.into());
        self
    }

    /// Whether the system font directories are searched.
    pub fn system_fonts(mut self, enabled: bool) -> Self {
        self.system_fonts = enabled;
        self
    }

    /// Whether Typst's embedded fonts are offered (needs `embedded-fonts`).
    pub fn typst_embedded_fonts(mut self, enabled: bool) -> Self {
        self.typst_embedded_fonts = enabled;
        self
    }

    /// The local package directory (`namespace/name/version` layout) instead
    /// of the system one.
    pub fn package_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.package_path = Some(path.into());
        self
    }

    /// The package cache directory instead of the system one. It is read
    /// whether or not downloads are enabled, and receives downloads.
    pub fn package_cache_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.package_cache_path = Some(path.into());
        self
    }

    /// Downloads packages found in no directory from the Typst Universe
    /// registry, trusting `certificate` in addition to the bundled roots.
    #[cfg(feature = "egress")]
    pub fn download_packages(mut self, certificate: Option<PathBuf>) -> Self {
        self.registry = Some(Registry { certificate });
        self
    }

    /// The read limits of every filesystem read.
    pub fn limits(mut self, limits: FilesystemReadLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Whether packages are embedded in created packs (the default) or
    /// recorded as external requirements.
    pub fn vendor_packages(mut self, vendor: bool) -> Self {
        self.vendor_packages = vendor;
        self
    }

    /// Whether the system and font-path fonts a created pack uses are
    /// embedded in it.
    pub fn embed_fonts(mut self, embed: bool) -> Self {
        self.embed_fonts = embed;
        self
    }

    /// Whether embedding also covers Typst's embedded fonts.
    pub fn include_typst_embedded_fonts(mut self, include: bool) -> Self {
        self.include_typst_embedded_fonts = include;
        self
    }

    /// The document model discovery compiles.
    pub fn target(mut self, target: TypstTarget) -> Self {
        self.target = target;
        self
    }

    /// Inputs, document time, and features of the discovery compile.
    pub fn options(mut self, options: CompileOptions) -> Self {
        self.options = options;
        self
    }

    /// Adds a discovery variant; see [`CreateRequest::variants`].
    pub fn variant(mut self, inputs: Vec<(String, Input)>) -> Self {
        self.variants.push(inputs);
        self
    }

    /// Descriptive metadata recorded in created packs.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Creates a pack of the project beneath `root` whose entrypoint is
    /// `entrypoint`, absolute or relative to `root`.
    ///
    /// Every eligible file beneath the root is packed. Creation runs until it
    /// issues a pack: each package it reports missing is read from the
    /// package directories (or downloaded), and a package that cannot be read
    /// is recorded so the compile fails at the import that needs it.
    pub fn pack(
        &self,
        root: impl AsRef<Path>,
        entrypoint: impl AsRef<Path>,
    ) -> Result<Packed, FilesystemPackError> {
        let canonical = |path: &Path| {
            path.canonicalize()
                .map_err(|source| FilesystemPackError::Io {
                    path: path.to_owned(),
                    source,
                })
        };
        let root = canonical(root.as_ref())?;
        let entrypoint = canonical(&root.join(entrypoint))?;
        let entrypoint = VirtualPath::virtualize(&root, &entrypoint)
            .map_err(|_| FilesystemPackError::OutsideRoot(entrypoint.clone()))?;
        let entrypoint = entrypoint.get_without_slash();
        let project =
            read_filesystem_project(&root, entrypoint, self.limits.project).map_err(|error| {
                match &error {
                    FilesystemReadError::Project(invalid)
                        if invalid.issues().iter().any(|issue| {
                            matches!(issue, PackInvariantIssue::MissingEntrypoint { .. })
                        }) =>
                    {
                        FilesystemPackError::EntrypointIgnored(entrypoint.to_owned())
                    }
                    _ => FilesystemPackError::Project(error),
                }
            })?;

        let disposition = FontDisposition::embedded_if(self.embed_fonts);
        let typst_disposition =
            FontDisposition::embedded_if(self.embed_fonts && self.include_typst_embedded_fonts);
        let fonts = self.read_fonts(disposition, typst_disposition)?;

        let mut request = CreateRequest::new(project);
        request.fonts = fonts.catalog;
        request.target = self.target;
        request.options = self.options.clone();
        request.variants = self.variants.clone();
        request.metadata = self.metadata.clone();
        let disposition = if self.vendor_packages {
            PackageDisposition::Embedded
        } else {
            PackageDisposition::External
        };
        let packages = self.package_dirs();
        let mut attempted = BTreeSet::new();
        loop {
            let created = create(&request).map_err(|error| match error {
                CreateError::Failed {
                    errors,
                    warnings,
                    sources,
                } => CreateError::Failed {
                    errors,
                    warnings,
                    sources: sources.with_root(root.clone()),
                },
                error => error,
            });
            let missing = match created? {
                CreateOutcome::Created {
                    pack,
                    warnings,
                    sources,
                } => {
                    return Ok(Packed {
                        pack,
                        warnings,
                        sources: sources.with_root(root.clone()),
                        skipped_fonts: fonts.skipped,
                    });
                }
                CreateOutcome::MissingPackages(missing) => missing,
            };
            for spec in missing {
                // Creation reports neither supplied nor failed packages, so a
                // repeat would loop forever.
                if !attempted.insert(spec.to_string()) {
                    return Err(FilesystemPackError::RepeatedPackage(spec));
                }
                let read = packages.read(&spec).and_then(|found| {
                    request
                        .packages
                        .insert(spec.clone(), found.tree, disposition)
                        .map_err(|error| {
                            PackageReadFailure::new(
                                spec.clone(),
                                PackageReadFailureReason::Other {
                                    detail: Some(error.to_string()),
                                },
                            )
                        })
                });
                if let Err(failure) = read {
                    request.package_failures.insert(failure);
                }
            }
        }
    }

    /// Reads the packages and font containers a pack requires externally.
    ///
    /// A package found nowhere and a font container no font source holds are
    /// left out, for [`compile`](crate::compile) to report as unfulfilled. A
    /// package that is found but cannot be read is an error.
    pub fn resolve(&self, pack: &Pack) -> Result<Resolved, FilesystemPackError> {
        let mut resolved = Resolved::default();
        let packages = self.package_dirs();
        for requirement in pack.package_requirements().iter().filter(|r| !r.embedded) {
            match packages.read(&requirement.spec) {
                Ok(found) => {
                    if let Some(root) = found.root {
                        resolved
                            .package_dirs
                            .insert(requirement.spec.to_string(), root);
                    }
                    resolved
                        .packages
                        .push((requirement.spec.clone(), found.tree));
                }
                Err(failure) if matches!(failure.reason(), PackageReadFailureReason::NotFound) => {}
                Err(failure) => return Err(FilesystemPackError::Package(failure)),
            }
        }
        let required = pack
            .font_requirements()
            .iter()
            .filter(|requirement| !requirement.embedded)
            .map(|requirement| requirement.digest)
            .collect::<BTreeSet<_>>();
        if !required.is_empty() {
            let fonts = self.read_fonts(FontDisposition::External, FontDisposition::External)?;
            let mut seen = BTreeSet::new();
            for entry in fonts.catalog.entries() {
                let digest = entry.container().digest();
                if required.contains(&digest) && seen.insert(digest) {
                    resolved.fonts.push(entry.container().clone());
                }
            }
            resolved.skipped_fonts = fonts.skipped;
        }
        Ok(resolved)
    }

    fn package_dirs(&self) -> PackageDirs {
        PackageDirs::new(
            self.package_path.as_deref(),
            self.package_cache_path.as_deref(),
            self.registry.clone(),
            self.limits.packages,
        )
    }

    fn read_fonts(
        &self,
        disposition: FontDisposition,
        typst_disposition: FontDisposition,
    ) -> Result<crate::FilesystemFonts, FilesystemPackError> {
        let mut sources = Vec::new();
        if self.system_fonts {
            sources.push(FilesystemFontSource::system(disposition));
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded_fonts {
            sources.push(FilesystemFontSource::typst_embedded(typst_disposition));
        }
        #[cfg(not(feature = "embedded-fonts"))]
        let _ = (typst_disposition, self.typst_embedded_fonts);
        sources.extend(
            self.font_paths
                .iter()
                .map(|path| FilesystemFontSource::directory(path, disposition)),
        );
        read_filesystem_fonts(sources, self.limits.fonts).map_err(FilesystemPackError::Fonts)
    }
}
