//! Pack identity vectors over the documented preimages, and the invariance
//! contracts of Pack, Compilation, and Compilation Result identities.

use proptest::prelude::*;
use sha2::{Digest as _, Sha256};
use typst::foundations::{Datetime, Dict, Smart, Value};
use typst::syntax::package::PackageSpec;
use typst_pack::pack_archive::{DecodeLimits, decode, encode};
use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationRequestRejection,
    CompilationResult, CreationTimestamp, Digest, DocumentTime, HtmlOutputSpecification, Pack,
    PackCompilationRequest, PackMetadata, PackOverrideSet, PackageDisposition, PackageTree,
    PdfOutputSpecification, PngOutputSpecification, SvgOutputSpecification,
    compile_with_limits as compile_to_report, parse_page_selection,
};

fn svg_output() -> CompilationOutputSpecification {
    CompilationOutputSpecification::Svg(SvgOutputSpecification::default())
}

fn compile(
    request: PackCompilationRequest,
) -> Result<CompilationResult, CompilationRequestRejection> {
    let report = compile_to_report(request, CompilationLimits::DEFAULT)?;
    Ok(report
        .result()
        .expect("expected a semantic Compilation Result")
        .clone())
}

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
            .metadata(PackMetadata {
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
    let tree =
        PackageTree::copy_from_entries([("lib.typ", b"lib".as_slice()), ("a.typ", b"a")]).unwrap();
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
    let data = typst_pack::typst_embedded_font_containers()
        .next()
        .unwrap()
        .data()
        .to_vec();
    let container = typst_pack::FontContainer::new(data.clone()).unwrap();
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
        .metadata(PackMetadata {
            name: Some("ordered".to_owned()),
            ..PackMetadata::default()
        })
        .build()
        .unwrap();
    let archive = encode(&pack).unwrap();

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
    let decoded = decode(&rewritten, DecodeLimits::DEFAULT).unwrap();
    assert_eq!(decoded.identity(), pack.identity());
    assert_eq!(encode(&decoded).unwrap(), archive);
}

fn manifest_of(pack: &Pack) -> String {
    let archive = encode(pack).unwrap();
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
        disposition: PackageDisposition,
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
        disposition: PackageDisposition::Embedded,
        extra_package_file: false,
    };
    let build = |case: PackCase<'_>| {
        let mut builder = Pack::builder(case.entrypoint)
            .file("main.typ", b"main".to_vec())
            .file(case.project_path, case.project_bytes.to_vec());
        builder = if case.disposition.is_embedded() {
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
            builder = if case.disposition.is_embedded() {
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
            disposition: PackageDisposition::External,
            ..baseline
        },
    ];

    for mutation in mutations {
        assert_ne!(build(mutation).identity(), baseline_identity);
    }
}

#[test]
fn compilation_identity_binds_every_effective_output_control() {
    let compile_output = |output| {
        compile(PackCompilationRequest::new(identity_pack(), output))
            .unwrap()
            .compilation_identity()
    };
    let timestamp =
        typst_pdf::Timestamp::new_utc(Datetime::from_ymd_hms(2024, 2, 3, 4, 5, 6).unwrap());

    let pdf_baseline = compile_output(CompilationOutputSpecification::Pdf(
        PdfOutputSpecification::default(),
    ));
    let png_baseline = compile_output(CompilationOutputSpecification::Png(
        PngOutputSpecification::default(),
    ));
    let svg_baseline = compile_output(svg_output());
    let html_baseline = compile_output(CompilationOutputSpecification::Html(
        HtmlOutputSpecification::default(),
    ));
    assert_eq!(
        [pdf_baseline, png_baseline, svg_baseline, html_baseline]
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );

    let pdf_mutations = [
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            standards: vec![typst_pdf::PdfStandard::A_2b],
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            identifier: Smart::Custom("identity".to_owned()),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            creator: Smart::Custom(Some("identity".to_owned())),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            tags: Smart::Custom(false),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            creation_timestamp: CreationTimestamp::Explicit(timestamp),
            ..PdfOutputSpecification::default()
        }),
        CompilationOutputSpecification::Pdf(PdfOutputSpecification {
            pretty: true,
            ..PdfOutputSpecification::default()
        }),
    ];
    for output in pdf_mutations {
        assert_ne!(compile_output(output), pdf_baseline);
    }

    let png_mutations = [
        CompilationOutputSpecification::Png(PngOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..PngOutputSpecification::default()
        }),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(216.0),
            ..PngOutputSpecification::default()
        }),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            render_bleed: true,
            ..PngOutputSpecification::default()
        }),
    ];
    for output in png_mutations {
        assert_ne!(compile_output(output), png_baseline);
    }

    let svg_mutations = [
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            page_selection: parse_page_selection("1").unwrap(),
            ..SvgOutputSpecification::default()
        }),
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            render_bleed: true,
            ..SvgOutputSpecification::default()
        }),
        CompilationOutputSpecification::Svg(SvgOutputSpecification {
            pretty: true,
            ..SvgOutputSpecification::default()
        }),
    ];
    for output in svg_mutations {
        assert_ne!(compile_output(output), svg_baseline);
    }

    assert_ne!(
        compile_output(CompilationOutputSpecification::Html(
            HtmlOutputSpecification { pretty: true }
        )),
        html_baseline
    );
}

#[test]
fn compilation_identity_binds_pack_inputs_overrides_features_and_document_time() {
    let baseline_pack = identity_pack();
    let baseline = compile(PackCompilationRequest::new(
        baseline_pack.clone(),
        svg_output(),
    ))
    .unwrap();
    let baseline_identity = baseline.compilation_identity();
    let mut inputs = Dict::new();
    inputs.insert("unused".into(), Value::Str("value".into()));
    let main_override = PackOverrideSet::new(&baseline_pack)
        .replace("main.typ", b"aaaaaaaa".to_vec())
        .unwrap();
    let path_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"aaaaaaaa".to_vec())
        .unwrap();
    let length_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"short".to_vec())
        .unwrap();
    let byte_override = PackOverrideSet::new(&baseline_pack)
        .replace("unused.txt", b"bbbbbbbb".to_vec())
        .unwrap();
    let changed_pack = Pack::builder("main.typ")
        .file("main.typ", b"changed pack".to_vec())
        .build()
        .unwrap();
    let main_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(main_override),
    )
    .unwrap()
    .compilation_identity();
    let path_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(path_override),
    )
    .unwrap()
    .compilation_identity();
    let length_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(length_override),
    )
    .unwrap()
    .compilation_identity();
    let byte_override = compile(
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).overrides(byte_override),
    )
    .unwrap()
    .compilation_identity();
    assert_ne!(main_override, path_override);
    assert_ne!(path_override, length_override);
    assert_ne!(path_override, byte_override);

    let requests = [
        PackCompilationRequest::new(changed_pack, svg_output()),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output()).inputs(inputs),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output())
            .feature(typst::Feature::A11yExtras),
        PackCompilationRequest::new(baseline_pack.clone(), svg_output())
            .document_time(DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap())),
        PackCompilationRequest::new(baseline_pack, svg_output())
            .document_time(DocumentTime::UnixTimestamp(1_700_000_000)),
    ];

    for request in requests {
        assert_ne!(
            compile(request).unwrap().compilation_identity(),
            baseline_identity
        );
    }
}

#[test]
fn identities_exclude_metadata_and_normalize_equivalent_request_values() {
    let metadata = PackMetadata {
        name: Some("Named Pack".to_owned()),
        description: Some("Operational description".to_owned()),
        authors: vec!["Pack Author".to_owned()],
    };
    let plain_pack = identity_pack();
    let metadata_pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 10pt, margin: 0pt)\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .file("unused.txt", b"baseline".to_vec())
        .metadata(metadata)
        .build()
        .unwrap();
    assert_eq!(plain_pack.identity(), metadata_pack.identity());

    let mut inputs = Dict::new();
    inputs.insert("unused".into(), Value::Str("value".into()));
    let time = DocumentTime::Fixed(Datetime::from_ymd(2024, 2, 3).unwrap());
    let caller = compile(
        PackCompilationRequest::new(plain_pack.clone(), svg_output())
            .inputs(inputs.clone())
            .feature(typst::Feature::A11yExtras)
            .document_time(time),
    )
    .unwrap();
    let described = compile(
        PackCompilationRequest::new(metadata_pack, svg_output())
            .inputs(inputs)
            .feature(typst::Feature::A11yExtras)
            .document_time(time),
    )
    .unwrap();
    assert_eq!(
        caller.compilation_identity(),
        described.compilation_identity()
    );
    assert_eq!(caller.result_identity(), described.result_identity());

    let default_png = compile(PackCompilationRequest::new(
        plain_pack.clone(),
        CompilationOutputSpecification::Png(PngOutputSpecification::default()),
    ))
    .unwrap();
    let explicit_png = compile(PackCompilationRequest::new(
        plain_pack.clone(),
        CompilationOutputSpecification::Png(PngOutputSpecification {
            pixels_per_inch: Some(144.0),
            ..PngOutputSpecification::default()
        }),
    ))
    .unwrap();
    assert_eq!(
        default_png.compilation_identity(),
        explicit_png.compilation_identity()
    );
    assert_eq!(
        default_png.result_identity(),
        explicit_png.result_identity()
    );

    let caller_empty_overrides = compile(
        PackCompilationRequest::new(plain_pack.clone(), svg_output())
            .overrides(PackOverrideSet::new(&plain_pack)),
    )
    .unwrap();
    let default_empty_overrides =
        compile(PackCompilationRequest::new(plain_pack, svg_output())).unwrap();
    assert_eq!(
        default_empty_overrides.compilation_identity(),
        caller_empty_overrides.compilation_identity()
    );
    assert_eq!(
        default_empty_overrides.result_identity(),
        caller_empty_overrides.result_identity()
    );
}

#[test]
fn compilation_and_result_identities_exclude_destination_facts() {
    let first = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();
    let second = compile(PackCompilationRequest::new(identity_pack(), svg_output())).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_destination = directory.path().join("first.svg");
    let second_destination = directory.path().join("nested/second.svg");
    std::fs::create_dir(second_destination.parent().unwrap()).unwrap();
    std::fs::write(&first_destination, first.artifacts()[0].bytes()).unwrap();
    std::fs::write(&second_destination, second.artifacts()[0].bytes()).unwrap();

    assert_ne!(first_destination, second_destination);
    assert_eq!(first.compilation_identity(), second.compilation_identity());
    assert_eq!(first.result_identity(), second.result_identity());
    assert_eq!(
        std::fs::read(first_destination).unwrap(),
        std::fs::read(second_destination).unwrap()
    );
}
