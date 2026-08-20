#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::{error::Error, fmt};

use scripted_opendal::{
    DestinationMutation, PendingPoint, WriteCapabilities, WriteCondition, WriteOperationLogEntry,
    WriteReadScript, WriteReadStep, WriteScript, WriteService, WriteStep,
};
use typst_pack::opendal::write::{
    OpenDalWritePhase, PackExtractionWriteErrorCause, PackExtractionWriteRequest,
    PackExtractionWriteRequestError, WriteKeyOutcome, WritePolicy, write_pack_extraction_plan,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::pack_archive::CommitCertainty;
use typst_pack::{
    Pack, PackExtractionSelection, PackExtractionWriteProgress, PackExtractionWriteReceipt,
    plan_pack_extraction,
};

#[test]
fn request_accepts_normalized_prefixes_and_rejects_exact_objects() {
    for destination in ["project:/", "project:/extracted/"] {
        let destination: Location = destination.parse().unwrap();
        let request =
            PackExtractionWriteRequest::new(destination.clone(), WritePolicy::CreateOrVerify)
                .unwrap();

        assert_eq!(request.destination(), &destination);
        assert_eq!(request.policy(), WritePolicy::CreateOrVerify);
    }

    let destination: Location = "project:/extracted".parse().unwrap();
    let error =
        PackExtractionWriteRequest::new(destination.clone(), WritePolicy::OverwriteExactKeys)
            .unwrap_err();

    assert_eq!(
        error,
        PackExtractionWriteRequestError::InvalidDestinationRole {
            location: destination,
            source: LocationRoleError::PrefixMissingTrailingSlash,
        }
    );
}

#[test]
fn overwrite_writes_exact_plan_bytes_and_complete_evidence_in_plan_order() {
    let plan = sample_plan();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [("stale.bin".to_owned(), b"untouched".to_vec())],
        [],
        [
            WriteScript::new("extracted/assets/logo.bin", WriteCondition::Direct, []),
            WriteScript::new("extracted/main.typ", WriteCondition::Direct, []),
        ],
        16,
    );
    let bindings = bindings(&service);
    let request = request(WritePolicy::OverwriteExactKeys);
    let mut progress = PackExtractionWriteProgress::new();

    let receipt: PackExtractionWriteReceipt = expect_ready(pin!(write_pack_extraction_plan(
        &bindings,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap();

    assert_eq!(receipt.pack_identity(), *plan.pack_identity());
    assert_eq!(receipt.progress(), &progress);
    assert_eq!(
        receipt
            .completed()
            .iter()
            .map(|entry| (entry.relative_path(), entry.outcome()))
            .collect::<Vec<_>>(),
        [
            ("assets/logo.bin", WriteKeyOutcome::Written),
            ("main.typ", WriteKeyOutcome::Written),
        ]
    );
    assert_eq!(
        service.destination().object("extracted/assets/logo.bin"),
        Some(b"logo".as_slice())
    );
    assert_eq!(
        service.destination().object("extracted/main.typ"),
        Some(b"main".as_slice())
    );
    assert_eq!(
        service.destination().object("stale.bin"),
        Some(b"untouched".as_slice())
    );
}

#[test]
fn every_composed_path_is_validated_before_operator_resolution() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("z ", b"invalid OpenDAL alias".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let request = request(WritePolicy::CreateOrVerify);
    let mut progress = PackExtractionWriteProgress::new();

    let error = expect_ready(pin!(write_pack_extraction_plan(
        &RejectingResolver,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();

    assert_eq!(error.destination(), request.destination());
    assert_eq!(error.policy(), WritePolicy::CreateOrVerify);
    assert_eq!(error.failed_relative_path(), Some("z "));
    assert_eq!(error.failed_destination_path(), None);
    assert_eq!(error.phase(), OpenDalWritePhase::DestinationValidation);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(error.progress().completed().is_empty());
    assert!(progress.completed().is_empty());
    assert!(matches!(
        error.cause(),
        PackExtractionWriteErrorCause::InvalidDestinationPath { relative_path }
            if relative_path == "z "
    ));
}

#[test]
fn create_or_verify_preflights_every_entry_then_reports_matching_and_created_entries() {
    let plan = sample_plan();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [("extracted/assets/logo.bin".to_owned(), b"logo".to_vec())],
        [
            WriteReadScript::new(
                "extracted/assets/logo.bin",
                2,
                [WriteReadStep::chunk(0..2), WriteReadStep::chunk(2..4)],
            )
            .unwrap(),
            WriteReadScript::new(
                "extracted/main.typ",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "extracted/main.typ",
            WriteCondition::IfNotExists,
            [],
        )],
        32,
    );
    let bindings = bindings(&service);
    let request = request(WritePolicy::CreateOrVerify);
    let mut progress = PackExtractionWriteProgress::new();

    let receipt = expect_ready(pin!(write_pack_extraction_plan(
        &bindings,
        &request,
        &plan,
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
        .position(|entry| matches!(entry, WriteOperationLogEntry::ReadInvoked { path, .. } if path == "extracted/main.typ"))
        .unwrap();
    let first_write = log
        .entries()
        .iter()
        .position(|entry| matches!(entry, WriteOperationLogEntry::WriteInvoked { .. }))
        .unwrap();
    assert!(second_read < first_write);
}

#[test]
fn conflicts_and_write_failures_retain_entry_context_and_contiguous_progress() {
    let plan = sample_plan();
    let create_request = request(WritePolicy::CreateOrVerify);
    let conflict_service = WriteService::new(
        WriteCapabilities::all(),
        [("extracted/assets/logo.bin".to_owned(), b"wrong".to_vec())],
        [
            WriteReadScript::new("extracted/assets/logo.bin", 1, [WriteReadStep::chunk(0..1)])
                .unwrap(),
        ],
        [],
        16,
    );
    let conflict_bindings = bindings(&conflict_service);
    let mut conflict_progress = PackExtractionWriteProgress::new();

    let conflict = expect_ready(pin!(write_pack_extraction_plan(
        &conflict_bindings,
        &create_request,
        &plan,
        &mut conflict_progress,
    )))
    .unwrap_err();

    assert_eq!(conflict.failed_relative_path(), Some("assets/logo.bin"));
    assert_eq!(
        conflict.failed_destination_path(),
        Some("extracted/assets/logo.bin")
    );
    assert_eq!(conflict.phase(), OpenDalWritePhase::PreflightRead);
    assert_eq!(conflict.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(conflict.progress().completed().is_empty());
    assert!(matches!(
        conflict.cause(),
        PackExtractionWriteErrorCause::ByteConflict {
            expected_byte_length: 4,
            observed_byte_length_at_least: 1,
        }
    ));

    let failure_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [
            WriteScript::new("extracted/assets/logo.bin", WriteCondition::Direct, []),
            WriteScript::write_failure(
                "extracted/main.typ",
                WriteCondition::Direct,
                opendal::ErrorKind::PermissionDenied,
            ),
        ],
        24,
    );
    let failure_bindings = bindings(&failure_service);
    let overwrite_request = request(WritePolicy::OverwriteExactKeys);
    let mut failure_progress = PackExtractionWriteProgress::new();

    let failure = expect_ready(pin!(write_pack_extraction_plan(
        &failure_bindings,
        &overwrite_request,
        &plan,
        &mut failure_progress,
    )))
    .unwrap_err();

    assert_eq!(failure.failed_relative_path(), Some("main.typ"));
    assert_eq!(
        failure.failed_destination_path(),
        Some("extracted/main.typ")
    );
    assert_eq!(failure.phase(), OpenDalWritePhase::DirectWrite);
    assert_eq!(failure.commit_certainty(), CommitCertainty::Indeterminate);
    assert_eq!(failure.progress().completed().len(), 1);
    assert_eq!(failure.progress(), &failure_progress);
    assert!(matches!(
        failure.cause(),
        PackExtractionWriteErrorCause::DirectWrite(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
}

#[test]
fn resolver_read_create_and_race_failures_retain_typed_public_causes() {
    let plan = main_only_plan();
    let request = request(WritePolicy::CreateOrVerify);
    let mut progress = PackExtractionWriteProgress::new();

    let resolve = expect_ready(pin!(write_pack_extraction_plan(
        &FailingResolver,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();
    assert_eq!(resolve.phase(), OpenDalWritePhase::ResolveOperator);
    let PackExtractionWriteErrorCause::ResolveOperator(source) = resolve.cause() else {
        panic!("unexpected cause: {:?}", resolve.cause());
    };
    assert!(source.downcast_ref::<ResolveError>().is_some());
    assert!(!format!("{resolve:?}").contains("resolver rejected"));
    let cause = resolve.source().unwrap().source().unwrap();
    assert!(cause.is::<PackExtractionWriteErrorCause>());
    assert!(cause.source().unwrap().is::<ResolveError>());

    let read_service = WriteService::new(
        WriteCapabilities::all(),
        [("extracted/main.typ".to_owned(), b"main".to_vec())],
        [WriteReadScript::new(
            "extracted/main.typ",
            0,
            [WriteReadStep::failure(opendal::ErrorKind::PermissionDenied)],
        )
        .unwrap()],
        [],
        8,
    );
    let read_bindings = bindings(&read_service);
    let read = expect_ready(pin!(write_pack_extraction_plan(
        &read_bindings,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();
    assert_eq!(read.phase(), OpenDalWritePhase::PreflightRead);
    assert!(matches!(
        read.cause(),
        PackExtractionWriteErrorCause::PreflightRead(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let create_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [WriteReadScript::new(
            "extracted/main.typ",
            0,
            [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::write_failure(
            "extracted/main.typ",
            WriteCondition::IfNotExists,
            opendal::ErrorKind::PermissionDenied,
        )],
        8,
    );
    let create_bindings = bindings(&create_service);
    let create = expect_ready(pin!(write_pack_extraction_plan(
        &create_bindings,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();
    assert_eq!(create.phase(), OpenDalWritePhase::ConditionalCreate);
    assert_eq!(create.commit_certainty(), CommitCertainty::Indeterminate);
    assert!(matches!(
        create.cause(),
        PackExtractionWriteErrorCause::ConditionalCreate(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let pending = PendingPoint::new();
    let race_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [
            WriteReadScript::new(
                "extracted/main.typ",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            WriteReadScript::new(
                "extracted/main.typ",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::PermissionDenied)],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "extracted/main.typ",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        16,
    );
    let race_bindings = bindings(&race_service);
    let race = {
        let mut write = pin!(write_pack_extraction_plan(
            &race_bindings,
            &request,
            &plan,
            &mut progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        race_service.mutate(DestinationMutation::set("extracted/main.typ", b"racing"));
        pending.release();
        expect_ready(write.as_mut()).unwrap_err()
    };
    assert_eq!(race.phase(), OpenDalWritePhase::RaceVerification);
    assert_eq!(race.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        race.cause(),
        PackExtractionWriteErrorCause::RaceVerification(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
}

#[test]
fn capability_and_size_failures_happen_before_destination_effects() {
    let plan = sample_plan();
    let create_request = request(WritePolicy::CreateOrVerify);
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
    let mut progress = PackExtractionWriteProgress::new();

    let unsupported = expect_ready(pin!(write_pack_extraction_plan(
        &unsupported_bindings,
        &create_request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();

    assert_eq!(unsupported.phase(), OpenDalWritePhase::CapabilityAppraisal);
    assert!(matches!(
        unsupported.cause(),
        PackExtractionWriteErrorCause::UnsupportedPolicy {
            policy: WritePolicy::CreateOrVerify,
        }
    ));
    assert!(unsupported_service.log().entries().is_empty());

    let size_service = WriteService::new(
        WriteCapabilities {
            write_total_max_size: Some(3),
            ..WriteCapabilities::all()
        },
        [],
        [],
        [],
        8,
    );
    let size_bindings = bindings(&size_service);
    let overwrite_request = request(WritePolicy::OverwriteExactKeys);
    let mut progress = PackExtractionWriteProgress::new();
    let size = expect_ready(pin!(write_pack_extraction_plan(
        &size_bindings,
        &overwrite_request,
        &plan,
        &mut progress,
    )))
    .unwrap_err();

    assert_eq!(size.failed_relative_path(), Some("assets/logo.bin"));
    assert!(matches!(
        size.cause(),
        PackExtractionWriteErrorCause::UnsupportedObjectSize { byte_length: 4 }
    ));
    assert!(size_service.log().entries().is_empty());
}

#[test]
fn mutable_streams_and_matching_conditional_races_are_read_only_successes() {
    let plan = main_only_plan();
    let mutable_service = WriteService::new(
        WriteCapabilities::all(),
        [("extracted/main.typ".to_owned(), b"maXX".to_vec())],
        [WriteReadScript::new(
            "extracted/main.typ",
            2,
            [
                WriteReadStep::chunk(0..2),
                WriteReadStep::mutate(DestinationMutation::set("extracted/main.typ", b"YYin")),
                WriteReadStep::chunk(2..4),
            ],
        )
        .unwrap()],
        [],
        24,
    );
    let mutable_bindings = bindings(&mutable_service);
    let request = request(WritePolicy::CreateOrVerify);
    let mut mutable_progress = PackExtractionWriteProgress::new();
    let mutable_receipt = expect_ready(pin!(write_pack_extraction_plan(
        &mutable_bindings,
        &request,
        &plan,
        &mut mutable_progress,
    )))
    .unwrap();
    assert_eq!(
        mutable_receipt.completed()[0].outcome(),
        WriteKeyOutcome::AlreadyMatching
    );

    let pending = PendingPoint::new();
    let race_service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [
            WriteReadScript::new(
                "extracted/main.typ",
                0,
                [WriteReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            WriteReadScript::new("extracted/main.typ", 1, [WriteReadStep::chunk(0..4)]).unwrap(),
        ],
        [WriteScript::new(
            "extracted/main.typ",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        32,
    );
    let race_bindings = bindings(&race_service);
    let mut race_progress = PackExtractionWriteProgress::new();
    let race_receipt = {
        let mut write = pin!(write_pack_extraction_plan(
            &race_bindings,
            &request,
            &plan,
            &mut race_progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        race_service.mutate(DestinationMutation::set("extracted/main.typ", b"main"));
        pending.release();
        expect_ready(write.as_mut()).unwrap()
    };
    assert_eq!(
        race_receipt.completed()[0].outcome(),
        WriteKeyOutcome::AlreadyMatching
    );
}

#[test]
fn dropping_mid_plan_leaves_the_contiguous_completed_prefix_with_the_caller() {
    let plan = sample_plan();
    let pending = PendingPoint::new();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [
            WriteScript::new("extracted/assets/logo.bin", WriteCondition::Direct, []),
            WriteScript::new(
                "extracted/main.typ",
                WriteCondition::Direct,
                [WriteStep::pending(pending.clone())],
            ),
        ],
        24,
    );
    let bindings = bindings(&service);
    let request = request(WritePolicy::OverwriteExactKeys);
    let mut progress = PackExtractionWriteProgress::new();
    {
        let mut write = pin!(write_pack_extraction_plan(
            &bindings,
            &request,
            &plan,
            &mut progress,
        ));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(progress.completed().len(), 1);
    assert_eq!(progress.completed()[0].relative_path(), "assets/logo.bin");
    assert_eq!(progress.completed()[0].outcome(), WriteKeyOutcome::Written);
}

#[test]
fn memory_proves_exact_root_state_and_create_or_verify_replay() {
    let plan = sample_plan();
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    expect_ready(pin!(operator.write("unrelated.bin", b"untouched".to_vec()))).unwrap();
    let bindings =
        OperatorBindings::new([(OperatorBinding::new("memory").unwrap(), operator.clone())])
            .unwrap();
    let overwrite = PackExtractionWriteRequest::new(
        "memory:/".parse().unwrap(),
        WritePolicy::OverwriteExactKeys,
    )
    .unwrap();
    let mut progress = PackExtractionWriteProgress::new();

    expect_ready(pin!(write_pack_extraction_plan(
        &bindings,
        &overwrite,
        &plan,
        &mut progress,
    )))
    .unwrap();
    for entry in plan.entries() {
        let bytes = expect_ready(pin!(operator.read(entry.relative_path()))).unwrap();
        assert_eq!(bytes.to_vec(), entry.bytes());
    }
    assert_eq!(
        expect_ready(pin!(operator.read("unrelated.bin")))
            .unwrap()
            .to_vec(),
        b"untouched"
    );

    let replay =
        PackExtractionWriteRequest::new("memory:/".parse().unwrap(), WritePolicy::CreateOrVerify)
            .unwrap();
    let receipt = expect_ready(pin!(write_pack_extraction_plan(
        &bindings,
        &replay,
        &plan,
        &mut progress,
    )))
    .unwrap();
    assert!(
        receipt
            .completed()
            .iter()
            .all(|entry| entry.outcome() == WriteKeyOutcome::AlreadyMatching)
    );
}

#[test]
fn unpolled_futures_clear_stale_progress_and_operator_bindings_make_send_futures() {
    let plan = main_only_plan();
    let service = WriteService::new(
        WriteCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "extracted/main.typ",
            WriteCondition::Direct,
            [],
        )],
        8,
    );
    let bindings = bindings(&service);
    let request = request(WritePolicy::OverwriteExactKeys);
    let mut progress = PackExtractionWriteProgress::new();
    expect_ready(pin!(write_pack_extraction_plan(
        &bindings,
        &request,
        &plan,
        &mut progress,
    )))
    .unwrap();
    assert_eq!(progress.completed().len(), 1);

    drop(write_pack_extraction_plan(
        &RejectingResolver,
        &request,
        &plan,
        &mut progress,
    ));
    assert!(progress.completed().is_empty());
    assert_send(write_pack_extraction_plan(
        &bindings,
        &request,
        &plan,
        &mut progress,
    ));
}

fn sample_plan() -> typst_pack::PackExtractionPlan {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .file("assets/logo.bin", b"logo".to_vec())
        .unwrap()
        .build()
        .unwrap();
    plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap()
}

fn main_only_plan() -> typst_pack::PackExtractionPlan {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"main".to_vec())
        .unwrap()
        .build()
        .unwrap();
    plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap()
}

fn bindings(service: &WriteService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("project").unwrap(), service.operator())]).unwrap()
}

fn request(policy: WritePolicy) -> PackExtractionWriteRequest {
    PackExtractionWriteRequest::new("project:/extracted/".parse().unwrap(), policy).unwrap()
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
        unreachable!("destination validation must precede operator resolution")
    }
}

#[derive(Debug)]
struct ResolveError;

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resolver rejected binding")
    }
}

impl Error for ResolveError {}

struct FailingResolver;

impl OperatorResolver for FailingResolver {
    type Error = ResolveError;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        Err(ResolveError)
    }
}
