//! Pack creation through the filesystem packer.
//!
//! Every test here drives the public filesystem creation interface over a real
//! project directory: what the adapter reads — project files under the
//! Project Ignore Policy, package trees from the configured Package Authority,
//! and font containers from the host's font sources — is what the Pack it
//! returns describes. Post-read mutation behavior is covered in-crate
//! because mutating the tree mid-creation needs a test hook this interface does
//! not offer.

#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};

use typst_pack::Pack;
use typst_pack::compile::{DocumentTime, Input, RequestIssue};
use typst_pack::{compile, create, fs};

/// A project directory with an image, a data file, an included chapter, and an
/// import from a local package, plus the package itself in a separate
/// directory laid out like a package path.
fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
    let project = dir.join("project");
    std::fs::create_dir_all(project.join("chapters")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(
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
    std::fs::write(project.join("chapters/intro.typ"), "= Introduction\n").unwrap();
    std::fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
    std::fs::write(project.join("data.csv"), "a,b\n1,2\n").unwrap();
    // A file the compile never reads:
    std::fs::write(project.join("notes.txt"), "extra").unwrap();

    let packages = dir.join("packages");
    let package = packages.join("local/greet/0.1.0");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("typst.toml"),
        "[package]\nname = \"greet\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    std::fs::write(
        package.join("lib.typ"),
        "#let greet(name) = [Hello #name!]\n",
    )
    .unwrap();
    std::fs::write(package.join("unused.txt"), "complete package").unwrap();

    (project, packages)
}

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

/// The contained project paths of a pack, in canonical order.
fn project_files(pack: &Pack) -> Vec<&str> {
    pack.files().map(|(path, _)| path).collect()
}

fn no_font_assembler() -> fs::Packer {
    fs::Packer {
        system_fonts: false,
        ..fs::Packer::default()
    }
}

/// Packs `project` with `main.typ` as its entrypoint.
trait PackProject {
    fn assemble(&self, project: &Path) -> Result<fs::Packed, fs::Error>;
}

impl PackProject for fs::Packer {
    fn assemble(&self, project: &Path) -> Result<fs::Packed, fs::Error> {
        self.pack(project, "main.typ")
    }
}

#[test]
fn structural_creation_packs_all_project_files_and_complete_packages() {
    let dir = tempfile::tempdir().unwrap();
    let (project, packages) = fixture(dir.path());

    let assembler = fs::Packer {
        package_path: Some(packages.to_path_buf()),
        system_fonts: false,
        ..fs::Packer::default()
    };
    let report = assembler.assemble(&project).unwrap();

    for expected in [
        "main.typ",
        "chapters/intro.typ",
        "assets/logo.png",
        "data.csv",
        "notes.txt",
    ] {
        let files = project_files(&report.pack);
        assert!(files.contains(&expected), "missing {expected} in {files:?}");
    }
    assert_eq!(report.pack.package_requirements().len(), 1);
    let requirement = &report.pack.package_requirements()[0];
    assert!(requirement.embedded);
    assert_eq!(requirement.spec.to_string(), "@local/greet:0.1.0");

    let (_, tree) = report.pack.packages().next().unwrap();
    assert_eq!(tree.digest(), requirement.digest);
    // The whole package tree travels, not only what was read.
    assert_eq!(
        tree.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["lib.typ", "typst.toml", "unused.txt"]
    );

    let reread = typst_pack::archive::decode(
        &typst_pack::archive::encode(&report.pack),
        Default::default(),
    )
    .unwrap();
    assert_eq!(reread.identity(), report.pack.identity());
}

#[test]
fn structural_creation_applies_the_root_project_ignore_policy() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(project.join("ignored/reincluded")).unwrap();
    std::fs::create_dir_all(project.join("nested")).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(project.join("unused.txt"), "packed").unwrap();
    std::fs::write(
        project.join(".typkignore"),
        "ignored/**\n!ignored/reincluded/\n!ignored/reincluded/keep.txt\n*.secret\n",
    )
    .unwrap();
    std::fs::write(project.join("ignored/drop.txt"), "drop").unwrap();
    std::fs::write(project.join("ignored/reincluded/keep.txt"), "keep").unwrap();
    std::fs::write(project.join("nested/.typkignore"), "*.txt\n").unwrap();
    std::fs::write(project.join("nested/ordinary.txt"), "packed").unwrap();
    std::fs::write(project.join("private.secret"), "drop").unwrap();
    std::fs::write(project.join("old.typk"), "drop").unwrap();

    let report = no_font_assembler().assemble(&project).unwrap();

    assert_eq!(
        project_files(&report.pack),
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
    std::fs::create_dir_all(project.join("ignored")).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(project.join(".typkignore"), "ignored/\n!ignored/keep.txt\n").unwrap();
    std::fs::write(project.join("ignored/keep.txt"), "still ignored").unwrap();

    let report = no_font_assembler().assemble(&project).unwrap();

    assert_eq!(project_files(&report.pack), [".typkignore", "main.typ"]);
}

#[test]
fn structural_creation_supports_gitignore_pattern_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(project.join("nested/cache")).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(
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
        ),
    )
    .unwrap();
    std::fs::write(project.join("# Project policy"), "packed").unwrap();
    std::fs::write(project.join("root-only.secret"), "ignored").unwrap();
    std::fs::write(project.join("nested/root-only.secret"), "packed").unwrap();
    std::fs::write(project.join("!important.txt"), "ignored").unwrap();
    std::fs::write(project.join("#notes.txt"), "ignored").unwrap();
    std::fs::write(project.join("nested/cache/data.txt"), "ignored").unwrap();
    std::fs::write(project.join("drop.tmp"), "ignored").unwrap();
    std::fs::write(project.join("nested/keep.tmp"), "packed").unwrap();
    std::fs::write(project.join(" leading.txt"), "ignored").unwrap();
    std::fs::write(project.join("trailing.txt"), "ignored").unwrap();

    let report = no_font_assembler().assemble(&project).unwrap();

    assert_eq!(
        project_files(&report.pack),
        [
            "# Project policy",
            ".typkignore",
            "main.typ",
            "nested/keep.tmp",
            "nested/root-only.secret",
        ]
    );
}

// Windows drops a trailing space from a file name, so only a unix filesystem
// can hold the name an escaped trailing space is written to match.
#[cfg(unix)]
#[test]
fn structural_creation_matches_an_escaped_trailing_space() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(project.join(".typkignore"), "literal\\ \n").unwrap();
    std::fs::write(project.join("literal "), "ignored").unwrap();
    std::fs::write(project.join("literal"), "packed").unwrap();

    let report = no_font_assembler().assemble(&project).unwrap();

    assert_eq!(
        project_files(&report.pack),
        [".typkignore", "literal", "main.typ"]
    );
}

#[cfg(unix)]
#[test]
fn structural_creation_prunes_conclusively_ignored_subtrees() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(project.join("ignored")).unwrap();
    std::fs::create_dir_all(project.join("other")).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(project.join(".typkignore"), "ignored/\n!/other/keep.txt\n").unwrap();
    std::fs::write(project.join("other/keep.txt"), "keep").unwrap();
    symlink(dir.path(), project.join("ignored/outside")).unwrap();

    let report = no_font_assembler().assemble(&project).unwrap();

    assert_eq!(
        project_files(&report.pack),
        [".typkignore", "main.typ", "other/keep.txt"]
    );
}

#[cfg(unix)]
#[test]
fn structural_creation_rejects_a_root_ignore_policy_linked_from_outside() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(dir.path().join("outside-ignore"), "*.txt\n").unwrap();
    symlink(
        dir.path().join("outside-ignore"),
        project.join(".typkignore"),
    )
    .unwrap();

    let policy = project.canonicalize().unwrap().join(".typkignore");
    let result = no_font_assembler().assemble(&project);

    assert!(matches!(
        result,
        Err(fs::Error::EscapingSymlink { ref path, .. })
            if path == &policy
    ));
}

#[test]
fn an_invalid_document_time_is_rejected_before_compiling() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();

    let result = fs::Packer {
        options: compile::Options {
            document_time: DocumentTime::UnixTimestamp(i64::MAX),
            ..compile::Options::default()
        },
        ..no_font_assembler()
    }
    .assemble(&project);

    let Err(error) = result else {
        panic!("the invalid document time was accepted");
    };
    assert!(matches!(
        &error,
        fs::Error::Create(create::Error::InvalidRequest(issues))
            if issues == &[RequestIssue::InvalidDocumentTimestamp]
    ));
    assert_eq!(
        error.to_string(),
        "the document-time UNIX timestamp is out of range"
    );
}

#[test]
fn an_ignored_entrypoint_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("main.typ"), "Hello").unwrap();
    std::fs::write(project.join(".typkignore"), "main.typ\n").unwrap();

    let error = no_font_assembler().assemble(&project).unwrap_err();
    assert!(matches!(&error, fs::Error::EntrypointIgnored(path) if path == "main.typ"));
    assert!(error.to_string().contains(".typkignore"), "{error}");
}

#[test]
fn creation_target_does_not_select_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#context if target() == \"html\" { read(\"html.txt\") } else { read(\"paged.txt\") }",
    )
    .unwrap();
    std::fs::write(project.join("paged.txt"), "paged").unwrap();
    std::fs::write(project.join("html.txt"), "html").unwrap();

    let report = fs::Packer {
        target: create::Target::Html,
        ..no_font_assembler()
    }
    .assemble(&project)
    .unwrap();

    assert_eq!(
        project_files(&report.pack),
        ["html.txt", "main.typ", "paged.txt"]
    );
}

#[test]
fn exact_inputs_and_document_time_drive_representative_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        r#"#if sys.inputs.at("pick") == "yes" { read("input.txt") }
#if datetime.today().year() == 2024 { read("time.txt") }"#,
    )
    .unwrap();
    std::fs::write(project.join("input.txt"), "input").unwrap();
    std::fs::write(project.join("time.txt"), "time").unwrap();
    let report = fs::Packer {
        options: compile::Options {
            inputs: vec![("pick".into(), Input::from("yes"))],
            document_time: DocumentTime::UnixTimestamp(1_704_067_200),
            ..compile::Options::default()
        },
        ..no_font_assembler()
    }
    .assemble(&project)
    .unwrap();

    assert_eq!(
        project_files(&report.pack),
        ["input.txt", "main.typ", "time.txt"]
    );
}

#[test]
fn package_data_precedes_package_cache_during_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#import \"@local/chosen:0.1.0\": mark\n#mark",
    )
    .unwrap();

    let data_package = dir.path().join("data/local/chosen/0.1.0");
    std::fs::create_dir_all(&data_package).unwrap();
    std::fs::write(
        data_package.join("typst.toml"),
        "[package]\nname = \"chosen\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    std::fs::write(
        data_package.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();

    let cache_package = dir.path().join("cache/local/chosen/0.1.0");
    std::fs::create_dir_all(&cache_package).unwrap();
    std::fs::write(cache_package.join("lib.typ"), "this is not valid Typst: {").unwrap();

    let assembler = fs::Packer {
        package_path: Some(dir.path().join("data")),
        package_cache_path: Some(dir.path().join("cache")),
        system_fonts: false,
        ..fs::Packer::default()
    };
    let report = assembler.assemble(&project).unwrap();

    assert_eq!(report.pack.package_requirements().len(), 1);
}

#[test]
fn package_cache_resolves_during_online_and_offline_creation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#import \"@preview/cached:0.1.0\": mark\n#mark",
    )
    .unwrap();

    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let cache = dir.path().join("cache/preview/cached/0.1.0");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("typst.toml"),
        "[package]\nname = \"cached\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .unwrap();
    std::fs::write(
        cache.join("lib.typ"),
        "#let mark = rect(width: 1pt, height: 1pt)",
    )
    .unwrap();

    let offline = fs::Packer {
        package_path: Some(data.to_path_buf()),
        package_cache_path: Some(dir.path().join("cache")),
        system_fonts: false,
        ..fs::Packer::default()
    };
    #[cfg(feature = "egress")]
    let online = fs::Packer {
        download_packages: true,
        certificate: None,
        ..offline.clone()
    };
    #[cfg(not(feature = "egress"))]
    let online = offline.clone();
    for assembler in [offline, online] {
        let report = assembler.assemble(&project).unwrap();

        let (spec, tree) = report.pack.packages().next().unwrap();
        assert_eq!(spec.to_string(), "@preview/cached:0.1.0");
        assert!(tree.file("lib.typ").is_some());
    }
}

#[test]
fn a_project_needing_chained_packages_is_resolved_to_completion() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
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

    let assembler = fs::Packer {
        package_path: Some(packages.to_path_buf()),
        system_fonts: false,
        ..fs::Packer::default()
    };
    let report = assembler.assemble(&project).unwrap();

    // The package only another package's tree imports is resolved too, however
    // many rounds of creation that took.
    assert_eq!(
        report
            .pack
            .package_requirements()
            .iter()
            .map(|requirement| requirement.spec.to_string())
            .collect::<Vec<_>>(),
        ["@local/inner:0.1.0", "@local/outer:0.1.0"]
    );
}

/// Writes one local package whose `lib.typ` holds `body`.
fn write_package(packages: &Path, name: &str, body: &str) {
    let package = packages.join(format!("local/{name}/0.1.0"));
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("typst.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n"),
    )
    .unwrap();
    std::fs::write(package.join("lib.typ"), body).unwrap();
}

#[test]
fn offline_creation_works_with_local_packages() {
    let dir = tempfile::tempdir().unwrap();
    let (project, packages) = fixture(dir.path());

    let assembler = fs::Packer {
        package_path: Some(packages.to_path_buf()),
        system_fonts: false,
        ..fs::Packer::default()
    };
    let report = assembler.assemble(&project).unwrap();

    assert_eq!(report.pack.package_requirements().len(), 1);
}

#[cfg(feature = "egress")]
#[test]
fn offline_creation_fails_on_an_uncached_universe_package() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#import \"@preview/typst-pack-no-such-package:0.0.1\": x\n",
    )
    .unwrap();
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    let assembler = fs::Packer {
        system_fonts: false,
        package_path: Some(empty.to_path_buf()),
        package_cache_path: Some(empty.to_path_buf()),
        ..fs::Packer::default()
    };
    let result = assembler.assemble(&project);

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
/// configuration: a Universe package no local source holds fails read
/// even though nothing asked for offline creation.
#[cfg(not(feature = "egress"))]
#[test]
fn creation_without_egress_never_downloads_a_universe_package() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#import \"@preview/typst-pack-no-such-package:0.0.1\": x\n",
    )
    .unwrap();
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    let assembler = fs::Packer {
        system_fonts: false,
        package_path: Some(empty.to_path_buf()),
        ..fs::Packer::default()
    };
    let result = assembler.assemble(&project);

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
fn unresolvable_package_diagnostics(result: Result<fs::Packed, fs::Error>) -> Vec<String> {
    let Err(fs::Error::Create(create::Error::Failed { errors, .. })) = result else {
        panic!("an unresolvable package did not fail the representative request");
    };
    for error in &errors {
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
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("main.typ"), "#import \"missing.typ\": x\n").unwrap();

    let result = no_font_assembler().assemble(&project);

    assert!(matches!(
        result,
        Err(fs::Error::Create(create::Error::Failed { .. }))
    ));
}

#[test]
fn creation_retains_representative_compile_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("main.typ"),
        "#set text(font: \"Definitely Missing\")\nWarning",
    )
    .unwrap();

    let assembler = fs::Packer {
        system_fonts: false,
        typst_embedded_fonts: false,
        ..fs::Packer::default()
    };
    let report = assembler.assemble(&project).unwrap();

    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{:?}",
        report.warnings
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

        let assembler = fs::Packer {
            package_path: Some(packages.to_path_buf()),
            system_fonts: false,
            ..fs::Packer::default()
        };
        let slim = fs::Packer {
            embed_fonts: true,
            ..assembler.clone()
        }
        .assemble(&project)
        .unwrap();
        assert!(
            !slim
                .pack
                .font_requirements()
                .iter()
                .any(|requirement| requirement.embedded),
            "only Typst embedded fonts are used, so nothing should be embedded"
        );

        let full = fs::Packer {
            embed_fonts: true,
            include_typst_embedded_fonts: true,
            ..assembler
        }
        .assemble(&project)
        .unwrap();
        assert!(
            full.pack
                .font_requirements()
                .iter()
                .any(|requirement| requirement.embedded)
        );
        // The embedded containers must load again from the pack.
        typst_pack::archive::decode(&typst_pack::archive::encode(&full.pack), Default::default())
            .unwrap();
    }

    #[test]
    fn html_creation_embeds_fonts_used_inside_frames() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("main.typ"), "#html.frame[Hello]").unwrap();

        let report = fs::Packer {
            target: create::Target::Html,
            embed_fonts: true,
            include_typst_embedded_fonts: true,
            ..no_font_assembler()
        }
        .assemble(&project)
        .unwrap();

        assert!(
            report
                .pack
                .font_requirements()
                .iter()
                .any(|requirement| requirement.embedded)
        );
    }

    #[test]
    fn a_created_pack_compiles_with_its_resolved_external_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let packer = fs::Packer {
            package_path: Some(packages.to_path_buf()),
            system_fonts: false,
            ..fs::Packer::default()
        };
        let report = packer.assemble(&project).unwrap();

        // Round-trip through bytes: nothing may depend on the read project.
        let pack = typst_pack::archive::decode(
            &typst_pack::archive::encode(&report.pack),
            Default::default(),
        )
        .unwrap();
        let mut request = compile::Request::new(
            pack.clone(),
            compile::Output::new(compile::OutputFormat::Pdf),
        );
        assert!(matches!(
            compile(&request),
            Err(typst_pack::compile::Error::Unfulfilled(_))
        ));

        // The fonts the packer offered were Typst's own, declared externally.
        let resolved = packer.resolve(&pack).unwrap();
        assert!(resolved.packages.is_empty());
        assert_eq!(resolved.fonts.len(), pack.font_requirements().len());
        for requirement in pack.font_requirements() {
            assert!(
                resolved
                    .fonts
                    .iter()
                    .any(|font| font.digest() == requirement.digest)
            );
        }
        request.fonts = resolved.fonts;
        let compiled = compile(&request).unwrap();
        assert!(compiled.artifacts[0].bytes.starts_with(b"%PDF"));
    }
}
