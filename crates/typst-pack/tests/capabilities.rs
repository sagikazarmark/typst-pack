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
    let manifest: toml::Table =
        toml::from_str(include_str!("../Cargo.toml")).expect("Cargo.toml must parse");
    manifest["features"]
        .as_table()
        .expect("Cargo.toml must declare features")
        .clone()
}
