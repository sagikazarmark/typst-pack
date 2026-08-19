//! A complete Typst [`World`] backed by a [`Pack`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::FontStore;

use crate::domain::DocumentTime;
use crate::pack::{CompilationDependencySnapshot, Pack, PackageFiles};

/// A complete Typst [`World`] backed by a [`Pack`].
///
/// Project files and embedded package files come from the Pack. Externally
/// fulfilled package files and exact fonts are available only through a
/// crate-verified Compilation Dependency Snapshot. Pack Overrides remain
/// separate exact request inputs.
pub(crate) struct PackWorld {
    library: LazyHash<Library>,
    main: FileId,
    store: FileStore<PackLoader>,
    fonts: FontStore,
    clock: Clock,
}

impl PackWorld {
    /// Constructs a world from one complete Pack-bound input set.
    pub(crate) fn new(
        pack: Pack,
        dependencies: CompilationDependencySnapshot,
        project_overrides: BTreeMap<String, Bytes>,
        inputs: Dict,
        features: Vec<Feature>,
        document_time: DocumentTime,
    ) -> Result<Self, PackWorldConstructionError> {
        if dependencies.pack_identity() != pack.identity() {
            return Err(PackWorldConstructionError::DependencySnapshotPackMismatch);
        }
        if let Some(path) = project_overrides
            .keys()
            .find(|path| pack.file(path).is_none())
        {
            return Err(PackWorldConstructionError::InvalidProjectOverride { path: path.clone() });
        }

        let clock = match document_time {
            DocumentTime::Absent => Clock::None,
            DocumentTime::Fixed(datetime) => Clock::FixedDate(datetime),
            DocumentTime::UnixTimestamp(timestamp) => Clock::FixedTimestamp(
                typst_kit::datetime::Time::fixed_timestamp(timestamp)
                    .map_err(|_| PackWorldConstructionError::InvalidDocumentTimestamp)?,
            ),
        };
        let (exact_packages, font_catalog) = dependencies.into_parts();
        let entrypoint = typst::syntax::VirtualPath::new(pack.entrypoint())
            .expect("Pack entrypoint invariant violated");
        let main = RootedPath::new(VirtualRoot::Project, entrypoint).intern();

        let mut fonts = FontStore::new();
        for font in font_catalog {
            let info = font.info().clone();
            fonts.push((font, info));
        }

        let library = Library::builder()
            .with_inputs(inputs)
            .with_features(features.into_iter().collect())
            .build();

        Ok(Self {
            library: LazyHash::new(library),
            main,
            store: FileStore::new(PackLoader {
                pack: Arc::new(pack),
                project_overrides,
                exact_packages,
            }),
            fonts,
            clock,
        })
    }
}

impl World for PackWorld {
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
        self.store.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.store.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, #[allow(unused_variables)] offset: Option<Duration>) -> Option<Datetime> {
        match &self.clock {
            Clock::None => None,
            // A fixed date is used as-is; the offset only matters relative to
            // an instant, which a plain date does not carry.
            Clock::FixedDate(datetime) => Some(*datetime),
            Clock::FixedTimestamp(time) => time.today(offset),
        }
    }
}

#[cfg(feature = "diagnostics")]
impl typst_kit::diagnostics::DiagnosticWorld for PackWorld {
    fn name(&self, id: FileId) -> String {
        match id.root() {
            VirtualRoot::Project => id.vpath().get_without_slash().to_owned(),
            VirtualRoot::Package(spec) => format!("{spec}{}", id.vpath().get_with_slash()),
        }
    }
}

impl PackWorld {
    #[cfg(feature = "diagnostics")]
    pub(crate) fn file_dependencies(&mut self) -> Vec<FileId> {
        let (_, dependencies) = self.store.dependencies();
        dependencies.collect()
    }
}

/// Where the world takes the current date from.
enum Clock {
    /// `datetime.today()` errors in document code.
    None,
    /// A fixed date, for reproducible output.
    FixedDate(Datetime),
    /// A fixed timestamp whose date respects requested timezone offsets.
    FixedTimestamp(typst_kit::datetime::Time),
}

/// Serves file requests only from a Pack and verified dependency snapshots.
struct PackLoader {
    pack: Arc<Pack>,
    project_overrides: BTreeMap<String, Bytes>,
    exact_packages: BTreeMap<String, PackageFiles>,
}

/// Pack World construction accepts only a complete Pack-bound input set.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PackWorldConstructionError {
    #[error("the Compilation Dependency Snapshot is bound to a different Pack")]
    DependencySnapshotPackMismatch,
    #[error("Pack Override path `{path}` is not a contained project file")]
    InvalidProjectOverride { path: String },
    #[error("the document-time UNIX timestamp is out of range")]
    InvalidDocumentTimestamp,
}

impl FileLoader for PackLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let _timing = typst_timing::TimingScope::new("Pack");
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => self
                .project_overrides
                .get(path)
                .cloned()
                .or_else(|| self.pack.shared_file(path).map(|data| data.to_typst()))
                .ok_or_else(|| FileError::NotFound(PathBuf::from(path))),
            VirtualRoot::Package(spec) => {
                if self.pack.has_package(spec) {
                    self.pack
                        .shared_package_file(spec, path)
                        .map(|data| data.to_typst())
                        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
                } else if let Some(package) = self.exact_packages.get(&spec.to_string()) {
                    package
                        .file(path)
                        .map(|data| data.to_typst())
                        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
                } else {
                    Err(FileError::Other(Some(
                        format!("package {spec} has no verified Package Tree").into(),
                    )))
                }
            }
        }
    }
}
