use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use typst_pack::{OutputFormat, Pack, PackCompilationRequest, compile};

fn baseline() -> toml::Value {
    toml::from_str(include_str!("../embedded-typst.toml")).unwrap()
}

#[test]
fn approved_baseline_covers_the_complete_differential_matrix() {
    let baseline = baseline();
    let differential_sources = concat!(
        include_str!("official_typst_oracle.rs"),
        include_str!("official_typst_cli.rs"),
    );
    let categories = baseline["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["category"].as_str().unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        categories,
        BTreeSet::from([
            "diagnostics",
            "discovery",
            "fonts",
            "html",
            "packages",
            "pack-overrides",
            "pdf",
            "png",
            "replay",
            "shared-requests",
            "svg",
        ])
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
        .chain(baseline["surface"].as_array().unwrap())
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
    for entry in baseline["matrix"].as_array().unwrap() {
        for coverage in entry["coverage"].as_array().unwrap() {
            let coverage = coverage.as_str().unwrap();
            assert!(
                differential_sources.contains(coverage),
                "differential matrix coverage `{coverage}` is missing"
            );
        }
    }
}

#[test]
fn classified_semantic_and_expectation_surfaces_have_reviewed_content() {
    let baseline = baseline();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut expected = BTreeSet::from([
        ".dagger/modules/tests".to_owned(),
        "dagger.dang".to_owned(),
        "tests/fixtures/official-oracle".to_owned(),
    ]);
    for directory in ["src", "tests", "tests/support"] {
        for entry in std::fs::read_dir(root.join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && path.file_name().and_then(|name| name.to_str()) != Some("embedded_typst_gate.rs")
            {
                expected.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                );
            }
        }
    }
    let actual = baseline["surface"]
        .as_array()
        .unwrap()
        .iter()
        .map(|surface| surface["path"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);

    for surface in baseline["surface"].as_array().unwrap() {
        let path = surface["path"].as_str().unwrap();
        let actual = surface_digest(&root.join(path));
        assert_eq!(
            actual,
            surface["sha256"].as_str().unwrap(),
            "`{path}` changed without an updated embedded Typst classification review"
        );
    }
}

fn surface_digest(path: &std::path::Path) -> String {
    let mut hasher = Sha256::new();
    if path.is_file() {
        hasher.update(std::fs::read(path).unwrap());
    } else {
        let mut files = Vec::new();
        collect_files(path, path, &mut files);
        files.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative, file) in files {
            hasher.update(relative.as_bytes());
            hasher.update([0]);
            hasher.update(std::fs::read(file).unwrap());
            hasher.update([0]);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn collect_files(
    root: &std::path::Path,
    directory: &std::path::Path,
    files: &mut Vec<(String, std::path::PathBuf)>,
) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|component| component.as_os_str().to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");
            files.push((relative, path));
        }
    }
}

#[test]
fn pinned_official_cli_artifact_agrees_with_the_dagger_gate() {
    let baseline = baseline();
    let dagger = include_str!("../dagger.dang");
    let release = include_str!("../.github/workflows/release.yml");
    for key in ["version", "url", "sha256"] {
        let value = baseline["official-cli"][key].as_str().unwrap();
        assert!(dagger.contains(value), "Dagger official CLI {key} drifted");
        assert!(
            release.contains(value),
            "release official CLI {key} drifted"
        );
    }
}

#[test]
fn public_compilation_attests_the_approved_engine_and_exporters() {
    let baseline = baseline();
    let crates = baseline["crate"].as_array().unwrap();
    let expected = |name: &str| {
        crates
            .iter()
            .find(|entry| entry["name"].as_str() == Some(name))
            .map(|entry| {
                (
                    entry["version"].as_str().unwrap(),
                    entry["checksum"].as_str().unwrap(),
                )
            })
            .unwrap()
    };

    for (format, source, exporter) in [
        (OutputFormat::Pdf, "Hello", "typst-pdf"),
        (OutputFormat::Png, "Hello", "typst-render"),
        (OutputFormat::Svg, "Hello", "typst-svg"),
        (OutputFormat::Html, "#html.div[Hello]", "typst-html"),
    ] {
        let pack = Pack::builder("main.typ")
            .file("main.typ", source.as_bytes().to_vec())
            .unwrap()
            .build()
            .unwrap();
        let result = compile(PackCompilationRequest::new(pack, format)).unwrap();
        let (engine_version, engine_checksum) = expected("typst");
        let (exporter_version, exporter_checksum) = expected(exporter);

        assert_eq!(result.engine_identity().implementation(), "typst");
        assert_eq!(result.engine_identity().version(), engine_version);
        assert_eq!(result.engine_identity().source_checksum(), engine_checksum);
        let mut features = Vec::new();
        for (enabled, name) in [
            (
                cfg!(feature = "_test-package-download-probe"),
                "_test-package-download-probe",
            ),
            (cfg!(feature = "cli"), "cli"),
            (cfg!(feature = "default"), "default"),
            (cfg!(feature = "embedded-fonts"), "embedded-fonts"),
            (cfg!(feature = "fs"), "fs"),
        ] {
            if enabled {
                features.push(name);
            }
        }
        assert_eq!(result.engine_identity().feature_set(), features.join(","));
        assert_eq!(result.exporter_identity().implementation(), exporter);
        assert_eq!(result.exporter_identity().version(), exporter_version);
        assert_eq!(
            result.exporter_identity().source_checksum(),
            exporter_checksum
        );
    }
}
