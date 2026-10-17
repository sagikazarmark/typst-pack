//! Shared resource ceilings for reading untrusted input.

/// Ceilings for reading an untrusted tree of files: a project directory, a
/// package tree, font roots, or a storage prefix.
///
/// Every reader that crosses a byte trust boundary takes one of these. The
/// [`Default`] values serve projects and font roots; [`ReadLimits::PACKAGE`]
/// is the tighter default for one package tree or package archive. Raise them
/// for unusually large inputs. Readers hold what they read in memory, so
/// `max_total_bytes` also bounds memory use.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ReadLimits {
    /// Maximum number of directory entries or listed objects visited.
    pub max_entries: u64,
    /// Maximum size of any one selected file.
    pub max_file_bytes: u64,
    /// Maximum combined size of all selected files.
    pub max_total_bytes: u64,
}

const MIB: u64 = 1024 * 1024;

impl ReadLimits {
    /// The default for one package: 100,000 entries, 256 MiB per file (and
    /// per package archive), and 256 MiB in total, far above any published
    /// package, so a downloaded or cached archive cannot expand into
    /// gigabytes.
    pub const PACKAGE: Self = Self {
        max_entries: 100_000,
        max_file_bytes: 256 * MIB,
        max_total_bytes: 256 * MIB,
    };
}

impl Default for ReadLimits {
    /// 100,000 entries, 256 MiB per file, and 2 GiB in total: enough for
    /// ordinary Typst projects, and for system font directories, which
    /// routinely hold gigabytes of font collections.
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_file_bytes: 256 * MIB,
            max_total_bytes: 2048 * MIB,
        }
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
