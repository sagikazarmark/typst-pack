//! Real font container bytes, for the suites whose fixtures need them.
//!
//! Typst only ships font bytes with the `embedded-fonts` feature, so every
//! helper here starts from a container Typst embeds and derives what a fixture
//! needs from it. Each suite uses its own subset, hence the allowance below.
#![allow(dead_code)]

use typst_pack::FontDisposition;

/// The exact bytes of the first Font Container Typst ships.
pub fn typst_container() -> Vec<u8> {
    typst_pack::typst_embedded_font_containers(FontDisposition::External)
        .next()
        .expect("Typst ships an embedded font container")
        .data()
        .to_vec()
}

/// The family the container's face at index zero offers.
pub fn family_of(font: &[u8]) -> String {
    typst::text::FontInfo::new(font, 0)
        .expect("the container holds a face")
        .family
}

/// The same container bytes under a different family name, so that one
/// container can offer a family no other container in a catalog offers.
///
/// The replacement has the family's exact length, so every name record keeps
/// its offset and only the stored characters change.
pub fn renamed_family(font: &[u8], from: &str, to: &str) -> Vec<u8> {
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

/// One multi-face Font Container holding every given font, so that
/// container-local face order is observable.
pub fn font_collection(faces: &[Vec<u8>]) -> Vec<u8> {
    /// The same font with every table offset shifted to where the face starts
    /// inside the collection.
    fn adjusted(font: &[u8], base: usize) -> Vec<u8> {
        let mut adjusted = font.to_vec();
        let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        for table in 0..table_count {
            let offset = 12 + table * 16 + 8;
            let original = u32::from_be_bytes(font[offset..offset + 4].try_into().unwrap());
            let shifted = original + u32::try_from(base).unwrap();
            adjusted[offset..offset + 4].copy_from_slice(&shifted.to_be_bytes());
        }
        adjusted
    }

    // The tag, the version, the face count, and one offset per face.
    let mut offsets = Vec::with_capacity(faces.len());
    let mut next = 12 + 4 * faces.len();
    for face in faces {
        next = (next + 3) & !3;
        offsets.push(next);
        next += face.len();
    }

    let mut collection = Vec::with_capacity(next);
    collection.extend_from_slice(b"ttcf");
    collection.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    collection.extend_from_slice(&u32::try_from(faces.len()).unwrap().to_be_bytes());
    for offset in &offsets {
        collection.extend_from_slice(&u32::try_from(*offset).unwrap().to_be_bytes());
    }
    for (face, offset) in faces.iter().zip(&offsets) {
        collection.resize(*offset, 0);
        collection.extend_from_slice(&adjusted(face, *offset));
    }
    collection
}
