#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod fonts;

use std::convert::Infallible;
use std::error::Error as _;
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use opendal::ErrorKind;
use scripted_opendal::{
    Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
    PendingPoint, ReadScript, ReadStep, ScriptedService,
};
use typst_pack::opendal::pack_assembly::{
    FontAcquisitionCeilings, FontAcquisitionErrorCause, FontAcquisitionIssue,
    FontAcquisitionLimitError, FontAcquisitionLimits, FontAcquisitionLimitsError,
    FontAcquisitionRequest, FontAcquisitionRequestIssue, FontAcquisitionResource, FontSource,
    acquire_fonts,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorResolver,
};
use typst_pack::{FontContainer, FontContainerError, FontDisposition};

#[test]
fn acquires_font_containers_in_source_then_relative_path_order() {
    let lists = [
        ListScript::new(
            "first/",
            4,
            [
                ListStep::page([
                    ListEntry::file("first/z.OTC"),
                    ListEntry::directory("first/ignored.ttf/"),
                ]),
                ListStep::page([
                    ListEntry::file("first/readme.txt"),
                    ListEntry::file("first/a.ttf"),
                ]),
            ],
        )
        .unwrap(),
        ListScript::new(
            "second/",
            2,
            [ListStep::page([
                ListEntry::file("second/b.TTC"),
                ListEntry::file("second/a.otf"),
            ])],
        )
        .unwrap(),
    ];
    let reads = [
        ReadScript::new("first/a.ttf", 1, [ReadStep::chunk(b"first a")]).unwrap(),
        ReadScript::new("first/z.OTC", 1, [ReadStep::chunk(b"first z")]).unwrap(),
        ReadScript::new("second/a.otf", 1, [ReadStep::chunk(b"second a")]).unwrap(),
        ReadScript::new("second/b.TTC", 1, [ReadStep::chunk(b"second b")]).unwrap(),
    ];
    let service = ScriptedService::new(Capabilities::all(), lists, reads, 32);
    let binding = OperatorBinding::new("fonts").unwrap();
    let bindings = OperatorBindings::new([(binding.clone(), service.operator())]).unwrap();
    let sources = [
        FontSource::new(
            Location::from_operation_path(binding.clone(), "first/").unwrap(),
            FontDisposition::Embedded,
        ),
        FontSource::new(
            Location::from_operation_path(binding, "second/").unwrap(),
            FontDisposition::External,
        ),
    ];
    let request =
        FontAcquisitionRequest::new(sources.clone(), FontAcquisitionLimits::reference_v1())
            .unwrap();

    assert_eq!(request.sources(), &sources);
    let acquisition = expect_ready(pin!(acquire_fonts(&bindings, &request))).unwrap();
    assert_eq!(acquisition.sources(), &sources);
    assert_eq!(
        acquisition
            .entries()
            .iter()
            .map(|entry| (
                entry.source_index(),
                entry.relative_path(),
                entry.disposition(),
                entry.bytes(),
            ))
            .collect::<Vec<_>>(),
        [
            (0, "a.ttf", FontDisposition::Embedded, b"first a".as_slice()),
            (0, "z.OTC", FontDisposition::Embedded, b"first z".as_slice()),
            (
                1,
                "a.otf",
                FontDisposition::External,
                b"second a".as_slice()
            ),
            (
                1,
                "b.TTC",
                FontDisposition::External,
                b"second b".as_slice()
            ),
        ]
    );

    let (acquired_sources, entries) = acquisition.into_parts();
    assert_eq!(acquired_sources, sources);
    let (source_index, source, path, disposition, bytes) =
        entries.into_iter().next().unwrap().into_parts();
    assert_eq!(source_index, 0);
    assert_eq!(source, sources[0].source().clone());
    assert_eq!(path, "a.ttf");
    assert_eq!(disposition, FontDisposition::Embedded);
    assert_eq!(bytes, b"first a");
    assert_eq!(
        FontContainer::new(bytes).unwrap_err(),
        FontContainerError::NoReadableFace
    );
}

#[test]
fn listing_permutations_produce_the_same_font_order() {
    for entries in [
        [
            ListEntry::file("fonts/a.ttf"),
            ListEntry::file("fonts/b.otf"),
        ],
        [
            ListEntry::file("fonts/b.otf"),
            ListEntry::file("fonts/a.ttf"),
        ],
    ] {
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new("fonts/", 2, [ListStep::page(entries)]).unwrap()],
            [
                ReadScript::new("fonts/a.ttf", 1, [ReadStep::chunk(b"a")]).unwrap(),
                ReadScript::new("fonts/b.otf", 1, [ReadStep::chunk(b"b")]).unwrap(),
            ],
            12,
        );
        let configured = bindings(&service);
        let request = request(&["fonts/"], FontAcquisitionLimits::reference_v1());

        let acquisition = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap();
        assert_eq!(
            acquisition
                .entries()
                .iter()
                .map(|entry| entry.relative_path())
                .collect::<Vec<_>>(),
            ["a.ttf", "b.otf"]
        );
    }
}

#[test]
fn named_font_ceilings_validate_probe_room_and_payload_relationships() {
    let reference = FontAcquisitionCeilings::reference_v1();
    assert_eq!(reference.listed_entries, 100_000);
    assert_eq!(reference.listed_path_bytes, 64 * 1024);
    assert_eq!(reference.total_listed_path_bytes, 64 * 1024 * 1024);
    assert_eq!(reference.selected_containers, 16_384);
    assert_eq!(reference.container_bytes, 256 * 1024 * 1024);
    assert_eq!(reference.total_bytes, 2 * 1024 * 1024 * 1024);

    let narrowed = FontAcquisitionLimits::new(FontAcquisitionCeilings {
        listed_entries: u64::MAX,
        listed_path_bytes: u64::MAX,
        total_listed_path_bytes: u64::MAX,
        total_bytes: reference.container_bytes,
        ..reference
    })
    .unwrap();
    assert_eq!(narrowed.listed_entries(), u64::MAX);
    assert_eq!(narrowed.listed_path_bytes(), u64::MAX);
    assert_eq!(narrowed.total_listed_path_bytes(), u64::MAX);
    assert_eq!(
        narrowed.selected_containers(),
        reference.selected_containers
    );
    assert_eq!(narrowed.container_bytes(), reference.container_bytes);
    assert_eq!(narrowed.total_bytes(), reference.container_bytes);

    for (resource, ceilings) in [
        (
            FontAcquisitionResource::ContainerBytes,
            FontAcquisitionCeilings {
                container_bytes: u64::MAX,
                total_bytes: u64::MAX,
                ..reference
            },
        ),
        (
            FontAcquisitionResource::TotalBytes,
            FontAcquisitionCeilings {
                total_bytes: u64::MAX,
                ..reference
            },
        ),
    ] {
        assert!(matches!(
            FontAcquisitionLimits::new(ceilings),
            Err(FontAcquisitionLimitsError::CannotProbe {
                resource: actual,
                ceiling: u64::MAX,
            }) if actual == resource
        ));
    }
    assert!(matches!(
        FontAcquisitionLimits::new(FontAcquisitionCeilings {
            container_bytes: 2,
            total_bytes: 1,
            ..reference
        }),
        Err(FontAcquisitionLimitsError::ContainerBytesExceedTotalBytes {
            container_bytes: 2,
            total_bytes: 1,
        })
    ));
}

#[test]
fn every_font_resource_maps_exact_and_plus_one_boundaries() {
    let reference = FontAcquisitionCeilings::reference_v1();
    let survey_cases = [
        (
            FontAcquisitionResource::ListedEntries,
            FontAcquisitionCeilings {
                listed_entries: 0,
                ..reference
            },
            ListEntry::directory("p/dir/"),
        ),
        (
            FontAcquisitionResource::ListedPathBytes,
            FontAcquisitionCeilings {
                listed_path_bytes: 5,
                ..reference
            },
            ListEntry::directory("p/long/"),
        ),
        (
            FontAcquisitionResource::TotalListedPathBytes,
            FontAcquisitionCeilings {
                total_listed_path_bytes: 11,
                ..reference
            },
            ListEntry::file("p/a.ttf"),
        ),
        (
            FontAcquisitionResource::SelectedContainers,
            FontAcquisitionCeilings {
                selected_containers: 0,
                ..reference
            },
            ListEntry::file("p/a.ttf"),
        ),
    ];
    for (resource, ceilings, entry) in survey_cases {
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new("p/", 1, [ListStep::page([entry])]).unwrap()],
            [],
            8,
        );
        let configured = bindings(&service);
        let request = request(&["p/"], FontAcquisitionLimits::new(ceilings).unwrap());
        let error = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap_err();

        assert!(matches!(
            error.cause(),
            FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
                resource: actual,
                ..
            }) if *actual == resource
        ));
    }

    for (resource, container_bytes, total_bytes) in
        [(FontAcquisitionResource::ContainerBytes, 3, 8)]
    {
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new("p/", 1, [ListStep::page([ListEntry::file("p/a.ttf")])]).unwrap()],
            [ReadScript::new("p/a.ttf", 1, [ReadStep::chunk(b"four")]).unwrap()],
            8,
        );
        let limits = FontAcquisitionLimits::new(FontAcquisitionCeilings {
            container_bytes,
            total_bytes,
            ..reference
        })
        .unwrap();
        let configured = bindings(&service);
        let request = request(&["p/"], limits);
        let error = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap_err();

        assert!(matches!(
            error.cause(),
            FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
                resource: actual,
                observed_at_least: 4,
                ..
            }) if *actual == resource
        ));
    }

    let exact_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new("p/", 1, [ListStep::page([ListEntry::file("p/a.ttf")])]).unwrap()],
        [ReadScript::new("p/a.ttf", 1, [ReadStep::chunk(b"four")]).unwrap()],
        8,
    );
    let exact_limits = FontAcquisitionLimits::new(FontAcquisitionCeilings {
        listed_entries: 1,
        listed_path_bytes: 7,
        total_listed_path_bytes: 12,
        selected_containers: 1,
        container_bytes: 4,
        total_bytes: 4,
    })
    .unwrap();
    let configured = bindings(&exact_service);
    let exact_request = request(&["p/"], exact_limits);
    let exact = expect_ready(pin!(acquire_fonts(&configured, &exact_request))).unwrap();
    assert_eq!(exact.entries()[0].bytes(), b"four");

    let precedence_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new("p/", 1, [ListStep::page([ListEntry::file("p/long.ttf")])]).unwrap()],
        [],
        8,
    );
    let precedence_limits = FontAcquisitionLimits::new(FontAcquisitionCeilings {
        listed_entries: 0,
        listed_path_bytes: 0,
        total_listed_path_bytes: 0,
        selected_containers: 0,
        container_bytes: 0,
        total_bytes: 0,
    })
    .unwrap();
    let configured = bindings(&precedence_service);
    let precedence_request = request(&["p/"], precedence_limits);
    let error = expect_ready(pin!(acquire_fonts(&configured, &precedence_request))).unwrap_err();
    assert!(matches!(
        error.cause(),
        FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
            resource: FontAcquisitionResource::ListedEntries,
            ceiling: 0,
            observed_at_least: 1,
        })
    ));
}

#[test]
fn request_aggregates_invalid_source_roles_in_caller_order() {
    let sources = [
        FontSource::new("fonts:/one.ttf".parse().unwrap(), FontDisposition::Embedded),
        FontSource::new("fonts:/valid/".parse().unwrap(), FontDisposition::External),
        FontSource::new("fonts:/two.otf".parse().unwrap(), FontDisposition::External),
    ];

    let rejection =
        FontAcquisitionRequest::new(sources, FontAcquisitionLimits::reference_v1()).unwrap_err();
    assert_eq!(
        rejection.issues(),
        [
            FontAcquisitionRequestIssue::InvalidSourceRole {
                source_index: 0,
                location: "fonts:/one.ttf".parse().unwrap(),
                source: LocationRoleError::PrefixMissingTrailingSlash,
            },
            FontAcquisitionRequestIssue::InvalidSourceRole {
                source_index: 2,
                location: "fonts:/two.otf".parse().unwrap(),
                source: LocationRoleError::PrefixMissingTrailingSlash,
            },
        ]
    );
}

#[test]
fn all_source_surveys_finish_before_reads_and_one_binding_is_resolved_once() {
    let lists = [
        ListScript::new(
            "first/",
            1,
            [ListStep::page([ListEntry::file("first/a.ttf")])],
        )
        .unwrap(),
        ListScript::new(
            "second/",
            1,
            [ListStep::page([ListEntry::file("second/b.otf")])],
        )
        .unwrap(),
    ];
    let reads = [
        ReadScript::new("first/a.ttf", 1, [ReadStep::chunk(b"a")]).unwrap(),
        ReadScript::new("second/b.otf", 1, [ReadStep::chunk(b"b")]).unwrap(),
    ];
    let service = ScriptedService::new(Capabilities::all(), lists, reads, 16);
    let resolver = CountingResolver::new(service.operator());
    let request = request(
        &["first/", "second/"],
        FontAcquisitionLimits::reference_v1(),
    );

    expect_ready(pin!(acquire_fonts(&resolver, &request))).unwrap();
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    let log = service.log();
    let first_read = log
        .entries()
        .iter()
        .position(|entry| matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        .unwrap();
    assert_eq!(
        log.entries()[..first_read]
            .iter()
            .filter(|entry| matches!(entry, OperationLogEntry::ListCompleted { .. }))
            .count(),
        2
    );
}

#[test]
fn survey_and_payload_limits_are_shared_across_sources() {
    let listed_service = ScriptedService::new(
        Capabilities::all(),
        [
            ListScript::new(
                "first/",
                1,
                [ListStep::page([ListEntry::directory("first/dir/")])],
            )
            .unwrap(),
            ListScript::new(
                "second/",
                1,
                [ListStep::page([ListEntry::directory("second/dir/")])],
            )
            .unwrap(),
        ],
        [],
        12,
    );
    let limits = FontAcquisitionLimits::new(FontAcquisitionCeilings {
        listed_entries: 1,
        ..FontAcquisitionCeilings::reference_v1()
    })
    .unwrap();
    let listed_request = request(&["first/", "second/"], limits);
    let listed_bindings = bindings(&listed_service);
    let listed = expect_ready(pin!(acquire_fonts(&listed_bindings, &listed_request))).unwrap_err();
    assert_eq!(listed.source_index(), 1);
    assert!(matches!(
        listed.cause(),
        FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
            resource: FontAcquisitionResource::ListedEntries,
            ceiling: 1,
            observed_at_least: 2,
        })
    ));

    let payload_service = ScriptedService::new(
        Capabilities::all(),
        [
            ListScript::new(
                "first/",
                1,
                [ListStep::page([ListEntry::file("first/a.ttf")])],
            )
            .unwrap(),
            ListScript::new(
                "second/",
                1,
                [ListStep::page([ListEntry::file("second/b.ttf")])],
            )
            .unwrap(),
        ],
        [
            ReadScript::new("first/a.ttf", 1, [ReadStep::chunk(b"12")]).unwrap(),
            ReadScript::new("second/b.ttf", 1, [ReadStep::chunk(b"34")]).unwrap(),
        ],
        16,
    );
    let limits = FontAcquisitionLimits::new(FontAcquisitionCeilings {
        container_bytes: 3,
        total_bytes: 3,
        ..FontAcquisitionCeilings::reference_v1()
    })
    .unwrap();
    let payload_request = request(&["first/", "second/"], limits);
    let payload_bindings = bindings(&payload_service);
    let payload =
        expect_ready(pin!(acquire_fonts(&payload_bindings, &payload_request))).unwrap_err();
    assert_eq!(payload.source_index(), 1);
    assert!(matches!(
        payload.cause(),
        FontAcquisitionErrorCause::Limit(FontAcquisitionLimitError::Exceeded {
            resource: FontAcquisitionResource::TotalBytes,
            ceiling: 3,
            observed_at_least: 4,
        })
    ));
}

#[test]
fn structural_issues_are_aggregated_in_source_then_path_order() {
    let service = ScriptedService::new(
        Capabilities::all(),
        [
            ListScript::new(
                "first/",
                1,
                [ListStep::page([ListEntry::unknown("first/z.ttf")])],
            )
            .unwrap(),
            ListScript::new(
                "second/",
                1,
                [ListStep::page([ListEntry::file("outside/a.ttf")])],
            )
            .unwrap(),
        ],
        [],
        12,
    );
    let configured = bindings(&service);
    let request = request(
        &["first/", "second/"],
        FontAcquisitionLimits::reference_v1(),
    );

    let error = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap_err();
    assert_eq!(error.source_index(), 0);
    let FontAcquisitionErrorCause::Structural(survey) = error.cause() else {
        panic!("unexpected cause: {:?}", error.cause());
    };
    assert_eq!(
        survey.issues(),
        [
            FontAcquisitionIssue::UnsupportedEntryKind {
                source_index: 0,
                operation_path: "first/z.ttf".to_owned(),
                kind: typst_pack::opendal::pack_assembly::FontAcquisitionEntryKind::Unknown,
            },
            FontAcquisitionIssue::ListedPathOutsidePrefix {
                source_index: 1,
                operation_path: "outside/a.ttf".to_owned(),
            },
        ]
    );
    assert!(
        service
            .log()
            .entries()
            .iter()
            .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
    );
}

#[test]
fn disappearance_cancellation_and_native_diagnostics_keep_source_evidence() {
    let request = request(&["fonts/"], FontAcquisitionLimits::reference_v1());
    let replacement =
        ReadScript::new("fonts/changing.ttf", 1, [ReadStep::chunk(b"after listing")]).unwrap();
    let mutation_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "fonts/",
            1,
            [
                ListStep::page([ListEntry::file("fonts/changing.ttf")]),
                ListStep::replace_read(replacement),
            ],
        )
        .unwrap()],
        [ReadScript::new(
            "fonts/changing.ttf",
            1,
            [ReadStep::chunk(b"during listing")],
        )
        .unwrap()],
        8,
    );
    let mutation_bindings = bindings(&mutation_service);
    let mutation = expect_ready(pin!(acquire_fonts(&mutation_bindings, &request))).unwrap();
    assert_eq!(mutation.entries()[0].bytes(), b"after listing");

    let absent_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "fonts/",
            1,
            [ListStep::page([ListEntry::file("fonts/gone.ttf")])],
        )
        .unwrap()],
        [],
        8,
    );
    let configured = bindings(&absent_service);
    let absent = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap_err();
    assert_eq!(absent.source_index(), 0);
    assert_eq!(absent.source_location(), request.sources()[0].source());
    assert_eq!(absent.failed_path(), Some("fonts/gone.ttf"));
    assert!(matches!(
        absent.cause(),
        FontAcquisitionErrorCause::ListedObjectAbsent(source)
            if source.kind() == ErrorKind::NotFound
    ));

    let pending = PendingPoint::new();
    let pending_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "fonts/",
            1,
            [
                ListStep::page([ListEntry::file("fonts/pending.ttf")]),
                ListStep::pending(pending.clone()),
            ],
        )
        .unwrap()],
        [ReadScript::new("fonts/pending.ttf", 1, [ReadStep::chunk(b"never")]).unwrap()],
        8,
    );
    let pending_bindings = bindings(&pending_service);
    {
        let mut acquisition = pin!(acquire_fonts(&pending_bindings, &request));
        assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
        assert!(pending.was_observed());
    }
    assert_eq!(
        pending_service.cancellations(),
        [DroppedOperation::List {
            id: 0,
            path: "fonts/".to_owned(),
        }]
    );

    let list_failure_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "fonts/",
            1,
            [
                ListStep::page([ListEntry::file("fonts/unread.ttf")]),
                ListStep::failure(ErrorKind::PermissionDenied),
            ],
        )
        .unwrap()],
        [ReadScript::new("fonts/unread.ttf", 1, [ReadStep::chunk(b"unread")]).unwrap()],
        8,
    );
    let list_failure_bindings = bindings(&list_failure_service);
    let list_failure =
        expect_ready(pin!(acquire_fonts(&list_failure_bindings, &request))).unwrap_err();
    assert!(matches!(
        list_failure.cause(),
        FontAcquisitionErrorCause::List(source)
            if source.kind() == ErrorKind::PermissionDenied
    ));
    assert!(
        list_failure_service
            .log()
            .entries()
            .iter()
            .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
    );

    let failure_service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "fonts/",
            1,
            [ListStep::page([ListEntry::file("fonts/secret.ttf")])],
        )
        .unwrap()],
        [ReadScript::new(
            "fonts/secret.ttf",
            1,
            [
                ReadStep::chunk(b"sensitive font bytes"),
                ReadStep::failure(ErrorKind::PermissionDenied),
            ],
        )
        .unwrap()],
        12,
    );
    let failure_bindings = bindings(&failure_service);
    let error = expect_ready(pin!(acquire_fonts(&failure_bindings, &request))).unwrap_err();
    assert!(matches!(
        error.cause(),
        FontAcquisitionErrorCause::Read(source)
            if source.kind() == ErrorKind::PermissionDenied
    ));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<opendal::Error>()
            .unwrap()
            .kind(),
        ErrorKind::PermissionDenied
    );
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(rendered.contains("fonts"));
        assert!(rendered.contains("fonts/secret.ttf"));
        assert!(!rendered.contains("sensitive"));
        assert!(!rendered.contains("scripted"));
    }
}

#[test]
fn operator_bindings_make_the_public_font_future_send() {
    let service = ScriptedService::new(Capabilities::all(), [], [], 1);
    let configured = bindings(&service);
    let request = FontAcquisitionRequest::new([], FontAcquisitionLimits::reference_v1()).unwrap();

    assert_send(acquire_fonts(&configured, &request));
}

#[test]
fn memory_acquires_root_and_non_root_font_prefixes_without_ambient_fonts() {
    for (prefix, path, relative) in [
        ("", "root.ttf", "root.ttf"),
        ("fonts/", "fonts/nested.OTF", "nested.OTF"),
    ] {
        let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
        expect_ready(pin!(
            operator.write(path, b"exact container input".to_vec())
        ))
        .unwrap();
        let binding = OperatorBinding::new("fonts").unwrap();
        let configured = OperatorBindings::new([(binding.clone(), operator)]).unwrap();
        let request = FontAcquisitionRequest::new(
            [FontSource::new(
                Location::from_operation_path(binding, prefix).unwrap(),
                FontDisposition::External,
            )],
            FontAcquisitionLimits::reference_v1(),
        )
        .unwrap();

        let acquisition = expect_ready(pin!(acquire_fonts(&configured, &request))).unwrap();
        assert_eq!(acquisition.entries().len(), 1);
        assert_eq!(acquisition.entries()[0].relative_path(), relative);
        assert_eq!(acquisition.entries()[0].bytes(), b"exact container input");
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn exact_acquired_bytes_build_the_authoritative_font_catalog() {
    use typst_pack::{FontCatalog, FontCatalogEntry, FontContainer};

    let bytes = fonts::typst_container();
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    expect_ready(pin!(operator.write("fonts/container.ttf", bytes.clone()))).unwrap();
    let binding = OperatorBinding::new("fonts").unwrap();
    let configured = OperatorBindings::new([(binding.clone(), operator)]).unwrap();
    let request = FontAcquisitionRequest::new(
        [FontSource::new(
            Location::from_operation_path(binding, "fonts/").unwrap(),
            FontDisposition::External,
        )],
        FontAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let (_, entries) = expect_ready(pin!(acquire_fonts(&configured, &request)))
        .unwrap()
        .into_parts();
    let mut catalog = FontCatalog::new();
    for entry in entries {
        let (_, _, path, disposition, acquired) = entry.into_parts();
        assert_eq!(path, "container.ttf");
        assert_eq!(acquired, bytes);
        catalog.push(FontCatalogEntry::new(
            FontContainer::new(acquired).unwrap(),
            disposition,
        ));
    }
    assert_eq!(catalog.entries().len(), 1);
    assert_eq!(
        catalog.entries()[0].disposition(),
        FontDisposition::External
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

fn request(paths: &[&str], limits: FontAcquisitionLimits) -> FontAcquisitionRequest {
    let binding = OperatorBinding::new("fonts").unwrap();
    FontAcquisitionRequest::new(
        paths.iter().enumerate().map(|(index, path)| {
            FontSource::new(
                Location::from_operation_path(binding.clone(), path).unwrap(),
                if index % 2 == 0 {
                    FontDisposition::Embedded
                } else {
                    FontDisposition::External
                },
            )
        }),
        limits,
    )
    .unwrap()
}

fn bindings(service: &ScriptedService) -> OperatorBindings {
    OperatorBindings::new([(OperatorBinding::new("fonts").unwrap(), service.operator())]).unwrap()
}

fn assert_send<T: Send>(_: T) {}

struct CountingResolver {
    calls: AtomicUsize,
    operator: opendal::Operator,
}

impl CountingResolver {
    fn new(operator: opendal::Operator) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            operator,
        }
    }
}

impl OperatorResolver for CountingResolver {
    type Error = Infallible;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.operator.clone())
    }
}
