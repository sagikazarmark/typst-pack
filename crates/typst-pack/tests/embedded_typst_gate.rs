use std::collections::BTreeSet;

#[path = "support/differential.rs"]
mod differential;

use differential::DIFFERENTIAL_COVERAGE;

fn baseline() -> toml::Value {
    toml::from_str(include_str!("../embedded-typst.toml")).unwrap()
}

#[test]
fn approved_baseline_covers_the_complete_differential_matrix() {
    let baseline = baseline();
    let categories = baseline["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["category"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        categories.len(),
        baseline["matrix"].as_array().unwrap().len(),
        "differential matrix categories must be unique"
    );

    let classifications = [
        "adapter-concern",
        "intentional-pack-difference",
        "pack-invariant",
        "unavoidable-mirror",
        "upstream-behavior",
    ];
    for entry in baseline["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .chain(baseline["semantic"].as_array().unwrap())
    {
        let classification = entry["classification"].as_str().unwrap();
        assert!(
            classifications.contains(&classification),
            "unclassified embedded Typst behavior: {entry}"
        );
        if let Some(coverage) = entry.get("coverage") {
            assert!(!coverage.as_array().unwrap().is_empty());
        }
    }
    let typed_categories = DIFFERENTIAL_COVERAGE
        .iter()
        .map(|coverage| coverage.category.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        typed_categories.len(),
        DIFFERENTIAL_COVERAGE.len(),
        "typed differential categories must be unique"
    );
    assert_eq!(typed_categories, categories);
    assert!(
        DIFFERENTIAL_COVERAGE
            .iter()
            .all(|coverage| !coverage.suites.is_empty()),
        "every differential category must name an executable suite"
    );
}

#[test]
fn the_reported_version_names_the_approved_engine() {
    let baseline = baseline();
    let engine = baseline["crate"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"].as_str() == Some("typst"))
        .map(|entry| entry["version"].as_str().unwrap())
        .unwrap();

    // `VERSION` is also the implementation string of compilation identities.
    assert!(
        typst_pack::VERSION.ends_with(&format!("(Typst {engine})")),
        "{}",
        typst_pack::VERSION
    );
}
