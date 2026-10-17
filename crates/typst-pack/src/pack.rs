//! The validated in-memory Pack model.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typst::syntax::package::PackageSpec;
use typst::text::{Font, FontStretch, FontStyle, FontWeight};

use crate::identity::Preimage;
use crate::manifest::PackMetadata;
use crate::paths::{CanonicalPath, canonical_relative_path, path_tree_conflicts};
use crate::payload::SharedBytes;
use crate::{Digest, FontContainer, PackageTree, PackageTreeError};

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
/// A pack holds project files (sources, images, and data files), the packages
/// and font containers the document needs, either embedded or declared by
/// digest for external fulfillment, and optional metadata. Its archive form is
/// a ZIP file with a `typst-pack.toml` manifest, conventionally named `*.typk`.
#[derive(Debug, Clone)]
pub struct Pack {
    identity: Digest,
    entrypoint: CanonicalPath,
    metadata: Option<PackMetadata>,
    files: BTreeMap<CanonicalPath, (SharedBytes, Digest)>,
    /// Embedded package trees, keyed by spec string.
    packages: BTreeMap<String, PackageTree>,
    package_requirements: Vec<PackageRequirement>,
    font_requirements: Vec<FontRequirement>,
    /// Embedded font container bytes.
    font_data: BTreeMap<Digest, SharedBytes>,
}

/// One exact package a Pack needs: its specification, its tree digest, and
/// whether the tree is embedded in the Pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequirement {
    pub spec: PackageSpec,
    /// The package tree digest.
    pub digest: Digest,
    /// Whether the tree's files are stored in the Pack.
    pub embedded: bool,
}

/// One exact font container a Pack needs. Every face of the container is
/// available to the document at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontRequirement {
    /// The SHA-256 of the container bytes.
    pub digest: Digest,
    /// Whether the container's bytes are stored in the Pack.
    pub embedded: bool,
    /// The faces of the container, in container order.
    pub faces: Vec<FontFace>,
}

/// Selection metadata of one face of a font container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFace {
    /// The face index within the container.
    pub index: u32,
    /// The typographic family name.
    pub family: String,
    pub style: FontStyle,
    pub weight: FontWeight,
    pub stretch: FontStretch,
}

impl FontFace {
    pub(crate) fn of(font: &Font) -> Self {
        let info = font.info();
        Self {
            index: font.index(),
            family: info.family.clone(),
            style: info.variant.style,
            weight: info.variant.weight,
            stretch: info.variant.stretch,
        }
    }
}

impl fmt::Display for FontFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = match self.style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        };
        write!(
            formatter,
            "{} ({style}, weight {}, stretch {:?})",
            self.family,
            self.weight.to_number(),
            self.stretch.to_ratio()
        )
    }
}

impl FontRequirement {
    /// The family names of the container's faces, deduplicated, for messages.
    pub fn families(&self) -> String {
        let mut families = Vec::<&str>::new();
        for face in &self.faces {
            if !families.contains(&face.family.as_str()) {
                families.push(&face.family);
            }
        }
        if families.is_empty() {
            "unknown family".to_owned()
        } else {
            families.join(", ")
        }
    }
}

/// Exact verified dependencies accepted by the synchronous Compilation Kernel.
pub(crate) struct CompilationDependencySnapshot {
    pub(crate) pack_identity: Digest,
    pub(crate) packages: BTreeMap<String, PackageTree>,
    pub(crate) fonts: Vec<Font>,
}

/// The unvalidated content of a Pack, shared by the builder, creation, and
/// the archive decoder.
#[derive(Debug, Default)]
pub(crate) struct PackInput {
    pub(crate) entrypoint: String,
    pub(crate) metadata: Option<PackMetadata>,
    pub(crate) files: Vec<(String, SharedBytes)>,
    pub(crate) packages: Vec<PackageInput>,
    pub(crate) fonts: Vec<FontInput>,
}

#[derive(Debug)]
pub(crate) struct PackageInput {
    pub(crate) spec: PackageSpec,
    pub(crate) embedded: bool,
    pub(crate) content: PackageContent,
}

#[derive(Debug)]
pub(crate) enum PackageContent {
    Files(Vec<(String, SharedBytes)>),
    /// A tree digest declared without its files; only for external packages.
    Declared(Digest),
}

#[derive(Debug)]
pub(crate) enum FontInput {
    Bytes {
        data: SharedBytes,
        embedded: bool,
    },
    /// An external container declared without its bytes.
    Declared {
        digest: Digest,
        faces: Vec<FontFace>,
    },
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    ///
    /// Building directly does not run Dependency Discovery: the pack contains
    /// exactly the files, packages, and fonts added here. Use
    /// [`create`](crate::create) when the library should discover requirements
    /// from values the caller already holds.
    ///
    /// ```
    /// use typst_pack::Pack;
    /// use typst_pack::pack_archive::encode;
    ///
    /// let pack = Pack::builder("main.typ")
    ///     .file("main.typ", b"= Report\n".to_vec())
    ///     .file("data/figures.csv", b"quarter,revenue\nQ1,120\n".to_vec())
    ///     .build()?;
    ///
    /// assert_eq!(pack.entrypoint(), "main.typ");
    ///
    /// let archive = encode(&pack)?;
    /// assert!(!archive.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder::new(entrypoint)
    }

    /// Validates every whole-Pack invariant. This is the one constructor shared
    /// by the builder, creation, and the archive decoder.
    pub(crate) fn construct(input: PackInput) -> Result<Self, PackInvariantError> {
        let mut issues = Vec::new();
        let files = validate_project(input.files, &mut issues);
        let entrypoint = validate_entrypoint(&input.entrypoint, &files, &mut issues);
        let (packages, package_requirements) = validate_packages(input.packages, &mut issues);
        let (font_requirements, font_data) = validate_fonts(input.fonts, &mut issues);
        let Some(entrypoint) = entrypoint.filter(|_| issues.is_empty()) else {
            return Err(PackInvariantError { issues });
        };

        let identity = pack_identity(
            &entrypoint,
            &files,
            &package_requirements,
            &font_requirements,
        );
        Ok(Self {
            identity,
            entrypoint,
            metadata: input.metadata,
            files,
            packages,
            package_requirements,
            font_requirements,
            font_data,
        })
    }

    /// The Pack Identity: the SHA-256 of the pack's content, excluding metadata.
    pub fn identity(&self) -> Digest {
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

    /// The project files, in root-relative path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, (data, _))| (path.as_str(), data.as_slice()))
    }

    /// Looks up a project file by root-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|(data, _)| data.as_slice())
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path).map(|(data, _)| data)
    }

    pub(crate) fn file_digests(&self) -> impl Iterator<Item = (&str, Digest)> {
        self.files
            .iter()
            .map(|(path, (_, digest))| (path.as_str(), *digest))
    }

    pub(crate) fn canonical_project_path(path: &str) -> Result<String, String> {
        project_path(path)
            .map(CanonicalPath::into_string)
            .map_err(|message| message.to_string())
    }

    /// The embedded packages and their files.
    pub fn packages(
        &self,
    ) -> impl Iterator<Item = (&PackageSpec, impl Iterator<Item = (&str, &[u8])>)> {
        self.package_requirements.iter().filter_map(|requirement| {
            let tree = self.packages.get(&requirement.spec.to_string())?;
            Some((&requirement.spec, tree.files()))
        })
    }

    /// Looks up an embedded package file.
    pub fn package_file(&self, spec: &PackageSpec, path: &str) -> Option<&[u8]> {
        self.packages.get(&spec.to_string())?.file(path)
    }

    pub(crate) fn shared_package_file(
        &self,
        spec: &PackageSpec,
        path: &str,
    ) -> Option<&SharedBytes> {
        self.packages.get(&spec.to_string())?.shared_file(path)
    }

    /// Whether the pack embeds the given package.
    pub fn has_package(&self, spec: &PackageSpec) -> bool {
        self.packages.contains_key(&spec.to_string())
    }

    /// The exact packages this Pack needs, in specification order.
    pub fn package_requirements(&self) -> &[PackageRequirement] {
        &self.package_requirements
    }

    /// The exact font containers this Pack needs, in font book order.
    pub fn font_requirements(&self) -> &[FontRequirement] {
        &self.font_requirements
    }

    /// The bytes of an embedded font container.
    pub fn font_data(&self, digest: &Digest) -> Option<&[u8]> {
        self.font_data.get(digest).map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_font_data(&self, digest: &Digest) -> Option<&SharedBytes> {
        self.font_data.get(digest)
    }

    /// Combines the Pack's embedded dependencies with verified external
    /// fulfillments. Every face of every required container is exposed, in
    /// requirement order.
    pub(crate) fn materialize_compilation_dependency_snapshot(
        &self,
        mut package_fulfillments: BTreeMap<String, PackageTree>,
        font_fulfillments: BTreeMap<Digest, FontContainer>,
    ) -> CompilationDependencySnapshot {
        let mut packages = self.packages.clone();
        for requirement in self.package_requirements.iter().filter(|r| !r.embedded) {
            let key = requirement.spec.to_string();
            let tree = package_fulfillments
                .remove(&key)
                .expect("exact fulfillment verification supplies every external package");
            packages.insert(key, tree);
        }
        let fonts = self
            .font_requirements
            .iter()
            .flat_map(
                |requirement| match self.font_data.get(&requirement.digest) {
                    Some(data) => Font::iter(data.to_typst()).collect::<Vec<_>>(),
                    None => font_fulfillments[&requirement.digest].fonts().to_vec(),
                },
            )
            .collect();
        CompilationDependencySnapshot {
            pack_identity: self.identity,
            packages,
            fonts,
        }
    }
}

fn project_path(path: &str) -> Result<CanonicalPath, String> {
    let canonical = canonical_relative_path(path).map_err(|error| error.to_string())?;
    // No route into a Pack can name a Pack as a project file.
    if names_pack_path(canonical.as_str()) {
        return Err(format!(
            "`.{FILE_EXTENSION}` paths are excluded from project membership"
        ));
    }
    Ok(canonical)
}

fn validate_project(
    files: Vec<(String, SharedBytes)>,
    issues: &mut Vec<PackInvariantIssue>,
) -> BTreeMap<CanonicalPath, (SharedBytes, Digest)> {
    let mut pass = Vec::new();
    let mut canonical = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for (path, data) in files {
        match project_path(&path) {
            Ok(path) => {
                let digest = Digest::of(&data);
                if canonical.insert(path.clone(), (data, digest)).is_some() {
                    duplicates.insert(path);
                }
            }
            Err(message) => pass.push(PackInvariantIssue::InvalidPath { path, message }),
        }
    }
    pass.extend(
        duplicates
            .into_iter()
            .map(|path| PackInvariantIssue::DuplicateProjectPath {
                path: path.into_string(),
            }),
    );
    pass.extend(
        path_tree_conflicts(canonical.keys().map(|path| (path, ())))
            .into_iter()
            .map(|conflict| PackInvariantIssue::PathTreeConflict {
                ancestor: conflict.ancestor.to_string(),
                descendant: conflict.descendant.to_string(),
            }),
    );
    push_sorted(issues, pass);
    canonical
}

fn validate_entrypoint(
    entrypoint: &str,
    files: &BTreeMap<CanonicalPath, (SharedBytes, Digest)>,
    issues: &mut Vec<PackInvariantIssue>,
) -> Option<CanonicalPath> {
    match project_path(entrypoint) {
        Ok(path) if files.contains_key(&path) => Some(path),
        Ok(path) => {
            issues.push(PackInvariantIssue::MissingEntrypoint {
                path: path.into_string(),
            });
            None
        }
        Err(message) => {
            issues.push(PackInvariantIssue::InvalidEntrypoint {
                path: entrypoint.to_owned(),
                message,
            });
            None
        }
    }
}

fn validate_packages(
    inputs: Vec<PackageInput>,
    issues: &mut Vec<PackInvariantIssue>,
) -> (BTreeMap<String, PackageTree>, Vec<PackageRequirement>) {
    let mut pass = Vec::new();
    let mut seen = BTreeSet::new();
    let mut embedded = BTreeMap::new();
    let mut requirements = Vec::new();
    for input in inputs {
        let key = input.spec.to_string();
        if let Err(message) = validate_package_spec(&input.spec) {
            pass.push(PackInvariantIssue::InvalidPackageSpec { spec: key, message });
            continue;
        }
        if !seen.insert(key.clone()) {
            pass.push(PackInvariantIssue::DuplicatePackage { spec: input.spec });
            continue;
        }
        let digest = match input.content {
            PackageContent::Declared(digest) if !input.embedded => digest,
            PackageContent::Declared(_) => {
                unreachable!("embedded packages are always supplied with their files")
            }
            PackageContent::Files(files) => match PackageTree::from_shared_entries(files) {
                Ok(tree) => {
                    let digest = tree.digest();
                    if input.embedded {
                        embedded.insert(key, tree);
                    }
                    digest
                }
                Err(source) => {
                    pass.push(PackInvariantIssue::InvalidPackageTree {
                        spec: input.spec,
                        source,
                    });
                    continue;
                }
            },
        };
        requirements.push(PackageRequirement {
            spec: input.spec,
            digest,
            embedded: input.embedded,
        });
    }
    push_sorted(issues, pass);
    requirements.sort_by_cached_key(|requirement| requirement.spec.to_string());
    (embedded, requirements)
}

fn validate_package_spec(spec: &PackageSpec) -> Result<(), String> {
    match PackageSpec::from_str(&spec.to_string()) {
        Ok(parsed) if parsed == *spec => Ok(()),
        Ok(_) => Err("package specification does not round-trip canonically".to_owned()),
        Err(message) => Err(message.to_string()),
    }
}

fn validate_fonts(
    inputs: Vec<FontInput>,
    issues: &mut Vec<PackInvariantIssue>,
) -> (Vec<FontRequirement>, BTreeMap<Digest, SharedBytes>) {
    let mut seen = BTreeSet::new();
    let mut requirements = Vec::new();
    let mut font_data = BTreeMap::new();
    for (position, input) in inputs.into_iter().enumerate() {
        let requirement = match input {
            FontInput::Bytes { data, embedded } => {
                let faces = Font::iter(data.to_typst())
                    .map(|font| FontFace::of(&font))
                    .collect::<Vec<_>>();
                if faces.is_empty() {
                    issues.push(PackInvariantIssue::InvalidFontData { position });
                    continue;
                }
                let digest = Digest::of(&data);
                if embedded {
                    font_data.insert(digest, data);
                }
                FontRequirement {
                    digest,
                    embedded,
                    faces,
                }
            }
            FontInput::Declared { digest, faces } => FontRequirement {
                digest,
                embedded: false,
                faces,
            },
        };
        if !seen.insert(requirement.digest) {
            issues.push(PackInvariantIssue::DuplicateFont {
                digest: requirement.digest,
            });
            continue;
        }
        requirements.push(requirement);
    }
    (requirements, font_data)
}

fn push_sorted(issues: &mut Vec<PackInvariantIssue>, mut pass: Vec<PackInvariantIssue>) {
    pass.sort_by_cached_key(ToString::to_string);
    issues.extend(pass);
}

/// The documented Pack Identity preimage; see the README's "Pack format".
fn pack_identity(
    entrypoint: &CanonicalPath,
    files: &BTreeMap<CanonicalPath, (SharedBytes, Digest)>,
    packages: &[PackageRequirement],
    fonts: &[FontRequirement],
) -> Digest {
    let mut preimage = Preimage::new("typst-pack/pack/v1");
    preimage.str(entrypoint.as_str()).count(files.len());
    for (path, (_, digest)) in files {
        preimage.str(path.as_str()).digest(digest);
    }
    preimage.count(packages.len());
    for package in packages {
        preimage
            .str(&package.spec.to_string())
            .digest(&package.digest)
            .bool(package.embedded);
    }
    preimage.count(fonts.len());
    for font in fonts {
        preimage.digest(&font.digest).bool(font.embedded);
    }
    preimage.finish()
}

/// Builds a [`Pack`] from in-memory data.
///
/// This is the constructor to use when the project does not live on a file
/// system, for example in a web editor. For packing a project directory, use
/// `FilesystemPackAssembler` instead (requires the `fs` feature). Every
/// invariant is checked once, by [`build`](Self::build).
#[derive(Debug)]
pub struct PackBuilder {
    input: PackInput,
    /// Package files grouped by spec and whether they are embedded.
    packages: BTreeMap<(String, bool), PackageInput>,
}

impl PackBuilder {
    /// Creates a builder for a pack with the given entrypoint path.
    pub fn new(entrypoint: impl Into<String>) -> Self {
        Self {
            input: PackInput {
                entrypoint: entrypoint.into(),
                ..PackInput::default()
            },
            packages: BTreeMap::new(),
        }
    }

    /// Adds a project file under a root-relative path.
    pub fn file(mut self, path: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        self.input
            .files
            .push((path.into(), SharedBytes::new(data.into())));
        self
    }

    /// Adds a file of an embedded package.
    pub fn package_file(
        self,
        spec: PackageSpec,
        path: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.add_package_file(spec, path.into(), data.into(), true)
    }

    /// Adds a file of a package that is declared by tree digest only and
    /// must be supplied when the Pack is compiled.
    pub fn external_package_file(
        self,
        spec: PackageSpec,
        path: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        self.add_package_file(spec, path.into(), data.into(), false)
    }

    fn add_package_file(
        mut self,
        spec: PackageSpec,
        path: String,
        data: Vec<u8>,
        embedded: bool,
    ) -> Self {
        let package = self
            .packages
            .entry((spec.to_string(), embedded))
            .or_insert_with(|| PackageInput {
                spec,
                embedded,
                content: PackageContent::Files(Vec::new()),
            });
        if let PackageContent::Files(files) = &mut package.content {
            files.push((path, SharedBytes::new(data)));
        }
        self
    }

    /// Embeds a font file or collection; every face becomes available.
    pub fn font(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.input.fonts.push(FontInput::Bytes {
            data: SharedBytes::new(data.into()),
            embedded: true,
        });
        self
    }

    /// Declares a font file or collection by digest without storing its
    /// bytes; it must be supplied when the Pack is compiled.
    pub fn external_font(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.input.fonts.push(FontInput::Bytes {
            data: SharedBytes::new(data.into()),
            embedded: false,
        });
        self
    }

    /// Sets descriptive metadata.
    pub fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.input.metadata = Some(metadata);
        self
    }

    /// Validates and finishes the pack.
    pub fn build(mut self) -> Result<Pack, PackInvariantError> {
        self.input.packages = self.packages.into_values().collect();
        Pack::construct(self.input)
    }
}

/// One violation of a whole-Pack invariant.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackInvariantIssue {
    /// A project path cannot name a canonical root-relative file.
    #[error("invalid project path {path:?}: {message}")]
    InvalidPath { path: String, message: String },
    /// The entrypoint cannot name a canonical root-relative file.
    #[error("invalid entrypoint {path:?}: {message}")]
    InvalidEntrypoint { path: String, message: String },
    /// The entrypoint is not among the project files.
    #[error("entrypoint {path:?} is not a project file")]
    MissingEntrypoint { path: String },
    /// Two project entries name one canonical path.
    #[error("project path {path:?} is supplied more than once")]
    DuplicateProjectPath { path: String },
    /// One project file path is an ancestor of another.
    #[error("project file {ancestor:?} conflicts with descendant {descendant:?}")]
    PathTreeConflict {
        ancestor: String,
        descendant: String,
    },
    /// A package specification does not round-trip canonically.
    #[error("invalid package spec {spec:?}: {message}")]
    InvalidPackageSpec { spec: String, message: String },
    /// A package's files do not form a valid package tree.
    #[error("package {spec}: {source}")]
    InvalidPackageTree {
        spec: PackageSpec,
        #[source]
        source: PackageTreeError,
    },
    /// A package is declared more than once, or both embedded and external.
    #[error("package {spec} is declared more than once")]
    DuplicatePackage { spec: PackageSpec },
    /// Font bytes contain no face the embedded Typst engine can read.
    #[error("font input {position} contains no readable face")]
    InvalidFontData { position: usize },
    /// One font container is declared more than once.
    #[error("font container {digest} is declared more than once")]
    DuplicateFont { digest: Digest },
}

/// A violation of the invariants shared by every [`Pack`] construction path.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("invalid pack: {}", display_issues(.issues))]
pub struct PackInvariantError {
    issues: Vec<PackInvariantIssue>,
}

impl PackInvariantError {
    /// Every detected issue: project, entrypoint, package, then font issues.
    pub fn issues(&self) -> &[PackInvariantIssue] {
        &self.issues
    }
}

fn display_issues(issues: &[PackInvariantIssue]) -> String {
    issues
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
