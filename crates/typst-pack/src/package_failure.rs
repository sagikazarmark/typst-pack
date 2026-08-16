//! Stable Package Acquisition Failure data carried by Pack Assembly.

use std::collections::BTreeMap;

use typst::syntax::package::{PackageSpec, PackageVersion};

/// Package Acquisition Failures keyed by exact package specification.
///
/// Pack Assembly updates this value between Pack Creation invocations. A
/// separately supplied Package Catalog entry always takes precedence during
/// Dependency Discovery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageAcquisitionFailures {
    failures: BTreeMap<String, PackageAcquisitionFailure>,
}

impl PackageAcquisitionFailures {
    /// An empty failure map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the latest failed attempt for one exact specification.
    pub fn insert(
        &mut self,
        failure: PackageAcquisitionFailure,
    ) -> Option<PackageAcquisitionFailure> {
        self.failures.insert(failure.spec.to_string(), failure)
    }

    /// Failures in canonical exact-specification order.
    pub fn entries(&self) -> impl Iterator<Item = &PackageAcquisitionFailure> {
        self.failures.values()
    }

    /// Looks up the failed attempt for one exact specification.
    pub fn get(&self, spec: &PackageSpec) -> Option<&PackageAcquisitionFailure> {
        self.failures.get(&spec.to_string())
    }

    /// Removes an older failed attempt after the specification is acquired.
    #[cfg(any(
        feature = "fs",
        all(feature = "opendal", feature = "package-acquisition")
    ))]
    pub(crate) fn remove(&mut self, spec: &PackageSpec) -> Option<PackageAcquisitionFailure> {
        self.failures.remove(&spec.to_string())
    }
}

impl FromIterator<PackageAcquisitionFailure> for PackageAcquisitionFailures {
    fn from_iter<T: IntoIterator<Item = PackageAcquisitionFailure>>(failures: T) -> Self {
        let mut result = Self::new();
        for failure in failures {
            result.insert(failure);
        }
        result
    }
}

/// An external attempt to acquire one exact package specification failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("failed to acquire {spec}: {reason}")]
pub struct PackageAcquisitionFailure {
    spec: PackageSpec,
    reason: PackageAcquisitionFailureReason,
}

impl PackageAcquisitionFailure {
    pub fn new(spec: PackageSpec, reason: PackageAcquisitionFailureReason) -> Self {
        Self { spec, reason }
    }

    /// The exact specification the failed attempt tried to acquire.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The typed operational reason the attempt failed.
    pub fn reason(&self) -> &PackageAcquisitionFailureReason {
        &self.reason
    }

    pub fn into_parts(self) -> (PackageSpec, PackageAcquisitionFailureReason) {
        (self.spec, self.reason)
    }
}

/// The stable operational reason for a Package Acquisition Failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionFailureReason {
    #[error("package not found")]
    NotFound,
    #[error("package version not found; latest available version is {latest}")]
    VersionNotFound { latest: PackageVersion },
    #[error("network request failed{detail}", detail = optional_detail(.detail))]
    NetworkFailed { detail: Option<String> },
    #[error("package archive is malformed{detail}", detail = optional_detail(.detail))]
    MalformedArchive { detail: Option<String> },
    #[error("package acquisition failed{detail}", detail = optional_detail(.detail))]
    Other { detail: Option<String> },
}

fn optional_detail(detail: &Option<String>) -> String {
    detail
        .as_ref()
        .map(|detail| format!(": {detail:?}"))
        .unwrap_or_default()
}
