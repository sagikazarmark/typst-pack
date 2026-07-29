//! The cross-adapter creation conformance corpus.
//!
//! One corpus of fixtures expressed as bytes — project files, ignore files,
//! package trees, font containers — driven through two Creation Adapters: the
//! reference filesystem one over a temporary directory, and an in-memory one
//! that assembles a Project Snapshot directly and drives the resume protocol.
//! Both must issue a Pack with the same Pack Identity, which is the property
//! ADR-0008 promises: identity stays free of host facts whichever adapter
//! obtained the bytes. That equality is deliberately strict, so an adapter that
//! diverges is caught here rather than in production.
//!
//! Every assertion stays on the public library surface: contained project
//! paths, requirement dispositions, Pack Font Catalog order, Pack Identity,
//! representative-compile warnings, and the typed outcomes and failures.
//! Nothing here observes loader structure, store internals, how many resume
//! rounds a fixture took, or the order in which files were requested.
//!
//! A fixture the reference adapter cannot express runs through
//! [`conform_in_memory`] instead, which asserts that the reference adapter
//! really cannot express it, so a fixture never loses cross-adapter coverage
//! silently.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CandidateFontCatalog, CandidateFontContainer, CreationError, CreationOutcome, CreationRequest,
    FontContainerIdentity, FontDisposition, IGNORE_FILE, Pack, PackageDisposition,
    ProjectIgnorePolicy, ProjectSnapshotAssembly, ProjectSnapshotError, ResolvedPackageTree,
    create,
};

/// 2023-11-14T22:13:20Z, the Document Time every representative request in the
/// corpus is fixed to, so that no adapter's host clock reaches a Pack.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// The version every fixture package is supplied under.
const PACKAGE_VERSION: &str = "1.0.0";

/// How many resume rounds a fixture may take before the corpus calls the loop
/// stuck. The number of rounds one actually takes is not asserted.
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

/// One Complete Package Tree an adapter may resolve for the fixture.
struct PackageFixture {
    spec: PackageSpec,
    source: PackageSource,
    disposition: PackageDisposition,
}

/// How an adapter obtains the tree for one specification.
enum PackageSource {
    /// The tree's files, as bytes: what a package directory holds, and what an
    /// in-memory host supplies directly.
    Files(Vec<(&'static str, Vec<u8>)>),
    /// The archive a registry serves, which a caller supplying its own
    /// transport fetches and expands under a required ceiling. The reference
    /// adapter never expands one, so a fixture offering it is driven in memory
    /// alone.
    #[cfg(feature = "package-acquisition")]
    Archive {
        bytes: Vec<u8>,
        ceiling: typst_pack::PackageExpansionCeiling,
    },
}

/// One candidate Font Container the fixture offers, at the catalog position it
/// is declared in.
struct FontFixture {
    source: FontSource,
    disposition: FontDisposition,
}

/// Where a fixture's candidate Font Container comes from.
enum FontSource {
    /// Exact container bytes, which the filesystem adapter offers by scanning a
    /// directory holding them and nothing else.
    Scanned(Vec<u8>),
    /// Every container Typst embeds, in Typst's own order, which the filesystem
    /// adapter offers from its own copy rather than from a scanned directory.
    #[cfg(feature = "embedded-fonts")]
    TypstEmbedded,
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

    /// Adds the root Project Ignore Policy file, as bytes.
    fn ignore_file(self, rules: &str) -> Self {
        self.file(IGNORE_FILE, rules.as_bytes().to_vec())
    }

    /// Offers a `@local` package tree whose `lib.typ` holds `body`, plus a file
    /// the representative request never reads, so that whole-tree containment
    /// is observable.
    fn package(self, name: &'static str, disposition: PackageDisposition, body: &str) -> Self {
        self.supplied_package(name, package_files(name, body), disposition)
    }

    /// Offers a `@local` package tree exactly as given, for a fixture about
    /// what a tree declares rather than what it holds.
    fn supplied_package(
        mut self,
        name: &'static str,
        files: Vec<(&'static str, Vec<u8>)>,
        disposition: PackageDisposition,
    ) -> Self {
        self.packages.push(PackageFixture {
            spec: local_spec(name),
            source: PackageSource::Files(files),
            disposition,
        });
        self
    }

    /// Offers the archive the registry serves for a `@preview` specification,
    /// which a caller with its own transport fetches at the registry URL and
    /// expands under the given ceiling.
    #[cfg(feature = "package-acquisition")]
    fn registry_archive(
        mut self,
        name: &'static str,
        bytes: Vec<u8>,
        ceiling: typst_pack::PackageExpansionCeiling,
    ) -> Self {
        self.packages.push(PackageFixture {
            spec: registry_spec(name),
            source: PackageSource::Archive { bytes, ceiling },
            disposition: PackageDisposition::Embedded,
        });
        self
    }

    /// Offers one candidate Font Container at the end of the catalog.
    fn font(mut self, source: FontSource, disposition: FontDisposition) -> Self {
        self.fonts.push(FontFixture {
            source,
            disposition,
        });
        self
    }

    /// The Project Ignore Policy an adapter derives from the fixture's listing,
    /// which is the root ignore file's bytes and nothing else.
    fn policy(&self) -> ProjectIgnorePolicy {
        match self.project.iter().find(|(path, _)| *path == IGNORE_FILE) {
            Some((_, rules)) => ProjectIgnorePolicy::from_ignore_file(rules)
                .expect("the fixture's ignore file parses"),
            None => ProjectIgnorePolicy::built_in(),
        }
    }

    /// The Candidate Font Catalog the fixture's containers compose, in the
    /// order they were declared.
    fn candidate_catalog(&self) -> CandidateFontCatalog {
        let mut catalog = CandidateFontCatalog::new();
        for font in &self.fonts {
            match &font.source {
                FontSource::Scanned(data) => {
                    catalog.push(CandidateFontContainer::new(data.clone(), font.disposition));
                }
                #[cfg(feature = "embedded-fonts")]
                FontSource::TypstEmbedded => {
                    catalog.extend(typst_pack::typst_embedded_font_containers(font.disposition));
                }
            }
        }
        catalog
    }

    /// The tree an in-memory host resolves for one reported specification,
    /// standing in for whatever acquisition that host allows.
    fn resolve(&self, spec: &PackageSpec) -> Result<ResolvedPackageTree, Failure> {
        let package = self
            .packages
            .iter()
            .find(|package| &package.spec == spec)
            .unwrap_or_else(|| panic!("the fixture offers no tree for `{spec}`"));
        match &package.source {
            PackageSource::Files(files) => Ok(ResolvedPackageTree::new(
                spec.clone(),
                files.iter().map(|(path, data)| (*path, data.clone())),
                package.disposition,
            )),
            #[cfg(feature = "package-acquisition")]
            PackageSource::Archive { ceiling, .. } => {
                // The core names where the registry serves the specification,
                // the host fetches it with whatever primitive it has, and
                // expansion needs no transport at all.
                let url = typst_pack::package_archive_url(spec)
                    .expect("the registry serves the reported specification");
                let bytes = self
                    .serve(&url)
                    .expect("the stand-in registry serves that URL");
                typst_pack::expand_package_archive(
                    spec.clone(),
                    bytes,
                    package.disposition,
                    *ceiling,
                )
                .map_err(|error| match error {
                    typst_pack::PackageAcquisitionError::ExpansionCeilingExceeded { .. } => {
                        Failure::ExpansionCeiling
                    }
                    error => panic!("the fixture archive failed to expand: {error}"),
                })
            }
        }
    }

    /// The archive the fixture's stand-in registry serves at one URL.
    #[cfg(feature = "package-acquisition")]
    fn serve(&self, url: &str) -> Option<&[u8]> {
        self.packages
            .iter()
            .find_map(|package| match &package.source {
                PackageSource::Archive { bytes, .. }
                    if typst_pack::package_archive_url(&package.spec)
                        .ok()
                        .as_deref()
                        == Some(url) =>
                {
                    Some(bytes.as_slice())
                }
                _ => None,
            })
    }
}

/// The `@local` specification a fixture package is supplied under.
fn local_spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:{PACKAGE_VERSION}"))
        .expect("the fixture specification is well formed")
}

/// The `@preview` specification a fixture package the registry serves is
/// supplied under, which is the only namespace with a registry URL.
#[cfg(feature = "package-acquisition")]
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
        (
            "unread.txt",
            b"the whole Complete Package Tree travels".to_vec(),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Adapter results
// ---------------------------------------------------------------------------

/// What one Creation Adapter produced for a fixture.
struct Created {
    pack: Pack,
    warnings: Vec<String>,
}

/// The failure kinds the corpus holds adapters to. Each reports in its own
/// vocabulary, and conformance is that both reach the same kind.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// The Project Ignore Policy excludes the entrypoint.
    ExcludedEntrypoint,
    /// No tree satisfying a reported specification could be supplied. The
    /// filesystem adapter reports every package acquisition failure in one
    /// vocabulary, so a tree that does not declare the specification it was
    /// supplied under reaches this kind there too.
    UnsatisfiedPackage,
    /// An archive claims to expand past the ceiling it was expanded under.
    #[cfg(feature = "package-acquisition")]
    ExpansionCeiling,
    /// The representative request did not compile, so no Pack was issued.
    Compile,
}

// ---------------------------------------------------------------------------
// The in-memory Creation Adapter
// ---------------------------------------------------------------------------

/// Creation Preparation for a host with no filesystem: the fixture's bytes are
/// already held, so the adapter derives the policy from them, assembles the
/// Project Snapshot, composes the catalog, and drives the resume protocol.
fn create_in_memory(fixture: &Fixture) -> Result<Created, Failure> {
    let policy = fixture.policy();
    let snapshot = ProjectSnapshotAssembly::new(fixture.entrypoint, &policy)
        .assemble(
            fixture
                .project
                .iter()
                .map(|(path, data)| (*path, data.clone())),
        )
        .map_err(|error| match error {
            ProjectSnapshotError::ExcludedEntrypoint(_) => Failure::ExcludedEntrypoint,
            error => panic!("in-memory snapshot assembly failed unexpectedly: {error}"),
        })?;
    let catalog = fixture.candidate_catalog();

    let mut resolved: Vec<ResolvedPackageTree> = Vec::new();
    for _ in 0..RESUME_BOUND {
        // Every round builds the request afresh from the same values, as a
        // caller resuming across a host request boundary must.
        let request = CreationRequest::new(snapshot.clone(), CREATION_TIMESTAMP)
            .font_catalog(catalog.clone())
            .package_trees(resolved.iter().cloned());
        match create(&request) {
            Ok(CreationOutcome::Issued(issued)) => {
                return Ok(Created {
                    pack: issued.pack,
                    warnings: messages(&issued.warnings),
                });
            }
            Ok(CreationOutcome::MissingPackages(missing)) => {
                assert!(
                    !missing.is_empty(),
                    "a missing outcome names a specification"
                );
                for spec in &missing {
                    resolved.push(fixture.resolve(spec)?);
                }
            }
            Err(CreationError::Compile { .. }) => return Err(Failure::Compile),
            Err(CreationError::MismatchedPackageTree { .. }) => {
                return Err(Failure::UnsatisfiedPackage);
            }
            Err(error) => panic!("in-memory creation failed unexpectedly: {error}"),
        }
    }
    panic!("in-memory creation issued no Pack within {RESUME_BOUND} resume rounds");
}

/// The messages of representative-compile warnings, which is as much of a
/// diagnostic as the corpus compares.
fn messages(warnings: &[typst::diag::SourceDiagnostic]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| warning.message.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// The reference filesystem Creation Adapter
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
    fn typst_embedded_disposition(&self) -> Option<Option<FontDisposition>> {
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
    fn typst_embedded_disposition(&self) -> Option<Option<FontDisposition>> {
        Some(None)
    }

    fn filesystem_plan(&self) -> Option<FilesystemPlan> {
        let mut vendor_packages: Option<bool> = None;
        for package in &self.packages {
            match &package.source {
                PackageSource::Files(_) => {}
                #[cfg(feature = "package-acquisition")]
                PackageSource::Archive { .. } => return None,
            }
            let embedded = package.disposition.is_embedded();
            if *vendor_packages.get_or_insert(embedded) != embedded {
                return None;
            }
        }

        // The adapter offers Typst's own containers before every scanned one,
        // so a catalog holding them anywhere but first is inexpressible.
        let typst_embedded = self.typst_embedded_disposition()?;

        let mut scanned_fonts = Vec::new();
        let mut scanned_disposition: Option<FontDisposition> = None;
        for font in &self.fonts {
            let data = match &font.source {
                FontSource::Scanned(data) => data,
                #[allow(unreachable_patterns)]
                _ => continue,
            };
            if *scanned_disposition.get_or_insert(font.disposition) != font.disposition {
                return None;
            }
            scanned_fonts.push(data.clone());
        }

        // Typst's containers are embedded only when scanned ones are, so a
        // catalog embedding Typst's own while referencing a scanned container
        // has no filesystem expression.
        let include_typst_embedded_fonts = typst_embedded.is_some_and(FontDisposition::is_embedded);
        let embed_fonts =
            scanned_disposition.map_or(include_typst_embedded_fonts, FontDisposition::is_embedded);
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

/// Creation Preparation over a real project directory: the fixture's bytes are
/// written out as a project tree, a package path, and one directory per scanned
/// font container, and the reference adapter acquires them from there.
#[cfg(feature = "fs")]
fn create_on_filesystem(fixture: &Fixture, plan: &FilesystemPlan) -> Result<Created, Failure> {
    use typst_pack::{Packer, PackerError};

    let dir = tempfile::tempdir().expect("a temporary directory for the fixture");
    let project = dir.path().join("project");
    for (path, data) in &fixture.project {
        write_file(&project.join(path), data);
    }

    let packages = dir.path().join("packages");
    std::fs::create_dir_all(&packages).expect("the package path is created");
    for package in &fixture.packages {
        let files = match &package.source {
            PackageSource::Files(files) => files,
            #[allow(unreachable_patterns)]
            _ => unreachable!("a fixture with no filesystem expression has no filesystem plan"),
        };
        let spec = &package.spec;
        let root = packages.join(format!("{}/{}/{}", spec.namespace, spec.name, spec.version));
        for (path, data) in files {
            write_file(&root.join(path), data);
        }
    }

    let mut packer = Packer::new(&project, fixture.entrypoint)
        .package_path(&packages)
        .creation_timestamp(Some(CREATION_TIMESTAMP))
        .vendor_packages(plan.vendor_packages)
        // No ambient source participates: the fixture's bytes are the catalog.
        .system_fonts(false)
        .typst_embedded_fonts(plan.typst_embedded_fonts)
        .include_typst_embedded_fonts(plan.include_typst_embedded_fonts)
        .embed_fonts(plan.embed_fonts);
    for (position, data) in plan.scanned_fonts.iter().enumerate() {
        let directory = dir.path().join(format!("fonts/{position}"));
        write_file(&directory.join(font_file_name(data)), data);
        packer = packer.font_path(&directory);
    }

    match packer.pack() {
        Ok(outcome) => Ok(Created {
            pack: outcome.pack,
            warnings: messages(&outcome.warnings),
        }),
        Err(PackerError::IgnoredEntrypoint(_)) => Err(Failure::ExcludedEntrypoint),
        Err(PackerError::Package { .. }) => Err(Failure::UnsatisfiedPackage),
        Err(PackerError::Compile { .. }) => Err(Failure::Compile),
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

/// Runs one fixture through the in-memory adapter, and through the reference
/// filesystem adapter when `filesystem` says the corpus expects that one to
/// express it.
///
/// Whether the reference adapter can express a fixture is declared by which
/// entry point the fixture uses and asserted here rather than inferred, so a
/// fixture never loses cross-adapter coverage silently.
///
/// Pack Identity equality is the strict assertion; the projection compared
/// first exists so that a divergence names what differs instead of two hashes.
fn run(fixture: &Fixture, filesystem: bool) -> Result<Created, Failure> {
    let memory = create_in_memory(fixture);

    #[cfg(feature = "fs")]
    {
        let plan = fixture.filesystem_plan();
        assert_eq!(
            plan.is_some(),
            filesystem,
            "the fixture's declared adapter coverage is not what the reference adapter can express"
        );
        if let Some(plan) = plan {
            let filesystem = create_on_filesystem(fixture, &plan);
            match (&filesystem, &memory) {
                (Ok(filesystem), Ok(memory)) => {
                    assert_eq!(
                        projection(&filesystem.pack),
                        projection(&memory.pack),
                        "the filesystem and in-memory adapters describe different Packs"
                    );
                    assert_eq!(
                        filesystem.warnings, memory.warnings,
                        "the adapters returned different representative-compile warnings"
                    );
                    assert_eq!(
                        filesystem.pack.identity(),
                        memory.pack.identity(),
                        "identical project bytes produced different Pack Identities"
                    );
                }
                (Err(filesystem), Err(memory)) => assert_eq!(
                    filesystem, memory,
                    "the adapters failed the same fixture differently"
                ),
                _ => panic!("one adapter issued a Pack for a fixture the other failed"),
            }
        }
    }
    #[cfg(not(feature = "fs"))]
    let _ = filesystem;

    memory
}

/// Runs one fixture through every adapter and returns the Pack they agreed on.
fn conform(fixture: &Fixture) -> Created {
    run(fixture, true).unwrap_or_else(|failure| panic!("creation issued no Pack: {failure:?}"))
}

/// Runs one fixture through every adapter and returns the failure kind they
/// agreed on.
fn conform_failure(fixture: &Fixture) -> Failure {
    run(fixture, true)
        .err()
        .expect("creation issued a Pack for a fixture that must fail")
}

/// Runs a fixture the reference adapter cannot express through the in-memory
/// adapter alone.
fn conform_in_memory(fixture: &Fixture) -> Created {
    run(fixture, false).unwrap_or_else(|failure| panic!("creation issued no Pack: {failure:?}"))
}

/// Runs a fixture the reference adapter cannot express through the in-memory
/// adapter alone, returning the failure kind it reached.
#[cfg(feature = "package-acquisition")]
fn conform_failure_in_memory(fixture: &Fixture) -> Failure {
    run(fixture, false)
        .err()
        .expect("creation issued a Pack for a fixture that must fail")
}

/// Everything about a Pack the corpus asserts on, as one comparable value.
#[cfg(feature = "fs")]
#[derive(Debug, PartialEq, Eq)]
struct Projection {
    entrypoint: String,
    project_paths: Vec<String>,
    packages: Vec<(String, bool)>,
    font_catalog: Vec<(FontContainerIdentity, u32, bool)>,
    font_requirements: Vec<(FontContainerIdentity, bool)>,
}

#[cfg(feature = "fs")]
fn projection(pack: &Pack) -> Projection {
    Projection {
        entrypoint: pack.entrypoint().to_owned(),
        project_paths: project_paths(pack).into_iter().map(str::to_owned).collect(),
        packages: package_dispositions(pack),
        font_catalog: font_catalog(pack),
        font_requirements: font_requirements(pack),
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
        .map(|requirement| (requirement.spec().to_string(), requirement.is_embedded()))
        .collect()
}

/// The Pack Font Catalog: each face's container, its container-local index, and
/// whether that container's bytes travel in the Pack, in catalog order.
fn font_catalog(pack: &Pack) -> Vec<(FontContainerIdentity, u32, bool)> {
    pack.font_catalog()
        .iter()
        .map(|face| {
            (
                face.identity().container(),
                face.identity().index(),
                face.is_embedded(),
            )
        })
        .collect()
}

/// Each Font Requirement's container and whether its bytes travel in the Pack.
fn font_requirements(pack: &Pack) -> Vec<(FontContainerIdentity, bool)> {
    pack.font_requirements()
        .iter()
        .map(|requirement| (requirement.container_identity(), requirement.is_embedded()))
        .collect()
}

// ---------------------------------------------------------------------------
// Project membership
// ---------------------------------------------------------------------------

#[test]
fn an_ignore_file_with_negation_and_last_match_precedence_decides_membership() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .ignore_file(
            "ignored/**\n\
             !ignored/reincluded/\n\
             !ignored/reincluded/keep.txt\n\
             *.secret\n\
             !keep.secret\n",
        )
        .file("ignored/drop.txt", b"drop".to_vec())
        .file("ignored/reincluded/keep.txt", b"keep".to_vec())
        .file("private.secret", b"drop".to_vec())
        .file("keep.secret", b"keep".to_vec())
        .file("notes.txt", b"keep".to_vec());

    let created = conform(&fixture);

    assert_eq!(
        project_paths(&created.pack),
        [
            IGNORE_FILE,
            "ignored/reincluded/keep.txt",
            "keep.secret",
            "main.typ",
            "notes.txt",
        ]
    );
}

#[test]
fn a_nested_ignore_file_is_an_ordinary_project_file() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .ignore_file("*.tmp\n")
        .file("nested/.typkignore", b"*.typ\n*.txt\n".to_vec())
        .file("nested/chapter.typ", b"= Nested".to_vec())
        .file("nested/ordinary.txt", b"keep".to_vec())
        .file("drop.tmp", b"drop".to_vec());

    let created = conform(&fixture);

    // Only the root policy decides membership, so the nested ignore file is
    // contained like any other file and excludes nothing beside it.
    assert_eq!(
        project_paths(&created.pack),
        [
            IGNORE_FILE,
            "main.typ",
            "nested/.typkignore",
            "nested/chapter.typ",
            "nested/ordinary.txt",
        ]
    );
}

#[test]
fn a_nested_pack_path_is_excluded_by_a_rule_no_policy_overrides() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .ignore_file("!*.typk\n")
        .file("nested/old.typk", b"drop".to_vec())
        .file("bundle.typk/main.typ", b"drop".to_vec())
        .file("keep.txt", b"keep".to_vec());

    let created = conform(&fixture);

    assert_eq!(
        project_paths(&created.pack),
        [IGNORE_FILE, "keep.txt", "main.typ"]
    );
}

#[test]
fn an_entrypoint_a_policy_would_otherwise_exclude_survives_its_negation() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .ignore_file("*.typ\n!main.typ\n")
        .file("chapter.typ", b"= Excluded".to_vec())
        .file("notes.txt", b"keep".to_vec());

    let created = conform(&fixture);

    assert_eq!(
        project_paths(&created.pack),
        [IGNORE_FILE, "main.typ", "notes.txt"]
    );
}

#[test]
fn an_entrypoint_the_policy_excludes_fails_every_adapter() {
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)")
        .ignore_file("*.typ\n")
        .file("notes.txt", b"keep".to_vec());

    assert_eq!(conform_failure(&fixture), Failure::ExcludedEntrypoint);
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
    .package(
        "outer",
        PackageDisposition::Embedded,
        "#import \"@local/inner:1.0.0\": inner\n#let mark = inner + 1",
    )
    .package("inner", PackageDisposition::Embedded, "#let inner = 2");

    let created = conform(&fixture);

    assert_eq!(project_paths(&created.pack), ["main.typ"]);
    assert_eq!(
        package_dispositions(&created.pack),
        [
            ("@local/inner:1.0.0".to_owned(), true),
            ("@local/outer:1.0.0".to_owned(), true),
        ]
    );
    // The whole Complete Package Tree travels, not only what was read.
    assert!(
        created
            .pack
            .package_file(&local_spec("outer"), "unread.txt")
            .is_some()
    );
}

/// The reference adapter vendors every tree under one choice, so this fixture
/// is driven in memory alone.
#[test]
fn mixed_package_dispositions_travel_per_tree() {
    let fixture = Fixture::document(
        "#import \"@local/embedded:1.0.0\": one\n\
         #import \"@local/external:1.0.0\": two\n\
         #rect(width: (one + two) * 1pt, height: 1pt)",
    )
    .package("embedded", PackageDisposition::Embedded, "#let one = 1")
    .package("external", PackageDisposition::External, "#let two = 2");

    let created = conform_in_memory(&fixture);

    assert_eq!(
        package_dispositions(&created.pack),
        [
            ("@local/embedded:1.0.0".to_owned(), true),
            ("@local/external:1.0.0".to_owned(), false),
        ]
    );
    assert!(created.pack.has_package(&local_spec("embedded")));
    assert!(!created.pack.has_package(&local_spec("external")));
}

#[test]
fn a_supplied_tree_that_does_not_satisfy_its_specification_fails_every_adapter() {
    let fixture = Fixture::document("#import \"@local/declared:1.0.0\": value\n#value")
        .supplied_package(
            "declared",
            package_files("other", "#let value = 1"),
            PackageDisposition::Embedded,
        );

    // A diagnosis rather than a loop that never progresses: a caller told the
    // same specification is missing forever would have nothing to act on.
    assert_eq!(conform_failure(&fixture), Failure::UnsatisfiedPackage);
}

/// The expansion ceiling is a value only a caller supplying its own transport
/// chooses, so this fixture is driven in memory alone.
#[cfg(feature = "package-acquisition")]
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
            typst_pack::PackageExpansionCeiling { max_bytes: 4096 },
        );

    assert_eq!(
        conform_failure_in_memory(&fixture),
        Failure::ExpansionCeiling
    );
}

/// The gzip-compressed tar a registry serves for one package, written from a
/// member's nominal size so that the archive stays small whatever it claims to
/// expand to.
#[cfg(feature = "package-acquisition")]
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

    assert_eq!(conform_failure(&fixture), Failure::Compile);
}

#[test]
fn representative_compile_warnings_are_returned_by_every_adapter() {
    let fixture = Fixture::document("#set text(font: \"Definitely Missing\")\nWarning\n");

    let created = conform(&fixture);

    assert!(
        created
            .warnings
            .iter()
            .any(|warning| warning.contains("unknown font family")),
        "{:?}",
        created.warnings
    );
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

#[test]
fn a_container_offering_no_face_contributes_nothing() {
    // The reference adapter's font scan never indexes these bytes at all, while
    // an in-memory adapter holds a container that expands to no face. The two
    // reach an empty catalog by different routes and must still describe the
    // same Pack.
    let fixture = Fixture::document("#rect(width: 10pt, height: 10pt)").font(
        FontSource::Scanned(b"not a font".to_vec()),
        FontDisposition::Embedded,
    );

    let created = conform(&fixture);

    assert!(font_requirements(&created.pack).is_empty());
    assert!(font_catalog(&created.pack).is_empty());
}

/// Face selection out of catalogs holding real font bytes, which Typst only
/// ships with the `embedded-fonts` feature.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::{FontContainerIdentity, FontDisposition};

    use crate::font_bytes::{family_of, font_collection, renamed_family, typst_container};
    use crate::{Fixture, FontSource, conform, font_catalog, font_requirements};

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
        .font(
            FontSource::Scanned(container.clone()),
            FontDisposition::Embedded,
        );

        let created = conform(&fixture);

        // Every face of a collection travels in the same container, so two
        // selected faces are one Font Requirement.
        let identity = FontContainerIdentity::from_bytes(&container);
        assert_eq!(font_requirements(&created.pack), [(identity, true)]);
        assert_eq!(
            font_catalog(&created.pack),
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
                fixture = fixture.font(
                    FontSource::Scanned(container.clone()),
                    FontDisposition::Embedded,
                );
            }
            font_requirements(&conform(&fixture).pack)
        };

        assert_eq!(
            selected([&first, &second]),
            [(FontContainerIdentity::from_bytes(&first), true)]
        );
        assert_eq!(
            selected([&second, &first]),
            [(FontContainerIdentity::from_bytes(&second), true)]
        );
    }

    #[test]
    fn mixed_font_dispositions_travel_per_container() {
        let typst_font = typst_container();
        let scanned_family = alternate_family('Z');
        let scanned = container_for(&scanned_family);
        let fixture = Fixture::document(&format!(
            "Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]\n"
        ))
        .font(FontSource::TypstEmbedded, FontDisposition::External)
        .font(
            FontSource::Scanned(scanned.clone()),
            FontDisposition::Embedded,
        );

        let created = conform(&fixture);

        // One Pack references a container it may not redistribute and embeds
        // one it may, and the Pack Font Catalog keeps candidate order.
        let typst_identity = FontContainerIdentity::from_bytes(&typst_font);
        let scanned_identity = FontContainerIdentity::from_bytes(&scanned);
        assert_eq!(
            font_catalog(&created.pack),
            [(typst_identity, 0, false), (scanned_identity, 0, true),]
        );
        assert_eq!(
            font_requirements(&created.pack),
            [(typst_identity, false), (scanned_identity, true)]
        );
    }
}
