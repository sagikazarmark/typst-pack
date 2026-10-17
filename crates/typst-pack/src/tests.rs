//! Crate tests that need private access.

use typst::syntax::package::PackageSpec;

use crate::compile::PageSelection;
use crate::*;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

fn spec(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

fn issues(result: Result<Pack, PackError>) -> Vec<PackIssue> {
    result.unwrap_err().issues
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
fn pack_construction_requires_a_contained_entrypoint() {
    assert_eq!(
        issues(Pack::builder("main.typ").build()),
        [PackIssue::MissingEntrypoint {
            path: "main.typ".to_owned()
        }]
    );
}

#[test]
fn pack_builder_rejects_paths_that_cannot_name_root_relative_files() {
    for entrypoint in ["", "main\0.typ", "../main.typ"] {
        assert!(matches!(
            issues(Pack::builder(entrypoint).build()).as_slice(),
            [PackIssue::InvalidEntrypoint { .. }]
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
            matches!(issues(result).as_slice(), [PackIssue::InvalidPath { .. }]),
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
            PackIssue::PathTreeConflict {
                ancestor: "assets".to_owned(),
                descendant: "assets/logo.png".to_owned()
            },
            PackIssue::DuplicatePath {
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
            .any(|issue| matches!(issue, PackIssue::InvalidPackageSpec { .. }))
    );
    assert!(issues.iter().any(|issue| matches!(
        issue,
        PackIssue::InvalidPackage { spec, .. } if spec.name == "tree"
    )));
    assert!(issues.iter().any(|issue| matches!(
        issue,
        PackIssue::DuplicatePackage { spec } if spec.name == "both"
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
    assert_eq!(issues(result), [PackIssue::InvalidFontData { position: 0 }]);

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
            [PackIssue::DuplicateFont {
                digest: Digest::of(&data)
            }]
        );
    }
}

#[test]
fn page_selections_parse_ranges() {
    use std::num::NonZeroUsize;
    let one = NonZeroUsize::new(1);
    let three = NonZeroUsize::new(3);
    let five = NonZeroUsize::new(5);
    let nine = NonZeroUsize::new(9);
    assert_eq!(
        "1,3-5,9-".parse::<PageSelection>().unwrap().ranges,
        [one..=one, three..=five, nine..=None]
    );
    assert!("nope".parse::<PageSelection>().is_err());
}

#[cfg(feature = "fs")]
mod fs {
    use super::*;
    use crate::extract::{self, Selection};
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

    fn assemble(project: &Path, packages: &Path) -> crate::fs::Packed {
        crate::fs::Packer {
            system_fonts: false,
            package_path: Some(packages.to_owned()),
            ..crate::fs::Packer::default()
        }
        .pack(project, "main.typ")
        .unwrap()
    }

    #[test]
    fn extract_writes_project_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let root = temp_path(&dir);
        let (project, packages) = fixture(&root);
        let packed = assemble(&project, &packages);

        let target = root.join("extracted");
        let plan = extract::plan(
            &packed.pack,
            Selection {
                packages: true,
                fonts: true,
            },
        )
        .unwrap();
        crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::CreateNew).unwrap();
        assert!(target.join("main.typ").exists());
        assert!(target.join("assets/logo.png").exists());
        assert!(target.join("packages/local/greet/0.1.0/lib.typ").exists());

        assert!(matches!(
            crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::CreateNew),
            Err(crate::fs::WriteError::Rejected { issues })
                if matches!(issues.as_slice(), [crate::fs::WriteIssue::ExistingTarget { .. }, ..])
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
                extract::plan(
                    &pack,
                    Selection {
                        packages: true,
                        fonts: false
                    }
                )
                .is_err()
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

        let plan = extract::plan(&pack, Selection::default()).unwrap();
        assert!(matches!(
            crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::CreateNew),
            Err(crate::fs::WriteError::Rejected { issues })
                if matches!(issues.as_slice(), [crate::fs::WriteIssue::ExistingTarget { .. }])
        ));
        assert!(!target.join("main.typ").exists());
        assert_eq!(fs::read(target.join("z.txt")).unwrap(), b"external");

        crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::Overwrite).unwrap();
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
        let nested_plan = extract::plan(&nested_pack, Selection::default()).unwrap();
        assert!(matches!(
            crate::fs::write_extraction(
                &nested_plan,
                &blocked_target,
                crate::fs::WritePolicy::Overwrite,
            ),
            Err(crate::fs::WriteError::Rejected { issues })
                if matches!(issues.as_slice(), [crate::fs::WriteIssue::ConflictingAncestor { .. }])
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

        let plan = extract::plan(&pack, Selection::default()).unwrap();
        assert!(matches!(
            crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::Overwrite),
            Err(crate::fs::WriteError::Rejected { issues })
                if matches!(issues.as_slice(), [crate::fs::WriteIssue::ConflictingAncestor { .. }])
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
            crate::archive::font_member_name(&font_pack.font_requirements()[0].digest, &data);

        let conflicting = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .file(&font_path, b"project".to_vec())
            .font(data)
            .build()
            .unwrap();
        assert!(
            extract::plan(
                &conflicting,
                Selection {
                    packages: false,
                    fonts: true
                }
            )
            .is_err()
        );

        let dir = tempfile::tempdir().unwrap();
        let target = temp_path(&dir).join("extracted");
        let plan = extract::plan(
            &font_pack,
            Selection {
                packages: false,
                fonts: true,
            },
        )
        .unwrap();
        crate::fs::write_extraction(&plan, &target, crate::fs::WritePolicy::CreateNew).unwrap();
        assert_eq!(plan.entries().len(), 2);
        assert!(target.join(font_path).is_file());
    }
}
