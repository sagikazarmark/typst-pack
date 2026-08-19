//! Font Catalog gathering for the reference filesystem Font Authority.

use std::fs::File;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error_display::format_error_list;
#[cfg(feature = "embedded-fonts")]
use crate::font_catalog::typst_embedded_font_containers;
use crate::font_catalog::{
    FontCatalog, FontCatalogEntry, FontContainer, FontContainerError, FontDisposition,
};

/// A resource bounded during filesystem Font Catalog gathering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemFontResource {
    VisitedEntries,
    AcceptedContainers,
    ContainerBytes,
    TotalAcceptedBytes,
}

/// A supplied gathering ceiling that cannot support bounded accounting.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemFontLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: FilesystemFontResource,
        ceiling: u64,
    },
}

/// A filesystem font source exceeded a mandatory gathering ceiling.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemFontLimitError {
    #[error(
        "filesystem Font Catalog gathering {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: FilesystemFontResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("filesystem Font Catalog gathering {resource:?} accounting overflowed")]
    AccountingOverflow { resource: FilesystemFontResource },
}

/// Mandatory finite resource ceilings for filesystem Font Catalog gathering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FilesystemFontLimits {
    visited_entries: u64,
    accepted_containers: u64,
    container_bytes: u64,
    total_accepted_bytes: u64,
}

impl FilesystemFontLimits {
    /// Constructs validated mandatory finite gathering ceilings.
    pub fn new(
        visited_entries: u64,
        accepted_containers: u64,
        container_bytes: u64,
        total_accepted_bytes: u64,
    ) -> Result<Self, FilesystemFontLimitsError> {
        let ceilings = [
            (FilesystemFontResource::VisitedEntries, visited_entries),
            (
                FilesystemFontResource::AcceptedContainers,
                accepted_containers,
            ),
            (FilesystemFontResource::ContainerBytes, container_bytes),
            (
                FilesystemFontResource::TotalAcceptedBytes,
                total_accepted_bytes,
            ),
        ];
        if let Some((resource, ceiling)) = ceilings
            .into_iter()
            .find(|(_, ceiling)| *ceiling == u64::MAX)
        {
            return Err(FilesystemFontLimitsError::CannotProbe { resource, ceiling });
        }
        Ok(Self {
            visited_entries,
            accepted_containers,
            container_bytes,
            total_accepted_bytes,
        })
    }

    /// The first-party limits for filesystem font sources.
    pub const fn reference_v1() -> Self {
        Self {
            visited_entries: 100_000,
            accepted_containers: 16_384,
            container_bytes: 256 * 1024 * 1024,
            total_accepted_bytes: 2 * 1024 * 1024 * 1024,
        }
    }

    pub const fn visited_entries(&self) -> u64 {
        self.visited_entries
    }

    pub const fn accepted_containers(&self) -> u64 {
        self.accepted_containers
    }

    pub const fn container_bytes(&self) -> u64 {
        self.container_bytes
    }

    pub const fn total_accepted_bytes(&self) -> u64 {
        self.total_accepted_bytes
    }
}

/// One explicitly configured source of Font Containers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemFontSource {
    kind: FilesystemFontSourceKind,
    disposition: FontDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FilesystemFontSourceKind {
    System,
    #[cfg(feature = "embedded-fonts")]
    TypstEmbedded,
    Directory(PathBuf),
}

impl FilesystemFontSource {
    /// Selects the host's standard system font directories at this position.
    pub fn system(disposition: FontDisposition) -> Self {
        Self {
            kind: FilesystemFontSourceKind::System,
            disposition,
        }
    }

    /// Selects Typst's compiled-in Font Containers at this position.
    #[cfg(feature = "embedded-fonts")]
    pub fn typst_embedded(disposition: FontDisposition) -> Self {
        Self {
            kind: FilesystemFontSourceKind::TypstEmbedded,
            disposition,
        }
    }

    /// Selects every eligible Font Container beneath one directory.
    pub fn directory(path: impl Into<PathBuf>, disposition: FontDisposition) -> Self {
        Self {
            kind: FilesystemFontSourceKind::Directory(path.into()),
            disposition,
        }
    }

    /// The disposition every catalog position from this source carries.
    pub fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// The kind of an eligible filesystem entry that cannot become a Font Container.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemFontEntryKind {
    Socket,
    Fifo,
    BlockDevice,
    CharacterDevice,
    Unknown,
}

/// The filesystem operation that failed while gathering a Font Catalog.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FilesystemFontOperation {
    InspectRoot,
    SurveyEntry,
    InspectContainer,
    ReadContainer,
}

impl std::fmt::Display for FilesystemFontOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InspectRoot => "inspect font source root",
            Self::SurveyEntry => "survey font source entry",
            Self::InspectContainer => "inspect selected Font Container",
            Self::ReadContainer => "read selected Font Container",
        })
    }
}

/// One independently detectable filesystem font survey issue.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemFontIssue {
    #[error("unsupported filesystem entry {path:?}: aliases cannot become Font Containers")]
    Alias { path: PathBuf },
    #[error("unsupported eligible font entry {path:?}")]
    UnsupportedEntry {
        path: PathBuf,
        kind: FilesystemFontEntryKind,
    },
    #[error("filesystem font source root {path:?} is not a directory")]
    RootNotDirectory { path: PathBuf },
}

impl FilesystemFontIssue {
    fn path(&self) -> &Path {
        match self {
            Self::Alias { path }
            | Self::UnsupportedEntry { path, .. }
            | Self::RootNotDirectory { path } => path,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Alias { .. } => 0,
            Self::UnsupportedEntry { .. } => 1,
            Self::RootNotDirectory { .. } => 2,
        }
    }
}

/// All safely detectable issues found by one filesystem font survey.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error(
    "filesystem font survey found {} issue(s){}",
    .issues.len(),
    format_error_list(.issues.as_slice())
)]
pub struct FilesystemFontSurveyError {
    issues: Vec<FilesystemFontIssue>,
}

impl FilesystemFontSurveyError {
    pub fn issues(&self) -> &[FilesystemFontIssue] {
        &self.issues
    }
}

/// One selected filesystem entry whose bytes are not a valid Font Container.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("invalid Font Container at {path:?}: {source}")]
pub struct FilesystemFontContainerIssue {
    path: PathBuf,
    source: FontContainerError,
}

impl FilesystemFontContainerIssue {
    /// The selected filesystem path whose exact bytes failed validation.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The authoritative Font Container validation failure.
    pub fn source(&self) -> FontContainerError {
        self.source
    }
}

/// Every invalid Font Container found while validating selected entries.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error(
    "filesystem font validation found {} invalid container(s){}",
    .issues.len(),
    format_error_list(.issues.as_slice())
)]
pub struct FilesystemFontValidationError {
    issues: Vec<FilesystemFontContainerIssue>,
}

impl FilesystemFontValidationError {
    pub fn issues(&self) -> &[FilesystemFontContainerIssue] {
        &self.issues
    }
}

/// A failure while gathering a Font Catalog from configured filesystem sources.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemFontGatherError {
    #[error("failed to {operation} {path:?}: {source}")]
    Io {
        operation: FilesystemFontOperation,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Survey(FilesystemFontSurveyError),
    #[error("filesystem font resource limit at {path:?}: {source}")]
    Limit {
        path: PathBuf,
        source: FilesystemFontLimitError,
    },
    #[error(transparent)]
    InvalidContainers(FilesystemFontValidationError),
}

impl FilesystemFontGatherError {
    fn io(
        operation: FilesystemFontOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    fn limit(path: &Path, source: FilesystemFontLimitError) -> Self {
        Self::Limit {
            path: path.to_owned(),
            source,
        }
    }
}

struct SelectedFont {
    root: PathBuf,
    boundary: PathBuf,
    path: PathBuf,
}

enum PlannedSource {
    Filesystem {
        selected: Vec<SelectedFont>,
        disposition: FontDisposition,
    },
    #[cfg(feature = "embedded-fonts")]
    TypstEmbedded {
        containers: Vec<FontContainer>,
        disposition: FontDisposition,
    },
}

#[derive(Default)]
struct SurveyState {
    visited_entries: u64,
    accepted_containers: u64,
    declared_total: u64,
    issues: Vec<FilesystemFontIssue>,
    deferred_limit: Option<FilesystemFontGatherError>,
}

/// Gathers one ordered Font Catalog from explicitly configured sources.
///
/// Sources compose in iterator order. Paths within one scanned root compose in
/// lexical order, and no system or embedded source is added implicitly.
pub fn gather_filesystem_font_catalog(
    sources: impl IntoIterator<Item = FilesystemFontSource>,
    limits: FilesystemFontLimits,
) -> Result<FontCatalog, FilesystemFontGatherError> {
    let mut state = SurveyState::default();
    let mut plans = Vec::new();

    for source in sources {
        match source.kind {
            FilesystemFontSourceKind::System => {
                let selected = survey_system_fonts(limits, &mut state)?;
                plans.push(PlannedSource::Filesystem {
                    selected,
                    disposition: source.disposition,
                });
            }
            #[cfg(feature = "embedded-fonts")]
            FilesystemFontSourceKind::TypstEmbedded => {
                let mut containers = Vec::new();
                for container in typst_embedded_font_containers() {
                    containers.push(container);
                }
                plans.push(PlannedSource::TypstEmbedded {
                    containers,
                    disposition: source.disposition,
                });
            }
            FilesystemFontSourceKind::Directory(root) => {
                let selected = survey_root(&root, true, limits, &mut state)?;
                plans.push(PlannedSource::Filesystem {
                    selected,
                    disposition: source.disposition,
                });
            }
        }
    }

    if !state.issues.is_empty() {
        state.issues.sort_by(|left, right| {
            left.path()
                .cmp(right.path())
                .then_with(|| left.rank().cmp(&right.rank()))
        });
        return Err(FilesystemFontGatherError::Survey(
            FilesystemFontSurveyError {
                issues: state.issues,
            },
        ));
    }
    if let Some(error) = state.deferred_limit {
        return Err(error);
    }

    let mut catalog = FontCatalog::new();
    let mut actual_total = 0u64;
    let mut invalid_containers = Vec::new();
    for plan in plans {
        match plan {
            PlannedSource::Filesystem {
                selected,
                disposition,
            } => {
                for selected in selected {
                    let bytes = read_bounded(
                        &selected.root,
                        &selected.boundary,
                        &selected.path,
                        actual_total,
                        limits,
                    )?;
                    actual_total = checked_add(
                        actual_total,
                        bytes.len() as u64,
                        FilesystemFontResource::TotalAcceptedBytes,
                    )
                    .map_err(|source| FilesystemFontGatherError::limit(&selected.path, source))?;
                    match FontContainer::new(bytes) {
                        Ok(container) => {
                            catalog.push(FontCatalogEntry::new(container, disposition));
                        }
                        Err(source) => invalid_containers.push(FilesystemFontContainerIssue {
                            path: selected.path,
                            source,
                        }),
                    }
                }
            }
            #[cfg(feature = "embedded-fonts")]
            PlannedSource::TypstEmbedded {
                containers,
                disposition,
            } => {
                for container in containers {
                    catalog.push(FontCatalogEntry::new(container, disposition));
                }
            }
        }
    }
    if !invalid_containers.is_empty() {
        invalid_containers.sort_by(|left, right| left.path.cmp(&right.path));
        return Err(FilesystemFontGatherError::InvalidContainers(
            FilesystemFontValidationError {
                issues: invalid_containers,
            },
        ));
    }
    Ok(catalog)
}

fn survey_system_fonts(
    limits: FilesystemFontLimits,
    state: &mut SurveyState,
) -> Result<Vec<SelectedFont>, FilesystemFontGatherError> {
    let mut selected = Vec::new();
    for root in system_font_roots() {
        selected.extend(survey_root(&root, false, limits, state)?);
    }

    #[cfg(target_os = "macos")]
    {
        selected.extend(survey_macos_downloadable_fonts(limits, state)?);
        for root in macos_system_font_roots_after_downloadable() {
            selected.extend(survey_root(&root, false, limits, state)?);
        }
    }

    Ok(selected)
}

#[cfg(target_os = "macos")]
fn survey_macos_downloadable_fonts(
    limits: FilesystemFontLimits,
    state: &mut SurveyState,
) -> Result<Vec<SelectedFont>, FilesystemFontGatherError> {
    let root = Path::new("/System/Library/AssetsV2");
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(FilesystemFontGatherError::io(
                FilesystemFontOperation::SurveyEntry,
                root,
                error,
            ));
        }
    };
    let mut roots = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            FilesystemFontGatherError::io(FilesystemFontOperation::SurveyEntry, root, error)
        })?;
        state.visited_entries = checked_add(
            state.visited_entries,
            1,
            FilesystemFontResource::VisitedEntries,
        )
        .map_err(|source| FilesystemFontGatherError::limit(&entry.path(), source))?;
        check_limit(
            FilesystemFontResource::VisitedEntries,
            limits.visited_entries,
            state.visited_entries,
        )
        .map_err(|source| FilesystemFontGatherError::limit(&entry.path(), source))?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("com_apple_MobileAsset_Font")
        {
            roots.push(entry.path());
        }
    }
    roots.sort();

    let mut selected = Vec::new();
    for root in roots {
        selected.extend(survey_root(&root, false, limits, state)?);
    }
    Ok(selected)
}

fn survey_root(
    root: &Path,
    required: bool,
    limits: FilesystemFontLimits,
    state: &mut SurveyState,
) -> Result<Vec<SelectedFont>, FilesystemFontGatherError> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => {
            return Err(FilesystemFontGatherError::io(
                FilesystemFontOperation::InspectRoot,
                root,
                error,
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        state.issues.push(FilesystemFontIssue::Alias {
            path: root.to_owned(),
        });
        return Ok(Vec::new());
    }
    if !metadata.is_dir() {
        state.issues.push(FilesystemFontIssue::RootNotDirectory {
            path: root.to_owned(),
        });
        return Ok(Vec::new());
    }
    let boundary = std::fs::canonicalize(root).map_err(|source| {
        FilesystemFontGatherError::io(FilesystemFontOperation::InspectRoot, root, source)
    })?;

    let mut selected = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| {
            let path = error.path().unwrap_or(root).to_owned();
            let source = error
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("filesystem traversal failed"));
            FilesystemFontGatherError::io(FilesystemFontOperation::SurveyEntry, path, source)
        })?;
        if entry.depth() == 0 {
            continue;
        }

        state.visited_entries = checked_add(
            state.visited_entries,
            1,
            FilesystemFontResource::VisitedEntries,
        )
        .map_err(|source| FilesystemFontGatherError::limit(entry.path(), source))?;
        check_limit(
            FilesystemFontResource::VisitedEntries,
            limits.visited_entries,
            state.visited_entries,
        )
        .map_err(|source| FilesystemFontGatherError::limit(entry.path(), source))?;

        let file_type = entry.file_type();
        if file_type.is_dir() {
            continue;
        }
        if file_type.is_symlink() {
            state.issues.push(FilesystemFontIssue::Alias {
                path: entry.path().to_owned(),
            });
            continue;
        }
        if !font_eligible(entry.path()) {
            continue;
        }
        if !file_type.is_file() {
            state.issues.push(FilesystemFontIssue::UnsupportedEntry {
                path: entry.path().to_owned(),
                kind: unsupported_kind(&file_type),
            });
            continue;
        }

        let metadata = entry.metadata().map_err(|error| {
            let path = error.path().unwrap_or(entry.path()).to_owned();
            let source = error
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("failed to inspect Font Container"));
            FilesystemFontGatherError::io(FilesystemFontOperation::InspectContainer, path, source)
        })?;
        let accepted = account_container(entry.path(), metadata.len(), limits, state)?;
        if accepted {
            selected.push(SelectedFont {
                root: root.to_owned(),
                boundary: boundary.clone(),
                path: entry.path().to_owned(),
            });
        }
    }
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(selected)
}

fn account_container(
    path: &Path,
    declared: u64,
    limits: FilesystemFontLimits,
    state: &mut SurveyState,
) -> Result<bool, FilesystemFontGatherError> {
    state.accepted_containers = checked_add(
        state.accepted_containers,
        1,
        FilesystemFontResource::AcceptedContainers,
    )
    .map_err(|source| FilesystemFontGatherError::limit(path, source))?;
    if let Err(source) = check_limit(
        FilesystemFontResource::AcceptedContainers,
        limits.accepted_containers,
        state.accepted_containers,
    ) {
        if state.deferred_limit.is_none() {
            state.deferred_limit = Some(FilesystemFontGatherError::limit(path, source));
        }
        return Ok(false);
    }
    if let Err(source) = check_limit(
        FilesystemFontResource::ContainerBytes,
        limits.container_bytes,
        declared,
    ) {
        if state.deferred_limit.is_none() {
            state.deferred_limit = Some(FilesystemFontGatherError::limit(path, source));
        }
        return Ok(false);
    }
    state.declared_total = checked_add(
        state.declared_total,
        declared,
        FilesystemFontResource::TotalAcceptedBytes,
    )
    .map_err(|source| FilesystemFontGatherError::limit(path, source))?;
    if let Err(source) = check_limit(
        FilesystemFontResource::TotalAcceptedBytes,
        limits.total_accepted_bytes,
        state.declared_total,
    ) {
        if state.deferred_limit.is_none() {
            state.deferred_limit = Some(FilesystemFontGatherError::limit(path, source));
        }
        return Ok(false);
    }
    Ok(true)
}

fn read_bounded(
    root: &Path,
    boundary: &Path,
    path: &Path,
    total_before: u64,
    limits: FilesystemFontLimits,
) -> Result<Vec<u8>, FilesystemFontGatherError> {
    let total_allowance = limits.total_accepted_bytes.saturating_sub(total_before);
    let allowance = limits.container_bytes.min(total_allowance);
    let mut file = open_without_following(root, boundary, path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(allowance + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            FilesystemFontGatherError::io(FilesystemFontOperation::ReadContainer, path, error)
        })?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        FilesystemFontGatherError::limit(
            path,
            FilesystemFontLimitError::AccountingOverflow {
                resource: FilesystemFontResource::ContainerBytes,
            },
        )
    })?;
    check_limit(
        FilesystemFontResource::ContainerBytes,
        limits.container_bytes,
        observed,
    )
    .map_err(|source| FilesystemFontGatherError::limit(path, source))?;
    let total = checked_add(
        total_before,
        observed,
        FilesystemFontResource::TotalAcceptedBytes,
    )
    .map_err(|source| FilesystemFontGatherError::limit(path, source))?;
    check_limit(
        FilesystemFontResource::TotalAcceptedBytes,
        limits.total_accepted_bytes,
        total,
    )
    .map_err(|source| FilesystemFontGatherError::limit(path, source))?;
    Ok(bytes)
}

#[cfg(unix)]
fn open_without_following(
    root: &Path,
    _boundary: &Path,
    path: &Path,
) -> Result<File, FilesystemFontGatherError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let relative = path
        .strip_prefix(root)
        .expect("a selected font path remains beneath its root");
    let mut components = relative.components().peekable();
    let mut current = root.to_owned();
    let root_path =
        CString::new(root.as_os_str().as_bytes()).expect("filesystem paths contain no NUL bytes");
    // SAFETY: the NUL-terminated path remains valid for the call, and a
    // successful descriptor is immediately owned.
    let root_descriptor = unsafe {
        libc::open(
            root_path.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | libc::O_NOFOLLOW
                | libc::O_DIRECTORY,
        )
    };
    if root_descriptor < 0 {
        if std::fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(alias_error(root));
        }
        return Err(FilesystemFontGatherError::io(
            FilesystemFontOperation::ReadContainer,
            root,
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: `open` returned a new owned descriptor.
    let mut directory = unsafe { File::from_raw_fd(root_descriptor) };
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
            return Err(FilesystemFontGatherError::io(
                FilesystemFontOperation::ReadContainer,
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
    unreachable!("a selected font file has a path beneath the root")
}

#[cfg(not(unix))]
fn open_without_following(
    root: &Path,
    boundary: &Path,
    path: &Path,
) -> Result<File, FilesystemFontGatherError> {
    #[cfg(not(windows))]
    let _ = boundary;
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
            return Err(FilesystemFontGatherError::io(
                FilesystemFontOperation::ReadContainer,
                path,
                error,
            ));
        }
    };
    if let Some(alias) = first_alias(root, path) {
        return Err(alias_error(&alias));
    }
    let file = validate_opened_file(file, path)?;
    #[cfg(windows)]
    validate_windows_boundary(&file, boundary, path)?;
    Ok(file)
}

#[cfg(windows)]
fn validate_windows_boundary(
    file: &File,
    boundary: &Path,
    path: &Path,
) -> Result<(), FilesystemFontGatherError> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;

    let handle = file.as_raw_handle();
    // SAFETY: `handle` remains owned by `file`; a null output buffer requests
    // the required UTF-16 length and writes no bytes.
    let required = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, 0) };
    if required == 0 {
        return Err(FilesystemFontGatherError::io(
            FilesystemFontOperation::ReadContainer,
            path,
            std::io::Error::last_os_error(),
        ));
    }
    let mut buffer = vec![0u16; required as usize + 1];
    // SAFETY: `buffer` has the size reported by the first call and `handle`
    // remains valid for the duration of this call.
    let written =
        unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
    if written == 0 || written as usize >= buffer.len() {
        return Err(FilesystemFontGatherError::io(
            FilesystemFontOperation::ReadContainer,
            path,
            std::io::Error::last_os_error(),
        ));
    }
    let opened = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..written as usize]));
    if !opened.starts_with(boundary) {
        return Err(alias_error(path));
    }
    Ok(())
}

fn validate_opened_file(file: File, path: &Path) -> Result<File, FilesystemFontGatherError> {
    let metadata = file.metadata().map_err(|error| {
        FilesystemFontGatherError::io(FilesystemFontOperation::ReadContainer, path, error)
    })?;
    if metadata.file_type().is_symlink() {
        return Err(alias_error(path));
    }
    if !metadata.file_type().is_file() {
        return Err(FilesystemFontGatherError::Survey(
            FilesystemFontSurveyError {
                issues: vec![FilesystemFontIssue::UnsupportedEntry {
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
        .expect("a selected font path remains beneath its root");
    let mut current = root.to_owned();
    if std::fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Some(current);
    }
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

fn alias_error(path: &Path) -> FilesystemFontGatherError {
    FilesystemFontGatherError::Survey(FilesystemFontSurveyError {
        issues: vec![FilesystemFontIssue::Alias {
            path: path.to_owned(),
        }],
    })
}

fn font_eligible(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(crate::acquisition_layout::is_font_container_extension)
}

fn unsupported_kind(file_type: &std::fs::FileType) -> FilesystemFontEntryKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        if file_type.is_socket() {
            return FilesystemFontEntryKind::Socket;
        }
        if file_type.is_fifo() {
            return FilesystemFontEntryKind::Fifo;
        }
        if file_type.is_block_device() {
            return FilesystemFontEntryKind::BlockDevice;
        }
        if file_type.is_char_device() {
            return FilesystemFontEntryKind::CharacterDevice;
        }
    }
    FilesystemFontEntryKind::Unknown
}

fn checked_add(
    total: u64,
    value: u64,
    resource: FilesystemFontResource,
) -> Result<u64, FilesystemFontLimitError> {
    total
        .checked_add(value)
        .ok_or(FilesystemFontLimitError::AccountingOverflow { resource })
}

fn check_limit(
    resource: FilesystemFontResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), FilesystemFontLimitError> {
    if observed > ceiling {
        return Err(FilesystemFontLimitError::Exceeded {
            resource,
            ceiling,
            observed_at_least: observed,
        });
    }
    Ok(())
}

fn system_font_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let system_root = std::env::var_os("SYSTEMROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        roots.push(system_root.join("Fonts"));
        if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            roots.push(home.join(r"AppData\Local\Microsoft\Windows\Fonts"));
            roots.push(home.join(r"AppData\Roaming\Microsoft\Windows\Fonts"));
        }
        if let Some(data) = std::env::var_os("APPDATA").map(PathBuf::from) {
            roots.push(data.join("Adobe/CoreSync/plugins/livetype/r"));
            roots.push(data.join("Adobe/User Owned Fonts"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        roots.extend([
            PathBuf::from("/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts"),
        ]);
    }

    #[cfg(target_os = "redox")]
    roots.push(PathBuf::from("/ui/fonts"));

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    roots.extend(fontconfig_roots());

    roots
}

#[cfg(target_os = "macos")]
fn macos_system_font_roots_after_downloadable() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Network/Library/Fonts")];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join("Library/Fonts"));
        let data = home.join("Library/Application Support/Adobe");
        roots.push(data.join("CoreSync/plugins/livetype/.r"));
        roots.push(data.join(".User Owned Fonts"));
    }
    roots
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn fontconfig_roots() -> Vec<PathBuf> {
    let mut config = fontconfig_parser::FontConfig::default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(config_file) = std::env::var_os("FONTCONFIG_FILE") {
        let _ = config.merge_config(Path::new(&config_file));
    } else {
        let user_config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".config")));
        let read_global = user_config.is_none_or(|path| {
            config
                .merge_config(&path.join("fontconfig/fonts.conf"))
                .is_err()
        });
        if read_global {
            let _ = config.merge_config(Path::new("/etc/fonts/local.conf"));
        }
        let _ = config.merge_config(Path::new("/etc/fonts/fonts.conf"));
    }

    let mut roots = config
        .dirs
        .into_iter()
        .filter_map(|directory| {
            if directory.path.starts_with("~") {
                home.as_ref()
                    .map(|home| home.join(directory.path.strip_prefix("~").unwrap()))
            } else {
                Some(directory.path)
            }
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.extend([
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ]);
        if let Some(home) = home {
            roots.push(home.join(".fonts"));
            roots.push(home.join(".local/share/fonts"));
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_container_accounting_overflow_is_typed() {
        assert_eq!(
            checked_add(u64::MAX, 1, FilesystemFontResource::AcceptedContainers),
            Err(FilesystemFontLimitError::AccountingOverflow {
                resource: FilesystemFontResource::AcceptedContainers,
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reads_do_not_follow_an_alias_created_after_survey() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("outside.ttf");
        let selected = directory.path().join("selected.ttf");
        let boundary = directory.path().canonicalize().unwrap();
        std::fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &selected).unwrap();

        let error = read_bounded(
            directory.path(),
            &boundary,
            &selected,
            0,
            FilesystemFontLimits::new(1, 1, 16, 16).unwrap(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FilesystemFontGatherError::Survey(ref survey)
                if matches!(survey.issues(), [FilesystemFontIssue::Alias { path }] if path == &selected)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reads_do_not_follow_a_root_alias_created_after_survey() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("fonts");
        let outside = directory.path().join("outside");
        let selected = root.join("selected.ttf");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(&selected, b"surveyed").unwrap();
        std::fs::write(outside.join("selected.ttf"), b"outside").unwrap();
        let boundary = root.canonicalize().unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        symlink(&outside, &root).unwrap();

        let error = read_bounded(
            &root,
            &boundary,
            &selected,
            0,
            FilesystemFontLimits::new(1, 1, 16, 16).unwrap(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            FilesystemFontGatherError::Survey(ref survey)
                if matches!(survey.issues(), [FilesystemFontIssue::Alias { path }] if path == &root)
        ));
    }
}
