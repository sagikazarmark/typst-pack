//! Extracting a pack back into a directory.

#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};

use crate::{
    FilesystemMergePolicy, FilesystemPublicationPreflightIssue, Pack, PackExtractionPlanError,
    PackExtractionPublicationError, PackExtractionSelection, plan_pack_extraction,
    publish_pack_extraction_plan_to_filesystem,
};

/// Options for [`extract`].
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    /// Also write vendored packages to `packages/<ns>/<name>/<version>/...`.
    pub packages: bool,
    /// Also write embedded fonts to their archive paths (`fonts/...`).
    pub fonts: bool,
    /// Overwrite existing files.
    pub force: bool,
}

/// A summary of an extraction.
#[derive(Debug, Clone, Default)]
pub struct ExtractReport {
    /// Paths written, relative to the target directory.
    pub written: Vec<PathBuf>,
}

/// A failure while extracting a pack.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExtractError {
    Plan(PackExtractionPlanError),
    Publication(PackExtractionPublicationError),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plan(error) => error.fmt(formatter),
            Self::Publication(error) => {
                match error.preflight_issues().and_then(|issues| issues.first()) {
                    Some(FilesystemPublicationPreflightIssue::ExistingTarget { relative_path }) => {
                        write!(
                            formatter,
                            "`{}` already exists (pass force to overwrite)",
                            error.destination().join(relative_path).display()
                        )
                    }
                    Some(FilesystemPublicationPreflightIssue::ConflictingTarget {
                        relative_path,
                        ..
                    }) => write!(
                        formatter,
                        "existing destination entry `{}` conflicts with extraction",
                        error.destination().join(relative_path).display()
                    ),
                    Some(FilesystemPublicationPreflightIssue::ConflictingAncestor {
                        ancestor,
                        ..
                    })
                    | Some(FilesystemPublicationPreflightIssue::ConflictingDestinationRoot {
                        path: ancestor,
                        ..
                    }) => write!(
                        formatter,
                        "existing destination entry `{}` conflicts with extraction",
                        ancestor.display()
                    ),
                    _ => error.fmt(formatter),
                }
            }
        }
    }
}

impl std::error::Error for ExtractError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            Self::Publication(error) => Some(error),
        }
    }
}

impl From<PackExtractionPlanError> for ExtractError {
    fn from(error: PackExtractionPlanError) -> Self {
        Self::Plan(error)
    }
}

/// Writes the project files of a pack into a directory.
///
/// Project files are written directly into `dir` so that the result is a
/// compilable project. With [`packages`](ExtractOptions::packages) and
/// [`fonts`](ExtractOptions::fonts), the vendored packages and embedded fonts
/// are additionally written to `packages/` and `fonts/` subdirectories. The
/// manifest itself is not recreated; it lives only inside the archive.
pub fn extract(
    pack: &Pack,
    dir: &Path,
    options: &ExtractOptions,
) -> Result<ExtractReport, ExtractError> {
    let plan = plan_pack_extraction(
        pack,
        PackExtractionSelection::new(options.packages, options.fonts),
    )?;
    let policy = if options.force {
        FilesystemMergePolicy::MergeReplaceExactFiles
    } else {
        FilesystemMergePolicy::MergeCreateOnly
    };
    let receipt = publish_pack_extraction_plan_to_filesystem(&plan, dir, policy)
        .map_err(ExtractError::Publication)?;
    Ok(ExtractReport {
        written: receipt
            .progress()
            .committed_files()
            .iter()
            .map(PathBuf::from)
            .collect(),
    })
}
