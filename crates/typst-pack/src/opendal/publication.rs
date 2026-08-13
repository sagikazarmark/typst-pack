//! OpenDAL exact-key publication vocabulary and crate-private execution.

use std::{collections::BTreeMap, error::Error, fmt, future::Future};

use futures_util::StreamExt;
use opendal::ErrorKind;

use super::location::validate_decoded_artifact_key_path;
use super::{Location, LocationError, LocationRoleError, OperatorBinding, OperatorResolver};
use crate::pack_archive::CommitCertainty;
use crate::{CompilationResult, CompilationResultIdentity, CompilationStatus};

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
#[derive(Clone, Eq, PartialEq)]
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

impl fmt::Display for CompilationArtifactPublicationRequestRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Compilation Output Artifact publication request rejected for binding {} beneath prefix operation path {:?} with {} issue(s)",
            self.destination.binding(),
            self.destination.operation_path(),
            self.issues.len()
        )
    }
}

impl fmt::Debug for CompilationArtifactPublicationRequestRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationArtifactPublicationRequestRejection")
            .field("binding", self.destination.binding())
            .field("role", &"prefix")
            .field("operation_path", &self.destination.operation_path())
            .field(
                "compilation_result_identity",
                &self.compilation_result_identity,
            )
            .field("issues", &self.issues)
            .finish()
    }
}

impl Error for CompilationArtifactPublicationRequestRejection {}

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
) -> impl Future<
    Output = Result<PackExtractionPublicationReceipt, PackExtractionPublicationError<R::Error>>,
> + 'a {
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
        let execution = publish_exact_keys(
            resolver,
            request.destination().binding(),
            request.policy(),
            &keys,
            |entry| {
                let index = entry.index;
                progress.push(PackExtractionPublicationEntry {
                    relative_path: plan.entries()[index].relative_path().to_owned(),
                    destination_path: destinations[index].operation_path().to_owned(),
                    outcome: entry.outcome,
                });
            },
        )
        .await;

        match execution {
            Ok(_) => Ok(PackExtractionPublicationReceipt {
                destination: request.destination().clone(),
                policy: request.policy(),
                pack_identity: *plan.pack_identity(),
                progress: progress.clone(),
            }),
            Err(error) => {
                let failed_index = error.failed_index;
                let failed_relative_path =
                    failed_index.map(|index| plan.entries()[index].relative_path().to_owned());
                let failed_destination_path = error.failed_path;
                let phase = error.phase;
                let commit_certainty = error.commit_certainty;
                let cause = match *error.cause {
                    ExactKeyPublicationErrorCause::ResolveOperator(source) => {
                        PackExtractionPublicationErrorCause::ResolveOperator(source)
                    }
                    ExactKeyPublicationErrorCause::UnsupportedPolicy => {
                        PackExtractionPublicationErrorCause::UnsupportedPolicy {
                            policy: request.policy(),
                        }
                    }
                    ExactKeyPublicationErrorCause::UnsupportedObjectSize { byte_length } => {
                        PackExtractionPublicationErrorCause::UnsupportedObjectSize { byte_length }
                    }
                    ExactKeyPublicationErrorCause::PreflightRead(source) => {
                        PackExtractionPublicationErrorCause::PreflightRead(source)
                    }
                    ExactKeyPublicationErrorCause::ByteConflict {
                        expected_byte_length,
                        observed_byte_length_at_least,
                    } => PackExtractionPublicationErrorCause::ByteConflict {
                        expected_byte_length,
                        observed_byte_length_at_least,
                    },
                    ExactKeyPublicationErrorCause::ConditionalCreate(source) => {
                        PackExtractionPublicationErrorCause::ConditionalCreate(source)
                    }
                    ExactKeyPublicationErrorCause::RaceVerification(source) => {
                        PackExtractionPublicationErrorCause::RaceVerification(source)
                    }
                    ExactKeyPublicationErrorCause::DirectWrite(source) => {
                        PackExtractionPublicationErrorCause::DirectWrite(source)
                    }
                };
                Err(pack_extraction_publication_error(
                    request,
                    failed_relative_path,
                    failed_destination_path,
                    phase,
                    progress,
                    commit_certainty,
                    cause,
                ))
            }
        }
    }
}

/// A failure while publishing a Pack Extraction Plan through OpenDAL.
///
/// This error's own `Display` and `Debug` output omits native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
pub struct PackExtractionPublicationError<E> {
    destination: Location,
    policy: PublicationPolicy,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: PackExtractionPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: PackExtractionPublicationErrorCause<E>,
}

impl<E> PackExtractionPublicationError<E> {
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

    pub fn cause(&self) -> &PackExtractionPublicationErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for PackExtractionPublicationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Pack Extraction publication failed for binding {} beneath prefix operation path {:?} during {:?}: {}",
            self.destination.binding(),
            self.destination.operation_path(),
            self.phase,
            self.cause.label(),
        )
    }
}

impl<E> fmt::Debug for PackExtractionPublicationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackExtractionPublicationError")
            .field("binding", self.destination.binding())
            .field("role", &"prefix")
            .field("operation_path", &self.destination.operation_path())
            .field("policy", &self.policy)
            .field("failed_relative_path", &self.failed_relative_path)
            .field("failed_destination_path", &self.failed_destination_path)
            .field("phase", &self.phase)
            .field("progress", &self.progress)
            .field("commit_certainty", &self.commit_certainty)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackExtractionPublicationError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackExtractionPublicationErrorCause::ResolveOperator(source) => Some(source),
            PackExtractionPublicationErrorCause::PreflightRead(source)
            | PackExtractionPublicationErrorCause::ConditionalCreate(source)
            | PackExtractionPublicationErrorCause::RaceVerification(source)
            | PackExtractionPublicationErrorCause::DirectWrite(source) => Some(source),
            PackExtractionPublicationErrorCause::InvalidDestinationPath { .. }
            | PackExtractionPublicationErrorCause::UnsupportedPolicy { .. }
            | PackExtractionPublicationErrorCause::UnsupportedObjectSize { .. }
            | PackExtractionPublicationErrorCause::ByteConflict { .. } => None,
        }
    }
}

/// The typed cause of an OpenDAL Pack Extraction publication failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackExtractionPublicationErrorCause<E> {
    InvalidDestinationPath {
        relative_path: String,
    },
    ResolveOperator(E),
    UnsupportedPolicy {
        policy: PublicationPolicy,
    },
    UnsupportedObjectSize {
        byte_length: u64,
    },
    PreflightRead(::opendal::Error),
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
    DirectWrite(::opendal::Error),
}

impl<E> PackExtractionPublicationErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::InvalidDestinationPath { .. } => "a composed destination path was invalid",
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedPolicy { .. } => "the publication policy is unsupported",
            Self::UnsupportedObjectSize { .. } => "an entry exceeds the advertised object size",
            Self::PreflightRead(_) => "a preflight read failed",
            Self::ByteConflict { .. } => "destination bytes conflict",
            Self::ConditionalCreate(_) => "a conditional create failed",
            Self::RaceVerification(_) => "race verification failed",
            Self::DirectWrite(_) => "a direct write failed",
        }
    }
}

fn pack_extraction_publication_error<E>(
    request: &PackExtractionPublicationRequest,
    failed_relative_path: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: &PackExtractionPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: PackExtractionPublicationErrorCause<E>,
) -> PackExtractionPublicationError<E> {
    PackExtractionPublicationError {
        destination: request.destination().clone(),
        policy: request.policy(),
        failed_relative_path,
        failed_destination_path,
        phase,
        progress: progress.clone(),
        commit_certainty,
        cause,
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
    Output = Result<
        CompilationArtifactPublicationReceipt,
        CompilationArtifactPublicationError<R::Error>,
    >,
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
        let execution = publish_exact_keys(
            resolver,
            request.destination().binding(),
            request.policy(),
            &keys,
            |entry| {
                let artifact_index = entry.index;
                progress.push(CompilationArtifactPublicationEntry {
                    artifact_index,
                    key: request.artifact_keys()[artifact_index].clone(),
                    destination_path: destinations[artifact_index].operation_path().to_owned(),
                    outcome: entry.outcome,
                });
            },
        )
        .await;

        match execution {
            Ok(_) => Ok(CompilationArtifactPublicationReceipt {
                compilation_result_identity: request.compilation_result_identity(),
                destination: request.destination().clone(),
                policy: request.policy(),
                progress: progress.clone(),
            }),
            Err(error) => {
                let failed_index = error.failed_index;
                let failed_destination_path = error.failed_path;
                let phase = error.phase;
                let commit_certainty = error.commit_certainty;
                let cause = match *error.cause {
                    ExactKeyPublicationErrorCause::ResolveOperator(source) => {
                        CompilationArtifactPublicationErrorCause::ResolveOperator(source)
                    }
                    ExactKeyPublicationErrorCause::UnsupportedPolicy => {
                        CompilationArtifactPublicationErrorCause::UnsupportedPolicy {
                            policy: request.policy(),
                        }
                    }
                    ExactKeyPublicationErrorCause::UnsupportedObjectSize { byte_length } => {
                        CompilationArtifactPublicationErrorCause::UnsupportedObjectSize {
                            artifact_index: failed_index
                                .expect("object-size failure identifies its artifact"),
                            byte_length,
                        }
                    }
                    ExactKeyPublicationErrorCause::PreflightRead(source) => {
                        CompilationArtifactPublicationErrorCause::PreflightRead(source)
                    }
                    ExactKeyPublicationErrorCause::ByteConflict {
                        expected_byte_length,
                        observed_byte_length_at_least,
                    } => CompilationArtifactPublicationErrorCause::ByteConflict {
                        expected_byte_length,
                        observed_byte_length_at_least,
                    },
                    ExactKeyPublicationErrorCause::ConditionalCreate(source) => {
                        CompilationArtifactPublicationErrorCause::ConditionalCreate(source)
                    }
                    ExactKeyPublicationErrorCause::RaceVerification(source) => {
                        CompilationArtifactPublicationErrorCause::RaceVerification(source)
                    }
                    ExactKeyPublicationErrorCause::DirectWrite(source) => {
                        CompilationArtifactPublicationErrorCause::DirectWrite(source)
                    }
                };
                Err(compilation_artifact_publication_error(
                    request,
                    failed_index,
                    failed_destination_path,
                    phase,
                    progress,
                    commit_certainty,
                    cause,
                ))
            }
        }
    }
}

/// A failure while publishing a Compilation Result's exact artifacts through OpenDAL.
///
/// This error's own `Display` and `Debug` output omits native resolver and
/// OpenDAL messages. Rendering its source chain may disclose backend context.
pub struct CompilationArtifactPublicationError<E> {
    compilation_result_identity: CompilationResultIdentity,
    destination: Location,
    policy: PublicationPolicy,
    failed_artifact_index: Option<usize>,
    failed_key: Option<String>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: CompilationArtifactPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: CompilationArtifactPublicationErrorCause<E>,
}

impl<E> CompilationArtifactPublicationError<E> {
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

    pub const fn cause(&self) -> &CompilationArtifactPublicationErrorCause<E> {
        &self.cause
    }
}

impl<E> fmt::Display for CompilationArtifactPublicationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Compilation Output Artifact publication failed for binding {} beneath prefix operation path {:?} during {:?}: {}",
            self.destination.binding(),
            self.destination.operation_path(),
            self.phase,
            self.cause.label(),
        )
    }
}

impl<E> fmt::Debug for CompilationArtifactPublicationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilationArtifactPublicationError")
            .field("binding", self.destination.binding())
            .field("role", &"prefix")
            .field("operation_path", &self.destination.operation_path())
            .field("policy", &self.policy)
            .field("failed_artifact_index", &self.failed_artifact_index)
            .field("failed_key", &self.failed_key)
            .field("failed_destination_path", &self.failed_destination_path)
            .field("phase", &self.phase)
            .field("progress", &self.progress)
            .field("commit_certainty", &self.commit_certainty)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for CompilationArtifactPublicationError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            CompilationArtifactPublicationErrorCause::ResolveOperator(source) => Some(source),
            CompilationArtifactPublicationErrorCause::PreflightRead(source)
            | CompilationArtifactPublicationErrorCause::ConditionalCreate(source)
            | CompilationArtifactPublicationErrorCause::RaceVerification(source)
            | CompilationArtifactPublicationErrorCause::DirectWrite(source) => Some(source),
            CompilationArtifactPublicationErrorCause::CompilationResultMismatch { .. }
            | CompilationArtifactPublicationErrorCause::InvalidDestinationPath { .. }
            | CompilationArtifactPublicationErrorCause::UnsupportedPolicy { .. }
            | CompilationArtifactPublicationErrorCause::UnsupportedObjectSize { .. }
            | CompilationArtifactPublicationErrorCause::ByteConflict { .. } => None,
        }
    }
}

/// The typed cause of an OpenDAL Compilation Output Artifact publication failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilationArtifactPublicationErrorCause<E> {
    CompilationResultMismatch {
        expected: CompilationResultIdentity,
        actual: CompilationResultIdentity,
    },
    InvalidDestinationPath {
        artifact_index: usize,
        key: String,
    },
    ResolveOperator(E),
    UnsupportedPolicy {
        policy: PublicationPolicy,
    },
    UnsupportedObjectSize {
        artifact_index: usize,
        byte_length: u64,
    },
    PreflightRead(::opendal::Error),
    ByteConflict {
        expected_byte_length: u64,
        observed_byte_length_at_least: u64,
    },
    ConditionalCreate(::opendal::Error),
    RaceVerification(::opendal::Error),
    DirectWrite(::opendal::Error),
}

impl<E> CompilationArtifactPublicationErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::CompilationResultMismatch { .. } => "the Compilation Result identity mismatched",
            Self::InvalidDestinationPath { .. } => "a composed destination path was invalid",
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedPolicy { .. } => "the publication policy is unsupported",
            Self::UnsupportedObjectSize { .. } => "an artifact exceeds the advertised object size",
            Self::PreflightRead(_) => "a preflight read failed",
            Self::ByteConflict { .. } => "destination bytes conflict",
            Self::ConditionalCreate(_) => "a conditional create failed",
            Self::RaceVerification(_) => "race verification failed",
            Self::DirectWrite(_) => "a direct write failed",
        }
    }
}

fn compilation_artifact_publication_error<E>(
    request: &CompilationArtifactPublicationRequest,
    failed_artifact_index: Option<usize>,
    failed_destination_path: Option<String>,
    phase: OpenDalPublicationPhase,
    progress: &CompilationArtifactPublicationProgress,
    commit_certainty: CommitCertainty,
    cause: CompilationArtifactPublicationErrorCause<E>,
) -> CompilationArtifactPublicationError<E> {
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
        cause,
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

#[derive(Debug)]
pub(crate) struct ExactKeyPublicationError<E> {
    pub(crate) phase: OpenDalPublicationPhase,
    pub(crate) failed_index: Option<usize>,
    pub(crate) failed_path: Option<String>,
    pub(crate) commit_certainty: CommitCertainty,
    pub(crate) cause: Box<ExactKeyPublicationErrorCause<E>>,
}

#[derive(Debug)]
pub(crate) enum ExactKeyPublicationErrorCause<E> {
    ResolveOperator(E),
    UnsupportedPolicy,
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

pub(crate) async fn publish_exact_keys<R, F>(
    resolver: &R,
    binding: &OperatorBinding,
    policy: PublicationPolicy,
    keys: &[ExactKey<'_>],
    mut completed_entry: F,
) -> Result<ExactKeyPublicationReceipt, ExactKeyPublicationError<R::Error>>
where
    R: OperatorResolver + ?Sized,
    F: FnMut(ExactKeyPublicationEntry),
{
    if keys.is_empty() {
        return Ok(ExactKeyPublicationReceipt {
            completed: Vec::new(),
        });
    }

    let operator = resolver
        .resolve(binding)
        .map_err(|source| ExactKeyPublicationError {
            phase: OpenDalPublicationPhase::ResolveOperator,
            failed_index: None,
            failed_path: None,
            commit_certainty: CommitCertainty::NotCommitted,
            cause: Box::new(ExactKeyPublicationErrorCause::ResolveOperator(source)),
        })?;
    appraise_capabilities(&operator, policy, keys)?;

    let mut completed = Vec::with_capacity(keys.len());
    match policy {
        PublicationPolicy::OverwriteExactKeys => {
            for (index, key) in keys.iter().enumerate() {
                operator
                    .write(key.path, key.bytes.to_vec())
                    .await
                    .map_err(|source| ExactKeyPublicationError {
                        phase: OpenDalPublicationPhase::DirectWrite,
                        failed_index: Some(index),
                        failed_path: Some(key.path.to_owned()),
                        commit_certainty: CommitCertainty::Indeterminate,
                        cause: Box::new(ExactKeyPublicationErrorCause::DirectWrite(source)),
                    })?;
                let entry = ExactKeyPublicationEntry {
                    index,
                    outcome: PublicationKeyOutcome::Written,
                };
                completed_entry(entry.clone());
                completed.push(entry);
            }
        }
        PublicationPolicy::CreateOrVerify => {
            publish_create_or_verify(&operator, keys, &mut completed, &mut completed_entry).await?;
        }
    }

    Ok(ExactKeyPublicationReceipt { completed })
}

fn appraise_capabilities<E>(
    operator: &opendal::Operator,
    policy: PublicationPolicy,
    keys: &[ExactKey<'_>],
) -> Result<(), ExactKeyPublicationError<E>> {
    let capability = operator.info().capability();
    let policy_supported = capability.write
        && (!keys.iter().any(|key| key.bytes.is_empty()) || capability.write_can_empty)
        && (policy != PublicationPolicy::CreateOrVerify
            || (capability.read && capability.write_with_if_not_exists));
    if !policy_supported {
        return Err(ExactKeyPublicationError {
            phase: OpenDalPublicationPhase::CapabilityAppraisal,
            failed_index: None,
            failed_path: None,
            commit_certainty: CommitCertainty::NotCommitted,
            cause: Box::new(ExactKeyPublicationErrorCause::UnsupportedPolicy),
        });
    }
    if let Some(maximum) = capability.write_total_max_size {
        for (index, key) in keys.iter().enumerate() {
            if key.bytes.len() > maximum {
                return Err(ExactKeyPublicationError {
                    phase: OpenDalPublicationPhase::CapabilityAppraisal,
                    failed_index: Some(index),
                    failed_path: Some(key.path.to_owned()),
                    commit_certainty: CommitCertainty::NotCommitted,
                    cause: Box::new(ExactKeyPublicationErrorCause::UnsupportedObjectSize {
                        byte_length: byte_length(key.bytes),
                    }),
                });
            }
        }
    }
    Ok(())
}

async fn publish_create_or_verify<E, F>(
    operator: &opendal::Operator,
    keys: &[ExactKey<'_>],
    completed: &mut Vec<ExactKeyPublicationEntry>,
    completed_entry: &mut F,
) -> Result<(), ExactKeyPublicationError<E>>
where
    F: FnMut(ExactKeyPublicationEntry),
{
    let mut observations = Vec::with_capacity(keys.len());
    for (index, key) in keys.iter().enumerate() {
        let observation = match compare_object(operator, key.path, key.bytes).await {
            Ok(observation) => observation,
            Err(CompareError::Read {
                source,
                observed_byte_length: 0,
            }) if source.kind() == ErrorKind::NotFound => ExistingObject::Absent,
            Err(CompareError::Read { source, .. }) => {
                return Err(ExactKeyPublicationError {
                    phase: OpenDalPublicationPhase::PreflightRead,
                    failed_index: Some(index),
                    failed_path: Some(key.path.to_owned()),
                    commit_certainty: CommitCertainty::NotCommitted,
                    cause: Box::new(ExactKeyPublicationErrorCause::PreflightRead(source)),
                });
            }
            Err(CompareError::Conflict {
                observed_byte_length_at_least,
            }) => {
                return Err(byte_conflict_error(
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
            completed_entry(entry.clone());
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
                                return Err(ExactKeyPublicationError {
                                    phase: OpenDalPublicationPhase::RaceVerification,
                                    failed_index: Some(index),
                                    failed_path: Some(key.path.to_owned()),
                                    commit_certainty: CommitCertainty::NotCommitted,
                                    cause: Box::new(
                                        ExactKeyPublicationErrorCause::RaceVerification(source),
                                    ),
                                });
                            }
                            Err(CompareError::Conflict {
                                observed_byte_length_at_least,
                            }) => {
                                return Err(byte_conflict_error(
                                    OpenDalPublicationPhase::RaceVerification,
                                    index,
                                    key,
                                    observed_byte_length_at_least,
                                ));
                            }
                        }
                    }
                    Err(source) => {
                        return Err(ExactKeyPublicationError {
                            phase: OpenDalPublicationPhase::ConditionalCreate,
                            failed_index: Some(index),
                            failed_path: Some(key.path.to_owned()),
                            commit_certainty: CommitCertainty::Indeterminate,
                            cause: Box::new(ExactKeyPublicationErrorCause::ConditionalCreate(
                                source,
                            )),
                        });
                    }
                }
            }
        };
        let entry = ExactKeyPublicationEntry { index, outcome };
        completed_entry(entry.clone());
        completed.push(entry);
    }
    Ok(())
}

fn byte_conflict_error<E>(
    phase: OpenDalPublicationPhase,
    index: usize,
    key: &ExactKey<'_>,
    observed_byte_length_at_least: u64,
) -> ExactKeyPublicationError<E> {
    ExactKeyPublicationError {
        phase,
        failed_index: Some(index),
        failed_path: Some(key.path.to_owned()),
        commit_certainty: CommitCertainty::NotCommitted,
        cause: Box::new(ExactKeyPublicationErrorCause::ByteConflict {
            expected_byte_length: byte_length(key.bytes),
            observed_byte_length_at_least,
        }),
    }
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
        CompilationArtifactPublicationRequest, ExactKey, ExactKeyPublicationErrorCause,
        OpenDalPublicationPhase, PackArchivePublicationEntry, PackArchivePublicationProgress,
        PublicationKeyOutcome, PublicationPolicy, publish_compilation_artifacts,
        publish_exact_keys,
    };

    #[test]
    fn empty_publication_succeeds_without_resolving_an_operator() {
        let resolver = RejectingResolver;
        let binding = binding();
        let mut completed = Vec::new();
        let receipt = {
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::OverwriteExactKeys,
                &[],
                |entry| completed.push(entry),
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
            |entry| completed.push(entry),
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
            |entry| completed.push(entry),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::PreflightRead);
        assert_eq!(error.failed_index, Some(1));
        assert_eq!(error.commit_certainty, CommitCertainty::NotCommitted);
        assert!(matches!(
            *error.cause,
            ExactKeyPublicationErrorCause::ByteConflict {
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
            |entry| completed.push(entry),
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
            |entry| completed.push(entry),
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
            |entry| completed.push(entry),
        )))
        .unwrap_err();

        assert_eq!(error.phase, OpenDalPublicationPhase::PreflightRead);
        assert!(matches!(
            *error.cause,
            ExactKeyPublicationErrorCause::PreflightRead(ref source)
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
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::CreateOrVerify,
                &keys,
                |entry| completed.push(entry),
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
                |entry| completed.push(entry),
            )))
            .unwrap_err();

            assert_eq!(error.phase, OpenDalPublicationPhase::CapabilityAppraisal);
            assert!(matches!(
                *error.cause,
                ExactKeyPublicationErrorCause::UnsupportedPolicy
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
            |entry| completed.push(entry),
        )))
        .unwrap_err();

        assert!(matches!(
            *error.cause,
            ExactKeyPublicationErrorCause::UnsupportedObjectSize { byte_length: 4 }
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
            |entry| completed.push(entry),
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
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::OverwriteExactKeys,
                &keys,
                |entry| completed.push(entry),
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
            let mut publication = pin!(publish_exact_keys(
                &resolver,
                &binding,
                PublicationPolicy::CreateOrVerify,
                &keys,
                |entry| completed.push(entry),
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
