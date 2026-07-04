//! Crate tests.

use crate::*;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

#[test]
fn manifest_roundtrip() {
    let manifest = Manifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"

        [packages]
        vendored = ["@preview/cetz:0.3.4"]

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

    let reparsed = Manifest::from_toml(&manifest.to_toml()).unwrap();
    assert_eq!(manifest, reparsed);
}

#[test]
fn manifest_rejects_future_version() {
    let result = Manifest::from_toml("format-version = 99\n[project]\nentrypoint = \"main.typ\"\n");
    assert!(matches!(result, Err(ManifestError::UnsupportedVersion(99))));
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
fn parse_pages_understands_ranges() {
    use std::num::NonZeroUsize;
    let one = NonZeroUsize::new(1);
    let three = NonZeroUsize::new(3);
    let five = NonZeroUsize::new(5);
    let nine = NonZeroUsize::new(9);
    assert_eq!(
        parse_pages("1,3-5,9-").unwrap(),
        vec![one..=one, three..=five, nine..=None]
    );
    assert!(parse_pages("nope").is_err());
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
    assert_eq!(pdf.outputs.len(), 1);
    assert!(pdf.outputs[0].starts_with(b"%PDF"));

    let svg = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
    assert_eq!(svg.outputs.len(), 1);
    assert!(
        std::str::from_utf8(&svg.outputs[0])
            .unwrap()
            .contains("<svg")
    );

    let png = compile(&world, OutputFormat::Png, &CompileOptions::default()).unwrap();
    assert!(png.outputs[0].starts_with(&[0x89, b'P', b'N', b'G']));
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
        assert!(report.packages_external.is_empty());

        let spec = &report.packages_vendored[0];
        assert!(outcome.pack.has_package(spec));
        assert!(outcome.pack.package_file(spec, "lib.typ").is_some());
        assert!(outcome.pack.package_file(spec, "typst.toml").is_some());
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
        assert!(output.outputs[0].starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn external_packages_resolve_through_package_loader() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .vendor_packages(false)
            .pack()
            .unwrap();
        assert!(outcome.report.packages_vendored.is_empty());
        assert_eq!(outcome.report.packages_external.len(), 1);

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
        assert!(output.outputs[0].starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_embedding_skips_default_fonts_unless_asked() {
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
            "only default fonts are used, so nothing should be embedded"
        );

        let full = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .embed_fonts(true)
            .include_default_fonts(true)
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
    let html = std::str::from_utf8(&output.outputs[0]).unwrap();
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
