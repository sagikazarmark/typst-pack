//! The one Typst [`World`] behind creation and compilation, and the
//! [`Sources`] a run leaves behind for presenting its diagnostics.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::FontStore;

use crate::compile::{DocumentTime, RequestIssue};
use crate::package::{ReadFailure, Tree};
use crate::project::ProjectSnapshot;

/// A world over bytes the caller already holds: one project, the package
/// trees available to it, and an ordered list of font containers.
pub(crate) struct SourceWorld {
    library: LazyHash<Library>,
    main: FileId,
    files: FileStore<Loader>,
    fonts: FontStore,
    /// The font container position of each face, by book index.
    containers: Vec<usize>,
    /// The book indices of the faces the run loaded.
    used_faces: Mutex<BTreeSet<usize>>,
    clock: Clock,
}

/// Where a world's file requests are served from, and nothing else.
/// Cloning a loader is cheap.
#[derive(Clone)]
pub(crate) struct Loader {
    pub(crate) project: ProjectSnapshot,
    /// Package trees keyed by specification text.
    pub(crate) packages: Arc<BTreeMap<String, Tree>>,
    /// Packages the caller declared it cannot supply.
    pub(crate) failures: Arc<HashMap<PackageSpec, ReadFailure>>,
}

/// What a world answers `datetime.today()` with.
pub(crate) enum Clock {
    Absent,
    /// A fixed date, whatever offset is requested.
    Date(Datetime),
    /// A fixed instant whose date respects requested UTC offsets, and is
    /// read at `utc_offset_seconds` (UTC when `None`) without one.
    Timestamp {
        seconds: i64,
        utc_offset_seconds: Option<i32>,
        /// The instant, for requested offsets.
        instant: Time,
        /// The instant shifted by the default offset, read in UTC.
        shifted: Time,
    },
}

impl Clock {
    /// The clock for a document time, or the reason it has none.
    pub(crate) fn new(document_time: DocumentTime) -> Result<Self, RequestIssue> {
        let (seconds, utc_offset_seconds) = match document_time {
            DocumentTime::Absent => return Ok(Self::Absent),
            // As Typst's fixed clock: the date of a datetime, never a time.
            DocumentTime::Fixed(datetime) => {
                let (Some(year), Some(month), Some(day)) =
                    (datetime.year(), datetime.month(), datetime.day())
                else {
                    return Err(RequestIssue::UndatedDocumentTime);
                };
                let date = Datetime::from_ymd(year, month, day);
                return date
                    .map(Self::Date)
                    .ok_or(RequestIssue::UndatedDocumentTime);
            }
            DocumentTime::UnixTimestamp(seconds) => (seconds, None),
            DocumentTime::Local {
                seconds,
                utc_offset_seconds,
            } => (seconds, Some(utc_offset_seconds)),
        };
        let offset = utc_offset_seconds.unwrap_or(0);
        if offset.unsigned_abs() >= 86_400 {
            return Err(RequestIssue::InvalidUtcOffset);
        }
        let time = |seconds: Option<i64>| {
            seconds
                .and_then(|seconds| Time::fixed_timestamp(seconds).ok())
                .ok_or(RequestIssue::InvalidDocumentTimestamp)
        };
        Ok(Self::Timestamp {
            seconds,
            utc_offset_seconds,
            instant: time(Some(seconds))?,
            shifted: time(seconds.checked_add(i64::from(offset)))?,
        })
    }
}

impl SourceWorld {
    /// A world whose fonts are the faces of `containers`, in order.
    pub(crate) fn new(
        loader: Loader,
        containers: impl IntoIterator<Item = Vec<Font>>,
        library: Library,
        clock: Clock,
    ) -> Self {
        let entrypoint = VirtualPath::new(loader.project.entrypoint())
            .expect("a validated project entrypoint is a virtual path");
        let mut fonts = FontStore::new();
        let mut positions = Vec::new();
        for (position, faces) in containers.into_iter().enumerate() {
            for font in faces {
                positions.push(position);
                let info = font.info().clone();
                fonts.push((font, info));
            }
        }
        Self {
            library: LazyHash::new(library),
            main: RootedPath::new(VirtualRoot::Project, entrypoint).intern(),
            files: FileStore::new(loader),
            fonts,
            containers: positions,
            used_faces: Mutex::new(BTreeSet::new()),
            clock,
        }
    }

    /// The positions of the font containers whose faces the run loaded.
    pub(crate) fn used_containers(&self) -> BTreeSet<usize> {
        self.used_faces
            .lock()
            .expect("used font lock poisoned")
            .iter()
            .filter_map(|index| self.containers.get(*index).copied())
            .collect()
    }

    /// Every file the run requested, whether or not it could be served.
    pub(crate) fn requested_files(&mut self) -> Vec<FileId> {
        let (_, files) = self.files.dependencies();
        let mut files = files.collect::<Vec<_>>();
        files.sort_by_cached_key(|id| file_key(*id));
        files
    }

    /// Retains the world for presenting the run's diagnostics.
    pub(crate) fn into_sources(mut self) -> Sources {
        let files = self.requested_files();
        Sources {
            world: Arc::new(self),
            files: files.into(),
            root: None,
        }
    }
}

fn file_key(id: FileId) -> (String, String) {
    let package = match id.root() {
        VirtualRoot::Project => String::new(),
        VirtualRoot::Package(spec) => spec.to_string(),
    };
    (package, id.vpath().get_without_slash().to_owned())
}

impl World for SourceWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        let font = self.fonts.font(index)?;
        self.used_faces
            .lock()
            .expect("used font lock poisoned")
            .insert(index);
        Some(font)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        match &self.clock {
            Clock::Absent => None,
            // A plain date carries no instant for an offset to shift.
            Clock::Date(date) => Some(*date),
            Clock::Timestamp { instant, .. } if offset.is_some() => instant.today(offset),
            Clock::Timestamp { shifted, .. } => shifted.today(None),
        }
    }
}

impl FileLoader for Loader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let _timing = typst_timing::TimingScope::new("load file");
        let path = id.vpath().get_without_slash();
        let data = match id.root() {
            VirtualRoot::Project => self.project.shared_file(path),
            VirtualRoot::Package(spec) => {
                let Some(tree) = self.packages.get(&spec.to_string()) else {
                    return Err(FileError::Package(match self.failures.get(spec) {
                        Some(failure) => package_error(spec, failure),
                        None => PackageError::NotFound(spec.clone()),
                    }));
                };
                tree.shared_file(path)
            }
        };
        data.map(|data| data.to_typst())
            .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
    }
}

fn package_error(spec: &PackageSpec, failure: &ReadFailure) -> PackageError {
    match failure {
        ReadFailure::NotFound => PackageError::NotFound(spec.clone()),
        ReadFailure::VersionNotFound { latest } => {
            PackageError::VersionNotFound(spec.clone(), *latest)
        }
        ReadFailure::NetworkFailed(detail) => {
            PackageError::NetworkFailed(detail.clone().map(Into::into))
        }
        ReadFailure::MalformedArchive(detail) => {
            PackageError::MalformedArchive(detail.clone().map(Into::into))
        }
        ReadFailure::Other(detail) => PackageError::Other(detail.clone().map(Into::into)),
    }
}

/// The sources one creation or compilation run read, retained so its
/// diagnostics can be presented with source context.
///
/// `Sources` is a Typst [`World`] over exactly the bytes the run used, so
/// spans in its diagnostics resolve against it, for example with
/// [`WorldExt::range`](typst::WorldExt::range) or, with the `diagnostics`
/// feature, `typst_kit::diagnostics::emit`. Cloning is cheap.
#[derive(Clone)]
pub struct Sources {
    world: Arc<SourceWorld>,
    files: Arc<[FileId]>,
    root: Option<Arc<Path>>,
}

impl Sources {
    /// Every file the run requested, including ones it could not read, in
    /// project-then-package path order.
    pub fn files(&self) -> &[FileId] {
        &self.files
    }

    /// The name diagnostics use for a file.
    ///
    /// Package files are named `@namespace/name:version/path`. Project files
    /// are named by their root-relative path, or, for a project read from the
    /// filesystem, by their path relative to the current directory.
    pub fn name(&self, id: FileId) -> String {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Package(spec) => format!("{spec}/{path}"),
            VirtualRoot::Project => self
                .root
                .as_deref()
                .and_then(|root| {
                    let current = std::env::current_dir().ok()?;
                    let current = current.canonicalize().unwrap_or(current);
                    relative_path(&id.vpath().realize(root).ok()?, &current)
                })
                .map_or_else(
                    || path.to_owned(),
                    |path| path.to_string_lossy().into_owned(),
                ),
        }
    }

    /// Names project files by their location beneath `root`, the directory
    /// the project was read from, relative to the current directory as
    /// Typst's CLI does. The sources of `fs::Packer` already do.
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into().into());
        self
    }
}

impl fmt::Debug for Sources {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sources")
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl World for Sources {
    fn library(&self) -> &LazyHash<Library> {
        self.world.library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.world.book()
    }

    fn main(&self) -> FileId {
        self.world.main()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.world.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.world.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.world.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.world.today(offset)
    }
}

#[cfg(feature = "diagnostics")]
impl typst_kit::diagnostics::DiagnosticWorld for Sources {
    fn name(&self, id: FileId) -> String {
        Sources::name(self, id)
    }
}

/// `path` relative to `base`, as Typst's CLI names files in diagnostics.
fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
    if path.is_absolute() != base.is_absolute() {
        return path.is_absolute().then(|| path.to_path_buf());
    }
    let mut path_components = path.components();
    let mut base_components = base.components();
    let mut relative = Vec::new();
    loop {
        match (path_components.next(), base_components.next()) {
            (None, None) => break,
            (Some(component), None) => {
                relative.push(component);
                relative.extend(path_components.by_ref());
                break;
            }
            (None, Some(_)) => relative.push(Component::ParentDir),
            (Some(path), Some(base)) if relative.is_empty() && path == base => {}
            (Some(path), Some(Component::CurDir)) => relative.push(path),
            (Some(_), Some(Component::ParentDir)) => return None,
            (Some(Component::Prefix(_) | Component::RootDir), Some(_))
            | (Some(_), Some(Component::Prefix(_) | Component::RootDir)) => {
                return path.is_absolute().then(|| path.to_path_buf());
            }
            (Some(path), Some(_)) => {
                relative.push(Component::ParentDir);
                relative.extend(base_components.map(|_| Component::ParentDir));
                relative.push(path);
                relative.extend(path_components.by_ref());
                break;
            }
        }
    }
    Some(relative.iter().map(|part| part.as_os_str()).collect())
}
