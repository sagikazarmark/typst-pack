//! Pack Archive Acquisition through caller-supplied OpenDAL operators.

use std::{error::Error, fmt};

use super::acquisition::{
    ExactObjectAcquisitionError, ExactObjectLimitError, acquire_exact_object,
};
use super::{Location, LocationRoleError, OperatorResolver};
use crate::PackArchiveBytes;
use crate::pack_archive::{AcquisitionLimitError, AcquisitionLimits, AcquisitionResource};

/// A validated request to acquire one exact Pack Archive object.
#[derive(Clone, Debug)]
pub struct PackArchiveAcquisitionRequest {
    source: Location,
    limits: AcquisitionLimits,
}

impl PackArchiveAcquisitionRequest {
    /// Validates an exact-object source and retains its acquisition limits.
    pub fn new(
        source: Location,
        limits: AcquisitionLimits,
    ) -> Result<Self, PackArchiveAcquisitionRequestError> {
        if let Err(role_error) = source.require_object() {
            return Err(PackArchiveAcquisitionRequestError::InvalidSourceRole {
                location: source,
                source: role_error,
            });
        }

        Ok(Self { source, limits })
    }

    /// The normalized exact-object source.
    pub fn source(&self) -> &Location {
        &self.source
    }

    /// The mandatory finite Pack Archive Acquisition limits.
    pub const fn limits(&self) -> AcquisitionLimits {
        self.limits
    }
}

/// A reason a Pack Archive Acquisition request is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveAcquisitionRequestError {
    #[error("Pack Archive source {location} is not an exact object: {source}")]
    InvalidSourceRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Acquires exact Pack Archive bytes without decoding or validating them.
///
/// Decoding borrows the acquired bytes, so a decode failure leaves the exact
/// bytes available for inspection or replay:
///
/// ```no_run
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_archive::{
///     PackArchiveAcquisitionRequest, acquire_pack_archive,
/// };
/// use typst_pack::pack_archive::{DecodeLimits, decode};
///
/// async fn acquire_then_decode(
///     bindings: &OperatorBindings,
///     request: &PackArchiveAcquisitionRequest,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let archive = acquire_pack_archive(bindings, request).await?;
///     if let Err(decode_error) = decode(&archive, DecodeLimits::reference_v1()) {
///         // Decoding borrowed `archive`; the exact acquired bytes are retained.
///         let retry_bytes = archive.as_slice();
///         eprintln!("decode failed for {} retained bytes: {decode_error}", retry_bytes.len());
///     }
///     Ok(())
/// }
/// ```
pub async fn acquire_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchiveAcquisitionRequest,
) -> Result<PackArchiveBytes, PackArchiveAcquisitionError<R::Error>> {
    let bytes = acquire_exact_object(resolver, request.source(), request.limits().archive_bytes())
        .await
        .map_err(|error| PackArchiveAcquisitionError {
            source_location: request.source().clone(),
            cause: map_acquisition_error(error),
        })?;

    Ok(PackArchiveBytes::from_vec(bytes))
}

/// A failure while acquiring exact Pack Archive bytes through OpenDAL.
///
/// This error's own `Display` and `Debug` output omits native resolver and
/// OpenDAL messages. Rendering its complete source chain may disclose backend
/// endpoints, bucket names, or other backend-provided context.
pub struct PackArchiveAcquisitionError<E> {
    source_location: Location,
    cause: PackArchiveAcquisitionErrorCause<E>,
}

impl<E> PackArchiveAcquisitionError<E> {
    /// The normalized exact-object source that failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The typed cause of the acquisition failure.
    pub fn cause(&self) -> &PackArchiveAcquisitionErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for PackArchiveAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Pack Archive Acquisition failed for binding {} at exact-object operation path {:?}: {}",
            self.source_location.binding(),
            self.source_location.operation_path(),
            self.cause.label(),
        )
    }
}

impl<E> fmt::Debug for PackArchiveAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackArchiveAcquisitionError")
            .field("binding", self.source_location.binding())
            .field("role", &"exact object")
            .field("operation_path", &self.source_location.operation_path())
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackArchiveAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackArchiveAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            PackArchiveAcquisitionErrorCause::ReadUnsupported => None,
            PackArchiveAcquisitionErrorCause::ObjectAbsent(source)
            | PackArchiveAcquisitionErrorCause::Read(source) => Some(source),
            PackArchiveAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

/// The typed cause of an OpenDAL Pack Archive Acquisition failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackArchiveAcquisitionErrorCause<E> {
    ResolveOperator(E),
    ReadUnsupported,
    ObjectAbsent(::opendal::Error),
    Read(::opendal::Error),
    Limit(AcquisitionLimitError),
}

impl<E> PackArchiveAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::ReadUnsupported => "read capability is unsupported",
            Self::ObjectAbsent(_) => "the exact object is absent",
            Self::Read(_) => "the exact object read failed",
            Self::Limit(_) => "the archive byte limit failed",
        }
    }
}

fn map_acquisition_error<E>(
    error: ExactObjectAcquisitionError<E>,
) -> PackArchiveAcquisitionErrorCause<E> {
    match error {
        ExactObjectAcquisitionError::InvalidLocationRole(_) => {
            unreachable!("PackArchiveAcquisitionRequest validates the exact-object role")
        }
        ExactObjectAcquisitionError::ResolveOperator(source) => {
            PackArchiveAcquisitionErrorCause::ResolveOperator(source)
        }
        ExactObjectAcquisitionError::ReadUnsupported => {
            PackArchiveAcquisitionErrorCause::ReadUnsupported
        }
        ExactObjectAcquisitionError::ObjectAbsent(source) => {
            PackArchiveAcquisitionErrorCause::ObjectAbsent(source)
        }
        ExactObjectAcquisitionError::Read(source) => PackArchiveAcquisitionErrorCause::Read(source),
        ExactObjectAcquisitionError::Limit(source) => {
            PackArchiveAcquisitionErrorCause::Limit(match source {
                ExactObjectLimitError::Exceeded {
                    ceiling,
                    observed_at_least,
                } => AcquisitionLimitError::Exceeded {
                    resource: AcquisitionResource::ArchiveBytes,
                    ceiling,
                    observed_at_least,
                },
                ExactObjectLimitError::AccountingOverflow => {
                    AcquisitionLimitError::AccountingOverflow {
                        resource: AcquisitionResource::ArchiveBytes,
                    }
                }
            })
        }
    }
}
