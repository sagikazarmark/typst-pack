//! The validated in-memory pack model.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::Arc;

use typst::syntax::package::PackageSpec;
use typst::text::Font;

use crate::digest::Preimage;
use crate::font::{self, Face};
use crate::manifest::{GENERATOR, Metadata};
use crate::package::{self, Tree};
use crate::payload::SharedBytes;
use crate::project::ProjectSnapshot;
use crate::{Digest, TYPST_VERSION, list};

/// The conventional file extension for packs.
pub const FILE_EXTENSION: &str = "typk";

/// The longest member name a ZIP archive can hold.
pub(crate) const MAX_MEMBER_NAME: usize = u16::MAX as usize;

/// Whether any segment of a root-relative path names a pack.
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
///
/// Cloning a pack is cheap.
#[derive(Debug, Clone)]
pub struct Pack(Arc<Contents>);

#[derive(Debug)]
struct Contents {
    identity: Digest,
    project: ProjectSnapshot,
    metadata: Option<Metadata>,
    producer: Producer,
    /// Embedded package trees, keyed by spec string.
    packages: BTreeMap<String, Tree>,
    package_requirements: Vec<package::Requirement>,
    font_requirements: Vec<font::Requirement>,
    /// Embedded font container bytes.
    font_data: BTreeMap<Digest, SharedBytes>,
}

/// The unvalidated content of a pack, shared by the builder and the archive
/// decoder.
#[derive(Debug, Default)]
pub(crate) struct PackInput {
    pub(crate) entrypoint: String,
    pub(crate) metadata: Option<Metadata>,
    pub(crate) producer: Producer,
    pub(crate) files: Vec<(String, SharedBytes)>,
    pub(crate) packages: Vec<PackageInput>,
    pub(crate) fonts: Vec<FontInput>,
}

/// The producer a pack's manifest records; informational.
#[derive(Debug, Clone, Default)]
pub(crate) struct Producer {
    pub(crate) typst_version: Option<String>,
    pub(crate) generator: Option<String>,
}

impl Producer {
    /// This crate and its embedded Typst version.
    pub(crate) fn current() -> Self {
        Self {
            typst_version: Some(TYPST_VERSION.to_owned()),
            generator: Some(GENERATOR.to_owned()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PackageInput {
    pub(crate) spec: PackageSpec,
    pub(crate) content: PackageContent,
}

#[derive(Debug)]
pub(crate) enum PackageContent {
    Files {
        files: Vec<(String, SharedBytes)>,
        embedded: bool,
    },
    /// An already validated tree.
    Tree { tree: Tree, embedded: bool },
    /// An external package's tree digest, declared without its files.
    Declared(Digest),
}

#[derive(Debug)]
pub(crate) enum FontInput {
    Bytes {
        data: SharedBytes,
        embedded: bool,
    },
    /// An already validated container.
    Container {
        container: font::Container,
        embedded: bool,
    },
    /// An external container declared without its bytes.
    Declared {
        digest: Digest,
        faces: Vec<Face>,
    },
}

impl Pack {
    /// Starts building a pack from in-memory data.
    ///
    /// `entrypoint` is the root-relative path of the main file, e.g.
    /// `main.typ`.
    ///
    /// The builder discovers no dependencies: the pack contains exactly the
    /// files, packages, and fonts added to it. Use [`create()`](crate::create())
    /// when the library should discover them by compiling the project.
    ///
    /// ```
    /// use typst_pack::{Pack, archive};
    ///
    /// let pack = Pack::builder("main.typ")
    ///     .file("main.typ", b"= Report\n".to_vec())
    ///     .file("data/figures.csv", b"quarter,revenue\nQ1,120\n".to_vec())
    ///     .build()?;
    ///
    /// assert_eq!(pack.entrypoint(), "main.typ");
    /// assert!(!archive::encode(&pack).is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder(entrypoint: impl Into<String>) -> PackBuilder {
        PackBuilder {
            input: PackInput {
                entrypoint: entrypoint.into(),
                producer: Producer::current(),
                ..PackInput::default()
            },
            packages: BTreeMap::new(),
        }
    }

    /// Validates every pack invariant; shared by the builder and the archive
    /// decoder.
    pub(crate) fn construct(input: PackInput) -> Result<Self, PackError> {
        let mut issues = Vec::new();
        let project = ProjectSnapshot::validate(&input.entrypoint, input.files, &mut issues);
        Self::assemble(
            project,
            input.metadata,
            input.producer,
            input.packages,
            input.fonts,
            issues,
        )
    }

    /// Adds validated dependencies to an already validated project.
    pub(crate) fn with_dependencies(
        project: ProjectSnapshot,
        metadata: Option<Metadata>,
        packages: Vec<PackageInput>,
        fonts: Vec<FontInput>,
    ) -> Result<Self, PackError> {
        Self::assemble(
            Some(project),
            metadata,
            Producer::current(),
            packages,
            fonts,
            Vec::new(),
        )
    }

    fn assemble(
        project: Option<ProjectSnapshot>,
        metadata: Option<Metadata>,
        producer: Producer,
        packages: Vec<PackageInput>,
        fonts: Vec<FontInput>,
        mut issues: Vec<PackIssue>,
    ) -> Result<Self, PackError> {
        let (packages, package_requirements) = validate_packages(packages, &mut issues);
        let (font_requirements, font_data) = validate_fonts(fonts, &mut issues);
        let Some(project) = project.filter(|_| issues.is_empty()) else {
            return Err(PackError { issues });
        };
        let identity = pack_identity(&project, &package_requirements, &font_requirements);
        Ok(Self(Arc::new(Contents {
            identity,
            project,
            metadata,
            producer,
            packages,
            package_requirements,
            font_requirements,
            font_data,
        })))
    }

    /// The pack identity: the SHA-256 of the pack's content, excluding
    /// metadata.
    pub fn identity(&self) -> Digest {
        self.0.identity
    }

    /// The root-relative path of the entrypoint file.
    pub fn entrypoint(&self) -> &str {
        self.0.project.entrypoint()
    }

    /// Optional descriptive metadata, excluded from the identity.
    pub fn metadata(&self) -> Option<&Metadata> {
        self.0.metadata.as_ref()
    }

    /// The Typst version of the pack's producer; informational.
    ///
    /// A pack built by this crate reports the embedded Typst version. A
    /// decoded pack reports its manifest's `typst-version`, or `None` when the
    /// manifest records none.
    pub fn typst_version(&self) -> Option<&str> {
        self.0.producer.typst_version.as_deref()
    }

    /// The tool that produced the pack and its version, such as
    /// `typst-pack 0.6.0`; informational.
    ///
    /// A pack built by this crate reports this crate. A decoded pack reports
    /// its manifest's `generator`, or `None` when the manifest records none.
    pub fn generator(&self) -> Option<&str> {
        self.0.producer.generator.as_deref()
    }

    /// The project files, in root-relative path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.0.project.files()
    }

    /// Looks up a project file by root-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.0.project.file(path)
    }

    /// The validated project: entrypoint and project files.
    pub fn project(&self) -> &ProjectSnapshot {
        &self.0.project
    }

    pub(crate) fn file_digests(&self) -> impl Iterator<Item = (&str, Digest)> {
        self.0.project.file_digests()
    }

    /// The embedded packages and their files, in specification order.
    pub fn packages(&self) -> impl Iterator<Item = (&PackageSpec, &Tree)> {
        self.0
            .package_requirements
            .iter()
            .filter_map(|requirement| {
                let tree = self.0.packages.get(&requirement.spec.to_string())?;
                Some((&requirement.spec, tree))
            })
    }

    /// The exact packages this pack needs, in specification order.
    pub fn package_requirements(&self) -> &[package::Requirement] {
        &self.0.package_requirements
    }

    /// The exact font containers this pack needs, in font book order.
    pub fn font_requirements(&self) -> &[font::Requirement] {
        &self.0.font_requirements
    }

    /// The bytes of an embedded font container.
    pub fn font_data(&self, digest: &Digest) -> Option<&[u8]> {
        self.0.font_data.get(digest).map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_font_data(&self, digest: &Digest) -> Option<&SharedBytes> {
        self.0.font_data.get(digest)
    }

    /// The embedded package trees, keyed by specification text.
    pub(crate) fn embedded_packages(&self) -> &BTreeMap<String, Tree> {
        &self.0.packages
    }
}

/// The archive member prefix of an embedded package.
pub(crate) fn package_member_prefix(spec: &PackageSpec) -> String {
    format!(
        "packages/{}/{}/{}/",
        spec.namespace, spec.name, spec.version
    )
}

fn validate_packages(
    inputs: Vec<PackageInput>,
    issues: &mut Vec<PackIssue>,
) -> (BTreeMap<String, Tree>, Vec<package::Requirement>) {
    let mut pass = Vec::new();
    let mut seen = BTreeSet::new();
    let mut trees = BTreeMap::new();
    let mut requirements = Vec::new();
    for input in inputs {
        let key = input.spec.to_string();
        if let Err(message) = validate_package_spec(&input.spec) {
            pass.push(PackIssue::InvalidPackageSpec { spec: key, message });
            continue;
        }
        if !seen.insert(key.clone()) {
            pass.push(PackIssue::DuplicatePackage { spec: input.spec });
            continue;
        }
        let (tree, embedded) = match input.content {
            PackageContent::Declared(digest) => {
                requirements.push(package::Requirement {
                    spec: input.spec,
                    digest,
                    embedded: false,
                });
                continue;
            }
            PackageContent::Files { files, embedded } => match Tree::from_shared(files) {
                Ok(tree) => (tree, embedded),
                Err(error) => {
                    pass.push(PackIssue::InvalidPackage {
                        spec: input.spec,
                        issues: error.issues,
                    });
                    continue;
                }
            },
            PackageContent::Tree { tree, embedded } => (tree, embedded),
        };
        // A tree that declares itself must declare its specification, as
        // the catalog requires of the trees creation selects.
        if tree.file("typst.toml").is_some()
            && let Err(message) = package::check_declaration(&input.spec, &tree)
        {
            pass.push(PackIssue::InvalidDeclaration {
                spec: input.spec,
                message,
            });
            continue;
        }
        if embedded {
            let prefix = package_member_prefix(&input.spec).len();
            let long = tree
                .files()
                .map(|(path, _)| path)
                .filter(|path| prefix + path.len() > MAX_MEMBER_NAME)
                .map(|path| PackIssue::InvalidPath {
                    path: path.to_owned(),
                    message: "the path is too long for a pack archive".to_owned(),
                })
                .collect::<Vec<_>>();
            if !long.is_empty() {
                pass.push(PackIssue::InvalidPackage {
                    spec: input.spec,
                    issues: long,
                });
                continue;
            }
        }
        requirements.push(package::Requirement {
            digest: tree.digest(),
            spec: input.spec,
            embedded,
        });
        if embedded {
            trees.insert(key, tree);
        }
    }
    pass.sort_by_cached_key(ToString::to_string);
    issues.extend(pass);
    requirements.sort_by_cached_key(|requirement| requirement.spec.to_string());
    (trees, requirements)
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
    issues: &mut Vec<PackIssue>,
) -> (Vec<font::Requirement>, BTreeMap<Digest, SharedBytes>) {
    let mut seen = BTreeSet::new();
    let mut requirements = Vec::new();
    let mut font_data = BTreeMap::new();
    for (position, input) in inputs.into_iter().enumerate() {
        let requirement = match input {
            FontInput::Bytes { data, embedded } => {
                let faces = Font::iter(data.to_typst())
                    .map(|font| Face::of(&font))
                    .collect::<Vec<_>>();
                if faces.is_empty() {
                    issues.push(PackIssue::InvalidFontData { position });
                    continue;
                }
                let digest = Digest::of(&data);
                if embedded {
                    font_data.insert(digest, data);
                }
                font::Requirement {
                    digest,
                    embedded,
                    faces,
                }
            }
            FontInput::Container {
                container,
                embedded,
            } => {
                if embedded {
                    font_data.insert(container.digest(), container.shared_data().clone());
                }
                font::Requirement {
                    digest: container.digest(),
                    embedded,
                    faces: container.faces().to_vec(),
                }
            }
            FontInput::Declared { digest, faces } => font::Requirement {
                digest,
                embedded: false,
                faces,
            },
        };
        if !seen.insert(requirement.digest) {
            issues.push(PackIssue::DuplicateFont {
                digest: requirement.digest,
            });
            continue;
        }
        requirements.push(requirement);
    }
    (requirements, font_data)
}

/// The documented pack identity preimage; see the README's "Pack format".
fn pack_identity(
    project: &ProjectSnapshot,
    packages: &[package::Requirement],
    fonts: &[font::Requirement],
) -> Digest {
    let files = project.file_digests();
    let mut preimage = Preimage::new("typst-pack/pack/v1");
    preimage.str(project.entrypoint()).count(files.len());
    for (path, digest) in files {
        preimage.str(path).digest(&digest);
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
/// system, for example in a web editor. Every invariant is checked once, by
/// [`build`](Self::build).
#[derive(Debug)]
pub struct PackBuilder {
    input: PackInput,
    /// Package files grouped by spec and whether they are embedded.
    packages: BTreeMap<(String, bool), PackageInput>,
}

impl PackBuilder {
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
    /// must be supplied when the pack is compiled.
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
                content: PackageContent::Files {
                    files: Vec::new(),
                    embedded,
                },
            });
        if let PackageContent::Files { files, .. } = &mut package.content {
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
    /// bytes; it must be supplied when the pack is compiled.
    pub fn external_font(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.input.fonts.push(FontInput::Bytes {
            data: SharedBytes::new(data.into()),
            embedded: false,
        });
        self
    }

    /// Sets descriptive metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.input.metadata = Some(metadata);
        self
    }

    /// Validates and finishes the pack.
    pub fn build(mut self) -> Result<Pack, PackError> {
        self.input.packages = self.packages.into_values().collect();
        Pack::construct(self.input)
    }
}

/// One problem with the content of a pack, a project snapshot, a package
/// tree, or a package catalog.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackIssue {
    /// A path cannot name a canonical root-relative file.
    #[error("invalid path {path:?}: {message}")]
    InvalidPath { path: String, message: String },
    /// The entrypoint cannot name a canonical root-relative file.
    #[error("invalid entrypoint {path:?}: {message}")]
    InvalidEntrypoint { path: String, message: String },
    /// The entrypoint is not among the project files.
    #[error("entrypoint {path:?} is not a project file")]
    MissingEntrypoint { path: String },
    /// Two entries name one canonical path.
    #[error("path {path:?} is supplied more than once")]
    DuplicatePath { path: String },
    /// One file path is an ancestor of another.
    #[error("file {ancestor:?} conflicts with descendant {descendant:?}")]
    PathTreeConflict {
        ancestor: String,
        descendant: String,
    },
    /// A package specification does not round-trip canonically.
    #[error("invalid package spec {spec:?}: {message}")]
    InvalidPackageSpec { spec: String, message: String },
    /// A package's files do not form a valid package tree.
    #[error("package {spec}: {}", list("invalid package tree", issues))]
    InvalidPackage {
        spec: PackageSpec,
        issues: Vec<PackIssue>,
    },
    /// A package is supplied more than once, or both embedded and external.
    #[error("package {spec} is supplied more than once")]
    DuplicatePackage { spec: PackageSpec },
    /// A package tree's `typst.toml` does not declare its specification.
    #[error("the tree supplied for package {spec} {message}")]
    InvalidDeclaration { spec: PackageSpec, message: String },
    /// Font bytes contain no face the embedded Typst engine can read.
    #[error("font input {position} contains no readable face")]
    InvalidFontData { position: usize },
    /// One font container is declared more than once.
    #[error("font container {digest} is declared more than once")]
    DuplicateFont { digest: Digest },
}

/// Invalid pack content, listing every issue found.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("{}", list("invalid pack content", .issues))]
pub struct PackError {
    /// Every issue, in a canonical order independent of the input order.
    pub issues: Vec<PackIssue>,
}
