//! One stabilized set of project files, assembled without a filesystem.

use std::collections::BTreeMap;

use typst::foundations::Bytes;

use crate::ignore_policy::ProjectIgnorePolicy;
use crate::pack::Pack;

/// One stabilized set of project files: canonical root-relative paths, exact
/// bytes, and the entrypoint they were assembled around.
///
/// Membership is decided by the [`ProjectIgnorePolicy`] the snapshot was
/// assembled under, not by the caller that supplied the entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSnapshot {
    entrypoint: String,
    files: BTreeMap<String, Bytes>,
}

impl ProjectSnapshot {
    /// The canonical root-relative path of the entrypoint.
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// The contained project files in canonical path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &Bytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    /// Looks up a contained project file by canonical root-relative path.
    pub fn file(&self, path: &str) -> Option<&Bytes> {
        self.files.get(path)
    }
}

/// Assembles a [`ProjectSnapshot`] from the path-and-bytes entries a Creation
/// Adapter acquired.
///
/// Assembly canonicalizes every supplied path, re-applies the Project Ignore
/// Policy, and verifies that the entrypoint survives filtering, so project
/// membership does not depend on adapters being well-behaved.
#[derive(Debug)]
pub struct ProjectSnapshotAssembly<'a> {
    entrypoint: String,
    policy: &'a ProjectIgnorePolicy,
    budget: ProjectSnapshotBudget,
}

impl<'a> ProjectSnapshotAssembly<'a> {
    /// Prepares assembly of a project with the given entrypoint under `policy`.
    pub fn new(entrypoint: impl Into<String>, policy: &'a ProjectIgnorePolicy) -> Self {
        Self {
            entrypoint: entrypoint.into(),
            policy,
            budget: ProjectSnapshotBudget::default(),
        }
    }

    /// Bounds the assembled snapshot. Exclusion runs before the budget is
    /// measured, so the budget governs what will actually be packed.
    pub fn budget(mut self, budget: ProjectSnapshotBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Assembles the snapshot from `entries`.
    pub fn assemble(
        &self,
        entries: impl IntoIterator<Item = (impl AsRef<str>, impl Into<Vec<u8>>)>,
    ) -> Result<ProjectSnapshot, ProjectSnapshotError> {
        let entrypoint = canonical_project_path(&self.entrypoint)?;
        if self.policy.excludes_file(&entrypoint) {
            return Err(ProjectSnapshotError::ExcludedEntrypoint(entrypoint));
        }

        let mut files = BTreeMap::new();
        let mut byte_size = 0u64;
        for (path, data) in entries {
            let path = canonical_project_path(path.as_ref())?;
            if self.policy.excludes_file(&path) {
                continue;
            }
            let data = data.into();
            byte_size = byte_size.saturating_add(data.len() as u64);
            if let Some(limit) = self.budget.max_bytes
                && byte_size > limit
            {
                return Err(ProjectSnapshotError::ByteSizeExceeded { limit });
            }
            if files.insert(path.clone(), Bytes::new(data)).is_some() {
                return Err(ProjectSnapshotError::DuplicatePath { path });
            }
            if let Some(limit) = self.budget.max_files
                && files.len() > limit
            {
                return Err(ProjectSnapshotError::FileCountExceeded { limit });
            }
        }

        if !files.contains_key(&entrypoint) {
            return Err(ProjectSnapshotError::MissingEntrypoint(entrypoint));
        }
        Ok(ProjectSnapshot { entrypoint, files })
    }
}

/// An optional bound on the size of an assembled [`ProjectSnapshot`].
///
/// A service operator uses it so that a malformed or hostile project fails
/// fast. Both bounds are measured over the entries that survive exclusion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectSnapshotBudget {
    /// The largest number of contained project files, if bounded.
    pub max_files: Option<usize>,
    /// The largest total byte size of contained project files, if bounded.
    pub max_bytes: Option<u64>,
}

/// Canonicalizes a supplied path so that the policy decides membership over
/// the same path the Pack will contain.
fn canonical_project_path(path: &str) -> Result<String, ProjectSnapshotError> {
    Pack::canonical_project_path_without_membership(path).map_err(|message| {
        ProjectSnapshotError::InvalidPath {
            path: path.to_owned(),
            message,
        }
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
    /// The entrypoint is excluded by the Project Ignore Policy.
    #[error("entrypoint `{0}` is excluded by the Project Ignore Policy")]
    ExcludedEntrypoint(String),
    /// The entrypoint is not among the supplied project files.
    #[error("entrypoint `{0}` is not a supplied project file")]
    MissingEntrypoint(String),
    /// The project has more files than the budget allows.
    #[error("the project has more than {limit} project file(s)")]
    FileCountExceeded { limit: usize },
    /// The project's project files are larger than the budget allows.
    #[error("the project's files exceed {limit} byte(s)")]
    ByteSizeExceeded { limit: u64 },
}
