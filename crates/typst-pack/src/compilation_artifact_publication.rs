//! Destination-independent Compilation Output Artifact publication planning.

use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroUsize;

use crate::payload::SharedBytes;
use crate::{CompilationResult, CompilationResultIdentity, CompilationStatus, OutputFormat};

/// One destination-relative entry in a Compilation Output Artifact publication plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactPublicationEntry {
    relative_path: String,
    format: OutputFormat,
    source_page_number: Option<NonZeroUsize>,
    bytes: SharedBytes,
}

impl CompilationArtifactPublicationEntry {
    /// The deterministic slash-separated path relative to a future destination.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The Compilation Output Artifact format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// The one-based physical source page for a Page Format artifact.
    pub fn source_page_number(&self) -> Option<NonZeroUsize> {
        self.source_page_number
    }

    /// The exact payload length.
    pub fn len(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }

    /// The exact immutable payload bytes.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// An owned, immutable, destination-independent artifact publication plan.
///
/// Entries retain canonical Compilation Result order. The plan contains no
/// destination, platform path, conflict policy, or write state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactPublicationPlan {
    result_identity: CompilationResultIdentity,
    entries: Vec<CompilationArtifactPublicationEntry>,
}

impl CompilationArtifactPublicationPlan {
    /// The identity of the succeeded Compilation Result projected by this plan.
    pub fn result_identity(&self) -> &CompilationResultIdentity {
        &self.result_identity
    }

    /// The publication entries in canonical Compilation Result order.
    pub fn entries(&self) -> &[CompilationArtifactPublicationEntry] {
        &self.entries
    }
}

/// One independently detectable issue in artifact publication planning.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ArtifactPublicationPlanIssue {
    /// Compiler or exporter rejection produces no publishable semantic result.
    #[error("a rejected Compilation Result cannot produce an artifact publication plan")]
    RejectedCompilationResult,
    /// Two artifact roles resolved to the same destination-relative name.
    #[error("multiple Compilation Output Artifacts resolve to {relative_path:?}")]
    NamingCollision { relative_path: String },
}

impl ArtifactPublicationPlanIssue {
    fn sort_key(&self) -> (u8, &str) {
        match self {
            Self::RejectedCompilationResult => (0, ""),
            Self::NamingCollision { relative_path } => (1, relative_path),
        }
    }
}

/// A failure while constructing an artifact publication plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactPublicationPlanError {
    issues: Vec<ArtifactPublicationPlanIssue>,
}

impl ArtifactPublicationPlanError {
    /// Every independently detectable issue in canonical order.
    pub fn issues(&self) -> &[ArtifactPublicationPlanIssue] {
        &self.issues
    }
}

impl fmt::Display for ArtifactPublicationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            return issue.fmt(formatter);
        }
        write!(
            formatter,
            "artifact publication planning failed with {} issue(s)",
            self.issues.len()
        )?;
        for issue in &self.issues {
            write!(formatter, ": {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ArtifactPublicationPlanError {}

/// Produces deterministic artifact names before any destination I/O.
pub fn plan_compilation_artifact_publication(
    result: &CompilationResult,
) -> Result<CompilationArtifactPublicationPlan, ArtifactPublicationPlanError> {
    if result.status() != CompilationStatus::Succeeded {
        return Err(ArtifactPublicationPlanError {
            issues: vec![ArtifactPublicationPlanIssue::RejectedCompilationResult],
        });
    }

    let page_width = result
        .source_page_count()
        .map(|count| count.to_string().len());
    let mut paths = BTreeSet::new();
    let mut collisions = BTreeSet::new();
    let mut entries = Vec::with_capacity(result.artifacts().len());

    for artifact in result.artifacts() {
        let relative_path = match artifact.format() {
            OutputFormat::Pdf => "output.pdf".to_owned(),
            OutputFormat::Html => "output.html".to_owned(),
            format @ (OutputFormat::Png | OutputFormat::Svg) => {
                let source_page_number = artifact
                    .source_page_number()
                    .expect("a Page Format artifact has a Source Page Number")
                    .get();
                let width = page_width.expect("a Page Format result has a source-page count");
                format!("page-{source_page_number:0width$}.{}", format.extension())
            }
        };
        if !paths.insert(relative_path.clone()) {
            collisions.insert(relative_path);
            continue;
        }
        entries.push(CompilationArtifactPublicationEntry {
            relative_path,
            format: artifact.format(),
            source_page_number: artifact.source_page_number(),
            bytes: artifact.shared_bytes().clone(),
        });
    }

    if !collisions.is_empty() {
        let mut issues = collisions
            .into_iter()
            .map(|relative_path| ArtifactPublicationPlanIssue::NamingCollision { relative_path })
            .collect::<Vec<_>>();
        issues.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        return Err(ArtifactPublicationPlanError { issues });
    }

    Ok(CompilationArtifactPublicationPlan {
        result_identity: result.result_identity(),
        entries,
    })
}
