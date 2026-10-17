use typst_pack::Pack;
use typst_pack::pack_archive::{DecodeError, DecodeLimits, EncodeError, decode, encode};

pub fn decode_reference(archive: Vec<u8>) -> Result<Pack, DecodeError> {
    decode(&archive, DecodeLimits::DEFAULT)
}

pub fn encode_reference(pack: &Pack) -> Result<Vec<u8>, EncodeError> {
    encode(pack)
}
