//! Package acquisition for a Pack Assembler that supplies its own transport.
//!
//! Creation reports the exact specifications its representative request needs
//! and was not given. These helpers cover the two transformations between that
//! report and a resolved tree — constructing the registry URL of a
//! specification, and expanding the archive bytes fetched from it into a
//! Package Tree — so a caller reimplements neither the registry layout
//! nor the archive encoding. Fetching those bytes is the adapter's own
//! obligation and stays outside: nothing here needs an HTTP client, so the
//! helpers are usable on a host whose only network access is asynchronous.

use std::io::Read;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;

use crate::package_catalog::{PackageTree, PackageTreeError};

/// The URL of the package registry these helpers describe the layout of, the
/// official Typst Universe registry. There is no standardized registry
/// protocol, so the layout is this registry's own.
pub const PACKAGE_REGISTRY_URL: &str = "https://packages.typst.org";

/// The one package namespace the registry serves. A specification in any other
/// namespace is resolved from wherever its namespace lives, which the registry
/// layout says nothing about.
pub const PACKAGE_REGISTRY_NAMESPACE: &str = "preview";

/// The URL of the archive holding one exact package specification's Package
/// Tree.
///
/// No index lookup is involved: creation only ever reports fully versioned
/// specifications, because a Typst import specification always carries an exact
/// version.
pub fn package_archive_url(spec: &PackageSpec) -> Result<String, PackageAcquisitionError> {
    if spec.namespace != PACKAGE_REGISTRY_NAMESPACE {
        return Err(PackageAcquisitionError::UnservedNamespace { spec: spec.clone() });
    }
    Ok(format!(
        "{PACKAGE_REGISTRY_URL}/{}/{}-{}.tar.gz",
        spec.namespace, spec.name, spec.version
    ))
}

/// The bound on how far one package archive may expand.
///
/// It is required rather than defaulted, so that the bound on a caller-named
/// package is always a deliberate choice: an archive names how far it expands,
/// and a service operator that took that number on trust would let one
/// specification exhaust the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageExpansionCeiling {
    /// The largest total byte size the expanded package files may reach.
    pub max_bytes: u64,
}

/// Expands the archive bytes served for one exact package specification into
/// the Package Tree creation accepts as a resolved tree for it.
///
/// The bytes are the gzip-compressed tar a registry serves, which the caller
/// fetched with whatever primitive its host provides; expansion itself needs no
/// transport. Only addressable regular files become tree entries: directories,
/// links, and other archive members are not package files. An entry whose path
/// cannot name one is rejected here rather than carried into a tree, because
/// expansion is where a hostile archive is met.
///
/// Expansion stops at `ceiling` and fails with
/// [`PackageAcquisitionError::ExpansionCeilingExceeded`] rather than
/// materializing what lies past it, so what one call costs is bounded by the
/// ceiling and not by what the archive claims. Every member is charged against
/// it, whether or not that member becomes a package file.
///
pub fn expand_package_archive(
    spec: PackageSpec,
    archive: &[u8],
    ceiling: PackageExpansionCeiling,
) -> Result<PackageTree, PackageAcquisitionError> {
    let malformed = |message: String| PackageAcquisitionError::MalformedArchive {
        spec: spec.clone(),
        message,
    };

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut files: Vec<(String, Bytes)> = Vec::new();
    let mut expanded = 0u64;

    for entry in archive
        .entries()
        .map_err(|error| malformed(error.to_string()))?
    {
        let mut entry = entry.map_err(|error| malformed(error.to_string()))?;

        // Every member is charged against the ceiling by what it declares,
        // before any of it is decompressed. Skipping an oversized member
        // instead of failing on it would still decompress and discard the whole
        // of it, so an archive that claims to expand past the ceiling stops
        // expansion whether or not it claims it in a package file.
        let remaining = ceiling.max_bytes - expanded;
        if entry.size() > remaining {
            return Err(PackageAcquisitionError::ExpansionCeilingExceeded { spec, ceiling });
        }
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path().map_err(|error| malformed(error.to_string()))?;
        let path = path
            .to_str()
            .ok_or_else(|| malformed(format!("path `{}` is not valid UTF-8", path.display())))?
            .to_owned();
        // Read one byte past what the ceiling still allows, so that a member
        // holding more than it declared is measured by what it expands to
        // rather than by what it claims.
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(remaining.saturating_add(1))
            .read_to_end(&mut data)
            .map_err(|error| malformed(error.to_string()))?;
        if data.len() as u64 > remaining {
            return Err(PackageAcquisitionError::ExpansionCeilingExceeded { spec, ceiling });
        }
        expanded += data.len() as u64;

        files.push((path, Bytes::new(data)));
    }

    PackageTree::from_typst_entries(files)
        .map_err(|source| PackageAcquisitionError::InvalidPackageTree { spec, source })
}

/// A failure while acquiring one Package Tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionError {
    /// The registry does not serve the specification's namespace, so it has no
    /// URL there.
    #[error(
        "the registry serves only the `{PACKAGE_REGISTRY_NAMESPACE}` namespace, and {spec} is not in it"
    )]
    UnservedNamespace { spec: PackageSpec },
    /// The archive expands past the ceiling the caller set, so nothing was
    /// expanded for the specification.
    #[error("the archive for {spec} expands past {} byte(s)", ceiling.max_bytes)]
    ExpansionCeilingExceeded {
        spec: PackageSpec,
        ceiling: PackageExpansionCeiling,
    },
    /// The bytes are not the archive a registry serves for the specification.
    #[error("the archive for {spec} is malformed: {message:?}")]
    MalformedArchive { spec: PackageSpec, message: String },
    /// The archive does not expand into a valid Package Tree.
    #[error("the archive for {spec} does not contain a valid package tree: {source}")]
    InvalidPackageTree {
        spec: PackageSpec,
        source: PackageTreeError,
    },
}
