//! The validated project part of a pack: an entrypoint and its project files.

use std::collections::{BTreeMap, BTreeSet};

use crate::Digest;
use crate::pack::{FILE_EXTENSION, MAX_MEMBER_NAME, PackError, PackIssue, names_pack_path};
use crate::paths::{CanonicalPath, canonical_relative_path, path_tree_conflicts};
use crate::payload::SharedBytes;

/// The archive member prefix of project files.
pub(crate) const PROJECT_PREFIX: &str = "project/";

/// An entrypoint and the exact bytes of every project file, validated under
/// the same rules a [`Pack`](crate::Pack) enforces.
///
/// Pack creation reads project files from a snapshot: the reader decides
/// which files belong to the project, and the snapshot checks canonical
/// paths, duplicates, path-tree conflicts, `.typk` paths, and that the
/// entrypoint is present. A pack created from a snapshot carries it
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSnapshot {
    entrypoint: CanonicalPath,
    files: BTreeMap<CanonicalPath, (SharedBytes, Digest)>,
}

impl ProjectSnapshot {
    /// Validates an entrypoint and project files given as path and bytes,
    /// reporting every issue.
    pub fn new(
        entrypoint: impl Into<String>,
        files: impl IntoIterator<Item = (impl Into<String>, impl Into<Vec<u8>>)>,
    ) -> Result<Self, PackError> {
        let files = files
            .into_iter()
            .map(|(path, data)| (path.into(), SharedBytes::new(data.into())))
            .collect();
        let mut issues = Vec::new();
        let snapshot = Self::validate(&entrypoint.into(), files, &mut issues);
        match snapshot {
            Some(snapshot) if issues.is_empty() => Ok(snapshot),
            _ => Err(PackError { issues }),
        }
    }

    /// The project validation shared by every pack construction path.
    pub(crate) fn validate(
        entrypoint: &str,
        files: Vec<(String, SharedBytes)>,
        issues: &mut Vec<PackIssue>,
    ) -> Option<Self> {
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
                Err(message) => pass.push(PackIssue::InvalidPath { path, message }),
            }
        }
        pass.extend(duplicates.into_iter().map(|path| PackIssue::DuplicatePath {
            path: path.into_string(),
        }));
        pass.extend(path_tree_conflicts(canonical.keys()).into_iter().map(
            |(ancestor, descendant)| PackIssue::PathTreeConflict {
                ancestor: ancestor.to_string(),
                descendant: descendant.to_string(),
            },
        ));
        pass.sort_by_cached_key(ToString::to_string);
        issues.extend(pass);

        let entrypoint = match project_path(entrypoint) {
            Ok(path) if canonical.contains_key(&path) => Some(path),
            Ok(path) => {
                issues.push(PackIssue::MissingEntrypoint {
                    path: path.into_string(),
                });
                None
            }
            Err(message) => {
                issues.push(PackIssue::InvalidEntrypoint {
                    path: entrypoint.to_owned(),
                    message,
                });
                None
            }
        }?;
        Some(Self {
            entrypoint,
            files: canonical,
        })
    }

    /// The canonical root-relative path of the entrypoint.
    pub fn entrypoint(&self) -> &str {
        self.entrypoint.as_str()
    }

    /// The project files in canonical path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, (data, _))| (path.as_str(), data.as_slice()))
    }

    /// Looks up a project file by canonical root-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.shared_file(path).map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path).map(|(data, _)| data)
    }

    pub(crate) fn file_digests(&self) -> impl ExactSizeIterator<Item = (&str, Digest)> {
        self.files
            .iter()
            .map(|(path, (_, digest))| (path.as_str(), *digest))
    }
}

fn project_path(path: &str) -> Result<CanonicalPath, String> {
    let canonical = canonical_relative_path(path).map_err(|error| error.to_string())?;
    // No route into a pack can name a pack as a project file.
    if names_pack_path(canonical.as_str()) {
        return Err(format!(
            "`.{FILE_EXTENSION}` paths are excluded from project membership"
        ));
    }
    if PROJECT_PREFIX.len() + canonical.as_str().len() > MAX_MEMBER_NAME {
        return Err("the path is too long for a pack archive".to_owned());
    }
    Ok(canonical)
}
