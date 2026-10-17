use std::io::{self, Read};

#[cfg(feature = "fs")]
use std::path::{Path, PathBuf};

use super::{DecodeError, DecodeLimits, decode};
use crate::{LimitExceeded, Pack};

/// A failure while reading archive bytes from a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    #[error(transparent)]
    Limit(#[from] LimitExceeded),
    #[error("failed to read pack archive bytes: {0}")]
    Io(#[from] io::Error),
}

/// Reads at most `max_bytes` archive bytes from a stream.
pub fn read(reader: impl Read, max_bytes: u64) -> Result<Vec<u8>, ReadError> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    LimitExceeded::check(bytes.len() as u64, max_bytes, "archive bytes")?;
    Ok(bytes)
}

/// A failure while reading and decoding a pack.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadPackError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

/// Reads and decodes one pack from a stream, bounded by
/// [`DecodeLimits::max_archive_bytes`].
pub fn read_pack(reader: impl Read, limits: DecodeLimits) -> Result<Pack, ReadPackError> {
    let archive = read(reader, limits.max_archive_bytes)?;
    Ok(decode(&archive, limits)?)
}

/// A failure while opening a pack file.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[error("cannot open pack `{}`: {source}", path.display())]
pub struct OpenPackError {
    pub path: PathBuf,
    #[source]
    pub source: ReadPackError,
}

/// Reads and decodes one pack file, refusing a file larger than
/// [`DecodeLimits::max_archive_bytes`] before reading it.
#[cfg(feature = "fs")]
pub fn open_pack(path: impl AsRef<Path>, limits: DecodeLimits) -> Result<Pack, OpenPackError> {
    let path = path.as_ref();
    let open = || -> Result<Pack, ReadPackError> {
        let file = std::fs::File::open(path).map_err(ReadError::Io)?;
        let size = file.metadata().map_err(ReadError::Io)?.len();
        LimitExceeded::check(size, limits.max_archive_bytes, "archive bytes")
            .map_err(ReadError::Limit)?;
        read_pack(file, limits)
    };
    open().map_err(|source| OpenPackError {
        path: path.to_owned(),
        source,
    })
}
