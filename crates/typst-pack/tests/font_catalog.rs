//! The ordered candidate font catalog Pack Creation selects faces from.
//!
//! The catalog itself is core: its tests run on a build with no crate feature
//! enabled, except the ones that need real font bytes, which Typst only ships
//! with the `embedded-fonts` feature. The filesystem section covers the
//! catalog the reference Creation Adapter composes.

use typst_pack::{CandidateFontCatalog, CandidateFontContainer, FontDisposition};

#[test]
fn an_empty_catalog_offers_no_faces() {
    let catalog = CandidateFontCatalog::new();

    assert!(catalog.containers().is_empty());
    assert!(catalog.faces().is_empty());
}

#[test]
fn a_container_that_holds_no_face_contributes_nothing() {
    let mut catalog = CandidateFontCatalog::new();
    catalog.push(CandidateFontContainer::embedded(b"not a font".to_vec()));

    assert_eq!(catalog.containers().len(), 1);
    assert!(catalog.faces().is_empty());
}

#[test]
fn a_disposition_names_whether_container_bytes_travel_in_the_pack() {
    assert!(FontDisposition::Embedded.is_embedded());
    assert!(!FontDisposition::External.is_embedded());
}

#[cfg(feature = "embedded-fonts")]
mod containers {
    use typst_pack::{
        CandidateFontCatalog, CandidateFontContainer, FontContainerIdentity, FontDisposition,
        typst_embedded_font_containers,
    };

    use crate::{two_face_collection, typst_font_container};

    #[test]
    fn faces_are_expanded_in_container_local_index_order() {
        let collection = two_face_collection(&typst_font_container());
        let mut catalog = CandidateFontCatalog::new();
        catalog.push(CandidateFontContainer::embedded(collection.clone()));

        let faces = catalog.faces();
        assert_eq!(
            faces
                .iter()
                .map(|face| face.identity().index())
                .collect::<Vec<_>>(),
            [0, 1]
        );
        let container = FontContainerIdentity::from_bytes(&collection);
        assert!(
            faces
                .iter()
                .all(|face| face.identity().container() == container)
        );
    }

    #[test]
    fn catalog_order_is_the_order_the_caller_chose() {
        let first = typst_font_container();
        let second = two_face_collection(&first);
        let mut catalog = CandidateFontCatalog::new();
        catalog.push(CandidateFontContainer::external(second.clone()));
        catalog.push(CandidateFontContainer::embedded(first.clone()));

        assert_eq!(
            catalog
                .faces()
                .iter()
                .map(|face| (face.identity().container(), face.identity().index()))
                .collect::<Vec<_>>(),
            [
                (FontContainerIdentity::from_bytes(&second), 0),
                (FontContainerIdentity::from_bytes(&second), 1),
                (FontContainerIdentity::from_bytes(&first), 0),
            ]
        );
    }

    #[test]
    fn disposition_travels_per_container() {
        let embedded = typst_font_container();
        let external = two_face_collection(&embedded);
        let catalog = CandidateFontCatalog::from_iter([
            CandidateFontContainer::embedded(embedded),
            CandidateFontContainer::external(external),
        ]);

        assert_eq!(
            catalog
                .faces()
                .iter()
                .map(|face| face.disposition())
                .collect::<Vec<_>>(),
            [
                FontDisposition::Embedded,
                FontDisposition::External,
                FontDisposition::External,
            ]
        );
    }

    #[test]
    fn identical_bytes_do_not_decide_disposition() {
        let data = typst_font_container();
        let catalog = CandidateFontCatalog::from_iter([
            CandidateFontContainer::external(data.clone()),
            CandidateFontContainer::embedded(data.clone()),
        ]);

        let faces = catalog.faces();
        assert_eq!(faces[0].identity(), faces[1].identity());
        assert_eq!(faces[0].disposition(), FontDisposition::External);
        assert_eq!(faces[1].disposition(), FontDisposition::Embedded);
    }

    #[test]
    fn typst_embedded_fonts_are_containers_the_caller_places_itself() {
        let containers =
            typst_embedded_font_containers(FontDisposition::External).collect::<Vec<_>>();
        assert!(!containers.is_empty());
        assert!(
            containers
                .iter()
                .all(|container| container.disposition() == FontDisposition::External)
        );

        let mut catalog = CandidateFontCatalog::new();
        catalog.push(CandidateFontContainer::embedded(typst_font_container()));
        catalog.extend(typst_embedded_font_containers(FontDisposition::External));

        let faces = catalog.faces();
        assert_eq!(faces[0].disposition(), FontDisposition::Embedded);
        assert!(
            faces[1..]
                .iter()
                .all(|face| face.disposition() == FontDisposition::External)
        );
    }
}

/// The candidate font catalog the filesystem Creation Adapter composes from
/// system fonts, Typst's embedded fonts, and scanned font directories.
#[cfg(all(feature = "fs", feature = "embedded-fonts"))]
mod filesystem {
    use std::fs;
    use std::path::{Path, PathBuf};

    use typst_pack::{FontContainerIdentity, Packer};

    use crate::{renamed_family, typst_font_container};

    /// Writes `data` as the only font file of a fresh directory.
    fn font_directory(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let directory = dir.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("font.ttf"), data).unwrap();
        directory
    }

    /// Writes a project whose text selects one exact family.
    fn project_using_family(dir: &Path, family: &str) -> PathBuf {
        let project = dir.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("#set text(font: \"{family}\")\nSelected"),
        )
        .unwrap();
        project
    }

    fn family_of(font: &[u8]) -> String {
        typst::text::FontInfo::new(font, 0).unwrap().family
    }

    #[test]
    fn catalog_order_decides_which_container_offers_a_family() {
        let dir = tempfile::tempdir().unwrap();
        let first = typst_font_container();
        // Same family and variant, distinct bytes: only catalog order can
        // decide between them.
        let mut second = first.clone();
        second.push(0);
        let project = project_using_family(dir.path(), &family_of(&first));
        let first_directory = font_directory(dir.path(), "first", &first);
        let second_directory = font_directory(dir.path(), "second", &second);

        let selected = |directories: [&Path; 2]| {
            let mut packer = Packer::new(&project, "main.typ")
                .system_fonts(false)
                .typst_embedded_fonts(false);
            for directory in directories {
                packer = packer.font_path(directory);
            }
            let outcome = packer.pack().unwrap();
            let requirements = outcome.pack.font_requirements();
            assert_eq!(requirements.len(), 1);
            requirements[0].container_identity()
        };

        assert_eq!(
            selected([&first_directory, &second_directory]),
            FontContainerIdentity::from_bytes(&first)
        );
        assert_eq!(
            selected([&second_directory, &first_directory]),
            FontContainerIdentity::from_bytes(&second)
        );
    }

    #[test]
    fn one_catalog_produces_a_pack_with_mixed_font_dispositions() {
        let dir = tempfile::tempdir().unwrap();
        let typst_font = typst_font_container();
        let typst_family = family_of(&typst_font);
        let scanned_family = format!("Z{}", &typst_family[1..]);
        let scanned_font = renamed_family(&typst_font, &typst_family, &scanned_family);
        let fonts = font_directory(dir.path(), "fonts", &scanned_font);
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("Typst's own\n\n#text(font: \"{scanned_family}\")[Scanned]"),
        )
        .unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .font_path(&fonts)
            .embed_fonts(true)
            .pack()
            .unwrap();

        let requirements = outcome.pack.font_requirements();
        let embedded = requirements
            .iter()
            .filter(|requirement| requirement.is_embedded())
            .collect::<Vec<_>>();
        let external = requirements
            .iter()
            .filter(|requirement| !requirement.is_embedded())
            .collect::<Vec<_>>();
        assert_eq!(embedded.len(), 1, "the scanned container is embedded");
        assert_eq!(
            embedded[0].container_identity(),
            FontContainerIdentity::from_bytes(&scanned_font)
        );
        assert_eq!(external.len(), 1, "Typst's own container stays external");
        assert_eq!(
            external[0].container_identity(),
            FontContainerIdentity::from_bytes(&typst_font)
        );

        // The Pack Font Catalog keeps the candidate catalog's relative order:
        // Typst's containers precede the scanned directory.
        assert_eq!(
            outcome
                .pack
                .font_catalog()
                .iter()
                .map(|face| face.identity().container())
                .collect::<Vec<_>>(),
            [
                FontContainerIdentity::from_bytes(&typst_font),
                FontContainerIdentity::from_bytes(&scanned_font),
            ]
        );
    }

    #[test]
    fn font_embedding_does_not_infer_disposition_from_container_bytes() {
        let dir = tempfile::tempdir().unwrap();
        // A scanned container whose bytes are exactly one Typst ships. Its
        // disposition comes from the catalog position it was supplied at, not
        // from a comparison against Typst's own containers.
        let data = typst_font_container();
        let project = project_using_family(dir.path(), &family_of(&data));
        let fonts = font_directory(dir.path(), "fonts", &data);

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .typst_embedded_fonts(false)
            .font_path(&fonts)
            .embed_fonts(true)
            .pack()
            .unwrap();

        let requirements = outcome.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(
            requirements[0].container_identity(),
            FontContainerIdentity::from_bytes(&data)
        );
        assert!(requirements[0].is_embedded());
    }
}

/// The exact bytes of the first Font Container Typst ships.
#[cfg(feature = "embedded-fonts")]
fn typst_font_container() -> Vec<u8> {
    typst_kit::fonts::embedded()
        .next()
        .expect("Typst embedded fonts are available")
        .0
        .data()
        .to_vec()
}

/// A two-face collection holding `font` twice, so that container-local face
/// order is observable.
#[cfg(feature = "embedded-fonts")]
fn two_face_collection(font: &[u8]) -> Vec<u8> {
    fn adjusted_font(font: &[u8], base: usize) -> Vec<u8> {
        let mut adjusted = font.to_vec();
        let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        for table in 0..table_count {
            let offset = 12 + table * 16 + 8;
            let original = u32::from_be_bytes(font[offset..offset + 4].try_into().unwrap());
            let adjusted_offset = original + u32::try_from(base).unwrap();
            adjusted[offset..offset + 4].copy_from_slice(&adjusted_offset.to_be_bytes());
        }
        adjusted
    }

    let first_offset = 20;
    let second_offset = (first_offset + font.len() + 3) & !3;
    let mut collection = Vec::with_capacity(second_offset + font.len());
    collection.extend_from_slice(b"ttcf");
    collection.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    collection.extend_from_slice(&2u32.to_be_bytes());
    collection.extend_from_slice(&u32::try_from(first_offset).unwrap().to_be_bytes());
    collection.extend_from_slice(&u32::try_from(second_offset).unwrap().to_be_bytes());
    collection.extend_from_slice(&adjusted_font(font, first_offset));
    collection.resize(second_offset, 0);
    collection.extend_from_slice(&adjusted_font(font, second_offset));
    collection
}

/// The same container bytes under a different family name, so that a candidate
/// container can offer a family no other container in the catalog offers.
///
/// The replacement has the family's exact length, so every name record keeps
/// its offset and only the stored characters change.
#[cfg(all(feature = "fs", feature = "embedded-fonts"))]
fn renamed_family(font: &[u8], from: &str, to: &str) -> Vec<u8> {
    assert_eq!(from.len(), to.len(), "the family name must keep its length");
    let mut data = font.to_vec();
    let wide = |name: &str| -> Vec<u8> {
        name.encode_utf16()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    };
    for (needle, replacement) in [
        (from.as_bytes().to_vec(), to.as_bytes().to_vec()),
        (wide(from), wide(to)),
    ] {
        let mut offset = 0;
        while offset + needle.len() <= data.len() {
            if data[offset..offset + needle.len()] == needle[..] {
                data[offset..offset + needle.len()].copy_from_slice(&replacement);
                offset += needle.len();
            } else {
                offset += 1;
            }
        }
    }
    data
}
