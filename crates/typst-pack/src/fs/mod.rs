//! Packing project directories, resolving a pack's external requirements
//! from the host, and writing extractions and artifacts to the filesystem.
//!
//! The adapter reads and the core transforms: [`Packer`] reads the project,
//! the host's fonts, and the packages creation reports missing, and creation
//! and compilation run in the core over those bytes.
//!
//! Reading is bounded by [`ReadLimits`] and follows symbolic links only when
//! they resolve beneath the root being read, except in font directories, which
//! are host configuration. These checks are validation, not confinement
//! against a concurrent local peer that rewrites the tree while it is read.

mod fonts;
mod packages;
mod project;
mod walk;
mod write;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use typst::diag::SourceDiagnostic;
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;

use self::fonts::{FontSource, read_fonts};
use self::packages::PackageDirs;
pub use self::packages::read_package;
pub use self::project::{IGNORE_FILE, read_project};
pub use self::write::{
    WriteError, WriteIssue, WritePolicy, write_artifacts, write_extraction, write_file,
};
use crate::compile::{DocumentTime, Input, Options};
use crate::create::{self, Outcome, Target};
use crate::{
    LimitExceeded, Metadata, Pack, PackError, PackIssue, ReadLimits, Sources, font, package,
};

/// A failed filesystem read, pack, or resolution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A path could not be read or opened.
    #[error("cannot read `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A symbolic link in a project or package leads outside its root.
    #[error("symbolic link `{}` resolves outside `{}`", path.display(), root.display())]
    EscapingSymlink { path: PathBuf, root: PathBuf },
    /// A symbolic link leads back into a directory being read.
    #[error("symbolic link `{}` leads back to a directory it is inside of", path.display())]
    SymlinkCycle { path: PathBuf },
    /// An entry is a FIFO, socket, device, or other special file.
    #[error(
        "unsupported filesystem entry `{}`: only regular files and directories can be read",
        path.display()
    )]
    Unsupported { path: PathBuf },
    /// A project or package path is not UTF-8.
    #[error("path `{}` is not valid UTF-8", path.display())]
    NonUtf8Path { path: PathBuf },
    /// A read limit was exceeded.
    #[error("cannot read `{}`: {source}", path.display())]
    Limit {
        path: PathBuf,
        #[source]
        source: LimitExceeded,
    },
    /// The project's ignore file cannot be parsed.
    #[error("invalid `{}`: {message}", path.display())]
    InvalidIgnoreFile { path: PathBuf, message: String },
    /// A font file holds no readable face.
    #[error("`{}` is not a readable font", path.display())]
    InvalidFont { path: PathBuf },
    /// The files read do not form a valid project or package tree.
    #[error(transparent)]
    Invalid(#[from] PackError),
    /// The entrypoint is outside the project root.
    #[error("`{}` is outside the project root and cannot be packed", .0.display())]
    OutsideRoot(PathBuf),
    /// The ignore file excludes the entrypoint.
    #[error("entrypoint `{0}` is excluded by the project's {IGNORE_FILE}")]
    EntrypointIgnored(String),
    /// A package a pack requires externally was found but cannot be read.
    #[error("failed to read {spec}: {reason}")]
    Package {
        spec: PackageSpec,
        reason: package::ReadFailure,
    },
    /// Creation reported a package missing again after it was supplied.
    #[error("creation requested package {0} again after it was supplied")]
    RepeatedPackage(PackageSpec),
    /// Creation failed.
    #[error(transparent)]
    Create(#[from] create::Error),
}

/// Creates packs from project directories, and resolves the external
/// requirements of packs, using the host's fonts and package directories.
///
/// Fonts come from the system font directories, Typst's embedded fonts (with
/// the `embedded-fonts` feature), and [`font_paths`](Self::font_paths), in
/// that order. Packages come from the local package directory, then the
/// package cache, then, only with `download_packages` (and the `egress`
/// feature), the Typst Universe registry.
///
/// ```no_run
/// use typst_pack::{archive, fs};
///
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let packer = fs::Packer {
///     embed_fonts: true,
///     ..fs::Packer::default()
/// };
/// let packed = packer.pack("path/to/project", "main.typ")?;
/// for warning in &packed.warnings {
///     eprintln!("warning: {}", warning.message);
/// }
/// archive::save("project.typk", &packed.pack, fs::WritePolicy::CreateNew)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Packer {
    /// Directories searched recursively for fonts.
    pub font_paths: Vec<PathBuf>,
    /// Whether the system font directories are searched.
    pub system_fonts: bool,
    /// Whether Typst's embedded fonts are offered (needs `embedded-fonts`).
    pub typst_embedded_fonts: bool,
    /// The local package directory (`namespace/name/version` layout) instead
    /// of the system one.
    pub package_path: Option<PathBuf>,
    /// The package cache directory instead of the system one. It is read
    /// whether or not downloads are enabled, and receives downloads.
    pub package_cache_path: Option<PathBuf>,
    /// Whether packages found in no directory are downloaded from the Typst
    /// Universe registry.
    #[cfg(feature = "egress")]
    pub download_packages: bool,
    /// A PEM file of certificates trusted for downloads in addition to the
    /// bundled roots.
    #[cfg(feature = "egress")]
    pub certificate: Option<PathBuf>,
    /// Limits for reading the project directory.
    pub project_limits: ReadLimits,
    /// Limits for reading each package tree.
    pub package_limits: ReadLimits,
    /// Limits shared by every font directory of one read.
    pub font_limits: ReadLimits,
    /// Whether created packs embed their packages (the default) or record
    /// them as external requirements.
    pub vendor_packages: bool,
    /// Whether created packs embed the system and font-path fonts they use.
    pub embed_fonts: bool,
    /// Whether embedding also covers Typst's embedded fonts.
    pub include_typst_embedded_fonts: bool,
    /// The document model discovery compiles.
    pub target: Target,
    /// Inputs, document time, and features of the discovery compile.
    pub options: Options,
    /// Input overrides for additional discovery compiles; see
    /// [`create::Request::variants`].
    pub variants: Vec<Vec<(String, Input)>>,
    /// Descriptive metadata recorded in created packs.
    pub metadata: Option<Metadata>,
}

impl Default for Packer {
    /// System and Typst fonts, the host's package directories without
    /// downloads, vendored packages, external fonts, the paged target, and the
    /// host's current time as document time.
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i64);
        Self {
            font_paths: Vec::new(),
            system_fonts: true,
            typst_embedded_fonts: true,
            package_path: None,
            package_cache_path: None,
            #[cfg(feature = "egress")]
            download_packages: false,
            #[cfg(feature = "egress")]
            certificate: None,
            project_limits: ReadLimits {
                max_entries: 1_000_000,
                max_file_bytes: 256 * MIB,
                max_total_bytes: 2048 * MIB,
            },
            package_limits: ReadLimits {
                max_entries: 100_000,
                max_file_bytes: 64 * MIB,
                max_total_bytes: 512 * MIB,
            },
            // System font directories routinely hold gigabytes of collections.
            font_limits: ReadLimits {
                max_entries: 100_000,
                max_file_bytes: 256 * MIB,
                max_total_bytes: 2048 * MIB,
            },
            vendor_packages: true,
            embed_fonts: false,
            include_typst_embedded_fonts: false,
            target: Target::Paged,
            options: Options {
                document_time: DocumentTime::UnixTimestamp(now),
                ..Options::default()
            },
            variants: Vec::new(),
            metadata: None,
        }
    }
}

/// A pack created from a project directory.
#[derive(Debug)]
pub struct Packed {
    /// The pack.
    pub pack: Pack,
    /// Warnings of the discovery compiles.
    pub warnings: Vec<SourceDiagnostic>,
    /// The sources the discovery compiles read; project files are named
    /// relative to the current directory.
    pub sources: Sources,
    /// Font files and directories that could not be read and were left out.
    pub skipped_fonts: Vec<Error>,
}

/// The external requirements of a pack found on the filesystem, ready for
/// [`compile::Request::packages`](crate::compile::Request::packages) and
/// [`compile::Request::fonts`](crate::compile::Request::fonts).
#[derive(Debug, Default)]
pub struct Resolved {
    /// The trees of the external packages found.
    pub packages: Vec<(PackageSpec, package::Tree)>,
    /// The external font containers found.
    pub fonts: Vec<font::Container>,
    /// The directory each package was read from, for dependency reporting.
    pub package_dirs: BTreeMap<String, PathBuf>,
    /// Font files and directories that could not be read and were left out.
    pub skipped_fonts: Vec<Error>,
}

impl Packer {
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
    ) -> Result<Packed, Error> {
        let canonical = |path: &Path| {
            path.canonicalize().map_err(|source| Error::Io {
                path: path.to_owned(),
                source,
            })
        };
        let root = canonical(root.as_ref())?;
        let entrypoint = canonical(&root.join(entrypoint))?;
        let entrypoint = VirtualPath::virtualize(&root, &entrypoint)
            .map_err(|_| Error::OutsideRoot(entrypoint.clone()))?;
        let entrypoint = entrypoint.get_without_slash();
        let project =
            read_project(&root, entrypoint, self.project_limits).map_err(|error| match &error {
                Error::Invalid(invalid)
                    if invalid
                        .issues
                        .iter()
                        .any(|issue| matches!(issue, PackIssue::MissingEntrypoint { .. })) =>
                {
                    Error::EntrypointIgnored(entrypoint.to_owned())
                }
                _ => error,
            })?;

        let (fonts, skipped_fonts) = self.read_fonts(
            self.embed_fonts,
            self.embed_fonts && self.include_typst_embedded_fonts,
        )?;
        let mut request = create::Request::new(project);
        request.fonts = fonts;
        request.target = self.target;
        request.options = self.options.clone();
        request.variants = self.variants.clone();
        request.metadata = self.metadata.clone();
        let packages = self.package_dirs();
        let mut attempted = BTreeSet::new();
        loop {
            let created = create::create(&request).map_err(|error| match error {
                create::Error::Failed {
                    errors,
                    warnings,
                    sources,
                } => create::Error::Failed {
                    errors,
                    warnings,
                    sources: sources.with_root(root.clone()),
                },
                error => error,
            });
            let missing = match created? {
                Outcome::Created(created) => {
                    return Ok(Packed {
                        pack: created.pack,
                        warnings: created.warnings,
                        sources: created.sources.with_root(root.clone()),
                        skipped_fonts,
                    });
                }
                Outcome::MissingPackages(missing) => missing,
            };
            for spec in missing {
                // Creation reports neither supplied nor failed packages, so a
                // repeat would loop forever.
                if !attempted.insert(spec.to_string()) {
                    return Err(Error::RepeatedPackage(spec));
                }
                let read = packages.read(&spec).and_then(|found| {
                    request
                        .packages
                        .insert(spec.clone(), found.tree, self.vendor_packages)
                        .map_err(|error| package::ReadFailure::Other(Some(error.to_string())))
                });
                if let Err(failure) = read {
                    request.package_failures.insert(spec, failure);
                }
            }
        }
    }

    /// Reads the packages and font containers a pack requires externally.
    ///
    /// A package found nowhere and a font container no font source holds are
    /// left out, for [`compile()`](crate::compile()) to report as
    /// unfulfilled. A package that is found but cannot be read is an error.
    pub fn resolve(&self, pack: &Pack) -> Result<Resolved, Error> {
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
                Err(package::ReadFailure::NotFound) => {}
                Err(reason) => {
                    return Err(Error::Package {
                        spec: requirement.spec.clone(),
                        reason,
                    });
                }
            }
        }
        let required = pack
            .font_requirements()
            .iter()
            .filter(|requirement| !requirement.embedded)
            .map(|requirement| requirement.digest)
            .collect::<BTreeSet<_>>();
        if !required.is_empty() {
            let (fonts, skipped) = self.read_fonts(false, false)?;
            let mut seen = BTreeSet::new();
            for entry in fonts {
                let digest = entry.container.digest();
                if required.contains(&digest) && seen.insert(digest) {
                    resolved.fonts.push(entry.container);
                }
            }
            resolved.skipped_fonts = skipped;
        }
        Ok(resolved)
    }

    fn package_dirs(&self) -> PackageDirs {
        PackageDirs::new(
            self.package_path.as_deref(),
            self.package_cache_path.as_deref(),
            #[cfg(feature = "egress")]
            self.download_packages
                .then(|| packages::Registry::new(self.certificate.clone())),
            self.package_limits,
        )
    }

    fn read_fonts(
        &self,
        embed: bool,
        embed_typst: bool,
    ) -> Result<(Vec<font::Entry>, Vec<Error>), Error> {
        let mut sources = Vec::new();
        if self.system_fonts {
            sources.push(FontSource::System { embedded: embed });
        }
        #[cfg(feature = "embedded-fonts")]
        if self.typst_embedded_fonts {
            sources.push(FontSource::Typst {
                embedded: embed_typst,
            });
        }
        #[cfg(not(feature = "embedded-fonts"))]
        let _ = embed_typst;
        sources.extend(self.font_paths.iter().map(|path| FontSource::Directory {
            path: path.clone(),
            embedded: embed,
        }));
        read_fonts(sources, self.font_limits)
    }
}
