use crate::CanonicalIdentity;

/// Knowledge about whether one attempted destination effect completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitCertainty {
    NotCommitted,
    Committed,
    Indeterminate,
}

/// The outcome observed for one successfully completed publication entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PublicationKeyOutcome {
    /// The adapter created an entry under a create-only guarantee.
    Created,
    /// The adapter observed that the destination already contained the exact bytes.
    AlreadyMatching,
    /// The adapter wrote an entry without distinguishing creation from replacement.
    Written,
}

/// One completed entry in Pack Extraction publication-plan order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionPublicationEntry {
    relative_path: String,
    outcome: PublicationKeyOutcome,
}

impl PackExtractionPublicationEntry {
    pub(crate) fn new(relative_path: String, outcome: PublicationKeyOutcome) -> Self {
        Self {
            relative_path,
            outcome,
        }
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub const fn outcome(&self) -> PublicationKeyOutcome {
        self.outcome
    }
}

/// Completed Pack Extraction publication entries in publication-plan order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackExtractionPublicationProgress {
    completed: Vec<PackExtractionPublicationEntry>,
}

impl PackExtractionPublicationProgress {
    pub const fn new() -> Self {
        Self {
            completed: Vec::new(),
        }
    }

    pub fn completed(&self) -> &[PackExtractionPublicationEntry] {
        &self.completed
    }

    #[cfg(feature = "fs")]
    pub(crate) fn from_completed(completed: Vec<PackExtractionPublicationEntry>) -> Self {
        Self { completed }
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn clear(&mut self) {
        self.completed.clear();
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn push(&mut self, entry: PackExtractionPublicationEntry) {
        self.completed.push(entry);
    }
}

/// Evidence from successful publication of one Pack Extraction Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionPublicationReceipt {
    pack_identity: CanonicalIdentity,
    progress: PackExtractionPublicationProgress,
}

impl PackExtractionPublicationReceipt {
    pub(crate) const fn new(
        pack_identity: CanonicalIdentity,
        progress: PackExtractionPublicationProgress,
    ) -> Self {
        Self {
            pack_identity,
            progress,
        }
    }

    pub const fn pack_identity(&self) -> CanonicalIdentity {
        self.pack_identity
    }

    pub const fn progress(&self) -> &PackExtractionPublicationProgress {
        &self.progress
    }

    pub fn completed(&self) -> &[PackExtractionPublicationEntry] {
        self.progress.completed()
    }
}

/// One completed Compilation Output Artifact publication entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactPublicationEntry {
    artifact_index: usize,
    outcome: PublicationKeyOutcome,
}

impl CompilationArtifactPublicationEntry {
    pub(crate) const fn new(artifact_index: usize, outcome: PublicationKeyOutcome) -> Self {
        Self {
            artifact_index,
            outcome,
        }
    }

    pub const fn artifact_index(&self) -> usize {
        self.artifact_index
    }

    pub const fn outcome(&self) -> PublicationKeyOutcome {
        self.outcome
    }
}

/// Completed Compilation Output Artifact entries in canonical artifact order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationArtifactPublicationProgress {
    completed: Vec<CompilationArtifactPublicationEntry>,
}

impl CompilationArtifactPublicationProgress {
    pub const fn new() -> Self {
        Self {
            completed: Vec::new(),
        }
    }

    pub fn completed(&self) -> &[CompilationArtifactPublicationEntry] {
        &self.completed
    }

    #[cfg(feature = "fs")]
    pub(crate) fn from_completed(completed: Vec<CompilationArtifactPublicationEntry>) -> Self {
        Self { completed }
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn clear(&mut self) {
        self.completed.clear();
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn push(&mut self, entry: CompilationArtifactPublicationEntry) {
        self.completed.push(entry);
    }
}

/// Evidence from successful publication of one Compilation Result's artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactPublicationReceipt {
    compilation_result_identity: CanonicalIdentity,
    progress: CompilationArtifactPublicationProgress,
}

impl CompilationArtifactPublicationReceipt {
    pub(crate) const fn new(
        compilation_result_identity: CanonicalIdentity,
        progress: CompilationArtifactPublicationProgress,
    ) -> Self {
        Self {
            compilation_result_identity,
            progress,
        }
    }

    pub const fn compilation_result_identity(&self) -> CanonicalIdentity {
        self.compilation_result_identity
    }

    pub const fn progress(&self) -> &CompilationArtifactPublicationProgress {
        &self.progress
    }

    pub fn completed(&self) -> &[CompilationArtifactPublicationEntry] {
        self.progress.completed()
    }
}
