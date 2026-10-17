//! The embedded Typst pinning policy. Typst crates share internal types, so
//! every `typst` and `typst-*` crate must resolve to the one release declared
//! in `embedded-typst.toml`. This runs in the repository only; downstream
//! builds of the published crate never read workspace files.

use std::path::Path;

fn read_toml(path: &Path) -> toml::Value {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn is_typst_crate(name: &str) -> bool {
    (name == "typst" || name.starts_with("typst-"))
        && !matches!(name, "typst-pack" | "typst-pack-cli")
}

fn baseline() -> toml::Value {
    toml::from_str(include_str!("../embedded-typst.toml")).unwrap()
}

fn engine_version(baseline: &toml::Value) -> &str {
    baseline["typst"]["version"].as_str().unwrap()
}

fn requirement<'a>(
    specification: &'a toml::Value,
    workspace: &'a toml::Value,
    name: &str,
) -> Option<&'a str> {
    if specification
        .get("workspace")
        .and_then(toml::Value::as_bool)
        == Some(true)
    {
        return requirement(
            &workspace["workspace"]["dependencies"][name],
            workspace,
            name,
        );
    }
    specification
        .as_str()
        .or_else(|| specification.get("version")?.as_str())
}

#[test]
fn every_direct_typst_dependency_is_pinned_exactly() {
    let baseline = baseline();
    let expected = format!("={}", engine_version(&baseline));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace = read_toml(&root.join("Cargo.toml"));

    let mut problems = Vec::new();
    for manifest in [
        "crates/typst-pack/Cargo.toml",
        "crates/typst-pack-cli/Cargo.toml",
    ] {
        let manifest_toml = read_toml(&root.join(manifest));
        let dependencies = manifest_toml["dependencies"].as_table().unwrap();
        for (name, specification) in dependencies.iter().filter(|(name, _)| is_typst_crate(name)) {
            let found = requirement(specification, &workspace, name);
            if found != Some(expected.as_str()) {
                problems.push(format!(
                    "{manifest}: `{name}` is {found:?}, expected {expected}"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_lockfile_resolves_each_typst_crate_once_at_the_embedded_release() {
    let baseline = baseline();
    let expected = engine_version(&baseline);
    let lockfile = read_toml(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"));

    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeMap::<&str, Vec<&str>>::new();
    for package in lockfile["package"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        if is_typst_crate(name) {
            seen.entry(name)
                .or_default()
                .push(package["version"].as_str().unwrap());
        }
    }
    assert!(
        seen.contains_key("typst"),
        "Cargo.lock does not resolve `typst`"
    );
    for (name, versions) in seen {
        if versions != [expected] {
            problems.push(format!(
                "`{name}` resolves to {versions:?}, expected [{expected:?}]"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_official_cli_oracle_matches_the_embedded_release() {
    let baseline = baseline();
    assert_eq!(
        baseline["official-cli"]["version"].as_str(),
        Some(engine_version(&baseline))
    );
}

#[test]
fn the_reported_version_names_the_embedded_release() {
    let baseline = baseline();
    // `VERSION` is also the implementation string of compilation identities.
    assert!(
        typst_pack::VERSION.ends_with(&format!("(Typst {})", engine_version(&baseline))),
        "{}",
        typst_pack::VERSION
    );
}
