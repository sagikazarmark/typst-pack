#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::{error::Error, fmt};

use scripted_opendal::{
    DestinationMutation, PendingPoint, PublicationCapabilities, PublicationOperationLogEntry,
    PublicationReadScript, PublicationReadStep, PublicationService, WriteCondition, WriteScript,
    WriteStep,
};
use typst_pack::PackArchiveBytes;
use typst_pack::opendal::pack_archive::{PackArchiveAcquisitionRequest, acquire_pack_archive};
use typst_pack::opendal::publication::{
    OpenDalPublicationPhase, PackArchivePublicationErrorCause, PackArchivePublicationRequest,
    PackArchivePublicationRequestError, PublicationKeyOutcome, PublicationPolicy,
    publish_pack_archive,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::pack_archive::{AcquisitionLimits, CommitCertainty, DecodeLimits, decode};

#[test]
fn request_accepts_exact_objects_and_rejects_prefixes() {
    let destination: Location = "archive:/packs/document.typk".parse().unwrap();
    let request =
        PackArchivePublicationRequest::new(destination.clone(), PublicationPolicy::CreateOrVerify)
            .unwrap();

    assert_eq!(request.destination(), &destination);
    assert_eq!(request.policy(), PublicationPolicy::CreateOrVerify);

    for (destination, expected) in [
        ("archive:/", LocationRoleError::ObjectAtRoot),
        ("archive:/packs/", LocationRoleError::ObjectHasTrailingSlash),
    ] {
        let destination: Location = destination.parse().unwrap();
        let error = PackArchivePublicationRequest::new(
            destination.clone(),
            PublicationPolicy::OverwriteExactKeys,
        )
        .unwrap_err();

        assert_eq!(
            error,
            PackArchivePublicationRequestError::InvalidDestinationRole {
                location: destination,
                source: expected,
            }
        );
    }
}

#[test]
fn overwrite_writes_exact_borrowed_bytes_once_without_reading_or_touching_other_objects() {
    let archive = PackArchiveBytes::from_vec(b"exact archive bytes".to_vec());
    let archive_address = archive.as_slice().as_ptr();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("unrelated.typk".to_owned(), b"untouched".to_vec())],
        [],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::Direct,
            [],
        )],
        16,
    );
    let bindings = bindings(&service);
    let request = request(PublicationPolicy::OverwriteExactKeys);

    let receipt = expect_ready(pin!(publish_pack_archive(&bindings, &request, &archive))).unwrap();

    assert_eq!(archive.as_slice().as_ptr(), archive_address);
    assert_eq!(receipt.destination(), request.destination());
    assert_eq!(receipt.policy(), PublicationPolicy::OverwriteExactKeys);
    assert_eq!(receipt.outcome(), PublicationKeyOutcome::Written);
    assert_eq!(
        service.destination().object("packs/document.typk"),
        Some(archive.as_slice())
    );
    assert_eq!(
        service.destination().object("unrelated.typk"),
        Some(b"untouched".as_slice())
    );
    assert_eq!(
        service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
            .count(),
        1
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
fn create_or_verify_compares_empty_and_exact_streams_as_read_only_successes() {
    for (bytes, chunks) in [
        (b"".as_slice(), Vec::new()),
        (b"abcd".as_slice(), vec![0..2, 2..4]),
    ] {
        let archive = PackArchiveBytes::from_vec(bytes.to_vec());
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("packs/document.typk".to_owned(), bytes.to_vec())],
            [PublicationReadScript::new(
                "packs/document.typk",
                chunks.len(),
                chunks.into_iter().map(PublicationReadStep::chunk),
            )
            .unwrap()],
            [],
            16,
        );
        let bindings = bindings(&service);

        let receipt = expect_ready(pin!(publish_pack_archive(
            &bindings,
            &request(PublicationPolicy::CreateOrVerify),
            &archive,
        )))
        .unwrap();

        assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
        );
    }
}

#[test]
fn create_or_verify_rejects_short_and_long_streams_before_mutation() {
    for (observed, expected_at_least) in [(b"abc".as_slice(), 3), (b"abcde".as_slice(), 5)] {
        let archive = PackArchiveBytes::from_vec(b"abcd".to_vec());
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("packs/document.typk".to_owned(), observed.to_vec())],
            [PublicationReadScript::new(
                "packs/document.typk",
                1,
                [PublicationReadStep::chunk(0..observed.len())],
            )
            .unwrap()],
            [],
            16,
        );
        let bindings = bindings(&service);

        let error = expect_ready(pin!(publish_pack_archive(
            &bindings,
            &request(PublicationPolicy::CreateOrVerify),
            &archive,
        )))
        .unwrap_err();

        assert_eq!(error.phase(), OpenDalPublicationPhase::PreflightRead);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(error.progress().completed().is_none());
        assert!(matches!(
            error.cause(),
            PackArchivePublicationErrorCause::ByteConflict {
                expected_byte_length: 4,
                observed_byte_length_at_least,
            } if *observed_byte_length_at_least == expected_at_least
        ));
        assert_eq!(
            service.destination().object("packs/document.typk"),
            Some(observed)
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
        );
    }
}

#[test]
fn create_or_verify_reports_a_mutable_matching_stream_without_claiming_an_effect() {
    let archive = PackArchiveBytes::from_vec(b"main".to_vec());
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("packs/document.typk".to_owned(), b"maXX".to_vec())],
        [PublicationReadScript::new(
            "packs/document.typk",
            2,
            [
                PublicationReadStep::chunk(0..2),
                PublicationReadStep::mutate(DestinationMutation::set(
                    "packs/document.typk",
                    b"YYin",
                )),
                PublicationReadStep::chunk(2..4),
            ],
        )
        .unwrap()],
        [],
        16,
    );
    let bindings = bindings(&service);

    let receipt = expect_ready(pin!(publish_pack_archive(
        &bindings,
        &request(PublicationPolicy::CreateOrVerify),
        &archive,
    )))
    .unwrap();

    assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
    assert_eq!(
        service.destination().object("packs/document.typk"),
        Some(b"YYin".as_slice())
    );
}

#[test]
fn create_or_verify_creates_absent_bytes_and_performs_one_bounded_race_verification() {
    let archive = PackArchiveBytes::from_vec(b"archive".to_vec());
    let created_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            "packs/document.typk",
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::IfNotExists,
            [],
        )],
        16,
    );
    let created_bindings = bindings(&created_service);
    let created = expect_ready(pin!(publish_pack_archive(
        &created_bindings,
        &request(PublicationPolicy::CreateOrVerify),
        &archive,
    )))
    .unwrap();
    assert_eq!(created.outcome(), PublicationKeyOutcome::Created);

    let pending = PendingPoint::new();
    let race_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [
            PublicationReadScript::new(
                "packs/document.typk",
                0,
                [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            PublicationReadScript::new(
                "packs/document.typk",
                1,
                [PublicationReadStep::chunk(0..archive.as_slice().len())],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        24,
    );
    let race_bindings = bindings(&race_service);
    let race_request = request(PublicationPolicy::CreateOrVerify);
    let receipt = {
        let mut publication = pin!(publish_pack_archive(
            &race_bindings,
            &race_request,
            &archive
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        race_service.mutate(DestinationMutation::set(
            "packs/document.typk",
            archive.as_slice(),
        ));
        pending.release();
        expect_ready(publication.as_mut()).unwrap()
    };

    assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
    assert_eq!(
        race_service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, PublicationOperationLogEntry::ReadInvoked { .. }))
            .count(),
        2
    );
}

#[test]
fn capability_and_size_failures_are_typed_and_have_no_effect() {
    let archive = PackArchiveBytes::from_vec(Vec::new());
    for (policy, capabilities) in [
        (
            PublicationPolicy::OverwriteExactKeys,
            PublicationCapabilities {
                write: false,
                ..PublicationCapabilities::all()
            },
        ),
        (
            PublicationPolicy::OverwriteExactKeys,
            PublicationCapabilities {
                write_can_empty: false,
                ..PublicationCapabilities::all()
            },
        ),
        (
            PublicationPolicy::CreateOrVerify,
            PublicationCapabilities {
                read: false,
                ..PublicationCapabilities::all()
            },
        ),
        (
            PublicationPolicy::CreateOrVerify,
            PublicationCapabilities {
                write_with_if_not_exists: false,
                ..PublicationCapabilities::all()
            },
        ),
    ] {
        let service = PublicationService::new(capabilities, [], [], [], 8);
        let bindings = bindings(&service);
        let error = expect_ready(pin!(publish_pack_archive(
            &bindings,
            &request(policy),
            &archive,
        )))
        .unwrap_err();

        assert_eq!(error.phase(), OpenDalPublicationPhase::CapabilityAppraisal);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause(),
            PackArchivePublicationErrorCause::UnsupportedPolicy { policy: actual }
                if *actual == policy
        ));
        assert!(service.log().entries().is_empty());
    }

    let archive = PackArchiveBytes::from_vec(b"large".to_vec());
    let service = PublicationService::new(
        PublicationCapabilities {
            write_total_max_size: Some(4),
            ..PublicationCapabilities::all()
        },
        [],
        [],
        [],
        8,
    );
    let bindings = bindings(&service);
    let error = expect_ready(pin!(publish_pack_archive(
        &bindings,
        &request(PublicationPolicy::OverwriteExactKeys),
        &archive,
    )))
    .unwrap_err();
    assert!(matches!(
        error.cause(),
        PackArchivePublicationErrorCause::UnsupportedObjectSize { byte_length: 5 }
    ));
    assert!(service.log().entries().is_empty());
}

#[test]
fn resolver_read_create_race_and_direct_write_failures_retain_typed_causes() {
    let archive = PackArchiveBytes::from_vec(b"archive".to_vec());
    let create_request = request(PublicationPolicy::CreateOrVerify);

    let resolve = expect_ready(pin!(publish_pack_archive(
        &FailingResolver,
        &create_request,
        &archive,
    )))
    .unwrap_err();
    assert_eq!(resolve.phase(), OpenDalPublicationPhase::ResolveOperator);
    assert_eq!(resolve.commit_certainty(), CommitCertainty::NotCommitted);
    let PackArchivePublicationErrorCause::ResolveOperator(source) = resolve.cause() else {
        panic!("unexpected cause: {:?}", resolve.cause());
    };
    assert!(source.downcast_ref::<ResolveError>().is_some());
    let cause = resolve.source().unwrap().source().unwrap();
    assert!(cause.is::<PackArchivePublicationErrorCause>());
    assert!(cause.source().unwrap().is::<ResolveError>());

    let read_service = PublicationService::new(
        PublicationCapabilities::all(),
        [("packs/document.typk".to_owned(), b"archive".to_vec())],
        [PublicationReadScript::new(
            "packs/document.typk",
            0,
            [PublicationReadStep::failure(
                opendal::ErrorKind::PermissionDenied,
            )],
        )
        .unwrap()],
        [],
        8,
    );
    let read_bindings = bindings(&read_service);
    let read = expect_ready(pin!(publish_pack_archive(
        &read_bindings,
        &create_request,
        &archive,
    )))
    .unwrap_err();
    assert_eq!(read.phase(), OpenDalPublicationPhase::PreflightRead);
    assert!(matches!(
        read.cause(),
        PackArchivePublicationErrorCause::PreflightRead(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let create_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            "packs/document.typk",
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::write_failure(
            "packs/document.typk",
            WriteCondition::IfNotExists,
            opendal::ErrorKind::PermissionDenied,
        )],
        8,
    );
    let create_bindings = bindings(&create_service);
    let create = expect_ready(pin!(publish_pack_archive(
        &create_bindings,
        &create_request,
        &archive,
    )))
    .unwrap_err();
    assert_eq!(create.phase(), OpenDalPublicationPhase::ConditionalCreate);
    assert_eq!(create.commit_certainty(), CommitCertainty::Indeterminate);
    assert!(matches!(
        create.cause(),
        PackArchivePublicationErrorCause::ConditionalCreate(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let pending = PendingPoint::new();
    let race_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [
            PublicationReadScript::new(
                "packs/document.typk",
                0,
                [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            PublicationReadScript::new(
                "packs/document.typk",
                0,
                [PublicationReadStep::failure(
                    opendal::ErrorKind::PermissionDenied,
                )],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        16,
    );
    let race_bindings = bindings(&race_service);
    let race = {
        let mut publication = pin!(publish_pack_archive(
            &race_bindings,
            &create_request,
            &archive,
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        race_service.mutate(DestinationMutation::set("packs/document.typk", b"racing"));
        pending.release();
        expect_ready(publication.as_mut()).unwrap_err()
    };
    assert_eq!(race.phase(), OpenDalPublicationPhase::RaceVerification);
    assert_eq!(race.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        race.cause(),
        PackArchivePublicationErrorCause::RaceVerification(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let direct_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::write_failure(
            "packs/document.typk",
            WriteCondition::Direct,
            opendal::ErrorKind::PermissionDenied,
        )],
        8,
    );
    let direct_bindings = bindings(&direct_service);
    let direct = expect_ready(pin!(publish_pack_archive(
        &direct_bindings,
        &request(PublicationPolicy::OverwriteExactKeys),
        &archive,
    )))
    .unwrap_err();
    assert_eq!(direct.phase(), OpenDalPublicationPhase::DirectWrite);
    assert_eq!(direct.commit_certainty(), CommitCertainty::Indeterminate);
    assert!(matches!(
        direct.cause(),
        PackArchivePublicationErrorCause::DirectWrite(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
    assert_eq!(direct.failed_path(), Some("packs/document.typk"));
    assert_eq!(archive.as_slice(), b"archive");
}

#[test]
fn dropping_publication_preserves_caller_bytes_for_full_replay() {
    let archive = PackArchiveBytes::from_vec(b"retry archive".to_vec());
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::Direct,
            [WriteStep::pending(pending.clone())],
        )],
        16,
    );
    let configured_bindings = bindings(&service);
    let overwrite = request(PublicationPolicy::OverwriteExactKeys);
    {
        let mut publication = pin!(publish_pack_archive(
            &configured_bindings,
            &overwrite,
            &archive,
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(archive.as_slice(), b"retry archive");
    assert_eq!(service.cancellations().len(), 1);

    let replay_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "packs/document.typk",
            WriteCondition::Direct,
            [],
        )],
        8,
    );
    let replay_bindings = bindings(&replay_service);
    expect_ready(pin!(publish_pack_archive(
        &replay_bindings,
        &overwrite,
        &archive,
    )))
    .unwrap();
    assert_eq!(
        replay_service.destination().object("packs/document.typk"),
        Some(archive.as_slice())
    );
}

#[test]
fn memory_publish_replay_and_acquire_preserve_exact_bytes_before_decode() {
    let archive = PackArchiveBytes::from_vec(b"not a valid Pack Archive".to_vec());
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let bindings =
        OperatorBindings::new([(OperatorBinding::new("memory").unwrap(), operator)]).unwrap();
    let destination: Location = "memory:/document.typk".parse().unwrap();

    let overwrite = PackArchivePublicationRequest::new(
        destination.clone(),
        PublicationPolicy::OverwriteExactKeys,
    )
    .unwrap();
    expect_ready(pin!(publish_pack_archive(&bindings, &overwrite, &archive))).unwrap();

    let replay =
        PackArchivePublicationRequest::new(destination.clone(), PublicationPolicy::CreateOrVerify)
            .unwrap();
    for _ in 0..2 {
        let receipt =
            expect_ready(pin!(publish_pack_archive(&bindings, &replay, &archive))).unwrap();
        assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
    }

    let acquisition =
        PackArchiveAcquisitionRequest::new(destination, AcquisitionLimits::reference_v1()).unwrap();
    let acquired = expect_ready(pin!(acquire_pack_archive(&bindings, &acquisition))).unwrap();
    assert_eq!(acquired.as_slice(), archive.as_slice());
    assert!(decode(&acquired, DecodeLimits::reference_v1()).is_err());
    assert_eq!(acquired.as_slice(), archive.as_slice());

    let changed_bytes = b"destination changed after publication and is longer";
    expect_ready(pin!(
        bindings
            .resolve(&OperatorBinding::new("memory").unwrap())
            .unwrap()
            .write("document.typk", changed_bytes.to_vec())
    ))
    .unwrap();
    let changed = expect_ready(pin!(acquire_pack_archive(&bindings, &acquisition))).unwrap();
    assert_ne!(changed.as_slice(), archive.as_slice());
    assert_eq!(changed.as_slice(), changed_bytes);
}

#[test]
fn outer_diagnostics_are_safe_and_operator_bindings_make_a_send_future() {
    let archive = PackArchiveBytes::from_vec(b"sensitive archive bytes".to_vec());
    let request = request(PublicationPolicy::OverwriteExactKeys);
    let error = expect_ready(pin!(publish_pack_archive(
        &FailingResolver,
        &request,
        &archive,
    )))
    .unwrap_err();

    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(rendered.contains("archive"));
        assert!(rendered.contains("packs/document.typk"));
        assert!(!rendered.contains("sensitive"));
        assert!(!rendered.contains("resolver rejected"));
    }

    let service = PublicationService::new(PublicationCapabilities::all(), [], [], [], 1);
    let bindings = bindings(&service);
    assert_send(publish_pack_archive(&bindings, &request, &archive));
}

fn bindings(service: &PublicationService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("archive").unwrap(), service.operator())]).unwrap()
}

fn request(policy: PublicationPolicy) -> PackArchivePublicationRequest {
    PackArchivePublicationRequest::new("archive:/packs/document.typk".parse().unwrap(), policy)
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

struct FailingResolver;

impl OperatorResolver for FailingResolver {
    type Error = ResolveError;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        Err(ResolveError)
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
