//! Planning the extraction of a pack.

use proptest::prelude::*;
use typst::syntax::package::PackageSpec;
use typst_pack::Pack;
use typst_pack::extract::{self, Conflict, Selection};

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

fn package_spec() -> PackageSpec {
    "@local/example:1.0.0".parse().unwrap()
}

fn pack_with_package() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file("assets/logo.bin", b"logo".to_vec())
        .package_file(package_spec(), "lib.typ", b"package".to_vec())
        .build()
        .unwrap()
}

fn selection(packages: bool, fonts: bool) -> Selection {
    Selection { packages, fonts }
}

fn observed_entries(pack: &Pack, selection: Selection) -> Vec<(String, Vec<u8>)> {
    extract::plan(pack, selection)
        .unwrap()
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.bytes().to_vec()))
        .collect()
}

fn conflicts(pack: &Pack) -> Vec<Conflict> {
    extract::plan(pack, selection(true, false))
        .unwrap_err()
        .issues
}

#[test]
fn plans_list_canonical_entries_in_path_order() {
    let plan = extract::plan(&pack_with_package(), selection(true, false)).unwrap();

    assert_eq!(
        plan.entries()
            .iter()
            .map(|entry| (entry.path(), entry.bytes()))
            .collect::<Vec<_>>(),
        [
            ("assets/logo.bin", b"logo".as_slice()),
            ("main.typ", b"main".as_slice()),
            (
                "packages/local/example/1.0.0/lib.typ",
                b"package".as_slice()
            ),
        ]
    );
}

#[test]
fn projects_are_unconditional_and_embedded_packages_are_selected_explicitly() {
    let pack = pack_with_package();

    for packages in [false, true] {
        for fonts in [false, true] {
            let entries = observed_entries(&pack, selection(packages, fonts));

            assert!(entries.iter().any(|entry| entry.0 == "main.typ"));
            assert_eq!(
                entries.iter().any(|entry| entry.0.starts_with("packages/")),
                packages
            );
        }
    }
}

#[test]
fn external_package_trees_are_never_planned() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .external_package_file(package_spec(), "lib.typ", b"external".to_vec())
        .build()
        .unwrap();

    let plan = extract::plan(&pack, selection(true, true)).unwrap();

    assert_eq!(plan.entries().len(), 1);
    assert_eq!(plan.entries()[0].path(), "main.typ");
}

#[test]
fn planning_aggregates_collisions_in_canonical_path_order() {
    let base = "packages/local/example/1.0.0";
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file(format!("{base}/z.typ"), b"project z".to_vec())
        .file(format!("{base}/a.typ"), b"project a".to_vec())
        .package_file(package_spec(), "z.typ", b"package z".to_vec())
        .package_file(package_spec(), "a.typ", b"package a".to_vec())
        .build()
        .unwrap();

    let error = extract::plan(&pack, selection(true, false)).unwrap_err();

    assert_eq!(
        error.issues,
        ["a.typ", "z.typ"].map(|name| {
            let path = format!("{base}/{name}");
            Conflict {
                path: path.clone(),
                kind: "project file",
                other_path: path,
                other_kind: "package file",
            }
        })
    );
    assert!(error.to_string().contains("a.typ"), "{error}");
    assert!(error.to_string().contains("z.typ"), "{error}");
}

#[test]
fn conflicts_are_ordered_by_kind_then_path() {
    let base = "packages/local/example/1.0.0";
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file(format!("{base}/a/descendant"), b"project a".to_vec())
        .file(format!("{base}/z"), b"project z".to_vec())
        .package_file(package_spec(), "a", b"package a".to_vec())
        .package_file(package_spec(), "z", b"package z".to_vec())
        .build()
        .unwrap();

    assert_eq!(
        conflicts(&pack)
            .iter()
            .map(|conflict| (conflict.kind, conflict.path.as_str()))
            .collect::<Vec<_>>(),
        [
            ("project file", "packages/local/example/1.0.0/z"),
            ("package file", "packages/local/example/1.0.0/a"),
        ]
    );
}

proptest! {
    #[test]
    fn planning_is_invariant_under_input_permutation(
        generated in prop::collection::btree_map(
            "[a-z][a-z0-9]{0,12}",
            prop::collection::vec(any::<u8>(), 0..64),
            0..16,
        ),
        include_packages in any::<bool>(),
        include_fonts in any::<bool>(),
    ) {
        let mut entries = vec![("main.typ".to_owned(), b"main".to_vec())];
        entries.extend(
            generated
                .into_iter()
                .map(|(stem, bytes)| (format!("files/{stem}.bin"), bytes)),
        );
        let mut reversed = entries.clone();
        reversed.reverse();

        let build = |entries: Vec<(String, Vec<u8>)>| {
            let mut builder = Pack::builder("main.typ");
            for (path, bytes) in entries {
                builder = builder.file(path, bytes);
            }
            builder.package_file(package_spec(), "lib.typ", b"package".to_vec())
                .build()
                .unwrap()
        };
        let selection = selection(include_packages, include_fonts);

        prop_assert_eq!(
            observed_entries(&build(entries), selection),
            observed_entries(&build(reversed), selection),
        );
    }

    #[test]
    fn planning_rejects_generated_exact_and_tree_collisions(
        stem in "[a-z][a-z0-9]{0,12}",
        collision_kind in 0u8..3,
    ) {
        let package_path = format!("packages/local/example/1.0.0/{stem}");
        let project_path = match collision_kind {
            0 => package_path.clone(),
            1 => "packages/local/example/1.0.0".to_owned(),
            _ => format!("{package_path}/descendant"),
        };
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file(project_path, b"project".to_vec())
            .package_file(package_spec(), stem, b"package".to_vec())
            .build().unwrap();

        prop_assert_eq!(conflicts(&pack).len(), 1);
    }

    #[test]
    fn generated_collision_order_is_invariant_under_input_permutation(
        stems in prop::collection::btree_set("[a-z][a-z0-9]{0,12}", 2..8),
    ) {
        let entries = stems.into_iter().collect::<Vec<_>>();
        let mut reversed = entries.clone();
        reversed.reverse();
        let build = |project_order: &[String], package_order: &[String]| {
            let mut builder = Pack::builder("main.typ")
                .file("main.typ", b"main".to_vec());
            for stem in project_order {
                builder = builder.file(
                    format!("packages/local/example/1.0.0/{stem}"),
                    format!("project {stem}").into_bytes(),
                );
            }
            for stem in package_order {
                builder = builder.package_file(package_spec(), stem, format!("package {stem}").into_bytes());
            }
            builder.build().unwrap()
        };

        prop_assert_eq!(
            conflicts(&build(&entries, &reversed)),
            conflicts(&build(&reversed, &entries))
        );
    }

    #[test]
    fn path_segment_prefixes_do_not_collide(
        stem in "[a-z][a-z0-9]{0,12}",
    ) {
        let project_path = format!("packages/local/example/1.0.0/{stem}-sibling");
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .file(project_path, b"project".to_vec())
            .package_file(package_spec(), stem, b"package".to_vec())
            .build().unwrap();

        let plan = extract::plan(&pack, selection(true, false)).unwrap();

        prop_assert_eq!(plan.entries().len(), 3);
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn embedded_font_containers_are_selected_explicitly_and_external_fonts_are_excluded() {
    let font = fonts::typst_container();
    let embedded = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .font(font.clone())
        .build()
        .unwrap();
    let external = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .external_font(font)
        .build()
        .unwrap();

    let excluded = extract::plan(&embedded, selection(false, false)).unwrap();
    let included = extract::plan(&embedded, selection(false, true)).unwrap();
    let unavailable = extract::plan(&external, selection(false, true)).unwrap();

    assert_eq!(excluded.entries().len(), 1);
    assert_eq!(included.entries().len(), 2);
    assert!(included.entries()[0].path().starts_with("fonts/"));
    assert_eq!(unavailable.entries().len(), 1);
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn a_multi_face_container_is_extracted_once() {
    let face = fonts::typst_container();
    let collection = fonts::font_collection(&[face.clone(), face]);
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .font(collection.clone())
        .build()
        .unwrap();
    let plan = extract::plan(&pack, selection(false, true)).unwrap();
    let font_entries = plan
        .entries()
        .iter()
        .filter(|entry| entry.path().starts_with("fonts/"))
        .collect::<Vec<_>>();

    assert_eq!(font_entries.len(), 1);
    assert_eq!(font_entries[0].bytes(), collection.as_slice());
}
