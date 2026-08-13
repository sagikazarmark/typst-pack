#![cfg(feature = "opendal")]

#[path = "support/opendal_scheduling.rs"]
mod scheduling;
#[allow(dead_code)]
#[path = "support/opendal.rs"]
mod scripted_opendal;

use std::future::Future;
use std::num::NonZeroUsize;
use std::task::{Context, Poll, Waker};

use typst::syntax::package::PackageSpec;
use typst_pack::FontContainerIdentity;

use scheduling::{
    FailureCause, LimitResource, OpenDalAllocation, Role, RoleBudget, SchedulingEvent,
    SchedulingHarness, Target,
};
use scripted_opendal::{
    Capabilities, OperationControls, OperationLogEntry, ReadScript, ReadStep, ScriptedService,
};

#[test]
fn indexed_opendal_reads_drive_harness_completion_and_accounting() {
    let controls = OperationControls::new();
    let first = controls.hold_read(0);
    let second = controls.hold_read(1);
    let service = ScriptedService::new_controlled(
        Capabilities::all(),
        [],
        [
            ReadScript::new("a.typ", 1, [ReadStep::chunk(b"aaa")]).unwrap(),
            ReadScript::new("b.typ", 1, [ReadStep::chunk(b"bbbb")]).unwrap(),
        ],
        controls,
        16,
    );
    let mut harness = SchedulingHarness::new(
        [RoleBudget::new(Role::PackOverride, 8, 4)],
        NonZeroUsize::new(2).unwrap(),
        [
            Target::pack_override("overrides-b", "b.typ"),
            Target::pack_override("overrides-a", "a.typ"),
        ],
    );
    harness.start();
    let operator = service.operator();
    let mut first_read = Box::pin(operator.read("a.typ"));
    let mut second_read = Box::pin(operator.read("b.typ"));
    assert!(matches!(poll_once(first_read.as_mut()), Poll::Pending));
    assert!(matches!(poll_once(second_read.as_mut()), Poll::Pending));

    second.release();
    let second_bytes = expect_ready(second_read.as_mut()).unwrap();
    harness.observe_payload(1, second_bytes.len() as u64);
    harness.complete_success(1);
    first.release();
    let first_bytes = expect_ready(first_read.as_mut()).unwrap();
    harness.observe_payload(0, first_bytes.len() as u64);
    harness.complete_success(0);

    assert!(harness.is_complete());
    assert_eq!(harness.accounting(Role::PackOverride).retained_success, 7);
    assert_eq!(
        harness.accounting(Role::PackOverride).peak_retained_payload,
        7
    );
    let completed = service
        .log()
        .entries()
        .iter()
        .filter_map(|event| match event {
            OperationLogEntry::ReadCompleted { id } => Some(*id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(completed, [1, 0]);
    harness.assert_payload_bounds();
}

#[test]
fn reservation_blocking_and_terminal_exhaustion_are_completion_order_independent() {
    let expected_failure = FailureCause::Exceeded {
        resource: LimitResource::TotalBytes,
        ceiling: 10,
        observed_at_least: 11,
    };
    for completion_order in [[0, 1], [1, 0]] {
        let mut harness = SchedulingHarness::new(
            [RoleBudget::new(Role::PackOverride, 10, 4)],
            NonZeroUsize::new(3).unwrap(),
            [
                Target::pack_override("alpha", "c.typ"),
                Target::pack_override("alpha", "a.typ"),
                Target::pack_override("alpha", "b.typ"),
            ],
        );

        harness.start();
        assert_eq!(harness.in_flight(), [0, 1]);
        assert!(
            harness
                .log()
                .contains(&SchedulingEvent::ReservationBlocked {
                    target: 2,
                    role: Role::PackOverride,
                    required: 4,
                    remaining: 2,
                })
        );

        for completed in completion_order {
            let bytes = if completed == 0 { 3 } else { 4 };
            harness.observe_payload(completed, bytes);
            harness.complete_success(completed);
        }

        let failure = harness.failure().expect("the role budget is exhausted");
        assert_eq!(failure.target, 2);
        assert_eq!(failure.role, Role::PackOverride);
        assert_eq!(failure.cause, expected_failure);
        let accounting = harness.accounting(Role::PackOverride);
        assert_eq!(accounting.retained_success, 7);
        assert_eq!(accounting.peak_retained_payload, 7);
        assert!(
            harness
                .log()
                .contains(&SchedulingEvent::ReservationRefunded {
                    target: 0,
                    role: Role::PackOverride,
                    bytes: 1,
                })
        );
        harness.assert_payload_bounds();
    }
}

#[test]
fn a_blocked_reservation_launches_after_a_short_read_refund() {
    let mut harness = SchedulingHarness::new(
        [RoleBudget::new(Role::PackOverride, 10, 4)],
        NonZeroUsize::new(3).unwrap(),
        [
            Target::pack_override("alpha", "c.typ"),
            Target::pack_override("alpha", "a.typ"),
            Target::pack_override("alpha", "b.typ"),
        ],
    );

    harness.start();
    harness.observe_payload(0, 1);
    harness.complete_success(0);

    assert_eq!(harness.in_flight(), [1, 2]);
    assert!(harness.log().contains(&SchedulingEvent::Launched {
        target: 2,
        role: Role::PackOverride,
        binding: "alpha".to_owned(),
        key: "c.typ".to_owned(),
    }));
    assert!(
        harness
            .log()
            .contains(&SchedulingEvent::ReservationRefunded {
                target: 0,
                role: Role::PackOverride,
                bytes: 3,
            })
    );
}

#[test]
fn exact_total_aggregate_exhaustion_and_probe_plus_one_stop_later_launches() {
    let mut exact = SchedulingHarness::new(
        [RoleBudget::new(Role::Font, 8, 4)],
        NonZeroUsize::new(2).unwrap(),
        [
            Target::font("fonts", FontContainerIdentity::from_bytes(b"font-b")),
            Target::font("fonts", FontContainerIdentity::from_bytes(b"font-a")),
        ],
    );
    exact.start();
    exact.observe_payload(1, 4);
    exact.complete_success(1);
    exact.observe_payload(0, 4);
    exact.complete_success(0);

    assert!(exact.is_complete());
    let accounting = exact.accounting(Role::Font);
    assert_eq!(accounting.retained_success, 8);
    assert_eq!(accounting.peak_retained_payload, 8);
    exact.assert_payload_bounds();

    let mut plus_one = SchedulingHarness::new(
        [RoleBudget::new(Role::Package, 4, 4)],
        NonZeroUsize::new(1).unwrap(),
        [
            Target::package("packages", package("@preview/b:1.0.0")),
            Target::package("packages", package("@preview/a:1.0.0")),
        ],
    );
    plus_one.start();
    plus_one.observe_payload(0, 4);
    plus_one.complete_success(0);

    assert_eq!(plus_one.failure().unwrap().target, 1);
    assert_eq!(
        plus_one.failure().unwrap().cause,
        FailureCause::Exceeded {
            resource: LimitResource::TotalBytes,
            ceiling: 4,
            observed_at_least: 5,
        }
    );
    assert!(
        !plus_one
            .log()
            .iter()
            .any(|event| matches!(event, SchedulingEvent::Launched { target: 1, .. }))
    );
    plus_one.assert_payload_bounds();

    let mut probe = SchedulingHarness::new(
        [RoleBudget::new(Role::Font, 8, 4)],
        NonZeroUsize::new(1).unwrap(),
        [
            Target::font("fonts", FontContainerIdentity::from_bytes(b"font-a")),
            Target::font("fonts", FontContainerIdentity::from_bytes(b"font-b")),
        ],
    );
    probe.start();
    probe.observe_payload(0, 5);
    probe.fail_probe_overage(0);

    assert_eq!(
        probe.failure().unwrap().cause,
        FailureCause::Exceeded {
            resource: LimitResource::ObjectBytes,
            ceiling: 4,
            observed_at_least: 5,
        }
    );
    assert!(probe.log().contains(&SchedulingEvent::ProbeCharged {
        target: 0,
        role: Role::Font,
        bytes: 1,
    }));
    assert!(probe.log().contains(&SchedulingEvent::ProbeReleased {
        target: 0,
        role: Role::Font,
        bytes: 1,
    }));
    assert!(
        !probe
            .log()
            .iter()
            .any(|event| matches!(event, SchedulingEvent::Launched { target: 1, .. }))
    );
    assert_eq!(probe.accounting(Role::Font).peak_retained_payload, 5);
    probe.assert_payload_bounds();
}

#[test]
fn delayed_canonical_failure_wins_after_mixed_later_payloads_complete() {
    for later_order in permutations([1, 2, 3]) {
        let mut harness = mixed_harness(4);
        harness.start();
        assert_eq!(harness.in_flight(), [0, 1, 2, 3]);

        for target in later_order {
            harness.observe_payload(target, 2);
            harness.complete_success(target);
        }
        assert!(harness.failure().is_none());

        harness.complete_failure(0, FailureCause::Read);

        assert_eq!(harness.failure().unwrap().target, 0);
        assert_eq!(harness.failure().unwrap().cause, FailureCause::Read);
        assert_eq!(harness.accounting(Role::PackOverride).retained_success, 2);
        assert_eq!(harness.accounting(Role::Package).retained_success, 2);
        assert_eq!(harness.accounting(Role::Font).retained_success, 2);
        assert_eq!(
            harness.accounting(Role::PackOverride).peak_retained_payload,
            2
        );
        harness.assert_payload_bounds();
    }
}

#[test]
fn mixed_roles_and_bindings_launch_canonically_during_slot_replenishment() {
    for completion_order in [
        [0, 1, 2, 3],
        [0, 1, 3, 2],
        [0, 2, 1, 3],
        [0, 2, 3, 1],
        [1, 0, 2, 3],
        [1, 0, 3, 2],
        [1, 2, 0, 3],
        [1, 2, 3, 0],
    ] {
        let mut harness = mixed_harness(2);
        harness.start();
        assert_eq!(harness.in_flight(), [0, 1]);

        for target in completion_order {
            assert!(harness.in_flight().contains(&target));
            harness.observe_payload(target, 1);
            harness.complete_success(target);
        }

        let launched = harness
            .log()
            .iter()
            .filter_map(|event| match event {
                SchedulingEvent::Launched {
                    role, binding, key, ..
                } => Some((*role, binding.as_str(), key.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            launched,
            [
                (Role::PackOverride, "overrides-a", "a.typ"),
                (Role::PackOverride, "overrides-b", "z.typ"),
                (Role::Package, "packages", "@preview/pkg:1.0.0"),
                (
                    Role::Font,
                    "fonts",
                    font_identity_key(b"font-container").as_str(),
                ),
            ]
        );
    }
}

#[test]
fn later_failure_waits_for_an_earlier_target_then_drops_later_work() {
    let mut harness = mixed_harness(3);
    harness.start();
    harness.complete_failure(1, FailureCause::Read);

    assert!(harness.failure().is_none());
    assert_eq!(harness.in_flight(), [0]);
    assert!(matches!(
        harness.log(),
        [
            ..,
            SchedulingEvent::ReservationReleased { target: 2, .. },
            SchedulingEvent::Dropped { target: 2, .. },
            SchedulingEvent::Cancelled { target: 2, .. }
        ]
    ));

    harness.complete_failure(0, FailureCause::Read);

    assert_eq!(harness.failure().unwrap().target, 0);
    assert!(
        !harness
            .log()
            .iter()
            .any(|event| matches!(event, SchedulingEvent::Launched { target: 3, .. }))
    );
}

#[test]
fn cancellation_releases_reservations_retained_payload_and_probes_in_canonical_order() {
    let mut harness = mixed_harness(4);
    harness.start();
    harness.observe_payload(0, 5);
    assert_eq!(
        harness.accounting(Role::PackOverride).peak_retained_payload,
        5
    );
    harness.cancel();

    let cancelled = harness
        .log()
        .iter()
        .filter_map(|event| match event {
            SchedulingEvent::Cancelled { target, .. } => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(cancelled, [0, 1, 2, 3]);
    assert!(harness.log().contains(&SchedulingEvent::ProbeReleased {
        target: 0,
        role: Role::PackOverride,
        bytes: 1,
    }));
    for role in [Role::PackOverride, Role::Package, Role::Font] {
        let accounting = harness.accounting(role);
        assert_eq!(accounting.reserved_in_flight, 0);
        assert_eq!(accounting.retained_in_flight, 0);
        assert_eq!(accounting.probe_bytes, 0);
    }
    harness.assert_payload_bounds();
}

#[test]
fn payload_scope_explicitly_excludes_opendal_owned_allocations() {
    let harness = mixed_harness(4);

    assert_eq!(
        harness.excluded_opendal_allocations(),
        [
            OpenDalAllocation::Service,
            OpenDalAllocation::Transport,
            OpenDalAllocation::YieldedBuffer,
        ]
    );
}

fn mixed_harness(max_in_flight: usize) -> SchedulingHarness {
    SchedulingHarness::new(
        [
            RoleBudget::new(Role::PackOverride, 8, 4),
            RoleBudget::new(Role::Package, 8, 4),
            RoleBudget::new(Role::Font, 8, 4),
        ],
        NonZeroUsize::new(max_in_flight).unwrap(),
        [
            Target::font(
                "fonts",
                FontContainerIdentity::from_bytes(b"font-container"),
            ),
            Target::pack_override("overrides-b", "z.typ"),
            Target::package("packages", package("@preview/pkg:1.0.0")),
            Target::pack_override("overrides-a", "a.typ"),
        ],
    )
}

fn package(value: &str) -> PackageSpec {
    value.parse().unwrap()
}

fn font_identity_key(bytes: &[u8]) -> String {
    FontContainerIdentity::from_bytes(bytes)
        .digest()
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn permutations(values: [usize; 3]) -> [[usize; 3]; 6] {
    let [a, b, c] = values;
    [
        [a, b, c],
        [a, c, b],
        [b, a, c],
        [b, c, a],
        [c, a, b],
        [c, b, a],
    ]
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
