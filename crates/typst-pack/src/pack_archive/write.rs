use std::io::{self, Write};

#[cfg(feature = "fs")]
use std::path::Path;

use super::{EncodeError, encode};
use crate::Pack;
#[cfg(feature = "fs")]
use crate::{FilesystemWriteError, WritePolicy};

/// A failure while encoding and writing a pack to a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WritePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("failed to write pack archive bytes: {0}")]
    Io(#[from] io::Error),
}

/// Encodes one pack and writes it to a stream, then flushes the stream.
pub fn write_pack(mut writer: impl Write, pack: &Pack) -> Result<(), WritePackError> {
    writer.write_all(&encode(pack)?)?;
    writer.flush()?;
    Ok(())
}

/// Writes archive bytes to a file through a temporary sibling.
///
/// See [`WritePolicy`] for how an existing file is treated.
#[cfg(feature = "fs")]
pub fn write_file(
    path: impl AsRef<Path>,
    archive: &[u8],
    policy: WritePolicy,
) -> Result<(), FilesystemWriteError> {
    crate::filesystem_write::write_file(path.as_ref(), archive, policy)
}

/// A failure while encoding and saving a pack to a file.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SavePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error(transparent)]
    Write(#[from] FilesystemWriteError),
}

/// Encodes one pack and writes it to a file.
#[cfg(feature = "fs")]
pub fn save_pack(
    path: impl AsRef<Path>,
    pack: &Pack,
    policy: WritePolicy,
) -> Result<(), SavePackError> {
    Ok(write_file(path, &encode(pack)?, policy)?)
}
