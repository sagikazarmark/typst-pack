//! The cross-adapter creation conformance corpus.
//!
//! Source-neutral scenarios are expressed as selected project bytes, Package
//! Trees or raw archives, and ordered Font Containers. Concrete runners drive
//! those records through direct core composition, OpenDAL Memory, and, where
//! the source model is intentionally equivalent, the reference filesystem Pack
//! Assembler. This is the operation-specific conformance shape required by
//! ADR-0014; it introduces no production conformance trait or storage policy.
//!
//! Project membership is already selected in shared scenarios. ADR-0011 leaves
//! that selection to each reader, so filesystem-only `.typkignore` behavior
//! belongs in `fs_creation.rs` and is deliberately absent here. Equivalent
//! selected bytes must issue the same semantic Pack promised by ADR-0008.
//!
//! Every assertion stays on the public library surface: contained project
//! bytes, requirements, Pack Font Catalog order, Pack Identity,
//! representative-compile warnings, Missing Package Specifications, and
//! corrected Package Read Failures. Nothing here observes loader
//! structure, store internals, archive encoding, or file request order.
//!
//! Scenario applicability states whether the filesystem source can intentionally
//! express the same inputs. OpenDAL and direct composition run every scenario.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

#[cfg(all(feature = "opendal", feature = "package-reading"))]
#[path = "support/opendal.rs"]
mod opendal_faults;

use std::collections::HashMap;
use std::str::FromStr;

use typst::diag::SourceDiagnostic;
use typst::syntax::package::PackageSpec;
use typst_pack::compile::DocumentTime;
use typst_pack::{Digest, Pack, PackError, PackIssue, ProjectSnapshot, create, font, package};
#[cfg(feature = "fs")]
use typst_pack::{compile, fs};

type Failures = HashMap<PackageSpec, package::ReadFailure>;

/// 2023-11-14T22:13:20Z, the Document Time every representative request in the
/// corpus is fixed to, so that no adapter's host clock reaches a Pack.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// The version every fixture package is supplied under.
const PACKAGE_VERSION: &str = "1.0.0";

/// How many resume rounds a fixture may take before the corpus calls the loop
/// stuck. Scenarios may separately declare their exact public missing trace.
const RESUME_BOUND: usize = 8;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// One fixture, expressed as bytes alone: an adapter turns it into whatever its
/// host needs — a directory tree, or values held in memory.
struct Fixture {
    entrypoint: &'static str,
    project: Vec<(&'static str, Vec<u8>)>,
    packages: Vec<PackageFixture>,
    fonts: Vec<FontFixture>,
}

/// One Package Tree an adapter may resolve for the fixture.
struct PackageFixture {
    spec: PackageSpec,
    source: PackageSource,
    disposition: bool,
    corrects_failure: bool,
}

/// How an adapter obtains the tree for one specification.
enum PackageSource {
    /// The tree's files, as bytes: what a package directory holds, and what an
    /// already-read source-neutral input supplies directly.
    Files(Vec<(&'static str, Vec<u8>)>),
    /// The archive a registry serves, which a caller supplying its own
    /// transport fetches and expands under a required ceiling.
    #[cfg(feature = "package-reading")]
    CachedArchive {
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
    },
    /// The exact archive bytes returned by an official registry.
    #[cfg(feature = "package-reading")]
    RegistryArchive {
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
    },
}

/// One Font Container the fixture offers, at the catalog position it
/// is declared in.
struct FontFixture {
    source: FontSource,
    disposition: bool,
}

/// Where a fixture's Font Container comes from.
enum FontSource {
    /// Exact container bytes, which the filesystem adapter offers by scanning a
    /// directory holding them and nothing else.
    #[cfg_attr(not(feature = "embedded-fonts"), allow(dead_code))]
    Scanned(Vec<u8>),
    /// Every container Typst embeds, in Typst's own order, which the filesystem
    /// adapter offers from its own copy rather than from a scanned directory.
    #[cfg(feature = "embedded-fonts")]
    TypstEmbedded,
}

#[cfg(feature = "fs")]
impl FontSource {
    /// The exact container bytes of a scanned container.
    fn scanned(&self) -> Option<&Vec<u8>> {
        match self {
            Self::Scanned(data) => Some(data),
            #[cfg(feature = "embedded-fonts")]
            Self::TypstEmbedded => None,
        }
    }
}

#[cfg(feature = "fs")]
impl PackageSource {
    /// The files of a tree supplied as files.
    fn files(&self) -> Option<&Vec<(&'static str, Vec<u8>)>> {
        match self {
            Self::Files(files) => Some(files),
            #[cfg(feature = "package-reading")]
            Self::CachedArchive { .. } | Self::RegistryArchive { .. } => None,
        }
    }
}

impl Fixture {
    /// A project whose only file is the entrypoint `main.typ`.
    fn document(source: &str) -> Self {
        Self {
            entrypoint: "main.typ",
            project: vec![("main.typ", source.as_bytes().to_vec())],
            packages: Vec::new(),
            fonts: Vec::new(),
        }
    }

    /// Adds one project file, which an adapter lists alongside the entrypoint.
    fn file(mut self, path: &'static str, data: impl Into<Vec<u8>>) -> Self {
        self.project.push((path, data.into()));
        self
    }

    /// Offers a `@local` package tree whose `lib.typ` holds `body`, plus a file
    /// the representative request never reads, so that whole-tree containment
    /// is observable.
    fn package(self, name: &'static str, disposition: bool, body: &str) -> Self {
        self.supplied_package(name, package_files(name, body), disposition)
    }

    /// Offers a `@local` package tree exactly as given, for a fixture about
    /// what a tree declares rather than what it holds.
    fn supplied_package(
        mut self,
        name: &'static str,
        files: Vec<(&'static str, Vec<u8>)>,
        disposition: bool,
    ) -> Self {
        self.packages.push(PackageFixture {
            spec: local_spec(name),
            source: PackageSource::Files(files),
            disposition,
            corrects_failure: false,
        });
        self
    }

    /// Offers a Package Tree after a stale read failure for the same
    /// specification, proving successful insertion corrects assembly state.
    fn corrected_package(mut self, name: &'static str, disposition: bool, body: &str) -> Self {
        self = self.package(name, disposition, body);
        self.packages.last_mut().unwrap().corrects_failure = true;
        self
    }

    /// Offers exact raw archive bytes from an OpenDAL cache.
    #[cfg(feature = "package-reading")]
    fn cached_archive(
        mut self,
        name: &'static str,
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
    ) -> Self {
        self.packages.push(PackageFixture {
            spec: registry_spec(name),
            source: PackageSource::CachedArchive { bytes, limits },
            disposition: true,
            corrects_failure: false,
        });
        self
    }

    /// Offers the archive the registry serves for a `@preview` specification,
    /// which a caller with its own transport fetches at the registry URL and
    /// expands under the given ceiling.
    #[cfg(feature = "package-reading")]
    fn registry_archive(
        mut self,
        name: &'static str,
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
    ) -> Self {
        self.packages.push(PackageFixture {
            spec: registry_spec(name),
            source: PackageSource::RegistryArchive { bytes, limits },
            disposition: true,
            corrects_failure: false,
        });
        self
    }

    /// Offers one Font Container at the end of the catalog.
    #[cfg(feature = "embedded-fonts")]
    fn font(mut self, source: FontSource, disposition: bool) -> Self {
        self.fonts.push(FontFixture {
            source,
            disposition,
        });
        self
    }

    /// The Font Catalog the fixture's containers compose, in declaration order.
    fn font_catalog(&self) -> Vec<font::Entry> {
        let mut catalog = Vec::new();
        for font in &self.fonts {
            match &font.source {
                FontSource::Scanned(data) => {
                    catalog.push(font::Entry {
                        container: font::Container::new(data.clone()).unwrap(),
                        embedded: font.disposition,
                    });
                }
                #[cfg(feature = "embedded-fonts")]
                FontSource::TypstEmbedded => {
                    catalog.extend(font::embedded().map(|container| font::Entry {
                        container,
                        embedded: font.disposition,
                    }));
                }
            }
        }
        catalog
    }

    /// The tree direct composition resolves for one reported specification.
    fn resolve(&self, spec: &PackageSpec) -> Result<(PackageSpec, package::Tree, bool), Failure> {
        let package = self
            .packages
            .iter()
            .find(|package| &package.spec == spec)
            .unwrap_or_else(|| panic!("the fixture offers no tree for `{spec}`"));
        match &package.source {
            PackageSource::Files(files) => Ok((
                spec.clone(),
                package::Tree::new(files.iter().map(|(path, data)| (*path, data.clone()))).unwrap(),
                package.disposition,
            )),
            #[cfg(feature = "package-reading")]
            PackageSource::CachedArchive { bytes, limits }
            | PackageSource::RegistryArchive { bytes, limits } => {
                // Read has completed; archive expansion is a
                // source-neutral core transformation.
                package::registry::expand_archive(bytes, *limits)
                    .map(|tree| (spec.clone(), tree, package.disposition))
                    .map_err(|error| match error {
                        package::registry::ExpandError::Limit(_) => Failure::PackageRead {
                            spec: spec.to_string(),
                            reason: package::ReadFailure::Other(None),
                            rejection: None,
                        },
                        error => panic!("the fixture archive failed to expand: {error}"),
                    })
            }
        }
    }
}

/// The `@local` specification a fixture package is supplied under.
fn local_spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:{PACKAGE_VERSION}"))
        .expect("the fixture specification is well formed")
}

/// The `@preview` specification a fixture package the registry serves is
/// supplied under, which is the only namespace with a registry URL.
#[cfg(feature = "package-reading")]
fn registry_spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@preview/{name}:{PACKAGE_VERSION}"))
        .expect("the fixture specification is well formed")
}

/// The files of a package tree declaring `name`, whose `lib.typ` holds `body`.
fn package_files(name: &str, body: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "typst.toml",
            format!(
                "[package]\n\
                 name = \"{name}\"\n\
                 version = \"{PACKAGE_VERSION}\"\n\
                 entrypoint = \"lib.typ\"\n"
            )
            .into_bytes(),
        ),
        ("lib.typ", body.as_bytes().to_vec()),
        ("unread.txt", b"the whole Package Tree travels".to_vec()),
    ]
}

// ---------------------------------------------------------------------------
// Adapter results
// ---------------------------------------------------------------------------

/// What one Pack Assembler produced for a fixture.
struct Created {
    pack: Pack,
    warnings: Vec<typst::diag::SourceDiagnostic>,
    missing_rounds: Vec<Vec<String>>,
    failure_states: Vec<Vec<(String, package::ReadFailure)>>,
}

/// The diagnostics of a discovery compile that failed.
#[derive(Debug, PartialEq, Eq)]
struct DependencyDiscoveryRejection {
    errors: Vec<SourceDiagnostic>,
    warnings: Vec<SourceDiagnostic>,
}

/// The rejection a failed creation carries, or a panic for any other error.
fn rejection(error: create::Error, runner: &str) -> DependencyDiscoveryRejection {
    match error {
        create::Error::Failed {
            errors, warnings, ..
        } => DependencyDiscoveryRejection { errors, warnings },
        error => panic!("{runner} creation failed unexpectedly: {error}"),
    }
}

/// Creation over values a runner holds, at the corpus's fixed document time.
fn creation(
    project: &ProjectSnapshot,
    packages: &package::Catalog,
    fonts: &[font::Entry],
    package_failures: &Failures,
) -> Result<create::Outcome, create::Error> {
    let mut request = create::Request::new(project.clone());
    request.packages = packages.clone();
    request.fonts = fonts.to_vec();
    request.package_failures = package_failures.clone();
    request.options.document_time = DocumentTime::UnixTimestamp(CREATION_TIMESTAMP);
    create(&request)
}

/// The failure kinds the corpus holds adapters to. Each reports in its own
/// vocabulary, and conformance is that both reach the same kind.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// One exact Package Read Failure retained for resumed creation.
    PackageRead {
        spec: String,
        reason: package::ReadFailure,
        rejection: Option<DependencyDiscoveryRejection>,
    },
    /// The representative request did not compile, so no Pack was issued.
    Compile(DependencyDiscoveryRejection),
}

// ---------------------------------------------------------------------------
// Direct core composition
// ---------------------------------------------------------------------------

/// Direct core composition from already selected source-neutral values.
fn create_directly(fixture: &Fixture) -> Result<Created, Failure> {
    let snapshot = ProjectSnapshot::new(
        fixture.entrypoint,
        fixture
            .project
            .iter()
            .map(|(path, data)| (*path, data.clone())),
    )
    .unwrap();
    let catalog = fixture.font_catalog();
    let mut package_failures = Failures::new();

    let mut resolved: Vec<(PackageSpec, package::Tree, bool)> = Vec::new();
    let mut missing_rounds = Vec::new();
    let mut failure_states = Vec::new();
    let mut terminal_package_failure = None;
    for _ in 0..RESUME_BOUND {
        // Every round builds the request afresh from the same values, as a
        // caller resuming across a host request boundary must.
        let packages = match catalog_from(resolved.iter().cloned()) {
            Ok(packages) => packages,
            Err(error) => {
                if error
                    .issues
                    .iter()
                    .any(|issue| matches!(issue, PackIssue::DuplicatePackage { .. }))
                {
                    panic!("the adapter resolved one specification twice");
                }
                let spec = resolved
                    .pop()
                    .map(|(spec, _, _)| spec.to_string())
                    .unwrap_or_default();
                let reason = package::ReadFailure::Other(None);
                package_failures.insert(spec.parse().unwrap(), reason.clone());
                failure_states.push(project_failures(&package_failures));
                terminal_package_failure = Some((spec, reason));
                package::Catalog::new()
            }
        };
        match creation(&snapshot, &packages, &catalog, &package_failures) {
            Ok(create::Outcome::Created(create::Created { pack, warnings, .. })) => {
                return Ok(Created {
                    pack,
                    warnings,
                    missing_rounds,
                    failure_states,
                });
            }
            Ok(create::Outcome::MissingPackages(missing)) => {
                assert!(
                    !missing.is_empty(),
                    "a missing outcome names a specification"
                );
                missing_rounds.push(missing.iter().map(ToString::to_string).collect());
                for spec in &missing {
                    let corrects_failure = fixture
                        .packages
                        .iter()
                        .find(|package| &package.spec == spec)
                        .is_some_and(|package| package.corrects_failure);
                    if corrects_failure {
                        package_failures.insert(spec.clone(), package::ReadFailure::NotFound);
                        failure_states.push(project_failures(&package_failures));
                    }
                    match fixture.resolve(spec) {
                        Ok(package) => resolved.push(package),
                        Err(Failure::PackageRead { spec, reason, .. }) => {
                            package_failures.insert(spec.parse().unwrap(), reason.clone());
                            failure_states.push(project_failures(&package_failures));
                            terminal_package_failure = Some((spec, reason));
                        }
                        Err(Failure::Compile(_)) => unreachable!(),
                    }
                    if corrects_failure {
                        // Pack Assembly owns this request value and reconstructs
                        // it without a corrected stale failure before resuming.
                        package_failures = Failures::new();
                        failure_states.push(project_failures(&package_failures));
                    }
                }
            }
            Err(error) => {
                let rejection = rejection(error, "core");
                return Err(match terminal_package_failure {
                    Some((spec, reason)) => Failure::PackageRead {
                        spec,
                        reason,
                        rejection: Some(rejection),
                    },
                    None => Failure::Compile(rejection),
                });
            }
        }
    }
    panic!("direct creation issued no Pack within {RESUME_BOUND} resume rounds");
}

fn project_failures(failures: &Failures) -> Vec<(String, package::ReadFailure)> {
    let mut failures = failures
        .iter()
        .map(|(spec, reason)| (spec.to_string(), reason.clone()))
        .collect::<Vec<_>>();
    failures.sort_by(|left, right| left.0.cmp(&right.0));
    failures
}

/// A catalog of every entry, failing at the first rejected one.
fn catalog_from(
    entries: impl IntoIterator<Item = (PackageSpec, package::Tree, bool)>,
) -> Result<package::Catalog, PackError> {
    let mut catalog = package::Catalog::new();
    for (spec, tree, embedded) in entries {
        catalog.insert(spec, tree, embedded)?;
    }
    Ok(catalog)
}

// ---------------------------------------------------------------------------
// The OpenDAL Memory Pack Assembler
// ---------------------------------------------------------------------------

/// Reads every Pack Assembly input through one real OpenDAL Memory backend,
/// then drives the existing synchronous Pack Creation resume protocol.
#[cfg(all(feature = "opendal", feature = "package-reading"))]
fn create_on_opendal(
    fixture: &Fixture,
    reverse_declaration_order: bool,
) -> Result<Created, Failure> {
    use opendal_faults::block_on;
    use typst_pack::ReadLimits;
    use typst_pack::opendal::{
        FontSource as OpenDalFontSource, insert_package, read_fonts, read_package, read_project,
    };

    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let mut objects = fixture
        .project
        .iter()
        .map(|(path, bytes)| (format!("project/{path}"), bytes.clone()))
        .collect::<Vec<_>>();
    let mut font_sources = Vec::new();

    for (source_index, font) in fixture.fonts.iter().enumerate() {
        let prefix = format!("fonts/{source_index:06}/");
        font_sources.push(OpenDalFontSource {
            prefix: prefix.clone(),
            embedded: font.disposition,
        });
        match &font.source {
            FontSource::Scanned(bytes) => {
                objects.push((format!("{prefix}000000.ttf"), bytes.clone()));
            }
            #[cfg(feature = "embedded-fonts")]
            FontSource::TypstEmbedded => {
                for (container_index, container) in typst_pack::font::embedded().enumerate() {
                    objects.push((
                        format!("{prefix}{container_index:06}.ttf"),
                        container.data().to_vec(),
                    ));
                }
            }
        }
    }

    for package in &fixture.packages {
        match &package.source {
            PackageSource::Files(files) => {
                let prefix = format!(
                    "trees/{}/{}/{}/",
                    package.spec.namespace, package.spec.name, package.spec.version
                );
                objects.extend(
                    files
                        .iter()
                        .map(|(path, bytes)| (format!("{prefix}{path}"), bytes.clone())),
                );
            }
            PackageSource::CachedArchive { bytes, .. } => objects.push((
                format!(
                    "cache/{}/{}/{}.tar.gz",
                    package.spec.namespace, package.spec.name, package.spec.version
                ),
                bytes.clone(),
            )),
            PackageSource::RegistryArchive { bytes, .. } => objects.push((
                format!(
                    "registry/{}/{}-{}.tar.gz",
                    package.spec.namespace, package.spec.name, package.spec.version
                ),
                bytes.clone(),
            )),
        }
    }
    if reverse_declaration_order {
        objects.reverse();
    }
    for (path, bytes) in objects {
        block_on(operator.write(&path, bytes)).unwrap();
    }

    let limits = ReadLimits::default();
    let snapshot = block_on(read_project(
        &operator,
        "project/",
        fixture.entrypoint,
        &limits,
    ))
    .unwrap();
    let fonts = block_on(read_fonts(&operator, &font_sources, &limits)).unwrap();
    let package_sources = [
        typst_pack::opendal::PackageSource::Trees {
            operator: operator.clone(),
            prefix: "trees/".into(),
        },
        typst_pack::opendal::PackageSource::ArchiveCache {
            operator: operator.clone(),
            prefix: "cache/".into(),
        },
        typst_pack::opendal::PackageSource::Registry {
            operator: operator.clone(),
            prefix: "registry/".into(),
        },
    ];

    let mut request = create::Request::new(snapshot);
    request.fonts = fonts;
    request.options.document_time = DocumentTime::UnixTimestamp(CREATION_TIMESTAMP);
    let mut missing_rounds = Vec::new();
    let mut failure_states = Vec::new();
    let mut terminal_package_failure = None;

    for _ in 0..RESUME_BOUND {
        match create(&request) {
            Ok(create::Outcome::Created(create::Created { pack, warnings, .. })) => {
                return Ok(Created {
                    pack,
                    warnings,
                    missing_rounds,
                    failure_states,
                });
            }
            Ok(create::Outcome::MissingPackages(missing)) => {
                missing_rounds.push(missing.iter().map(ToString::to_string).collect());
                let mut pending = missing.into_iter().collect::<Vec<_>>();
                if reverse_declaration_order {
                    pending.reverse();
                }
                for spec in pending {
                    let package = fixture
                        .packages
                        .iter()
                        .find(|package| package.spec == spec)
                        .unwrap_or_else(|| panic!("the fixture offers no source for `{spec}`"));
                    if package.corrects_failure {
                        request
                            .package_failures
                            .insert(spec.clone(), package::ReadFailure::NotFound);
                        failure_states.push(project_failures(&request.package_failures));
                    }
                    let read = block_on(read_package(&package_sources, &spec, &limits)).unwrap();
                    let expansion_limits = match package.source {
                        PackageSource::Files(_) => typst_pack::ReadLimits::default(),
                        PackageSource::CachedArchive { limits, .. }
                        | PackageSource::RegistryArchive { limits, .. } => limits,
                    };
                    if insert_package(
                        &mut request,
                        &spec,
                        read,
                        package.disposition,
                        expansion_limits,
                    )
                    .is_err()
                    {
                        failure_states.push(project_failures(&request.package_failures));
                        let reason = request.package_failures[&spec].clone();
                        terminal_package_failure = Some((spec.to_string(), reason));
                    }
                    if package.corrects_failure {
                        failure_states.push(project_failures(&request.package_failures));
                    }
                }
            }
            Err(error) => {
                let rejection = rejection(error, "core");
                return Err(match terminal_package_failure {
                    Some((spec, reason)) => Failure::PackageRead {
                        spec,
                        reason,
                        rejection: Some(rejection),
                    },
                    None => Failure::Compile(rejection),
                });
            }
        }
    }
    panic!("OpenDAL creation issued no Pack within {RESUME_BOUND} resume rounds");
}

// ---------------------------------------------------------------------------
// The reference filesystem Pack Assembler
// ---------------------------------------------------------------------------

/// How the reference adapter is configured to offer one fixture, or `None`
/// when it cannot offer it at all.
///
/// The adapter vendors every package tree under one choice, expands no archive
/// of its own, and composes its catalog as system fonts, then Typst's own
/// containers, then each scanned directory under one shared disposition. A
/// fixture that mixes package dispositions, supplies an archive, or puts
/// Typst's containers anywhere but first has no filesystem expression.
#[cfg(feature = "fs")]
struct FilesystemPlan {
    vendor_packages: bool,
    typst_embedded_fonts: bool,
    include_typst_embedded_fonts: bool,
    embed_fonts: bool,
    scanned_fonts: Vec<Vec<u8>>,
}

#[cfg(feature = "fs")]
impl Fixture {
    /// The disposition Typst's own containers carry, when the catalog opens
    /// with them, or `None` when it does not hold them at all. Fails outright
    /// when they appear anywhere but first, which the adapter cannot compose.
    #[cfg(feature = "embedded-fonts")]
    fn typst_embedded_disposition(&self) -> Option<Option<bool>> {
        let leading = match self.fonts.first() {
            Some(FontFixture {
                source: FontSource::TypstEmbedded,
                disposition,
            }) => Some(*disposition),
            _ => None,
        };
        let trailing = self
            .fonts
            .iter()
            .skip(1)
            .any(|font| matches!(font.source, FontSource::TypstEmbedded));
        (!trailing).then_some(leading)
    }

    /// Which a build shipping none of Typst's containers never holds.
    #[cfg(not(feature = "embedded-fonts"))]
    fn typst_embedded_disposition(&self) -> Option<Option<bool>> {
        Some(None)
    }

    fn filesystem_plan(&self) -> Option<FilesystemPlan> {
        let mut vendor_packages: Option<bool> = None;
        for package in &self.packages {
            match &package.source {
                PackageSource::Files(_) => {}
                #[cfg(feature = "package-reading")]
                PackageSource::CachedArchive { .. } | PackageSource::RegistryArchive { .. } => {
                    return None;
                }
            }
            let embedded = package.disposition;
            if *vendor_packages.get_or_insert(embedded) != embedded {
                return None;
            }
        }

        // The adapter offers Typst's own containers before every scanned one,
        // so a catalog holding them anywhere but first is inexpressible.
        let typst_embedded = self.typst_embedded_disposition()?;

        let mut scanned_fonts = Vec::new();
        let mut scanned_disposition: Option<bool> = None;
        for font in &self.fonts {
            let Some(data) = font.source.scanned() else {
                continue;
            };
            if *scanned_disposition.get_or_insert(font.disposition) != font.disposition {
                return None;
            }
            scanned_fonts.push(data.clone());
        }

        // Typst's containers are embedded only when scanned ones are, so a
        // catalog embedding Typst's own while referencing a scanned container
        // has no filesystem expression.
        let include_typst_embedded_fonts = typst_embedded == Some(true);
        let embed_fonts = scanned_disposition.unwrap_or(include_typst_embedded_fonts);
        if include_typst_embedded_fonts && !embed_fonts {
            return None;
        }
        let typst_embedded_fonts = typst_embedded.is_some();

        Some(FilesystemPlan {
            vendor_packages: vendor_packages.unwrap_or(true),
            typst_embedded_fonts,
            include_typst_embedded_fonts,
            embed_fonts,
            scanned_fonts,
        })
    }
}

/// Pack Assembly over a real project directory: the fixture's bytes are
/// written out as a project tree, a package path, and one directory per scanned
/// font container, and the reference adapter reads them from there.
#[cfg(feature = "fs")]
fn create_on_filesystem(fixture: &Fixture, plan: &FilesystemPlan) -> Result<Created, Failure> {
    let dir = tempfile::tempdir().expect("a temporary directory for the fixture");
    let project = dir.path().join("project");
    for (path, data) in &fixture.project {
        write_file(&project.join(path), data);
    }

    let packages = dir.path().join("packages");
    std::fs::create_dir_all(&packages).expect("the package path is created");
    for package in &fixture.packages {
        let files = package
            .source
            .files()
            .expect("a fixture with no filesystem expression has no filesystem plan");
        let spec = &package.spec;
        let root = packages.join(format!("{}/{}/{}", spec.namespace, spec.name, spec.version));
        for (path, data) in files {
            write_file(&root.join(path), data);
        }
    }

    let mut packer = fs::Packer {
        package_path: Some(packages.to_path_buf()),
        // No ambient source participates: the fixture's bytes are the catalog.
        system_fonts: false,
        typst_embedded_fonts: plan.typst_embedded_fonts,
        options: compile::Options {
            document_time: DocumentTime::UnixTimestamp(CREATION_TIMESTAMP),
            ..compile::Options::default()
        },
        vendor_packages: plan.vendor_packages,
        include_typst_embedded_fonts: plan.include_typst_embedded_fonts,
        embed_fonts: plan.embed_fonts,
        ..fs::Packer::default()
    };
    for (position, data) in plan.scanned_fonts.iter().enumerate() {
        let directory = dir.path().join(format!("fonts/{position}"));
        write_file(&directory.join(font_file_name(data)), data);
        packer.font_paths.push(directory);
    }

    match packer.pack(&project, fixture.entrypoint) {
        Ok(packed) => Ok(Created {
            pack: packed.pack,
            warnings: packed.warnings,
            missing_rounds: Vec::new(),
            failure_states: Vec::new(),
        }),
        Err(fs::Error::Create(error)) => {
            let rejection = rejection(error, "filesystem");
            // A tree the packer read but could not catalog fails the import
            // that needed it.
            let unloadable = rejection
                .errors
                .iter()
                .any(|error| error.message.starts_with("failed to load package"));
            Err(match fixture.packages.first() {
                Some(package) if unloadable => Failure::PackageRead {
                    spec: package.spec.to_string(),
                    reason: package::ReadFailure::Other(None),
                    rejection: Some(rejection),
                },
                _ => Failure::Compile(rejection),
            })
        }
        Err(error) => panic!("filesystem creation failed unexpectedly: {error}"),
    }
}

/// The name a scanned container is written under, which decides whether the
/// host's font scan reads it as a collection.
#[cfg(feature = "fs")]
fn font_file_name(data: &[u8]) -> &'static str {
    if data.starts_with(b"ttcf") {
        "font.ttc"
    } else {
        "font.ttf"
    }
}

#[cfg(feature = "fs")]
fn write_file(path: &std::path::Path, data: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the fixture's directories are created");
    }
    std::fs::write(path, data).expect("the fixture's bytes are written");
}

// ---------------------------------------------------------------------------
// Conformance
// ---------------------------------------------------------------------------

/// Which configured sources intentionally express one declarative scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScenarioApplicability {
    /// Direct values, OpenDAL, and the reference filesystem select equal inputs.
    Shared,
    /// Direct values and OpenDAL express source behavior the filesystem does not.
    ObjectStorage,
}

/// Runs one fixture through every applicable concrete scenario runner.
fn run(fixture: &Fixture, applicability: ScenarioApplicability) -> Result<Created, Failure> {
    let direct = create_directly(fixture);

    #[cfg(all(feature = "opendal", feature = "package-reading"))]
    {
        let opendal = create_on_opendal(fixture, false);
        let reordered = create_on_opendal(fixture, true);
        assert_results_equal(
            &opendal,
            &reordered,
            "OpenDAL declaration and Memory hash order changed observations",
            true,
        );
        assert_results_equal(
            &direct,
            &opendal,
            "direct and OpenDAL Pack Assembly produced different observations",
            true,
        );
    }

    #[cfg(feature = "fs")]
    {
        let plan = fixture.filesystem_plan();
        assert_eq!(
            plan.is_some(),
            applicability == ScenarioApplicability::Shared,
            "the fixture's declared adapter coverage is not what the reference adapter can express"
        );
        if let Some(plan) = plan {
            let filesystem = create_on_filesystem(fixture, &plan);
            assert_results_equal(
                &direct,
                &filesystem,
                "filesystem and direct Pack Assembly produced different terminal observations",
                false,
            );
        }
    }
    #[cfg(not(feature = "fs"))]
    let _ = applicability;

    direct
}

#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
fn assert_results_equal(
    expected: &Result<Created, Failure>,
    actual: &Result<Created, Failure>,
    context: &str,
    compare_resume_evidence: bool,
) {
    match (expected, actual) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(
                projection(&expected.pack),
                projection(&actual.pack),
                "{context}"
            );
            assert_eq!(expected.warnings, actual.warnings, "{context}");
            if compare_resume_evidence {
                assert_eq!(expected.missing_rounds, actual.missing_rounds, "{context}");
                assert_eq!(expected.failure_states, actual.failure_states, "{context}");
            }
        }
        (Err(expected), Err(actual)) if compare_resume_evidence => {
            assert_eq!(expected, actual, "{context}");
        }
        (
            Err(Failure::PackageRead {
                spec: expected_spec,
                reason: expected_reason,
                ..
            }),
            Err(Failure::PackageRead {
                spec: actual_spec,
                reason: actual_reason,
                ..
            }),
        ) => {
            assert_eq!(expected_spec, actual_spec, "{context}");
            assert_eq!(expected_reason, actual_reason, "{context}");
        }
        (Err(expected), Err(actual)) => assert_eq!(expected, actual, "{context}"),
        _ => panic!("{context}: one runner issued a Pack while the other failed"),
    }
}

/// Runs one fixture through every adapter and returns the Pack they agreed on.
fn conform(fixture: &Fixture) -> Created {
    run(fixture, ScenarioApplicability::Shared)
        .unwrap_or_else(|failure| panic!("creation issued no Pack: {failure:?}"))
}

/// Runs one fixture through every adapter and returns the failure kind they
/// agreed on.
fn conform_failure(fixture: &Fixture) -> Failure {
    run(fixture, ScenarioApplicability::Shared)
        .err()
        .expect("creation issued a Pack for a fixture that must fail")
}

fn assert_package_reading_failure(
    failure: Failure,
    expected_spec: &str,
    expected_reason: package::ReadFailure,
) {
    let Failure::PackageRead {
        spec,
        reason,
        rejection: Some(rejection),
    } = failure
    else {
        panic!("expected a Package Read Failure carried by Dependency Discovery");
    };
    assert_eq!(spec, expected_spec);
    assert_eq!(reason, expected_reason);
    assert!(
        rejection
            .errors
            .iter()
            .any(|diagnostic| diagnostic.message.starts_with("failed to load package")),
        "{:#?}",
        rejection.errors
    );
}

/// Runs a source-neutral fixture whose source behavior the filesystem adapter
/// cannot express.
fn conform_object_storage(fixture: &Fixture) -> Created {
    run(fixture, ScenarioApplicability::ObjectStorage)
        .unwrap_or_else(|failure| panic!("creation issued no Pack: {failure:?}"))
}

/// Runs an object-storage fixture that must fail, returning its failure kind.
#[cfg(feature = "package-reading")]
fn conform_failure_object_storage(fixture: &Fixture) -> Failure {
    run(fixture, ScenarioApplicability::ObjectStorage)
        .err()
        .expect("creation issued a Pack for a fixture that must fail")
}

/// Everything about a Pack the corpus asserts on, as one comparable value.
#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
type ProjectFiles = Vec<(String, Vec<u8>)>;
#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
type EmbeddedPackages = Vec<(String, ProjectFiles)>;

#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
#[derive(Debug, PartialEq)]
struct Projection {
    identity: Digest,
    entrypoint: String,
    metadata: Option<typst_pack::Metadata>,
    project: ProjectFiles,
    embedded_packages: EmbeddedPackages,
    packages: Vec<(String, Digest, bool)>,
    embedded_fonts: Vec<(Digest, Vec<u8>)>,
    font_requirements: Vec<typst_pack::font::Requirement>,
}

#[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
fn projection(pack: &Pack) -> Projection {
    Projection {
        identity: pack.identity(),
        entrypoint: pack.entrypoint().to_owned(),
        metadata: pack.metadata().cloned(),
        project: pack
            .files()
            .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
            .collect(),
        embedded_packages: pack
            .packages()
            .map(|(spec, tree)| {
                (
                    spec.to_string(),
                    tree.files()
                        .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                        .collect(),
                )
            })
            .collect(),
        packages: pack
            .package_requirements()
            .iter()
            .map(|requirement| {
                (
                    requirement.spec.to_string(),
                    requirement.digest,
                    requirement.embedded,
                )
            })
            .collect(),
        embedded_fonts: pack
            .font_requirements()
            .iter()
            .filter_map(|requirement| {
                let data = pack.font_data(&requirement.digest)?;
                Some((requirement.digest, data.to_vec()))
            })
            .collect(),
        font_requirements: pack.font_requirements().to_vec(),
    }
}

/// The contained project paths of a Pack, in canonical order.
fn project_paths(pack: &Pack) -> Vec<&str> {
    pack.files().map(|(path, _)| path).collect()
}

/// Each Package Requirement and whether its tree travels in the Pack, in
/// canonical specification order.
fn package_dispositions(pack: &Pack) -> Vec<(String, bool)> {
    pack.package_requirements()
        .iter()
        .map(|requirement| (requirement.spec.to_string(), requirement.embedded))
        .collect()
}

/// Every face available at compile time: its container, its container-local
/// index, and whether that container's bytes travel in the Pack, in font book
/// order.
#[cfg(feature = "embedded-fonts")]
fn font_faces(pack: &Pack) -> Vec<(Digest, u32, bool)> {
    pack.font_requirements()
        .iter()
        .flat_map(|requirement| {
            requirement
                .faces
                .iter()
                .map(|face| (requirement.digest, face.index, requirement.embedded))
        })
        .collect()
}

/// Each Font Requirement's container and whether its bytes travel in the Pack.
fn font_requirements(pack: &Pack) -> Vec<(Digest, bool)> {
    pack.font_requirements()
        .iter()
        .map(|requirement| (requirement.digest, requirement.embedded))
        .collect()
}

// ---------------------------------------------------------------------------
// Packages
// ---------------------------------------------------------------------------

#[test]
fn a_project_with_no_packages_requires_none() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .file("data/notes.txt", b"unread".to_vec());

    let created = conform(&fixture);

    // Project files come from the snapshot, never from compiler observations.
    assert_eq!(project_paths(&created.pack), ["data/notes.txt", "main.typ"]);
    assert!(package_dispositions(&created.pack).is_empty());
    assert!(font_requirements(&created.pack).is_empty());
}

#[test]
fn a_project_requiring_several_packages_completes_over_repeated_invocation() {
    // `outer` needs `inner`, which only `outer`'s own tree imports, so no round
    // can report both at once.
    let fixture = Fixture::document(
        "#import \"@local/outer:1.0.0\": mark\n#rect(width: mark * 1pt, height: 1pt)",
    )
    .corrected_package(
        "outer",
        true,
        "#import \"@local/inner:1.0.0\": inner\n#let mark = inner + 1",
    )
    .package("inner", true, "#let inner = 2");

    let created = conform(&fixture);

    assert_eq!(project_paths(&created.pack), ["main.typ"]);
    assert_eq!(
        package_dispositions(&created.pack),
        [
            ("@local/inner:1.0.0".to_owned(), true),
            ("@local/outer:1.0.0".to_owned(), true),
        ]
    );
    // The whole Package Tree travels, not only what was read.
    assert!(
        created
            .pack
            .packages()
            .find(|(s, _)| *s == &local_spec("outer"))
            .and_then(|(_, t)| t.file("unread.txt"))
            .is_some()
    );
    assert_eq!(
        created.missing_rounds,
        [
            vec!["@local/outer:1.0.0".to_owned()],
            vec!["@local/inner:1.0.0".to_owned()],
        ]
    );
    assert_eq!(
        created.failure_states,
        [
            vec![(
                "@local/outer:1.0.0".to_owned(),
                package::ReadFailure::NotFound,
            )],
            vec![],
        ]
    );
}

/// The reference adapter vendors every tree under one choice, so this scenario
/// is intentionally direct-and-OpenDAL only.
#[test]
fn mixed_package_dispositions_travel_per_tree() {
    let fixture = Fixture::document(
        "#import \"@local/embedded:1.0.0\": one\n\
         #import \"@local/external:1.0.0\": two\n\
         #rect(width: (one + two) * 1pt, height: 1pt)",
    )
    .package("embedded", true, "#let one = 1")
    .package("external", false, "#let two = 2");

    let created = conform_object_storage(&fixture);

    assert_eq!(
        package_dispositions(&created.pack),
        [
            ("@local/embedded:1.0.0".to_owned(), true),
            ("@local/external:1.0.0".to_owned(), false),
        ]
    );
    assert!(
        created
            .pack
            .packages()
            .any(|(s, _)| s == &local_spec("embedded"))
    );
    assert!(
        !created
            .pack
            .packages()
            .any(|(s, _)| s == &local_spec("external"))
    );
}

#[test]
fn a_supplied_tree_that_does_not_satisfy_its_specification_fails_every_adapter() {
    let fixture = Fixture::document("#import \"@local/declared:1.0.0\": value\n#value")
        .supplied_package("declared", package_files("other", "#let value = 1"), true);

    // A diagnosis rather than a loop that never progresses: a caller told the
    // same specification is missing forever would have nothing to act on.
    assert_package_reading_failure(
        conform_failure(&fixture),
        "@local/declared:1.0.0",
        package::ReadFailure::Other(None),
    );
}

/// The expansion ceiling is a value only a caller supplying its own transport
/// chooses, so this scenario is intentionally direct-and-OpenDAL only.
#[cfg(feature = "package-reading")]
#[test]
fn an_archive_beyond_the_expansion_ceiling_fails_the_resume_loop() {
    // A hundred and twenty-eight megabytes of zeros in a few kilobytes of
    // archive, against a four-kilobyte ceiling. The loop is told what it may
    // not expand, so it stops with a diagnosis rather than exhausting the
    // process it runs in.
    let fixture = Fixture::document("#import \"@preview/oversized:1.0.0\": value\n#value")
        .registry_archive(
            "oversized",
            archive(&[("lib.typ", 128 * 1024 * 1024, b"#let value = 1")]),
            typst_pack::ReadLimits {
                max_entries: 10,
                max_file_bytes: 4096,
                max_total_bytes: 4096,
            },
        );

    assert_package_reading_failure(
        conform_failure_object_storage(&fixture),
        "@preview/oversized:1.0.0",
        package::ReadFailure::Other(None),
    );
}

#[cfg(feature = "package-reading")]
#[test]
fn cache_and_registry_archives_compose_into_the_same_semantic_pack() {
    let fixture = Fixture::document(
        "#import \"@preview/cached:1.0.0\": cached\n\
         #import \"@preview/registered:1.0.0\": registered\n\
         #rect(width: (cached + registered) * 1pt, height: 1pt)",
    )
    .cached_archive(
        "cached",
        valid_archive("cached", "#let cached = 1"),
        typst_pack::ReadLimits::default(),
    )
    .registry_archive(
        "registered",
        valid_archive("registered", "#let registered = 2"),
        typst_pack::ReadLimits::default(),
    );

    let created = conform_object_storage(&fixture);

    assert_eq!(
        package_dispositions(&created.pack),
        [
            ("@preview/cached:1.0.0".to_owned(), true),
            ("@preview/registered:1.0.0".to_owned(), true),
        ]
    );
    assert_eq!(
        created.missing_rounds,
        [
            vec!["@preview/cached:1.0.0".to_owned()],
            vec!["@preview/registered:1.0.0".to_owned()],
        ]
    );
}

#[cfg(feature = "package-reading")]
fn valid_archive(name: &str, body: &str) -> Vec<u8> {
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"{PACKAGE_VERSION}\"\nentrypoint = \"lib.typ\"\n"
    );
    archive(&[
        ("typst.toml", manifest.len() as u64, manifest.as_bytes()),
        ("lib.typ", body.len() as u64, body.as_bytes()),
        ("unread.txt", 8, b"included"),
    ])
}

/// The gzip-compressed tar a registry serves for one package, written from a
/// member's nominal size so that the archive stays small whatever it claims to
/// expand to.
#[cfg(feature = "package-reading")]
fn archive(members: &[(&str, u64, &[u8])]) -> Vec<u8> {
    use std::io::{Read, Write};

    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    for (path, size, data) in members {
        let mut header = tar::Header::new_gnu();
        header.set_size(*size);
        header.set_mode(0o644);
        let padding = std::io::repeat(0).take(size.saturating_sub(data.len() as u64));
        builder
            .append_data(&mut header, path, data.chain(padding))
            .expect("the fixture archive is written");
    }
    let mut encoder = builder.into_inner().expect("the archive is finished");
    encoder.flush().expect("the archive is flushed");
    encoder.finish().expect("the archive is compressed")
}

#[test]
fn a_representative_request_that_does_not_compile_fails_every_adapter() {
    let fixture = Fixture::document("#import \"missing.typ\": value\n#value");

    assert!(matches!(conform_failure(&fixture), Failure::Compile(_)));
}

#[test]
fn representative_compile_warnings_are_returned_by_every_adapter() {
    let fixture = Fixture::document("#set text(font: \"Definitely Missing\")\nWarning\n");

    let created = conform(&fixture);

    assert!(
        created
            .warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{:?}",
        created.warnings
    );
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

/// Face selection out of catalogs holding real font bytes, which Typst only
/// ships with the `embedded-fonts` feature.
#[cfg(feature = "embedded-fonts")]
mod fonts {

    use typst_pack::Digest;

    use crate::font_bytes::{family_of, font_collection, renamed_family, typst_container};
    use crate::{Fixture, FontSource, conform, font_faces, font_requirements};

    /// A container offering the given family and nothing else, renamed out of
    /// one Typst ships so that its bytes are a real font.
    fn container_for(family: &str) -> Vec<u8> {
        let font = typst_container();
        let original = family_of(&font);
        renamed_family(&font, &original, family)
    }

    /// A family name of the same length as the one Typst's first container
    /// offers, so that renaming keeps every name record's offset.
    fn alternate_family(prefix: char) -> String {
        let original = family_of(&typst_container());
        format!("{prefix}{}", &original[1..])
    }

    #[test]
    fn every_selected_face_of_a_multi_face_collection_reaches_the_catalog() {
        let default_family = family_of(&typst_container());
        let second_family = alternate_family('Z');
        let container = font_collection(&[typst_container(), container_for(&second_family)]);
        let fixture = Fixture::document(&format!(
            "#set text(font: \"{default_family}\")\nFirst face\n\n\
             #text(font: \"{second_family}\")[Second face]\n"
        ))
        .font(FontSource::Scanned(container.clone()), true);

        let created = conform(&fixture);

        // Every face of a collection travels in the same container, so the
        // selected faces are one Font Requirement offering every face.
        let identity = Digest::of(&container);
        assert_eq!(font_requirements(&created.pack), [(identity, true)]);
        assert_eq!(
            font_faces(&created.pack),
            [(identity, 0, true), (identity, 1, true)]
        );
    }

    #[test]
    fn catalog_order_decides_which_face_wins() {
        let family = family_of(&typst_container());
        let first = typst_container();
        // Same family and variant, distinct bytes: only catalog order can
        // decide between them.
        let mut second = first.clone();
        second.push(0);

        let selected = |containers: [&Vec<u8>; 2]| {
            let mut fixture =
                Fixture::document(&format!("#set text(font: \"{family}\")\nSelected\n"));
            for container in containers {
                fixture = fixture.font(FontSource::Scanned(container.clone()), true);
            }
            font_requirements(&conform(&fixture).pack)
        };

        assert_eq!(selected([&first, &second]), [(Digest::of(&first), true)]);
        assert_eq!(selected([&second, &first]), [(Digest::of(&second), true)]);
    }

    #[test]
    fn mixed_font_dispositions_travel_per_container() {
        let typst_font = typst_container();
        let scanned_family = alternate_family('Z');
        let scanned = container_for(&scanned_family);
        let fixture = Fixture::document(&format!(
            "Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]\n"
        ))
        .font(FontSource::TypstEmbedded, false)
        .font(FontSource::Scanned(scanned.clone()), true);

        let created = conform(&fixture);

        // One Pack references a container it may not redistribute and embeds
        // one it may, and the Pack Font Catalog keeps candidate order.
        let typst_identity = Digest::of(&typst_font);
        let scanned_identity = Digest::of(&scanned);
        assert_eq!(
            font_faces(&created.pack),
            [(typst_identity, 0, false), (scanned_identity, 0, true),]
        );
        assert_eq!(
            font_requirements(&created.pack),
            [(typst_identity, false), (scanned_identity, true)]
        );
    }
}
