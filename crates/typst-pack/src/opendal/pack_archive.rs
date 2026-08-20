//! Pack Archive Read through caller-supplied OpenDAL operators.

use super::BoxError;
use super::read::{
    ExactPathReadOperation, ResolvedOperators, exact_path_absent_error, read_exact_path,
};
use super::{Location, LocationRoleError, OperatorResolver};
use crate::PackArchiveBytes;
use crate::pack_archive::{ReadLimitError, ReadLimits, ReadResource};
use crate::redacted_error::RedactedError;

/// A validated request to read one exact Pack Archive object.
#[derive(Clone, Debug)]
pub struct PackArchiveReadRequest {
    source: Location,
    limits: ReadLimits,
}

impl PackArchiveReadRequest {
    /// Validates an exact-object source and retains its read limits.
    pub fn new(source: Location, limits: ReadLimits) -> Result<Self, PackArchiveReadRequestError> {
        if let Err(role_error) = source.require_object() {
            return Err(PackArchiveReadRequestError::InvalidSourceRole {
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

    /// The mandatory finite Pack Archive Read limits.
    pub const fn limits(&self) -> ReadLimits {
        self.limits
    }
}

/// A reason a Pack Archive Read request is invalid.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveReadRequestError {
    #[error("Pack Archive source {location} is not an exact object: {source}")]
    InvalidSourceRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Reads exact Pack Archive bytes without decoding or validating them.
///
/// Decoding borrows the read bytes, so a decode failure leaves the exact
/// bytes available for inspection or replay:
///
/// ```no_run
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_archive::{
///     PackArchiveReadRequest, read_pack_archive,
/// };
/// use typst_pack::pack_archive::{DecodeLimits, decode};
///
/// async fn read_then_decode(
///     bindings: &OperatorBindings,
///     request: &PackArchiveReadRequest,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let archive = read_pack_archive(bindings, request).await?;
///     if let Err(decode_error) = decode(&archive, DecodeLimits::reference_v1()) {
///         // Decoding borrowed `archive`; the exact read bytes are retained.
///         let retry_bytes = archive.as_slice();
///         eprintln!("decode failed for {} retained bytes: {decode_error}", retry_bytes.len());
///     }
///     Ok(())
/// }
/// ```
pub async fn read_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchiveReadRequest,
) -> Result<PackArchiveBytes, PackArchiveReadError> {
    let error = |cause| PackArchiveReadError {
        source_location: request.source().clone(),
        cause: RedactedError::new(cause),
    };
    let mut operators = ResolvedOperators::new(resolver);
    let resolved = operators
        .resolve(request.source().binding())
        .map_err(|source| error(PackArchiveReadErrorCause::ResolveOperator(Box::new(source))))?;
    if !resolved.read {
        return Err(error(PackArchiveReadErrorCause::ReadUnsupported));
    }
    let ceiling = request.limits().archive_bytes();
    let operation = PackArchiveExactPathOperation { request };
    let bytes = read_exact_path(
        &resolved.operator,
        request.source().dispatch_path(),
        ceiling,
        ceiling,
        &operation,
    )
    .await?
    .ok_or_else(|| {
        operation.error(PackArchiveReadErrorCause::ObjectAbsent(
            exact_path_absent_error(),
        ))
    })?;

    Ok(PackArchiveBytes::from_vec(bytes))
}

/// A failure while reading exact Pack Archive bytes through OpenDAL.
///
/// Rendering the complete source chain may disclose backend endpoints, bucket
/// names, or other backend-provided context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive Read failed for binding {binding} at exact-object operation path {operation_path:?}: {cause}",
    binding = .source_location.binding(),
    operation_path = .source_location.operation_path(),
)]
pub struct PackArchiveReadError {
    source_location: Location,
    #[source]
    cause: RedactedError<PackArchiveReadErrorCause>,
}

impl PackArchiveReadError {
    /// The normalized exact-object source that failed.
    pub fn source_location(&self) -> &Location {
        &self.source_location
    }

    /// The typed cause of the read failure.
    pub fn cause(&self) -> &PackArchiveReadErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Archive Read failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveReadErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("read capability is unsupported")]
    ReadUnsupported,
    #[error("the exact object is absent")]
    ObjectAbsent(#[source] ::opendal::Error),
    #[error("the exact object read failed")]
    Read(#[source] ::opendal::Error),
    #[error("the archive byte limit failed")]
    Limit(#[source] ReadLimitError),
}

struct PackArchiveExactPathOperation<'a> {
    request: &'a PackArchiveReadRequest,
}

impl PackArchiveExactPathOperation<'_> {
    fn error(&self, cause: PackArchiveReadErrorCause) -> PackArchiveReadError {
        PackArchiveReadError {
            source_location: self.request.source().clone(),
            cause: RedactedError::new(cause),
        }
    }
}

impl ExactPathReadOperation for PackArchiveExactPathOperation<'_> {
    type Error = PackArchiveReadError;

    fn read(&self, source: ::opendal::Error) -> PackArchiveReadError {
        self.error(PackArchiveReadErrorCause::Read(source))
    }

    fn limit_exceeded(&self, ceiling: u64, _: u64) -> PackArchiveReadError {
        self.error(PackArchiveReadErrorCause::Limit(ReadLimitError::exceeded(
            ReadResource::ArchiveBytes,
            ceiling,
        )))
    }

    fn accounting_overflow(&self) -> PackArchiveReadError {
        self.error(PackArchiveReadErrorCause::Limit(
            ReadLimitError::AccountingOverflow {
                resource: ReadResource::ArchiveBytes,
            },
        ))
    }
}
