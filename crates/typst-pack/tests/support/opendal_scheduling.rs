use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use typst::syntax::package::PackageSpec;
use typst_pack::FontContainerIdentity;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Role {
    PackOverride,
    Package,
    Font,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleBudget {
    role: Role,
    total_bytes: u64,
    per_object_bytes: u64,
}

impl RoleBudget {
    pub const fn new(role: Role, total_bytes: u64, per_object_bytes: u64) -> Self {
        assert!(
            total_bytes < u64::MAX,
            "the total must leave room for a probe"
        );
        assert!(
            per_object_bytes < u64::MAX,
            "the per-object limit must leave room for a probe"
        );
        assert!(per_object_bytes <= total_bytes);
        Self {
            role,
            total_bytes,
            per_object_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    PackOverride {
        binding: String,
        path: String,
    },
    Package {
        binding: String,
        spec: PackageSpec,
    },
    Font {
        binding: String,
        identity: FontContainerIdentity,
    },
}

impl Target {
    pub fn pack_override(binding: impl Into<String>, path: impl Into<String>) -> Self {
        Self::PackOverride {
            binding: binding.into(),
            path: path.into(),
        }
    }

    pub fn package(binding: impl Into<String>, spec: PackageSpec) -> Self {
        Self::Package {
            binding: binding.into(),
            spec: spec.into(),
        }
    }

    pub fn font(binding: impl Into<String>, identity: FontContainerIdentity) -> Self {
        Self::Font {
            binding: binding.into(),
            identity: identity.into(),
        }
    }

    const fn role(&self) -> Role {
        match self {
            Self::PackOverride { .. } => Role::PackOverride,
            Self::Package { .. } => Role::Package,
            Self::Font { .. } => Role::Font,
        }
    }

    fn binding(&self) -> &str {
        match self {
            Self::PackOverride { binding, .. }
            | Self::Package { binding, .. }
            | Self::Font { binding, .. } => binding,
        }
    }

    fn key(&self) -> String {
        match self {
            Self::PackOverride { path, .. } => path.clone(),
            Self::Package { spec, .. } => spec.to_string(),
            Self::Font { identity, .. } => hex(identity.digest()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitResource {
    ObjectBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCause {
    Read,
    Exceeded {
        resource: LimitResource,
        ceiling: u64,
        observed_at_least: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulingFailure {
    pub target: usize,
    pub role: Role,
    pub cause: FailureCause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleAccounting {
    pub reserved_in_flight: u64,
    pub retained_in_flight: u64,
    pub retained_success: u64,
    pub probe_bytes: u64,
    pub peak_retained_payload: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenDalAllocation {
    Service,
    Transport,
    YieldedBuffer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulingEvent {
    Reserved {
        target: usize,
        role: Role,
        bytes: u64,
    },
    Launched {
        target: usize,
        role: Role,
        binding: String,
        key: String,
    },
    ReservationBlocked {
        target: usize,
        role: Role,
        required: u64,
        remaining: u64,
    },
    InFlightPayloadRetained {
        target: usize,
        role: Role,
        bytes: u64,
    },
    RetainedSuccessCharged {
        target: usize,
        role: Role,
        bytes: u64,
    },
    ProbeCharged {
        target: usize,
        role: Role,
        bytes: u64,
    },
    ProbeReleased {
        target: usize,
        role: Role,
        bytes: u64,
    },
    ReservationRefunded {
        target: usize,
        role: Role,
        bytes: u64,
    },
    ReservationReleased {
        target: usize,
        role: Role,
        bytes: u64,
    },
    Failed {
        target: usize,
        role: Role,
        cause: FailureCause,
    },
    Dropped {
        target: usize,
        role: Role,
    },
    Cancelled {
        target: usize,
        role: Role,
    },
}

pub struct SchedulingHarness {
    budgets: BTreeMap<Role, RoleState>,
    max_in_flight: NonZeroUsize,
    targets: Vec<Target>,
    states: Vec<TargetState>,
    candidates: BTreeMap<usize, SchedulingFailure>,
    failure: Option<SchedulingFailure>,
    stopped: bool,
    log: Vec<SchedulingEvent>,
}

impl SchedulingHarness {
    pub fn new(
        budgets: impl IntoIterator<Item = RoleBudget>,
        max_in_flight: NonZeroUsize,
        targets: impl IntoIterator<Item = Target>,
    ) -> Self {
        let budgets = budgets
            .into_iter()
            .map(|budget| (budget.role, RoleState::new(budget)))
            .collect::<BTreeMap<_, _>>();
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by(|left, right| {
            left.role()
                .cmp(&right.role())
                .then_with(|| left.key().cmp(&right.key()))
        });
        assert!(
            targets
                .iter()
                .all(|target| budgets.contains_key(&target.role())),
            "every target role must have a budget"
        );
        assert!(
            targets
                .windows(2)
                .all(|pair| (pair[0].role(), pair[0].key()) != (pair[1].role(), pair[1].key())),
            "canonical targets must be unique"
        );
        let states = vec![TargetState::Unlaunched; targets.len()];

        Self {
            budgets,
            max_in_flight,
            targets,
            states,
            candidates: BTreeMap::new(),
            failure: None,
            stopped: false,
            log: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.schedule();
    }

    pub fn observe_payload(&mut self, target: usize, bytes: u64) {
        let TargetState::InFlight {
            reservation,
            retained,
            probe,
        } = self.states[target]
        else {
            panic!("target {target} is not in flight")
        };
        let previously_observed = checked_add(retained, probe);
        assert!(
            bytes >= previously_observed,
            "observed bytes cannot decrease"
        );
        assert!(
            bytes <= checked_add(reservation, 1),
            "a bounded read retains at most its reservation plus one probe byte"
        );
        let next_retained = bytes.min(reservation);
        let next_probe = bytes - next_retained;
        let role = self.targets[target].role();
        let state = self.budgets.get_mut(&role).unwrap();
        state.retained_in_flight = checked_add(
            checked_sub(state.retained_in_flight, retained),
            next_retained,
        );
        state.probe_bytes = checked_add(checked_sub(state.probe_bytes, probe), next_probe);
        self.states[target] = TargetState::InFlight {
            reservation,
            retained: next_retained,
            probe: next_probe,
        };
        if next_retained > retained {
            self.log.push(SchedulingEvent::InFlightPayloadRetained {
                target,
                role,
                bytes: next_retained - retained,
            });
        }
        if next_probe > probe {
            self.log.push(SchedulingEvent::ProbeCharged {
                target,
                role,
                bytes: next_probe - probe,
            });
        }
        self.update_peak(role);
    }

    pub fn complete_success(&mut self, target: usize) {
        let TargetState::InFlight {
            reservation,
            retained,
            probe: 0,
        } = self.states[target]
        else {
            panic!("a successful target must be in flight without a probe byte")
        };
        let role = self.targets[target].role();
        let state = self.budgets.get_mut(&role).unwrap();
        state.reserved_in_flight = checked_sub(state.reserved_in_flight, reservation);
        state.retained_in_flight = checked_sub(state.retained_in_flight, retained);
        state.retained_success = checked_add(state.retained_success, retained);
        self.log.push(SchedulingEvent::RetainedSuccessCharged {
            target,
            role,
            bytes: retained,
        });
        if reservation > retained {
            self.log.push(SchedulingEvent::ReservationRefunded {
                target,
                role,
                bytes: reservation - retained,
            });
        }
        self.states[target] = TargetState::Succeeded;
        self.update_peak(role);
        self.select_failure();
        if !self.stopped {
            self.schedule();
        }
    }

    pub fn complete_failure(&mut self, target: usize, cause: FailureCause) {
        let TargetState::InFlight {
            reservation,
            retained,
            probe,
        } = self.states[target]
        else {
            panic!("target {target} is not in flight")
        };
        self.release_in_flight(target, reservation, retained, probe);
        self.states[target] = TargetState::Failed;
        self.record_failure(target, cause);
        self.select_failure();
    }

    pub fn fail_probe_overage(&mut self, target: usize) {
        let TargetState::InFlight { probe: 1, .. } = self.states[target] else {
            panic!("a plus-one failure requires one retained probe byte")
        };
        let role = self.targets[target].role();
        let ceiling = self.budgets[&role].budget.per_object_bytes;
        self.complete_failure(
            target,
            FailureCause::Exceeded {
                resource: LimitResource::ObjectBytes,
                ceiling,
                observed_at_least: checked_add(ceiling, 1),
            },
        );
    }

    pub fn cancel(&mut self) {
        self.stopped = true;
        for target in self.in_flight() {
            self.drop_target(target);
        }
    }

    pub fn in_flight(&self) -> Vec<usize> {
        self.states
            .iter()
            .enumerate()
            .filter_map(|(index, state)| {
                matches!(state, TargetState::InFlight { .. }).then_some(index)
            })
            .collect()
    }

    pub fn failure(&self) -> Option<&SchedulingFailure> {
        self.failure.as_ref()
    }

    pub fn accounting(&self, role: Role) -> RoleAccounting {
        self.budgets.get(&role).unwrap().accounting()
    }

    pub fn log(&self) -> &[SchedulingEvent] {
        &self.log
    }

    pub fn is_complete(&self) -> bool {
        self.failure.is_none()
            && self
                .states
                .iter()
                .all(|state| matches!(state, TargetState::Succeeded))
    }

    pub fn assert_payload_bounds(&self) {
        let max_probe_bytes = u64::try_from(self.max_in_flight.get()).unwrap();
        for state in self.budgets.values() {
            assert!(
                state.peak_retained_payload
                    <= checked_add(state.budget.total_bytes, max_probe_bytes),
                "peak retained payload exceeded the role-total ceiling plus one probe byte per in-flight read"
            );
        }
    }

    pub const fn excluded_opendal_allocations(&self) -> [OpenDalAllocation; 3] {
        [
            OpenDalAllocation::Service,
            OpenDalAllocation::Transport,
            OpenDalAllocation::YieldedBuffer,
        ]
    }

    fn schedule(&mut self) {
        while self.in_flight().len() < self.max_in_flight.get() {
            let Some(target) = self
                .states
                .iter()
                .position(|state| matches!(state, TargetState::Unlaunched))
            else {
                break;
            };
            let role = self.targets[target].role();
            let state = self.budgets.get(&role).unwrap();
            let allowance = state.budget.per_object_bytes;
            let charged = checked_add(state.retained_success, state.reserved_in_flight);
            let remaining = checked_sub(state.budget.total_bytes, charged);
            if remaining < allowance {
                self.log.push(SchedulingEvent::ReservationBlocked {
                    target,
                    role,
                    required: allowance,
                    remaining,
                });
                if state.reserved_in_flight == 0 {
                    self.states[target] = TargetState::Failed;
                    self.record_failure(target, self.total_bytes_failure(role));
                    self.select_failure();
                }
                break;
            }

            let state = self.budgets.get_mut(&role).unwrap();
            state.reserved_in_flight = checked_add(state.reserved_in_flight, allowance);
            self.states[target] = TargetState::InFlight {
                reservation: allowance,
                retained: 0,
                probe: 0,
            };
            self.log.push(SchedulingEvent::Reserved {
                target,
                role,
                bytes: allowance,
            });
            self.log.push(SchedulingEvent::Launched {
                target,
                role,
                binding: self.targets[target].binding().to_owned(),
                key: self.targets[target].key(),
            });
        }
    }

    fn total_bytes_failure(&self, role: Role) -> FailureCause {
        let ceiling = self.budgets[&role].budget.total_bytes;
        FailureCause::Exceeded {
            resource: LimitResource::TotalBytes,
            ceiling,
            observed_at_least: checked_add(ceiling, 1),
        }
    }

    fn record_failure(&mut self, target: usize, cause: FailureCause) {
        let role = self.targets[target].role();
        self.log.push(SchedulingEvent::Failed {
            target,
            role,
            cause,
        });
        self.candidates.insert(
            target,
            SchedulingFailure {
                target,
                role,
                cause,
            },
        );
        self.stopped = true;
        let later = self
            .in_flight()
            .into_iter()
            .filter(|in_flight| *in_flight > target)
            .collect::<Vec<_>>();
        for later_target in later {
            self.drop_target(later_target);
        }
    }

    fn select_failure(&mut self) {
        let Some((&candidate, failure)) = self.candidates.first_key_value() else {
            return;
        };
        let earlier_resolved = self.states[..candidate].iter().all(|state| {
            matches!(
                state,
                TargetState::Succeeded | TargetState::Failed | TargetState::Dropped
            )
        });
        if earlier_resolved {
            self.failure = Some(*failure);
        }
    }

    fn drop_target(&mut self, target: usize) {
        let TargetState::InFlight {
            reservation,
            retained,
            probe,
        } = self.states[target]
        else {
            return;
        };
        let role = self.targets[target].role();
        self.release_in_flight(target, reservation, retained, probe);
        self.states[target] = TargetState::Dropped;
        self.log.push(SchedulingEvent::Dropped { target, role });
        self.log.push(SchedulingEvent::Cancelled { target, role });
    }

    fn release_in_flight(&mut self, target: usize, reservation: u64, retained: u64, probe: u64) {
        let role = self.targets[target].role();
        let state = self.budgets.get_mut(&role).unwrap();
        state.reserved_in_flight = checked_sub(state.reserved_in_flight, reservation);
        state.retained_in_flight = checked_sub(state.retained_in_flight, retained);
        state.probe_bytes = checked_sub(state.probe_bytes, probe);
        if probe > 0 {
            self.log.push(SchedulingEvent::ProbeReleased {
                target,
                role,
                bytes: probe,
            });
        }
        self.log.push(SchedulingEvent::ReservationReleased {
            target,
            role,
            bytes: reservation,
        });
    }

    fn update_peak(&mut self, role: Role) {
        let role_in_flight = u64::try_from(
            self.states
                .iter()
                .zip(&self.targets)
                .filter(|(state, target)| {
                    target.role() == role && matches!(state, TargetState::InFlight { .. })
                })
                .count(),
        )
        .unwrap();
        let state = self.budgets.get_mut(&role).unwrap();
        let retained = checked_add(
            checked_add(state.retained_success, state.retained_in_flight),
            state.probe_bytes,
        );
        assert!(
            retained <= checked_add(state.budget.total_bytes, role_in_flight),
            "role payload exceeded its total plus one probe byte per in-flight read"
        );
        state.peak_retained_payload = state.peak_retained_payload.max(retained);
    }
}

#[derive(Clone, Copy)]
enum TargetState {
    Unlaunched,
    InFlight {
        reservation: u64,
        retained: u64,
        probe: u64,
    },
    Succeeded,
    Failed,
    Dropped,
}

struct RoleState {
    budget: RoleBudget,
    reserved_in_flight: u64,
    retained_in_flight: u64,
    retained_success: u64,
    probe_bytes: u64,
    peak_retained_payload: u64,
}

impl RoleState {
    const fn new(budget: RoleBudget) -> Self {
        Self {
            budget,
            reserved_in_flight: 0,
            retained_in_flight: 0,
            retained_success: 0,
            probe_bytes: 0,
            peak_retained_payload: 0,
        }
    }

    const fn accounting(&self) -> RoleAccounting {
        RoleAccounting {
            reserved_in_flight: self.reserved_in_flight,
            retained_in_flight: self.retained_in_flight,
            retained_success: self.retained_success,
            probe_bytes: self.probe_bytes,
            peak_retained_payload: self.peak_retained_payload,
        }
    }
}

fn checked_add(left: u64, right: u64) -> u64 {
    left.checked_add(right).expect("test accounting overflowed")
}

fn checked_sub(left: u64, right: u64) -> u64 {
    left.checked_sub(right)
        .expect("test accounting underflowed")
}

fn hex(bytes: [u8; 16]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}
