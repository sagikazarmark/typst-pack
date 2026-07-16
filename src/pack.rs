//! The in-memory pack model and its archive serialization.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::str::FromStr;

use typst::foundations::Bytes;
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;
use typst::text::{Font, FontInfo};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::manifest::{FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata};

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
    files: BTreeMap<CanonicalPath, Bytes>,
    /// Vendored packages, keyed by spec string for deterministic order.
    packages: BTreeMap<String, PackageFiles>,
    fonts: Vec<PackFont>,
}

#[derive(Debug, Clone)]
struct PackageFiles {
    spec: PackageSpec,
    files: BTreeMap<CanonicalPath, Bytes>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalPath(String);

impl CanonicalPath {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn into_string(self) -> String {
        self.0
    }
}

impl Borrow<str> for CanonicalPath {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for CanonicalPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A font embedded in a pack.
#[derive(Debug, Clone)]
pub struct PackFont {
    /// The manifest entry describing this font.
    entry: FontManifest,
    /// The raw font file data.
    data: Bytes,
    font: Font,
}

impl PackFont {
    /// The declaration describing this font face.
    pub fn manifest(&self) -> &FontManifest {
        &self.entry
    }

    /// The contained font bytes.
    pub fn data(&self) -> &Bytes {
        &self.data
    }

    pub(crate) fn font(&self) -> &Font {
        &self.font
    }
}

#[derive(Debug, Clone)]
struct PackFontInput {
    entry: FontManifest,
    data: Bytes,
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder::new(entrypoint)
    }

    fn construct(
        manifest: PackManifest,
        files: BTreeMap<CanonicalPath, Bytes>,
        packages: BTreeMap<String, PackageFiles>,
        font_data: BTreeMap<CanonicalPath, Bytes>,
    ) -> Result<Self, PackInvariantError> {
        let entrypoint = canonical_path(PackPathRole::Entrypoint, manifest.project().entrypoint())?;
        let canonical_files = files;
        let external_resources = manifest
            .project()
            .external_resources()
            .map(|path| canonical_path(PackPathRole::ExternalProjectResource, path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let font_entries = manifest
            .fonts()
            .iter()
            .cloned()
            .map(|entry| Ok((canonical_path(PackPathRole::FontData, entry.path())?, entry)))
            .collect::<Result<Vec<_>, PackInvariantError>>()?;

        let vendored_packages = manifest
            .vendored_packages()
            .iter()
            .cloned()
            .map(|spec| (spec.to_string(), spec))
            .collect::<BTreeMap<_, _>>();
        let external_packages = manifest
            .unvendored_packages()
            .iter()
            .cloned()
            .map(|spec| (spec.to_string(), spec))
            .collect::<BTreeMap<_, _>>();
        if let Some(spec) = vendored_packages
            .keys()
            .find(|spec| external_packages.contains_key(*spec))
        {
            return Err(PackInvariantError::PackageRoleConflict(spec.clone()));
        }

        validate_project_declarations(canonical_files.keys().cloned(), &external_resources)?;

        for package in packages.values() {
            let paths = package
                .files
                .keys()
                .cloned()
                .map(|path| (path, PackPathRole::PackageFile))
                .collect();
            if let Some((ancestor, ancestor_role, descendant, descendant_role)) =
                find_path_tree_conflict(paths)
            {
                return Err(PackInvariantError::PackagePathTreeConflict {
                    package: package.spec.to_string(),
                    ancestor: ancestor.to_string(),
                    ancestor_role,
                    descendant: descendant.to_string(),
                    descendant_role,
                });
            }
        }

        for (path, _) in &font_entries {
            if let Some(conflicting_role) = reserved_font_path_role(path) {
                return Err(PackInvariantError::ReservedFontPath {
                    path: path.to_string(),
                    conflicting_role,
                });
            }
        }
        let font_paths = font_entries
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|path| (path, PackPathRole::FontData))
            .collect();
        if let Some((ancestor, ancestor_role, descendant, descendant_role)) =
            find_path_tree_conflict(font_paths)
        {
            return Err(PackInvariantError::PathTreeConflict {
                ancestor: ancestor.to_string(),
                ancestor_role,
                descendant: descendant.to_string(),
                descendant_role,
            });
        }

        if external_resources.contains(&entrypoint) {
            return Err(PackInvariantError::EntrypointIsExternalResource(
                entrypoint.to_string(),
            ));
        }
        if !canonical_files.contains_key(&entrypoint) {
            return Err(PackInvariantError::MissingEntrypoint(
                entrypoint.to_string(),
            ));
        }

        let mut canonical_packages = BTreeMap::new();
        for (_, package) in packages {
            let key = package.spec.to_string();
            if !vendored_packages.contains_key(&key) {
                return Err(PackInvariantError::UndeclaredPackageData(key));
            }
            let package_files = package.files;
            canonical_packages.insert(
                key,
                PackageFiles {
                    spec: package.spec,
                    files: package_files,
                },
            );
        }
        if let Some(spec) = vendored_packages
            .keys()
            .find(|spec| !canonical_packages.contains_key(*spec))
        {
            return Err(PackInvariantError::MissingVendoredPackageData(spec.clone()));
        }

        let mut canonical_fonts = Vec::new();
        let mut font_faces = BTreeSet::new();
        for (path, entry) in font_entries {
            let index = entry.index();
            if !font_faces.insert((path.clone(), index)) {
                return Err(PackInvariantError::DuplicateFontFace {
                    path: path.to_string(),
                    index,
                });
            }
            let data = font_data
                .get(&path)
                .cloned()
                .ok_or_else(|| PackInvariantError::MissingFontData(path.to_string()))?;
            let parsed = Font::new(data.clone(), index).ok_or_else(|| {
                PackInvariantError::InvalidFontData {
                    path: path.to_string(),
                    index,
                }
            })?;
            canonical_fonts.push(PackFont {
                entry: FontManifest::new(path.into_string(), index, entry.families().to_vec()),
                data,
                font: parsed,
            });
        }

        let manifest = PackManifest::new(
            entrypoint.into_string(),
            external_resources
                .into_iter()
                .map(CanonicalPath::into_string)
                .collect(),
            vendored_packages.into_values().collect(),
            external_packages.into_values().collect(),
            canonical_fonts
                .iter()
                .map(|font| font.entry.clone())
                .collect(),
            manifest.metadata().cloned(),
        );

        Ok(Self {
            manifest,
            files: canonical_files,
            packages: canonical_packages,
            fonts: canonical_fonts,
        })
    }

    /// The pack manifest.
    pub fn manifest(&self) -> &PackManifest {
        &self.manifest
    }

    /// The root-relative path of the entrypoint file.
    pub fn entrypoint(&self) -> &str {
        self.manifest.project().entrypoint()
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
        self.manifest.project().external_resources()
    }

    pub(crate) fn is_external_resource(&self, path: &str) -> bool {
        self.manifest.project().contains_external_resource(path)
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
        let archive = ZipArchive::new(reader)?;
        let retained_entry_count = archive.len();
        let central_directory_start = archive.central_directory_start();
        let mut reader = archive.into_inner();
        let raw_entries = raw_central_entries(&mut reader, central_directory_start)?;
        let mut raw_names = BTreeSet::new();
        for entry in &raw_entries {
            if !raw_names.insert(entry.name.clone()) {
                if entry.name == MANIFEST_PATH.as_bytes() {
                    return Err(PackReadError::DuplicateManifest);
                }
                return Err(PackReadError::DuplicateArchiveEntry(entry.name.clone()));
            }
        }
        if raw_entries.len() != retained_entry_count {
            return Err(PackReadError::AmbiguousArchiveEntries);
        }
        let mut archive = ZipArchive::new(reader)?;

        struct ProjectEntry {
            index: usize,
            path: CanonicalPath,
        }
        struct PackageEntry {
            index: usize,
            spec: PackageSpec,
            path: CanonicalPath,
        }
        struct UnknownEntry {
            index: usize,
            archive_name: String,
            raw_name: Vec<u8>,
            canonical_name: String,
            regular_file: bool,
        }

        let mut manifest_entry = None;
        let mut project_entries = Vec::new();
        let mut package_entries = Vec::new();
        let mut unknown_entries = Vec::new();
        let mut canonical_archive_entries = BTreeMap::new();
        for (index, raw_entry) in raw_entries.iter().enumerate() {
            let entry = archive.by_index_raw(index)?;
            let archive_name = entry.name().to_owned();
            let raw_name = raw_entry.name.clone();
            let prefix_normalized_name = strip_current_directory_prefix(&archive_name);
            if archive_name.is_empty()
                || archive_name.starts_with('/')
                || archive_name.starts_with('\\')
                || archive_name.contains('\\')
                || has_windows_drive_prefix(prefix_normalized_name)
            {
                return Err(PackReadError::UnsafeEntry(archive_name));
            }
            let enclosed_name = entry
                .enclosed_name()
                .ok_or_else(|| PackReadError::UnsafeEntry(archive_name.clone()))?;
            let canonical_name = enclosed_name
                .to_str()
                .ok_or_else(|| PackReadError::UnsafeEntry(archive_name.clone()))?
                .to_owned();
            if has_windows_drive_prefix(&canonical_name) {
                return Err(PackReadError::UnsafeEntry(archive_name));
            }
            register_archive_identity(
                &mut canonical_archive_entries,
                canonical_name.clone(),
                &raw_name,
            )?;
            if entry.is_dir() {
                continue;
            }
            const FILE_TYPE_MASK: u32 = 0o170000;
            const REGULAR_FILE: u32 = 0o100000;
            let regular_file = entry.is_file()
                && entry
                    .unix_mode()
                    .is_none_or(|mode| matches!(mode & FILE_TYPE_MASK, 0 | REGULAR_FILE));
            let role_name = if prefix_normalized_name == MANIFEST_PATH
                || prefix_normalized_name.starts_with(PROJECT_PREFIX)
                || prefix_normalized_name.starts_with(PACKAGES_PREFIX)
            {
                prefix_normalized_name
            } else {
                canonical_name.as_str()
            };

            if role_name == MANIFEST_PATH {
                register_archive_identity(
                    &mut canonical_archive_entries,
                    MANIFEST_PATH.to_owned(),
                    &raw_name,
                )?;
                manifest_entry = Some((index, regular_file));
            } else if let Some(path) = role_name.strip_prefix(PROJECT_PREFIX) {
                if !regular_file {
                    return Err(PackReadError::UnsupportedEntryType(archive_name));
                }
                let path = canonical_path(PackPathRole::ProjectFile, path)?;
                register_archive_identity(
                    &mut canonical_archive_entries,
                    format!("{PROJECT_PREFIX}{path}"),
                    &raw_name,
                )?;
                project_entries.push(ProjectEntry { index, path });
            } else if let Some(rest) = role_name.strip_prefix(PACKAGES_PREFIX) {
                if !regular_file {
                    return Err(PackReadError::UnsupportedEntryType(archive_name));
                }
                let (spec, path) = split_package_entry(rest, &archive_name)?;
                register_archive_identity(
                    &mut canonical_archive_entries,
                    format!(
                        "{PACKAGES_PREFIX}{}/{}/{}/{path}",
                        spec.namespace, spec.name, spec.version
                    ),
                    &raw_name,
                )?;
                package_entries.push(PackageEntry { index, spec, path });
            } else {
                unknown_entries.push(UnknownEntry {
                    index,
                    archive_name,
                    raw_name,
                    canonical_name,
                    regular_file,
                });
            }
        }

        let (manifest_index, manifest_is_file) =
            manifest_entry.ok_or(PackReadError::MissingManifest)?;
        if !manifest_is_file {
            return Err(PackReadError::ManifestNotFile);
        }
        let manifest = {
            let mut entry = archive.by_index(manifest_index)?;
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(PackReadError::ManifestUnreadable)?;
            let text = std::str::from_utf8(&bytes).map_err(PackReadError::ManifestNotUtf8)?;
            PackManifest::from_toml(text)?
        };

        let font_paths = manifest
            .fonts()
            .iter()
            .filter_map(|font| canonical_path(PackPathRole::FontData, font.path()).ok())
            .collect::<BTreeSet<_>>();
        let mut font_entries = Vec::new();
        for entry in unknown_entries {
            if let Some(path) = font_paths.get(entry.canonical_name.as_str()) {
                if !entry.regular_file {
                    return Err(PackReadError::UnsupportedEntryType(entry.archive_name));
                }
                register_archive_identity(
                    &mut canonical_archive_entries,
                    path.to_string(),
                    &entry.raw_name,
                )?;
                font_entries.push((entry.index, path.clone()));
            }
        }

        let mut files = BTreeMap::new();
        for project in project_entries {
            let mut data = Vec::new();
            archive.by_index(project.index)?.read_to_end(&mut data)?;
            files.insert(project.path, Bytes::new(data));
        }
        let mut packages: BTreeMap<String, PackageFiles> = BTreeMap::new();
        for package in package_entries {
            let mut data = Vec::new();
            archive.by_index(package.index)?.read_to_end(&mut data)?;
            packages
                .entry(package.spec.to_string())
                .or_insert_with(|| PackageFiles {
                    spec: package.spec,
                    files: BTreeMap::new(),
                })
                .files
                .insert(package.path, Bytes::new(data));
        }
        let mut fonts_by_path = BTreeMap::new();
        for (index, path) in font_entries {
            let mut data = Vec::new();
            archive.by_index(index)?.read_to_end(&mut data)?;
            fonts_by_path.insert(path, Bytes::new(data));
        }

        Ok(Self::construct(manifest, files, packages, fonts_by_path)?)
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
            if written.insert(font.manifest().path()) {
                zip.start_file(font.manifest().path(), options)?;
                zip.write_all(font.data())?;
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

struct RawCentralEntry {
    name: Vec<u8>,
}

fn raw_central_entries<R: Read + Seek>(
    reader: &mut R,
    central_directory_start: u64,
) -> Result<Vec<RawCentralEntry>, PackReadError> {
    reader.seek(SeekFrom::Start(central_directory_start))?;
    let mut entries = Vec::new();
    loop {
        let header_start = reader.stream_position()?;
        let mut signature = [0; 4];
        reader.read_exact(&mut signature)?;
        if signature != *b"PK\x01\x02" {
            reader.seek(SeekFrom::Start(header_start))?;
            break;
        }

        let mut fixed = [0; 42];
        reader.read_exact(&mut fixed)?;
        let name_len = u16::from_le_bytes([fixed[24], fixed[25]]) as usize;
        let extra_len = u16::from_le_bytes([fixed[26], fixed[27]]) as i64;
        let comment_len = u16::from_le_bytes([fixed[28], fixed[29]]) as i64;
        let mut name = vec![0; name_len];
        reader.read_exact(&mut name)?;
        reader.seek(SeekFrom::Current(extra_len + comment_len))?;
        entries.push(RawCentralEntry { name });
    }
    Ok(entries)
}

/// Splits `namespace/name/version/rest...` into a package spec and file path.
fn split_package_entry(
    rest: &str,
    entry: &str,
) -> Result<(PackageSpec, CanonicalPath), PackReadError> {
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
    let path = canonical_path(PackPathRole::PackageFile, path)?;
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
    #[error("the archive contains more than one {MANIFEST_PATH} manifest")]
    DuplicateManifest,
    #[error("the archive contains a duplicate entry named {0:?}")]
    DuplicateArchiveEntry(Vec<u8>),
    #[error("the archive contains entries with ambiguous effective names")]
    AmbiguousArchiveEntries,
    #[error("the {MANIFEST_PATH} manifest is not a regular file")]
    ManifestNotFile,
    #[error("the {MANIFEST_PATH} manifest could not be read: {0}")]
    ManifestUnreadable(#[source] std::io::Error),
    #[error("the {MANIFEST_PATH} manifest is not valid UTF-8: {0}")]
    ManifestNotUtf8(#[source] std::str::Utf8Error),
    #[error(transparent)]
    Manifest(#[from] PackManifestError),
    #[error("archive entry `{0}` has an unsafe path")]
    UnsafeEntry(String),
    #[error("invalid archive entry `{entry}`: {message}")]
    InvalidEntry { entry: String, message: String },
    #[error("archive entry `{0}` is not a regular file")]
    UnsupportedEntryType(String),
    #[error(transparent)]
    Invariant(#[from] PackInvariantError),
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
    files: BTreeMap<CanonicalPath, Bytes>,
    external_resources: BTreeSet<CanonicalPath>,
    packages: BTreeMap<String, PackageFiles>,
    unvendored_packages: Vec<PackageSpec>,
    fonts: Vec<PackFontInput>,
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
        let path = canonical_path(PackPathRole::ProjectFile, path.as_ref())?;
        self.files.insert(path, Bytes::new(data.into()));
        Ok(self)
    }

    /// Declares a non-source project resource whose bytes will be supplied externally.
    pub fn external_resource(mut self, path: impl AsRef<str>) -> Result<Self, PackBuildError> {
        self.external_resources.insert(canonical_path(
            PackPathRole::ExternalProjectResource,
            path.as_ref(),
        )?);
        Ok(self)
    }

    #[cfg(feature = "fs")]
    pub(crate) fn external_resource_paths(&self) -> BTreeSet<String> {
        self.external_resources
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[cfg(feature = "fs")]
    pub(crate) fn validate_declarations(&self) -> Result<(), PackBuildError> {
        let entrypoint = canonical_path(PackPathRole::Entrypoint, &self.entrypoint)?;
        validate_project_declarations(std::iter::once(entrypoint), &self.external_resources)?;
        Ok(())
    }

    /// Adds a file of a vendored package.
    pub fn package_file(
        mut self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        let path = canonical_path(PackPathRole::PackageFile, path.as_ref())?;
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
        let info =
            FontInfo::new(&data, index).ok_or(PackInvariantError::InvalidFontInput { index })?;
        let family = info.family.to_string();
        let path = self.font_path(&family, &data);
        self.fonts.push(PackFontInput {
            entry: FontManifest::new(path, index, vec![family]),
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
        let entrypoint = canonical_path(PackPathRole::Entrypoint, &self.entrypoint)?;
        let font_data = self
            .fonts
            .iter()
            .map(|font| {
                Ok((
                    canonical_path(PackPathRole::FontData, font.entry.path())?,
                    font.data.clone(),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, PackInvariantError>>()?;
        let manifest = PackManifest::new(
            entrypoint.into_string(),
            self.external_resources
                .into_iter()
                .map(CanonicalPath::into_string)
                .collect(),
            self.packages
                .values()
                .map(|package| package.spec.clone())
                .collect(),
            self.unvendored_packages.clone(),
            self.fonts.iter().map(|font| font.entry.clone()).collect(),
            self.metadata,
        );

        Ok(Pack::construct(
            manifest,
            self.files,
            self.packages,
            font_data,
        )?)
    }

    /// Picks a unique archive path for a font file.
    fn font_path(&self, family: &str, data: &[u8]) -> String {
        if let Some(existing) = self.fonts.iter().find(|font| font.data.as_slice() == data) {
            return existing.entry.path().to_owned();
        }
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

        let mut candidate = format!("fonts/{stem}.{extension}");
        let mut counter = 1;
        loop {
            match self
                .fonts
                .iter()
                .find(|font| font.entry.path() == candidate)
            {
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

fn canonical_path(role: PackPathRole, path: &str) -> Result<CanonicalPath, PackInvariantError> {
    let invalid = |message: String| PackInvariantError::InvalidPath {
        role,
        path: path.to_owned(),
        message,
    };
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err(invalid("path must name a root-relative file".to_owned()));
    }
    if path.contains('\\') {
        return Err(invalid(
            "backslashes are not portable path separators".to_owned(),
        ));
    }
    if has_windows_drive_prefix(path) {
        return Err(invalid(
            "path must not contain a platform root prefix".to_owned(),
        ));
    }
    let vpath = VirtualPath::new(path).map_err(|err| invalid(err.to_string()))?;
    let canonical = vpath.get_without_slash();
    if canonical.is_empty() {
        return Err(invalid("path must name a file".to_owned()));
    }
    if has_windows_drive_prefix(canonical) {
        return Err(invalid(
            "path must not contain a platform root prefix".to_owned(),
        ));
    }
    Ok(CanonicalPath(canonical.to_owned()))
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn strip_current_directory_prefix(mut path: &str) -> &str {
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    path
}

fn find_path_tree_conflict(
    mut paths: Vec<(CanonicalPath, PackPathRole)>,
) -> Option<(CanonicalPath, PackPathRole, CanonicalPath, PackPathRole)> {
    paths.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (ancestor, ancestor_role) in &paths {
        let prefix = format!("{ancestor}/");
        let candidate = paths.partition_point(|(path, _)| path.as_str() < prefix.as_str());
        if let Some((descendant, descendant_role)) = paths.get(candidate)
            && descendant.as_str().starts_with(&prefix)
        {
            return Some((
                ancestor.clone(),
                *ancestor_role,
                descendant.clone(),
                *descendant_role,
            ));
        }
    }
    None
}

fn validate_project_declarations(
    project_files: impl IntoIterator<Item = CanonicalPath>,
    external_resources: &BTreeSet<CanonicalPath>,
) -> Result<(), PackInvariantError> {
    let mut project_paths = Vec::new();
    for path in project_files {
        if external_resources.contains(&path) {
            return Err(PackInvariantError::PathRoleConflict {
                path: path.to_string(),
                first: PackPathRole::ProjectFile,
                second: PackPathRole::ExternalProjectResource,
            });
        }
        project_paths.push((path, PackPathRole::ProjectFile));
    }
    project_paths.extend(
        external_resources
            .iter()
            .cloned()
            .map(|path| (path, PackPathRole::ExternalProjectResource)),
    );
    if let Some((ancestor, ancestor_role, descendant, descendant_role)) =
        find_path_tree_conflict(project_paths)
    {
        return Err(PackInvariantError::PathTreeConflict {
            ancestor: ancestor.to_string(),
            ancestor_role,
            descendant: descendant.to_string(),
            descendant_role,
        });
    }
    Ok(())
}

fn reserved_font_path_role(path: &CanonicalPath) -> Option<PackPathRole> {
    if is_same_or_descendant(path.as_str(), MANIFEST_PATH) {
        Some(PackPathRole::PackManifest)
    } else if is_same_or_descendant(path.as_str(), PROJECT_PREFIX.trim_end_matches('/')) {
        Some(PackPathRole::ProjectFile)
    } else if is_same_or_descendant(path.as_str(), PACKAGES_PREFIX.trim_end_matches('/')) {
        Some(PackPathRole::PackageFile)
    } else {
        None
    }
}

fn is_same_or_descendant(path: &str, ancestor: &str) -> bool {
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn register_archive_identity(
    entries: &mut BTreeMap<String, Vec<u8>>,
    canonical: String,
    raw_name: &[u8],
) -> Result<(), PackInvariantError> {
    if let Some(first_entry) = entries.get(&canonical) {
        if first_entry == raw_name {
            return Ok(());
        }
        return Err(PackInvariantError::CanonicalArchiveEntryCollision {
            canonical,
            first_entry: display_archive_name(first_entry),
            second_entry: display_archive_name(raw_name),
        });
    }
    entries.insert(canonical, raw_name.to_owned());
    Ok(())
}

fn display_archive_name(raw_name: &[u8]) -> String {
    String::from_utf8(raw_name.to_owned()).unwrap_or_else(|_| {
        raw_name
            .iter()
            .flat_map(|byte| std::ascii::escape_default(*byte).map(char::from))
            .collect()
    })
}

/// A failure while building a pack in memory.
#[derive(Debug, thiserror::Error)]
pub enum PackBuildError {
    #[error(transparent)]
    Invariant(#[from] PackInvariantError),
}

/// A violation of the invariants shared by every [`Pack`] construction path.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum PackInvariantError {
    /// A path cannot identify a canonical file for its declared role.
    #[error("invalid {role} path `{path}`: {message}")]
    InvalidPath {
        role: PackPathRole,
        path: String,
        message: String,
    },
    /// Distinct archive entries identify one canonical contained file.
    #[error("archive entries `{first_entry}` and `{second_entry}` both identify `{canonical}`")]
    CanonicalArchiveEntryCollision {
        canonical: String,
        first_entry: String,
        second_entry: String,
    },
    /// One canonical path was assigned two incompatible roles.
    #[error("path `{path}` cannot be both {first} and {second}")]
    PathRoleConflict {
        path: String,
        first: PackPathRole,
        second: PackPathRole,
    },
    /// One file path is an ancestor of another file path in the same tree.
    #[error(
        "{ancestor_role} path `{ancestor}` conflicts with {descendant_role} descendant `{descendant}`"
    )]
    PathTreeConflict {
        ancestor: String,
        ancestor_role: PackPathRole,
        descendant: String,
        descendant_role: PackPathRole,
    },
    /// One package file path is an ancestor of another file path in that package.
    #[error(
        "package `{package}` {ancestor_role} path `{ancestor}` conflicts with {descendant_role} descendant `{descendant}`"
    )]
    PackagePathTreeConflict {
        package: String,
        ancestor: String,
        ancestor_role: PackPathRole,
        descendant: String,
        descendant_role: PackPathRole,
    },
    /// The entrypoint was declared as a non-source External Project Resource.
    #[error("entrypoint `{0}` cannot be an External Project Resource")]
    EntrypointIsExternalResource(String),
    /// A package was declared both vendored and externally resolved.
    #[error("package `{0}` cannot be both vendored and unvendored")]
    PackageRoleConflict(String),
    /// Package bytes exist without a matching vendored declaration.
    #[error("package `{0}` has contained data but is not declared vendored")]
    UndeclaredPackageData(String),
    /// A vendored package declaration has no contained bytes.
    #[error("vendored package `{0}` has no contained data")]
    MissingVendoredPackageData(String),
    /// A font declaration uses an archive path reserved for another role.
    #[error("font data path `{path}` conflicts with the {conflicting_role} archive role")]
    ReservedFontPath {
        path: String,
        conflicting_role: PackPathRole,
    },
    /// A font declaration has no contained bytes.
    #[error("font data `{0}` is missing")]
    MissingFontData(String),
    /// Builder-provided font bytes do not contain the requested face.
    #[error("font input does not contain a valid face at index {index}")]
    InvalidFontInput { index: u32 },
    /// Contained font bytes do not contain the declared face.
    #[error("font data `{path}` does not contain a valid face at index {index}")]
    InvalidFontData { path: String, index: u32 },
    /// The same contained font face was declared more than once.
    #[error("font `{path}` declares face index {index} more than once")]
    DuplicateFontFace { path: String, index: u32 },
    /// The declared entrypoint is not present among the packed project files.
    #[error("entrypoint `{0}` is not a contained project file")]
    MissingEntrypoint(String),
}

/// The role a path plays in a Pack invariant.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PackPathRole {
    PackManifest,
    Entrypoint,
    ProjectFile,
    ExternalProjectResource,
    PackageFile,
    FontData,
}

impl std::fmt::Display for PackPathRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::PackManifest => "Pack Manifest",
            Self::Entrypoint => "entrypoint",
            Self::ProjectFile => "project file",
            Self::ExternalProjectResource => "External Project Resource",
            Self::PackageFile => "package file",
            Self::FontData => "font data",
        })
    }
}
