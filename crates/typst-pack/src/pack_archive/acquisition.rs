use std::io::{self, Read};

#[cfg(feature = "fs")]
use std::path::{Path, PathBuf};

use super::{DecodeError, DecodeLimits, decode};
use crate::limits::{LimitError, Limits, LimitsError, ResourceKind};
use crate::{Pack, PackArchiveBytes};

/// A resource bounded during Pack Archive Acquisition.
pub type AcquisitionResource = ResourceKind<7>;

#[allow(non_upper_case_globals)]
impl ResourceKind<7> {
    pub const ArchiveBytes: Self = Self::new(0);
}

/// A supplied acquisition ceiling that cannot support bounded accounting.
pub type AcquisitionLimitsError = LimitsError<AcquisitionResource>;

/// Pack Archive Acquisition exceeded a mandatory ceiling.
pub type AcquisitionLimitError = LimitError<AcquisitionResource>;

/// Mandatory finite resource ceilings for Pack Archive Acquisition.
pub type AcquisitionLimits = Limits<AcquisitionResource>;

impl Limits<AcquisitionResource> {
    /// Constructs a validated acquisition ceiling.
    pub fn new(archive_bytes: u64) -> Result<Self, AcquisitionLimitsError> {
        Self::from_ceilings([archive_bytes, 0, 0, 0, 0, 0, 0])
            .validate_probe_resources([AcquisitionResource::ArchiveBytes])
    }

    /// The first-party acquisition limit for version-1 Pack Archives.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([512 * 1024 * 1024, 0, 0, 0, 0, 0, 0])
    }

    pub const fn archive_bytes(&self) -> u64 {
        self.ceilings[0]
    }
}

/// A failure while acquiring exact Pack Archive bytes from a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AcquisitionError {
    #[error(transparent)]
    Limit(#[from] AcquisitionLimitError),
    #[error("failed to read Pack Archive bytes: {0}")]
    Read(#[source] io::Error),
}

/// Acquires exact bytes from a stream under a mandatory finite ceiling.
pub fn acquire(
    mut reader: impl Read,
    limits: AcquisitionLimits,
) -> Result<PackArchiveBytes, AcquisitionError> {
    const BUFFER_BYTES: usize = 8 * 1024;

    let mut bytes = Vec::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut observed = 0u64;
    loop {
        let probe_end = limits.archive_bytes().checked_add(1).ok_or(
            AcquisitionLimitError::AccountingOverflow {
                resource: AcquisitionResource::ArchiveBytes,
            },
        )?;
        let remaining =
            probe_end
                .checked_sub(observed)
                .ok_or(AcquisitionLimitError::AccountingOverflow {
                    resource: AcquisitionResource::ArchiveBytes,
                })?;
        let read_length = usize::try_from(remaining.min(BUFFER_BYTES as u64)).map_err(|_| {
            AcquisitionLimitError::AccountingOverflow {
                resource: AcquisitionResource::ArchiveBytes,
            }
        })?;
        let read = match reader.read(&mut buffer[..read_length]) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(AcquisitionError::Read(error)),
        };
        if read == 0 {
            return Ok(PackArchiveBytes::from_vec(bytes));
        }
        observed = observed
            .checked_add(u64::try_from(read).map_err(|_| {
                AcquisitionLimitError::AccountingOverflow {
                    resource: AcquisitionResource::ArchiveBytes,
                }
            })?)
            .ok_or(AcquisitionLimitError::AccountingOverflow {
                resource: AcquisitionResource::ArchiveBytes,
            })?;
        if observed > limits.archive_bytes() {
            return Err(AcquisitionLimitError::exceeded(
                AcquisitionResource::ArchiveBytes,
                limits.archive_bytes(),
            )
            .into());
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

/// A failure in bounded acquisition followed by Pack Archive Decoding.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadPackError {
    #[error(transparent)]
    Acquire(#[from] AcquisitionError),
    #[error("acquired bytes could not be decoded as a Pack: {source}")]
    Decode {
        archive: PackArchiveBytes,
        #[source]
        source: DecodeError,
    },
}

/// Acquires and decodes one Pack while preserving exact bytes on decode failure.
pub fn read_pack(
    reader: impl Read,
    acquisition_limits: AcquisitionLimits,
    decode_limits: DecodeLimits,
) -> Result<Pack, ReadPackError> {
    let archive = acquire(reader, acquisition_limits)?;
    match decode(&archive, decode_limits) {
        Ok(pack) => Ok(pack),
        Err(source) => Err(ReadPackError::Decode { archive, source }),
    }
}

/// The filesystem phase in which Pack Archive Acquisition failed.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum FileAcquisitionPhase {
    Open,
    Metadata,
    Read,
}

/// A failure while acquiring exact Pack Archive bytes from a file.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FileAcquisitionError {
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
    #[error("Pack Archive {path:?} exceeds its acquisition limit: {source}")]
    Limit {
        path: PathBuf,
        phase: FileAcquisitionPhase,
        #[source]
        source: AcquisitionLimitError,
    },
    #[error("failed to acquire Pack Archive {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(feature = "fs")]
impl FileAcquisitionError {
    pub const fn phase(&self) -> FileAcquisitionPhase {
        match self {
            Self::Open { .. } => FileAcquisitionPhase::Open,
            Self::Metadata { .. } => FileAcquisitionPhase::Metadata,
            Self::Limit { phase, .. } => *phase,
            Self::Read { .. } => FileAcquisitionPhase::Read,
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

/// Acquires exact bytes from one file using known-size preflight and metered reads.
#[cfg(feature = "fs")]
pub fn acquire_file(
    path: impl AsRef<Path>,
    limits: AcquisitionLimits,
) -> Result<PackArchiveBytes, FileAcquisitionError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|source| FileAcquisitionError::Open {
        path: path.to_owned(),
        source,
    })?;
    let known_size = file
        .metadata()
        .map_err(|source| FileAcquisitionError::Metadata {
            path: path.to_owned(),
            source,
        })?
        .len();
    if known_size > limits.archive_bytes() {
        return Err(FileAcquisitionError::Limit {
            path: path.to_owned(),
            phase: FileAcquisitionPhase::Metadata,
            source: AcquisitionLimitError::exceeded(
                AcquisitionResource::ArchiveBytes,
                limits.archive_bytes(),
            ),
        });
    }
    acquire(file, limits).map_err(|error| match error {
        AcquisitionError::Limit(source) => FileAcquisitionError::Limit {
            path: path.to_owned(),
            phase: FileAcquisitionPhase::Read,
            source,
        },
        AcquisitionError::Read(source) => FileAcquisitionError::Read {
            path: path.to_owned(),
            source,
        },
    })
}

/// A failure in bounded file acquisition followed by Pack Archive Decoding.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OpenPackError {
    #[error(transparent)]
    Acquire(#[from] FileAcquisitionError),
    #[error("acquired bytes from {path:?} could not be decoded as a Pack: {source}")]
    Decode {
        path: PathBuf,
        archive: PackArchiveBytes,
        #[source]
        source: Box<DecodeError>,
    },
}

/// Acquires and decodes one Pack file while preserving exact bytes on decode failure.
#[cfg(feature = "fs")]
pub fn open_pack(
    path: impl AsRef<Path>,
    acquisition_limits: AcquisitionLimits,
    decode_limits: DecodeLimits,
) -> Result<Pack, OpenPackError> {
    let path = path.as_ref();
    let archive = acquire_file(path, acquisition_limits)?;
    match decode(&archive, decode_limits) {
        Ok(pack) => Ok(pack),
        Err(source) => Err(OpenPackError::Decode {
            path: path.to_owned(),
            archive,
            source: Box::new(source),
        }),
    }
}
