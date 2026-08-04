//! One stabilized set of project files, assembled without a filesystem.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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
        let mut issues = Vec::new();
        let entrypoint = match canonical_project_path(&self.entrypoint) {
            Ok(path) => Some(path),
            Err(issue) => {
                issues.push(issue);
                None
            }
        };

        let mut selected_entries = Vec::new();
        for (path, data) in entries {
            match canonical_project_path(path.as_ref()) {
                Ok(path) => selected_entries.push((path, SharedBytes::new(data.into()))),
                Err(issue) => issues.push(issue),
            }
        }

        let mut paths = BTreeSet::new();
        let mut duplicate_paths = BTreeSet::new();
        for (path, _) in &selected_entries {
            if !paths.insert(path.as_str()) {
                duplicate_paths.insert(path.clone());
            }
        }
        issues.extend(
            duplicate_paths
                .into_iter()
                .map(|path| ProjectSnapshotIssue::DuplicatePath { path }),
        );

        if let Some(entrypoint) = &entrypoint
            && !paths.contains(entrypoint.as_str())
        {
            issues.push(ProjectSnapshotIssue::MissingEntrypoint {
                path: entrypoint.clone(),
            });
        }
        issues.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        if !issues.is_empty() {
            return Err(ProjectSnapshotError { issues });
        }
        let files = selected_entries.into_iter().collect::<BTreeMap<_, _>>();
        Ok(ProjectSnapshot {
            entrypoint: entrypoint.expect("a valid Project Snapshot has a canonical entrypoint"),
            files,
        })
    }
}

/// Canonicalizes a supplied path under the universal project membership rules.
fn canonical_project_path(path: &str) -> Result<String, ProjectSnapshotIssue> {
    Pack::canonical_project_path(path).map_err(|message| ProjectSnapshotIssue::InvalidPath {
        path: path.to_owned(),
        message,
    })
}

/// One independently detectable issue while assembling a [`ProjectSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectSnapshotIssue {
    /// A supplied path cannot name a root-relative project file.
    #[error("project path {path:?} cannot be represented: {message}")]
    InvalidPath { path: String, message: String },
    /// Two supplied entries name one canonical project file.
    #[error("project path {path:?} is supplied more than once")]
    DuplicatePath { path: String },
    /// The entrypoint is not among the supplied project files.
    #[error("entrypoint {path:?} is not a supplied project file")]
    MissingEntrypoint { path: String },
}

impl ProjectSnapshotIssue {
    fn sort_key(&self) -> (&str, u8, &str) {
        match self {
            Self::InvalidPath { path, message } => (path, 0, message),
            Self::DuplicatePath { path } => (path, 1, ""),
            Self::MissingEntrypoint { path } => (path, 2, ""),
        }
    }
}

/// A failure while assembling a [`ProjectSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSnapshotError {
    issues: Vec<ProjectSnapshotIssue>,
}

impl ProjectSnapshotError {
    /// Every independently detectable issue in canonical path order.
    pub fn issues(&self) -> &[ProjectSnapshotIssue] {
        &self.issues
    }
}

impl fmt::Display for ProjectSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            return issue.fmt(formatter);
        }
        write!(
            formatter,
            "Project Snapshot assembly failed with {} issue(s)",
            self.issues.len()
        )?;
        for issue in &self.issues {
            write!(formatter, ": {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProjectSnapshotError {}
