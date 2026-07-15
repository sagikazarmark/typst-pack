//! A complete Typst [`World`] backed by a [`Pack`].

use std::path::PathBuf;
use std::sync::Arc;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualRoot};
use typst::text::FontInfo;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{FontSource, FontStore};

use crate::pack::Pack;

/// A complete Typst [`World`] backed by a [`Pack`].
///
/// Project files and vendored package files come from the pack. Fonts come
/// from the pack, plus any fonts configured on the builder. Files of packages
/// that are not vendored are only available if a package loader is configured.
/// Declared External Project Resources may come from external resource loaders;
/// these loaders cannot replace packed files or supply Typst source or package
/// files.
pub struct PackWorld {
    library: LazyHash<Library>,
    main: FileId,
    store: FileStore<PackLoader>,
    fonts: FontStore,
    clock: Clock,
}

impl PackWorld {
    /// Starts configuring a world for the given pack.
    pub fn builder(pack: Pack) -> PackWorldBuilder {
        PackWorldBuilder::new(pack)
    }

    /// Creates a world with default configuration.
    pub fn new(pack: Pack) -> Result<Self, PackWorldError> {
        Self::builder(pack).build()
    }

    /// The pack this world serves resources from.
    pub fn pack(&self) -> &Pack {
        self.store.loader().pack.as_ref()
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
        if matches!(id.root(), VirtualRoot::Project)
            && self
                .store
                .loader()
                .pack
                .file(id.vpath().get_without_slash())
                .is_none()
        {
            return Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            )));
        }
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
            Clock::Fixed(datetime) => Some(*datetime),
            #[cfg(feature = "fs")]
            Clock::System(time) => time.today(offset),
        }
    }
}

/// Where the world takes the current date from.
enum Clock {
    /// `datetime.today()` errors in document code.
    None,
    /// A fixed date, for reproducible output.
    Fixed(Datetime),
    /// The system clock.
    #[cfg(feature = "fs")]
    System(typst_kit::datetime::Time),
}

/// Serves file requests from a pack, with an optional fallback for packages
/// that are not vendored.
struct PackLoader {
    pack: Arc<Pack>,
    external_resource_loaders: Vec<Box<dyn FileLoader + Send + Sync>>,
    package_loader: Option<Box<dyn FileLoader + Send + Sync>>,
}

pub(crate) fn load_external_resource(
    loaders: &[Box<dyn FileLoader + Send + Sync>],
    id: FileId,
) -> FileResult<Bytes> {
    for loader in loaders {
        match loader.load(id) {
            Err(FileError::NotFound(_)) => {}
            result => return result,
        }
    }
    Err(FileError::NotFound(PathBuf::from(
        id.vpath().get_without_slash(),
    )))
}

impl FileLoader for PackLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => {
                if let Some(data) = self.pack.file(path) {
                    return Ok(data.clone());
                }
                if !self.pack.is_external_resource(path) {
                    return Err(FileError::NotFound(PathBuf::from(path)));
                }
                load_external_resource(&self.external_resource_loaders, id)
            }
            VirtualRoot::Package(spec) => {
                if self.pack.has_package(spec) {
                    self.pack
                        .package_file(spec, path)
                        .cloned()
                        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
                } else if let Some(loader) = &self.package_loader {
                    loader.load(id)
                } else {
                    Err(FileError::Other(Some(
                        format!(
                            "package {spec} is not vendored in the pack \
                             and no package loader is configured"
                        )
                        .into(),
                    )))
                }
            }
        }
    }
}

/// Configures a [`PackWorld`].
pub struct PackWorldBuilder {
    pack: Pack,
    inputs: Dict,
    features: Vec<Feature>,
    clock: Clock,
    #[cfg_attr(not(feature = "embedded-fonts"), allow(dead_code))]
    embedded_fonts: bool,
    extra_fonts: Vec<(BoxedFontSource, FontInfo)>,
    external_resource_loaders: Vec<Box<dyn FileLoader + Send + Sync>>,
    package_loader: Option<Box<dyn FileLoader + Send + Sync>>,
}

/// Adapter that lets heterogeneous font sources live in one list.
struct BoxedFontSource(Box<dyn FontSource>);

impl FontSource for BoxedFontSource {
    fn load(&self) -> Option<Font> {
        self.0.load()
    }
}

impl PackWorldBuilder {
    fn new(pack: Pack) -> Self {
        Self {
            pack,
            inputs: Dict::new(),
            features: Vec::new(),
            clock: Clock::None,
            embedded_fonts: cfg!(feature = "embedded-fonts"),
            extra_fonts: Vec::new(),
            external_resource_loaders: Vec::new(),
            package_loader: None,
        }
    }

    /// Values made available to document code as `sys.inputs`.
    pub fn inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self
    }

    /// Enables an experimental Typst language feature.
    ///
    /// [`Feature::Html`](typst::Feature::Html) is required for compiling to
    /// [`OutputFormat::Html`](crate::OutputFormat::Html).
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Uses a fixed date for `datetime.today()`, for reproducible output.
    pub fn fixed_date(mut self, datetime: Datetime) -> Self {
        self.clock = Clock::Fixed(datetime);
        self
    }

    /// Uses the system clock for `datetime.today()`.
    #[cfg(feature = "fs")]
    pub fn system_date(mut self) -> Self {
        self.clock = Clock::System(typst_kit::datetime::Time::system());
        self
    }

    /// Whether to include Typst's default embedded fonts. Defaults to `true`
    /// when the `embedded-fonts` feature is enabled.
    #[cfg(feature = "embedded-fonts")]
    pub fn embedded_fonts(mut self, include: bool) -> Self {
        self.embedded_fonts = include;
        self
    }

    /// Adds fonts on top of the ones embedded in the pack.
    ///
    /// These rank behind pack fonts but before Typst's embedded fonts, so use
    /// this for system fonts or other host-provided fonts. Accepts the same
    /// `(source, info)` entries yielded by the `typst_kit::fonts` providers,
    /// so fonts are only loaded into memory when actually used.
    pub fn extra_fonts<T: FontSource>(
        mut self,
        fonts: impl IntoIterator<Item = (T, FontInfo)>,
    ) -> Self {
        self.extra_fonts.extend(
            fonts
                .into_iter()
                .map(|(source, info)| (BoxedFontSource(Box::new(source)), info)),
        );
        self
    }

    /// Serves files of packages that are not vendored in the pack, e.g. from
    /// a package cache or the network.
    pub fn package_loader(mut self, loader: impl FileLoader + Send + Sync + 'static) -> Self {
        self.package_loader = Some(Box::new(loader));
        self
    }

    /// Adds a loader for declared External Project Resources.
    ///
    /// Loaders are tried in registration order after packed project files.
    pub fn external_resource_loader(
        mut self,
        loader: impl FileLoader + Send + Sync + 'static,
    ) -> Self {
        self.external_resource_loaders.push(Box::new(loader));
        self
    }

    /// Builds the world.
    pub fn build(self) -> Result<PackWorld, PackWorldError> {
        let entrypoint = self
            .pack
            .manifest()
            .entrypoint()
            .map_err(|err| PackWorldError::InvalidPack(err.to_string()))?;
        let main = RootedPath::new(VirtualRoot::Project, entrypoint).intern();

        let mut fonts = FontStore::new();
        for pack_font in self.pack.fonts() {
            let font = Font::new(pack_font.data.clone(), pack_font.entry.index)
                .ok_or_else(|| PackWorldError::InvalidFont(pack_font.entry.path.clone()))?;
            let info = font.info().clone();
            fonts.push((font, info));
        }
        fonts.extend(self.extra_fonts);
        #[cfg(feature = "embedded-fonts")]
        if self.embedded_fonts {
            fonts.extend(typst_kit::fonts::embedded());
        }

        let library = Library::builder()
            .with_inputs(self.inputs)
            .with_features(self.features.into_iter().collect())
            .build();

        Ok(PackWorld {
            library: LazyHash::new(library),
            main,
            store: FileStore::new(PackLoader {
                pack: Arc::new(self.pack),
                external_resource_loaders: self.external_resource_loaders,
                package_loader: self.package_loader,
            }),
            fonts,
            clock: self.clock,
        })
    }
}

/// A [`FileLoader`] that resolves package files from standard system
/// locations (and Typst Universe), for compiling packs whose dependencies are
/// not vendored. Project file requests always fail: those must come from the
/// pack.
#[cfg(feature = "fs")]
pub struct SystemPackageLoader(pub typst_kit::packages::SystemPackages);

#[cfg(feature = "fs")]
impl SystemPackageLoader {
    /// Creates a loader using the standard package directories and the
    /// official Typst Universe registry.
    pub fn system() -> Self {
        Self(typst_kit::packages::SystemPackages::new(
            typst_kit::downloader::SystemDownloader::new(concat!(
                "typst-pack/",
                env!("CARGO_PKG_VERSION")
            )),
        ))
    }

    /// Creates a loader that only uses the standard local package
    /// directories and never accesses the network.
    pub fn offline() -> Self {
        Self(typst_kit::packages::SystemPackages::new(OfflineDownloader))
    }
}

/// A package downloader that refuses to download.
///
/// Plug this into [`typst_kit::packages::UniversePackages`] to guarantee
/// that package resolution never accesses the network: every download
/// attempt fails as not found, so only local directories (or the pack
/// itself) can satisfy dependencies.
#[cfg(feature = "fs")]
pub struct OfflineDownloader;

#[cfg(feature = "fs")]
impl typst_kit::downloader::Downloader for OfflineDownloader {
    fn stream(
        &self,
        _key: &dyn std::any::Any,
        _url: &str,
    ) -> std::io::Result<(Option<usize>, Box<dyn std::io::Read>)> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "network access is disabled (offline mode)",
        ))
    }
}

#[cfg(feature = "fs")]
impl FileLoader for SystemPackageLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            ))),
            VirtualRoot::Package(spec) => Ok(self.0.obtain(spec)?.load(id.vpath())?),
        }
    }
}

/// A failure while building a [`PackWorld`].
#[derive(Debug, thiserror::Error)]
pub enum PackWorldError {
    #[error("pack is not usable: {0}")]
    InvalidPack(String),
    #[error("embedded font `{0}` could not be loaded")]
    InvalidFont(String),
}
