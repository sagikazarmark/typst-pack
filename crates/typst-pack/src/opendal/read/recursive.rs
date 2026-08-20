use futures_util::StreamExt;

use super::super::BoxError;
use super::super::location::{Location, LocationRoleError, OperatorResolver};
use super::super::read::{
    ExactPathReadOperation, ResolvedOperator, ResolvedOperators, exact_path_absent_error,
    read_exact_path,
};
use crate::limits::{Limits, ResourceKind};
use crate::package_catalog::{
    PackageTreeError, PackageTreePathPreflightError, preflight_package_tree_paths,
};
use crate::read_layout;

pub(crate) type RecursiveReadResource = ResourceKind<8>;

#[allow(non_upper_case_globals)]
impl ResourceKind<8> {
    pub(crate) const ListedEntries: Self = Self::new(0);
    pub(crate) const ListedPathBytes: Self = Self::new(1);
    pub(crate) const TotalListedPathBytes: Self = Self::new(2);
    pub(crate) const SelectedObjects: Self = Self::new(3);
    pub(crate) const ObjectBytes: Self = Self::new(4);
    pub(crate) const TotalBytes: Self = Self::new(5);
}

pub(crate) type RecursiveReadLimits = Limits<RecursiveReadResource>;

impl Limits<RecursiveReadResource> {
    pub(crate) const fn new(
        listed_entries: u64,
        listed_path_bytes: u64,
        total_listed_path_bytes: u64,
        selected_objects: u64,
        object_bytes: u64,
        total_bytes: u64,
    ) -> Self {
        Self::from_ceilings([
            listed_entries,
            listed_path_bytes,
            total_listed_path_bytes,
            selected_objects,
            object_bytes,
            total_bytes,
            0,
        ])
    }

    pub(crate) const fn listed_entries(&self) -> u64 {
        self.ceilings[0]
    }

    pub(crate) const fn listed_path_bytes(&self) -> u64 {
        self.ceilings[1]
    }

    pub(crate) const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    pub(crate) const fn selected_objects(&self) -> u64 {
        self.ceilings[3]
    }

    pub(crate) const fn object_bytes(&self) -> u64 {
        self.ceilings[4]
    }

    pub(crate) const fn total_bytes(&self) -> u64 {
        self.ceilings[5]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecursiveReadSelection {
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

pub(crate) trait RecursiveReadOperation {
    type Error;

    fn invalid_location_role(&self, source_index: usize, source: LocationRoleError) -> Self::Error;
    fn resolve_operator(&self, source_index: usize, source: BoxError) -> Self::Error;
    fn unsupported_capabilities(
        &self,
        source_index: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> Self::Error;
    fn list(&self, source_index: usize, source: opendal::Error) -> Self::Error;
    fn read(
        &self,
        source_index: usize,
        operation_path: String,
        source: opendal::Error,
    ) -> Self::Error;
    fn listed_object_absent(
        &self,
        source_index: usize,
        operation_path: String,
        source: opendal::Error,
    ) -> Self::Error;
    fn structural(&self, source_index: usize, issues: Vec<RecursiveSurveyIssue>) -> Self::Error;
    fn limit(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
        ceiling: u64,
        observed_at_least: u64,
    ) -> Self::Error;
    fn accounting_overflow(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
    ) -> Self::Error;
}

pub(crate) trait PackageTreeRecursiveReadOperation: RecursiveReadOperation {
    fn invalid_package_tree(&self, source_index: usize, source: PackageTreeError) -> Self::Error;
}

pub(crate) async fn read_recursive_prefix<R, O>(
    resolver: &R,
    location: &Location,
    selection: RecursiveReadSelection,
    limits: RecursiveReadLimits,
    operation: &O,
) -> Result<Vec<RecursiveSurveyedObject>, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: RecursiveReadOperation,
{
    debug_assert_ne!(selection, RecursiveReadSelection::PackageTree);
    let mut sources =
        read_recursive_prefixes(resolver, &[location], selection, limits, operation).await?;
    Ok(sources.pop().expect("one requested prefix has one result"))
}

pub(crate) async fn read_first_present_package_tree_prefix_with_resolved<R, O>(
    resolved: &mut ResolvedOperators<'_, R>,
    locations: impl IntoIterator<Item = Result<Location, LocationRoleError>>,
    limits: RecursiveReadLimits,
    operation: &O,
) -> Result<Option<(usize, Location, Vec<RecursiveSurveyedObject>)>, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: PackageTreeRecursiveReadOperation,
{
    let mut accounting = SurveyAccounting::new(limits);
    let mut retained_bytes = 0u64;

    for (source_index, location) in locations.into_iter().enumerate() {
        let location =
            location.map_err(|source| operation.invalid_location_role(source_index, source))?;
        let mut issues = Vec::new();
        let mut plan = survey_recursive_prefix(
            resolved,
            &location,
            source_index,
            RecursiveReadSelection::PackageTree,
            &mut accounting,
            &mut issues,
            operation,
        )
        .await?;
        check_survey_limits_and_envelope_issues(&accounting, &mut issues, operation)?;
        preflight_package_tree_plan(&mut accounting, source_index, &mut plan, operation)?;
        if plan.selected.is_empty() {
            continue;
        }

        let objects =
            read_source_plan(limits, source_index, plan, &mut retained_bytes, operation).await?;
        return Ok(Some((source_index, location, objects)));
    }

    Ok(None)
}

#[cfg(test)]
async fn read_package_tree_recursive_prefix<R, O>(
    resolver: &R,
    location: &Location,
    limits: RecursiveReadLimits,
    operation: &O,
) -> Result<Vec<RecursiveSurveyedObject>, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: PackageTreeRecursiveReadOperation,
{
    let mut resolved = ResolvedOperators::new(resolver);
    Ok(read_first_present_package_tree_prefix_with_resolved(
        &mut resolved,
        [Ok(location.clone())],
        limits,
        operation,
    )
    .await?
    .map(|(_, _, objects)| objects)
    .unwrap_or_default())
}

pub(crate) async fn read_recursive_prefixes<R, O>(
    resolver: &R,
    locations: &[&Location],
    selection: RecursiveReadSelection,
    limits: RecursiveReadLimits,
    operation: &O,
) -> Result<Vec<Vec<RecursiveSurveyedObject>>, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: RecursiveReadOperation,
{
    debug_assert_ne!(selection, RecursiveReadSelection::PackageTree);
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
                operation,
            )
            .await?,
        );
    }

    check_survey_limits_and_envelope_issues(&accounting, &mut issues, operation)?;
    let mut retained_bytes = 0u64;
    let mut sources = Vec::with_capacity(plans.len());
    for (source_index, plan) in plans.into_iter().enumerate() {
        sources.push(
            read_source_plan(limits, source_index, plan, &mut retained_bytes, operation).await?,
        );
    }

    Ok(sources)
}

async fn survey_recursive_prefix<R, O>(
    resolved: &mut ResolvedOperators<'_, R>,
    location: &Location,
    source_index: usize,
    selection: RecursiveReadSelection,
    accounting: &mut SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
    operation: &O,
) -> Result<RecursiveSurveyPlan, O::Error>
where
    R: OperatorResolver + ?Sized,
    O: RecursiveReadOperation,
{
    location
        .require_prefix()
        .map_err(|source| operation.invalid_location_role(source_index, source))?;
    let resolved = resolved
        .resolve(location.binding())
        .map_err(|source| operation.resolve_operator(source_index, Box::new(source)))?;
    survey_recursive_prefix_with_operator(
        resolved,
        location,
        source_index,
        selection,
        accounting,
        issues,
        operation,
    )
    .await
}

async fn survey_recursive_prefix_with_operator<O: RecursiveReadOperation>(
    resolved: ResolvedOperator,
    location: &Location,
    source_index: usize,
    selection: RecursiveReadSelection,
    accounting: &mut SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
    operation: &O,
) -> Result<RecursiveSurveyPlan, O::Error> {
    if !(resolved.list && resolved.list_with_recursive) {
        return Err(operation.unsupported_capabilities(
            source_index,
            resolved.list,
            resolved.list_with_recursive,
            resolved.read,
        ));
    }

    let mut lister = resolved
        .operator
        .lister_with(location.dispatch_path())
        .recursive(true)
        .await
        .map_err(|source| operation.list(source_index, source))?;
    let mut selected = Vec::new();

    while let Some(entry) = lister.next().await {
        let entry = entry.map_err(|source| operation.list(source_index, source))?;
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

        if selection != RecursiveReadSelection::PackageTree
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
    if selection != RecursiveReadSelection::PackageTree {
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

fn check_survey_limits_and_envelope_issues<O: RecursiveReadOperation>(
    accounting: &SurveyAccounting,
    issues: &mut Vec<RecursiveSurveyIssue>,
    operation: &O,
) -> Result<(), O::Error> {
    if let Some(error) = accounting.error(operation) {
        return Err(error);
    }
    issues.sort_by(|left, right| {
        left.source_index
            .cmp(&right.source_index)
            .then_with(|| left.operation_path.cmp(&right.operation_path))
            .then_with(|| issue_rank(left.kind).cmp(&issue_rank(right.kind)))
    });
    issues.dedup();
    if let Some(first) = issues.first() {
        return Err(operation.structural(first.source_index, std::mem::take(issues)));
    }
    Ok(())
}

fn preflight_package_tree_plan<O: PackageTreeRecursiveReadOperation>(
    accounting: &mut SurveyAccounting,
    source_index: usize,
    plan: &mut RecursiveSurveyPlan,
    operation: &O,
) -> Result<(), O::Error> {
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
            Err(operation.invalid_package_tree(source_index, source))
        }
        Err(PackageTreePathPreflightError::RetentionLimit) => Err(accounting
            .error(operation)
            .expect("path preflight retention failure records a survey limit")),
    }
}

async fn read_source_plan<O: RecursiveReadOperation>(
    limits: RecursiveReadLimits,
    source_index: usize,
    plan: RecursiveSurveyPlan,
    retained_bytes: &mut u64,
    operation: &O,
) -> Result<Vec<RecursiveSurveyedObject>, O::Error> {
    if !plan.selected.is_empty() && !plan.read {
        return Err(operation.unsupported_capabilities(source_index, true, true, false));
    }
    let mut objects = Vec::with_capacity(plan.selected.len());
    for path in plan.selected {
        let remaining = limits
            .total_bytes()
            .checked_sub(*retained_bytes)
            .ok_or_else(|| {
                operation.accounting_overflow(source_index, RecursiveReadResource::TotalBytes)
            })?;
        let ceiling = limits.object_bytes().min(remaining);
        let exact_operation = RecursiveExactPathOperation {
            operation,
            source_index,
            operation_path: &path.operation_path,
            limits,
            remaining,
        };
        let bytes = read_exact_path(
            &plan.operator,
            &path.operation_path,
            ceiling,
            limits.object_bytes(),
            &exact_operation,
        )
        .await?
        .ok_or_else(|| {
            operation.listed_object_absent(
                source_index,
                path.operation_path.clone(),
                exact_path_absent_error(),
            )
        })?;
        let length = u64::try_from(bytes.len()).map_err(|_| {
            operation.accounting_overflow(source_index, RecursiveReadResource::TotalBytes)
        })?;
        *retained_bytes = retained_bytes.checked_add(length).ok_or_else(|| {
            operation.accounting_overflow(source_index, RecursiveReadResource::TotalBytes)
        })?;
        objects.push(RecursiveSurveyedObject {
            operation_path: path.operation_path,
            relative_path: path.relative_path,
            bytes,
        });
    }
    Ok(objects)
}

struct RecursiveExactPathOperation<'a, O> {
    operation: &'a O,
    source_index: usize,
    operation_path: &'a str,
    limits: RecursiveReadLimits,
    remaining: u64,
}

impl<O: RecursiveReadOperation> ExactPathReadOperation for RecursiveExactPathOperation<'_, O> {
    type Error = O::Error;

    fn read(&self, source: opendal::Error) -> O::Error {
        self.operation
            .read(self.source_index, self.operation_path.to_owned(), source)
    }

    fn limit_exceeded(&self, _: u64, observed_at_least: u64) -> O::Error {
        let (resource, ceiling) = if observed_at_least > self.limits.object_bytes() {
            (
                RecursiveReadResource::ObjectBytes,
                self.limits.object_bytes(),
            )
        } else {
            (RecursiveReadResource::TotalBytes, self.limits.total_bytes())
        };
        self.operation.limit(
            self.source_index,
            resource,
            ceiling,
            ceiling.saturating_add(1),
        )
    }

    fn accounting_overflow(&self) -> O::Error {
        self.operation.accounting_overflow(
            self.source_index,
            if self.limits.object_bytes() <= self.remaining {
                RecursiveReadResource::ObjectBytes
            } else {
                RecursiveReadResource::TotalBytes
            },
        )
    }
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
    limits: RecursiveReadLimits,
    listed_entries: u64,
    retained_path_bytes: u64,
    selected_objects: u64,
    violations: [Option<Violation>; 4],
}

impl SurveyAccounting {
    fn new(limits: RecursiveReadLimits) -> Self {
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
                if observed > self.limits.listed_entries() {
                    self.record_exceeded(0, source_index, self.limits.listed_entries(), observed);
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
            Ok(observed) if observed > self.limits.listed_path_bytes() => {
                self.record_exceeded(
                    1,
                    source_index,
                    self.limits.listed_path_bytes(),
                    self.limits.listed_path_bytes().saturating_add(1),
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
                if observed > self.limits.selected_objects() {
                    self.record_exceeded(3, source_index, self.limits.selected_objects(), observed);
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
        if observed > self.limits.total_listed_path_bytes() {
            self.record_exceeded(
                2,
                source_index,
                self.limits.total_listed_path_bytes(),
                self.limits.total_listed_path_bytes().saturating_add(1),
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

    fn error<O: RecursiveReadOperation>(&self, operation: &O) -> Option<O::Error> {
        let resources = [
            RecursiveReadResource::ListedEntries,
            RecursiveReadResource::ListedPathBytes,
            RecursiveReadResource::TotalListedPathBytes,
            RecursiveReadResource::SelectedObjects,
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
                } => Some(operation.limit(source_index, resource, ceiling, observed_at_least)),
                Violation::Overflow { source_index } => {
                    Some(operation.accounting_overflow(source_index, resource))
                }
            })
    }
}

impl RecursiveReadSelection {
    fn selects(self, relative_path: &str) -> bool {
        match self {
            Self::AllFiles | Self::PackageTree => true,
            Self::FontContainers => read_layout::is_font_container_key(relative_path),
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

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;

    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
        PendingPoint, ReadScript, ReadStep, ScriptedService,
    };
    use crate::opendal::{Location, OperatorBinding, OperatorResolver};

    use super::*;

    static TEST_OPERATION: TestOperation = TestOperation;

    async fn read_recursive_prefix<R: OperatorResolver + ?Sized>(
        resolver: &R,
        location: &Location,
        selection: RecursiveReadSelection,
        limits: RecursiveReadLimits,
    ) -> Result<Vec<RecursiveSurveyedObject>, TestError> {
        if selection == RecursiveReadSelection::PackageTree {
            read_package_tree_recursive_prefix(resolver, location, limits, &TEST_OPERATION).await
        } else {
            super::read_recursive_prefix(resolver, location, selection, limits, &TEST_OPERATION)
                .await
        }
    }

    #[derive(Debug)]
    enum TestError {
        Other,
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
            resource: RecursiveReadResource,
            ceiling: u64,
            observed_at_least: u64,
        },
        AccountingOverflow {
            resource: RecursiveReadResource,
        },
    }

    struct TestOperation;

    impl RecursiveReadOperation for TestOperation {
        type Error = TestError;

        fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> TestError {
            TestError::Other
        }

        fn resolve_operator(&self, _: usize, _: BoxError) -> TestError {
            TestError::Other
        }

        fn unsupported_capabilities(
            &self,
            _: usize,
            list: bool,
            list_with_recursive: bool,
            read: bool,
        ) -> TestError {
            TestError::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            }
        }

        fn list(&self, _: usize, source: opendal::Error) -> TestError {
            TestError::List(source)
        }

        fn read(&self, _: usize, operation_path: String, source: opendal::Error) -> TestError {
            TestError::Read {
                operation_path,
                source,
            }
        }

        fn listed_object_absent(
            &self,
            _: usize,
            operation_path: String,
            source: opendal::Error,
        ) -> TestError {
            TestError::ListedObjectAbsent {
                operation_path,
                source,
            }
        }

        fn structural(&self, _: usize, issues: Vec<RecursiveSurveyIssue>) -> TestError {
            TestError::Structural(issues)
        }

        fn limit(
            &self,
            _: usize,
            resource: RecursiveReadResource,
            ceiling: u64,
            observed_at_least: u64,
        ) -> TestError {
            TestError::Limit {
                resource,
                ceiling,
                observed_at_least,
            }
        }

        fn accounting_overflow(&self, _: usize, resource: RecursiveReadResource) -> TestError {
            TestError::AccountingOverflow { resource }
        }
    }

    impl PackageTreeRecursiveReadOperation for TestOperation {
        fn invalid_package_tree(&self, _: usize, source: PackageTreeError) -> TestError {
            TestError::InvalidPackageTree(source)
        }
    }

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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));

        let objects = expect_ready(read.as_mut()).unwrap();
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));

        let error = expect_ready(read.as_mut()).unwrap_err();
        let TestError::Structural(issues) = error else {
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
            let constrained = RecursiveReadLimits::new(2, 6, 5, 1, 128, 1024);
            let location = location("p/");
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(read.as_mut()).unwrap_err(),
                TestError::Limit {
                    resource: RecursiveReadResource::ListedEntries,
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
            let constrained = RecursiveReadLimits::new(32, 128, 10, 1, 128, 1024);
            let prefix = location("p/");
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &prefix,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(read.as_mut()).unwrap_err(),
                TestError::Limit {
                    resource: RecursiveReadResource::TotalListedPathBytes,
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
                RecursiveReadResource::ListedEntries,
                RecursiveReadLimits::new(1, 128, 1024, 32, 128, 1024),
            ),
            (
                RecursiveReadResource::ListedPathBytes,
                RecursiveReadLimits::new(32, 6, 1024, 32, 128, 1024),
            ),
            (
                RecursiveReadResource::TotalListedPathBytes,
                RecursiveReadLimits::new(32, 128, 4, 32, 128, 1024),
            ),
            (
                RecursiveReadResource::SelectedObjects,
                RecursiveReadLimits::new(32, 128, 1024, 1, 128, 1024),
            ),
        ];

        for (resource, constrained) in cases {
            let exact_entries = match resource {
                RecursiveReadResource::ListedEntries => {
                    vec![ListEntry::directory("p/dir/")]
                }
                RecursiveReadResource::ListedPathBytes => {
                    vec![ListEntry::directory("p/dir/")]
                }
                RecursiveReadResource::TotalListedPathBytes
                | RecursiveReadResource::SelectedObjects => {
                    vec![ListEntry::file("p/a")]
                }
                _ => unreachable!(),
            };
            let exact_reads = matches!(
                resource,
                RecursiveReadResource::TotalListedPathBytes
                    | RecursiveReadResource::SelectedObjects
            )
            .then(|| ReadScript::new("p/a", 1, [ReadStep::chunk(b"a")]).unwrap());
            let exact_list =
                ListScript::new("p/", exact_entries.len(), [ListStep::page(exact_entries)])
                    .unwrap();
            let exact_service =
                ScriptedService::new(Capabilities::all(), [exact_list], exact_reads, 4);
            let exact_resolver = DirectResolver(exact_service.operator());
            let prefix = location("p/");
            let mut exact = pin!(read_recursive_prefix(
                &exact_resolver,
                &prefix,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));
            expect_ready(exact.as_mut()).unwrap();

            let over_entries = match resource {
                RecursiveReadResource::ListedEntries => {
                    vec![ListEntry::directory("p/a/"), ListEntry::directory("p/b/")]
                }
                RecursiveReadResource::ListedPathBytes => {
                    vec![ListEntry::directory("p/long/")]
                }
                RecursiveReadResource::TotalListedPathBytes => {
                    vec![ListEntry::file("p/ab")]
                }
                RecursiveReadResource::SelectedObjects => {
                    vec![ListEntry::file("p/a"), ListEntry::file("p/b")]
                }
                _ => unreachable!(),
            };
            let over_list =
                ListScript::new("p/", over_entries.len(), [ListStep::page(over_entries)]).unwrap();
            let over_service = ScriptedService::new(Capabilities::all(), [over_list], [], 4);
            let over_resolver = DirectResolver(over_service.operator());
            let prefix = location("p/");
            let mut over = pin!(read_recursive_prefix(
                &over_resolver,
                &prefix,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));
            assert!(matches!(
                expect_ready(over.as_mut()).unwrap_err(),
                TestError::Limit {
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
            ("object", 3, 8, Some(RecursiveReadResource::ObjectBytes)),
            ("both", 3, 3, Some(RecursiveReadResource::ObjectBytes)),
        ] {
            let path = format!("p/{name}");
            let list = ListScript::new("p/", 1, [ListStep::page([ListEntry::file(path.clone())])])
                .unwrap();
            let read = ReadScript::new(&path, 1, [ReadStep::chunk(b"four")]).unwrap();
            let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
            let resolver = DirectResolver(service.operator());
            let location = location("p/");
            let constrained =
                RecursiveReadLimits::new(32, 128, 1024, 32, object_bytes, total_bytes);
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));

            match expected {
                None => assert_eq!(expect_ready(read.as_mut()).unwrap()[0].bytes, b"four"),
                Some(resource) => assert!(matches!(
                    expect_ready(read.as_mut()).unwrap_err(),
                    TestError::Limit {
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
        let constrained = RecursiveReadLimits::new(32, 128, 1024, 32, 5, 7);
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            constrained,
        ));

        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::Limit {
                resource: RecursiveReadResource::ObjectBytes,
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
            let constrained = RecursiveReadLimits::new(32, 128, 1024, 32, 5, 7);
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                constrained,
            ));

            assert!(matches!(
                expect_ready(read.as_mut()).unwrap_err(),
                TestError::Limit {
                    resource: RecursiveReadResource::TotalBytes,
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
        let exact = RecursiveReadLimits::new(32, 128, 1024, 32, 5, 8);
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            exact,
        ));
        assert_eq!(
            expect_ready(read.as_mut())
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
            accounting.error(&TEST_OPERATION),
            Some(TestError::AccountingOverflow {
                resource: RecursiveReadResource::ListedEntries,
            })
        ));

        let mut accounting =
            SurveyAccounting::new(RecursiveReadLimits::new(32, 128, u64::MAX, 32, 128, 1024));
        accounting.retained_path_bytes = u64::MAX;
        assert!(!accounting.retain_paths(0, &[1]));
        assert!(matches!(
            accounting.error(&TEST_OPERATION),
            Some(TestError::AccountingOverflow {
                resource: RecursiveReadResource::TotalListedPathBytes,
            })
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
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                limits(),
            ));
            assert!(matches!(
                expect_ready(read.as_mut()).unwrap_err(),
                TestError::UnsupportedCapabilities { .. }
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &source,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));
        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::UnsupportedCapabilities {
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));
        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::List(source)
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::FontContainers,
            limits(),
        ));

        assert_eq!(
            expect_ready(read.as_mut())
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::PackageTree,
            limits(),
        ));

        let objects = expect_ready(read.as_mut()).unwrap();
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));

        assert_eq!(
            expect_ready(read.as_mut()).unwrap()[0].bytes,
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
        let constrained = RecursiveReadLimits::new(32, 4, 1024, 32, 128, 1024);
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            constrained,
        ));

        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::Limit {
                resource: RecursiveReadResource::ListedPathBytes,
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
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                limits(),
            ));

            let objects = expect_ready(read.as_mut()).unwrap();
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::PackageTree,
            limits(),
        ));

        let error = expect_ready(read.as_mut()).unwrap_err();
        assert!(matches!(
            error,
            TestError::InvalidPackageTree(source)
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
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));

        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::ListedObjectAbsent { operation_path, .. }
                if operation_path == "race/gone.typ"
        ));
    }

    #[test]
    fn not_found_after_a_yielded_buffer_is_a_terminal_read_failure() {
        let list = ListScript::new(
            "race/",
            1,
            [ListStep::page([ListEntry::file("race/partial.typ")])],
        )
        .unwrap();
        let read = ReadScript::new(
            "race/partial.typ",
            1,
            [
                ReadStep::chunk(b"partial"),
                ReadStep::failure(ErrorKind::NotFound),
            ],
        )
        .unwrap();
        let service = ScriptedService::new(Capabilities::all(), [list], [read], 8);
        let resolver = DirectResolver(service.operator());
        let location = location("race/");
        let mut read = pin!(read_recursive_prefix(
            &resolver,
            &location,
            RecursiveReadSelection::AllFiles,
            limits(),
        ));

        assert!(matches!(
            expect_ready(read.as_mut()).unwrap_err(),
            TestError::Read { operation_path, source }
                if operation_path == "race/partial.typ" && source.kind() == ErrorKind::NotFound
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
            let mut read = pin!(read_recursive_prefix(
                &resolver,
                &location,
                RecursiveReadSelection::AllFiles,
                limits(),
            ));
            assert!(matches!(poll_once(read.as_mut()), Poll::Pending));
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

    fn limits() -> RecursiveReadLimits {
        RecursiveReadLimits::new(32, 128, 1024, 32, 128, 1024)
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
