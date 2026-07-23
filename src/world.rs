//! A complete Typst [`World`] backed by a [`Pack`].

use std::collections::BTreeMap;
#[cfg(feature = "fs")]
use std::io::{self, BufReader, Read};
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "fs")]
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualRoot};
use typst::text::FontInfo;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{FontSource, FontStore};

use crate::pack::{FontCatalogError, FontContainerIdentity, Pack, PackageFiles};
use crate::resource::{CompilationResources, Provider};

#[cfg(feature = "fs")]
const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

// Integration tests execute a separate non-`cfg(test)` binary.
#[cfg(all(
    feature = "fs",
    feature = "_test-package-download-probe",
    debug_assertions
))]
const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

#[cfg(feature = "fs")]
pub(crate) fn system_packages(
    package_path: Option<&std::path::Path>,
    package_cache_path: Option<&std::path::Path>,
    offline: bool,
    certificate: Option<&std::path::Path>,
) -> typst_kit::packages::SystemPackages {
    use typst_kit::packages::UniversePackages;

    system_packages_with_online(
        package_path,
        package_cache_path,
        offline,
        certificate,
        |certificate| {
            #[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
            if let Some(output) = std::env::var_os(PACKAGE_DOWNLOAD_PROBE_ENV) {
                return UniversePackages::new(PackageDownloadProbe {
                    certificate: certificate.map(PathBuf::from),
                    output: output.into(),
                });
            }

            let downloader = RustlsDownloader::new(USER_AGENT, certificate.map(PathBuf::from));
            UniversePackages::new(downloader)
        },
    )
}

#[cfg(feature = "fs")]
struct RustlsDownloader {
    user_agent: &'static str,
    certificate: Option<PathBuf>,
    tls: OnceLock<Result<Option<Arc<ureq::rustls::ClientConfig>>, String>>,
}

#[cfg(feature = "fs")]
impl RustlsDownloader {
    fn new(user_agent: &'static str, certificate: Option<PathBuf>) -> Self {
        Self {
            user_agent,
            certificate,
            tls: OnceLock::new(),
        }
    }

    fn tls_config(&self) -> io::Result<Option<Arc<ureq::rustls::ClientConfig>>> {
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
            Err(error) => Err(io::Error::other(error.clone())),
        }
    }
}

#[cfg(feature = "fs")]
impl typst_kit::downloader::Downloader for RustlsDownloader {
    fn stream(
        &self,
        _key: &dyn std::any::Any,
        url: &str,
    ) -> io::Result<(Option<usize>, Box<dyn Read>)> {
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
                ureq::Error::Status(404, _) => io::Error::new(io::ErrorKind::NotFound, error),
                error => io::Error::other(error),
            })?;
        let content_length = response
            .header("Content-Length")
            .and_then(|value| value.parse().ok());
        Ok((content_length, response.into_reader()))
    }
}

#[cfg(feature = "fs")]
fn system_packages_with_online(
    package_path: Option<&std::path::Path>,
    package_cache_path: Option<&std::path::Path>,
    offline: bool,
    certificate: Option<&std::path::Path>,
    online: impl FnOnce(Option<&std::path::Path>) -> typst_kit::packages::UniversePackages,
) -> typst_kit::packages::SystemPackages {
    use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

    let data = match package_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_data(),
    };
    let cache = match package_cache_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_cache(),
    };
    let universe = if offline {
        UniversePackages::new(OfflineDownloader)
    } else {
        online(certificate)
    };

    SystemPackages::from_parts(data, cache, universe)
}

#[cfg(feature = "fs")]
pub(crate) fn read_complete_package_tree(
    root: &std::path::Path,
) -> Result<Vec<(String, Bytes)>, String> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = typst::syntax::VirtualPath::virtualize(root, entry.path()).map_err(|_| {
            format!(
                "package file `{}` is outside its root",
                entry.path().display()
            )
        })?;
        let data = std::fs::read(entry.path()).map_err(|error| {
            format!(
                "failed to read package file `{}`: {error}",
                entry.path().display()
            )
        })?;
        files.push((path.get_without_slash().to_owned(), Bytes::new(data)));
    }
    Ok(files)
}

/// A complete Typst [`World`] backed by a [`Pack`].
///
/// Project files and embedded package files come from the Pack. Externally
/// fulfilled package files are available only through a crate-verified exact
/// dependency snapshot. Fonts come from the Pack plus any configured fonts.
/// Declared Resource Slots may come from Resource Providers; providers cannot
/// replace packed files or supply Typst source or package files.
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
    pub fn new(pack: Pack) -> Result<Self, PackWorldBuildError> {
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
        let loader = self.store.loader();
        loader
            .resources
            .source(&loader.pack, id, || self.store.source(id))
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
            #[cfg(feature = "fs")]
            Clock::FixedTimestamp(time) => time.today(offset),
            #[cfg(feature = "fs")]
            Clock::System(time) => time.today(offset),
        }
    }
}

impl PackWorld {
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
    #[cfg(feature = "fs")]
    FixedTimestamp(typst_kit::datetime::Time),
    /// The system clock.
    #[cfg(feature = "fs")]
    System(typst_kit::datetime::Time),
}

/// Serves file requests only from a Pack and verified dependency snapshots.
struct PackLoader {
    pack: Arc<Pack>,
    project_overrides: BTreeMap<String, Bytes>,
    resources: CompilationResources,
    exact_packages: BTreeMap<String, PackageFiles>,
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
                .map(Ok)
                .unwrap_or_else(|| {
                    self.resources
                        .file(&self.pack, id)
                        .expect("project requests are handled by Resource Slot resolution")
                }),
            VirtualRoot::Package(spec) => {
                if self.pack.has_package(spec) {
                    self.pack
                        .package_file(spec, path)
                        .cloned()
                        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
                } else if let Some(package) = self.exact_packages.get(&spec.to_string()) {
                    package
                        .file(path)
                        .cloned()
                        .ok_or_else(|| FileError::NotFound(PathBuf::from(path)))
                } else {
                    Err(FileError::Other(Some(
                        format!("package {spec} has no verified Complete Package Tree").into(),
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
    catalog_fonts: Option<Vec<Font>>,
    exact_packages: Option<BTreeMap<String, PackageFiles>>,
    resource_providers: Vec<Provider>,
    project_overrides: BTreeMap<String, Bytes>,
}

/// A Pack cannot be exposed to Typst without exact dependency snapshots.
#[derive(Debug, thiserror::Error)]
pub enum PackWorldBuildError {
    #[error("external package fulfillment is unavailable for {packages:?}")]
    MissingExternalPackages {
        packages: Vec<typst::syntax::package::PackageSpec>,
    },
    #[error(transparent)]
    FontCatalog(#[from] FontCatalogError),
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
            embedded_fonts: false,
            extra_fonts: Vec::new(),
            catalog_fonts: None,
            exact_packages: None,
            resource_providers: Vec::new(),
            project_overrides: BTreeMap::new(),
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
        self.clock = Clock::FixedDate(datetime);
        self
    }

    /// Uses a fixed UNIX timestamp for `datetime.today()`, for reproducible output.
    #[cfg(feature = "fs")]
    pub fn fixed_timestamp(mut self, timestamp: i64) -> typst::diag::StrResult<Self> {
        self.clock = Clock::FixedTimestamp(typst_kit::datetime::Time::fixed_timestamp(timestamp)?);
        Ok(self)
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

    pub(crate) fn exact_font_catalog(mut self, fonts: Vec<Font>) -> Self {
        self.catalog_fonts = Some(fonts);
        self
    }

    pub(crate) fn exact_project_overrides(mut self, overrides: BTreeMap<String, Bytes>) -> Self {
        self.project_overrides = overrides;
        self
    }

    pub(crate) fn exact_packages(mut self, packages: BTreeMap<String, PackageFiles>) -> Self {
        self.exact_packages = Some(packages);
        self
    }

    /// Adds a Resource Provider for declared Resource Slots.
    ///
    /// Providers are tried in registration order after packed project files.
    ///
    /// ```compile_fail
    /// use std::path::PathBuf;
    /// use typst::diag::{FileError, FileResult};
    /// use typst::foundations::Bytes;
    /// use typst::syntax::FileId;
    /// use typst_kit::files::FileLoader;
    /// use typst_pack::{Pack, PackWorld};
    ///
    /// struct Missing;
    /// impl FileLoader for Missing {
    ///     fn load(&self, id: FileId) -> FileResult<Bytes> {
    ///         Err(FileError::NotFound(PathBuf::from(
    ///             id.vpath().get_without_slash(),
    ///         )))
    ///     }
    /// }
    ///
    /// let pack = Pack::builder("main.typ")
    ///     .file("main.typ", Vec::new())?
    ///     .build()?;
    /// let _ = PackWorld::builder(pack).external_resource_reference(Missing);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn resource_provider(mut self, provider: impl FileLoader + Send + Sync + 'static) -> Self {
        self.resource_providers.push(Box::new(provider));
        self
    }

    pub(crate) fn resource_providers(mut self, providers: Vec<Provider>) -> Self {
        self.resource_providers.extend(providers);
        self
    }

    /// Builds the world.
    pub fn build(self) -> Result<PackWorld, PackWorldBuildError> {
        let missing_packages = self
            .pack
            .package_requirements()
            .iter()
            .filter(|requirement| !requirement.is_embedded())
            .map(|requirement| requirement.spec().clone())
            .collect::<Vec<_>>();
        if self.exact_packages.is_none() && !missing_packages.is_empty() {
            return Err(PackWorldBuildError::MissingExternalPackages {
                packages: missing_packages,
            });
        }
        let entrypoint = typst::syntax::VirtualPath::new(self.pack.entrypoint())
            .expect("Pack entrypoint invariant violated");
        let main = RootedPath::new(VirtualRoot::Project, entrypoint).intern();

        let catalog = if let Some(catalog) = self.catalog_fonts {
            catalog
        } else {
            let mut fulfillments = std::collections::BTreeMap::new();
            for (source, _) in self.extra_fonts {
                if let Some(font) = source.load() {
                    fulfillments
                        .entry(FontContainerIdentity::from_bytes(font.data().as_slice()))
                        .or_insert_with(|| font.data().clone());
                }
            }
            #[cfg(feature = "embedded-fonts")]
            if self.embedded_fonts {
                for (font, _) in typst_kit::fonts::embedded() {
                    fulfillments
                        .entry(FontContainerIdentity::from_bytes(font.data().as_slice()))
                        .or_insert_with(|| font.data().clone());
                }
            }
            self.pack.materialize_font_catalog(&fulfillments)?
        };
        let mut fonts = FontStore::new();
        for font in catalog {
            let info = font.info().clone();
            fonts.push((font, info));
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
                project_overrides: self.project_overrides,
                resources: CompilationResources::new(self.resource_providers),
                exact_packages: self.exact_packages.unwrap_or_default(),
            }),
            fonts,
            clock: self.clock,
        })
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

#[cfg(all(
    feature = "fs",
    feature = "_test-package-download-probe",
    debug_assertions
))]
struct PackageDownloadProbe {
    certificate: Option<PathBuf>,
    output: PathBuf,
}

#[cfg(all(
    feature = "fs",
    feature = "_test-package-download-probe",
    debug_assertions
))]
impl typst_kit::downloader::Downloader for PackageDownloadProbe {
    fn stream(
        &self,
        _key: &dyn std::any::Any,
        _url: &str,
    ) -> std::io::Result<(Option<usize>, Box<dyn std::io::Read>)> {
        let certificate = self
            .certificate
            .as_deref()
            .map(|path| path.to_string_lossy())
            .unwrap_or_default();
        std::fs::write(&self.output, certificate.as_bytes())?;
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "package download stopped by test probe",
        ))
    }
}

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

#[cfg(all(test, feature = "fs"))]
mod tests {
    use super::*;

    #[test]
    fn certificate_path_is_forwarded_to_the_online_downloader_factory() {
        use typst_kit::packages::UniversePackages;

        let directory = tempfile::tempdir().unwrap();
        let certificate = directory.path().join("certificate.pem");
        let mut seen = None;

        let _packages = system_packages_with_online(
            Some(directory.path()),
            Some(directory.path()),
            false,
            Some(&certificate),
            |path| {
                seen = path.map(PathBuf::from);
                UniversePackages::new(OfflineDownloader)
            },
        );

        assert_eq!(seen.as_deref(), Some(certificate.as_path()));
    }
}
