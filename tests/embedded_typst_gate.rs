use std::collections::BTreeSet;

#[path = "support/differential.rs"]
mod differential;

use differential::DIFFERENTIAL_COVERAGE;
use typst_pack::{OutputFormat, Pack, PackCompilationRequest, compile};

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
fn official_cli_consumers_use_the_canonical_pin() {
    let dagger = include_str!("../dagger.dang");
    let release = include_str!("../.github/workflows/release.yml");
    let installer = include_str!("../scripts/install-official-typst.sh");

    for consumer in [dagger, release] {
        assert!(consumer.contains("embedded-typst.toml"));
        assert!(consumer.contains("install-official-typst.sh"));
    }

    let baseline = baseline();
    for key in ["version", "url", "sha256"] {
        let value = baseline["official-cli"][key].as_str().unwrap();
        assert!(
            !dagger.contains(value),
            "Dagger must not copy the official CLI {key}"
        );
        assert!(
            !release.contains(value),
            "release verification must not copy the official CLI {key}"
        );
    }

    for key in ["version", "url", "sha256"] {
        assert!(
            installer.contains(&format!("read_pin {key}")),
            "the installer must consume the canonical `{key}`"
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
        let specification = match format {
            OutputFormat::Pdf => typst_pack::CompilationOutputSpecification::Pdf(
                typst_pack::PdfOutputSpecification::default(),
            ),
            OutputFormat::Png => typst_pack::CompilationOutputSpecification::Png(
                typst_pack::PngOutputSpecification::default(),
            ),
            OutputFormat::Svg => typst_pack::CompilationOutputSpecification::Svg(
                typst_pack::SvgOutputSpecification::default(),
            ),
            OutputFormat::Html => typst_pack::CompilationOutputSpecification::Html(
                typst_pack::HtmlOutputSpecification::default(),
            ),
        };
        let result = compile(PackCompilationRequest::new(pack, specification)).unwrap();
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
