//! Package trees, requirements, and the catalog creation selects packages
//! from.

#[cfg(feature = "package-reading")]
mod expand;
pub mod registry;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use typst::syntax::package::{PackageSpec, PackageVersion};

use crate::digest::{Digest, Preimage};
use crate::pack::{PackError, PackIssue};
use crate::paths::{canonical_relative_path, path_tree_conflicts};
use crate::payload::SharedBytes;

/// One exact package a pack needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// The exact package specification.
    pub spec: PackageSpec,
    /// The package tree digest.
    pub digest: Digest,
    /// Whether the tree's files are stored in the pack.
    pub embedded: bool,
}

/// Every file beneath one package root, with the package tree digest derived
/// from every path and its bytes. Cloning a tree is cheap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    files: Arc<BTreeMap<String, SharedBytes>>,
    digest: Digest,
}

impl Tree {
    /// Validates package-relative paths and their bytes, reporting every
    /// issue.
    pub fn new(
        files: impl IntoIterator<Item = (impl Into<String>, impl Into<Vec<u8>>)>,
    ) -> Result<Self, PackError> {
        Self::from_shared(
            files
                .into_iter()
                .map(|(path, data)| (path.into(), SharedBytes::new(data.into())))
                .collect(),
        )
    }

    pub(crate) fn from_shared(entries: Vec<(String, SharedBytes)>) -> Result<Self, PackError> {
        let mut issues = Vec::new();
        let mut files = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for (path, data) in entries {
            match canonical_relative_path(&path) {
                Ok(canonical) => {
                    if files.insert(canonical.clone(), data).is_some() {
                        duplicates.insert(canonical.into_string());
                    }
                }
                Err(message) => issues.push(PackIssue::InvalidPath {
                    path,
                    message: message.to_owned(),
                }),
            }
        }
        issues.extend(
            duplicates
                .into_iter()
                .map(|path| PackIssue::DuplicatePath { path }),
        );
        issues.extend(path_tree_conflicts(files.keys()).into_iter().map(
            |(ancestor, descendant)| PackIssue::PathTreeConflict {
                ancestor: ancestor.to_string(),
                descendant: descendant.to_string(),
            },
        ));
        if !issues.is_empty() {
            issues.sort_by_cached_key(ToString::to_string);
            return Err(PackError { issues });
        }
        let files = files
            .into_iter()
            .map(|(path, data)| (path.into_string(), data))
            .collect::<BTreeMap<_, _>>();
        let digest = tree_digest(
            files
                .iter()
                .map(|(path, data)| (path.as_str(), Digest::of(data))),
        );
        Ok(Self {
            files: Arc::new(files),
            digest,
        })
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

    /// The package tree digest.
    pub fn digest(&self) -> Digest {
        self.digest
    }

    pub(crate) fn shared_files(&self) -> impl Iterator<Item = (&str, &SharedBytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }
}

/// The documented package tree digest preimage over `(path, file digest)`
/// records in path order; see the README's "Pack format".
fn tree_digest<'a>(files: impl ExactSizeIterator<Item = (&'a str, Digest)>) -> Digest {
    let mut preimage = Preimage::new("typst-pack/package-tree/v1");
    preimage.count(files.len());
    for (path, digest) in files {
        preimage.str(path).digest(&digest);
    }
    preimage.finish()
}

/// The package trees creation may select, keyed by exact specification, each
/// either embedded in the pack or left for external fulfillment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) spec: PackageSpec,
    pub(crate) tree: Tree,
    pub(crate) embedded: bool,
}

impl Catalog {
    /// An empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the tree of `spec`, embedded in created packs or not.
    ///
    /// The tree's `typst.toml` must declare `spec`'s name and version, and
    /// an already cataloged specification is not replaced.
    pub fn insert(
        &mut self,
        spec: PackageSpec,
        tree: Tree,
        embedded: bool,
    ) -> Result<(), PackError> {
        let key = spec.to_string();
        let mut issues = Vec::new();
        if self.entries.contains_key(&key) {
            issues.push(PackIssue::DuplicatePackage { spec: spec.clone() });
        }
        if let Err(message) = check_declaration(&spec, &tree) {
            issues.push(PackIssue::InvalidDeclaration {
                spec: spec.clone(),
                message,
            });
        }
        if !issues.is_empty() {
            return Err(PackError { issues });
        }
        self.entries.insert(
            key,
            Entry {
                spec,
                tree,
                embedded,
            },
        );
        Ok(())
    }

    /// Entries in canonical specification order.
    pub(crate) fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Looks up one exact specification.
    pub(crate) fn get(&self, spec: &PackageSpec) -> Option<&Entry> {
        self.entries.get(&spec.to_string())
    }
}

/// Checks that a tree's `typst.toml` declares the specification's name and
/// version.
pub(crate) fn check_declaration(spec: &PackageSpec, tree: &Tree) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        package: Declared,
    }
    #[derive(serde::Deserialize)]
    struct Declared {
        name: String,
        version: PackageVersion,
    }

    let data = tree.file("typst.toml").ok_or("holds no `typst.toml`")?;
    let text = std::str::from_utf8(data).map_err(|_| "has a non-UTF-8 `typst.toml`")?;
    let manifest = toml::from_str::<Manifest>(text)
        .map_err(|error| format!("has malformed `typst.toml`: {:?}", error.message()))?;
    let mut mismatches = Vec::new();
    if manifest.package.name != spec.name.as_str() {
        mismatches.push(format!("the name {:?}", manifest.package.name));
    }
    if manifest.package.version != spec.version {
        mismatches.push(format!("the version {}", manifest.package.version));
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!("declares {}", mismatches.join(" and ")))
    }
}

/// Why a package could not be read, recorded for creation in
/// [`create::Request::package_failures`](crate::create::Request::package_failures).
///
/// A compile that imports the package fails with this reason, as Typst would
/// report it.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadFailure {
    /// No source holds the package.
    #[error("package not found")]
    NotFound,
    /// The package exists, but not in this version.
    #[error("package version not found; latest available version is {latest}")]
    VersionNotFound { latest: PackageVersion },
    /// Fetching the package failed.
    #[error("network request failed{}", detail(.0))]
    NetworkFailed(Option<String>),
    /// The fetched archive is malformed.
    #[error("package archive is malformed{}", detail(.0))]
    MalformedArchive(Option<String>),
    /// Reading failed for another reason.
    #[error("package read failed{}", detail(.0))]
    Other(Option<String>),
}

fn detail(detail: &Option<String>) -> String {
    detail
        .as_ref()
        .map(|detail| format!(": {detail}"))
        .unwrap_or_default()
}
