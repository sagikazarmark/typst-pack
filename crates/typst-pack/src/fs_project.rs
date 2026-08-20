//! Project reading for the reference filesystem source.

use std::fs::File;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::error_display::format_error_list;
use crate::limits::{LimitError, Limits, LimitsError, ResourceKind};
use crate::pack::names_pack_path;
use crate::project_snapshot::{ProjectSnapshot, ProjectSnapshotAssembly, ProjectSnapshotError};

/// The root-relative path of the filesystem Project Ignore Policy file.
pub const IGNORE_FILE: &str = ".typkignore";

/// A resource bounded during filesystem project reading.
pub type FilesystemProjectResource = ResourceKind<0>;

#[allow(non_upper_case_globals)]
impl ResourceKind<0> {
    pub const VisitedEntries: Self = Self::new(0);
    pub const SelectedFiles: Self = Self::new(1);
    pub const RootPolicyBytes: Self = Self::new(2);
    pub const SelectedFileBytes: Self = Self::new(3);
    pub const TotalSelectedBytes: Self = Self::new(4);
}

pub type FilesystemProjectLimitsError = LimitsError<FilesystemProjectResource>;

/// A filesystem project exceeded a mandatory reading ceiling.
pub type FilesystemProjectLimitError = LimitError<FilesystemProjectResource>;

/// Mandatory finite resource ceilings for filesystem project reading.
pub type FilesystemProjectLimits = Limits<FilesystemProjectResource>;

impl Limits<FilesystemProjectResource> {
    /// Constructs validated mandatory finite reading ceilings.
    pub fn new(
        visited_entries: u64,
        selected_files: u64,
        root_policy_bytes: u64,
        selected_file_bytes: u64,
        total_selected_bytes: u64,
    ) -> Result<Self, FilesystemProjectLimitsError> {
        Self::from_ceilings([
            visited_entries,
            selected_files,
            root_policy_bytes,
            selected_file_bytes,
            total_selected_bytes,
            0,
            0,
        ])
        .validate_probe_resources([
            FilesystemProjectResource::VisitedEntries,
            FilesystemProjectResource::SelectedFiles,
            FilesystemProjectResource::RootPolicyBytes,
            FilesystemProjectResource::SelectedFileBytes,
            FilesystemProjectResource::TotalSelectedBytes,
        ])
    }

    /// The first-party limits for filesystem projects.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            1_000_000,
            100_000,
            1024 * 1024,
            256 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
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

    pub const fn root_policy_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    pub const fn selected_file_bytes(&self) -> u64 {
        self.ceilings[3]
    }

    pub const fn total_selected_bytes(&self) -> u64 {
        self.ceilings[4]
    }
}

/// The kind of an eligible filesystem entry that cannot become a project file.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemProjectEntryKind {
    Socket,
    Fifo,
    BlockDevice,
    CharacterDevice,
    Unknown,
}

/// The filesystem operation that produced an I/O error while reading.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemProjectOperation {
    InspectRootPolicy,
    ReadRootPolicy,
    SurveyEntry,
    InspectSelectedFile,
    ReadSelectedFile,
}

impl std::fmt::Display for FilesystemProjectOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InspectRootPolicy => "inspect root Project Ignore Policy",
            Self::ReadRootPolicy => "read root Project Ignore Policy",
            Self::SurveyEntry => "survey project entry",
            Self::InspectSelectedFile => "inspect selected project file",
            Self::ReadSelectedFile => "read selected project file",
        })
    }
}

/// One independently detectable filesystem project survey issue.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemProjectIssue {
    #[error("unsupported filesystem entry `{}`: aliases cannot become project files", path.display())]
    Alias { path: PathBuf },
    #[error("unsupported filesystem entry `{}` in the project", path.display())]
    UnsupportedEntry {
        path: PathBuf,
        kind: FilesystemProjectEntryKind,
    },
    #[error("project path `{}` is not valid UTF-8", path.display())]
    UnrepresentablePath { path: PathBuf },
}

impl FilesystemProjectIssue {
    fn path(&self) -> &Path {
        match self {
            Self::Alias { path }
            | Self::UnsupportedEntry { path, .. }
            | Self::UnrepresentablePath { path } => path,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Alias { .. } => 0,
            Self::UnsupportedEntry { .. } => 1,
            Self::UnrepresentablePath { .. } => 2,
        }
    }
}

/// All safely detectable issues found by one filesystem structural survey.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error(
    "filesystem project survey found {} issue(s){}",
    .issues.len(),
    format_error_list(.issues.as_slice())
)]
pub struct FilesystemProjectSurveyError {
    issues: Vec<FilesystemProjectIssue>,
}

impl FilesystemProjectSurveyError {
    pub fn issues(&self) -> &[FilesystemProjectIssue] {
        &self.issues
    }
}

/// A failure while parsing the root filesystem Project Ignore Policy.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemProjectPolicyError {
    #[error("the policy file is not valid UTF-8")]
    NotUtf8,
    #[error("line {line}: {message}")]
    InvalidRule { line: usize, message: String },
    #[error("{0}")]
    Invalid(String),
}

/// A failure while reading a Project Snapshot from the filesystem.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemProjectReadError {
    #[error("failed to {operation} `{}`: {source}", path.display())]
    Io {
        operation: FilesystemProjectOperation,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid Project Ignore Policy at `{}`: {source}", path.display())]
    InvalidPolicy {
        path: PathBuf,
        source: FilesystemProjectPolicyError,
    },
    #[error(transparent)]
    Survey(FilesystemProjectSurveyError),
    #[error("filesystem project resource limit at {path:?}: {source}")]
    Limit {
        path: PathBuf,
        source: FilesystemProjectLimitError,
    },
    #[error("selected filesystem entries do not form a Project Snapshot: {0}")]
    Snapshot(#[source] ProjectSnapshotError),
}

impl FilesystemProjectReadError {
    fn io(
        operation: FilesystemProjectOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    fn limit(path: impl Into<PathBuf>, source: FilesystemProjectLimitError) -> Self {
        Self::Limit {
            path: path.into(),
            source,
        }
    }
}

/// Reads one Project Snapshot from the reference filesystem source.
///
/// The root policy is read and parsed once. Reading then surveys and
/// filters the complete eligible structure before reading ordinary selected
/// files, and submits only the selected exact bytes to Project Snapshot
/// Assembly.
pub fn read_filesystem_project(
    root: impl AsRef<Path>,
    entrypoint: impl Into<String>,
    limits: FilesystemProjectLimits,
) -> Result<ProjectSnapshot, FilesystemProjectReadError> {
    let root = root.as_ref();
    let policy_path = root.join(IGNORE_FILE);
    let policy_file = read_policy(&policy_path, limits)?;
    let policy = &policy_file.policy;

    let mut visited_entries = 0u64;
    let mut selected_files = u64::from(policy_file.bytes.is_some());
    check_limit(
        FilesystemProjectResource::SelectedFiles,
        limits.selected_files(),
        selected_files,
    )
    .map_err(|source| FilesystemProjectReadError::limit(&policy_path, source))?;
    let mut declared_total = policy_file
        .bytes
        .as_ref()
        .map_or(0, |bytes| bytes.len() as u64);
    let mut selected = Vec::new();
    let mut issues = Vec::new();
    let mut deferred_limit = None;
    let mut walk = walkdir::WalkDir::new(root).follow_links(false).into_iter();

    while let Some(entry) = walk.next() {
        let entry = entry.map_err(|error| {
            let path = error.path().unwrap_or(root).to_owned();
            let source = error
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("filesystem traversal failed"));
            FilesystemProjectReadError::io(FilesystemProjectOperation::SurveyEntry, path, source)
        })?;
        if entry.depth() == 0 {
            continue;
        }

        visited_entries = checked_add(
            visited_entries,
            1,
            FilesystemProjectResource::VisitedEntries,
        )
        .map_err(|source| FilesystemProjectReadError::limit(entry.path(), source))?;
        check_limit(
            FilesystemProjectResource::VisitedEntries,
            limits.visited_entries(),
            visited_entries,
        )
        .map_err(|source| FilesystemProjectReadError::limit(entry.path(), source))?;

        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walk remains beneath root");
        let Some(path) = slash_path(relative) else {
            issues.push(FilesystemProjectIssue::UnrepresentablePath {
                path: entry.path().to_owned(),
            });
            if entry.file_type().is_dir() {
                walk.skip_current_dir();
            }
            continue;
        };

        if path == IGNORE_FILE {
            if entry.file_type().is_dir() {
                walk.skip_current_dir();
            }
            continue;
        }

        let file_type = entry.file_type();
        if file_type.is_dir() {
            if policy.excludes_directory(&path) {
                walk.skip_current_dir();
            }
            continue;
        }
        if policy.excludes_file(&path) {
            continue;
        }
        if file_type.is_symlink() {
            issues.push(FilesystemProjectIssue::Alias {
                path: entry.path().to_owned(),
            });
            continue;
        }
        if !file_type.is_file() {
            issues.push(FilesystemProjectIssue::UnsupportedEntry {
                path: entry.path().to_owned(),
                kind: unsupported_kind(&file_type),
            });
            continue;
        }

        selected_files =
            checked_add(selected_files, 1, FilesystemProjectResource::SelectedFiles)
                .map_err(|source| FilesystemProjectReadError::limit(entry.path(), source))?;
        if let Err(source) = check_limit(
            FilesystemProjectResource::SelectedFiles,
            limits.selected_files(),
            selected_files,
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemProjectReadError::limit(entry.path(), source));
            continue;
        }

        let metadata = entry.metadata().map_err(|error| {
            let path = error.path().unwrap_or(entry.path()).to_owned();
            let source = error.into_io_error().unwrap_or_else(|| {
                std::io::Error::other("failed to inspect selected project file")
            });
            FilesystemProjectReadError::io(
                FilesystemProjectOperation::InspectSelectedFile,
                path,
                source,
            )
        })?;
        let declared = metadata.len();
        if let Err(source) = check_limit(
            FilesystemProjectResource::SelectedFileBytes,
            limits.selected_file_bytes(),
            declared,
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemProjectReadError::limit(entry.path(), source));
            continue;
        }
        declared_total = checked_add(
            declared_total,
            declared,
            FilesystemProjectResource::TotalSelectedBytes,
        )
        .map_err(|source| FilesystemProjectReadError::limit(entry.path(), source))?;
        if let Err(source) = check_limit(
            FilesystemProjectResource::TotalSelectedBytes,
            limits.total_selected_bytes(),
            declared_total,
        ) {
            deferred_limit
                .get_or_insert_with(|| FilesystemProjectReadError::limit(entry.path(), source));
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
        return Err(FilesystemProjectReadError::Survey(
            FilesystemProjectSurveyError { issues },
        ));
    }
    if let Some(error) = deferred_limit {
        return Err(error);
    }

    selected.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut entries = Vec::with_capacity(selected.len() + usize::from(policy_file.bytes.is_some()));
    let mut actual_total = 0u64;
    if let Some(bytes) = policy_file.bytes {
        actual_total = bytes.len() as u64;
        entries.push((IGNORE_FILE.to_owned(), bytes));
    }
    for (path, source) in selected {
        let remaining = limits.total_selected_bytes() - actual_total;
        let bytes = read_bounded(
            root,
            &source,
            &[
                (
                    FilesystemProjectResource::SelectedFileBytes,
                    limits.selected_file_bytes(),
                    limits.selected_file_bytes(),
                    0,
                ),
                (
                    FilesystemProjectResource::TotalSelectedBytes,
                    remaining,
                    limits.total_selected_bytes(),
                    actual_total,
                ),
            ],
            FilesystemProjectOperation::ReadSelectedFile,
        )?;
        actual_total = checked_add(
            actual_total,
            bytes.len() as u64,
            FilesystemProjectResource::TotalSelectedBytes,
        )
        .map_err(|source_error| FilesystemProjectReadError::limit(&source, source_error))?;
        entries.push((path, bytes));
    }

    ProjectSnapshotAssembly::new(entrypoint)
        .assemble(entries)
        .map_err(FilesystemProjectReadError::Snapshot)
}

struct ReadPolicy {
    policy: ProjectIgnorePolicy,
    bytes: Option<Vec<u8>>,
}

fn read_policy(
    path: &Path,
    limits: FilesystemProjectLimits,
) -> Result<ReadPolicy, FilesystemProjectReadError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReadPolicy {
                policy: ProjectIgnorePolicy::built_in(),
                bytes: None,
            });
        }
        Err(error) => {
            return Err(FilesystemProjectReadError::io(
                FilesystemProjectOperation::InspectRootPolicy,
                path,
                error,
            ));
        }
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(FilesystemProjectReadError::Survey(
            FilesystemProjectSurveyError {
                issues: vec![FilesystemProjectIssue::Alias {
                    path: path.to_owned(),
                }],
            },
        ));
    }
    if !file_type.is_file() {
        return Err(FilesystemProjectReadError::Survey(
            FilesystemProjectSurveyError {
                issues: vec![FilesystemProjectIssue::UnsupportedEntry {
                    path: path.to_owned(),
                    kind: unsupported_kind(&file_type),
                }],
            },
        ));
    }

    check_limit(
        FilesystemProjectResource::VisitedEntries,
        limits.visited_entries(),
        1,
    )
    .and_then(|_| {
        check_limit(
            FilesystemProjectResource::SelectedFiles,
            limits.selected_files(),
            1,
        )
    })
    .map_err(|source| FilesystemProjectReadError::limit(path, source))?;
    check_limit(
        FilesystemProjectResource::RootPolicyBytes,
        limits.root_policy_bytes(),
        metadata.len(),
    )
    .and_then(|_| {
        check_limit(
            FilesystemProjectResource::SelectedFileBytes,
            limits.selected_file_bytes(),
            metadata.len(),
        )
    })
    .and_then(|_| {
        check_limit(
            FilesystemProjectResource::TotalSelectedBytes,
            limits.total_selected_bytes(),
            metadata.len(),
        )
    })
    .map_err(|source| FilesystemProjectReadError::limit(path, source))?;
    let bytes = read_bounded(
        path.parent().expect("the policy path is beneath a root"),
        path,
        &[
            (
                FilesystemProjectResource::RootPolicyBytes,
                limits.root_policy_bytes(),
                limits.root_policy_bytes(),
                0,
            ),
            (
                FilesystemProjectResource::SelectedFileBytes,
                limits.selected_file_bytes(),
                limits.selected_file_bytes(),
                0,
            ),
            (
                FilesystemProjectResource::TotalSelectedBytes,
                limits.total_selected_bytes(),
                limits.total_selected_bytes(),
                0,
            ),
        ],
        FilesystemProjectOperation::ReadRootPolicy,
    )?;
    let policy = ProjectIgnorePolicy::from_bytes(&bytes).map_err(|source| {
        FilesystemProjectReadError::InvalidPolicy {
            path: path.to_owned(),
            source,
        }
    })?;
    Ok(ReadPolicy {
        policy,
        bytes: Some(bytes),
    })
}

fn read_bounded(
    root: &Path,
    path: &Path,
    ceilings: &[(FilesystemProjectResource, u64, u64, u64)],
    operation: FilesystemProjectOperation,
) -> Result<Vec<u8>, FilesystemProjectReadError> {
    let probe_ceiling = ceilings
        .iter()
        .map(|(_, allowance, _, _)| *allowance)
        .min()
        .expect("a bounded read has at least one ceiling");
    let mut file = open_without_following(root, path, operation)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(probe_ceiling + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| FilesystemProjectReadError::io(operation, path, error))?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        FilesystemProjectReadError::limit(
            path,
            FilesystemProjectLimitError::AccountingOverflow {
                resource: ceilings[0].0,
            },
        )
    })?;
    for (resource, _, ceiling, base) in ceilings {
        let cumulative = checked_add(*base, observed, *resource)
            .map_err(|source| FilesystemProjectReadError::limit(path, source))?;
        check_limit(*resource, *ceiling, cumulative)
            .map_err(|source| FilesystemProjectReadError::limit(path, source))?;
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_without_following(
    root: &Path,
    path: &Path,
    operation: FilesystemProjectOperation,
) -> Result<File, FilesystemProjectReadError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = path
        .strip_prefix(root)
        .expect("a selected path remains beneath its project root");
    let mut components = relative.components().peekable();
    let mut current = root.to_owned();
    let mut directory =
        File::open(root).map_err(|error| FilesystemProjectReadError::io(operation, root, error))?;
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
            return Err(FilesystemProjectReadError::io(
                operation,
                &current,
                std::io::Error::last_os_error(),
            ));
        }
        // SAFETY: `openat` returned a new owned descriptor.
        let opened = unsafe { File::from_raw_fd(descriptor) };
        if final_component {
            return validate_opened_file(opened, &current, operation);
        }
        directory = opened;
    }
    unreachable!("a selected project file has a path beneath the root")
}

#[cfg(not(unix))]
fn open_without_following(
    root: &Path,
    path: &Path,
    operation: FilesystemProjectOperation,
) -> Result<File, FilesystemProjectReadError> {
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
            return Err(FilesystemProjectReadError::io(operation, path, error));
        }
    };
    if let Some(alias) = first_alias(root, path) {
        return Err(alias_error(&alias));
    }
    validate_opened_file(file, path, operation)
}

fn validate_opened_file(
    file: File,
    path: &Path,
    operation: FilesystemProjectOperation,
) -> Result<File, FilesystemProjectReadError> {
    let metadata = file
        .metadata()
        .map_err(|error| FilesystemProjectReadError::io(operation, path, error))?;
    if metadata.file_type().is_symlink() {
        return Err(alias_error(path));
    }
    if !metadata.file_type().is_file() {
        return Err(FilesystemProjectReadError::Survey(
            FilesystemProjectSurveyError {
                issues: vec![FilesystemProjectIssue::UnsupportedEntry {
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
        .expect("a selected path remains beneath its project root");
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

fn alias_error(path: &Path) -> FilesystemProjectReadError {
    FilesystemProjectReadError::Survey(FilesystemProjectSurveyError {
        issues: vec![FilesystemProjectIssue::Alias {
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

fn unsupported_kind(file_type: &std::fs::FileType) -> FilesystemProjectEntryKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        if file_type.is_socket() {
            return FilesystemProjectEntryKind::Socket;
        }
        if file_type.is_fifo() {
            return FilesystemProjectEntryKind::Fifo;
        }
        if file_type.is_block_device() {
            return FilesystemProjectEntryKind::BlockDevice;
        }
        if file_type.is_char_device() {
            return FilesystemProjectEntryKind::CharacterDevice;
        }
    }
    FilesystemProjectEntryKind::Unknown
}

fn checked_add(
    total: u64,
    value: u64,
    resource: FilesystemProjectResource,
) -> Result<u64, FilesystemProjectLimitError> {
    total
        .checked_add(value)
        .ok_or(FilesystemProjectLimitError::AccountingOverflow { resource })
}

fn check_limit(
    resource: FilesystemProjectResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), FilesystemProjectLimitError> {
    if observed > ceiling {
        return Err(FilesystemProjectLimitError::exceeded(resource, ceiling));
    }
    Ok(())
}

struct ProjectIgnorePolicy {
    rules: Gitignore,
}

impl ProjectIgnorePolicy {
    fn built_in() -> Self {
        Self {
            rules: Gitignore::empty(),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, FilesystemProjectPolicyError> {
        let contents =
            std::str::from_utf8(bytes).map_err(|_| FilesystemProjectPolicyError::NotUtf8)?;
        let mut builder = GitignoreBuilder::new(".");
        for (index, line) in contents.lines().enumerate() {
            let line = if index == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                line
            };
            builder.add_line(None, line).map_err(|error| {
                FilesystemProjectPolicyError::InvalidRule {
                    line: index + 1,
                    message: error.to_string(),
                }
            })?;
        }
        let rules = builder
            .build()
            .map_err(|error| FilesystemProjectPolicyError::Invalid(error.to_string()))?;
        Ok(Self { rules })
    }

    fn excludes_file(&self, path: &str) -> bool {
        self.excludes(path, false)
    }

    fn excludes_directory(&self, path: &str) -> bool {
        self.excludes(path, true)
    }

    fn excludes(&self, path: &str, is_directory: bool) -> bool {
        if path == IGNORE_FILE {
            return false;
        }
        if names_pack_path(path) {
            return true;
        }
        let mut ancestor_end = 0;
        while let Some(offset) = path[ancestor_end..].find('/') {
            ancestor_end += offset;
            if self.rules.matched(&path[..ancestor_end], true).is_ignore() {
                return true;
            }
            ancestor_end += 1;
        }
        self.rules.matched(path, is_directory).is_ignore()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn bounded_reads_do_not_follow_an_alias_created_after_survey() {
        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside");
        let selected = directory.path().join("selected");
        std::fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &selected).unwrap();

        let error = read_bounded(
            directory.path(),
            &selected,
            &[(FilesystemProjectResource::SelectedFileBytes, 16, 16, 0)],
            FilesystemProjectOperation::ReadSelectedFile,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FilesystemProjectReadError::Survey(ref survey)
                if matches!(survey.issues(), [FilesystemProjectIssue::Alias { path }] if path == &selected)
        ));
    }

    #[test]
    fn bounded_reads_do_not_follow_an_ancestor_alias_created_after_survey() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        let ancestor = root.join("nested");
        let selected = ancestor.join("selected");
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(&ancestor).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(&selected, b"surveyed").unwrap();
        std::fs::write(outside.join("selected"), b"outside").unwrap();
        std::fs::remove_dir_all(&ancestor).unwrap();
        symlink(&outside, &ancestor).unwrap();

        let error = read_bounded(
            &root,
            &selected,
            &[(FilesystemProjectResource::SelectedFileBytes, 16, 16, 0)],
            FilesystemProjectOperation::ReadSelectedFile,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FilesystemProjectReadError::Survey(ref survey)
                if matches!(survey.issues(), [FilesystemProjectIssue::Alias { path }] if path == &ancestor)
        ));
    }

    #[test]
    fn bounded_reads_do_not_block_on_a_fifo_created_after_survey() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let directory = tempfile::tempdir().unwrap();
        let selected = directory.path().join("selected");
        std::fs::write(&selected, b"surveyed").unwrap();
        std::fs::remove_file(&selected).unwrap();
        let path = CString::new(selected.as_os_str().as_bytes()).unwrap();
        // SAFETY: `path` is a valid NUL-terminated filesystem path.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

        let error = read_bounded(
            directory.path(),
            &selected,
            &[(FilesystemProjectResource::SelectedFileBytes, 16, 16, 0)],
            FilesystemProjectOperation::ReadSelectedFile,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FilesystemProjectReadError::Survey(ref survey)
                if matches!(survey.issues(), [FilesystemProjectIssue::UnsupportedEntry {
                    path,
                    kind: FilesystemProjectEntryKind::Fifo,
                }] if path == &selected)
        ));
    }
}
