//! Shared resource ceilings for reading untrusted input.

/// Ceilings for reading an untrusted tree of files: a project directory, a
/// package tree, font roots, or a storage prefix.
///
/// Every reader that crosses a byte trust boundary takes one of these. The
/// [`Default`] values suit ordinary documents; raise them for unusually large
/// projects.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ReadLimits {
    /// Maximum number of directory entries or listed objects visited.
    pub max_entries: u64,
    /// Maximum size of any one selected file.
    pub max_file_bytes: u64,
    /// Maximum combined size of all selected files.
    pub max_total_bytes: u64,
}

impl ReadLimits {
    /// Ceilings suitable for ordinary Typst projects, packages, and font roots.
    pub const DEFAULT: Self = Self {
        max_entries: 100_000,
        max_file_bytes: 256 * 1024 * 1024,
        max_total_bytes: 1024 * 1024 * 1024,
    };
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A resource ceiling was exceeded.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[error("{resource} limit of {limit} exceeded")]
pub struct LimitExceeded {
    /// Human-readable resource name, such as `"file bytes"`.
    pub resource: &'static str,
    /// The ceiling that was exceeded.
    pub limit: u64,
}

impl LimitExceeded {
    /// Fails when `amount` alone passes `limit`.
    pub(crate) fn check(amount: u64, limit: u64, resource: &'static str) -> Result<(), Self> {
        if amount > limit {
            return Err(Self { resource, limit });
        }
        Ok(())
    }

    /// Adds `amount` to `total`, failing when the result passes `limit`.
    pub(crate) fn add(
        total: &mut u64,
        amount: u64,
        limit: u64,
        resource: &'static str,
    ) -> Result<(), Self> {
        match total.checked_add(amount) {
            Some(next) if next <= limit => {
                *total = next;
                Ok(())
            }
            _ => Err(Self { resource, limit }),
        }
    }
}
