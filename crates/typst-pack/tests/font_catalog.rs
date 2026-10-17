//! Font containers and the order creation selects them in.
//!
//! The tests that need real font bytes need the `embedded-fonts` feature,
//! because Typst only ships font bytes with it. The filesystem section covers
//! the fonts the filesystem packer offers.

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use typst_pack::font;

#[test]
fn bytes_that_hold_no_readable_face_are_no_container() {
    assert!(font::Container::new(b"not a font".to_vec()).is_none());
}

#[cfg(feature = "embedded-fonts")]
mod containers {
    use typst_pack::compile::{Input, Output, OutputFormat};
    use typst_pack::{Digest, ProjectSnapshot, compile, create, font};

    use crate::font_bytes::{font_collection, typst_container};

    /// A two-face collection holding one container's face twice, so that
    /// container-local face order is observable.
    fn two_face_collection(font: &[u8]) -> Vec<u8> {
        font_collection(&[font.to_vec(), font.to_vec()])
    }

    #[test]
    fn standalone_container_digest_depends_only_on_exact_bytes() {
        let data = typst_container();
        let expected = Digest::of(&data);

        let first = font::Container::new(data.clone()).unwrap();
        let second = font::Container::new(data).unwrap();

        assert_eq!(first.digest(), expected);
        assert_eq!(second.digest(), expected);
        assert_eq!(first.faces().len(), 1);
        assert_eq!(first.faces()[0].index, 0);
    }

    #[test]
    fn faces_are_listed_in_container_local_index_order() {
        let collection = two_face_collection(&typst_container());
        let container = font::Container::new(collection).unwrap();

        assert_eq!(
            container
                .faces()
                .iter()
                .map(|face| face.index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    /// The exact bytes of the container Typst ships one family and weight in.
    fn typst_face(family: &str, weight: u16) -> Vec<u8> {
        typst_kit::fonts::embedded()
            .find(|(font, _)| {
                font.info().family == family
                    && font.info().variant.weight.to_number() == weight
                    && font.info().variant.style == typst::text::FontStyle::Normal
            })
            .map(|(font, _)| font.data().to_vec())
            .unwrap_or_else(|| panic!("Typst ships `{family}` at weight {weight}"))
    }

    /// Discovery that rendered only the regular face must not hide the other
    /// faces of the container from a later compile.
    #[test]
    fn a_compile_can_use_faces_discovery_did_not_render() {
        let collection = font_collection(&[
            typst_face("Libertinus Serif", 400),
            typst_face("Libertinus Serif", 700),
        ]);
        let source = "#set text(font: \"Libertinus Serif\")\n\
                      #if sys.inputs.at(\"bold\", default: \"no\") == \"yes\" [*Text*] else [Text]";
        let project = ProjectSnapshot::new("main.typ", [("main.typ", source)]).unwrap();
        let mut request = create::Request::new(project);
        request.fonts = Vec::from([font::Entry {
            container: font::Container::new(collection.clone()).unwrap(),
            embedded: true,
        }]);
        let create::Outcome::Created(create::Created { pack, .. }) = create(&request).unwrap()
        else {
            panic!("creation needs no packages");
        };
        assert_eq!(pack.font_requirements().len(), 1);
        assert_eq!(pack.font_requirements()[0].digest, Digest::of(&collection));
        assert_eq!(pack.font_requirements()[0].faces.len(), 2);

        // Typst does not synthesize bold, so different glyphs prove the bold
        // face of the container was available.
        let svg = |bold: &str| {
            let mut request = compile::Request::new(pack.clone(), Output::new(OutputFormat::Svg));
            request.options.inputs = vec![("bold".into(), Input::from(bold))];
            let compiled = compile(&request).unwrap();
            assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
            compiled.artifacts[0].bytes.clone()
        };
        assert_ne!(svg("yes"), svg("no"));
    }
}

/// The Font Catalog the filesystem Pack Assembler composes from
/// system fonts, Typst's embedded fonts, and scanned font directories.
#[cfg(all(feature = "fs", feature = "embedded-fonts"))]
mod filesystem {
    use std::path::{Path, PathBuf};

    use typst_pack::{Digest, fs};

    use crate::font_bytes::{family_of, renamed_family, typst_container};

    /// Writes `data` as the only font file of a fresh directory.
    fn font_directory(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let directory = dir.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("font.ttf"), data).unwrap();
        directory
    }

    /// Writes a project whose text selects one exact family.
    fn project_using_family(dir: &Path, family: &str) -> PathBuf {
        let project = dir.join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("main.typ"),
            format!("#set text(font: \"{family}\")\nSelected"),
        )
        .unwrap();
        project
    }

    #[test]
    fn catalog_order_decides_which_container_offers_a_family() {
        let dir = tempfile::tempdir().unwrap();
        let first = typst_container();
        // Same family and variant, distinct bytes: only catalog order can
        // decide between them.
        let mut second = first.clone();
        second.push(0);
        let project = project_using_family(dir.path(), &family_of(&first));
        let first_directory = font_directory(dir.path(), "first", &first);
        let second_directory = font_directory(dir.path(), "second", &second);

        let selected = |directories: [&Path; 2]| {
            let mut packer = fs::Packer {
                system_fonts: false,
                typst_embedded_fonts: false,
                ..fs::Packer::default()
            };
            for directory in directories {
                packer.font_paths.push(directory.to_path_buf());
            }
            let packed = packer.pack(&project, "main.typ").unwrap();
            let requirements = packed.pack.font_requirements();
            assert_eq!(requirements.len(), 1);
            requirements[0].digest
        };

        assert_eq!(
            selected([&first_directory, &second_directory]),
            Digest::of(&first)
        );
        assert_eq!(
            selected([&second_directory, &first_directory]),
            Digest::of(&second)
        );
    }

    #[test]
    fn one_catalog_produces_a_pack_with_mixed_font_dispositions() {
        let dir = tempfile::tempdir().unwrap();
        let typst_font = typst_container();
        let typst_family = family_of(&typst_font);
        let scanned_family = format!("Z{}", &typst_family[1..]);
        let scanned_font = renamed_family(&typst_font, &typst_family, &scanned_family);
        let fonts = font_directory(dir.path(), "fonts", &scanned_font);
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("main.typ"),
            format!("Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]"),
        )
        .unwrap();

        let packed = fs::Packer {
            font_paths: vec![fonts.to_path_buf()],
            system_fonts: false,
            embed_fonts: true,
            ..fs::Packer::default()
        }
        .pack(&project, "main.typ")
        .unwrap();

        let requirements = packed.pack.font_requirements();
        let embedded = requirements
            .iter()
            .filter(|requirement| requirement.embedded)
            .collect::<Vec<_>>();
        let external = requirements
            .iter()
            .filter(|requirement| !requirement.embedded)
            .collect::<Vec<_>>();
        assert_eq!(embedded.len(), 1, "the scanned container is embedded");
        assert_eq!(embedded[0].digest, Digest::of(&scanned_font));
        assert_eq!(external.len(), 1, "Typst's own container stays external");
        assert_eq!(external[0].digest, Digest::of(&typst_font));

        // Font requirements keep the candidate catalog's relative order:
        // Typst's containers precede the scanned directory.
        assert_eq!(
            requirements
                .iter()
                .map(|requirement| requirement.digest)
                .collect::<Vec<_>>(),
            [Digest::of(&typst_font), Digest::of(&scanned_font),]
        );
    }

    #[test]
    fn font_embedding_does_not_infer_disposition_from_container_bytes() {
        let dir = tempfile::tempdir().unwrap();
        // A scanned container whose bytes are exactly one Typst ships. Its
        // disposition comes from the catalog position it was supplied at, not
        // from a comparison against Typst's own containers.
        let data = typst_container();
        let project = project_using_family(dir.path(), &family_of(&data));
        let fonts = font_directory(dir.path(), "fonts", &data);

        let packed = fs::Packer {
            font_paths: vec![fonts.to_path_buf()],
            system_fonts: false,
            typst_embedded_fonts: false,
            embed_fonts: true,
            ..fs::Packer::default()
        }
        .pack(&project, "main.typ")
        .unwrap();

        let requirements = packed.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].digest, Digest::of(&data));
        assert!(requirements[0].embedded);
    }
}
