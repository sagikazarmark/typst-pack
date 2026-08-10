//! The crate features that select capabilities, as the manifest declares them.
//!
//! Filesystem access and network egress are separately selectable, so a
//! deployment owner chooses whether the library can reach the network at all.
//! Read from the feature table, they keep the one door to a transport the only
//! door, whichever features a build selects. What the resolved dependency graph
//! of one build actually contains is a stronger property that only a graph
//! check can establish; the containerized filesystem-without-egress check reads
//! that graph, and the last test here holds the crates it probes to the ones
//! this manifest can activate.

use std::collections::BTreeSet;

/// Every crate this manifest can activate that speaks HTTP, together with the
/// typst-kit feature that would bring one of its own.
const TRANSPORT: &[&str] = &[
    "env_proxy",
    "rustls-pemfile",
    "typst-kit/system-downloader",
    "ureq",
    "webpki-roots",
];

#[test]
fn opendal_selects_only_its_minimal_async_dependencies() {
    let manifest = manifest();
    let declared = manifest["features"]["opendal"]
        .as_array()
        .expect("the opendal feature must enable a list")
        .iter()
        .map(|entry| entry.as_str().expect("feature entries must be strings"))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        declared,
        BTreeSet::from(["dep:futures-util", "dep:opendal"])
    );
    assert!(
        !feature_closure("opendal").contains("package-acquisition"),
        "OpenDAL archive acquisition must not imply archive expansion"
    );
}

#[test]
fn opendal_dependencies_preserve_the_caller_owned_runtime_boundary() {
    let manifest = manifest();
    let dependencies = manifest["dependencies"]
        .as_table()
        .expect("Cargo.toml must declare dependencies");
    let opendal = dependencies["opendal"]
        .as_table()
        .expect("OpenDAL must use a detailed dependency declaration");
    let futures = dependencies["futures-util"]
        .as_table()
        .expect("futures-util must use a detailed dependency declaration");

    assert_eq!(opendal["version"].as_str(), Some("0.58"));
    assert_eq!(opendal["default-features"].as_bool(), Some(false));
    assert_eq!(opendal["optional"].as_bool(), Some(true));
    assert_eq!(futures["version"].as_str(), Some("0.3.31"));
    assert_eq!(futures["default-features"].as_bool(), Some(false));
    assert_eq!(futures["optional"].as_bool(), Some(true));
    assert_eq!(
        futures["features"]
            .as_array()
            .expect("futures-util must name its minimal features")
            .iter()
            .map(|feature| feature
                .as_str()
                .expect("dependency features must be strings"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["alloc", "async-await"])
    );
    assert!(
        !dependencies.contains_key("opendal-core"),
        "opendal-core must remain transitive"
    );
    assert!(
        !manifest
            .get("build-dependencies")
            .and_then(toml::Value::as_table)
            .is_some_and(|dependencies| dependencies.contains_key("opendal-core")),
        "opendal-core must not be a direct build dependency"
    );
    for target in manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
    {
        assert!(
            !target
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .is_some_and(|dependencies| dependencies.contains_key("opendal-core")),
            "opendal-core must not be a direct target dependency"
        );
    }
}

#[test]
fn docs_rs_builds_the_opendal_namespace() {
    let manifest = manifest();
    let features = manifest["package"]["metadata"]["docs"]["rs"]["features"]
        .as_array()
        .expect("docs.rs metadata must select features");

    assert!(
        features
            .iter()
            .any(|feature| feature.as_str() == Some("opendal")),
        "docs.rs must build the OpenDAL API"
    );
}

#[test]
fn egress_builds_on_the_filesystem_and_acquisition_features() {
    let enabled = feature_closure("egress");

    assert!(enabled.contains("fs"), "egress must imply the fs feature");
    assert!(
        enabled.contains("package-acquisition"),
        "egress must build on the package-acquisition feature"
    );
}

/// Which covers the filesystem feature: it is one of the features that must
/// reach a transport only by enabling egress, and it never enables it.
#[test]
fn egress_is_the_only_feature_that_links_a_transport() {
    for feature in features().keys() {
        let linked = transport_linked_by(feature);
        if linked.is_empty() {
            continue;
        }
        assert!(
            feature_closure(feature).contains("egress"),
            "feature `{feature}` links {linked:?} without enabling egress"
        );
    }
}

/// The graph check can only find a transport crate it names, so the crates it
/// probes are exactly the ones this manifest can activate.
#[test]
fn the_dependency_graph_check_probes_every_transport_crate() {
    let named = TRANSPORT
        .iter()
        .copied()
        // Another crate's feature is no entry of its own in a dependency graph;
        // the transport it would bring is probed under its own name.
        .filter(|transport| !transport.contains('/'))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        probed_transport_crates(),
        named,
        "the filesystem-without-egress check must probe every transport crate, and only those"
    );
}

/// The crates the graph check probes, read from the check itself.
fn probed_transport_crates() -> BTreeSet<String> {
    let dagger = include_str!("../../../dagger.dang");
    let (_, probed) = dagger
        .split_once("let transportCrates = [")
        .expect("the filesystem-without-egress check must name the crates it probes");
    let (probed, _) = probed
        .split_once(']')
        .expect("the probed crates must be one closed list");

    probed
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// The transport crates a build with `feature` enabled links.
fn transport_linked_by(feature: &str) -> BTreeSet<&'static str> {
    let enabled = feature_closure(feature);
    TRANSPORT
        .iter()
        .copied()
        .filter(|transport| {
            // An optional dependency is activated as `dep:ureq`, another
            // crate's feature named as it is written.
            enabled.contains(&format!("dep:{transport}")) || enabled.contains(*transport)
        })
        .collect()
}

/// What a build with `feature` enabled has: the feature itself and, transitively,
/// this crate's other features it enables, the optional dependencies it
/// activates, and the features it selects in another crate.
fn feature_closure(feature: &str) -> BTreeSet<String> {
    let features = features();
    let mut enabled = BTreeSet::new();
    let mut pending = vec![feature.to_owned()];
    while let Some(name) = pending.pop() {
        if !enabled.insert(name.clone()) {
            continue;
        }
        let Some(entries) = features.get(&name) else {
            // A dependency activation or another crate's feature enables
            // nothing of ours, so the walk ends here.
            continue;
        };
        for entry in entries.as_array().expect("a feature enables a list") {
            pending.push(
                entry
                    .as_str()
                    .expect("a feature entry is a string")
                    .to_owned(),
            );
        }
    }
    enabled
}

fn features() -> toml::Table {
    let manifest = manifest();
    manifest["features"]
        .as_table()
        .expect("Cargo.toml must declare features")
        .clone()
}

fn manifest() -> toml::Table {
    toml::from_str(include_str!("../Cargo.toml")).expect("Cargo.toml must parse")
}
