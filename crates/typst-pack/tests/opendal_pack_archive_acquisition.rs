#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::error::Error as _;
use std::fmt;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use scripted_opendal::{
    Capabilities, DroppedOperation, OperationLogEntry, PendingPoint, ReadScript, ReadStep,
    ScriptedService,
};
use typst_pack::opendal::pack_archive::{
    PackArchiveAcquisitionErrorCause, PackArchiveAcquisitionRequest,
    PackArchiveAcquisitionRequestError, acquire_pack_archive,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::pack_archive::{AcquisitionLimitError, AcquisitionLimits, AcquisitionResource};

#[test]
fn acquires_exact_pack_archive_bytes_without_decoding() {
    let script = ReadScript::new(
        "packs/document.typk",
        2,
        [ReadStep::chunk(b"not "), ReadStep::chunk(b"a pack")],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
    let binding = OperatorBinding::new("archive").unwrap();
    let bindings = OperatorBindings::new([(binding, service.operator())]).unwrap();
    let source = "archive:/packs/document.typk".parse().unwrap();
    let limits = AcquisitionLimits::new(10).unwrap();
    let request = PackArchiveAcquisitionRequest::new(source, limits).unwrap();

    assert_eq!(request.source().to_string(), "archive:/packs/document.typk");
    assert_eq!(request.limits(), limits);

    let mut acquisition = pin!(acquire_pack_archive(&bindings, &request));
    let archive = expect_ready(acquisition.as_mut()).unwrap();

    assert_eq!(archive.as_slice(), b"not a pack");
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "packs/document.typk".to_owned(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"not ".to_vec(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"a pack".to_vec(),
            },
            OperationLogEntry::ReadCompleted { id: 0 },
        ]
    );
}

#[test]
fn memory_acquires_empty_short_exact_and_chunked_archives() {
    for (path, bytes, ceiling) in [
        ("empty.typk", b"".as_slice(), 0),
        ("short.typk", b"abc".as_slice(), 4),
        ("exact.typk", b"abcd".as_slice(), 4),
        ("chunked.typk", &vec![b'x'; 64 * 1024], 64 * 1024),
    ] {
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        {
            let mut write = pin!(operator.write(path, bytes.to_vec()));
            expect_ready(write.as_mut()).unwrap();
        }
        let bindings =
            OperatorBindings::new([(OperatorBinding::new("memory").unwrap(), operator)]).unwrap();
        let source =
            Location::from_operation_path(OperatorBinding::new("memory").unwrap(), path).unwrap();
        let request =
            PackArchiveAcquisitionRequest::new(source, AcquisitionLimits::new(ceiling).unwrap())
                .unwrap();
        let mut acquisition = pin!(acquire_pack_archive(&bindings, &request));

        assert_eq!(
            expect_ready(acquisition.as_mut()).unwrap().as_slice(),
            bytes
        );
    }
}

#[test]
fn memory_reports_one_byte_over_the_archive_ceiling() {
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    {
        let mut write = pin!(operator.write("large.typk", b"abcde".to_vec()));
        expect_ready(write.as_mut()).unwrap();
    }
    let bindings =
        OperatorBindings::new([(OperatorBinding::new("memory").unwrap(), operator)]).unwrap();
    let source: Location = "memory:/large.typk".parse().unwrap();
    let request =
        PackArchiveAcquisitionRequest::new(source, AcquisitionLimits::new(4).unwrap()).unwrap();
    let mut acquisition = pin!(acquire_pack_archive(&bindings, &request));

    let error = expect_ready(acquisition.as_mut()).unwrap_err();

    assert!(matches!(
        error.cause(),
        PackArchiveAcquisitionErrorCause::Limit(AcquisitionLimitError::Exceeded {
            resource: AcquisitionResource::ArchiveBytes,
            ceiling: 4,
            observed_at_least: 5,
        })
    ));
}

#[test]
fn request_rejects_non_object_locations_before_resolution() {
    for (source, expected) in [
        ("archive:/", LocationRoleError::ObjectAtRoot),
        ("archive:/packs/", LocationRoleError::ObjectHasTrailingSlash),
    ] {
        let source: Location = source.parse().unwrap();
        let error =
            PackArchiveAcquisitionRequest::new(source.clone(), AcquisitionLimits::new(8).unwrap())
                .unwrap_err();

        assert!(matches!(
            error,
            PackArchiveAcquisitionRequestError::InvalidSourceRole {
                location,
                source: role_error,
            } if location == source && role_error == expected
        ));
    }
}

#[test]
fn preserves_empty_short_and_exact_ceiling_archives() {
    for (path, chunks, ceiling, expected) in [
        ("empty.typk", Vec::new(), 0, b"".as_slice()),
        (
            "short.typk",
            vec![b"ab".as_slice(), b"c"],
            4,
            b"abc".as_slice(),
        ),
        (
            "exact.typk",
            vec![b"ab".as_slice(), b"cd"],
            4,
            b"abcd".as_slice(),
        ),
    ] {
        let script =
            ReadScript::new(path, chunks.len(), chunks.into_iter().map(ReadStep::chunk)).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
        let bindings = bindings(&service);
        let request = request(path, ceiling);
        let mut acquisition = pin!(acquire_pack_archive(&bindings, &request));

        assert_eq!(
            expect_ready(acquisition.as_mut()).unwrap().as_slice(),
            expected
        );
        assert!(matches!(
            service.log().entries().last(),
            Some(OperationLogEntry::ReadCompleted { id: 0 })
        ));
    }
}

#[test]
fn reports_archive_limit_with_only_one_probe_byte_retained() {
    let script = ReadScript::new(
        "large.typk",
        1,
        [ReadStep::chunk(b"sensitive archive bytes")],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
    let bindings = bindings(&service);
    let request = request("large.typk", 4);
    let mut acquisition = pin!(acquire_pack_archive(&bindings, &request));

    let error = expect_ready(acquisition.as_mut()).unwrap_err();

    assert!(matches!(
        error.cause(),
        PackArchiveAcquisitionErrorCause::Limit(AcquisitionLimitError::Exceeded {
            resource: AcquisitionResource::ArchiveBytes,
            ceiling: 4,
            observed_at_least: 5,
        })
    ));
    assert_eq!(
        service.cancellations(),
        [DroppedOperation::Read {
            id: 0,
            path: "large.typk".to_owned(),
        }]
    );
    assert_safe_outer_diagnostics(&error, "archive", "large.typk");
}

#[test]
fn keeps_absence_unsupported_resolution_and_read_failures_distinct() {
    let absent_service = ScriptedService::new(Capabilities::all(), [], [], 8);
    let absent_bindings = bindings(&absent_service);
    let absent_request = request("absent.typk", 8);
    let mut absent = pin!(acquire_pack_archive(&absent_bindings, &absent_request));
    let absent = expect_ready(absent.as_mut()).unwrap_err();
    assert!(matches!(
        absent.cause(),
        PackArchiveAcquisitionErrorCause::ObjectAbsent(source)
            if source.kind() == opendal::ErrorKind::NotFound
    ));
    assert_eq!(
        absent
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<opendal::Error>()
            .unwrap()
            .kind(),
        opendal::ErrorKind::NotFound
    );

    let unsupported_service = ScriptedService::new(
        Capabilities {
            list: true,
            list_with_recursive: true,
            read: false,
        },
        [],
        [],
        8,
    );
    let unsupported_bindings = bindings(&unsupported_service);
    let unsupported_request = request("unsupported.typk", 8);
    let mut unsupported = pin!(acquire_pack_archive(
        &unsupported_bindings,
        &unsupported_request
    ));
    let unsupported = expect_ready(unsupported.as_mut()).unwrap_err();
    assert!(matches!(
        unsupported.cause(),
        PackArchiveAcquisitionErrorCause::ReadUnsupported
    ));
    assert!(
        unsupported
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .is_none()
    );
    assert!(unsupported_service.log().entries().is_empty());

    let read_script = ReadScript::new(
        "broken.typk",
        1,
        [
            ReadStep::chunk(b"sensitive payload"),
            ReadStep::failure(opendal::ErrorKind::PermissionDenied),
        ],
    )
    .unwrap();
    let read_service = ScriptedService::new(Capabilities::all(), [], [read_script], 8);
    let read_bindings = bindings(&read_service);
    let read_request = request("broken.typk", 32);
    let mut read = pin!(acquire_pack_archive(&read_bindings, &read_request));
    let read = expect_ready(read.as_mut()).unwrap_err();
    assert!(matches!(
        read.cause(),
        PackArchiveAcquisitionErrorCause::Read(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
    assert_safe_outer_diagnostics(&read, "archive", "broken.typk");

    let partial_not_found = ReadScript::new(
        "partial.typk",
        1,
        [
            ReadStep::chunk(b"partial"),
            ReadStep::failure(opendal::ErrorKind::NotFound),
        ],
    )
    .unwrap();
    let partial_service = ScriptedService::new(Capabilities::all(), [], [partial_not_found], 8);
    let partial_bindings = bindings(&partial_service);
    let partial_request = request("partial.typk", 32);
    let partial = expect_ready(pin!(acquire_pack_archive(
        &partial_bindings,
        &partial_request
    )))
    .unwrap_err();
    assert!(matches!(
        partial.cause(),
        PackArchiveAcquisitionErrorCause::Read(source)
            if source.kind() == opendal::ErrorKind::NotFound
    ));

    let resolver = RejectingResolver;
    let resolve_request = request("secret.typk", 8);
    let mut resolve = pin!(acquire_pack_archive(&resolver, &resolve_request));
    let resolve = expect_ready(resolve.as_mut()).unwrap_err();
    assert!(matches!(
        resolve.cause(),
        PackArchiveAcquisitionErrorCause::ResolveOperator(source)
            if source.downcast_ref::<ResolverFailure>().is_some()
    ));
    assert!(
        resolve
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<ResolverFailure>()
    );
    assert_safe_outer_diagnostics(&resolve, "archive", "secret.typk");
}

#[test]
fn dropping_pending_acquisition_returns_no_terminal_value() {
    let pending = PendingPoint::new();
    let script = ReadScript::new(
        "pending.typk",
        1,
        [
            ReadStep::chunk(b"partial"),
            ReadStep::pending(pending.clone()),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
    let configured_bindings = bindings(&service);
    let request = request("pending.typk", 16);
    {
        let mut acquisition = pin!(acquire_pack_archive(&configured_bindings, &request));
        assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(
        service.cancellations(),
        [DroppedOperation::Read {
            id: 0,
            path: "pending.typk".to_owned(),
        }]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "pending.typk".to_owned(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"partial".to_vec(),
            },
            OperationLogEntry::ReadDropped {
                id: 0,
                path: "pending.typk".to_owned(),
            },
        ]
    );

    let replay = ReadScript::new(
        "pending.typk",
        2,
        [ReadStep::chunk(b"partial"), ReadStep::chunk(b" complete")],
    )
    .unwrap();
    let replay_service = ScriptedService::new(Capabilities::all(), [], [replay], 8);
    let replay_bindings = bindings(&replay_service);
    let mut replay = pin!(acquire_pack_archive(&replay_bindings, &request));

    assert_eq!(
        expect_ready(replay.as_mut()).unwrap().as_slice(),
        b"partial complete"
    );
    assert!(matches!(
        replay_service.log().entries().first(),
        Some(OperationLogEntry::ReadInvoked { id: 0, path }) if path == "pending.typk"
    ));
}

#[test]
fn operator_bindings_produce_a_send_acquisition_future() {
    let service = ScriptedService::new(Capabilities::all(), [], [], 1);
    let bindings = bindings(&service);
    let request = request("archive.typk", 8);

    assert_send(acquire_pack_archive(&bindings, &request));
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

fn request(path: &str, ceiling: u64) -> PackArchiveAcquisitionRequest {
    let source =
        Location::from_operation_path(OperatorBinding::new("archive").unwrap(), path).unwrap();
    PackArchiveAcquisitionRequest::new(source, AcquisitionLimits::new(ceiling).unwrap()).unwrap()
}

fn bindings(service: &ScriptedService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("archive").unwrap(), service.operator())]).unwrap()
}

fn assert_safe_outer_diagnostics(
    error: &typst_pack::opendal::pack_archive::PackArchiveAcquisitionError,
    binding: &str,
    operation_path: &str,
) {
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(rendered.contains(binding));
        assert!(rendered.contains(operation_path));
        assert!(!rendered.contains("sensitive"));
        assert!(!rendered.contains("scripted"));
        assert!(!rendered.contains("resolver endpoint secret"));
    }
}

fn assert_send<T: Send>(_: T) {}

struct RejectingResolver;

impl OperatorResolver for RejectingResolver {
    type Error = ResolverFailure;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        Err(ResolverFailure)
    }
}

#[derive(Debug)]
struct ResolverFailure;

impl fmt::Display for ResolverFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resolver endpoint secret")
    }
}

impl std::error::Error for ResolverFailure {}
