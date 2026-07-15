//! The in-memory pack model and its archive serialization.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek, Write};
use std::str::FromStr;

use typst::foundations::Bytes;
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;
use typst::text::FontInfo;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::manifest::{
    FORMAT_VERSION, FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata,
    PackagesManifest, ProjectManifest,
};

/// The conventional file extension for packs.
pub const FILE_EXTENSION: &str = "typk";

const PROJECT_PREFIX: &str = "project/";
const PACKAGES_PREFIX: &str = "packages/";

/// A portable pack of a Typst project.
///
/// A pack holds project files (sources, images, and data files), optionally
/// package files and fonts, and the declared paths of any External Project
/// Resources whose bytes are supplied externally when requested. Its archive form is
/// a Zip file with a `typst-pack.toml` manifest, conventionally named `*.typk`.
#[derive(Debug, Clone)]
pub struct Pack {
    manifest: PackManifest,
    files: BTreeMap<String, Bytes>,
    /// Vendored packages, keyed by spec string for deterministic order.
    packages: BTreeMap<String, PackageFiles>,
    fonts: Vec<PackFont>,
}

#[derive(Debug, Clone)]
struct PackageFiles {
    spec: PackageSpec,
    files: BTreeMap<String, Bytes>,
}

/// A font embedded in a pack.
#[derive(Debug, Clone)]
pub struct PackFont {
    /// The manifest entry describing this font.
    pub entry: FontManifest,
    /// The raw font file data.
    pub data: Bytes,
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder::new(entrypoint)
    }

    /// The pack manifest.
    pub fn manifest(&self) -> &PackManifest {
        &self.manifest
    }

    /// The root-relative path of the entrypoint file.
    pub fn entrypoint(&self) -> &str {
        &self.manifest.project.entrypoint
    }

    /// The project files, keyed by root-relative path.
    pub fn files(&self) -> impl Iterator<Item = (&str, &Bytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    /// Looks up a project file by root-relative path.
    pub fn file(&self, path: &str) -> Option<&Bytes> {
        self.files.get(path)
    }

    /// The root-relative paths of resources supplied externally at compilation time.
    pub fn external_resources(&self) -> impl Iterator<Item = &str> {
        self.manifest
            .project
            .external_resources
            .iter()
            .map(String::as_str)
    }

    pub(crate) fn is_external_resource(&self, path: &str) -> bool {
        self.manifest.project.external_resources.contains(path)
    }

    /// The vendored packages and their files.
    pub fn packages(
        &self,
    ) -> impl Iterator<Item = (&PackageSpec, impl Iterator<Item = (&str, &Bytes)>)> {
        self.packages.values().map(|package| {
            (
                &package.spec,
                package
                    .files
                    .iter()
                    .map(|(path, data)| (path.as_str(), data)),
            )
        })
    }

    /// Looks up a vendored package file.
    pub fn package_file(&self, spec: &PackageSpec, path: &str) -> Option<&Bytes> {
        self.packages.get(&spec.to_string())?.files.get(path)
    }

    /// Whether the pack vendors the given package.
    pub fn has_package(&self, spec: &PackageSpec) -> bool {
        self.packages.contains_key(&spec.to_string())
    }

    /// The fonts embedded in the pack.
    pub fn fonts(&self) -> &[PackFont] {
        &self.fonts
    }

    /// Reads a pack from a seekable reader.
    pub fn read<R: Read + Seek>(reader: R) -> Result<Self, PackReadError> {
        let mut archive = ZipArchive::new(reader)?;

        let manifest = {
            let mut entry = archive
                .by_name(MANIFEST_PATH)
                .map_err(|_| PackReadError::MissingManifest)?;
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            PackManifest::from_toml(&text)?
        };

        let mut files = BTreeMap::new();
        let mut packages: BTreeMap<String, PackageFiles> = BTreeMap::new();
        let mut fonts_by_path: BTreeMap<String, Bytes> = BTreeMap::new();
        let font_paths: Vec<&str> = manifest
            .fonts
            .iter()
            .map(|font| font.path.as_str())
            .collect();

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.is_dir() {
                continue;
            }
            let Some(name) = entry.enclosed_name() else {
                return Err(PackReadError::UnsafeEntry(entry.name().to_owned()));
            };
            let name = name.to_string_lossy().replace('\\', "/");

            if name == MANIFEST_PATH {
                continue;
            } else if let Some(path) = name.strip_prefix(PROJECT_PREFIX) {
                let path = normalize_path(path, &name)?;
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                files.insert(path, Bytes::new(data));
            } else if let Some(rest) = name.strip_prefix(PACKAGES_PREFIX) {
                let (spec, path) = split_package_entry(rest, &name)?;
                let key = spec.to_string();
                if !manifest.packages.vendored.contains(&key) {
                    return Err(PackReadError::UndeclaredPackage(key));
                }
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                packages
                    .entry(key)
                    .or_insert_with(|| PackageFiles {
                        spec,
                        files: BTreeMap::new(),
                    })
                    .files
                    .insert(path, Bytes::new(data));
            } else if font_paths.contains(&name.as_str()) {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                fonts_by_path
                    .entry(name)
                    .or_insert_with(|| Bytes::new(data));
            }
            // Unknown top-level entries are ignored for forward compatibility.
        }

        if let Some(path) = manifest
            .project
            .external_resources
            .iter()
            .find(|path| files.contains_key(path.as_str()))
        {
            return Err(PackReadError::ExternalResourceConflict(path.clone()));
        }

        if !files.contains_key(&manifest.project.entrypoint) {
            return Err(PackReadError::MissingEntrypoint(
                manifest.project.entrypoint.clone(),
            ));
        }

        for spec in manifest.vendored_packages()? {
            if !packages.contains_key(&spec.to_string()) {
                return Err(PackReadError::MissingPackage(spec.to_string()));
            }
        }

        let mut fonts = Vec::new();
        for entry in &manifest.fonts {
            let data = fonts_by_path
                .get(&entry.path)
                .cloned()
                .ok_or_else(|| PackReadError::MissingFont(entry.path.clone()))?;
            fonts.push(PackFont {
                entry: entry.clone(),
                data,
            });
        }

        Ok(Self {
            manifest,
            files,
            packages,
            fonts,
        })
    }

    /// Reads a pack from a byte buffer.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self, PackReadError> {
        Self::read(Cursor::new(bytes.into()))
    }

    /// Writes the pack archive to a seekable writer.
    pub fn write<W: Write + Seek>(&self, writer: W) -> Result<(), PackWriteError> {
        let mut zip = ZipWriter::new(writer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file(MANIFEST_PATH, options)?;
        zip.write_all(self.manifest.to_toml().as_bytes())?;

        for (path, data) in &self.files {
            zip.start_file(format!("{PROJECT_PREFIX}{path}"), options)?;
            zip.write_all(data)?;
        }

        for package in self.packages.values() {
            let spec = &package.spec;
            for (path, data) in &package.files {
                zip.start_file(
                    format!(
                        "{PACKAGES_PREFIX}{}/{}/{}/{path}",
                        spec.namespace, spec.name, spec.version
                    ),
                    options,
                )?;
                zip.write_all(data)?;
            }
        }

        let mut written = std::collections::BTreeSet::new();
        for font in &self.fonts {
            if written.insert(&font.entry.path) {
                zip.start_file(&font.entry.path, options)?;
                zip.write_all(&font.data)?;
            }
        }

        zip.finish()?;
        Ok(())
    }

    /// Serializes the pack archive to a byte buffer.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PackWriteError> {
        let mut buffer = Cursor::new(Vec::new());
        self.write(&mut buffer)?;
        Ok(buffer.into_inner())
    }
}

/// Normalizes an archive path into a root-relative virtual path string.
fn normalize_path(path: &str, entry: &str) -> Result<String, PackReadError> {
    match VirtualPath::new(path) {
        Ok(vpath) => Ok(vpath.get_without_slash().to_owned()),
        Err(err) => Err(PackReadError::InvalidEntry {
            entry: entry.to_owned(),
            message: err.to_string(),
        }),
    }
}

/// Splits `namespace/name/version/rest...` into a package spec and file path.
fn split_package_entry(rest: &str, entry: &str) -> Result<(PackageSpec, String), PackReadError> {
    let mut parts = rest.splitn(4, '/');
    let (Some(namespace), Some(name), Some(version), Some(path)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(PackReadError::InvalidEntry {
            entry: entry.to_owned(),
            message: "expected packages/<namespace>/<name>/<version>/<path>".into(),
        });
    };
    let spec = PackageSpec::from_str(&format!("@{namespace}/{name}:{version}")).map_err(|err| {
        PackReadError::InvalidEntry {
            entry: entry.to_owned(),
            message: err.to_string(),
        }
    })?;
    let path = normalize_path(path, entry)?;
    Ok((spec, path))
}

/// A failure while reading a pack archive.
#[derive(Debug, thiserror::Error)]
pub enum PackReadError {
    #[error("failed to read archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("i/o error while reading archive: {0}")]
    Io(#[from] std::io::Error),
    #[error("the archive contains no {MANIFEST_PATH} manifest (is this a Typst pack?)")]
    MissingManifest,
    #[error(transparent)]
    Manifest(#[from] PackManifestError),
    #[error("archive entry `{0}` has an unsafe path")]
    UnsafeEntry(String),
    #[error("invalid archive entry `{entry}`: {message}")]
    InvalidEntry { entry: String, message: String },
    #[error("package `{0}` has files in the archive but is not declared in the manifest")]
    UndeclaredPackage(String),
    #[error("the manifest declares vendored package `{0}` but the archive has no files for it")]
    MissingPackage(String),
    #[error("entrypoint `{0}` is missing from the archive")]
    MissingEntrypoint(String),
    #[error("project path `{0}` cannot be both packed and an external project resource")]
    ExternalResourceConflict(String),
    #[error("font file `{0}` is declared in the manifest but missing from the archive")]
    MissingFont(String),
}

/// A failure while writing a pack archive.
#[derive(Debug, thiserror::Error)]
pub enum PackWriteError {
    #[error("failed to write archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("i/o error while writing archive: {0}")]
    Io(#[from] std::io::Error),
}

/// Builds a [`Pack`] from in-memory data.
///
/// This is the constructor to use when the project does not live on a file
/// system, for example in a web editor. For packing a project directory, use
/// `Packer` instead (requires the `fs` feature).
#[derive(Debug)]
pub struct PackBuilder {
    entrypoint: String,
    files: BTreeMap<String, Bytes>,
    external_resources: BTreeSet<String>,
    packages: BTreeMap<String, PackageFiles>,
    unvendored_packages: Vec<PackageSpec>,
    fonts: Vec<PackFont>,
    metadata: Option<PackMetadata>,
}

impl PackBuilder {
    /// Creates a builder for a pack with the given entrypoint path.
    pub fn new(entrypoint: impl Into<String>) -> Self {
        Self {
            entrypoint: entrypoint.into(),
            files: BTreeMap::new(),
            external_resources: BTreeSet::new(),
            packages: BTreeMap::new(),
            unvendored_packages: Vec::new(),
            fonts: Vec::new(),
            metadata: None,
        }
    }

    /// Adds a project file under a root-relative path.
    pub fn file(
        mut self,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        let path = valid_path(path.as_ref())?;
        self.files.insert(path, Bytes::new(data.into()));
        Ok(self)
    }

    /// Declares a non-source project resource whose bytes will be supplied externally.
    pub fn external_resource(mut self, path: impl AsRef<str>) -> Result<Self, PackBuildError> {
        self.external_resources.insert(valid_path(path.as_ref())?);
        Ok(self)
    }

    /// Adds a file of a vendored package.
    pub fn package_file(
        mut self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        let path = valid_path(path.as_ref())?;
        self.packages
            .entry(spec.to_string())
            .or_insert_with(|| PackageFiles {
                spec,
                files: BTreeMap::new(),
            })
            .files
            .insert(path, Bytes::new(data.into()));
        Ok(self)
    }

    /// Records a package dependency that is intentionally not vendored.
    pub fn unvendored_package(mut self, spec: PackageSpec) -> Self {
        if !self.unvendored_packages.contains(&spec) {
            self.unvendored_packages.push(spec);
        }
        self
    }

    /// Embeds a font file.
    ///
    /// `index` is the face index for font collections and zero otherwise. The
    /// entry name and family list are derived from the font data.
    pub fn font(mut self, data: impl Into<Vec<u8>>, index: u32) -> Result<Self, PackBuildError> {
        let data = data.into();
        let info = FontInfo::new(&data, index).ok_or(PackBuildError::UnrecognizedFont)?;
        let family = info.family.to_string();
        let path = self.font_path(&family, &data);
        self.fonts.push(PackFont {
            entry: FontManifest {
                path,
                index,
                families: vec![family],
            },
            data: Bytes::new(data),
        });
        Ok(self)
    }

    /// Sets descriptive metadata.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Finishes the pack.
    pub fn build(self) -> Result<Pack, PackBuildError> {
        let entrypoint = valid_path(&self.entrypoint)?;
        if !self.files.contains_key(&entrypoint) {
            return Err(PackBuildError::MissingEntrypoint(entrypoint));
        }
        if let Some(path) = self
            .external_resources
            .iter()
            .find(|path| self.files.contains_key(path.as_str()))
        {
            return Err(PackBuildError::ExternalResourceConflict(path.clone()));
        }

        let manifest = PackManifest {
            format_version: FORMAT_VERSION,
            project: ProjectManifest {
                entrypoint,
                external_resources: self.external_resources,
            },
            packages: PackagesManifest {
                vendored: self.packages.keys().cloned().collect(),
                unvendored: self
                    .unvendored_packages
                    .iter()
                    .map(|spec| spec.to_string())
                    .collect(),
            },
            fonts: self.fonts.iter().map(|font| font.entry.clone()).collect(),
            metadata: self.metadata,
        };

        Ok(Pack {
            manifest,
            files: self.files,
            packages: self.packages,
            fonts: self.fonts,
        })
    }

    /// Picks a unique archive path for a font file.
    fn font_path(&self, family: &str, data: &[u8]) -> String {
        let extension = match data.get(..4) {
            Some(b"OTTO") => "otf",
            Some(b"ttcf") => "ttc",
            _ => "ttf",
        };
        let stem: String = family
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let stem = stem.trim_matches('-');
        let stem = if stem.is_empty() { "font" } else { stem };

        // Reuse the path if the identical file is already embedded (e.g.
        // several faces of one collection), otherwise disambiguate.
        let mut candidate = format!("fonts/{stem}.{extension}");
        let mut counter = 1;
        loop {
            match self.fonts.iter().find(|font| font.entry.path == candidate) {
                None => return candidate,
                Some(existing) if existing.data.as_slice() == data => return candidate,
                Some(_) => {
                    counter += 1;
                    candidate = format!("fonts/{stem}-{counter}.{extension}");
                }
            }
        }
    }
}

pub(crate) fn valid_path(path: &str) -> Result<String, PackBuildError> {
    match VirtualPath::new(path) {
        Ok(vpath) => Ok(vpath.get_without_slash().to_owned()),
        Err(err) => Err(PackBuildError::InvalidPath {
            path: path.to_owned(),
            message: err.to_string(),
        }),
    }
}

/// A failure while building a pack in memory.
#[derive(Debug, thiserror::Error)]
pub enum PackBuildError {
    #[error("invalid project path `{path}`: {message}")]
    InvalidPath { path: String, message: String },
    #[error("entrypoint `{0}` was not added as a file")]
    MissingEntrypoint(String),
    #[error("project path `{0}` cannot be both packed and an external project resource")]
    ExternalResourceConflict(String),
    #[error("font data could not be parsed as a font")]
    UnrecognizedFont,
}
