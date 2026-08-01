//! Pack Creation over supplied inputs.
//!
//! Every test here runs on a build with no crate feature enabled: the inputs
//! are bytes the caller already holds, and creation acquires nothing itself.
//! The font section needs real font bytes, which Typst only ships with the
//! `embedded-fonts` feature.

use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CreationError, CreationOutcome, CreationRequest, IssuedPack, PackMetadata, PackageCatalog,
    PackageCatalogError, PackageCatalogIssue, PackageDisposition, PackageTree, PackageTreeIssue,
    ProjectSnapshot, ProjectSnapshotAssembly, TypstTarget, create,
};

/// 2023-11-14T22:13:20Z, the Document Time every representative request here
/// is fixed to.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// Assembles a project whose entrypoint is `main.typ`.
fn project(entries: impl IntoIterator<Item = (&'static str, Vec<u8>)>) -> ProjectSnapshot {
    ProjectSnapshotAssembly::new("main.typ")
        .assemble(entries)
        .unwrap()
}

fn document(source: &str) -> ProjectSnapshot {
    project([("main.typ", source.as_bytes().to_vec())])
}

/// The Pack of a request every tree of which is already supplied.
fn issue(request: &CreationRequest) -> IssuedPack {
    match create(request).unwrap() {
        CreationOutcome::Issued(issued) => *issued,
        CreationOutcome::MissingPackages(missing) => {
            panic!("every tree was supplied, yet creation reported {missing:?} as missing")
        }
    }
}

fn spec(name: &str) -> PackageSpec {
    PackageSpec::from_str(&format!("@local/{name}:1.0.0")).unwrap()
}

/// The Package Tree of a package whose `lib.typ` holds `body`.
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

type CatalogEntry = (PackageSpec, PackageTree, PackageDisposition);

fn package_entry(
    spec: PackageSpec,
    files: impl IntoIterator<Item = (&'static str, Vec<u8>)>,
    disposition: PackageDisposition,
) -> CatalogEntry {
    (
        spec,
        PackageTree::from_owned_entries(files).unwrap(),
        disposition,
    )
}

fn package_catalog(entries: impl IntoIterator<Item = CatalogEntry>) -> PackageCatalog {
    PackageCatalog::from_entries(entries).unwrap()
}

fn embedded_package(spec: PackageSpec, files: Vec<(&'static str, Vec<u8>)>) -> CatalogEntry {
    package_entry(spec, files, PackageDisposition::Embedded)
}

#[test]
fn a_pack_is_created_from_supplied_bytes_alone() {
    let snapshot = project([
        ("main.typ", b"#rect(width: 10pt, height: 10pt)".to_vec()),
        ("data/notes.txt", b"notes".to_vec()),
    ]);

    let issued = issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP));

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

    let issued = issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP));

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

    issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP));
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

    let first = issue(&request);
    let second = issue(&request);

    assert_eq!(first.pack.identity(), second.pack.identity());
    assert_eq!(
        second.pack.metadata().and_then(PackMetadata::name),
        Some("Reused")
    );
}

#[test]
fn typst_inputs_reach_the_representative_request() {
    let snapshot = document("#assert.eq(sys.inputs.at(\"key\"), \"value\")");
    let mut inputs = typst::foundations::Dict::new();
    inputs.insert("key".into(), typst::foundations::Value::Str("value".into()));

    issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).inputs(inputs));

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
    let issued = issue(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP)
            .target(TypstTarget::Html)
            .feature(typst::Feature::Html),
    );

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

    let issued = issue(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP).package_catalog(package_catalog([
            embedded_package(
                embedded.clone(),
                package_files("embedded", "#let value = 3"),
            ),
            package_entry(
                external.clone(),
                package_files("external", "#let other = 4"),
                PackageDisposition::External,
            ),
            embedded_package(unused.clone(), package_files("unused", "#let unused = 5")),
        ])),
    );

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
    // The whole Package Tree travels, not only the observed files.
    assert!(issued.pack.package_file(&embedded, "typst.toml").is_some());
    assert!(issued.pack.package_file(&embedded, "lib.typ").is_some());
}

#[test]
fn a_package_no_supplied_tree_covers_is_reported_as_a_resumable_outcome() {
    let snapshot = document("#import \"@local/absent:1.0.0\": value\n#value");

    let outcome = create(&CreationRequest::new(snapshot, CREATION_TIMESTAMP)).unwrap();

    // A normal outcome, not a failure, and no Pack: the caller resolves what it
    // names and invokes creation again. The specification is the one the
    // compiler asked for, fully versioned, so no diagnostic text is parsed.
    let CreationOutcome::MissingPackages(missing) = outcome else {
        panic!("the package no tree covers is reported, not packed");
    };
    assert_eq!(
        missing
            .iter()
            .map(|spec| spec.to_string())
            .collect::<Vec<_>>(),
        ["@local/absent:1.0.0"]
    );
}

/// A document needing `first`, which itself needs `third`, and `second`.
const CHAINED_PACKAGES: &str = "#import \"@local/first:1.0.0\": first\n\
                                #import \"@local/second:1.0.0\": second\n\
                                #rect(width: (first + second) * 1pt, height: 1pt)";

/// The tree a resume round can resolve for one reported specification,
/// standing in for whatever acquisition the caller's host allows.
fn resolvable(spec: &PackageSpec) -> CatalogEntry {
    let body = match spec.name.as_str() {
        "first" => "#import \"@local/third:1.0.0\": third\n#let first = 1 + third",
        "second" => "#let second = 2",
        "third" => "#let third = 3",
        name => panic!("no tree is resolvable for `{name}`"),
    };
    embedded_package(spec.clone(), package_files(spec.name.as_str(), body))
}

/// Drives the resume protocol to an issued Pack, returning it with the trees
/// the loop resolved. Every round builds a fresh Creation Request from the same
/// values, as a caller resuming across a host request boundary must.
fn resume(source: &str) -> (IssuedPack, Vec<CatalogEntry>) {
    let mut resolved: Vec<CatalogEntry> = Vec::new();
    // Bounded so that a loop making no progress fails instead of hanging; the
    // number of rounds it actually takes is not asserted.
    for _ in 0..8 {
        let request = CreationRequest::new(document(source), CREATION_TIMESTAMP)
            .package_catalog(package_catalog(resolved.iter().cloned()));
        let outcome = create(&request).unwrap();
        match outcome {
            CreationOutcome::Issued(issued) => return (*issued, resolved),
            CreationOutcome::MissingPackages(missing) => {
                assert!(
                    !missing.is_empty(),
                    "a missing outcome names a specification"
                );
                resolved.extend(missing.iter().map(resolvable));
            }
        }
    }
    panic!("creation never issued a Pack");
}

#[test]
fn a_project_needing_several_packages_completes_through_repeated_invocation() {
    let (issued, resolved) = resume(CHAINED_PACKAGES);

    assert_eq!(
        issued
            .pack
            .package_requirements()
            .iter()
            .map(|requirement| requirement.spec().to_string())
            .collect::<Vec<_>>(),
        [
            "@local/first:1.0.0",
            "@local/second:1.0.0",
            "@local/third:1.0.0"
        ]
    );
    // The loop resolved exactly what creation reported, including the package
    // only another package's tree imports.
    assert_eq!(resolved.len(), 3);
}

#[test]
fn a_resumed_creation_issues_the_pack_one_invocation_would_have() {
    let (resumed, resolved) = resume(CHAINED_PACKAGES);

    let single = issue(
        &CreationRequest::new(document(CHAINED_PACKAGES), CREATION_TIMESTAMP)
            .package_catalog(package_catalog(resolved)),
    );

    assert_eq!(resumed.pack.identity(), single.pack.identity());
}

#[test]
fn a_specification_declared_unresolvable_fails_the_request_that_needed_it() {
    let source = "#import \"@local/first:1.0.0\": first\n#first";
    let reason = typst::diag::PackageError::NetworkFailed(Some("connection refused".into()));

    let error = create(
        &CreationRequest::new(document(source), CREATION_TIMESTAMP)
            .unresolvable_package(spec("first"), reason),
    )
    .unwrap_err();

    // The caller's own reason reaches the import that asked for the package,
    // which is the only place it and a source location can meet.
    let CreationError::Compile { errors, .. } = error else {
        panic!("a declared-unresolvable specification did not fail the request: {error}");
    };
    assert_eq!(
        errors
            .iter()
            .map(|error| error.message.to_string())
            .collect::<Vec<_>>(),
        ["failed to download package (connection refused)"]
    );
    assert!(errors.iter().all(|error| !error.span.is_detached()));
}

#[test]
fn a_declared_specification_is_no_longer_reported_as_missing() {
    let source = "#import \"@local/first:1.0.0\": first\n#first";
    let request = CreationRequest::new(document(source), CREATION_TIMESTAMP).unresolvable_package(
        spec("first"),
        typst::diag::PackageError::NotFound(spec("first")),
    );

    // Reporting it again would ask the caller for what it said it cannot
    // supply, which is a loop that never progresses.
    assert!(matches!(
        create(&request).unwrap_err(),
        CreationError::Compile { .. }
    ));
}

#[test]
fn a_tree_supplied_for_a_declared_specification_takes_precedence() {
    let source = "#import \"@local/first:1.0.0\": first\n#rect(width: first * 1pt, height: 1pt)";
    let issued = issue(
        &CreationRequest::new(document(source), CREATION_TIMESTAMP)
            .unresolvable_package(
                spec("first"),
                typst::diag::PackageError::NotFound(spec("first")),
            )
            .package_catalog(package_catalog([embedded_package(
                spec("first"),
                package_files("first", "#let first = 1"),
            )])),
    );

    assert!(issued.pack.has_package(&spec("first")));
}

/// Creates over one tree supplied for `@local/declared:1.0.0`, which the
/// document imports, and returns the failure that tree produced.
fn declared_tree_failure(files: Vec<(&'static str, Vec<u8>)>) -> PackageCatalogError {
    PackageCatalog::from_entries([embedded_package(spec("declared"), files)]).unwrap_err()
}

/// Whether the failure is the distinct one a tree that does not declare its
/// specification produces, rather than a missing-package outcome or a compile
/// failure.
fn is_mismatched_declared_tree(error: &PackageCatalogError) -> bool {
    error.issues().iter().any(|issue| {
        matches!(
            issue,
            PackageCatalogIssue::MissingDeclaration { spec: reported }
                | PackageCatalogIssue::DeclarationNotUtf8 { spec: reported }
                | PackageCatalogIssue::MalformedDeclaration { spec: reported, .. }
                | PackageCatalogIssue::MismatchedName { spec: reported, .. }
                | PackageCatalogIssue::MismatchedVersion { spec: reported, .. }
                if reported == &spec("declared")
        )
    })
}

#[test]
fn a_supplied_tree_that_declares_another_package_fails_creation() {
    let error = declared_tree_failure(package_files("other", "#let value = 1"));

    // A distinct failure, not a missing-package outcome: a caller that resolved
    // this tree would otherwise be told the same specification is missing
    // forever.
    assert!(is_mismatched_declared_tree(&error), "{error}");
}

#[test]
fn a_supplied_tree_that_declares_another_version_fails_creation() {
    let error = declared_tree_failure(vec![
        (
            "typst.toml",
            b"[package]\nname = \"declared\"\nversion = \"2.0.0\"\nentrypoint = \"lib.typ\"\n"
                .to_vec(),
        ),
        ("lib.typ", b"#let value = 1".to_vec()),
    ]);

    assert!(is_mismatched_declared_tree(&error), "{error}");
}

#[test]
fn a_tree_the_representative_request_never_reads_is_checked_too() {
    // What the caller supplied is checked, not what one run happened to reach,
    // exactly as a package path that cannot be represented is.
    let unread = spec("unread");

    let error = PackageCatalog::from_entries([embedded_package(
        unread.clone(),
        package_files("other", "#let value = 1"),
    )])
    .unwrap_err();

    assert!(
        error.issues().iter().any(
            |issue| matches!(issue, PackageCatalogIssue::MismatchedName { spec, .. } if spec == &unread)
        ),
        "{error}"
    );
}

#[test]
fn a_tree_declaring_its_specification_is_accepted_whatever_else_it_declares() {
    let declared = spec("declared");
    let snapshot = document("#import \"@local/declared:1.0.0\": value\n#rect(width: value * 1pt)");
    let files = vec![
        (
            "typst.toml",
            b"[package]\n\
              name = \"declared\"\n\
              version = \"1.0.0\"\n\
              entrypoint = \"lib.typ\"\n\
              authors = [\"Author\"]\n\
              license = \"MIT\"\n\
              exclude = [\"tests/**\"]\n\
              \n\
              [template]\n\
              path = \"template\"\n\
              entrypoint = \"main.typ\"\n\
              \n\
              [tool.some-tool]\n\
              key = \"value\"\n"
                .to_vec(),
        ),
        ("lib.typ", b"#let value = 1".to_vec()),
    ];

    let issued = issue(
        &CreationRequest::new(snapshot, CREATION_TIMESTAMP)
            .package_catalog(package_catalog([embedded_package(declared.clone(), files)])),
    );

    assert!(issued.pack.has_package(&declared));
}

#[test]
fn a_supplied_tree_whose_declaration_cannot_be_read_fails_creation() {
    let absent = declared_tree_failure(vec![("lib.typ", b"#let value = 1".to_vec())]);
    let malformed = declared_tree_failure(vec![
        ("typst.toml", b"[package\nname =".to_vec()),
        ("lib.typ", b"#let value = 1".to_vec()),
    ]);

    for error in [absent, malformed] {
        assert!(is_mismatched_declared_tree(&error), "{error}");
    }
}

#[test]
fn a_supplied_tree_path_that_cannot_name_a_package_file_is_rejected() {
    let error = PackageTree::from_owned_entries([("../escape.typ", b"nope".to_vec())]).unwrap_err();

    assert!(
        error.issues().iter().any(
            |issue| matches!(issue, PackageTreeIssue::InvalidPath { path, .. }
                if path == "../escape.typ")
        ),
        "{error}"
    );
}

/// Face selection out of the supplied Font Catalog.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::{
        CreationOutcome, CreationRequest, FontCatalog, FontCatalogEntry, FontContainer,
        FontContainerIdentity, FontDisposition, create,
    };

    use crate::{
        CREATION_TIMESTAMP, CatalogEntry, document, embedded_package, issue, package_catalog,
        package_files, spec,
    };

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
        let catalog = FontCatalog::from_iter([
            FontCatalogEntry::new(
                FontContainer::new(serif.clone()).unwrap(),
                FontDisposition::Embedded,
            ),
            FontCatalogEntry::new(
                FontContainer::new(mono.clone()).unwrap(),
                FontDisposition::External,
            ),
        ]);

        let issued =
            issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).font_catalog(catalog));

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
        // The Pack Font Catalog keeps the supplied catalog's relative order.
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
        let catalog = FontCatalog::from_iter([
            FontCatalogEntry::new(
                FontContainer::new(serif.clone()).unwrap(),
                FontDisposition::Embedded,
            ),
            FontCatalogEntry::new(FontContainer::new(mono).unwrap(), FontDisposition::Embedded),
        ]);

        let issued =
            issue(&CreationRequest::new(snapshot, CREATION_TIMESTAMP).font_catalog(catalog));

        let requirements = issued.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(
            requirements[0].container_identity(),
            FontContainerIdentity::from_bytes(&serif)
        );
    }

    #[test]
    fn selection_uses_the_first_matching_catalog_position_and_its_disposition() {
        let serif = FontContainer::new(typst_container("Libertinus Serif")).unwrap();
        let catalog = FontCatalog::from_iter([
            FontCatalogEntry::new(serif.clone(), FontDisposition::External),
            FontCatalogEntry::new(serif, FontDisposition::Embedded),
        ]);

        let issued = issue(
            &CreationRequest::new(document("Selected text"), CREATION_TIMESTAMP)
                .font_catalog(catalog),
        );

        assert_eq!(issued.pack.font_requirements().len(), 1);
        assert!(!issued.pack.font_requirements()[0].is_embedded());
        assert_eq!(issued.pack.font_catalog().len(), 1);
        assert!(!issued.pack.font_catalog()[0].is_embedded());
    }

    /// Face selection is recorded as the representative request asks for a
    /// face, and Typst's memoization cache is deliberately not evicted between
    /// resume rounds, so a round served from that cache must still select the
    /// faces it used. Every other resume fixture lays out no text, which is
    /// why this one exists.
    #[test]
    fn a_resumed_creation_selects_the_faces_one_invocation_would_have() {
        let serif = typst_container("Libertinus Serif");
        let catalog = FontCatalog::from_iter([FontCatalogEntry::new(
            FontContainer::new(serif.clone()).unwrap(),
            FontDisposition::Embedded,
        )]);
        // Text before the import, so the round that reports the missing
        // package is one that already laid out a face.
        let source = "Selected text\n\n#import \"@local/first:1.0.0\": first\n#first";

        let mut resolved: Vec<CatalogEntry> = Vec::new();
        let resumed = loop {
            let request = CreationRequest::new(document(source), CREATION_TIMESTAMP)
                .font_catalog(catalog.clone())
                .package_catalog(package_catalog(resolved.iter().cloned()));
            match create(&request).unwrap() {
                CreationOutcome::Issued(issued) => break *issued,
                CreationOutcome::MissingPackages(missing) => {
                    resolved.extend(missing.iter().map(|missing| {
                        embedded_package(
                            missing.clone(),
                            package_files(missing.name.as_str(), "#let first = [resolved]"),
                        )
                    }));
                }
            }
        };

        let single = issue(
            &CreationRequest::new(document(source), CREATION_TIMESTAMP)
                .font_catalog(catalog)
                .package_catalog(package_catalog(resolved)),
        );

        assert_eq!(
            resumed.pack.font_requirements().len(),
            1,
            "a resumed creation selected no face"
        );
        assert_eq!(resumed.pack.identity(), single.pack.identity());
        let _ = spec("first");
    }
}
