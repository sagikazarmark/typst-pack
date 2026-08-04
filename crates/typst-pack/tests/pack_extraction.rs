//! Featureless semantic planning for Pack Extraction.

use proptest::prelude::*;
use typst::syntax::package::PackageSpec;
use typst_pack::{
    Pack, PackExtractionEntryRole, PackExtractionPlanIssue, PackExtractionSelection,
    plan_pack_extraction,
};

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

fn package_spec() -> PackageSpec {
    "@local/example:1.0.0".parse().unwrap()
}

fn pack_with_package() -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("assets/logo.bin", b"logo".to_vec())
        .unwrap()
        .package_file(package_spec(), "lib.typ", b"package".to_vec())
        .unwrap()
        .build()
        .unwrap()
}

fn observed_entries(
    pack: &Pack,
    selection: PackExtractionSelection,
) -> Vec<(String, PackExtractionEntryRole, u64, Vec<u8>)> {
    plan_pack_extraction(pack, selection)
        .unwrap()
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.relative_path().to_owned(),
                entry.role(),
                entry.len(),
                entry.bytes().to_vec(),
            )
        })
        .collect()
}

#[test]
fn plan_owns_identity_selection_and_canonical_entries() {
    let pack = pack_with_package();
    let identity = pack.identity();
    let selection = PackExtractionSelection::new(true, false);
    let plan = plan_pack_extraction(&pack, selection).unwrap();

    assert_eq!(plan.pack_identity(), &identity);
    assert_eq!(plan.selection(), selection);
    assert_eq!(
        plan.entries()
            .iter()
            .map(|entry| (
                entry.relative_path(),
                entry.role(),
                entry.len(),
                entry.bytes(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                "assets/logo.bin",
                PackExtractionEntryRole::ProjectFile,
                4,
                b"logo".as_slice(),
            ),
            (
                "main.typ",
                PackExtractionEntryRole::ProjectFile,
                4,
                b"main".as_slice(),
            ),
            (
                "packages/local/example/1.0.0/lib.typ",
                PackExtractionEntryRole::PackageFile,
                7,
                b"package".as_slice(),
            ),
        ]
    );

    drop(pack);
    assert_eq!(plan.entries()[0].bytes(), b"logo");
}

#[test]
fn projects_are_unconditional_and_embedded_packages_are_selected_explicitly() {
    let pack = pack_with_package();

    for include_packages in [false, true] {
        for include_fonts in [false, true] {
            let selection = PackExtractionSelection::new(include_packages, include_fonts);
            let entries = observed_entries(&pack, selection);

            assert!(entries.iter().any(|entry| entry.0 == "main.typ"));
            assert_eq!(
                entries
                    .iter()
                    .any(|entry| entry.1 == PackExtractionEntryRole::PackageFile),
                include_packages
            );
        }
    }
}

#[test]
fn external_package_trees_are_never_planned() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .external_package_file(package_spec(), "lib.typ", b"external".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let plan = plan_pack_extraction(&pack, PackExtractionSelection::new(true, true)).unwrap();

    assert_eq!(plan.entries().len(), 1);
    assert_eq!(
        plan.entries()[0].role(),
        PackExtractionEntryRole::ProjectFile
    );
}

#[test]
fn planning_aggregates_collisions_in_canonical_path_order() {
    let base = "packages/local/example/1.0.0";
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file(format!("{base}/z.typ"), b"project z".to_vec())
        .unwrap()
        .file(format!("{base}/a.typ"), b"project a".to_vec())
        .unwrap()
        .package_file(package_spec(), "z.typ", b"package z".to_vec())
        .unwrap()
        .package_file(package_spec(), "a.typ", b"package a".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let error = plan_pack_extraction(&pack, PackExtractionSelection::new(true, false))
        .expect_err("both projected path collisions must reject the plan");
    let paths = error
        .issues()
        .iter()
        .filter_map(|issue| match issue {
            PackExtractionPlanIssue::PathConflict { first_path, .. } => Some(first_path.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        [
            "packages/local/example/1.0.0/a.typ",
            "packages/local/example/1.0.0/z.typ",
        ]
    );
}

#[test]
fn collision_issue_order_places_roles_before_paths() {
    let base = "packages/local/example/1.0.0";
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file(format!("{base}/a/descendant"), b"project a".to_vec())
        .unwrap()
        .file(format!("{base}/z"), b"project z".to_vec())
        .unwrap()
        .package_file(package_spec(), "a", b"package a".to_vec())
        .unwrap()
        .package_file(package_spec(), "z", b"package z".to_vec())
        .unwrap()
        .build()
        .unwrap();

    let error = plan_pack_extraction(&pack, PackExtractionSelection::new(true, false))
        .expect_err("both role collisions must reject the plan");
    let roles_and_paths = error
        .issues()
        .iter()
        .filter_map(|issue| match issue {
            PackExtractionPlanIssue::PathConflict {
                first_path,
                first_role,
                ..
            } => Some((*first_role, first_path.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        roles_and_paths,
        [
            (
                PackExtractionEntryRole::ProjectFile,
                "packages/local/example/1.0.0/z",
            ),
            (
                PackExtractionEntryRole::PackageFile,
                "packages/local/example/1.0.0/a",
            ),
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
                builder = builder.file(path, bytes).unwrap();
            }
            builder
                .package_file(package_spec(), "lib.typ", b"package".to_vec())
                .unwrap()
                .build()
                .unwrap()
        };
        let forward = build(entries);
        let backward = build(reversed);
        let selection = PackExtractionSelection::new(include_packages, include_fonts);

        prop_assert_eq!(
            observed_entries(&forward, selection),
            observed_entries(&backward, selection),
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
            .file("main.typ", b"main".to_vec()).unwrap()
            .file(project_path, b"project".to_vec()).unwrap()
            .package_file(package_spec(), stem, b"package".to_vec()).unwrap()
            .build().unwrap();

        let error = plan_pack_extraction(
            &pack,
            PackExtractionSelection::new(true, false),
        ).unwrap_err();

        let is_path_conflict = matches!(error.issues(), [PackExtractionPlanIssue::PathConflict { .. }]);
        prop_assert!(is_path_conflict);
    }

    #[test]
    fn path_segment_prefixes_do_not_collide(
        stem in "[a-z][a-z0-9]{0,12}",
    ) {
        let project_path = format!("packages/local/example/1.0.0/{stem}-sibling");
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec()).unwrap()
            .file(project_path, b"project".to_vec()).unwrap()
            .package_file(package_spec(), stem, b"package".to_vec()).unwrap()
            .build().unwrap();

        let plan = plan_pack_extraction(
            &pack,
            PackExtractionSelection::new(true, false),
        ).unwrap();

        prop_assert_eq!(plan.entries().len(), 3);
    }

    #[test]
    fn planning_and_plan_clones_share_generated_payloads(
        bytes in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let source_pointer = bytes.as_ptr();
        let pack = Pack::builder("main.typ")
            .file("main.typ", bytes).unwrap()
            .build().unwrap();
        let pack_pointer = pack.file("main.typ").unwrap().as_ptr();
        let plan = plan_pack_extraction(
            &pack,
            PackExtractionSelection::new(false, false),
        ).unwrap();
        let cloned = plan.clone();

        prop_assert_eq!(pack_pointer, source_pointer);
        prop_assert_eq!(plan.entries()[0].bytes().as_ptr(), pack_pointer);
        prop_assert_eq!(cloned.entries()[0].bytes().as_ptr(), pack_pointer);
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn embedded_font_containers_are_selected_explicitly_and_external_fonts_are_excluded() {
    let font = fonts::typst_container();
    let embedded = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .font(font.clone(), 0)
        .unwrap()
        .build()
        .unwrap();
    let external = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .external_font(font, 0)
        .unwrap()
        .build()
        .unwrap();

    let excluded =
        plan_pack_extraction(&embedded, PackExtractionSelection::new(false, false)).unwrap();
    let included =
        plan_pack_extraction(&embedded, PackExtractionSelection::new(false, true)).unwrap();
    let unavailable =
        plan_pack_extraction(&external, PackExtractionSelection::new(false, true)).unwrap();

    assert_eq!(excluded.entries().len(), 1);
    assert_eq!(included.entries().len(), 2);
    assert_eq!(
        included.entries()[0].role(),
        PackExtractionEntryRole::FontContainer
    );
    assert_eq!(unavailable.entries().len(), 1);
}

#[cfg(feature = "embedded-fonts")]
proptest! {
    #[test]
    fn repeated_font_container_references_coalesce(
        reverse_faces in any::<bool>(),
    ) {
        let face = fonts::typst_container();
        let collection = fonts::font_collection(&[face.clone(), face]);
        let container_pointer = collection.as_ptr();
        let indices = if reverse_faces { [1, 0] } else { [0, 1] };
        let mut builder = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec()).unwrap();
        for index in indices {
            builder = builder.font(collection.clone(), index).unwrap();
        }
        let pack = builder.build().unwrap();
        let plan = plan_pack_extraction(
            &pack,
            PackExtractionSelection::new(false, true),
        ).unwrap();
        let font_entries = plan.entries().iter()
            .filter(|entry| entry.role() == PackExtractionEntryRole::FontContainer)
            .collect::<Vec<_>>();

        prop_assert_eq!(font_entries.len(), 1);
        prop_assert_eq!(font_entries[0].bytes(), collection.as_slice());
        prop_assert_eq!(font_entries[0].bytes().as_ptr(), pack.fonts()[0].data().as_ptr());
        prop_assert_ne!(font_entries[0].bytes().as_ptr(), container_pointer);
    }
}
