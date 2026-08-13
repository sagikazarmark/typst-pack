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
    Capabilities, DestinationMutation, DroppedOperation, ListEntry, ListEntryKind, ListScript,
    ListStep, OperationControls, OperationLogEntry, PendingPoint, PublicationCapabilities,
    PublicationDroppedOperation, PublicationOperationLogEntry, PublicationReadScript,
    PublicationReadStep, PublicationService, ReadScript, ReadStep, ScriptError, ScriptedService,
    WriteCondition, WriteEffect, WriteScript, WriteStage, WriteStep,
};

#[test]
fn indexed_read_controls_choose_completion_and_cancellation_order() {
    let controls = OperationControls::new();
    let first = controls.hold_read(0);
    let second = controls.hold_read(1);
    let service = ScriptedService::new_controlled(
        Capabilities::all(),
        [],
        [
            ReadScript::new("first.bin", 1, [ReadStep::chunk(b"first")]).unwrap(),
            ReadScript::new("second.bin", 1, [ReadStep::chunk(b"second")]).unwrap(),
        ],
        controls,
        16,
    );
    let operator = service.operator();
    let mut first_read = Box::pin(operator.read("first.bin"));
    let mut second_read = Box::pin(operator.read("second.bin"));

    assert!(matches!(poll_once(first_read.as_mut()), Poll::Pending));
    assert!(matches!(poll_once(second_read.as_mut()), Poll::Pending));
    assert!(first.was_observed());
    assert!(second.was_observed());

    second.release();
    assert_eq!(
        expect_ready(second_read.as_mut()).unwrap().to_vec(),
        b"second"
    );
    drop(first_read);

    assert_eq!(
        service.cancellations(),
        [DroppedOperation::Read {
            id: 0,
            path: "first.bin".to_owned(),
        }]
    );
    assert_eq!(
        service.log().entries(),
        [
            OperationLogEntry::ReadInvoked {
                id: 0,
                path: "first.bin".to_owned(),
            },
            OperationLogEntry::ReadInvoked {
                id: 1,
                path: "second.bin".to_owned(),
            },
            OperationLogEntry::ReadChunkYielded {
                id: 1,
                bytes: b"second".to_vec(),
            },
            OperationLogEntry::ReadCompleted { id: 1 },
            OperationLogEntry::ReadDropped {
                id: 0,
                path: "first.bin".to_owned(),
            },
        ]
    );
}

#[test]
fn indexed_list_controls_release_operations_independently() {
    let controls = OperationControls::new();
    let first = controls.hold_list(0);
    let second = controls.hold_list(1);
    let service = ScriptedService::new_controlled(
        Capabilities::all(),
        [
            ListScript::new("first/", 0, []).unwrap(),
            ListScript::new("second/", 0, []).unwrap(),
        ],
        [],
        controls,
        8,
    );
    let operator = service.operator();
    let mut first_open = pin!(operator.lister_with("first/").recursive(true).into_future());
    let mut second_open = pin!(
        operator
            .lister_with("second/")
            .recursive(true)
            .into_future()
    );
    let mut first_lister = expect_ready(first_open.as_mut()).unwrap();
    let mut second_lister = expect_ready(second_open.as_mut()).unwrap();
    let mut first_next = pin!(first_lister.next());
    let mut second_next = pin!(second_lister.next());

    assert!(matches!(poll_once(first_next.as_mut()), Poll::Pending));
    assert!(matches!(poll_once(second_next.as_mut()), Poll::Pending));
    assert!(first.was_observed());
    assert!(second.was_observed());

    first.release();
    assert!(expect_ready(first_next.as_mut()).is_none());
    assert!(matches!(poll_once(second_next.as_mut()), Poll::Pending));
    second.release();
    assert!(expect_ready(second_next.as_mut()).is_none());

    assert!(matches!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked { path: first, .. },
            OperationLogEntry::ListInvoked { path: second, .. },
            OperationLogEntry::ListCompleted { id: 0 },
            OperationLogEntry::ListCompleted { id: 1 },
        ] if first == "first/" && second == "second/"
    ));
}

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
    assert_eq!(
        PublicationReadScript::new(
            "object.bin",
            1,
            [
                PublicationReadStep::chunk(0..1),
                PublicationReadStep::chunk(1..2),
            ],
        )
        .unwrap_err(),
        ScriptError::PublicationReadChunksExceeded {
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
fn a_listed_object_can_be_replaced_before_read() {
    let replacement =
        ReadScript::new("race/changing.typ", 1, [ReadStep::chunk(b"replacement")]).unwrap();
    let list = ListScript::new(
        "race/",
        1,
        [
            ListStep::page([ListEntry::file("race/changing.typ")]),
            ListStep::replace_read(replacement),
        ],
    )
    .unwrap();
    let original = ReadScript::new("race/changing.typ", 1, [ReadStep::chunk(b"original")]).unwrap();
    let service = ScriptedService::new(Capabilities::all(), [list], [original], 8);
    let operator = service.operator();
    let mut open = pin!(operator.lister_with("race/").recursive(true).into_future());
    let mut lister = expect_ready(open.as_mut()).unwrap();
    let mut entry = pin!(lister.next());
    assert_eq!(
        expect_ready(entry.as_mut()).unwrap().unwrap().path(),
        "race/changing.typ"
    );
    drop(entry);
    let mut completed = pin!(lister.next());
    assert!(expect_ready(completed.as_mut()).is_none());
    drop(completed);

    let mut read = pin!(operator.read("race/changing.typ"));
    assert_eq!(
        expect_ready(read.as_mut()).unwrap().to_vec(),
        b"replacement"
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

#[test]
fn publication_capabilities_are_advertised_independently() {
    let service = PublicationService::new(
        PublicationCapabilities {
            write: true,
            write_can_empty: false,
            write_with_if_not_exists: true,
            read: false,
            write_total_max_size: Some(17),
        },
        [],
        [],
        [],
        1,
    );
    let capability = service.operator().info().capability();

    assert!(capability.write);
    assert!(!capability.write_can_empty);
    assert!(capability.write_with_if_not_exists);
    assert!(!capability.read);
    assert_eq!(capability.write_total_max_size, Some(17));
    assert!(!capability.write_can_multi);
}

#[test]
fn publication_service_applies_direct_conditional_and_empty_writes() {
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("existing.bin".to_owned(), b"old".to_vec())],
        [],
        [
            WriteScript::new("existing.bin", WriteCondition::Direct, []),
            WriteScript::new("created.bin", WriteCondition::IfNotExists, []),
            WriteScript::new("empty.bin", WriteCondition::Direct, []),
        ],
        32,
    );
    let operator = service.operator();

    let mut direct = pin!(operator.write("existing.bin", b"new".to_vec()));
    expect_ready(direct.as_mut()).unwrap();
    let mut conditional = pin!(
        operator
            .write_with("created.bin", b"created".to_vec())
            .if_not_exists(true)
            .into_future()
    );
    expect_ready(conditional.as_mut()).unwrap();
    let mut empty = pin!(operator.write("empty.bin", Vec::<u8>::new()));
    expect_ready(empty.as_mut()).unwrap();

    let destination = service.destination();
    assert_eq!(destination.object("existing.bin"), Some(b"new".as_slice()));
    assert_eq!(
        destination.object("created.bin"),
        Some(b"created".as_slice())
    );
    assert_eq!(destination.object("empty.bin"), Some(b"".as_slice()));
    assert!(service.log().entries().iter().any(|entry| matches!(
        entry,
        PublicationOperationLogEntry::WriteCompleted {
            path,
            length: 7,
            condition: WriteCondition::IfNotExists,
            effect: WriteEffect::Committed,
            ..
        } if path == "created.bin"
    )));
}

#[test]
fn publication_write_failures_preserve_typed_kinds_and_known_no_effect() {
    for kind in [ErrorKind::AlreadyExists, ErrorKind::ConditionNotMatch] {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [("object.bin".to_owned(), b"original".to_vec())],
            [],
            [WriteScript::setup_failure(
                "object.bin",
                WriteCondition::IfNotExists,
                kind,
            )],
            8,
        );
        let operator = service.operator();
        let mut write = pin!(
            operator
                .write_with("object.bin", b"replacement".to_vec())
                .if_not_exists(true)
                .into_future()
        );
        let error = expect_ready(write.as_mut()).unwrap_err();

        assert_eq!(error.kind(), kind);
        assert_eq!(
            service.destination().object("object.bin"),
            Some(b"original".as_slice())
        );
        assert!(matches!(
            service.log().entries().last(),
            Some(PublicationOperationLogEntry::WriteFailed {
                kind: logged_kind,
                stage: WriteStage::Setup,
                issued: false,
                effect: WriteEffect::NoEffect,
                ..
            }) if *logged_kind == kind
        ));
    }
}

#[test]
fn direct_write_failures_after_issue_are_indeterminate_regardless_of_kind() {
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::AlreadyExists,
        ErrorKind::ConditionNotMatch,
    ] {
        let service = PublicationService::new(
            PublicationCapabilities::all(),
            [],
            [],
            [WriteScript::write_failure(
                "object.bin",
                WriteCondition::Direct,
                kind,
            )],
            8,
        );
        let operator = service.operator();
        let mut write = pin!(operator.write("object.bin", b"payload".to_vec()));
        let error = expect_ready(write.as_mut()).unwrap_err();

        assert_eq!(error.kind(), kind);
        assert!(service.destination().object("object.bin").is_none());
        assert!(matches!(
            service.log().entries().last(),
            Some(PublicationOperationLogEntry::WriteFailed {
                length: 7,
                kind: logged_kind,
                stage: WriteStage::Write,
                issued: true,
                effect: WriteEffect::Indeterminate,
                ..
            }) if *logged_kind == kind
        ));
    }
}

#[test]
fn publication_read_observes_mutation_between_chunks_not_a_snapshot() {
    let read = PublicationReadScript::new(
        "race.bin",
        2,
        [
            PublicationReadStep::chunk(0..3),
            PublicationReadStep::mutate(DestinationMutation::set("race.bin", b"abcXYZ")),
            PublicationReadStep::chunk(3..6),
        ],
    )
    .unwrap();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("race.bin".to_owned(), b"abcdef".to_vec())],
        [read],
        [],
        16,
    );
    let operator = service.operator();
    let mut read = pin!(operator.read("race.bin"));

    assert_eq!(expect_ready(read.as_mut()).unwrap().to_vec(), b"abcXYZ");
    assert_eq!(
        service.destination().object("race.bin"),
        Some(b"abcXYZ".as_slice())
    );
    assert_eq!(
        service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, PublicationOperationLogEntry::ReadChunkYielded { .. }))
            .count(),
        2
    );
}

#[test]
fn matching_publication_read_completes_without_a_write_effect() {
    let read =
        PublicationReadScript::new("matching.bin", 1, [PublicationReadStep::chunk(0..8)]).unwrap();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("matching.bin".to_owned(), b"matching".to_vec())],
        [read],
        [],
        8,
    );
    let operator = service.operator();
    let mut read = pin!(operator.read("matching.bin"));

    assert_eq!(expect_ready(read.as_mut()).unwrap().to_vec(), b"matching");
    assert_eq!(
        service.destination().object("matching.bin"),
        Some(b"matching".as_slice())
    );
    assert!(service.log().entries().iter().all(|entry| !matches!(
        entry,
        PublicationOperationLogEntry::WriteInvoked { .. }
            | PublicationOperationLogEntry::WriteAccepted { .. }
            | PublicationOperationLogEntry::WriteCompleted { .. }
            | PublicationOperationLogEntry::WriteFailed { .. }
            | PublicationOperationLogEntry::WriteDropped { .. }
    )));
}

#[test]
fn publication_read_preserves_typed_failure_without_mutation() {
    let read = PublicationReadScript::new(
        "denied.bin",
        0,
        [PublicationReadStep::failure(ErrorKind::PermissionDenied)],
    )
    .unwrap();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("denied.bin".to_owned(), b"original".to_vec())],
        [read],
        [],
        8,
    );
    let operator = service.operator();
    let mut read = pin!(operator.read("denied.bin"));
    let error = expect_ready(read.as_mut()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(
        service.destination().object("denied.bin"),
        Some(b"original".as_slice())
    );
    assert!(matches!(
        service.log().entries().last(),
        Some(PublicationOperationLogEntry::ReadFailed {
            kind: ErrorKind::PermissionDenied,
            ..
        })
    ));
}

#[test]
fn dropping_a_pending_publication_write_records_issued_cancellation() {
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "pending.bin",
            WriteCondition::Direct,
            [WriteStep::pending(pending.clone())],
        )],
        8,
    );
    let operator = service.operator();
    {
        let mut write = pin!(operator.write("pending.bin", b"payload".to_vec()));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(
        service.cancellations(),
        [PublicationDroppedOperation::Write {
            id: 0,
            path: "pending.bin".to_owned(),
            length: 7,
            condition: WriteCondition::Direct,
            issued: true,
        }]
    );
    assert!(service.destination().object("pending.bin").is_none());
}

#[test]
fn dropping_a_pending_publication_read_records_cancellation() {
    let pending = PendingPoint::new();
    let read = PublicationReadScript::new(
        "pending.bin",
        1,
        [
            PublicationReadStep::pending(pending.clone()),
            PublicationReadStep::chunk(0..7),
        ],
    )
    .unwrap();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [("pending.bin".to_owned(), b"payload".to_vec())],
        [read],
        [],
        8,
    );
    let operator = service.operator();
    {
        let mut read = pin!(operator.read("pending.bin"));
        assert!(matches!(poll_once(read.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(
        service.cancellations(),
        [PublicationDroppedOperation::Read {
            id: 0,
            path: "pending.bin".to_owned(),
        }]
    );
}

#[test]
fn caller_mutation_controls_a_conditional_create_race() {
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "race.bin",
            WriteCondition::IfNotExists,
            [WriteStep::pending(pending.clone()), WriteStep::commit()],
        )],
        16,
    );
    let operator = service.operator();
    let mut write = pin!(
        operator
            .write_with("race.bin", b"planned".to_vec())
            .if_not_exists(true)
            .into_future()
    );
    assert!(matches!(poll_once(write.as_mut()), Poll::Pending));

    service.mutate(DestinationMutation::set("race.bin", b"racer"));
    pending.release();
    let error = expect_ready(write.as_mut()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(
        service.destination().object("race.bin"),
        Some(b"racer".as_slice())
    );
    assert!(matches!(
        service.log().entries().last(),
        Some(PublicationOperationLogEntry::WriteFailed {
            stage: WriteStage::Close,
            issued: true,
            effect: WriteEffect::NoEffect,
            ..
        })
    ));
}

#[test]
fn an_issued_write_can_commit_then_fail_with_indeterminate_evidence() {
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "uncertain.bin",
            WriteCondition::Direct,
            [
                WriteStep::commit(),
                WriteStep::failure(ErrorKind::Unexpected),
            ],
        )],
        16,
    );
    let operator = service.operator();
    let mut write = pin!(operator.write("uncertain.bin", b"committed".to_vec()));
    let error = expect_ready(write.as_mut()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::Unexpected);
    assert_eq!(
        service.destination().object("uncertain.bin"),
        Some(b"committed".as_slice())
    );
    assert!(matches!(
        service.log().entries().last(),
        Some(PublicationOperationLogEntry::WriteFailed {
            stage: WriteStage::Close,
            issued: true,
            effect: WriteEffect::Indeterminate,
            destination,
            ..
        }) if destination.object("uncertain.bin") == Some(b"committed".as_slice())
    ));
}

#[test]
fn dropping_a_pending_empty_write_records_issued_indeterminate_evidence() {
    let pending = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new(
            "empty.bin",
            WriteCondition::Direct,
            [WriteStep::pending(pending.clone())],
        )],
        8,
    );
    let operator = service.operator();
    {
        let mut write = pin!(operator.write("empty.bin", Vec::<u8>::new()));
        assert!(matches!(poll_once(write.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }

    assert_eq!(
        service.cancellations(),
        [PublicationDroppedOperation::Write {
            id: 0,
            path: "empty.bin".to_owned(),
            length: 0,
            condition: WriteCondition::Direct,
            issued: true,
        }]
    );
    assert!(matches!(
        service.log().entries().last(),
        Some(PublicationOperationLogEntry::WriteDropped {
            length: 0,
            issued: true,
            effect: WriteEffect::Indeterminate,
            ..
        })
    ));
}

#[test]
fn publication_completion_order_is_controlled_by_pending_points() {
    let first = PendingPoint::new();
    let second = PendingPoint::new();
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [
            WriteScript::new(
                "first.bin",
                WriteCondition::Direct,
                [WriteStep::pending(first.clone())],
            ),
            WriteScript::new(
                "second.bin",
                WriteCondition::Direct,
                [WriteStep::pending(second.clone())],
            ),
        ],
        32,
    );
    let operator = service.operator();
    let mut first_write = pin!(operator.write("first.bin", b"first".to_vec()));
    let mut second_write = pin!(operator.write("second.bin", b"second".to_vec()));
    assert!(matches!(poll_once(first_write.as_mut()), Poll::Pending));
    assert!(matches!(poll_once(second_write.as_mut()), Poll::Pending));

    second.release();
    expect_ready(second_write.as_mut()).unwrap();
    first.release();
    expect_ready(first_write.as_mut()).unwrap();

    let log = service.log();
    let completed = log
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            PublicationOperationLogEntry::WriteCompleted { path, .. } => Some(path.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(completed, ["second.bin", "first.bin"]);
}

#[test]
fn publication_log_never_retains_more_than_its_capacity() {
    let service = PublicationService::new(
        PublicationCapabilities::all(),
        [],
        [],
        [WriteScript::new("object.bin", WriteCondition::Direct, [])],
        1,
    );
    let operator = service.operator();
    let mut write = pin!(operator.write("object.bin", b"payload".to_vec()));
    expect_ready(write.as_mut()).unwrap();

    assert_eq!(service.log().entries().len(), 1);
    assert_eq!(service.log().omitted_entries(), 2);
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
