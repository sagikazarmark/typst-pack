#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{Digest, Pack, font};

fuzz_target!(|data: &[u8]| {
    let Some(container) = font::Container::new(data.to_vec()) else {
        return;
    };

    let faces = container.faces();
    assert!(!faces.is_empty());
    assert_eq!(container.digest(), Digest::of(data));
    assert!(faces.windows(2).all(|pair| pair[0].index < pair[1].index));

    // A readable container is accepted by a Pack with every face listed.
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .font(data.to_vec())
        .build()
        .expect("a readable container is a valid embedded font");
    assert_eq!(pack.font_requirements()[0].faces, faces);
});
