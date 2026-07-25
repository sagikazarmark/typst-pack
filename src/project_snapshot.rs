use std::collections::BTreeMap;
use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use typst::foundations::Bytes;

use crate::pack::Pack;
use crate::packer::PackerError;

const IGNORE_FILE: &str = ".typkignore";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectSnapshot {
    files: BTreeMap<String, Bytes>,
}

impl ProjectSnapshot {
    pub(crate) fn acquire(root: &Path) -> Result<Self, PackerError> {
        let policy_path = root.join(IGNORE_FILE);
        let policy = build_policy(root, &policy_path)?;

        let mut files = BTreeMap::new();
        let mut walk = walkdir::WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter();
        while let Some(entry) = walk.next() {
            let entry = entry.map_err(|error| PackerError::Walk(error.to_string()))?;
            if entry.depth() == 0 {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("walk remains beneath root");
            let path = relative
                .to_str()
                .ok_or_else(|| PackerError::UnrepresentablePath {
                    path: entry.path().to_owned(),
                })?;
            let file_type = entry.file_type();
            let forced_policy = entry.depth() == 1 && path == IGNORE_FILE;
            let built_in_excluded = Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "typk");
            let ignored = !forced_policy
                && (built_in_excluded
                    || policy
                        .matched_path_or_any_parents(relative, file_type.is_dir())
                        .is_ignore());

            if file_type.is_dir() {
                if ignored {
                    walk.skip_current_dir();
                }
                continue;
            }
            if ignored {
                continue;
            }
            if !file_type.is_file() {
                return Err(PackerError::UnsupportedProjectEntry {
                    path: entry.path().to_owned(),
                });
            }
            let canonical = Pack::canonical_project_path(path).map_err(|message| {
                PackerError::InvalidProjectPath {
                    path: path.to_owned(),
                    message,
                }
            })?;
            let data = std::fs::read(entry.path()).map_err(|error| {
                PackerError::io(
                    &format!("failed to read project file `{}`", entry.path().display()),
                    error,
                )
            })?;
            files.insert(canonical, Bytes::new(data));
        }
        Ok(Self { files })
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = (&str, &Bytes)> {
        self.files.iter().map(|(path, data)| (path.as_str(), data))
    }

    pub(crate) fn file(&self, path: &str) -> Option<&Bytes> {
        self.files.get(path)
    }

    pub(crate) fn revalidate(&self, root: &Path) -> Result<(), PackerError> {
        let current = Self::acquire(root)?;
        if current == *self {
            return Ok(());
        }
        let path = self
            .files
            .iter()
            .find(|(path, data)| current.files.get(*path) != Some(*data))
            .map(|(path, _)| root.join(path))
            .or_else(|| {
                current
                    .files
                    .keys()
                    .find(|path| !self.files.contains_key(*path))
                    .map(|path| root.join(path))
            })
            .unwrap_or_else(|| root.to_owned());
        Err(PackerError::CreationEvidenceChanged {
            path: path.display().to_string(),
        })
    }
}

fn build_policy(root: &Path, policy_path: &Path) -> Result<Gitignore, PackerError> {
    let mut builder = GitignoreBuilder::new(root);
    match std::fs::symlink_metadata(policy_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(PackerError::UnsupportedProjectEntry {
                    path: policy_path.to_owned(),
                });
            }
            std::fs::read_to_string(policy_path)
                .map_err(|error| PackerError::io("failed to read root `.typkignore`", error))?;
            if let Some(error) = builder.add(policy_path) {
                return Err(PackerError::InvalidIgnorePolicy(error.to_string()));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(PackerError::io(
                "failed to inspect root `.typkignore`",
                error,
            ));
        }
    }
    builder
        .build()
        .map_err(|error| PackerError::InvalidIgnorePolicy(error.to_string()))
}
