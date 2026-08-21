use std::io::{self, Read};

#[cfg(feature = "fs")]
use std::path::{Path, PathBuf};

use super::{DecodeError, DecodeLimits, decode};
use crate::limits::{LimitError, Limits, ResourceKind};
use crate::{Pack, PackArchiveBytes};

/// A resource bounded during Pack Archive Read.
pub type ReadResource = ResourceKind<7>;

#[allow(non_upper_case_globals)]
impl ResourceKind<7> {
    pub const ArchiveBytes: Self = Self::new(0);
}

/// Pack Archive Read exceeded a mandatory ceiling.
pub type ReadLimitError = LimitError<ReadResource>;

/// Mandatory finite resource ceilings for Pack Archive Read.
pub type ReadLimits = Limits<ReadResource>;

impl Limits<ReadResource> {
    /// Constructs a validated read ceiling.
    #[track_caller]
    pub fn new(archive_bytes: u64) -> Self {
        Self::from_ceilings([archive_bytes, 0, 0, 0, 0, 0, 0])
            .assert_probe_resources([ReadResource::ArchiveBytes])
    }

    /// The first-party read limit for version-1 Pack Archives.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([512 * 1024 * 1024, 0, 0, 0, 0, 0, 0])
    }

    pub const fn archive_bytes(&self) -> u64 {
        self.ceilings[0]
    }
}

/// A failure while reading exact Pack Archive bytes from a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    #[error(transparent)]
    Limit(#[from] ReadLimitError),
    #[error("failed to read Pack Archive bytes: {0}")]
    Read(#[source] io::Error),
}

/// Reads exact bytes from a stream under a mandatory finite ceiling.
pub fn read(mut reader: impl Read, limits: ReadLimits) -> Result<PackArchiveBytes, ReadError> {
    const BUFFER_BYTES: usize = 8 * 1024;

    let mut bytes = Vec::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut observed = 0u64;
    loop {
        let probe_end =
            limits
                .archive_bytes()
                .checked_add(1)
                .ok_or(ReadLimitError::AccountingOverflow {
                    resource: ReadResource::ArchiveBytes,
                })?;
        let remaining =
            probe_end
                .checked_sub(observed)
                .ok_or(ReadLimitError::AccountingOverflow {
                    resource: ReadResource::ArchiveBytes,
                })?;
        let read_length = usize::try_from(remaining.min(BUFFER_BYTES as u64)).map_err(|_| {
            ReadLimitError::AccountingOverflow {
                resource: ReadResource::ArchiveBytes,
            }
        })?;
        let read = match reader.read(&mut buffer[..read_length]) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ReadError::Read(error)),
        };
        if read == 0 {
            return Ok(PackArchiveBytes::from_vec(bytes));
        }
        observed = observed
            .checked_add(
                u64::try_from(read).map_err(|_| ReadLimitError::AccountingOverflow {
                    resource: ReadResource::ArchiveBytes,
                })?,
            )
            .ok_or(ReadLimitError::AccountingOverflow {
                resource: ReadResource::ArchiveBytes,
            })?;
        if observed > limits.archive_bytes() {
            return Err(ReadLimitError::exceeded(
                ReadResource::ArchiveBytes,
                limits.archive_bytes(),
            )
            .into());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

/// A failure in bounded read followed by Pack Archive Decoding.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadPackError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error("read bytes could not be decoded as a Pack: {source}")]
    Decode {
        archive: PackArchiveBytes,
        #[source]
        source: DecodeError,
    },
}

/// Reads and decodes one Pack while preserving exact bytes on decode failure.
pub fn read_pack(
    reader: impl Read,
    read_limits: ReadLimits,
    decode_limits: DecodeLimits,
) -> Result<Pack, ReadPackError> {
    let archive = read(reader, read_limits)?;
    match decode(&archive, decode_limits) {
        Ok(pack) => Ok(pack),
        Err(source) => Err(ReadPackError::Decode { archive, source }),
    }
}

/// The filesystem phase in which Pack Archive Read failed.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FileReadPhase {
    Open,
    Metadata,
    Read,
}

/// A failure while reading exact Pack Archive bytes from a file.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileReadError {
    #[error("failed to open Pack Archive {path:?}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to inspect Pack Archive {path:?}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Pack Archive {path:?} exceeds its read limit: {source}")]
    Limit {
        path: PathBuf,
        phase: FileReadPhase,
        #[source]
        source: ReadLimitError,
    },
    #[error("failed to read Pack Archive {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(feature = "fs")]
impl FileReadError {
    pub const fn phase(&self) -> FileReadPhase {
        match self {
            Self::Open { .. } => FileReadPhase::Open,
            Self::Metadata { .. } => FileReadPhase::Metadata,
            Self::Limit { phase, .. } => *phase,
            Self::Read { .. } => FileReadPhase::Read,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Open { path, .. }
            | Self::Metadata { path, .. }
            | Self::Limit { path, .. }
            | Self::Read { path, .. } => path,
        }
    }
}

/// Reads exact bytes from one file using known-size preflight and metered reads.
#[cfg(feature = "fs")]
pub fn read_file(
    path: impl AsRef<Path>,
    limits: ReadLimits,
) -> Result<PackArchiveBytes, FileReadError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|source| FileReadError::Open {
        path: path.to_owned(),
        source,
    })?;
    let known_size = file
        .metadata()
        .map_err(|source| FileReadError::Metadata {
            path: path.to_owned(),
            source,
        })?
        .len();
    if known_size > limits.archive_bytes() {
        return Err(FileReadError::Limit {
            path: path.to_owned(),
            phase: FileReadPhase::Metadata,
            source: ReadLimitError::exceeded(ReadResource::ArchiveBytes, limits.archive_bytes()),
        });
    }
    read(file, limits).map_err(|error| match error {
        ReadError::Limit(source) => FileReadError::Limit {
            path: path.to_owned(),
            phase: FileReadPhase::Read,
            source,
        },
        ReadError::Read(source) => FileReadError::Read {
            path: path.to_owned(),
            source,
        },
    })
}

/// A failure in bounded file read followed by Pack Archive Decoding.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OpenPackError {
    #[error(transparent)]
    Read(#[from] FileReadError),
    #[error("read bytes from {path:?} could not be decoded as a Pack: {source}")]
    Decode {
        path: PathBuf,
        archive: PackArchiveBytes,
        #[source]
        source: Box<DecodeError>,
    },
}

/// Reads and decodes one Pack file while preserving exact bytes on decode failure.
#[cfg(feature = "fs")]
pub fn open_pack(
    path: impl AsRef<Path>,
    read_limits: ReadLimits,
    decode_limits: DecodeLimits,
) -> Result<Pack, OpenPackError> {
    let path = path.as_ref();
    let archive = read_file(path, read_limits)?;
    match decode(&archive, decode_limits) {
        Ok(pack) => Ok(pack),
        Err(source) => Err(OpenPackError::Decode {
            path: path.to_owned(),
            archive,
            source: Box::new(source),
        }),
    }
}
