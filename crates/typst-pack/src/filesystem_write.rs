//! Concrete filesystem writing for Pack Extraction Plans and Compilation Results.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};

use crate::pack_archive::StagingResidueStatus;
use crate::{
    CommitCertainty, CompilationArtifactWriteEntry, CompilationArtifactWriteProgress,
    CompilationArtifactWriteReceipt, CompilationResult, CompilationStatus, PackExtractionPlan,
    PackExtractionWriteEntry, PackExtractionWriteProgress, PackExtractionWriteReceipt,
    WriteKeyOutcome,
};

/// An explicit policy for writing planned files to the filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemMergePolicy {
    /// Write a complete plan at an absent destination through one root commit.
    WriteNewTree,
    /// Create every planned file and reject any existing planned target.
    MergeCreateOnly,
    /// Create missing planned files and atomically replace existing regular files.
    MergeReplaceExactFiles,
}

/// The filesystem phase reached by a plan write attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemWritePhase {
    Policy,
    Preflight,
    DirectoryCreate,
    StagingCreate,
    StagingWrite,
    StagingFlush,
    Commit,
    StagingCleanup,
    Complete,
}

/// A destination entry kind relevant to merge preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemDestinationEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

impl fmt::Display for FilesystemDestinationEntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symbolic link",
            Self::Other => "unsupported entry",
        })
    }
}

/// One safely detectable issue found before filesystem writes begin.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemWritePreflightIssue {
    #[error("planned path {relative_path:?} is not a canonical relative filesystem path")]
    InvalidRelativePath { relative_path: PathBuf },
    #[error("planned paths {first_path:?} and {second_path:?} alias on this platform")]
    PathAlias {
        first_path: PathBuf,
        second_path: PathBuf,
    },
    #[error("planned path {relative_path:?} contains reserved component {component:?}")]
    ReservedName {
        relative_path: PathBuf,
        component: String,
    },
    #[error("component {component:?} in planned path {relative_path:?} exceeds the platform limit")]
    ComponentTooLong {
        relative_path: PathBuf,
        component: String,
    },
    #[error("planned destination path {relative_path:?} exceeds the platform path limit")]
    PathTooLong { relative_path: PathBuf },
    #[error("destination path {path:?} contains reserved component {component:?}")]
    DestinationReservedName { path: PathBuf, component: String },
    #[error("component {component:?} in destination path {path:?} exceeds the platform limit")]
    DestinationComponentTooLong { path: PathBuf, component: String },
    #[error("destination path {path:?} exceeds the platform path limit")]
    DestinationPathTooLong { path: PathBuf },
    #[error("planned target {relative_path:?} already exists")]
    ExistingTarget { relative_path: PathBuf },
    #[error("planned target {relative_path:?} is an existing {kind}, not a regular file")]
    ConflictingTarget {
        relative_path: PathBuf,
        kind: FilesystemDestinationEntryKind,
    },
    #[error("ancestor {ancestor:?} of planned target {relative_path:?} is a {kind}")]
    ConflictingAncestor {
        relative_path: PathBuf,
        ancestor: PathBuf,
        kind: FilesystemDestinationEntryKind,
    },
    #[error("destination root {path:?} already exists")]
    ExistingDestinationRoot { path: PathBuf },
    #[error("destination root {path:?} is a {kind}")]
    ConflictingDestinationRoot {
        path: PathBuf,
        kind: FilesystemDestinationEntryKind,
    },
    #[error("could not inspect destination path {path:?}: {source}")]
    InspectionFailed {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
}

impl FilesystemWritePreflightIssue {
    fn sort_key(&self) -> (PathBuf, u8, PathBuf, String) {
        match self {
            Self::InvalidRelativePath { relative_path } => {
                (relative_path.clone(), 0, PathBuf::new(), String::new())
            }
            Self::PathAlias {
                first_path,
                second_path,
            } => (first_path.clone(), 1, second_path.clone(), String::new()),
            Self::ReservedName {
                relative_path,
                component,
            } => (relative_path.clone(), 2, PathBuf::new(), component.clone()),
            Self::ComponentTooLong {
                relative_path,
                component,
            } => (relative_path.clone(), 3, PathBuf::new(), component.clone()),
            Self::PathTooLong { relative_path } => {
                (relative_path.clone(), 4, PathBuf::new(), String::new())
            }
            Self::DestinationReservedName { path, component } => {
                (PathBuf::new(), 0, path.clone(), component.clone())
            }
            Self::DestinationComponentTooLong { path, component } => {
                (PathBuf::new(), 1, path.clone(), component.clone())
            }
            Self::DestinationPathTooLong { path } => {
                (PathBuf::new(), 2, path.clone(), String::new())
            }
            Self::ExistingTarget { relative_path } => {
                (relative_path.clone(), 5, PathBuf::new(), String::new())
            }
            Self::ConflictingTarget { relative_path, .. } => {
                (relative_path.clone(), 6, PathBuf::new(), String::new())
            }
            Self::ConflictingAncestor {
                relative_path,
                ancestor,
                ..
            } => (relative_path.clone(), 7, ancestor.clone(), String::new()),
            Self::ExistingDestinationRoot { path } => {
                (PathBuf::new(), 8, path.clone(), String::new())
            }
            Self::ConflictingDestinationRoot { path, .. } => {
                (PathBuf::new(), 9, path.clone(), String::new())
            }
            Self::InspectionFailed { path, .. } => {
                (PathBuf::new(), 10, path.clone(), String::new())
            }
        }
    }
}

/// The concrete cause retained by a failed filesystem plan write.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemWriteErrorCause {
    #[error("filesystem write preflight rejected the destination")]
    Preflight,
    #[error("the platform cannot guarantee {0:?}")]
    UnsupportedPolicy(FilesystemMergePolicy),
    #[error(transparent)]
    Io(#[from] io::Error),
}

macro_rules! workflow_error {
    ($progress:ident, $error:ident) => {
        #[derive(Debug, thiserror::Error)]
        #[error(
            "filesystem write to {destination:?} failed during {phase:?} with {commit_certainty:?} certainty: {source}"
        )]
        pub struct $error {
            destination: Box<Path>,
            policy: FilesystemMergePolicy,
            phase: FilesystemWritePhase,
            failed_target: Option<Box<Path>>,
            staging_residue: Option<Box<Path>>,
            staging_residue_status: StagingResidueStatus,
            commit_certainty: CommitCertainty,
            progress: Box<$progress>,
            preflight_issues: Option<Box<[FilesystemWritePreflightIssue]>>,
            #[source]
            source: Box<FilesystemWriteErrorCause>,
        }

        impl $error {
            pub fn destination(&self) -> &Path {
                &self.destination
            }

            pub const fn policy(&self) -> FilesystemMergePolicy {
                self.policy
            }

            pub const fn phase(&self) -> FilesystemWritePhase {
                self.phase
            }

            pub fn failed_target(&self) -> Option<&Path> {
                self.failed_target.as_deref()
            }

            pub fn staging_residue(&self) -> Option<&Path> {
                self.staging_residue.as_deref()
            }

            pub const fn staging_residue_status(&self) -> StagingResidueStatus {
                self.staging_residue_status
            }

            pub const fn commit_certainty(&self) -> CommitCertainty {
                self.commit_certainty
            }

            pub fn progress(&self) -> &$progress {
                self.progress.as_ref()
            }

            pub fn preflight_issues(&self) -> Option<&[FilesystemWritePreflightIssue]> {
                self.preflight_issues.as_deref()
            }

            pub fn cause(&self) -> &FilesystemWriteErrorCause {
                self.source.as_ref()
            }
        }
    };
}

workflow_error!(PackExtractionWriteProgress, PackExtractionWriteError);
workflow_error!(
    CompilationArtifactWriteProgress,
    CompilationArtifactWriteError
);

struct PlannedFile<'a> {
    relative_path: &'a Path,
    bytes: &'a [u8],
}

#[cfg(fuzzing)]
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct FilesystemWriteFaultProbe {
    pub maximum_write: usize,
    pub write_fault_file: Option<usize>,
    pub write_fault_after: usize,
    pub flush_fault_file: Option<usize>,
    pub commit_fault_file: Option<usize>,
    pub ancestor_symlink_race_file: Option<usize>,
    pub new_tree_commit_unsupported: bool,
    pub new_tree_policy_unsupported: bool,
    pub tree_staging_open_fault: bool,
    pub tree_staging_cleanup_fault: bool,
}

#[derive(Clone, Copy)]
struct WriteFaults {
    maximum_write: usize,
    write_fault_file: Option<usize>,
    write_fault_after: usize,
    flush_fault_file: Option<usize>,
    commit_fault_file: Option<usize>,
    ancestor_symlink_race_file: Option<usize>,
    new_tree_commit_unsupported: bool,
    new_tree_policy_unsupported: bool,
    tree_staging_open_fault: bool,
    tree_staging_cleanup_fault: bool,
}

impl Default for WriteFaults {
    fn default() -> Self {
        Self {
            maximum_write: usize::MAX,
            write_fault_file: None,
            write_fault_after: usize::MAX,
            flush_fault_file: None,
            commit_fault_file: None,
            ancestor_symlink_race_file: None,
            new_tree_commit_unsupported: false,
            new_tree_policy_unsupported: false,
            tree_staging_open_fault: false,
            tree_staging_cleanup_fault: false,
        }
    }
}

#[cfg(fuzzing)]
impl From<FilesystemWriteFaultProbe> for WriteFaults {
    fn from(probe: FilesystemWriteFaultProbe) -> Self {
        Self {
            maximum_write: probe.maximum_write,
            write_fault_file: probe.write_fault_file,
            write_fault_after: probe.write_fault_after,
            flush_fault_file: probe.flush_fault_file,
            commit_fault_file: probe.commit_fault_file,
            ancestor_symlink_race_file: probe.ancestor_symlink_race_file,
            new_tree_commit_unsupported: probe.new_tree_commit_unsupported,
            new_tree_policy_unsupported: probe.new_tree_policy_unsupported,
            tree_staging_open_fault: probe.tree_staging_open_fault,
            tree_staging_cleanup_fault: probe.tree_staging_cleanup_fault,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitScope {
    PlannedFile(usize),
    DestinationRoot,
}

#[derive(Debug)]
struct CoreReceipt {
    completed: Vec<usize>,
}

struct CoreError {
    phase: FilesystemWritePhase,
    failed_target: Option<PathBuf>,
    staging_residue: Option<PathBuf>,
    staging_residue_status: Option<StagingResidueStatus>,
    commit_certainty: CommitCertainty,
    completed: Vec<usize>,
    preflight_issues: Option<Vec<FilesystemWritePreflightIssue>>,
    source: FilesystemWriteErrorCause,
}

/// Writes a Pack Extraction Plan under one explicit filesystem policy.
///
/// [`FilesystemMergePolicy::WriteNewTree`] requires an absent destination,
/// stages the complete plan in a sibling directory, and exposes it through one
/// root commit where supported. Unsupported guarantees are returned as
/// [`FilesystemWriteErrorCause::UnsupportedPolicy`] rather than
/// weakened to a merge. Errors describe visibility and staging residue; the
/// adapter makes no crash-durability guarantee.
pub fn write_pack_extraction_plan_to_filesystem(
    plan: &PackExtractionPlan,
    destination: impl AsRef<Path>,
    policy: FilesystemMergePolicy,
) -> Result<PackExtractionWriteReceipt, PackExtractionWriteError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| PlannedFile {
            relative_path: Path::new(entry.relative_path()),
            bytes: entry.bytes(),
        })
        .collect::<Vec<_>>();
    match write_files(&files, destination, policy) {
        Ok(receipt) => Ok(PackExtractionWriteReceipt::new(
            *plan.pack_identity(),
            pack_extraction_progress(&files, receipt.completed, policy),
        )),
        Err(error) => Err(pack_extraction_error(&files, destination, policy, error)),
    }
}

#[cfg(fuzzing)]
#[doc(hidden)]
pub fn write_pack_extraction_plan_to_filesystem_with_fault_probe(
    plan: &PackExtractionPlan,
    destination: impl AsRef<Path>,
    policy: FilesystemMergePolicy,
    probe: FilesystemWriteFaultProbe,
) -> Result<PackExtractionWriteReceipt, PackExtractionWriteError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| PlannedFile {
            relative_path: Path::new(entry.relative_path()),
            bytes: entry.bytes(),
        })
        .collect::<Vec<_>>();
    match write_files_with_faults(&files, destination, policy, probe.into(), |_, _, _| {}) {
        Ok(receipt) => Ok(PackExtractionWriteReceipt::new(
            *plan.pack_identity(),
            pack_extraction_progress(&files, receipt.completed, policy),
        )),
        Err(error) => Err(pack_extraction_error(&files, destination, policy, error)),
    }
}

/// One independently detectable issue before Compilation Output Artifact write.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationArtifactWriteIssue {
    #[error("a rejected Compilation Result cannot be written")]
    RejectedCompilationResult,
    #[error(
        "the Compilation Result has {artifact_count} artifact(s), but {path_count} output path(s) were supplied"
    )]
    PathCountMismatch {
        artifact_count: usize,
        path_count: usize,
    },
    #[error("caller-selected artifact paths {first_path:?} and {second_path:?} conflict")]
    PathConflict {
        first_path: PathBuf,
        second_path: PathBuf,
    },
}

#[derive(Debug)]
enum CompilationArtifactPathWriteErrorKind {
    Issues(Box<[CompilationArtifactWriteIssue]>),
    Write(CompilationArtifactWriteError),
}

/// A failure while writing Compilation Output Artifacts to caller-selected paths.
#[derive(Debug)]
pub struct CompilationArtifactPathWriteError {
    kind: CompilationArtifactPathWriteErrorKind,
}

impl CompilationArtifactPathWriteError {
    /// Every independently detectable pre-write issue.
    pub fn issues(&self) -> Option<&[CompilationArtifactWriteIssue]> {
        match &self.kind {
            CompilationArtifactPathWriteErrorKind::Issues(issues) => Some(issues),
            _ => None,
        }
    }

    /// The concrete filesystem write failure, when write was attempted.
    pub fn write_error(&self) -> Option<&CompilationArtifactWriteError> {
        match &self.kind {
            CompilationArtifactPathWriteErrorKind::Write(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for CompilationArtifactPathWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CompilationArtifactPathWriteErrorKind::Issues(issues) => {
                if let [issue] = issues.as_ref() {
                    return issue.fmt(formatter);
                }
                write!(
                    formatter,
                    "Compilation Output Artifact write rejected with {} issue(s)",
                    issues.len()
                )?;
                for issue in issues {
                    write!(formatter, ": {issue}")?;
                }
                Ok(())
            }
            CompilationArtifactPathWriteErrorKind::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompilationArtifactPathWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.write_error()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

impl From<CompilationArtifactWriteError> for CompilationArtifactPathWriteError {
    fn from(error: CompilationArtifactWriteError) -> Self {
        Self {
            kind: CompilationArtifactPathWriteErrorKind::Write(error),
        }
    }
}

/// A failure to derive one filesystem write root from output paths.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemWritePathError {
    #[error("cannot resolve the current directory: {source}")]
    CurrentDirectory {
        #[source]
        source: io::Error,
    },
    #[error("cannot resolve output directory `{path}`: {source}")]
    OutputDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("output path `{path}` does not name a file")]
    OutputPathDoesNotNameFile { path: PathBuf },
    #[error("output paths do not share a filesystem root")]
    NoSharedRoot,
}

/// Resolves output paths into one existing filesystem root and relative targets.
///
/// Parent directories are resolved using native filesystem canonicalization,
/// while each output file name may remain absent for later write.
pub fn resolve_filesystem_write_paths(
    targets: &[PathBuf],
) -> Result<(PathBuf, Vec<PathBuf>), FilesystemWritePathError> {
    if targets.is_empty() {
        let current = Path::new(".")
            .canonicalize()
            .map_err(|source| FilesystemWritePathError::CurrentDirectory { source })?;
        return Ok((current, Vec::new()));
    }
    let resolved_targets = targets
        .iter()
        .map(|target| {
            let parent = target.parent().unwrap_or_else(|| Path::new("."));
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            let parent = parent.canonicalize().map_err(|source| {
                FilesystemWritePathError::OutputDirectory {
                    path: parent.to_owned(),
                    source,
                }
            })?;
            let file_name = target.file_name().ok_or_else(|| {
                FilesystemWritePathError::OutputPathDoesNotNameFile {
                    path: target.to_owned(),
                }
            })?;
            Ok(parent.join(file_name))
        })
        .collect::<Result<Vec<PathBuf>, FilesystemWritePathError>>()?;
    let mut destination = resolved_targets[0]
        .parent()
        .expect("a resolved output file has a parent")
        .to_owned();
    while resolved_targets
        .iter()
        .any(|target| !target.starts_with(&destination))
    {
        if !destination.pop() {
            return Err(FilesystemWritePathError::NoSharedRoot);
        }
    }
    let relative_paths = resolved_targets
        .iter()
        .map(|target| {
            target
                .strip_prefix(&destination)
                .expect("the selected destination is a common path prefix")
                .to_owned()
        })
        .collect();
    Ok((destination, relative_paths))
}

/// Writes a succeeded Compilation Result through caller-selected
/// destination-relative filesystem paths.
///
/// This concrete adapter supports platform paths used by CLI output templates
/// while consuming artifact roles, canonical order, and exact bytes directly
/// from the Compilation Result.
pub fn write_compilation_artifacts_to_filesystem_paths(
    result: &CompilationResult,
    destination: impl AsRef<Path>,
    relative_paths: &[PathBuf],
    policy: FilesystemMergePolicy,
) -> Result<CompilationArtifactWriteReceipt, CompilationArtifactPathWriteError> {
    let destination = destination.as_ref();
    let mut issues = Vec::new();
    if result.status() != CompilationStatus::Succeeded {
        issues.push(CompilationArtifactWriteIssue::RejectedCompilationResult);
    }
    if result.artifacts().len() != relative_paths.len() {
        issues.push(CompilationArtifactWriteIssue::PathCountMismatch {
            artifact_count: result.artifacts().len(),
            path_count: relative_paths.len(),
        });
    }
    let mut ordered_paths = relative_paths.iter().collect::<Vec<_>>();
    ordered_paths.sort();
    for (index, first) in ordered_paths.iter().enumerate() {
        for second in &ordered_paths[index + 1..] {
            if second.starts_with(first) {
                issues.push(CompilationArtifactWriteIssue::PathConflict {
                    first_path: (*first).to_owned(),
                    second_path: (*second).to_owned(),
                });
            }
        }
    }
    if !issues.is_empty() {
        return Err(CompilationArtifactPathWriteError {
            kind: CompilationArtifactPathWriteErrorKind::Issues(issues.into_boxed_slice()),
        });
    }
    let files = result
        .artifacts()
        .iter()
        .zip(relative_paths)
        .map(|(artifact, relative_path)| PlannedFile {
            relative_path,
            bytes: artifact.bytes(),
        })
        .collect::<Vec<_>>();
    write_compilation_artifact_files(result, &files, destination, policy).map_err(Into::into)
}

fn write_compilation_artifact_files(
    result: &CompilationResult,
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
) -> Result<CompilationArtifactWriteReceipt, CompilationArtifactWriteError> {
    match write_files(files, destination, policy) {
        Ok(receipt) => Ok(CompilationArtifactWriteReceipt::new(
            result.result_identity(),
            compilation_artifact_progress(receipt.completed, policy),
        )),
        Err(error) => Err(compilation_artifact_error(destination, policy, error)),
    }
}

fn pack_extraction_error(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
    error: CoreError,
) -> PackExtractionWriteError {
    let staging_residue_status = error
        .staging_residue_status
        .unwrap_or_else(|| residue_status(error.staging_residue.as_deref()));
    let staging_residue = retained_residue(error.staging_residue, staging_residue_status)
        .map(PathBuf::into_boxed_path);
    PackExtractionWriteError {
        destination: destination.into(),
        policy,
        phase: error.phase,
        failed_target: error.failed_target.map(PathBuf::into_boxed_path),
        staging_residue: staging_residue.clone(),
        staging_residue_status,
        commit_certainty: error.commit_certainty,
        progress: Box::new(pack_extraction_progress(files, error.completed, policy)),
        preflight_issues: error.preflight_issues.map(Vec::into_boxed_slice),
        source: Box::new(error.source),
    }
}

fn compilation_artifact_error(
    destination: &Path,
    policy: FilesystemMergePolicy,
    error: CoreError,
) -> CompilationArtifactWriteError {
    let staging_residue_status = error
        .staging_residue_status
        .unwrap_or_else(|| residue_status(error.staging_residue.as_deref()));
    let staging_residue = retained_residue(error.staging_residue, staging_residue_status)
        .map(PathBuf::into_boxed_path);
    CompilationArtifactWriteError {
        destination: destination.into(),
        policy,
        phase: error.phase,
        failed_target: error.failed_target.map(PathBuf::into_boxed_path),
        staging_residue: staging_residue.clone(),
        staging_residue_status,
        commit_certainty: error.commit_certainty,
        progress: Box::new(compilation_artifact_progress(error.completed, policy)),
        preflight_issues: error.preflight_issues.map(Vec::into_boxed_slice),
        source: Box::new(error.source),
    }
}

fn pack_extraction_progress(
    files: &[PlannedFile<'_>],
    completed: Vec<usize>,
    policy: FilesystemMergePolicy,
) -> PackExtractionWriteProgress {
    let outcome = filesystem_outcome(policy);
    PackExtractionWriteProgress::from_completed(
        completed
            .into_iter()
            .map(|index| {
                PackExtractionWriteEntry::new(
                    files[index]
                        .relative_path
                        .to_str()
                        .expect("Pack Extraction paths are UTF-8")
                        .to_owned(),
                    outcome,
                )
            })
            .collect(),
    )
}

fn compilation_artifact_progress(
    completed: Vec<usize>,
    policy: FilesystemMergePolicy,
) -> CompilationArtifactWriteProgress {
    let outcome = filesystem_outcome(policy);
    CompilationArtifactWriteProgress::from_completed(
        completed
            .into_iter()
            .map(|index| CompilationArtifactWriteEntry::new(index, outcome))
            .collect(),
    )
}

const fn filesystem_outcome(policy: FilesystemMergePolicy) -> WriteKeyOutcome {
    match policy {
        FilesystemMergePolicy::WriteNewTree | FilesystemMergePolicy::MergeCreateOnly => {
            WriteKeyOutcome::Created
        }
        FilesystemMergePolicy::MergeReplaceExactFiles => WriteKeyOutcome::Written,
    }
}

#[allow(clippy::result_large_err)]
fn write_files(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
) -> Result<CoreReceipt, CoreError> {
    write_files_before_commit(files, destination, policy, |_, _, _| {})
}

#[allow(clippy::result_large_err)]
fn write_files_before_commit(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
    before_commit: impl FnMut(CommitScope, &Path, &Path),
) -> Result<CoreReceipt, CoreError> {
    write_files_with_faults(
        files,
        destination,
        policy,
        WriteFaults::default(),
        before_commit,
    )
}

#[allow(clippy::result_large_err)]
fn write_files_with_faults(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
    faults: WriteFaults,
    mut before_commit: impl FnMut(CommitScope, &Path, &Path),
) -> Result<CoreReceipt, CoreError> {
    if !merge_policy_supported(policy) {
        return Err(CoreError {
            phase: FilesystemWritePhase::Policy,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            completed: Vec::new(),
            preflight_issues: None,
            source: FilesystemWriteErrorCause::UnsupportedPolicy(policy),
        });
    }
    if policy == FilesystemMergePolicy::WriteNewTree && faults.new_tree_policy_unsupported {
        return Err(CoreError {
            phase: FilesystemWritePhase::Policy,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            completed: Vec::new(),
            preflight_issues: None,
            source: FilesystemWriteErrorCause::UnsupportedPolicy(policy),
        });
    }

    let destination_anchor = DestinationAnchor::capture(destination);
    let mut issues = preflight(files, destination, policy);
    let destination_anchor = match destination_anchor {
        Ok(anchor) => Some(anchor),
        Err(source) => {
            issues.push(FilesystemWritePreflightIssue::InspectionFailed {
                path: destination.to_owned(),
                source: Arc::new(source),
            });
            None
        }
    };
    issues.sort_by_key(FilesystemWritePreflightIssue::sort_key);
    issues.dedup_by(|left, right| left.sort_key() == right.sort_key());
    if !issues.is_empty() {
        return Err(CoreError {
            phase: FilesystemWritePhase::Preflight,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            completed: Vec::new(),
            preflight_issues: Some(issues),
            source: FilesystemWriteErrorCause::Preflight,
        });
    }

    let destination_anchor =
        destination_anchor.expect("successful destination capture accompanies clean preflight");
    if !merge_policy_supported_for_plan(&destination_anchor, files, policy) {
        return Err(CoreError {
            phase: FilesystemWritePhase::Policy,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            completed: Vec::new(),
            preflight_issues: None,
            source: FilesystemWriteErrorCause::UnsupportedPolicy(policy),
        });
    }
    if policy == FilesystemMergePolicy::WriteNewTree {
        return write_new_tree(
            files,
            destination,
            destination_anchor,
            faults,
            before_commit,
        );
    }
    let destination_dir = prepare_destination(destination_anchor).map_err(|source| {
        io_core_error(
            FilesystemWritePhase::DirectoryCreate,
            Some(destination.to_owned()),
            None,
            CommitCertainty::NotCommitted,
            Vec::new(),
            source,
        )
    })?;

    let mut completed = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let relative = Path::new(file.relative_path);
        let target = destination.join(relative);
        let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
        let target_name = relative
            .file_name()
            .expect("a canonical planned file has a file name");
        let parent = match open_or_create_directory(&destination_dir, parent_relative) {
            Ok(parent) => parent,
            Err(source) => {
                return Err(io_core_error(
                    FilesystemWritePhase::DirectoryCreate,
                    Some(target),
                    None,
                    CommitCertainty::NotCommitted,
                    completed,
                    source,
                ));
            }
        };

        let (staging, staging_name, mut writer) = match create_staging(&parent, &target) {
            Ok(staging) => staging,
            Err(source) => {
                return Err(io_core_error(
                    FilesystemWritePhase::StagingCreate,
                    Some(target),
                    None,
                    CommitCertainty::NotCommitted,
                    completed,
                    source,
                ));
            }
        };
        let write_result = {
            let mut fault_writer = FaultInjectingWriter::new(&mut writer, index, faults);
            write_staging(&mut fault_writer, file.bytes)
        };
        if let Err((phase, source)) = write_result {
            return Err(staging_core_error(
                &parent,
                &writer,
                &staging_name,
                io_core_error(
                    phase,
                    Some(target),
                    Some(staging),
                    CommitCertainty::NotCommitted,
                    completed,
                    source,
                ),
            ));
        }
        before_commit(CommitScope::PlannedFile(index), &target, &staging);
        #[cfg(unix)]
        if faults.ancestor_symlink_race_file == Some(index) && target.parent() != Some(destination)
        {
            use std::os::unix::fs::symlink;

            let target_parent = target.parent().expect("a target has a parent");
            let displaced = destination.join(format!(".typst-pack-race-{index}"));
            let outside = destination
                .parent()
                .expect("a destination has a parent")
                .join(format!(".typst-pack-outside-{index}"));
            let _ = std::fs::create_dir(&outside);
            if std::fs::rename(target_parent, &displaced).is_ok() {
                let _ = symlink(&outside, target_parent);
            }
        }
        if faults.commit_fault_file == Some(index) {
            let _ = parent.remove_file(&staging_name);
        }
        if let Err(source) =
            validate_directory_binding(&parent, target.parent().expect("a target has a parent"))
        {
            return Err(staging_core_error(
                &parent,
                &writer,
                &staging_name,
                io_core_error(
                    FilesystemWritePhase::Commit,
                    Some(target),
                    Some(staging),
                    CommitCertainty::NotCommitted,
                    completed,
                    source,
                ),
            ));
        }
        if let Err(source) = validate_commit_target(&parent, target_name, policy) {
            return Err(staging_core_error(
                &parent,
                &writer,
                &staging_name,
                io_core_error(
                    FilesystemWritePhase::Commit,
                    Some(target),
                    Some(staging),
                    CommitCertainty::NotCommitted,
                    completed,
                    source,
                ),
            ));
        }
        if let Err(error) = commit_staging(&parent, &writer, &staging_name, target_name, policy) {
            return Err(staging_core_error(
                &parent,
                &writer,
                &staging_name,
                io_core_error(
                    error.phase,
                    Some(target),
                    Some(staging),
                    error.commit_certainty,
                    completed,
                    error.source,
                ),
            ));
        }
        completed.push(index);
        if let Err(source) =
            validate_directory_binding(&parent, target.parent().expect("a target has a parent"))
        {
            return Err(io_core_error(
                FilesystemWritePhase::Commit,
                Some(target),
                None,
                CommitCertainty::Committed,
                completed,
                source,
            ));
        }
    }

    Ok(CoreReceipt { completed })
}

#[allow(clippy::result_large_err)]
fn write_new_tree(
    files: &[PlannedFile<'_>],
    destination: &Path,
    mut anchor: DestinationAnchor,
    faults: WriteFaults,
    mut before_commit: impl FnMut(CommitScope, &Path, &Path),
) -> Result<CoreReceipt, CoreError> {
    let destination_name = anchor.missing.pop().ok_or_else(|| {
        io_core_error(
            FilesystemWritePhase::Commit,
            Some(destination.to_owned()),
            None,
            CommitCertainty::NotCommitted,
            Vec::new(),
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "new-tree destination appeared after preflight",
            ),
        )
    })?;
    for component in &anchor.missing {
        create_and_open_directory(&mut anchor.directory, component).map_err(|source| {
            io_core_error(
                FilesystemWritePhase::DirectoryCreate,
                Some(destination.to_owned()),
                None,
                CommitCertainty::NotCommitted,
                Vec::new(),
                source,
            )
        })?;
    }
    let parent = anchor.directory;
    let absolute_destination = absolute_destination(destination);
    let parent_path = absolute_destination
        .parent()
        .expect("a non-root new-tree destination has a parent");
    let (staging_path, staging_name, staging_root) =
        create_tree_staging(&parent, parent_path, faults).map_err(|error| CoreError {
            phase: error.phase,
            failed_target: Some(destination.to_owned()),
            staging_residue: error.staging_residue,
            staging_residue_status: Some(error.staging_residue_status),
            commit_certainty: CommitCertainty::NotCommitted,
            completed: Vec::new(),
            preflight_issues: None,
            source: FilesystemWriteErrorCause::Io(error.source),
        })?;

    for (index, file) in files.iter().enumerate() {
        let target = destination.join(file.relative_path);
        let relative = Path::new(file.relative_path);
        let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
        let file_name = relative
            .file_name()
            .expect("a canonical planned file has a file name");
        let file_parent =
            open_or_create_directory(&staging_root, parent_relative).map_err(|source| {
                tree_staging_error(
                    &parent,
                    &staging_root,
                    &staging_name,
                    io_core_error(
                        FilesystemWritePhase::DirectoryCreate,
                        Some(target.clone()),
                        Some(staging_path.clone()),
                        CommitCertainty::NotCommitted,
                        Vec::new(),
                        source,
                    ),
                )
            })?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut writer = file_parent
            .open_with(file_name, &options)
            .map_err(|source| {
                tree_staging_error(
                    &parent,
                    &staging_root,
                    &staging_name,
                    io_core_error(
                        FilesystemWritePhase::StagingCreate,
                        Some(target.clone()),
                        Some(staging_path.clone()),
                        CommitCertainty::NotCommitted,
                        Vec::new(),
                        source,
                    ),
                )
            })?;
        let write_result = {
            let mut fault_writer = FaultInjectingWriter::new(&mut writer, index, faults);
            write_staging(&mut fault_writer, file.bytes)
        };
        if let Err((phase, source)) = write_result {
            return Err(tree_staging_error(
                &parent,
                &staging_root,
                &staging_name,
                io_core_error(
                    phase,
                    Some(target),
                    Some(staging_path),
                    CommitCertainty::NotCommitted,
                    Vec::new(),
                    source,
                ),
            ));
        }
    }

    before_commit(CommitScope::DestinationRoot, destination, &staging_path);
    if let Err(source) = validate_directory_binding(&parent, parent_path) {
        return Err(tree_staging_error(
            &parent,
            &staging_root,
            &staging_name,
            io_core_error(
                FilesystemWritePhase::Commit,
                Some(destination.to_owned()),
                Some(staging_path),
                CommitCertainty::NotCommitted,
                Vec::new(),
                source,
            ),
        ));
    }
    if let Err(source) = validate_new_tree_commit_target(&parent, &destination_name) {
        let commit_certainty =
            observe_tree_commit_certainty(&parent, &staging_root, &staging_name, &destination_name);
        let completed = if commit_certainty == CommitCertainty::Committed {
            planned_file_indices(files)
        } else {
            Vec::new()
        };
        return Err(tree_staging_error(
            &parent,
            &staging_root,
            &staging_name,
            io_core_error(
                FilesystemWritePhase::Commit,
                Some(destination.to_owned()),
                Some(staging_path),
                commit_certainty,
                completed,
                source,
            ),
        ));
    }
    let commit_result = if faults.new_tree_commit_unsupported {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "scripted unsupported new-tree commit",
        ))
    } else {
        commit_new_tree(&parent, &staging_root, &staging_name, &destination_name)
    };
    if let Err(source) = commit_result {
        let commit_certainty =
            observe_tree_commit_certainty(&parent, &staging_root, &staging_name, &destination_name);
        let completed = if commit_certainty == CommitCertainty::Committed {
            planned_file_indices(files)
        } else {
            Vec::new()
        };
        let source = if commit_policy_unsupported(&source) {
            FilesystemWriteErrorCause::UnsupportedPolicy(FilesystemMergePolicy::WriteNewTree)
        } else {
            FilesystemWriteErrorCause::Io(source)
        };
        let mut error = CoreError {
            phase: FilesystemWritePhase::Commit,
            failed_target: Some(destination.to_owned()),
            staging_residue: Some(staging_path),
            staging_residue_status: None,
            commit_certainty,
            completed,
            preflight_issues: None,
            source,
        };
        error.staging_residue_status = Some(observe_captured_tree_staging(
            &parent,
            &staging_root,
            &staging_name,
            error.staging_residue.as_deref(),
        ));
        return Err(error);
    }

    let completed = planned_file_indices(files);
    if let Err(source) = validate_directory_binding(&parent, parent_path) {
        return Err(io_core_error(
            FilesystemWritePhase::Commit,
            Some(destination.to_owned()),
            None,
            CommitCertainty::Committed,
            completed,
            source,
        ));
    }

    Ok(CoreReceipt { completed })
}

fn planned_file_indices(files: &[PlannedFile<'_>]) -> Vec<usize> {
    (0..files.len()).collect()
}

fn write_staging(
    writer: &mut impl Write,
    bytes: &[u8],
) -> Result<(), (FilesystemWritePhase, io::Error)> {
    let mut written = 0;
    while written < bytes.len() {
        match writer.write(&bytes[written..]) {
            Ok(0) => {
                return Err((
                    FilesystemWritePhase::StagingWrite,
                    io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write the complete staging file",
                    ),
                ));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err((FilesystemWritePhase::StagingWrite, error));
            }
        }
    }
    writer
        .flush()
        .map_err(|error| (FilesystemWritePhase::StagingFlush, error))
}

struct FaultInjectingWriter<'a, W> {
    writer: &'a mut W,
    file_index: usize,
    faults: WriteFaults,
    written: usize,
}

impl<'a, W> FaultInjectingWriter<'a, W> {
    fn new(writer: &'a mut W, file_index: usize, faults: WriteFaults) -> Self {
        Self {
            writer,
            file_index,
            faults,
            written: 0,
        }
    }
}

impl<W: Write> Write for FaultInjectingWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.faults.write_fault_file == Some(self.file_index)
            && self.written >= self.faults.write_fault_after
        {
            return Err(io::Error::other("scripted staging write fault"));
        }
        let before_fault = if self.faults.write_fault_file == Some(self.file_index) {
            self.faults.write_fault_after.saturating_sub(self.written)
        } else {
            usize::MAX
        };
        let limit = buffer
            .len()
            .min(self.faults.maximum_write)
            .min(before_fault);
        let written = self.writer.write(&buffer[..limit])?;
        self.written += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.faults.flush_fault_file == Some(self.file_index) {
            Err(io::Error::other("scripted staging flush fault"))
        } else {
            self.writer.flush()
        }
    }
}

fn preflight(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
) -> Vec<FilesystemWritePreflightIssue> {
    let mut issues = Vec::new();
    let mut aliases = BTreeMap::<PlatformAliasKey, &Path>::new();
    let case_insensitive = platform_case_insensitive(destination);
    let limits = platform_path_limits(destination);

    inspect_destination_platform_path(destination, limits, &mut issues);
    inspect_destination_root(destination, policy, &mut issues);
    for file in files {
        let relative = Path::new(file.relative_path);
        if !is_canonical_relative_path(relative) {
            issues.push(FilesystemWritePreflightIssue::InvalidRelativePath {
                relative_path: file.relative_path.to_owned(),
            });
            continue;
        }
        inspect_platform_path(file.relative_path, destination, limits, &mut issues);

        let alias = platform_alias_key(file.relative_path, case_insensitive);
        if let Some(first) = aliases.insert(alias, file.relative_path)
            && first != file.relative_path
        {
            issues.push(FilesystemWritePreflightIssue::PathAlias {
                first_path: first.to_owned(),
                second_path: file.relative_path.to_owned(),
            });
        }

        inspect_target(file.relative_path, destination, policy, &mut issues);
    }
    issues.sort_by_key(FilesystemWritePreflightIssue::sort_key);
    issues.dedup_by(|left, right| left.sort_key() == right.sort_key());
    issues
}

fn inspect_destination_platform_path(
    destination: &Path,
    limits: PlatformPathLimits,
    issues: &mut Vec<FilesystemWritePreflightIssue>,
) {
    for component in destination
        .components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component),
            _ => None,
        })
    {
        let rendered = component.to_string_lossy();
        if path_length(Path::new(component)) > limits.component {
            issues.push(FilesystemWritePreflightIssue::DestinationComponentTooLong {
                path: destination.to_owned(),
                component: rendered.clone().into_owned(),
            });
        }
        if is_reserved_component(&rendered) {
            issues.push(FilesystemWritePreflightIssue::DestinationReservedName {
                path: destination.to_owned(),
                component: rendered.into_owned(),
            });
        }
    }
    let absolute = absolute_destination(destination);
    if path_length(&absolute).saturating_sub(limits.path_prefix) > limits.path {
        issues.push(FilesystemWritePreflightIssue::DestinationPathTooLong {
            path: destination.to_owned(),
        });
    }
}

fn inspect_destination_root(
    destination: &Path,
    policy: FilesystemMergePolicy,
    issues: &mut Vec<FilesystemWritePreflightIssue>,
) {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if policy == FilesystemMergePolicy::WriteNewTree {
                issues.push(FilesystemWritePreflightIssue::ExistingDestinationRoot {
                    path: destination.to_owned(),
                });
            }
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                issues.push(FilesystemWritePreflightIssue::ConflictingDestinationRoot {
                    path: destination.to_owned(),
                    kind: entry_kind(&metadata),
                });
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => issues.push(FilesystemWritePreflightIssue::InspectionFailed {
            path: destination.to_owned(),
            source: Arc::new(error),
        }),
    }
}

fn inspect_target(
    relative_path: &Path,
    destination: &Path,
    policy: FilesystemMergePolicy,
    issues: &mut Vec<FilesystemWritePreflightIssue>,
) {
    let target = destination.join(relative_path);
    let mut ancestor = target.parent();
    while let Some(path) = ancestor.filter(|path| path.starts_with(destination)) {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                issues.push(FilesystemWritePreflightIssue::ConflictingAncestor {
                    relative_path: relative_path.to_owned(),
                    ancestor: path.to_owned(),
                    kind: entry_kind(&metadata),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == io::ErrorKind::NotADirectory => {}
            Err(error) => issues.push(FilesystemWritePreflightIssue::InspectionFailed {
                path: path.to_owned(),
                source: Arc::new(error),
            }),
        }
        if path == destination {
            break;
        }
        ancestor = path.parent();
    }

    match std::fs::symlink_metadata(&target) {
        Ok(metadata)
            if policy != FilesystemMergePolicy::MergeReplaceExactFiles && metadata.is_file() =>
        {
            issues.push(FilesystemWritePreflightIssue::ExistingTarget {
                relative_path: relative_path.to_owned(),
            });
        }
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            issues.push(FilesystemWritePreflightIssue::ConflictingTarget {
                relative_path: relative_path.to_owned(),
                kind: entry_kind(&metadata),
            });
        }
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) => {}
        Err(error) => issues.push(FilesystemWritePreflightIssue::InspectionFailed {
            path: target,
            source: Arc::new(error),
        }),
    }
}

fn inspect_platform_path(
    relative_path: &Path,
    destination: &Path,
    limits: PlatformPathLimits,
    issues: &mut Vec<FilesystemWritePreflightIssue>,
) {
    for component in relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component),
            _ => None,
        })
    {
        let rendered_component = component.to_string_lossy();
        if path_length(Path::new(component)) > limits.component {
            issues.push(FilesystemWritePreflightIssue::ComponentTooLong {
                relative_path: relative_path.to_owned(),
                component: rendered_component.clone().into_owned(),
            });
        }
        if is_reserved_component(&rendered_component) {
            issues.push(FilesystemWritePreflightIssue::ReservedName {
                relative_path: relative_path.to_owned(),
                component: rendered_component.into_owned(),
            });
        }
    }
    let target = destination.join(relative_path);
    let target = if target.is_absolute() {
        target
    } else {
        std::env::current_dir()
            .map(|current| current.join(&target))
            .unwrap_or(target)
    };
    if path_length(&target).saturating_sub(limits.path_prefix) > limits.path {
        issues.push(FilesystemWritePreflightIssue::PathTooLong {
            relative_path: relative_path.to_owned(),
        });
    }
}

fn is_canonical_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum PlatformAliasKey {
    Exact(std::ffi::OsString),
    Folded(String),
}

#[cfg(windows)]
fn platform_alias_key(path: &Path, _case_insensitive: bool) -> PlatformAliasKey {
    path.to_str().map_or_else(
        || PlatformAliasKey::Exact(path.as_os_str().to_owned()),
        |path| {
            PlatformAliasKey::Folded(
                path.split(['/', '\\'])
                    .map(|component| component.trim_end_matches([' ', '.']).to_lowercase())
                    .collect::<Vec<_>>()
                    .join("/"),
            )
        },
    )
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_alias_key(path: &Path, case_insensitive: bool) -> PlatformAliasKey {
    use unicode_normalization::UnicodeNormalization;

    path.to_str().map_or_else(
        || PlatformAliasKey::Exact(path.as_os_str().to_owned()),
        |path| {
            let normalized = path.nfd().collect::<String>();
            PlatformAliasKey::Folded(if case_insensitive {
                normalized.to_lowercase()
            } else {
                normalized
            })
        },
    )
}

#[cfg(not(any(windows, target_os = "macos", target_os = "ios")))]
fn platform_alias_key(path: &Path, case_insensitive: bool) -> PlatformAliasKey {
    if !case_insensitive {
        return PlatformAliasKey::Exact(path.as_os_str().to_owned());
    }
    path.to_str().map_or_else(
        || PlatformAliasKey::Exact(path.as_os_str().to_owned()),
        |path| {
            use unicode_normalization::UnicodeNormalization;

            PlatformAliasKey::Folded(path.nfc().collect::<String>().to_lowercase())
        },
    )
}

#[cfg(windows)]
fn platform_case_insensitive(destination: &Path) -> bool {
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_CASE_SENSITIVE_INFO, FileCaseSensitiveInfo, GetFileInformationByHandleEx,
    };

    let Some(directory) = nearest_existing_directory(destination)
        .and_then(|path| open_directory_nofollow(&path).ok())
    else {
        return true;
    };
    let mut info = unsafe { zeroed::<FILE_CASE_SENSITIVE_INFO>() };
    let result = unsafe {
        GetFileInformationByHandleEx(
            directory.as_raw_handle().cast(),
            FileCaseSensitiveInfo,
            std::ptr::addr_of_mut!(info).cast(),
            size_of::<FILE_CASE_SENSITIVE_INFO>() as u32,
        )
    };
    result == 0 || info.Flags & 1 == 0
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_case_insensitive(destination: &Path) -> bool {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;

    const MNT_CASE_SENSITIVE: u32 = 0x0000_0040;
    let absolute = absolute_destination(destination);
    let mut probe = absolute.as_path();
    while !probe.exists() {
        let Some(parent) = probe.parent() else {
            return false;
        };
        probe = parent;
    }
    let Ok(probe) = CString::new(probe.as_os_str().as_bytes()) else {
        return false;
    };
    let mut status = MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(probe.as_ptr(), status.as_mut_ptr()) } != 0 {
        return false;
    }
    let status = unsafe { status.assume_init() };
    status.f_flags as u32 & MNT_CASE_SENSITIVE == 0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_case_insensitive(destination: &Path) -> bool {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    #[cfg(target_pointer_width = "64")]
    const FS_IOC_GETFLAGS: libc::c_ulong = 0x8008_6601;
    #[cfg(target_pointer_width = "32")]
    const FS_IOC_GETFLAGS: libc::c_ulong = 0x8004_6601;
    const FS_CASEFOLD_FL: libc::c_int = 0x4000_0000;
    const MSDOS_SUPER_MAGIC: u64 = 0x4d44;
    const EXFAT_SUPER_MAGIC: u64 = 0x2011_bab0;
    const NTFS_SB_MAGIC: u64 = 0x5346_544e;
    let absolute = absolute_destination(destination);
    let mut probe = absolute.as_path();
    while !probe.exists() {
        let Some(parent) = probe.parent() else {
            return false;
        };
        probe = parent;
    }
    let Ok(directory) = std::fs::File::open(probe) else {
        return false;
    };
    let mut flags = 0 as libc::c_int;
    if unsafe { libc::ioctl(directory.as_raw_fd(), FS_IOC_GETFLAGS, &mut flags) == 0 }
        && flags & FS_CASEFOLD_FL != 0
    {
        return true;
    }
    let Ok(probe) = CString::new(probe.as_os_str().as_bytes()) else {
        return false;
    };
    let mut status = MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(probe.as_ptr(), status.as_mut_ptr()) } != 0 {
        return false;
    }
    matches!(
        unsafe { status.assume_init() }.f_type as u64,
        MSDOS_SUPER_MAGIC | EXFAT_SUPER_MAGIC | NTFS_SB_MAGIC
    )
}

#[cfg(not(any(
    windows,
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
)))]
const fn platform_case_insensitive(_destination: &Path) -> bool {
    false
}

#[derive(Clone, Copy)]
struct PlatformPathLimits {
    component: usize,
    path: usize,
    path_prefix: usize,
}

#[cfg(unix)]
fn platform_path_limits(destination: &Path) -> PlatformPathLimits {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let absolute = absolute_destination(destination);
    let mut probe = absolute.as_path();
    while !probe.exists() {
        let Some(parent) = probe.parent() else {
            break;
        };
        probe = parent;
    }
    let queried = CString::new(probe.as_os_str().as_bytes())
        .ok()
        .map(|probe| {
            let component = unsafe { libc::pathconf(probe.as_ptr(), libc::_PC_NAME_MAX) };
            let path = unsafe { libc::pathconf(probe.as_ptr(), libc::_PC_PATH_MAX) };
            (component, path)
        });
    PlatformPathLimits {
        component: queried
            .filter(|(component, _)| *component > 0)
            .map_or(255, |(component, _)| component as usize),
        path: queried
            .filter(|(_, path)| *path > 0)
            .map_or(libc::PATH_MAX as usize, |(_, path)| path as usize),
        path_prefix: path_length(probe).saturating_add(1),
    }
}

fn absolute_destination(destination: &Path) -> PathBuf {
    if destination.is_absolute() {
        destination.to_owned()
    } else {
        std::env::current_dir()
            .map(|current| current.join(destination))
            .unwrap_or_else(|_| destination.to_owned())
    }
}

#[cfg(windows)]
fn nearest_existing_directory(destination: &Path) -> Option<PathBuf> {
    let absolute = absolute_destination(destination);
    absolute
        .ancestors()
        .find(|path| path.is_dir())
        .map(Path::to_owned)
}

#[cfg(windows)]
fn platform_path_limits(destination: &Path) -> PlatformPathLimits {
    let component = nearest_existing_directory(destination)
        .and_then(|path| open_directory_nofollow(&path).ok())
        .and_then(|directory| windows_volume_information(&directory).ok())
        .map_or(255, |(component, _)| component as usize);
    PlatformPathLimits {
        component,
        path: 32_767,
        path_prefix: 0,
    }
}

#[cfg(windows)]
fn windows_volume_information(directory: &std::fs::File) -> io::Result<(u32, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationByHandleW;

    let mut component = 0;
    let mut flags = 0;
    let result = unsafe {
        GetVolumeInformationByHandleW(
            directory.as_raw_handle().cast(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut component,
            &mut flags,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        Ok((component, flags))
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
const fn platform_path_limits(_destination: &Path) -> PlatformPathLimits {
    PlatformPathLimits {
        component: 255,
        path: usize::MAX,
        path_prefix: 0,
    }
}

#[cfg(windows)]
fn is_reserved_component(component: &str) -> bool {
    if component.ends_with([' ', '.'])
        || component
            .encode_utf16()
            .any(|unit| unit < 32 || matches!(unit, 34 | 42 | 47 | 58 | 60 | 62 | 63 | 92 | 124))
    {
        return true;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

#[cfg(not(windows))]
const fn is_reserved_component(_component: &str) -> bool {
    false
}

#[cfg(windows)]
fn path_length(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().count()
}

#[cfg(not(windows))]
fn path_length(path: &Path) -> usize {
    path.as_os_str().len()
}

fn entry_kind(metadata: &std::fs::Metadata) -> FilesystemDestinationEntryKind {
    if metadata.file_type().is_symlink() {
        FilesystemDestinationEntryKind::Symlink
    } else if metadata.is_file() {
        FilesystemDestinationEntryKind::File
    } else if metadata.is_dir() {
        FilesystemDestinationEntryKind::Directory
    } else {
        FilesystemDestinationEntryKind::Other
    }
}

struct DestinationAnchor {
    directory: Dir,
    missing: Vec<std::ffi::OsString>,
}

impl DestinationAnchor {
    fn capture(destination: &Path) -> io::Result<Self> {
        let absolute = absolute_destination(destination);
        let root = absolute.ancestors().last().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination has no filesystem root",
            )
        })?;
        let relative = absolute.strip_prefix(root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is not beneath its filesystem root",
            )
        })?;
        let mut directory = Dir::open_ambient_dir(root, ambient_authority())?;
        let mut missing = Vec::new();
        let mut components = relative.components();
        while let Some(component) = components.next() {
            let Component::Normal(component) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "destination contains an unsupported component",
                ));
            };
            match directory.symlink_metadata(component) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    let child = open_child_directory_nofollow(&directory, component)?;
                    let metadata = directory.symlink_metadata(component)?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(io::Error::other(format!(
                            "destination component {component:?} changed while it was opened"
                        )));
                    }
                    directory = child;
                }
                Ok(_) => {
                    return Err(io::Error::other(format!(
                        "destination component {component:?} is not a real directory"
                    )));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    missing.push(component.to_owned());
                    for component in components {
                        let Component::Normal(component) = component else {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "destination contains an unsupported component",
                            ));
                        };
                        missing.push(component.to_owned());
                    }
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Self { directory, missing })
    }
}

#[cfg(windows)]
fn open_directory_nofollow(path: &Path) -> io::Result<std::fs::File> {
    use std::os::windows::fs::{FileTypeExt, OpenOptionsExt};

    const FILE_SHARE_ALL: u32 = 0x7;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_ALL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let file_type = file.metadata()?.file_type();
    if file_type.is_symlink_dir() || !file_type.is_dir() {
        Err(io::Error::other(
            "destination ancestor is not a real directory",
        ))
    } else {
        Ok(file)
    }
}

fn prepare_destination(mut anchor: DestinationAnchor) -> io::Result<Dir> {
    for component in &anchor.missing {
        create_and_open_directory(&mut anchor.directory, component)?;
    }
    Ok(anchor.directory)
}

fn open_or_create_directory(root: &Dir, relative: &Path) -> io::Result<Dir> {
    let mut directory = root.try_clone()?;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "planned directory is not canonical and relative",
            ));
        };
        create_and_open_directory(&mut directory, component)?;
    }
    Ok(directory)
}

fn create_and_open_directory(directory: &mut Dir, component: &std::ffi::OsStr) -> io::Result<()> {
    match directory.create_dir(component) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = directory.symlink_metadata(component)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "destination component {component:?} is not a real directory"
        )));
    }
    let child = open_child_directory_nofollow(directory, component)?;
    let metadata = directory.symlink_metadata(component)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "destination component {component:?} changed while it was opened"
        )));
    }
    *directory = child;
    Ok(())
}

fn open_child_directory_nofollow(directory: &Dir, component: &std::ffi::OsStr) -> io::Result<Dir> {
    let parent = directory.try_clone()?.into_std_file();
    cap_primitives::fs::open_dir_nofollow(&parent, Path::new(component)).map(Dir::from_std_file)
}

fn validate_commit_target(
    parent: &Dir,
    target_name: &std::ffi::OsStr,
    policy: FilesystemMergePolicy,
) -> io::Result<()> {
    match parent.symlink_metadata(target_name) {
        Ok(_) if policy == FilesystemMergePolicy::MergeCreateOnly => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "create-only planned target appeared after preflight",
        )),
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(io::Error::other(
            "planned target changed to a non-file after preflight",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_new_tree_commit_target(parent: &Dir, target_name: &std::ffi::OsStr) -> io::Result<()> {
    match parent.symlink_metadata(target_name) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "new-tree destination appeared after preflight",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_directory_binding(directory: &Dir, ambient_path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(ambient_path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "destination directory binding changed during write",
        ));
    }
    let captured = same_file::Handle::from_file(directory.try_clone()?.into_std_file())?;
    let ambient = same_file::Handle::from_path(ambient_path)?;
    if captured == ambient {
        Ok(())
    } else {
        Err(io::Error::other(
            "destination directory binding changed during write",
        ))
    }
}

fn create_staging(
    parent: &Dir,
    target: &Path,
) -> io::Result<(PathBuf, std::ffi::OsString, cap_std::fs::File)> {
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
    const ATTEMPTS: usize = 128;

    let file_name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    for _ in 0..ATTEMPTS {
        let sequence = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let mut staging_name = file_name.to_os_string();
        staging_name.push(format!(
            ".typst-pack-stage-{}-{sequence}",
            std::process::id()
        ));
        let staging = target
            .parent()
            .expect("a target has a parent")
            .join(&staging_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;

            const DELETE: u32 = 0x0001_0000;
            const GENERIC_WRITE: u32 = 0x4000_0000;
            const FILE_SHARE_READ: u32 = 0x1;
            const FILE_SHARE_WRITE: u32 = 0x2;
            const FILE_SHARE_DELETE: u32 = 0x4;

            options
                .access_mode(GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        match parent.open_with(&staging_name, &options) {
            Ok(file) => return Ok((staging, staging_name, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique same-directory staging file",
    ))
}

struct CreateTreeStagingError {
    phase: FilesystemWritePhase,
    staging_residue: Option<PathBuf>,
    staging_residue_status: StagingResidueStatus,
    source: io::Error,
}

fn create_tree_staging(
    parent: &Dir,
    parent_path: &Path,
    faults: WriteFaults,
) -> Result<(PathBuf, std::ffi::OsString, Dir), CreateTreeStagingError> {
    static NEXT_TREE_STAGE: AtomicU64 = AtomicU64::new(0);
    const ATTEMPTS: usize = 128;

    for _ in 0..ATTEMPTS {
        let sequence = NEXT_TREE_STAGE.fetch_add(1, Ordering::Relaxed);
        let staging_name = std::ffi::OsString::from(format!(
            ".typst-pack-tree-stage-{}-{sequence}",
            std::process::id()
        ));
        match parent.create_dir(&staging_name) {
            Ok(()) => {
                let staging_path = parent_path.join(&staging_name);
                let open_result = if faults.tree_staging_open_fault {
                    Err(io::Error::other("scripted tree staging open fault"))
                } else {
                    open_tree_staging_directory(parent, &staging_name)
                };
                match open_result {
                    Ok(staging_root) => {
                        return Ok((staging_path, staging_name, staging_root));
                    }
                    Err(source) => match if faults.tree_staging_cleanup_fault {
                        Err(io::Error::other("scripted tree staging cleanup fault"))
                    } else {
                        parent.remove_dir(&staging_name)
                    } {
                        Ok(()) => {
                            return Err(CreateTreeStagingError {
                                phase: FilesystemWritePhase::StagingCreate,
                                staging_residue: None,
                                staging_residue_status: StagingResidueStatus::Absent,
                                source,
                            });
                        }
                        Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => {
                            return Err(CreateTreeStagingError {
                                phase: FilesystemWritePhase::StagingCreate,
                                staging_residue: None,
                                staging_residue_status: StagingResidueStatus::Absent,
                                source,
                            });
                        }
                        Err(cleanup) => {
                            return Err(CreateTreeStagingError {
                                phase: FilesystemWritePhase::StagingCleanup,
                                staging_residue: Some(staging_path),
                                staging_residue_status: StagingResidueStatus::Indeterminate,
                                source: cleanup,
                            });
                        }
                    },
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(CreateTreeStagingError {
                    phase: FilesystemWritePhase::StagingCreate,
                    staging_residue: None,
                    staging_residue_status: StagingResidueStatus::Absent,
                    source,
                });
            }
        }
    }
    Err(CreateTreeStagingError {
        phase: FilesystemWritePhase::StagingCreate,
        staging_residue: None,
        staging_residue_status: StagingResidueStatus::Absent,
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique sibling staging directory",
        ),
    })
}

#[cfg(not(windows))]
fn open_tree_staging_directory(parent: &Dir, name: &std::ffi::OsStr) -> io::Result<Dir> {
    open_child_directory_nofollow(parent, name)
}

#[cfg(windows)]
fn open_tree_staging_directory(parent: &Dir, name: &std::ffi::OsStr) -> io::Result<Dir> {
    use cap_std::fs::OpenOptionsExt;
    use std::os::windows::fs::FileTypeExt;

    const DELETE: u32 = 0x0001_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode(GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    let parent = parent.try_clone()?.into_std_file();
    let directory = cap_primitives::fs::open(&parent, Path::new(name), &options)?;
    let file_type = directory.metadata()?.file_type();
    if file_type.is_symlink_dir() || !file_type.is_dir() {
        Err(io::Error::other(
            "tree staging entry is not a real directory",
        ))
    } else {
        Ok(Dir::from_std_file(directory))
    }
}

struct CommitStagingError {
    phase: FilesystemWritePhase,
    commit_certainty: CommitCertainty,
    source: io::Error,
}

fn commit_staging(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
    policy: FilesystemMergePolicy,
) -> Result<(), CommitStagingError> {
    let result = match policy {
        FilesystemMergePolicy::WriteNewTree => {
            unreachable!("new-tree write commits a staging directory")
        }
        FilesystemMergePolicy::MergeCreateOnly => {
            commit_create_only(parent, staging_file, staging_name, target_name)
        }
        FilesystemMergePolicy::MergeReplaceExactFiles => {
            commit_replace_exact(parent, staging_file, staging_name, target_name)
        }
    };
    if let Err(source) = result {
        return Err(CommitStagingError {
            phase: FilesystemWritePhase::Commit,
            commit_certainty: observe_commit_certainty(
                parent,
                staging_file,
                staging_name,
                target_name,
            ),
            source,
        });
    }

    Ok(())
}

const fn merge_policy_supported(policy: FilesystemMergePolicy) -> bool {
    match policy {
        FilesystemMergePolicy::WriteNewTree | FilesystemMergePolicy::MergeCreateOnly => {
            cfg!(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                windows
            ))
        }
        FilesystemMergePolicy::MergeReplaceExactFiles => true,
    }
}

fn merge_policy_supported_for_plan(
    anchor: &DestinationAnchor,
    files: &[PlannedFile<'_>],
    policy: FilesystemMergePolicy,
) -> bool {
    if !merge_policy_supported_on(&anchor.directory, policy) {
        return false;
    }
    if !anchor.missing.is_empty() {
        return true;
    }
    for file in files {
        let mut directory = match anchor.directory.try_clone() {
            Ok(directory) => directory,
            Err(_) => return false,
        };
        for component in Path::new(file.relative_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .components()
        {
            let Component::Normal(component) = component else {
                return false;
            };
            match directory.symlink_metadata(component) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    directory = match open_child_directory_nofollow(&directory, component) {
                        Ok(directory) => directory,
                        Err(_) => return false,
                    };
                    if !merge_policy_supported_on(&directory, policy) {
                        return false;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                _ => return false,
            }
        }
    }
    true
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn merge_policy_supported_on(parent: &Dir, policy: FilesystemMergePolicy) -> bool {
    use std::os::fd::AsRawFd;

    if policy == FilesystemMergePolicy::MergeReplaceExactFiles {
        return true;
    }
    const EXT4_SUPER_MAGIC: u64 = 0xef53;
    const XFS_SUPER_MAGIC: u64 = 0x5846_5342;
    const BTRFS_SUPER_MAGIC: u64 = 0x9123_683e;
    const TMPFS_MAGIC: u64 = 0x0102_1994;
    const OVERLAYFS_SUPER_MAGIC: u64 = 0x794c_7630;
    const F2FS_SUPER_MAGIC: u64 = 0xf2f5_2010;
    const ZFS_SUPER_MAGIC: u64 = 0x2fc1_2fc1;
    const RAMFS_MAGIC: u64 = 0x8584_58f6;
    let mut status = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(parent.as_raw_fd(), status.as_mut_ptr()) } != 0 {
        return false;
    }
    matches!(
        unsafe { status.assume_init() }.f_type as u64,
        EXT4_SUPER_MAGIC
            | XFS_SUPER_MAGIC
            | BTRFS_SUPER_MAGIC
            | TMPFS_MAGIC
            | OVERLAYFS_SUPER_MAGIC
            | F2FS_SUPER_MAGIC
            | ZFS_SUPER_MAGIC
            | RAMFS_MAGIC
    )
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn merge_policy_supported_on(parent: &Dir, policy: FilesystemMergePolicy) -> bool {
    use std::os::fd::AsRawFd;

    if policy == FilesystemMergePolicy::MergeReplaceExactFiles {
        return true;
    }
    let mut status = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(parent.as_raw_fd(), status.as_mut_ptr()) } != 0 {
        return false;
    }
    let status = unsafe { status.assume_init() };
    let name = status
        .f_fstypename
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .map(|byte| byte as u8)
        .collect::<Vec<_>>();
    matches!(name.as_slice(), b"apfs" | b"hfs")
}

#[cfg(windows)]
fn merge_policy_supported_on(parent: &Dir, _policy: FilesystemMergePolicy) -> bool {
    const FILE_SUPPORTS_POSIX_UNLINK_RENAME: u32 = 0x0000_0400;
    parent
        .try_clone()
        .map(Dir::into_std_file)
        .and_then(|directory| windows_volume_information(&directory))
        .is_ok_and(|(_, flags)| flags & FILE_SUPPORTS_POSIX_UNLINK_RENAME != 0)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
const fn merge_policy_supported_on(_parent: &Dir, policy: FilesystemMergePolicy) -> bool {
    policy == FilesystemMergePolicy::MergeReplaceExactFiles
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn commit_create_only(
    parent: &Dir,
    _staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_create_only_names(parent, staging_name, target_name)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn commit_new_tree(
    parent: &Dir,
    _staging_root: &Dir,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_create_only_names(parent, staging_name, target_name)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn commit_create_only_names(
    parent: &Dir,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let staging_name = CString::new(staging_name.as_bytes())?;
    let target_name = CString::new(target_name.as_bytes())?;
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            parent.as_raw_fd(),
            staging_name.as_ptr(),
            parent.as_raw_fd(),
            target_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn commit_create_only(
    parent: &Dir,
    _staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_create_only_names(parent, staging_name, target_name)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn commit_new_tree(
    parent: &Dir,
    _staging_root: &Dir,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_create_only_names(parent, staging_name, target_name)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn commit_create_only_names(
    parent: &Dir,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let staging_name = CString::new(staging_name.as_bytes())?;
    let target_name = CString::new(target_name.as_bytes())?;
    let result = unsafe {
        libc::renameatx_np(
            parent.as_raw_fd(),
            staging_name.as_ptr(),
            parent.as_raw_fd(),
            target_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn commit_create_only(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    _staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_windows_file(parent, staging_file, target_name, false)
}

#[cfg(windows)]
fn commit_new_tree(
    parent: &Dir,
    staging_root: &Dir,
    _staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    let staging_root = staging_root.try_clone()?.into_std_file();
    commit_windows_handle(parent, staging_root.as_raw_handle(), target_name, false)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn commit_create_only(
    _parent: &Dir,
    _staging_file: &cap_std::fs::File,
    _staging_name: &std::ffi::OsStr,
    _target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic create-only write is unsupported",
    ))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn commit_new_tree(
    _parent: &Dir,
    _staging_root: &Dir,
    _staging_name: &std::ffi::OsStr,
    _target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic new-tree write is unsupported",
    ))
}

#[cfg(not(windows))]
fn commit_replace_exact(
    parent: &Dir,
    _staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    parent.rename(staging_name, parent, target_name)
}

#[cfg(windows)]
fn commit_replace_exact(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    _staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    commit_windows_file(parent, staging_file, target_name, true)
}

#[cfg(windows)]
fn commit_windows_file(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    target_name: &std::ffi::OsStr,
    replace: bool,
) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    commit_windows_handle(parent, staging_file.as_raw_handle(), target_name, replace)
}

#[cfg(windows)]
fn commit_windows_handle(
    _parent: &Dir,
    staging_handle: std::os::windows::io::RawHandle,
    target_name: &std::ffi::OsStr,
    replace: bool,
) -> io::Result<()> {
    use std::mem::{offset_of, size_of};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_RENAME_INFO, FILE_RENAME_INFO_0, FileRenameInfoEx, SetFileInformationByHandle,
    };

    const FILE_RENAME_FLAG_REPLACE_IF_EXISTS: u32 = 0x1;
    const FILE_RENAME_FLAG_POSIX_SEMANTICS: u32 = 0x2;
    let name = target_name.encode_wide().collect::<Vec<_>>();
    let name_bytes = name.len() * size_of::<u16>();
    // FileNameLength excludes the trailing NUL, but the request buffer includes it.
    let bytes = offset_of!(FILE_RENAME_INFO, FileName) + name_bytes + size_of::<u16>();
    let words = bytes.div_ceil(size_of::<usize>());
    let mut buffer = vec![0usize; words];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*info).Anonymous = FILE_RENAME_INFO_0 {
            Flags: FILE_RENAME_FLAG_POSIX_SEMANTICS
                | if replace {
                    FILE_RENAME_FLAG_REPLACE_IF_EXISTS
                } else {
                    0
                },
        };
        // A simple name with no root handle is the documented same-directory form.
        (*info).RootDirectory = std::ptr::null_mut();
        (*info).FileNameLength = u32::try_from(name_bytes)
            .map_err(|_| io::Error::other("destination file name is too long"))?;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
        if SetFileInformationByHandle(
            staging_handle.cast(),
            FileRenameInfoEx,
            info.cast(),
            u32::try_from(bytes).map_err(|_| io::Error::other("rename request is too large"))?,
        ) != 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(unix)]
fn commit_policy_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
        || error.raw_os_error().is_some_and(|code| {
            matches!(code, libc::ENOSYS | libc::EINVAL) || code == libc::EOPNOTSUPP
        })
}

#[cfg(windows)]
fn commit_policy_unsupported(error: &io::Error) -> bool {
    const ERROR_INVALID_FUNCTION: i32 = 1;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const ERROR_INVALID_PARAMETER: i32 = 87;

    error.kind() == io::ErrorKind::Unsupported
        || matches!(
            error.raw_os_error(),
            Some(ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED | ERROR_INVALID_PARAMETER)
        )
}

#[cfg(not(any(unix, windows)))]
fn commit_policy_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
}

fn observe_commit_certainty(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> CommitCertainty {
    let expected = staging_file
        .try_clone()
        .and_then(|file| same_file::Handle::from_file(file.into_std()));
    let stage = parent
        .open(staging_name)
        .and_then(|file| same_file::Handle::from_file(file.into_std()));
    let target = parent
        .open(target_name)
        .and_then(|file| same_file::Handle::from_file(file.into_std()));
    match (expected, stage, target) {
        (Ok(expected), _, Ok(target)) if expected == target => CommitCertainty::Committed,
        (Ok(expected), Ok(stage), _) if expected == stage => CommitCertainty::NotCommitted,
        _ => CommitCertainty::Indeterminate,
    }
}

fn observe_tree_commit_certainty(
    parent: &Dir,
    staging_root: &Dir,
    staging_name: &std::ffi::OsStr,
    target_name: &std::ffi::OsStr,
) -> CommitCertainty {
    let expected = staging_root
        .try_clone()
        .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()));
    let stage = open_tree_staging_directory(parent, staging_name)
        .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()));
    let target = open_tree_staging_directory(parent, target_name)
        .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()));
    match (expected, stage, target) {
        (Ok(expected), _, Ok(target)) if expected == target => CommitCertainty::Committed,
        (Ok(expected), Ok(stage), _) if expected == stage => CommitCertainty::NotCommitted,
        _ => CommitCertainty::Indeterminate,
    }
}

fn io_core_error(
    phase: FilesystemWritePhase,
    failed_target: Option<PathBuf>,
    staging_residue: Option<PathBuf>,
    commit_certainty: CommitCertainty,
    completed: Vec<usize>,
    source: io::Error,
) -> CoreError {
    CoreError {
        phase,
        failed_target,
        staging_residue,
        staging_residue_status: None,
        commit_certainty,
        completed,
        preflight_issues: None,
        source: FilesystemWriteErrorCause::Io(source),
    }
}

fn staging_core_error(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    mut error: CoreError,
) -> CoreError {
    error.staging_residue_status = Some(observe_captured_staging(
        parent,
        staging_file,
        staging_name,
        error.staging_residue.as_deref(),
    ));
    error
}

fn tree_staging_error(
    parent: &Dir,
    staging_root: &Dir,
    staging_name: &std::ffi::OsStr,
    mut error: CoreError,
) -> CoreError {
    error.staging_residue_status = Some(observe_captured_tree_staging(
        parent,
        staging_root,
        staging_name,
        error.staging_residue.as_deref(),
    ));
    error
}

fn observe_captured_staging(
    parent: &Dir,
    staging_file: &cap_std::fs::File,
    staging_name: &std::ffi::OsStr,
    ambient_path: Option<&Path>,
) -> StagingResidueStatus {
    match parent.symlink_metadata(staging_name) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => StagingResidueStatus::Absent,
        Err(_) => StagingResidueStatus::Indeterminate,
        Ok(_) => {
            let Some(ambient_path) = ambient_path else {
                return StagingResidueStatus::Indeterminate;
            };
            let expected = staging_file
                .try_clone()
                .and_then(|file| same_file::Handle::from_file(file.into_std()));
            let captured = parent
                .open(staging_name)
                .and_then(|file| same_file::Handle::from_file(file.into_std()));
            let ambient = same_file::Handle::from_path(ambient_path);
            match (expected, captured, ambient) {
                (Ok(expected), Ok(captured), Ok(ambient))
                    if expected == captured && expected == ambient =>
                {
                    StagingResidueStatus::Present
                }
                _ => StagingResidueStatus::Indeterminate,
            }
        }
    }
}

fn observe_captured_tree_staging(
    parent: &Dir,
    staging_root: &Dir,
    staging_name: &std::ffi::OsStr,
    ambient_path: Option<&Path>,
) -> StagingResidueStatus {
    match parent.symlink_metadata(staging_name) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => StagingResidueStatus::Absent,
        Err(_) => StagingResidueStatus::Indeterminate,
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            let Some(ambient_path) = ambient_path else {
                return StagingResidueStatus::Indeterminate;
            };
            let expected = staging_root
                .try_clone()
                .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()));
            let captured = open_tree_staging_directory(parent, staging_name)
                .and_then(|directory| same_file::Handle::from_file(directory.into_std_file()));
            let ambient = same_file::Handle::from_path(ambient_path);
            match (expected, captured, ambient) {
                (Ok(expected), Ok(captured), Ok(ambient))
                    if expected == captured && expected == ambient =>
                {
                    StagingResidueStatus::Present
                }
                _ => StagingResidueStatus::Indeterminate,
            }
        }
        Ok(_) => StagingResidueStatus::Indeterminate,
    }
}

fn residue_status(staging: Option<&Path>) -> StagingResidueStatus {
    match staging {
        None => StagingResidueStatus::Absent,
        Some(path) => match std::fs::symlink_metadata(path) {
            Ok(_) => StagingResidueStatus::Present,
            Err(error) if error.kind() == io::ErrorKind::NotFound => StagingResidueStatus::Absent,
            Err(_) => StagingResidueStatus::Indeterminate,
        },
    }
}

fn retained_residue(staging: Option<PathBuf>, status: StagingResidueStatus) -> Option<PathBuf> {
    (status != StagingResidueStatus::Absent)
        .then_some(staging)
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(directory: &tempfile::TempDir) -> PathBuf {
        // macOS temp paths can start with `/var`, a symlink to `/private/var`.
        std::fs::canonicalize(directory.path()).unwrap()
    }

    #[test]
    fn staging_handles_short_writes_and_reports_write_and_flush_faults() {
        let mut short = FaultWriter::new(2, None, false);
        write_staging(&mut short, b"complete bytes").unwrap();
        assert_eq!(short.bytes, b"complete bytes");
        assert!(short.flush_called);

        let mut write_fault = FaultWriter::new(3, Some(6), false);
        let (phase, _) = write_staging(&mut write_fault, b"complete bytes").unwrap_err();
        assert_eq!(phase, FilesystemWritePhase::StagingWrite);
        assert_eq!(write_fault.bytes, b"comple");

        let mut flush_fault = FaultWriter::new(usize::MAX, None, true);
        let (phase, _) = write_staging(&mut flush_fault, b"complete bytes").unwrap_err();
        assert_eq!(phase, FilesystemWritePhase::StagingFlush);
        assert_eq!(flush_fault.bytes, b"complete bytes");
    }

    #[test]
    fn later_commit_fault_retains_ordered_committed_file_progress() {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        let files = [
            PlannedFile {
                relative_path: Path::new("a.txt"),
                bytes: b"a",
            },
            PlannedFile {
                relative_path: Path::new("b.txt"),
                bytes: b"b",
            },
        ];

        let core_error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::MergeCreateOnly,
            |scope, _, staging| {
                if scope == CommitScope::PlannedFile(1) {
                    std::fs::remove_file(staging).unwrap();
                }
            },
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::MergeCreateOnly,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(
            error.failed_target(),
            Some(destination.join("b.txt").as_path())
        );
        assert_eq!(error.commit_certainty(), CommitCertainty::Indeterminate);
        assert_eq!(error.progress().completed()[0].relative_path(), "a.txt");
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert!(matches!(
            error.cause(),
            FilesystemWriteErrorCause::Io(source)
                if source.kind() == io::ErrorKind::NotFound
        ));
        assert_eq!(std::fs::read(destination.join("a.txt")).unwrap(), b"a");
        assert!(!destination.join("b.txt").exists());
    }

    #[test]
    fn new_tree_commit_race_reports_when_the_staged_root_was_committed() {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        let files = [PlannedFile {
            relative_path: Path::new("nested/file.txt"),
            bytes: b"complete",
        }];

        let core_error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            |_, target, staging| std::fs::rename(staging, target).unwrap(),
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(error.failed_target(), Some(destination.as_path()));
        assert_eq!(error.commit_certainty(), CommitCertainty::Committed);
        assert_eq!(
            error.progress().completed()[0].relative_path(),
            "nested/file.txt"
        );
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert_eq!(
            std::fs::read(destination.join("nested/file.txt")).unwrap(),
            b"complete"
        );
    }

    #[test]
    fn new_tree_target_race_preserves_the_complete_staging_residue() {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        let files = [PlannedFile {
            relative_path: Path::new("nested/file.txt"),
            bytes: b"complete",
        }];

        let core_error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            |_, target, staging| {
                assert_eq!(staging.parent(), target.parent());
                assert!(!target.exists());
                assert_eq!(
                    std::fs::read(staging.join("nested/file.txt")).unwrap(),
                    b"complete"
                );
                std::fs::create_dir(target).unwrap();
            },
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(error.failed_target(), Some(destination.as_path()));
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(
            error.staging_residue_status(),
            StagingResidueStatus::Present
        );
        let staging = error.staging_residue().unwrap();
        assert_eq!(staging.parent(), destination.parent());
        assert_eq!(
            std::fs::read(staging.join("nested/file.txt")).unwrap(),
            b"complete"
        );
        assert!(std::fs::read_dir(&destination).unwrap().next().is_none());
    }

    #[test]
    fn new_tree_vanished_staging_reports_indeterminate_commit() {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        let files = [PlannedFile {
            relative_path: Path::new("file.txt"),
            bytes: b"complete",
        }];

        let core_error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            |_, _, staging| std::fs::remove_dir_all(staging).unwrap(),
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(error.failed_target(), Some(destination.as_path()));
        assert_eq!(error.commit_certainty(), CommitCertainty::Indeterminate);
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert_eq!(error.staging_residue(), None);
        assert!(!destination.exists());
    }

    #[test]
    fn new_tree_unsupported_commit_remains_typed_without_merge_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let destination = temp_path(&directory).join("written");
        let files = [PlannedFile {
            relative_path: Path::new("file.txt"),
            bytes: b"complete",
        }];

        let core_error = write_files_with_faults(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            WriteFaults {
                new_tree_commit_unsupported: true,
                ..WriteFaults::default()
            },
            |_, _, _| {},
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause(),
            FilesystemWriteErrorCause::UnsupportedPolicy(FilesystemMergePolicy::WriteNewTree)
        ));
        assert_eq!(
            error.staging_residue_status(),
            StagingResidueStatus::Present
        );
        assert!(!destination.exists());
    }

    #[test]
    fn new_tree_unsupported_policy_is_rejected_before_staging() {
        let directory = tempfile::tempdir().unwrap();
        let root = temp_path(&directory);
        let destination = root.join("written");
        let files = [PlannedFile {
            relative_path: Path::new("file.txt"),
            bytes: b"complete",
        }];

        let core_error = write_files_with_faults(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            WriteFaults {
                new_tree_policy_unsupported: true,
                ..WriteFaults::default()
            },
            |_, _, _| {},
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::WriteNewTree,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Policy);
        assert_eq!(error.failed_target(), None);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(matches!(
            error.cause(),
            FilesystemWriteErrorCause::UnsupportedPolicy(FilesystemMergePolicy::WriteNewTree)
        ));
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert!(std::fs::read_dir(root).unwrap().next().is_none());
    }

    #[test]
    fn new_tree_staging_open_fault_reports_cleanup_and_residue_truthfully() {
        for (cleanup_fault, phase, residue_status) in [
            (
                false,
                FilesystemWritePhase::StagingCreate,
                StagingResidueStatus::Absent,
            ),
            (
                true,
                FilesystemWritePhase::StagingCleanup,
                StagingResidueStatus::Indeterminate,
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let destination = temp_path(&directory).join("written");
            let files = [PlannedFile {
                relative_path: Path::new("file.txt"),
                bytes: b"complete",
            }];

            let core_error = write_files_with_faults(
                &files,
                &destination,
                FilesystemMergePolicy::WriteNewTree,
                WriteFaults {
                    tree_staging_open_fault: true,
                    tree_staging_cleanup_fault: cleanup_fault,
                    ..WriteFaults::default()
                },
                |_, _, _| {},
            )
            .unwrap_err();
            let error = pack_extraction_error(
                &files,
                &destination,
                FilesystemMergePolicy::WriteNewTree,
                core_error,
            );

            assert_eq!(error.phase(), phase);
            assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
            assert_eq!(error.staging_residue_status(), residue_status);
            assert_eq!(error.staging_residue().is_some(), cleanup_fault);
            assert!(!destination.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn target_symlink_race_is_rejected_without_writing_through_the_link() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = temp_path(&directory);
        let destination = root.join("written");
        let outside = root.join("outside.txt");
        std::fs::write(&outside, b"outside").unwrap();
        let files = [PlannedFile {
            relative_path: Path::new("target.txt"),
            bytes: b"planned",
        }];

        let error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::MergeReplaceExactFiles,
            |_, target, _| symlink(&outside, target).unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.phase, FilesystemWritePhase::Commit);
        assert_eq!(error.commit_certainty, CommitCertainty::NotCommitted);
        assert!(error.completed.is_empty());
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_race_keeps_staging_confined_and_reports_indeterminate_residue() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = temp_path(&directory);
        let destination = root.join("written");
        let displaced = root.join("displaced");
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let files = [PlannedFile {
            relative_path: Path::new("nested/target.txt"),
            bytes: b"planned",
        }];

        let core_error = write_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::MergeReplaceExactFiles,
            |_, target, _| {
                std::fs::rename(target.parent().unwrap(), &displaced).unwrap();
                symlink(&outside, target.parent().unwrap()).unwrap();
            },
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &files,
            &destination,
            FilesystemMergePolicy::MergeReplaceExactFiles,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemWritePhase::Commit);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(
            error.staging_residue_status(),
            StagingResidueStatus::Indeterminate
        );
        assert!(error.staging_residue().is_some());
        assert!(error.progress().completed().is_empty());
        assert!(!outside.join("target.txt").exists());
        assert!(std::fs::read_dir(&displaced).unwrap().next().is_some());
    }

    struct FaultWriter {
        bytes: Vec<u8>,
        maximum_write: usize,
        fail_after: Option<usize>,
        fail_flush: bool,
        flush_called: bool,
    }

    impl FaultWriter {
        fn new(maximum_write: usize, fail_after: Option<usize>, fail_flush: bool) -> Self {
            Self {
                bytes: Vec::new(),
                maximum_write,
                fail_after,
                fail_flush,
                flush_called: false,
            }
        }
    }

    impl Write for FaultWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.fail_after == Some(self.bytes.len()) {
                return Err(io::Error::other("scripted write fault"));
            }
            let before_failure = self
                .fail_after
                .map_or(usize::MAX, |limit| limit - self.bytes.len());
            let written = buffer.len().min(self.maximum_write).min(before_failure);
            self.bytes.extend_from_slice(&buffer[..written]);
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_called = true;
            if self.fail_flush {
                Err(io::Error::other("scripted flush fault"))
            } else {
                Ok(())
            }
        }
    }
}
