//! OpenDAL exact-key publication vocabulary and crate-private execution.

use std::{collections::BTreeMap, future::Future};

use futures_util::StreamExt;
use opendal::ErrorKind;

use super::location::validate_decoded_artifact_key_path;
use super::{
    BoxError, Location, LocationError, LocationRoleError, OperatorBinding, OperatorResolver,
};
use crate::pack_archive::CommitCertainty;
use crate::redacted_error::RedactedError;
use crate::{CompilationResult, CompilationResultIdentity, CompilationStatus, PackArchiveBytes};

/// The exact-key conflict policy for an OpenDAL publication operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublicationPolicy {
    /// Create absent objects and accept existing objects only when their bytes match.
    CreateOrVerify,
    /// Write every exact key without inspecting its existing value.
    OverwriteExactKeys,
}

/// The outcome observed for one successfully completed exact key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublicationKeyOutcome {
    Created,
    AlreadyMatching,
    Written,
}

impl PublicationKeyOutcome {
    /// Commit Certainty for the destination effect represented by this outcome.
    pub const fn commit_certainty(self) -> Option<CommitCertainty> {
        match self {
            Self::AlreadyMatching => None,
            Self::Created | Self::Written => Some(CommitCertainty::Committed),
        }
    }
}

/// The OpenDAL adapter phase reached by a publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OpenDalPublicationPhase {
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

/// A validated request to publish one exact Pack Archive object.
#[derive(Clone, Debug)]
pub struct PackArchivePublicationRequest {
    destination: Location,
    policy: PublicationPolicy,
}

impl PackArchivePublicationRequest {
    /// Validates an exact-object destination and retains the explicit policy.
    pub fn new(
        destination: Location,
        policy: PublicationPolicy,
    ) -> Result<Self, PackArchivePublicationRequestError> {
        destination.require_object().map_err(|source| {
            PackArchivePublicationRequestError::InvalidDestinationRole {
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

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }
}

/// A reason a Pack Archive publication request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchivePublicationRequestError {
    #[error("Pack Archive destination {location} is not an exact object: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Publishes exact borrowed Pack Archive bytes to one normalized object.
///
/// Dropping the returned future yields no receipt, and already-issued storage
/// work may have occurred. The caller retains `archive`; full replay with the
/// same exact bytes is the recovery contract.
///
/// ```no_run
/// use typst_pack::{Pack, PackArchiveBytes};
/// use typst_pack::opendal::{Location, OperatorBindings};
/// use typst_pack::opendal::pack_archive::{
///     PackArchiveAcquisitionRequest, acquire_pack_archive,
/// };
/// use typst_pack::opendal::publication::{
///     PackArchivePublicationRequest, PublicationPolicy, publish_pack_archive,
/// };
/// use typst_pack::pack_archive::{AcquisitionLimits, DecodeError, DecodeLimits, decode};
///
/// enum PublishThenAcquireOutcome {
///     Matching {
///         acquired: PackArchiveBytes,
///         decoded: Result<Pack, DecodeError>,
///     },
///     DestinationChanged {
///         acquired: PackArchiveBytes,
///     },
/// }
///
/// async fn publish_replay_and_acquire(
///     bindings: &OperatorBindings,
///     destination: Location,
///     archive: &PackArchiveBytes,
/// ) -> Result<PublishThenAcquireOutcome, Box<dyn std::error::Error>> {
///     let overwrite = PackArchivePublicationRequest::new(
///         destination.clone(),
///         PublicationPolicy::OverwriteExactKeys,
///     )?;
///     publish_pack_archive(bindings, &overwrite, archive).await?;
///
///     let replay = PackArchivePublicationRequest::new(
///         destination.clone(),
///         PublicationPolicy::CreateOrVerify,
///     )?;
///     publish_pack_archive(bindings, &replay, archive).await?;
///     publish_pack_archive(bindings, &replay, archive).await?;
///
///     let acquisition = PackArchiveAcquisitionRequest::new(
///         destination,
///         AcquisitionLimits::reference_v1(),
///     )?;
///     let acquired = acquire_pack_archive(bindings, &acquisition).await?;
///
///     // The caller still owns `archive`; preserve the independently acquired
///     // bytes and do not decode when the mutable destination changed.
///     if archive.as_slice() != acquired.as_slice() {
///         return Ok(PublishThenAcquireOutcome::DestinationChanged { acquired });
///     }
///
///     let decoded = decode(&acquired, DecodeLimits::reference_v1());
///     Ok(PublishThenAcquireOutcome::Matching { acquired, decoded })
/// }
/// ```
pub async fn publish_pack_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackArchivePublicationRequest,
    archive: &PackArchiveBytes,
) -> Result<PackArchivePublicationReceipt, PackArchivePublicationError> {
    let mut progress = PackArchivePublicationProgress::new();
    let destination_path = request.destination().operation_path();
    let keys = [ExactKey::new(destination_path, archive.as_slice())];
    {
        let mut operation = PackArchivePublicationOperation {
            request,
            progress: &mut progress,
        };
        publish_exact_keys(
            resolver,
            request.destination().binding(),
            request.policy(),
            &keys,
            &mut operation,
        )
        .await?;
    }

    Ok(PackArchivePublicationReceipt {
        destination: request.destination().clone(),
        policy: request.policy(),
        progress,
    })
}

/// A failure while publishing exact Pack Archive bytes through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive publication failed for binding {} at exact-object operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackArchivePublicationError {
    destination: Location,
    policy: PublicationPolicy,
    failed_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: PackArchivePublicationProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackArchivePublicationErrorCause>,
}

impl PackArchivePublicationError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }

    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalPublicationPhase {
        self.phase
    }

    pub fn progress(&self) -> &PackArchivePublicationProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackArchivePublicationErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Archive publication failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackArchivePublicationErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the publication policy is unsupported")]
    UnsupportedPolicy { policy: PublicationPolicy },
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

/// A validated request to publish caller-supplied bytes to one package-cache object.
///
/// This request fixes [`PublicationPolicy::CreateOrVerify`]. It does not offer a
/// replacement mode and does not represent Package Archive Expansion or Package
/// Catalog insertion.
#[derive(Clone, Debug)]
pub struct PackageCacheArchivePublicationRequest {
    destination: Location,
}

impl PackageCacheArchivePublicationRequest {
    /// Validates and retains a normalized exact-object cache destination.
    pub fn new(destination: Location) -> Result<Self, PackageCacheArchivePublicationRequestError> {
        destination.require_object().map_err(|source| {
            PackageCacheArchivePublicationRequestError::InvalidDestinationRole {
                location: destination.clone(),
                source,
            }
        })?;

        Ok(Self { destination })
    }

    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> PublicationPolicy {
        PublicationPolicy::CreateOrVerify
    }
}

/// A reason a package-cache archive publication request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageCacheArchivePublicationRequestError {
    #[error("package-cache archive destination {location} is not an exact object: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// Publishes caller-supplied exact archive bytes to one package-cache object.
///
/// This low-level operation does not expand the archive, validate a Package
/// Tree, or insert it into a Package Catalog. Direct use with unvalidated bytes
/// can poison a cache because a present malformed cache candidate is terminal.
/// Callers should publish registry bytes only after successful expansion,
/// validation, and insertion.
///
/// Dropping the returned future yields no receipt, and already-issued storage
/// work may have occurred. The caller retains `archive`; full replay with the
/// same exact bytes is the recovery contract.
///
/// ```no_run
/// # #[cfg(feature = "package-acquisition")]
/// # mod example {
/// use std::error::Error;
/// use typst_pack::{
///     PackageAcquisitionFailures, PackageCatalog, PackageDisposition,
///     PackageExpansionLimits,
/// };
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::pack_assembly::{
///     PackageAcquisition, RegistryArchiveResidue, insert_acquired_package,
/// };
/// use typst_pack::opendal::publication::{
///     PackageCacheArchivePublicationRequest, publish_package_cache_archive,
/// };
///
/// async fn insert_then_publish_registry_archive(
///     bindings: &OperatorBindings,
///     catalog: &mut PackageCatalog,
///     failures: &mut PackageAcquisitionFailures,
///     acquisition: PackageAcquisition,
/// ) -> Result<Option<RegistryArchiveResidue>, Box<dyn Error>> {
///     let Some(residue) = insert_acquired_package(
///         catalog,
///         failures,
///         acquisition,
///         PackageDisposition::Embedded,
///         PackageExpansionLimits::reference_v1(),
///     )? else {
///         return Ok(None);
///     };
///
///     let request = PackageCacheArchivePublicationRequest::new(
///         residue.destination().clone(),
///     )?;
///     if let Err(cache_failure) =
///         publish_package_cache_archive(bindings, &request, residue.bytes()).await
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
pub async fn publish_package_cache_archive<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageCacheArchivePublicationRequest,
    archive: &[u8],
) -> Result<PackageCacheArchivePublicationReceipt, PackageCacheArchivePublicationError> {
    let mut progress = PackageCacheArchivePublicationProgress::new();
    let destination_path = request.destination().operation_path();
    let keys = [ExactKey::new(destination_path, archive)];
    {
        let mut operation = PackageCacheArchivePublicationOperation {
            request,
            progress: &mut progress,
        };
        publish_create_or_verify_exact_keys(
            resolver,
            request.destination().binding(),
            &keys,
            &mut operation,
        )
        .await?;
    }

    Ok(PackageCacheArchivePublicationReceipt {
        destination: request.destination().clone(),
        policy: request.policy(),
        progress,
    })
}

/// A failure while publishing caller-supplied package-cache archive bytes.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "package-cache archive publication failed for binding {} at exact-object operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackageCacheArchivePublicationError {
    destination: Location,
    policy: PublicationPolicy,
    failed_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: PackageCacheArchivePublicationProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackageCacheArchivePublicationErrorCause>,
}

impl PackageCacheArchivePublicationError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }

    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalPublicationPhase {
        self.phase
    }

    pub fn progress(&self) -> &PackageCacheArchivePublicationProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackageCacheArchivePublicationErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL package-cache archive publication failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageCacheArchivePublicationErrorCause {
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the publication policy is unsupported")]
    UnsupportedPolicy { policy: PublicationPolicy },
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

/// A validated request to publish one Pack Extraction Plan beneath a prefix.
#[derive(Clone, Debug)]
pub struct PackExtractionPublicationRequest {
    destination: Location,
    policy: PublicationPolicy,
}

impl PackExtractionPublicationRequest {
    /// Validates a normalized prefix destination and retains the explicit policy.
    pub fn new(
        destination: Location,
        policy: PublicationPolicy,
    ) -> Result<Self, PackExtractionPublicationRequestError> {
        destination.require_prefix().map_err(|source| {
            PackExtractionPublicationRequestError::InvalidDestinationRole {
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

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }
}

/// A reason a Pack Extraction publication request cannot be accepted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackExtractionPublicationRequestError {
    #[error("Pack Extraction destination {location} is not a prefix: {source}")]
    InvalidDestinationRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// A validated request to publish every artifact in one succeeded Compilation Result.
#[derive(Clone, Debug)]
pub struct CompilationArtifactPublicationRequest {
    compilation_result_identity: CompilationResultIdentity,
    destination: Location,
    artifact_keys: Vec<String>,
    policy: PublicationPolicy,
}

impl CompilationArtifactPublicationRequest {
    /// Validates a prefix destination and one decoded relative key per canonical artifact.
    pub fn new(
        result: &CompilationResult,
        destination: Location,
        artifact_keys: impl IntoIterator<Item = impl Into<String>>,
        policy: PublicationPolicy,
    ) -> Result<Self, CompilationArtifactPublicationRequestRejection> {
        let compilation_result_identity = result.result_identity();
        let artifact_keys = artifact_keys
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let mut issues = Vec::new();

        if result.status() != CompilationStatus::Succeeded {
            issues.push(CompilationArtifactPublicationRequestIssue::ResultNotSucceeded);
        }
        if let Err(source) = destination.require_prefix() {
            issues.push(
                CompilationArtifactPublicationRequestIssue::InvalidDestinationRole {
                    location: destination.clone(),
                    source,
                },
            );
        }
        if result.artifacts().len() != artifact_keys.len() {
            issues.push(
                CompilationArtifactPublicationRequestIssue::ArtifactKeyCountMismatch {
                    expected: result.artifacts().len(),
                    actual: artifact_keys.len(),
                },
            );
        }
        let mut first_indices = BTreeMap::new();
        for (artifact_index, key) in artifact_keys.iter().enumerate() {
            if let Err(reason) = validate_artifact_key(key) {
                issues.push(
                    CompilationArtifactPublicationRequestIssue::InvalidArtifactKey {
                        artifact_index,
                        key: key.clone(),
                        reason,
                    },
                );
            }
            if let Some(&first_artifact_index) = first_indices.get(key) {
                issues.push(
                    CompilationArtifactPublicationRequestIssue::DuplicateArtifactKey {
                        key: key.clone(),
                        first_artifact_index,
                        duplicate_artifact_index: artifact_index,
                    },
                );
            } else {
                first_indices.insert(key.clone(), artifact_index);
            }
        }

        if !issues.is_empty() {
            return Err(CompilationArtifactPublicationRequestRejection {
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

    pub const fn compilation_result_identity(&self) -> CompilationResultIdentity {
        self.compilation_result_identity
    }

    pub const fn destination(&self) -> &Location {
        &self.destination
    }

    pub fn artifact_keys(&self) -> &[String] {
        &self.artifact_keys
    }

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }
}

/// Complete deterministic rejection of a Compilation Output Artifact publication request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "Compilation Output Artifact publication request rejected for binding {} beneath prefix operation path {:?} with {} issue(s)",
    .destination.binding(),
    .destination.operation_path(),
    .issues.len(),
)]
pub struct CompilationArtifactPublicationRequestRejection {
    compilation_result_identity: CompilationResultIdentity,
    destination: Location,
    issues: Box<[CompilationArtifactPublicationRequestIssue]>,
}

impl CompilationArtifactPublicationRequestRejection {
    pub const fn compilation_result_identity(&self) -> CompilationResultIdentity {
        self.compilation_result_identity
    }

    pub fn issues(&self) -> &[CompilationArtifactPublicationRequestIssue] {
        &self.issues
    }
}

/// One independently detectable issue in a Compilation Output Artifact publication request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactPublicationRequestIssue {
    #[error("a rejected Compilation Result cannot be published")]
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

/// Publishes every entry in one Pack Extraction Plan beneath the request's prefix.
///
/// The caller-owned progress is cleared synchronously before the returned future
/// can be polled or dropped. Replaying the same plan with `CreateOrVerify`
/// accepts objects whose bytes already match.
///
/// ```no_run
/// use typst_pack::PackExtractionPlan;
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::publication::{
///     PackExtractionPublicationProgress, PackExtractionPublicationRequest,
///     PublicationPolicy, publish_pack_extraction_plan,
/// };
///
/// async fn publish_and_replay_partial_attempt(
///     bindings: &OperatorBindings,
///     plan: &PackExtractionPlan,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let request = PackExtractionPublicationRequest::new(
///         "project:/extracted/".parse()?,
///         PublicationPolicy::CreateOrVerify,
///     )?;
///     let mut progress = PackExtractionPublicationProgress::new();
///
///     if let Err(error) =
///         publish_pack_extraction_plan(bindings, &request, plan, &mut progress).await
///     {
///         // The caller retains the exact completed prefix after a partial attempt.
///         assert_eq!(error.progress(), &progress);
///         publish_pack_extraction_plan(bindings, &request, plan, &mut progress).await?;
///     }
///
///     Ok(())
/// }
/// ```
pub fn publish_pack_extraction_plan<'a, R: OperatorResolver + ?Sized>(
    resolver: &'a R,
    request: &'a PackExtractionPublicationRequest,
    plan: &'a crate::PackExtractionPlan,
    progress: &'a mut PackExtractionPublicationProgress,
) -> impl Future<Output = Result<PackExtractionPublicationReceipt, PackExtractionPublicationError>> + 'a
{
    progress.clear();
    async move {
        let mut destinations = Vec::with_capacity(plan.entries().len());
        for entry in plan.entries() {
            let destination = request
                .destination()
                .compose(entry.relative_path())
                .map_err(|_| {
                    pack_extraction_publication_error(
                        request,
                        Some(entry.relative_path().to_owned()),
                        None,
                        OpenDalPublicationPhase::DestinationValidation,
                        progress,
                        CommitCertainty::NotCommitted,
                        PackExtractionPublicationErrorCause::InvalidDestinationPath {
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
            let mut operation = PackExtractionPublicationOperation {
                request,
                plan,
                destinations: &destinations,
                progress,
            };
            publish_exact_keys(
                resolver,
                request.destination().binding(),
                request.policy(),
                &keys,
                &mut operation,
            )
            .await?;
        }

        Ok(PackExtractionPublicationReceipt {
            destination: request.destination().clone(),
            policy: request.policy(),
            pack_identity: *plan.pack_identity(),
            progress: progress.clone(),
        })
    }
}

/// A failure while publishing a Pack Extraction Plan through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Extraction publication failed for binding {} beneath prefix operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct PackExtractionPublicationError {
    destination: Location,
    policy: PublicationPolicy,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: PackExtractionPublicationProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<PackExtractionPublicationErrorCause>,
}

impl PackExtractionPublicationError {
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> PublicationPolicy {
        self.policy
    }

    pub fn failed_relative_path(&self) -> Option<&str> {
        self.failed_relative_path.as_deref()
    }

    pub fn failed_destination_path(&self) -> Option<&str> {
        self.failed_destination_path.as_deref()
    }

    pub const fn phase(&self) -> OpenDalPublicationPhase {
        self.phase
    }

    pub fn progress(&self) -> &PackExtractionPublicationProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub fn cause(&self) -> &PackExtractionPublicationErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Pack Extraction publication failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackExtractionPublicationErrorCause {
    #[error("a composed destination path was invalid")]
    InvalidDestinationPath { relative_path: String },
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the publication policy is unsupported")]
    UnsupportedPolicy { policy: PublicationPolicy },
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

fn pack_extraction_publication_error(
    request: &PackExtractionPublicationRequest,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: &PackExtractionPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: PackExtractionPublicationErrorCause,
) -> PackExtractionPublicationError {
    PackExtractionPublicationError {
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

/// Publishes every canonical artifact beneath the request's normalized prefix.
///
/// The caller-owned progress is cleared synchronously before the returned future
/// can be polled or dropped. Replaying the same result with `CreateOrVerify`
/// accepts objects whose bytes already match.
///
/// ```no_run
/// use typst_pack::CompilationResult;
/// use typst_pack::opendal::OperatorBindings;
/// use typst_pack::opendal::publication::{
///     CompilationArtifactPublicationProgress, CompilationArtifactPublicationRequest,
///     PublicationPolicy, publish_compilation_artifacts,
/// };
///
/// async fn publish_and_replay(
///     bindings: &OperatorBindings,
///     document_result: &CompilationResult,
///     page_result: &CompilationResult,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let document_request = CompilationArtifactPublicationRequest::new(
///         document_result,
///         "artifacts:/document/".parse()?,
///         ["document.pdf"],
///         PublicationPolicy::CreateOrVerify,
///     )?;
///     let page_keys = page_result
///         .artifacts()
///         .iter()
///         .map(|artifact| format!("page-{}.svg", artifact.source_page_number().unwrap()))
///         .collect::<Vec<_>>();
///     let page_request = CompilationArtifactPublicationRequest::new(
///         page_result,
///         "artifacts:/pages/".parse()?,
///         page_keys,
///         PublicationPolicy::CreateOrVerify,
///     )?;
///
///     let mut document_progress = CompilationArtifactPublicationProgress::new();
///     publish_compilation_artifacts(
///         bindings,
///         &document_request,
///         document_result,
///         &mut document_progress,
///     )
///     .await?;
///     publish_compilation_artifacts(
///         bindings,
///         &document_request,
///         document_result,
///         &mut document_progress,
///     )
///     .await?;
///
///     let mut page_progress = CompilationArtifactPublicationProgress::new();
///     publish_compilation_artifacts(bindings, &page_request, page_result, &mut page_progress)
///         .await?;
///     publish_compilation_artifacts(bindings, &page_request, page_result, &mut page_progress)
///         .await?;
///     Ok(())
/// }
/// ```
pub fn publish_compilation_artifacts<'a, R: OperatorResolver + ?Sized>(
    resolver: &'a R,
    request: &'a CompilationArtifactPublicationRequest,
    result: &'a CompilationResult,
    progress: &'a mut CompilationArtifactPublicationProgress,
) -> impl Future<
    Output = Result<CompilationArtifactPublicationReceipt, CompilationArtifactPublicationError>,
> + 'a {
    progress.clear();
    async move {
        if request.compilation_result_identity() != result.result_identity() {
            return Err(compilation_artifact_publication_error(
                request,
                None,
                None,
                OpenDalPublicationPhase::ResultValidation,
                progress,
                CommitCertainty::NotCommitted,
                CompilationArtifactPublicationErrorCause::CompilationResultMismatch {
                    expected: request.compilation_result_identity(),
                    actual: result.result_identity(),
                },
            ));
        }

        let mut destinations = Vec::with_capacity(request.artifact_keys().len());
        for (artifact_index, key) in request.artifact_keys().iter().enumerate() {
            let destination = request.destination().compose(key).map_err(|_| {
                compilation_artifact_publication_error(
                    request,
                    Some(artifact_index),
                    None,
                    OpenDalPublicationPhase::DestinationValidation,
                    progress,
                    CommitCertainty::NotCommitted,
                    CompilationArtifactPublicationErrorCause::InvalidDestinationPath {
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
            let mut operation = CompilationArtifactPublicationOperation {
                request,
                destinations: &destinations,
                progress,
            };
            publish_exact_keys(
                resolver,
                request.destination().binding(),
                request.policy(),
                &keys,
                &mut operation,
            )
            .await?;
        }

        Ok(CompilationArtifactPublicationReceipt {
            compilation_result_identity: request.compilation_result_identity(),
            destination: request.destination().clone(),
            policy: request.policy(),
            progress: progress.clone(),
        })
    }
}

/// A failure while publishing a Compilation Result's exact artifacts through OpenDAL.
///
/// This error's own `Display` and `Debug` output omit native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
#[derive(Debug, thiserror::Error)]
#[error(
    "Compilation Output Artifact publication failed for binding {} beneath prefix operation path {:?} during {phase:?}: {cause}",
    .destination.binding(),
    .destination.operation_path(),
)]
pub struct CompilationArtifactPublicationError {
    compilation_result_identity: CompilationResultIdentity,
    destination: Location,
    policy: PublicationPolicy,
    failed_artifact_index: Option<usize>,
    failed_key: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: CompilationArtifactPublicationProgress,
    commit_certainty: CommitCertainty,
    #[source]
    cause: RedactedError<CompilationArtifactPublicationErrorCause>,
}

impl CompilationArtifactPublicationError {
    pub const fn compilation_result_identity(&self) -> CompilationResultIdentity {
        self.compilation_result_identity
    }

    pub const fn destination(&self) -> &Location {
        &self.destination
    }

    pub const fn policy(&self) -> PublicationPolicy {
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

    pub const fn phase(&self) -> OpenDalPublicationPhase {
        self.phase
    }

    pub const fn progress(&self) -> &CompilationArtifactPublicationProgress {
        &self.progress
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub const fn cause(&self) -> &CompilationArtifactPublicationErrorCause {
        self.cause.inner()
    }
}

/// The typed cause of an OpenDAL Compilation Output Artifact publication failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactPublicationErrorCause {
    #[error("the Compilation Result identity mismatched")]
    CompilationResultMismatch {
        expected: CompilationResultIdentity,
        actual: CompilationResultIdentity,
    },
    #[error("a composed destination path was invalid")]
    InvalidDestinationPath { artifact_index: usize, key: String },
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    #[error("the publication policy is unsupported")]
    UnsupportedPolicy { policy: PublicationPolicy },
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

fn compilation_artifact_publication_error(
    request: &CompilationArtifactPublicationRequest,
    failed_artifact_index: Option<usize>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: &CompilationArtifactPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: CompilationArtifactPublicationErrorCause,
) -> CompilationArtifactPublicationError {
    let failed_key = failed_artifact_index.map(|index| request.artifact_keys()[index].clone());
    CompilationArtifactPublicationError {
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

fn attempted_effects_commit_certainty<'a>(
    outcomes: impl IntoIterator<Item = &'a PublicationKeyOutcome>,
) -> Option<CommitCertainty> {
    outcomes
        .into_iter()
        .any(|outcome| outcome.commit_certainty().is_some())
        .then_some(CommitCertainty::Committed)
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
            outcome: PublicationKeyOutcome,
        }

        impl $entry {
            $($entry_accessors)*

            pub const fn outcome(&self) -> PublicationKeyOutcome {
                self.outcome
            }

            pub const fn commit_certainty(&self) -> Option<CommitCertainty> {
                self.outcome.commit_certainty()
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

            pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty> {
                attempted_effects_commit_certainty(
                    self.completed.iter().map(|entry| &entry.outcome),
                )
            }

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

            pub const fn phase(&self) -> OpenDalPublicationPhase {
                OpenDalPublicationPhase::Complete
            }

            pub const fn progress(&self) -> &$progress {
                &self.progress
            }

            pub fn attempted_effects_commit_certainty(&self) -> Option<CommitCertainty> {
                self.progress.attempted_effects_commit_certainty()
            }
        }
    };
}

workflow_evidence!(
    PackArchivePublicationEntry,
    PackArchivePublicationProgress,
    PackArchivePublicationReceipt,
    entry { destination_path: String },
    entry_accessors {
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> Option<&PackArchivePublicationEntry> { self.completed.first() }
        pub fn outcome(&self) -> Option<PublicationKeyOutcome> {
            self.completed().map(PackArchivePublicationEntry::outcome)
        }
    },
    receipt { destination: Location, policy: PublicationPolicy },
    receipt_accessors {
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> PublicationPolicy { self.policy }
        pub fn completed(&self) -> &PackArchivePublicationEntry {
            self.progress.completed().expect("a Pack Archive receipt has one completed entry")
        }
        pub const fn outcome(&self) -> PublicationKeyOutcome {
            match self.progress.completed.as_slice() {
                [entry, ..] => entry.outcome,
                [] => panic!("a Pack Archive receipt has one completed entry"),
            }
        }
    }
);

workflow_evidence!(
    PackageCacheArchivePublicationEntry,
    PackageCacheArchivePublicationProgress,
    PackageCacheArchivePublicationReceipt,
    entry { destination_path: String },
    entry_accessors {
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> Option<&PackageCacheArchivePublicationEntry> { self.completed.first() }
        pub fn outcome(&self) -> Option<PublicationKeyOutcome> {
            self.completed().map(PackageCacheArchivePublicationEntry::outcome)
        }
    },
    receipt { destination: Location, policy: PublicationPolicy },
    receipt_accessors {
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> PublicationPolicy { self.policy }
        pub fn completed(&self) -> &PackageCacheArchivePublicationEntry {
            self.progress.completed().expect("a package-cache archive receipt has one completed entry")
        }
        pub const fn outcome(&self) -> PublicationKeyOutcome {
            match self.progress.completed.as_slice() {
                [entry, ..] => entry.outcome,
                [] => panic!("a package-cache archive receipt has one completed entry"),
            }
        }
    }
);

workflow_evidence!(
    PackExtractionPublicationEntry,
    PackExtractionPublicationProgress,
    PackExtractionPublicationReceipt,
    entry { relative_path: String, destination_path: String },
    entry_accessors {
        pub fn relative_path(&self) -> &str { &self.relative_path }
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> &[PackExtractionPublicationEntry] { &self.completed }
    },
    receipt { destination: Location, policy: PublicationPolicy, pack_identity: crate::PackIdentity },
    receipt_accessors {
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> PublicationPolicy { self.policy }
        pub fn pack_identity(&self) -> crate::PackIdentity { self.pack_identity }
        pub fn completed(&self) -> &[PackExtractionPublicationEntry] { self.progress.completed() }
    }
);

workflow_evidence!(
    CompilationArtifactPublicationEntry,
    CompilationArtifactPublicationProgress,
    CompilationArtifactPublicationReceipt,
    entry { artifact_index: usize, key: String, destination_path: String },
    entry_accessors {
        pub const fn artifact_index(&self) -> usize { self.artifact_index }
        pub fn key(&self) -> &str { &self.key }
        pub fn destination_path(&self) -> &str { &self.destination_path }
    },
    progress_accessors {
        pub fn completed(&self) -> &[CompilationArtifactPublicationEntry] { &self.completed }
    },
    receipt {
        compilation_result_identity: crate::CompilationResultIdentity,
        destination: Location,
        policy: PublicationPolicy
    },
    receipt_accessors {
        pub fn compilation_result_identity(&self) -> crate::CompilationResultIdentity {
            self.compilation_result_identity
        }
        pub fn destination(&self) -> &Location { &self.destination }
        pub const fn policy(&self) -> PublicationPolicy { self.policy }
        pub fn completed(&self) -> &[CompilationArtifactPublicationEntry] { self.progress.completed() }
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
pub(crate) struct ExactKeyPublicationReceipt {
    completed: Vec<ExactKeyPublicationEntry>,
}

impl ExactKeyPublicationReceipt {
    #[cfg(test)]
    fn completed(&self) -> &[ExactKeyPublicationEntry] {
        &self.completed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactKeyPublicationEntry {
    pub(crate) index: usize,
    pub(crate) outcome: PublicationKeyOutcome,
}

struct ExactKeyPublicationFailure {
    phase: OpenDalPublicationPhase,
    failed_index: Option<usize>,
    failed_path: Option<String>,
    commit_certainty: CommitCertainty,
}

impl ExactKeyPublicationFailure {
    fn operation(phase: OpenDalPublicationPhase) -> Self {
        Self {
            phase,
            failed_index: None,
            failed_path: None,
            commit_certainty: CommitCertainty::NotCommitted,
        }
    }

    fn key(
        phase: OpenDalPublicationPhase,
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

trait ExactKeyPublicationCause: Sized {
    fn resolve_operator(source: BoxError) -> Self;
    fn unsupported_policy(policy: PublicationPolicy) -> Self;
    fn unsupported_object_size(index: usize, byte_length: u64) -> Self;
    fn preflight_read(source: opendal::Error) -> Self;
    fn byte_conflict(expected_byte_length: u64, observed_byte_length_at_least: u64) -> Self;
    fn conditional_create(source: opendal::Error) -> Self;
    fn race_verification(source: opendal::Error) -> Self;
}

trait ExactKeyOverwriteCause: ExactKeyPublicationCause {
    fn direct_write(source: opendal::Error) -> Self;
}

trait ExactKeyPublicationOperation {
    type Error;
    type Cause: ExactKeyPublicationCause;

    fn completed_entry(&mut self, entry: ExactKeyPublicationEntry);
    fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error;
}

struct PackArchivePublicationOperation<'a> {
    request: &'a PackArchivePublicationRequest,
    progress: &'a mut PackArchivePublicationProgress,
}

impl ExactKeyPublicationOperation for PackArchivePublicationOperation<'_> {
    type Error = PackArchivePublicationError;
    type Cause = PackArchivePublicationErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyPublicationEntry) {
        self.progress.push(PackArchivePublicationEntry {
            destination_path: self.request.destination().operation_path().to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error {
        PackArchivePublicationError {
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

impl ExactKeyPublicationCause for PackArchivePublicationErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: PublicationPolicy) -> Self {
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

impl ExactKeyOverwriteCause for PackArchivePublicationErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

struct PackageCacheArchivePublicationOperation<'a> {
    request: &'a PackageCacheArchivePublicationRequest,
    progress: &'a mut PackageCacheArchivePublicationProgress,
}

impl ExactKeyPublicationOperation for PackageCacheArchivePublicationOperation<'_> {
    type Error = PackageCacheArchivePublicationError;
    type Cause = PackageCacheArchivePublicationErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyPublicationEntry) {
        self.progress.push(PackageCacheArchivePublicationEntry {
            destination_path: self.request.destination().operation_path().to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error {
        PackageCacheArchivePublicationError {
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

impl ExactKeyPublicationCause for PackageCacheArchivePublicationErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: PublicationPolicy) -> Self {
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

struct PackExtractionPublicationOperation<'a> {
    request: &'a PackExtractionPublicationRequest,
    plan: &'a crate::PackExtractionPlan,
    destinations: &'a [Location],
    progress: &'a mut PackExtractionPublicationProgress,
}

impl ExactKeyPublicationOperation for PackExtractionPublicationOperation<'_> {
    type Error = PackExtractionPublicationError;
    type Cause = PackExtractionPublicationErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyPublicationEntry) {
        let index = entry.index;
        self.progress.push(PackExtractionPublicationEntry {
            relative_path: self.plan.entries()[index].relative_path().to_owned(),
            destination_path: self.destinations[index].operation_path().to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error {
        let failed_relative_path = failure
            .failed_index
            .map(|index| self.plan.entries()[index].relative_path().to_owned());
        pack_extraction_publication_error(
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

impl ExactKeyPublicationCause for PackExtractionPublicationErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: PublicationPolicy) -> Self {
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

impl ExactKeyOverwriteCause for PackExtractionPublicationErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

struct CompilationArtifactPublicationOperation<'a> {
    request: &'a CompilationArtifactPublicationRequest,
    destinations: &'a [Location],
    progress: &'a mut CompilationArtifactPublicationProgress,
}

impl ExactKeyPublicationOperation for CompilationArtifactPublicationOperation<'_> {
    type Error = CompilationArtifactPublicationError;
    type Cause = CompilationArtifactPublicationErrorCause;

    fn completed_entry(&mut self, entry: ExactKeyPublicationEntry) {
        let artifact_index = entry.index;
        self.progress.push(CompilationArtifactPublicationEntry {
            artifact_index,
            key: self.request.artifact_keys()[artifact_index].clone(),
            destination_path: self.destinations[artifact_index]
                .operation_path()
                .to_owned(),
            outcome: entry.outcome,
        });
    }

    fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error {
        compilation_artifact_publication_error(
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

impl ExactKeyPublicationCause for CompilationArtifactPublicationErrorCause {
    fn resolve_operator(source: BoxError) -> Self {
        Self::ResolveOperator(source)
    }

    fn unsupported_policy(policy: PublicationPolicy) -> Self {
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

impl ExactKeyOverwriteCause for CompilationArtifactPublicationErrorCause {
    fn direct_write(source: opendal::Error) -> Self {
        Self::DirectWrite(source)
    }
}

async fn publish_exact_keys<R, O>(
    resolver: &R,
    binding: &OperatorBinding,
    policy: PublicationPolicy,
    keys: &[ExactKey<'_>],
    operation: &mut O,
) -> Result<ExactKeyPublicationReceipt, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: ExactKeyPublicationOperation,
    O::Cause: ExactKeyOverwriteCause,
{
    if keys.is_empty() {
        return Ok(ExactKeyPublicationReceipt {
            completed: Vec::new(),
        });
    }

    let operator = resolver.resolve(binding).map_err(|source| {
        operation.error(
            ExactKeyPublicationFailure::operation(OpenDalPublicationPhase::ResolveOperator),
            O::Cause::resolve_operator(Box::new(source)),
        )
    })?;
    appraise_capabilities(&operator, policy, keys, operation)?;

    let mut completed = Vec::with_capacity(keys.len());
    match policy {
        PublicationPolicy::OverwriteExactKeys => {
            for (index, key) in keys.iter().enumerate() {
                operator
                    .write(key.path, key.bytes.to_vec())
                    .await
                    .map_err(|source| {
                        operation.error(
                            ExactKeyPublicationFailure::key(
                                OpenDalPublicationPhase::DirectWrite,
                                index,
                                key,
                                CommitCertainty::Indeterminate,
                            ),
                            O::Cause::direct_write(source),
                        )
                    })?;
                let entry = ExactKeyPublicationEntry {
                    index,
                    outcome: PublicationKeyOutcome::Written,
                };
                operation.completed_entry(entry.clone());
                completed.push(entry);
            }
        }
        PublicationPolicy::CreateOrVerify => {
            publish_create_or_verify(&operator, keys, &mut completed, operation).await?;
        }
    }

    Ok(ExactKeyPublicationReceipt { completed })
}

async fn publish_create_or_verify_exact_keys<R, O>(
    resolver: &R,
    binding: &OperatorBinding,
    keys: &[ExactKey<'_>],
    operation: &mut O,
) -> Result<ExactKeyPublicationReceipt, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: ExactKeyPublicationOperation,
{
    if keys.is_empty() {
        return Ok(ExactKeyPublicationReceipt {
            completed: Vec::new(),
        });
    }

    let operator = resolver.resolve(binding).map_err(|source| {
        operation.error(
            ExactKeyPublicationFailure::operation(OpenDalPublicationPhase::ResolveOperator),
            O::Cause::resolve_operator(Box::new(source)),
        )
    })?;
    appraise_capabilities(
        &operator,
        PublicationPolicy::CreateOrVerify,
        keys,
        operation,
    )?;

    let mut completed = Vec::with_capacity(keys.len());
    publish_create_or_verify(&operator, keys, &mut completed, operation).await?;
    Ok(ExactKeyPublicationReceipt { completed })
}

fn appraise_capabilities<O: ExactKeyPublicationOperation>(
    operator: &opendal::Operator,
    policy: PublicationPolicy,
    keys: &[ExactKey<'_>],
    operation: &O,
) -> Result<(), O::Error> {
    let capability = operator.info().capability();
    let policy_supported = capability.write
        && (!keys.iter().any(|key| key.bytes.is_empty()) || capability.write_can_empty)
        && (policy != PublicationPolicy::CreateOrVerify
            || (capability.read && capability.write_with_if_not_exists));
    if !policy_supported {
        return Err(operation.error(
            ExactKeyPublicationFailure::operation(OpenDalPublicationPhase::CapabilityAppraisal),
            O::Cause::unsupported_policy(policy),
        ));
    }
    if let Some(maximum) = capability.write_total_max_size {
        for (index, key) in keys.iter().enumerate() {
            if key.bytes.len() > maximum {
                return Err(operation.error(
                    ExactKeyPublicationFailure::key(
                        OpenDalPublicationPhase::CapabilityAppraisal,
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

async fn publish_create_or_verify<O: ExactKeyPublicationOperation>(
    operator: &opendal::Operator,
    keys: &[ExactKey<'_>],
    completed: &mut Vec<ExactKeyPublicationEntry>,
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
                    ExactKeyPublicationFailure::key(
                        OpenDalPublicationPhase::PreflightRead,
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
                    OpenDalPublicationPhase::PreflightRead,
                    index,
                    key,
                    observed_byte_length_at_least,
                ));
            }
        };
        if observation == ExistingObject::Matching && completed.len() == index {
            let entry = ExactKeyPublicationEntry {
                index,
                outcome: PublicationKeyOutcome::AlreadyMatching,
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
            ExistingObject::Matching => PublicationKeyOutcome::AlreadyMatching,
            ExistingObject::Absent => {
                match operator
                    .write_with(key.path, key.bytes.to_vec())
                    .if_not_exists(true)
                    .await
                {
                    Ok(_) => PublicationKeyOutcome::Created,
                    Err(source)
                        if matches!(
                            source.kind(),
                            ErrorKind::AlreadyExists | ErrorKind::ConditionNotMatch
                        ) =>
                    {
                        match compare_object(operator, key.path, key.bytes).await {
                            Ok(ExistingObject::Matching) => PublicationKeyOutcome::AlreadyMatching,
                            Ok(ExistingObject::Absent) => {
                                unreachable!("a successful comparison never reports absence")
                            }
                            Err(CompareError::Read { source, .. }) => {
                                return Err(operation.error(
                                    ExactKeyPublicationFailure::key(
                                        OpenDalPublicationPhase::RaceVerification,
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
                                    OpenDalPublicationPhase::RaceVerification,
                                    index,
                                    key,
                                    observed_byte_length_at_least,
                                ));
                            }
                        }
                    }
                    Err(source) => {
                        return Err(operation.error(
                            ExactKeyPublicationFailure::key(
                                OpenDalPublicationPhase::ConditionalCreate,
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
        let entry = ExactKeyPublicationEntry { index, outcome };
        operation.completed_entry(entry.clone());
        completed.push(entry);
    }
    Ok(())
}

fn byte_conflict_error<O: ExactKeyPublicationOperation>(
    operation: &O,
    phase: OpenDalPublicationPhase,
    index: usize,
    key: &ExactKey<'_>,
    observed_byte_length_at_least: u64,
) -> O::Error {
    operation.error(
        ExactKeyPublicationFailure::key(phase, index, key, CommitCertainty::NotCommitted),
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
    u64::try_from(bytes.len()).expect("OpenDAL publication supports no 128-bit target")
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::opendal::scripted_service::{
        DestinationMutation, PendingPoint, PublicationCapabilities, PublicationDroppedOperation,
        PublicationOperationLogEntry, PublicationReadScript, PublicationReadStep,
        PublicationService, WriteCondition, WriteScript, WriteStep,
    };
    use crate::opendal::{OperatorBinding, OperatorResolver};
    use crate::pack_archive::CommitCertainty;
    use crate::{
        CompilationLimits, CompilationOutputSpecification, Pack, PackCompilationRequest,
        SvgOutputSpecification, compile,
    };

    use super::{
        CompilationArtifactPublicationErrorCause, CompilationArtifactPublicationProgress,
        CompilationArtifactPublicationRequest, ExactKey, ExactKeyOverwriteCause,
        ExactKeyPublicationCause, ExactKeyPublicationEntry, ExactKeyPublicationFailure,
        ExactKeyPublicationOperation, OpenDalPublicationPhase, PackArchivePublicationEntry,
        PackArchivePublicationProgress, PublicationKeyOutcome, PublicationPolicy,
        publish_compilation_artifacts, publish_exact_keys,
    };

    #[test]
    fn empty_publication_succeeds_without_resolving_an_operator() {
        let resolver = RejectingResolver;
        let binding = binding();
        let mut completed = Vec::new();
        let receipt = {
            let mut operation = TestPublicationOperation::new(&mut completed);
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::OverwriteExactKeys,
                &[],
                &mut operation,
            ));
            expect_ready(publication.as_mut()).unwrap()
        };

        assert!(receipt.completed().is_empty());
        assert!(completed.is_empty());
    }

    #[test]
    fn invalid_composed_artifact_destination_fails_before_resolution() {
        let result = two_artifact_result();
        let request = CompilationArtifactPublicationRequest {
            compilation_result_identity: result.result_identity(),
            destination: "destination:/prefix/".parse().unwrap(),
            artifact_keys: vec!["valid.svg".to_owned(), "../alias.svg".to_owned()],
            policy: PublicationPolicy::OverwriteExactKeys,
        };
        let mut progress = CompilationArtifactPublicationProgress::new();

        let error = expect_ready(pin!(publish_compilation_artifacts(
            &RejectingResolver,
            &request,
            &result,
            &mut progress,
        )))
        .unwrap_err();

        assert_eq!(
            error.phase(),
            OpenDalPublicationPhase::DestinationValidation
        );
        assert_eq!(error.failed_artifact_index(), Some(1));
        assert_eq!(error.failed_key(), Some("../alias.svg"));
        assert_eq!(error.failed_destination_path(), None);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(error.progress().completed().is_empty());
        assert!(matches!(
            error.cause(),
            CompilationArtifactPublicationErrorCause::InvalidDestinationPath {
                artifact_index: 1,
                key,
            } if key == "../alias.svg"
        ));
    }

    #[test]
    fn overwrite_writes_each_key_once_in_order_without_reading() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
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

        let receipt = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::OverwriteExactKeys,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
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
            [
                (0, PublicationKeyOutcome::Written),
                (1, PublicationKeyOutcome::Written),
            ]
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn create_or_verify_compares_every_key_before_mutation() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("conflict.bin".to_owned(), b"wrong".to_vec())],
            [
                PublicationReadScript::new(
                    "absent.bin",
                    0,
                    [PublicationReadStep::failure(ErrorKind::NotFound)],
                )
                .unwrap(),
                PublicationReadScript::new("conflict.bin", 1, [PublicationReadStep::chunk(0..5)])
                    .unwrap(),
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

        let error = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::CreateOrVerify,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::PreflightRead);
        assert_eq!(error.failed_index, Some(1));
        assert_eq!(error.commit_certainty, CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause,
            TestPublicationErrorCause::ByteConflict {
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
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn later_preflight_conflict_retains_the_leading_matching_prefix() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [
                ("matching.bin".to_owned(), b"matching".to_vec()),
                ("conflict.bin".to_owned(), b"wrong".to_vec()),
            ],
            [
                PublicationReadScript::new("matching.bin", 1, [PublicationReadStep::chunk(0..8)])
                    .unwrap(),
                PublicationReadScript::new("conflict.bin", 1, [PublicationReadStep::chunk(0..5)])
                    .unwrap(),
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

        let error = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::CreateOrVerify,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::PreflightRead);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(completed[0].outcome, PublicationKeyOutcome::AlreadyMatching);
    }

    #[test]
    fn mutable_matching_stream_is_read_only_evidence_without_commit_certainty() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("mutable.bin".to_owned(), b"abcdef".to_vec())],
            [PublicationReadScript::new(
                "mutable.bin",
                2,
                [
                    PublicationReadStep::chunk(0..3),
                    PublicationReadStep::mutate(DestinationMutation::set("mutable.bin", b"abcXYZ")),
                    PublicationReadStep::chunk(3..6),
                ],
            )
            .unwrap()],
            [],
            16,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("mutable.bin", b"abcXYZ")];
        let mut completed = Vec::new();

        let receipt = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::CreateOrVerify,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap();

        assert_eq!(
            receipt.completed()[0].outcome,
            PublicationKeyOutcome::AlreadyMatching
        );
        assert_eq!(receipt.completed()[0].outcome.commit_certainty(), None);
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn disappearance_after_a_partial_stream_is_not_treated_as_absence() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("unstable.bin".to_owned(), b"planned".to_vec())],
            [PublicationReadScript::new(
                "unstable.bin",
                1,
                [
                    PublicationReadStep::chunk(0..3),
                    PublicationReadStep::failure(ErrorKind::NotFound),
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

        let error = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::CreateOrVerify,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::PreflightRead);
        assert!(matches!(
            error.cause,
            TestPublicationErrorCause::PreflightRead(ref source)
                if source.kind() == ErrorKind::NotFound
        ));
        assert!(completed.is_empty());
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
        );
    }

    #[test]
    fn conditional_conflict_performs_one_bounded_verification() {
        let pending = PendingPoint::new();
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [],
            [
                PublicationReadScript::new(
                    "race.bin",
                    0,
                    [PublicationReadStep::failure(ErrorKind::NotFound)],
                )
                .unwrap(),
                PublicationReadScript::new("race.bin", 1, [PublicationReadStep::chunk(0..7)])
                    .unwrap(),
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
            let mut operation = TestPublicationOperation::new(&mut completed);
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::CreateOrVerify,
                &keys,
                &mut operation,
            ));

            assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
            service.mutate(DestinationMutation::set("race.bin", b"planned"));
            pending.release();
            expect_ready(publication.as_mut()).unwrap()
        };

        assert_eq!(
            receipt.completed()[0].outcome,
            PublicationKeyOutcome::AlreadyMatching
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, PublicationOperationLogEntry::ReadInvoked { .. }))
                .count(),
            2
        );
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn appraisal_rejects_capabilities_and_sizes_before_effects() {
        let cases = [
            PublicationCapabilities {
                write: false,
                write_can_empty: true,
                write_with_if_not_exists: true,
                read: true,
                write_total_max_size: None,
            },
            PublicationCapabilities {
                write: true,
                write_can_empty: false,
                write_with_if_not_exists: true,
                read: true,
                write_total_max_size: None,
            },
            PublicationCapabilities {
                write: true,
                write_can_empty: true,
                write_with_if_not_exists: false,
                read: true,
                write_total_max_size: None,
            },
            PublicationCapabilities {
                write: true,
                write_can_empty: true,
                write_with_if_not_exists: true,
                read: false,
                write_total_max_size: None,
            },
        ];
        for capabilities in cases {
            let service = PublicationService::new(capabilities, [], [], [], 4);
            let resolver = ServiceResolver(service.operator());
            let keys = [ExactKey::new("empty.bin", b"")];
            let mut completed = Vec::new();

            let error = expect_ready(pin!(publish_exact_keys(
                &resolver,
                &binding(),
                PublicationPolicy::CreateOrVerify,
                &keys,
                &mut TestPublicationOperation::new(&mut completed),
            )))
            .unwrap_err();

            assert_eq!(error.phase, OpenDalPublicationPhase::CapabilityAppraisal);
            assert!(matches!(
                error.cause,
                TestPublicationErrorCause::UnsupportedPolicy { .. }
            ));
            assert!(service.log().entries().is_empty());
        }

        let service = PublicationService::new(
            PublicationCapabilities {
                write_total_max_size: Some(3),
                ..PublicationCapabilities::all()
            },
            [],
            [],
            [],
            4,
        );
        let resolver = ServiceResolver(service.operator());
        let keys = [ExactKey::new("large.bin", b"four")];
        let mut completed = Vec::new();
        let error = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::OverwriteExactKeys,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert!(matches!(
            error.cause,
            TestPublicationErrorCause::UnsupportedObjectSize { byte_length: 4 }
        ));
        assert!(service.log().entries().is_empty());
    }

    #[test]
    fn issued_write_failure_is_indeterminate_and_retains_the_completed_prefix() {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
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

        let error = expect_ready(pin!(publish_exact_keys(
            &resolver,
            &binding(),
            PublicationPolicy::OverwriteExactKeys,
            &keys,
            &mut TestPublicationOperation::new(&mut completed),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::DirectWrite);
        assert_eq!(error.failed_index, Some(1));
        assert_eq!(error.failed_path.as_deref(), Some("second.bin"));
        assert_eq!(error.commit_certainty, CommitCertainty::Indeterminate);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
    }

    #[test]
    fn dropping_a_pending_publication_leaves_the_completed_prefix_with_the_caller() {
        let pending = PendingPoint::new();
        let service = PublicationService::new(
            PublicationCapabilities::all(),
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
            let mut operation = TestPublicationOperation::new(&mut completed);
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::OverwriteExactKeys,
                &keys,
                &mut operation,
            ));
            assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(
            service.cancellations(),
            [PublicationDroppedOperation::Write {
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
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("matching.bin".to_owned(), b"matching".to_vec())],
            [
                PublicationReadScript::new("matching.bin", 1, [PublicationReadStep::chunk(0..8)])
                    .unwrap(),
                PublicationReadScript::new(
                    "pending.bin",
                    0,
                    [PublicationReadStep::pending(pending.clone())],
                )
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
            let mut operation = TestPublicationOperation::new(&mut completed);
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::CreateOrVerify,
                &keys,
                &mut operation,
            ));
            assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].index, 0);
        assert_eq!(completed[0].outcome, PublicationKeyOutcome::AlreadyMatching);
    }

    #[test]
    fn workflow_evidence_does_not_treat_matching_observations_as_effects() {
        let mut progress = PackArchivePublicationProgress::new();
        progress.push(PackArchivePublicationEntry {
            destination_path: "archive.typk".to_owned(),
            outcome: PublicationKeyOutcome::AlreadyMatching,
        });

        assert_eq!(
            progress.outcome(),
            Some(PublicationKeyOutcome::AlreadyMatching)
        );
        assert_eq!(progress.completed().unwrap().commit_certainty(), None);
        assert_eq!(progress.attempted_effects_commit_certainty(), None);

        progress.clear();
        progress.push(PackArchivePublicationEntry {
            destination_path: "archive.typk".to_owned(),
            outcome: PublicationKeyOutcome::Created,
        });
        assert_eq!(
            progress.attempted_effects_commit_certainty(),
            Some(CommitCertainty::Committed)
        );
    }

    #[derive(Debug)]
    struct TestPublicationError {
        phase: OpenDalPublicationPhase,
        failed_index: Option<usize>,
        failed_path: Option<String>,
        commit_certainty: CommitCertainty,
        cause: TestPublicationErrorCause,
    }

    #[derive(Debug)]
    enum TestPublicationErrorCause {
        ResolveOperator(crate::opendal::BoxError),
        UnsupportedPolicy {
            policy: PublicationPolicy,
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

    impl ExactKeyPublicationCause for TestPublicationErrorCause {
        fn resolve_operator(source: crate::opendal::BoxError) -> Self {
            Self::ResolveOperator(source)
        }

        fn unsupported_policy(policy: PublicationPolicy) -> Self {
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

    impl ExactKeyOverwriteCause for TestPublicationErrorCause {
        fn direct_write(source: opendal::Error) -> Self {
            Self::DirectWrite(source)
        }
    }

    struct TestPublicationOperation<'a> {
        completed: &'a mut Vec<ExactKeyPublicationEntry>,
    }

    impl<'a> TestPublicationOperation<'a> {
        fn new(completed: &'a mut Vec<ExactKeyPublicationEntry>) -> Self {
            Self { completed }
        }
    }

    impl ExactKeyPublicationOperation for TestPublicationOperation<'_> {
        type Error = TestPublicationError;
        type Cause = TestPublicationErrorCause;

        fn completed_entry(&mut self, entry: ExactKeyPublicationEntry) {
            self.completed.push(entry);
        }

        fn error(&self, failure: ExactKeyPublicationFailure, cause: Self::Cause) -> Self::Error {
            TestPublicationError {
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
        compile(
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
            panic!("an empty publication must not resolve an operator")
        }
    }
}
