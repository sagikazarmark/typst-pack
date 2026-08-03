//! Validated package input values.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "package-acquisition")]
use typst::foundations::Bytes;
use typst::syntax::package::{PackageSpec, PackageVersion};

use crate::pack::{Pack, PackageTreeIdentity};
use crate::payload::SharedBytes;

/// Whether a Package Tree's bytes travel inside the Pack or must be fulfilled
/// externally when the Pack is compiled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackageDisposition {
    /// The tree's exact bytes are stored in the Pack.
    Embedded,
    /// The tree is declared and must be supplied at compilation.
    External,
}

impl PackageDisposition {
    /// Whether the tree's exact bytes are stored in the Pack.
    pub fn is_embedded(self) -> bool {
        matches!(self, Self::Embedded)
    }
}

/// Every addressable regular file beneath one acquired package root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageTree {
    files: BTreeMap<String, SharedBytes>,
    identity: PackageTreeIdentity,
    file_count: u64,
    byte_length: u64,
}

impl PackageTree {
    /// Constructs a tree by taking ownership of every file's exact bytes.
    pub fn from_owned_entries(
        entries: impl IntoIterator<Item = (impl AsRef<str>, Vec<u8>)>,
    ) -> Result<Self, PackageTreeError> {
        Self::from_shared_entries(
            entries
                .into_iter()
                .map(|(path, data)| (path.as_ref().to_owned(), SharedBytes::new(data)))
                .collect(),
        )
    }

    #[cfg(feature = "package-acquisition")]
    pub(crate) fn from_typst_entries(
        entries: Vec<(String, Bytes)>,
    ) -> Result<Self, PackageTreeError> {
        Self::from_shared_entries(
            entries
                .into_iter()
                .map(|(path, data)| (path, SharedBytes::from_typst(data)))
                .collect(),
        )
    }

    fn from_shared_entries(entries: Vec<(String, SharedBytes)>) -> Result<Self, PackageTreeError> {
        let mut canonical_entries = Vec::new();
        let mut issues = Vec::new();
        for (path, data) in entries {
            match Pack::canonical_package_path(&path) {
                Ok(canonical) => canonical_entries.push((canonical, data)),
                Err(message) => issues.push(PackageTreeIssue::InvalidPath { path, message }),
            }
        }

        let mut paths = BTreeSet::new();
        let mut duplicate_paths = BTreeSet::new();
        for (path, _) in &canonical_entries {
            if !paths.insert(path.clone()) {
                duplicate_paths.insert(path.clone());
            }
        }
        issues.extend(
            duplicate_paths
                .into_iter()
                .map(|path| PackageTreeIssue::DuplicatePath { path }),
        );
        for ancestor in &paths {
            let prefix = format!("{ancestor}/");
            for descendant in paths
                .range(prefix.clone()..)
                .take_while(|descendant| descendant.starts_with(&prefix))
            {
                issues.push(PackageTreeIssue::PathTreeConflict {
                    ancestor: ancestor.clone(),
                    descendant: descendant.clone(),
                });
            }
        }
        issues.sort_by_key(PackageTreeIssue::sort_key);
        if !issues.is_empty() {
            return Err(PackageTreeError { issues });
        }

        let files = canonical_entries.into_iter().collect::<BTreeMap<_, _>>();
        let (identity, file_count, byte_length) =
            derive_package_tree_identity(files.iter().map(|(path, data)| (path.as_str(), data)));
        Ok(Self {
            files,
            identity,
            file_count,
            byte_length,
        })
    }

    /// Constructs a tree by copying every file from caller-owned bytes.
    pub fn copy_from_entries(
        entries: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<[u8]>)>,
    ) -> Result<Self, PackageTreeError> {
        Self::from_owned_entries(
            entries
                .into_iter()
                .map(|(path, data)| (path.as_ref().to_owned(), data.as_ref().to_vec())),
        )
    }

    /// The files in canonical package-relative path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, data)| (path.as_str(), data.as_slice()))
    }

    /// Looks up a file by canonical package-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(SharedBytes::as_slice)
    }

    /// The Canonical Identity derived from every path and exact byte value.
    pub fn identity(&self) -> PackageTreeIdentity {
        self.identity
    }

    /// The number of files in the tree.
    pub fn file_count(&self) -> u64 {
        self.file_count
    }

    /// The total exact byte length of every file in the tree.
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub(crate) fn shared_files(&self) -> impl Iterator<Item = (&str, &SharedBytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }

    pub(crate) fn into_shared_files(self) -> BTreeMap<String, SharedBytes> {
        self.files
    }
}

pub(crate) fn derive_package_tree_identity<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a SharedBytes)>,
) -> (PackageTreeIdentity, u64, u64) {
    let projection = files
        .into_iter()
        .map(|(path, data)| (path, data.len() as u64, typst::utils::hash128(data)))
        .collect::<Vec<_>>();
    let file_count = projection.len() as u64;
    let byte_length = projection.iter().map(|(_, length, _)| length).sum();
    (
        PackageTreeIdentity::from_digest(typst::utils::hash128(&(
            crate::pack::PACKAGE_TREE_IDENTITY_SCHEMA,
            file_count,
            byte_length,
            projection,
        ))),
        file_count,
        byte_length,
    )
}

/// One independently detectable issue in a supplied Package Tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageTreeIssue {
    /// A supplied path cannot name a package-relative file.
    #[error("package path {path:?} cannot be represented: {message:?}")]
    InvalidPath { path: String, message: String },
    /// Two supplied entries name one canonical package file.
    #[error("package path {path:?} is supplied more than once")]
    DuplicatePath { path: String },
    /// One package file path is an ancestor of another package file path.
    #[error("package path {ancestor:?} is a file ancestor of {descendant:?}")]
    PathTreeConflict {
        ancestor: String,
        descendant: String,
    },
}

impl PackageTreeIssue {
    fn sort_key(&self) -> (String, u8, String) {
        match self {
            Self::InvalidPath { path, .. } => (path.clone(), 0, String::new()),
            Self::DuplicatePath { path } => (path.clone(), 1, String::new()),
            Self::PathTreeConflict {
                ancestor,
                descendant,
            } => (ancestor.clone(), 2, descendant.clone()),
        }
    }
}

/// A failure while constructing a [`PackageTree`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("package tree construction failed with {} issue(s)", .issues.len())]
pub struct PackageTreeError {
    issues: Vec<PackageTreeIssue>,
}

impl PackageTreeError {
    /// Every independently detectable issue in canonical path and kind order.
    pub fn issues(&self) -> &[PackageTreeIssue] {
        &self.issues
    }
}

/// One Package Catalog entry under its claimed exact specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageCatalogEntry {
    spec: PackageSpec,
    tree: PackageTree,
    disposition: PackageDisposition,
}

impl PackageCatalogEntry {
    /// The exact specification this entry claims to satisfy.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The validated Package Tree.
    pub fn tree(&self) -> &PackageTree {
        &self.tree
    }

    /// Whether this tree is embedded or externally fulfilled.
    pub fn disposition(&self) -> PackageDisposition {
        self.disposition
    }
}

/// The validated Package Trees Pack Creation may select, keyed canonically by
/// exact package specification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackageCatalog {
    entries: BTreeMap<String, PackageCatalogEntry>,
}

impl PackageCatalog {
    /// An empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a catalog from claimed specifications, trees, and explicit
    /// dispositions.
    pub fn from_entries(
        entries: impl IntoIterator<Item = (PackageSpec, PackageTree, PackageDisposition)>,
    ) -> Result<Self, PackageCatalogError> {
        let entries = entries.into_iter().collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        let mut issues = Vec::new();
        for (spec, tree, _) in &entries {
            let key = spec.to_string();
            if !seen.insert(key.clone()) {
                duplicates.insert(key);
            }
            issues.extend(verify_package_declaration(spec, tree));
        }
        for key in duplicates {
            let spec = entries
                .iter()
                .find(|(spec, _, _)| spec.to_string() == key)
                .expect("duplicate specification came from an entry")
                .0
                .clone();
            issues.push(PackageCatalogIssue::DuplicateSpecification { spec });
        }
        issues.sort_by_key(PackageCatalogIssue::sort_key);
        if !issues.is_empty() {
            return Err(PackageCatalogError { issues });
        }
        Ok(Self {
            entries: entries
                .into_iter()
                .map(|(spec, tree, disposition)| {
                    (
                        spec.to_string(),
                        PackageCatalogEntry {
                            spec,
                            tree,
                            disposition,
                        },
                    )
                })
                .collect(),
        })
    }

    /// Inserts one validated tree, rejecting an existing exact specification
    /// rather than replacing its evidence.
    pub fn insert(
        &mut self,
        spec: PackageSpec,
        tree: PackageTree,
        disposition: PackageDisposition,
    ) -> Result<(), PackageCatalogError> {
        let key = spec.to_string();
        let mut issues = Vec::new();
        if self.entries.contains_key(&key) {
            issues.push(PackageCatalogIssue::DuplicateSpecification { spec: spec.clone() });
        }
        issues.extend(verify_package_declaration(&spec, &tree));
        issues.sort_by_key(PackageCatalogIssue::sort_key);
        if !issues.is_empty() {
            return Err(PackageCatalogError { issues });
        }
        self.entries.insert(
            key,
            PackageCatalogEntry {
                spec,
                tree,
                disposition,
            },
        );
        Ok(())
    }

    /// Catalog entries in canonical exact-specification order.
    pub fn entries(&self) -> impl Iterator<Item = &PackageCatalogEntry> {
        self.entries.values()
    }

    /// Looks up one exact package specification.
    pub fn get(&self, spec: &PackageSpec) -> Option<&PackageCatalogEntry> {
        self.entries.get(&spec.to_string())
    }
}

const PACKAGE_DECLARATION_PATH: &str = "typst.toml";

fn verify_package_declaration(spec: &PackageSpec, tree: &PackageTree) -> Vec<PackageCatalogIssue> {
    let Some(data) = tree.file(PACKAGE_DECLARATION_PATH) else {
        return vec![PackageCatalogIssue::MissingDeclaration { spec: spec.clone() }];
    };
    let Ok(text) = std::str::from_utf8(data) else {
        return vec![PackageCatalogIssue::DeclarationNotUtf8 { spec: spec.clone() }];
    };
    let declaration = match toml::from_str::<SuppliedPackageDeclaration>(text) {
        Ok(declaration) => declaration,
        Err(error) => {
            return vec![PackageCatalogIssue::MalformedDeclaration {
                spec: spec.clone(),
                message: error.message().to_owned(),
            }];
        }
    };

    let mut issues = Vec::new();
    if declaration.package.name != spec.name.as_str() {
        issues.push(PackageCatalogIssue::MismatchedName {
            spec: spec.clone(),
            declared: declaration.package.name,
        });
    }
    if declaration.package.version != spec.version {
        issues.push(PackageCatalogIssue::MismatchedVersion {
            spec: spec.clone(),
            declared: declaration.package.version,
        });
    }
    issues
}

#[derive(serde::Deserialize)]
struct SuppliedPackageDeclaration {
    package: DeclaredPackage,
}

#[derive(serde::Deserialize)]
struct DeclaredPackage {
    name: String,
    version: PackageVersion,
}

/// One independently detectable issue in a supplied Package Catalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageCatalogIssue {
    /// An exact package specification was supplied more than once.
    #[error("package specification {spec} is supplied more than once")]
    DuplicateSpecification { spec: PackageSpec },
    /// The tree has no `typst.toml` declaration.
    #[error("the tree supplied for package {spec} holds no `typst.toml`")]
    MissingDeclaration { spec: PackageSpec },
    /// The tree's `typst.toml` is not UTF-8.
    #[error("the tree supplied for package {spec} has a non-UTF-8 `typst.toml`")]
    DeclarationNotUtf8 { spec: PackageSpec },
    /// The tree's `typst.toml` cannot be parsed.
    #[error("the tree supplied for package {spec} has malformed `typst.toml`: {message:?}")]
    MalformedDeclaration { spec: PackageSpec, message: String },
    /// The declaration names another package.
    #[error("the tree supplied for package {spec} declares the name {declared:?}")]
    MismatchedName { spec: PackageSpec, declared: String },
    /// The declaration names another package version.
    #[error("the tree supplied for package {spec} declares the version {declared}")]
    MismatchedVersion {
        spec: PackageSpec,
        declared: PackageVersion,
    },
}

impl PackageCatalogIssue {
    fn spec(&self) -> &PackageSpec {
        match self {
            Self::DuplicateSpecification { spec }
            | Self::MissingDeclaration { spec }
            | Self::DeclarationNotUtf8 { spec }
            | Self::MalformedDeclaration { spec, .. }
            | Self::MismatchedName { spec, .. }
            | Self::MismatchedVersion { spec, .. } => spec,
        }
    }

    fn sort_key(&self) -> (String, u8, String) {
        let (rank, detail) = match self {
            Self::DuplicateSpecification { .. } => (0, String::new()),
            Self::MissingDeclaration { .. } => (1, String::new()),
            Self::DeclarationNotUtf8 { .. } => (2, String::new()),
            Self::MalformedDeclaration { message, .. } => (3, message.clone()),
            Self::MismatchedName { declared, .. } => (4, declared.clone()),
            Self::MismatchedVersion { declared, .. } => (5, declared.to_string()),
        };
        (self.spec().to_string(), rank, detail)
    }
}

/// A failure while constructing a [`PackageCatalog`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("package catalog construction failed with {} issue(s)", .issues.len())]
pub struct PackageCatalogError {
    issues: Vec<PackageCatalogIssue>,
}

impl PackageCatalogError {
    /// Every independently detectable issue in canonical specification and
    /// issue-kind order.
    pub fn issues(&self) -> &[PackageCatalogIssue] {
        &self.issues
    }
}
