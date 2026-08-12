#![cfg(feature = "opendal")]

#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::future::{Future, IntoFuture};
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use futures_util::StreamExt;
use opendal::ErrorKind;
use scripted_opendal::{
    Capabilities, DroppedOperation, ListEntry, ListEntryKind, ListScript, ListStep,
    OperationLogEntry, PendingPoint, ReadScript, ReadStep, ScriptError, ScriptedService,
};

#[test]
fn scripts_reject_records_beyond_their_declared_bounds() {
    assert_eq!(
        ListScript::new(
            "project/",
            1,
            [ListStep::page([
                ListEntry::file("project/main.typ"),
                ListEntry::file("project/asset.png"),
            ])],
        )
        .unwrap_err(),
        ScriptError::TooManyListEntries {
            declared: 1,
            scripted: 2,
        }
    );
    assert_eq!(
        ReadScript::new(
            "project/main.typ",
            1,
            [ReadStep::chunk(b"one"), ReadStep::chunk(b"two")],
        )
        .unwrap_err(),
        ScriptError::TooManyReadChunks {
            declared: 1,
            scripted: 2,
        }
    );
}

#[test]
fn list_preserves_scripted_pages_order_kinds_and_terminal_failure() {
    let script = ListScript::new(
        "project/",
        4,
        [
            ListStep::page([
                ListEntry::file("project/z.typ"),
                ListEntry::directory("project/assets/"),
            ]),
            ListStep::page([
                ListEntry::unknown("outside-prefix"),
                ListEntry::file("project/z.typ"),
            ]),
            ListStep::failure(ErrorKind::PermissionDenied),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(
        Capabilities {
            list: true,
            list_with_recursive: true,
            read: false,
        },
        [script],
        [],
        16,
    );
    let operator = service.operator();

    let mut open = pin!(
        operator
            .lister_with("project/")
            .recursive(true)
            .into_future()
    );
    let mut lister = expect_ready(open.as_mut()).unwrap();
    let mut observed = Vec::new();
    loop {
        let mut next = pin!(lister.next());
        match expect_ready(next.as_mut()) {
            Some(Ok(entry)) => observed.push((entry.path().to_owned(), entry.metadata().mode())),
            Some(Err(error)) => {
                assert_eq!(error.kind(), ErrorKind::PermissionDenied);
                break;
            }
            None => panic!("the scripted failure must terminate the list"),
        }
    }

    assert_eq!(
        observed,
        [
            ("project/z.typ".to_owned(), opendal::EntryMode::FILE),
            ("project/assets/".to_owned(), opendal::EntryMode::DIR),
            ("outside-prefix".to_owned(), opendal::EntryMode::Unknown),
            ("project/z.typ".to_owned(), opendal::EntryMode::FILE),
        ]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked {
                id: 0,
                path: "project/".to_owned(),
                recursive: true,
            },
            OperationLogEntry::ListPageYielded {
                id: 0,
                entries: vec![
                    ListEntry::file("project/z.typ"),
                    ListEntry::directory("project/assets/"),
                ],
            },
            OperationLogEntry::ListPageYielded {
                id: 0,
                entries: vec![
                    ListEntry::unknown("outside-prefix"),
                    ListEntry::file("project/z.typ"),
                ],
            },
            OperationLogEntry::ListFailed {
                id: 0,
                kind: ErrorKind::PermissionDenied,
            },
        ]
    );
    assert!(service.cancellations().is_empty());
}

#[test]
fn read_yields_exact_chunks_then_a_typed_failure() {
    let script = ReadScript::new(
        "object.bin",
        3,
        [
            ReadStep::chunk(b"short"),
            ReadStep::chunk(b"exact"),
            ReadStep::chunk(b"plus-one"),
            ReadStep::failure(ErrorKind::Unexpected),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(
        Capabilities {
            list: false,
            list_with_recursive: false,
            read: true,
        },
        [],
        [script],
        16,
    );
    let operator = service.operator();

    let mut reader = pin!(operator.reader("object.bin"));
    let reader = expect_ready(reader.as_mut()).unwrap();
    let mut open = pin!(reader.into_stream(..));
    let mut stream = expect_ready(open.as_mut()).unwrap();

    let mut observed = Vec::new();
    loop {
        let mut next = pin!(stream.next());
        match expect_ready(next.as_mut()) {
            Some(Ok(chunk)) => observed.push(chunk.to_vec()),
            Some(Err(error)) => {
                assert_eq!(error.kind(), ErrorKind::Unexpected);
                break;
            }
            None => panic!("the scripted failure must terminate the read"),
        }
    }

    assert_eq!(
        observed,
        [b"short".to_vec(), b"exact".to_vec(), b"plus-one".to_vec()]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "object.bin".to_owned(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"short".to_vec(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"exact".to_vec(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 0,
                bytes: b"plus-one".to_vec(),
            },
            OperationLogEntry::ReadFailed {
                id: 0,
                kind: ErrorKind::Unexpected,
            },
        ]
    );
}

#[test]
fn empty_read_stream_completes_without_yielding_a_chunk() {
    let script = ReadScript::new("empty.bin", 0, []).unwrap();
    let service = ScriptedService::new(Capabilities::all(), [], [script], 4);
    let operator = service.operator();
    let mut reader = pin!(operator.reader("empty.bin"));
    let reader = expect_ready(reader.as_mut()).unwrap();
    let mut open = pin!(reader.into_stream(..));
    let mut stream = expect_ready(open.as_mut()).unwrap();
    let mut next = pin!(stream.next());

    assert!(expect_ready(next.as_mut()).is_none());
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "empty.bin".to_owned(),
            },
            OperationLogEntry::ReadCompleted { id: 0 },
        ]
    );
}

#[test]
fn list_pending_points_release_without_sleeping() {
    let pending = PendingPoint::new();
    let script = ListScript::new(
        "release/",
        1,
        [
            ListStep::pending(pending.clone()),
            ListStep::page([ListEntry::file("release/ready.typ")]),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [script], [], 8);
    let operator = service.operator();
    let mut open = pin!(
        operator
            .lister_with("release/")
            .recursive(true)
            .into_future()
    );
    let mut lister = expect_ready(open.as_mut()).unwrap();
    let mut next = pin!(lister.next());

    assert!(matches!(poll_once(next.as_mut()), Poll::Pending));
    assert!(pending.was_observed());
    pending.release();
    let entry = expect_ready(next.as_mut()).unwrap().unwrap();
    assert_eq!(entry.path(), "release/ready.typ");
    drop(next);

    let mut completed = pin!(lister.next());
    assert!(expect_ready(completed.as_mut()).is_none());
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked {
                id: 0,
                path: "release/".to_owned(),
                recursive: true,
            },
            OperationLogEntry::ListPageYielded {
                id: 0,
                entries: vec![ListEntry::file("release/ready.typ")],
            },
            OperationLogEntry::ListCompleted { id: 0 },
        ]
    );
}

#[test]
fn dropping_a_partial_list_records_only_executed_entries_and_cancellation() {
    let script = ListScript::new(
        "partial/",
        2,
        [ListStep::page([
            ListEntry::file("partial/first.typ"),
            ListEntry::file("partial/second.typ"),
        ])],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [script], [], 8);
    let operator = service.operator();
    let mut open = pin!(
        operator
            .lister_with("partial/")
            .recursive(true)
            .into_future()
    );
    let mut lister = expect_ready(open.as_mut()).unwrap();
    let mut first = pin!(lister.next());

    assert_eq!(
        expect_ready(first.as_mut()).unwrap().unwrap().path(),
        "partial/first.typ"
    );
    drop(first);
    drop(lister);

    assert_eq!(
        service.cancellations(),
        [DroppedOperation::List {
            id: 0,
            path: "partial/".to_owned(),
        }]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked {
                id: 0,
                path: "partial/".to_owned(),
                recursive: true,
            },
            OperationLogEntry::ListDropped {
                id: 0,
                path: "partial/".to_owned(),
            },
        ]
    );
}

#[test]
fn a_listed_object_can_disappear_before_read() {
    let script = ListScript::new(
        "race/",
        1,
        [ListStep::page([ListEntry::file("race/gone.typ")])],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [script], [], 8);
    let operator = service.operator();
    let mut open = pin!(operator.lister_with("race/").recursive(true).into_future());
    let mut lister = expect_ready(open.as_mut()).unwrap();
    let mut entry = pin!(lister.next());
    assert_eq!(
        expect_ready(entry.as_mut()).unwrap().unwrap().path(),
        "race/gone.typ"
    );
    drop(entry);
    let mut completed = pin!(lister.next());
    assert!(expect_ready(completed.as_mut()).is_none());

    let mut read = pin!(operator.reader("race/gone.typ"));
    let error = match expect_ready(read.as_mut()) {
        Ok(_) => panic!("the listed object must have disappeared"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked {
                id: 0,
                path: "race/".to_owned(),
                recursive: true,
            },
            OperationLogEntry::ListPageYielded {
                id: 0,
                entries: vec![ListEntry::file("race/gone.typ")],
            },
            OperationLogEntry::ListCompleted { id: 0 },
            OperationLogEntry::ReadInvoked {
                id: 1,
                path: "race/gone.typ".to_owned(),
            },
            OperationLogEntry::ReadFailed {
                id: 1,
                kind: ErrorKind::NotFound,
            },
        ]
    );
}

#[test]
fn dropping_a_pending_read_records_exact_cancellation_state() {
    let pending = PendingPoint::new();
    let script = ReadScript::new(
        "later.bin",
        1,
        [
            ReadStep::pending(pending.clone()),
            ReadStep::chunk(b"never-yielded"),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [], [script], 8);
    let operator = service.operator();
    let mut reader = pin!(operator.reader("later.bin"));
    let reader = expect_ready(reader.as_mut()).unwrap();
    let mut open = pin!(reader.into_stream(..));
    let mut stream = expect_ready(open.as_mut()).unwrap();
    let mut next = pin!(stream.next());

    assert!(matches!(poll_once(next.as_mut()), Poll::Pending));
    assert!(pending.was_observed());
    drop(next);
    drop(stream);

    assert_eq!(
        service.cancellations(),
        [DroppedOperation::Read {
            id: 0,
            path: "later.bin".to_owned(),
        }]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "later.bin".to_owned(),
            },
            OperationLogEntry::ReadDropped {
                id: 0,
                path: "later.bin".to_owned(),
            },
        ]
    );
}

#[test]
fn capabilities_are_projected_without_enabling_unimplemented_operations() {
    let service = ScriptedService::new(
        Capabilities {
            list: true,
            list_with_recursive: false,
            read: true,
        },
        [],
        [],
        1,
    );
    let capability = service.operator().info().capability();

    assert!(capability.list);
    assert!(!capability.list_with_recursive);
    assert!(capability.read);
    assert!(!capability.write);
    assert!(!capability.create_dir);
    assert!(!capability.delete);
    assert!(!capability.copy);
    assert!(!capability.rename);
}

#[test]
fn operation_log_never_retains_more_than_its_capacity() {
    let service = ScriptedService::new(Capabilities::all(), [], [], 1);
    let operator = service.operator();
    let mut reader = pin!(operator.reader("absent.bin"));
    let error = match expect_ready(reader.as_mut()) {
        Ok(_) => panic!("the absent script must fail"),
        Err(error) => error,
    };

    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert_eq!(
        service.log().entries(),
        [OperationLogEntry::ReadInvoked {
            id: 0,
            path: "absent.bin".to_owned(),
        }]
    );
    assert_eq!(service.log().omitted_entries(), 1);
}

fn expect_ready<F: Future>(future: std::pin::Pin<&mut F>) -> F::Output {
    match poll_once(future) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("future unexpectedly pending"),
    }
}

fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(NoopWake));
    future.poll(&mut Context::from_waker(&waker))
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

#[test]
fn entry_kind_helpers_cover_the_raw_entry_vocabulary() {
    let file = ListEntry::file("a");
    assert_eq!(file.path(), "a");
    assert_eq!(file.kind(), ListEntryKind::File);
    assert_eq!(ListEntry::directory("a/").kind(), ListEntryKind::Directory);
    assert_eq!(ListEntry::unknown("a").kind(), ListEntryKind::Unknown);
}
