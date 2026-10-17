//! Filesystem writing for Pack Extraction Plans, Compilation Results, and
//! Pack Archives.
//!
//! Every file is first written to a uniquely named temporary file in its
//! target directory, then committed under one [`WritePolicy`]:
//!
//! - [`WritePolicy::Overwrite`] renames the temporary file over the target,
//!   so readers observe either the old or the new content.
//! - [`WritePolicy::CreateNew`] hard-links the temporary file to the target,
//!   which fails when the target exists. Where the filesystem has no hard
//!   links (FAT, exFAT, some network and FUSE filesystems), it falls back to
//!   an exclusive create followed by a write, so an interrupted write can
//!   leave a partial file there.
//!
//! The temporary file is removed on every path. Nothing is flushed to stable
//! storage: the adapter makes no crash-durability guarantee.
//!
//! A Pack Extraction Plan is checked before anything is written: its paths
//! must be canonical, portable to every platform, and free of case-insensitive
//! collisions, and existing destination entries must not conflict with it.
//! Directories beneath the destination are created without following
//! symbolic links, so the destination path itself may contain links but a
//! linked or non-directory ancestor beneath it is refused. These checks are
//! validation, not confinement against a concurrent local peer.

use std::collections::{BTreeMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};

use crate::error_display::format_error_list;
use crate::{CompilationResult, CompilationStatus, PackExtractionPlan};

/// How a filesystem write treats a target that already exists.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WritePolicy {
    /// Fail when any target already exists.
    #[default]
    CreateNew,
    /// Replace existing regular files, and create missing ones.
    Overwrite,
}

/// One problem found before anything was written.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum WriteIssue {
    #[error("`{}` is not a canonical relative path", path.display())]
    InvalidPath { path: PathBuf },
    #[error("`{}` contains `{component}`, which is not a portable file name", path.display())]
    ReservedName { path: PathBuf, component: String },
    #[error("`{}` and `{}` collide on case-insensitive filesystems", first.display(), second.display())]
    PathConflict { first: PathBuf, second: PathBuf },
    #[error("`{}` already exists", path.display())]
    ExistingTarget { path: PathBuf },
    #[error("`{}` exists and is not a regular file", path.display())]
    ConflictingTarget { path: PathBuf },
    #[error("`{}` is a symbolic link or a file, not a directory", ancestor.display())]
    ConflictingAncestor { path: PathBuf, ancestor: PathBuf },
    #[error("the Compilation Result did not succeed")]
    FailedCompilation,
    #[error("the Compilation Result has {artifacts} artifact(s), but {paths} path(s) were given")]
    PathCountMismatch { artifacts: usize, paths: usize },
}

/// A failed filesystem write.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemWriteError {
    /// The write was refused before anything was written.
    #[error("nothing was written{}", format_error_list(issues))]
    Rejected { issues: Vec<WriteIssue> },
    /// Writing `path` failed after the `completed` files were written.
    #[error("cannot write `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        /// Files committed before the failure, in write order.
        completed: Vec<PathBuf>,
        #[source]
        source: io::Error,
    },
}

/// Writes every file of a Pack Extraction Plan beneath `destination`.
///
/// The destination is created when missing. Nothing is written when the plan
/// or an existing destination entry conflicts with it; a failure while
/// writing reports the files already committed.
pub fn write_pack_extraction_plan_to_filesystem(
    plan: &PackExtractionPlan,
    destination: impl AsRef<Path>,
    policy: WritePolicy,
) -> Result<(), FilesystemWriteError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| (Path::new(entry.relative_path()), entry.bytes()))
        .collect::<Vec<_>>();

    let mut issues = check_plan(&files);
    if issues.is_empty() {
        check_destination(destination, &files, policy, &mut issues)?;
    }
    if !issues.is_empty() {
        return Err(FilesystemWriteError::Rejected { issues });
    }

    let failed = |path: PathBuf, completed: Vec<PathBuf>| {
        move |source| FilesystemWriteError::Io {
            path,
            completed,
            source,
        }
    };
    std::fs::create_dir_all(destination).map_err(failed(destination.to_owned(), Vec::new()))?;
    let root = Dir::open_ambient_dir(destination, ambient_authority())
        .map_err(failed(destination.to_owned(), Vec::new()))?;
    let mut completed = Vec::with_capacity(files.len());
    for (relative, bytes) in files {
        let name = relative.file_name().expect("a checked path names a file");
        create_parents(&root, relative)
            .and_then(|parent| commit(&parent, name, bytes, policy))
            .map_err(failed(destination.join(relative), completed.clone()))?;
        completed.push(relative.to_owned());
    }
    Ok(())
}

/// Writes each artifact of a succeeded Compilation Result to its path.
///
/// `paths` pairs with the result's artifacts in order. Each path's directory
/// must exist; the path is resolved natively, so it may contain symbolic
/// links and `..` components.
pub fn write_compilation_artifacts_to_filesystem_paths(
    result: &CompilationResult,
    paths: &[PathBuf],
    policy: WritePolicy,
) -> Result<(), FilesystemWriteError> {
    let mut issues = Vec::new();
    if result.status() != CompilationStatus::Succeeded {
        issues.push(WriteIssue::FailedCompilation);
    }
    if result.artifacts().len() != paths.len() {
        issues.push(WriteIssue::PathCountMismatch {
            artifacts: result.artifacts().len(),
            paths: paths.len(),
        });
    }
    let mut sorted = paths.iter().collect::<Vec<_>>();
    sorted.sort();
    for (index, first) in sorted.iter().enumerate() {
        for second in &sorted[index + 1..] {
            if second.starts_with(first) {
                issues.push(WriteIssue::PathConflict {
                    first: first.to_path_buf(),
                    second: second.to_path_buf(),
                });
            }
        }
    }
    if !issues.is_empty() {
        return Err(FilesystemWriteError::Rejected { issues });
    }

    let mut completed = Vec::with_capacity(paths.len());
    for (artifact, path) in result.artifacts().iter().zip(paths) {
        write_one(path, artifact.bytes(), policy).map_err(|source| FilesystemWriteError::Io {
            path: path.clone(),
            completed: completed.clone(),
            source,
        })?;
        completed.push(path.clone());
    }
    Ok(())
}

/// Writes one file through a temporary sibling under `policy`.
pub(crate) fn write_file(
    path: &Path,
    bytes: &[u8],
    policy: WritePolicy,
) -> Result<(), FilesystemWriteError> {
    write_one(path, bytes, policy).map_err(|source| FilesystemWriteError::Io {
        path: path.to_owned(),
        completed: Vec::new(),
        source,
    })
}

fn write_one(path: &Path, bytes: &[u8], policy: WritePolicy) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "the path does not name a file")
    })?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    commit(&dir, name, bytes, policy)
}

/// Writes `bytes` to a temporary file in `dir` and commits it as `name`.
fn commit(dir: &Dir, name: &OsStr, bytes: &[u8], policy: WritePolicy) -> io::Result<()> {
    let (temp, mut file) = create_temp(dir)?;
    let written = file.write_all(bytes).and_then(|()| file.flush());
    drop(file);
    let committed = written.and_then(|()| match policy {
        WritePolicy::Overwrite => dir.rename(&temp, dir, name),
        WritePolicy::CreateNew => match dir.hard_link(&temp, dir, name) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
                // No hard links here (or the link failed for another
                // reason): an exclusive create still refuses an existing
                // target, at the cost of atomic content.
                write_exclusive(dir, name, bytes)
            }
            linked => linked,
        },
    });
    if policy == WritePolicy::CreateNew || committed.is_err() {
        let _ = dir.remove_file(&temp);
    }
    committed
}

fn write_exclusive(dir: &Dir, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = dir.open_with(name, &options)?;
    let written = file.write_all(bytes).and_then(|()| file.flush());
    drop(file);
    if written.is_err() {
        let _ = dir.remove_file(name);
    }
    written
}

fn create_temp(dir: &Dir) -> io::Result<(OsString, cap_std::fs::File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut attempts = 0;
    loop {
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = OsString::from(format!(".typst-pack-{}-{sequence}.tmp", std::process::id()));
        match dir.open_with(&name, &options) {
            Ok(file) => return Ok((name, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && attempts < 64 => {
                attempts += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Creates the directories above `relative` without following links.
fn create_parents(root: &Dir, relative: &Path) -> io::Result<Dir> {
    let mut dir = root.try_clone()?;
    for component in relative.parent().into_iter().flat_map(Path::components) {
        match dir.create_dir(component) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
            _ => {}
        }
        dir = dir.open_dir_nofollow(component)?;
    }
    Ok(dir)
}

/// Checks the planned paths themselves.
fn check_plan(files: &[(&Path, &[u8])]) -> Vec<WriteIssue> {
    let mut issues = Vec::new();
    let mut folded = BTreeMap::<String, &Path>::new();
    for (path, _) in files {
        let components = path
            .components()
            .map(|component| match component {
                Component::Normal(name) => name.to_str(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        let Some(components) = components.filter(|components| !components.is_empty()) else {
            issues.push(WriteIssue::InvalidPath {
                path: path.to_path_buf(),
            });
            continue;
        };
        for component in &components {
            if !is_portable_name(component) {
                issues.push(WriteIssue::ReservedName {
                    path: path.to_path_buf(),
                    component: (*component).to_owned(),
                });
            }
        }
        if let Some(first) = folded.insert(components.join("/").to_lowercase(), path) {
            issues.push(WriteIssue::PathConflict {
                first: first.to_path_buf(),
                second: path.to_path_buf(),
            });
        }
    }
    // A file must not collide with a directory another file needs.
    for (path, _) in files {
        let mut ancestor = String::new();
        for component in path.parent().into_iter().flat_map(Path::iter) {
            if !ancestor.is_empty() {
                ancestor.push('/');
            }
            ancestor.push_str(&component.to_string_lossy().to_lowercase());
            if let Some(first) = folded.get(&ancestor) {
                issues.push(WriteIssue::PathConflict {
                    first: first.to_path_buf(),
                    second: path.to_path_buf(),
                });
            }
        }
    }
    issues
}

/// Whether one path component can be created on every supported platform.
///
/// Windows reserves device names, strips trailing dots and spaces, and
/// forbids control characters; these are refused everywhere so a Pack
/// extracts the same on every host. Characters Windows forbids are refused
/// only there, where `:` would otherwise name an alternate data stream.
fn is_portable_name(component: &str) -> bool {
    if component.ends_with(['.', ' ']) || component.chars().any(char::is_control) {
        return false;
    }
    if cfg!(windows) && component.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*']) {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            });
    !device
}

/// Checks existing destination entries against the plan.
fn check_destination(
    destination: &Path,
    files: &[(&Path, &[u8])],
    policy: WritePolicy,
    issues: &mut Vec<WriteIssue>,
) -> Result<(), FilesystemWriteError> {
    let failed = |path: PathBuf| {
        move |source| FilesystemWriteError::Io {
            path,
            completed: Vec::new(),
            source,
        }
    };
    // The destination itself may be reached through symbolic links.
    match std::fs::metadata(destination) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            issues.push(WriteIssue::ConflictingAncestor {
                path: PathBuf::new(),
                ancestor: destination.to_owned(),
            });
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(failed(destination.to_owned())(error)),
    }
    let root = Dir::open_ambient_dir(destination, ambient_authority())
        .map_err(failed(destination.to_owned()))?;

    let mut reported = HashSet::new();
    'files: for (relative, _) in files {
        let mut ancestor = PathBuf::new();
        for component in relative.parent().into_iter().flat_map(Path::components) {
            ancestor.push(component);
            match root.symlink_metadata(&ancestor) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    if reported.insert(ancestor.clone()) {
                        issues.push(WriteIssue::ConflictingAncestor {
                            path: relative.to_path_buf(),
                            ancestor: destination.join(&ancestor),
                        });
                    }
                    continue 'files;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue 'files,
                Err(error) => return Err(failed(destination.join(&ancestor))(error)),
            }
        }
        match root.symlink_metadata(relative) {
            Ok(metadata) if metadata.is_file() => {
                if policy == WritePolicy::CreateNew {
                    issues.push(WriteIssue::ExistingTarget {
                        path: relative.to_path_buf(),
                    });
                }
            }
            Ok(_) => issues.push(WriteIssue::ConflictingTarget {
                path: relative.to_path_buf(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed(destination.join(relative))(error)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_names_refuse_windows_reserved_forms_everywhere() {
        for name in [
            "CON",
            "con.txt",
            "Aux",
            "nul.tar.gz",
            "COM1",
            "lpt9.md",
            "a.",
            "a ",
            "a\u{7}",
        ] {
            assert!(!is_portable_name(name), "{name}");
        }
        for name in ["main.typ", ".typkignore", "console.txt", "COM10", "lpt"] {
            assert!(is_portable_name(name), "{name}");
        }
    }

    #[test]
    fn a_failed_commit_removes_the_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let dir = Dir::open_ambient_dir(directory.path(), ambient_authority()).unwrap();
        std::fs::create_dir(directory.path().join("taken")).unwrap();

        assert!(commit(&dir, OsStr::new("taken"), b"bytes", WritePolicy::Overwrite).is_err());
        assert!(commit(&dir, OsStr::new("taken"), b"bytes", WritePolicy::CreateNew).is_err());

        let names = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(names, ["taken"]);
    }

    #[test]
    fn create_new_falls_back_to_an_exclusive_create() {
        let directory = tempfile::tempdir().unwrap();
        let dir = Dir::open_ambient_dir(directory.path(), ambient_authority()).unwrap();

        write_exclusive(&dir, OsStr::new("new"), b"bytes").unwrap();
        let error = write_exclusive(&dir, OsStr::new("new"), b"other").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(directory.path().join("new")).unwrap(),
            b"bytes"
        );
    }
}
