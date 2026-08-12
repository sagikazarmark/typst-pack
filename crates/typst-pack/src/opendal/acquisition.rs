use futures_util::StreamExt;
use opendal::ErrorKind;

use super::location::{Location, LocationRoleError, OperatorResolver};

/// A failure while acquiring one exact object's bytes through OpenDAL.
#[derive(Debug)]
pub(crate) enum ExactObjectAcquisitionError<E> {
    InvalidLocationRole(LocationRoleError),
    ResolveOperator(E),
    ReadUnsupported,
    ObjectAbsent(opendal::Error),
    Read(opendal::Error),
    Limit(ExactObjectLimitError),
}

/// A bounded exact-object read exceeded or could not account for its ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExactObjectLimitError {
    Exceeded {
        ceiling: u64,
        observed_at_least: u64,
    },
    AccountingOverflow,
}

/// Acquires one exact object's bytes while retaining at most `ceiling + 1` bytes.
///
/// The bound covers payload and probe bytes retained by typst-pack. It excludes
/// allocations owned by OpenDAL services and transports, including each buffer
/// yielded to this function.
pub(crate) async fn acquire_exact_object<R: OperatorResolver + ?Sized>(
    resolver: &R,
    location: &Location,
    ceiling: u64,
) -> Result<Vec<u8>, ExactObjectAcquisitionError<R::Error>> {
    location
        .require_object()
        .map_err(ExactObjectAcquisitionError::InvalidLocationRole)?;
    let operator = resolver
        .resolve(location.binding())
        .map_err(ExactObjectAcquisitionError::ResolveOperator)?;
    if !operator.info().capability().read {
        return Err(ExactObjectAcquisitionError::ReadUnsupported);
    }

    let probe_end = ceiling
        .checked_add(1)
        .ok_or(ExactObjectLimitError::AccountingOverflow)
        .map_err(ExactObjectAcquisitionError::Limit)?;
    let reader = operator
        .reader(location.dispatch_path())
        .await
        .map_err(classify_read_error)?;
    let mut stream = reader.into_stream(..).await.map_err(classify_read_error)?;
    let mut bytes = Vec::new();
    let mut observed = 0u64;

    while let Some(buffer) = stream.next().await {
        let buffer = buffer.map_err(classify_read_error)?;
        let buffer_len = u64::try_from(buffer.len()).map_err(|_| {
            ExactObjectAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow)
        })?;
        let retained = probe_end
            .checked_sub(observed)
            .ok_or(ExactObjectAcquisitionError::Limit(
                ExactObjectLimitError::AccountingOverflow,
            ))?
            .min(buffer_len);
        let retained = usize::try_from(retained).map_err(|_| {
            ExactObjectAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow)
        })?;

        let mut remaining = retained;
        for chunk in buffer {
            let take = remaining.min(chunk.len());
            bytes.extend_from_slice(&chunk[..take]);
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
        observed = observed
            .checked_add(u64::try_from(retained).map_err(|_| {
                ExactObjectAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow)
            })?)
            .ok_or(ExactObjectAcquisitionError::Limit(
                ExactObjectLimitError::AccountingOverflow,
            ))?;
        if observed > ceiling {
            return Err(ExactObjectAcquisitionError::Limit(
                ExactObjectLimitError::Exceeded {
                    ceiling,
                    observed_at_least: probe_end,
                },
            ));
        }
    }

    Ok(bytes)
}

fn classify_read_error<E>(error: opendal::Error) -> ExactObjectAcquisitionError<E> {
    if error.kind() == ErrorKind::NotFound {
        ExactObjectAcquisitionError::ObjectAbsent(error)
    } else {
        ExactObjectAcquisitionError::Read(error)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, OperationLogEntry, PendingPoint, ReadScript, ReadStep,
        ScriptedService,
    };
    use crate::opendal::{Location, LocationRoleError, OperatorBinding, OperatorResolver};

    use super::{ExactObjectAcquisitionError, ExactObjectLimitError, acquire_exact_object};

    #[test]
    fn acquires_one_bounded_exact_object_end_to_end() {
        let script = ReadScript::new(
            "archives/document.typk",
            2,
            [ReadStep::chunk(b"exact "), ReadStep::chunk(b"archive")],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
        let binding = OperatorBinding::new("archive").unwrap();
        let resolver = CountingResolver::new(service.operator());
        let location = Location::from_operation_path(binding, "archives/document.typk").unwrap();

        let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 13));
        let bytes = expect_ready(acquisition.as_mut()).unwrap();

        assert_eq!(bytes, b"exact archive");
        assert_eq!(resolver.calls(), 1);
        assert_eq!(
            service.log().entries(),
            [
                OperationLogEntry::ReadInvoked {
                    id: 0,
                    path: "archives/document.typk".to_owned(),
                },
                OperationLogEntry::ReadChunkYielded {
                    id: 0,
                    bytes: b"exact ".to_vec(),
                },
                OperationLogEntry::ReadChunkYielded {
                    id: 0,
                    bytes: b"archive".to_vec(),
                },
                OperationLogEntry::ReadCompleted { id: 0 },
            ]
        );
    }

    #[test]
    fn rejects_root_and_prefix_roles_before_resolving() {
        let service = ScriptedService::new(Capabilities::all(), [], [], 1);
        let resolver = CountingResolver::new(service.operator());

        for (path, expected) in [
            ("", LocationRoleError::ObjectAtRoot),
            ("archives/", LocationRoleError::ObjectHasTrailingSlash),
        ] {
            let location = Location::from_operation_path(binding(), path).unwrap();
            let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 8));
            let error = expect_ready(acquisition.as_mut()).unwrap_err();

            assert!(matches!(
                error,
                ExactObjectAcquisitionError::InvalidLocationRole(source) if source == expected
            ));
        }

        assert_eq!(resolver.calls(), 0);
        assert!(service.log().entries().is_empty());
    }

    #[test]
    fn rejects_missing_read_capability_before_storage_io() {
        let service = ScriptedService::new(
            Capabilities {
                list: true,
                list_with_recursive: true,
                read: false,
            },
            [],
            [],
            1,
        );
        let resolver = CountingResolver::new(service.operator());
        let location = object_location("object.bin");

        let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 8));
        let error = expect_ready(acquisition.as_mut()).unwrap_err();

        assert!(matches!(
            error,
            ExactObjectAcquisitionError::ReadUnsupported
        ));
        assert_eq!(resolver.calls(), 1);
        assert!(service.log().entries().is_empty());
    }

    #[test]
    fn preserves_empty_short_and_exact_ceiling_objects() {
        for (name, chunks, ceiling, expected) in [
            ("empty.bin", Vec::new(), 0, b"".as_slice()),
            ("short.bin", vec![b"ab".as_slice(), b"c"], 4, b"abc"),
            ("exact.bin", vec![b"ab".as_slice(), b"cd"], 4, b"abcd"),
        ] {
            let script =
                ReadScript::new(name, chunks.len(), chunks.into_iter().map(ReadStep::chunk))
                    .unwrap();
            let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
            let resolver = CountingResolver::new(service.operator());
            let location = object_location(name);

            let mut acquisition = pin!(acquire_exact_object(&resolver, &location, ceiling));
            assert_eq!(expect_ready(acquisition.as_mut()).unwrap(), expected);
            assert!(matches!(
                service.log().entries().last(),
                Some(OperationLogEntry::ReadCompleted { id: 0 })
            ));
        }
    }

    #[test]
    fn exact_plus_one_and_larger_buffers_retain_only_one_probe_byte() {
        for yielded in [b"01234".as_slice(), b"0123456789"] {
            let script = ReadScript::new("large.bin", 1, [ReadStep::chunk(yielded)]).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
            let resolver = CountingResolver::new(service.operator());
            let location = object_location("large.bin");

            let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 4));
            let error = expect_ready(acquisition.as_mut()).unwrap_err();

            assert!(matches!(
                error,
                ExactObjectAcquisitionError::Limit(ExactObjectLimitError::Exceeded {
                    ceiling: 4,
                    observed_at_least: 5,
                })
            ));
            assert_eq!(
                service.cancellations(),
                [DroppedOperation::Read {
                    id: 0,
                    path: "large.bin".to_owned(),
                }]
            );
        }
    }

    #[test]
    fn rejects_a_ceiling_without_probe_room_before_storage_io() {
        let service = ScriptedService::new(Capabilities::all(), [], [], 1);
        let resolver = CountingResolver::new(service.operator());
        let location = object_location("object.bin");

        let mut acquisition = pin!(acquire_exact_object(&resolver, &location, u64::MAX));
        let error = expect_ready(acquisition.as_mut()).unwrap_err();

        assert!(matches!(
            error,
            ExactObjectAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow)
        ));
        assert_eq!(resolver.calls(), 1);
        assert!(service.log().entries().is_empty());
    }

    #[test]
    fn classifies_disappearance_separately_from_backend_read_failure() {
        let absent_service = ScriptedService::new(Capabilities::all(), [], [], 4);
        let absent_resolver = CountingResolver::new(absent_service.operator());
        let location = object_location("gone.bin");
        let mut absent = pin!(acquire_exact_object(&absent_resolver, &location, 4));

        let error = expect_ready(absent.as_mut()).unwrap_err();
        assert!(matches!(
            error,
            ExactObjectAcquisitionError::ObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let script = ReadScript::new(
            "broken.bin",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::failure(ErrorKind::PermissionDenied),
            ],
        )
        .unwrap();
        let broken_service = ScriptedService::new(Capabilities::all(), [], [script], 8);
        let broken_resolver = CountingResolver::new(broken_service.operator());
        let location = object_location("broken.bin");
        let mut broken = pin!(acquire_exact_object(&broken_resolver, &location, 16));

        let error = expect_ready(broken.as_mut()).unwrap_err();
        assert!(matches!(
            error,
            ExactObjectAcquisitionError::Read(source)
                if source.kind() == ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn dropping_a_pending_acquisition_discards_partial_bytes_and_reader() {
        for chunks_before_pending in [0, 1] {
            let pending = PendingPoint::new();
            let mut steps = Vec::new();
            if chunks_before_pending == 1 {
                steps.push(ReadStep::chunk(b"partial"));
            }
            steps.push(ReadStep::pending(pending.clone()));
            let script = ReadScript::new("pending.bin", chunks_before_pending, steps).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
            let resolver = CountingResolver::new(service.operator());
            let location = object_location("pending.bin");
            {
                let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 16));
                assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
                assert!(pending.was_observed());
            }

            assert_eq!(
                service.cancellations(),
                [DroppedOperation::Read {
                    id: 0,
                    path: "pending.bin".to_owned(),
                }]
            );
        }
    }

    #[test]
    fn preserves_typed_resolver_failures_without_storage_io() {
        let location = object_location("object.bin");
        let resolver = RejectingResolver {
            calls: Cell::new(0),
        };

        let mut acquisition = pin!(acquire_exact_object(&resolver, &location, 8));
        let error = expect_ready(acquisition.as_mut()).unwrap_err();

        assert!(matches!(
            error,
            ExactObjectAcquisitionError::ResolveOperator(ResolveFailure)
        ));
        assert_eq!(resolver.calls.get(), 1);
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
        OperatorBinding::new("archive").unwrap()
    }

    fn object_location(path: &str) -> Location {
        Location::from_operation_path(binding(), path).unwrap()
    }

    struct CountingResolver {
        calls: Cell<usize>,
        operator: opendal::Operator,
    }

    impl CountingResolver {
        fn new(operator: opendal::Operator) -> Self {
            Self {
                calls: Cell::new(0),
                operator,
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl OperatorResolver for CountingResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.operator.clone())
        }
    }

    struct RejectingResolver {
        calls: Cell<usize>,
    }

    impl OperatorResolver for RejectingResolver {
        type Error = ResolveFailure;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            Err(ResolveFailure)
        }
    }

    #[derive(Debug)]
    struct ResolveFailure;

    impl std::fmt::Display for ResolveFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("resolver failed")
        }
    }

    impl std::error::Error for ResolveFailure {}
}
