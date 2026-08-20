//! OpenDAL exact-key writing vocabulary and crate-private execution.

use std::{collections::BTreeMap, future::Future};

use futures_util::StreamExt;
use opendal::ErrorKind;

use super::location::validate_decoded_artifact_key_path;
use super::{
    BoxError, Location, LocationError, LocationRoleError, OperatorBinding, OperatorResolver,
};
use crate::redacted_error::RedactedError;
use crate::{
    CanonicalIdentity, CommitCertainty, CompilationResult, CompilationStatus, PackArchiveBytes,
};
pub use crate::{
    CompilationArtifactWriteEntry, CompilationArtifactWriteProgress,
    CompilationArtifactWriteReceipt, PackExtractionWriteEntry, PackExtractionWriteProgress,
    PackExtractionWriteReceipt, WriteKeyOutcome,
};

/// The exact-key conflict policy for an OpenDAL write operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WritePolicy {
    /// Create absent objects and accept existing objects only when their bytes match.
    CreateOrVerify,
    /// Write every exact key without inspecting its existing value.
    OverwriteExactKeys,
}

/// The OpenDAL adapter phase reached by a write attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OpenDalWritePhase {
    ResultValidation,
    DestinationValidation,
    ResolveOperator,
    CapabilityAppraisal,
    PreflightRead,
    ConditionalCreate,
    RaceVerification,
    DirectWrite,
    Complete,
}

/// A validated request to write one exact Pack Archive object.
#[derive(Clone, Debug)]
pub struct PackArchiveWriteRequest {
    destination: Location,
    policy: WritePolicy,
}

impl PackArchiveWriteRequest {
    /// Validates an exact-object destination and retains the explicit policy.
    pub fn new(
        destination: Location,
        policy: WritePolicy,
    ) -> Result<Self, PackArchiveWriteRequestError> {
        destination.require_object().map_err(|source| {
            PackArchiveWriteRequestError::InvalidDestinationRole {
                location: destination.clone(),
                source,
            }
        })?;

        Ok(Self {
            destination,
            policy,
        })
    }

    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }
}

/// A reason a Pack Archive write request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveWriteRequestError {
    #[error("Pack Archive destination {location} is not an exact object: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Writes exact borrowed Pack Archive bytes to one normalized object.
///
/// Dropping the returned future yields no receipt, and already-issued storage
/// work may have occurred. The caller retains `archive`; full replay with the
/// same exact bytes is the recovery contract.
///
/// ```no_run
/// use typst_pack::{Pack, PackArchiveBytes};
/// use typst_pack::opendal::{Location, OperatorBindings};
/// use typst_pack::opendal::pack_archive::{
///     PackArchiveReadRequest, read_pack_archive,
/// };
/// use typst_pack::opendal::write::{
///     PackArchiveWriteRequest, WritePolicy, write_pack_archive,
/// };
/// use typst_pack::pack_archive::{ReadLimits, DecodeError, DecodeLimits, decode};
///
/// enum WriteThenReadOutcome {
///     Matching {
///         read: PackArchiveBytes,
///         decoded: Result<Pack, DecodeError>,
///     },
///     DestinationChanged {
///         read: PackArchiveBytes,
///     },
/// }
///
/// async fn write_replay_and_read(
///     bindings: &OperatorBindings,
///     destination: Location,
///     archive: &PackArchiveBytes,
/// ) -> Result<WriteThenReadOutcome, Box<dyn std::error::Error>> {
///     let overwrite = PackArchiveWriteRequest::new(
///         destination.clone(),
///         WritePolicy::OverwriteExactKeys,
///     )?;
///     write_pack_archive(bindings, &overwrite, archive).await?;
///
///     let replay = PackArchiveWriteRequest::new(
///         destination.clone(),
///         WritePolicy::CreateOrVerify,
///     )?;
///     write_pack_archive(bindings, &replay, archive).await?;
///     write_pack_archive(bindings, &replay, archive).await?;
///
///     let read = PackArchiveReadRequest::new(
///         destination,
///         ReadLimits::reference_v1(),
///     )?;
///     let read = read_pack_archive(bindings, &read).await?;
///
///     // The caller still owns `archive`; preserve the independently read
///     // bytes and do not decode when the mutable destination changed.
///     if archive.as_slice() != read.as_slice() {
///         return Ok(WriteThenReadOutcome::DestinationChanged { read });
///     }
///
///     let decoded = decode(&read, DecodeLimits::reference_v1());
///     Ok(WriteThenReadOutcome::Matching { read, decoded })
/// }
/// ```
pub async fn write_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchiveWriteRequest,
    archive: &PackArchiveBytes,
) -> Result<PackArchiveWriteReceipt, PackArchiveWriteError> {
    let mut progress = PackArchiveWriteProgress::new();
    let destination_path = request.destination().operation_path();
    let keys = [ExactKey::new(destination_path, archive.as_slice())];
    {
        let mut operation = PackArchiveWriteOperation {
            request,
            progress: &mut progress,
        };
        write_exact_keys(
            resolver,
            request.destination().binding(),
            request.policy(),
            &keys,
            &mut operation,
        )
        .await?;
    }

    Ok(PackArchiveWriteReceipt {
        destination: request.destination().clone(),
        policy: request.policy(),
        progress,
    })
}

/// A failure while writing exact Pack Archive bytes through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive write failed for binding {} at exact-object operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackArchiveWriteError {
    destination: Location,
    policy: WritePolicy,
    failed_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: PackArchiveWriteProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackArchiveWriteErrorCause>,
}

impl PackArchiveWriteError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }

    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalWritePhase {
        self.phase
    }

    pub fn progress(&self) -> &PackArchiveWriteProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackArchiveWriteErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Archive write failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchiveWriteErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the write policy is unsupported")]
    UnsupportedPolicy { policy: WritePolicy },
    #[error("the archive exceeds the advertised object size")]
    UnsupportedObjectSize { byte_length: u64 },
    #[error("a preflight read failed")]
    PreflightRead(#[source] ::opendal::Error),
    #[error("destination bytes conflict")]
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    #[error("a conditional create failed")]
    ConditionalCreate(#[source] ::opendal::Error),
    #[error("race verification failed")]
    RaceVerification(#[source] ::opendal::Error),
    #[error("a direct write failed")]
    DirectWrite(#[source] ::opendal::Error),
}

/// A validated request to write caller-supplied bytes to one package-cache object.
///
/// This request fixes [`WritePolicy::CreateOrVerify`]. It does not offer a
/// replacement mode and does not represent Package Archive Expansion or Package
/// Catalog insertion.
#[derive(Clone, Debug)]
pub struct PackageCacheArchiveWriteRequest {
    destination: Location,
}

impl PackageCacheArchiveWriteRequest {
    /// Validates and retains a normalized exact-object cache destination.
    pub fn new(destination: Location) -> Result<Self, PackageCacheArchiveWriteRequestError> {
        destination.require_object().map_err(|source| {
            PackageCacheArchiveWriteRequestError::InvalidDestinationRole {
                location: destination.clone(),
                source,
            }
        })?;

        Ok(Self { destination })
    }

    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        WritePolicy::CreateOrVerify
    }
}

/// A reason a package-cache archive write request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageCacheArchiveWriteRequestError {
    #[error("package-cache archive destination {location} is not an exact object: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Writes caller-supplied exact archive bytes to one package-cache object.
///
/// This low-level operation does not expand the archive, validate a Package
/// Tree, or insert it into a Package Catalog. Direct use with unvalidated bytes
/// can poison a cache because a present malformed cache candidate is terminal.
/// Callers should write registry bytes only after successful expansion,
/// validation, and insertion.
///
/// Dropping the returned future yields no receipt, and already-issued storage
/// work may have occurred. The caller retains `archive`; full replay with the
/// same exact bytes is the recovery contract.
///
/// ```no_run
/// # #[cfg(feature = "package-reading")]
/// # mod example {
/// use std::error::Error;
/// use typst_pack::{
///     PackageReadFailures, PackageCatalog, PackageDisposition,
///     PackageExpansionLimits,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     PackageRead, RegistryArchiveResidue, insert_read_package,
/// };
/// use typst_pack::opendal::write::{
///     PackageCacheArchiveWriteRequest, write_package_cache_archive,
/// };
///
/// async fn insert_then_write_registry_archive(
///     bindings: &OperatorBindings,
///     catalog: &mut PackageCatalog,
///     failures: &mut PackageReadFailures,
///     read: PackageRead,
/// ) -> Result<Option<RegistryArchiveResidue>, Box<dyn Error>> {
///     let Some(residue) = insert_read_package(
///         catalog,
///         failures,
///         read,
///         PackageDisposition::Embedded,
///         PackageExpansionLimits::reference_v1(),
///     )? else {
///         return Ok(None);
///     };
///
///     let request = PackageCacheArchiveWriteRequest::new(
///         residue.destination().clone(),
///     )?;
///     if let Err(cache_failure) =
///         write_package_cache_archive(bindings, &request, residue.bytes()).await
///     {
///         // Insertion remains successful. The residue retains the exact bytes
///         // and destination so the caller can report and replay independently.
///         drop(cache_failure);
///     }
///
///     Ok(Some(residue))
/// }
/// # }
/// ```
pub async fn write_package_cache_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageCacheArchiveWriteRequest,
    archive: &[u8],
) -> Result<PackageCacheArchiveWriteReceipt, PackageCacheArchiveWriteError> {
    let mut progress = PackageCacheArchiveWriteProgress::new();
    let destination_path = request.destination().operation_path();
    let keys = [ExactKey::new(destination_path, archive)];
    {
        let mut operation = PackageCacheArchiveWriteOperation {
            request,
            progress: &mut progress,
        };
        write_create_or_verify_exact_keys(
            resolver,
            request.destination().binding(),
            &keys,
            &mut operation,
        )
        .await?;
    }

    Ok(PackageCacheArchiveWriteReceipt {
        destination: request.destination().clone(),
        policy: request.policy(),
        progress,
    })
}

/// A failure while writing caller-supplied package-cache archive bytes.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "package-cache archive write failed for binding {} at exact-object operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackageCacheArchiveWriteError {
    destination: Location,
    policy: WritePolicy,
    failed_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: PackageCacheArchiveWriteProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackageCacheArchiveWriteErrorCause>,
}

impl PackageCacheArchiveWriteError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }

    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalWritePhase {
        self.phase
    }

    pub fn progress(&self) -> &PackageCacheArchiveWriteProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackageCacheArchiveWriteErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL package-cache archive write failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageCacheArchiveWriteErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the write policy is unsupported")]
    UnsupportedPolicy { policy: WritePolicy },
    #[error("the archive exceeds the advertised object size")]
    UnsupportedObjectSize { byte_length: u64 },
    #[error("a preflight read failed")]
    PreflightRead(#[source] ::opendal::Error),
    #[error("destination bytes conflict")]
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    #[error("a conditional create failed")]
    ConditionalCreate(#[source] ::opendal::Error),
    #[error("race verification failed")]
    RaceVerification(#[source] ::opendal::Error),
}

/// A validated request to write one Pack Extraction Plan beneath a prefix.
#[derive(Clone, Debug)]
pub struct PackExtractionWriteRequest {
    destination: Location,
    policy: WritePolicy,
}

impl PackExtractionWriteRequest {
    /// Validates a normalized prefix destination and retains the explicit policy.
    pub fn new(
        destination: Location,
        policy: WritePolicy,
    ) -> Result<Self, PackExtractionWriteRequestError> {
        destination.require_prefix().map_err(|source| {
            PackExtractionWriteRequestError::InvalidDestinationRole {
                location: destination.clone(),
                source,
            }
        })?;

        Ok(Self {
            destination,
            policy,
        })
    }

    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }
}

/// A reason a Pack Extraction write request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackExtractionWriteRequestError {
    #[error("Pack Extraction destination {location} is not a prefix: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// A validated request to write every artifact in one succeeded Compilation Result.
#[derive(Clone, Debug)]
pub struct CompilationArtifactWriteRequest {
    compilation_result_identity: CanonicalIdentity,
    destination: Location,
    artifact_keys: Vec<String>,
    policy: WritePolicy,
}

impl CompilationArtifactWriteRequest {
    /// Validates a prefix destination and one decoded relative key per canonical artifact.
    pub fn new(
        result: &CompilationResult,
        destination: Location,
        artifact_keys: impl IntoIterator<Item = impl Into<String>>,
        policy: WritePolicy,
    ) -> Result<Self, CompilationArtifactWriteRequestRejection> {
        let compilation_result_identity = result.result_identity();
        let artifact_keys = artifact_keys
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let mut issues = Vec::new();

        if result.status() != CompilationStatus::Succeeded {
            issues.push(CompilationArtifactWriteRequestIssue::ResultNotSucceeded);
        }
        if let Err(source) = destination.require_prefix() {
            issues.push(
                CompilationArtifactWriteRequestIssue::InvalidDestinationRole {
                    location: destination.clone(),
                    source,
                },
            );
        }
        if result.artifacts().len() != artifact_keys.len() {
            issues.push(
                CompilationArtifactWriteRequestIssue::ArtifactKeyCountMismatch {
                    expected: result.artifacts().len(),
                    actual: artifact_keys.len(),
                },
            );
        }
        let mut first_indices = BTreeMap::new();
        for (artifact_index, key) in artifact_keys.iter().enumerate() {
            if let Err(reason) = validate_artifact_key(key) {
                issues.push(CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                    artifact_index,
                    key: key.clone(),
                    reason,
                });
            }
            if let Some(&first_artifact_index) = first_indices.get(key) {
                issues.push(CompilationArtifactWriteRequestIssue::DuplicateArtifactKey {
                    key: key.clone(),
                    first_artifact_index,
                    duplicate_artifact_index: artifact_index,
                });
            } else {
                first_indices.insert(key.clone(), artifact_index);
            }
        }

        if !issues.is_empty() {
            return Err(CompilationArtifactWriteRequestRejection {
                compilation_result_identity,
                destination,
                issues: issues.into_boxed_slice(),
            });
        }

        Ok(Self {
            compilation_result_identity,
            destination,
            artifact_keys,
            policy,
        })
    }

    pub const fn compilation_result_identity(&self) -> CanonicalIdentity {
        self.compilation_result_identity
    }

    pub const fn destination(&self) -> &Location {
        &self.destination
    }

    pub fn artifact_keys(&self) -> &[String] {
        &self.artifact_keys
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }
}

/// Complete deterministic rejection of a Compilation Output Artifact write request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "Compilation Output Artifact write request rejected for binding {} beneath prefix operation path {:?} with {} issue(s)",
    .destination.binding(),
    .destination.operation_path(),
    .issues.len(),
)]
pub struct CompilationArtifactWriteRequestRejection {
    compilation_result_identity: CanonicalIdentity,
    destination: Location,
    issues: Box<[CompilationArtifactWriteRequestIssue]>,
}

impl CompilationArtifactWriteRequestRejection {
    pub const fn compilation_result_identity(&self) -> CanonicalIdentity {
        self.compilation_result_identity
    }

    pub fn issues(&self) -> &[CompilationArtifactWriteRequestIssue] {
        &self.issues
    }
}

/// One independently detectable issue in a Compilation Output Artifact write request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactWriteRequestIssue {
    #[error("a rejected Compilation Result cannot be written")]
    ResultNotSucceeded,
    #[error("Compilation Output Artifact destination {location} is not a prefix: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
    #[error("expected {expected} artifact key(s), but received {actual}")]
    ArtifactKeyCountMismatch { expected: usize, actual: usize },
    #[error("artifact key {key:?} at index {artifact_index} is invalid: {reason}")]
    InvalidArtifactKey {
        artifact_index: usize,
        key: String,
        reason: CompilationArtifactKeyIssue,
    },
    #[error(
        "artifact key {key:?} at index {duplicate_artifact_index} duplicates index {first_artifact_index}"
    )]
    DuplicateArtifactKey {
        key: String,
        first_artifact_index: usize,
        duplicate_artifact_index: usize,
    },
}

/// A reason a decoded relative Compilation Output Artifact key is unsafe or ambiguous.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactKeyIssue {
    #[error("an artifact key cannot be empty")]
    Empty,
    #[error("an artifact key cannot start with a slash")]
    LeadingSlash,
    #[error("an artifact key cannot end with a slash")]
    TrailingSlash,
    #[error("an artifact key cannot contain a repeated separator")]
    RepeatedSeparator,
    #[error("an artifact key cannot contain a dot segment")]
    DotSegment,
    #[error("an artifact key cannot contain a backslash")]
    Backslash,
    #[error("an artifact key cannot contain a control character")]
    ControlCharacter,
    #[error("an artifact key aliases another operation path at byte {index}")]
    NormalizationAlias { index: usize },
}

fn validate_artifact_key(key: &str) -> Result<(), CompilationArtifactKeyIssue> {
    if key.is_empty() {
        return Err(CompilationArtifactKeyIssue::Empty);
    }
    if key.starts_with('/') {
        return Err(CompilationArtifactKeyIssue::LeadingSlash);
    }
    if key.ends_with('/') {
        return Err(CompilationArtifactKeyIssue::TrailingSlash);
    }
    validate_decoded_artifact_key_path(key).map_err(|error| match error {
        LocationError::RepeatedSeparator { .. } => CompilationArtifactKeyIssue::RepeatedSeparator,
        LocationError::DotSegment { .. } => CompilationArtifactKeyIssue::DotSegment,
        LocationError::Backslash { .. } => CompilationArtifactKeyIssue::Backslash,
        LocationError::ControlCharacter { .. } => CompilationArtifactKeyIssue::ControlCharacter,
        LocationError::NormalizationAlias { index } => {
            CompilationArtifactKeyIssue::NormalizationAlias { index }
        }
        _ => unreachable!("decoded operation-path validation returned an unrelated error"),
    })
}

/// Writes every entry in one Pack Extraction Plan beneath the request's prefix.
///
/// The caller-owned progress is cleared synchronously before the returned future
/// can be polled or dropped. Replaying the same plan with `CreateOrVerify`
/// accepts objects whose bytes already match.
///
/// ```no_run
/// use typst_pack::PackExtractionPlan;
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::write::{
///     PackExtractionWriteProgress, PackExtractionWriteRequest,
///     WritePolicy, write_pack_extraction_plan,
/// };
///
/// async fn write_and_replay_partial_attempt(
///     bindings: &OperatorBindings,
///     plan: &PackExtractionPlan,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let request = PackExtractionWriteRequest::new(
///         "project:/extracted/".parse()?,
///         WritePolicy::CreateOrVerify,
///     )?;
///     let mut progress = PackExtractionWriteProgress::new();
///
///     if let Err(error) =
///         write_pack_extraction_plan(bindings, &request, plan, &mut progress).await
///     {
///         // The caller retains the exact completed prefix after a partial attempt.
///         assert_eq!(error.progress(), &progress);
///         write_pack_extraction_plan(bindings, &request, plan, &mut progress).await?;
///     }
///
///     Ok(())
/// }
/// ```
pub fn write_pack_extraction_plan<'a, R: OperatorResolver + ?Sized>(
    resolver: &'a R,
    request: &'a PackExtractionWriteRequest,
    plan: &'a crate::PackExtractionPlan,
    progress: &'a mut PackExtractionWriteProgress,
) -> impl Future<Output = Result<PackExtractionWriteReceipt, PackExtractionWriteError>> + 'a {
    progress.clear();
    async move {
        let mut destinations = Vec::with_capacity(plan.entries().len());
        for entry in plan.entries() {
            let destination = request
                .destination()
                .compose(entry.relative_path())
                .map_err(|_| {
                    pack_extraction_write_error(
                        request,
                        Some(entry.relative_path().to_owned()),
                        None,
                        OpenDalWritePhase::DestinationValidation,
                        progress,
                        CommitCertainty::NotCommitted,
                        PackExtractionWriteErrorCause::InvalidDestinationPath {
                            relative_path: entry.relative_path().to_owned(),
                        },
                    )
                })?;
            destinations.push(destination);
        }

        let keys = destinations
            .iter()
            .zip(plan.entries())
            .map(|(destination, entry)| ExactKey::new(destination.operation_path(), entry.bytes()))
            .collect::<Vec<_>>();
        {
            let mut operation = PackExtractionWriteOperation {
                request,
                plan,
                progress,
            };
            write_exact_keys(
                resolver,
                request.destination().binding(),
                request.policy(),
                &keys,
                &mut operation,
            )
            .await?;
        }

        Ok(PackExtractionWriteReceipt::new(
            *plan.pack_identity(),
            progress.clone(),
        ))
    }
}

/// A failure while writing a Pack Extraction Plan through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Extraction write failed for binding {} beneath prefix operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackExtractionWriteError {
    destination: Location,
    policy: WritePolicy,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: PackExtractionWriteProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackExtractionWriteErrorCause>,
}

impl PackExtractionWriteError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }

    pub fn failed_relative_path(&self) -> Option<&str> {
        self.failed_relative_path.as_deref()
    }

    pub fn failed_destination_path(&self) -> Option<&str> {
        self.failed_destination_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalWritePhase {
        self.phase
    }

    pub fn progress(&self) -> &PackExtractionWriteProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackExtractionWriteErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Extraction write failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackExtractionWriteErrorCause {
    #[error("a composed destination path was invalid")]
    InvalidDestinationPath { relative_path: String },
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the write policy is unsupported")]
    UnsupportedPolicy { policy: WritePolicy },
    #[error("an entry exceeds the advertised object size")]
    UnsupportedObjectSize { byte_length: u64 },
    #[error("a preflight read failed")]
    PreflightRead(#[source] ::opendal::Error),
    #[error("destination bytes conflict")]
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    #[error("a conditional create failed")]
    ConditionalCreate(#[source] ::opendal::Error),
    #[error("race verification failed")]
    RaceVerification(#[source] ::opendal::Error),
    #[error("a direct write failed")]
    DirectWrite(#[source] ::opendal::Error),
}

fn pack_extraction_write_error(
    request: &PackExtractionWriteRequest,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: &PackExtractionWriteProgress,
    commit_certainty: CommitCertainty,
    cause: PackExtractionWriteErrorCause,
) -> PackExtractionWriteError {
    PackExtractionWriteError {
        destination: request.destination().clone(),
        policy: request.policy(),
        failed_relative_path,
        failed_destination_path,
        phase,
        progress: progress.clone(),
        commit_certainty,
        cause: RedactedError::new(cause),
    }
}

/// Writes every canonical artifact beneath the request's normalized prefix.
///
/// The caller-owned progress is cleared synchronously before the returned future
/// can be polled or dropped. Replaying the same result with `CreateOrVerify`
/// accepts objects whose bytes already match.
///
/// ```no_run
/// use typst_pack::CompilationResult;
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::write::{
///     CompilationArtifactWriteProgress, CompilationArtifactWriteRequest,
///     WritePolicy, write_compilation_artifacts,
/// };
///
/// async fn write_and_replay(
///     bindings: &OperatorBindings,
///     document_result: &CompilationResult,
///     page_result: &CompilationResult,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let document_request = CompilationArtifactWriteRequest::new(
///         document_result,
///         "artifacts:/document/".parse()?,
///         ["document.pdf"],
///         WritePolicy::CreateOrVerify,
///     )?;
///     let page_keys = page_result
///         .artifacts()
///         .iter()
///         .map(|artifact| format!("page-{}.svg", artifact.source_page_number().unwrap()))
///         .collect::<Vec<_>>();
///     let page_request = CompilationArtifactWriteRequest::new(
///         page_result,
///         "artifacts:/pages/".parse()?,
///         page_keys,
///         WritePolicy::CreateOrVerify,
///     )?;
///
///     let mut document_progress = CompilationArtifactWriteProgress::new();
///     write_compilation_artifacts(
///         bindings,
///         &document_request,
///         document_result,
///         &mut document_progress,
///     )
///     .await?;
///     write_compilation_artifacts(
///         bindings,
///         &document_request,
///         document_result,
///         &mut document_progress,
///     )
///     .await?;
///
///     let mut page_progress = CompilationArtifactWriteProgress::new();
///     write_compilation_artifacts(bindings, &page_request, page_result, &mut page_progress)
///         .await?;
///     write_compilation_artifacts(bindings, &page_request, page_result, &mut page_progress)
///         .await?;
///     Ok(())
/// }
/// ```
pub fn write_compilation_artifacts<'a, R: OperatorResolver + ?Sized>(
    resolver: &'a R,
    request: &'a CompilationArtifactWriteRequest,
    result: &'a CompilationResult,
    progress: &'a mut CompilationArtifactWriteProgress,
) -> impl Future<Output = Result<CompilationArtifactWriteReceipt, CompilationArtifactWriteError>> + 'a
{
    progress.clear();
    async move {
        if request.compilation_result_identity() != result.result_identity() {
            return Err(compilation_artifact_write_error(
                request,
                None,
                None,
                OpenDalWritePhase::ResultValidation,
                progress,
                CommitCertainty::NotCommitted,
                CompilationArtifactWriteErrorCause::CompilationResultMismatch {
                    expected: request.compilation_result_identity(),
                    actual: result.result_identity(),
                },
            ));
        }

        let mut destinations = Vec::with_capacity(request.artifact_keys().len());
        for (artifact_index, key) in request.artifact_keys().iter().enumerate() {
            let destination = request.destination().compose(key).map_err(|_| {
                compilation_artifact_write_error(
                    request,
                    Some(artifact_index),
                    None,
                    OpenDalWritePhase::DestinationValidation,
                    progress,
                    CommitCertainty::NotCommitted,
                    CompilationArtifactWriteErrorCause::InvalidDestinationPath {
                        artifact_index,
                        key: key.clone(),
                    },
                )
            })?;
            destinations.push(destination);
        }

        let keys = destinations
            .iter()
            .zip(result.artifacts())
            .map(|(destination, artifact)| {
                ExactKey::new(destination.operation_path(), artifact.bytes())
            })
            .collect::<Vec<_>>();
        {
            let mut operation = CompilationArtifactWriteOperation { request, progress };
            write_exact_keys(
                resolver,
                request.destination().binding(),
                request.policy(),
                &keys,
                &mut operation,
            )
            .await?;
        }

        Ok(CompilationArtifactWriteReceipt::new(
            request.compilation_result_identity(),
            progress.clone(),
        ))
    }
}

/// A failure while writing a Compilation Result's exact artifacts through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Compilation Output Artifact write failed for binding {} beneath prefix operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct CompilationArtifactWriteError {
    compilation_result_identity: CanonicalIdentity,
    destination: Location,
    policy: WritePolicy,
    failed_artifact_index: Option<usize>,
    failed_key: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: CompilationArtifactWriteProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<CompilationArtifactWriteErrorCause>,
}

impl CompilationArtifactWriteError {
    pub const fn compilation_result_identity(&self) -> CanonicalIdentity {
        self.compilation_result_identity
    }

    pub const fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> WritePolicy {
        self.policy
    }

    pub const fn failed_artifact_index(&self) -> Option<usize> {
        self.failed_artifact_index
    }

    pub fn failed_key(&self) -> Option<&str> {
        self.failed_key.as_deref()
    }

    pub fn failed_destination_path(&self) -> Option<&str> {
        self.failed_destination_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalWritePhase {
        self.phase
    }

    pub const fn progress(&self) -> &CompilationArtifactWriteProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub const fn cause(&self) -> &CompilationArtifactWriteErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Compilation Output Artifact write failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactWriteErrorCause {
    #[error("the Compilation Result identity mismatched")]
    CompilationResultMismatch {
        expected: CanonicalIdentity,
        actual: CanonicalIdentity,
    },
    #[error("a composed destination path was invalid")]
    InvalidDestinationPath { artifact_index: usize, key: String },
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the write policy is unsupported")]
    UnsupportedPolicy { policy: WritePolicy },
    #[error("an artifact exceeds the advertised object size")]
    UnsupportedObjectSize {
        artifact_index: usize,
        byte_length: u64,
    },
    #[error("a preflight read failed")]
    PreflightRead(#[source] ::opendal::Error),
    #[error("destination bytes conflict")]
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    #[error("a conditional create failed")]
    ConditionalCreate(#[source] ::opendal::Error),
    #[error("race verification failed")]
    RaceVerification(#[source] ::opendal::Error),
    #[error("a direct write failed")]
    DirectWrite(#[source] ::opendal::Error),
}

fn compilation_artifact_write_error(
    request: &CompilationArtifactWriteRequest,
    failed_artifact_index: Option<usize>,
    failed_destination_path: Option<String>,
    phase: OpenDalWritePhase,
    progress: &CompilationArtifactWriteProgress,
    commit_certainty: CommitCertainty,
    cause: CompilationArtifactWriteErrorCause,
) -> CompilationArtifactWriteError {
    let failed_key = failed_artifact_index.map(|index| request.artifact_keys()[index].clone());
    CompilationArtifactWriteError {
        compilation_result_identity: request.compilation_result_identity(),
        destination: request.destination().clone(),
        policy: request.policy(),
        failed_artifact_index,
        failed_key,
        failed_destination_path,
        phase,
        progress: progress.clone(),
        commit_certainty,
        cause: RedactedError::new(cause),
    }
}

macro_rules! workflow_evidence {
    (
        $entry:ident, $progress:ident, $receipt:ident,
        entry { $($entry_field:ident: $entry_type:ty),* $(,)? },
        entry_accessors { $($entry_accessors:item)* },
        progress_accessors { $($progress_accessors:item)* },
        receipt { $($receipt_field:ident: $receipt_type:ty),* $(,)? },
        receipt_accessors { $($receipt_accessors:item)* }
    ) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $entry {
            $($entry_field: $entry_type,)*
            outcome: WriteKeyOutcome,
        }

        impl $entry {
            $($entry_accessors)*

            pub const fn outcome(&self) -> WriteKeyOutcome {
                self.outcome
            }

        }

        #[derive(Clone, Debug, Default, Eq, PartialEq)]
        pub struct $progress {
            completed: Vec<$entry>,
        }

        impl $progress {
            pub const fn new() -> Self {
                Self { completed: Vec::new() }
            }

            $($progress_accessors)*

            pub(crate) fn clear(&mut self) {
                self.completed.clear();
            }

            pub(crate) fn push(&mut self, entry: $entry) {
                self.completed.push(entry);
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $receipt {
            $($receipt_field: $receipt_type,)*
            progress: $progress,
        }

        impl $receipt {
            $($receipt_accessors)*

            pub const fn progress(&self) -> &$progress {
                &self.progress
            }
        }
    };
}

workflow_evidence!(
    PackArchiveWriteEntry,
    PackArchiveWriteProgress,
    PackArchiveWriteReceipt,
    entry { destination_path: String },
    entry_accessors {
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> Option<&PackArchiveWriteEntry> { self.completed.first() }
        pub fn outcome(&self) -> Option<WriteKeyOutcome> {
            self.completed().map(PackArchiveWriteEntry::outcome)
        }
    },
    receipt { destination: Location, policy: WritePolicy },
    receipt_accessors {
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> WritePolicy { self.policy }
        pub fn completed(&self) -> &PackArchiveWriteEntry {
            self.progress.completed().expect("a Pack Archive receipt has one completed entry")
        }
        pub const fn outcome(&self) -> WriteKeyOutcome {
            match self.progress.completed.as_slice() {
                [entry, ..] => entry.outcome,
                [] => panic!("a Pack Archive receipt has one completed entry"),
            }
        }
    }
);

workflow_evidence!(
    PackageCacheArchiveWriteEntry,
    PackageCacheArchiveWriteProgress,
    PackageCacheArchiveWriteReceipt,
    entry { destination_path: String },
    entry_accessors {
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> Option<&PackageCacheArchiveWriteEntry> { self.completed.first() }
        pub fn outcome(&self) -> Option<WriteKeyOutcome> {
            self.completed().map(PackageCacheArchiveWriteEntry::outcome)
        }
    },
    receipt { destination: Location, policy: WritePolicy },
    receipt_accessors {
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> WritePolicy { self.policy }
        pub fn completed(&self) -> &PackageCacheArchiveWriteEntry {
            self.progress.completed().expect("a package-cache archive receipt has one completed entry")
        }
        pub const fn outcome(&self) -> WriteKeyOutcome {
            match self.progress.completed.as_slice() {
                [entry, ..] => entry.outcome,
                [] => panic!("a package-cache archive receipt has one completed entry"),
            }
        }
    }
);

pub(crate) struct ExactKey<'a> {
    path: &'a str,
    bytes: &'a [u8],
}

impl<'a> ExactKey<'a> {
    pub(crate) const fn new(path: &'a str, bytes: &'a [u8]) -> Self {
        Self { path, bytes }
    }
}

#[derive(Debug)]
pub(crate) struct ExactKeyWriteReceipt {
    completed: Vec<ExactKeyWriteEntry>,
}

impl ExactKeyWriteReceipt {
    #[cfg(test)]
    fn completed(&self) -> &[ExactKeyWriteEntry] {
        &self.completed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactKeyWriteEntry {
    pub(crate) index: usize,
    pub(crate) outcome: WriteKeyOutcome,
}

struct ExactKeyWriteFailure {
    phase: OpenDalWritePhase,
    failed_index: Option<usize>,
    failed_path: Option<String>,
    commit_certainty: CommitCertainty,
}

impl ExactKeyWriteFailure {
    fn operation(phase: OpenDalWritePhase) -> Self {
        Self {
            phase,
            failed_index: None,
            failed_path: None,
            commit_certainty: CommitCertainty::NotCommitted,
        }
    }

    fn key(
        phase: OpenDalWritePhase,
        index: usize,
        key: &ExactKey<'_>,
        commit_certainty: CommitCertainty,
    ) -> Self {
        Self {
            phase,
            failed_index: Some(index),
            failed_path: Some(key.path.to_owned()),
            commit_certainty,
        }
    }
}

trait ExactKeyWriteCause: Sized {
    fn resolve_operator(source: BoxError) -> Self;
    fn unsupported_policy(policy: WritePolicy) -> Self;
    fn unsupported_object_size(index: usize, byte_length: u64) -> Self;
    fn preflight_read(source: opendal::Error) -> Self;
    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self;
    fn conditional_create(source: opendal::Error) -> Self;
    fn race_verification(source: opendal::Error) -> Self;
}

trait ExactKeyOverwriteCause: ExactKeyWriteCause {
    fn direct_write(source: opendal::Error) -> Self;
}

trait ExactKeyWriteOperation {
    type Error;
    type Cause: ExactKeyWriteCause;

    fn completed_entry(&mut self, entry: ExactKeyWriteEntry);
    fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error;
}

struct PackArchiveWriteOperation<'a> {
    request: &'a PackArchiveWriteRequest,
    progress: &'a mut PackArchiveWriteProgress,
}

impl ExactKeyWriteOperation for PackArchiveWriteOperation<'_> {
    type Error = PackArchiveWriteError;
    type Cause = PackArchiveWriteErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyWriteEntry) {
        self.progress.push(PackArchiveWriteEntry {
            destination_path: self.request.destination().operation_path().to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error {
        PackArchiveWriteError {
            destination: self.request.destination().clone(),
            policy: self.request.policy(),
            failed_path: failure.failed_path,
            phase: failure.phase,
            progress: self.progress.clone(),
            commit_certainty: failure.commit_certainty,
            cause: RedactedError::new(cause),
        }
    }
}

impl ExactKeyWriteCause for PackArchiveWriteErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: WritePolicy) -> Self {
        Self::UnsupportedPolicy { policy }
    }

    fn unsupported_object_size(_: usize, byte_length: u64) -> Self {
        Self::UnsupportedObjectSize { byte_length }
    }

    fn preflight_read(source: opendal::Error) -> Self {
        Self::PreflightRead(source)
    }

    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self {
        Self::ByteConflict {
            expected_byte_length,
            observed_byte_length_at_least,
        }
    }

    fn conditional_create(source: opendal::Error) -> Self {
        Self::ConditionalCreate(source)
    }

    fn race_verification(source: opendal::Error) -> Self {
        Self::RaceVerification(source)
    }
}

impl ExactKeyOverwriteCause for PackArchiveWriteErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

struct PackageCacheArchiveWriteOperation<'a> {
    request: &'a PackageCacheArchiveWriteRequest,
    progress: &'a mut PackageCacheArchiveWriteProgress,
}

impl ExactKeyWriteOperation for PackageCacheArchiveWriteOperation<'_> {
    type Error = PackageCacheArchiveWriteError;
    type Cause = PackageCacheArchiveWriteErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyWriteEntry) {
        self.progress.push(PackageCacheArchiveWriteEntry {
            destination_path: self.request.destination().operation_path().to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error {
        PackageCacheArchiveWriteError {
            destination: self.request.destination().clone(),
            policy: self.request.policy(),
            failed_path: failure.failed_path,
            phase: failure.phase,
            progress: self.progress.clone(),
            commit_certainty: failure.commit_certainty,
            cause: RedactedError::new(cause),
        }
    }
}

impl ExactKeyWriteCause for PackageCacheArchiveWriteErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: WritePolicy) -> Self {
        Self::UnsupportedPolicy { policy }
    }

    fn unsupported_object_size(_: usize, byte_length: u64) -> Self {
        Self::UnsupportedObjectSize { byte_length }
    }

    fn preflight_read(source: opendal::Error) -> Self {
        Self::PreflightRead(source)
    }

    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self {
        Self::ByteConflict {
            expected_byte_length,
            observed_byte_length_at_least,
        }
    }

    fn conditional_create(source: opendal::Error) -> Self {
        Self::ConditionalCreate(source)
    }

    fn race_verification(source: opendal::Error) -> Self {
        Self::RaceVerification(source)
    }
}

struct PackExtractionWriteOperation<'a> {
    request: &'a PackExtractionWriteRequest,
    plan: &'a crate::PackExtractionPlan,
    progress: &'a mut PackExtractionWriteProgress,
}

impl ExactKeyWriteOperation for PackExtractionWriteOperation<'_> {
    type Error = PackExtractionWriteError;
    type Cause = PackExtractionWriteErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyWriteEntry) {
        let index = entry.index;
        self.progress.push(PackExtractionWriteEntry::new(
            self.plan.entries()[index].relative_path().to_owned(),
            entry.outcome,
        ));
    }

    fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error {
        let failed_relative_path = failure
            .failed_index
            .map(|index| self.plan.entries()[index].relative_path().to_owned());
        pack_extraction_write_error(
            self.request,
            failed_relative_path,
            failure.failed_path,
            failure.phase,
            self.progress,
            failure.commit_certainty,
            cause,
        )
    }
}

impl ExactKeyWriteCause for PackExtractionWriteErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: WritePolicy) -> Self {
        Self::UnsupportedPolicy { policy }
    }

    fn unsupported_object_size(_: usize, byte_length: u64) -> Self {
        Self::UnsupportedObjectSize { byte_length }
    }

    fn preflight_read(source: opendal::Error) -> Self {
        Self::PreflightRead(source)
    }

    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self {
        Self::ByteConflict {
            expected_byte_length,
            observed_byte_length_at_least,
        }
    }

    fn conditional_create(source: opendal::Error) -> Self {
        Self::ConditionalCreate(source)
    }

    fn race_verification(source: opendal::Error) -> Self {
        Self::RaceVerification(source)
    }
}

impl ExactKeyOverwriteCause for PackExtractionWriteErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

struct CompilationArtifactWriteOperation<'a> {
    request: &'a CompilationArtifactWriteRequest,
    progress: &'a mut CompilationArtifactWriteProgress,
}

impl ExactKeyWriteOperation for CompilationArtifactWriteOperation<'_> {
    type Error = CompilationArtifactWriteError;
    type Cause = CompilationArtifactWriteErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyWriteEntry) {
        let artifact_index = entry.index;
        self.progress.push(CompilationArtifactWriteEntry::new(
            artifact_index,
            entry.outcome,
        ));
    }

    fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error {
        compilation_artifact_write_error(
            self.request,
            failure.failed_index,
            failure.failed_path,
            failure.phase,
            self.progress,
            failure.commit_certainty,
            cause,
        )
    }
}

impl ExactKeyWriteCause for CompilationArtifactWriteErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: WritePolicy) -> Self {
        Self::UnsupportedPolicy { policy }
    }

    fn unsupported_object_size(artifact_index: usize, byte_length: u64) -> Self {
        Self::UnsupportedObjectSize {
            artifact_index,
            byte_length,
        }
    }

    fn preflight_read(source: opendal::Error) -> Self {
        Self::PreflightRead(source)
    }

    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self {
        Self::ByteConflict {
            expected_byte_length,
            observed_byte_length_at_least,
        }
    }

    fn conditional_create(source: opendal::Error) -> Self {
        Self::ConditionalCreate(source)
    }

    fn race_verification(source: opendal::Error) -> Self {
        Self::RaceVerification(source)
    }
}

impl ExactKeyOverwriteCause for CompilationArtifactWriteErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

async fn write_exact_keys<R, O>(
    resolver: &R,
    binding: &OperatorBinding,
    policy: WritePolicy,
    keys: &[ExactKey<'_>],
    operation: &mut O,
) -> Result<ExactKeyWriteReceipt, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: ExactKeyWriteOperation,
    O::Cause: ExactKeyOverwriteCause,
{
    if keys.is_empty() {
        return Ok(ExactKeyWriteReceipt {
            completed: Vec::new(),
        });
    }

    let operator = resolver.resolve(binding).map_err(|source| {
        operation.error(
            ExactKeyWriteFailure::operation(OpenDalWritePhase::ResolveOperator),
            O::Cause::resolve_operator(Box::new(source)),
        )
    })?;
    appraise_capabilities(&operator, policy, keys, operation)?;

    let mut completed = Vec::with_capacity(keys.len());
    match policy {
        WritePolicy::OverwriteExactKeys => {
            for (index, key) in keys.iter().enumerate() {
                operator
                    .write(key.path, key.bytes.to_vec())
                    .await
                    .map_err(|source| {
                        operation.error(
                            ExactKeyWriteFailure::key(
                                OpenDalWritePhase::DirectWrite,
                                index,
                                key,
                                CommitCertainty::Indeterminate,
                            ),
                            O::Cause::direct_write(source),
                        )
                    })?;
                let entry = ExactKeyWriteEntry {
                    index,
                    outcome: WriteKeyOutcome::Written,
                };
                operation.completed_entry(entry.clone());
                completed.push(entry);
            }
        }
        WritePolicy::CreateOrVerify => {
            write_create_or_verify(&operator, keys, &mut completed, operation).await?;
        }
    }

    Ok(ExactKeyWriteReceipt { completed })
}

async fn write_create_or_verify_exact_keys<R, O>(
    resolver: &R,
    binding: &OperatorBinding,
    keys: &[ExactKey<'_>],
    operation: &mut O,
) -> Result<ExactKeyWriteReceipt, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: ExactKeyWriteOperation,
{
    if keys.is_empty() {
        return Ok(ExactKeyWriteReceipt {
            completed: Vec::new(),
        });
    }

    let operator = resolver.resolve(binding).map_err(|source| {
        operation.error(
            ExactKeyWriteFailure::operation(OpenDalWritePhase::ResolveOperator),
            O::Cause::resolve_operator(Box::new(source)),
        )
    })?;
    appraise_capabilities(&operator, WritePolicy::CreateOrVerify, keys, operation)?;

    let mut completed = Vec::with_capacity(keys.len());
    write_create_or_verify(&operator, keys, &mut completed, operation).await?;
    Ok(ExactKeyWriteReceipt { completed })
}

fn appraise_capabilities<O: ExactKeyWriteOperation>(
    operator: &opendal::Operator,
    policy: WritePolicy,
    keys: &[ExactKey<'_>],
    operation: &O,
) -> Result<(), O::Error> {
    let capability = operator.info().capability();
    let policy_supported = capability.write
        && (!keys.iter().any(|key| key.bytes.is_empty()) || capability.write_can_empty)
        && (policy != WritePolicy::CreateOrVerify
            || (capability.read && capability.write_with_if_not_exists));
    if !policy_supported {
        return Err(operation.error(
            ExactKeyWriteFailure::operation(OpenDalWritePhase::CapabilityAppraisal),
            O::Cause::unsupported_policy(policy),
        ));
    }
    if let Some(maximum) = capability.write_total_max_size {
        for (index, key) in keys.iter().enumerate() {
            if key.bytes.len() > maximum {
                return Err(operation.error(
                    ExactKeyWriteFailure::key(
                        OpenDalWritePhase::CapabilityAppraisal,
                        index,
                        key,
                        CommitCertainty::NotCommitted,
                    ),
                    O::Cause::unsupported_object_size(index, byte_length(key.bytes)),
                ));
            }
        }
    }
    Ok(())
}

async fn write_create_or_verify<O: ExactKeyWriteOperation>(
    operator: &opendal::Operator,
    keys: &[ExactKey<'_>],
    completed: &mut Vec<ExactKeyWriteEntry>,
    operation: &mut O,
) -> Result<(), O::Error> {
    let mut observations = Vec::with_capacity(keys.len());
    for (index, key) in keys.iter().enumerate() {
        let observation = match compare_object(operator, key.path, key.bytes).await {
            Ok(observation) => observation,
            Err(CompareError::Read {
                source,
                observed_byte_length: 0,
            }) if source.kind() == ErrorKind::NotFound => ExistingObject::Absent,
            Err(CompareError::Read { source, .. }) => {
                return Err(operation.error(
                    ExactKeyWriteFailure::key(
                        OpenDalWritePhase::PreflightRead,
                        index,
                        key,
                        CommitCertainty::NotCommitted,
                    ),
                    O::Cause::preflight_read(source),
                ));
            }
            Err(CompareError::Conflict {
                observed_byte_length_at_least,
            }) => {
                return Err(byte_conflict_error(
                    operation,
                    OpenDalWritePhase::PreflightRead,
                    index,
                    key,
                    observed_byte_length_at_least,
                ));
            }
        };
        if observation == ExistingObject::Matching && completed.len() == index {
            let entry = ExactKeyWriteEntry {
                index,
                outcome: WriteKeyOutcome::AlreadyMatching,
            };
            operation.completed_entry(entry.clone());
            completed.push(entry);
        }
        observations.push(observation);
    }

    for (index, (key, observation)) in keys.iter().zip(observations).enumerate() {
        if index < completed.len() {
            debug_assert_eq!(observation, ExistingObject::Matching);
            continue;
        }
        let outcome = match observation {
            ExistingObject::Matching => WriteKeyOutcome::AlreadyMatching,
            ExistingObject::Absent => {
                match operator
                    .write_with(key.path, key.bytes.to_vec())
                    .if_not_exists(true)
                    .await
                {
                    Ok(_) => WriteKeyOutcome::Created,
                    Err(source)
                        if matches!(
                            source.kind(),
                            ErrorKind::AlreadyExists | ErrorKind::ConditionNotMatch
                        ) =>
                    {
                        match compare_object(operator, key.path, key.bytes).await {
                            Ok(ExistingObject::Matching) => WriteKeyOutcome::AlreadyMatching,
                            Ok(ExistingObject::Absent) => {
                                unreachable!("a successful comparison never reports absence")
                            }
                            Err(CompareError::Read { source, .. }) => {
                                return Err(operation.error(
                                    ExactKeyWriteFailure::key(
                                        OpenDalWritePhase::RaceVerification,
                                        index,
                                        key,
                                        CommitCertainty::NotCommitted,
                                    ),
                                    O::Cause::race_verification(source),
                                ));
                            }
                            Err(CompareError::Conflict {
                                observed_byte_length_at_least,
                            }) => {
                                return Err(byte_conflict_error(
                                    operation,
                                    OpenDalWritePhase::RaceVerification,
                                    index,
                                    key,
                                    observed_byte_length_at_least,
                                ));
                            }
                        }
                    }
                    Err(source) => {
                        return Err(operation.error(
                            ExactKeyWriteFailure::key(
                                OpenDalWritePhase::ConditionalCreate,
                                index,
                                key,
                                CommitCertainty::Indeterminate,
                            ),
                            O::Cause::conditional_create(source),
                        ));
                    }
                }
            }
        };
        let entry = ExactKeyWriteEntry { index, outcome };
        operation.completed_entry(entry.clone());
        completed.push(entry);
    }
    Ok(())
}

fn byte_conflict_error<O: ExactKeyWriteOperation>(
    operation: &O,
    phase: OpenDalWritePhase,
    index: usize,
    key: &ExactKey<'_>,
    observed_byte_length_at_least: u64,
) -> O::Error {
    operation.error(
        ExactKeyWriteFailure::key(phase, index, key, CommitCertainty::NotCommitted),
        O::Cause::byte_conflict(byte_length(key.bytes), observed_byte_length_at_least),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExistingObject {
    Absent,
    Matching,
}

enum CompareError {
    Read {
        source: opendal::Error,
        observed_byte_length: u64,
    },
    Conflict {
        observed_byte_length_at_least: u64,
    },
}

async fn compare_object(
    operator: &opendal::Operator,
    path: &str,
    expected: &[u8],
) -> Result<ExistingObject, CompareError> {
    let expected_byte_length = byte_length(expected);
    let reader = operator
        .reader(path)
        .await
        .map_err(|source| CompareError::Read {
            source,
            observed_byte_length: 0,
        })?;
    let mut stream = reader
        .into_stream(..)
        .await
        .map_err(|source| CompareError::Read {
            source,
            observed_byte_length: 0,
        })?;
    let mut observed = 0u64;

    while let Some(buffer) = stream.next().await {
        let buffer = buffer.map_err(|source| CompareError::Read {
            source,
            observed_byte_length: observed,
        })?;
        for chunk in buffer {
            for byte in chunk {
                if observed == expected_byte_length {
                    return Err(CompareError::Conflict {
                        observed_byte_length_at_least: expected_byte_length
                            .checked_add(1)
                            .expect("an addressable slice is shorter than u64::MAX bytes"),
                    });
                }
                let index = usize::try_from(observed)
                    .expect("observed bytes fit usize while comparing an addressable slice");
                observed = observed
                    .checked_add(1)
                    .expect("an addressable slice is shorter than u64::MAX bytes");
                if expected[index] != byte {
                    return Err(CompareError::Conflict {
                        observed_byte_length_at_least: observed,
                    });
                }
            }
        }
    }

    if observed != expected_byte_length {
        return Err(CompareError::Conflict {
            observed_byte_length_at_least: observed,
        });
    }
    Ok(ExistingObject::Matching)
}

fn byte_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("OpenDAL write supports no 128-bit target")
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::opendal::scripted_service::{
        DestinationMutation, PendingPoint, WriteCapabilities, WriteCondition,
        WriteDroppedOperation, WriteOperationLogEntry, WriteReadScript, WriteReadStep, WriteScript,
        WriteService, WriteStep,
    };
    use crate::opendal::{OperatorBinding, OperatorResolver};
    use crate::pack_archive::CommitCertainty;
    use crate::{
        CompilationLimits, CompilationOutputSpecification, Pack, PackCompilationRequest,
        SvgOutputSpecification, compile_with_limits,
    };

    use super::{
        CompilationArtifactWriteErrorCause, CompilationArtifactWriteProgress,
        CompilationArtifactWriteRequest, ExactKey, ExactKeyOverwriteCause, ExactKeyWriteCause,
        ExactKeyWriteEntry, ExactKeyWriteFailure, ExactKeyWriteOperation, OpenDalWritePhase,
        PackArchiveWriteEntry, PackArchiveWriteProgress, WriteKeyOutcome, WritePolicy,
        write_compilation_artifacts, write_exact_keys,
    };

    #[test]
    fn empty_write_succeeds_without_resolving_an_operator() {
        let resolver = RejectingResolver;
        let binding = binding();
        let mut completed = Vec::new();
        let receipt = {
            let mut operation = TestWriteOperation::new(&mut completed);
            let mut write = pin!(write_exact_keys(
                &resolver,
                &binding,
                WritePolicy::OverwriteExactKeys,
                &[],
                &mut operation,
            ));
            expect_ready(write.as_mut()).unwrap()
        };

        assert!(receipt.completed().is_empty());
        assert!(completed.is_empty());
    }

    #[test]
    fn invalid_composed_artifact_destination_fails_before_resolution() {
        let result = two_artifact_result();
        let request = CompilationArtifactWriteRequest {
            compilation_result_identity: result.result_identity(),
            destination: "destination:/prefix/".parse().unwrap(),
            artifact_keys: vec!["valid.svg".to_owned(), "../alias.svg".to_owned()],
            policy: WritePolicy::OverwriteExactKeys,
        };
        let mut progress = CompilationArtifactWriteProgress::new();

        let error = expect_ready(pin!(write_compilation_artifacts(
            &RejectingResolver,
            &request,
            &result,
            &mut progress,
        )))
        .unwrap_err();

        assert_eq!(error.phase(), OpenDalWritePhase::DestinationValidation);
        assert_eq!(error.failed_artifact_index(), Some(1));
        assert_eq!(error.failed_key(), Some("../alias.svg"));
        assert_eq!(error.failed_destination_path(), None);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(error.progress().completed().is_empty());
        assert!(matches!(
            error.cause(),
            CompilationArtifactWriteErrorCause::InvalidDestinationPath {
                artifact_index: 1,
                key,
            } if key == "../alias.svg"
        ));
    }

    #[test]
    fn overwrite_writes_each_key_once_in_order_without_reading() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [],
            [],
            [
                WriteScript::new("first.bin", WriteCondition::Direct, []),
                WriteScript::new("second.bin", WriteCondition::Direct, []),
            ],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [
            ExactKey::new("first.bin", b"first"),
            ExactKey::new("second.bin", b"second"),
        ];
        let mut completed = Vec::new();

        let receipt = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::OverwriteExactKeys,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap();

        assert_eq!(
            service.destination().object("first.bin"),
            Some(b"first".as_slice())
        );
        assert_eq!(
            service.destination().object("second.bin"),
            Some(b"second".as_slice())
        );
        assert_eq!(completed, receipt.completed());
        assert_eq!(
            completed
                .iter()
                .map(|entry| (entry.index, entry.outcome))
                .collect::<Vec<_>>(),
            [(0, WriteKeyOutcome::Written), (1, WriteKeyOutcome::Written),]
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, WriteOperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn create_or_verify_compares_every_key_before_mutation() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [("conflict.bin".to_owned(), b"wrong".to_vec())],
            [
                WriteReadScript::new(
                    "absent.bin",
                    0,
                    [WriteReadStep::failure(ErrorKind::NotFound)],
                )
                .unwrap(),
                WriteReadScript::new("conflict.bin", 1, [WriteReadStep::chunk(0..5)]).unwrap(),
            ],
            [WriteScript::new(
                "absent.bin",
                WriteCondition::IfNotExists,
                [],
            )],
            32,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [
            ExactKey::new("absent.bin", b"new"),
            ExactKey::new("conflict.bin", b"right"),
        ];
        let mut completed = Vec::new();

        let error = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::CreateOrVerify,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalWritePhase::PreflightRead);
        assert_eq!(error.failed_index, Some(1));
        assert_eq!(error.commit_certainty, CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause,
            TestWriteErrorCause::ByteConflict {
                expected_byte_length: 5,
                observed_byte_length_at_least: 1,
            }
        ));
        assert!(completed.is_empty());
        assert!(service.destination().object("absent.bin").is_none());
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, WriteOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn later_preflight_conflict_retains_the_leading_matching_prefix() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [
                ("matching.bin".to_owned(), b"matching".to_vec()),
                ("conflict.bin".to_owned(), b"wrong".to_vec()),
            ],
            [
                WriteReadScript::new("matching.bin", 1, [WriteReadStep::chunk(0..8)]).unwrap(),
                WriteReadScript::new("conflict.bin", 1, [WriteReadStep::chunk(0..5)]).unwrap(),
            ],
            [],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [
            ExactKey::new("matching.bin", b"matching"),
            ExactKey::new("conflict.bin", b"right"),
        ];
        let mut completed = Vec::new();

        let error = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::CreateOrVerify,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalWritePhase::PreflightRead);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(completed[0].outcome, WriteKeyOutcome::AlreadyMatching);
    }

    #[test]
    fn mutable_matching_stream_is_read_only_evidence_without_commit_certainty() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [("mutable.bin".to_owned(), b"abcdef".to_vec())],
            [WriteReadScript::new(
                "mutable.bin",
                2,
                [
                    WriteReadStep::chunk(0..3),
                    WriteReadStep::mutate(DestinationMutation::set("mutable.bin", b"abcXYZ")),
                    WriteReadStep::chunk(3..6),
                ],
            )
            .unwrap()],
            [],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("mutable.bin", b"abcXYZ")];
        let mut completed = Vec::new();

        let receipt = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::CreateOrVerify,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap();

        assert_eq!(
            receipt.completed()[0].outcome,
            WriteKeyOutcome::AlreadyMatching
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, WriteOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn disappearance_after_a_partial_stream_is_not_treated_as_absence() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [("unstable.bin".to_owned(), b"planned".to_vec())],
            [WriteReadScript::new(
                "unstable.bin",
                1,
                [
                    WriteReadStep::chunk(0..3),
                    WriteReadStep::failure(ErrorKind::NotFound),
                ],
            )
            .unwrap()],
            [WriteScript::new(
                "unstable.bin",
                WriteCondition::IfNotExists,
                [],
            )],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("unstable.bin", b"planned")];
        let mut completed = Vec::new();

        let error = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::CreateOrVerify,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalWritePhase::PreflightRead);
        assert!(matches!(
            error.cause,
            TestWriteErrorCause::PreflightRead(ref source)
                if source.kind() == ErrorKind::NotFound
        ));
        assert!(completed.is_empty());
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, WriteOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn conditional_conflict_performs_one_bounded_verification() {
        let pending = PendingPoint::new();
        let service = WriteService::new(
            WriteCapabilities::all(),
            [],
            [
                WriteReadScript::new("race.bin", 0, [WriteReadStep::failure(ErrorKind::NotFound)])
                    .unwrap(),
                WriteReadScript::new("race.bin", 1, [WriteReadStep::chunk(0..7)]).unwrap(),
            ],
            [WriteScript::new(
                "race.bin",
                WriteCondition::IfNotExists,
                [WriteStep::pending(pending.clone()), WriteStep::commit()],
            )],
            32,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("race.bin", b"planned")];
        let mut completed = Vec::new();
        let binding = binding();
        let receipt = {
            let mut operation = TestWriteOperation::new(&mut completed);
            let mut write = pin!(write_exact_keys(
                &resolver,
                &binding,
                WritePolicy::CreateOrVerify,
                &keys,
                &mut operation,
            ));

            assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
            service.mutate(DestinationMutation::set("race.bin", b"planned"));
            pending.release();
            expect_ready(write.as_mut()).unwrap()
        };

        assert_eq!(
            receipt.completed()[0].outcome,
            WriteKeyOutcome::AlreadyMatching
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, WriteOperationLogEntry::ReadInvoked { .. }))
                .count(),
            2
        );
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn appraisal_rejects_capabilities_and_sizes_before_effects() {
        let cases = [
            WriteCapabilities {
                write: false,
                write_can_empty: true,
                write_with_if_not_exists: true,
                read: true,
                write_total_max_size: None,
            },
            WriteCapabilities {
                write: true,
                write_can_empty: false,
                write_with_if_not_exists: true,
                read: true,
                write_total_max_size: None,
            },
            WriteCapabilities {
                write: true,
                write_can_empty: true,
                write_with_if_not_exists: false,
                read: true,
                write_total_max_size: None,
            },
            WriteCapabilities {
                write: true,
                write_can_empty: true,
                write_with_if_not_exists: true,
                read: false,
                write_total_max_size: None,
            },
        ];
        for capabilities in cases {
            let service = WriteService::new(capabilities, [], [], [], 4);
            let resolver = ServiceResolver(service.operator());
            let keys = [ExactKey::new("empty.bin", b"")];
            let mut completed = Vec::new();

            let error = expect_ready(pin!(write_exact_keys(
                &resolver,
                &binding(),
                WritePolicy::CreateOrVerify,
                &keys,
                &mut TestWriteOperation::new(&mut completed),
            )))
            .unwrap_err();

            assert_eq!(error.phase, OpenDalWritePhase::CapabilityAppraisal);
            assert!(matches!(
                error.cause,
                TestWriteErrorCause::UnsupportedPolicy { .. }
            ));
            assert!(service.log().entries().is_empty());
        }

        let service = WriteService::new(
            WriteCapabilities {
                write_total_max_size: Some(3),
                ..WriteCapabilities::all()
            },
            [],
            [],
            [],
            4,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("large.bin", b"four")];
        let mut completed = Vec::new();
        let error = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::OverwriteExactKeys,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert!(matches!(
            error.cause,
            TestWriteErrorCause::UnsupportedObjectSize { byte_length: 4 }
        ));
        assert!(service.log().entries().is_empty());
    }

    #[test]
    fn issued_write_failure_is_indeterminate_and_retains_the_completed_prefix() {
        let service = WriteService::new(
            WriteCapabilities::all(),
            [],
            [],
            [
                WriteScript::new("first.bin", WriteCondition::Direct, []),
                WriteScript::write_failure(
                    "second.bin",
                    WriteCondition::Direct,
                    ErrorKind::Unexpected,
                ),
            ],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [
            ExactKey::new("first.bin", b"first"),
            ExactKey::new("second.bin", b"second"),
        ];
        let mut completed = Vec::new();

        let error = expect_ready(pin!(write_exact_keys(
            &resolver,
            &binding(),
            WritePolicy::OverwriteExactKeys,
            &keys,
            &mut TestWriteOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalWritePhase::DirectWrite);
        assert_eq!(error.failed_index, Some(1));
        assert_eq!(error.failed_path.as_deref(), Some("second.bin"));
        assert_eq!(error.commit_certainty, CommitCertainty::Indeterminate);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
    }

    #[test]
    fn dropping_a_pending_write_leaves_the_completed_prefix_with_the_caller() {
        let pending = PendingPoint::new();
        let service = WriteService::new(
            WriteCapabilities::all(),
            [],
            [],
            [
                WriteScript::new("first.bin", WriteCondition::Direct, []),
                WriteScript::new(
                    "second.bin",
                    WriteCondition::Direct,
                    [WriteStep::pending(pending.clone())],
                ),
            ],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let binding = binding();
        let keys = [
            ExactKey::new("first.bin", b"first"),
            ExactKey::new("second.bin", b"second"),
        ];
        let mut completed = Vec::new();
        {
            let mut operation = TestWriteOperation::new(&mut completed);
            let mut write = pin!(write_exact_keys(
                &resolver,
                &binding,
                WritePolicy::OverwriteExactKeys,
                &keys,
                &mut operation,
            ));
            assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(
            service.cancellations(),
            [WriteDroppedOperation::Write {
                id: 1,
                path: "second.bin".to_owned(),
                length: 6,
                condition: WriteCondition::Direct,
                issued: true,
            }]
        );
    }

    #[test]
    fn dropping_a_later_preflight_read_retains_the_leading_matching_prefix() {
        let pending = PendingPoint::new();
        let service = WriteService::new(
            WriteCapabilities::all(),
            [("matching.bin".to_owned(), b"matching".to_vec())],
            [
                WriteReadScript::new("matching.bin", 1, [WriteReadStep::chunk(0..8)]).unwrap(),
                WriteReadScript::new("pending.bin", 0, [WriteReadStep::pending(pending.clone())])
                    .unwrap(),
            ],
            [],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let binding = binding();
        let keys = [
            ExactKey::new("matching.bin", b"matching"),
            ExactKey::new("pending.bin", b"pending"),
        ];
        let mut completed = Vec::new();
        {
            let mut operation = TestWriteOperation::new(&mut completed);
            let mut write = pin!(write_exact_keys(
                &resolver,
                &binding,
                WritePolicy::CreateOrVerify,
                &keys,
                &mut operation,
            ));
            assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(completed[0].outcome, WriteKeyOutcome::AlreadyMatching);
    }

    #[test]
    fn workflow_evidence_retains_observed_outcomes() {
        let mut progress = PackArchiveWriteProgress::new();
        progress.push(PackArchiveWriteEntry {
            destination_path: "archive.typk".to_owned(),
            outcome: WriteKeyOutcome::AlreadyMatching,
        });

        assert_eq!(progress.outcome(), Some(WriteKeyOutcome::AlreadyMatching));

        progress.clear();
        progress.push(PackArchiveWriteEntry {
            destination_path: "archive.typk".to_owned(),
            outcome: WriteKeyOutcome::Created,
        });
        assert_eq!(progress.outcome(), Some(WriteKeyOutcome::Created));
    }

    #[derive(Debug)]
    struct TestWriteError {
        phase: OpenDalWritePhase,
        failed_index: Option<usize>,
        failed_path: Option<String>,
        commit_certainty: CommitCertainty,
        cause: TestWriteErrorCause,
    }

    #[derive(Debug)]
    enum TestWriteErrorCause {
        ResolveOperator(crate::opendal::BoxError),
        UnsupportedPolicy {
            policy: WritePolicy,
        },
        UnsupportedObjectSize {
            byte_length: u64,
        },
        PreflightRead(opendal::Error),
        ByteConflict {
            expected_byte_length: u64,
            observed_byte_length_at_least: u64,
        },
        ConditionalCreate(opendal::Error),
        RaceVerification(opendal::Error),
        DirectWrite(opendal::Error),
    }

    impl ExactKeyWriteCause for TestWriteErrorCause {
        fn resolve_operator(source: crate::opendal::BoxError) -> Self {
            Self::ResolveOperator(source)
        }

        fn unsupported_policy(policy: WritePolicy) -> Self {
            Self::UnsupportedPolicy { policy }
        }

        fn unsupported_object_size(_: usize, byte_length: u64) -> Self {
            Self::UnsupportedObjectSize { byte_length }
        }

        fn preflight_read(source: opendal::Error) -> Self {
            Self::PreflightRead(source)
        }

        fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self {
            Self::ByteConflict {
                expected_byte_length,
                observed_byte_length_at_least,
            }
        }

        fn conditional_create(source: opendal::Error) -> Self {
            Self::ConditionalCreate(source)
        }

        fn race_verification(source: opendal::Error) -> Self {
            Self::RaceVerification(source)
        }
    }

    impl ExactKeyOverwriteCause for TestWriteErrorCause {
        fn direct_write(source: opendal::Error) -> Self {
            Self::DirectWrite(source)
        }
    }

    struct TestWriteOperation<'a> {
        completed: &'a mut Vec<ExactKeyWriteEntry>,
    }

    impl<'a> TestWriteOperation<'a> {
        fn new(completed: &'a mut Vec<ExactKeyWriteEntry>) -> Self {
            Self { completed }
        }
    }

    impl ExactKeyWriteOperation for TestWriteOperation<'_> {
        type Error = TestWriteError;
        type Cause = TestWriteErrorCause;

        fn completed_entry(&mut self, entry: ExactKeyWriteEntry) {
            self.completed.push(entry);
        }

        fn error(&self, failure: ExactKeyWriteFailure, cause: Self::Cause) -> Self::Error {
            TestWriteError {
                phase: failure.phase,
                failed_index: failure.failed_index,
                failed_path: failure.failed_path,
                commit_certainty: failure.commit_certainty,
                cause,
            }
        }
    }

    fn expect_ready<F: Future>(future: std::pin::Pin<&mut F>) -> F::Output {
        match poll_once(future) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    fn binding() -> OperatorBinding {
        OperatorBinding::new("destination").unwrap()
    }

    fn two_artifact_result() -> crate::CompilationResult {
        let pack = Pack::builder("main.typ")
            .file(
                "main.typ",
                b"composition validation\n#pagebreak()\nsecond page".to_vec(),
            )
            .unwrap()
            .build()
            .unwrap();
        compile_with_limits(
            PackCompilationRequest::new(
                pack,
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap()
        .result()
        .unwrap()
        .clone()
    }

    struct ServiceResolver(opendal::Operator);

    impl OperatorResolver for ServiceResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            Ok(self.0.clone())
        }
    }

    struct RejectingResolver;

    impl OperatorResolver for RejectingResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            panic!("an empty write must not resolve an operator")
        }
    }
}
