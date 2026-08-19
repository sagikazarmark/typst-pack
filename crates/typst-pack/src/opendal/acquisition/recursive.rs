use futures_util::StreamExt;
use opendal::ErrorKind;

use super::super::acquisition::{
    ExactObjectLimitError, ExactPathAcquisitionError, ResolvedOperator, ResolvedOperators,
    acquire_exact_path,
};
use super::super::location::{Location, LocationRoleError, OperatorResolver};
use crate::acquisition_layout;
use crate::package_catalog::{
    PackageTreeError, PackageTreePathPreflightError, preflight_package_tree_paths,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecursiveAcquisitionResource {
    ListedEntries,
    ListedPathBytes,
    TotalListedPathBytes,
    SelectedObjects,
    ObjectBytes,
    TotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecursiveAcquisitionLimits {
    pub(crate) listed_entries: u64,
    pub(crate) listed_path_bytes: u64,
    pub(crate) total_listed_path_bytes: u64,
    pub(crate) selected_objects: u64,
    pub(crate) object_bytes: u64,
    pub(crate) total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecursiveAcquisitionSelection {
    AllFiles,
    FontContainers,
    PackageTree,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RecursiveSurveyIssueKind {
    ListedPathOutsidePrefix,
    PrefixMarkerWhereFileRequired,
    EmptyRelativeOperationPath,
    InvalidRelativeOperationPath,
    DuplicateListedObject,
    UnsupportedEntryKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecursiveSurveyIssue {
    pub(crate) source_index: usize,
    pub(crate) operation_path: String,
    pub(crate) kind: RecursiveSurveyIssueKind,
}

#[derive(Debug)]
pub(crate) struct RecursiveSurveyedObject {
    pub(crate) operation_path: String,
    pub(crate) relative_path: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum RecursiveAcquisitionError<E> {
    InvalidLocationRole(LocationRoleError),
    ResolveOperator(E),
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    List(opendal::Error),
    Read {
        operation_path: String,
        source: opendal::Error,
    },
    ListedObjectAbsent {
        operation_path: String,
        source: opendal::Error,
    },
    Structural(Vec<RecursiveSurveyIssue>),
    InvalidPackageTree(PackageTreeError),
    Limit {
        resource: RecursiveAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    AccountingOverflow {
        resource: RecursiveAcquisitionResource,
    },
}

#[derive(Debug)]
pub(crate) struct RecursiveSourcesAcquisitionError<E> {
    pub(crate) source_index: usize,
    pub(crate) source: RecursiveAcquisitionError<E>,
}

pub(crate) async fn acquire_recursive_prefix<R: OperatorResolver + ?Sized>(
    resolver: &R,
    location: &Location,
    selection: RecursiveAcquisitionSelection,
    limits: RecursiveAcquisitionLimits,
) -> Result<Vec<RecursiveSurveyedObject>, RecursiveAcquisitionError<R::Error>> {
    let mut sources = acquire_recursive_prefixes(resolver, &[location], selection, limits)
        .await
        .map_err(|error| error.source)?;
    Ok(sources.pop().expect("one requested prefix has one result"))
}

pub(crate) async fn acquire_first_present_recursive_prefix<R: OperatorResolver + ?Sized>(
    resolver: &R,
    locations: impl IntoIterator<Item = Result<Location, LocationRoleError>>,
    selection: RecursiveAcquisitionSelection,
    limits: RecursiveAcquisitionLimits,
) -> Result<
    Option<(usize, Location, Vec<RecursiveSurveyedObject>)>,
    RecursiveSourcesAcquisitionError<R::Error>,
> {
    let mut resolved = ResolvedOperators::new(resolver);
    acquire_first_present_recursive_prefix_with_resolved(
        &mut resolved,
        locations,
        selection,
        limits,
    )
    .await
}

pub(crate) async fn acquire_first_present_recursive_prefix_with_resolved<
    R: OperatorResolver + ?Sized,
>(
    resolved: &mut ResolvedOperators<'_, R>,
    locations: impl IntoIterator<Item = Result<Location, LocationRoleError>>,
    selection: RecursiveAcquisitionSelection,
    limits: RecursiveAcquisitionLimits,
) -> Result<
    Option<(usize, Location, Vec<RecursiveSurveyedObject>)>,
    RecursiveSourcesAcquisitionError<R::Error>,
> {
    let mut accounting = SurveyAccounting::new(limits);
    let mut retained_bytes = 0u64;

    for (source_index, location) in locations.into_iter().enumerate() {
        let location = location.map_err(|source| RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::InvalidLocationRole(source),
        })?;
        let mut issues = Vec::new();
        let mut plan = survey_recursive_prefix(
            resolved,
            &location,
            source_index,
            selection,
            &mut accounting,
            &mut issues,
        )
        .await?;
        check_survey_limits_and_envelope_issues(&accounting, &mut issues)?;
        if selection == RecursiveAcquisitionSelection::PackageTree {
            preflight_package_tree_plan(&mut accounting, source_index, &mut plan)?;
        }
        if plan.selected.is_empty() {
            continue;
        }

        let objects = read_source_plan(limits, source_index, plan, &mut retained_bytes).await?;
        return Ok(Some((source_index, location, objects)));
    }

    Ok(None)
}

pub(crate) async fn acquire_recursive_prefixes<R: OperatorResolver + ?Sized>(
    resolver: &R,
    locations: &[&Location],
    selection: RecursiveAcquisitionSelection,
    limits: RecursiveAcquisitionLimits,
) -> Result<Vec<Vec<RecursiveSurveyedObject>>, RecursiveSourcesAcquisitionError<R::Error>> {
    let mut accounting = SurveyAccounting::new(limits);
    let mut issues = Vec::new();
    let mut resolved = ResolvedOperators::new(resolver);
    let mut plans = Vec::with_capacity(locations.len());

    for (source_index, location) in locations.iter().copied().enumerate() {
        plans.push(
            survey_recursive_prefix(
                &mut resolved,
                location,
                source_index,
                selection,
                &mut accounting,
                &mut issues,
            )
            .await?,
        );
    }

    check_survey_limits_and_envelope_issues(&accounting, &mut issues)?;
    if selection == RecursiveAcquisitionSelection::PackageTree {
        for (source_index, plan) in plans.iter_mut().enumerate() {
            preflight_package_tree_plan(&mut accounting, source_index, plan)?;
        }
    }

    let mut retained_bytes = 0u64;
    let mut sources = Vec::with_capacity(plans.len());
    for (source_index, plan) in plans.into_iter().enumerate() {
        sources.push(read_source_plan(limits, source_index, plan, &mut retained_bytes).await?);
    }

    Ok(sources)
}

pub(crate) enum RequiredRecursiveSurvey {
    Complete(Vec<RecursiveSurveyPlan>),
    PrefixAbsent { source_index: usize },
}

pub(crate) async fn survey_required_recursive_prefixes_with_operators(
    sources: &[(&Location, ResolvedOperator)],
    limits: RecursiveAcquisitionLimits,
) -> Result<RequiredRecursiveSurvey, RecursiveSourcesAcquisitionError<std::convert::Infallible>> {
    let mut accounting = SurveyAccounting::new(limits);
    let mut issues = Vec::new();
    let mut plans = Vec::with_capacity(sources.len());

    for (source_index, (location, resolved)) in sources.iter().enumerate() {
        location
            .require_prefix()
            .map_err(|source| RecursiveSourcesAcquisitionError {
                source_index,
                source: RecursiveAcquisitionError::InvalidLocationRole(source),
            })?;
        plans.push(
            survey_recursive_prefix_with_operator(
                resolved.clone(),
                location,
                source_index,
                RecursiveAcquisitionSelection::PackageTree,
                &mut accounting,
                &mut issues,
            )
            .await?,
        );
        if plans.last().is_some_and(|plan| plan.selected.is_empty()) {
            check_survey_limits_and_envelope_issues(&accounting, &mut issues)?;
            preflight_package_tree_plans(&mut accounting, &mut plans)?;
            return Ok(RequiredRecursiveSurvey::PrefixAbsent { source_index });
        }
    }

    check_survey_limits_and_envelope_issues(&accounting, &mut issues)?;
    preflight_package_tree_plans(&mut accounting, &mut plans)?;
    Ok(RequiredRecursiveSurvey::Complete(plans))
}

async fn survey_recursive_prefix<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    location: &Location,
    source_index: usize,
    selection: RecursiveAcquisitionSelection,
    accounting: &mut SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
) -> Result<RecursiveSurveyPlan, RecursiveSourcesAcquisitionError<R::Error>> {
    location
        .require_prefix()
        .map_err(|source| RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::InvalidLocationRole(source),
        })?;
    let resolved = resolved.resolve(location.binding()).map_err(|source| {
        RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::ResolveOperator(source),
        }
    })?;
    survey_recursive_prefix_with_operator(
        resolved,
        location,
        source_index,
        selection,
        accounting,
        issues,
    )
    .await
}

async fn survey_recursive_prefix_with_operator<E>(
    resolved: ResolvedOperator,
    location: &Location,
    source_index: usize,
    selection: RecursiveAcquisitionSelection,
    accounting: &mut SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
) -> Result<RecursiveSurveyPlan, RecursiveSourcesAcquisitionError<E>> {
    if !(resolved.list && resolved.list_with_recursive) {
        return Err(RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::UnsupportedCapabilities {
                list: resolved.list,
                list_with_recursive: resolved.list_with_recursive,
                read: resolved.read,
            },
        });
    }

    let mut lister = resolved
        .operator
        .lister_with(location.dispatch_path())
        .recursive(true)
        .await
        .map_err(|source| RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::List(source),
        })?;
    let mut selected = Vec::new();

    while let Some(entry) = lister.next().await {
        let entry = entry.map_err(|source| RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::List(source),
        })?;
        let operation_path = entry.path();
        let retain_entry_evidence = accounting.observe_entry(source_index, operation_path);

        let relative = match location.relative_file_path(operation_path) {
            Ok(relative) => relative,
            Err(super::super::location::PrefixConfinementError::OutsidePrefix) => {
                retain_issue(
                    accounting,
                    issues,
                    source_index,
                    operation_path,
                    RecursiveSurveyIssueKind::ListedPathOutsidePrefix,
                    retain_entry_evidence,
                );
                continue;
            }
            Err(super::super::location::PrefixConfinementError::PrefixMarker) => {
                let mode = entry.metadata().mode();
                if mode.is_file() {
                    retain_issue(
                        accounting,
                        issues,
                        source_index,
                        operation_path,
                        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired,
                        retain_entry_evidence,
                    );
                } else if !mode.is_dir() {
                    retain_issue(
                        accounting,
                        issues,
                        source_index,
                        operation_path,
                        RecursiveSurveyIssueKind::UnsupportedEntryKind,
                        retain_entry_evidence,
                    );
                }
                continue;
            }
            Err(super::super::location::PrefixConfinementError::EmptyPath) => {
                retain_issue(
                    accounting,
                    issues,
                    source_index,
                    operation_path,
                    RecursiveSurveyIssueKind::EmptyRelativeOperationPath,
                    retain_entry_evidence,
                );
                continue;
            }
        };

        if selection != RecursiveAcquisitionSelection::PackageTree
            && Location::from_operation_path(location.binding().clone(), operation_path).is_err()
        {
            retain_issue(
                accounting,
                issues,
                source_index,
                operation_path,
                RecursiveSurveyIssueKind::InvalidRelativeOperationPath,
                retain_entry_evidence,
            );
            continue;
        }

        let mode = entry.metadata().mode();
        if mode.is_dir() {
            continue;
        }
        if !mode.is_file() {
            retain_issue(
                accounting,
                issues,
                source_index,
                operation_path,
                RecursiveSurveyIssueKind::UnsupportedEntryKind,
                retain_entry_evidence,
            );
            continue;
        }
        if operation_path.ends_with('/') || relative.is_empty() {
            retain_issue(
                accounting,
                issues,
                source_index,
                operation_path,
                RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired,
                retain_entry_evidence,
            );
            continue;
        }
        if !selection.selects(relative) {
            continue;
        }

        let retain_selected = accounting.observe_selected(source_index);
        if retain_entry_evidence
            && accounting.can_retain_evidence()
            && accounting.retain_paths(source_index, &[operation_path.len(), relative.len()])
            && retain_selected
        {
            selected.push(RecursiveSurveyedPath {
                operation_path: operation_path.to_owned(),
                relative_path: relative.to_owned(),
            });
        }
    }

    selected.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if selection != RecursiveAcquisitionSelection::PackageTree {
        let mut last_duplicate = None;
        for duplicate in selected
            .windows(2)
            .filter(|pair| pair[0].operation_path == pair[1].operation_path)
        {
            if last_duplicate == Some(duplicate[0].operation_path.as_str()) {
                continue;
            }
            last_duplicate = Some(duplicate[0].operation_path.as_str());
            retain_issue(
                accounting,
                issues,
                source_index,
                &duplicate[0].operation_path,
                RecursiveSurveyIssueKind::DuplicateListedObject,
                true,
            );
        }
    }

    Ok(RecursiveSurveyPlan {
        operator: resolved.operator,
        read: resolved.read,
        selected,
    })
}

fn check_survey_limits_and_envelope_issues<E>(
    accounting: &SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
) -> Result<(), RecursiveSourcesAcquisitionError<E>> {
    if let Some((source_index, source)) = accounting.survey_error() {
        return Err(RecursiveSourcesAcquisitionError {
            source_index,
            source,
        });
    }
    issues.sort_by(|left, right| {
        left.source_index
            .cmp(&right.source_index)
            .then_with(|| left.operation_path.cmp(&right.operation_path))
            .then_with(|| issue_rank(left.kind).cmp(&issue_rank(right.kind)))
    });
    issues.dedup();
    if let Some(first) = issues.first() {
        return Err(RecursiveSourcesAcquisitionError {
            source_index: first.source_index,
            source: RecursiveAcquisitionError::Structural(std::mem::take(issues)),
        });
    }
    Ok(())
}

fn preflight_package_tree_plan<E>(
    accounting: &mut SurveyAccounting,
    source_index: usize,
    plan: &mut RecursiveSurveyPlan,
) -> Result<(), RecursiveSourcesAcquisitionError<E>> {
    let preflight = preflight_package_tree_paths(
        plan.selected
            .iter()
            .map(|object| object.relative_path.as_str()),
        |lengths| accounting.retain_paths(source_index, lengths),
    );
    match preflight {
        Ok(canonical_paths) => {
            for (path, canonical) in plan.selected.iter_mut().zip(canonical_paths) {
                path.relative_path = canonical;
            }
            plan.selected
                .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
            Ok(())
        }
        Err(PackageTreePathPreflightError::Invalid(source)) => {
            Err(RecursiveSourcesAcquisitionError {
                source_index,
                source: RecursiveAcquisitionError::InvalidPackageTree(source),
            })
        }
        Err(PackageTreePathPreflightError::RetentionLimit) => {
            let (source_index, source) = accounting
                .survey_error()
                .expect("path preflight retention failure records a survey limit");
            Err(RecursiveSourcesAcquisitionError {
                source_index,
                source,
            })
        }
    }
}

fn preflight_package_tree_plans<E>(
    accounting: &mut SurveyAccounting,
    plans: &mut [RecursiveSurveyPlan],
) -> Result<(), RecursiveSourcesAcquisitionError<E>> {
    let mut first_error = None;
    for (source_index, plan) in plans.iter_mut().enumerate() {
        if let Err(error) = preflight_package_tree_plan(accounting, source_index, plan)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    if let Some((source_index, source)) = accounting.survey_error() {
        return Err(RecursiveSourcesAcquisitionError {
            source_index,
            source,
        });
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(())
}

async fn read_source_plan<E>(
    limits: RecursiveAcquisitionLimits,
    source_index: usize,
    plan: RecursiveSurveyPlan,
    retained_bytes: &mut u64,
) -> Result<Vec<RecursiveSurveyedObject>, RecursiveSourcesAcquisitionError<E>> {
    if !plan.selected.is_empty() && !plan.read {
        return Err(RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::UnsupportedCapabilities {
                list: true,
                list_with_recursive: true,
                read: false,
            },
        });
    }
    let mut objects = Vec::with_capacity(plan.selected.len());
    for path in plan.selected {
        let remaining = limits.total_bytes.checked_sub(*retained_bytes).ok_or(
            RecursiveSourcesAcquisitionError {
                source_index,
                source: RecursiveAcquisitionError::AccountingOverflow {
                    resource: RecursiveAcquisitionResource::TotalBytes,
                },
            },
        )?;
        let ceiling = limits.object_bytes.min(remaining);
        let bytes = acquire_exact_path(
            &plan.operator,
            &path.operation_path,
            ceiling,
            limits.object_bytes,
        )
        .await
        .map_err(|error| RecursiveSourcesAcquisitionError {
            source_index,
            source: map_read_error(error, &path.operation_path, limits, remaining),
        })?;
        let length = u64::try_from(bytes.len()).map_err(|_| RecursiveSourcesAcquisitionError {
            source_index,
            source: RecursiveAcquisitionError::AccountingOverflow {
                resource: RecursiveAcquisitionResource::TotalBytes,
            },
        })?;
        *retained_bytes =
            retained_bytes
                .checked_add(length)
                .ok_or(RecursiveSourcesAcquisitionError {
                    source_index,
                    source: RecursiveAcquisitionError::AccountingOverflow {
                        resource: RecursiveAcquisitionResource::TotalBytes,
                    },
                })?;
        objects.push(RecursiveSurveyedObject {
            operation_path: path.operation_path,
            relative_path: path.relative_path,
            bytes,
        });
    }
    Ok(objects)
}

#[derive(Debug)]
pub(crate) struct RecursiveSurveyedPath {
    pub(crate) operation_path: String,
    pub(crate) relative_path: String,
}

#[derive(Debug)]
pub(crate) struct RecursiveSurveyPlan {
    pub(crate) operator: opendal::Operator,
    pub(crate) read: bool,
    pub(crate) selected: Vec<RecursiveSurveyedPath>,
}

#[derive(Clone, Copy, Debug)]
enum Violation {
    Exceeded {
        source_index: usize,
        ceiling: u64,
        observed_at_least: u64,
    },
    Overflow {
        source_index: usize,
    },
}

struct SurveyAccounting {
    limits: RecursiveAcquisitionLimits,
    listed_entries: u64,
    retained_path_bytes: u64,
    selected_objects: u64,
    violations: [Option<Violation>; 4],
}

impl SurveyAccounting {
    fn new(limits: RecursiveAcquisitionLimits) -> Self {
        Self {
            limits,
            listed_entries: 0,
            retained_path_bytes: 0,
            selected_objects: 0,
            violations: [None; 4],
        }
    }

    fn observe_entry(&mut self, source_index: usize, operation_path: &str) -> bool {
        let mut retain_entry_evidence = true;
        self.listed_entries = match self.listed_entries.checked_add(1) {
            Some(observed) => {
                if observed > self.limits.listed_entries {
                    self.record_exceeded(0, source_index, self.limits.listed_entries, observed);
                    retain_entry_evidence = false;
                }
                observed
            }
            None => {
                self.violations[0] = Some(Violation::Overflow { source_index });
                retain_entry_evidence = false;
                u64::MAX
            }
        };
        match u64::try_from(operation_path.len()) {
            Ok(observed) if observed > self.limits.listed_path_bytes => {
                self.record_exceeded(
                    1,
                    source_index,
                    self.limits.listed_path_bytes,
                    self.limits.listed_path_bytes.saturating_add(1),
                );
                retain_entry_evidence = false;
            }
            Ok(_) => {}
            Err(_) => {
                self.violations[1] = Some(Violation::Overflow { source_index });
                retain_entry_evidence = false;
            }
        }
        retain_entry_evidence
    }

    fn observe_selected(&mut self, source_index: usize) -> bool {
        let mut within_limit = true;
        self.selected_objects = match self.selected_objects.checked_add(1) {
            Some(observed) => {
                if observed > self.limits.selected_objects {
                    self.record_exceeded(3, source_index, self.limits.selected_objects, observed);
                    within_limit = false;
                }
                observed
            }
            None => {
                self.violations[3] = Some(Violation::Overflow { source_index });
                within_limit = false;
                u64::MAX
            }
        };
        within_limit
    }

    fn retain_paths(&mut self, source_index: usize, lengths: &[usize]) -> bool {
        if self.violations[2].is_some() {
            return false;
        }
        let mut observed = self.retained_path_bytes;
        for length in lengths {
            let Ok(length) = u64::try_from(*length) else {
                self.violations[2] = Some(Violation::Overflow { source_index });
                return false;
            };
            let Some(next) = observed.checked_add(length) else {
                self.violations[2] = Some(Violation::Overflow { source_index });
                return false;
            };
            observed = next;
        }
        if observed > self.limits.total_listed_path_bytes {
            self.record_exceeded(
                2,
                source_index,
                self.limits.total_listed_path_bytes,
                self.limits.total_listed_path_bytes.saturating_add(1),
            );
            return false;
        }
        self.retained_path_bytes = observed;
        true
    }

    fn can_retain_evidence(&self) -> bool {
        self.violations[..3].iter().all(Option::is_none)
    }

    fn record_exceeded(
        &mut self,
        index: usize,
        source_index: usize,
        ceiling: u64,
        observed_at_least: u64,
    ) {
        if self.violations[index].is_none() {
            self.violations[index] = Some(Violation::Exceeded {
                source_index,
                ceiling,
                observed_at_least,
            });
        }
    }

    fn survey_error<E>(&self) -> Option<(usize, RecursiveAcquisitionError<E>)> {
        let resources = [
            RecursiveAcquisitionResource::ListedEntries,
            RecursiveAcquisitionResource::ListedPathBytes,
            RecursiveAcquisitionResource::TotalListedPathBytes,
            RecursiveAcquisitionResource::SelectedObjects,
        ];
        self.violations
            .iter()
            .copied()
            .zip(resources)
            .find_map(|(violation, resource)| match violation? {
                Violation::Exceeded {
                    source_index,
                    ceiling,
                    observed_at_least,
                } => Some((
                    source_index,
                    RecursiveAcquisitionError::Limit {
                        resource,
                        ceiling,
                        observed_at_least,
                    },
                )),
                Violation::Overflow { source_index } => Some((
                    source_index,
                    RecursiveAcquisitionError::AccountingOverflow { resource },
                )),
            })
    }
}

impl RecursiveAcquisitionSelection {
    fn selects(self, relative_path: &str) -> bool {
        match self {
            Self::AllFiles | Self::PackageTree => true,
            Self::FontContainers => acquisition_layout::is_font_container_key(relative_path),
        }
    }
}

fn retain_issue(
    accounting: &mut SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
    source_index: usize,
    operation_path: &str,
    kind: RecursiveSurveyIssueKind,
    retain_entry_evidence: bool,
) {
    if !retain_entry_evidence || !accounting.can_retain_evidence() {
        return;
    }
    if accounting.retain_paths(source_index, &[operation_path.len()]) {
        issues.push(RecursiveSurveyIssue {
            source_index,
            operation_path: operation_path.to_owned(),
            kind,
        });
    }
}

fn issue_rank(kind: RecursiveSurveyIssueKind) -> u8 {
    match kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => 0,
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => 1,
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => 2,
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath => 3,
        RecursiveSurveyIssueKind::DuplicateListedObject => 4,
        RecursiveSurveyIssueKind::UnsupportedEntryKind => 5,
    }
}

fn map_read_error<E>(
    error: ExactPathAcquisitionError,
    operation_path: &str,
    limits: RecursiveAcquisitionLimits,
    remaining: u64,
) -> RecursiveAcquisitionError<E> {
    match error {
        ExactPathAcquisitionError::ObjectAbsent(source) => {
            debug_assert_eq!(source.kind(), ErrorKind::NotFound);
            RecursiveAcquisitionError::ListedObjectAbsent {
                operation_path: operation_path.to_owned(),
                source,
            }
        }
        ExactPathAcquisitionError::Read(source) => RecursiveAcquisitionError::Read {
            operation_path: operation_path.to_owned(),
            source,
        },
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::AccountingOverflow) => {
            RecursiveAcquisitionError::AccountingOverflow {
                resource: if limits.object_bytes <= remaining {
                    RecursiveAcquisitionResource::ObjectBytes
                } else {
                    RecursiveAcquisitionResource::TotalBytes
                },
            }
        }
        ExactPathAcquisitionError::Limit(ExactObjectLimitError::Exceeded {
            observed_at_least,
            ..
        }) => {
            let (resource, ceiling) = if observed_at_least > limits.object_bytes {
                (
                    RecursiveAcquisitionResource::ObjectBytes,
                    limits.object_bytes,
                )
            } else {
                (RecursiveAcquisitionResource::TotalBytes, limits.total_bytes)
            };
            RecursiveAcquisitionError::Limit {
                resource,
                ceiling,
                observed_at_least: ceiling.saturating_add(1),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
        PendingPoint, ReadScript, ReadStep, ScriptedService,
    };
    use crate::opendal::{Location, OperatorBinding, OperatorResolver};

    use super::*;

    #[test]
    fn drains_root_survey_before_reading_and_returns_exact_bytes_in_path_order() {
        let list = ListScript::new(
            "/",
            3,
            [ListStep::page([
                ListEntry::file("z.typ"),
                ListEntry::directory("assets/"),
                ListEntry::file("a.typ"),
            ])],
        )
        .unwrap();
        let reads = [
            ReadScript::new("a.typ", 1, [ReadStep::chunk(b"a exact")]).unwrap(),
            ReadScript::new("z.typ", 1, [ReadStep::chunk(b"z exact")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [list], reads, 16);
        let resolver = DirectResolver(service.operator());
        let location = location("");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));

        let objects = expect_ready(acquisition.as_mut()).unwrap();
        assert_eq!(
            objects
                .iter()
                .map(|object| (object.relative_path.as_str(), object.bytes.as_slice()))
                .collect::<Vec<_>>(),
            [
                ("a.typ", b"a exact".as_slice()),
                ("z.typ", b"z exact".as_slice())
            ]
        );
        let log = service.log();
        assert!(matches!(
            log.entries(),
            [
                OperationLogEntry::ListInvoked { path, recursive: true, .. },
                OperationLogEntry::ListPageYielded { .. },
                OperationLogEntry::ListCompleted { .. },
                OperationLogEntry::ReadInvoked { .. },
                ..
            ] if path == "/"
        ));
    }

    #[test]
    fn non_root_survey_reports_confined_structural_issues_in_canonical_order() {
        let list = ListScript::new(
            "project/",
            5,
            [ListStep::page([
                ListEntry::unknown("project/z"),
                ListEntry::file("project-sibling/a"),
                ListEntry::file("project/a//b"),
                ListEntry::file("project/good.typ"),
                ListEntry::file("project/good.typ"),
            ])],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [], 16);
        let resolver = DirectResolver(service.operator());
        let location = location("project/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));

        let error = expect_ready(acquisition.as_mut()).unwrap_err();
        let RecursiveAcquisitionError::Structural(issues) = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(
            issues,
            [
                issue(
                    "project-sibling/a",
                    RecursiveSurveyIssueKind::ListedPathOutsidePrefix
                ),
                issue(
                    "project/a//b",
                    RecursiveSurveyIssueKind::InvalidRelativeOperationPath
                ),
                issue(
                    "project/good.typ",
                    RecursiveSurveyIssueKind::DuplicateListedObject
                ),
                issue("project/z", RecursiveSurveyIssueKind::UnsupportedEntryKind),
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
    fn listing_permutations_choose_fixed_limit_precedence() {
        let entries = [
            ListEntry::file("p/long-name.typ"),
            ListEntry::file("p/a.typ"),
            ListEntry::file("p/b.typ"),
        ];
        for entries in [
            entries.clone().to_vec(),
            entries.into_iter().rev().collect(),
        ] {
            let list = ListScript::new("p/", 3, [ListStep::page(entries)]).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [list], [], 8);
            let resolver = DirectResolver(service.operator());
            let mut constrained = limits();
            constrained.listed_entries = 2;
            constrained.listed_path_bytes = 6;
            constrained.total_listed_path_bytes = 5;
            constrained.selected_objects = 1;
            let location = location("p/");
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(acquisition.as_mut()).unwrap_err(),
                RecursiveAcquisitionError::Limit {
                    resource: RecursiveAcquisitionResource::ListedEntries,
                    ceiling: 2,
                    observed_at_least: 3,
                }
            ));
            assert!(matches!(
                service.log().entries().last(),
                Some(OperationLogEntry::ListCompleted { .. })
            ));
        }
    }

    #[test]
    fn retained_path_limit_precedes_an_already_observed_selected_object_limit() {
        for entries in [
            [ListEntry::file("p/a"), ListEntry::file("p/long")],
            [ListEntry::file("p/long"), ListEntry::file("p/a")],
        ] {
            let list = ListScript::new("p/", 2, [ListStep::page(entries)]).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [list], [], 8);
            let resolver = DirectResolver(service.operator());
            let constrained = RecursiveAcquisitionLimits {
                total_listed_path_bytes: 10,
                selected_objects: 1,
                ..limits()
            };
            let prefix = location("p/");
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &prefix,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(acquisition.as_mut()).unwrap_err(),
                RecursiveAcquisitionError::Limit {
                    resource: RecursiveAcquisitionResource::TotalListedPathBytes,
                    ceiling: 10,
                    observed_at_least: 11,
                }
            ));
        }
    }

    #[test]
    fn every_finite_survey_limit_accepts_exact_and_rejects_plus_one() {
        let cases = [
            (
                RecursiveAcquisitionResource::ListedEntries,
                RecursiveAcquisitionLimits {
                    listed_entries: 1,
                    ..limits()
                },
            ),
            (
                RecursiveAcquisitionResource::ListedPathBytes,
                RecursiveAcquisitionLimits {
                    listed_path_bytes: 6,
                    ..limits()
                },
            ),
            (
                RecursiveAcquisitionResource::TotalListedPathBytes,
                RecursiveAcquisitionLimits {
                    total_listed_path_bytes: 4,
                    ..limits()
                },
            ),
            (
                RecursiveAcquisitionResource::SelectedObjects,
                RecursiveAcquisitionLimits {
                    selected_objects: 1,
                    ..limits()
                },
            ),
        ];

        for (resource, constrained) in cases {
            let exact_entries = match resource {
                RecursiveAcquisitionResource::ListedEntries => {
                    vec![ListEntry::directory("p/dir/")]
                }
                RecursiveAcquisitionResource::ListedPathBytes => {
                    vec![ListEntry::directory("p/dir/")]
                }
                RecursiveAcquisitionResource::TotalListedPathBytes
                | RecursiveAcquisitionResource::SelectedObjects => {
                    vec![ListEntry::file("p/a")]
                }
                _ => unreachable!(),
            };
            let exact_reads = matches!(
                resource,
                RecursiveAcquisitionResource::TotalListedPathBytes
                    | RecursiveAcquisitionResource::SelectedObjects
            )
            .then(|| ReadScript::new("p/a", 1, [ReadStep::chunk(b"a")]).unwrap());
            let exact_list =
                ListScript::new("p/", exact_entries.len(), [ListStep::page(exact_entries)])
                    .unwrap();
            let exact_service =
                ScriptedService::new(Capabilities::all(), [exact_list], exact_reads, 4);
            let exact_resolver = DirectResolver(exact_service.operator());
            let prefix = location("p/");
            let mut exact = pin!(acquire_recursive_prefix(
                &exact_resolver,
                &prefix,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));
            expect_ready(exact.as_mut()).unwrap();

            let over_entries = match resource {
                RecursiveAcquisitionResource::ListedEntries => {
                    vec![ListEntry::directory("p/a/"), ListEntry::directory("p/b/")]
                }
                RecursiveAcquisitionResource::ListedPathBytes => {
                    vec![ListEntry::directory("p/long/")]
                }
                RecursiveAcquisitionResource::TotalListedPathBytes => {
                    vec![ListEntry::file("p/ab")]
                }
                RecursiveAcquisitionResource::SelectedObjects => {
                    vec![ListEntry::file("p/a"), ListEntry::file("p/b")]
                }
                _ => unreachable!(),
            };
            let over_list =
                ListScript::new("p/", over_entries.len(), [ListStep::page(over_entries)]).unwrap();
            let over_service = ScriptedService::new(Capabilities::all(), [over_list], [], 4);
            let over_resolver = DirectResolver(over_service.operator());
            let prefix = location("p/");
            let mut over = pin!(acquire_recursive_prefix(
                &over_resolver,
                &prefix,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));
            assert!(matches!(
                expect_ready(over.as_mut()).unwrap_err(),
                RecursiveAcquisitionError::Limit {
                    resource: actual,
                    ..
                } if actual == resource
            ));
        }
    }

    #[test]
    fn payload_limits_use_per_object_before_total_and_preserve_exact_boundaries() {
        for (name, object_bytes, total_bytes, expected) in [
            ("exact", 4, 4, None),
            (
                "object",
                3,
                8,
                Some(RecursiveAcquisitionResource::ObjectBytes),
            ),
            (
                "both",
                3,
                3,
                Some(RecursiveAcquisitionResource::ObjectBytes),
            ),
        ] {
            let path = format!("p/{name}");
            let list = ListScript::new("p/", 1, [ListStep::page([ListEntry::file(path.clone())])])
                .unwrap();
            let read = ReadScript::new(&path, 1, [ReadStep::chunk(b"four")]).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
            let resolver = DirectResolver(service.operator());
            let location = location("p/");
            let constrained = RecursiveAcquisitionLimits {
                object_bytes,
                total_bytes,
                ..limits()
            };
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));

            match expected {
                None => assert_eq!(
                    expect_ready(acquisition.as_mut()).unwrap()[0].bytes,
                    b"four"
                ),
                Some(resource) => assert!(matches!(
                    expect_ready(acquisition.as_mut()).unwrap_err(),
                    RecursiveAcquisitionError::Limit {
                        resource: actual,
                        observed_at_least: 4,
                        ..
                    } if actual == resource
                )),
            }
        }
    }

    #[test]
    fn later_payload_crossing_object_and_total_reports_object_bytes() {
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
            ReadScript::new("p/a", 1, [ReadStep::chunk(b"1234")]).unwrap(),
            ReadScript::new("p/b", 1, [ReadStep::chunk(b"123456")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [list], reads, 16);
        let resolver = DirectResolver(service.operator());
        let location = location("p/");
        let constrained = RecursiveAcquisitionLimits {
            object_bytes: 5,
            total_bytes: 7,
            ..limits()
        };
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            constrained,
        ));

        assert!(matches!(
            expect_ready(acquisition.as_mut()).unwrap_err(),
            RecursiveAcquisitionError::Limit {
                resource: RecursiveAcquisitionResource::ObjectBytes,
                ceiling: 5,
                observed_at_least: 6,
            }
        ));
    }

    #[test]
    fn valid_multi_object_permutations_preserve_total_payload_boundaries() {
        for entries in [
            [ListEntry::file("p/a"), ListEntry::file("p/b")],
            [ListEntry::file("p/b"), ListEntry::file("p/a")],
        ] {
            let list = ListScript::new("p/", 2, [ListStep::page(entries)]).unwrap();
            let reads = [
                ReadScript::new("p/a", 1, [ReadStep::chunk(b"1234")]).unwrap(),
                ReadScript::new("p/b", 1, [ReadStep::chunk(b"5678")]).unwrap(),
            ];
            let service = ScriptedService::new(Capabilities::all(), [list], reads, 16);
            let resolver = DirectResolver(service.operator());
            let location = location("p/");
            let constrained = RecursiveAcquisitionLimits {
                object_bytes: 5,
                total_bytes: 7,
                ..limits()
            };
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(acquisition.as_mut()).unwrap_err(),
                RecursiveAcquisitionError::Limit {
                    resource: RecursiveAcquisitionResource::TotalBytes,
                    ceiling: 7,
                    observed_at_least: 8,
                }
            ));
        }

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
            ReadScript::new("p/a", 1, [ReadStep::chunk(b"1234")]).unwrap(),
            ReadScript::new("p/b", 1, [ReadStep::chunk(b"5678")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [list], reads, 16);
        let resolver = DirectResolver(service.operator());
        let location = location("p/");
        let exact = RecursiveAcquisitionLimits {
            object_bytes: 5,
            total_bytes: 8,
            ..limits()
        };
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            exact,
        ));
        assert_eq!(
            expect_ready(acquisition.as_mut())
                .unwrap()
                .iter()
                .map(|object| object.bytes.len())
                .sum::<usize>(),
            8
        );
    }

    #[test]
    fn checked_accounting_overflow_names_the_resource() {
        let mut accounting = SurveyAccounting::new(limits());
        accounting.listed_entries = u64::MAX;
        accounting.observe_entry(0, "p/a");
        assert!(matches!(
            accounting.survey_error::<Infallible>(),
            Some((
                0,
                RecursiveAcquisitionError::AccountingOverflow {
                    resource: RecursiveAcquisitionResource::ListedEntries,
                }
            ))
        ));

        let mut accounting = SurveyAccounting::new(RecursiveAcquisitionLimits {
            total_listed_path_bytes: u64::MAX,
            ..limits()
        });
        accounting.retained_path_bytes = u64::MAX;
        assert!(!accounting.retain_paths(0, &[1]));
        assert!(matches!(
            accounting.survey_error::<Infallible>(),
            Some((
                0,
                RecursiveAcquisitionError::AccountingOverflow {
                    resource: RecursiveAcquisitionResource::TotalListedPathBytes,
                }
            ))
        ));
    }

    #[test]
    fn capability_and_terminal_list_failures_precede_payload_reads() {
        for capabilities in [
            Capabilities {
                list: false,
                list_with_recursive: true,
                read: true,
            },
            Capabilities {
                list: true,
                list_with_recursive: false,
                read: true,
            },
        ] {
            let service = ScriptedService::new(capabilities, [], [], 4);
            let resolver = DirectResolver(service.operator());
            let location = location("p/");
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                limits(),
            ));
            assert!(matches!(
                expect_ready(acquisition.as_mut()).unwrap_err(),
                RecursiveAcquisitionError::UnsupportedCapabilities { .. }
            ));
            assert!(service.log().entries().is_empty());
        }

        let service = ScriptedService::new(
            Capabilities {
                list: true,
                list_with_recursive: true,
                read: false,
            },
            [ListScript::new("p/", 1, [ListStep::page([ListEntry::file("p/a")])]).unwrap()],
            [],
            4,
        );
        let resolver = DirectResolver(service.operator());
        let source = location("p/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &source,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));
        assert!(matches!(
            expect_ready(acquisition.as_mut()).unwrap_err(),
            RecursiveAcquisitionError::UnsupportedCapabilities {
                list: true,
                list_with_recursive: true,
                read: false,
            }
        ));
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );

        let list = ListScript::new(
            "p/",
            1,
            [
                ListStep::page([ListEntry::file("p/a")]),
                ListStep::failure(ErrorKind::PermissionDenied),
            ],
        )
        .unwrap();
        let read = ReadScript::new("p/a", 1, [ReadStep::chunk(b"unread")]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("p/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));
        assert!(matches!(
            expect_ready(acquisition.as_mut()).unwrap_err(),
            RecursiveAcquisitionError::List(source)
                if source.kind() == ErrorKind::PermissionDenied
        ));
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn font_selection_is_ascii_case_insensitive_and_ignores_other_files() {
        let list = ListScript::new(
            "fonts/",
            3,
            [ListStep::page([
                ListEntry::file("fonts/A.TTF"),
                ListEntry::file("fonts/b.otc"),
                ListEntry::file("fonts/readme.txt"),
            ])],
        )
        .unwrap();
        let reads = [
            ReadScript::new("fonts/A.TTF", 1, [ReadStep::chunk(b"a")]).unwrap(),
            ReadScript::new("fonts/b.otc", 1, [ReadStep::chunk(b"b")]).unwrap(),
        ];
        let service = ScriptedService::new(Capabilities::all(), [list], reads, 16);
        let resolver = DirectResolver(service.operator());
        let location = location("fonts/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::FontContainers,
            limits(),
        ));

        assert_eq!(
            expect_ready(acquisition.as_mut())
                .unwrap()
                .into_iter()
                .map(|object| object.relative_path)
                .collect::<Vec<_>>(),
            ["A.TTF", "b.otc"]
        );
    }

    #[test]
    fn package_tree_results_use_core_canonical_relative_paths() {
        let list = ListScript::new(
            "package/",
            1,
            [ListStep::page([ListEntry::file("package/./lib.typ")])],
        )
        .unwrap();
        let read = ReadScript::new("package/./lib.typ", 1, [ReadStep::chunk(b"library")]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("package/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::PackageTree,
            limits(),
        ));

        let objects = expect_ready(acquisition.as_mut()).unwrap();
        assert_eq!(objects[0].operation_path, "package/./lib.typ");
        assert_eq!(objects[0].relative_path, "lib.typ");
        assert_eq!(objects[0].bytes, b"library");
    }

    #[test]
    fn reads_exact_bytes_observed_after_the_completed_listing() {
        let replacement = ReadScript::new(
            "race/changing.typ",
            1,
            [ReadStep::chunk(b"value after listing")],
        )
        .unwrap();
        let list = ListScript::new(
            "race/",
            1,
            [
                ListStep::page([ListEntry::file("race/changing.typ")]),
                ListStep::replace_read(replacement),
            ],
        )
        .unwrap();
        let original = ReadScript::new(
            "race/changing.typ",
            1,
            [ReadStep::chunk(b"value during listing")],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [original], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("race/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));

        assert_eq!(
            expect_ready(acquisition.as_mut()).unwrap()[0].bytes,
            b"value after listing"
        );
        assert!(matches!(
            service.log().entries(),
            [
                OperationLogEntry::ListInvoked { .. },
                OperationLogEntry::ListPageYielded { .. },
                OperationLogEntry::ListCompleted { .. },
                OperationLogEntry::ReadInvoked { .. },
                ..
            ]
        ));
    }

    #[test]
    fn unretained_path_overage_reports_only_the_plus_one_bound() {
        let list = ListScript::new(
            "p/",
            1,
            [ListStep::page([ListEntry::file("p/a-very-long-path")])],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [], 4);
        let resolver = DirectResolver(service.operator());
        let location = location("p/");
        let constrained = RecursiveAcquisitionLimits {
            listed_path_bytes: 4,
            ..limits()
        };
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            constrained,
        ));

        assert!(matches!(
            expect_ready(acquisition.as_mut()).unwrap_err(),
            RecursiveAcquisitionError::Limit {
                resource: RecursiveAcquisitionResource::ListedPathBytes,
                ceiling: 4,
                observed_at_least: 5,
            }
        ));
    }

    #[test]
    fn memory_service_supports_root_and_non_root_surveys() {
        for (prefix, path) in [("", "root.typ"), ("project/", "project/main.typ")] {
            let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
            {
                let mut write = pin!(operator.write(path, b"memory bytes".to_vec()));
                expect_ready(write.as_mut()).unwrap();
            }
            let resolver = DirectResolver(operator);
            let location = location(prefix);
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                limits(),
            ));

            let objects = expect_ready(acquisition.as_mut()).unwrap();
            assert_eq!(objects.len(), 1);
            assert_eq!(objects[0].bytes, b"memory bytes");
        }
    }

    #[test]
    fn package_tree_preflight_precedes_reads_and_preserves_typed_cause() {
        let list = ListScript::new(
            "package/",
            2,
            [ListStep::page([
                ListEntry::file("package/assets"),
                ListEntry::file("package/assets/logo.svg"),
            ])],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("package/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::PackageTree,
            limits(),
        ));

        let error = expect_ready(acquisition.as_mut()).unwrap_err();
        assert!(matches!(
            error,
            RecursiveAcquisitionError::InvalidPackageTree(source)
                if source.issues().len() == 1
        ));
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn classifies_disappearance_after_a_completed_listing() {
        let list = ListScript::new(
            "race/",
            1,
            [ListStep::page([ListEntry::file("race/gone.typ")])],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("race/");
        let mut acquisition = pin!(acquire_recursive_prefix(
            &resolver,
            &location,
            RecursiveAcquisitionSelection::AllFiles,
            limits(),
        ));

        assert!(matches!(
            expect_ready(acquisition.as_mut()).unwrap_err(),
            RecursiveAcquisitionError::ListedObjectAbsent { operation_path, .. }
                if operation_path == "race/gone.typ"
        ));
    }

    #[test]
    fn dropping_a_pending_survey_cancels_without_starting_reads() {
        let pending = PendingPoint::new();
        let list = ListScript::new(
            "pending/",
            1,
            [
                ListStep::page([ListEntry::file("pending/a.typ")]),
                ListStep::pending(pending.clone()),
            ],
        )
        .unwrap();
        let read = ReadScript::new("pending/a.typ", 1, [ReadStep::chunk(b"never")]).unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
        let resolver = DirectResolver(service.operator());
        {
            let location = location("pending/");
            let mut acquisition = pin!(acquire_recursive_prefix(
                &resolver,
                &location,
                RecursiveAcquisitionSelection::AllFiles,
                limits(),
            ));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(pending.was_observed());
        }

        assert_eq!(
            service.cancellations(),
            [DroppedOperation::List {
                id: 0,
                path: "pending/".to_owned(),
            }]
        );
    }

    fn limits() -> RecursiveAcquisitionLimits {
        RecursiveAcquisitionLimits {
            listed_entries: 32,
            listed_path_bytes: 128,
            total_listed_path_bytes: 1024,
            selected_objects: 32,
            object_bytes: 128,
            total_bytes: 1024,
        }
    }

    fn issue(path: &str, kind: RecursiveSurveyIssueKind) -> RecursiveSurveyIssue {
        RecursiveSurveyIssue {
            source_index: 0,
            operation_path: path.to_owned(),
            kind,
        }
    }

    fn location(path: &str) -> Location {
        Location::from_operation_path(OperatorBinding::new("store").unwrap(), path).unwrap()
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

    struct DirectResolver(opendal::Operator);

    impl OperatorResolver for DirectResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            Ok(self.0.clone())
        }
    }
}
