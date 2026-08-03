//! Pack Creation through the reference Creation Adapter, the filesystem one.
//!
//! Every test here drives the public filesystem creation interface over a real
//! project directory: what the adapter acquires — project files under the
//! Project Ignore Policy, package trees from the configured Package Authority,
//! and font containers from the host's font sources — is what the Pack it
//! returns describes. Post-acquisition mutation behavior is covered in-crate
//! because mutating the tree mid-creation needs a test hook this interface does
//! not offer.

#![cfg(feature = "fs")]

#[path = "support/archive.rs"]
mod archive_support;

use std::fs;
use std::path::{Path, PathBuf};

use archive_support::{decode_reference, encode_reference};
use typst_pack::{
    DocumentTime, FilesystemPackAssembler, FilesystemPackAssemblerConfig,
    FilesystemPackAssemblyError, FilesystemPackAssemblyRequest, FilesystemProjectGatherError,
    FilesystemProjectIssue, Pack, PackCreationError, TypstTarget,
};
#[cfg(feature = "egress")]
use typst_pack::{FilesystemPackageAcquisitionError, PackageAcquisitionFailureReason};

/// A project directory with an image, a data file, an included chapter, and an
/// import from a local package, plus the package itself in a separate
/// directory laid out like a package path.
fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
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
    fs::write(package.join("unused.txt"), "complete package").unwrap();

    (project, packages)
}

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

/// The contained project paths of a pack, in canonical order.
fn project_files(pack: &Pack) -> Vec<&str> {
    pack.files().map(|(path, _)| path).collect()
}

fn request(project: &Path) -> FilesystemPackAssemblyRequest<'_> {
    FilesystemPackAssemblyRequest::new(project, Path::new("main.typ"))
}

fn no_font_assembler() -> FilesystemPackAssembler {
    FilesystemPackAssembler::new(FilesystemPackAssemblerConfig::new().system_fonts(false))
}

#[test]
fn structural_creation_packs_all_project_files_and_complete_packages() {
    let dir = tempfile::tempdir().unwrap();
    let (project, packages) = fixture(dir.path());

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .package_path(&packages)
            .system_fonts(false),
    );
    let report = assembler.assemble(request(&project)).unwrap();

    for expected in [
        "main.typ",
        "chapters/intro.typ",
        "assets/logo.png",
        "data.csv",
        "notes.txt",
    ] {
        let files = project_files(report.pack());
        assert!(files.contains(&expected), "missing {expected} in {files:?}");
    }
    assert_eq!(report.pack().package_requirements().len(), 1);
    let requirement = &report.pack().package_requirements()[0];
    assert!(requirement.is_embedded());
    assert_eq!(requirement.spec().to_string(), "@local/greet:0.1.0");

    let spec = requirement.spec();
    assert!(report.pack().has_package(spec));
    assert!(report.pack().package_file(spec, "lib.typ").is_some());
    assert!(report.pack().package_file(spec, "typst.toml").is_some());
    // The whole Complete Package Tree travels, not only what was read.
    assert!(report.pack().package_file(spec, "unused.txt").is_some());

    let reread = decode_reference(encode_reference(report.pack()).unwrap()).unwrap();
    assert_eq!(reread.identity(), report.pack().identity());
}

#[test]
fn structural_creation_applies_the_root_project_ignore_policy() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join("ignored/reincluded")).unwrap();
    fs::create_dir_all(project.join("nested")).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();
    fs::write(project.join("unused.txt"), "packed").unwrap();
    fs::write(
        project.join(".typkignore"),
        "ignored/**\n!ignored/reincluded/\n!ignored/reincluded/keep.txt\n*.secret\n",
    )
    .unwrap();
    fs::write(project.join("ignored/drop.txt"), "drop").unwrap();
    fs::write(project.join("ignored/reincluded/keep.txt"), "keep").unwrap();
    fs::write(project.join("nested/.typkignore"), "*.txt\n").unwrap();
    fs::write(project.join("nested/ordinary.txt"), "packed").unwrap();
    fs::write(project.join("private.secret"), "drop").unwrap();
    fs::write(project.join("old.typk"), "drop").unwrap();

    let report = no_font_assembler().assemble(request(&project)).unwrap();

    assert_eq!(
        project_files(report.pack()),
        [
            ".typkignore",
            "ignored/reincluded/keep.txt",
            "main.typ",
            "nested/.typkignore",
            "nested/ordinary.txt",
            "unused.txt",
        ]
    );
}

#[test]
fn structural_creation_does_not_reinclude_files_beneath_ignored_parents() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join("ignored")).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();
    fs::write(project.join(".typkignore"), "ignored/\n!ignored/keep.txt\n").unwrap();
    fs::write(project.join("ignored/keep.txt"), "still ignored").unwrap();

    let report = no_font_assembler().assemble(request(&project)).unwrap();

    assert_eq!(project_files(report.pack()), [".typkignore", "main.typ"]);
}

#[test]
fn structural_creation_supports_gitignore_pattern_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join("nested/cache")).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();
    fs::write(
        project.join(".typkignore"),
        concat!(
            "# Project policy\n",
            "/root-only.secret\n",
            "\\!important.txt\n",
            "\\#notes.txt\n",
            "cache/\n",
            "*.tmp\n",
            "!keep.tmp\n",
            " leading.txt\n",
            "trailing.txt   \n",
            "literal\\ \n",
        ),
    )
    .unwrap();
    fs::write(project.join("# Project policy"), "packed").unwrap();
    fs::write(project.join("root-only.secret"), "ignored").unwrap();
    fs::write(project.join("nested/root-only.secret"), "packed").unwrap();
    fs::write(project.join("!important.txt"), "ignored").unwrap();
    fs::write(project.join("#notes.txt"), "ignored").unwrap();
    fs::write(project.join("nested/cache/data.txt"), "ignored").unwrap();
    fs::write(project.join("drop.tmp"), "ignored").unwrap();
    fs::write(project.join("nested/keep.tmp"), "packed").unwrap();
    fs::write(project.join(" leading.txt"), "ignored").unwrap();
    fs::write(project.join("trailing.txt"), "ignored").unwrap();
    fs::write(project.join("literal "), "ignored").unwrap();

    let report = no_font_assembler().assemble(request(&project)).unwrap();

    assert_eq!(
        project_files(report.pack()),
        [
            "# Project policy",
            ".typkignore",
            "main.typ",
            "nested/keep.tmp",
            "nested/root-only.secret",
        ]
    );
}

#[cfg(unix)]
#[test]
fn structural_creation_prunes_conclusively_ignored_subtrees() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(project.join("ignored")).unwrap();
    fs::create_dir_all(project.join("other")).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();
    fs::write(project.join(".typkignore"), "ignored/\n!/other/keep.txt\n").unwrap();
    fs::write(project.join("other/keep.txt"), "keep").unwrap();
    symlink(dir.path(), project.join("ignored/outside")).unwrap();

    let report = no_font_assembler().assemble(request(&project)).unwrap();

    assert_eq!(
        project_files(report.pack()),
        [".typkignore", "main.typ", "other/keep.txt"]
    );
}

#[cfg(unix)]
#[test]
fn structural_creation_rejects_a_symlinked_root_ignore_policy() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();
    fs::write(dir.path().join("outside-ignore"), "*.txt\n").unwrap();
    symlink(
        dir.path().join("outside-ignore"),
        project.join(".typkignore"),
    )
    .unwrap();

    let policy = project.canonicalize().unwrap().join(".typkignore");
    let result = no_font_assembler().assemble(request(&project));

    assert!(matches!(
        result,
        Err(FilesystemPackAssemblyError::ProjectGather(FilesystemProjectGatherError::Survey(ref survey)))
            if matches!(survey.issues(), [FilesystemProjectIssue::Alias { path }] if path == &policy)
    ));
}

#[test]
fn the_adapter_preserves_the_timestamp_range_error() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("main.typ"), "Hello").unwrap();

    let result = no_font_assembler()
        .assemble(request(&project).document_time(DocumentTime::UnixTimestamp(i64::MAX)));

    let Err(error) = result else {
        panic!("the invalid Discovery Specification was accepted");
    };
    assert!(matches!(
        &error,
        FilesystemPackAssemblyError::DiscoverySpecification(_)
    ));
    assert_eq!(
        error.to_string(),
        "invalid Discovery Specification: the discovery document-time UNIX timestamp is out of range"
    );
}

#[test]
fn creation_target_does_not_select_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        "#context if target() == \"html\" { read(\"html.txt\") } else { read(\"paged.txt\") }",
    )
    .unwrap();
    fs::write(project.join("paged.txt"), "paged").unwrap();
    fs::write(project.join("html.txt"), "html").unwrap();

    let report = no_font_assembler()
        .assemble(
            request(&project)
                .feature(typst::Feature::Html)
                .target(TypstTarget::Html),
        )
        .unwrap();

    assert_eq!(
        project_files(report.pack()),
        ["html.txt", "main.typ", "paged.txt"]
    );
}

#[test]
fn exact_inputs_and_document_time_drive_representative_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        r#"#if sys.inputs.at("pick") == "yes" { read("input.txt") }
#if datetime.today().year() == 2024 { read("time.txt") }"#,
    )
    .unwrap();
    fs::write(project.join("input.txt"), "input").unwrap();
    fs::write(project.join("time.txt"), "time").unwrap();
    let mut inputs = typst::foundations::Dict::new();
    inputs.insert("pick".into(), typst::foundations::Value::Str("yes".into()));

    let report = no_font_assembler()
        .assemble(
            request(&project)
                .inputs(inputs)
                .document_time(DocumentTime::UnixTimestamp(1_704_067_200)),
        )
        .unwrap();

    assert_eq!(
        project_files(report.pack()),
        ["input.txt", "main.typ", "time.txt"]
    );
}

#[test]
fn package_data_precedes_package_cache_during_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        "#import \"@local/chosen:0.1.0\": mark\n#mark",
    )
    .unwrap();

    let data_package = dir.path().join("data/local/chosen/0.1.0");
    fs::create_dir_all(&data_package).unwrap();
    fs::write(
        data_package.join("typst.toml"),
        "[package]\nname = \"chosen\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    fs::write(
        data_package.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();

    let cache_package = dir.path().join("cache/local/chosen/0.1.0");
    fs::create_dir_all(&cache_package).unwrap();
    fs::write(cache_package.join("lib.typ"), "this is not valid Typst: {").unwrap();

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .package_path(dir.path().join("data"))
            .package_cache_path(dir.path().join("cache"))
            .system_fonts(false),
    );
    let report = assembler.assemble(request(&project)).unwrap();

    assert_eq!(report.pack().package_requirements().len(), 1);
}

#[test]
fn package_cache_resolves_during_online_and_offline_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        "#import \"@preview/cached:0.1.0\": mark\n#mark",
    )
    .unwrap();

    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let cache = dir.path().join("cache/preview/cached/0.1.0");
    fs::create_dir_all(&cache).unwrap();
    fs::write(
        cache.join("typst.toml"),
        "[package]\nname = \"cached\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    fs::write(
        cache.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();

    for offline in [false, true] {
        let assembler = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .package_path(&data)
                .package_cache_path(dir.path().join("cache"))
                .offline(offline)
                .system_fonts(false),
        );
        let report = assembler.assemble(request(&project)).unwrap();

        let spec = report.pack().package_requirements()[0].spec();
        assert_eq!(spec.to_string(), "@preview/cached:0.1.0");
        assert!(report.pack().package_file(spec, "lib.typ").is_some());
    }
}

#[test]
fn a_project_needing_chained_packages_is_resolved_to_completion() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        "#import \"@local/outer:0.1.0\": mark\n#mark",
    )
    .unwrap();

    let packages = dir.path().join("packages");
    write_package(
        &packages,
        "outer",
        "#import \"@local/inner:0.1.0\": inner\n#let mark = inner",
    );
    write_package(
        &packages,
        "inner",
        "#let inner = rect(width: 1pt, height: 1pt)",
    );

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .package_path(&packages)
            .system_fonts(false),
    );
    let report = assembler.assemble(request(&project)).unwrap();

    // The package only another package's tree imports is resolved too, however
    // many rounds of creation that took.
    assert_eq!(
        report
            .pack()
            .package_requirements()
            .iter()
            .map(|requirement| requirement.spec().to_string())
            .collect::<Vec<_>>(),
        ["@local/inner:0.1.0", "@local/outer:0.1.0"]
    );
}

/// Writes one local package whose `lib.typ` holds `body`.
fn write_package(packages: &Path, name: &str, body: &str) {
    let package = packages.join(format!("local/{name}/0.1.0"));
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("typst.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n"),
    )
    .unwrap();
    fs::write(package.join("lib.typ"), body).unwrap();
}

#[test]
fn offline_creation_works_with_local_packages() {
    let dir = tempfile::tempdir().unwrap();
    let (project, packages) = fixture(dir.path());

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .package_path(&packages)
            .system_fonts(false)
            .offline(true),
    );
    let report = assembler.assemble(request(&project)).unwrap();

    assert_eq!(report.pack().package_requirements().len(), 1);
}

#[cfg(feature = "egress")]
#[test]
fn offline_creation_fails_on_an_uncached_universe_package() {
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

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .system_fonts(false)
            .offline(true)
            .package_path(&empty)
            .package_cache_path(&empty),
    );
    let result = assembler.assemble(request(&project));

    assert!(matches!(
        &result,
        Err(FilesystemPackAssemblyError::Creation(error))
            if matches!(
                error.package_failures(),
                [FilesystemPackageAcquisitionError::Unavailable(failure)]
                    if failure.spec().to_string()
                        == "@preview/typst-pack-no-such-package:0.0.1"
                        && failure.reason() == &PackageAcquisitionFailureReason::NotFound
            )
    ));

    // A specification the Package Authority cannot resolve fails the
    // representative request at the import that needed it, carrying the
    // authority's own reason, so the failure keeps the source location the
    // caller can act on.
    assert_eq!(
        unresolvable_package_diagnostics(result),
        // Offline never reaches the network, so it never reports one.
        ["package not found (searched for @preview/typst-pack-no-such-package:0.0.1)"]
    );
}

/// A build without egress has no download capability under any runtime
/// configuration: a Universe package no local source holds fails acquisition
/// even though nothing asked for offline creation.
#[cfg(not(feature = "egress"))]
#[test]
fn creation_without_egress_never_downloads_a_universe_package() {
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

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .system_fonts(false)
            .package_path(&empty),
    );
    let result = assembler.assemble(request(&project));

    assert_eq!(
        unresolvable_package_diagnostics(result),
        ["package not found (searched for @preview/typst-pack-no-such-package:0.0.1)"]
    );
}

/// The messages of the representative request that failed because the Package
/// Authority could not resolve a specification creation reported.
///
/// Asserting on them is asserting that the failure reaches the import: only a
/// failed representative request carries a span, and the adapter has none of
/// its own to offer.
fn unresolvable_package_diagnostics(
    result: Result<typst_pack::PackAssemblyReport, FilesystemPackAssemblyError>,
) -> Vec<String> {
    let Err(FilesystemPackAssemblyError::Creation(error)) = result else {
        panic!("an unresolvable package did not fail the representative request");
    };
    let PackCreationError::DependencyDiscoveryRejected(rejection) = error.error() else {
        panic!("the compile failure did not retain its discovery rejection");
    };
    let errors = rejection.diagnostics();
    for error in errors {
        assert!(
            !error.span.is_detached(),
            "the failure is reported away from the import that needed the package"
        );
    }
    errors
        .iter()
        .map(|error| error.message.to_string())
        .collect()
}

#[test]
fn a_representative_compile_that_fails_issues_no_pack() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("broken");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("main.typ"), "#import \"missing.typ\": x\n").unwrap();

    let result = no_font_assembler().assemble(request(&project));

    assert!(matches!(
        result,
        Err(FilesystemPackAssemblyError::Creation(error))
            if matches!(
                error.error(),
                PackCreationError::DependencyDiscoveryRejected(_)
            )
    ));
}

#[test]
fn creation_retains_representative_compile_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("main.typ"),
        "#set text(font: \"Definitely Missing\")\nWarning",
    )
    .unwrap();

    let assembler = FilesystemPackAssembler::new(
        FilesystemPackAssemblerConfig::new()
            .system_fonts(false)
            .typst_embedded_fonts(false),
    );
    let report = assembler.assemble(request(&project)).unwrap();

    assert!(
        report
            .warnings()
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{:?}",
        report.warnings()
    );
}

/// Face selection out of the catalog the adapter composes from host font
/// sources.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use super::*;

    #[test]
    fn font_embedding_skips_typst_embedded_fonts_unless_asked() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());

        let assembler = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .package_path(&packages)
                .system_fonts(false),
        );
        let slim = assembler
            .assemble(request(&project).embed_fonts(true))
            .unwrap();
        assert!(
            slim.pack().fonts().is_empty(),
            "only Typst embedded fonts are used, so nothing should be embedded"
        );

        let full = assembler
            .assemble(
                request(&project)
                    .embed_fonts(true)
                    .include_typst_embedded_fonts(true),
            )
            .unwrap();
        assert!(!full.pack().fonts().is_empty());
        // The embedded containers must load again from the pack.
        decode_reference(encode_reference(full.pack()).unwrap()).unwrap();
    }

    #[test]
    fn html_creation_embeds_fonts_used_inside_frames() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#html.frame[Hello]").unwrap();

        let report = no_font_assembler()
            .assemble(
                request(&project)
                    .target(TypstTarget::Html)
                    .feature(typst::Feature::Html)
                    .embed_fonts(true)
                    .include_typst_embedded_fonts(true),
            )
            .unwrap();

        assert!(!report.pack().fonts().is_empty());
    }

    #[test]
    fn a_created_pack_compiles_with_no_filesystem() {
        use typst_pack::{
            CompilationOutputSpecification, FontContainerFulfillment, PackCompilationRequest,
            PdfOutputSpecification, compile,
        };

        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let assembler = FilesystemPackAssembler::new(
            FilesystemPackAssemblerConfig::new()
                .package_path(&packages)
                .system_fonts(false),
        );
        let report = assembler.assemble(request(&project)).unwrap();

        // Round-trip through bytes: nothing may depend on the filesystem.
        let pack = decode_reference(encode_reference(report.pack()).unwrap()).unwrap();
        let mut request = PackCompilationRequest::new(
            pack.clone(),
            CompilationOutputSpecification::Pdf(PdfOutputSpecification::default()),
        );
        // The fonts the adapter offered were Typst's own, declared externally.
        for requirement in pack.font_requirements() {
            let identity = requirement.container_identity();
            let data = typst_kit::fonts::embedded()
                .map(|(font, _)| font.data().to_vec())
                .find(|data| {
                    typst_pack::FontContainerIdentity::from_bytes(data.as_slice()) == identity
                })
                .expect("a Typst embedded container fulfills the requirement");
            request = request.font_fulfillment(identity, FontContainerFulfillment::new(data));
        }

        let report = compile(request).unwrap();
        let result = report.result().expect("semantic Compilation Result");
        assert!(result.artifacts()[0].bytes().starts_with(b"%PDF"));
    }
}
