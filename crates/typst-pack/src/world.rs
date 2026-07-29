//! A complete Typst [`World`] backed by a [`Pack`].

use std::collections::BTreeMap;
#[cfg(feature = "egress")]
use std::io::{self, BufReader, Read};
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "egress")]
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::FontStore;

use crate::compile::DocumentTime;
use crate::pack::{CompilationDependencySnapshot, Pack, PackageFiles};

#[cfg(feature = "egress")]
const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

// Integration tests execute a separate non-`cfg(test)` binary.
#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

/// The Package Authority a build with egress resolves specifications through:
/// the local package directories, the package cache, and a download from the
/// Typst Universe registry unless creation is offline.
#[cfg(feature = "egress")]
#[doc(hidden)]
pub fn system_packages(
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

#[cfg(feature = "egress")]
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

#[cfg(feature = "egress")]
fn system_packages_with_online(
    package_path: Option<&std::path::Path>,
    package_cache_path: Option<&std::path::Path>,
    offline: bool,
    certificate: Option<&std::path::Path>,
    online: impl FnOnce(Option<&std::path::Path>) -> typst_kit::packages::UniversePackages,
) -> typst_kit::packages::SystemPackages {
    use typst_kit::packages::{SystemPackages, UniversePackages};

    let (data, cache) = local_package_directories(package_path, package_cache_path);
    let universe = if offline {
        UniversePackages::new(OfflineDownloader)
    } else {
        online(certificate)
    };

    SystemPackages::from_parts(data, cache, universe)
}

/// The Package Authority a build without egress resolves specifications
/// through: the local package directories and whichever package cache the host
/// has. No download is reachable, whatever the offline switch says, because no
/// transport is linked in to reach one with.
#[cfg(all(feature = "fs", not(feature = "egress")))]
pub(crate) fn local_packages(
    package_path: Option<&std::path::Path>,
) -> typst_kit::packages::SystemPackages {
    use typst_kit::packages::{SystemPackages, UniversePackages};

    let (data, cache) = local_package_directories(package_path, None);

    SystemPackages::from_parts(data, cache, UniversePackages::new(OfflineDownloader))
}

/// The local package directory and package cache a Package Authority reads,
/// each explicitly chosen or the host's own.
#[cfg(feature = "fs")]
fn local_package_directories(
    package_path: Option<&std::path::Path>,
    package_cache_path: Option<&std::path::Path>,
) -> (
    Option<typst_kit::packages::FsPackages>,
    Option<typst_kit::packages::FsPackages>,
) {
    use typst_kit::packages::FsPackages;

    let data = match package_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_data(),
    };
    let cache = match package_cache_path {
        Some(path) => Some(FsPackages::new(path)),
        None => FsPackages::system_cache(),
    };
    (data, cache)
}

#[cfg(feature = "fs")]
#[doc(hidden)]
pub fn read_complete_package_tree(root: &std::path::Path) -> Result<Vec<(String, Bytes)>, String> {
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
                .or_else(|| self.pack.file(path).cloned())
                .ok_or_else(|| FileError::NotFound(PathBuf::from(path))),
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

/// A package downloader that refuses to download.
///
/// Plug this into [`typst_kit::packages::UniversePackages`] to guarantee
/// that package resolution never accesses the network: every download
/// attempt fails as not found, so only local directories (or the pack
/// itself) can satisfy dependencies.
///
/// This is a runtime guarantee, for a build that can reach the network. A build
/// without the `egress` feature links no transport at all, so it resolves
/// packages this way whatever its runtime configuration.
#[cfg(feature = "fs")]
pub struct OfflineDownloader;

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
struct PackageDownloadProbe {
    certificate: Option<PathBuf>,
    output: PathBuf,
}

#[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
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

#[cfg(all(test, feature = "egress"))]
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
