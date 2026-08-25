//! The symlink-refusing open shared by the reference filesystem readers.
//!
//! Project, package, and font reading each expose their own public issue,
//! entry-kind, and error vocabulary, but they reach a selected file under one
//! rule: resolve every component beneath the root without following a symlink,
//! and accept nothing that is not a regular file. This module owns that rule
//! once. Each reader keeps its own vocabulary by implementing
//! [`TraversalPolicy`], so a shared mechanism never widens a public surface.
//!
//! Resolution goes through `cap-primitives`, the same capability-based layer
//! the filesystem write adapter uses. On Linux that resolves each component
//! with `openat2` under `RESOLVE_BENEATH` and `RESOLVE_NO_MAGICLINKS`, falling
//! back to `openat` where the syscall is unavailable, so a component can
//! neither escape the root nor traverse a magic link. Windows and macOS reach
//! the same refusals through their own implementations of the same options.

use std::fs::{File, FileType};
use std::io;
use std::path::Path;

use cap_fs_ext::{OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_primitives::fs::{FollowSymlinks, OpenOptions};
use cap_std::ambient_authority;

/// An eligible entry that cannot become a regular file the reader accepts.
///
/// Each reader maps this into its own public entry-kind vocabulary. Only Unix
/// distinguishes the named kinds; elsewhere every such entry is `Unknown`, so
/// the remaining variants are constructed on Unix alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) enum UnsupportedEntry {
    Socket,
    Fifo,
    BlockDevice,
    CharacterDevice,
    Unknown,
}

impl UnsupportedEntry {
    /// Classifies an opened entry that is not a regular file.
    pub(crate) fn of(file_type: &FileType) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;

            if file_type.is_socket() {
                return Self::Socket;
            }
            if file_type.is_fifo() {
                return Self::Fifo;
            }
            if file_type.is_block_device() {
                return Self::BlockDevice;
            }
            if file_type.is_char_device() {
                return Self::CharacterDevice;
            }
        }
        #[cfg(not(unix))]
        let _ = file_type;
        Self::Unknown
    }
}

/// How one reader names the failures this traversal can produce.
pub(crate) trait TraversalPolicy {
    /// The reader's own read error.
    type Error;

    /// The message asserting that a selected path stays beneath its root.
    const ROOT_INVARIANT: &'static str;

    /// Whether an aliased root is itself a refusal.
    ///
    /// Font roots are host configuration opened exactly as configured, so a
    /// symlinked root is refused there. Project and package roots are
    /// established before traversal begins and are opened as given.
    const ALIASED_ROOT_IS_REFUSED: bool = false;

    /// Reports an I/O failure reaching `path`.
    fn io(&self, path: &Path, source: io::Error) -> Self::Error;

    /// Reports that `path` is, or lies behind, a symlink.
    fn alias(&self, path: &Path) -> Self::Error;

    /// Reports that `path` is not a regular file.
    fn unsupported_entry(&self, path: &Path, entry: UnsupportedEntry) -> Self::Error;
}

/// Opens one selected file beneath `root` without following a symlink.
///
/// Each component is resolved against the descriptor of the directory that
/// contains it, so no path string is reopened between the check and the use.
pub(crate) fn open_without_following<P: TraversalPolicy>(
    policy: &P,
    root: &Path,
    path: &Path,
) -> Result<File, P::Error> {
    let relative = path.strip_prefix(root).expect(P::ROOT_INVARIANT);
    let mut components = relative.components().peekable();
    let mut current = root.to_owned();
    let mut directory = open_root(policy, root)?;
    while let Some(component) = components.next() {
        current.push(component.as_os_str());
        let name = Path::new(component.as_os_str());
        if components.peek().is_none() {
            let file = cap_primitives::fs::open(&directory, name, &selected_file_options())
                .map_err(|error| classify(policy, &current, error))?;
            return validate_opened_file(policy, file, &current);
        }
        directory = cap_primitives::fs::open_dir_nofollow(&directory, name)
            .map_err(|error| classify(policy, &current, error))?;
    }
    unreachable!("{}", P::ROOT_INVARIANT)
}

/// Opens the traversal root, refusing an aliased root where the policy does.
fn open_root<P: TraversalPolicy>(policy: &P, root: &Path) -> Result<File, P::Error> {
    if !P::ALIASED_ROOT_IS_REFUSED {
        return cap_primitives::fs::open_ambient_dir(root, ambient_authority())
            .map_err(|error| policy.io(root, error));
    }

    let mut options = OpenOptions::new();
    options
        .read(true)
        .follow(FollowSymlinks::No)
        .maybe_dir(true);
    cap_primitives::fs::open_ambient(root, &options, ambient_authority())
        .map_err(|error| classify(policy, root, error))
}

/// The options one selected file is opened under.
///
/// `maybe_dir` keeps a directory reaching [`validate_opened_file`], so an
/// eligible entry of the wrong kind stays a typed survey issue instead of
/// becoming a bare I/O error on Windows. On Unix `O_NONBLOCK` keeps a FIFO
/// from blocking the open.
fn selected_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .follow(FollowSymlinks::No)
        .maybe_dir(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;

        options.custom_flags(libc::O_NONBLOCK);
    }
    options
}

/// Separates a refused alias from an ordinary I/O failure.
///
/// The open already failed, so re-reading the path decides only which typed
/// failure to report; it never promotes a path to one that gets read.
fn classify<P: TraversalPolicy>(policy: &P, path: &Path, error: io::Error) -> P::Error {
    if is_alias(path) {
        return policy.alias(path);
    }
    policy.io(path, error)
}

/// Accepts an opened entry only when it is still a regular file.
fn validate_opened_file<P: TraversalPolicy>(
    policy: &P,
    file: File,
    path: &Path,
) -> Result<File, P::Error> {
    let metadata = file.metadata().map_err(|error| policy.io(path, error))?;
    if metadata.file_type().is_symlink() {
        return Err(policy.alias(path));
    }
    if !metadata.file_type().is_file() {
        return Err(policy.unsupported_entry(path, UnsupportedEntry::of(&metadata.file_type())));
    }
    Ok(file)
}

fn is_alias(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}
