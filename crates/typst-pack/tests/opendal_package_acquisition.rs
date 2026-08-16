#![cfg(feature = "opendal")]

#[allow(dead_code, clippy::collapsible_if)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::convert::Infallible;
use std::error::Error as _;
use std::fmt;
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use scripted_opendal::{
    Capabilities, ListEntry, ListScript, ListStep, OperationLogEntry, ReadScript, ReadStep,
    ScriptedService,
};
use typst_pack::PackageAcquisitionFailureReason;
#[cfg(feature = "package-acquisition")]
use typst_pack::opendal::pack_assembly::{
    AcquiredPackageInsertionErrorCause, AcquiredPackageInsertionTarget, insert_acquired_package,
};
use typst_pack::opendal::pack_assembly::{
    PackageAcquisition, PackageAcquisitionCeilings, PackageAcquisitionErrorCause,
    PackageAcquisitionLimits, PackageAcquisitionRequest, PackageAcquisitionRequestIssue,
    PackageArchiveAcquisitionCeilings, PackageArchiveAcquisitionLimitError,
    PackageArchiveAcquisitionLimitsError, PackageArchiveAcquisitionResource,
    PackageTreeAcquisitionCeilings, PackageTreeSource, acquire_package,
};
use typst_pack::opendal::{
    Location, LocationRoleError, OperatorBinding, OperatorBindings, OperatorBindingsResolveError,
    OperatorResolver,
};
#[cfg(feature = "package-acquisition")]
use typst_pack::{
    PackageAcquisitionFailure, PackageAcquisitionFailures, PackageCatalog, PackageDisposition,
    PackageExpansionLimits, PackageTree,
};

#[test]
fn absent_tree_falls_through_to_exact_cached_archive() {
    let service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new("trees/preview/example/1.2.3/", 0, []).unwrap()],
        [ReadScript::new(
            "cache/preview/example/1.2.3.tar.gz",
            1,
            [ReadStep::chunk(b"raw cached archive")],
        )
        .unwrap()],
        8,
    );
    let binding = OperatorBinding::new("packages").unwrap();
    let resolver = CountingResolver {
        calls: AtomicUsize::new(0),
        operator: service.operator(),
    };
    let tree_source =
        PackageTreeSource::new(Location::from_operation_path(binding.clone(), "trees/").unwrap());
    let cache = Location::from_operation_path(binding, "cache/").unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [tree_source.clone()],
        Some(cache.clone()),
        Some("registry:/packages/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    assert_eq!(request.spec().to_string(), "@preview/example:1.2.3");
    assert_eq!(request.tree_sources(), &[tree_source]);
    assert_eq!(request.archive_cache(), Some(&cache));
    assert_eq!(
        request.registry().unwrap().to_string(),
        "registry:/packages/"
    );
    assert_eq!(request.limits(), PackageAcquisitionLimits::reference_v1());

    let acquisition = expect_ready(pin!(acquire_package(&resolver, &request))).unwrap();
    let PackageAcquisition::CachedArchive(archive) = acquisition else {
        panic!("expected cached archive");
    };

    assert_eq!(archive.configured_source(), &cache);
    assert_eq!(
        archive.candidate_location().operation_path(),
        "cache/preview/example/1.2.3.tar.gz"
    );
    assert_eq!(archive.bytes(), b"raw cached archive");
    assert_eq!(resolver.calls.load(Ordering::Relaxed), 1);
    let (spec, configured, candidate, bytes) = archive.into_parts();
    assert_eq!(spec.to_string(), "@preview/example:1.2.3");
    assert_eq!(configured, cache);
    assert_eq!(
        candidate.operation_path(),
        "cache/preview/example/1.2.3.tar.gz"
    );
    assert_eq!(bytes, b"raw cached archive");
    assert_eq!(
        service
            .log()
            .entries()
            .iter()
            .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
            .count(),
        1
    );
}

#[cfg(feature = "package-acquisition")]
#[test]
fn acquired_tree_is_inserted_and_clears_an_older_failure() {
    let candidate = "trees/preview/example/1.2.3/";
    let service = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            candidate,
            2,
            [ListStep::page([
                ListEntry::file(format!("{candidate}lib.typ")),
                ListEntry::file(format!("{candidate}typst.toml")),
            ])],
        )
        .unwrap()],
        [
            ReadScript::new(
                format!("{candidate}lib.typ"),
                1,
                [ReadStep::chunk(b"package library")],
            )
            .unwrap(),
            ReadScript::new(
                format!("{candidate}typst.toml"),
                1,
                [ReadStep::chunk(
                    b"[package]\nname = \"example\"\nversion = \"1.2.3\"\n",
                )],
            )
            .unwrap(),
        ],
        12,
    );
    let resolver = CountingResolver {
        calls: AtomicUsize::new(0),
        operator: service.operator(),
    };
    let spec: typst::syntax::package::PackageSpec = "@preview/example:1.2.3".parse().unwrap();
    let request = PackageAcquisitionRequest::new(
        spec.clone(),
        [PackageTreeSource::new("packages:/trees/".parse().unwrap())],
        None,
        None,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    let acquisition = expect_ready(pin!(acquire_package(&resolver, &request))).unwrap();
    let PackageAcquisition::Tree(tree) = &acquisition else {
        panic!("expected Package Tree");
    };
    assert_eq!(tree.spec(), &spec);
    assert_eq!(tree.source_index(), 0);
    assert_eq!(tree.configured_source().to_string(), "packages:/trees/");
    assert_eq!(
        tree.entries()
            .iter()
            .map(|entry| entry.relative_path())
            .collect::<Vec<_>>(),
        ["lib.typ", "typst.toml"]
    );
    let mut catalog = PackageCatalog::new();
    let mut failures = PackageAcquisitionFailures::new();
    failures.insert(PackageAcquisitionFailure::new(
        spec.clone(),
        PackageAcquisitionFailureReason::NotFound,
    ));

    let residue = insert_acquired_package(
        &mut catalog,
        &mut failures,
        acquisition,
        PackageDisposition::Embedded,
        PackageExpansionLimits::reference_v1(),
    )
    .unwrap();

    assert!(residue.is_none());
    assert_eq!(
        catalog.get(&spec).unwrap().tree().file("lib.typ"),
        Some(b"package library".as_slice())
    );
    assert!(failures.get(&spec).is_none());
}

#[test]
fn request_aggregates_invalid_roles_and_limits_keep_the_reference_profile() {
    let spec = "@preview/example:1.2.3".parse().unwrap();
    let rejection = PackageAcquisitionRequest::new(
        spec,
        [PackageTreeSource::new("tree:/exact".parse().unwrap())],
        Some("cache:/exact".parse().unwrap()),
        Some("registry:/exact".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap_err();

    assert_eq!(rejection.issues().len(), 3);
    assert!(matches!(
        &rejection.issues()[0],
        PackageAcquisitionRequestIssue::InvalidTreeSourceRole {
            source_index: 0,
            source: LocationRoleError::PrefixMissingTrailingSlash,
            ..
        }
    ));
    assert!(matches!(
        &rejection.issues()[1],
        PackageAcquisitionRequestIssue::InvalidArchiveCacheRole {
            source: LocationRoleError::PrefixMissingTrailingSlash,
            ..
        }
    ));
    assert!(matches!(
        &rejection.issues()[2],
        PackageAcquisitionRequestIssue::InvalidRegistryRole {
            source: LocationRoleError::PrefixMissingTrailingSlash,
            ..
        }
    ));

    let reference = PackageAcquisitionCeilings::reference_v1();
    assert_eq!(reference.trees.selected_files, 50_000);
    assert_eq!(reference.trees.total_bytes, 512 * 1024 * 1024);
    assert_eq!(reference.archives.archive_bytes, 128 * 1024 * 1024);
    assert!(matches!(
        PackageAcquisitionLimits::new(PackageAcquisitionCeilings {
            archives: PackageArchiveAcquisitionCeilings {
                archive_bytes: u64::MAX,
            },
            ..reference
        }),
        Err(
            typst_pack::opendal::pack_assembly::PackageAcquisitionLimitsError::Archives(
                PackageArchiveAcquisitionLimitsError::CannotProbe {
                    resource: PackageArchiveAcquisitionResource::ArchiveBytes,
                    ceiling: u64::MAX,
                }
            )
        )
    ));
}

#[test]
fn registry_success_preserves_raw_bytes_and_derives_the_cache_destination() {
    let raw = b"raw registry archive";
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(
            "registry/preview/example-1.2.3.tar.gz",
            1,
            [ReadStep::chunk(raw)],
        )
        .unwrap()],
        8,
    );
    let bindings = OperatorBindings::new([
        (OperatorBinding::new("cache").unwrap(), service.operator()),
        (
            OperatorBinding::new("registry").unwrap(),
            service.operator(),
        ),
    ])
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("cache:/archives/".parse().unwrap()),
        Some("registry:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let acquisition = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap();
    assert_eq!(
        acquisition.configured_source().unwrap().to_string(),
        "registry:/registry/"
    );
    let PackageAcquisition::RegistryArchive(archive) = acquisition else {
        panic!("expected registry archive");
    };
    assert_eq!(archive.bytes(), raw);
    assert_eq!(archive.len(), raw.len() as u64);
    assert!(!archive.is_empty());
    assert_eq!(
        archive.cache_destination().unwrap().to_string(),
        "cache:/archives/preview/example/1.2.3.tar.gz"
    );
    assert!(!format!("{archive:?}").contains("raw registry archive"));
}

#[test]
fn unserved_registry_namespace_is_skipped_and_exhaustion_is_not_found() {
    let resolver = RejectingResolver {
        calls: AtomicUsize::new(0),
    };
    let spec: typst::syntax::package::PackageSpec = "@local/example:1.2.3".parse().unwrap();
    let request = PackageAcquisitionRequest::new(
        spec.clone(),
        [],
        None,
        Some("registry:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let acquisition = expect_ready(pin!(acquire_package(&resolver, &request))).unwrap();

    assert_eq!(resolver.calls.load(Ordering::Relaxed), 0);
    assert!(acquisition.configured_source().is_none());
    let PackageAcquisition::Unavailable(unavailable) = acquisition else {
        panic!("expected unavailable acquisition");
    };
    assert_eq!(unavailable.spec(), &spec);
    assert_eq!(
        unavailable.reason(),
        &PackageAcquisitionFailureReason::NotFound
    );
}

#[test]
fn non_not_found_registry_errors_are_terminal_other_failures() {
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(
            "registry/preview/example-1.2.3.tar.gz",
            0,
            [ReadStep::failure(opendal::ErrorKind::PermissionDenied)],
        )
        .unwrap()],
        4,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("registry").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        None,
        Some("registry:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let error = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap_err();

    assert!(matches!(
        error.cause(),
        PackageAcquisitionErrorCause::RegistryRead(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));
    assert_eq!(
        error.reason(),
        &PackageAcquisitionFailureReason::Other { detail: None }
    );
    assert_eq!(error.spec(), request.spec());
    assert_eq!(error.source_index(), None);
    assert_eq!(
        error.configured_source().unwrap().to_string(),
        "registry:/registry/"
    );
    assert_eq!(
        error.candidate_location().unwrap().operation_path(),
        "registry/preview/example-1.2.3.tar.gz"
    );
    assert!(error.failed_path().is_none());
    assert_eq!(error.failure().reason(), error.reason());
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<opendal::Error>()
            .unwrap()
            .kind(),
        opendal::ErrorKind::PermissionDenied
    );
}

#[test]
fn resolver_and_archive_capability_failures_remain_typed() {
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("packages:/cache/".parse().unwrap()),
        None,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    let resolver = RejectingResolver {
        calls: AtomicUsize::new(0),
    };
    let resolve_error = expect_ready(pin!(acquire_package(&resolver, &request))).unwrap_err();
    assert!(matches!(
        resolve_error.cause(),
        PackageAcquisitionErrorCause::ResolveOperator(ResolverFailure)
    ));
    assert!(resolve_error.source().unwrap().is::<ResolverFailure>());

    let service = ScriptedService::new(
        Capabilities {
            list: true,
            list_with_recursive: true,
            read: false,
        },
        [],
        [],
        2,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let capability_error = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap_err();
    assert!(matches!(
        capability_error.cause(),
        PackageAcquisitionErrorCause::UnsupportedArchiveRead
    ));
    assert!(capability_error.source().is_none());
}

#[test]
fn public_tree_errors_preserve_every_typed_cause_family() {
    let unsupported = ScriptedService::new(
        Capabilities {
            list: false,
            list_with_recursive: true,
            read: true,
        },
        [],
        [],
        2,
    );
    assert!(matches!(
        tree_error(&unsupported, PackageAcquisitionLimits::reference_v1()).cause(),
        PackageAcquisitionErrorCause::UnsupportedTreeCapabilities { list: false, .. }
    ));

    let list_failure = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            "trees/preview/example/1.2.3/",
            0,
            [ListStep::failure(opendal::ErrorKind::PermissionDenied)],
        )
        .unwrap()],
        [],
        2,
    );
    assert!(matches!(
        tree_error(&list_failure, PackageAcquisitionLimits::reference_v1()).cause(),
        PackageAcquisitionErrorCause::TreeList(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let candidate = "trees/preview/example/1.2.3/";
    let object = format!("{candidate}lib.typ");
    let read_failure = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])]).unwrap()],
        [ReadScript::new(
            &object,
            0,
            [ReadStep::failure(opendal::ErrorKind::PermissionDenied)],
        )
        .unwrap()],
        4,
    );
    assert!(matches!(
        tree_error(&read_failure, PackageAcquisitionLimits::reference_v1()).cause(),
        PackageAcquisitionErrorCause::TreeRead(source)
            if source.kind() == opendal::ErrorKind::PermissionDenied
    ));

    let disappeared = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])]).unwrap()],
        [],
        4,
    );
    let disappeared = tree_error(&disappeared, PackageAcquisitionLimits::reference_v1());
    assert_eq!(disappeared.failed_path(), Some(object.as_str()));
    assert!(matches!(
        disappeared.cause(),
        PackageAcquisitionErrorCause::ListedTreeObjectAbsent(source)
            if source.kind() == opendal::ErrorKind::NotFound
    ));

    let structural = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            candidate,
            1,
            [ListStep::page([ListEntry::unknown(&object)])],
        )
        .unwrap()],
        [],
        4,
    );
    assert!(matches!(
        tree_error(&structural, PackageAcquisitionLimits::reference_v1()).cause(),
        PackageAcquisitionErrorCause::TreeStructural(_)
    ));

    let conflict = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(
            candidate,
            2,
            [ListStep::page([
                ListEntry::file(format!("{candidate}assets")),
                ListEntry::file(format!("{candidate}assets/logo.svg")),
            ])],
        )
        .unwrap()],
        [],
        4,
    );
    assert!(matches!(
        tree_error(&conflict, PackageAcquisitionLimits::reference_v1()).cause(),
        PackageAcquisitionErrorCause::InvalidPackageTree(_)
    ));

    let limited = ScriptedService::new(
        Capabilities::all(),
        [ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])]).unwrap()],
        [],
        4,
    );
    let limits = PackageAcquisitionLimits::new(PackageAcquisitionCeilings {
        trees: PackageTreeAcquisitionCeilings {
            listed_entries: 0,
            ..PackageTreeAcquisitionCeilings::reference_v1()
        },
        ..PackageAcquisitionCeilings::reference_v1()
    })
    .unwrap();
    assert!(matches!(
        tree_error(&limited, limits).cause(),
        PackageAcquisitionErrorCause::TreeLimit(_)
    ));
}

#[test]
fn present_oversized_cache_is_terminal_before_registry() {
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(
            "cache/preview/example/1.2.3.tar.gz",
            1,
            [ReadStep::chunk(b"12345")],
        )
        .unwrap()],
        4,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let limits = PackageAcquisitionLimits::new(PackageAcquisitionCeilings {
        archives: PackageArchiveAcquisitionCeilings { archive_bytes: 4 },
        ..PackageAcquisitionCeilings::reference_v1()
    })
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("packages:/cache/".parse().unwrap()),
        Some("unreached:/registry/".parse().unwrap()),
        limits,
    )
    .unwrap();

    let error = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap_err();

    assert!(matches!(
        error.cause(),
        PackageAcquisitionErrorCause::ArchiveLimit(PackageArchiveAcquisitionLimitError::Exceeded {
            resource: PackageArchiveAcquisitionResource::ArchiveBytes,
            ceiling: 4,
            observed_at_least: 5,
        })
    ));
}

#[test]
fn cache_disappearance_after_yielding_bytes_is_terminal() {
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(
            "cache/preview/example/1.2.3.tar.gz",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::failure(opendal::ErrorKind::NotFound),
            ],
        )
        .unwrap()],
        8,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("packages:/cache/".parse().unwrap()),
        Some("packages:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let error = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap_err();

    assert!(matches!(
        error.cause(),
        PackageAcquisitionErrorCause::CacheRead(source)
            if source.kind() == opendal::ErrorKind::NotFound
    ));
}

#[test]
fn empty_tree_survey_does_not_require_payload_read_capability() {
    let service = ScriptedService::new(
        Capabilities {
            list: true,
            list_with_recursive: true,
            read: false,
        },
        [ListScript::new("trees/preview/example/1.2.3/", 0, []).unwrap()],
        [],
        4,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [PackageTreeSource::new("packages:/trees/".parse().unwrap())],
        None,
        None,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    let acquisition = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap();

    assert!(matches!(acquisition, PackageAcquisition::Unavailable(_)));
    assert!(matches!(
        service.log().entries(),
        [
            OperationLogEntry::ListInvoked { .. },
            OperationLogEntry::ListCompleted { .. }
        ]
    ));
}

#[test]
fn operator_bindings_produce_a_send_package_acquisition_future() {
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let bindings =
        OperatorBindings::new([(OperatorBinding::new("packages").unwrap(), operator)]).unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        Some("packages:/cache/".parse().unwrap()),
        None,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();

    assert_send(acquire_package(&bindings, &request));
}

#[cfg(feature = "package-acquisition")]
#[test]
fn registry_archive_is_validated_before_exact_bytes_are_returned_for_publication() {
    let archive = package_archive();
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(
            "registry/preview/example-1.2.3.tar.gz",
            1,
            [ReadStep::chunk(&archive)],
        )
        .unwrap()],
        8,
    );
    let bindings = OperatorBindings::new([
        (OperatorBinding::new("cache").unwrap(), service.operator()),
        (
            OperatorBinding::new("registry").unwrap(),
            service.operator(),
        ),
    ])
    .unwrap();
    let spec: typst::syntax::package::PackageSpec = "@preview/example:1.2.3".parse().unwrap();
    let request = PackageAcquisitionRequest::new(
        spec.clone(),
        [],
        Some("cache:/archives/".parse().unwrap()),
        Some("registry:/registry/".parse().unwrap()),
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    let acquisition = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap();
    let mut catalog = PackageCatalog::new();
    let mut failures = PackageAcquisitionFailures::new();

    let residue = insert_acquired_package(
        &mut catalog,
        &mut failures,
        acquisition,
        PackageDisposition::External,
        PackageExpansionLimits::reference_v1(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(residue.spec(), &spec);
    assert_eq!(
        residue.destination().to_string(),
        "cache:/archives/preview/example/1.2.3.tar.gz"
    );
    assert_eq!(residue.bytes(), archive);
    assert_eq!(
        catalog.get(&spec).unwrap().tree().file("lib.typ"),
        Some(b"package library".as_slice())
    );
}

#[cfg(feature = "package-acquisition")]
#[test]
fn malformed_cache_and_registry_archives_map_to_the_same_stable_failure() {
    for (cache, registry, expected_target) in [
        (
            Some("packages:/cache/"),
            None,
            AcquiredPackageInsertionTarget::CachedArchive,
        ),
        (
            None,
            Some("packages:/registry/"),
            AcquiredPackageInsertionTarget::RegistryArchive,
        ),
    ] {
        let path = if cache.is_some() {
            "cache/preview/example/1.2.3.tar.gz"
        } else {
            "registry/preview/example-1.2.3.tar.gz"
        };
        let service = ScriptedService::new(
            Capabilities::all(),
            [],
            [ReadScript::new(path, 1, [ReadStep::chunk(b"malformed")]).unwrap()],
            4,
        );
        let bindings = OperatorBindings::new([(
            OperatorBinding::new("packages").unwrap(),
            service.operator(),
        )])
        .unwrap();
        let spec: typst::syntax::package::PackageSpec = "@preview/example:1.2.3".parse().unwrap();
        let request = PackageAcquisitionRequest::new(
            spec.clone(),
            [],
            cache.map(|value| value.parse().unwrap()),
            registry.map(|value| value.parse().unwrap()),
            PackageAcquisitionLimits::reference_v1(),
        )
        .unwrap();
        let acquisition = expect_ready(pin!(acquire_package(&bindings, &request))).unwrap();
        let mut catalog = PackageCatalog::new();
        let mut failures = PackageAcquisitionFailures::new();

        let error = insert_acquired_package(
            &mut catalog,
            &mut failures,
            acquisition,
            PackageDisposition::Embedded,
            PackageExpansionLimits::reference_v1(),
        )
        .unwrap_err();

        assert_eq!(error.target(), &expected_target);
        assert_eq!(
            error.reason(),
            &PackageAcquisitionFailureReason::MalformedArchive { detail: None }
        );
        assert!(matches!(
            error.cause(),
            AcquiredPackageInsertionErrorCause::ArchiveExpansion(_)
        ));
        assert_eq!(failures.get(&spec), Some(error.failure()));
        assert!(catalog.get(&spec).is_none());
    }
}

#[cfg(feature = "package-acquisition")]
#[test]
fn unavailable_expansion_limit_and_catalog_failures_update_the_failure_map() {
    let spec: typst::syntax::package::PackageSpec = "@preview/example:1.2.3".parse().unwrap();

    let resolver = RejectingResolver {
        calls: AtomicUsize::new(0),
    };
    let unavailable_request = PackageAcquisitionRequest::new(
        spec.clone(),
        [],
        None,
        None,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    let unavailable = expect_ready(pin!(acquire_package(&resolver, &unavailable_request))).unwrap();
    let mut catalog = PackageCatalog::new();
    let mut failures = PackageAcquisitionFailures::new();
    assert!(
        insert_acquired_package(
            &mut catalog,
            &mut failures,
            unavailable,
            PackageDisposition::Embedded,
            PackageExpansionLimits::reference_v1(),
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        failures.get(&spec).unwrap().reason(),
        &PackageAcquisitionFailureReason::NotFound
    );

    let expansion_acquisition = acquire_raw_archive(&package_archive(), false);
    let reference = PackageExpansionLimits::reference_v1();
    let expansion_limits = PackageExpansionLimits::new(
        0,
        reference.members(),
        reference.member_name_bytes(),
        reference.member_bytes(),
        reference.total_expanded_bytes(),
    )
    .unwrap();
    let expansion_error = insert_acquired_package(
        &mut PackageCatalog::new(),
        &mut PackageAcquisitionFailures::new(),
        expansion_acquisition,
        PackageDisposition::Embedded,
        expansion_limits,
    )
    .unwrap_err();
    assert_eq!(
        expansion_error.target(),
        &AcquiredPackageInsertionTarget::CachedArchive
    );
    assert_eq!(
        expansion_error.reason(),
        &PackageAcquisitionFailureReason::Other { detail: None }
    );

    let catalog_acquisition = acquire_raw_archive(&package_archive(), true);
    let mut catalog = PackageCatalog::new();
    catalog
        .insert(
            spec.clone(),
            PackageTree::copy_from_entries([
                ("lib.typ", b"existing".as_slice()),
                (
                    "typst.toml",
                    b"[package]\nname = \"example\"\nversion = \"1.2.3\"\n".as_slice(),
                ),
            ])
            .unwrap(),
            PackageDisposition::Embedded,
        )
        .unwrap();
    let mut failures = PackageAcquisitionFailures::new();
    let catalog_error = insert_acquired_package(
        &mut catalog,
        &mut failures,
        catalog_acquisition,
        PackageDisposition::External,
        PackageExpansionLimits::reference_v1(),
    )
    .unwrap_err();
    assert_eq!(
        catalog_error.target(),
        &AcquiredPackageInsertionTarget::PackageCatalog
    );
    assert_eq!(
        catalog_error.reason(),
        &PackageAcquisitionFailureReason::Other { detail: None }
    );
    assert!(matches!(
        catalog_error.cause(),
        AcquiredPackageInsertionErrorCause::PackageCatalog(_)
    ));
    assert_eq!(failures.get(&spec), Some(catalog_error.failure()));
}

struct CountingResolver {
    calls: AtomicUsize,
    operator: opendal::Operator,
}

impl OperatorResolver for CountingResolver {
    type Error = Infallible;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.operator.clone())
    }
}

struct RejectingResolver {
    calls: AtomicUsize,
}

impl OperatorResolver for RejectingResolver {
    type Error = ResolverFailure;

    fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(ResolverFailure)
    }
}

#[derive(Debug)]
struct ResolverFailure;

impl fmt::Display for ResolverFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resolver failed")
    }
}

impl std::error::Error for ResolverFailure {}

#[cfg(feature = "package-acquisition")]
fn package_archive() -> Vec<u8> {
    use std::io::Write as _;

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

#[cfg(feature = "package-acquisition")]
fn acquire_raw_archive(bytes: &[u8], registry: bool) -> PackageAcquisition {
    let path = if registry {
        "registry/preview/example-1.2.3.tar.gz"
    } else {
        "cache/preview/example/1.2.3.tar.gz"
    };
    let service = ScriptedService::new(
        Capabilities::all(),
        [],
        [ReadScript::new(path, 1, [ReadStep::chunk(bytes)]).unwrap()],
        4,
    );
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let (cache, registry_source) = if registry {
        (None, Some("packages:/registry/".parse().unwrap()))
    } else {
        (Some("packages:/cache/".parse().unwrap()), None)
    };
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [],
        cache,
        registry_source,
        PackageAcquisitionLimits::reference_v1(),
    )
    .unwrap();
    expect_ready(pin!(acquire_package(&bindings, &request))).unwrap()
}

fn assert_send<T: Send>(_: T) {}

fn tree_error(
    service: &ScriptedService,
    limits: PackageAcquisitionLimits,
) -> typst_pack::opendal::pack_assembly::PackageAcquisitionError<OperatorBindingsResolveError> {
    let bindings = OperatorBindings::new([(
        OperatorBinding::new("packages").unwrap(),
        service.operator(),
    )])
    .unwrap();
    let request = PackageAcquisitionRequest::new(
        "@preview/example:1.2.3".parse().unwrap(),
        [PackageTreeSource::new("packages:/trees/".parse().unwrap())],
        None,
        None,
        limits,
    )
    .unwrap();
    expect_ready(pin!(acquire_package(&bindings, &request))).unwrap_err()
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
