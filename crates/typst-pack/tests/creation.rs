//! Pack creation over supplied inputs.
//!
//! Every test here runs on a build with no crate feature enabled: the inputs
//! are bytes the caller already holds, and creation reads nothing itself.
//! The font section needs real font bytes, which Typst only ships with the
//! `embedded-fonts` feature.

use std::str::FromStr;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    CreateError, CreateOutcome, CreateRequest, DocumentTime, Input, Pack, PackMetadata,
    PackageCatalog, PackageCatalogError, PackageCatalogIssue, PackageDisposition,
    PackageReadFailure, PackageReadFailureReason, PackageTree, PackageTreeIssue, ProjectSnapshot,
    RequestIssue, TypstTarget, create,
};

/// 2023-11-14T22:13:20Z, the Document Time every representative request here
/// is fixed to.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// A project whose entrypoint is `main.typ`.
fn project(entries: impl IntoIterator<Item = (&'static str, Vec<u8>)>) -> ProjectSnapshot {
    ProjectSnapshot::new("main.typ", entries).unwrap()
}

fn document(source: &str) -> ProjectSnapshot {
    project([("main.typ", source.as_bytes().to_vec())])
}

/// Creation of `project` at a fixed document time.
fn request(project: ProjectSnapshot) -> CreateRequest {
    let mut request = CreateRequest::new(project);
    request.options.document_time = DocumentTime::UnixTimestamp(CREATION_TIMESTAMP);
    request
}

/// Chained request setup for the tests below.
trait Setup {
    fn packages(self, packages: PackageCatalog) -> Self;
    #[cfg(feature = "embedded-fonts")]
    fn fonts(self, fonts: typst_pack::FontCatalog) -> Self;
    fn failure(self, failure: PackageReadFailure) -> Self;
    fn metadata(self, metadata: PackMetadata) -> Self;
}

impl Setup for CreateRequest {
    fn packages(mut self, packages: PackageCatalog) -> Self {
        self.packages = packages;
        self
    }

    #[cfg(feature = "embedded-fonts")]
    fn fonts(mut self, fonts: typst_pack::FontCatalog) -> Self {
        self.fonts = fonts;
        self
    }

    fn failure(mut self, failure: PackageReadFailure) -> Self {
        self.package_failures.insert(failure);
        self
    }

    fn metadata(mut self, metadata: PackMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

struct Created {
    pack: Pack,
    warnings: Vec<typst::diag::SourceDiagnostic>,
}

/// The errors of a creation whose discovery failed.
fn failed(request: &CreateRequest) -> Vec<typst::diag::SourceDiagnostic> {
    match create(request) {
        Err(CreateError::Failed { errors, .. }) => errors,
        other => panic!("expected a failed discovery, got {other:?}"),
    }
}

/// The Pack of an invocation every tree of which is already supplied.
fn issue(request: &CreateRequest) -> Created {
    match create(request).unwrap() {
        CreateOutcome::Created { pack, warnings, .. } => Created { pack, warnings },
        CreateOutcome::MissingPackages(missing) => {
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

    let created = issue(&request(snapshot));

    assert_eq!(created.pack.entrypoint(), "main.typ");
    // Project files come from the snapshot, never from compiler observations:
    // the unread data file is contained too.
    assert_eq!(
        created
            .pack
            .files()
            .map(|(path, _)| path)
            .collect::<Vec<_>>(),
        ["data/notes.txt", "main.typ"]
    );
    assert!(created.pack.package_requirements().is_empty());
    assert!(created.pack.font_requirements().is_empty());
}

#[test]
fn representative_compile_warnings_are_returned_with_the_pack() {
    let snapshot = document("#set text(font: \"Definitely Missing\")\nWarning");

    let issued = issue(&request(snapshot));

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

    assert!(!failed(&request(snapshot)).is_empty());
}

#[test]
fn discovery_rejection_retains_complete_diagnostics_and_warnings() {
    let snapshot = document(
        "#set text(font: \"Definitely Missing\")\n\
         Warning\n\
         #context { assert(false, message: \"first rejection\") }\n\
         #context { assert(false, message: \"second rejection\") }",
    );
    let Err(CreateError::Failed {
        errors, warnings, ..
    }) = create(&request(snapshot))
    else {
        panic!("the rejected discovery must retain its compiler evidence");
    };

    assert_eq!(
        errors
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        [
            "assertion failed: first rejection",
            "assertion failed: second rejection"
        ]
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("unknown font family")),
        "{warnings:?}"
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

    issue(&request(snapshot));
}

#[test]
fn an_out_of_range_creation_timestamp_is_rejected() {
    let mut request = request(document("Hello"));
    request.options.document_time = DocumentTime::UnixTimestamp(i64::MAX);
    request.options.features.push(typst::Feature::Bundle);

    let Err(CreateError::InvalidRequest(issues)) = create(&request) else {
        panic!("an invalid request was compiled");
    };
    assert_eq!(
        issues,
        [
            RequestIssue::InvalidDocumentTimestamp,
            RequestIssue::UnsupportedBundleFeature
        ]
    );
}

#[test]
fn the_request_is_reusable_and_creation_retains_nothing() {
    let request = request(document("#rect(width: 5pt, height: 5pt)")).metadata(PackMetadata {
        name: Some("Reused".into()),
        ..PackMetadata::default()
    });

    let first = issue(&request);
    let second = issue(&request);

    assert_eq!(first.pack.identity(), second.pack.identity());
    assert_eq!(
        second
            .pack
            .metadata()
            .and_then(|metadata| metadata.name.as_deref()),
        Some("Reused")
    );
}

#[test]
fn typst_inputs_reach_the_representative_request() {
    let source = "#assert.eq(sys.inputs.at(\"key\"), \"value\")\n\
                  #assert.eq(sys.inputs.data, bytes((1, 2)))";
    let mut with_inputs = request(document(source));
    with_inputs.options.inputs = vec![
        ("key".into(), Input::from("value")),
        ("data".into(), Input::Bytes(vec![1, 2])),
    ];
    issue(&with_inputs);

    assert!(!failed(&request(document(source))).is_empty());
}

#[test]
fn the_html_target_enables_the_html_feature_itself() {
    let source = "#html.elem(\"p\")[Paragraph]";
    assert!(!failed(&request(document(source))).is_empty());

    let mut html = request(document(source));
    html.target = TypstTarget::Html;
    let issued = issue(&html);

    // The target fixes the discovery run only; it does not become Pack state.
    assert_eq!(issued.pack.entrypoint(), "main.typ");
}

#[test]
fn variants_union_the_packages_and_fonts_every_input_set_reaches() {
    let source = "#let edition = sys.inputs.at(\"edition\", default: \"alpha\")\n\
                  #if edition == \"alpha\" { import \"@local/alpha:1.0.0\": value; value }\n\
                  #if edition == \"beta\" { import \"@local/beta:1.0.0\": value; value }";
    let catalog = || {
        package_catalog([
            embedded_package(spec("alpha"), package_files("alpha", "#let value = 1")),
            embedded_package(spec("beta"), package_files("beta", "#let value = 2")),
        ])
    };
    let required = |request: &CreateRequest| {
        let pack = issue(request).pack;
        let specs = pack.package_requirements().iter();
        specs
            .map(|requirement| requirement.spec.name.to_string())
            .collect::<Vec<_>>()
    };

    let mut base = request(document(source)).packages(catalog());
    assert_eq!(required(&base), ["alpha"]);
    base.variants = vec![vec![("edition".into(), Input::from("beta"))]];
    assert_eq!(required(&base), ["alpha", "beta"]);

    // A package only one variant reaches is reported missing like any other.
    base.packages = PackageCatalog::new();
    assert!(matches!(
        create(&base).unwrap(),
        CreateOutcome::MissingPackages(missing) if missing == [spec("alpha"), spec("beta")]
    ));

    // A variant that does not compile fails creation.
    let mut broken = request(document(
        "#assert.eq(sys.inputs.at(\"ok\", default: \"yes\"), \"yes\")",
    ));
    broken.variants = vec![vec![("ok".into(), Input::from("no"))]];
    assert!(!failed(&broken).is_empty());
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

    let issued = issue(&request(snapshot).packages(package_catalog([
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
    ])));

    // Compiler observations select package requirements: the supplied tree the
    // document never imported is not one.
    let requirements = issued.pack.package_requirements();
    assert_eq!(
        requirements
            .iter()
            .map(|requirement| requirement.spec.to_string())
            .collect::<Vec<_>>(),
        [embedded.to_string(), external.to_string()]
    );
    assert!(requirements[0].embedded);
    assert!(!requirements[1].embedded);
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

    let request = request(snapshot);
    let outcome = create(&request).unwrap();

    // A normal outcome, not a failure, and no Pack: the caller resolves what it
    // names and invokes creation again. The specification is the one the
    // compiler asked for, fully versioned, so no diagnostic text is parsed.
    let CreateOutcome::MissingPackages(missing) = outcome else {
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

#[test]
fn missing_package_specifications_are_nonempty_deduplicated_and_canonical() {
    let snapshot = document(
        "#context { import \"@local/zeta:1.0.0\": value; value }\n\
         #context { import \"@local/alpha:1.0.0\": value; value }\n\
         #context { import \"@local/zeta:1.0.0\": value; value }",
    );
    let request = request(snapshot);

    let CreateOutcome::MissingPackages(missing) = create(&request).unwrap() else {
        panic!("missing package trees must produce a resumable outcome");
    };

    assert_eq!(
        missing.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["@local/alpha:1.0.0", "@local/zeta:1.0.0"]
    );
}

/// A document needing `first`, which itself needs `third`, and `second`.
const CHAINED_PACKAGES: &str = "#import \"@local/first:1.0.0\": first\n\
                                #import \"@local/second:1.0.0\": second\n\
                                #rect(width: (first + second) * 1pt, height: 1pt)";

/// The tree a resume round can resolve for one reported specification,
/// standing in for whatever read the caller's host allows.
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
/// the loop resolved. Every round builds fresh Pack Creation input from the same
/// values, as a caller resuming across a host request boundary must.
fn resume(source: &str) -> (Created, Vec<CatalogEntry>) {
    let mut resolved: Vec<CatalogEntry> = Vec::new();
    // Bounded so that a loop making no progress fails instead of hanging; the
    // number of rounds it actually takes is not asserted.
    for _ in 0..8 {
        let request = request(document(source)).packages(package_catalog(resolved.iter().cloned()));
        let outcome = create(&request).unwrap();
        match outcome {
            CreateOutcome::Created { pack, warnings, .. } => {
                return (Created { pack, warnings }, resolved);
            }
            CreateOutcome::MissingPackages(missing) => {
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
            .map(|requirement| requirement.spec.to_string())
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

    let single = issue(&request(document(CHAINED_PACKAGES)).packages(package_catalog(resolved)));

    assert_eq!(resumed.pack.identity(), single.pack.identity());
}

#[test]
fn a_specification_declared_unresolvable_fails_the_request_that_needed_it() {
    let source = "#import \"@local/first:1.0.0\": first\n#first";
    let failure = PackageReadFailure::new(
        spec("first"),
        PackageReadFailureReason::NetworkFailed {
            detail: Some("connection refused".to_owned()),
        },
    );
    let errors = failed(&request(document(source)).failure(failure));

    // The caller's own reason reaches the import that asked for the package,
    // which is the only place it and a source location can meet.
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
    let request = request(document(source)).failure(PackageReadFailure::new(
        spec("first"),
        PackageReadFailureReason::NotFound,
    ));

    // Reporting it again would ask the caller for what it said it cannot
    // supply, which is a loop that never progresses.
    assert!(!failed(&request).is_empty());
}

#[test]
fn a_tree_supplied_for_a_declared_specification_takes_precedence() {
    let source = "#import \"@local/first:1.0.0\": first\n#rect(width: first * 1pt, height: 1pt)";
    let issued = issue(
        &request(document(source))
            .failure(PackageReadFailure::new(
                spec("first"),
                PackageReadFailureReason::NotFound,
            ))
            .packages(package_catalog([embedded_package(
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
        &request(snapshot).packages(package_catalog([embedded_package(declared.clone(), files)])),
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
        CreateOutcome, Digest, FontCatalog, FontCatalogEntry, FontContainer, FontDisposition,
        create,
    };

    use crate::{
        CatalogEntry, Created, Setup, document, embedded_package, issue, package_catalog,
        package_files, request, spec,
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

        let issued = issue(&request(snapshot).fonts(catalog));

        let requirements = issued.pack.font_requirements();
        let disposition = |data: &[u8]| {
            requirements
                .iter()
                .find(|requirement| requirement.digest == Digest::of(data))
                .map(|requirement| requirement.embedded)
        };
        assert_eq!(requirements.len(), 2);
        assert_eq!(disposition(&serif), Some(true));
        assert_eq!(disposition(&mono), Some(false));
        // Font requirements keep the supplied catalog's relative order.
        assert_eq!(
            requirements
                .iter()
                .map(|requirement| requirement.digest)
                .collect::<Vec<_>>(),
            [Digest::of(&serif), Digest::of(&mono)]
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

        let issued = issue(&request(snapshot).fonts(catalog));

        let requirements = issued.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].digest, Digest::of(&serif));
    }

    #[test]
    fn selection_uses_the_first_matching_catalog_position_and_its_disposition() {
        let serif = FontContainer::new(typst_container("Libertinus Serif")).unwrap();
        let catalog = FontCatalog::from_iter([
            FontCatalogEntry::new(serif.clone(), FontDisposition::External),
            FontCatalogEntry::new(serif, FontDisposition::Embedded),
        ]);

        let issued = issue(&request(document("Selected text")).fonts(catalog));

        assert_eq!(issued.pack.font_requirements().len(), 1);
        assert!(!issued.pack.font_requirements()[0].embedded);
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
            let request = request(document(source))
                .fonts(catalog.clone())
                .packages(package_catalog(resolved.iter().cloned()));
            match create(&request).unwrap() {
                CreateOutcome::Created { pack, warnings, .. } => {
                    break Created { pack, warnings };
                }
                CreateOutcome::MissingPackages(missing) => {
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
            &request(document(source))
                .fonts(catalog)
                .packages(package_catalog(resolved)),
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
