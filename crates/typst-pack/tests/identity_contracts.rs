//! Pack and compilation identity vectors over the documented preimages, and
//! what each identity binds and excludes.

use std::collections::HashSet;

use proptest::prelude::*;
use sha2::{Digest as _, Sha256};
use typst::foundations::{Datetime, Smart};
use typst::syntax::package::PackageSpec;
use typst_pack::archive::{DecodeLimits, decode, encode};
use typst_pack::compile::{CreationTimestamp, DocumentTime, Input, Output, OutputFormat};
use typst_pack::{Digest, Metadata, Pack};
use typst_pack::{compile, package};

fn identity_pack() -> Pack {
    Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .file("unused.txt", b"baseline".to_vec())
        .build()
        .unwrap()
}

proptest! {
    #[test]
    fn pack_identity_binds_generated_project_bytes_and_excludes_metadata(
        project_bytes in prop::collection::vec(any::<u8>(), 1..128),
        mutation_index in any::<usize>(),
        metadata_name in "[a-zA-Z0-9 ]{0,32}",
        metadata_author in "[a-zA-Z0-9 ]{0,32}",
    ) {
        let plain = Pack::builder("main.typ")
            .file("main.typ", project_bytes.clone())
            .build().unwrap();
        let described = Pack::builder("main.typ")
            .file("main.typ", project_bytes.clone())
            .metadata(Metadata {
                name: Some(metadata_name),
                description: None,
                authors: vec![metadata_author],
            })
            .build().unwrap();
        let mut changed_bytes = project_bytes;
        let mutation_index = mutation_index % changed_bytes.len();
        changed_bytes[mutation_index] ^= u8::MAX;
        let changed = Pack::builder("main.typ")
            .file("main.typ", changed_bytes)
            .build().unwrap();

        prop_assert_eq!(plain.identity(), described.identity());
        prop_assert_ne!(plain.identity(), changed.identity());
    }
}

/// The documented preimage encoding, built independently of the crate.
#[derive(Default)]
struct Preimage(Sha256);

impl Preimage {
    fn new(domain: &str) -> Self {
        let mut preimage = Self::default();
        preimage.str(domain);
        preimage
    }

    fn str(&mut self, value: &str) -> &mut Self {
        self.count(value.len());
        self.0.update(value.as_bytes());
        self
    }

    fn count(&mut self, value: usize) -> &mut Self {
        self.0.update((value as u64).to_be_bytes());
        self
    }

    fn digest(&mut self, value: [u8; 32]) -> &mut Self {
        self.0.update(value);
        self
    }

    fn int(&mut self, value: i64) -> &mut Self {
        self.0.update(value.to_be_bytes());
        self
    }

    fn bool(&mut self, value: bool) -> &mut Self {
        self.0.update([u8::from(value)]);
        self
    }

    fn finish(&mut self) -> String {
        hex(std::mem::take(&mut self.0).finalize().into())
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(digest: [u8; 32]) -> String {
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[test]
fn file_digests_are_plain_sha256() {
    let pack = identity_pack();
    let manifest = manifest_of(&pack);
    assert!(manifest.contains(&format!("digest = \"{}\"", hex(sha256(b"baseline")))));
    assert_eq!(
        Digest::of(b"abc").to_string(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn frozen_package_tree_and_pack_identity_vectors() {
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let tree = package::Tree::new([("lib.typ", b"lib".as_slice()), ("a.typ", b"a")]).unwrap();
    let tree_digest = Preimage::new("typst-pack/package-tree/v1")
        .count(2)
        .str("a.typ")
        .digest(sha256(b"a"))
        .str("lib.typ")
        .digest(sha256(b"lib"))
        .finish();
    assert_eq!(tree.digest().to_string(), tree_digest);

    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file("data/x.csv", b"x".to_vec())
        .package_file(spec.clone(), "lib.typ", b"lib".to_vec())
        .package_file(spec.clone(), "a.typ", b"a".to_vec())
        .build()
        .unwrap();
    let mut preimage = Preimage::new("typst-pack/pack/v1");
    preimage
        .str("main.typ")
        .count(2)
        .str("data/x.csv")
        .digest(sha256(b"x"))
        .str("main.typ")
        .digest(sha256(b"main"))
        .count(1)
        .str("@local/example:1.0.0")
        .digest(tree.digest().as_bytes().to_owned())
        .bool(true)
        .count(0);
    assert_eq!(pack.identity().to_string(), preimage.finish());
    assert_eq!(
        pack.identity().to_string(),
        "sha256:56729a615dfa1de63c64cd6e1c36e33f2029aae27812394b5f51ed47f11ee9fc"
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn font_containers_bind_their_digest_and_disposition_into_pack_identity() {
    let data = typst_pack::font::embedded().next().unwrap().data().to_vec();
    let container = typst_pack::font::Container::new(data.clone()).unwrap();
    assert_eq!(container.digest().to_string(), hex(sha256(&data)));

    for embedded in [true, false] {
        let builder = Pack::builder("main.typ").file("main.typ", b"main".to_vec());
        let builder = if embedded {
            builder.font(data.clone())
        } else {
            builder.external_font(data.clone())
        };
        let pack = builder.build().unwrap();
        let expected = Preimage::new("typst-pack/pack/v1")
            .str("main.typ")
            .count(1)
            .str("main.typ")
            .digest(sha256(b"main"))
            .count(0)
            .count(1)
            .digest(sha256(&data))
            .bool(embedded)
            .finish();
        assert_eq!(pack.identity().to_string(), expected);
    }
}

#[test]
fn pack_identity_ignores_archive_order_and_compression() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .file("b.txt", b"b".to_vec())
        .file("a.txt", b"a".to_vec())
        .metadata(Metadata {
            name: Some("ordered".to_owned()),
            ..Metadata::default()
        })
        .build()
        .unwrap();
    let archive = encode(&pack);

    // Rewrite every member in reverse order, stored rather than deflated.
    let mut source = zip::ZipArchive::new(std::io::Cursor::new(&archive)).unwrap();
    let mut members = (0..source.len())
        .map(|index| {
            let mut entry = source.by_index(index).unwrap();
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
            (entry.name().unwrap().into_owned(), data)
        })
        .collect::<Vec<_>>();
    members.reverse();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in members {
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        std::io::Write::write_all(&mut writer, &data).unwrap();
    }
    let rewritten = writer.finish().unwrap().into_inner();

    assert_ne!(rewritten, archive);
    let decoded = decode(&rewritten, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.identity(), pack.identity());
    assert_eq!(encode(&decoded), archive);
}

fn manifest_of(pack: &Pack) -> String {
    let archive = encode(pack);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).unwrap();
    let mut manifest = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("typst-pack.toml").unwrap(), &mut manifest)
        .unwrap();
    manifest
}

/// Asserts that no two of `identities` are equal.
fn assert_distinct(identities: Vec<Digest>) {
    let count = identities.len();
    let unique = identities.into_iter().collect::<HashSet<_>>();
    assert_eq!(unique.len(), count);
}

#[test]
fn pack_identity_binds_every_project_and_package_semantic() {
    // The entrypoint, one project file, and one package's files.
    type Case<'a> = (
        &'a str,
        (&'a str, &'a str),
        &'a str,
        &'a [(&'a str, &'a str)],
    );
    let identity = |(entrypoint, (path, text), package, files): Case, embedded: bool| {
        let spec = package.parse::<PackageSpec>().unwrap();
        let mut builder = Pack::builder(entrypoint)
            .file("main.typ", b"main".to_vec())
            .file(path, text.as_bytes().to_vec());
        for &(file, text) in files {
            let data = text.as_bytes().to_vec();
            builder = if embedded {
                builder.package_file(spec.clone(), file, data)
            } else {
                builder.external_package_file(spec.clone(), file, data)
            };
        }
        builder.build().unwrap().identity()
    };
    let (example, lib) = ("@local/example:1.0.0", &[("lib.typ", "package")][..]);

    assert_distinct(vec![
        identity(("main.typ", ("other.typ", "other"), example, lib), true),
        identity(("main.typ", ("other.typ", "other"), example, lib), false),
        identity(("other.typ", ("other.typ", "other"), example, lib), true),
        identity(("main.typ", ("renamed.typ", "other"), example, lib), true),
        identity(("main.typ", ("other.typ", "changed"), example, lib), true),
        identity(
            (
                "main.typ",
                ("other.typ", "other"),
                "@local/other:1.0.0",
                lib,
            ),
            true,
        ),
        identity(
            (
                "main.typ",
                ("other.typ", "other"),
                example,
                &[("x.typ", "package")],
            ),
            true,
        ),
        identity(
            (
                "main.typ",
                ("other.typ", "other"),
                example,
                &[("lib.typ", "changed")],
            ),
            true,
        ),
        identity(
            (
                "main.typ",
                ("other.typ", "other"),
                example,
                &[("lib.typ", "package"), ("extra.typ", "extra")],
            ),
            true,
        ),
    ]);
}

#[test]
fn frozen_compilation_identity_vector() {
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let manifest = b"[package]\nname = \"example\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"";
    let tree = package::Tree::new([
        ("lib.typ", b"#let value = 1".as_slice()),
        ("typst.toml", manifest),
    ])
    .unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/example:1.0.0\": value\n#value".to_vec(),
        )
        .external_package_file(spec.clone(), "lib.typ", b"#let value = 1".to_vec())
        .external_package_file(spec.clone(), "typst.toml", manifest.to_vec())
        .build()
        .unwrap();
    let mut request = compile::Request::new(
        pack.clone(),
        Output {
            pages: "2-3,1,5-".parse::<compile::PageSelection>().unwrap(),
            ..Output::new(OutputFormat::Svg)
        },
    );
    request.packages = vec![(spec, tree.clone())];
    type Encoding = fn(&mut Preimage);
    let times: [(DocumentTime, Encoding); 4] = [
        (DocumentTime::Absent, |p| {
            p.str("absent");
        }),
        (DocumentTime::UnixTimestamp(946_684_800), |p| {
            p.str("unix").int(946_684_800);
        }),
        (
            DocumentTime::Local {
                seconds: 946_684_800,
                utc_offset_seconds: -3600,
            },
            |p| {
                p.str("local").int(946_684_800).int(-3600);
            },
        ),
        // A fixed datetime is normalized to its date.
        (
            DocumentTime::Fixed(Datetime::from_ymd_hms(2000, 1, 2, 3, 4, 5).unwrap()),
            |p| {
                p.str("date").int(2000).int(1).int(2);
            },
        ),
    ];
    for (document_time, time) in times {
        request.options = compile::Options {
            inputs: vec![
                ("a".into(), Input::from("x")),
                ("b".into(), Input::from(b"y".to_vec())),
                ("a".into(), Input::from("z")),
            ],
            document_time,
            features: vec![typst::Feature::A11yExtras],
        };

        let mut expected = Preimage::new("typst-pack/compilation/v1");
        expected
            .str(typst_pack::VERSION)
            .digest(*pack.identity().as_bytes())
            // Output: SVG, ranges 1-3 and 5- (open end), no bleed, not pretty.
            .str("svg")
            .count(2)
            .count(1)
            .int(3)
            .count(5)
            .int(-1)
            .bool(false)
            .bool(false)
            // Inputs in effective order; a repeated key replaces in place.
            .count(2)
            .str("a")
            .str("str")
            .str("z")
            .str("b")
            .str("bytes")
            .digest(sha256(b"y"));
        time(&mut expected);
        let expected = expected
            .count(1)
            .str("a11y-extras")
            // External packages by specification, then external fonts.
            .count(1)
            .str("@local/example:1.0.0")
            .digest(*tree.digest().as_bytes())
            .count(0)
            .finish();
        let compiled = compile(&request).unwrap();
        assert_eq!(compiled.identity.to_string(), expected, "{document_time:?}");
    }
}

#[test]
fn frozen_pdf_output_identity_vector() {
    let pack = identity_pack();
    let mut output = Output::new(OutputFormat::Pdf);
    output.pdf_identifier = Smart::Custom("doc".into());
    output.pdf_creator = Smart::Custom(None);
    output.pdf_tags = Smart::Custom(false);
    output.pdf_timestamp = CreationTimestamp::Local {
        seconds: 1_700_000_000,
        utc_offset_seconds: 19_800,
    };
    let request = compile::Request::new(pack.clone(), output);

    let expected = Preimage::new("typst-pack/compilation/v1")
        .str(typst_pack::VERSION)
        .digest(*pack.identity().as_bytes())
        // Output: PDF, all pages, no standards, custom identifier, no creator,
        // untagged, a local timestamp with its UTC offset in seconds, not pretty.
        .str("pdf")
        .count(0)
        .count(0)
        .str("custom")
        .str("doc")
        .str("none")
        .bool(false)
        .str("local")
        .int(1_700_000_000)
        .int(19_800)
        .bool(false)
        // No inputs, absent document time, no features or external dependencies.
        .count(0)
        .str("absent")
        .count(0)
        .count(0)
        .count(0)
        .finish();
    assert_eq!(compile(&request).unwrap().identity.to_string(), expected);
}

#[test]
fn compilation_identity_binds_every_effective_output_control() {
    let identity = |format, change: fn(&mut Output)| {
        let mut output = Output::new(format);
        change(&mut output);
        compile(&compile::Request::new(identity_pack(), output))
            .unwrap()
            .identity
    };
    use OutputFormat::{Html, Pdf, Png, Svg};

    assert_distinct(vec![
        identity(Pdf, |_| {}),
        identity(Pdf, |o| o.pages = "1".parse().unwrap()),
        identity(Pdf, |o| {
            o.pdf_standards = vec![typst_pdf::PdfStandard::V_1_7]
        }),
        identity(Pdf, |o| o.pdf_identifier = Smart::Custom("identity".into())),
        identity(Pdf, |o| {
            o.pdf_creator = Smart::Custom(Some("identity".into()))
        }),
        identity(Pdf, |o| o.pdf_creator = Smart::Custom(None)),
        identity(Pdf, |o| o.pdf_tags = Smart::Custom(false)),
        identity(Pdf, |o| {
            o.pdf_timestamp = CreationTimestamp::Utc(1_700_000_000)
        }),
        identity(Pdf, |o| {
            o.pdf_timestamp = CreationTimestamp::Local {
                seconds: 1_700_000_000,
                utc_offset_seconds: 3600,
            }
        }),
        identity(Pdf, |o| o.pretty = true),
        identity(Png, |_| {}),
        identity(Png, |o| o.pages = "1".parse().unwrap()),
        identity(Png, |o| o.ppi = Some(216.0)),
        identity(Png, |o| o.render_bleed = true),
        identity(Svg, |_| {}),
        identity(Svg, |o| o.render_bleed = true),
        identity(Html, |_| {}),
        identity(Html, |o| o.pretty = true),
    ]);

    // Defaults and their explicit values are one request, and so are
    // equivalent page selections.
    assert_eq!(
        identity(Png, |_| {}),
        identity(Png, |o| o.ppi = Some(144.0))
    );
    assert_eq!(
        identity(Pdf, |_| {}),
        identity(Pdf, |o| o.pdf_tags = Smart::Custom(true))
    );
    assert_eq!(
        identity(Svg, |o| o.pages = "2-3,1".parse().unwrap()),
        identity(Svg, |o| o.pages = "1-3".parse().unwrap())
    );
    assert_ne!(
        identity(Svg, |o| o.pages = "1-3".parse().unwrap()),
        identity(Svg, |o| o.pages = "1-2".parse().unwrap())
    );
}

#[test]
fn compilation_identity_binds_pack_inputs_features_and_document_time() {
    let identity = |pack: Pack, change: fn(&mut compile::Options)| {
        let mut request = compile::Request::new(pack, Output::new(OutputFormat::Svg));
        change(&mut request.options);
        compile(&request).unwrap().identity
    };
    let changed = Pack::builder("main.typ")
        .file("main.typ", b"changed pack".to_vec())
        .build()
        .unwrap();
    let baseline = identity(identity_pack(), |_| {});

    assert_distinct(vec![
        baseline,
        identity(changed, |_| {}),
        identity(identity_pack(), |o| {
            o.inputs = vec![("unused".into(), "value".into())]
        }),
        identity(identity_pack(), |o| {
            o.inputs = vec![("unused".into(), Input::from(b"value".to_vec()))]
        }),
        identity(identity_pack(), |o| {
            o.inputs = vec![("unused".into(), Input::from(b"other".to_vec()))]
        }),
        identity(identity_pack(), |o| {
            o.features = vec![typst::Feature::A11yExtras]
        }),
        identity(identity_pack(), |o| {
            o.document_time = DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap())
        }),
        identity(identity_pack(), |o| {
            o.document_time = DocumentTime::UnixTimestamp(1_700_000_000)
        }),
        identity(identity_pack(), |o| {
            o.document_time = DocumentTime::Local {
                seconds: 1_700_000_000,
                utc_offset_seconds: 0,
            }
        }),
        identity(identity_pack(), |o| {
            o.document_time = DocumentTime::Local {
                seconds: 1_700_000_000,
                utc_offset_seconds: 3600,
            }
        }),
    ]);

    // Metadata is not part of what compiles.
    let described = Pack::builder("main.typ")
        .file(
            "main.typ",
            identity_pack().file("main.typ").unwrap().to_vec(),
        )
        .file("unused.txt", b"baseline".to_vec())
        .metadata(Metadata {
            name: Some("Named Pack".to_owned()),
            description: Some("Operational description".to_owned()),
            authors: vec!["Pack Author".to_owned()],
        })
        .build()
        .unwrap();
    assert_eq!(identity(described, |_| {}), baseline);
}

#[test]
fn compilation_identity_excludes_limits_and_unrequired_dependencies() {
    let baseline = compile::Request::new(identity_pack(), Output::new(OutputFormat::Png));
    let identity = |request: &compile::Request| compile(request).unwrap().identity;

    let mut limited = baseline.clone();
    limited.limits = compile::Limits {
        max_pages: 1,
        max_total_artifact_bytes: 1 << 20,
        export_workers: 1,
        ..compile::Limits::default()
    };
    let mut supplied = baseline.clone();
    let spec: PackageSpec = "@local/unused:1.0.0".parse().unwrap();
    let tree = package::Tree::new([("lib.typ", b"#let value = 1".as_slice())]).unwrap();
    supplied.packages = vec![(spec, tree)];
    supplied.fonts = font_containers();

    assert_eq!(identity(&limited), identity(&baseline));
    assert_eq!(identity(&supplied), identity(&baseline));
}

/// Font containers of the fonts Typst ships, or none without them.
fn font_containers() -> Vec<typst_pack::font::Container> {
    #[cfg(feature = "embedded-fonts")]
    return typst_pack::font::embedded().take(1).collect();
    #[cfg(not(feature = "embedded-fonts"))]
    Vec::new()
}
