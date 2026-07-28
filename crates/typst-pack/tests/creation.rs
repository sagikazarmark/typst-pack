//! Pack Creation over supplied inputs.
//!
//! Every test here runs on a build with no crate feature enabled: the inputs
//! are bytes the caller already holds, and creation acquires nothing itself.
//! The font section needs real font bytes, which Typst only ships with the
//! `embedded-fonts` feature.

use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CreationError, CreationRequest, PackMetadata, ProjectIgnorePolicy, ProjectSnapshot,
    ProjectSnapshotAssembly, ResolvedPackageTree, TypstTarget, create,
};

/// 2023-11-14T22:13:20Z, the Document Time every representative request here
/// is fixed to.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// Assembles a project whose entrypoint is `main.typ`.
fn project(entries: impl IntoIterator<Item = (&'static str, Vec<u8>)>) -> ProjectSnapshot {
    let policy = ProjectIgnorePolicy::built_in();
    ProjectSnapshotAssembly::new("main.typ", &policy)
        .assemble(entries)
        .unwrap()
}

fn document(source: &str) -> ProjectSnapshot {
    project([("main.typ", source.as_bytes().to_vec())])
}

fn spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:1.0.0")).unwrap()
}

/// The Complete Package Tree of a package whose `lib.typ` holds `body`.
fn package_files(name: &str, body: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "typst.toml",
            format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n"
            )
            .into_bytes(),
        ),
        ("lib.typ", body.as_bytes().to_vec()),
    ]
}

#[test]
fn a_pack_is_created_from_supplied_bytes_alone() {
    let snapshot = project([
        ("main.typ", b"#rect(width: 10pt, height: 10pt)".to_vec()),
        ("data/notes.txt", b"notes".to_vec()),
    ]);

    let issued = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap();

    assert_eq!(issued.pack.entrypoint(), "main.typ");
    // Project files come from the snapshot, never from compiler observations:
    // the unread data file is contained too.
    assert_eq!(
        issued
            .pack
            .files()
            .map(|(path, _)| path)
            .collect::<Vec<_>>(),
        ["data/notes.txt", "main.typ"]
    );
    assert!(issued.pack.package_requirements().is_empty());
    assert!(issued.pack.font_requirements().is_empty());
}

#[test]
fn representative_compile_warnings_are_returned_with_the_pack() {
    let snapshot = document("#set text(font: \"Definitely Missing\")\nWarning");

    let issued = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap();

    assert!(
        issued
            .warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{:?}",
        issued.warnings
    );
}

#[test]
fn a_representative_request_that_does_not_compile_issues_no_pack() {
    let snapshot = document("#import \"missing.typ\": value\n#value");

    let error = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap_err();

    assert!(
        matches!(&error, CreationError::Compile { errors, .. } if !errors.is_empty()),
        "{error}"
    );
}

#[test]
fn the_creation_timestamp_fixes_the_representative_document_time() {
    let snapshot = document(
        "#let today = datetime.today()\n\
         #assert.eq(today.year(), 2023)\n\
         #assert.eq(today.month(), 11)\n\
         #assert.eq(today.day(), 14)\n",
    );

    create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap();
}

#[test]
fn an_out_of_range_creation_timestamp_is_rejected() {
    let snapshot = document("#rect()");

    let error = create(&CreationRequest::new(snapshot, i64::MAX)).unwrap_err();

    assert!(
        matches!(error, CreationError::InvalidTimestamp(_)),
        "{error}"
    );
}

#[test]
fn the_request_is_reusable_and_creation_retains_nothing() {
    let request = CreationRequest::new(document("#rect(width: 5pt, height: 5pt)"), 0)
        .metadata(PackMetadata::new().with_name("Reused"));

    let first = create(&request).unwrap();
    let second = create(&request).unwrap();

    assert_eq!(first.pack.identity(), second.pack.identity());
    assert_eq!(
        second
            .pack
            .manifest()
            .metadata()
            .and_then(PackMetadata::name),
        Some("Reused")
    );
}

#[test]
fn typst_inputs_reach_the_representative_request() {
    let snapshot = document("#assert.eq(sys.inputs.at(\"key\"), \"value\")");
    let mut inputs = typst::foundations::Dict::new();
    inputs.insert("key".into(), typst::foundations::Value::Str("value".into()));

    create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).inputs(inputs)).unwrap();

    let snapshot = document("#assert.eq(sys.inputs.at(\"key\"), \"value\")");
    let error = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap_err();
    assert!(matches!(error, CreationError::Compile { .. }), "{error}");
}

#[test]
fn the_target_and_engine_features_belong_to_the_representative_request() {
    let snapshot = document("#html.elem(\"p\")[Paragraph]");

    let error = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap_err();
    assert!(matches!(error, CreationError::Compile { .. }), "{error}");

    let snapshot = document("#html.elem(\"p\")[Paragraph]");
    let issued = create(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP)
            .target(TypstTarget::Html)
            .feature(typst::Feature::Html),
    )
    .unwrap();

    // The target fixes that one run only; it does not become Pack state.
    assert_eq!(issued.pack.entrypoint(), "main.typ");
}

#[test]
fn package_trees_are_supplied_per_specification_with_their_own_disposition() {
    let embedded = spec("embedded");
    let external = spec("external");
    let unused = spec("unused");
    let snapshot = document(
        "#import \"@local/embedded:1.0.0\": value\n\
         #import \"@local/external:1.0.0\": other\n\
         #rect(width: (value + other) * 1pt, height: 1pt)",
    );

    let issued = create(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP)
            .package_tree(ResolvedPackageTree::embedded(
                embedded.clone(),
                package_files("embedded", "#let value = 3"),
            ))
            .package_tree(ResolvedPackageTree::external(
                external.clone(),
                package_files("external", "#let other = 4"),
            ))
            .package_tree(ResolvedPackageTree::embedded(
                unused.clone(),
                package_files("unused", "#let unused = 5"),
            )),
    )
    .unwrap();

    // Compiler observations select package requirements: the supplied tree the
    // document never imported is not one.
    let requirements = issued.pack.package_requirements();
    assert_eq!(
        requirements
            .iter()
            .map(|requirement| requirement.spec().to_string())
            .collect::<Vec<_>>(),
        [embedded.to_string(), external.to_string()]
    );
    assert!(requirements[0].is_embedded());
    assert!(!requirements[1].is_embedded());
    assert!(issued.pack.has_package(&embedded));
    assert!(!issued.pack.has_package(&external));
    assert!(!issued.pack.has_package(&unused));
    // The whole Complete Package Tree travels, not only the observed files.
    assert!(issued.pack.package_file(&embedded, "typst.toml").is_some());
    assert!(issued.pack.package_file(&embedded, "lib.typ").is_some());
}

#[test]
fn an_import_no_supplied_tree_satisfies_fails_the_representative_compile() {
    let snapshot = document("#import \"@local/absent:1.0.0\": value\n#value");

    let error = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap_err();

    assert!(matches!(error, CreationError::Compile { .. }), "{error}");
}

#[test]
fn a_supplied_tree_path_that_cannot_name_a_package_file_is_rejected() {
    let absent = spec("malformed");
    let snapshot = document("#rect()");

    let error = create(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP).package_tree(
            ResolvedPackageTree::embedded(absent.clone(), [("../escape.typ", b"nope".to_vec())]),
        ),
    )
    .unwrap_err();

    assert!(
        matches!(&error, CreationError::InvalidPackagePath { spec, path, .. }
            if spec == &absent && path == "../escape.typ"),
        "{error}"
    );
}

/// Face selection out of the supplied Candidate Font Catalog.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::{
        CandidateFontCatalog, CandidateFontContainer, CreationRequest, FontContainerIdentity,
        create,
    };

    use crate::{CREATION_TIMESTAMP, document};

    /// The exact bytes of the Font Container Typst ships the given family in.
    fn typst_container(family: &str) -> Vec<u8> {
        typst_kit::fonts::embedded()
            .find(|(font, _)| font.info().family == family)
            .map(|(font, _)| font.data().to_vec())
            .unwrap_or_else(|| panic!("Typst ships `{family}`"))
    }

    #[test]
    fn font_container_dispositions_reach_the_pack_font_requirements() {
        let serif = typst_container("Libertinus Serif");
        let mono = typst_container("DejaVu Sans Mono");
        let snapshot = document("Serif text\n\n#text(font: \"DejaVu Sans Mono\")[Mono text]\n");
        let catalog = CandidateFontCatalog::from_iter([
            CandidateFontContainer::embedded(serif.clone()),
            CandidateFontContainer::external(mono.clone()),
        ]);

        let issued =
            create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).font_catalog(catalog))
                .unwrap();

        let requirements = issued.pack.font_requirements();
        let disposition = |data: &[u8]| {
            requirements
                .iter()
                .find(|requirement| {
                    requirement.container_identity() == FontContainerIdentity::from_bytes(data)
                })
                .map(|requirement| requirement.is_embedded())
        };
        assert_eq!(requirements.len(), 2);
        assert_eq!(disposition(&serif), Some(true));
        assert_eq!(disposition(&mono), Some(false));
        // The Pack Font Catalog keeps the candidate catalog's relative order.
        assert_eq!(
            issued
                .pack
                .font_catalog()
                .iter()
                .map(|face| face.identity().container())
                .collect::<Vec<_>>(),
            [
                FontContainerIdentity::from_bytes(&serif),
                FontContainerIdentity::from_bytes(&mono),
            ]
        );
    }

    #[test]
    fn only_selected_containers_become_requirements() {
        let serif = typst_container("Libertinus Serif");
        let mono = typst_container("DejaVu Sans Mono");
        let snapshot = document("Serif text only\n");
        let catalog = CandidateFontCatalog::from_iter([
            CandidateFontContainer::embedded(serif.clone()),
            CandidateFontContainer::embedded(mono),
        ]);

        let issued =
            create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).font_catalog(catalog))
                .unwrap();

        let requirements = issued.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(
            requirements[0].container_identity(),
            FontContainerIdentity::from_bytes(&serif)
        );
    }
}
