//! Stable Package Read Failure data carried by Pack Assembly.

use std::collections::BTreeMap;

use typst::syntax::package::{PackageSpec, PackageVersion};

/// Package Read Failures keyed by exact package specification.
///
/// Pack Assembly updates this value between Pack Creation invocations. A
/// separately supplied Package Catalog entry always takes precedence during
/// Dependency Discovery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageReadFailures {
    failures: BTreeMap<String, PackageReadFailure>,
}

impl PackageReadFailures {
    /// An empty failure map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the latest failed attempt for one exact specification.
    pub fn insert(&mut self, failure: PackageReadFailure) -> Option<PackageReadFailure> {
        self.failures.insert(failure.spec.to_string(), failure)
    }

    /// Failures in canonical exact-specification order.
    pub fn entries(&self) -> impl Iterator<Item = &PackageReadFailure> {
        self.failures.values()
    }

    /// Looks up the failed attempt for one exact specification.
    pub fn get(&self, spec: &PackageSpec) -> Option<&PackageReadFailure> {
        self.failures.get(&spec.to_string())
    }

    /// Removes an older failed attempt after the specification is read.
    #[cfg(any(feature = "fs", all(feature = "opendal", feature = "package-reading")))]
    pub(crate) fn remove(&mut self, spec: &PackageSpec) -> Option<PackageReadFailure> {
        self.failures.remove(&spec.to_string())
    }
}

impl FromIterator<PackageReadFailure> for PackageReadFailures {
    fn from_iter<T: IntoIterator<Item = PackageReadFailure>>(failures: T) -> Self {
        let mut result = Self::new();
        for failure in failures {
            result.insert(failure);
        }
        result
    }
}

/// An external attempt to read one exact package specification failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("failed to read {spec}: {reason}")]
pub struct PackageReadFailure {
    spec: PackageSpec,
    reason: PackageReadFailureReason,
}

impl PackageReadFailure {
    pub fn new(spec: PackageSpec, reason: PackageReadFailureReason) -> Self {
        Self { spec, reason }
    }

    /// The exact specification the failed attempt tried to read.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The typed operational reason the attempt failed.
    pub fn reason(&self) -> &PackageReadFailureReason {
        &self.reason
    }

    pub fn into_parts(self) -> (PackageSpec, PackageReadFailureReason) {
        (self.spec, self.reason)
    }
}

/// The stable operational reason for a Package Read Failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageReadFailureReason {
    #[error("package not found")]
    NotFound,
    #[error("package version not found; latest available version is {latest}")]
    VersionNotFound { latest: PackageVersion },
    #[error("network request failed{detail}", detail = optional_detail(.detail))]
    NetworkFailed { detail: Option<String> },
    #[error("package archive is malformed{detail}", detail = optional_detail(.detail))]
    MalformedArchive { detail: Option<String> },
    #[error("package read failed{detail}", detail = optional_detail(.detail))]
    Other { detail: Option<String> },
}

fn optional_detail(detail: &Option<String>) -> String {
    detail
        .as_ref()
        .map(|detail| format!(": {detail:?}"))
        .unwrap_or_default()
}
