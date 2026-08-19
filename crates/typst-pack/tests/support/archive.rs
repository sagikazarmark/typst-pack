use typst_pack::pack_archive::{
    DecodeError, DecodeLimits, EncodeError, EncodeLimits, decode, encode_with_limits,
};
use typst_pack::{Pack, PackArchiveBytes};

pub fn decode_reference(bytes: impl Into<PackArchiveBytes>) -> Result<Pack, DecodeError> {
    let archive = bytes.into();
    decode(&archive, DecodeLimits::reference_v1())
}

pub fn encode_reference(pack: &Pack) -> Result<PackArchiveBytes, EncodeError> {
    encode_with_limits(pack, EncodeLimits::reference_v1())
}
