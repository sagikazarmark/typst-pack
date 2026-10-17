//! Crate tests that need private access to the Pack World or compile kernel.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use typst::World;
use typst::foundations::{Bytes, Dict};
use typst::syntax::package::PackageSpec;
use typst::syntax::{RootedPath, VirtualPath, VirtualRoot};

use crate::compile::{CompileError, compile_world as compile};
use crate::world::{PackWorld, PackWorldConstructionError};
use crate::*;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

fn pack_world_with_features(pack: Pack, features: Vec<typst::Feature>) -> PackWorld {
    let dependencies =
        pack.materialize_compilation_dependency_snapshot(BTreeMap::new(), BTreeMap::new());
    PackWorld::new(
        pack,
        dependencies,
        BTreeMap::new(),
        Dict::new(),
        features,
        DocumentTime::Absent,
    )
    .unwrap()
}

fn pack_world(pack: Pack) -> PackWorld {
    pack_world_with_features(pack, vec![])
}

fn spec(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

fn issues(result: Result<Pack, PackInvariantError>) -> Vec<PackInvariantIssue> {
    result.unwrap_err().issues().to_vec()
}

#[cfg(feature = "embedded-fonts")]
fn embedded_font_data() -> Vec<u8> {
    typst_kit::fonts::embedded()
        .next()
        .expect("Typst embedded fonts are available")
        .0
        .data()
        .to_vec()
}

#[test]
fn png_export_error_preserves_the_failing_source_page() {
    let error = CompileError::PngExport {
        message: "encoding failed".to_owned(),
        warnings: ecow::EcoVec::new(),
        pack_warnings: ecow::EcoVec::new(),
        source_page_count: 3,
        source_page_number: std::num::NonZeroUsize::new(2).unwrap(),
    };
    assert_eq!(
        error.to_string(),
        "PNG export failed for source page 2: encoding failed"
    );
}

#[test]
fn pack_construction_requires_a_contained_entrypoint() {
    assert_eq!(
        issues(Pack::builder("main.typ").build()),
        [PackInvariantIssue::MissingEntrypoint {
            path: "main.typ".to_owned()
        }]
    );
}

#[test]
fn pack_builder_rejects_paths_that_cannot_name_root_relative_files() {
    for entrypoint in ["", "main\0.typ", "../main.typ"] {
        assert!(matches!(
            issues(Pack::builder(entrypoint).build()).as_slice(),
            [PackInvariantIssue::InvalidEntrypoint { .. }]
        ));
    }
    for path in [
        "/main.typ",
        "C:outside.txt",
        "./C:/outside.txt",
        "main\0.typ",
        "../escape.typ",
        "nested/pack.typk",
    ] {
        let result = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .file(path, Vec::new())
            .build();
        assert!(
            matches!(
                issues(result).as_slice(),
                [PackInvariantIssue::InvalidPath { .. }]
            ),
            "{path}"
        );
    }
}

#[test]
fn pack_construction_rejects_conflicting_project_paths() {
    let result = Pack::builder("main.typ")
        .file("main.typ", b"first".to_vec())
        .file("./main.typ", b"second".to_vec())
        .file("assets", Vec::new())
        .file("assets/logo.png", tiny_png())
        .build();
    assert_eq!(
        issues(result),
        [
            PackInvariantIssue::PathTreeConflict {
                ancestor: "assets".to_owned(),
                descendant: "assets/logo.png".to_owned()
            },
            PackInvariantIssue::DuplicateProjectPath {
                path: "main.typ".to_owned()
            },
        ]
    );
}

#[test]
fn pack_construction_rejects_invalid_packages() {
    let mut invalid = spec("@local/example:1.0.0");
    invalid.name = "bad/name".into();
    let result = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .package_file(invalid, "lib.typ", Vec::new())
        .package_file(spec("@local/tree:1.0.0"), "lib.typ", b"a".to_vec())
        .package_file(spec("@local/tree:1.0.0"), "./lib.typ", b"b".to_vec())
        .package_file(spec("@local/both:1.0.0"), "lib.typ", Vec::new())
        .external_package_file(spec("@local/both:1.0.0"), "lib.typ", Vec::new())
        .build();
    let issues = issues(result);
    assert_eq!(issues.len(), 3, "{issues:#?}");
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, PackInvariantIssue::InvalidPackageSpec { .. }))
    );
    assert!(issues.iter().any(|issue| matches!(
        issue,
        PackInvariantIssue::InvalidPackageTree { spec, .. } if spec.name == "tree"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        PackInvariantIssue::DuplicatePackage { spec } if spec.name == "both"
    )));
}

#[test]
fn pack_issues_do_not_depend_on_input_order() {
    let build = |reverse: bool| {
        let mut files = vec![
            ("b.typ", "b"),
            ("b.typ/c", "c"),
            ("a//x", "x"),
            ("/abs", ""),
        ];
        if reverse {
            files.reverse();
        }
        files
            .into_iter()
            .fold(Pack::builder("main.typ"), |builder, (path, data)| {
                builder.file(path, data.as_bytes().to_vec())
            })
            .build()
            .unwrap_err()
    };
    assert_eq!(build(false), build(true));
}

#[test]
fn pack_construction_rejects_unreadable_and_duplicate_fonts() {
    let result = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .font(b"not a font".to_vec())
        .build();
    assert_eq!(
        issues(result),
        [PackInvariantIssue::InvalidFontData { position: 0 }]
    );

    #[cfg(feature = "embedded-fonts")]
    {
        let data = embedded_font_data();
        let result = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .font(data.clone())
            .external_font(data.clone())
            .build();
        assert_eq!(
            issues(result),
            [PackInvariantIssue::DuplicateFont {
                digest: Digest::of(&data)
            }]
        );
    }
}

#[test]
fn a_constructed_pack_builds_a_world_without_revalidation() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .build()
        .unwrap();
    let _: PackWorld = pack_world(pack);
}

#[test]
fn pack_world_construction_rejects_invalid_overrides() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .build()
        .unwrap();
    let dependencies =
        pack.materialize_compilation_dependency_snapshot(BTreeMap::new(), BTreeMap::new());
    let overrides = BTreeMap::from([(
        "missing.typ".to_owned(),
        Bytes::new(b"replacement".to_vec()),
    )]);
    assert!(matches!(
        PackWorld::new(
            pack,
            dependencies,
            overrides,
            Dict::new(),
            vec![],
            DocumentTime::Absent,
        ),
        Err(PackWorldConstructionError::InvalidProjectOverride { path })
            if path == "missing.typ"
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
        .build()
        .unwrap();
    let world = pack_world(pack);

    let pdf = compile(
        &world,
        &CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
    )
    .unwrap();
    assert!(pdf.artifacts[0].bytes().starts_with(b"%PDF"));

    let svg = compile(
        &world,
        &CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
    )
    .unwrap();
    assert!(
        std::str::from_utf8(svg.artifacts[0].bytes())
            .unwrap()
            .contains("<svg")
    );

    let png = compile(
        &world,
        &CompilationOutputSpecification::Png(PngOutputSpecification::default()),
    )
    .unwrap();
    assert!(png.artifacts[0].bytes().starts_with(b"\x89PNG"));
}

#[test]
fn pdf_default_timestamp_is_resolved_after_compilation() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#read(\"timestamp-trigger.bin\")\n#rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .file("timestamp-trigger.bin", b"read".to_vec())
        .build()
        .unwrap();
    let world = pack_world(pack);
    let timestamp = typst_pdf::Timestamp::new_utc(
        typst::foundations::Datetime::from_ymd_hms(2000, 1, 2, 3, 4, 5).unwrap(),
    );
    let default_resolutions = AtomicUsize::new(0);

    let default_output = crate::compile::compile_with_default_pdf_timestamp(
        &world,
        &CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        CompilationLimits::DEFAULT,
        || {
            default_resolutions.fetch_add(1, Ordering::Relaxed);
            Some(timestamp)
        },
    )
    .unwrap();
    let explicit_output = crate::compile::compile_with_default_pdf_timestamp(
        &world,
        &CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            creation_timestamp: CreationTimestamp::Explicit(timestamp),
            ..PdfOutputSpecification::default()
        }),
        CompilationLimits::DEFAULT,
        || panic!("an explicit timestamp must not resolve the default"),
    )
    .unwrap();

    assert_eq!(default_resolutions.load(Ordering::Relaxed), 1);
    assert_eq!(
        default_output.artifacts[0].bytes(),
        explicit_output.artifacts[0].bytes()
    );
}

#[test]
fn undeclared_package_requests_have_no_ambient_fallback() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .build()
        .unwrap();
    let world = pack_world(pack);
    let id = RootedPath::new(
        VirtualRoot::Package(spec("@local/undeclared:1.0.0")),
        VirtualPath::new("lib.typ").unwrap(),
    )
    .intern();
    assert!(world.file(id).is_err());
}

#[test]
fn vendored_package_compiles_from_the_pack_without_ambient_fallback() {
    let build = |entrypoint: &str| {
        Pack::builder("main.typ")
            .file(
                "main.typ",
                b"#import \"@local/inside:1.0.0\": mark\n#mark".to_vec(),
            )
            .package_file(
                spec("@local/inside:1.0.0"),
                "typst.toml",
                format!(
                    "[package]\nname = \"inside\"\nversion = \"1.0.0\"\nentrypoint = \"{entrypoint}\"\n"
                ),
            )
            .package_file(
                spec("@local/inside:1.0.0"),
                "lib.typ",
                b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
            )
            .build()
            .unwrap()
    };
    let svg = CompilationOutputSpecification::Svg(SvgOutputSpecification::default());
    assert!(compile(&pack_world(build("lib.typ")), &svg).is_ok());
    assert!(compile(&pack_world(build("missing.typ")), &svg).is_err());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn html_output_is_gated_by_the_html_feature() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", "Hello from HTML".as_bytes().to_vec())
        .build()
        .unwrap();
    let html = CompilationOutputSpecification::Html(HtmlOutputSpecification::default());

    assert!(compile(&pack_world(pack.clone()), &html).is_err());

    let world = pack_world_with_features(pack, vec![typst::Feature::Html]);
    let output = compile(&world, &html).unwrap();
    let text = std::str::from_utf8(output.artifacts[0].bytes()).unwrap();
    assert!(text.contains("<html"));
    assert!(text.contains("Hello from HTML"));
    assert!(!output.warnings.is_empty());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn exact_font_catalog_is_authoritative() {
    let embedded_data = embedded_font_data();
    let mut pack_data = embedded_data.clone();
    pack_data.push(0);
    let pack_font = typst::text::Font::new(Bytes::new(pack_data.clone()), 0).unwrap();
    let family = pack_font.info().family.to_lowercase();
    let variant = pack_font.info().variant;
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .font(pack_data.clone())
        .build()
        .unwrap();

    let world = pack_world(pack);
    let selected = world.book().select(&family, variant).unwrap();
    let selected = world.font(selected).unwrap();

    assert_ne!(pack_data, embedded_data);
    assert_eq!(selected.data().as_slice(), pack_data);
}

#[cfg(feature = "fs")]
mod fs {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Resolves a temporary directory to a destination the write preflight accepts.
    ///
    /// macOS temp paths can start with `/var`, a symlink to `/private/var`.
    fn temp_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
        std::fs::canonicalize(directory.path()).unwrap()
    }

    /// Creates a project directory with an image, a data file, an included
    /// chapter, and an import from a local package, plus the package itself
    /// in a separate directory laid out like a package path.
    fn fixture(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
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
        fs::write(package.join("unused.txt"), "complete package").unwrap();

        (project, packages)
    }

    fn assemble(project: &Path, config: FilesystemPackAssemblerConfig) -> PackAssemblyReport {
        FilesystemPackAssembler::new(config.system_fonts(false))
            .assemble(FilesystemPackAssemblyRequest::new(
                project,
                Path::new("main.typ"),
            ))
            .unwrap()
    }

    #[test]
    fn a_project_change_after_reading_does_not_replace_read_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("ignored")).unwrap();
        fs::write(project.join(".typkignore"), "ignored/\n").unwrap();
        let main = project.join("main.typ");
        fs::write(&main, "original").unwrap();
        let report =
            FilesystemPackAssembler::new(FilesystemPackAssemblerConfig::new().system_fonts(false))
                .after_creation_hook({
                    let project = project.clone();
                    move || {
                        fs::write(project.join("main.typ"), "changed").unwrap();
                        fs::write(project.join("added.txt"), "added").unwrap();
                        fs::write(project.join("ignored/added.txt"), "added").unwrap();
                    }
                })
                .assemble(FilesystemPackAssemblyRequest::new(
                    &project,
                    Path::new("main.typ"),
                ))
                .unwrap();

        assert_eq!(report.pack().file("main.typ"), Some(&b"original"[..]));
        assert!(report.pack().file("added.txt").is_none());
        assert!(report.pack().file("ignored/added.txt").is_none());
    }

    #[test]
    fn a_package_change_after_read_does_not_replace_read_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let library = packages.join("local/greet/0.1.0/lib.typ");
        let report = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .package_path(&packages)
                .system_fonts(false),
        )
        .after_creation_hook(move || {
            fs::write(&library, "#let greet(name) = [Changed #name!]\n").unwrap()
        })
        .assemble(FilesystemPackAssemblyRequest::new(
            &project,
            Path::new("main.typ"),
        ))
        .unwrap();

        assert_eq!(
            report
                .pack()
                .package_file(&spec("@local/greet:0.1.0"), "lib.typ"),
            Some(&b"#let greet(name) = [Hello #name!]\n"[..])
        );
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn a_selected_font_is_not_reread_after_pack_creation() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let fonts = dir.path().join("fonts");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&fonts).unwrap();
        let data = embedded_font_data();
        let family = typst::text::FontInfo::new(&data, 0).unwrap().family;
        let font_path = fonts.join("selected.ttf");
        fs::write(&font_path, &data).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("#set text(font: \"{family}\")\nselected"),
        )
        .unwrap();

        let report = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .system_fonts(false)
                .typst_embedded_fonts(false)
                .font_path(&fonts),
        )
        .after_creation_hook(move || fs::write(&font_path, b"changed").unwrap())
        .assemble(
            FilesystemPackAssemblyRequest::new(&project, Path::new("main.typ")).embed_fonts(true),
        )
        .unwrap();

        let requirements = report.pack().font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(
            report.pack().font_data(&requirements[0].digest),
            Some(&data[..])
        );
    }

    #[test]
    fn extract_writes_project_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let root = temp_path(&dir);
        let (project, packages) = fixture(&root);
        let report = assemble(
            &project,
            FilesystemPackAssemblerConfig::new().package_path(&packages),
        );

        let target = root.join("extracted");
        let plan =
            plan_pack_extraction(report.pack(), PackExtractionSelection::new(true, true)).unwrap();
        write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::CreateNew).unwrap();
        assert!(target.join("main.typ").exists());
        assert!(target.join("assets/logo.png").exists());
        assert!(target.join("packages/local/greet/0.1.0/lib.typ").exists());

        assert!(matches!(
            write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::CreateNew),
            Err(FilesystemWriteError::Rejected { issues })
                if matches!(issues.as_slice(), [WriteIssue::ExistingTarget { .. }, ..])
        ));
    }

    #[test]
    fn extraction_rejects_project_package_conflicts_before_writing() {
        let projected_package = "packages/local/example/1.0.0/lib.typ";
        for project_path in [projected_package, "packages/local/example/1.0.0"] {
            let pack = Pack::builder("main.typ")
                .file("main.typ", b"main".to_vec())
                .file(project_path, b"project".to_vec())
                .package_file(spec("@local/example:1.0.0"), "lib.typ", b"package".to_vec())
                .build()
                .unwrap();
            assert!(
                plan_pack_extraction(&pack, PackExtractionSelection::new(true, false)).is_err()
            );
        }
    }

    #[test]
    fn extraction_preflights_existing_destination_conflicts() {
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file("z.txt", b"packed".to_vec())
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = temp_path(&dir);
        let target = root.join("extracted");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("z.txt"), b"external").unwrap();

        let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
        assert!(matches!(
            write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::CreateNew),
            Err(FilesystemWriteError::Rejected { issues })
                if matches!(issues.as_slice(), [WriteIssue::ExistingTarget { .. }])
        ));
        assert!(!target.join("main.typ").exists());
        assert_eq!(fs::read(target.join("z.txt")).unwrap(), b"external");

        write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::Overwrite).unwrap();
        assert_eq!(fs::read(target.join("main.typ")).unwrap(), b"main");
        assert_eq!(fs::read(target.join("z.txt")).unwrap(), b"packed");

        let blocked_target = root.join("blocked");
        fs::create_dir(&blocked_target).unwrap();
        fs::write(blocked_target.join("tree"), b"external").unwrap();
        let nested_pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file("tree/nested.txt", b"nested".to_vec())
            .build()
            .unwrap();
        let nested_plan =
            plan_pack_extraction(&nested_pack, PackExtractionSelection::default()).unwrap();
        assert!(matches!(
            write_pack_extraction_plan_to_filesystem(
                &nested_plan,
                &blocked_target,
                WritePolicy::Overwrite,
            ),
            Err(FilesystemWriteError::Rejected { issues })
                if matches!(issues.as_slice(), [WriteIssue::ConflictingAncestor { .. }])
        ));
        assert!(!blocked_target.join("main.typ").exists());
        assert_eq!(fs::read(blocked_target.join("tree")).unwrap(), b"external");
    }

    #[cfg(unix)]
    #[test]
    fn extraction_rejects_symlinked_destination_components() {
        use std::os::unix::fs::symlink;

        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file("assets/logo.txt", b"packed".to_vec())
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = temp_path(&dir);
        let target = root.join("extracted");
        let outside = root.join("outside");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&outside).unwrap();
        symlink(&outside, target.join("assets")).unwrap();

        let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
        assert!(matches!(
            write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::Overwrite),
            Err(FilesystemWriteError::Rejected { issues })
                if matches!(issues.as_slice(), [WriteIssue::ConflictingAncestor { .. }])
        ));
        assert!(!target.join("main.typ").exists());
        assert!(!outside.join("logo.txt").exists());
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn extraction_writes_each_font_container_once() {
        let data = embedded_font_data();
        let font_pack = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .font(data.clone())
            .build()
            .unwrap();
        let font_path =
            crate::pack_archive::font_member_name(&font_pack.font_requirements()[0].digest, &data);

        let conflicting = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .file(&font_path, b"project".to_vec())
            .font(data)
            .build()
            .unwrap();
        assert!(
            plan_pack_extraction(&conflicting, PackExtractionSelection::new(false, true)).is_err()
        );

        let dir = tempfile::tempdir().unwrap();
        let target = temp_path(&dir).join("extracted");
        let plan =
            plan_pack_extraction(&font_pack, PackExtractionSelection::new(false, true)).unwrap();
        write_pack_extraction_plan_to_filesystem(&plan, &target, WritePolicy::CreateNew).unwrap();
        assert_eq!(plan.entries().len(), 2);
        assert!(target.join(font_path).is_file());
    }
}
