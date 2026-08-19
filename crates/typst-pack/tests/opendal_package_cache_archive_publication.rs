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
use typst_pack::opendal::publication::{
    OpenDalPublicationPhase, PackageCacheArchivePublicationErrorCause,
    PackageCacheArchivePublicationRequest, PublicationKeyOutcome, PublicationPolicy,
    publish_package_cache_archive,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::pack_archive::CommitCertainty;

#[test]
fn request_accepts_only_an_exact_cache_object_and_fixes_create_or_verify() {
    let destination: Location = "cache:/archives/preview/example/1.2.3.tar.gz"
        .parse()
        .unwrap();
    let request = PackageCacheArchivePublicationRequest::new(destination.clone()).unwrap();

    assert_eq!(request.destination(), &destination);
    assert_eq!(request.policy(), PublicationPolicy::CreateOrVerify);

    for (destination, expected) in [
        ("cache:/", LocationRoleError::ObjectAtRoot),
        (
            "cache:/archives/preview/",
            LocationRoleError::ObjectHasTrailingSlash,
        ),
    ] {
        let destination: Location = destination.parse().unwrap();
        let error = PackageCacheArchivePublicationRequest::new(destination.clone()).unwrap_err();

        assert_eq!(
            error,
            typst_pack::opendal::publication::PackageCacheArchivePublicationRequestError::InvalidDestinationRole {
                location: destination,
                source: expected,
            }
        );
    }
}

#[test]
fn absent_cache_object_is_created_from_exact_borrowed_bytes() {
    let archive = b"exact registry archive";
    let archive_address = archive.as_ptr();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("unrelated".to_owned(), b"untouched".to_vec())],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [],
        )],
        16,
    );
    let bindings = bindings(&service);

    let receipt = expect_ready(pin!(publish_package_cache_archive(
        &bindings,
        &request(),
        archive,
    )))
    .unwrap();

    assert_eq!(archive.as_ptr(), archive_address);
    assert_eq!(receipt.destination(), request().destination());
    assert_eq!(receipt.policy(), PublicationPolicy::CreateOrVerify);
    assert_eq!(receipt.outcome(), PublicationKeyOutcome::Created);
    assert_eq!(receipt.completed().destination_path(), cache_path());
    assert_eq!(
        service.destination().object(cache_path()),
        Some(archive.as_slice())
    );
    assert_eq!(
        service.destination().object("unrelated"),
        Some(b"untouched".as_slice())
    );
}

#[test]
fn replay_incrementally_distinguishes_empty_exact_shorter_divergent_and_longer_streams() {
    for (expected, observed, chunks, conflict_at_least) in [
        (b"".as_slice(), b"".as_slice(), vec![], None),
        (
            b"abcd".as_slice(),
            b"abcd".as_slice(),
            vec![0..2, 2..4],
            None,
        ),
        (b"abcd".as_slice(), b"abc".as_slice(), vec![0..3], Some(3)),
        (b"abcd".as_slice(), b"abXd".as_slice(), vec![0..4], Some(3)),
        (b"abcd".as_slice(), b"abcde".as_slice(), vec![0..5], Some(5)),
    ] {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [(cache_path().to_owned(), observed.to_vec())],
            [PublicationReadScript::new(
                cache_path(),
                chunks.len(),
                chunks.into_iter().map(PublicationReadStep::chunk),
            )
            .unwrap()],
            [],
            16,
        );
        let bindings = bindings(&service);
        let result = expect_ready(pin!(publish_package_cache_archive(
            &bindings,
            &request(),
            expected,
        )));

        if let Some(observed_byte_length_at_least) = conflict_at_least {
            let error = result.unwrap_err();
            assert_eq!(error.phase(), OpenDalPublicationPhase::PreflightRead);
            assert_eq!(error.failed_path(), Some(cache_path()));
            assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
            assert!(error.progress().completed().is_none());
            assert!(matches!(
                error.cause(),
                PackageCacheArchivePublicationErrorCause::ByteConflict {
                    expected_byte_length,
                    observed_byte_length_at_least: actual,
                } if *expected_byte_length == expected.len() as u64
                    && *actual == observed_byte_length_at_least
            ));
        } else {
            let receipt = result.unwrap();
            assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
        }
        assert_eq!(service.destination().object(cache_path()), Some(observed));
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
fn mutable_matching_stream_reports_only_a_read_observation() {
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [(cache_path().to_owned(), b"maXX".to_vec())],
        [PublicationReadScript::new(
            cache_path(),
            2,
            [
                PublicationReadStep::chunk(0..2),
                PublicationReadStep::mutate(DestinationMutation::set(cache_path(), b"YYin")),
                PublicationReadStep::chunk(2..4),
            ],
        )
        .unwrap()],
        [],
        16,
    );
    let bindings = bindings(&service);

    let receipt = expect_ready(pin!(publish_package_cache_archive(
        &bindings,
        &request(),
        b"main",
    )))
    .unwrap();

    assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
    assert_eq!(
        service.destination().object(cache_path()),
        Some(b"YYin".as_slice())
    );
}

#[test]
fn unsupported_fixed_policy_capabilities_and_size_are_operation_specific() {
    for capabilities in [
        PublicationCapabilities {
            write: false,
            ..PublicationCapabilities::all()
        },
        PublicationCapabilities {
            write_can_empty: false,
            ..PublicationCapabilities::all()
        },
        PublicationCapabilities {
            read: false,
            ..PublicationCapabilities::all()
        },
        PublicationCapabilities {
            write_with_if_not_exists: false,
            ..PublicationCapabilities::all()
        },
    ] {
        let service = PublicationService::new(capabilities, [], [], [], 4);
        let bindings = bindings(&service);
        let error = expect_ready(pin!(publish_package_cache_archive(
            &bindings,
            &request(),
            b"",
        )))
        .unwrap_err();

        assert_eq!(error.policy(), PublicationPolicy::CreateOrVerify);
        assert_eq!(error.phase(), OpenDalPublicationPhase::CapabilityAppraisal);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause(),
            PackageCacheArchivePublicationErrorCause::UnsupportedPolicy {
                policy: PublicationPolicy::CreateOrVerify,
            }
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
    let bindings = bindings(&service);
    let error = expect_ready(pin!(publish_package_cache_archive(
        &bindings,
        &request(),
        b"four",
    )))
    .unwrap_err();
    assert_eq!(error.failed_path(), Some(cache_path()));
    assert!(matches!(
        error.cause(),
        PackageCacheArchivePublicationErrorCause::UnsupportedObjectSize { byte_length: 4 }
    ));
    assert!(service.log().entries().is_empty());
}

#[test]
fn conditional_race_is_verified_once_without_treating_the_read_as_a_commit() {
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [
            PublicationReadScript::new(
                cache_path(),
                0,
                [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            PublicationReadScript::new(cache_path(), 1, [PublicationReadStep::chunk(0..7)])
                .unwrap(),
        ],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        24,
    );
    let bindings = bindings(&service);
    let request = request();
    let receipt = {
        let mut publication = pin!(publish_package_cache_archive(
            &bindings, &request, b"archive",
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        service.mutate(DestinationMutation::set(cache_path(), b"archive"));
        pending.release();
        expect_ready(publication.as_mut()).unwrap()
    };

    assert_eq!(receipt.outcome(), PublicationKeyOutcome::AlreadyMatching);
    assert_eq!(
        service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, PublicationOperationLogEntry::ReadInvoked { .. }))
            .count(),
        2
    );
}

#[test]
fn divergent_race_verification_conflicts_without_a_second_create() {
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [
            PublicationReadScript::new(
                cache_path(),
                0,
                [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            PublicationReadScript::new(cache_path(), 1, [PublicationReadStep::chunk(0..7)])
                .unwrap(),
        ],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        24,
    );
    let bindings = bindings(&service);
    let request = request();
    let error = {
        let mut publication = pin!(publish_package_cache_archive(
            &bindings, &request, b"archive",
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        service.mutate(DestinationMutation::set(cache_path(), b"diverge"));
        pending.release();
        expect_ready(publication.as_mut()).unwrap_err()
    };

    assert_eq!(error.phase(), OpenDalPublicationPhase::RaceVerification);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        error.cause(),
        PackageCacheArchivePublicationErrorCause::ByteConflict {
            expected_byte_length: 7,
            observed_byte_length_at_least: 1,
        }
    ));
    assert_eq!(
        service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, PublicationOperationLogEntry::WriteInvoked { .. }))
            .count(),
        1
    );
}

#[test]
fn resolver_read_create_and_race_failures_cover_every_error_certainty() {
    let resolve = expect_ready(pin!(publish_package_cache_archive(
        &FailingResolver,
        &request(),
        b"archive",
    )))
    .unwrap_err();
    assert_eq!(resolve.phase(), OpenDalPublicationPhase::ResolveOperator);
    assert_eq!(resolve.commit_certainty(), CommitCertainty::NotCommitted);
    let PackageCacheArchivePublicationErrorCause::ResolveOperator(source) = resolve.cause() else {
        panic!("unexpected cause: {:?}", resolve.cause());
    };
    assert!(source.downcast_ref::<ResolveError>().is_some());
    let cause = resolve.source().unwrap().source().unwrap();
    assert!(cause.is::<PackageCacheArchivePublicationErrorCause>());
    assert!(cause.source().unwrap().is::<ResolveError>());

    let read_service = PublicationService::new(
        PublicationCapabilities::all(),
        [(cache_path().to_owned(), b"archive".to_vec())],
        [PublicationReadScript::new(
            cache_path(),
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
    let read = expect_ready(pin!(publish_package_cache_archive(
        &read_bindings,
        &request(),
        b"archive",
    )))
    .unwrap_err();
    assert_eq!(read.phase(), OpenDalPublicationPhase::PreflightRead);
    assert_eq!(read.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        read.cause(),
        PackageCacheArchivePublicationErrorCause::PreflightRead(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let create_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::write_failure(
            cache_path(),
            WriteCondition::IfNotExists,
            opendal::ErrorKind::PermissionDenied,
        )],
        8,
    );
    let create_bindings = bindings(&create_service);
    let create = expect_ready(pin!(publish_package_cache_archive(
        &create_bindings,
        &request(),
        b"archive",
    )))
    .unwrap_err();
    assert_eq!(create.phase(), OpenDalPublicationPhase::ConditionalCreate);
    assert_eq!(create.commit_certainty(), CommitCertainty::Indeterminate);
    assert!(matches!(
        create.cause(),
        PackageCacheArchivePublicationErrorCause::ConditionalCreate(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let pending = PendingPoint::new();
    let race_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [
            PublicationReadScript::new(
                cache_path(),
                0,
                [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
            )
            .unwrap(),
            PublicationReadScript::new(
                cache_path(),
                0,
                [PublicationReadStep::failure(
                    opendal::ErrorKind::PermissionDenied,
                )],
            )
            .unwrap(),
        ],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        16,
    );
    let race_bindings = bindings(&race_service);
    let request = request();
    let race = {
        let mut publication = pin!(publish_package_cache_archive(
            &race_bindings,
            &request,
            b"archive",
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        race_service.mutate(DestinationMutation::set(cache_path(), b"racing"));
        pending.release();
        expect_ready(publication.as_mut()).unwrap_err()
    };
    assert_eq!(race.phase(), OpenDalPublicationPhase::RaceVerification);
    assert_eq!(race.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(matches!(
        race.cause(),
        PackageCacheArchivePublicationErrorCause::RaceVerification(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
}

#[test]
fn cancellation_preserves_exact_caller_bytes_for_full_replay() {
    let archive = b"retry archive".to_vec();
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone())],
        )],
        16,
    );
    let configured_bindings = bindings(&service);
    let request = request();
    {
        let mut publication = pin!(publish_package_cache_archive(
            &configured_bindings,
            &request,
            &archive,
        ));
        assert!(matches!(poll_once(publication.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(archive, b"retry archive");
    assert_eq!(service.cancellations().len(), 1);

    let replay_service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [],
        )],
        8,
    );
    let replay_bindings = bindings(&replay_service);
    expect_ready(pin!(publish_package_cache_archive(
        &replay_bindings,
        &request,
        &archive,
    )))
    .unwrap();
    assert_eq!(
        replay_service.destination().object(cache_path()),
        Some(archive.as_slice())
    );
}

#[test]
fn outer_diagnostics_are_safe_and_operator_bindings_make_a_send_future() {
    let archive = b"sensitive archive bytes";
    let request = request();
    let error = expect_ready(pin!(publish_package_cache_archive(
        &FailingResolver,
        &request,
        archive,
    )))
    .unwrap_err();

    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(rendered.contains("cache"));
        assert!(rendered.contains(cache_path()));
        assert!(!rendered.contains("sensitive"));
        assert!(!rendered.contains("resolver rejected"));
    }

    let service = PublicationService::new(PublicationCapabilities::all(), [], [], [], 1);
    let bindings = bindings(&service);
    assert_send(publish_package_cache_archive(&bindings, &request, archive));
}

#[test]
fn memory_creation_and_replay_preserve_exact_bytes() {
    let archive = b"exact package archive";
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let bindings =
        OperatorBindings::new([(OperatorBinding::new("cache").unwrap(), operator.clone())])
            .unwrap();
    let request = request();

    let created = expect_ready(pin!(publish_package_cache_archive(
        &bindings, &request, archive,
    )))
    .unwrap();
    assert_eq!(created.outcome(), PublicationKeyOutcome::Created);

    let replay = expect_ready(pin!(publish_package_cache_archive(
        &bindings, &request, archive,
    )))
    .unwrap();
    assert_eq!(replay.outcome(), PublicationKeyOutcome::AlreadyMatching);
    assert_eq!(
        expect_ready(pin!(operator.read(cache_path())))
            .unwrap()
            .to_vec(),
        archive
    );
}

#[cfg(feature = "package-acquisition")]
#[test]
fn validated_registry_residue_survives_independent_cache_failure_and_replay() {
    use std::io::Write as _;

    use typst_pack::opendal::pack_assembly::{
        PackageAcquisitionLimits, PackageAcquisitionRequest, acquire_package,
        insert_acquired_package,
    };
    use typst_pack::{
        PackageAcquisitionFailures, PackageCatalog, PackageDisposition, PackageExpansionLimits,
    };

    let archive = package_archive();
    let registry = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    expect_ready(pin!(
        registry.write("registry/preview/example-1.2.3.tar.gz", archive.clone(),)
    ))
    .unwrap();
    let failed_cache = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::write_failure(
            cache_path(),
            WriteCondition::IfNotExists,
            opendal::ErrorKind::PermissionDenied,
        )],
        8,
    );
    let configured_bindings = OperatorBindings::new([
        (
            OperatorBinding::new("cache").unwrap(),
            failed_cache.operator(),
        ),
        (OperatorBinding::new("registry").unwrap(), registry.clone()),
    ])
    .unwrap();
    let acquisition_request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("cache:/archives/".parse().unwrap()),
        Some("registry:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    let acquisition = expect_ready(pin!(acquire_package(
        &configured_bindings,
        &acquisition_request,
    )))
    .unwrap();
    let mut catalog = PackageCatalog::new();
    let mut failures = PackageAcquisitionFailures::new();
    let residue = insert_acquired_package(
        &mut catalog,
        &mut failures,
        acquisition,
        PackageDisposition::Embedded,
        PackageExpansionLimits::reference_v1(),
    )
    .unwrap()
    .unwrap();
    let publication_request =
        PackageCacheArchivePublicationRequest::new(residue.destination().clone()).unwrap();

    let cache_error = expect_ready(pin!(publish_package_cache_archive(
        &configured_bindings,
        &publication_request,
        residue.bytes(),
    )))
    .unwrap_err();

    assert_eq!(
        cache_error.commit_certainty(),
        CommitCertainty::Indeterminate
    );
    assert_eq!(residue.bytes(), archive);
    assert_eq!(
        catalog.get(residue.spec()).unwrap().tree().file("lib.typ"),
        Some(b"package library".as_slice())
    );
    assert!(failures.get(residue.spec()).is_none());

    let replay_cache = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [PublicationReadScript::new(
            cache_path(),
            0,
            [PublicationReadStep::failure(opendal::ErrorKind::NotFound)],
        )
        .unwrap()],
        [WriteScript::new(
            cache_path(),
            WriteCondition::IfNotExists,
            [],
        )],
        8,
    );
    let replay_bindings = bindings(&replay_cache);
    let receipt = expect_ready(pin!(publish_package_cache_archive(
        &replay_bindings,
        &publication_request,
        residue.bytes(),
    )))
    .unwrap();
    assert_eq!(receipt.outcome(), PublicationKeyOutcome::Created);
    assert_eq!(
        replay_cache.destination().object(cache_path()),
        Some(archive.as_slice())
    );

    fn package_archive() -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for (path, bytes) in [
            (
                "typst.toml",
                b"[package]\nname = \"example\"\nversion = \"1.2.3\"\n".as_slice(),
            ),
            ("lib.typ", b"package library".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append(&header, bytes).unwrap();
        }
        let mut encoder = archive.into_inner().unwrap();
        encoder.flush().unwrap();
        encoder.finish().unwrap()
    }
}

fn bindings(service: &PublicationService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("cache").unwrap(), service.operator())]).unwrap()
}

fn request() -> PackageCacheArchivePublicationRequest {
    PackageCacheArchivePublicationRequest::new(
        "cache:/archives/preview/example/1.2.3.tar.gz"
            .parse()
            .unwrap(),
    )
    .unwrap()
}

fn cache_path() -> &'static str {
    "archives/preview/example/1.2.3.tar.gz"
}

fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
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
