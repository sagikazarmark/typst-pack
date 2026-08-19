//! Pack Archive Acquisition through caller-supplied OpenDAL operators.

use super::BoxError;
use super::acquisition::{
    ExactPathAcquisitionOperation, ResolvedOperators, acquire_exact_path, exact_path_absent_error,
};
use super::{Location, LocationRoleError, OperatorResolver};
use crate::PackArchiveBytes;
use crate::pack_archive::{AcquisitionLimitError, AcquisitionLimits, AcquisitionResource};
use crate::redacted_error::RedactedError;

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
) -> Result<PackArchiveBytes, PackArchiveAcquisitionError> {
    let error = |cause| PackArchiveAcquisitionError {
        source_location: request.source().clone(),
        cause: RedactedError::new(cause),
    };
    let mut operators = ResolvedOperators::new(resolver);
    let resolved = operators
        .resolve(request.source().binding())
        .map_err(|source| {
            error(PackArchiveAcquisitionErrorCause::ResolveOperator(Box::new(
                source,
            )))
        })?;
    if !resolved.read {
        return Err(error(PackArchiveAcquisitionErrorCause::ReadUnsupported));
    }
    let ceiling = request.limits().archive_bytes();
    let operation = PackArchiveExactPathOperation { request };
    let bytes = acquire_exact_path(
        &resolved.operator,
        request.source().dispatch_path(),
        ceiling,
        ceiling,
        &operation,
    )
    .await?
    .ok_or_else(|| {
        operation.error(PackArchiveAcquisitionErrorCause::ObjectAbsent(
            exact_path_absent_error(),
        ))
    })?;

    Ok(PackArchiveBytes::from_vec(bytes))
}

/// A failure while acquiring exact Pack Archive bytes through OpenDAL.
///
/// Rendering the complete source chain may disclose backend endpoints, bucket
/// names, or other backend-provided context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive Acquisition failed for binding {binding} at exact-object operation path {operation_path:?}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
)]
pub struct PackArchiveAcquisitionError {
    source_location: Location,
    #[source]
    cause: RedactedError<PackArchiveAcquisitionErrorCause>,
}

impl PackArchiveAcquisitionError {
    /// The normalized exact-object source that failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The typed cause of the acquisition failure.
    pub fn cause(&self) -> &PackArchiveAcquisitionErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Archive Acquisition failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveAcquisitionErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("read capability is unsupported")]
    ReadUnsupported,
    #[error("the exact object is absent")]
    ObjectAbsent(#[source] ::opendal::Error),
    #[error("the exact object read failed")]
    Read(#[source] ::opendal::Error),
    #[error("the archive byte limit failed")]
    Limit(#[source] AcquisitionLimitError),
}

struct PackArchiveExactPathOperation<'a> {
    request: &'a PackArchiveAcquisitionRequest,
}

impl PackArchiveExactPathOperation<'_> {
    fn error(&self, cause: PackArchiveAcquisitionErrorCause) -> PackArchiveAcquisitionError {
        PackArchiveAcquisitionError {
            source_location: self.request.source().clone(),
            cause: RedactedError::new(cause),
        }
    }
}

impl ExactPathAcquisitionOperation for PackArchiveExactPathOperation<'_> {
    type Error = PackArchiveAcquisitionError;

    fn read(&self, source: ::opendal::Error) -> PackArchiveAcquisitionError {
        self.error(PackArchiveAcquisitionErrorCause::Read(source))
    }

    fn limit_exceeded(&self, ceiling: u64, observed_at_least: u64) -> PackArchiveAcquisitionError {
        self.error(PackArchiveAcquisitionErrorCause::Limit(
            AcquisitionLimitError::Exceeded {
                resource: AcquisitionResource::ArchiveBytes,
                ceiling,
                observed_at_least,
            },
        ))
    }

    fn accounting_overflow(&self) -> PackArchiveAcquisitionError {
        self.error(PackArchiveAcquisitionErrorCause::Limit(
            AcquisitionLimitError::AccountingOverflow {
                resource: AcquisitionResource::ArchiveBytes,
            },
        ))
    }
}
