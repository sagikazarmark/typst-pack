//! Writing extraction plans, compiled artifacts, and pack archives.
//!
//! Every function takes its target first, then the data, then the policy.
//!
//! Every file is first written to a uniquely named temporary file in its
//! target directory, then committed under one [`WritePolicy`]:
//!
//! - [`WritePolicy::Overwrite`] renames the temporary file over the target,
//!   so readers observe either the old or the new content. On Unix the
//!   replaced file's permissions carry over to the new one.
//! - [`WritePolicy::CreateNew`] hard-links the temporary file to the target,
//!   which fails when the target exists. Any other link failure (FAT, exFAT,
//!   and some network and FUSE filesystems have no hard links) removes the
//!   temporary file and creates the target exclusively, then writes it; an
//!   exclusive create never replaces an existing entry, but an interrupted
//!   write can leave a partial file. Later files of the same write call skip
//!   the temporary file.
//!
//! The temporary file is removed on every path. Nothing is flushed to stable
//! storage: the adapter makes no crash-durability guarantee.
//!
//! [`write_file`], [`write_archive`], and [`write_artifacts`] write through a
//! symbolic link at the
//! final path component, as Typst writes its output. An extraction plan is
//! instead checked before anything is written: its paths must be canonical,
//! portable to every platform, and free of collisions when case and Unicode
//! normalization are ignored, and existing destination entries must not
//! conflict with it. Directories beneath the destination are created without
//! following symbolic links, so the destination path itself may contain links
//! but a linked target, or a linked or non-directory ancestor, beneath it is
//! refused. These checks are validation, not confinement against a concurrent
//! local peer.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use unicode_normalization::UnicodeNormalization;

use crate::compile::Artifact;
use crate::extract::Plan;
use crate::{Pack, archive, list};

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
    /// A planned path is not a canonical relative path.
    #[error("`{}` is not a canonical relative path", path.display())]
    InvalidPath { path: PathBuf },
    /// A planned path has a component some platform cannot create.
    #[error("`{}` contains `{component}`, which is not a portable file name", path.display())]
    ReservedName { path: PathBuf, component: String },
    /// Two paths collide when case is ignored, or one needs the other as a directory.
    #[error("`{}` and `{}` collide on case-insensitive filesystems", first.display(), second.display())]
    PathConflict { first: PathBuf, second: PathBuf },
    /// A target exists under `CreateNew`.
    #[error("`{}` already exists", path.display())]
    ExistingTarget { path: PathBuf },
    /// A target exists and is not a regular file.
    #[error("`{}` exists and is not a regular file", path.display())]
    ConflictingTarget { path: PathBuf },
    /// A directory a target needs is a link or a file.
    #[error("`{}` is a symbolic link or a file, not a directory", ancestor.display())]
    ConflictingAncestor { path: PathBuf, ancestor: PathBuf },
    /// The numbers of artifacts and paths differ.
    #[error("{artifacts} artifact(s) were given, but {paths} path(s)")]
    PathCountMismatch { artifacts: usize, paths: usize },
}

/// A failed filesystem write.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The write was refused before anything was written.
    #[error("{}", list("nothing was written", issues))]
    Rejected { issues: Vec<WriteIssue> },
    /// Writing `path` failed after the `completed` files were written.
    #[error("cannot write `{}`", path.display())]
    Io {
        path: PathBuf,
        /// Files committed before the failure, in write order.
        completed: Vec<PathBuf>,
        #[source]
        source: io::Error,
    },
}

/// Writes every file of an extraction plan beneath `destination`.
///
/// The destination is created when missing. Nothing is written when the plan
/// or an existing destination entry conflicts with it; a failure while
/// writing reports the files already committed.
pub fn write_extraction(
    destination: impl AsRef<Path>,
    plan: &Plan,
    policy: WritePolicy,
) -> Result<(), WriteError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| (Path::new(entry.path()), entry.bytes()))
        .collect::<Vec<_>>();

    let mut issues = check_plan(&files);
    if issues.is_empty() {
        check_destination(destination, &files, policy, &mut issues)?;
    }
    if !issues.is_empty() {
        return Err(WriteError::Rejected { issues });
    }

    let failed = |path: PathBuf, completed: &[PathBuf], source| WriteError::Io {
        path,
        completed: completed.to_vec(),
        source,
    };
    let root = std::fs::create_dir_all(destination)
        .and_then(|()| Dir::open_ambient_dir(destination, ambient_authority()))
        .map_err(|source| failed(destination.to_owned(), &[], source))?;
    let mut hard_links = true;
    let mut completed = Vec::with_capacity(files.len());
    for (relative, bytes) in files {
        let name = relative.file_name().expect("a checked path names a file");
        create_parents(&root, relative)
            .and_then(|parent| commit(&parent, name, bytes, policy, &mut hard_links))
            .map_err(|source| failed(destination.join(relative), &completed, source))?;
        completed.push(relative.to_owned());
    }
    Ok(())
}

/// Writes each compiled artifact to its path.
///
/// `paths` pairs with the artifacts in order. Each path's directory
/// must exist; the path is resolved natively, so it may contain symbolic
/// links and `..` components, and a symbolic link at the final component is
/// written through as [`write_file`] does.
pub fn write_artifacts(
    paths: &[impl AsRef<Path>],
    artifacts: &[Artifact],
    policy: WritePolicy,
) -> Result<(), WriteError> {
    let paths = paths.iter().map(AsRef::as_ref).collect::<Vec<&Path>>();
    let mut issues = Vec::new();
    if artifacts.len() != paths.len() {
        issues.push(WriteIssue::PathCountMismatch {
            artifacts: artifacts.len(),
            paths: paths.len(),
        });
    }
    let mut sorted = paths.clone();
    sorted.sort();
    let unique = sorted.iter().copied().collect::<BTreeSet<_>>();
    for (index, path) in sorted.iter().enumerate() {
        if index > 0 && sorted[index - 1] == *path {
            issues.push(WriteIssue::PathConflict {
                first: path.to_path_buf(),
                second: path.to_path_buf(),
            });
            continue;
        }
        for ancestor in path.ancestors().skip(1) {
            if unique.contains(ancestor) {
                issues.push(WriteIssue::PathConflict {
                    first: ancestor.to_path_buf(),
                    second: path.to_path_buf(),
                });
            }
        }
    }
    if !issues.is_empty() {
        return Err(WriteError::Rejected { issues });
    }

    let mut hard_links = true;
    let mut completed = Vec::with_capacity(paths.len());
    for (artifact, path) in artifacts.iter().zip(paths) {
        write_one(path, &artifact.bytes, policy, &mut hard_links).map_err(|source| {
            WriteError::Io {
                path: path.to_owned(),
                completed: completed.clone(),
                source,
            }
        })?;
        completed.push(path.to_owned());
    }
    Ok(())
}

/// Writes one file through a temporary sibling under `policy`.
///
/// A symbolic link at the final path component is written through, as
/// Typst writes its output: [`WritePolicy::Overwrite`] replaces the file the
/// link leads to (creating it when the link dangles) and keeps the link,
/// while under [`WritePolicy::CreateNew`] any link, even a dangling one, is an
/// existing target.
pub fn write_file(
    path: impl AsRef<Path>,
    bytes: &[u8],
    policy: WritePolicy,
) -> Result<(), WriteError> {
    let path = path.as_ref();
    write_one(path, bytes, policy, &mut true).map_err(|source| WriteError::Io {
        path: path.to_owned(),
        completed: Vec::new(),
        source,
    })
}

/// Encodes a pack and writes it to `path` as [`write_file`] does.
pub fn write_archive(
    path: impl AsRef<Path>,
    pack: &Pack,
    policy: WritePolicy,
) -> Result<(), WriteError> {
    write_file(path, &archive::encode(pack), policy)
}

fn write_one(
    path: &Path,
    bytes: &[u8],
    policy: WritePolicy,
    hard_links: &mut bool,
) -> io::Result<()> {
    let path = match policy {
        WritePolicy::Overwrite => follow_links(path)?,
        WritePolicy::CreateNew => path.to_owned(),
    };
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "the path does not name a file")
    })?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    commit(&dir, name, bytes, policy, hard_links)
}

/// Follows symbolic links at the final component of `path` to the path a
/// write through it would reach, which need not exist.
fn follow_links(path: &Path) -> io::Result<PathBuf> {
    // Linux's own limit on links followed in one lookup.
    const MAX_LINKS: usize = 40;

    let mut path = path.to_owned();
    for _ in 0..MAX_LINKS {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::read_link(&path)?;
                // A relative target is relative to the link's directory.
                path = match path.parent() {
                    Some(parent) => parent.join(target),
                    None => target,
                };
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => return Ok(path),
        }
    }
    Err(io::Error::other(format!(
        "`{}` is reached through more than {MAX_LINKS} symbolic links",
        path.display()
    )))
}

/// Writes `bytes` to a temporary file in `dir` and commits it as `name`.
///
/// `hard_links` starts `true` for one write call and turns `false` once
/// linking fails, so later files of the call are created exclusively
/// without a temporary file.
fn commit(
    dir: &Dir,
    name: &OsStr,
    bytes: &[u8],
    policy: WritePolicy,
    hard_links: &mut bool,
) -> io::Result<()> {
    if policy == WritePolicy::CreateNew && !*hard_links {
        return write_exclusive(dir, name, bytes);
    }
    let (temp, mut file) = create_temp(dir)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| match policy {
            WritePolicy::Overwrite => keep_permissions(dir, name, &file),
            WritePolicy::CreateNew => Ok(()),
        });
    drop(file);
    if let Err(error) = written {
        let _ = dir.remove_file(&temp);
        return Err(error);
    }
    match policy {
        WritePolicy::Overwrite => {
            let renamed = dir.rename(&temp, dir, name);
            if renamed.is_err() {
                let _ = dir.remove_file(&temp);
            }
            renamed
        }
        WritePolicy::CreateNew => {
            let linked = dir.hard_link(&temp, dir, name);
            settle_link(dir, &temp, name, bytes, linked, hard_links)
        }
    }
}

/// Finishes a `CreateNew` commit once the temporary file `temp` was linked
/// to `name` with the result `linked`.
///
/// Any link failure other than an existing target means no hard links here
/// (or none usable): the target is then created exclusively and written,
/// which never replaces an existing entry either.
fn settle_link(
    dir: &Dir,
    temp: &OsStr,
    name: &OsStr,
    bytes: &[u8],
    linked: io::Result<()>,
    hard_links: &mut bool,
) -> io::Result<()> {
    // The temporary file goes first: the fallback writes the bytes again, and
    // the filesystem may have no room for two copies.
    let _ = dir.remove_file(temp);
    match linked {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
            *hard_links = false;
            write_exclusive(dir, name, bytes)
        }
        linked => linked,
    }
}

/// Gives the temporary `file` the permissions of the regular file it
/// replaces, as rewriting that file in place would keep them.
#[cfg(unix)]
fn keep_permissions(dir: &Dir, name: &OsStr, file: &cap_std::fs::File) -> io::Result<()> {
    match dir.metadata(name) {
        Ok(metadata) if metadata.is_file() => file.set_permissions(metadata.permissions()),
        _ => Ok(()),
    }
}

/// Windows permissions are only the read-only flag, and a read-only target
/// cannot be replaced anyway.
#[cfg(not(unix))]
fn keep_permissions(_: &Dir, _: &OsStr, _: &cap_std::fs::File) -> io::Result<()> {
    Ok(())
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
    let mut ancestors = Vec::new();
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
        let mut key = String::new();
        for (index, component) in components.iter().enumerate() {
            if index > 0 {
                key.push('/');
                ancestors.push((key[..key.len() - 1].to_owned(), *path));
            }
            key.push_str(&fold(component));
        }
        if let Some(first) = folded.insert(key, path) {
            issues.push(WriteIssue::PathConflict {
                first: first.to_path_buf(),
                second: path.to_path_buf(),
            });
        }
    }
    // A file must not collide with a directory another file needs.
    for (ancestor, path) in ancestors {
        if let Some(first) = folded.get(&ancestor) {
            issues.push(WriteIssue::PathConflict {
                first: first.to_path_buf(),
                second: path.to_path_buf(),
            });
        }
    }
    issues
}

/// A path component as filesystems that ignore case and Unicode
/// normalization (APFS, HFS+, NTFS) compare it.
///
/// Lowercasing goes character by character, so a final sigma folds like any
/// other sigma.
fn fold(component: &str) -> String {
    component
        .chars()
        .flat_map(char::to_lowercase)
        .nfc()
        .collect()
}

/// Whether one path component can be created on every supported platform.
///
/// Windows reserves device names, strips trailing dots and spaces, and
/// forbids control characters; these are refused everywhere so a pack
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
    let device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
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
) -> Result<(), WriteError> {
    let failed = |path: PathBuf| {
        move |source| WriteError::Io {
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
            "CONIN$",
            "conout$.log",
            "a.",
            "a ",
            "a\u{7}",
        ] {
            assert!(!is_portable_name(name), "{name}");
        }
        for name in [
            "main.typ",
            ".typkignore",
            "console.txt",
            "COM10",
            "lpt",
            "CONIN",
        ] {
            assert!(is_portable_name(name), "{name}");
        }
    }

    fn names(directory: &Path) -> Vec<OsString> {
        let mut names = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[test]
    fn a_failed_commit_removes_the_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let dir = Dir::open_ambient_dir(directory.path(), ambient_authority()).unwrap();
        std::fs::create_dir(directory.path().join("taken")).unwrap();

        for policy in [WritePolicy::Overwrite, WritePolicy::CreateNew] {
            assert!(commit(&dir, OsStr::new("taken"), b"bytes", policy, &mut true).is_err());
        }

        assert_eq!(names(directory.path()), ["taken"]);
    }

    #[test]
    fn without_hard_links_create_new_removes_the_temporary_file_then_creates_exclusively() {
        let directory = tempfile::tempdir().unwrap();
        let dir = Dir::open_ambient_dir(directory.path(), ambient_authority()).unwrap();
        let unsupported = || Err(io::Error::from(io::ErrorKind::Unsupported));
        let mut hard_links = true;

        // The temporary file shares the target's name here, so the exclusive
        // create succeeds only if the temporary file is removed first, never
        // holding two copies of the bytes.
        std::fs::write(directory.path().join("first"), b"temporary").unwrap();
        let first = OsStr::new("first");
        settle_link(&dir, first, first, b"bytes", unsupported(), &mut hard_links).unwrap();
        assert_eq!(
            std::fs::read(directory.path().join("first")).unwrap(),
            b"bytes"
        );
        assert!(!hard_links);

        // The exclusive create still refuses an existing target.
        std::fs::write(directory.path().join("temporary"), b"temporary").unwrap();
        let temporary = OsStr::new("temporary");
        let error =
            settle_link(&dir, temporary, first, b"other", unsupported(), &mut true).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(directory.path().join("first")).unwrap(),
            b"bytes"
        );

        // Once links are known to fail, later files skip the temporary file.
        commit(
            &dir,
            OsStr::new("second"),
            b"second",
            WritePolicy::CreateNew,
            &mut hard_links,
        )
        .unwrap();
        let error = commit(
            &dir,
            first,
            b"other",
            WritePolicy::CreateNew,
            &mut hard_links,
        );
        assert_eq!(error.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(names(directory.path()), ["first", "second"]);
    }

    #[test]
    fn an_existing_target_does_not_trigger_the_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let dir = Dir::open_ambient_dir(directory.path(), ambient_authority()).unwrap();
        std::fs::write(directory.path().join("target"), b"old").unwrap();
        std::fs::write(directory.path().join("temporary"), b"new").unwrap();
        let mut hard_links = true;

        let error = settle_link(
            &dir,
            OsStr::new("temporary"),
            OsStr::new("target"),
            b"new",
            Err(io::Error::from(io::ErrorKind::AlreadyExists)),
            &mut hard_links,
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(hard_links);
        assert_eq!(names(directory.path()), ["target"]);
    }

    #[test]
    fn folding_ignores_case_and_unicode_normalization() {
        assert_eq!(fold("Caf\u{e9}"), fold("cafe\u{301}"));
        assert_eq!(fold("a\u{3a3}"), fold("a\u{3c3}"));
        assert_ne!(fold("a"), fold("b"));
    }
}
