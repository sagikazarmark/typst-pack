use crate::CanonicalIdentity;

/// Knowledge about whether one attempted destination effect completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitCertainty {
    NotCommitted,
    Committed,
    Indeterminate,
}

/// The outcome observed for one successfully completed write entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WriteKeyOutcome {
    /// The adapter created an entry under a create-only guarantee.
    Created,
    /// The adapter observed that the destination already contained the exact bytes.
    AlreadyMatching,
    /// The adapter wrote an entry without distinguishing creation from replacement.
    Written,
}

/// One completed entry in Pack Extraction write-plan order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionWriteEntry {
    relative_path: String,
    outcome: WriteKeyOutcome,
}

impl PackExtractionWriteEntry {
    pub(crate) fn new(relative_path: String, outcome: WriteKeyOutcome) -> Self {
        Self {
            relative_path,
            outcome,
        }
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub const fn outcome(&self) -> WriteKeyOutcome {
        self.outcome
    }
}

/// Completed Pack Extraction write entries in write-plan order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackExtractionWriteProgress {
    completed: Vec<PackExtractionWriteEntry>,
}

impl PackExtractionWriteProgress {
    pub const fn new() -> Self {
        Self {
            completed: Vec::new(),
        }
    }

    pub fn completed(&self) -> &[PackExtractionWriteEntry] {
        &self.completed
    }

    #[cfg(feature = "fs")]
    pub(crate) fn from_completed(completed: Vec<PackExtractionWriteEntry>) -> Self {
        Self { completed }
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn clear(&mut self) {
        self.completed.clear();
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn push(&mut self, entry: PackExtractionWriteEntry) {
        self.completed.push(entry);
    }
}

/// Evidence from successful write of one Pack Extraction Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionWriteReceipt {
    pack_identity: CanonicalIdentity,
    progress: PackExtractionWriteProgress,
}

impl PackExtractionWriteReceipt {
    pub(crate) const fn new(
        pack_identity: CanonicalIdentity,
        progress: PackExtractionWriteProgress,
    ) -> Self {
        Self {
            pack_identity,
            progress,
        }
    }

    pub const fn pack_identity(&self) -> CanonicalIdentity {
        self.pack_identity
    }

    pub const fn progress(&self) -> &PackExtractionWriteProgress {
        &self.progress
    }

    pub fn completed(&self) -> &[PackExtractionWriteEntry] {
        self.progress.completed()
    }
}

/// One completed Compilation Output Artifact write entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactWriteEntry {
    artifact_index: usize,
    outcome: WriteKeyOutcome,
}

impl CompilationArtifactWriteEntry {
    pub(crate) const fn new(artifact_index: usize, outcome: WriteKeyOutcome) -> Self {
        Self {
            artifact_index,
            outcome,
        }
    }

    pub const fn artifact_index(&self) -> usize {
        self.artifact_index
    }

    pub const fn outcome(&self) -> WriteKeyOutcome {
        self.outcome
    }
}

/// Completed Compilation Output Artifact entries in canonical artifact order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationArtifactWriteProgress {
    completed: Vec<CompilationArtifactWriteEntry>,
}

impl CompilationArtifactWriteProgress {
    pub const fn new() -> Self {
        Self {
            completed: Vec::new(),
        }
    }

    pub fn completed(&self) -> &[CompilationArtifactWriteEntry] {
        &self.completed
    }

    #[cfg(feature = "fs")]
    pub(crate) fn from_completed(completed: Vec<CompilationArtifactWriteEntry>) -> Self {
        Self { completed }
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn clear(&mut self) {
        self.completed.clear();
    }

    #[cfg(feature = "opendal")]
    pub(crate) fn push(&mut self, entry: CompilationArtifactWriteEntry) {
        self.completed.push(entry);
    }
}

/// Evidence from successful write of one Compilation Result's artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationArtifactWriteReceipt {
    compilation_result_identity: CanonicalIdentity,
    progress: CompilationArtifactWriteProgress,
}

impl CompilationArtifactWriteReceipt {
    pub(crate) const fn new(
        compilation_result_identity: CanonicalIdentity,
        progress: CompilationArtifactWriteProgress,
    ) -> Self {
        Self {
            compilation_result_identity,
            progress,
        }
    }

    pub const fn compilation_result_identity(&self) -> CanonicalIdentity {
        self.compilation_result_identity
    }

    pub const fn progress(&self) -> &CompilationArtifactWriteProgress {
        &self.progress
    }

    pub fn completed(&self) -> &[CompilationArtifactWriteEntry] {
        self.progress.completed()
    }
}
