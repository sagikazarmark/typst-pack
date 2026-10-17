//! SHA-256 content digests and the documented preimages built from them.
//!
//! See the README's "Pack format" section for the exact byte layouts.

use std::fmt;
use std::str::FromStr;

use sha2::{Digest as _, Sha256};

/// A SHA-256 digest, written as `sha256:<64 lowercase hex digits>`.
///
/// File and font container digests are the SHA-256 of the exact bytes.
/// Package tree digests and pack identities are SHA-256 of a documented,
/// domain-separated preimage (see the README's "Pack format" section).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// The SHA-256 digest of exact bytes.
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw 32 digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The 64 lowercase hexadecimal digits, without the `sha256:` prefix.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Parses 64 lowercase hexadecimal digits, without the `sha256:` prefix.
    pub(crate) fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }
}

/// Builds a documented digest preimage from length-prefixed fields.
///
/// `str` writes the UTF-8 byte length as a big-endian `u64` and then the bytes;
/// `digest` writes the 32 raw digest bytes; `count` writes a big-endian `u64`;
/// `int` writes a two's-complement big-endian `i64`; `bool` writes one byte,
/// `0` or `1`. Every preimage starts with a domain tag written by `str`.
pub(crate) struct Preimage(Sha256);

impl Preimage {
    pub(crate) fn new(domain: &str) -> Self {
        let mut preimage = Self(Sha256::new());
        preimage.str(domain);
        preimage
    }

    pub(crate) fn str(&mut self, value: &str) -> &mut Self {
        self.count(value.len());
        self.0.update(value.as_bytes());
        self
    }

    pub(crate) fn digest(&mut self, value: &Digest) -> &mut Self {
        self.0.update(value.0);
        self
    }

    pub(crate) fn count(&mut self, value: usize) -> &mut Self {
        self.0.update((value as u64).to_be_bytes());
        self
    }

    pub(crate) fn int(&mut self, value: i64) -> &mut Self {
        self.0.update(value.to_be_bytes());
        self
    }

    pub(crate) fn bool(&mut self, value: bool) -> &mut Self {
        self.0.update([u8::from(value)]);
        self
    }

    pub(crate) fn finish(&mut self) -> Digest {
        Digest(std::mem::take(&mut self.0).finalize().into())
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "sha256:{}", self.to_hex())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// A string that is not `sha256:` followed by 64 lowercase hexadecimal digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid digest {0:?}: expected `sha256:` and 64 lowercase hexadecimal digits")]
pub struct InvalidDigest(pub String);

impl FromStr for Digest {
    type Err = InvalidDigest;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .strip_prefix("sha256:")
            .and_then(Self::from_hex)
            .ok_or_else(|| InvalidDigest(value.to_owned()))
    }
}

impl serde::Serialize for Digest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_text_round_trips_and_rejects_other_forms() {
        let digest = Digest::of(b"abc");
        assert_eq!(
            digest.to_string(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest.to_string().parse::<Digest>(), Ok(digest));
        for invalid in [
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "sha256:BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
            "sha256:ba78",
            "sha1:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ] {
            assert!(invalid.parse::<Digest>().is_err(), "{invalid}");
        }
    }
}
