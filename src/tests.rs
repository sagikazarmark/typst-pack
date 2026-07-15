//! Crate tests.

use crate::*;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use typst::World;
use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use typst_kit::files::FileLoader;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

struct MemoryProjectFile {
    path: String,
    data: Bytes,
    calls: Arc<AtomicUsize>,
}

impl MemoryProjectFile {
    fn new(path: &str, data: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.to_owned(),
            data: Bytes::new(data.into()),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn tracked(path: &str, data: impl Into<Vec<u8>>) -> (Self, Arc<AtomicUsize>) {
        let loader = Self::new(path, data);
        let calls = Arc::clone(&loader.calls);
        (loader, calls)
    }
}

impl FileLoader for MemoryProjectFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project) && path == self.path {
            Ok(self.data.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
    }
}

struct ErrorProjectLoader {
    error: FileError,
    calls: Arc<AtomicUsize>,
}

#[cfg(feature = "fs")]
struct BlockingProjectFile {
    path: String,
    data: Bytes,
    entered: Arc<std::sync::Barrier>,
    release: Arc<std::sync::Barrier>,
}

#[cfg(feature = "fs")]
impl FileLoader for BlockingProjectFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project) && path == self.path {
            self.entered.wait();
            self.release.wait();
            Ok(self.data.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
    }
}

impl ErrorProjectLoader {
    fn tracked(error: FileError) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Self {
                error,
                calls: Arc::clone(&calls),
            },
            calls,
        )
    }
}

impl FileLoader for ErrorProjectLoader {
    fn load(&self, _id: FileId) -> FileResult<Bytes> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(self.error.clone())
    }
}

fn project_file_id(path: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).unwrap()).intern()
}

#[test]
fn manifest_roundtrip() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"

        [packages]
        vendored = ["@preview/cetz:0.3.4"]
        external = ["@preview/tablex:0.0.9"]

        [[fonts]]
        path = "fonts/test.ttf"
        families = ["Test"]

        [metadata]
        name = "Test pack"
        "#,
    )
    .unwrap();
    assert_eq!(manifest.project.entrypoint, "main.typ");
    assert_eq!(manifest.vendored_packages().unwrap().len(), 1);
    assert_eq!(manifest.unvendored_packages().unwrap().len(), 1);

    let serialized = manifest.to_toml();
    assert!(serialized.contains("external ="));
    assert!(!serialized.contains("unvendored ="));
    let reparsed = PackManifest::from_toml(&serialized).unwrap();
    assert_eq!(manifest, reparsed);
}

#[test]
fn old_manifest_has_no_external_project_resources() {
    let manifest =
        PackManifest::from_toml("format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
            .unwrap();

    assert!(manifest.project.external_resources.is_empty());
}

#[test]
fn manifest_rejects_unsafe_external_project_resource_path() {
    let result = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"
        external-resources = ["../secret.png"]
        "#,
    );

    assert!(matches!(
        result,
        Err(PackManifestError::InvalidExternalResource { .. })
    ));
}

#[test]
fn manifest_normalizes_external_project_resources_deterministically() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"
        external-resources = ["z.png", "assets/../logo.png", "./logo.png"]
        "#,
    )
    .unwrap();

    assert_eq!(
        manifest
            .project
            .external_resources
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["logo.png", "z.png"]
    );
    assert!(
        manifest
            .to_toml()
            .contains("external-resources = [\n    \"logo.png\",\n    \"z.png\",\n]")
    );
}

#[test]
fn manifest_validation_rejects_noncanonical_external_project_resource_path() {
    let mut manifest =
        PackManifest::from_toml("format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
            .unwrap();
    manifest
        .project
        .external_resources
        .insert("assets/../logo.png".to_owned());

    assert!(matches!(
        manifest.validate(),
        Err(PackManifestError::InvalidExternalResource { .. })
    ));
}

#[test]
fn manifest_rejects_future_version() {
    let result =
        PackManifest::from_toml("format-version = 99\n[project]\nentrypoint = \"main.typ\"\n");
    assert!(matches!(
        result,
        Err(PackManifestError::UnsupportedVersion(99))
    ));
}

#[test]
fn builder_requires_entrypoint_file() {
    let result = Pack::builder("main.typ").build();
    assert!(matches!(result, Err(PackBuildError::MissingEntrypoint(_))));
}

#[test]
fn pack_roundtrip_in_memory() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", "#include \"note.typ\"".as_bytes().to_vec())
        .unwrap()
        .file("note.typ", "Hello".as_bytes().to_vec())
        .unwrap()
        .file("assets/logo.png", tiny_png())
        .unwrap()
        .build()
        .unwrap();

    let bytes = pack.to_bytes().unwrap();
    let reread = Pack::from_bytes(bytes).unwrap();

    assert_eq!(reread.entrypoint(), "main.typ");
    assert_eq!(reread.files().count(), 3);
    assert_eq!(
        reread.file("note.typ").unwrap().as_slice(),
        "Hello".as_bytes()
    );
}

#[test]
fn manually_declared_external_project_resource_survives_archive_roundtrip() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#image(\"assets/logo.png\")".to_vec())
        .unwrap()
        .external_resource("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    assert!(pack.file("assets/logo.png").is_none());
    let pack = Pack::from_bytes(pack.to_bytes().unwrap()).unwrap();
    assert_eq!(
        pack.external_resources().collect::<Vec<_>>(),
        ["assets/logo.png"]
    );
}

#[test]
fn pack_builder_rejects_external_project_resource_file_conflicts() {
    let packed_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("logo.png", tiny_png())
        .unwrap()
        .external_resource("logo.png")
        .unwrap()
        .build();
    let declared_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("logo.png")
        .unwrap()
        .file("logo.png", tiny_png())
        .unwrap()
        .build();

    assert!(matches!(
        packed_first,
        Err(PackBuildError::ExternalResourceConflict(path)) if path == "logo.png"
    ));
    assert!(matches!(
        declared_first,
        Err(PackBuildError::ExternalResourceConflict(path)) if path == "logo.png"
    ));
}

#[test]
fn read_rejects_archives_without_manifest() {
    use std::io::Write;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    zip.start_file("project/main.typ", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"hi").unwrap();
    zip.finish().unwrap();
    let result = Pack::from_bytes(buffer.into_inner());
    assert!(matches!(result, Err(PackReadError::MissingManifest)));
}

#[test]
fn read_rejects_external_project_resource_file_conflicts() {
    use std::io::Write;

    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(MANIFEST_PATH, options).unwrap();
    zip.write_all(
        b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"logo.png\"]\n",
    )
    .unwrap();
    zip.start_file("project/main.typ", options).unwrap();
    zip.write_all(b"Hello").unwrap();
    zip.start_file("project/logo.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();
    zip.finish().unwrap();

    let result = Pack::from_bytes(buffer.into_inner());
    assert!(matches!(
        result,
        Err(PackReadError::ExternalResourceConflict(path)) if path == "logo.png"
    ));
}

#[test]
fn parse_page_selection_understands_ranges() {
    use std::num::NonZeroUsize;
    let one = NonZeroUsize::new(1);
    let three = NonZeroUsize::new(3);
    let five = NonZeroUsize::new(5);
    let nine = NonZeroUsize::new(9);
    assert_eq!(
        parse_page_selection("1,3-5,9-").unwrap().ranges(),
        &[one..=one, three..=five, nine..=None]
    );
    assert!(parse_page_selection("nope").is_err());
}

#[cfg(feature = "cli")]
#[test]
fn cli_accepts_external_project_resource_options() {
    use clap::Parser as _;

    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--resource-path",
            "resources/first",
            "--resource-path",
            "resources/second",
            "--external-resource",
            "assets/logo.png",
        ])
        .is_ok()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "compile",
            "project.typk",
            "--resource-path",
            "resources/first",
            "--resource-path",
            "resources/second",
        ])
        .is_ok()
    );
}

#[cfg(feature = "cli")]
#[test]
fn cli_uses_typst_embedded_font_terminology() {
    use clap::Parser as _;

    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--embed-fonts",
            "--include-typst-embedded-fonts",
        ])
        .is_ok()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--embed-fonts",
            "--include-default-fonts",
        ])
        .is_err()
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn compile_in_memory_pack_to_pdf_and_svg() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            "#set page(width: 10cm, height: 4cm)\nHello from a pack!"
                .as_bytes()
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack).build().unwrap();

    let pdf = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
    assert_eq!(pdf.artifacts.len(), 1);
    assert!(pdf.artifacts[0].bytes().starts_with(b"%PDF"));

    let svg = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
    assert_eq!(svg.artifacts.len(), 1);
    assert!(
        std::str::from_utf8(svg.artifacts[0].bytes())
            .unwrap()
            .contains("<svg")
    );

    let png = compile(&world, OutputFormat::Png, &CompileOptions::default()).unwrap();
    assert!(
        png.artifacts[0]
            .bytes()
            .starts_with(&[0x89, b'P', b'N', b'G'])
    );
}

#[test]
fn declared_external_project_resource_compiles_through_loader() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")"
                .to_vec(),
        )
        .unwrap()
        .external_resource("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack.clone())
        .external_resource_loader(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .build()
        .unwrap();
    let pdf = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
    assert!(pdf.artifacts[0].bytes().starts_with(b"%PDF"));
    let png = compile(&world, OutputFormat::Png, &CompileOptions::default()).unwrap();
    assert!(
        png.artifacts[0]
            .bytes()
            .starts_with(&[0x89, b'P', b'N', b'G'])
    );
    let svg = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
    assert!(
        std::str::from_utf8(svg.artifacts[0].bytes())
            .unwrap()
            .contains("<svg")
    );

    let world = PackWorld::builder(pack)
        .external_resource_loader(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .feature(typst::Feature::Html)
        .build()
        .unwrap();
    let html = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    assert!(
        std::str::from_utf8(html.artifacts[0].bytes())
            .unwrap()
            .contains("<html")
    );
}

#[test]
fn external_project_resource_loader_cannot_supply_typst_source() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#let _ = read(\"external.typ\")\n#import \"external.typ\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .external_resource("external.typ")
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack)
        .external_resource_loader(MemoryProjectFile::new(
            "external.typ",
            b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
        ))
        .build()
        .unwrap();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
}

#[test]
fn external_project_resource_loaders_follow_registration_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("resource.bin")
        .unwrap()
        .build()
        .unwrap();
    let id = project_file_id("resource.bin");

    let (first, first_calls) = MemoryProjectFile::tracked("resource.bin", b"first".to_vec());
    let (second, second_calls) = MemoryProjectFile::tracked("resource.bin", b"second".to_vec());
    let world = PackWorld::builder(pack.clone())
        .external_resource_loader(first)
        .external_resource_loader(second)
        .build()
        .unwrap();
    assert_eq!(world.file(id).unwrap().as_slice(), b"first");
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);

    let (missing, missing_calls) = MemoryProjectFile::tracked("other.bin", Vec::new());
    let (fallback, fallback_calls) =
        MemoryProjectFile::tracked("resource.bin", b"fallback".to_vec());
    let world = PackWorld::builder(pack.clone())
        .external_resource_loader(missing)
        .external_resource_loader(fallback)
        .build()
        .unwrap();
    assert_eq!(world.file(id).unwrap().as_slice(), b"fallback");
    assert_eq!(missing_calls.load(Ordering::Relaxed), 1);
    assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);

    let (denied, denied_calls) = ErrorProjectLoader::tracked(FileError::AccessDenied);
    let (masked, masked_calls) = MemoryProjectFile::tracked("resource.bin", b"masked".to_vec());
    let world = PackWorld::builder(pack)
        .external_resource_loader(denied)
        .external_resource_loader(masked)
        .build()
        .unwrap();
    assert_eq!(world.file(id), Err(FileError::AccessDenied));
    assert_eq!(denied_calls.load(Ordering::Relaxed), 1);
    assert_eq!(masked_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn packed_and_undeclared_project_paths_do_not_consult_external_loaders() {
    let packed = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("resource.bin", b"packed".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("resource.bin", b"external".to_vec());
    let world = PackWorld::builder(packed)
        .external_resource_loader(loader)
        .build()
        .unwrap();
    assert_eq!(
        world
            .file(project_file_id("resource.bin"))
            .unwrap()
            .as_slice(),
        b"packed"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let undeclared = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("missing.bin", b"external".to_vec());
    let world = PackWorld::builder(undeclared)
        .external_resource_loader(loader)
        .build()
        .unwrap();
    assert!(matches!(
        world.file(project_file_id("missing.bin")),
        Err(FileError::NotFound(_))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn package_requests_do_not_consult_external_project_resource_loaders() {
    use std::str::FromStr as _;

    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("lib.typ")
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("lib.typ", b"external".to_vec());
    let world = PackWorld::builder(pack)
        .external_resource_loader(loader)
        .build()
        .unwrap();
    let spec = typst::syntax::package::PackageSpec::from_str("@local/example:1.0.0").unwrap();
    let id = RootedPath::new(
        VirtualRoot::Package(spec),
        VirtualPath::new("lib.typ").unwrap(),
    )
    .intern();

    assert!(world.file(id).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[cfg(feature = "fs")]
mod fs {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Creates a project directory with an image, a data file, an included
    /// chapter, and an import from a local package, plus the package itself
    /// in a separate directory laid out like a package path.
    pub(crate) fn fixture(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let project = dir.join("project");
        fs::create_dir_all(project.join("chapters")).unwrap();
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            r#"#import "@local/greet:0.1.0": greet
#set page(width: 10cm, height: 8cm)
#include "chapters/intro.typ"
#image("assets/logo.png", width: 8pt)
#greet("World")
Rows: #csv("data.csv").len()
"#,
        )
        .unwrap();
        fs::write(project.join("chapters/intro.typ"), "= Introduction\n").unwrap();
        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        fs::write(project.join("data.csv"), "a,b\n1,2\n").unwrap();
        // A file the compile never reads:
        fs::write(project.join("notes.txt"), "extra").unwrap();

        let packages = dir.join("packages");
        let package = packages.join("local/greet/0.1.0");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("typst.toml"),
            "[package]\nname = \"greet\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        )
        .unwrap();
        fs::write(
            package.join("lib.typ"),
            "#let greet(name) = [Hello #name!]\n",
        )
        .unwrap();

        (project, packages)
    }

    fn pack_fixture(dir: &Path) -> PackOutcome {
        let (project, packages) = fixture(dir);
        Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .include("notes.txt")
            .pack()
            .unwrap()
    }

    #[test]
    fn discovery_finds_all_used_files_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        let report = &outcome.report;
        for expected in [
            "main.typ",
            "chapters/intro.typ",
            "assets/logo.png",
            "data.csv",
            "notes.txt",
        ] {
            assert!(
                report.files.iter().any(|file| file == expected),
                "missing {expected} in {:?}",
                report.files
            );
        }
        assert_eq!(report.packages_vendored.len(), 1);
        assert_eq!(
            report.packages_vendored[0].to_string(),
            "@local/greet:0.1.0"
        );
        assert!(report.packages_unvendored.is_empty());

        let spec = &report.packages_vendored[0];
        assert!(outcome.pack.has_package(spec));
        assert!(outcome.pack.package_file(spec, "lib.typ").is_some());
        assert!(outcome.pack.package_file(spec, "typst.toml").is_some());
    }

    #[test]
    fn externally_loaded_project_resource_survives_the_pack_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_loader(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .pack()
            .unwrap();

        assert_eq!(outcome.report.files, ["main.typ"]);
        assert_eq!(outcome.report.external_resources, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        assert_eq!(
            pack.external_resources().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );
        let world = PackWorld::builder(pack.clone()).build().unwrap();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing External Project Resource did not produce a file diagnostic"),
        }

        let world = PackWorld::builder(pack)
            .external_resource_loader(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build()
            .unwrap();
        let output = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
        assert!(
            std::str::from_utf8(output.artifacts[0].bytes())
                .unwrap()
                .contains("<svg")
        );
    }

    #[test]
    fn explicitly_external_source_project_resource_is_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();
        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("assets/logo.png")
            .pack()
            .unwrap();

        assert_eq!(outcome.report.external_resources, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());
        assert_eq!(
            outcome.pack.external_resources().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        let world = PackWorld::builder(pack.clone()).build().unwrap();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing External Project Resource did not produce a file diagnostic"),
        }
        let world = PackWorld::builder(pack.clone())
            .external_resource_loader(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build()
            .unwrap();
        assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_ok());

        let target = dir.path().join("extracted");
        let report = extract(&pack, &target, &ExtractOptions::default()).unwrap();
        assert!(
            !report
                .written
                .iter()
                .any(|path| path == Path::new("assets/logo.png"))
        );
        assert!(!target.join("assets/logo.png").exists());
    }

    #[test]
    fn unrequested_external_project_resource_is_still_declared() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("conditional/logo.png")
            .pack()
            .unwrap();

        assert_eq!(
            outcome.pack.external_resources().collect::<Vec<_>>(),
            ["conditional/logo.png"]
        );
    }

    #[test]
    fn explicitly_included_file_cannot_be_an_external_project_resource() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();
        fs::write(project.join("conditional.txt"), "packed").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("conditional.txt")
            .include("conditional.txt")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::ExternalResourceConflict(path)))
                if path == "conditional.txt"
        ));
    }

    #[test]
    fn discovery_policy_keeps_source_project_files_authoritative() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();

        let (strict_loader, strict_calls) =
            MemoryProjectFile::tracked("assets/logo.png", tiny_png());
        let strict = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource_loader(strict_loader)
            .pack();
        assert!(matches!(strict, Err(PackerError::Compile { .. })));
        assert_eq!(strict_calls.load(Ordering::Relaxed), 0);

        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        let (fallback, fallback_calls) =
            MemoryProjectFile::tracked("assets/logo.png", b"not the packed image".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_loader(fallback)
            .pack()
            .unwrap();
        assert!(outcome.pack.file("assets/logo.png").is_some());
        assert!(outcome.report.external_resources.is_empty());
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
    }

    #[cfg(unix)]
    #[test]
    fn discovery_reads_project_sources_once_without_external_fallback() {
        use std::io::Write as _;
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"chapter.typ\": chapter\n#chapter",
        )
        .unwrap();

        let chapter = project.join("chapter.typ");
        assert!(
            Command::new("mkfifo")
                .arg(&chapter)
                .status()
                .unwrap()
                .success()
        );
        let writer = std::thread::spawn({
            let chapter = chapter.clone();
            move || {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&chapter)
                    .unwrap();
                fs::remove_file(&chapter).unwrap();
                file.write_all(b"#let chapter = rect(width: 1pt, height: 1pt)")
                    .unwrap();
            }
        });

        let (loader, calls) = MemoryProjectFile::tracked(
            "chapter.typ",
            b"#let chapter = rect(width: 2pt, height: 2pt)".to_vec(),
        );
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_loader(loader)
            .pack()
            .unwrap();
        writer.join().unwrap();

        assert!(outcome.pack.file("chapter.typ").is_some());
        assert!(outcome.report.external_resources.is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn concurrent_external_file_load_cannot_satisfy_discovery_source_request() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let raw_entered = Arc::new(std::sync::Barrier::new(2));
        let raw_release = Arc::new(std::sync::Barrier::new(2));
        let mut outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_loader(BlockingProjectFile {
                path: "external.typ".to_owned(),
                data: Bytes::new(b"#let injected = true".to_vec()),
                entered: Arc::clone(&raw_entered),
                release: Arc::clone(&raw_release),
            })
            .pack()
            .unwrap();
        let id = project_file_id("external.typ");
        let source_entered = Arc::new(std::sync::Barrier::new(2));
        let source_release = Arc::new(std::sync::Barrier::new(2));
        outcome.world.set_source_request_hook({
            let source_entered = Arc::clone(&source_entered);
            let source_release = Arc::clone(&source_release);
            move |source_id| {
                if source_id == id {
                    source_entered.wait();
                    source_release.wait();
                }
            }
        });

        std::thread::scope(|scope| {
            let file = scope.spawn(|| outcome.world.file(id));
            raw_entered.wait();
            let source = scope.spawn(|| outcome.world.source(id));
            source_entered.wait();

            raw_release.wait();
            assert!(file.join().unwrap().is_ok());
            source_release.wait();

            assert!(matches!(
                source.join().unwrap(),
                Err(FileError::NotFound(_))
            ));
        });
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn packed_project_compiles_offline() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        // Round-trip through bytes: nothing may depend on the file system.
        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        let world = PackWorld::builder(pack).build().unwrap();
        let output = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
        assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn unvendored_packages_resolve_through_package_loader() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .vendor_packages(false)
            .pack()
            .unwrap();
        assert!(outcome.report.packages_vendored.is_empty());
        assert_eq!(outcome.report.packages_unvendored.len(), 1);

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();

        // Without a loader, compilation must fail...
        let world = PackWorld::builder(pack.clone()).build().unwrap();
        assert!(compile(&world, OutputFormat::Pdf, &CompileOptions::default()).is_err());

        // ...with a loader pointed at the package path, it succeeds.
        use typst_kit::downloader::SystemDownloader;
        use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};
        let loader = SystemPackageLoader(SystemPackages::from_parts(
            Some(FsPackages::new(&packages)),
            None,
            UniversePackages::new(SystemDownloader::new("typst-pack-test")),
        ));
        let world = PackWorld::builder(pack)
            .package_loader(loader)
            .build()
            .unwrap();
        let output = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
        assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_embedding_skips_typst_embedded_fonts_unless_asked() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());

        let slim = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .embed_fonts(true)
            .pack()
            .unwrap();
        assert!(
            slim.pack.fonts().is_empty(),
            "only Typst embedded fonts are used, so nothing should be embedded"
        );

        let full = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .embed_fonts(true)
            .include_typst_embedded_fonts(true)
            .pack()
            .unwrap();
        assert!(!full.pack.fonts().is_empty());
        // The embedded fonts must load again from the pack.
        let pack = Pack::from_bytes(full.pack.to_bytes().unwrap()).unwrap();
        PackWorld::builder(pack).build().unwrap();
    }

    #[test]
    fn extract_writes_project_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        let target = dir.path().join("extracted");
        let report = extract(
            &outcome.pack,
            &target,
            &ExtractOptions {
                packages: true,
                fonts: true,
                force: false,
            },
        )
        .unwrap();
        assert!(!report.written.is_empty());
        assert!(target.join("main.typ").exists());
        assert!(target.join("assets/logo.png").exists());
        assert!(target.join("packages/local/greet/0.1.0/lib.typ").exists());

        // Refuses to overwrite without force.
        let result = extract(&outcome.pack, &target, &ExtractOptions::default());
        assert!(matches!(result, Err(ExtractError::Exists(_))));
    }

    #[test]
    fn packer_reports_compile_errors() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("broken");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#import \"missing.typ\": x\n").unwrap();

        let result = Packer::new(&project, "main.typ").system_fonts(false).pack();
        assert!(matches!(result, Err(PackerError::Compile { .. })));
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn html_output_is_gated_by_the_html_feature() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", "Hello from HTML".as_bytes().to_vec())
        .unwrap()
        .build()
        .unwrap();

    // Without the feature, Typst itself rejects HTML export.
    let world = PackWorld::builder(pack.clone()).build().unwrap();
    assert!(compile(&world, OutputFormat::Html, &CompileOptions::default()).is_err());

    // With the feature, it produces a document plus an "experimental" warning.
    let world = PackWorld::builder(pack)
        .feature(typst::Feature::Html)
        .build()
        .unwrap();
    let output = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    let html = std::str::from_utf8(output.artifacts[0].bytes()).unwrap();
    assert!(html.contains("<html"));
    assert!(html.contains("Hello from HTML"));
    assert!(!output.warnings.is_empty());
}

#[cfg(feature = "fs")]
mod offline {
    use super::*;
    use std::fs;

    #[test]
    fn offline_packing_works_with_local_packages() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = super::fs::fixture(dir.path());
        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .offline(true)
            .pack()
            .unwrap();
        assert_eq!(outcome.report.packages_vendored.len(), 1);
    }

    #[test]
    fn offline_packing_fails_on_uncached_universe_package() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"@preview/typst-pack-no-such-package:0.0.1\": x\n",
        )
        .unwrap();
        let empty = dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .offline(true)
            .package_path(&empty)
            .package_cache_path(&empty)
            .pack();
        assert!(matches!(result, Err(PackerError::Compile { .. })));
    }
}
