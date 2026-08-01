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

use crate::manifest::{
    FontManifest, MANIFEST_PATH, PackManifest, PackManifestError, PackMetadata, PackageManifest,
};
use crate::payload::{PackArchiveBytes, SharedBytes};

/// The conventional file extension for packs.
pub const FILE_EXTENSION: &str = "typk";

/// Whether any segment of a root-relative path names a Pack.
pub(crate) fn names_pack_path(path: &str) -> bool {
    path.split('/').any(|segment| {
        segment.strip_prefix('.') == Some(FILE_EXTENSION)
            || std::path::Path::new(segment)
                .extension()
                .is_some_and(|extension| extension == FILE_EXTENSION)
    })
}

const PROJECT_PREFIX: &str = "project/";
const PACKAGES_PREFIX: &str = "packages/";
const MAX_ZIP_ENTRY_NAME_LEN: usize = u16::MAX as usize;
pub(crate) const PACKAGE_TREE_IDENTITY_KIND: &str = "complete-package-tree";
pub(crate) const PACKAGE_TREE_IDENTITY_SCHEMA: &str = "typst-pack-complete-package-tree-v1";
pub(crate) const PACKAGE_TREE_IDENTITY_ALGORITHM: &str = "typst-hash128-0.15";

/// A portable pack of a Typst project.
///
/// A pack holds project files (sources, images, and data files), optionally
/// package files and fonts. Every project path has contained bytes.
/// Its archive form is a Zip file with a `typst-pack.toml`
/// manifest, conventionally named `*.typk`.
#[derive(Debug, Clone)]
pub struct Pack {
    entrypoint: CanonicalPath,
    metadata: Option<PackMetadata>,
    files: BTreeMap<CanonicalPath, SharedBytes>,
    /// Vendored packages, keyed by spec string for deterministic order.
    packages: BTreeMap<String, PackageFiles>,
    package_requirements: Vec<PackageRequirement>,
    fonts: Vec<PackFont>,
    font_catalog: Vec<PackFontCatalogFace>,
    font_requirements: Vec<FontRequirement>,
}

/// The canonical semantic identity of a [`Pack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackIdentity(u128);

impl PackIdentity {
    pub fn kind(self) -> &'static str {
        "pack"
    }

    pub fn schema(self) -> &'static str {
        "typst-pack-identity-v1"
    }

    pub fn algorithm(self) -> &'static str {
        "typst-hash128-0.15"
    }

    pub fn digest(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PackageFiles {
    pub(crate) spec: PackageSpec,
    files: BTreeMap<CanonicalPath, SharedBytes>,
}

impl PackageFiles {
    pub(crate) fn file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }
}

/// Exact verified dependencies accepted by the synchronous Compilation Kernel.
pub(crate) struct CompilationDependencySnapshot {
    pack_identity: PackIdentity,
    packages: BTreeMap<String, PackageFiles>,
    font_catalog: Vec<Font>,
}

impl CompilationDependencySnapshot {
    pub(crate) fn pack_identity(&self) -> PackIdentity {
        self.pack_identity
    }

    pub(crate) fn into_parts(self) -> (BTreeMap<String, PackageFiles>, Vec<Font>) {
        (self.packages, self.font_catalog)
    }
}

/// The canonical content identity of one Package Tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageTreeIdentity(u128);

impl PackageTreeIdentity {
    pub(crate) fn from_digest(digest: u128) -> Self {
        Self(digest)
    }

    pub fn digest(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
    pub fn kind(self) -> &'static str {
        PACKAGE_TREE_IDENTITY_KIND
    }
    pub fn schema(self) -> &'static str {
        PACKAGE_TREE_IDENTITY_SCHEMA
    }
    pub fn algorithm(self) -> &'static str {
        PACKAGE_TREE_IDENTITY_ALGORITHM
    }
    fn encode(self) -> String {
        format!("{:032x}", self.0)
    }
    fn decode(value: &str) -> Option<Self> {
        (value.len() == 32)
            .then(|| u128::from_str_radix(value, 16).ok().map(Self))
            .flatten()
    }
}

/// One exact package specification and Package Tree identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequirement {
    spec: PackageSpec,
    tree: PackageTreeIdentity,
    file_count: u64,
    byte_length: u64,
    embedded: bool,
}

impl PackageRequirement {
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }
    pub fn tree_identity(&self) -> PackageTreeIdentity {
        self.tree
    }
    pub fn file_count(&self) -> u64 {
        self.file_count
    }
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub fn is_embedded(&self) -> bool {
        self.embedded
    }
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalPath(String);

#[derive(Debug)]
struct PathTreeConflict {
    ancestor: CanonicalPath,
    ancestor_role: PackPathRole,
    descendant: CanonicalPath,
    descendant_role: PackPathRole,
}

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
    identity: FontFaceIdentity,
    data: SharedBytes,
    font: Font,
}

/// The canonical content identity of one exact Font Container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontContainerIdentity(u128);

impl FontContainerIdentity {
    /// Derives the identity from exact container bytes.
    pub fn from_bytes(data: &[u8]) -> Self {
        Self(typst::utils::hash128(&data))
    }

    /// The identity digest in big-endian order.
    pub fn digest(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    pub fn kind(self) -> &'static str {
        "font-container"
    }

    pub fn schema(self) -> &'static str {
        "typst-pack-font-container-identity-v1"
    }

    pub fn algorithm(self) -> &'static str {
        "typst-hash128-0.15"
    }

    fn encode(self) -> String {
        format!("{:032x}", self.0)
    }

    fn decode(value: &str) -> Option<Self> {
        u128::from_str_radix(value, 16).ok().map(Self)
    }
}

/// The exact identity of one face within a Font Container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontFaceIdentity {
    container: FontContainerIdentity,
    index: u32,
}

impl FontFaceIdentity {
    /// The face at a container-local index within the given container.
    pub(crate) fn new(container: FontContainerIdentity, index: u32) -> Self {
        Self { container, index }
    }

    /// The containing font file or collection.
    pub fn container(self) -> FontContainerIdentity {
        self.container
    }

    /// The face's container-local index.
    pub fn index(self) -> u32 {
        self.index
    }
}

/// One ordered face in the exact Pack Font Catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFontCatalogFace {
    identity: FontFaceIdentity,
    embedded: bool,
}

impl PackFontCatalogFace {
    /// The exact container and face index.
    pub fn identity(&self) -> FontFaceIdentity {
        self.identity
    }

    /// Whether the Font Container bytes are stored in the Pack.
    pub fn is_embedded(&self) -> bool {
        self.embedded
    }
}

/// One exact Font Container and the faces required from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontRequirement {
    container: FontContainerIdentity,
    length: u64,
    face_indices: Vec<u32>,
    embedded: bool,
}

impl FontRequirement {
    pub fn container_identity(&self) -> FontContainerIdentity {
        self.container
    }

    pub fn container_length(&self) -> u64 {
        self.length
    }

    pub fn face_indices(&self) -> &[u32] {
        &self.face_indices
    }

    pub fn is_embedded(&self) -> bool {
        self.embedded
    }
}

impl PackFont {
    /// The exact container and container-local face index.
    pub fn identity(&self) -> FontFaceIdentity {
        self.identity
    }

    /// The contained font bytes.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Official selection metadata derived from the verified container bytes.
    pub fn info(&self) -> &FontInfo {
        self.font.info()
    }
}

#[derive(Debug, Clone)]
struct PackFontInput {
    path: Option<String>,
    index: u32,
    declared_container_digest: Option<String>,
    declared_container_identity_kind: Option<String>,
    declared_container_identity_schema: Option<String>,
    declared_container_identity_algorithm: Option<String>,
    declared_container_length: Option<u64>,
    data: Option<SharedBytes>,
    embedded: bool,
}

#[derive(Debug)]
struct ProjectFileInput {
    path: String,
    data: SharedBytes,
}

#[derive(Debug)]
struct PackageFileInput {
    spec: PackageSpec,
    path: String,
    data: SharedBytes,
    embedded: bool,
}

#[derive(Debug)]
struct PackageRequirementInput {
    entry: PackageManifest,
    embedded: bool,
}

#[derive(Debug)]
struct PackConstructionInput {
    entrypoint: String,
    metadata: Option<PackMetadata>,
    files: Vec<ProjectFileInput>,
    package_files: Vec<PackageFileInput>,
    package_requirements: Vec<PackageRequirementInput>,
    package_requirements_are_declared: bool,
    fonts: Vec<PackFontInput>,
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder::new(entrypoint)
    }

    fn construct(input: PackConstructionInput) -> Result<Self, PackInvariantError> {
        let mut issues = Vec::new();

        let entrypoint = match canonical_path(PackPathRole::Entrypoint, &input.entrypoint) {
            Ok(path) => Some(path),
            Err(issue) => {
                issues.push(issue);
                None
            }
        };

        let mut canonical_files = BTreeMap::new();
        let mut duplicate_project_paths = BTreeSet::new();
        for file in input.files {
            match canonical_path(PackPathRole::ProjectFile, &file.path) {
                Ok(path) => {
                    if canonical_files.insert(path.clone(), file.data).is_some() {
                        duplicate_project_paths.insert(path);
                    }
                }
                Err(issue) => issues.push(issue),
            }
        }
        issues.extend(duplicate_project_paths.into_iter().map(|path| {
            PackInvariantIssue::DuplicateProjectPath {
                path: path.into_string(),
            }
        }));
        issues.extend(path_tree_conflicts(
            canonical_files.keys().cloned(),
            PackPathRole::ProjectFile,
        ));

        let mut package_groups = BTreeMap::<(String, bool), PackageFiles>::new();
        let mut invalid_package_groups = BTreeSet::new();
        let mut duplicate_package_paths = BTreeSet::new();
        for file in input.package_files {
            let key = file.spec.to_string();
            if let Err(issue) = validate_package_spec(&file.spec) {
                issues.push(issue);
            }
            let path = match canonical_path(PackPathRole::PackageFile, &file.path) {
                Ok(path) => path,
                Err(issue) => {
                    issues.push(issue);
                    continue;
                }
            };
            let package = package_groups
                .entry((key.clone(), file.embedded))
                .or_insert_with(|| PackageFiles {
                    spec: file.spec.clone(),
                    files: BTreeMap::new(),
                });
            if package.files.insert(path.clone(), file.data).is_some() {
                duplicate_package_paths.insert((key.clone(), file.embedded, path));
                invalid_package_groups.insert((key, file.embedded));
            }
        }
        for (package, _, path) in duplicate_package_paths {
            if let Some(spec) = package_groups
                .values()
                .find(|entry| entry.spec.to_string() == package)
                .map(|entry| entry.spec.clone())
            {
                issues.push(PackInvariantIssue::DuplicatePackagePath {
                    package: spec,
                    path: path.into_string(),
                });
            }
        }
        for ((package, embedded), files) in &package_groups {
            let conflicts = find_path_tree_conflicts(
                files
                    .files
                    .keys()
                    .cloned()
                    .map(|path| (path, PackPathRole::PackageFile))
                    .collect(),
            );
            if !conflicts.is_empty() {
                invalid_package_groups.insert((package.clone(), *embedded));
            }
            for conflict in conflicts {
                issues.push(PackInvariantIssue::PackagePathTreeConflict {
                    package: package.clone(),
                    ancestor: conflict.ancestor.into_string(),
                    ancestor_role: conflict.ancestor_role,
                    descendant: conflict.descendant.into_string(),
                    descendant_role: conflict.descendant_role,
                });
            }
        }

        let declarations_are_explicit = input.package_requirements_are_declared;
        let mut declared_requirements = BTreeMap::<(String, bool), Vec<PackageRequirement>>::new();
        let mut declared_requirement_roles = BTreeSet::new();
        let mut duplicate_requirements = BTreeSet::new();
        for declaration in input.package_requirements {
            let role = match declaration.entry.spec() {
                Ok(spec) => (spec.to_string(), declaration.embedded),
                Err(PackManifestError::InvalidPackageSpec { spec, message }) => {
                    issues.push(PackInvariantIssue::InvalidPackageSpec { spec, message });
                    continue;
                }
                Err(error) => {
                    issues.push(PackInvariantIssue::InvalidPackageRequirement {
                        spec: error.to_string(),
                    });
                    continue;
                }
            };
            if !declared_requirement_roles.insert(role.clone()) {
                duplicate_requirements.insert(role.clone());
            }
            match package_manifest_requirement(&declaration.entry, declaration.embedded) {
                Ok((key, requirement)) => {
                    declared_requirements
                        .entry((key, declaration.embedded))
                        .or_default()
                        .push(requirement);
                }
                Err(issue) => issues.push(issue),
            }
        }
        issues.extend(duplicate_requirements.into_iter().map(|(spec, embedded)| {
            PackInvariantIssue::DuplicatePackageRequirement { spec, embedded }
        }));

        let requirement_specs = package_groups
            .keys()
            .map(|(spec, _)| spec.clone())
            .chain(
                declared_requirement_roles
                    .iter()
                    .map(|(spec, _)| spec.clone()),
            )
            .collect::<BTreeSet<_>>();
        for spec in &requirement_specs {
            let embedded = package_groups.contains_key(&(spec.clone(), true))
                || declared_requirement_roles.contains(&(spec.clone(), true));
            let external = package_groups.contains_key(&(spec.clone(), false))
                || declared_requirement_roles.contains(&(spec.clone(), false));
            if embedded && external {
                issues.push(PackInvariantIssue::PackageRoleConflict { spec: spec.clone() });
            }
        }

        let mut canonical_packages = BTreeMap::new();
        let mut package_requirements = Vec::new();
        if declarations_are_explicit {
            for ((spec, embedded), declarations) in &declared_requirements {
                let declared = &declarations[0];
                if *embedded {
                    if let Some(package) = package_groups.get(&(spec.clone(), true)) {
                        let (tree, file_count, byte_length) = package_tree_identity(&package.files);
                        if !invalid_package_groups.contains(&(spec.clone(), true))
                            && declarations.iter().any(|declared| {
                                declared.tree != tree
                                    || declared.file_count != file_count
                                    || declared.byte_length != byte_length
                            })
                        {
                            issues.push(PackInvariantIssue::MismatchedEmbeddedPackageIdentity {
                                spec: spec.clone(),
                            });
                        }
                    }
                }
                package_requirements.push(declared.clone());
            }
            for (spec, embedded) in &declared_requirement_roles {
                if *embedded && !package_groups.contains_key(&(spec.clone(), true)) {
                    issues.push(PackInvariantIssue::MissingVendoredPackageData {
                        spec: spec.clone(),
                    });
                }
            }
            for ((spec, embedded), package) in &package_groups {
                if *embedded && !declared_requirement_roles.contains(&(spec.clone(), true)) {
                    issues.push(PackInvariantIssue::UndeclaredPackageData { spec: spec.clone() });
                }
                if *embedded {
                    canonical_packages.insert(spec.clone(), package.clone());
                }
            }
        } else {
            for ((spec, embedded), package) in &package_groups {
                let (tree, file_count, byte_length) = package_tree_identity(&package.files);
                package_requirements.push(PackageRequirement {
                    spec: package.spec.clone(),
                    tree,
                    file_count,
                    byte_length,
                    embedded: *embedded,
                });
                if *embedded {
                    canonical_packages.insert(spec.clone(), package.clone());
                }
            }
        }
        package_requirements.sort_by_key(|requirement| requirement.spec.to_string());
        let mut canonical_fonts = Vec::new();
        let mut font_catalog = Vec::new();
        let mut font_requirements = Vec::<FontRequirement>::new();
        let mut font_faces = BTreeSet::new();
        for (position, entry) in input.fonts.into_iter().enumerate() {
            let path = entry
                .path
                .clone()
                .unwrap_or_else(|| format!("font input {position}"));
            let index = entry.index;
            let embedded = entry.embedded;
            let parsed_data = entry.data.as_ref().and_then(|data| {
                Font::new(data.to_typst(), index).map(|font| {
                    let container = FontContainerIdentity::from_bytes(data.as_slice());
                    (data.clone(), font, container, data.len() as u64)
                })
            });
            let (data, parsed, container, length) = if embedded {
                let Some((data, parsed, container, length)) = parsed_data else {
                    issues.push(if entry.data.is_some() {
                        PackInvariantIssue::InvalidFontData { path, index }
                    } else {
                        PackInvariantIssue::MissingFontData { path }
                    });
                    continue;
                };
                if entry
                    .declared_container_digest
                    .as_deref()
                    .is_some_and(|digest| FontContainerIdentity::decode(digest) != Some(container))
                    || entry
                        .declared_container_length
                        .is_some_and(|declared| declared != length)
                    || entry
                        .declared_container_identity_kind
                        .as_deref()
                        .is_some_and(|kind| kind != container.kind())
                    || entry
                        .declared_container_identity_schema
                        .as_deref()
                        .is_some_and(|schema| schema != container.schema())
                    || entry
                        .declared_container_identity_algorithm
                        .as_deref()
                        .is_some_and(|algorithm| algorithm != container.algorithm())
                {
                    issues.push(PackInvariantIssue::MismatchedEmbeddedFontIdentity {
                        path: path.clone(),
                    });
                }
                (Some(data), Some(parsed), container, length)
            } else if entry.path.is_none() {
                let Some((_, _, container, length)) = parsed_data else {
                    issues.push(PackInvariantIssue::InvalidFontData { path, index });
                    continue;
                };
                (None, None, container, length)
            } else {
                if entry.data.is_some() {
                    issues.push(PackInvariantIssue::ExternalFontHasContainedData {
                        path: path.clone(),
                    });
                }
                let valid_identity = entry.declared_container_identity_kind.as_deref()
                    == Some("font-container")
                    && entry.declared_container_identity_schema.as_deref()
                        == Some("typst-pack-font-container-identity-v1")
                    && entry.declared_container_identity_algorithm.as_deref()
                        == Some("typst-hash128-0.15");
                let container = entry
                    .declared_container_digest
                    .as_deref()
                    .and_then(FontContainerIdentity::decode);
                let length = entry.declared_container_length.filter(|length| *length > 0);
                let (Some(container), Some(length)) = (container, length) else {
                    issues.push(PackInvariantIssue::InvalidExternalFontIdentity { path });
                    continue;
                };
                if !valid_identity {
                    issues.push(PackInvariantIssue::InvalidExternalFontIdentity { path });
                    continue;
                }
                (None, None, container, length)
            };

            if !font_faces.insert((container, index)) {
                issues.push(PackInvariantIssue::DuplicateFontFace {
                    path: path.clone(),
                    index,
                });
            }
            font_catalog.push(PackFontCatalogFace {
                identity: FontFaceIdentity::new(container, index),
                embedded,
            });
            match font_requirements
                .iter_mut()
                .find(|requirement| requirement.container == container)
            {
                Some(requirement)
                    if requirement.length != length || requirement.embedded != embedded =>
                {
                    issues.push(PackInvariantIssue::InconsistentFontContainer { path });
                }
                Some(requirement) => requirement.face_indices.push(index),
                None => font_requirements.push(FontRequirement {
                    container,
                    length,
                    face_indices: vec![index],
                    embedded,
                }),
            }
            if let (Some(data), Some(font)) = (data, parsed) {
                canonical_fonts.push(PackFont {
                    identity: FontFaceIdentity::new(container, index),
                    data,
                    font,
                });
            }
        }
        if let Some(entrypoint) = &entrypoint
            && !canonical_files.contains_key(entrypoint)
        {
            issues.push(PackInvariantIssue::MissingEntrypoint {
                path: entrypoint.to_string(),
            });
        }

        issues.sort_by_key(PackInvariantIssue::sort_key);
        if !issues.is_empty() {
            return Err(PackInvariantError { issues });
        }

        Ok(Self {
            entrypoint: entrypoint.expect("a valid Pack has a canonical entrypoint"),
            metadata: input.metadata,
            files: canonical_files,
            packages: canonical_packages,
            package_requirements,
            fonts: canonical_fonts,
            font_catalog,
            font_requirements,
        })
    }

    /// Derives the Pack's identity-bearing semantic projection.
    pub fn identity(&self) -> PackIdentity {
        let project_files = self
            .files
            .iter()
            .map(|(path, data)| (path.as_str(), typst::utils::hash128(data)))
            .collect::<Vec<_>>();
        let packages = self
            .package_requirements()
            .iter()
            .map(|requirement| {
                (
                    requirement.spec.to_string(),
                    requirement.tree.0,
                    requirement.file_count,
                    requirement.byte_length,
                    requirement.embedded,
                )
            })
            .collect::<Vec<_>>();
        let fonts = self
            .font_catalog()
            .iter()
            .map(|face| {
                (
                    face.identity.container.0,
                    face.identity.index,
                    face.embedded,
                )
            })
            .collect::<Vec<_>>();
        PackIdentity(typst::utils::hash128(&(
            "typst-pack-identity-v1",
            self.entrypoint(),
            project_files,
            packages,
            fonts,
        )))
    }

    /// The root-relative path of the entrypoint file.
    pub fn entrypoint(&self) -> &str {
        self.entrypoint.as_str()
    }

    /// Optional descriptive metadata, excluded from Pack Identity.
    pub fn metadata(&self) -> Option<&PackMetadata> {
        self.metadata.as_ref()
    }

    /// The project files, keyed by root-relative path.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, data)| (path.as_str(), data.as_slice()))
    }

    /// Looks up a project file by root-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }

    pub(crate) fn canonical_project_path(path: &str) -> Result<String, String> {
        canonical_path(PackPathRole::ProjectFile, path)
            .map(CanonicalPath::into_string)
            .map_err(|error| error.to_string())
    }

    /// Canonicalizes a supplied package-relative path, so that a tree is
    /// looked up and contained under the same path.
    pub(crate) fn canonical_package_path(path: &str) -> Result<String, String> {
        canonical_path(PackPathRole::PackageFile, path)
            .map(CanonicalPath::into_string)
            .map_err(|error| error.to_string())
    }

    /// The vendored packages and their files.
    pub fn packages(
        &self,
    ) -> impl Iterator<Item = (&PackageSpec, impl Iterator<Item = (&str, &[u8])>)> {
        self.packages.values().map(|package| {
            (
                &package.spec,
                package
                    .files
                    .iter()
                    .map(|(path, data)| (path.as_str(), data.as_slice())),
            )
        })
    }

    /// Looks up a vendored package file.
    pub fn package_file(&self, spec: &PackageSpec, path: &str) -> Option<&[u8]> {
        self.packages
            .get(&spec.to_string())?
            .files
            .get(path)
            .map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_package_file(
        &self,
        spec: &PackageSpec,
        path: &str,
    ) -> Option<&SharedBytes> {
        self.packages.get(&spec.to_string())?.files.get(path)
    }

    /// Whether the pack vendors the given package.
    pub fn has_package(&self, spec: &PackageSpec) -> bool {
        self.packages.contains_key(&spec.to_string())
    }

    /// The Pack's exact Package Requirements in canonical specification order.
    pub fn package_requirements(&self) -> &[PackageRequirement] {
        &self.package_requirements
    }

    pub(crate) fn materialize_package_trees(
        &self,
        fulfillments: BTreeMap<String, Vec<(String, Bytes)>>,
    ) -> Result<BTreeMap<String, PackageFiles>, PackageFulfillmentError> {
        let missing = self
            .package_requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
            .filter(|requirement| !fulfillments.contains_key(&requirement.spec.to_string()))
            .map(|requirement| requirement.spec.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PackageFulfillmentError::Missing { packages: missing });
        }

        let mut materialized = self.packages.clone();
        for requirement in self
            .package_requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
        {
            let key = requirement.spec.to_string();
            let mut files = BTreeMap::new();
            for (path, data) in &fulfillments[&key] {
                let canonical =
                    canonical_path(PackPathRole::PackageFile, path).map_err(|error| {
                        PackageFulfillmentError::Malformed {
                            spec: requirement.spec.clone(),
                            path: path.clone(),
                            message: error.to_string(),
                        }
                    })?;
                if files
                    .insert(canonical, SharedBytes::from_typst(data.clone()))
                    .is_some()
                {
                    return Err(PackageFulfillmentError::Malformed {
                        spec: requirement.spec.clone(),
                        path: path.clone(),
                        message: "duplicate package file path".to_owned(),
                    });
                }
            }
            let paths = files
                .keys()
                .cloned()
                .map(|path| (path, PackPathRole::PackageFile))
                .collect();
            if let Some(conflict) = find_path_tree_conflicts(paths).into_iter().next() {
                return Err(PackageFulfillmentError::Malformed {
                    spec: requirement.spec.clone(),
                    path: conflict.descendant.to_string(),
                    message: format!("file path has file ancestor `{}`", conflict.ancestor),
                });
            }
            let (actual, actual_file_count, actual_byte_length) = package_tree_identity(&files);
            if actual != requirement.tree
                || actual_file_count != requirement.file_count
                || actual_byte_length != requirement.byte_length
            {
                return Err(PackageFulfillmentError::Mismatched {
                    spec: requirement.spec.clone(),
                    expected: requirement.tree,
                    actual,
                    expected_file_count: requirement.file_count,
                    actual_file_count,
                    expected_byte_length: requirement.byte_length,
                    actual_byte_length,
                });
            }
            materialized.insert(
                key,
                PackageFiles {
                    spec: requirement.spec.clone(),
                    files,
                },
            );
        }
        Ok(materialized)
    }

    /// The fonts embedded in the pack.
    pub fn fonts(&self) -> &[PackFont] {
        &self.fonts
    }

    /// The exact candidate faces exposed to official Typst, in stable order.
    pub fn font_catalog(&self) -> &[PackFontCatalogFace] {
        &self.font_catalog
    }

    /// The exact Font Containers required by this Pack.
    pub fn font_requirements(&self) -> &[FontRequirement] {
        &self.font_requirements
    }

    pub(crate) fn materialize_font_catalog(
        &self,
        fulfillments: &BTreeMap<FontContainerIdentity, Bytes>,
    ) -> Result<Vec<Font>, FontCatalogError> {
        let missing = self
            .font_requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
            .map(|requirement| requirement.container)
            .filter(|container| !fulfillments.contains_key(container))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(FontCatalogError::Missing {
                containers: missing,
            });
        }
        self.font_catalog
            .iter()
            .map(|face| {
                let identity = face.identity;
                if face.embedded {
                    return Ok(self
                        .fonts
                        .iter()
                        .find(|font| {
                            FontContainerIdentity::from_bytes(font.data.as_slice())
                                == identity.container
                                && font.identity.index() == identity.index
                        })
                        .expect("Pack Font Catalog embedded face invariant violated")
                        .font
                        .clone());
                }
                let data = &fulfillments[&identity.container];
                let actual = FontContainerIdentity::from_bytes(data.as_slice());
                let actual_length = data.len() as u64;
                let expected_length = self
                    .font_requirements
                    .iter()
                    .find(|requirement| requirement.container == identity.container)
                    .expect("Pack Font Catalog requirement invariant violated")
                    .length;
                if actual != identity.container || actual_length != expected_length {
                    return Err(FontCatalogError::Mismatched {
                        expected: identity.container,
                        actual,
                        expected_length,
                        actual_length,
                    });
                }
                Font::new(data.clone(), identity.index).ok_or(FontCatalogError::Malformed {
                    container: identity.container,
                    index: identity.index,
                })
            })
            .collect()
    }

    pub(crate) fn materialize_compilation_dependency_snapshot(
        &self,
        package_fulfillments: BTreeMap<String, Vec<(String, Bytes)>>,
        font_fulfillments: &BTreeMap<FontContainerIdentity, Bytes>,
    ) -> Result<CompilationDependencySnapshot, CompilationDependencySnapshotError> {
        let packages = self
            .materialize_package_trees(package_fulfillments)
            .map_err(|error| CompilationDependencySnapshotError::Package(Box::new(error)))?;
        let font_catalog = self
            .materialize_font_catalog(font_fulfillments)
            .map_err(CompilationDependencySnapshotError::Font)?;
        Ok(CompilationDependencySnapshot {
            pack_identity: self.identity(),
            packages,
            font_catalog,
        })
    }

    /// Reads a pack from a seekable reader.
    pub fn read<R: Read + Seek>(reader: R) -> Result<Self, PackReadError> {
        let archive = ZipArchive::new(reader)?;
        let retained_entry_count = archive.len();
        let central_directory_start = archive.central_directory_start();
        let mut reader = archive.into_inner();
        let raw_entries = raw_central_entries(&mut reader, central_directory_start)?;
        if let Some(entry) = raw_entries
            .iter()
            .find(|entry| entry.utf8 && std::str::from_utf8(&entry.name).is_err())
        {
            return Err(PackReadError::InvalidUtf8EntryName(entry.name.clone()));
        }
        let mut archive = ZipArchive::new(reader)?;
        const FILE_TYPE_MASK: u32 = 0o170000;
        const REGULAR_FILE: u32 = 0o100000;
        const DIRECTORY: u32 = 0o040000;

        let mut manifest_entry = None;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index)?;
            let prefix_normalized_name = strip_current_directory_prefix(entry.name());
            let canonical_manifest_alias = !prefix_normalized_name.starts_with(PROJECT_PREFIX)
                && !prefix_normalized_name.starts_with(PACKAGES_PREFIX)
                && canonical_archive_name(entry.name()).is_ok_and(|name| name == MANIFEST_PATH);
            if prefix_normalized_name == MANIFEST_PATH || canonical_manifest_alias {
                let regular_file = entry.is_file()
                    && entry
                        .unix_mode()
                        .is_none_or(|mode| matches!(mode & FILE_TYPE_MASK, 0 | REGULAR_FILE));
                manifest_entry = Some((index, regular_file));
                break;
            }
        }
        let (manifest_index, manifest_is_file) =
            manifest_entry.ok_or(PackReadError::MissingManifest)?;
        if !manifest_is_file {
            return Err(PackReadError::ManifestNotFile);
        }
        let manifest_value = {
            let mut entry = archive.by_index(manifest_index)?;
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(PackReadError::ManifestUnreadable)?;
            let text = std::str::from_utf8(&bytes).map_err(PackReadError::ManifestNotUtf8)?;
            toml::from_str::<toml::Value>(text).map_err(PackManifestError::from)?
        };

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
        let manifest = PackManifest::from_toml_value(manifest_value)?;

        let mut font_paths = BTreeSet::new();
        let mut font_path_values = Vec::new();
        for font in manifest.fonts() {
            let path = canonical_path_without_membership(PackPathRole::FontData, font.path())
                .map_err(|issue| PackReadError::InvalidEntry {
                    entry: font.path().to_owned(),
                    message: issue.to_string(),
                })?;
            if let Some(conflicting_role) = reserved_font_archive_role(&path) {
                return Err(PackReadError::InvalidEntry {
                    entry: font.path().to_owned(),
                    message: format!("font data path conflicts with the {conflicting_role} role"),
                });
            }
            font_paths.insert(path.to_string());
            font_path_values.push((path, PackPathRole::FontData));
        }
        if let Some(conflict) = find_path_tree_conflicts(font_path_values)
            .into_iter()
            .next()
        {
            return Err(PackReadError::InvalidEntry {
                entry: conflict.descendant.to_string(),
                message: format!("font data path has file ancestor `{}`", conflict.ancestor),
            });
        }

        struct ProjectEntry {
            index: usize,
            path: String,
        }
        struct PackageEntry {
            index: usize,
            spec: PackageSpec,
            path: String,
        }
        struct UnknownEntry {
            index: usize,
            canonical_name: String,
        }

        let mut project_entries = Vec::new();
        let mut package_entries = Vec::new();
        let mut unknown_entries = Vec::new();
        let mut canonical_archive_entries = BTreeMap::new();
        for (index, raw_entry) in raw_entries.iter().enumerate() {
            let entry = archive.by_index_raw(index)?;
            let archive_name = entry.name().to_owned();
            let raw_name = raw_entry.name.clone();
            let prefix_normalized_name = strip_current_directory_prefix(&archive_name);
            let canonical_name = canonical_archive_name(&archive_name)?;
            register_archive_identity(
                &mut canonical_archive_entries,
                canonical_name.clone(),
                &raw_name,
            )?;
            if entry.is_dir()
                && entry
                    .unix_mode()
                    .is_none_or(|mode| matches!(mode & FILE_TYPE_MASK, 0 | DIRECTORY))
            {
                continue;
            }
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
                continue;
            } else if let Some(path) = role_name.strip_prefix(PROJECT_PREFIX) {
                if !regular_file {
                    return Err(PackReadError::UnsupportedEntryType(archive_name));
                }
                let path = path.trim_start_matches('/').to_owned();
                project_entries.push(ProjectEntry { index, path });
            } else if let Some(rest) = role_name.strip_prefix(PACKAGES_PREFIX) {
                if !regular_file {
                    return Err(PackReadError::UnsupportedEntryType(archive_name));
                }
                let (spec, path) = split_package_entry(rest, &archive_name)?;
                package_entries.push(PackageEntry { index, spec, path });
            } else {
                if !regular_file {
                    return Err(PackReadError::UnsupportedEntryType(archive_name));
                }
                unknown_entries.push(UnknownEntry {
                    index,
                    canonical_name,
                });
            }
        }

        let mut font_entries = Vec::new();
        for entry in unknown_entries {
            if let Some(path) = font_paths.get(entry.canonical_name.as_str()) {
                font_entries.push((entry.index, path.clone()));
            }
        }

        let mut files = Vec::new();
        for project in project_entries {
            let mut data = Vec::new();
            archive.by_index(project.index)?.read_to_end(&mut data)?;
            files.push(ProjectFileInput {
                path: project.path,
                data: SharedBytes::new(data),
            });
        }
        let mut package_files = Vec::new();
        for package in package_entries {
            let mut data = Vec::new();
            archive.by_index(package.index)?.read_to_end(&mut data)?;
            package_files.push(PackageFileInput {
                spec: package.spec,
                path: package.path,
                data: SharedBytes::new(data),
                embedded: true,
            });
        }
        let mut fonts_by_path = BTreeMap::new();
        for (index, path) in font_entries {
            let mut data = Vec::new();
            archive.by_index(index)?.read_to_end(&mut data)?;
            fonts_by_path.insert(path, SharedBytes::new(data));
        }

        let package_requirements = manifest
            .packages()
            .vendored()
            .iter()
            .cloned()
            .map(|entry| PackageRequirementInput {
                entry,
                embedded: true,
            })
            .chain(
                manifest
                    .packages()
                    .unvendored()
                    .iter()
                    .cloned()
                    .map(|entry| PackageRequirementInput {
                        entry,
                        embedded: false,
                    }),
            )
            .collect();
        let fonts = manifest
            .fonts()
            .iter()
            .map(|entry| {
                let canonical = canonical_archive_name(entry.path()).ok();
                PackFontInput {
                    path: Some(entry.path().to_owned()),
                    index: entry.index(),
                    declared_container_digest: entry.container_digest().map(str::to_owned),
                    declared_container_identity_kind: entry
                        .container_identity_kind()
                        .map(str::to_owned),
                    declared_container_identity_schema: entry
                        .container_identity_schema()
                        .map(str::to_owned),
                    declared_container_identity_algorithm: entry
                        .container_identity_algorithm()
                        .map(str::to_owned),
                    declared_container_length: entry.container_length(),
                    data: canonical.and_then(|path| fonts_by_path.get(&path).cloned()),
                    embedded: !entry.is_external(),
                }
            })
            .collect();
        Ok(Self::construct(PackConstructionInput {
            entrypoint: manifest.project().entrypoint().to_owned(),
            metadata: manifest.metadata().cloned(),
            files,
            package_files,
            package_requirements,
            package_requirements_are_declared: true,
            fonts,
        })?)
    }

    /// Reads a pack from a byte buffer.
    pub fn from_bytes(bytes: impl Into<PackArchiveBytes>) -> Result<Self, PackReadError> {
        let bytes = bytes.into();
        Self::from_archive_bytes(&bytes)
    }

    /// Reads a Pack Archive without taking its exact retry bytes.
    pub fn from_archive_bytes(bytes: &PackArchiveBytes) -> Result<Self, PackReadError> {
        Self::read(Cursor::new(bytes.as_slice()))
    }

    /// Writes the pack archive to a seekable writer.
    pub fn write<W: Write + Seek>(&self, writer: W) -> Result<(), PackWriteError> {
        for path in self.files.keys() {
            validate_archive_entry_name(PROJECT_PREFIX.len() + path.as_str().len())?;
        }
        for package in self.packages.values() {
            let spec = &package.spec;
            let version = spec.version.to_string();
            let package_prefix_len =
                PACKAGES_PREFIX.len() + spec.namespace.len() + spec.name.len() + version.len() + 3;
            for path in package.files.keys() {
                validate_archive_entry_name(package_prefix_len + path.as_str().len())?;
            }
        }

        let mut zip = ZipWriter::new(writer);
        let manifest = self.archive_manifest().to_toml();

        zip.start_file(MANIFEST_PATH, zip_file_options(manifest.len()))?;
        zip.write_all(manifest.as_bytes())?;

        for (path, data) in &self.files {
            zip.start_file(
                format!("{PROJECT_PREFIX}{path}"),
                zip_file_options(data.len()),
            )?;
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
                    zip_file_options(data.len()),
                )?;
                zip.write_all(data)?;
            }
        }

        let mut written = std::collections::BTreeSet::new();
        for font in &self.fonts {
            let path = font_archive_path(font.identity.container(), Some(font.data()));
            if written.insert(path.clone()) {
                zip.start_file(&path, zip_file_options(font.data().len()))?;
                zip.write_all(font.data())?;
            }
        }

        zip.finish()?;
        Ok(())
    }

    /// Serializes the pack archive to a byte buffer.
    pub fn to_bytes(&self) -> Result<PackArchiveBytes, PackWriteError> {
        let mut buffer = Cursor::new(Vec::new());
        self.write(&mut buffer)?;
        Ok(PackArchiveBytes::from(buffer.into_inner()))
    }

    fn archive_manifest(&self) -> PackManifest {
        let fonts = self
            .font_catalog
            .iter()
            .map(|face| {
                let embedded = self
                    .fonts
                    .iter()
                    .find(|font| font.identity == face.identity);
                let requirement = self
                    .font_requirements
                    .iter()
                    .find(|requirement| requirement.container == face.identity.container)
                    .expect("Pack Font Catalog requirement invariant violated");
                FontManifest::new(
                    font_archive_path(
                        face.identity.container,
                        embedded.map(|font| font.data.as_slice()),
                    ),
                    face.identity.index,
                    embedded
                        .map(|font| vec![font.info().family.to_string()])
                        .unwrap_or_default(),
                    !face.embedded,
                    face.identity.container.encode(),
                    requirement.length,
                )
            })
            .collect();
        PackManifest::new(
            self.entrypoint.to_string(),
            self.package_requirements
                .iter()
                .filter(|requirement| requirement.embedded)
                .map(package_requirement_manifest)
                .collect(),
            self.package_requirements
                .iter()
                .filter(|requirement| !requirement.embedded)
                .map(package_requirement_manifest)
                .collect(),
            fonts,
            self.metadata.clone(),
        )
    }
}

/// A Pack-owned failure to materialize its exact Font Catalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FontCatalogError {
    #[error("exact font containers {containers:?} are unavailable")]
    Missing {
        containers: Vec<FontContainerIdentity>,
    },
    #[error("font container fulfillment does not match {expected:?}")]
    Mismatched {
        expected: FontContainerIdentity,
        actual: FontContainerIdentity,
        expected_length: u64,
        actual_length: u64,
    },
    #[error("font container {container:?} has no valid face at index {index}")]
    Malformed {
        container: FontContainerIdentity,
        index: u32,
    },
}

/// A Pack-owned failure to materialize exact Package Trees.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PackageFulfillmentError {
    #[error("exact package trees {packages:?} are unavailable")]
    Missing { packages: Vec<PackageSpec> },
    #[error("package fulfillment for {spec} does not match its Package Tree identity")]
    Mismatched {
        spec: PackageSpec,
        expected: PackageTreeIdentity,
        actual: PackageTreeIdentity,
        expected_file_count: u64,
        actual_file_count: u64,
        expected_byte_length: u64,
        actual_byte_length: u64,
    },
    #[error("package fulfillment for {spec} has malformed path `{path}`: {message}")]
    Malformed {
        spec: PackageSpec,
        path: String,
        message: String,
    },
}

/// A Pack-owned failure to construct a complete Compilation Dependency Snapshot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CompilationDependencySnapshotError {
    #[error(transparent)]
    Package(Box<PackageFulfillmentError>),
    #[error(transparent)]
    Font(FontCatalogError),
}

fn package_tree_identity(
    files: &BTreeMap<CanonicalPath, SharedBytes>,
) -> (PackageTreeIdentity, u64, u64) {
    crate::package_catalog::derive_package_tree_identity(
        files.iter().map(|(path, data)| (path.as_str(), data)),
    )
}

fn package_manifest_requirement(
    manifest: &PackageManifest,
    embedded: bool,
) -> Result<(String, PackageRequirement), PackInvariantIssue> {
    let spec = manifest.spec().map_err(|error| match error {
        PackManifestError::InvalidPackageSpec { spec, message } => {
            PackInvariantIssue::InvalidPackageSpec { spec, message }
        }
        error => PackInvariantIssue::InvalidPackageRequirement {
            spec: error.to_string(),
        },
    })?;
    if manifest.tree_identity_kind() != PACKAGE_TREE_IDENTITY_KIND
        || manifest.tree_identity_schema() != PACKAGE_TREE_IDENTITY_SCHEMA
        || manifest.tree_identity_algorithm() != PACKAGE_TREE_IDENTITY_ALGORITHM
        || manifest.file_count() == 0
    {
        return Err(PackInvariantIssue::InvalidPackageRequirement {
            spec: spec.to_string(),
        });
    }
    let tree = PackageTreeIdentity::decode(manifest.tree_digest()).ok_or_else(|| {
        PackInvariantIssue::InvalidPackageRequirement {
            spec: spec.to_string(),
        }
    })?;
    let key = spec.to_string();
    Ok((
        key,
        PackageRequirement {
            spec,
            tree,
            file_count: manifest.file_count(),
            byte_length: manifest.byte_length(),
            embedded,
        },
    ))
}

fn package_requirement_manifest(requirement: &PackageRequirement) -> PackageManifest {
    PackageManifest::new(
        requirement.spec.clone(),
        requirement.tree.encode(),
        requirement.file_count,
        requirement.byte_length,
    )
}

fn zip_file_options(size: usize) -> SimpleFileOptions {
    // Deflate may expand incompressible input. Nine bits per input byte plus
    // framing is a conservative bound for the configured encoder.
    let compressed_bound = size.saturating_add(size.div_ceil(8)).saturating_add(16);
    let compressed_bound = u64::try_from(compressed_bound).unwrap_or(u64::MAX);
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(compressed_bound > zip::ZIP64_BYTES_THR)
}

pub(crate) fn font_archive_path(identity: FontContainerIdentity, data: Option<&[u8]>) -> String {
    let extension = match data.and_then(|data| data.get(..4)) {
        Some(b"OTTO") => "otf",
        Some(b"ttcf") => "ttc",
        Some(_) => "ttf",
        None => "font",
    };
    format!("fonts/{}.{extension}", identity.encode())
}

struct RawCentralEntry {
    name: Vec<u8>,
    utf8: bool,
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
        let flags = u16::from_le_bytes([fixed[4], fixed[5]]);
        let name_len = u16::from_le_bytes([fixed[24], fixed[25]]) as usize;
        let extra_len = u16::from_le_bytes([fixed[26], fixed[27]]) as i64;
        let comment_len = u16::from_le_bytes([fixed[28], fixed[29]]) as i64;
        let mut name = vec![0; name_len];
        reader.read_exact(&mut name)?;
        reader.seek(SeekFrom::Current(extra_len + comment_len))?;
        entries.push(RawCentralEntry {
            name,
            utf8: flags & (1 << 11) != 0,
        });
    }
    Ok(entries)
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
    Ok((spec, path.trim_start_matches('/').to_owned()))
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
    #[error("the archive contains a malformed UTF-8 entry name {0:?}")]
    InvalidUtf8EntryName(Vec<u8>),
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
    #[error("invalid archive entry {entry:?}: {message:?}")]
    InvalidEntry { entry: String, message: String },
    #[error("archive entry {0:?} is not a regular file")]
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
    files: Vec<ProjectFileInput>,
    package_files: Vec<PackageFileInput>,
    fonts: Vec<PackFontInput>,
    metadata: Option<PackMetadata>,
}

impl PackBuilder {
    /// Creates a builder for a pack with the given entrypoint path.
    pub fn new(entrypoint: impl Into<String>) -> Self {
        Self {
            entrypoint: entrypoint.into(),
            files: Vec::new(),
            package_files: Vec::new(),
            fonts: Vec::new(),
            metadata: None,
        }
    }

    /// Adds a project file under a root-relative path.
    pub fn file(
        self,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        self.shared_file(path, SharedBytes::new(data.into()))
    }

    pub(crate) fn shared_file(
        mut self,
        path: impl AsRef<str>,
        data: SharedBytes,
    ) -> Result<Self, PackBuildError> {
        self.files.push(ProjectFileInput {
            path: path.as_ref().to_owned(),
            data,
        });
        Ok(self)
    }

    /// Adds a file of a vendored package.
    pub fn package_file(
        self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        self.shared_package_file(spec, path, SharedBytes::new(data.into()))
    }

    pub(crate) fn shared_package_file(
        mut self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: SharedBytes,
    ) -> Result<Self, PackBuildError> {
        self.package_files.push(PackageFileInput {
            spec,
            path: path.as_ref().to_owned(),
            data,
            embedded: true,
        });
        Ok(self)
    }

    /// Adds a file to an exact Package Tree fulfilled outside the Pack.
    pub fn external_package_file(
        self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, PackBuildError> {
        self.shared_external_package_file(spec, path, SharedBytes::new(data.into()))
    }

    pub(crate) fn shared_external_package_file(
        mut self,
        spec: PackageSpec,
        path: impl AsRef<str>,
        data: SharedBytes,
    ) -> Result<Self, PackBuildError> {
        self.package_files.push(PackageFileInput {
            spec,
            path: path.as_ref().to_owned(),
            data,
            embedded: false,
        });
        Ok(self)
    }

    /// Embeds a font file.
    ///
    /// `index` is the face index for font collections and zero otherwise. The
    /// entry name and family list are derived from the font data.
    pub fn font(self, data: impl Into<Vec<u8>>, index: u32) -> Result<Self, PackBuildError> {
        self.shared_font(SharedBytes::new(data.into()), index)
    }

    pub(crate) fn shared_font(
        mut self,
        data: SharedBytes,
        index: u32,
    ) -> Result<Self, PackBuildError> {
        self.fonts.push(PackFontInput {
            path: None,
            index,
            declared_container_digest: None,
            declared_container_identity_kind: None,
            declared_container_identity_schema: None,
            declared_container_identity_algorithm: None,
            declared_container_length: None,
            data: Some(data),
            embedded: true,
        });
        Ok(self)
    }

    /// Records an exact font dependency without storing its container bytes.
    pub fn external_font(
        self,
        data: impl Into<Vec<u8>>,
        index: u32,
    ) -> Result<Self, PackBuildError> {
        self.shared_external_font(SharedBytes::new(data.into()), index)
    }

    pub(crate) fn shared_external_font(
        mut self,
        data: SharedBytes,
        index: u32,
    ) -> Result<Self, PackBuildError> {
        self.fonts.push(PackFontInput {
            path: None,
            index,
            declared_container_digest: None,
            declared_container_identity_kind: None,
            declared_container_identity_schema: None,
            declared_container_identity_algorithm: None,
            declared_container_length: None,
            data: Some(data),
            embedded: false,
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
        Ok(Pack::construct(PackConstructionInput {
            entrypoint: self.entrypoint,
            metadata: self.metadata,
            files: self.files,
            package_files: self.package_files,
            package_requirements: Vec::new(),
            package_requirements_are_declared: false,
            fonts: self.fonts,
        })?)
    }
}

fn canonical_path(role: PackPathRole, path: &str) -> Result<CanonicalPath, PackInvariantIssue> {
    let canonical = canonical_path_without_membership(role, path)?;
    // No route into a Pack can name a Pack as a project file.
    if matches!(role, PackPathRole::ProjectFile | PackPathRole::Entrypoint)
        && names_pack_path(canonical.as_str())
    {
        return Err(PackInvariantIssue::InvalidPath {
            role,
            path: path.to_owned(),
            message: format!("`.{FILE_EXTENSION}` paths are excluded from project membership"),
        });
    }
    Ok(canonical)
}

/// Canonicalizes a path for its role without deciding project membership.
fn canonical_path_without_membership(
    role: PackPathRole,
    path: &str,
) -> Result<CanonicalPath, PackInvariantIssue> {
    let invalid = |message: String| PackInvariantIssue::InvalidPath {
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
    if path.contains('\0') {
        return Err(invalid("path must not contain NUL bytes".to_owned()));
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

fn canonical_archive_name(path: &str) -> Result<String, PackReadError> {
    let prefix_normalized_path = strip_current_directory_prefix(path);
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains('\0')
        || has_windows_drive_prefix(prefix_normalized_path)
    {
        return Err(PackReadError::UnsafeEntry(path.to_owned()));
    }
    let canonical = VirtualPath::new(path)
        .map_err(|_| PackReadError::UnsafeEntry(path.to_owned()))?
        .get_without_slash()
        .to_owned();
    if has_windows_drive_prefix(&canonical) {
        return Err(PackReadError::UnsafeEntry(path.to_owned()));
    }
    Ok(canonical)
}

fn validate_package_spec(spec: &PackageSpec) -> Result<(), PackInvariantIssue> {
    let serialized = spec.to_string();
    let parsed = PackageSpec::from_str(&serialized).map_err(|message| {
        PackInvariantIssue::InvalidPackageSpec {
            spec: serialized.clone(),
            message: message.to_string(),
        }
    })?;
    if parsed != *spec {
        return Err(PackInvariantIssue::InvalidPackageSpec {
            spec: serialized,
            message: "package specification does not round-trip canonically".to_owned(),
        });
    }
    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn validate_archive_entry_name(archive_name_len: usize) -> Result<(), PackWriteError> {
    if archive_name_len > MAX_ZIP_ENTRY_NAME_LEN {
        return Err(PackWriteError::Zip(zip::result::ZipError::InvalidArchive(
            "entry name exceeds ZIP's limit".into(),
        )));
    }
    Ok(())
}

fn strip_current_directory_prefix(mut path: &str) -> &str {
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    path
}

fn find_path_tree_conflicts(
    mut paths: Vec<(CanonicalPath, PackPathRole)>,
) -> Vec<PathTreeConflict> {
    paths.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut conflicts = Vec::new();
    for (ancestor, ancestor_role) in &paths {
        let prefix = format!("{ancestor}/");
        for (descendant, descendant_role) in paths
            .iter()
            .skip(paths.partition_point(|(path, _)| path.as_str() < prefix.as_str()))
            .take_while(|(path, _)| path.as_str().starts_with(&prefix))
        {
            conflicts.push(PathTreeConflict {
                ancestor: ancestor.clone(),
                ancestor_role: *ancestor_role,
                descendant: descendant.clone(),
                descendant_role: *descendant_role,
            });
        }
    }
    conflicts
}

fn path_tree_conflicts(
    paths: impl IntoIterator<Item = CanonicalPath>,
    role: PackPathRole,
) -> Vec<PackInvariantIssue> {
    find_path_tree_conflicts(paths.into_iter().map(|path| (path, role)).collect())
        .into_iter()
        .map(|conflict| PackInvariantIssue::PathTreeConflict {
            ancestor: conflict.ancestor.into_string(),
            ancestor_role: conflict.ancestor_role,
            descendant: conflict.descendant.into_string(),
            descendant_role: conflict.descendant_role,
        })
        .collect()
}

fn reserved_font_archive_role(path: &CanonicalPath) -> Option<&'static str> {
    if is_same_or_descendant(path.as_str(), MANIFEST_PATH) {
        Some("Pack Manifest")
    } else if is_same_or_descendant(path.as_str(), PROJECT_PREFIX.trim_end_matches('/')) {
        Some("project file")
    } else if is_same_or_descendant(path.as_str(), PACKAGES_PREFIX.trim_end_matches('/')) {
        Some("package file")
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
) -> Result<(), PackReadError> {
    if let Some(first_entry) = entries.get(&canonical) {
        if first_entry == raw_name {
            return Ok(());
        }
        return Err(PackReadError::AmbiguousArchiveEntries);
    }
    entries.insert(canonical, raw_name.to_owned());
    Ok(())
}

/// A failure while building a pack in memory.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackBuildError {
    #[error(transparent)]
    Invariant(#[from] PackInvariantError),
}

/// One independently detectable violation of a whole-Pack invariant.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackInvariantIssue {
    /// A path cannot identify a canonical file for its declared role.
    #[error("invalid {role} path {path:?}: {message:?}")]
    InvalidPath {
        role: PackPathRole,
        path: String,
        message: String,
    },
    /// A package value cannot be represented as a canonical package specification.
    #[error("invalid package spec {spec:?}: {message:?}")]
    InvalidPackageSpec { spec: String, message: String },
    /// A Package Requirement has a malformed or unsupported tree identity.
    #[error("package requirement {spec:?} has an invalid Package Tree identity")]
    InvalidPackageRequirement { spec: String },
    /// Embedded package bytes disagree with their declared tree identity.
    #[error("embedded package {spec:?} does not match its declared Package Tree identity")]
    MismatchedEmbeddedPackageIdentity { spec: String },
    /// Two project entries identify one canonical path.
    #[error("project path {path:?} is supplied more than once")]
    DuplicateProjectPath { path: String },
    /// Two package entries identify one canonical path.
    #[error("package {package} path {path:?} is supplied more than once")]
    DuplicatePackagePath { package: PackageSpec, path: String },
    /// One exact Package Requirement is declared more than once in one role.
    #[error("package requirement {spec:?} is declared more than once")]
    DuplicatePackageRequirement { spec: String, embedded: bool },
    /// One file path is an ancestor of another file path in the same tree.
    #[error(
        "{ancestor_role} path {ancestor:?} conflicts with {descendant_role} descendant {descendant:?}"
    )]
    PathTreeConflict {
        ancestor: String,
        ancestor_role: PackPathRole,
        descendant: String,
        descendant_role: PackPathRole,
    },
    /// One package file path is an ancestor of another file path in that package.
    #[error(
        "package {package:?} {ancestor_role} path {ancestor:?} conflicts with {descendant_role} descendant {descendant:?}"
    )]
    PackagePathTreeConflict {
        package: String,
        ancestor: String,
        ancestor_role: PackPathRole,
        descendant: String,
        descendant_role: PackPathRole,
    },
    /// A package was declared both vendored and unvendored.
    #[error("package {spec:?} cannot be both vendored and unvendored")]
    PackageRoleConflict { spec: String },
    /// Package bytes exist without a matching vendored declaration.
    #[error("package {spec:?} has contained data but is not declared vendored")]
    UndeclaredPackageData { spec: String },
    /// A vendored package declaration has no contained bytes.
    #[error("vendored package {spec:?} has no contained data")]
    MissingVendoredPackageData { spec: String },
    /// A font declaration has no contained bytes.
    #[error("font data {path:?} is missing")]
    MissingFontData { path: String },
    /// Contained font bytes do not contain the declared face.
    #[error("font data {path:?} does not contain a valid face at index {index}")]
    InvalidFontData { path: String, index: u32 },
    /// An external font declaration has no valid exact container identity.
    #[error("external font {path:?} has an invalid container identity or length")]
    InvalidExternalFontIdentity { path: String },
    /// Embedded font bytes disagree with their declared exact identity.
    #[error("embedded font {path:?} does not match its declared container identity")]
    MismatchedEmbeddedFontIdentity { path: String },
    /// Faces from one exact container disagree about its length or fulfillment role.
    #[error("font {path:?} conflicts with another declaration for the same container")]
    InconsistentFontContainer { path: String },
    /// An externally fulfilled declaration also has bytes in the Pack.
    #[error("external font {path:?} cannot also have contained data")]
    ExternalFontHasContainedData { path: String },
    /// The same contained font face was declared more than once.
    #[error("font {path:?} declares face index {index} more than once")]
    DuplicateFontFace { path: String, index: u32 },
    /// The declared entrypoint is not present among the packed project files.
    #[error("entrypoint {path:?} is not a contained project file")]
    MissingEntrypoint { path: String },
}

impl PackInvariantIssue {
    fn sort_key(&self) -> (u8, String, u8, u64, String) {
        match self {
            Self::InvalidPath {
                role: PackPathRole::Entrypoint,
                path,
                ..
            } => (3, path.clone(), 0, 0, String::new()),
            Self::InvalidPath { role, path, .. } => {
                (role_sort_rank(*role), path.clone(), 0, 0, String::new())
            }
            Self::DuplicateProjectPath { path } => (0, path.clone(), 1, 0, String::new()),
            Self::PathTreeConflict {
                ancestor,
                ancestor_role,
                descendant,
                ..
            } => (
                role_sort_rank(*ancestor_role),
                ancestor.clone(),
                2,
                0,
                descendant.clone(),
            ),
            Self::InvalidPackageSpec { spec, .. } => (1, spec.clone(), 0, 0, String::new()),
            Self::DuplicatePackagePath { package, path } => {
                (1, package.to_string(), 1, 0, path.clone())
            }
            Self::PackagePathTreeConflict {
                package,
                ancestor,
                descendant,
                ..
            } => (
                1,
                package.clone(),
                2,
                0,
                format!("{ancestor}\0{descendant}"),
            ),
            Self::DuplicatePackageRequirement { spec, embedded } => {
                (1, spec.clone(), 3, u64::from(*embedded), String::new())
            }
            Self::InvalidPackageRequirement { spec } => (1, spec.clone(), 4, 0, String::new()),
            Self::MismatchedEmbeddedPackageIdentity { spec } => {
                (1, spec.clone(), 5, 0, String::new())
            }
            Self::PackageRoleConflict { spec } => (1, spec.clone(), 6, 0, String::new()),
            Self::UndeclaredPackageData { spec } => (1, spec.clone(), 7, 0, String::new()),
            Self::MissingVendoredPackageData { spec } => (1, spec.clone(), 8, 0, String::new()),
            Self::MissingFontData { path } => (2, path.clone(), 0, 0, String::new()),
            Self::InvalidFontData { path, index } => {
                (2, path.clone(), 1, u64::from(*index), String::new())
            }
            Self::InvalidExternalFontIdentity { path } => (2, path.clone(), 2, 0, String::new()),
            Self::MismatchedEmbeddedFontIdentity { path } => (2, path.clone(), 3, 0, String::new()),
            Self::InconsistentFontContainer { path } => (2, path.clone(), 4, 0, String::new()),
            Self::ExternalFontHasContainedData { path } => (2, path.clone(), 5, 0, String::new()),
            Self::DuplicateFontFace { path, index } => {
                (2, path.clone(), 6, u64::from(*index), String::new())
            }
            Self::MissingEntrypoint { path } => (3, path.clone(), 1, 0, String::new()),
        }
    }
}

fn role_sort_rank(role: PackPathRole) -> u8 {
    match role {
        PackPathRole::Entrypoint => 3,
        PackPathRole::ProjectFile => 0,
        PackPathRole::PackageFile => 1,
        PackPathRole::FontData => 2,
    }
}

/// A violation of the invariants shared by every [`Pack`] construction path.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("Pack construction failed with {} issue(s)", .issues.len())]
pub struct PackInvariantError {
    issues: Vec<PackInvariantIssue>,
}

impl PackInvariantError {
    /// Every independently detectable issue in canonical domain order.
    pub fn issues(&self) -> &[PackInvariantIssue] {
        &self.issues
    }
}

/// The role a path plays in a Pack invariant.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PackPathRole {
    Entrypoint,
    ProjectFile,
    PackageFile,
    FontData,
}

impl std::fmt::Display for PackPathRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Entrypoint => "entrypoint",
            Self::ProjectFile => "project file",
            Self::PackageFile => "package file",
            Self::FontData => "font data",
        })
    }
}
