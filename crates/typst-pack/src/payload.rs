//! Byte ownership shared by immutable semantic values.

use std::fmt;
use std::ops::Deref;

use typst::foundations::Bytes;

/// Privately shared immutable payload bytes.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct SharedBytes(Bytes);

impl SharedBytes {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self(Bytes::new(data))
    }

    pub(crate) fn from_typst(data: Bytes) -> Self {
        Self(data)
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub(crate) fn to_typst(&self) -> Bytes {
        self.0.clone()
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.0.into_vec()
    }
}

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for SharedBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl fmt::Debug for SharedBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SharedBytes")
            .field(&self.0.len())
            .finish()
    }
}

/// Exact uniquely owned bytes of one Pack Archive.
///
/// This value is intentionally not cloneable: retries transfer the same exact
/// archive bytes rather than silently duplicating potentially large buffers.
///
/// ```compile_fail
/// use typst_pack::PackArchiveBytes;
///
/// let archive = PackArchiveBytes::from(Vec::new());
/// let duplicate = archive.clone();
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct PackArchiveBytes(Vec<u8>);

impl PackArchiveBytes {
    /// Borrows the exact archive bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Transfers the exact archive bytes back into their vector.
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for PackArchiveBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for PackArchiveBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for PackArchiveBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
