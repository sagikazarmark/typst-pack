//! Concrete filesystem publication for destination-independent plans.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};

use crate::pack_archive::{CommitCertainty, StagingResidueStatus};
use crate::{CompilationArtifactPublicationPlan, PackExtractionPlan};

/// An explicit merge policy for publishing planned files into a directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemMergePolicy {
    /// Create every planned file and reject any existing planned target.
    MergeCreateOnly,
    /// Create missing planned files and atomically replace existing regular files.
    MergeReplaceExactFiles,
}

/// The filesystem phase reached by a plan publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesystemPlanPublicationPhase {
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
pub enum FilesystemPublicationPreflightIssue {
    #[error("planned path {relative_path:?} is not a canonical relative filesystem path")]
    InvalidRelativePath { relative_path: String },
    #[error("planned paths {first_path:?} and {second_path:?} alias on this platform")]
    PathAlias {
        first_path: String,
        second_path: String,
    },
    #[error("planned path {relative_path:?} contains reserved component {component:?}")]
    ReservedName {
        relative_path: String,
        component: String,
    },
    #[error("component {component:?} in planned path {relative_path:?} exceeds the platform limit")]
    ComponentTooLong {
        relative_path: String,
        component: String,
    },
    #[error("planned destination path {relative_path:?} exceeds the platform path limit")]
    PathTooLong { relative_path: String },
    #[error("planned target {relative_path:?} already exists")]
    ExistingTarget { relative_path: String },
    #[error("planned target {relative_path:?} is an existing {kind}, not a regular file")]
    ConflictingTarget {
        relative_path: String,
        kind: FilesystemDestinationEntryKind,
    },
    #[error("ancestor {ancestor:?} of planned target {relative_path:?} is a {kind}")]
    ConflictingAncestor {
        relative_path: String,
        ancestor: PathBuf,
        kind: FilesystemDestinationEntryKind,
    },
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

impl FilesystemPublicationPreflightIssue {
    fn sort_key(&self) -> (String, u8, String) {
        match self {
            Self::InvalidRelativePath { relative_path } => {
                (relative_path.clone(), 0, String::new())
            }
            Self::PathAlias {
                first_path,
                second_path,
            } => (first_path.clone(), 1, second_path.clone()),
            Self::ReservedName {
                relative_path,
                component,
            } => (relative_path.clone(), 2, component.clone()),
            Self::ComponentTooLong {
                relative_path,
                component,
            } => (relative_path.clone(), 3, component.clone()),
            Self::PathTooLong { relative_path } => (relative_path.clone(), 4, String::new()),
            Self::ExistingTarget { relative_path } => (relative_path.clone(), 5, String::new()),
            Self::ConflictingTarget { relative_path, .. } => {
                (relative_path.clone(), 6, String::new())
            }
            Self::ConflictingAncestor {
                relative_path,
                ancestor,
                ..
            } => (
                relative_path.clone(),
                7,
                ancestor.to_string_lossy().into_owned(),
            ),
            Self::ConflictingDestinationRoot { path, .. } | Self::InspectionFailed { path, .. } => {
                (String::new(), 8, path.to_string_lossy().into_owned())
            }
        }
    }
}

/// The concrete cause retained by a failed filesystem plan publication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemPlanPublicationErrorCause {
    #[error("filesystem publication preflight rejected the destination")]
    Preflight,
    #[error("the platform cannot guarantee {0:?}")]
    UnsupportedPolicy(FilesystemMergePolicy),
    #[error(transparent)]
    Io(#[from] io::Error),
}

macro_rules! workflow_evidence {
    ($progress:ident, $receipt:ident, $error:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $progress {
            committed_files: Vec<String>,
            commit_certainty: CommitCertainty,
            staging_residue: Option<Box<Path>>,
            staging_residue_status: StagingResidueStatus,
        }

        impl $progress {
            /// Planned relative paths committed before the attempt returned.
            pub fn committed_files(&self) -> &[String] {
                &self.committed_files
            }

            /// Certainty for the file effect attempted when this progress ended.
            pub const fn commit_certainty(&self) -> CommitCertainty {
                self.commit_certainty
            }

            /// Retry-relevant same-directory staging residue, when observable.
            pub fn staging_residue(&self) -> Option<&Path> {
                self.staging_residue.as_deref()
            }

            /// The observed state of retry-relevant staging residue.
            pub const fn staging_residue_status(&self) -> StagingResidueStatus {
                self.staging_residue_status
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $receipt {
            destination: PathBuf,
            policy: FilesystemMergePolicy,
            progress: $progress,
        }

        impl $receipt {
            pub const fn phase(&self) -> FilesystemPlanPublicationPhase {
                FilesystemPlanPublicationPhase::Complete
            }

            pub fn destination(&self) -> &Path {
                &self.destination
            }

            pub const fn policy(&self) -> FilesystemMergePolicy {
                self.policy
            }

            pub const fn commit_certainty(&self) -> CommitCertainty {
                CommitCertainty::Committed
            }

            pub const fn staging_residue(&self) -> Option<&Path> {
                None
            }

            pub const fn staging_residue_status(&self) -> StagingResidueStatus {
                StagingResidueStatus::Absent
            }

            pub const fn progress(&self) -> &$progress {
                &self.progress
            }
        }

        #[derive(Debug, thiserror::Error)]
        #[error(
            "filesystem publication to {destination:?} failed during {phase:?} with {commit_certainty:?} certainty: {source}"
        )]
        pub struct $error {
            destination: Box<Path>,
            policy: FilesystemMergePolicy,
            phase: FilesystemPlanPublicationPhase,
            failed_target: Option<Box<Path>>,
            staging_residue: Option<Box<Path>>,
            staging_residue_status: StagingResidueStatus,
            commit_certainty: CommitCertainty,
            progress: Box<$progress>,
            preflight_issues: Option<Box<[FilesystemPublicationPreflightIssue]>>,
            #[source]
            source: Box<FilesystemPlanPublicationErrorCause>,
        }

        impl $error {
            pub fn destination(&self) -> &Path {
                &self.destination
            }

            pub const fn policy(&self) -> FilesystemMergePolicy {
                self.policy
            }

            pub const fn phase(&self) -> FilesystemPlanPublicationPhase {
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

            pub fn preflight_issues(&self) -> Option<&[FilesystemPublicationPreflightIssue]> {
                self.preflight_issues.as_deref()
            }

            pub fn cause(&self) -> &FilesystemPlanPublicationErrorCause {
                self.source.as_ref()
            }
        }
    };
}

workflow_evidence!(
    PackExtractionPublicationProgress,
    PackExtractionPublicationReceipt,
    PackExtractionPublicationError
);
workflow_evidence!(
    CompilationArtifactPublicationProgress,
    CompilationArtifactPublicationReceipt,
    CompilationArtifactPublicationError
);

struct PlannedFile<'a> {
    relative_path: &'a str,
    bytes: &'a [u8],
}

#[cfg(fuzzing)]
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct FilesystemPublicationFaultProbe {
    pub maximum_write: usize,
    pub write_fault_file: Option<usize>,
    pub write_fault_after: usize,
    pub flush_fault_file: Option<usize>,
    pub commit_fault_file: Option<usize>,
    pub ancestor_symlink_race_file: Option<usize>,
}

#[derive(Clone, Copy)]
struct PublicationFaults {
    maximum_write: usize,
    write_fault_file: Option<usize>,
    write_fault_after: usize,
    flush_fault_file: Option<usize>,
    commit_fault_file: Option<usize>,
    ancestor_symlink_race_file: Option<usize>,
}

impl Default for PublicationFaults {
    fn default() -> Self {
        Self {
            maximum_write: usize::MAX,
            write_fault_file: None,
            write_fault_after: usize::MAX,
            flush_fault_file: None,
            commit_fault_file: None,
            ancestor_symlink_race_file: None,
        }
    }
}

#[cfg(fuzzing)]
impl From<FilesystemPublicationFaultProbe> for PublicationFaults {
    fn from(probe: FilesystemPublicationFaultProbe) -> Self {
        Self {
            maximum_write: probe.maximum_write,
            write_fault_file: probe.write_fault_file,
            write_fault_after: probe.write_fault_after,
            flush_fault_file: probe.flush_fault_file,
            commit_fault_file: probe.commit_fault_file,
            ancestor_symlink_race_file: probe.ancestor_symlink_race_file,
        }
    }
}

#[derive(Debug)]
struct CoreReceipt {
    committed_files: Vec<String>,
}

struct CoreError {
    phase: FilesystemPlanPublicationPhase,
    failed_target: Option<PathBuf>,
    staging_residue: Option<PathBuf>,
    staging_residue_status: Option<StagingResidueStatus>,
    commit_certainty: CommitCertainty,
    committed_files: Vec<String>,
    preflight_issues: Option<Vec<FilesystemPublicationPreflightIssue>>,
    source: FilesystemPlanPublicationErrorCause,
}

/// Publishes a Pack Extraction Plan under one explicit filesystem merge policy.
pub fn publish_pack_extraction_plan_to_filesystem(
    plan: &PackExtractionPlan,
    destination: impl AsRef<Path>,
    policy: FilesystemMergePolicy,
) -> Result<PackExtractionPublicationReceipt, PackExtractionPublicationError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| PlannedFile {
            relative_path: entry.relative_path(),
            bytes: entry.bytes(),
        })
        .collect::<Vec<_>>();
    match publish_files(&files, destination, policy) {
        Ok(receipt) => Ok(PackExtractionPublicationReceipt {
            destination: destination.to_owned(),
            policy,
            progress: PackExtractionPublicationProgress {
                committed_files: receipt.committed_files,
                commit_certainty: CommitCertainty::Committed,
                staging_residue: None,
                staging_residue_status: StagingResidueStatus::Absent,
            },
        }),
        Err(error) => Err(pack_extraction_error(destination, policy, error)),
    }
}

#[cfg(fuzzing)]
#[doc(hidden)]
pub fn publish_pack_extraction_plan_to_filesystem_with_fault_probe(
    plan: &PackExtractionPlan,
    destination: impl AsRef<Path>,
    policy: FilesystemMergePolicy,
    probe: FilesystemPublicationFaultProbe,
) -> Result<PackExtractionPublicationReceipt, PackExtractionPublicationError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| PlannedFile {
            relative_path: entry.relative_path(),
            bytes: entry.bytes(),
        })
        .collect::<Vec<_>>();
    match publish_files_with_faults(&files, destination, policy, probe.into(), |_, _, _| {}) {
        Ok(receipt) => Ok(PackExtractionPublicationReceipt {
            destination: destination.to_owned(),
            policy,
            progress: PackExtractionPublicationProgress {
                committed_files: receipt.committed_files,
                commit_certainty: CommitCertainty::Committed,
                staging_residue: None,
                staging_residue_status: StagingResidueStatus::Absent,
            },
        }),
        Err(error) => Err(pack_extraction_error(destination, policy, error)),
    }
}

/// Publishes an artifact plan under one explicit filesystem merge policy.
pub fn publish_compilation_artifact_plan_to_filesystem(
    plan: &CompilationArtifactPublicationPlan,
    destination: impl AsRef<Path>,
    policy: FilesystemMergePolicy,
) -> Result<CompilationArtifactPublicationReceipt, CompilationArtifactPublicationError> {
    let destination = destination.as_ref();
    let files = plan
        .entries()
        .iter()
        .map(|entry| PlannedFile {
            relative_path: entry.relative_path(),
            bytes: entry.bytes(),
        })
        .collect::<Vec<_>>();
    match publish_files(&files, destination, policy) {
        Ok(receipt) => Ok(CompilationArtifactPublicationReceipt {
            destination: destination.to_owned(),
            policy,
            progress: CompilationArtifactPublicationProgress {
                committed_files: receipt.committed_files,
                commit_certainty: CommitCertainty::Committed,
                staging_residue: None,
                staging_residue_status: StagingResidueStatus::Absent,
            },
        }),
        Err(error) => Err(compilation_artifact_error(destination, policy, error)),
    }
}

fn pack_extraction_error(
    destination: &Path,
    policy: FilesystemMergePolicy,
    error: CoreError,
) -> PackExtractionPublicationError {
    let staging_residue_status = error
        .staging_residue_status
        .unwrap_or_else(|| residue_status(error.staging_residue.as_deref()));
    let staging_residue = retained_residue(error.staging_residue, staging_residue_status)
        .map(PathBuf::into_boxed_path);
    PackExtractionPublicationError {
        destination: destination.into(),
        policy,
        phase: error.phase,
        failed_target: error.failed_target.map(PathBuf::into_boxed_path),
        staging_residue: staging_residue.clone(),
        staging_residue_status,
        commit_certainty: error.commit_certainty,
        progress: Box::new(PackExtractionPublicationProgress {
            committed_files: error.committed_files,
            commit_certainty: error.commit_certainty,
            staging_residue,
            staging_residue_status,
        }),
        preflight_issues: error.preflight_issues.map(Vec::into_boxed_slice),
        source: Box::new(error.source),
    }
}

fn compilation_artifact_error(
    destination: &Path,
    policy: FilesystemMergePolicy,
    error: CoreError,
) -> CompilationArtifactPublicationError {
    let staging_residue_status = error
        .staging_residue_status
        .unwrap_or_else(|| residue_status(error.staging_residue.as_deref()));
    let staging_residue = retained_residue(error.staging_residue, staging_residue_status)
        .map(PathBuf::into_boxed_path);
    CompilationArtifactPublicationError {
        destination: destination.into(),
        policy,
        phase: error.phase,
        failed_target: error.failed_target.map(PathBuf::into_boxed_path),
        staging_residue: staging_residue.clone(),
        staging_residue_status,
        commit_certainty: error.commit_certainty,
        progress: Box::new(CompilationArtifactPublicationProgress {
            committed_files: error.committed_files,
            commit_certainty: error.commit_certainty,
            staging_residue,
            staging_residue_status,
        }),
        preflight_issues: error.preflight_issues.map(Vec::into_boxed_slice),
        source: Box::new(error.source),
    }
}

fn publish_files(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
) -> Result<CoreReceipt, CoreError> {
    publish_files_before_commit(files, destination, policy, |_, _, _| {})
}

fn publish_files_before_commit(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
    before_commit: impl FnMut(usize, &Path, &Path),
) -> Result<CoreReceipt, CoreError> {
    publish_files_with_faults(
        files,
        destination,
        policy,
        PublicationFaults::default(),
        before_commit,
    )
}

fn publish_files_with_faults(
    files: &[PlannedFile<'_>],
    destination: &Path,
    policy: FilesystemMergePolicy,
    faults: PublicationFaults,
    mut before_commit: impl FnMut(usize, &Path, &Path),
) -> Result<CoreReceipt, CoreError> {
    if !merge_policy_supported(policy) {
        return Err(CoreError {
            phase: FilesystemPlanPublicationPhase::Policy,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            committed_files: Vec::new(),
            preflight_issues: None,
            source: FilesystemPlanPublicationErrorCause::UnsupportedPolicy(policy),
        });
    }

    let destination_anchor = DestinationAnchor::capture(destination);
    let mut issues = preflight(files, destination, policy);
    let destination_anchor = match destination_anchor {
        Ok(anchor) => Some(anchor),
        Err(source) => {
            issues.push(FilesystemPublicationPreflightIssue::InspectionFailed {
                path: destination.to_owned(),
                source: Arc::new(source),
            });
            None
        }
    };
    issues.sort_by_key(FilesystemPublicationPreflightIssue::sort_key);
    issues.dedup_by(|left, right| left.sort_key() == right.sort_key());
    if !issues.is_empty() {
        return Err(CoreError {
            phase: FilesystemPlanPublicationPhase::Preflight,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            committed_files: Vec::new(),
            preflight_issues: Some(issues),
            source: FilesystemPlanPublicationErrorCause::Preflight,
        });
    }

    let destination_anchor =
        destination_anchor.expect("successful destination capture accompanies clean preflight");
    if !merge_policy_supported_for_plan(&destination_anchor, files, policy) {
        return Err(CoreError {
            phase: FilesystemPlanPublicationPhase::Policy,
            failed_target: None,
            staging_residue: None,
            staging_residue_status: None,
            commit_certainty: CommitCertainty::NotCommitted,
            committed_files: Vec::new(),
            preflight_issues: None,
            source: FilesystemPlanPublicationErrorCause::UnsupportedPolicy(policy),
        });
    }
    let destination_dir = prepare_destination(destination_anchor).map_err(|source| {
        io_core_error(
            FilesystemPlanPublicationPhase::DirectoryCreate,
            Some(destination.to_owned()),
            None,
            CommitCertainty::NotCommitted,
            Vec::new(),
            source,
        )
    })?;

    let mut committed_files = Vec::with_capacity(files.len());
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
                    FilesystemPlanPublicationPhase::DirectoryCreate,
                    Some(target),
                    None,
                    CommitCertainty::NotCommitted,
                    committed_files,
                    source,
                ));
            }
        };

        let (staging, staging_name, mut writer) = match create_staging(&parent, &target) {
            Ok(staging) => staging,
            Err(source) => {
                return Err(io_core_error(
                    FilesystemPlanPublicationPhase::StagingCreate,
                    Some(target),
                    None,
                    CommitCertainty::NotCommitted,
                    committed_files,
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
                    committed_files,
                    source,
                ),
            ));
        }
        before_commit(index, &target, &staging);
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
                    FilesystemPlanPublicationPhase::Commit,
                    Some(target),
                    Some(staging),
                    CommitCertainty::NotCommitted,
                    committed_files,
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
                    FilesystemPlanPublicationPhase::Commit,
                    Some(target),
                    Some(staging),
                    CommitCertainty::NotCommitted,
                    committed_files,
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
                    committed_files,
                    error.source,
                ),
            ));
        }
        committed_files.push(file.relative_path.to_owned());
        if let Err(source) =
            validate_directory_binding(&parent, target.parent().expect("a target has a parent"))
        {
            return Err(io_core_error(
                FilesystemPlanPublicationPhase::Commit,
                Some(target),
                None,
                CommitCertainty::Committed,
                committed_files,
                source,
            ));
        }
    }

    Ok(CoreReceipt { committed_files })
}

fn write_staging(
    writer: &mut impl Write,
    bytes: &[u8],
) -> Result<(), (FilesystemPlanPublicationPhase, io::Error)> {
    let mut written = 0;
    while written < bytes.len() {
        match writer.write(&bytes[written..]) {
            Ok(0) => {
                return Err((
                    FilesystemPlanPublicationPhase::StagingWrite,
                    io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write the complete staging file",
                    ),
                ));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err((FilesystemPlanPublicationPhase::StagingWrite, error));
            }
        }
    }
    writer
        .flush()
        .map_err(|error| (FilesystemPlanPublicationPhase::StagingFlush, error))
}

struct FaultInjectingWriter<'a, W> {
    writer: &'a mut W,
    file_index: usize,
    faults: PublicationFaults,
    written: usize,
}

impl<'a, W> FaultInjectingWriter<'a, W> {
    fn new(writer: &'a mut W, file_index: usize, faults: PublicationFaults) -> Self {
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
) -> Vec<FilesystemPublicationPreflightIssue> {
    let mut issues = Vec::new();
    let mut aliases = BTreeMap::<String, &str>::new();
    let case_insensitive = platform_case_insensitive(destination);
    let limits = platform_path_limits(destination);

    inspect_destination_root(destination, &mut issues);
    for file in files {
        let relative = Path::new(file.relative_path);
        if !is_canonical_relative_path(relative) {
            issues.push(FilesystemPublicationPreflightIssue::InvalidRelativePath {
                relative_path: file.relative_path.to_owned(),
            });
            continue;
        }
        inspect_platform_path(file.relative_path, destination, limits, &mut issues);

        let alias = platform_alias_key(file.relative_path, case_insensitive);
        if let Some(first) = aliases.insert(alias, file.relative_path)
            && first != file.relative_path
        {
            issues.push(FilesystemPublicationPreflightIssue::PathAlias {
                first_path: first.to_owned(),
                second_path: file.relative_path.to_owned(),
            });
        }

        inspect_target(file.relative_path, destination, policy, &mut issues);
    }
    issues.sort_by_key(FilesystemPublicationPreflightIssue::sort_key);
    issues.dedup_by(|left, right| left.sort_key() == right.sort_key());
    issues
}

fn inspect_destination_root(
    destination: &Path,
    issues: &mut Vec<FilesystemPublicationPreflightIssue>,
) {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            issues.push(
                FilesystemPublicationPreflightIssue::ConflictingDestinationRoot {
                    path: destination.to_owned(),
                    kind: entry_kind(&metadata),
                },
            );
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => issues.push(FilesystemPublicationPreflightIssue::InspectionFailed {
            path: destination.to_owned(),
            source: Arc::new(error),
        }),
    }
}

fn inspect_target(
    relative_path: &str,
    destination: &Path,
    policy: FilesystemMergePolicy,
    issues: &mut Vec<FilesystemPublicationPreflightIssue>,
) {
    let target = destination.join(Path::new(relative_path));
    let mut ancestor = target.parent();
    while let Some(path) = ancestor.filter(|path| path.starts_with(destination)) {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                issues.push(FilesystemPublicationPreflightIssue::ConflictingAncestor {
                    relative_path: relative_path.to_owned(),
                    ancestor: path.to_owned(),
                    kind: entry_kind(&metadata),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == io::ErrorKind::NotADirectory => {}
            Err(error) => issues.push(FilesystemPublicationPreflightIssue::InspectionFailed {
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
        Ok(metadata) if policy == FilesystemMergePolicy::MergeCreateOnly && metadata.is_file() => {
            issues.push(FilesystemPublicationPreflightIssue::ExistingTarget {
                relative_path: relative_path.to_owned(),
            });
        }
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            issues.push(FilesystemPublicationPreflightIssue::ConflictingTarget {
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
        Err(error) => issues.push(FilesystemPublicationPreflightIssue::InspectionFailed {
            path: target,
            source: Arc::new(error),
        }),
    }
}

fn inspect_platform_path(
    relative_path: &str,
    destination: &Path,
    limits: PlatformPathLimits,
    issues: &mut Vec<FilesystemPublicationPreflightIssue>,
) {
    for component in relative_path.split('/') {
        if component_length(component) > limits.component {
            issues.push(FilesystemPublicationPreflightIssue::ComponentTooLong {
                relative_path: relative_path.to_owned(),
                component: component.to_owned(),
            });
        }
        if is_reserved_component(component) {
            issues.push(FilesystemPublicationPreflightIssue::ReservedName {
                relative_path: relative_path.to_owned(),
                component: component.to_owned(),
            });
        }
    }
    let target = destination.join(Path::new(relative_path));
    let target = if target.is_absolute() {
        target
    } else {
        std::env::current_dir()
            .map(|current| current.join(&target))
            .unwrap_or(target)
    };
    if path_length(&target).saturating_sub(limits.path_prefix) > limits.path {
        issues.push(FilesystemPublicationPreflightIssue::PathTooLong {
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

#[cfg(windows)]
fn platform_alias_key(path: &str, _case_insensitive: bool) -> String {
    path.split('/')
        .map(|component| component.trim_end_matches([' ', '.']).to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_alias_key(path: &str, case_insensitive: bool) -> String {
    use unicode_normalization::UnicodeNormalization;

    let normalized = path.nfd().collect::<String>();
    if case_insensitive {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "ios")))]
fn platform_alias_key(path: &str, case_insensitive: bool) -> String {
    if case_insensitive {
        use unicode_normalization::UnicodeNormalization;

        path.nfc().collect::<String>().to_lowercase()
    } else {
        path.to_owned()
    }
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
fn component_length(component: &str) -> usize {
    component.encode_utf16().count()
}

#[cfg(unix)]
fn component_length(component: &str) -> usize {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::new(component).as_bytes().len()
}

#[cfg(not(any(unix, windows)))]
fn component_length(component: &str) -> usize {
    component.len()
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

fn validate_directory_binding(directory: &Dir, ambient_path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(ambient_path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "destination directory binding changed during publication",
        ));
    }
    let captured = same_file::Handle::from_file(directory.try_clone()?.into_std_file())?;
    let ambient = same_file::Handle::from_path(ambient_path)?;
    if captured == ambient {
        Ok(())
    } else {
        Err(io::Error::other(
            "destination directory binding changed during publication",
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

struct CommitStagingError {
    phase: FilesystemPlanPublicationPhase,
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
        FilesystemMergePolicy::MergeCreateOnly => {
            commit_create_only(parent, staging_file, staging_name, target_name)
        }
        FilesystemMergePolicy::MergeReplaceExactFiles => {
            commit_replace_exact(parent, staging_file, staging_name, target_name)
        }
    };
    if let Err(source) = result {
        return Err(CommitStagingError {
            phase: FilesystemPlanPublicationPhase::Commit,
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
        FilesystemMergePolicy::MergeCreateOnly => cfg!(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            windows
        )),
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
        "atomic create-only publication is unsupported",
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
    use std::mem::{offset_of, size_of};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_RENAME_INFO, FILE_RENAME_INFO_0, FileRenameInfoEx, SetFileInformationByHandle,
    };

    const FILE_RENAME_FLAG_REPLACE_IF_EXISTS: u32 = 0x1;
    const FILE_RENAME_FLAG_POSIX_SEMANTICS: u32 = 0x2;
    let name = target_name.encode_wide().collect::<Vec<_>>();
    let bytes = offset_of!(FILE_RENAME_INFO, FileName) + name.len() * size_of::<u16>();
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
        (*info).RootDirectory = parent.as_raw_handle().cast();
        (*info).FileNameLength = u32::try_from(name.len() * size_of::<u16>())
            .map_err(|_| io::Error::other("destination file name is too long"))?;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
        if SetFileInformationByHandle(
            staging_file.as_raw_handle().cast(),
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

fn io_core_error(
    phase: FilesystemPlanPublicationPhase,
    failed_target: Option<PathBuf>,
    staging_residue: Option<PathBuf>,
    commit_certainty: CommitCertainty,
    committed_files: Vec<String>,
    source: io::Error,
) -> CoreError {
    CoreError {
        phase,
        failed_target,
        staging_residue,
        staging_residue_status: None,
        commit_certainty,
        committed_files,
        preflight_issues: None,
        source: FilesystemPlanPublicationErrorCause::Io(source),
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

    #[test]
    fn staging_handles_short_writes_and_reports_write_and_flush_faults() {
        let mut short = FaultWriter::new(2, None, false);
        write_staging(&mut short, b"complete bytes").unwrap();
        assert_eq!(short.bytes, b"complete bytes");
        assert!(short.flush_called);

        let mut write_fault = FaultWriter::new(3, Some(6), false);
        let (phase, _) = write_staging(&mut write_fault, b"complete bytes").unwrap_err();
        assert_eq!(phase, FilesystemPlanPublicationPhase::StagingWrite);
        assert_eq!(write_fault.bytes, b"comple");

        let mut flush_fault = FaultWriter::new(usize::MAX, None, true);
        let (phase, _) = write_staging(&mut flush_fault, b"complete bytes").unwrap_err();
        assert_eq!(phase, FilesystemPlanPublicationPhase::StagingFlush);
        assert_eq!(flush_fault.bytes, b"complete bytes");
    }

    #[test]
    fn later_commit_fault_retains_ordered_committed_file_progress() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("published");
        let files = [
            PlannedFile {
                relative_path: "a.txt",
                bytes: b"a",
            },
            PlannedFile {
                relative_path: "b.txt",
                bytes: b"b",
            },
        ];

        let core_error = publish_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::MergeCreateOnly,
            |index, _, staging| {
                if index == 1 {
                    std::fs::remove_file(staging).unwrap();
                }
            },
        )
        .unwrap_err();
        let error = pack_extraction_error(
            &destination,
            FilesystemMergePolicy::MergeCreateOnly,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemPlanPublicationPhase::Commit);
        assert_eq!(
            error.failed_target(),
            Some(destination.join("b.txt").as_path())
        );
        assert_eq!(error.commit_certainty(), CommitCertainty::Indeterminate);
        assert_eq!(error.progress().committed_files(), ["a.txt"]);
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert!(matches!(
            error.cause(),
            FilesystemPlanPublicationErrorCause::Io(source)
                if source.kind() == io::ErrorKind::NotFound
        ));
        assert_eq!(std::fs::read(destination.join("a.txt")).unwrap(), b"a");
        assert!(!destination.join("b.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn target_symlink_race_is_rejected_without_writing_through_the_link() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("published");
        let outside = directory.path().join("outside.txt");
        std::fs::write(&outside, b"outside").unwrap();
        let files = [PlannedFile {
            relative_path: "target.txt",
            bytes: b"planned",
        }];

        let error = publish_files_before_commit(
            &files,
            &destination,
            FilesystemMergePolicy::MergeReplaceExactFiles,
            |_, target, _| symlink(&outside, target).unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.phase, FilesystemPlanPublicationPhase::Commit);
        assert_eq!(error.commit_certainty, CommitCertainty::NotCommitted);
        assert!(error.committed_files.is_empty());
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_race_keeps_staging_confined_and_reports_indeterminate_residue() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("published");
        let displaced = directory.path().join("displaced");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let files = [PlannedFile {
            relative_path: "nested/target.txt",
            bytes: b"planned",
        }];

        let core_error = publish_files_before_commit(
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
            &destination,
            FilesystemMergePolicy::MergeReplaceExactFiles,
            core_error,
        );

        assert_eq!(error.phase(), FilesystemPlanPublicationPhase::Commit);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(
            error.staging_residue_status(),
            StagingResidueStatus::Indeterminate
        );
        assert!(error.staging_residue().is_some());
        assert!(error.progress().committed_files().is_empty());
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
