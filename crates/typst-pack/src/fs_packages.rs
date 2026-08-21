//! The package-reading half of Pack Assembly for the reference filesystem
//! Pack Assembler.
//!
//! Reading is resume-driven: the core reports the exact specifications its
//! representative request read and was not given, and the adapter obtains each
//! of them through the configured Package Authority — local package
//! directories, then the package cache, then a download unless creation is
//! offline or the build has no egress compiled in to download with.

use std::fs::File;
#[cfg(not(unix))]
use std::fs::OpenOptions;
#[cfg(feature = "egress")]
use std::io::BufReader;
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "egress")]
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(feature = "egress")]
use std::sync::OnceLock;

#[cfg(feature = "egress")]
use typst::diag::PackageError;
use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst_kit::packages::FsPackages;

use crate::error_display::format_error_list;
use crate::limits::{LimitError, Limits, ResourceKind};
use crate::package_catalog::{PackageTree, PackageTreeError};
use crate::package_failure::{PackageReadFailure, PackageReadFailureReason};
#[cfg(feature = "egress")]
use crate::{
    PackageArchiveReadError, PackageExpansionLimits, PackageReadError, expand_package_archive,
    package_archive_url, read_package_archive,
};

#[cfg(feature = "egress")]
const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

/// A resource bounded during filesystem Package Tree reading.
pub type FilesystemPackageResource = ResourceKind<1>;

#[allow(non_upper_case_globals)]
impl ResourceKind<1> {
    pub const VisitedEntries: Self = Self::new(0);
    pub const SelectedFiles: Self = Self::new(1);
    pub const SelectedFileBytes: Self = Self::new(2);
    pub const PackageTreeBytes: Self = Self::new(3);
}

/// A filesystem package exceeded a mandatory reading ceiling.
pub type FilesystemPackageLimitError = LimitError<FilesystemPackageResource>;

/// Mandatory finite resource ceilings for filesystem Package Tree reading.
pub type FilesystemPackageLimits = Limits<FilesystemPackageResource>;

impl Limits<FilesystemPackageResource> {
    #[track_caller]
    pub fn new(
        visited_entries: u64,
        selected_files: u64,
        selected_file_bytes: u64,
        package_tree_bytes: u64,
    ) -> Self {
        Self::from_ceilings([
            visited_entries,
            selected_files,
            selected_file_bytes,
            package_tree_bytes,
            0,
            0,
            0,
        ])
        .assert_probe_resources([
            FilesystemPackageResource::VisitedEntries,
            FilesystemPackageResource::SelectedFiles,
            FilesystemPackageResource::SelectedFileBytes,
            FilesystemPackageResource::PackageTreeBytes,
        ])
    }

    /// The first-party limits for package trees read from filesystems.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            100_000,
            50_000,
            64 * 1024 * 1024,
            512 * 1024 * 1024,
            0,
            0,
            0,
        ])
    }

    pub const fn visited_entries(&self) -> u64 {
        self.ceilings[0]
    }

    pub const fn selected_files(&self) -> u64 {
        self.ceilings[1]
    }

    pub const fn selected_file_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    pub const fn package_tree_bytes(&self) -> u64 {
        self.ceilings[3]
    }
}

/// The kind of a filesystem entry that cannot become a package file.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemPackageEntryKind {
    Socket,
    Fifo,
    BlockDevice,
    CharacterDevice,
    Unknown,
}

/// The filesystem operation that failed while reading a Package Tree.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemPackageOperation {
    InspectRoot,
    SurveyEntry,
    InspectSelectedFile,
    ReadSelectedFile,
}

impl std::fmt::Display for FilesystemPackageOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InspectRoot => "inspect package root",
            Self::SurveyEntry => "survey package entry",
            Self::InspectSelectedFile => "inspect selected package file",
            Self::ReadSelectedFile => "read selected package file",
        })
    }
}

/// One independently detectable filesystem Package Tree survey issue.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemPackageIssue {
    #[error("unsupported filesystem entry {path:?}: aliases cannot become package files")]
    Alias { path: PathBuf },
    #[error("unsupported filesystem entry {path:?} in the package tree")]
    UnsupportedEntry {
        path: PathBuf,
        kind: FilesystemPackageEntryKind,
    },
    #[error("package path {path:?} is not valid UTF-8")]
    UnrepresentablePath { path: PathBuf },
    #[error("filesystem package root {path:?} is not a directory")]
    RootNotDirectory { path: PathBuf },
}

impl FilesystemPackageIssue {
    fn path(&self) -> &Path {
        match self {
            Self::Alias { path }
            | Self::UnsupportedEntry { path, .. }
            | Self::UnrepresentablePath { path }
            | Self::RootNotDirectory { path } => path,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Alias { .. } => 0,
            Self::UnsupportedEntry { .. } => 1,
            Self::UnrepresentablePath { .. } => 2,
            Self::RootNotDirectory { .. } => 3,
        }
    }
}

/// All safely detectable issues found by one filesystem package survey.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error(
    "filesystem package survey found {} issue(s){}",
    .issues.len(),
    format_error_list(.issues.as_slice())
)]
pub struct FilesystemPackageSurveyError {
    issues: Vec<FilesystemPackageIssue>,
}

impl FilesystemPackageSurveyError {
    pub fn issues(&self) -> &[FilesystemPackageIssue] {
        &self.issues
    }
}

/// A failure while reading a Package Tree from the filesystem.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemPackageReadError {
    #[error("failed to {operation} {path:?}: {source}")]
    Io {
        operation: FilesystemPackageOperation,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Survey(FilesystemPackageSurveyError),
    #[error("filesystem package resource limit at {path:?}: {source}")]
    Limit {
        path: PathBuf,
        source: FilesystemPackageLimitError,
    },
    #[error("selected filesystem entries do not form a Package Tree: {0}")]
    PackageTree(#[source] PackageTreeError),
}

impl FilesystemPackageReadError {
    fn io(
        operation: FilesystemPackageOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    fn limit(path: impl Into<PathBuf>, source: FilesystemPackageLimitError) -> Self {
        Self::Limit {
            path: path.into(),
            source,
        }
    }
}

/// Reads every addressable regular file beneath one filesystem package root.
pub fn read_filesystem_package(
    root: impl AsRef<Path>,
    limits: FilesystemPackageLimits,
) -> Result<PackageTree, FilesystemPackageReadError> {
    let root = root.as_ref();
    let root_metadata = std::fs::symlink_metadata(root).map_err(|source| {
        FilesystemPackageReadError::io(FilesystemPackageOperation::InspectRoot, root, source)
    })?;
    if root_metadata.file_type().is_symlink() {
        return Err(FilesystemPackageReadError::Survey(
            FilesystemPackageSurveyError {
                issues: vec![FilesystemPackageIssue::Alias {
                    path: root.to_owned(),
                }],
            },
        ));
    }
    if !root_metadata.is_dir() {
        return Err(FilesystemPackageReadError::Survey(
            FilesystemPackageSurveyError {
                issues: vec![FilesystemPackageIssue::RootNotDirectory {
                    path: root.to_owned(),
                }],
            },
        ));
    }

    let mut visited_entries = 0u64;
    let mut selected_files = 0u64;
    let mut declared_total = 0u64;
    let mut selected = Vec::new();
    let mut issues = Vec::new();
    let mut deferred_limit = None;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| {
            let path = error.path().unwrap_or(root).to_owned();
            let source = error
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("filesystem traversal failed"));
            FilesystemPackageReadError::io(FilesystemPackageOperation::SurveyEntry, path, source)
        })?;
        if entry.depth() == 0 {
            continue;
        }

        visited_entries = checked_add(
            visited_entries,
            1,
            FilesystemPackageResource::VisitedEntries,
        )
        .map_err(|source| FilesystemPackageReadError::limit(entry.path(), source))?;
        check_limit(
            FilesystemPackageResource::VisitedEntries,
            limits.visited_entries(),
            visited_entries,
        )
        .map_err(|source| FilesystemPackageReadError::limit(entry.path(), source))?;

        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walk remains beneath package root");
        let Some(path) = slash_path(relative) else {
            issues.push(FilesystemPackageIssue::UnrepresentablePath {
                path: entry.path().to_owned(),
            });
            continue;
        };
        let file_type = entry.file_type();
        if file_type.is_dir() {
            continue;
        }
        if file_type.is_symlink() {
            issues.push(FilesystemPackageIssue::Alias {
                path: entry.path().to_owned(),
            });
            continue;
        }
        if !file_type.is_file() {
            issues.push(FilesystemPackageIssue::UnsupportedEntry {
                path: entry.path().to_owned(),
                kind: unsupported_kind(&file_type),
            });
            continue;
        }

        selected_files =
            checked_add(selected_files, 1, FilesystemPackageResource::SelectedFiles)
                .map_err(|source| FilesystemPackageReadError::limit(entry.path(), source))?;
        if let Err(source) = check_limit(
            FilesystemPackageResource::SelectedFiles,
            limits.selected_files(),
            selected_files,
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemPackageReadError::limit(entry.path(), source));
            continue;
        }
        let metadata = entry.metadata().map_err(|error| {
            let path = error.path().unwrap_or(entry.path()).to_owned();
            let source = error.into_io_error().unwrap_or_else(|| {
                std::io::Error::other("failed to inspect selected package file")
            });
            FilesystemPackageReadError::io(
                FilesystemPackageOperation::InspectSelectedFile,
                path,
                source,
            )
        })?;
        if let Err(source) = check_limit(
            FilesystemPackageResource::SelectedFileBytes,
            limits.selected_file_bytes(),
            metadata.len(),
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemPackageReadError::limit(entry.path(), source));
            continue;
        }
        declared_total = checked_add(
            declared_total,
            metadata.len(),
            FilesystemPackageResource::PackageTreeBytes,
        )
        .map_err(|source| FilesystemPackageReadError::limit(entry.path(), source))?;
        if let Err(source) = check_limit(
            FilesystemPackageResource::PackageTreeBytes,
            limits.package_tree_bytes(),
            declared_total,
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemPackageReadError::limit(entry.path(), source));
            continue;
        }
        selected.push((path, entry.path().to_owned()));
    }

    if !issues.is_empty() {
        issues.sort_by(|left, right| {
            left.path()
                .cmp(right.path())
                .then_with(|| left.rank().cmp(&right.rank()))
        });
        return Err(FilesystemPackageReadError::Survey(
            FilesystemPackageSurveyError { issues },
        ));
    }
    if let Some(error) = deferred_limit {
        return Err(error);
    }

    selected.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut actual_total = 0u64;
    let mut entries = Vec::with_capacity(selected.len());
    for (path, source) in selected {
        let mut file = open_without_following(root, &source)?;
        let bytes = read_bounded_package_file(
            &mut file,
            &source,
            limits.selected_file_bytes(),
            actual_total,
            limits.package_tree_bytes(),
        )?;
        let observed = u64::try_from(bytes.len()).map_err(|_| {
            FilesystemPackageReadError::limit(
                &source,
                FilesystemPackageLimitError::AccountingOverflow {
                    resource: FilesystemPackageResource::PackageTreeBytes,
                },
            )
        })?;
        actual_total = checked_add(
            actual_total,
            observed,
            FilesystemPackageResource::PackageTreeBytes,
        )
        .map_err(|source_error| FilesystemPackageReadError::limit(&source, source_error))?;
        entries.push((path, bytes));
    }

    PackageTree::from_owned_entries(entries).map_err(FilesystemPackageReadError::PackageTree)
}

fn read_bounded_package_file(
    mut reader: impl Read,
    path: &Path,
    selected_file_ceiling: u64,
    total_before: u64,
    package_tree_ceiling: u64,
) -> Result<Vec<u8>, FilesystemPackageReadError> {
    let total_allowance = package_tree_ceiling.saturating_sub(total_before);
    let allowance = selected_file_ceiling.min(total_allowance);
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(allowance + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            FilesystemPackageReadError::io(
                FilesystemPackageOperation::ReadSelectedFile,
                path,
                error,
            )
        })?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        FilesystemPackageReadError::limit(
            path,
            FilesystemPackageLimitError::AccountingOverflow {
                resource: FilesystemPackageResource::SelectedFileBytes,
            },
        )
    })?;
    check_limit(
        FilesystemPackageResource::SelectedFileBytes,
        selected_file_ceiling,
        observed,
    )
    .map_err(|source| FilesystemPackageReadError::limit(path, source))?;
    let total = checked_add(
        total_before,
        observed,
        FilesystemPackageResource::PackageTreeBytes,
    )
    .map_err(|source| FilesystemPackageReadError::limit(path, source))?;
    check_limit(
        FilesystemPackageResource::PackageTreeBytes,
        package_tree_ceiling,
        total,
    )
    .map_err(|source| FilesystemPackageReadError::limit(path, source))?;
    Ok(bytes)
}

#[cfg(unix)]
fn open_without_following(root: &Path, path: &Path) -> Result<File, FilesystemPackageReadError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = path
        .strip_prefix(root)
        .expect("a selected package path remains beneath its root");
    let mut components = relative.components().peekable();
    let mut current = root.to_owned();
    let mut directory = File::open(root).map_err(|error| {
        FilesystemPackageReadError::io(FilesystemPackageOperation::ReadSelectedFile, root, error)
    })?;
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
            if std::fs::symlink_metadata(&current)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err(alias_error(&current));
            }
            return Err(FilesystemPackageReadError::io(
                FilesystemPackageOperation::ReadSelectedFile,
                &current,
                std::io::Error::last_os_error(),
            ));
        }
        // SAFETY: `openat` returned a new owned descriptor.
        let opened = unsafe { File::from_raw_fd(descriptor) };
        if final_component {
            return validate_opened_file(opened, &current);
        }
        directory = opened;
    }
    unreachable!("a selected package file has a path beneath the root")
}

#[cfg(not(unix))]
fn open_without_following(root: &Path, path: &Path) -> Result<File, FilesystemPackageReadError> {
    if let Some(alias) = first_alias(root, path) {
        return Err(alias_error(&alias));
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
            if let Some(alias) = first_alias(root, path) {
                return Err(alias_error(&alias));
            }
            return Err(FilesystemPackageReadError::io(
                FilesystemPackageOperation::ReadSelectedFile,
                path,
                error,
            ));
        }
    };
    if let Some(alias) = first_alias(root, path) {
        return Err(alias_error(&alias));
    }
    validate_opened_file(file, path)
}

fn validate_opened_file(file: File, path: &Path) -> Result<File, FilesystemPackageReadError> {
    let metadata = file.metadata().map_err(|error| {
        FilesystemPackageReadError::io(FilesystemPackageOperation::ReadSelectedFile, path, error)
    })?;
    if metadata.file_type().is_symlink() {
        return Err(alias_error(path));
    }
    if !metadata.file_type().is_file() {
        return Err(FilesystemPackageReadError::Survey(
            FilesystemPackageSurveyError {
                issues: vec![FilesystemPackageIssue::UnsupportedEntry {
                    path: path.to_owned(),
                    kind: unsupported_kind(&metadata.file_type()),
                }],
            },
        ));
    }
    Ok(file)
}

#[cfg(not(unix))]
fn first_alias(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path
        .strip_prefix(root)
        .expect("a selected package path remains beneath its root");
    let mut current = root.to_owned();
    for component in relative.components() {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Some(current);
        }
    }
    None
}

fn alias_error(path: &Path) -> FilesystemPackageReadError {
    FilesystemPackageReadError::Survey(FilesystemPackageSurveyError {
        issues: vec![FilesystemPackageIssue::Alias {
            path: path.to_owned(),
        }],
    })
}

fn slash_path(path: &Path) -> Option<String> {
    path.components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
}

fn unsupported_kind(file_type: &std::fs::FileType) -> FilesystemPackageEntryKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        if file_type.is_socket() {
            return FilesystemPackageEntryKind::Socket;
        }
        if file_type.is_fifo() {
            return FilesystemPackageEntryKind::Fifo;
        }
        if file_type.is_block_device() {
            return FilesystemPackageEntryKind::BlockDevice;
        }
        if file_type.is_char_device() {
            return FilesystemPackageEntryKind::CharacterDevice;
        }
    }
    FilesystemPackageEntryKind::Unknown
}

fn checked_add(
    total: u64,
    value: u64,
    resource: FilesystemPackageResource,
) -> Result<u64, FilesystemPackageLimitError> {
    total
        .checked_add(value)
        .ok_or(FilesystemPackageLimitError::AccountingOverflow { resource })
}

fn check_limit(
    resource: FilesystemPackageResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), FilesystemPackageLimitError> {
    if observed > ceiling {
        return Err(FilesystemPackageLimitError::exceeded(resource, ceiling));
    }
    Ok(())
}

/// A typed failure from the concrete filesystem Package Authority.
///
/// The stable Package Read Failure remains available through
/// [`Self::failure`], while adapter and transformation failures retain their
/// authoritative lower-module source.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemPackageAuthorityReadError {
    #[error(transparent)]
    Unavailable(PackageReadFailure),
    #[error("{failure}: {source}")]
    Filesystem {
        failure: PackageReadFailure,
        #[source]
        source: Box<FilesystemPackageReadError>,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    RegistryUrl {
        failure: PackageReadFailure,
        #[source]
        source: Box<PackageReadError>,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    Download {
        failure: PackageReadFailure,
        #[source]
        source: std::io::Error,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    DownloadSize {
        failure: PackageReadFailure,
        #[source]
        source: std::num::TryFromIntError,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    ArchiveRead {
        failure: PackageReadFailure,
        #[source]
        source: Box<PackageArchiveReadError>,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    ArchiveExpansion {
        failure: PackageReadFailure,
        #[source]
        source: Box<PackageReadError>,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    Cache {
        failure: PackageReadFailure,
        #[source]
        source: Box<PackageError>,
    },
}

impl FilesystemPackageAuthorityReadError {
    /// The stable exact-specification failure represented by this adapter error.
    pub fn failure(&self) -> &PackageReadFailure {
        match self {
            Self::Unavailable(failure) | Self::Filesystem { failure, .. } => failure,
            #[cfg(feature = "egress")]
            Self::RegistryUrl { failure, .. }
            | Self::Download { failure, .. }
            | Self::DownloadSize { failure, .. }
            | Self::ArchiveRead { failure, .. }
            | Self::ArchiveExpansion { failure, .. }
            | Self::Cache { failure, .. } => failure,
        }
    }
}

/// The concrete Package Authority used by the reference filesystem workflows.
///
/// Local package data, package cache, offline policy, and registry read
/// remain explicit here rather than being fallback behavior in Pack Creation.
#[derive(Debug)]
pub struct FilesystemPackageAuthority {
    data: Option<FsPackages>,
    cache: Option<FsPackages>,
    offline: bool,
    source_limits: FilesystemPackageLimits,
    #[cfg(feature = "egress")]
    expansion_limits: PackageExpansionLimits,
    #[cfg(feature = "egress")]
    certificate: Option<PathBuf>,
}

impl FilesystemPackageAuthority {
    /// Configures local and cache package directories plus offline policy.
    pub fn new(
        package_path: Option<&Path>,
        package_cache_path: Option<&Path>,
        offline: bool,
    ) -> Self {
        Self::with_limits(
            package_path,
            package_cache_path,
            offline,
            FilesystemPackageLimits::reference_v1(),
            #[cfg(feature = "egress")]
            PackageExpansionLimits::reference_v1(),
        )
    }

    pub(crate) fn with_limits(
        package_path: Option<&Path>,
        package_cache_path: Option<&Path>,
        offline: bool,
        source_limits: FilesystemPackageLimits,
        #[cfg(feature = "egress")] expansion_limits: PackageExpansionLimits,
    ) -> Self {
        let data = match package_path {
            Some(path) => Some(FsPackages::new(path)),
            None => FsPackages::system_data(),
        };
        let cache = match package_cache_path {
            Some(path) => Some(FsPackages::new(path)),
            None => FsPackages::system_cache(),
        };
        Self {
            data,
            cache,
            offline,
            source_limits,
            #[cfg(feature = "egress")]
            expansion_limits,
            #[cfg(feature = "egress")]
            certificate: None,
        }
    }

    /// Configures a custom CA certificate for registry downloads.
    #[cfg(feature = "egress")]
    pub fn certificate(mut self, certificate: Option<PathBuf>) -> Self {
        self.certificate = certificate;
        self
    }

    /// Reads one exact validated tree and identifies its filesystem root
    /// when the bytes came from or were written to one.
    pub fn read(
        &self,
        spec: &PackageSpec,
    ) -> Result<ReadPackage, FilesystemPackageAuthorityReadError> {
        if let Some(read) = self.read_from(&self.data, spec)? {
            return Ok(read);
        }
        if let Some(read) = self.read_from(&self.cache, spec)? {
            return Ok(read);
        }
        if self.offline {
            return Err(FilesystemPackageAuthorityReadError::Unavailable(not_found(
                spec,
            )));
        }

        #[cfg(feature = "egress")]
        {
            self.read_from_registry(spec)
        }
        #[cfg(not(feature = "egress"))]
        {
            Err(FilesystemPackageAuthorityReadError::Unavailable(not_found(
                spec,
            )))
        }
    }

    fn read_from(
        &self,
        packages: &Option<FsPackages>,
        spec: &PackageSpec,
    ) -> Result<Option<ReadPackage>, FilesystemPackageAuthorityReadError> {
        let Some(root) = packages.as_ref().and_then(|packages| packages.obtain(spec)) else {
            return Ok(None);
        };
        let tree = read_filesystem_package(root.path(), self.source_limits).map_err(|source| {
            let failure = other_failure(spec, source.to_string());
            FilesystemPackageAuthorityReadError::Filesystem {
                failure,
                source: Box::new(source),
            }
        })?;
        Ok(Some(ReadPackage {
            tree,
            root: Some(root.path().to_owned()),
        }))
    }

    #[cfg(feature = "egress")]
    fn read_from_registry(
        &self,
        spec: &PackageSpec,
    ) -> Result<ReadPackage, FilesystemPackageAuthorityReadError> {
        let url = package_archive_url(spec).map_err(|source| {
            FilesystemPackageAuthorityReadError::RegistryUrl {
                failure: not_found(spec),
                source: Box::new(source),
            }
        })?;
        let downloader = RustlsDownloader::new(USER_AGENT, self.certificate.clone());
        use typst_kit::downloader::Downloader;
        let (known_size, reader) = downloader.stream(spec, &url).map_err(|source| {
            // Do not turn a bounded archive request into an unprofiled package
            // index download merely to refine NotFound into VersionNotFound.
            let failure = if source.kind() == std::io::ErrorKind::NotFound {
                not_found(spec)
            } else {
                PackageReadFailure::new(
                    spec.clone(),
                    PackageReadFailureReason::NetworkFailed {
                        detail: Some(source.to_string()),
                    },
                )
            };
            FilesystemPackageAuthorityReadError::Download { failure, source }
        })?;
        let known_size = known_size
            .map(u64::try_from)
            .transpose()
            .map_err(|source| FilesystemPackageAuthorityReadError::DownloadSize {
                failure: other_failure(spec, "download size is not representable"),
                source,
            })?;
        let tree = read_registry_tree(spec, reader, known_size, self.expansion_limits)?;

        let root = if let Some(cache) = &self.cache {
            cache
                .store(spec, |directory| write_tree(directory, &tree))
                .map_err(|source| FilesystemPackageAuthorityReadError::Cache {
                    failure: other_failure(spec, source.to_string()),
                    source: Box::new(source),
                })?;
            Some(package_root(cache.path(), spec))
        } else {
            None
        };
        Ok(ReadPackage { tree, root })
    }
}

/// One successful read from the concrete filesystem Package Authority.
#[derive(Debug)]
pub struct ReadPackage {
    tree: PackageTree,
    root: Option<PathBuf>,
}

impl ReadPackage {
    /// The validated Package Tree produced by this read.
    pub fn tree(&self) -> &PackageTree {
        &self.tree
    }

    /// The package's filesystem root, when one backs dependency reporting.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Separates the validated tree from optional filesystem source evidence.
    pub fn into_parts(self) -> (PackageTree, Option<PathBuf>) {
        (self.tree, self.root)
    }
}

fn not_found(spec: &PackageSpec) -> PackageReadFailure {
    PackageReadFailure::new(spec.clone(), PackageReadFailureReason::NotFound)
}

fn other_failure(spec: &PackageSpec, detail: impl Into<String>) -> PackageReadFailure {
    PackageReadFailure::new(
        spec.clone(),
        PackageReadFailureReason::Other {
            detail: Some(detail.into()),
        },
    )
}

#[cfg(feature = "egress")]
fn read_registry_tree(
    spec: &PackageSpec,
    reader: impl Read,
    known_size: Option<u64>,
    limits: PackageExpansionLimits,
) -> Result<PackageTree, FilesystemPackageAuthorityReadError> {
    let archive = read_package_archive(reader, known_size, limits).map_err(|source| {
        let failure = match &source {
            PackageArchiveReadError::Read(error) => PackageReadFailure::new(
                spec.clone(),
                PackageReadFailureReason::NetworkFailed {
                    detail: Some(error.to_string()),
                },
            ),
            PackageArchiveReadError::Limit(error) => other_failure(spec, error.to_string()),
        };
        FilesystemPackageAuthorityReadError::ArchiveRead {
            failure,
            source: Box::new(source),
        }
    })?;
    expand_package_archive(spec.clone(), &archive, limits).map_err(|source| {
        let failure = match &source {
            PackageReadError::UnservedNamespace { .. } => not_found(spec),
            PackageReadError::ExpansionLimit { .. } => other_failure(spec, source.to_string()),
            PackageReadError::MalformedArchive { .. }
            | PackageReadError::InvalidPackageTree { .. } => PackageReadFailure::new(
                spec.clone(),
                PackageReadFailureReason::MalformedArchive {
                    detail: Some(source.to_string()),
                },
            ),
        };
        FilesystemPackageAuthorityReadError::ArchiveExpansion {
            failure,
            source: Box::new(source),
        }
    })
}

#[cfg(feature = "egress")]
fn package_root(base: &Path, spec: &PackageSpec) -> PathBuf {
    base.join(crate::read_layout::package_tree_key(spec))
}

#[cfg(feature = "egress")]
fn write_tree(directory: &Path, tree: &PackageTree) -> typst::diag::PackageResult<()> {
    for (path, data) in tree.files() {
        let destination = directory.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(package_cache_error)?;
        }
        std::fs::write(destination, data).map_err(package_cache_error)?;
    }
    Ok(())
}

#[cfg(feature = "egress")]
fn package_cache_error(error: std::io::Error) -> PackageError {
    PackageError::Other(Some(
        format!("failed to cache downloaded package: {error}").into(),
    ))
}

/// Exact Package Trees retained for representative-compile diagnostics.
pub(crate) struct ReadPackages {
    trees: Mutex<Vec<(PackageSpec, PackageTree)>>,
}

impl ReadPackages {
    pub(crate) fn new() -> Self {
        Self {
            trees: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn record(&self, spec: PackageSpec, tree: PackageTree) {
        self.trees
            .lock()
            .expect("read package lock poisoned")
            .push((spec, tree));
    }

    /// The exact bytes read for one package file, which is all creation
    /// diagnostics and timing spans may still resolve a package source from.
    pub(crate) fn file(&self, spec: &PackageSpec, path: &str) -> Option<Bytes> {
        self.trees
            .lock()
            .expect("read package lock poisoned")
            .iter()
            .find(|(candidate, _)| candidate == spec)?
            .1
            .shared_file(path)
            .map(|data| data.to_typst())
    }
}

#[cfg(feature = "egress")]
struct RustlsDownloader {
    user_agent: &'static str,
    certificate: Option<PathBuf>,
    tls: OnceLock<Result<Option<Arc<ureq::rustls::ClientConfig>>, String>>,
}

#[cfg(feature = "egress")]
impl RustlsDownloader {
    fn new(user_agent: &'static str, certificate: Option<PathBuf>) -> Self {
        Self {
            user_agent,
            certificate,
            tls: OnceLock::new(),
        }
    }

    fn tls_config(&self) -> std::io::Result<Option<Arc<ureq::rustls::ClientConfig>>> {
        match self.tls.get_or_init(|| {
            let Some(path) = &self.certificate else {
                return Ok(None);
            };
            let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
            let mut reader = BufReader::new(file);
            let mut roots = ureq::rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            for certificate in rustls_pemfile::certs(&mut reader) {
                let certificate = certificate.map_err(|error| error.to_string())?;
                roots.add(certificate).map_err(|error| error.to_string())?;
            }
            let tls = ureq::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(Some(Arc::new(tls)))
        }) {
            Ok(tls) => Ok(tls.clone()),
            Err(error) => Err(std::io::Error::other(error.clone())),
        }
    }
}

#[cfg(feature = "egress")]
impl typst_kit::downloader::Downloader for RustlsDownloader {
    fn stream(
        &self,
        _key: &dyn std::any::Any,
        url: &str,
    ) -> std::io::Result<(Option<usize>, Box<dyn Read>)> {
        #[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
        if let Some(output) = std::env::var_os(PACKAGE_DOWNLOAD_PROBE_ENV) {
            let certificate = self
                .certificate
                .as_deref()
                .map(|path| path.to_string_lossy())
                .unwrap_or_default();
            std::fs::write(output, certificate.as_bytes())?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "package download stopped by test probe",
            ));
        }

        let mut builder = ureq::AgentBuilder::new().user_agent(self.user_agent);
        if let Some(proxy) = env_proxy::for_url_str(url)
            .to_url()
            .and_then(|url| ureq::Proxy::new(url).ok())
        {
            builder = builder.proxy(proxy);
        }
        if let Some(tls) = self.tls_config()? {
            builder = builder.tls_config(tls);
        }
        let response = builder
            .build()
            .get(url)
            .call()
            .map_err(|error| match error {
                ureq::Error::Status(404, _) => {
                    std::io::Error::new(std::io::ErrorKind::NotFound, error)
                }
                error => std::io::Error::other(error),
            })?;
        let content_length = response
            .header("Content-Length")
            .and_then(|value| value.parse().ok());
        Ok((content_length, response.into_reader()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "egress")]
    fn package_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for (path, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            builder.append_data(&mut header, path, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn package_tree_accounting_overflow_is_typed() {
        assert_eq!(
            checked_add(u64::MAX, 1, FilesystemPackageResource::PackageTreeBytes),
            Err(FilesystemPackageLimitError::AccountingOverflow {
                resource: FilesystemPackageResource::PackageTreeBytes,
            })
        );
    }

    #[test]
    fn incremental_package_file_read_stops_at_the_plus_one_byte() {
        let error =
            read_bounded_package_file(&b"12345-extra"[..], Path::new("package.typ"), 4, 0, 100)
                .unwrap_err();
        assert!(matches!(
            error,
            FilesystemPackageReadError::Limit {
                source,
                ..
            } if source == FilesystemPackageLimitError::exceeded(
                FilesystemPackageResource::SelectedFileBytes,
                4,
            )
        ));
    }

    #[test]
    fn incremental_package_tree_read_reports_accounting_overflow() {
        let error =
            read_bounded_package_file(&b"x"[..], Path::new("package.typ"), 1, u64::MAX, u64::MAX)
                .unwrap_err();
        assert!(matches!(
            error,
            FilesystemPackageReadError::Limit {
                source: FilesystemPackageLimitError::AccountingOverflow {
                    resource: FilesystemPackageResource::PackageTreeBytes,
                },
                ..
            }
        ));
    }

    #[cfg(feature = "egress")]
    #[test]
    fn registry_response_bytes_are_bounded_then_expanded_without_a_reread() {
        let spec = "@preview/example:1.0.0".parse().unwrap();
        let archive = package_archive(&[("lib.typ", b"exact registry bytes")]);

        let tree = read_registry_tree(
            &spec,
            std::io::Cursor::new(&archive),
            Some(archive.len() as u64),
            PackageExpansionLimits::reference_v1(),
        )
        .unwrap();

        assert_eq!(tree.file("lib.typ"), Some(&b"exact registry bytes"[..]));
    }

    #[cfg(feature = "egress")]
    #[test]
    fn registry_expansion_limits_retain_the_typed_cause_without_claiming_malformed_bytes() {
        let spec = "@preview/example:1.0.0".parse().unwrap();
        let archive = package_archive(&[("lib.typ", b"12345")]);
        let limits = PackageExpansionLimits::new(1024 * 1024, 10, 100, 4, 100);

        let error = read_registry_tree(
            &spec,
            std::io::Cursor::new(&archive),
            Some(archive.len() as u64),
            limits,
        )
        .unwrap_err();

        let FilesystemPackageAuthorityReadError::ArchiveExpansion { failure, source } = error
        else {
            panic!("expected a typed archive expansion cause");
        };
        assert!(matches!(
            failure.reason(),
            PackageReadFailureReason::Other { .. }
        ));
        assert!(matches!(
            source.as_ref(),
            PackageReadError::ExpansionLimit { .. }
        ));
    }
}
