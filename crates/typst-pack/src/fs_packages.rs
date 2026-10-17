//! The package-reading half of Pack Assembly for the reference filesystem
//! Pack Assembler.
//!
//! Reading is resume-driven: the core reports the exact specifications its
//! representative request read and was not given, and the adapter obtains each
//! of them through the configured Package Authority — local package
//! directories, then the package cache, then a download unless creation is
//! offline or the build has no egress compiled in to download with.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst_kit::packages::FsPackages;

use crate::fs_read::{FilesystemReadError, Root, Walker, utf8_path};
use crate::limits::ReadLimits;
use crate::package_catalog::PackageTree;
use crate::package_failure::{PackageReadFailure, PackageReadFailureReason};

/// Reads every file beneath one package root into a validated Package Tree.
///
/// The root may itself be a symbolic link, as `@local` development packages
/// commonly are. Links inside the tree are followed when they resolve beneath
/// the root.
pub fn read_filesystem_package(
    root: impl AsRef<Path>,
    limits: ReadLimits,
) -> Result<PackageTree, FilesystemReadError> {
    let root_path = root.as_ref();
    let root = Root::open(root_path)?;
    let mut entries = Vec::new();
    Walker::strict(limits).walk(
        &root,
        &mut |relative, _| utf8_path(root_path, relative).map(|_| true),
        &mut |relative, bytes| {
            entries.push((utf8_path(root_path, &relative)?, bytes));
            Ok(())
        },
    )?;
    PackageTree::from_owned_entries(entries).map_err(FilesystemReadError::PackageTree)
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
        source: Box<FilesystemReadError>,
    },
    #[cfg(feature = "egress")]
    #[error("{failure}: {source}")]
    Download {
        failure: PackageReadFailure,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl FilesystemPackageAuthorityReadError {
    /// The stable exact-specification failure represented by this adapter error.
    pub fn failure(&self) -> &PackageReadFailure {
        match self {
            Self::Unavailable(failure) | Self::Filesystem { failure, .. } => failure,
            #[cfg(feature = "egress")]
            Self::Download { failure, .. } => failure,
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
    limits: ReadLimits,
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
            crate::FilesystemReadLimits::default().packages,
        )
    }

    pub(crate) fn with_limits(
        package_path: Option<&Path>,
        package_cache_path: Option<&Path>,
        offline: bool,
        limits: ReadLimits,
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
            limits,
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
        // A build without egress has no transport, so it behaves as offline.
        if self.offline || cfg!(not(feature = "egress")) {
            return Err(FilesystemPackageAuthorityReadError::Unavailable(not_found(
                spec,
            )));
        }
        #[cfg(feature = "egress")]
        {
            egress::read_from_registry(self, spec)
        }
        #[cfg(not(feature = "egress"))]
        {
            unreachable!("a build without egress behaves as offline")
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
        let tree = read_filesystem_package(root.path(), self.limits).map_err(|source| {
            FilesystemPackageAuthorityReadError::Filesystem {
                failure: other_failure(spec, source.to_string()),
                source: Box::new(source),
            }
        })?;
        Ok(Some(ReadPackage {
            tree,
            root: Some(root.path().to_owned()),
        }))
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

/// Registry downloads, compiled only into builds with network egress.
#[cfg(feature = "egress")]
mod egress {
    use std::io::{BufReader, Read};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, OnceLock};

    use typst::diag::PackageError;
    use typst::syntax::package::PackageSpec;
    use typst_kit::downloader::Downloader;

    use super::{
        FilesystemPackageAuthority, FilesystemPackageAuthorityReadError, ReadPackage, not_found,
        other_failure,
    };
    use crate::package_catalog::PackageTree;
    use crate::package_failure::{PackageReadFailure, PackageReadFailureReason};
    use crate::{
        PackageArchiveReadError, PackageReadError, ReadLimits, expand_package_archive,
        package_archive_url, read_package_archive,
    };

    const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

    #[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
    const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

    pub(super) fn read_from_registry(
        authority: &FilesystemPackageAuthority,
        spec: &PackageSpec,
    ) -> Result<ReadPackage, FilesystemPackageAuthorityReadError> {
        let failed = |failure, source: Box<dyn std::error::Error + Send + Sync>| {
            FilesystemPackageAuthorityReadError::Download { failure, source }
        };
        let url =
            package_archive_url(spec).map_err(|source| failed(not_found(spec), source.into()))?;
        let downloader = RustlsDownloader::new(USER_AGENT, authority.certificate.clone());
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
            failed(failure, source.into())
        })?;
        let tree = read_registry_tree(
            spec,
            reader,
            known_size.map(|size| size as u64),
            authority.limits,
        )?;

        // typst-kit stages the tree in a temporary sibling directory and
        // renames it into place, so a failed or concurrent download never
        // leaves a partial package in the cache.
        let root = if let Some(cache) = &authority.cache {
            cache
                .store(spec, |directory| write_tree(directory, &tree))
                .map_err(|source| {
                    failed(other_failure(spec, source.to_string()), Box::new(source))
                })?;
            Some(
                cache
                    .path()
                    .join(crate::read_layout::package_tree_key(spec)),
            )
        } else {
            None
        };
        Ok(ReadPackage { tree, root })
    }

    pub(super) fn read_registry_tree(
        spec: &PackageSpec,
        reader: impl Read,
        known_size: Option<u64>,
        limits: ReadLimits,
    ) -> Result<PackageTree, FilesystemPackageAuthorityReadError> {
        let failed = |failure, source: Box<dyn std::error::Error + Send + Sync>| {
            FilesystemPackageAuthorityReadError::Download { failure, source }
        };
        let archive =
            read_package_archive(reader, known_size, limits.max_file_bytes).map_err(|source| {
                let failure = match &source {
                    PackageArchiveReadError::Read(error) => PackageReadFailure::new(
                        spec.clone(),
                        PackageReadFailureReason::NetworkFailed {
                            detail: Some(error.to_string()),
                        },
                    ),
                    PackageArchiveReadError::Limit(error) => other_failure(spec, error.to_string()),
                };
                failed(failure, Box::new(source))
            })?;
        expand_package_archive(spec.clone(), &archive, limits).map_err(|source| {
            let failure = match &source {
                PackageReadError::UnservedNamespace { .. } => not_found(spec),
                PackageReadError::Limit { .. } => other_failure(spec, source.to_string()),
                PackageReadError::MalformedArchive { .. }
                | PackageReadError::InvalidPackageTree { .. } => PackageReadFailure::new(
                    spec.clone(),
                    PackageReadFailureReason::MalformedArchive {
                        detail: Some(source.to_string()),
                    },
                ),
            };
            failed(failure, Box::new(source))
        })
    }

    /// Writes a downloaded tree into typst-kit's private staging directory.
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

    fn package_cache_error(error: std::io::Error) -> PackageError {
        PackageError::Other(Some(
            format!("failed to cache downloaded package: {error}").into(),
        ))
    }

    pub(super) struct RustlsDownloader {
        user_agent: &'static str,
        certificate: Option<PathBuf>,
        tls: OnceLock<Result<Option<Arc<ureq::rustls::ClientConfig>>, String>>,
    }

    impl RustlsDownloader {
        pub(super) fn new(user_agent: &'static str, certificate: Option<PathBuf>) -> Self {
            Self {
                user_agent,
                certificate,
                tls: OnceLock::new(),
            }
        }

        /// Reads one PEM file of extra trust anchors on top of the bundled roots.
        ///
        /// PEM parsing comes from rustls-pki-types, which absorbed it from the
        /// now-unmaintained rustls-pemfile (RUSTSEC-2025-0134).
        pub(super) fn root_store(path: &Path) -> Result<ureq::rustls::RootCertStore, String> {
            use ureq::rustls::pki_types::CertificateDer;
            use ureq::rustls::pki_types::pem::PemObject;

            let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
            let reader = BufReader::new(file);
            let mut roots = ureq::rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            for certificate in CertificateDer::pem_reader_iter(reader) {
                let certificate = certificate.map_err(|error| error.to_string())?;
                roots.add(certificate).map_err(|error| error.to_string())?;
            }
            Ok(roots)
        }

        pub(super) fn tls_config(
            &self,
        ) -> std::io::Result<Option<Arc<ureq::rustls::ClientConfig>>> {
            match self.tls.get_or_init(|| {
                let Some(path) = &self.certificate else {
                    return Ok(None);
                };
                let roots = Self::root_store(path)?;
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

    impl Downloader for RustlsDownloader {
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

        /// The custom-certificate path is the crate's only PEM parsing. It has no
        /// process-level coverage, so this pins the behavior directly.
        const TEST_CERTIFICATE_PEM: &str = "\
-----BEGIN CERTIFICATE-----\n\
MIIDHTCCAgWgAwIBAgIUd7rNizjvFLekUK8kk8YGERd1Ru8wDQYJKoZIhvcNAQEL\n\
BQAwHTEbMBkGA1UEAwwSdHlwc3QtcGFjayB0ZXN0IENBMCAXDTI2MDgyNTEyNTcz\n\
NloYDzIxMjYwODAxMTI1NzM2WjAdMRswGQYDVQQDDBJ0eXBzdC1wYWNrIHRlc3Qg\n\
Q0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDaimCA3A7auE+n08On\n\
4RKiJPJzM8Aqv9zYvjXRJsMuo1aXBme9MLYEAu+T9DCxPWaVJYEKqZEkNYJVOF04\n\
5ONBZYsk7uQ8a72DZtBV123RXWPa/cD9ieXVgN4oRC2FdfyELyTcUHtRo1zFlSSx\n\
dz2TY0Zzk5ON62d1O89zdeUPJ/mah4jxuPJeFPTeJZFdYOjaOSUr06MVWgYQ1hC0\n\
nNYG3rFqVucb+83stE1nZFBAegwrdE6poQUFT3rE+ApxVzHDWi1gaD3VVBszmMjZ\n\
FwufcnW2kCXVvmM5YebnvLyqIHvNddWkMygWSPwAJDFXWRdi6MzYkoC1qTpbkj8J\n\
PI+7AgMBAAGjUzBRMB0GA1UdDgQWBBRlvjvjxE0mp20Do4trUIL0RMDa8jAfBgNV\n\
HSMEGDAWgBRlvjvjxE0mp20Do4trUIL0RMDa8jAPBgNVHRMBAf8EBTADAQH/MA0G\n\
CSqGSIb3DQEBCwUAA4IBAQAigZCXG4KVwXHFE7G0wLxWuWizmLgj2fEU7W0Wy1rp\n\
qXEOlKCNG0MX/JTlqydyv+ZSKPMyl9eX+SPFlhlm/Az8kPHYWd7CqdO4Kz0fSEhx\n\
IrChOWM5OfEUn+JXRSDJnBFcYVgucKdeC9yiP3WqQc0fJ5j14+tWUsMU1lr/392L\n\
xZni29e/+sP8VvO2eGO+CyYsAlgTJ0Ka1QwSJYS3jLMfYhsNpMpnmzL7bAy+Uyuz\n\
/Xt4qjy1STvhUAruckTFLjJ/6i0GK2mm0XjI4/xXoyh2c40yj16l87Ncur2bjQ3o\n\
D/11HiutermQt0RJByBT1FPDdLyRtRPwY1PoAi2/OlAb\n\
-----END CERTIFICATE-----\n\
";

        #[test]
        fn custom_certificate_pem_is_added_to_the_root_store() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("ca.pem");
            std::fs::write(&path, TEST_CERTIFICATE_PEM).unwrap();

            // The bundled roots plus exactly the one certificate supplied here.
            let roots =
                RustlsDownloader::root_store(&path).expect("a valid PEM certificate is read");
            assert_eq!(roots.roots.len(), webpki_roots::TLS_SERVER_ROOTS.len() + 1);

            let downloader = RustlsDownloader::new("typst-pack-test", Some(path));
            assert!(
                downloader
                    .tls_config()
                    .expect("a valid PEM certificate is accepted")
                    .is_some()
            );
        }

        /// A file with no PEM section adds no trust anchor and is not an error, so
        /// a misdirected `--cert` silently falls back to the bundled roots.
        #[test]
        fn certificate_file_without_a_pem_section_keeps_the_default_roots() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("ca.pem");
            std::fs::write(&path, "not a certificate").unwrap();

            let roots =
                RustlsDownloader::root_store(&path).expect("a section-free file is not an error");
            assert_eq!(roots.roots.len(), webpki_roots::TLS_SERVER_ROOTS.len());
        }

        #[test]
        fn corrupt_certificate_pem_is_rejected() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("ca.pem");
            std::fs::write(
                &path,
                "-----BEGIN CERTIFICATE-----\nnot base64!!\n-----END CERTIFICATE-----\n",
            )
            .unwrap();

            assert!(RustlsDownloader::root_store(&path).is_err());

            let downloader = RustlsDownloader::new("typst-pack-test", Some(path));
            assert!(downloader.tls_config().is_err());
        }

        #[test]
        fn absent_certificate_leaves_the_default_roots_in_place() {
            let downloader = RustlsDownloader::new("typst-pack-test", None);
            assert!(downloader.tls_config().unwrap().is_none());
        }

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
        fn registry_response_bytes_are_bounded_then_expanded_without_a_reread() {
            let spec = "@preview/example:1.0.0".parse().unwrap();
            let archive = package_archive(&[("lib.typ", b"exact registry bytes")]);

            let tree = read_registry_tree(
                &spec,
                std::io::Cursor::new(&archive),
                Some(archive.len() as u64),
                ReadLimits::default(),
            )
            .unwrap();

            assert_eq!(tree.file("lib.typ"), Some(&b"exact registry bytes"[..]));
        }

        #[test]
        fn registry_expansion_limits_are_not_reported_as_malformed_bytes() {
            let spec = "@preview/example:1.0.0".parse().unwrap();
            let archive = package_archive(&[("lib.typ", b"12345")]);
            let limits = ReadLimits {
                max_total_bytes: 4,
                ..ReadLimits::default()
            };

            let error = read_registry_tree(
                &spec,
                std::io::Cursor::new(&archive),
                Some(archive.len() as u64),
                limits,
            )
            .unwrap_err();

            assert!(matches!(
                error.failure().reason(),
                PackageReadFailureReason::Other { .. }
            ));
            let FilesystemPackageAuthorityReadError::Download { source, .. } = error else {
                panic!("expected a download failure");
            };
            assert!(matches!(
                source.downcast_ref::<PackageReadError>(),
                Some(PackageReadError::Limit { .. })
            ));
        }
    }
}
