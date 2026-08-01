//! One stabilized set of project files, assembled without a filesystem.

use std::collections::{BTreeMap, BTreeSet};

use crate::pack::Pack;
use crate::payload::SharedBytes;

/// One stabilized set of project files: canonical root-relative paths, exact
/// bytes, and the entrypoint they were assembled around.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSnapshot {
    entrypoint: String,
    files: BTreeMap<String, SharedBytes>,
}

impl ProjectSnapshot {
    /// The canonical root-relative path of the entrypoint.
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// The contained project files in canonical path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, data)| (path.as_str(), data.as_slice()))
    }

    /// Looks up a contained project file by canonical root-relative path.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(SharedBytes::as_slice)
    }

    pub(crate) fn shared_files(&self) -> impl Iterator<Item = (&str, &SharedBytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    pub(crate) fn shared_file(&self, path: &str) -> Option<&SharedBytes> {
        self.files.get(path)
    }
}

/// Assembles a [`ProjectSnapshot`] from already selected path-and-bytes entries.
///
/// Source gatherers decide membership before this operation. Assembly owns only
/// universal snapshot invariants: canonical paths, duplicate and `.typk` path
/// rejection, exact bytes, canonical order, and entrypoint presence.
#[derive(Debug)]
pub struct ProjectSnapshotAssembly {
    entrypoint: String,
}

impl ProjectSnapshotAssembly {
    /// Prepares assembly of a project with the selected entrypoint.
    pub fn new(entrypoint: impl Into<String>) -> Self {
        Self {
            entrypoint: entrypoint.into(),
        }
    }

    /// Assembles the snapshot from `entries`.
    pub fn assemble(
        &self,
        entries: impl IntoIterator<Item = (impl AsRef<str>, impl Into<Vec<u8>>)>,
    ) -> Result<ProjectSnapshot, ProjectSnapshotError> {
        let entrypoint = canonical_project_path(&self.entrypoint)?;

        let entries = entries
            .into_iter()
            .map(|(path, data)| {
                Ok((
                    canonical_project_path(path.as_ref())?,
                    SharedBytes::new(data.into()),
                ))
            })
            .collect::<Result<Vec<_>, ProjectSnapshotError>>()?;

        let mut paths = BTreeSet::new();
        for (path, _) in &entries {
            if !paths.insert(path.as_str()) {
                return Err(ProjectSnapshotError::DuplicatePath { path: path.clone() });
            }
        }

        if !paths.contains(entrypoint.as_str()) {
            return Err(ProjectSnapshotError::MissingEntrypoint(entrypoint));
        }
        let files = entries.into_iter().collect::<BTreeMap<_, _>>();
        Ok(ProjectSnapshot { entrypoint, files })
    }
}

/// Canonicalizes a supplied path under the universal project membership rules.
fn canonical_project_path(path: &str) -> Result<String, ProjectSnapshotError> {
    Pack::canonical_project_path(path).map_err(|message| ProjectSnapshotError::InvalidPath {
        path: path.to_owned(),
        message,
    })
}

/// A failure while assembling a [`ProjectSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectSnapshotError {
    /// A supplied path cannot name a root-relative project file.
    #[error("project path `{path}` cannot be represented: {message}")]
    InvalidPath { path: String, message: String },
    /// Two supplied entries name one canonical project file.
    #[error("project path `{path}` is supplied more than once")]
    DuplicatePath { path: String },
    /// The entrypoint is not among the supplied project files.
    #[error("entrypoint `{0}` is not a supplied project file")]
    MissingEntrypoint(String),
}
