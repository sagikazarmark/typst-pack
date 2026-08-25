//! The symlink-refusing open shared by the reference filesystem readers.
//!
//! Project, package, and font reading each expose their own public issue,
//! entry-kind, and error vocabulary, but they reach a selected file under one
//! rule: resolve every component beneath the root without following a symlink,
//! and accept nothing that is not a regular file. This module owns that rule
//! once. Each reader keeps its own vocabulary by implementing
//! [`TraversalPolicy`], so a shared mechanism never widens a public surface.

use std::fs::{File, FileType};
use std::path::Path;

#[cfg(not(unix))]
use std::fs::OpenOptions;
#[cfg(not(unix))]
use std::path::PathBuf;

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
    fn io(&self, path: &Path, source: std::io::Error) -> Self::Error;

    /// Reports that `path` is, or lies behind, a symlink.
    fn alias(&self, path: &Path) -> Self::Error;

    /// Reports that `path` is not a regular file.
    fn unsupported_entry(&self, path: &Path, entry: UnsupportedEntry) -> Self::Error;
}

/// Opens one selected file beneath `root` without following a symlink.
///
/// On Unix each component is resolved with `openat` under `O_NOFOLLOW`, so no
/// path string is ever reopened between the check and the use. Elsewhere the
/// traversal is checked before the open, again on failure, and once more after
/// a successful open, because those platforms expose no equivalent flag.
#[cfg(unix)]
pub(crate) fn open_without_following<P: TraversalPolicy>(
    policy: &P,
    root: &Path,
    path: &Path,
) -> Result<File, P::Error> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = path.strip_prefix(root).expect(P::ROOT_INVARIANT);
    let mut components = relative.components().peekable();
    let mut current = root.to_owned();
    let mut directory = open_root(policy, root)?;
    while let Some(component) = components.next() {
        current.push(component.as_os_str());
        let name = CString::new(component.as_os_str().as_bytes())
            .expect("filesystem path components contain no NUL bytes");
        let final_component = components.peek().is_none();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | libc::O_NOFOLLOW
            | if final_component {
                0
            } else {
                libc::O_DIRECTORY
            };
        // SAFETY: the directory descriptor and NUL-terminated component remain
        // valid for the call, and a successful descriptor is immediately owned.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            if is_alias(&current) {
                return Err(policy.alias(&current));
            }
            return Err(policy.io(&current, std::io::Error::last_os_error()));
        }
        // SAFETY: `openat` returned a new owned descriptor.
        let opened = unsafe { File::from_raw_fd(descriptor) };
        if final_component {
            return validate_opened_file(policy, opened, &current);
        }
        directory = opened;
    }
    unreachable!("{}", P::ROOT_INVARIANT)
}

/// Opens the traversal root, refusing an aliased root where the policy does.
#[cfg(unix)]
fn open_root<P: TraversalPolicy>(policy: &P, root: &Path) -> Result<File, P::Error> {
    use std::ffi::CString;
    use std::os::fd::FromRawFd;
    use std::os::unix::ffi::OsStrExt;

    if !P::ALIASED_ROOT_IS_REFUSED {
        return File::open(root).map_err(|error| policy.io(root, error));
    }

    let root_path =
        CString::new(root.as_os_str().as_bytes()).expect("filesystem paths contain no NUL bytes");
    // SAFETY: the NUL-terminated path remains valid for the call, and a
    // successful descriptor is immediately owned.
    let descriptor = unsafe {
        libc::open(
            root_path.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | libc::O_NOFOLLOW
                | libc::O_DIRECTORY,
        )
    };
    if descriptor < 0 {
        if is_alias(root) {
            return Err(policy.alias(root));
        }
        return Err(policy.io(root, std::io::Error::last_os_error()));
    }
    // SAFETY: `open` returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

#[cfg(not(unix))]
pub(crate) fn open_without_following<P: TraversalPolicy>(
    policy: &P,
    root: &Path,
    path: &Path,
) -> Result<File, P::Error> {
    if let Some(alias) = first_alias::<P>(root, path) {
        return Err(policy.alias(&alias));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }

    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) => {
            if let Some(alias) = first_alias::<P>(root, path) {
                return Err(policy.alias(&alias));
            }
            return Err(policy.io(path, error));
        }
    };
    if let Some(alias) = first_alias::<P>(root, path) {
        return Err(policy.alias(&alias));
    }
    validate_opened_file(policy, file, path)
}

/// Returns the first component of `path` that is a symlink, if any.
#[cfg(not(unix))]
fn first_alias<P: TraversalPolicy>(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).expect(P::ROOT_INVARIANT);
    let mut current = root.to_owned();
    if P::ALIASED_ROOT_IS_REFUSED && is_alias(&current) {
        return Some(current);
    }
    for component in relative.components() {
        current.push(component.as_os_str());
        if is_alias(&current) {
            return Some(current);
        }
    }
    None
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
