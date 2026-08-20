#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::error::Error as _;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use scripted_opendal::{
    PendingPoint, WriteCapabilities, WriteCondition, WriteOperationLogEntry, WriteReadScript,
    WriteReadStep, WriteScript, WriteService, WriteStep,
};
use typst_pack::opendal::write::{
    CompilationArtifactKeyIssue, CompilationArtifactWriteErrorCause,
    CompilationArtifactWriteRequest, CompilationArtifactWriteRequestIssue, OpenDalWritePhase,
    WriteKeyOutcome, WritePolicy, write_compilation_artifacts,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::pack_archive::CommitCertainty;
use typst_pack::{
    CompilationArtifactWriteProgress, CompilationArtifactWriteReceipt, CompilationLimits,
    CompilationOutputSpecification, CompilationResult, CompilationStatus, Pack,
    PackCompilationRequest, SvgOutputSpecification, compile_with_limits,
};

#[test]
fn request_aggregates_rejection_issues_in_canonical_order() {
    let result = rejected_result();
    let destination: Location = "artifacts:/output".parse().unwrap();

    let rejection = CompilationArtifactWriteRequest::new(
        &result,
        destination.clone(),
        [
            "",
            "/bad",
            "/bad",
            "trailing/",
            "repeated//separator",
            "dot/../segment",
            "back\\slash",
            "control\u{7f}",
            " alias",
        ],
        WritePolicy::CreateOrVerify,
    )
    .unwrap_err();

    assert_eq!(
        rejection.compilation_result_identity(),
        result.result_identity()
    );
    assert_eq!(
        rejection.issues(),
        [
            CompilationArtifactWriteRequestIssue::ResultNotSucceeded,
            CompilationArtifactWriteRequestIssue::InvalidDestinationRole {
                location: destination,
                source: LocationRoleError::PrefixMissingTrailingSlash,
            },
            CompilationArtifactWriteRequestIssue::ArtifactKeyCountMismatch {
                expected: 0,
                actual: 9,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 0,
                key: String::new(),
                reason: CompilationArtifactKeyIssue::Empty,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 1,
                key: "/bad".to_owned(),
                reason: CompilationArtifactKeyIssue::LeadingSlash,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 2,
                key: "/bad".to_owned(),
                reason: CompilationArtifactKeyIssue::LeadingSlash,
            },
            CompilationArtifactWriteRequestIssue::DuplicateArtifactKey {
                key: "/bad".to_owned(),
                first_artifact_index: 1,
                duplicate_artifact_index: 2,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 3,
                key: "trailing/".to_owned(),
                reason: CompilationArtifactKeyIssue::TrailingSlash,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 4,
                key: "repeated//separator".to_owned(),
                reason: CompilationArtifactKeyIssue::RepeatedSeparator,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 5,
                key: "dot/../segment".to_owned(),
                reason: CompilationArtifactKeyIssue::DotSegment,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 6,
                key: "back\\slash".to_owned(),
                reason: CompilationArtifactKeyIssue::Backslash,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 7,
                key: "control\u{7f}".to_owned(),
                reason: CompilationArtifactKeyIssue::ControlCharacter,
            },
            CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 8,
                key: " alias".to_owned(),
                reason: CompilationArtifactKeyIssue::NormalizationAlias { index: 0 },
            },
        ]
    );
    let rendered = rejection.to_string();
    assert!(rendered.contains("artifacts"));
    assert!(rendered.contains("prefix"));
    assert!(rendered.contains("output"));
}

#[test]
fn request_accepts_literal_percent_and_ancestor_keys_without_uri_decoding() {
    let result = two_page_result();
    let destination: Location = "artifacts:/output/".parse().unwrap();

    let request = CompilationArtifactWriteRequest::new(
        &result,
        destination.clone(),
        ["tree%", "tree%/page%2F.svg"],
        WritePolicy::OverwriteExactKeys,
    )
    .unwrap();

    assert_eq!(
        request.compilation_result_identity(),
        result.result_identity()
    );
    assert_eq!(request.destination(), &destination);
    assert_eq!(request.artifact_keys(), ["tree%", "tree%/page%2F.svg"]);
    assert_eq!(request.policy(), WritePolicy::OverwriteExactKeys);
}

#[test]
fn artifact_key_reasons_use_documented_variant_precedence() {
    let result = two_page_result();
    let destination: Location = "artifacts:/output/".parse().unwrap();

    for (key, expected) in [
        (
            "repeat//control\u{7f}",
            CompilationArtifactKeyIssue::RepeatedSeparator,
        ),
        (
            "dot/../back\\slash",
            CompilationArtifactKeyIssue::DotSegment,
        ),
        (
            "back\\control\u{7f}",
            CompilationArtifactKeyIssue::Backslash,
        ),
    ] {
        let rejection = CompilationArtifactWriteRequest::new(
            &result,
            destination.clone(),
            [key, "valid.svg"],
            WritePolicy::CreateOrVerify,
        )
        .unwrap_err();
        assert!(matches!(
            rejection.issues(),
            [CompilationArtifactWriteRequestIssue::InvalidArtifactKey {
                artifact_index: 0,
                reason,
                ..
            }] if *reason == expected
        ));
    }
}

#[test]
fn overwrite_writes_exact_artifact_bytes_and_complete_evidence_in_order() {
    let result = two_page_result();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [("unrelated.bin".to_owned(), b"untouched".to_vec())],
        [],
        [
            WriteScript::new("output/document.svg", WriteCondition::Direct, []),
            WriteScript::new("output/pages/2.svg", WriteCondition::Direct, []),
        ],
        32,
    );
    let bindings = bindings(&service);
    let request = CompilationArtifactWriteRequest::new(
        &result,
        "artifacts:/output/".parse().unwrap(),
        ["document.svg", "pages/2.svg"],
        WritePolicy::OverwriteExactKeys,
    )
    .unwrap();
    let identity = result.result_identity();
    let mut progress = CompilationArtifactWriteProgress::new();

    let receipt: CompilationArtifactWriteReceipt = expect_ready(pin!(write_compilation_artifacts(
        &bindings,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap();

    assert_eq!(result.result_identity(), identity);
    assert_eq!(receipt.compilation_result_identity(), identity);
    assert_eq!(receipt.progress(), &progress);
    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| (entry.artifact_index(), entry.outcome()))
            .collect::<Vec<_>>(),
        [(0, WriteKeyOutcome::Written), (1, WriteKeyOutcome::Written),]
    );
    assert_eq!(
        service.destination().object("output/document.svg"),
        Some(result.artifacts()[0].bytes())
    );
    assert_eq!(
        service.destination().object("output/pages/2.svg"),
        Some(result.artifacts()[1].bytes())
    );
    assert_eq!(
        service.destination().object("unrelated.bin"),
        Some(b"untouched".as_slice())
    );
}

#[test]
fn result_mismatch_fails_before_operator_resolution() {
    let expected = two_page_result();
    let actual = compilation_result("different");
    let request = CompilationArtifactWriteRequest::new(
        &expected,
        "artifacts:/output/".parse().unwrap(),
        ["one.svg", "two.svg"],
        WritePolicy::CreateOrVerify,
    )
    .unwrap();
    let mut progress = CompilationArtifactWriteProgress::new();

    let error = expect_ready(pin!(write_compilation_artifacts(
        &RejectingResolver,
        &request,
        &actual,
        &mut progress,
    )))
    .unwrap_err();

    assert_eq!(error.phase(), OpenDalWritePhase::ResultValidation);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(error.progress().completed().is_empty());
    assert!(matches!(
        error.cause(),
        CompilationArtifactWriteErrorCause::CompilationResultMismatch {
            expected: expected_identity,
            actual: actual_identity,
        } if *expected_identity == expected.result_identity()
            && *actual_identity == actual.result_identity()
    ));
}

#[test]
fn dropping_mid_write_leaves_the_completed_prefix_in_caller_progress() {
    let result = two_page_result();
    let pending = PendingPoint::new();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [
            WriteScript::new("output/one.svg", WriteCondition::Direct, []),
            WriteScript::new(
                "output/two.svg",
                WriteCondition::Direct,
                [WriteStep::pending(pending.clone())],
            ),
        ],
        32,
    );
    let bindings = bindings(&service);
    let request = CompilationArtifactWriteRequest::new(
        &result,
        "artifacts:/output/".parse().unwrap(),
        ["one.svg", "two.svg"],
        WritePolicy::OverwriteExactKeys,
    )
    .unwrap();
    let mut progress = CompilationArtifactWriteProgress::new();
    {
        let mut write = pin!(write_compilation_artifacts(
            &bindings,
            &request,
            &result,
            &mut progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(progress.completed().len(), 1);
    assert_eq!(progress.completed()[0].artifact_index(), 0);
    assert_eq!(progress.completed()[0].outcome(), WriteKeyOutcome::Written);
}

#[test]
fn create_or_verify_completes_all_comparisons_before_creating_absent_artifacts() {
    let result = two_page_result();
    let first_bytes = result.artifacts()[0].bytes().to_vec();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [("output/one.svg".to_owned(), first_bytes.clone())],
        [WriteReadScript::new(
            "output/one.svg",
            2,
            [
                WriteReadStep::chunk(0..1),
                WriteReadStep::chunk(1..first_bytes.len()),
            ],
        )
        .unwrap()],
        [WriteScript::new(
            "output/two.svg",
            WriteCondition::IfNotExists,
            [],
        )],
        32,
    );
    let bindings = bindings(&service);
    let request = CompilationArtifactWriteRequest::new(
        &result,
        "artifacts:/output/".parse().unwrap(),
        ["one.svg", "two.svg"],
        WritePolicy::CreateOrVerify,
    )
    .unwrap();
    let mut progress = CompilationArtifactWriteProgress::new();

    let receipt = expect_ready(pin!(write_compilation_artifacts(
        &bindings,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap();

    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| entry.outcome())
            .collect::<Vec<_>>(),
        [WriteKeyOutcome::AlreadyMatching, WriteKeyOutcome::Created,]
    );
    let log = service.log();
    let second_read = log
        .entries()
        .iter()
        .position(|entry| matches!(entry, WriteOperationLogEntry::ReadInvoked { path, .. } if path == "output/two.svg"))
        .unwrap();
    let first_write = log
        .entries()
        .iter()
        .position(|entry| matches!(entry, WriteOperationLogEntry::WriteInvoked { .. }))
        .unwrap();
    assert!(second_read < first_write);
}

#[test]
fn matching_create_or_verify_receipt_reports_the_read_only_outcome() {
    let result = compilation_result("matching");
    let bytes = result.artifacts()[0].bytes().to_vec();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [("output/document.svg".to_owned(), bytes.clone())],
        [WriteReadScript::new(
            "output/document.svg",
            1,
            [WriteReadStep::chunk(0..bytes.len())],
        )
        .unwrap()],
        [],
        16,
    );
    let bindings = bindings(&service);
    let request = request_for(&result, ["document.svg"], WritePolicy::CreateOrVerify);
    let mut progress = CompilationArtifactWriteProgress::new();

    let receipt = expect_ready(pin!(write_compilation_artifacts(
        &bindings,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap();

    assert_eq!(
        receipt.completed()[0].outcome(),
        WriteKeyOutcome::AlreadyMatching
    );
}

#[test]
fn mutable_comparison_and_conditional_race_project_read_only_success_evidence() {
    let mutable_result = compilation_result("mutable comparison");
    let mutable_bytes = mutable_result.artifacts()[0].bytes().to_vec();
    let split = mutable_bytes.len() / 2;
    let mutable_service = WriteService::new(
        WriteCapabilities::all(),
        [("output/document.svg".to_owned(), mutable_bytes.clone())],
        [WriteReadScript::new(
            "output/document.svg",
            2,
            [
                WriteReadStep::chunk(0..split),
                WriteReadStep::mutate(scripted_opendal::DestinationMutation::set(
                    "output/document.svg",
                    &mutable_bytes,
                )),
                WriteReadStep::chunk(split..mutable_bytes.len()),
            ],
        )
        .unwrap()],
        [],
        24,
    );
    let mutable_bindings = bindings(&mutable_service);
    let mutable_request = request_for(
        &mutable_result,
        ["document.svg"],
        WritePolicy::CreateOrVerify,
    );
    let mut mutable_progress = CompilationArtifactWriteProgress::new();
    let mutable_receipt = expect_ready(pin!(write_compilation_artifacts(
        &mutable_bindings,
        &mutable_request,
        &mutable_result,
        &mut mutable_progress,
    )))
    .unwrap();
    assert_eq!(
        mutable_receipt.completed()[0].outcome(),
        WriteKeyOutcome::AlreadyMatching
    );

    let race_result = compilation_result("race verification");
    let race_bytes = race_result.artifacts()[0].bytes().to_vec();
    let pending = PendingPoint::new();
    let race_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [
            WriteReadScript::new(
                "output/document.svg",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            WriteReadScript::new(
                "output/document.svg",
                1,
                [WriteReadStep::chunk(0..race_bytes.len())],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "output/document.svg",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        32,
    );
    let race_bindings = bindings(&race_service);
    let race_request = request_for(&race_result, ["document.svg"], WritePolicy::CreateOrVerify);
    let mut race_progress = CompilationArtifactWriteProgress::new();
    let race_receipt = {
        let mut write = pin!(write_compilation_artifacts(
            &race_bindings,
            &race_request,
            &race_result,
            &mut race_progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        race_service.mutate(scripted_opendal::DestinationMutation::set(
            "output/document.svg",
            race_bytes,
        ));
        pending.release();
        expect_ready(write.as_mut()).unwrap()
    };
    assert_eq!(
        race_receipt.completed()[0].outcome(),
        WriteKeyOutcome::AlreadyMatching
    );
}

#[test]
fn failed_race_verification_retains_typed_cause_and_not_committed_certainty() {
    let result = compilation_result("failed race verification");
    let request = request_for(&result, ["document.svg"], WritePolicy::CreateOrVerify);

    let conflict_pending = PendingPoint::new();
    let conflict_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [
            WriteReadScript::new(
                "output/document.svg",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            WriteReadScript::new("output/document.svg", 1, [WriteReadStep::chunk(0..1)]).unwrap(),
        ],
        [WriteScript::new(
            "output/document.svg",
            WriteCondition::IfNotExists,
            [
                WriteStep::pending(conflict_pending.clone()),
                WriteStep::commit(),
            ],
        )],
        32,
    );
    let conflict_bindings = bindings(&conflict_service);
    let mut conflict_progress = CompilationArtifactWriteProgress::new();
    let conflict = {
        let mut write = pin!(write_compilation_artifacts(
            &conflict_bindings,
            &request,
            &result,
            &mut conflict_progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        conflict_service.mutate(scripted_opendal::DestinationMutation::set(
            "output/document.svg",
            b"wrong",
        ));
        conflict_pending.release();
        expect_ready(write.as_mut()).unwrap_err()
    };
    assert_eq!(conflict.phase(), OpenDalWritePhase::RaceVerification);
    assert_eq!(conflict.failed_artifact_index(), Some(0));
    assert_eq!(conflict.failed_key(), Some("document.svg"));
    assert_eq!(conflict.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(conflict.progress().completed().is_empty());
    assert!(matches!(
        conflict.cause(),
        CompilationArtifactWriteErrorCause::ByteConflict { .. }
    ));

    let read_pending = PendingPoint::new();
    let read_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [
            WriteReadScript::new(
                "output/document.svg",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            WriteReadScript::new(
                "output/document.svg",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::PermissionDenied)],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "output/document.svg",
            WriteCondition::IfNotExists,
            [
                WriteStep::pending(read_pending.clone()),
                WriteStep::commit(),
            ],
        )],
        32,
    );
    let read_bindings = bindings(&read_service);
    let mut read_progress = CompilationArtifactWriteProgress::new();
    let read_error = {
        let mut write = pin!(write_compilation_artifacts(
            &read_bindings,
            &request,
            &result,
            &mut read_progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        read_service.mutate(scripted_opendal::DestinationMutation::set(
            "output/document.svg",
            b"racing object",
        ));
        read_pending.release();
        expect_ready(write.as_mut()).unwrap_err()
    };
    assert_eq!(read_error.phase(), OpenDalWritePhase::RaceVerification);
    assert_eq!(read_error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(read_error.progress().completed().is_empty());
    assert!(matches!(
        read_error.cause(),
        CompilationArtifactWriteErrorCause::RaceVerification(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
}

#[test]
fn conflict_and_write_failure_retain_failed_artifact_context_and_contiguous_progress() {
    let result = two_page_result();
    let conflict_service = WriteService::new(
        WriteCapabilities::all(),
        [("output/one.svg".to_owned(), b"different".to_vec())],
        [WriteReadScript::new("output/one.svg", 1, [WriteReadStep::chunk(0..1)]).unwrap()],
        [],
        16,
    );
    let conflict_bindings = bindings(&conflict_service);
    let create_request = request_for(&result, ["one.svg", "two.svg"], WritePolicy::CreateOrVerify);
    let mut conflict_progress = CompilationArtifactWriteProgress::new();
    let conflict = expect_ready(pin!(write_compilation_artifacts(
        &conflict_bindings,
        &create_request,
        &result,
        &mut conflict_progress,
    )))
    .unwrap_err();
    assert_eq!(conflict.failed_artifact_index(), Some(0));
    assert_eq!(conflict.failed_key(), Some("one.svg"));
    assert_eq!(conflict.failed_destination_path(), Some("output/one.svg"));
    assert_eq!(conflict.phase(), OpenDalWritePhase::PreflightRead);
    assert_eq!(conflict.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        conflict.cause(),
        CompilationArtifactWriteErrorCause::ByteConflict { .. }
    ));

    let failure_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [
            WriteScript::new("output/one.svg", WriteCondition::Direct, []),
            WriteScript::write_failure(
                "output/two.svg",
                WriteCondition::Direct,
                opendal::ErrorKind::PermissionDenied,
            ),
        ],
        24,
    );
    let failure_bindings = bindings(&failure_service);
    let overwrite_request = request_for(
        &result,
        ["one.svg", "two.svg"],
        WritePolicy::OverwriteExactKeys,
    );
    let mut failure_progress = CompilationArtifactWriteProgress::new();
    let failure = expect_ready(pin!(write_compilation_artifacts(
        &failure_bindings,
        &overwrite_request,
        &result,
        &mut failure_progress,
    )))
    .unwrap_err();
    assert_eq!(failure.failed_artifact_index(), Some(1));
    assert_eq!(failure.failed_key(), Some("two.svg"));
    assert_eq!(failure.failed_destination_path(), Some("output/two.svg"));
    assert_eq!(failure.phase(), OpenDalWritePhase::DirectWrite);
    assert_eq!(failure.commit_certainty(), CommitCertainty::Indeterminate);
    assert_eq!(failure.progress().completed().len(), 1);
    assert!(matches!(
        failure.cause(),
        CompilationArtifactWriteErrorCause::DirectWrite(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
}

#[test]
fn capability_and_size_failures_are_projected_without_effects() {
    let result = compilation_result("capabilities");
    let request = request_for(&result, ["document.svg"], WritePolicy::CreateOrVerify);
    let unsupported_service = WriteService::new(
        WriteCapabilities {
            read: false,
            ..WriteCapabilities::all()
        },
        [],
        [],
        [],
        8,
    );
    let unsupported_bindings = bindings(&unsupported_service);
    let mut progress = CompilationArtifactWriteProgress::new();
    let unsupported = expect_ready(pin!(write_compilation_artifacts(
        &unsupported_bindings,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap_err();
    assert!(matches!(
        unsupported.cause(),
        CompilationArtifactWriteErrorCause::UnsupportedPolicy {
            policy: WritePolicy::CreateOrVerify,
        }
    ));
    assert_eq!(
        unsupported.commit_certainty(),
        CommitCertainty::NotCommitted
    );
    assert!(unsupported_service.log().entries().is_empty());

    let size_service = WriteService::new(
        WriteCapabilities {
            write_total_max_size: Some(0),
            ..WriteCapabilities::all()
        },
        [],
        [],
        [],
        8,
    );
    let size_bindings = bindings(&size_service);
    let overwrite_request = request_for(&result, ["document.svg"], WritePolicy::OverwriteExactKeys);
    let mut progress = CompilationArtifactWriteProgress::new();
    let size = expect_ready(pin!(write_compilation_artifacts(
        &size_bindings,
        &overwrite_request,
        &result,
        &mut progress,
    )))
    .unwrap_err();
    assert!(matches!(
        size.cause(),
        CompilationArtifactWriteErrorCause::UnsupportedObjectSize {
            artifact_index: 0,
            byte_length,
        } if *byte_length == result.artifacts()[0].bytes().len() as u64
    ));
    assert_eq!(size.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(size_service.log().entries().is_empty());
}

#[test]
fn empty_succeeded_results_skip_resolution_and_unpolled_futures_clear_stale_progress() {
    let empty = empty_result();
    let empty_request = request_for(
        &empty,
        std::iter::empty::<&str>(),
        WritePolicy::CreateOrVerify,
    );
    let mut progress = CompilationArtifactWriteProgress::new();
    let receipt = expect_ready(pin!(write_compilation_artifacts(
        &RejectingResolver,
        &empty_request,
        &empty,
        &mut progress,
    )))
    .unwrap();
    assert!(receipt.completed().is_empty());

    let result = compilation_result("stale progress");
    let service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "output/document.svg",
            WriteCondition::Direct,
            [],
        )],
        8,
    );
    let bindings = bindings(&service);
    let request = request_for(&result, ["document.svg"], WritePolicy::OverwriteExactKeys);
    expect_ready(pin!(write_compilation_artifacts(
        &bindings,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap();
    assert_eq!(progress.completed().len(), 1);
    drop(write_compilation_artifacts(
        &RejectingResolver,
        &request,
        &result,
        &mut progress,
    ));
    assert!(progress.completed().is_empty());
}

#[test]
fn operator_bindings_produce_a_send_write_future() {
    let result = compilation_result("send");
    let service = WriteService::new(WriteCapabilities::all(), [], [], [], 1);
    let bindings = bindings(&service);
    let request = request_for(&result, ["document.svg"], WritePolicy::CreateOrVerify);
    let mut progress = CompilationArtifactWriteProgress::new();

    assert_send(write_compilation_artifacts(
        &bindings,
        &request,
        &result,
        &mut progress,
    ));
}

#[test]
fn resolver_failure_is_boxed_beneath_the_public_cause() {
    let result = compilation_result("resolver failure");
    let request = request_for(&result, ["document.svg"], WritePolicy::CreateOrVerify);
    let mut progress = CompilationArtifactWriteProgress::new();

    let error = expect_ready(pin!(write_compilation_artifacts(
        &FailingResolver,
        &request,
        &result,
        &mut progress,
    )))
    .unwrap_err();

    assert_eq!(error.phase(), OpenDalWritePhase::ResolveOperator);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    let CompilationArtifactWriteErrorCause::ResolveOperator(source) = error.cause() else {
        panic!("unexpected cause: {:?}", error.cause());
    };
    assert!(source.downcast_ref::<ResolveError>().is_some());
    assert!(!format!("{error:?}").contains("resolver rejected"));
    let cause = error.source().unwrap().source().unwrap();
    assert!(cause.is::<CompilationArtifactWriteErrorCause>());
    assert!(cause.source().unwrap().is::<ResolveError>());
}

fn two_page_result() -> CompilationResult {
    compilation_result(
        "#set page(width: 10pt, height: 10pt, margin: 0pt)\n\
         #rect(width: 1pt, height: 1pt)\n\
         #pagebreak()\n\
         #rect(width: 2pt, height: 2pt)",
    )
}

fn rejected_result() -> CompilationResult {
    let result = compilation_result("#let =");
    assert_eq!(result.status(), CompilationStatus::Rejected);
    result
}

fn empty_result() -> CompilationResult {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"empty result".to_vec())
        .unwrap()
        .build()
        .unwrap();
    compile_with_limits(
        PackCompilationRequest::new(
            pack,
            CompilationOutputSpecification::Svg(SvgOutputSpecification {
                page_selection: typst_pack::parse_page_selection("9").unwrap(),
                ..SvgOutputSpecification::default()
            }),
        ),
        CompilationLimits::reference_v1(),
    )
    .unwrap()
    .result()
    .unwrap()
    .clone()
}

fn compilation_result(source: &str) -> CompilationResult {
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
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
    .expect("compilation should produce a semantic result")
    .clone()
}

fn bindings(service: &WriteService) -> OperatorBindings {
    OperatorBindings::new([(
        OperatorBinding::new("artifacts").unwrap(),
        service.operator(),
    )])
    .unwrap()
}

fn request_for(
    result: &CompilationResult,
    keys: impl IntoIterator<Item = &'static str>,
    policy: WritePolicy,
) -> CompilationArtifactWriteRequest {
    CompilationArtifactWriteRequest::new(
        result,
        "artifacts:/output/".parse().unwrap(),
        keys,
        policy,
    )
    .unwrap()
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

fn assert_send<T: Send>(_: T) {}

struct RejectingResolver;

impl OperatorResolver for RejectingResolver {
    type Error = std::convert::Infallible;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, std::convert::Infallible> {
        unreachable!("result validation must precede operator resolution")
    }
}

struct FailingResolver;

impl OperatorResolver for FailingResolver {
    type Error = ResolveError;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        Err(ResolveError)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("resolver rejected")]
struct ResolveError;
