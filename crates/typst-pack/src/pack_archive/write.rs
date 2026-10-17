use std::io::{self, Write};

#[cfg(feature = "fs")]
use std::path::Path;

use super::{EncodeError, EncodeLimits, encode_with_limits};
use crate::{CommitCertainty, Pack, PackArchiveBytes};
#[cfg(feature = "fs")]
use crate::{FilesystemWriteError, WritePolicy};

/// The stream write phase reached by an attempt.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StreamWritePhase {
    Write,
    Flush,
    Complete,
}

/// Evidence from successful exact stream write.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StreamWriteReceipt {
    visible_prefix: u64,
}

impl StreamWriteReceipt {
    pub const fn visible_prefix(&self) -> u64 {
        self.visible_prefix
    }
}

/// Evidence and the concrete cause from failed exact stream write.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive stream write failed during {phase:?} after a {visible_prefix}-byte visible prefix: {source}"
)]
pub struct StreamWriteError {
    phase: StreamWritePhase,
    visible_prefix: u64,
    commit_certainty: CommitCertainty,
    #[source]
    source: io::Error,
}

impl StreamWriteError {
    pub const fn phase(&self) -> StreamWritePhase {
        self.phase
    }

    pub const fn visible_prefix(&self) -> u64 {
        self.visible_prefix
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub const fn io_error(&self) -> &io::Error {
        &self.source
    }
}

/// Writes exact Pack Archive bytes to a stream and flushes the writer.
pub fn write(
    mut writer: impl Write,
    archive: &PackArchiveBytes,
) -> Result<StreamWriteReceipt, StreamWriteError> {
    let mut visible_prefix = 0usize;
    while visible_prefix < archive.as_slice().len() {
        match writer.write(&archive.as_slice()[visible_prefix..]) {
            Ok(0) => {
                return Err(StreamWriteError {
                    phase: StreamWritePhase::Write,
                    visible_prefix: visible_prefix as u64,
                    commit_certainty: CommitCertainty::NotCommitted,
                    source: io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write the complete Pack Archive",
                    ),
                });
            }
            Ok(written) => visible_prefix += written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => {
                return Err(StreamWriteError {
                    phase: StreamWritePhase::Write,
                    visible_prefix: visible_prefix as u64,
                    commit_certainty: CommitCertainty::NotCommitted,
                    source,
                });
            }
        }
    }
    if let Err(source) = writer.flush() {
        return Err(StreamWriteError {
            phase: StreamWritePhase::Flush,
            visible_prefix: visible_prefix as u64,
            commit_certainty: CommitCertainty::Indeterminate,
            source,
        });
    }
    Ok(StreamWriteReceipt {
        visible_prefix: visible_prefix as u64,
    })
}

/// A failure in Pack Archive Encoding followed by stream write.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WritePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("encoded Pack Archive could not be written: {source}")]
    Write {
        archive: PackArchiveBytes,
        #[source]
        source: StreamWriteError,
    },
}

/// Encodes and writes one Pack while preserving exact bytes on write failure.
pub fn write_pack(writer: impl Write, pack: &Pack) -> Result<StreamWriteReceipt, WritePackError> {
    write_pack_with_limits(writer, pack, EncodeLimits::reference_v1())
}

/// Encodes under explicit resource ceilings and writes one Pack.
pub fn write_pack_with_limits(
    writer: impl Write,
    pack: &Pack,
    encode_limits: EncodeLimits,
) -> Result<StreamWriteReceipt, WritePackError> {
    let archive = encode_with_limits(pack, encode_limits)?;
    match write(writer, &archive) {
        Ok(receipt) => Ok(receipt),
        Err(source) => Err(WritePackError::Write { archive, source }),
    }
}

/// Writes exact Pack Archive bytes to a file through a temporary sibling.
///
/// See [`WritePolicy`] for how an existing file is treated.
#[cfg(feature = "fs")]
pub fn write_file(
    path: impl AsRef<Path>,
    archive: &PackArchiveBytes,
    policy: WritePolicy,
) -> Result<(), FilesystemWriteError> {
    crate::filesystem_write::write_file(path.as_ref(), archive.as_slice(), policy)
}

/// A failure in Pack Archive Encoding followed by file write.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SavePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("encoded Pack Archive could not be written: {source}")]
    Write {
        /// The encoded bytes, kept so a caller can retry without re-encoding.
        archive: PackArchiveBytes,
        #[source]
        source: FilesystemWriteError,
    },
}

/// Encodes and writes one Pack to a file.
#[cfg(feature = "fs")]
pub fn save_pack(
    path: impl AsRef<Path>,
    pack: &Pack,
    policy: WritePolicy,
) -> Result<(), SavePackError> {
    save_pack_with_limits(path, pack, EncodeLimits::reference_v1(), policy)
}

/// Encodes under explicit resource ceilings and writes one Pack to a file.
#[cfg(feature = "fs")]
pub fn save_pack_with_limits(
    path: impl AsRef<Path>,
    pack: &Pack,
    encode_limits: EncodeLimits,
    policy: WritePolicy,
) -> Result<(), SavePackError> {
    let archive = encode_with_limits(pack, encode_limits)?;
    write_file(path, &archive, policy).map_err(|source| SavePackError::Write { archive, source })
}
