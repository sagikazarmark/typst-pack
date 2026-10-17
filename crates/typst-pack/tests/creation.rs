//! Pack creation over supplied inputs.
//!
//! The inputs are bytes the caller already holds, and creation reads nothing
//! itself. The cross-adapter scenarios live in `creation_conformance.rs`.

use typst::syntax::package::PackageSpec;
use typst_pack::compile::{DocumentTime, Input, RequestIssue};
use typst_pack::{Metadata, ProjectSnapshot, create, package};

/// 2023-11-14T22:13:20Z.
const CREATION_TIMESTAMP: i64 = 1_700_000_000;

/// Creation of a `main.typ` holding `source` at a fixed document time.
fn request(source: &str) -> create::Request {
    let project = ProjectSnapshot::new("main.typ", [("main.typ", source)]).unwrap();
    let mut request = create::Request::new(project);
    request.options.document_time = DocumentTime::UnixTimestamp(CREATION_TIMESTAMP);
    request
}

/// The errors of a creation whose discovery failed.
fn failed(request: &create::Request) -> Vec<typst::diag::SourceDiagnostic> {
    match create(request) {
        Err(create::Error::Failed { errors, .. }) => errors,
        other => panic!("expected a failed discovery, got {other:?}"),
    }
}

/// The result of a creation every tree of which is already supplied.
fn created(request: &create::Request) -> create::Created {
    match create(request).unwrap() {
        create::Outcome::Created(created) => created,
        other => panic!("expected a created pack, got {other:?}"),
    }
}

fn missing(request: &create::Request) -> Vec<String> {
    match create(request).unwrap() {
        create::Outcome::MissingPackages(missing) => {
            missing.iter().map(ToString::to_string).collect()
        }
        other => panic!("expected missing packages, got {other:?}"),
    }
}

fn spec(name: &str) -> PackageSpec {
    format!("@local/{name}:1.0.0").parse().unwrap()
}

/// The tree of a package whose `lib.typ` holds `body`.
fn tree(name: &str, body: &str) -> package::Tree {
    let manifest =
        format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n");
    package::Tree::new([("typst.toml", manifest), ("lib.typ", body.to_owned())]).unwrap()
}

fn catalog(
    entries: impl IntoIterator<Item = (&'static str, &'static str, bool)>,
) -> package::Catalog {
    let mut catalog = package::Catalog::new();
    for (name, body, embedded) in entries {
        catalog
            .insert(spec(name), tree(name, body), embedded)
            .unwrap();
    }
    catalog
}

fn required(pack: &typst_pack::Pack) -> Vec<String> {
    let requirements = pack.package_requirements().iter();
    requirements
        .map(|requirement| requirement.spec.to_string())
        .collect()
}

#[test]
fn discovery_rejection_retains_every_error_and_warning() {
    let request = request(
        "#set text(font: \"Definitely Missing\")\n\
         Warning\n\
         #context { assert(false, message: \"first rejection\") }\n\
         #context { assert(false, message: \"second rejection\") }",
    );
    let Err(create::Error::Failed {
        errors, warnings, ..
    }) = create(&request)
    else {
        panic!("the rejected discovery must retain its compiler evidence");
    };

    let messages = errors.iter().map(|diagnostic| diagnostic.message.as_str());
    assert_eq!(
        messages.collect::<Vec<_>>(),
        [
            "assertion failed: first rejection",
            "assertion failed: second rejection"
        ]
    );
    let mut warnings = warnings.iter();
    assert!(warnings.any(|warning| warning.message.contains("unknown font family")));
}

#[test]
fn the_document_time_inputs_and_metadata_reach_creation() {
    let mut request = request(
        "#let today = datetime.today()\n\
         #assert.eq((today.year(), today.month(), today.day()), (2023, 11, 14))\n\
         #assert.eq(sys.inputs.at(\"key\"), \"value\")\n\
         #assert.eq(sys.inputs.data, bytes((1, 2)))",
    );
    assert!(!failed(&request).is_empty());
    request.options.inputs = vec![
        ("key".into(), Input::from("value")),
        ("data".into(), Input::from(vec![1, 2])),
    ];
    request.metadata = Some(Metadata {
        name: Some("Reused".into()),
        ..Metadata::default()
    });

    // The request is reusable: creation retains nothing.
    let first = created(&request);
    let second = created(&request);
    assert_eq!(first.pack.identity(), second.pack.identity());
    let name = second
        .pack
        .metadata()
        .and_then(|metadata| metadata.name.as_deref());
    assert_eq!(name, Some("Reused"));
}

#[test]
fn invalid_requests_are_rejected_before_compiling() {
    let mut request = request("Hello");
    request.options.document_time = DocumentTime::UnixTimestamp(i64::MAX);
    request.options.features.push(typst::Feature::Bundle);

    let Err(create::Error::InvalidRequest(issues)) = create(&request) else {
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
fn the_html_target_enables_the_html_feature_itself() {
    let mut request = request("#html.elem(\"p\")[Paragraph]");
    assert!(!failed(&request).is_empty());

    request.target = create::Target::Html;
    // The target fixes the discovery run only; it does not become Pack state.
    assert_eq!(created(&request).pack.entrypoint(), "main.typ");
}

#[test]
fn variants_union_the_packages_every_input_set_reaches() {
    let source = "#let edition = sys.inputs.at(\"edition\", default: \"alpha\")\n\
                  #if edition == \"alpha\" { import \"@local/alpha:1.0.0\": value; value }\n\
                  #if edition == \"beta\" { import \"@local/beta:1.0.0\": value; value }";
    let mut base = request(source);
    base.packages = catalog([
        ("alpha", "#let value = 1", true),
        ("beta", "#let value = 2", true),
    ]);
    assert_eq!(required(&created(&base).pack), ["@local/alpha:1.0.0"]);

    base.variants = vec![vec![("edition".into(), Input::from("beta"))]];
    assert_eq!(
        required(&created(&base).pack),
        ["@local/alpha:1.0.0", "@local/beta:1.0.0"]
    );

    // A package only one variant reaches is reported missing like any other.
    base.packages = package::Catalog::new();
    assert_eq!(missing(&base), ["@local/alpha:1.0.0", "@local/beta:1.0.0"]);

    // A variant that does not compile fails creation, and its errors name
    // the variant's inputs.
    let mut broken = request("#assert.eq(sys.inputs.at(\"ok\", default: \"yes\"), \"yes\")");
    broken.variants = vec![vec![("ok".into(), Input::from("no"))]];
    let errors = failed(&broken);
    assert!(
        errors[0]
            .hints
            .iter()
            .any(|hint| hint.v.contains("variant `ok=no`"))
    );
}

#[test]
fn a_variant_replaces_base_inputs_and_shares_warnings() {
    let source = "#set text(font: \"Definitely Missing\")\nWarning\n\
                  #let edition = sys.inputs.edition\n\
                  #if edition == \"alpha\" { import \"@local/alpha:1.0.0\": value; value }\n\
                  #if edition == \"beta\" { import \"@local/beta:1.0.0\": value; value }";
    let mut request = request(source);
    request.packages = catalog([
        ("alpha", "#let value = 1", true),
        ("beta", "#let value = 2", true),
    ]);
    request.options.inputs = vec![("edition".into(), Input::from("alpha"))];
    request.variants = vec![vec![("edition".into(), Input::from("beta"))]];

    let created = created(&request);
    assert_eq!(
        required(&created.pack),
        ["@local/alpha:1.0.0", "@local/beta:1.0.0"]
    );
    let warnings = created.warnings.iter();
    let missing_font = warnings.filter(|warning| warning.message.contains("unknown font family"));
    assert_eq!(missing_font.count(), 1);
}

#[test]
fn only_imported_packages_become_requirements_with_their_own_disposition() {
    let mut request = request(
        "#import \"@local/embedded:1.0.0\": value\n\
         #import \"@local/external:1.0.0\": other\n\
         #rect(width: (value + other) * 1pt, height: 1pt)",
    );
    request.packages = catalog([
        ("embedded", "#let value = 3", true),
        ("external", "#let other = 4", false),
        ("unused", "#let unused = 5", true),
    ]);

    let pack = created(&request).pack;

    let requirements = pack.package_requirements();
    assert_eq!(
        required(&pack),
        ["@local/embedded:1.0.0", "@local/external:1.0.0"]
    );
    assert!(requirements[0].embedded);
    assert!(!requirements[1].embedded);
    let contained = pack
        .packages()
        .map(|(spec, _)| spec.clone())
        .collect::<Vec<_>>();
    assert_eq!(contained, [spec("embedded")]);
}

#[test]
fn missing_packages_are_a_resumable_deduplicated_canonical_outcome() {
    let request = request(
        "#context { import \"@local/zeta:1.0.0\": value; value }\n\
         #context { import \"@local/alpha:1.0.0\": value; value }\n\
         #context { import \"@local/zeta:1.0.0\": value; value }",
    );

    assert_eq!(
        missing(&request),
        ["@local/alpha:1.0.0", "@local/zeta:1.0.0"]
    );
}

#[test]
fn a_resumed_creation_issues_the_pack_one_invocation_would_have() {
    // `first` needs `third`, which only `first`'s own tree imports.
    let source = "#import \"@local/first:1.0.0\": first\n\
                  #import \"@local/second:1.0.0\": second\n\
                  #rect(width: (first + second) * 1pt, height: 1pt)";
    let body = |name: &str| match name {
        "first" => "#import \"@local/third:1.0.0\": third\n#let first = 1 + third",
        "second" => "#let second = 2",
        _ => "#let third = 3",
    };
    let mut resumed = request(source);
    let pack = loop {
        match create(&resumed).unwrap() {
            create::Outcome::Created(created) => break created.pack,
            create::Outcome::MissingPackages(missing) => {
                for spec in missing {
                    let tree = tree(&spec.name, body(&spec.name));
                    resumed.packages.insert(spec, tree, true).unwrap();
                }
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    };

    let mut single = request(source);
    single.packages = resumed.packages.clone();
    assert_eq!(created(&single).pack.identity(), pack.identity());
    assert_eq!(
        required(&pack),
        [
            "@local/first:1.0.0",
            "@local/second:1.0.0",
            "@local/third:1.0.0"
        ]
    );
}

#[test]
fn a_declared_package_failure_fails_the_import_unless_a_tree_is_supplied() {
    let source = "#import \"@local/first:1.0.0\": first\n#rect(width: first * 1pt)";
    let mut request = request(source);
    let failure = package::ReadFailure::NetworkFailed(Some("connection refused".to_owned()));
    request.package_failures.insert(spec("first"), failure);

    // The caller's own reason reaches the import that asked for the package,
    // instead of the specification being reported missing again.
    let errors = failed(&request);
    let messages = errors.iter().map(|error| error.message.as_str());
    assert_eq!(
        messages.collect::<Vec<_>>(),
        ["failed to download package (connection refused)"]
    );
    assert!(errors.iter().all(|error| !error.span.is_detached()));

    request.packages = catalog([("first", "#let first = 1", true)]);
    assert_eq!(required(&created(&request).pack), ["@local/first:1.0.0"]);
}

/// Face selection out of the supplied fonts.
#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::compile::Input;
    use typst_pack::{Digest, font};

    use crate::{catalog, created, request};

    /// The exact bytes of the container Typst ships `family` in.
    fn container(family: &str) -> Vec<u8> {
        let mut fonts = typst_kit::fonts::embedded();
        let (font, _) = fonts
            .find(|(font, _)| font.info().family == family)
            .unwrap();
        font.data().to_vec()
    }

    fn entries(containers: &[(&[u8], bool)]) -> Vec<font::Entry> {
        let entries = containers.iter().map(|(data, embedded)| font::Entry {
            container: font::Container::new(data.to_vec()).unwrap(),
            embedded: *embedded,
        });
        entries.collect()
    }

    fn requirements(pack: &typst_pack::Pack) -> Vec<(Digest, bool)> {
        let requirements = pack.font_requirements().iter();
        requirements
            .map(|requirement| (requirement.digest, requirement.embedded))
            .collect()
    }

    #[test]
    fn only_selected_containers_become_requirements_with_their_disposition() {
        let (serif, mono) = (container("Libertinus Serif"), container("DejaVu Sans Mono"));
        let mut request = request("Serif\n\n#text(font: \"DejaVu Sans Mono\")[Mono]\n");
        request.fonts = entries(&[(&serif, true), (&mono, false)]);
        assert_eq!(
            requirements(&created(&request).pack),
            [(Digest::of(&serif), true), (Digest::of(&mono), false)]
        );

        let mut request = crate::request("Serif text only\n");
        request.fonts = entries(&[(&serif, true), (&mono, true)]);
        assert_eq!(
            requirements(&created(&request).pack),
            [(Digest::of(&serif), true)]
        );

        // The first matching catalog position and its disposition win.
        request.fonts = entries(&[(&serif, false), (&serif, true)]);
        assert_eq!(
            requirements(&created(&request).pack),
            [(Digest::of(&serif), false)]
        );
    }

    #[test]
    fn a_font_reached_only_under_a_variant_becomes_a_requirement() {
        let (serif, mono) = (container("Libertinus Serif"), container("DejaVu Sans Mono"));
        let mut request = request(
            "Serif\n\n#if sys.inputs.at(\"code\", default: \"no\") == \"yes\" {\n\
               text(font: \"DejaVu Sans Mono\")[Mono]\n\
             }",
        );
        request.fonts = entries(&[(&serif, true), (&mono, false)]);
        assert_eq!(
            requirements(&created(&request).pack),
            [(Digest::of(&serif), true)]
        );

        request.variants = vec![vec![("code".into(), Input::from("yes"))]];
        assert_eq!(
            requirements(&created(&request).pack),
            [(Digest::of(&serif), true), (Digest::of(&mono), false)]
        );
    }

    /// Typst's memoization cache is not evicted between resume rounds, so a
    /// round served from that cache must still select the faces it used.
    #[test]
    fn a_resumed_creation_selects_the_faces_one_invocation_would_have() {
        let serif = container("Libertinus Serif");
        // Text before the import, so the round that reports the missing
        // package already laid out a face.
        let source = "Selected text\n\n#import \"@local/first:1.0.0\": first\n#first";
        let mut resumed = request(source);
        resumed.fonts = entries(&[(&serif, true)]);
        assert!(matches!(
            typst_pack::create(&resumed).unwrap(),
            typst_pack::create::Outcome::MissingPackages(_)
        ));
        resumed.packages = catalog([("first", "#let first = [resolved]", true)]);
        let resumed = created(&resumed).pack;

        let mut single = request(source);
        single.fonts = entries(&[(&serif, true)]);
        single.packages = catalog([("first", "#let first = [resolved]", true)]);

        assert_eq!(resumed.font_requirements().len(), 1);
        assert_eq!(resumed.identity(), created(&single).pack.identity());
    }
}
