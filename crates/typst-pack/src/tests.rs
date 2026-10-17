//! Crate tests that need private access or check builder-level invariants.

use typst::syntax::package::PackageSpec;

use crate::*;

fn spec(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

fn issues(builder: PackBuilder) -> Vec<PackIssue> {
    builder.build().unwrap_err().issues
}

#[test]
fn the_builder_requires_a_contained_valid_entrypoint() {
    assert_eq!(
        issues(Pack::builder("main.typ")),
        [PackIssue::MissingEntrypoint {
            path: "main.typ".to_owned()
        }]
    );
    for entrypoint in ["", "main\0.typ", "../main.typ"] {
        assert!(matches!(
            issues(Pack::builder(entrypoint)).as_slice(),
            [PackIssue::InvalidEntrypoint { .. }]
        ));
    }
    for path in [
        "/main.typ",
        "C:outside.txt",
        "./C:/outside.txt",
        "main\0.typ",
    ] {
        let builder = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .file(path, Vec::new());
        assert!(
            matches!(issues(builder).as_slice(), [PackIssue::InvalidPath { .. }]),
            "{path}"
        );
    }
}

#[test]
fn the_builder_reports_every_conflict_independent_of_input_order() {
    let builder = Pack::builder("main.typ")
        .file("main.typ", b"first".to_vec())
        .file("./main.typ", b"second".to_vec())
        .file("assets", Vec::new())
        .file("assets/logo.png", Vec::new());
    assert_eq!(
        issues(builder),
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
        let builder = files
            .into_iter()
            .fold(Pack::builder("main.typ"), |builder, (path, data)| {
                builder.file(path, data.as_bytes().to_vec())
            });
        issues(builder)
    };
    assert_eq!(build(false), build(true));
}

#[test]
fn the_builder_rejects_invalid_packages_and_fonts() {
    let mut invalid = spec("@local/example:1.0.0");
    invalid.name = "bad/name".into();
    let builder = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .package_file(invalid, "lib.typ", Vec::new())
        .package_file(spec("@local/tree:1.0.0"), "lib.typ", b"a".to_vec())
        .package_file(spec("@local/tree:1.0.0"), "./lib.typ", b"b".to_vec())
        .package_file(spec("@local/both:1.0.0"), "lib.typ", Vec::new())
        .external_package_file(spec("@local/both:1.0.0"), "lib.typ", Vec::new())
        .font(b"not a font".to_vec());
    let issues = issues(builder);

    assert_eq!(issues.len(), 4, "{issues:#?}");
    let has = |predicate: &dyn Fn(&PackIssue) -> bool| issues.iter().any(predicate);
    assert!(has(&|issue| matches!(
        issue,
        PackIssue::InvalidPackageSpec { .. }
    )));
    assert!(has(
        &|issue| matches!(issue, PackIssue::InvalidPackage { spec, .. } if spec.name == "tree")
    ));
    assert!(has(
        &|issue| matches!(issue, PackIssue::DuplicatePackage { spec } if spec.name == "both")
    ));
    assert!(has(
        &|issue| *issue == PackIssue::InvalidFontData { position: 0 }
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn a_font_is_either_embedded_or_external_and_never_collides_with_project_files() {
    let data = typst_kit::fonts::embedded()
        .next()
        .unwrap()
        .0
        .data()
        .to_vec();
    let builder = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .font(data.clone())
        .external_font(data.clone());
    assert_eq!(
        issues(builder),
        [PackIssue::DuplicateFont {
            digest: Digest::of(&data)
        }]
    );

    // A project file at the path a font container extracts to conflicts.
    let font_path = crate::archive::font_member_name(&Digest::of(&data), &data);
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .file(&font_path, b"project".to_vec())
        .font(data)
        .build()
        .unwrap();
    let selection = extract::Selection {
        packages: false,
        fonts: true,
    };
    assert!(extract::plan(&pack, selection).is_err());
}
