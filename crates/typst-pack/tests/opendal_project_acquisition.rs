#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::error::Error as _;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use opendal::ErrorKind;
use scripted_opendal::{
    Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
    PendingPoint, ReadScript, ReadStep, ScriptedService,
};
use typst_pack::ProjectSnapshotAssembly;
use typst_pack::opendal::pack_assembly::{
    ProjectAcquisitionCeilings, ProjectAcquisitionEntry, ProjectAcquisitionErrorCause,
    ProjectAcquisitionIssue, ProjectAcquisitionLimitError, ProjectAcquisitionLimits,
    ProjectAcquisitionLimitsError, ProjectAcquisitionRequest, ProjectAcquisitionRequestError,
    ProjectAcquisitionResource, acquire_project,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};

#[test]
fn acquires_every_yielded_project_file_and_hands_exact_entries_to_snapshot_assembly() {
    let list = ListScript::new(
        "/",
        4,
        [ListStep::page([
            ListEntry::file("notes.txt"),
            ListEntry::directory("chapters/"),
            ListEntry::file(".typkignore"),
            ListEntry::file("main.typ"),
        ])],
    )
    .unwrap();
    let reads = [
        ReadScript::new(".typkignore", 1, [ReadStep::chunk(b"ignored.typ")]).unwrap(),
        ReadScript::new("main.typ", 1, [ReadStep::chunk(b"= Main")]).unwrap(),
        ReadScript::new("notes.txt", 1, [ReadStep::chunk(b"exact notes")]).unwrap(),
    ];
    let service = ScriptedService::new(Capabilities::all(), [list], reads, 32);
    let binding = OperatorBinding::new("project").unwrap();
    let bindings = OperatorBindings::new([(binding.clone(), service.operator())]).unwrap();
    let source = Location::from_operation_path(binding, "").unwrap();
    let request =
        ProjectAcquisitionRequest::new(source.clone(), ProjectAcquisitionLimits::reference_v1())
            .unwrap();

    assert_eq!(request.source(), &source);
    assert_eq!(request.limits(), ProjectAcquisitionLimits::reference_v1());

    let acquisition = expect_ready(pin!(acquire_project(&bindings, &request))).unwrap();
    assert_eq!(acquisition.source(), &source);
    assert_eq!(
        acquisition
            .entries()
            .iter()
            .map(|entry| (
                entry.relative_path(),
                entry.bytes(),
                entry.len(),
                entry.is_empty()
            ))
            .collect::<Vec<_>>(),
        [
            (".typkignore", b"ignored.typ".as_slice(), 11, false),
            ("main.typ", b"= Main".as_slice(), 6, false),
            ("notes.txt", b"exact notes".as_slice(), 11, false),
        ]
    );

    let (acquired_source, entries) = acquisition.into_parts();
    assert_eq!(acquired_source, source);
    let snapshot = ProjectSnapshotAssembly::new("main.typ")
        .assemble(entries.into_iter().map(ProjectAcquisitionEntry::into_parts))
        .unwrap();

    assert_eq!(snapshot.file("main.typ"), Some(b"= Main".as_slice()));
    assert_eq!(
        snapshot.file(".typkignore"),
        Some(b"ignored.typ".as_slice())
    );
    assert_eq!(snapshot.file("notes.txt"), Some(b"exact notes".as_slice()));
}

#[test]
fn named_project_ceilings_validate_probe_room_and_payload_relationships() {
    let reference = ProjectAcquisitionCeilings::reference_v1();
    let narrowed = ProjectAcquisitionLimits::new(ProjectAcquisitionCeilings {
        listed_entries: u64::MAX,
        listed_path_bytes: u64::MAX,
        total_listed_path_bytes: u64::MAX,
        total_bytes: reference.object_bytes,
        ..reference
    })
    .unwrap();

    assert_eq!(narrowed.listed_entries(), u64::MAX);
    assert_eq!(narrowed.listed_path_bytes(), u64::MAX);
    assert_eq!(narrowed.total_listed_path_bytes(), u64::MAX);
    assert_eq!(narrowed.selected_files(), reference.selected_files);
    assert_eq!(narrowed.object_bytes(), reference.object_bytes);
    assert_eq!(narrowed.total_bytes(), reference.object_bytes);

    for (resource, ceilings) in [
        (
            ProjectAcquisitionResource::ObjectBytes,
            ProjectAcquisitionCeilings {
                object_bytes: u64::MAX,
                total_bytes: u64::MAX,
                ..reference
            },
        ),
        (
            ProjectAcquisitionResource::TotalBytes,
            ProjectAcquisitionCeilings {
                total_bytes: u64::MAX,
                ..reference
            },
        ),
    ] {
        assert!(matches!(
            ProjectAcquisitionLimits::new(ceilings),
            Err(ProjectAcquisitionLimitsError::CannotProbe {
                resource: actual,
                ceiling: u64::MAX,
            }) if actual == resource
        ));
    }

    assert!(matches!(
        ProjectAcquisitionLimits::new(ProjectAcquisitionCeilings {
            object_bytes: 2,
            total_bytes: 1,
            ..reference
        }),
        Err(ProjectAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
            object_bytes: 2,
            total_bytes: 1,
        })
    ));
}

#[test]
fn request_rejects_an_exact_object_before_operator_resolution() {
    let source: Location = "project:/main.typ".parse().unwrap();
    let error =
        ProjectAcquisitionRequest::new(source.clone(), ProjectAcquisitionLimits::reference_v1())
            .unwrap_err();

    assert!(matches!(
        error,
        ProjectAcquisitionRequestError::InvalidSourceRole {
            location,
            source: LocationRoleError::PrefixMissingTrailingSlash,
        } if location == source
    ));
}

#[test]
fn structural_issues_are_typed_canonical_and_precede_all_reads() {
    let list = ListScript::new(
        "project/",
        5,
        [ListStep::page([
            ListEntry::unknown("project/z"),
            ListEntry::file("project-sibling/a"),
            ListEntry::file("project/a//b"),
            ListEntry::file("project/main.typ"),
            ListEntry::file("project/main.typ"),
        ])],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [list], [], 16);
    let request = request("project/", ProjectAcquisitionLimits::reference_v1());
    let bindings = bindings(&service);

    let error = expect_ready(pin!(acquire_project(&bindings, &request))).unwrap_err();
    let ProjectAcquisitionErrorCause::Structural(survey) = error.cause() else {
        panic!("unexpected cause: {:?}", error.cause());
    };
    assert_eq!(
        survey.issues(),
        [
            ProjectAcquisitionIssue::ListedPathOutsidePrefix {
                operation_path: "project-sibling/a".to_owned(),
            },
            ProjectAcquisitionIssue::InvalidRelativeOperationPath {
                operation_path: "project/a//b".to_owned(),
            },
            ProjectAcquisitionIssue::DuplicateListedObject {
                operation_path: "project/main.typ".to_owned(),
            },
            ProjectAcquisitionIssue::UnsupportedEntryKind {
                operation_path: "project/z".to_owned(),
                kind: typst_pack::opendal::pack_assembly::ProjectAcquisitionEntryKind::Unknown,
            },
        ]
    );
    assert_eq!(survey.to_string(), "project survey failed with 4 issue(s)");
    assert!(
        service
            .log()
            .entries()
            .iter()
            .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
    );
}

#[test]
fn project_limits_map_every_operation_specific_resource_at_exact_boundaries() {
    let reference = ProjectAcquisitionCeilings::reference_v1();
    let cases = [
        (
            ProjectAcquisitionResource::ListedEntries,
            ProjectAcquisitionCeilings {
                listed_entries: 0,
                ..reference
            },
            ListEntry::directory("p/dir/"),
        ),
        (
            ProjectAcquisitionResource::ListedPathBytes,
            ProjectAcquisitionCeilings {
                listed_path_bytes: 1,
                ..reference
            },
            ListEntry::directory("p/dir/"),
        ),
        (
            ProjectAcquisitionResource::TotalListedPathBytes,
            ProjectAcquisitionCeilings {
                total_listed_path_bytes: 1,
                ..reference
            },
            ListEntry::file("p/a"),
        ),
        (
            ProjectAcquisitionResource::SelectedFiles,
            ProjectAcquisitionCeilings {
                selected_files: 0,
                ..reference
            },
            ListEntry::file("p/a"),
        ),
    ];

    for (resource, ceilings, entry) in cases {
        let list = ListScript::new("p/", 1, [ListStep::page([entry])]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [], 8);
        let limits = ProjectAcquisitionLimits::new(ceilings).unwrap();
        let request = request("p/", limits);
        let configured = bindings(&service);
        let error = expect_ready(pin!(acquire_project(&configured, &request))).unwrap_err();

        assert!(matches!(
            error.cause(),
            ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::Exceeded {
                resource: actual,
                ..
            }) if *actual == resource
        ));
    }

    let list = ListScript::new("p/", 1, [ListStep::page([ListEntry::file("p/a")])]).unwrap();
    let read = ReadScript::new("p/a", 1, [ReadStep::chunk(b"four")]).unwrap();
    let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
    let limits = ProjectAcquisitionLimits::new(ProjectAcquisitionCeilings {
        object_bytes: 3,
        total_bytes: 8,
        ..reference
    })
    .unwrap();
    let object_request = request("p/", limits);
    let configured = bindings(&service);
    let error = expect_ready(pin!(acquire_project(&configured, &object_request))).unwrap_err();
    assert!(matches!(
        error.cause(),
        ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::Exceeded {
            resource: ProjectAcquisitionResource::ObjectBytes,
            ceiling: 3,
            observed_at_least: 4,
        })
    ));

    let list = ListScript::new(
        "p/",
        2,
        [ListStep::page([
            ListEntry::file("p/a"),
            ListEntry::file("p/b"),
        ])],
    )
    .unwrap();
    let reads = [
        ReadScript::new("p/a", 1, [ReadStep::chunk(b"12")]).unwrap(),
        ReadScript::new("p/b", 1, [ReadStep::chunk(b"34")]).unwrap(),
    ];
    let service = ScriptedService::new(Capabilities::all(), [list], reads, 12);
    let limits = ProjectAcquisitionLimits::new(ProjectAcquisitionCeilings {
        object_bytes: 3,
        total_bytes: 3,
        ..reference
    })
    .unwrap();
    let request = request("p/", limits);
    let configured = bindings(&service);
    let error = expect_ready(pin!(acquire_project(&configured, &request))).unwrap_err();
    assert!(matches!(
        error.cause(),
        ProjectAcquisitionErrorCause::Limit(ProjectAcquisitionLimitError::Exceeded {
            resource: ProjectAcquisitionResource::TotalBytes,
            ceiling: 3,
            observed_at_least: 4,
        })
    ));
}

#[test]
fn mutation_disappearance_and_cancellation_retain_exact_typed_evidence() {
    let replacement =
        ReadScript::new("race/changing.typ", 1, [ReadStep::chunk(b"after listing")]).unwrap();
    let list = ListScript::new(
        "race/",
        1,
        [
            ListStep::page([ListEntry::file("race/changing.typ")]),
            ListStep::replace_read(replacement),
        ],
    )
    .unwrap();
    let original =
        ReadScript::new("race/changing.typ", 1, [ReadStep::chunk(b"during listing")]).unwrap();
    let service = ScriptedService::new(Capabilities::all(), [list], [original], 8);
    let configured = bindings(&service);
    let request = request("race/", ProjectAcquisitionLimits::reference_v1());
    let acquired = expect_ready(pin!(acquire_project(&configured, &request))).unwrap();
    assert_eq!(acquired.entries()[0].bytes(), b"after listing");

    let absent_list = ListScript::new(
        "race/",
        1,
        [ListStep::page([ListEntry::file("race/gone.typ")])],
    )
    .unwrap();
    let absent_service = ScriptedService::new(Capabilities::all(), [absent_list], [], 8);
    let absent_bindings = bindings(&absent_service);
    let absent = expect_ready(pin!(acquire_project(&absent_bindings, &request))).unwrap_err();
    assert_eq!(absent.failed_path(), Some("race/gone.typ"));
    assert!(matches!(
        absent.cause(),
        ProjectAcquisitionErrorCause::ListedObjectAbsent(source)
            if source.kind() == ErrorKind::NotFound
    ));

    let pending = PendingPoint::new();
    let pending_list = ListScript::new(
        "race/",
        1,
        [
            ListStep::page([ListEntry::file("race/pending.typ")]),
            ListStep::pending(pending.clone()),
        ],
    )
    .unwrap();
    let pending_read = ReadScript::new("race/pending.typ", 1, [ReadStep::chunk(b"never")]).unwrap();
    let pending_service =
        ScriptedService::new(Capabilities::all(), [pending_list], [pending_read], 8);
    let pending_bindings = bindings(&pending_service);
    {
        let mut acquisition = pin!(acquire_project(&pending_bindings, &request));
        assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }
    assert_eq!(
        pending_service.cancellations(),
        [DroppedOperation::List {
            id: 0,
            path: "race/".to_owned(),
        }]
    );
}

#[test]
fn operator_bindings_make_the_public_project_future_send() {
    let service = ScriptedService::new(Capabilities::all(), [], [], 1);
    let configured = bindings(&service);
    let request = request("project/", ProjectAcquisitionLimits::reference_v1());

    assert_send(acquire_project(&configured, &request));
}

#[test]
fn memory_acquires_root_and_non_root_projects_through_the_public_operation() {
    for (prefix, path, relative) in [
        ("", "main.typ", "main.typ"),
        ("project/", "project/main.typ", "main.typ"),
    ] {
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        expect_ready(pin!(operator.write(path, b"memory project".to_vec()))).unwrap();
        let configured =
            OperatorBindings::new([(OperatorBinding::new("project").unwrap(), operator)]).unwrap();
        let request = request(prefix, ProjectAcquisitionLimits::reference_v1());

        let acquisition = expect_ready(pin!(acquire_project(&configured, &request))).unwrap();
        assert_eq!(acquisition.entries().len(), 1);
        assert_eq!(acquisition.entries()[0].relative_path(), relative);
        assert_eq!(acquisition.entries()[0].bytes(), b"memory project");
    }
}

#[test]
fn failures_keep_native_causes_typed_but_out_of_outer_diagnostics() {
    let list = ListScript::new(
        "project/",
        1,
        [ListStep::page([ListEntry::file("project/secret.typ")])],
    )
    .unwrap();
    let read = ReadScript::new(
        "project/secret.typ",
        1,
        [
            ReadStep::chunk(b"sensitive payload"),
            ReadStep::failure(ErrorKind::PermissionDenied),
        ],
    )
    .unwrap();
    let service = ScriptedService::new(Capabilities::all(), [list], [read], 16);
    let configured = bindings(&service);
    let request = request("project/", ProjectAcquisitionLimits::reference_v1());

    let error = expect_ready(pin!(acquire_project(&configured, &request))).unwrap_err();
    assert_eq!(error.source_location(), request.source());
    assert_eq!(error.failed_path(), Some("project/secret.typ"));
    assert!(matches!(
        error.cause(),
        ProjectAcquisitionErrorCause::Read(source)
            if source.kind() == ErrorKind::PermissionDenied
    ));
    assert_eq!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<opendal::Error>()
            .unwrap()
            .kind(),
        ErrorKind::PermissionDenied
    );
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(rendered.contains("project"));
        assert!(rendered.contains("project/secret.typ"));
        assert!(!rendered.contains("sensitive"));
        assert!(!rendered.contains("scripted"));
    }
}

#[test]
fn resolver_failures_are_boxed_and_reachable_through_the_typed_cause() {
    let request = request("project/", ProjectAcquisitionLimits::reference_v1());

    let error = expect_ready(pin!(acquire_project(&FailingResolver, &request))).unwrap_err();
    assert!(matches!(
        error.cause(),
        ProjectAcquisitionErrorCause::ResolveOperator(source)
            if source.downcast_ref::<ResolveFailure>().is_some()
    ));
    assert!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<ResolveFailure>()
            .is_some()
    );
}

fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
    match poll_once(future.as_mut()) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("future unexpectedly pending"),
    }
}

fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn request(path: &str, limits: ProjectAcquisitionLimits) -> ProjectAcquisitionRequest {
    let source =
        Location::from_operation_path(OperatorBinding::new("project").unwrap(), path).unwrap();
    ProjectAcquisitionRequest::new(source, limits).unwrap()
}

fn bindings(service: &ScriptedService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("project").unwrap(), service.operator())]).unwrap()
}

fn assert_send<T: Send>(_: T) {}

#[derive(Debug, thiserror::Error)]
#[error("project resolver failure")]
struct ResolveFailure;

struct FailingResolver;

impl OperatorResolver for FailingResolver {
    type Error = ResolveFailure;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        Err(ResolveFailure)
    }
}
