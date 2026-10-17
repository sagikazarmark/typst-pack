//! Pack and compilation identity vectors over the documented preimages, and
//! what each identity binds and excludes.

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
            (entry.name().to_owned(), data)
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

#[test]
fn pack_identity_binds_every_project_and_package_semantic() {
    #[derive(Clone)]
    struct PackCase<'a> {
        entrypoint: &'a str,
        project_path: &'a str,
        project_bytes: &'a [u8],
        package: typst::syntax::package::PackageSpec,
        package_path: &'a str,
        package_bytes: &'a [u8],
        disposition: bool,
        extra_package_file: bool,
    }

    let package: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let other_package: typst::syntax::package::PackageSpec = "@local/other:1.0.0".parse().unwrap();
    let baseline = PackCase {
        entrypoint: "main.typ",
        project_path: "other.typ",
        project_bytes: b"other",
        package,
        package_path: "lib.typ",
        package_bytes: b"package",
        disposition: true,
        extra_package_file: false,
    };
    let build = |case: PackCase<'_>| {
        let mut builder = Pack::builder(case.entrypoint)
            .file("main.typ", b"main".to_vec())
            .file(case.project_path, case.project_bytes.to_vec());
        builder = if case.disposition {
            builder.package_file(
                case.package.clone(),
                case.package_path,
                case.package_bytes.to_vec(),
            )
        } else {
            builder.external_package_file(
                case.package.clone(),
                case.package_path,
                case.package_bytes.to_vec(),
            )
        };
        if case.extra_package_file {
            builder = if case.disposition {
                builder.package_file(case.package, "extra.typ", b"extra".to_vec())
            } else {
                builder.external_package_file(case.package, "extra.typ", b"extra".to_vec())
            };
        }
        builder.build().unwrap()
    };
    let baseline_identity = build(baseline.clone()).identity();

    let mutations = [
        PackCase {
            entrypoint: "other.typ",
            ..baseline.clone()
        },
        PackCase {
            project_path: "renamed.typ",
            ..baseline.clone()
        },
        PackCase {
            project_bytes: b"changed",
            ..baseline.clone()
        },
        PackCase {
            package: other_package,
            ..baseline.clone()
        },
        PackCase {
            package_path: "renamed.typ",
            ..baseline.clone()
        },
        PackCase {
            package_bytes: b"changed",
            ..baseline.clone()
        },
        PackCase {
            extra_package_file: true,
            ..baseline.clone()
        },
        PackCase {
            disposition: false,
            ..baseline
        },
    ];

    for mutation in mutations {
        assert_ne!(build(mutation).identity(), baseline_identity);
    }
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
    request.options = compile::Options {
        inputs: vec![
            ("a".into(), Input::from("x")),
            ("b".into(), Input::Bytes(b"y".to_vec())),
            ("a".into(), Input::from("z")),
        ],
        document_time: DocumentTime::UnixTimestamp(946_684_800),
        features: vec![typst::Feature::A11yExtras],
    };

    let expected = Preimage::new("typst-pack/compilation/v1")
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
        .digest(sha256(b"y"))
        .str("unix")
        .int(946_684_800)
        .count(1)
        .str("a11y-extras")
        // External packages by specification, then external fonts.
        .count(1)
        .str("@local/example:1.0.0")
        .digest(*tree.digest().as_bytes())
        .count(0)
        .finish();
    assert_eq!(compile(&request).unwrap().identity.to_string(), expected);
}

#[test]
fn compilation_identity_binds_every_effective_output_control() {
    let identity = |output| {
        compile(&compile::Request::new(identity_pack(), output))
            .unwrap()
            .identity
    };
    let distinct = |identities: Vec<Digest>| {
        let count = identities.len();
        let unique = identities
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), count);
    };

    distinct(vec![
        identity(Output::new(OutputFormat::Pdf)),
        identity(Output {
            pages: "1".parse::<compile::PageSelection>().unwrap(),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_standards: vec![typst_pdf::PdfStandard::V_1_7],
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_identifier: Smart::Custom("identity".to_owned()),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_creator: Smart::Custom(Some("identity".to_owned())),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_creator: Smart::Custom(None),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_tags: Smart::Custom(false),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_timestamp: CreationTimestamp::Utc(1_700_000_000),
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pdf_timestamp: CreationTimestamp::Local {
                unix: 1_700_000_000,
                offset_minutes: 60,
            },
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output {
            pretty: true,
            ..Output::new(OutputFormat::Pdf)
        }),
        identity(Output::new(OutputFormat::Png)),
        identity(Output {
            pages: "1".parse::<compile::PageSelection>().unwrap(),
            ..Output::new(OutputFormat::Png)
        }),
        identity(Output {
            ppi: Some(216.0),
            ..Output::new(OutputFormat::Png)
        }),
        identity(Output {
            render_bleed: true,
            ..Output::new(OutputFormat::Png)
        }),
        identity(Output::new(OutputFormat::Svg)),
        identity(Output {
            render_bleed: true,
            ..Output::new(OutputFormat::Svg)
        }),
        identity(Output::new(OutputFormat::Html)),
        identity(Output {
            pretty: true,
            ..Output::new(OutputFormat::Html)
        }),
    ]);

    // Defaults and their explicit values are one request.
    assert_eq!(
        identity(Output::new(OutputFormat::Png)),
        identity(Output {
            ppi: Some(144.0),
            ..Output::new(OutputFormat::Png)
        })
    );
    assert_eq!(
        identity(Output::new(OutputFormat::Pdf)),
        identity(Output {
            pdf_tags: Smart::Custom(true),
            ..Output::new(OutputFormat::Pdf)
        })
    );
}

#[test]
fn compilation_identity_binds_pack_inputs_features_and_document_time() {
    let identity = |pack: Pack, options: compile::Options| {
        let mut request = compile::Request::new(pack, Output::new(OutputFormat::Svg));
        request.options = options;
        compile(&request).unwrap().identity
    };
    let baseline = identity(identity_pack(), compile::Options::default());
    let input = |value: Input| compile::Options {
        inputs: vec![("unused".into(), value)],
        ..compile::Options::default()
    };
    let changed = Pack::builder("main.typ")
        .file("main.typ", b"changed pack".to_vec())
        .build()
        .unwrap();

    let identities = vec![
        baseline,
        identity(changed, compile::Options::default()),
        identity(identity_pack(), input(Input::from("value"))),
        identity(identity_pack(), input(Input::Bytes(b"value".to_vec()))),
        identity(identity_pack(), input(Input::Bytes(b"other".to_vec()))),
        identity(
            identity_pack(),
            compile::Options {
                features: vec![typst::Feature::A11yExtras],
                ..compile::Options::default()
            },
        ),
        identity(
            identity_pack(),
            compile::Options {
                document_time: DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap()),
                ..compile::Options::default()
            },
        ),
        identity(
            identity_pack(),
            compile::Options {
                document_time: DocumentTime::UnixTimestamp(1_700_000_000),
                ..compile::Options::default()
            },
        ),
    ];
    let count = identities.len();
    let unique = identities
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), count);

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
    assert_eq!(identity(described, compile::Options::default()), baseline);
}
