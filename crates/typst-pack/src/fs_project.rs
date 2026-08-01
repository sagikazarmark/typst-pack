//! Project gathering for the reference filesystem Pack Assembler.
//!
//! Acquisition is list, filter, then read: the walker lists the entries beneath
//! the project root, the Project Ignore Policy decides which of them are
//! project files, and only the survivors are read.

use std::path::{Path, PathBuf};

use crate::ignore_policy::{IGNORE_FILE, ProjectIgnorePolicy};
use crate::packer::PackerError;
use crate::project_snapshot::{ProjectSnapshot, ProjectSnapshotAssembly};

/// Acquires the structural project closure beneath `root` as a Project
/// Snapshot.
pub(crate) fn acquire_snapshot(
    root: &Path,
    entrypoint: &str,
) -> Result<ProjectSnapshot, PackerError> {
    let policy = read_policy(root)?;
    let mut entries = Vec::new();
    for (path, source) in list_project_files(root, &policy)? {
        let data = std::fs::read(&source).map_err(|error| {
            PackerError::io(
                &format!("failed to read project file `{}`", source.display()),
                error,
            )
        })?;
        entries.push((path, data));
    }
    Ok(ProjectSnapshotAssembly::new(entrypoint).assemble(entries)?)
}

/// Fails when the project files backing `snapshot` no longer agree with the
/// filesystem, which is the project half of the Creation Evidence Fence.
pub(crate) fn revalidate(snapshot: &ProjectSnapshot, root: &Path) -> Result<(), PackerError> {
    let current = match acquire_snapshot(root, snapshot.entrypoint()) {
        Ok(current) => current,
        Err(PackerError::IgnoredEntrypoint(path)) => {
            return Err(PackerError::CreationEvidenceChanged {
                path: root.join(path).display().to_string(),
            });
        }
        Err(error) => return Err(error),
    };
    let Some(changed) = first_difference(snapshot, &current) else {
        return Ok(());
    };
    Err(PackerError::CreationEvidenceChanged {
        path: root.join(changed).display().to_string(),
    })
}

/// The first path on which two snapshots disagree, in canonical path order.
fn first_difference<'a>(
    snapshot: &'a ProjectSnapshot,
    current: &'a ProjectSnapshot,
) -> Option<&'a str> {
    snapshot
        .files()
        .find(|(path, data)| current.file(path) != Some(*data))
        .or_else(|| {
            current
                .files()
                .find(|(path, _)| snapshot.file(path).is_none())
        })
        .map(|(path, _)| path)
}

/// Lists the project files beneath `root` that the policy does not exclude,
/// pruning excluded directories instead of descending into them.
fn list_project_files(
    root: &Path,
    policy: &ProjectIgnorePolicy,
) -> Result<Vec<(String, PathBuf)>, PackerError> {
    let mut listing = Vec::new();
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

        if file_type.is_dir() {
            if policy.excludes_directory(path) {
                walk.skip_current_dir();
            }
            continue;
        }
        if policy.excludes_file(path) {
            continue;
        }
        if !file_type.is_file() {
            return Err(PackerError::UnsupportedProjectEntry {
                path: entry.path().to_owned(),
            });
        }
        listing.push((path.to_owned(), entry.path().to_owned()));
    }
    Ok(listing)
}

/// Reads the root Project Ignore Policy file, if the project has one.
fn read_policy(root: &Path) -> Result<ProjectIgnorePolicy, PackerError> {
    let path = root.join(IGNORE_FILE);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(PackerError::UnsupportedProjectEntry { path });
            }
            let bytes = std::fs::read(&path)
                .map_err(|error| PackerError::io("failed to read root `.typkignore`", error))?;
            Ok(ProjectIgnorePolicy::from_ignore_file(&bytes)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ProjectIgnorePolicy::built_in())
        }
        Err(error) => Err(PackerError::io(
            "failed to inspect root `.typkignore`",
            error,
        )),
    }
}
