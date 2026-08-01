#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{
    FontCatalog, FontCatalogEntry, FontContainer, FontContainerIdentity, FontDisposition,
};

fuzz_target!(|data: &[u8]| {
    let Ok(container) = FontContainer::new(data.to_vec()) else {
        return;
    };

    let faces = container.faces();
    assert!(!faces.is_empty());
    assert_eq!(
        container.identity(),
        FontContainerIdentity::from_bytes(data)
    );
    assert!(faces.iter().all(|face| {
        face.identity().container() == container.identity() && face.data() == data
    }));
    assert!(
        faces
            .windows(2)
            .all(|pair| pair[0].identity().index() < pair[1].identity().index())
    );
    let face_count = faces.len();

    let catalog = FontCatalog::from_iter([
        FontCatalogEntry::new(container.clone(), FontDisposition::External),
        FontCatalogEntry::new(container, FontDisposition::Embedded),
    ]);
    let catalog_faces = catalog.faces();
    assert_eq!(catalog_faces.len(), face_count * 2);
    assert!(
        catalog_faces[..face_count]
            .iter()
            .all(|face| face.disposition() == FontDisposition::External)
    );
    assert!(
        catalog_faces[face_count..]
            .iter()
            .all(|face| face.disposition() == FontDisposition::Embedded)
    );
});
