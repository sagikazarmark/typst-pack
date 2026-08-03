//! Stable Package Acquisition Failure data carried by Pack Assembly.

use typst::syntax::package::{PackageSpec, PackageVersion};

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
