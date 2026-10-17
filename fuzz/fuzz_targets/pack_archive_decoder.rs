#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::archive::{DecodeLimits, decode, encode};

fuzz_target!(|data: &[u8]| {
    // Every accepted archive re-encodes into an equal Pack.
    if let Ok(pack) = decode(data, DecodeLimits::default()) {
        let archive = encode(&pack);
        let reread = decode(&archive, DecodeLimits::default()).expect("re-encoding is decodable");
        assert_eq!(reread.identity(), pack.identity());
    }
});
