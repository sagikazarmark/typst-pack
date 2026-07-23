use std::collections::{BTreeMap, BTreeSet};

fn main() {
    for path in ["Cargo.toml", "Cargo.lock", "embedded-typst.toml"] {
        println!("cargo:rerun-if-changed={path}");
    }

    verify_embedded_typst_baseline();

    for variable in ["TARGET", "CARGO_CFG_TARGET_FEATURE"] {
        println!(
            "cargo:rustc-env=TYPST_PACK_{variable}={}",
            std::env::var(variable).unwrap_or_default()
        );
    }

    let features = enabled_feature_set();
    println!(
        "cargo:rustc-env=TYPST_PACK_FEATURE_SET={}",
        if features.is_empty() {
            "none".to_owned()
        } else {
            features.join(",")
        }
    );
}

fn enabled_feature_set() -> Vec<String> {
    let manifest = parse("Cargo.toml", include_str!("Cargo.toml"));
    let features = manifest["features"]
        .as_table()
        .expect("Cargo.toml must contain features");
    let mut encoded = BTreeSet::new();
    let mut enabled = Vec::new();
    for feature in features.keys() {
        let environment = format!(
            "CARGO_FEATURE_{}",
            feature.to_ascii_uppercase().replace('-', "_")
        );
        assert!(
            encoded.insert(environment.clone()),
            "Cargo features must have distinct environment encodings"
        );
        if std::env::var_os(environment).is_some() {
            enabled.push(feature.clone());
        }
    }
    enabled.sort();
    enabled
}

fn verify_embedded_typst_baseline() {
    let manifest = parse("Cargo.toml", include_str!("Cargo.toml"));
    let lockfile = parse("Cargo.lock", include_str!("Cargo.lock"));
    let baseline = parse("embedded-typst.toml", include_str!("embedded-typst.toml"));
    let declarations = baseline["crate"]
        .as_array()
        .expect("embedded-typst.toml must contain [[crate]] entries");
    let mut approved = BTreeMap::new();
    let mut identities = BTreeMap::new();

    for declaration in declarations {
        let name = string(declaration, "name");
        assert!(
            approved.insert(name, declaration).is_none(),
            "duplicate approved Typst crate `{name}`"
        );
        if let Some(identity) = declaration.get("identity").and_then(toml::Value::as_str) {
            assert!(
                identities.insert(identity, declaration).is_none(),
                "duplicate embedded Typst identity `{identity}`"
            );
        }
    }

    assert_eq!(
        identities.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from(["engine", "html", "pdf", "png", "svg"]),
        "embedded-typst.toml must classify the engine and every exporter identity"
    );
    assert_eq!(
        baseline["baseline"]["engine"].as_str(),
        Some(string(identities["engine"], "version")),
        "the Engine baseline must match the built typst crate"
    );
    assert_eq!(
        baseline["official-cli"]["version"].as_str(),
        baseline["baseline"]["engine"].as_str(),
        "the official CLI oracle must match the Engine baseline"
    );

    let packages = lockfile["package"]
        .as_array()
        .expect("Cargo.lock must contain packages");
    let resolved_typst = packages
        .iter()
        .filter_map(|package| package["name"].as_str())
        .filter(|name| (*name == "typst" || name.starts_with("typst-")) && *name != "typst-pack")
        .collect::<BTreeSet<_>>();
    assert_eq!(
        approved.keys().copied().collect::<BTreeSet<_>>(),
        resolved_typst,
        "every resolved Typst crate must be explicitly approved"
    );

    for (&name, declaration) in &approved {
        let version = string(declaration, "version");
        let checksum = string(declaration, "checksum");
        assert_eq!(checksum.len(), 64, "invalid approved checksum for `{name}`");
        let matches = packages
            .iter()
            .filter(|package| package["name"].as_str() == Some(name))
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "`{name}` must resolve exactly once; incompatible duplicate releases are forbidden"
        );
        assert_eq!(matches[0]["version"].as_str(), Some(version));
        assert_eq!(matches[0]["checksum"].as_str(), Some(checksum));
    }

    let dependencies = manifest["dependencies"]
        .as_table()
        .expect("Cargo.toml must contain dependencies");
    for (name, specification) in dependencies
        .iter()
        .filter(|(name, _)| *name == "typst" || name.starts_with("typst-"))
    {
        let declaration = approved
            .get(name.as_str())
            .unwrap_or_else(|| panic!("direct Typst dependency `{name}` is not approved"));
        assert_eq!(declaration["direct"].as_bool(), Some(true));
        let requirement = specification
            .as_str()
            .or_else(|| specification["version"].as_str())
            .unwrap_or_else(|| panic!("direct Typst dependency `{name}` has no version"));
        assert_eq!(
            requirement,
            format!("={}", string(declaration, "version")),
            "direct Typst dependency `{name}` must use its approved exact pin"
        );
    }
    for declaration in declarations
        .iter()
        .filter(|entry| entry["direct"].as_bool() == Some(true))
    {
        let name = string(declaration, "name");
        assert!(
            dependencies.contains_key(name),
            "approved direct Typst dependency `{name}` is missing"
        );
    }

    for (identity, declaration) in identities {
        let prefix = format!("TYPST_PACK_{}", identity.to_ascii_uppercase());
        println!(
            "cargo:rustc-env={prefix}_CRATE={}",
            string(declaration, "name")
        );
        println!(
            "cargo:rustc-env={prefix}_VERSION={}",
            string(declaration, "version")
        );
        println!(
            "cargo:rustc-env={prefix}_CHECKSUM={}",
            string(declaration, "checksum")
        );
    }
}

fn parse(name: &str, source: &str) -> toml::Value {
    toml::from_str(source).unwrap_or_else(|error| panic!("failed to parse {name}: {error}"))
}

fn string<'a>(value: &'a toml::Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string `{key}` in {value}"))
}
