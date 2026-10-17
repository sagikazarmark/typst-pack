//! The cross-adapter creation conformance corpus.
//!
//! Every scenario is plain bytes: project files, package trees or registry
//! archives, and ordered font containers. Each runs through direct core
//! composition, OpenDAL Memory (in two storage orders), and, when the
//! filesystem can express it, the filesystem packer. The runners must agree on
//! the Pack, the representative-compile diagnostics, and the resume trace
//! (ADR-0019, ADR-0021). Filesystem-only selection such as `.typkignore` is
//! tested in `fs.rs`.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

#[cfg(all(feature = "opendal", feature = "package-reading"))]
#[path = "support/opendal.rs"]
mod opendal_support;

use std::collections::BTreeMap;

use typst::diag::SourceDiagnostic;
use typst::syntax::package::PackageSpec;
use typst_pack::compile::DocumentTime;
use typst_pack::package::ReadFailure;
use typst_pack::{Pack, ProjectSnapshot, archive, create, font, package};

/// No adapter's host clock may reach a Pack.
const DOCUMENT_TIME: DocumentTime = DocumentTime::UnixTimestamp(1_700_000_000);

type Files = Vec<(&'static str, Vec<u8>)>;

/// One scenario, as bytes alone.
#[derive(Default)]
struct Fixture {
    project: Files,
    packages: Vec<Package>,
    /// Font containers in catalog order, each with its disposition and
    /// whether it is one of Typst's own, which the filesystem packer offers
    /// itself.
    fonts: Vec<(Vec<u8>, bool, bool)>,
}

struct Package {
    spec: PackageSpec,
    source: Source,
    embedded: bool,
    /// A stale `NotFound` failure is recorded before the tree is supplied,
    /// which the successful insertion must clear.
    stale_failure: bool,
}

enum Source {
    Files(Files),
    /// A registry archive, expanded under `limits`, from the archive cache or
    /// the registry layout.
    #[cfg(feature = "package-reading")]
    Archive {
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
        #[cfg_attr(not(feature = "opendal"), allow(dead_code))]
        cached: bool,
    },
}

/// A package tree declaring `name`, plus a file no compile reads, so that
/// whole-tree containment is observable.
fn package_files(name: &str, body: &str) -> Files {
    let manifest =
        format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n");
    vec![
        ("typst.toml", manifest.into_bytes()),
        ("lib.typ", body.as_bytes().to_vec()),
        ("unread.txt", b"the whole Package Tree travels".to_vec()),
    ]
}

impl Fixture {
    fn document(source: &str) -> Self {
        let project = vec![("main.typ", source.as_bytes().to_vec())];
        Self {
            project,
            ..Self::default()
        }
    }

    fn file(mut self, path: &'static str, data: &[u8]) -> Self {
        self.project.push((path, data.to_vec()));
        self
    }

    fn package(self, name: &str, embedded: bool, body: &str) -> Self {
        self.tree(name, package_files(name, body), embedded)
    }

    /// Supplies `files` as the tree of `@local/{name}:1.0.0`.
    fn tree(self, name: &str, files: Files, embedded: bool) -> Self {
        self.supply(
            format!("@local/{name}:1.0.0"),
            Source::Files(files),
            embedded,
        )
    }

    #[cfg(feature = "package-reading")]
    fn archive(
        self,
        name: &str,
        bytes: Vec<u8>,
        limits: typst_pack::ReadLimits,
        cached: bool,
    ) -> Self {
        let source = Source::Archive {
            bytes,
            limits,
            cached,
        };
        self.supply(format!("@preview/{name}:1.0.0"), source, true)
    }

    fn supply(mut self, spec: String, source: Source, embedded: bool) -> Self {
        let spec = spec.parse().unwrap();
        let stale_failure = false;
        self.packages.push(Package {
            spec,
            source,
            embedded,
            stale_failure,
        });
        self
    }

    fn with_stale_failure(mut self) -> Self {
        self.packages.last_mut().unwrap().stale_failure = true;
        self
    }

    #[cfg(feature = "embedded-fonts")]
    fn font(mut self, container: Vec<u8>, embedded: bool) -> Self {
        self.fonts.push((container, embedded, false));
        self
    }

    #[cfg(feature = "embedded-fonts")]
    fn typst_fonts(mut self, embedded: bool) -> Self {
        let fonts = font::embedded().map(|font| (font.data().to_vec(), embedded, true));
        self.fonts.extend(fonts);
        self
    }

    fn offered(&self, spec: &PackageSpec) -> &Package {
        let mut packages = self.packages.iter();
        packages.find(|package| &package.spec == spec).unwrap()
    }
}

/// What one runner observed.
#[derive(Debug, Default, PartialEq)]
struct Run {
    /// The created Pack, encoded.
    archive: Option<Vec<u8>>,
    /// The diagnostics of a failed creation.
    errors: Vec<SourceDiagnostic>,
    warnings: Vec<SourceDiagnostic>,
    /// The specifications each resume round reported missing.
    missing_rounds: Vec<Vec<String>>,
    /// Every distinct state of the request's package failures.
    failure_states: Vec<BTreeMap<String, ReadFailure>>,
}

impl Run {
    fn pack(&self) -> Pack {
        let archive = self.archive.as_ref();
        let archive = archive.unwrap_or_else(|| panic!("no Pack: {:#?}", self.errors));
        archive::decode(archive, Default::default()).unwrap()
    }

    fn finish(mut self, result: Result<create::Outcome, create::Error>) -> Self {
        match result {
            Ok(create::Outcome::Created(created)) => {
                self.archive = Some(archive::encode(&created.pack));
                self.warnings = created.warnings;
            }
            Err(create::Error::Failed {
                errors, warnings, ..
            }) => {
                self.errors = errors;
                self.warnings = warnings;
            }
            other => panic!("creation did not finish: {other:?}"),
        }
        self
    }
}

/// Drives creation to a Pack or a failure, supplying each missing package
/// through `supply` as a caller resuming across rounds does.
fn resume(
    fixture: &Fixture,
    mut request: create::Request,
    reversed: bool,
    mut supply: impl FnMut(&mut create::Request, &Package),
) -> Run {
    request.options.document_time = DOCUMENT_TIME;
    let mut run = Run::default();
    let record = |run: &mut Run, request: &create::Request| {
        let failures = request.package_failures.iter();
        let state = failures
            .map(|(spec, reason)| (spec.to_string(), reason.clone()))
            .collect();
        if run.failure_states.last().unwrap_or(&BTreeMap::new()) != &state {
            run.failure_states.push(state);
        }
    };
    for _ in 0..8 {
        let mut missing = match create(&request) {
            Ok(create::Outcome::MissingPackages(missing)) => missing,
            result => return run.finish(result),
        };
        run.missing_rounds
            .push(missing.iter().map(ToString::to_string).collect());
        if reversed {
            missing.reverse();
        }
        for spec in missing {
            let package = fixture.offered(&spec);
            if package.stale_failure {
                request.package_failures.insert(spec, ReadFailure::NotFound);
                record(&mut run, &request);
            }
            supply(&mut request, package);
            record(&mut run, &request);
        }
    }
    panic!("creation issued no Pack within eight resume rounds");
}

/// Direct core composition from already selected values.
fn create_directly(fixture: &Fixture) -> Run {
    let files = fixture
        .project
        .iter()
        .map(|(path, data)| (*path, data.clone()));
    let mut request = create::Request::new(ProjectSnapshot::new("main.typ", files).unwrap());
    for (data, embedded, _) in &fixture.fonts {
        let container = font::Container::new(data.clone()).unwrap();
        request.fonts.push(font::Entry {
            container,
            embedded: *embedded,
        });
    }
    resume(fixture, request, false, |request, package| {
        let tree = match &package.source {
            Source::Files(files) => package::Tree::new(files.clone()).map_err(|e| e.to_string()),
            #[cfg(feature = "package-reading")]
            Source::Archive { bytes, limits, .. } => {
                package::registry::expand_archive(bytes, *limits).map_err(|e| chain(&e))
            }
        };
        let spec = package.spec.clone();
        let catalog = &mut request.packages;
        let inserted = tree.and_then(|tree| {
            let inserted = catalog.insert(spec.clone(), tree, package.embedded);
            inserted.map_err(|e| e.to_string())
        });
        // A rejected package is recorded with its reason, as adapters do.
        match inserted {
            Ok(()) => request.package_failures.remove(&spec),
            Err(reason) => request
                .package_failures
                .insert(spec, ReadFailure::Other(Some(reason))),
        };
    })
}

/// An error and its sources, as adapters record a failure reason.
#[cfg(feature = "package-reading")]
fn chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message = format!("{message}: {error}");
        source = error.source();
    }
    message
}

/// Reads every input through one OpenDAL Memory backend, written in
/// declaration order or its reverse.
#[cfg(all(feature = "opendal", feature = "package-reading"))]
fn create_on_opendal(fixture: &Fixture, reversed: bool) -> Run {
    use opendal_support::block_on;
    use typst_pack::ReadLimits;
    use typst_pack::opendal::{self as adapter, PackageSource};

    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let mut objects = Vec::new();
    for (path, data) in &fixture.project {
        objects.push((format!("project/{path}"), data.clone()));
    }
    let mut font_sources = Vec::new();
    for (index, (data, embedded, _)) in fixture.fonts.iter().enumerate() {
        let prefix = format!("fonts/{index:06}/");
        objects.push((format!("{prefix}font.ttf"), data.clone()));
        font_sources.push(adapter::FontSource {
            prefix,
            embedded: *embedded,
        });
    }
    for package in &fixture.packages {
        let spec = &package.spec;
        let (namespace, name, version) = (&spec.namespace, &spec.name, &spec.version);
        match &package.source {
            Source::Files(files) => objects.extend(files.iter().map(|(path, data)| {
                (
                    format!("trees/{namespace}/{name}/{version}/{path}"),
                    data.clone(),
                )
            })),
            Source::Archive {
                bytes,
                cached: true,
                ..
            } => {
                objects.push((
                    format!("cache/{namespace}/{name}/{version}.tar.gz"),
                    bytes.clone(),
                ));
            }
            Source::Archive {
                bytes,
                cached: false,
                ..
            } => {
                objects.push((
                    format!("registry/{namespace}/{name}-{version}.tar.gz"),
                    bytes.clone(),
                ));
            }
        }
    }
    if reversed {
        objects.reverse();
    }
    for (path, data) in objects {
        block_on(operator.write(&path, data)).unwrap();
    }

    let limits = ReadLimits::default();
    let project = adapter::read_project(&operator, "project/", "main.typ", limits);
    let mut request = create::Request::new(block_on(project).unwrap());
    request.fonts = block_on(adapter::read_fonts(&operator, &font_sources, limits)).unwrap();
    let (operator, prefix) = (&operator, String::from);
    let sources = [
        PackageSource::Trees {
            operator: operator.clone(),
            prefix: prefix("trees/"),
        },
        PackageSource::ArchiveCache {
            operator: operator.clone(),
            prefix: prefix("cache/"),
        },
        PackageSource::Registry {
            operator: operator.clone(),
            prefix: prefix("registry/"),
        },
    ];
    resume(fixture, request, reversed, |request, package| {
        let read = block_on(adapter::read_package(&sources, &package.spec, limits)).unwrap();
        let limits = match &package.source {
            Source::Files(_) => ReadLimits::default(),
            Source::Archive { limits, .. } => *limits,
        };
        // A rejected tree is recorded as a package failure.
        let _ = adapter::insert_package(request, &package.spec, read, package.embedded, limits);
    })
}

/// The filesystem packer over the fixture written out as a project directory,
/// a package path, and one directory per scanned font container.
#[cfg(feature = "fs")]
fn create_on_filesystem(fixture: &Fixture) -> Run {
    use typst_pack::{compile, fs};

    let dir = tempfile::tempdir().unwrap();
    let write = |path: std::path::PathBuf, data: &[u8]| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    };
    for (path, data) in &fixture.project {
        write(dir.path().join("project").join(path), data);
    }
    let packages = dir.path().join("packages");
    std::fs::create_dir_all(&packages).unwrap();
    for package in &fixture.packages {
        let files = match &package.source {
            Source::Files(files) => files,
            #[cfg(feature = "package-reading")]
            Source::Archive { .. } => unreachable!("the filesystem packer reads no archives"),
        };
        let spec = &package.spec;
        let root = packages.join(format!("{}/{}/{}", spec.namespace, spec.name, spec.version));
        for (path, data) in files {
            write(root.join(path), data);
        }
    }

    let mut packer = fs::Packer {
        package_path: Some(packages),
        system_fonts: false,
        typst_embedded_fonts: false,
        embed_packages: fixture.packages.iter().all(|package| package.embedded),
        options: compile::Options {
            document_time: DOCUMENT_TIME,
            ..Default::default()
        },
        ..fs::Packer::default()
    };
    for (index, (data, embedded, typst)) in fixture.fonts.iter().enumerate() {
        if *typst {
            packer.typst_embedded_fonts = true;
            packer.include_typst_embedded_fonts = *embedded;
            continue;
        }
        packer.embed_fonts = *embedded;
        // The extension decides whether the scan reads a collection.
        let directory = dir.path().join(format!("fonts/{index}"));
        let name = if data.starts_with(b"ttcf") {
            "font.ttc"
        } else {
            "font.ttf"
        };
        write(directory.join(name), data);
        packer.font_paths.push(directory);
    }

    let run = Run::default();
    match packer.pack(dir.path().join("project"), "main.typ") {
        Ok(packed) => Run {
            archive: Some(archive::encode(&packed.pack)),
            warnings: packed.warnings,
            ..run
        },
        Err(fs::Error::Create(error)) => run.finish(Err(*error)),
        Err(error) => panic!("filesystem creation failed unexpectedly: {error}"),
    }
}

/// Runs a fixture through every runner, asserting they agree, and returns the
/// direct run. `filesystem` says whether the filesystem packer can express
/// the fixture: it vendors every tree under one choice and expands no archive.
fn run(fixture: &Fixture, filesystem: bool) -> Run {
    let direct = create_directly(fixture);

    #[cfg(all(feature = "opendal", feature = "package-reading"))]
    for reversed in [false, true] {
        let opendal = create_on_opendal(fixture, reversed);
        assert_eq!(opendal, direct, "OpenDAL (reversed: {reversed})");
    }

    #[cfg(feature = "fs")]
    if filesystem {
        let fs = create_on_filesystem(fixture);
        assert_eq!(fs.archive, direct.archive, "filesystem");
        // The packer reports a rejected tree through its own discovery.
        if direct.failure_states.is_empty() {
            assert_eq!(
                (fs.errors, fs.warnings),
                (direct.errors.clone(), direct.warnings.clone())
            );
        }
    }
    #[cfg(not(feature = "fs"))]
    let _ = filesystem;

    direct
}

fn conform(fixture: &Fixture) -> Run {
    run(fixture, true)
}

/// For scenarios only direct composition and OpenDAL can express.
fn conform_object_storage(fixture: &Fixture) -> Run {
    run(fixture, false)
}

/// Asserts the run failed on `spec` alone, with a recorded failure whose
/// reason contains `reason` and reaches the import's diagnostic.
fn assert_package_failure(run: &Run, spec: &str, reason: &str) {
    assert_eq!(run.archive, None);
    let failures = run.failure_states.last().unwrap();
    assert_eq!(failures.keys().collect::<Vec<_>>(), [spec]);
    assert!(
        matches!(&failures[spec], ReadFailure::Other(Some(detail)) if detail.contains(reason)),
        "{failures:?}"
    );
    let mut errors = run.errors.iter();
    assert!(
        errors.any(|error| error.message.starts_with("failed to load package")
            && error.message.contains(reason))
    );
}

fn package_dispositions(pack: &Pack) -> Vec<(String, bool)> {
    let requirements = pack.package_requirements().iter();
    requirements
        .map(|requirement| (requirement.spec.to_string(), requirement.embedded))
        .collect()
}

#[test]
fn a_project_with_no_packages_requires_none() {
    let fixture =
        Fixture::document("#rect(width: 10pt, height: 10pt)").file("data/notes.txt", b"unread");

    let pack = conform(&fixture).pack();

    // Project files come from the snapshot, never from compiler observations.
    let paths = pack.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(paths, ["data/notes.txt", "main.typ"]);
    assert!(package_dispositions(&pack).is_empty());
    assert!(pack.font_requirements().is_empty());
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
        true,
        "#import \"@local/inner:1.0.0\": inner\n#let mark = inner + 1",
    )
    .with_stale_failure()
    .package("inner", true, "#let inner = 2");

    let run = conform(&fixture);
    let pack = run.pack();

    assert_eq!(
        package_dispositions(&pack),
        [
            ("@local/inner:1.0.0".to_owned(), true),
            ("@local/outer:1.0.0".to_owned(), true)
        ]
    );
    // The whole Package Tree travels, not only what was read.
    let (_, outer) = pack
        .packages()
        .find(|(spec, _)| spec.name == "outer")
        .unwrap();
    assert!(outer.file("unread.txt").is_some());
    assert_eq!(
        run.missing_rounds,
        [["@local/outer:1.0.0"], ["@local/inner:1.0.0"]]
    );
    // The stale failure is cleared once the tree is supplied.
    let stale = BTreeMap::from([("@local/outer:1.0.0".to_owned(), ReadFailure::NotFound)]);
    assert_eq!(run.failure_states, [stale, BTreeMap::new()]);
}

#[test]
fn mixed_package_dispositions_travel_per_tree() {
    let fixture = Fixture::document(
        "#import \"@local/embedded:1.0.0\": one\n\
         #import \"@local/external:1.0.0\": two\n\
         #rect(width: (one + two) * 1pt, height: 1pt)",
    )
    .package("embedded", true, "#let one = 1")
    .package("external", false, "#let two = 2");

    let pack = conform_object_storage(&fixture).pack();

    assert_eq!(
        package_dispositions(&pack),
        [
            ("@local/embedded:1.0.0".to_owned(), true),
            ("@local/external:1.0.0".to_owned(), false)
        ]
    );
    let names = pack.packages().map(|(spec, _)| spec.name.to_string());
    assert_eq!(names.collect::<Vec<_>>(), ["embedded"]);
}

#[test]
fn a_supplied_tree_that_does_not_satisfy_its_specification_fails_every_adapter() {
    let fixture = Fixture::document("#import \"@local/declared:1.0.0\": value\n#value").tree(
        "declared",
        package_files("other", "#let value = 1"),
        true,
    );

    // A diagnosis rather than a loop that never progresses.
    assert_package_failure(
        &conform(&fixture),
        "@local/declared:1.0.0",
        "declares the name \"other\"",
    );
}

#[cfg(feature = "package-reading")]
#[test]
fn an_archive_beyond_the_expansion_ceiling_fails_the_resume_loop() {
    // 128 MiB of zeros in a few kilobytes of archive, against a 4 KiB ceiling.
    let limits = typst_pack::ReadLimits {
        max_entries: 10,
        max_file_bytes: 4096,
        max_total_bytes: 4096,
    };
    let bytes = archive_of(&[("lib.typ", 128 * 1024 * 1024, b"#let value = 1")]);
    let fixture = Fixture::document("#import \"@preview/oversized:1.0.0\": value\n#value").archive(
        "oversized",
        bytes,
        limits,
        false,
    );

    assert_package_failure(
        &conform_object_storage(&fixture),
        "@preview/oversized:1.0.0",
        "file bytes limit of 4096 exceeded",
    );
}

#[cfg(feature = "package-reading")]
#[test]
fn cache_and_registry_archives_compose_into_the_same_semantic_pack() {
    let valid = |name: &str, body: &str| {
        let files = package_files(name, body);
        let members = files
            .iter()
            .map(|(path, data)| (*path, data.len() as u64, data.as_slice()));
        archive_of(&members.collect::<Vec<_>>())
    };
    let fixture = Fixture::document(
        "#import \"@preview/cached:1.0.0\": cached\n\
         #import \"@preview/registered:1.0.0\": registered\n\
         #rect(width: (cached + registered) * 1pt, height: 1pt)",
    )
    .archive(
        "cached",
        valid("cached", "#let cached = 1"),
        Default::default(),
        true,
    )
    .archive(
        "registered",
        valid("registered", "#let registered = 2"),
        Default::default(),
        false,
    );

    let run = conform_object_storage(&fixture);

    assert_eq!(
        package_dispositions(&run.pack()),
        [
            ("@preview/cached:1.0.0".to_owned(), true),
            ("@preview/registered:1.0.0".to_owned(), true)
        ]
    );
    assert_eq!(
        run.missing_rounds,
        [["@preview/cached:1.0.0"], ["@preview/registered:1.0.0"]]
    );
}

/// The gzip-compressed tar a registry serves, written from each member's
/// nominal size so that the archive stays small whatever it expands to.
#[cfg(feature = "package-reading")]
fn archive_of(members: &[(&str, u64, &[u8])]) -> Vec<u8> {
    use std::io::Read;

    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for (path, size, data) in members {
        let mut header = tar::Header::new_gnu();
        header.set_size(*size);
        header.set_mode(0o644);
        let padding = std::io::repeat(0).take(size.saturating_sub(data.len() as u64));
        builder
            .append_data(&mut header, path, data.chain(padding))
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn a_representative_request_that_does_not_compile_fails_every_adapter() {
    let run = conform(&Fixture::document("#import \"missing.typ\": value\n#value"));

    assert_eq!(run.archive, None);
    assert!(!run.errors.is_empty());
    assert!(run.failure_states.is_empty());
}

#[test]
fn representative_compile_warnings_are_returned_by_every_adapter() {
    let run = conform(&Fixture::document(
        "#set text(font: \"Definitely Missing\")\nWarning\n",
    ));

    let mut warnings = run.warnings.iter();
    assert!(warnings.any(|warning| warning.message.contains("unknown font family")));
}

/// Face selection out of catalogs holding real font bytes, which Typst only
/// ships with the `embedded-fonts` feature.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::{Digest, Pack};

    use crate::font_bytes::{family_of, font_collection, renamed_family, typst_container};
    use crate::{Fixture, conform};

    /// A container offering only `family`, renamed out of one Typst ships.
    fn container_for(family: &str) -> Vec<u8> {
        let font = typst_container();
        renamed_family(&font, &family_of(&font), family)
    }

    /// A family name as long as the one Typst's first container offers.
    fn alternate_family() -> String {
        format!("Z{}", &family_of(&typst_container())[1..])
    }

    /// Every face available at compile time, in font book order: its
    /// container, its index there, and whether the container travels.
    fn faces(pack: &Pack) -> Vec<(Digest, u32, bool)> {
        let requirements = pack.font_requirements().iter();
        let faces = requirements.flat_map(|requirement| {
            let faces = requirement.faces.iter();
            faces.map(|face| (requirement.digest, face.index, requirement.embedded))
        });
        faces.collect()
    }

    #[test]
    fn every_selected_face_of_a_multi_face_collection_reaches_the_catalog() {
        let default_family = family_of(&typst_container());
        let second_family = alternate_family();
        let container = font_collection(&[typst_container(), container_for(&second_family)]);
        let fixture = Fixture::document(&format!(
            "#set text(font: \"{default_family}\")\nFirst face\n\n\
             #text(font: \"{second_family}\")[Second face]\n"
        ))
        .font(container.clone(), true);

        let pack = conform(&fixture).pack();

        // The selected faces are one Font Requirement offering every face.
        let identity = Digest::of(&container);
        assert_eq!(pack.font_requirements().len(), 1);
        assert_eq!(faces(&pack), [(identity, 0, true), (identity, 1, true)]);
    }

    #[test]
    fn catalog_order_decides_which_face_wins() {
        let family = family_of(&typst_container());
        let first = typst_container();
        // Same family and variant, distinct bytes: only order can decide.
        let mut second = first.clone();
        second.push(0);

        let selected = |containers: [&Vec<u8>; 2]| {
            let mut fixture =
                Fixture::document(&format!("#set text(font: \"{family}\")\nSelected\n"));
            for container in containers {
                fixture = fixture.font(container.clone(), true);
            }
            faces(&conform(&fixture).pack())
        };

        assert_eq!(selected([&first, &second]), [(Digest::of(&first), 0, true)]);
        assert_eq!(
            selected([&second, &first]),
            [(Digest::of(&second), 0, true)]
        );
    }

    #[test]
    fn mixed_font_dispositions_travel_per_container() {
        let typst_font = typst_container();
        let scanned_family = alternate_family();
        let scanned = container_for(&scanned_family);
        let fixture = Fixture::document(&format!(
            "Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]\n"
        ))
        .typst_fonts(false)
        .font(scanned.clone(), true);

        let pack = conform(&fixture).pack();

        // One Pack references a container it may not redistribute and embeds
        // one it may, in candidate order.
        let (typst, scanned) = (Digest::of(&typst_font), Digest::of(&scanned));
        assert_eq!(faces(&pack), [(typst, 0, false), (scanned, 0, true)]);
    }
}
