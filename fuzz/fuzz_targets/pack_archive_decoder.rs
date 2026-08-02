#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::PackArchiveBytes;
use typst_pack::pack_archive::{DecodeLimits, decode};

fuzz_target!(|data: &[u8]| {
    let archive = PackArchiveBytes::from_vec(data.to_vec());
    let _ = decode(&archive, DecodeLimits::reference_v1());
});
