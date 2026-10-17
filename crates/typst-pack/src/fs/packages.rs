//! Reading packages for the packer: the local package directory, then the
//! package cache, then, only when downloads are enabled in a build with
//! `egress`, the Typst Universe registry.

use std::path::{Path, PathBuf};

use typst::syntax::package::PackageSpec;
use typst_kit::packages::FsPackages;

use super::Error;
use super::walk::{Root, Walker, utf8_path};
use crate::ReadLimits;
use crate::package::{ReadFailure, Tree};

/// Reads every file beneath one package root into a package tree.
///
/// The root may itself be a symbolic link, as `@local` development packages
/// commonly are. Links inside the tree are followed when they resolve beneath
/// the root.
pub fn read_package(root: impl AsRef<Path>, limits: ReadLimits) -> Result<Tree, Error> {
    let root_path = root.as_ref();
    let root = Root::open(root_path)?;
    let mut files = Vec::new();
    Walker::strict(limits).walk(
        &root,
        |relative, _| utf8_path(root_path, relative).map(|_| true),
        |relative, bytes| {
            files.push((utf8_path(root_path, &relative)?, bytes));
            Ok(())
        },
    )?;
    Ok(Tree::new(files)?)
}

/// Where the packer reads packages from.
#[derive(Clone, Debug)]
pub(crate) struct PackageDirs {
    data: Option<FsPackages>,
    cache: Option<FsPackages>,
    #[cfg(feature = "egress")]
    registry: Option<Registry>,
    limits: ReadLimits,
}

/// A package tree read from the filesystem or registry.
pub(crate) struct Found {
    pub(crate) tree: Tree,
    /// The directory the tree was read from or cached to.
    pub(crate) root: Option<PathBuf>,
}

impl PackageDirs {
    pub(crate) fn new(
        package_path: Option<&Path>,
        package_cache_path: Option<&Path>,
        #[cfg(feature = "egress")] registry: Option<Registry>,
        limits: ReadLimits,
    ) -> Self {
        Self {
            data: package_path
                .map_or_else(FsPackages::system_data, |path| Some(FsPackages::new(path))),
            cache: package_cache_path
                .map_or_else(FsPackages::system_cache, |path| Some(FsPackages::new(path))),
            #[cfg(feature = "egress")]
            registry,
            limits,
        }
    }

    /// Reads one exact package tree.
    pub(crate) fn read(&self, spec: &PackageSpec) -> Result<Found, ReadFailure> {
        for packages in [&self.data, &self.cache].into_iter().flatten() {
            let Some(root) = packages.obtain(spec) else {
                continue;
            };
            let tree = read_package(root.path(), self.limits)
                .map_err(|error| ReadFailure::Other(Some(error.to_string())))?;
            return Ok(Found {
                tree,
                root: Some(root.path().to_owned()),
            });
        }
        #[cfg(feature = "egress")]
        if let Some(registry) = &self.registry {
            return registry.read(spec, self.cache.as_ref(), self.limits);
        }
        Err(ReadFailure::NotFound)
    }
}

#[cfg(feature = "egress")]
pub(crate) use egress::Registry;

/// Registry downloads, compiled only into builds with network egress.
#[cfg(feature = "egress")]
mod egress {
    use std::io::{BufReader, Read};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, OnceLock};

    use typst::diag::PackageError;
    use typst::syntax::package::PackageSpec;
    use typst_kit::downloader::Downloader;
    use typst_kit::packages::FsPackages;

    use super::Found;
    use crate::package::registry::{ExpandError, archive_url, expand_archive};
    use crate::package::{ReadFailure, Tree, tree_key};
    use crate::{LimitExceeded, ReadLimits};

    const USER_AGENT: &str = concat!("typst-pack/", env!("CARGO_PKG_VERSION"));

    #[cfg(all(feature = "_test-package-download-probe", debug_assertions))]
    const PACKAGE_DOWNLOAD_PROBE_ENV: &str = "TYPST_PACK_TEST_PACKAGE_DOWNLOAD_PROBE";

    /// Downloads from the official registry over TLS.
    #[derive(Clone, Debug)]
    pub(crate) struct Registry {
        certificate: Option<PathBuf>,
    }

    impl Registry {
        pub(crate) fn new(certificate: Option<PathBuf>) -> Self {
            Self { certificate }
        }

        pub(super) fn read(
            &self,
            spec: &PackageSpec,
            cache: Option<&FsPackages>,
            limits: ReadLimits,
        ) -> Result<Found, ReadFailure> {
            let url = archive_url(spec).ok_or(ReadFailure::NotFound)?;
            let downloader = RustlsDownloader::new(USER_AGENT, self.certificate.clone());
            let (known_size, reader) = downloader.stream(spec, &url).map_err(|source| {
                // Do not turn a bounded archive request into an unbounded
                // package index download merely to refine NotFound into
                // VersionNotFound.
                if source.kind() == std::io::ErrorKind::NotFound {
                    ReadFailure::NotFound
                } else {
                    ReadFailure::NetworkFailed(Some(source.to_string()))
                }
            })?;
            let tree = read_registry_tree(reader, known_size.map(|size| size as u64), limits)?;

            // typst-kit stages the tree in a temporary sibling directory and
            // renames it into place, so a failed or concurrent download never
            // leaves a partial package in the cache.
            let root = match cache {
                Some(cache) => {
                    cache
                        .store(spec, |directory| write_tree(directory, &tree))
                        .map_err(|source| ReadFailure::Other(Some(source.to_string())))?;
                    Some(cache.path().join(tree_key(spec)))
                }
                None => None,
            };
            Ok(Found { tree, root })
        }
    }

    /// Reads at most `limits.max_file_bytes` of archive bytes, checking a
    /// known size before reading, and expands them.
    pub(super) fn read_registry_tree(
        reader: impl Read,
        known_size: Option<u64>,
        limits: ReadLimits,
    ) -> Result<Tree, ReadFailure> {
        let max = limits.max_file_bytes;
        let limit = |error: LimitExceeded| ReadFailure::Other(Some(error.to_string()));
        if let Some(size) = known_size {
            LimitExceeded::check(size, max, "package archive bytes").map_err(limit)?;
        }
        let mut archive = Vec::new();
        reader
            .take(max.saturating_add(1))
            .read_to_end(&mut archive)
            .map_err(|error| ReadFailure::NetworkFailed(Some(error.to_string())))?;
        LimitExceeded::check(archive.len() as u64, max, "package archive bytes").map_err(limit)?;
        expand_archive(&archive, limits).map_err(|error| match error {
            ExpandError::Limit(_) => ReadFailure::Other(Some(error.to_string())),
            _ => ReadFailure::MalformedArchive(Some(error.to_string())),
        })
    }

    /// Writes a downloaded tree into typst-kit's private staging directory.
    fn write_tree(directory: &Path, tree: &Tree) -> typst::diag::PackageResult<()> {
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
            let archive = package_archive(&[("lib.typ", b"exact registry bytes")]);

            let tree = read_registry_tree(
                std::io::Cursor::new(&archive),
                Some(archive.len() as u64),
                ReadLimits::default(),
            )
            .unwrap();

            assert_eq!(tree.file("lib.typ"), Some(&b"exact registry bytes"[..]));
        }

        #[test]
        fn registry_expansion_limits_are_not_reported_as_malformed_bytes() {
            let archive = package_archive(&[("lib.typ", b"12345")]);
            let limits = ReadLimits {
                max_total_bytes: 4,
                ..ReadLimits::default()
            };

            let error = read_registry_tree(
                std::io::Cursor::new(&archive),
                Some(archive.len() as u64),
                limits,
            )
            .unwrap_err();

            assert!(matches!(error, ReadFailure::Other(_)));
        }

        #[test]
        fn registry_responses_are_bounded_before_and_while_reading() {
            struct Unread;
            impl Read for Unread {
                fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                    panic!("a known oversized archive is refused before it is read");
                }
            }
            let limits = ReadLimits {
                max_file_bytes: 4,
                ..ReadLimits::default()
            };

            let known = read_registry_tree(Unread, Some(5), limits).unwrap_err();
            let streamed =
                read_registry_tree(std::io::Cursor::new(b"12345"), None, limits).unwrap_err();

            assert!(matches!(known, ReadFailure::Other(_)));
            assert!(matches!(streamed, ReadFailure::Other(_)));
        }
    }
}
