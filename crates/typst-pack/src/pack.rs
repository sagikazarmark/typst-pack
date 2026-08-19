//! The validated in-memory Pack model.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst::text::{Font, FontInfo};

use crate::manifest::PackMetadata;
use crate::paths::{
    CanonicalPath, canonical_relative_path, path_tree_conflicts as shared_path_tree_conflicts,
};
use crate::payload::SharedBytes;
use crate::{CanonicalIdentity, CanonicalIdentityRole, FontContainer, PackageTree};

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

/// A portable pack of a Typst project.
///
/// A pack holds project files (sources, images, and data files), optionally
/// package files and fonts. Every project path has contained bytes.
/// Its archive form is a Zip file with a `typst-pack.toml`
/// manifest, conventionally named `*.typk`.
#[derive(Debug, Clone)]
pub struct Pack {
    identity: CanonicalIdentity,
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

#[derive(Debug, Clone)]
pub(crate) struct PackageFiles {
    pub(crate) spec: PackageSpec,
    files: BTreeMap<CanonicalPath, SharedBytes>,
}

impl PackageFiles {
    pub(crate) fn file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }

    fn from_validated_tree(spec: PackageSpec, tree: PackageTree) -> Self {
        Self {
            spec,
            files: tree
                .into_shared_files()
                .into_iter()
                .map(|(path, data)| (CanonicalPath::from_canonical(path), data))
                .collect(),
        }
    }
}

/// Exact verified dependencies accepted by the synchronous Compilation Kernel.
pub(crate) struct CompilationDependencySnapshot {
    pack_identity: CanonicalIdentity,
    packages: BTreeMap<String, PackageFiles>,
    font_catalog: Vec<Font>,
}

impl CompilationDependencySnapshot {
    pub(crate) fn pack_identity(&self) -> CanonicalIdentity {
        self.pack_identity
    }

    pub(crate) fn into_parts(self) -> (BTreeMap<String, PackageFiles>, Vec<Font>) {
        (self.packages, self.font_catalog)
    }
}

/// One exact package specification and Package Tree identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequirement {
    spec: PackageSpec,
    tree: CanonicalIdentity,
    file_count: u64,
    byte_length: u64,
    embedded: bool,
}

impl PackageRequirement {
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }
    pub fn tree_identity(&self) -> CanonicalIdentity {
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

/// A font embedded in a pack.
#[derive(Debug, Clone)]
pub struct PackFont {
    identity: FontFaceIdentity,
    data: SharedBytes,
    font: Font,
}

pub(crate) fn font_container_identity(data: &[u8]) -> CanonicalIdentity {
    CanonicalIdentity::for_font_container_bytes(data)
}

pub(crate) fn font_container_path(identity: CanonicalIdentity, data: Option<&[u8]>) -> String {
    let extension = match data.and_then(|data| data.get(..4)) {
        Some(b"OTTO") => "otf",
        Some(b"ttcf") => "ttc",
        Some(_) => "ttf",
        None => "font",
    };
    format!("fonts/{}.{extension}", identity.encode())
}

/// The exact identity of one face within a Font Container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontFaceIdentity {
    container: CanonicalIdentity,
    index: u32,
}

impl FontFaceIdentity {
    /// The face at a container-local index within the given container.
    pub(crate) fn new(container: CanonicalIdentity, index: u32) -> Self {
        Self { container, index }
    }

    /// The containing font file or collection.
    pub fn container(self) -> CanonicalIdentity {
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
    container: CanonicalIdentity,
    length: u64,
    face_indices: Vec<u32>,
    embedded: bool,
}

impl FontRequirement {
    pub fn container_identity(&self) -> CanonicalIdentity {
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

    pub(crate) fn shared_data(&self) -> &SharedBytes {
        &self.data
    }

    /// Official selection metadata derived from the verified container bytes.
    pub fn info(&self) -> &FontInfo {
        self.font.info()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PackFontInput {
    pub(crate) source: PackFontSourceInput,
    pub(crate) index: u32,
    pub(crate) embedded: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum PackFontSourceInput {
    ExactBytes(SharedBytes),
    Declared {
        label: String,
        identity: DeclaredFontContainerIdentity,
        length: Option<u64>,
        data: Option<SharedBytes>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeclaredFontContainerIdentity {
    Absent,
    Partial(Option<CanonicalIdentity>),
    Valid(CanonicalIdentity),
    Invalid,
}

#[derive(Debug)]
pub(crate) struct ProjectFileInput {
    pub(crate) path: String,
    pub(crate) data: SharedBytes,
}

#[derive(Debug)]
pub(crate) struct PackageFileInput {
    pub(crate) spec: PackageSpec,
    pub(crate) path: String,
    pub(crate) data: SharedBytes,
    pub(crate) embedded: bool,
}

#[derive(Debug)]
pub(crate) struct PackageRequirementInput {
    pub(crate) spec: Result<PackageSpec, InvalidPackageSpecInput>,
    pub(crate) tree: Option<CanonicalIdentity>,
    pub(crate) file_count: u64,
    pub(crate) byte_length: u64,
    pub(crate) embedded: bool,
}

#[derive(Debug)]
pub(crate) struct InvalidPackageSpecInput {
    pub(crate) spec: String,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) enum PackageRequirementsInput {
    Inferred,
    Declared(Vec<PackageRequirementInput>),
}

#[derive(Debug)]
pub(crate) struct PackConstructionInput {
    pub(crate) entrypoint: String,
    pub(crate) metadata: Option<PackMetadata>,
    pub(crate) files: Vec<ProjectFileInput>,
    pub(crate) package_files: Vec<PackageFileInput>,
    pub(crate) package_requirements: PackageRequirementsInput,
    pub(crate) fonts: Vec<PackFontInput>,
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder::new(entrypoint)
    }

    pub(crate) fn construct(input: PackConstructionInput) -> Result<Self, PackInvariantError> {
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
            canonical_files.keys(),
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
            let conflicts = shared_path_tree_conflicts(
                files
                    .files
                    .keys()
                    .map(|path| (path, PackPathRole::PackageFile)),
            );
            if !conflicts.is_empty() {
                invalid_package_groups.insert((package.clone(), *embedded));
            }
            for conflict in conflicts {
                issues.push(PackInvariantIssue::PackagePathTreeConflict {
                    package: package.clone(),
                    ancestor: conflict.ancestor.to_string(),
                    ancestor_role: conflict.ancestor_role,
                    descendant: conflict.descendant.to_string(),
                    descendant_role: conflict.descendant_role,
                });
            }
        }

        let declared_inputs = match input.package_requirements {
            PackageRequirementsInput::Inferred => None,
            PackageRequirementsInput::Declared(entries) => Some(entries),
        };
        let declarations_are_explicit = declared_inputs.is_some();
        let mut declared_requirements = BTreeMap::<(String, bool), Vec<PackageRequirement>>::new();
        let mut declared_requirement_roles = BTreeSet::new();
        let mut duplicate_requirements = BTreeSet::new();
        for declaration in declared_inputs.unwrap_or_default() {
            let spec = match declaration.spec {
                Ok(spec) => spec,
                Err(error) => {
                    issues.push(PackInvariantIssue::InvalidPackageSpec {
                        spec: error.spec,
                        message: error.message,
                    });
                    continue;
                }
            };
            let role = (spec.to_string(), declaration.embedded);
            if !declared_requirement_roles.insert(role.clone()) {
                duplicate_requirements.insert(role.clone());
            }
            let Some(tree) = declaration.tree.filter(|_| declaration.file_count > 0) else {
                issues.push(PackInvariantIssue::InvalidPackageRequirement {
                    spec: spec.to_string(),
                });
                continue;
            };
            declared_requirements
                .entry(role)
                .or_default()
                .push(PackageRequirement {
                    spec,
                    tree,
                    file_count: declaration.file_count,
                    byte_length: declaration.byte_length,
                    embedded: declaration.embedded,
                });
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
                if *embedded && let Some(package) = package_groups.get(&(spec.clone(), true)) {
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
        let mut declared_font_paths = BTreeSet::new();
        for (position, entry) in input.fonts.into_iter().enumerate() {
            let (path, data, declared_identity, declared_length, exact_bytes) = match entry.source {
                PackFontSourceInput::ExactBytes(data) => (
                    format!("font input {position}"),
                    Some(data),
                    DeclaredFontContainerIdentity::Absent,
                    None,
                    true,
                ),
                PackFontSourceInput::Declared {
                    label,
                    identity,
                    length,
                    data,
                } => {
                    match canonical_path(PackPathRole::FontData, &label) {
                        Ok(path) => {
                            declared_font_paths.insert(path);
                        }
                        Err(issue) => issues.push(issue),
                    }
                    (label, data, identity, length, false)
                }
            };
            let index = entry.index;
            let embedded = entry.embedded;
            let parsed_data = data.as_ref().and_then(|data| {
                Font::new(data.to_typst(), index).map(|font| {
                    let container = font_container_identity(data.as_slice());
                    (data.clone(), font, container, data.len() as u64)
                })
            });
            let (data, parsed, container, length) = if embedded {
                let Some((data, parsed, container, length)) = parsed_data else {
                    issues.push(if data.is_some() {
                        PackInvariantIssue::InvalidFontData { path, index }
                    } else {
                        PackInvariantIssue::MissingFontData { path }
                    });
                    continue;
                };
                if matches!(declared_identity, DeclaredFontContainerIdentity::Invalid)
                    || matches!(
                        declared_identity,
                        DeclaredFontContainerIdentity::Valid(declared)
                            | DeclaredFontContainerIdentity::Partial(Some(declared))
                            if declared != container
                    )
                    || declared_length.is_some_and(|declared| declared != length)
                {
                    issues.push(PackInvariantIssue::MismatchedEmbeddedFontIdentity {
                        path: path.clone(),
                    });
                }
                (Some(data), Some(parsed), container, length)
            } else if exact_bytes {
                let Some((_, _, container, length)) = parsed_data else {
                    issues.push(PackInvariantIssue::InvalidFontData { path, index });
                    continue;
                };
                (None, None, container, length)
            } else {
                if data.is_some() {
                    issues.push(PackInvariantIssue::ExternalFontHasContainedData {
                        path: path.clone(),
                    });
                }
                let DeclaredFontContainerIdentity::Valid(container) = declared_identity else {
                    issues.push(PackInvariantIssue::InvalidExternalFontIdentity { path });
                    continue;
                };
                let Some(length) = declared_length.filter(|length| *length > 0) else {
                    issues.push(PackInvariantIssue::InvalidExternalFontIdentity { path });
                    continue;
                };
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
        issues.extend(path_tree_conflicts(
            &declared_font_paths,
            PackPathRole::FontData,
        ));
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

        let entrypoint = entrypoint.expect("a valid Pack has a canonical entrypoint");
        let identity = pack_identity(
            &entrypoint,
            &canonical_files,
            &package_requirements,
            &font_catalog,
        );
        Ok(Self {
            identity,
            entrypoint,
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
    pub fn identity(&self) -> CanonicalIdentity {
        self.identity
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

    /// The fonts embedded in the pack.
    pub fn fonts(&self) -> &[PackFont] {
        &self.fonts
    }

    /// The exact Pack Font Catalog faces exposed to official Typst, in stable order.
    pub fn font_catalog(&self) -> &[PackFontCatalogFace] {
        &self.font_catalog
    }

    /// The exact Font Containers required by this Pack.
    pub fn font_requirements(&self) -> &[FontRequirement] {
        &self.font_requirements
    }

    pub(crate) fn materialize_compilation_dependency_snapshot(
        &self,
        mut package_fulfillments: BTreeMap<String, PackageTree>,
        font_fulfillments: BTreeMap<CanonicalIdentity, FontContainer>,
    ) -> CompilationDependencySnapshot {
        let mut packages = self.packages.clone();
        for requirement in self
            .package_requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
        {
            let key = requirement.spec.to_string();
            let tree = package_fulfillments
                .remove(&key)
                .expect("exact fulfillment verification supplies every external Package Tree");
            packages.insert(
                key,
                PackageFiles::from_validated_tree(requirement.spec.clone(), tree),
            );
        }
        let font_catalog = self
            .font_catalog
            .iter()
            .map(|face| {
                let identity = face.identity;
                if face.embedded {
                    self.fonts
                        .iter()
                        .find(|font| font.identity == identity)
                        .expect("Pack Font Catalog embedded face invariant violated")
                        .font
                        .clone()
                } else {
                    font_fulfillments[&identity.container]
                        .font(identity.index)
                        .expect("exact validated Font Container holds every required face")
                }
            })
            .collect();
        CompilationDependencySnapshot {
            pack_identity: self.identity(),
            packages,
            font_catalog,
        }
    }
}

fn pack_identity(
    entrypoint: &CanonicalPath,
    files: &BTreeMap<CanonicalPath, SharedBytes>,
    package_requirements: &[PackageRequirement],
    font_catalog: &[PackFontCatalogFace],
) -> CanonicalIdentity {
    let project_files = files
        .iter()
        .map(|(path, data)| (path.as_str(), typst::utils::hash128(data)))
        .collect::<Vec<_>>();
    let packages = package_requirements
        .iter()
        .map(|requirement| {
            (
                requirement.spec.to_string(),
                requirement.tree,
                requirement.file_count,
                requirement.byte_length,
                requirement.embedded,
            )
        })
        .collect::<Vec<_>>();
    let fonts = font_catalog
        .iter()
        .map(|face| (face.identity.container, face.identity.index, face.embedded))
        .collect::<Vec<_>>();
    CanonicalIdentity::from_digest(
        CanonicalIdentityRole::Pack,
        typst::utils::hash128(&(
            "typst-pack-identity-v1",
            entrypoint.as_str(),
            project_files,
            packages,
            fonts,
        )),
    )
}

fn package_tree_identity(
    files: &BTreeMap<CanonicalPath, SharedBytes>,
) -> (CanonicalIdentity, u64, u64) {
    crate::package_catalog::derive_package_tree_identity(
        files.iter().map(|(path, data)| (path.as_str(), data)),
    )
}

/// Builds a [`Pack`] from in-memory data.
///
/// This is the constructor to use when the project does not live on a file
/// system, for example in a web editor. For packing a project directory, use
/// `FilesystemPackAssembler` instead (requires the `fs` feature).
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
            source: PackFontSourceInput::ExactBytes(data),
            index,
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
            source: PackFontSourceInput::ExactBytes(data),
            index,
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
            package_requirements: PackageRequirementsInput::Inferred,
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
    canonical_relative_path(path).map_err(|error| invalid(error.to_string()))
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

fn path_tree_conflicts<'a>(
    paths: impl IntoIterator<Item = &'a CanonicalPath>,
    role: PackPathRole,
) -> Vec<PackInvariantIssue> {
    shared_path_tree_conflicts(paths.into_iter().map(|path| (path, role)))
        .into_iter()
        .map(|conflict| PackInvariantIssue::PathTreeConflict {
            ancestor: conflict.ancestor.to_string(),
            ancestor_role: conflict.ancestor_role,
            descendant: conflict.descendant.to_string(),
            descendant_role: conflict.descendant_role,
        })
        .collect()
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
