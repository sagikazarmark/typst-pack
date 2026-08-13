//! OpenDAL exact-key publication vocabulary and crate-private execution.

use futures_util::StreamExt;
use opendal::ErrorKind;

use super::{Location, OperatorBinding, OperatorResolver};
use crate::pack_archive::CommitCertainty;

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

    use super::{
        ExactKey, ExactKeyPublicationErrorCause, OpenDalPublicationPhase,
        PackArchivePublicationEntry, PackArchivePublicationProgress, PublicationKeyOutcome,
        PublicationPolicy, publish_exact_keys,
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
