#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::pack_archive::{DecodeLimits, decode, encode};

fuzz_target!(|data: &[u8]| {
    // Every accepted archive re-encodes into an equal Pack.
    if let Ok(pack) = decode(data, DecodeLimits::DEFAULT) {
        let archive = encode(&pack).expect("a decoded Pack is representable");
        let reread = decode(&archive, DecodeLimits::DEFAULT).expect("re-encoding is decodable");
        assert_eq!(reread.identity(), pack.identity());
    }
});
