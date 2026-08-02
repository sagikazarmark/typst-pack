use typst_pack::pack_archive::{DecodeError, DecodeLimits, decode};
use typst_pack::{Pack, PackArchiveBytes};

pub fn decode_reference(bytes: impl Into<PackArchiveBytes>) -> Result<Pack, DecodeError> {
    let archive = bytes.into();
    decode(&archive, DecodeLimits::reference_v1())
}
