use std::{error::Error, fmt};

use typst::syntax::package::PackageSpec;

use super::super::acquisition::recursive::{
    RecursiveAcquisitionError, RecursiveAcquisitionLimits, RecursiveAcquisitionResource,
    RecursiveAcquisitionSelection, RecursiveSourcesAcquisitionError, RecursiveSurveyIssue,
    RecursiveSurveyIssueKind, acquire_first_present_recursive_prefix_with_resolved,
};
use super::super::acquisition::{
    ExactObjectAcquisitionError, ExactObjectLimitError, ResolvedOperators,
    acquire_exact_object_with_resolved,
};
use super::super::{Location, LocationRoleError, OperatorResolver};
use crate::acquisition_layout;
use crate::package_catalog::PackageTreeError;

/// Named finite ceilings for one OpenDAL Package Tree Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTreeAcquisitionCeilings {
    /// Entries yielded by recursive listings across attempted tree sources.
    pub listed_entries: u64,
    /// Bytes in one yielded operation path.
    pub listed_path_bytes: u64,
    /// Bytes retained for paths and structural evidence across tree sources.
    pub total_listed_path_bytes: u64,
    /// File objects selected from one present tree source.
    pub selected_files: u64,
    /// Exact bytes retained for one selected file.
    pub object_bytes: u64,
    /// Exact file bytes retained for the selected Package Tree.
    pub total_bytes: u64,
}

impl PackageTreeAcquisitionCeilings {
    /// The first-party version-1 Package Tree Acquisition profile.
    pub const fn reference_v1() -> Self {
        Self {
            listed_entries: 100_000,
            listed_path_bytes: 64 * 1024,
            total_listed_path_bytes: 64 * 1024 * 1024,
            selected_files: 50_000,
            object_bytes: 64 * 1024 * 1024,
            total_bytes: 512 * 1024 * 1024,
        }
    }
}

/// A resource bounded during OpenDAL Package Tree Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageTreeAcquisitionResource {
    /// Entries yielded by recursive listings.
    ListedEntries,
    /// Bytes in one yielded operation path.
    ListedPathBytes,
    /// Bytes retained for paths and structural evidence.
    TotalListedPathBytes,
    /// Selected file objects.
    SelectedFiles,
    /// Bytes retained for one selected file.
    ObjectBytes,
    /// Bytes retained across the selected tree.
    TotalBytes,
}

/// A supplied Package Tree Acquisition ceiling is internally inconsistent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageTreeAcquisitionLimitsError {
    /// A payload ceiling cannot accommodate the required plus-one probe.
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: PackageTreeAcquisitionResource,
        ceiling: u64,
    },
    /// The per-object ceiling exceeds the aggregate payload ceiling.
    #[error("the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}")]
    ObjectBytesExceedTotalBytes { object_bytes: u64, total_bytes: u64 },
}

/// Mandatory finite limits for OpenDAL Package Tree Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTreeAcquisitionLimits {
    ceilings: PackageTreeAcquisitionCeilings,
}

impl PackageTreeAcquisitionLimits {
    /// Validates every named Package Tree Acquisition ceiling.
    pub fn new(
        ceilings: PackageTreeAcquisitionCeilings,
    ) -> Result<Self, PackageTreeAcquisitionLimitsError> {
        for (resource, ceiling) in [
            (
                PackageTreeAcquisitionResource::ObjectBytes,
                ceilings.object_bytes,
            ),
            (
                PackageTreeAcquisitionResource::TotalBytes,
                ceilings.total_bytes,
            ),
        ] {
            if ceiling == u64::MAX {
                return Err(PackageTreeAcquisitionLimitsError::CannotProbe { resource, ceiling });
            }
        }
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(
                PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: ceilings.object_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }
        Ok(Self { ceilings })
    }

    /// The validated first-party version-1 limits.
    pub const fn reference_v1() -> Self {
        Self {
            ceilings: PackageTreeAcquisitionCeilings::reference_v1(),
        }
    }

    /// The maximum number of entries yielded across attempted tree sources.
    pub const fn listed_entries(&self) -> u64 {
        self.ceilings.listed_entries
    }

    /// The maximum byte length of one yielded operation path.
    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings.listed_path_bytes
    }

    /// The maximum retained bytes for paths and structural evidence.
    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings.total_listed_path_bytes
    }

    /// The maximum selected file count.
    pub const fn selected_files(&self) -> u64 {
        self.ceilings.selected_files
    }

    /// The maximum exact bytes retained for one selected file.
    pub const fn object_bytes(&self) -> u64 {
        self.ceilings.object_bytes
    }

    /// The maximum exact bytes retained for the selected Package Tree.
    pub const fn total_bytes(&self) -> u64 {
        self.ceilings.total_bytes
    }
}

/// Package Tree Acquisition exceeded or could not account for a mandatory limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageTreeAcquisitionLimitError {
    /// The observed resource exceeded its ceiling.
    #[error(
        "OpenDAL Package Tree Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: PackageTreeAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    /// Resource accounting could not be represented in `u64`.
    #[error("OpenDAL Package Tree Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: PackageTreeAcquisitionResource,
    },
}

/// Named finite ceilings for one raw Package Archive Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageArchiveAcquisitionCeilings {
    /// Exact raw archive bytes retained from one candidate.
    pub archive_bytes: u64,
}

impl PackageArchiveAcquisitionCeilings {
    /// The first-party 128 MiB archive profile.
    pub const fn reference_v1() -> Self {
        Self {
            archive_bytes: 128 * 1024 * 1024,
        }
    }
}

/// A resource bounded while acquiring a raw Package Archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageArchiveAcquisitionResource {
    /// Exact raw Package Archive bytes.
    ArchiveBytes,
}

/// A supplied Package Archive Acquisition ceiling is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageArchiveAcquisitionLimitsError {
    /// The archive ceiling cannot accommodate the required plus-one probe.
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: PackageArchiveAcquisitionResource,
        ceiling: u64,
    },
}

/// Mandatory finite limits for one raw Package Archive Acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageArchiveAcquisitionLimits {
    ceilings: PackageArchiveAcquisitionCeilings,
}

impl PackageArchiveAcquisitionLimits {
    /// Validates the named raw archive ceiling.
    pub fn new(
        ceilings: PackageArchiveAcquisitionCeilings,
    ) -> Result<Self, PackageArchiveAcquisitionLimitsError> {
        if ceilings.archive_bytes == u64::MAX {
            return Err(PackageArchiveAcquisitionLimitsError::CannotProbe {
                resource: PackageArchiveAcquisitionResource::ArchiveBytes,
                ceiling: ceilings.archive_bytes,
            });
        }
        Ok(Self { ceilings })
    }

    /// The validated first-party version-1 limits.
    pub const fn reference_v1() -> Self {
        Self {
            ceilings: PackageArchiveAcquisitionCeilings::reference_v1(),
        }
    }

    /// The maximum exact raw archive bytes retained from one candidate.
    pub const fn archive_bytes(&self) -> u64 {
        self.ceilings.archive_bytes
    }
}

/// A raw Package Archive exceeded or could not account for its limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageArchiveAcquisitionLimitError {
    /// The observed archive length exceeded its ceiling.
    #[error(
        "OpenDAL Package Archive Acquisition {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: PackageArchiveAcquisitionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    /// Archive byte accounting could not be represented in `u64`.
    #[error("OpenDAL Package Archive Acquisition {resource:?} accounting overflowed")]
    AccountingOverflow {
        resource: PackageArchiveAcquisitionResource,
    },
}

/// Named finite ceilings for Package Acquisition fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageAcquisitionCeilings {
    /// Ceilings shared across ordered Package Tree candidates.
    pub trees: PackageTreeAcquisitionCeilings,
    /// Ceilings applied independently to cache and registry candidates.
    pub archives: PackageArchiveAcquisitionCeilings,
}

impl PackageAcquisitionCeilings {
    /// The first-party version-1 composite profile.
    pub const fn reference_v1() -> Self {
        Self {
            trees: PackageTreeAcquisitionCeilings::reference_v1(),
            archives: PackageArchiveAcquisitionCeilings::reference_v1(),
        }
    }
}

/// A supplied Package Acquisition limit family is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionLimitsError {
    /// Invalid Package Tree Acquisition limits.
    #[error("invalid Package Tree Acquisition limits: {0}")]
    Trees(PackageTreeAcquisitionLimitsError),
    /// Invalid raw Package Archive Acquisition limits.
    #[error("invalid Package Archive Acquisition limits: {0}")]
    Archives(PackageArchiveAcquisitionLimitsError),
}

/// Mandatory finite limits for Package Acquisition fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageAcquisitionLimits {
    trees: PackageTreeAcquisitionLimits,
    archives: PackageArchiveAcquisitionLimits,
}

impl PackageAcquisitionLimits {
    /// Validates both Package Acquisition limit families.
    pub fn new(
        ceilings: PackageAcquisitionCeilings,
    ) -> Result<Self, PackageAcquisitionLimitsError> {
        Ok(Self {
            trees: PackageTreeAcquisitionLimits::new(ceilings.trees)
                .map_err(PackageAcquisitionLimitsError::Trees)?,
            archives: PackageArchiveAcquisitionLimits::new(ceilings.archives)
                .map_err(PackageAcquisitionLimitsError::Archives)?,
        })
    }

    /// Limits shared across ordered Package Tree candidates.
    pub const fn trees(&self) -> PackageTreeAcquisitionLimits {
        self.trees
    }

    /// Limits applied independently to cache and registry candidates.
    pub const fn archives(&self) -> PackageArchiveAcquisitionLimits {
        self.archives
    }

    /// The validated first-party version-1 composite limits.
    pub const fn reference_v1() -> Self {
        Self {
            trees: PackageTreeAcquisitionLimits::reference_v1(),
            archives: PackageArchiveAcquisitionLimits::reference_v1(),
        }
    }
}

/// One explicitly configured prefix that may hold Package Trees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTreeSource {
    source: Location,
}

impl PackageTreeSource {
    /// Configures one Package Tree prefix.
    pub fn new(source: Location) -> Self {
        Self { source }
    }

    /// The normalized configured prefix.
    pub fn source(&self) -> &Location {
        &self.source
    }
}

/// A validated request to acquire one exact package from ordered sources.
#[derive(Clone, Debug)]
pub struct PackageAcquisitionRequest {
    spec: PackageSpec,
    tree_sources: Vec<PackageTreeSource>,
    archive_cache: Option<Location>,
    registry: Option<Location>,
    limits: PackageAcquisitionLimits,
}

impl PackageAcquisitionRequest {
    /// Validates every configured prefix role before accepting the request.
    pub fn new(
        spec: PackageSpec,
        tree_sources: impl IntoIterator<Item = PackageTreeSource>,
        archive_cache: Option<Location>,
        registry: Option<Location>,
        limits: PackageAcquisitionLimits,
    ) -> Result<Self, PackageAcquisitionRequestRejection> {
        let tree_sources = tree_sources.into_iter().collect::<Vec<_>>();
        let mut issues = tree_sources
            .iter()
            .enumerate()
            .filter_map(|(source_index, configured)| {
                configured.source.require_prefix().err().map(|source| {
                    PackageAcquisitionRequestIssue::InvalidTreeSourceRole {
                        source_index,
                        location: configured.source.clone(),
                        source,
                    }
                })
            })
            .collect::<Vec<_>>();
        if let Some(location) = &archive_cache
            && let Err(source) = location.require_prefix()
        {
            issues.push(PackageAcquisitionRequestIssue::InvalidArchiveCacheRole {
                location: location.clone(),
                source,
            });
        }
        if let Some(location) = &registry
            && let Err(source) = location.require_prefix()
        {
            issues.push(PackageAcquisitionRequestIssue::InvalidRegistryRole {
                location: location.clone(),
                source,
            });
        }
        if !issues.is_empty() {
            return Err(PackageAcquisitionRequestRejection { spec, issues });
        }
        Ok(Self {
            spec,
            tree_sources,
            archive_cache,
            registry,
            limits,
        })
    }

    /// The exact package specification being acquired.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// Package Tree prefixes in caller precedence order.
    pub fn tree_sources(&self) -> &[PackageTreeSource] {
        &self.tree_sources
    }

    /// The optional raw Package Archive cache prefix.
    pub fn archive_cache(&self) -> Option<&Location> {
        self.archive_cache.as_ref()
    }

    /// The optional official Package Registry prefix.
    pub fn registry(&self) -> Option<&Location> {
        self.registry.as_ref()
    }

    /// Mandatory finite limits for this fallback operation.
    pub const fn limits(&self) -> PackageAcquisitionLimits {
        self.limits
    }
}

/// Every invalid source role in a rejected Package Acquisition request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageAcquisitionRequestRejection {
    spec: PackageSpec,
    issues: Vec<PackageAcquisitionRequestIssue>,
}

impl PackageAcquisitionRequestRejection {
    /// The exact package specification from the rejected request.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// Invalid source roles in request-field and caller-source order.
    pub fn issues(&self) -> &[PackageAcquisitionRequestIssue] {
        &self.issues
    }
}

impl fmt::Display for PackageAcquisitionRequestRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Package Acquisition request for {} was rejected with {} issue(s)",
            self.spec,
            self.issues.len()
        )
    }
}

impl Error for PackageAcquisitionRequestRejection {}

/// One invalid source role in a Package Acquisition request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionRequestIssue {
    /// A configured Package Tree source is not a prefix.
    #[error("Package Tree source {source_index} at {location} is not a prefix: {source}")]
    InvalidTreeSourceRole {
        source_index: usize,
        location: Location,
        #[source]
        source: LocationRoleError,
    },
    /// The configured raw archive cache is not a prefix.
    #[error("Package Archive cache at {location} is not a prefix: {source}")]
    InvalidArchiveCacheRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
    /// The configured Package Registry is not a prefix.
    #[error("Package Registry at {location} is not a prefix: {source}")]
    InvalidRegistryRole {
        location: Location,
        #[source]
        source: LocationRoleError,
    },
}

/// One exact file acquired below a Package Tree candidate prefix.
pub struct PackageTreeAcquisitionEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl PackageTreeAcquisitionEntry {
    /// The canonical package-relative file path.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The exact bytes observed by the completed object read.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The exact acquired byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the acquired file was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the owned path and exact bytes.
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for PackageTreeAcquisitionEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeAcquisitionEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact files acquired from the first present Package Tree candidate.
pub struct PackageTreeAcquisition {
    spec: PackageSpec,
    source_index: usize,
    configured_source: Location,
    candidate_location: Location,
    entries: Vec<PackageTreeAcquisitionEntry>,
}

impl PackageTreeAcquisition {
    /// The exact package specification acquired.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The caller-order index of the selected tree source.
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    /// The configured Package Tree prefix.
    pub fn configured_source(&self) -> &Location {
        &self.configured_source
    }

    /// The specification-derived candidate prefix.
    pub fn candidate_location(&self) -> &Location {
        &self.candidate_location
    }

    /// Acquired entries in canonical package-relative path order.
    pub fn entries(&self) -> &[PackageTreeAcquisitionEntry] {
        &self.entries
    }

    /// Recovers the specification, source evidence, and owned entries.
    pub fn into_parts(
        self,
    ) -> (
        PackageSpec,
        usize,
        Location,
        Location,
        Vec<PackageTreeAcquisitionEntry>,
    ) {
        (
            self.spec,
            self.source_index,
            self.configured_source,
            self.candidate_location,
            self.entries,
        )
    }
}

impl fmt::Debug for PackageTreeAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeAcquisition")
            .field("spec", &self.spec)
            .field("source_index", &self.source_index)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("entries", &self.entries)
            .finish()
    }
}

/// Exact raw Package Archive bytes acquired from a configured cache.
pub struct CachedPackageArchiveAcquisition {
    spec: PackageSpec,
    configured_source: Location,
    candidate_location: Location,
    bytes: Vec<u8>,
}

impl CachedPackageArchiveAcquisition {
    /// The exact package specification acquired.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The configured raw archive cache prefix.
    pub fn configured_source(&self) -> &Location {
        &self.configured_source
    }

    /// The specification-derived exact cache object.
    pub fn candidate_location(&self) -> &Location {
        &self.candidate_location
    }

    /// The exact raw Package Archive bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The exact raw archive byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the present raw archive object was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the specification, source evidence, and exact bytes.
    pub fn into_parts(self) -> (PackageSpec, Location, Location, Vec<u8>) {
        (
            self.spec,
            self.configured_source,
            self.candidate_location,
            self.bytes,
        )
    }
}

impl fmt::Debug for CachedPackageArchiveAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CachedPackageArchiveAcquisition")
            .field("spec", &self.spec)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact raw Package Archive bytes acquired from the official registry.
pub struct RegistryPackageArchiveAcquisition {
    spec: PackageSpec,
    configured_source: Location,
    candidate_location: Location,
    cache_destination: Option<Location>,
    bytes: Vec<u8>,
}

impl RegistryPackageArchiveAcquisition {
    /// The exact package specification acquired.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The configured official Package Registry prefix.
    pub fn configured_source(&self) -> &Location {
        &self.configured_source
    }

    /// The specification-derived exact registry object.
    pub fn candidate_location(&self) -> &Location {
        &self.candidate_location
    }

    /// The derived cache object available for later optional publication.
    pub fn cache_destination(&self) -> Option<&Location> {
        self.cache_destination.as_ref()
    }

    /// The exact raw bytes returned by the registry.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The exact raw archive byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the present registry object was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the specification, source evidence, cache destination, and bytes.
    pub fn into_parts(self) -> (PackageSpec, Location, Location, Option<Location>, Vec<u8>) {
        (
            self.spec,
            self.configured_source,
            self.candidate_location,
            self.cache_destination,
            self.bytes,
        )
    }
}

impl fmt::Debug for RegistryPackageArchiveAcquisition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryPackageArchiveAcquisition")
            .field("spec", &self.spec)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("cache_destination", &self.cache_destination)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Evidence that every applicable configured package source was absent.
#[derive(Debug)]
pub struct UnavailablePackageAcquisition {
    spec: PackageSpec,
    failure: crate::PackageAcquisitionFailure,
}

impl UnavailablePackageAcquisition {
    /// The exact package specification that was unavailable.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The stable failure to carry into resumed Pack Creation.
    pub fn failure(&self) -> &crate::PackageAcquisitionFailure {
        &self.failure
    }

    /// The stable reason the package was unavailable.
    pub fn reason(&self) -> &crate::PackageAcquisitionFailureReason {
        self.failure.reason()
    }

    /// Recovers the exact specification and owned failure.
    pub fn into_parts(self) -> (PackageSpec, crate::PackageAcquisitionFailure) {
        (self.spec, self.failure)
    }
}

/// The raw result of acquiring one package through configured OpenDAL sources.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackageAcquisition {
    /// A present Package Tree candidate and its exact files.
    Tree(PackageTreeAcquisition),
    /// A present raw Package Archive cache object.
    CachedArchive(CachedPackageArchiveAcquisition),
    /// A present official Package Registry object.
    RegistryArchive(RegistryPackageArchiveAcquisition),
    /// Every applicable configured candidate was definitely absent.
    Unavailable(UnavailablePackageAcquisition),
}

impl PackageAcquisition {
    /// The selected configured source, or `None` when unavailable.
    pub fn configured_source(&self) -> Option<&Location> {
        match self {
            Self::Tree(value) => Some(value.configured_source()),
            Self::CachedArchive(value) => Some(value.configured_source()),
            Self::RegistryArchive(value) => Some(value.configured_source()),
            Self::Unavailable(_) => None,
        }
    }

    /// The selected derived candidate, or `None` when unavailable.
    pub fn candidate_location(&self) -> Option<&Location> {
        match self {
            Self::Tree(value) => Some(value.candidate_location()),
            Self::CachedArchive(value) => Some(value.candidate_location()),
            Self::RegistryArchive(value) => Some(value.candidate_location()),
            Self::Unavailable(_) => None,
        }
    }
}

/// Acquires one package from ordered trees, an optional cache, then an optional registry.
///
/// Only definite absence advances fallback. Registry lookup is skipped when the
/// official registry does not serve the requested namespace. This operation
/// returns exact raw values and performs no archive expansion or cache write.
pub async fn acquire_package<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageAcquisitionRequest,
) -> Result<PackageAcquisition, PackageAcquisitionError<R::Error>> {
    let mut resolved = ResolvedOperators::new(resolver);
    match acquire_package_tree_candidates_with_resolved(
        &mut resolved,
        request.spec(),
        request.tree_sources(),
        request.limits().trees(),
    )
    .await
    {
        Ok(Some(tree)) => return Ok(PackageAcquisition::Tree(tree)),
        Ok(None) => {}
        Err(error) => return Err(PackageAcquisitionError::from_tree(request.spec(), error)),
    }

    if let Some(configured_source) = request.archive_cache() {
        let candidate_location = compose_candidate(
            configured_source,
            &acquisition_layout::package_archive_cache_key(request.spec()),
        );
        match acquire_exact_object_with_resolved(
            &mut resolved,
            &candidate_location,
            request.limits().archives().archive_bytes(),
        )
        .await
        {
            Ok(bytes) => {
                return Ok(PackageAcquisition::CachedArchive(
                    CachedPackageArchiveAcquisition {
                        spec: request.spec().clone(),
                        configured_source: configured_source.clone(),
                        candidate_location,
                        bytes,
                    },
                ));
            }
            Err(ExactObjectAcquisitionError::ObjectAbsent(_)) => {}
            Err(error) => {
                return Err(PackageAcquisitionError::from_archive(
                    request.spec(),
                    configured_source.clone(),
                    candidate_location,
                    ArchiveSource::Cache,
                    error,
                ));
            }
        }
    }

    if let (Some(configured_source), Some(registry_key)) = (
        request.registry(),
        acquisition_layout::official_registry_archive_key(request.spec()),
    ) {
        let candidate_location = compose_candidate(configured_source, &registry_key);
        match acquire_exact_object_with_resolved(
            &mut resolved,
            &candidate_location,
            request.limits().archives().archive_bytes(),
        )
        .await
        {
            Ok(bytes) => {
                let cache_destination = request.archive_cache().map(|cache| {
                    compose_candidate(
                        cache,
                        &acquisition_layout::package_archive_cache_key(request.spec()),
                    )
                });
                return Ok(PackageAcquisition::RegistryArchive(
                    RegistryPackageArchiveAcquisition {
                        spec: request.spec().clone(),
                        configured_source: configured_source.clone(),
                        candidate_location,
                        cache_destination,
                        bytes,
                    },
                ));
            }
            Err(ExactObjectAcquisitionError::ObjectAbsent(_)) => {}
            Err(error) => {
                return Err(PackageAcquisitionError::from_archive(
                    request.spec(),
                    configured_source.clone(),
                    candidate_location,
                    ArchiveSource::Registry,
                    error,
                ));
            }
        }
    }

    let failure = crate::PackageAcquisitionFailure::new(
        request.spec().clone(),
        crate::PackageAcquisitionFailureReason::NotFound,
    );
    Ok(PackageAcquisition::Unavailable(
        UnavailablePackageAcquisition {
            spec: request.spec().clone(),
            failure,
        },
    ))
}

enum ArchiveSource {
    Cache,
    Registry,
}

/// A terminal failure while acquiring one package through OpenDAL.
pub struct PackageAcquisitionError<E> {
    spec: PackageSpec,
    source_index: Option<usize>,
    configured_source: Option<Location>,
    candidate_location: Option<Location>,
    failed_path: Option<String>,
    failure: crate::PackageAcquisitionFailure,
    cause: PackageAcquisitionErrorCause<E>,
}

impl<E> PackageAcquisitionError<E> {
    /// The exact package specification whose acquisition failed.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The caller-order tree source index, when failure occurred at a tree.
    pub fn source_index(&self) -> Option<usize> {
        self.source_index
    }

    /// The configured source reached when failure occurred.
    pub fn configured_source(&self) -> Option<&Location> {
        self.configured_source.as_ref()
    }

    /// The specification-derived candidate reached when available.
    pub fn candidate_location(&self) -> Option<&Location> {
        self.candidate_location.as_ref()
    }

    /// The listed tree object that failed to read, when applicable.
    pub fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    /// The stable failure suitable for resumed Pack Creation.
    pub fn failure(&self) -> &crate::PackageAcquisitionFailure {
        &self.failure
    }

    /// The stable Package Acquisition Failure reason.
    pub fn reason(&self) -> &crate::PackageAcquisitionFailureReason {
        self.failure.reason()
    }

    /// The typed adapter cause retained by this failure.
    pub fn cause(&self) -> &PackageAcquisitionErrorCause<E> {
        &self.cause
    }

    fn from_tree(spec: &PackageSpec, error: PackageTreeSourceAcquisitionError<E>) -> Self {
        let cause = match error.cause {
            PackageTreeSourceAcquisitionErrorCause::InvalidSourceRole(_) => {
                unreachable!("PackageAcquisitionRequest validates every tree prefix")
            }
            PackageTreeSourceAcquisitionErrorCause::ResolveOperator(source) => {
                PackageAcquisitionErrorCause::ResolveOperator(source)
            }
            PackageTreeSourceAcquisitionErrorCause::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            } => PackageAcquisitionErrorCause::UnsupportedTreeCapabilities {
                list,
                list_with_recursive,
                read,
            },
            PackageTreeSourceAcquisitionErrorCause::List(source) => {
                PackageAcquisitionErrorCause::TreeList(source)
            }
            PackageTreeSourceAcquisitionErrorCause::Read(source) => {
                PackageAcquisitionErrorCause::TreeRead(source)
            }
            PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source) => {
                PackageAcquisitionErrorCause::ListedTreeObjectAbsent(source)
            }
            PackageTreeSourceAcquisitionErrorCause::Structural(source) => {
                PackageAcquisitionErrorCause::TreeStructural(source)
            }
            PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source) => {
                PackageAcquisitionErrorCause::InvalidPackageTree(source)
            }
            PackageTreeSourceAcquisitionErrorCause::Limit(source) => {
                PackageAcquisitionErrorCause::TreeLimit(source)
            }
        };
        Self {
            spec: spec.clone(),
            source_index: Some(error.source_index),
            configured_source: Some(error.configured_source),
            candidate_location: error.candidate_location,
            failed_path: error.failed_path,
            failure: other_failure(spec),
            cause,
        }
    }

    fn from_archive(
        spec: &PackageSpec,
        configured_source: Location,
        candidate_location: Location,
        archive_source: ArchiveSource,
        error: ExactObjectAcquisitionError<E>,
    ) -> Self {
        let cause = match error {
            ExactObjectAcquisitionError::InvalidLocationRole(_) => {
                unreachable!("a package archive key below a prefix is an exact object")
            }
            ExactObjectAcquisitionError::ResolveOperator(source) => {
                PackageAcquisitionErrorCause::ResolveOperator(source)
            }
            ExactObjectAcquisitionError::ReadUnsupported => {
                PackageAcquisitionErrorCause::UnsupportedArchiveRead
            }
            ExactObjectAcquisitionError::ObjectAbsent(_) => {
                unreachable!("definite archive absence advances fallback")
            }
            ExactObjectAcquisitionError::Read(source) => match archive_source {
                ArchiveSource::Cache => PackageAcquisitionErrorCause::CacheRead(source),
                ArchiveSource::Registry => PackageAcquisitionErrorCause::RegistryRead(source),
            },
            ExactObjectAcquisitionError::Limit(source) => {
                PackageAcquisitionErrorCause::ArchiveLimit(map_archive_limit(source))
            }
        };
        Self {
            spec: spec.clone(),
            source_index: None,
            configured_source: Some(configured_source),
            candidate_location: Some(candidate_location),
            failed_path: None,
            failure: other_failure(spec),
            cause,
        }
    }
}

impl<E> fmt::Display for PackageAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Package Acquisition failed for {}", self.spec)?;
        if let Some(source_index) = self.source_index {
            write!(formatter, " at tree source {source_index}")?;
        }
        if let Some(candidate) = &self.candidate_location {
            write!(formatter, " at candidate {candidate}")?;
        }
        write!(formatter, ": {}", self.cause.label())
    }
}

impl<E> fmt::Debug for PackageAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageAcquisitionError")
            .field("spec", &self.spec)
            .field("source_index", &self.source_index)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("failed_path", &self.failed_path)
            .field("reason", self.failure.reason())
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackageAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackageAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            PackageAcquisitionErrorCause::UnsupportedTreeCapabilities { .. }
            | PackageAcquisitionErrorCause::UnsupportedArchiveRead => None,
            PackageAcquisitionErrorCause::TreeList(source)
            | PackageAcquisitionErrorCause::TreeRead(source)
            | PackageAcquisitionErrorCause::ListedTreeObjectAbsent(source)
            | PackageAcquisitionErrorCause::CacheRead(source)
            | PackageAcquisitionErrorCause::RegistryRead(source) => Some(source),
            PackageAcquisitionErrorCause::TreeStructural(source) => Some(source),
            PackageAcquisitionErrorCause::InvalidPackageTree(source) => Some(source),
            PackageAcquisitionErrorCause::TreeLimit(source) => Some(source),
            PackageAcquisitionErrorCause::ArchiveLimit(source) => Some(source),
        }
    }
}

/// The typed cause of a terminal OpenDAL Package Acquisition failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackageAcquisitionErrorCause<E> {
    /// The reached binding could not be resolved.
    ResolveOperator(E),
    /// A reached tree binding cannot recursively list or read selected files.
    UnsupportedTreeCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    /// A reached raw archive binding cannot read objects.
    UnsupportedArchiveRead,
    /// A Package Tree recursive listing failed.
    TreeList(::opendal::Error),
    /// A listed Package Tree object read failed.
    TreeRead(::opendal::Error),
    /// A listed Package Tree object became absent when read.
    ListedTreeObjectAbsent(::opendal::Error),
    /// The raw Package Archive cache read failed.
    CacheRead(::opendal::Error),
    /// The official Package Registry read failed.
    RegistryRead(::opendal::Error),
    /// A completed Package Tree listing had envelope issues.
    TreeStructural(PackageTreeAcquisitionSurveyError),
    /// Listed paths do not form a valid Package Tree.
    InvalidPackageTree(PackageTreeError),
    /// Package Tree Acquisition exceeded a mandatory limit.
    TreeLimit(PackageTreeAcquisitionLimitError),
    /// Raw Package Archive Acquisition exceeded a mandatory limit.
    ArchiveLimit(PackageArchiveAcquisitionLimitError),
}

impl<E> PackageAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedTreeCapabilities { .. } => {
                "required Package Tree capabilities are unsupported"
            }
            Self::UnsupportedArchiveRead => "Package Archive read capability is unsupported",
            Self::TreeList(_) => "the Package Tree listing failed",
            Self::TreeRead(_) => "a Package Tree object read failed",
            Self::ListedTreeObjectAbsent(_) => "a listed Package Tree object became absent",
            Self::CacheRead(_) => "the Package Archive cache read failed",
            Self::RegistryRead(_) => "the Package Registry read failed",
            Self::TreeStructural(_) => "the Package Tree listing had structural issues",
            Self::InvalidPackageTree(_) => "the listed objects do not form a Package Tree",
            Self::TreeLimit(_) => "a Package Tree Acquisition limit failed",
            Self::ArchiveLimit(_) => "a Package Archive Acquisition limit failed",
        }
    }
}

fn map_archive_limit(source: ExactObjectLimitError) -> PackageArchiveAcquisitionLimitError {
    match source {
        ExactObjectLimitError::Exceeded {
            ceiling,
            observed_at_least,
        } => PackageArchiveAcquisitionLimitError::Exceeded {
            resource: PackageArchiveAcquisitionResource::ArchiveBytes,
            ceiling,
            observed_at_least,
        },
        ExactObjectLimitError::AccountingOverflow => {
            PackageArchiveAcquisitionLimitError::AccountingOverflow {
                resource: PackageArchiveAcquisitionResource::ArchiveBytes,
            }
        }
    }
}

fn other_failure(spec: &PackageSpec) -> crate::PackageAcquisitionFailure {
    crate::PackageAcquisitionFailure::new(
        spec.clone(),
        crate::PackageAcquisitionFailureReason::Other { detail: None },
    )
}

/// Exact registry bytes retained after successful validation and insertion.
#[cfg(feature = "package-acquisition")]
pub struct RegistryArchiveResidue {
    spec: PackageSpec,
    destination: Location,
    bytes: Vec<u8>,
}

#[cfg(feature = "package-acquisition")]
impl RegistryArchiveResidue {
    /// The exact package specification validated and inserted.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The derived exact cache object for optional publication.
    pub fn destination(&self) -> &Location {
        &self.destination
    }

    /// The exact original registry bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The exact raw archive byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the original registry object was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the specification, cache destination, and exact registry bytes.
    pub fn into_parts(self) -> (PackageSpec, Location, Vec<u8>) {
        (self.spec, self.destination, self.bytes)
    }
}

#[cfg(feature = "package-acquisition")]
impl fmt::Debug for RegistryArchiveResidue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryArchiveResidue")
            .field("spec", &self.spec)
            .field("destination", &self.destination)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// The stage at which an acquired package could not be inserted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[cfg(feature = "package-acquisition")]
pub enum AcquiredPackageInsertionTarget {
    /// Construction of an acquired Package Tree failed.
    PackageTree,
    /// Expansion of cached raw archive bytes failed.
    CachedArchive,
    /// Expansion of registry raw archive bytes failed.
    RegistryArchive,
    /// Package Catalog validation or insertion failed.
    PackageCatalog,
}

/// A failure while converting raw acquisition into Pack Creation inputs.
#[cfg(feature = "package-acquisition")]
pub struct AcquiredPackageInsertionError {
    spec: PackageSpec,
    failure: crate::PackageAcquisitionFailure,
    target: AcquiredPackageInsertionTarget,
    cause: AcquiredPackageInsertionErrorCause,
}

#[cfg(feature = "package-acquisition")]
impl AcquiredPackageInsertionError {
    /// The exact package specification that could not be inserted.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The stable failure recorded for resumed Pack Creation.
    pub fn failure(&self) -> &crate::PackageAcquisitionFailure {
        &self.failure
    }

    /// The stable Package Acquisition Failure reason.
    pub fn reason(&self) -> &crate::PackageAcquisitionFailureReason {
        self.failure.reason()
    }

    /// The insertion stage that failed.
    pub fn target(&self) -> &AcquiredPackageInsertionTarget {
        &self.target
    }

    /// The typed core cause retained by this failure.
    pub fn cause(&self) -> &AcquiredPackageInsertionErrorCause {
        &self.cause
    }
}

#[cfg(feature = "package-acquisition")]
impl fmt::Display for AcquiredPackageInsertionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to insert acquired package {} at {:?}",
            self.spec, self.target
        )
    }
}

#[cfg(feature = "package-acquisition")]
impl fmt::Debug for AcquiredPackageInsertionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcquiredPackageInsertionError")
            .field("spec", &self.spec)
            .field("reason", self.failure.reason())
            .field("target", &self.target)
            .field("cause", &self.cause)
            .finish()
    }
}

#[cfg(feature = "package-acquisition")]
impl Error for AcquiredPackageInsertionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            AcquiredPackageInsertionErrorCause::PackageTree(source) => Some(source),
            AcquiredPackageInsertionErrorCause::ArchiveExpansion(source) => Some(source),
            AcquiredPackageInsertionErrorCause::PackageCatalog(source) => Some(source),
        }
    }
}

/// The typed cause of an acquired-package insertion failure.
#[derive(Debug)]
#[non_exhaustive]
#[cfg(feature = "package-acquisition")]
pub enum AcquiredPackageInsertionErrorCause {
    /// Acquired entries could not construct a Package Tree.
    PackageTree(crate::PackageTreeError),
    /// Raw archive bytes could not expand into a Package Tree.
    ArchiveExpansion(crate::PackageAcquisitionError),
    /// The Package Catalog rejected the constructed tree.
    PackageCatalog(crate::PackageCatalogError),
}

/// Expands or constructs an acquired package, inserts it, and updates failures.
///
/// Registry residue is returned only after expansion, validation, and catalog
/// insertion succeed. Publishing those exact bytes is a separate low-level
/// operation; publishing before this function succeeds can poison a cache with
/// terminal malformed bytes.
#[cfg(feature = "package-acquisition")]
#[allow(unreachable_patterns)]
pub fn insert_acquired_package(
    catalog: &mut crate::PackageCatalog,
    failures: &mut crate::PackageAcquisitionFailures,
    acquisition: PackageAcquisition,
    disposition: crate::PackageDisposition,
    expansion_limits: crate::PackageExpansionLimits,
) -> Result<Option<RegistryArchiveResidue>, AcquiredPackageInsertionError> {
    let (spec, tree, residue) = match acquisition {
        PackageAcquisition::Tree(acquisition) => {
            let (spec, _, _, _, entries) = acquisition.into_parts();
            let tree = crate::PackageTree::from_owned_entries(
                entries
                    .into_iter()
                    .map(PackageTreeAcquisitionEntry::into_parts),
            )
            .map_err(|source| {
                insertion_error(
                    &spec,
                    AcquiredPackageInsertionTarget::PackageTree,
                    AcquiredPackageInsertionErrorCause::PackageTree(source),
                    crate::PackageAcquisitionFailureReason::Other { detail: None },
                    failures,
                )
            })?;
            (spec, tree, None)
        }
        PackageAcquisition::CachedArchive(acquisition) => {
            let (spec, _, _, bytes) = acquisition.into_parts();
            let tree = expand_acquired_archive(
                &spec,
                &bytes,
                AcquiredPackageInsertionTarget::CachedArchive,
                expansion_limits,
                failures,
            )?;
            (spec, tree, None)
        }
        PackageAcquisition::RegistryArchive(acquisition) => {
            let (spec, _, _, destination, bytes) = acquisition.into_parts();
            let tree = expand_acquired_archive(
                &spec,
                &bytes,
                AcquiredPackageInsertionTarget::RegistryArchive,
                expansion_limits,
                failures,
            )?;
            let residue = destination.map(|destination| RegistryArchiveResidue {
                spec: spec.clone(),
                destination,
                bytes,
            });
            (spec, tree, residue)
        }
        PackageAcquisition::Unavailable(acquisition) => {
            let (_, failure) = acquisition.into_parts();
            failures.insert(failure);
            return Ok(None);
        }
        _ => unreachable!("future Package Acquisition outcomes require explicit composition"),
    };

    catalog
        .insert(spec.clone(), tree, disposition)
        .map_err(|source| {
            insertion_error(
                &spec,
                AcquiredPackageInsertionTarget::PackageCatalog,
                AcquiredPackageInsertionErrorCause::PackageCatalog(source),
                crate::PackageAcquisitionFailureReason::Other { detail: None },
                failures,
            )
        })?;
    failures.remove(&spec);
    Ok(residue)
}

#[cfg(feature = "package-acquisition")]
fn expand_acquired_archive(
    spec: &PackageSpec,
    bytes: &[u8],
    target: AcquiredPackageInsertionTarget,
    limits: crate::PackageExpansionLimits,
    failures: &mut crate::PackageAcquisitionFailures,
) -> Result<crate::PackageTree, AcquiredPackageInsertionError> {
    crate::expand_package_archive(spec.clone(), bytes, limits).map_err(|source| {
        let reason = match &source {
            crate::PackageAcquisitionError::MalformedArchive { .. }
            | crate::PackageAcquisitionError::InvalidPackageTree { .. } => {
                crate::PackageAcquisitionFailureReason::MalformedArchive { detail: None }
            }
            crate::PackageAcquisitionError::UnservedNamespace { .. }
            | crate::PackageAcquisitionError::ExpansionLimit { .. } => {
                crate::PackageAcquisitionFailureReason::Other { detail: None }
            }
        };
        insertion_error(
            spec,
            target,
            AcquiredPackageInsertionErrorCause::ArchiveExpansion(source),
            reason,
            failures,
        )
    })
}

#[cfg(feature = "package-acquisition")]
fn insertion_error(
    spec: &PackageSpec,
    target: AcquiredPackageInsertionTarget,
    cause: AcquiredPackageInsertionErrorCause,
    reason: crate::PackageAcquisitionFailureReason,
    failures: &mut crate::PackageAcquisitionFailures,
) -> AcquiredPackageInsertionError {
    let failure = crate::PackageAcquisitionFailure::new(spec.clone(), reason);
    failures.insert(failure.clone());
    AcquiredPackageInsertionError {
        spec: spec.clone(),
        failure,
        target,
        cause,
    }
}

/// An unsupported entry kind yielded by a Package Tree listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageTreeAcquisitionEntryKind {
    /// OpenDAL did not classify the entry as a file or directory.
    Unknown,
}

/// One storage-envelope issue found during a Package Tree survey.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageTreeAcquisitionIssue {
    /// A yielded path is outside the candidate prefix.
    #[error("listed operation path {operation_path:?} is outside the Package Tree prefix")]
    ListedPathOutsidePrefix { operation_path: String },
    /// A yielded file names the candidate prefix itself or ends in a separator.
    #[error("listed operation path {operation_path:?} is a prefix marker where a file is required")]
    PrefixMarkerWhereFileRequired { operation_path: String },
    /// A yielded path has no package-relative component.
    #[error("listed operation path {operation_path:?} has an empty relative path")]
    EmptyRelativeOperationPath { operation_path: String },
    /// A yielded entry has an unsupported kind.
    #[error("listed operation path {operation_path:?} has unsupported kind {kind:?}")]
    UnsupportedEntryKind {
        operation_path: String,
        kind: PackageTreeAcquisitionEntryKind,
    },
}

/// The nonempty canonical set of Package Tree survey envelope issues.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTreeAcquisitionSurveyError {
    issues: Vec<PackageTreeAcquisitionIssue>,
}

impl PackageTreeAcquisitionSurveyError {
    /// Every independently detectable issue in canonical path and kind order.
    pub fn issues(&self) -> &[PackageTreeAcquisitionIssue] {
        &self.issues
    }
}

impl fmt::Display for PackageTreeAcquisitionSurveyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            issue.fmt(formatter)
        } else {
            write!(
                formatter,
                "Package Tree survey failed with {} issue(s)",
                self.issues.len()
            )
        }
    }
}

impl Error for PackageTreeAcquisitionSurveyError {}

pub(crate) struct PackageTreeSourceAcquisitionError<E> {
    source_index: usize,
    configured_source: Location,
    candidate_location: Option<Location>,
    failed_path: Option<String>,
    cause: PackageTreeSourceAcquisitionErrorCause<E>,
}

impl<E> PackageTreeSourceAcquisitionError<E> {
    #[cfg(test)]
    pub(crate) fn source_index(&self) -> usize {
        self.source_index
    }

    #[cfg(test)]
    pub(crate) fn failed_path(&self) -> Option<&str> {
        self.failed_path.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn cause(&self) -> &PackageTreeSourceAcquisitionErrorCause<E> {
        &self.cause
    }

    fn from_recursive(
        sources: &[PackageTreeSource],
        child: &str,
        error: RecursiveSourcesAcquisitionError<E>,
    ) -> Self {
        let source_index = error.source_index;
        let configured_source = sources[source_index].source.clone();
        let candidate_location = configured_source
            .require_prefix()
            .is_ok()
            .then(|| compose_candidate(&configured_source, child));
        let (failed_path, cause) = match error.source {
            RecursiveAcquisitionError::InvalidLocationRole(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::InvalidSourceRole(source),
            ),
            RecursiveAcquisitionError::ResolveOperator(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::ResolveOperator(source),
            ),
            RecursiveAcquisitionError::UnsupportedCapabilities {
                list,
                list_with_recursive,
                read,
            } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::UnsupportedCapabilities {
                    list,
                    list_with_recursive,
                    read,
                },
            ),
            RecursiveAcquisitionError::List(source) => {
                (None, PackageTreeSourceAcquisitionErrorCause::List(source))
            }
            RecursiveAcquisitionError::Read {
                operation_path,
                source,
            } => (
                Some(operation_path),
                PackageTreeSourceAcquisitionErrorCause::Read(source),
            ),
            RecursiveAcquisitionError::ListedObjectAbsent {
                operation_path,
                source,
            } => (
                Some(operation_path),
                PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source),
            ),
            RecursiveAcquisitionError::Structural(issues) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Structural(
                    PackageTreeAcquisitionSurveyError {
                        issues: issues.into_iter().map(map_issue).collect(),
                    },
                ),
            ),
            RecursiveAcquisitionError::InvalidPackageTree(source) => (
                None,
                PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source),
            ),
            RecursiveAcquisitionError::Limit {
                resource,
                ceiling,
                observed_at_least,
            } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::Exceeded {
                        resource: map_resource(resource),
                        ceiling,
                        observed_at_least,
                    },
                ),
            ),
            RecursiveAcquisitionError::AccountingOverflow { resource } => (
                None,
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::AccountingOverflow {
                        resource: map_resource(resource),
                    },
                ),
            ),
        };
        Self {
            source_index,
            configured_source,
            candidate_location,
            failed_path,
            cause,
        }
    }
}

impl<E> fmt::Display for PackageTreeSourceAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Package Tree Acquisition failed at source {} for binding {} at configured operation path {:?}",
            self.source_index,
            self.configured_source.binding(),
            self.configured_source.operation_path(),
        )?;
        if let Some(candidate) = &self.candidate_location {
            write!(
                formatter,
                " using candidate prefix operation path {:?}",
                candidate.operation_path()
            )?;
        }
        if let Some(path) = &self.failed_path {
            write!(formatter, " while reading object operation path {path:?}")?;
        }
        write!(formatter, ": {}", self.cause.label())
    }
}

impl<E> fmt::Debug for PackageTreeSourceAcquisitionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeSourceAcquisitionError")
            .field("source_index", &self.source_index)
            .field("binding", self.configured_source.binding())
            .field(
                "configured_operation_path",
                &self.configured_source.operation_path(),
            )
            .field(
                "candidate_prefix_operation_path",
                &self
                    .candidate_location
                    .as_ref()
                    .map(Location::operation_path),
            )
            .field("failed_path", &self.failed_path)
            .field("cause", &self.cause.label())
            .finish()
    }
}

impl<E: Error + 'static> Error for PackageTreeSourceAcquisitionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            PackageTreeSourceAcquisitionErrorCause::InvalidSourceRole(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::ResolveOperator(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::UnsupportedCapabilities { .. } => None,
            PackageTreeSourceAcquisitionErrorCause::List(source)
            | PackageTreeSourceAcquisitionErrorCause::Read(source)
            | PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::Structural(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source) => Some(source),
            PackageTreeSourceAcquisitionErrorCause::Limit(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PackageTreeSourceAcquisitionErrorCause<E> {
    InvalidSourceRole(LocationRoleError),
    ResolveOperator(E),
    UnsupportedCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    List(opendal::Error),
    Read(opendal::Error),
    ListedObjectAbsent(opendal::Error),
    Structural(PackageTreeAcquisitionSurveyError),
    InvalidPackageTree(PackageTreeError),
    Limit(PackageTreeAcquisitionLimitError),
}

impl<E> PackageTreeSourceAcquisitionErrorCause<E> {
    fn label(&self) -> &'static str {
        match self {
            Self::InvalidSourceRole(_) => "the configured source is not a prefix",
            Self::ResolveOperator(_) => "operator resolution failed",
            Self::UnsupportedCapabilities { .. } => {
                "required listing or read capability is unsupported"
            }
            Self::List(_) => "the recursive listing failed",
            Self::Read(_) => "a listed Package Tree object read failed",
            Self::ListedObjectAbsent(_) => "a listed Package Tree object was absent when read",
            Self::Structural(_) => "the completed listing had structural issues",
            Self::InvalidPackageTree(_) => "the completed listing does not form a Package Tree",
            Self::Limit(_) => "a Package Tree Acquisition limit failed",
        }
    }
}

#[cfg(test)]
pub(crate) async fn acquire_package_tree_candidates<R: OperatorResolver + ?Sized>(
    resolver: &R,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeAcquisitionLimits,
) -> Result<Option<PackageTreeAcquisition>, PackageTreeSourceAcquisitionError<R::Error>> {
    let mut resolved = ResolvedOperators::new(resolver);
    acquire_package_tree_candidates_with_resolved(&mut resolved, spec, sources, limits).await
}

async fn acquire_package_tree_candidates_with_resolved<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeAcquisitionLimits,
) -> Result<Option<PackageTreeAcquisition>, PackageTreeSourceAcquisitionError<R::Error>> {
    let child = format!("{}/", acquisition_layout::package_tree_key(spec));
    let candidates = sources
        .iter()
        .map(|source| {
            source.source.require_prefix()?;
            Ok(compose_candidate(&source.source, &child))
        })
        .collect::<Vec<_>>();
    let Some((source_index, candidate_location, objects)) =
        acquire_first_present_recursive_prefix_with_resolved(
            resolved,
            candidates,
            RecursiveAcquisitionSelection::PackageTree,
            limits.into(),
        )
        .await
        .map_err(|error| {
            PackageTreeSourceAcquisitionError::from_recursive(sources, &child, error)
        })?
    else {
        return Ok(None);
    };

    Ok(Some(PackageTreeAcquisition {
        spec: spec.clone(),
        source_index,
        configured_source: sources[source_index].source.clone(),
        candidate_location,
        entries: objects
            .into_iter()
            .map(|object| PackageTreeAcquisitionEntry {
                relative_path: object.relative_path,
                bytes: object.bytes,
            })
            .collect(),
    }))
}

fn compose_candidate(source: &Location, child: &str) -> Location {
    source
        .compose(child)
        .expect("a package key composed below a canonical prefix remains canonical")
}

impl From<PackageTreeAcquisitionLimits> for RecursiveAcquisitionLimits {
    fn from(limits: PackageTreeAcquisitionLimits) -> Self {
        Self {
            listed_entries: limits.listed_entries(),
            listed_path_bytes: limits.listed_path_bytes(),
            total_listed_path_bytes: limits.total_listed_path_bytes(),
            selected_objects: limits.selected_files(),
            object_bytes: limits.object_bytes(),
            total_bytes: limits.total_bytes(),
        }
    }
}

fn map_resource(resource: RecursiveAcquisitionResource) -> PackageTreeAcquisitionResource {
    match resource {
        RecursiveAcquisitionResource::ListedEntries => {
            PackageTreeAcquisitionResource::ListedEntries
        }
        RecursiveAcquisitionResource::ListedPathBytes => {
            PackageTreeAcquisitionResource::ListedPathBytes
        }
        RecursiveAcquisitionResource::TotalListedPathBytes => {
            PackageTreeAcquisitionResource::TotalListedPathBytes
        }
        RecursiveAcquisitionResource::SelectedObjects => {
            PackageTreeAcquisitionResource::SelectedFiles
        }
        RecursiveAcquisitionResource::ObjectBytes => PackageTreeAcquisitionResource::ObjectBytes,
        RecursiveAcquisitionResource::TotalBytes => PackageTreeAcquisitionResource::TotalBytes,
    }
}

fn map_issue(issue: RecursiveSurveyIssue) -> PackageTreeAcquisitionIssue {
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            PackageTreeAcquisitionIssue::ListedPathOutsidePrefix { operation_path }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            PackageTreeAcquisitionIssue::PrefixMarkerWhereFileRequired { operation_path }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            PackageTreeAcquisitionIssue::EmptyRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            PackageTreeAcquisitionIssue::UnsupportedEntryKind {
                operation_path,
                kind: PackageTreeAcquisitionEntryKind::Unknown,
            }
        }
        RecursiveSurveyIssueKind::InvalidRelativeOperationPath
        | RecursiveSurveyIssueKind::DuplicateListedObject => {
            unreachable!("Package Tree path issues are owned by core preflight")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::convert::Infallible;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use opendal::ErrorKind;
    use typst::syntax::package::PackageSpec;

    use crate::opendal::scripted_service::{
        Capabilities, DroppedOperation, ListEntry, ListScript, ListStep, OperationLogEntry,
        PendingPoint, ReadScript, ReadStep, ScriptedService,
    };
    use crate::opendal::{Location, OperatorBinding, OperatorBindings, OperatorResolver};
    use crate::{PackageTree, PackageTreeIssue};

    use super::{
        PackageTreeAcquisitionCeilings, PackageTreeAcquisitionLimitError,
        PackageTreeAcquisitionLimits, PackageTreeAcquisitionLimitsError,
        PackageTreeAcquisitionResource, PackageTreeSource, PackageTreeSourceAcquisitionErrorCause,
        acquire_package_tree_candidates,
    };

    #[test]
    fn empty_candidate_falls_through_and_present_candidate_stops_fallback() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new("first/preview/example/1.2.3/", 0, []).unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    2,
                    [ListStep::page([
                        ListEntry::file("second/preview/example/1.2.3/z.typ"),
                        ListEntry::file("second/preview/example/1.2.3/a.typ"),
                    ])],
                )
                .unwrap(),
            ],
            [
                ReadScript::new(
                    "second/preview/example/1.2.3/a.typ",
                    1,
                    [ReadStep::chunk(b"a")],
                )
                .unwrap(),
                ReadScript::new(
                    "second/preview/example/1.2.3/z.typ",
                    1,
                    [ReadStep::chunk(b"z")],
                )
                .unwrap(),
            ],
            16,
        );
        let binding = OperatorBinding::new("trees").unwrap();
        let resolver = CountingResolver::new(service.operator());
        let sources = [
            PackageTreeSource::new(
                Location::from_operation_path(binding.clone(), "first/").unwrap(),
            ),
            PackageTreeSource::new(Location::from_operation_path(binding, "second/").unwrap()),
            PackageTreeSource::new("unreached:/not-a-prefix".parse().unwrap()),
        ];

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &resolver,
            &"@preview/example:1.2.3".parse().unwrap(),
            &sources,
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();

        assert_eq!(acquisition.source_index(), 1);
        assert_eq!(acquisition.configured_source(), sources[1].source());
        assert_eq!(
            acquisition.candidate_location().operation_path(),
            "second/preview/example/1.2.3/"
        );
        assert_eq!(
            acquisition
                .entries()
                .iter()
                .map(|entry| (entry.relative_path(), entry.bytes()))
                .collect::<Vec<_>>(),
            [("a.typ", b"a".as_slice()), ("z.typ", b"z".as_slice())]
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            2
        );
        assert_eq!(resolver.calls(), 1);
    }

    #[test]
    fn named_limits_keep_the_finite_reference_profile_and_validate_payload_ceilings() {
        let reference = PackageTreeAcquisitionCeilings::reference_v1();
        assert_eq!(reference.listed_entries, 100_000);
        assert_eq!(reference.listed_path_bytes, 64 * 1024);
        assert_eq!(reference.total_listed_path_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.selected_files, 50_000);
        assert_eq!(reference.object_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.total_bytes, 512 * 1024 * 1024);

        let narrowed = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: u64::MAX,
            listed_path_bytes: u64::MAX,
            total_listed_path_bytes: u64::MAX,
            total_bytes: reference.object_bytes,
            ..reference
        })
        .unwrap();
        assert_eq!(narrowed.listed_entries(), u64::MAX);
        assert_eq!(narrowed.listed_path_bytes(), u64::MAX);
        assert_eq!(narrowed.total_listed_path_bytes(), u64::MAX);
        assert_eq!(narrowed.selected_files(), reference.selected_files);
        assert_eq!(narrowed.object_bytes(), reference.object_bytes);
        assert_eq!(narrowed.total_bytes(), reference.object_bytes);

        for (resource, ceilings) in [
            (
                PackageTreeAcquisitionResource::ObjectBytes,
                PackageTreeAcquisitionCeilings {
                    object_bytes: u64::MAX,
                    total_bytes: u64::MAX,
                    ..reference
                },
            ),
            (
                PackageTreeAcquisitionResource::TotalBytes,
                PackageTreeAcquisitionCeilings {
                    total_bytes: u64::MAX,
                    ..reference
                },
            ),
        ] {
            assert!(matches!(
                PackageTreeAcquisitionLimits::new(ceilings),
                Err(PackageTreeAcquisitionLimitsError::CannotProbe {
                    resource: actual,
                    ceiling: u64::MAX,
                }) if actual == resource
            ));
        }
        assert!(matches!(
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 2,
                total_bytes: 1,
                ..reference
            }),
            Err(
                PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: 2,
                    total_bytes: 1,
                }
            )
        ));
    }

    #[test]
    fn tree_resources_map_shared_survey_and_payload_boundaries() {
        let reference = PackageTreeAcquisitionCeilings::reference_v1();
        let survey_cases = [
            (
                PackageTreeAcquisitionResource::ListedEntries,
                PackageTreeAcquisitionCeilings {
                    listed_entries: 0,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeAcquisitionResource::ListedPathBytes,
                PackageTreeAcquisitionCeilings {
                    listed_path_bytes: 1,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeAcquisitionResource::TotalListedPathBytes,
                PackageTreeAcquisitionCeilings {
                    total_listed_path_bytes: 0,
                    ..reference
                },
                ListEntry::file("trees/preview/example/1.2.3/a.typ"),
            ),
            (
                PackageTreeAcquisitionResource::SelectedFiles,
                PackageTreeAcquisitionCeilings {
                    selected_files: 0,
                    ..reference
                },
                ListEntry::file("trees/preview/example/1.2.3/a.typ"),
            ),
        ];
        for (resource, ceilings, entry) in survey_cases {
            let service = ScriptedService::new(
                Capabilities::all(),
                [
                    ListScript::new("trees/preview/example/1.2.3/", 1, [ListStep::page([entry])])
                        .unwrap(),
                ],
                [],
                8,
            );
            let bindings = configured(&service);
            let error = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeAcquisitionLimits::new(ceilings).unwrap(),
            )))
            .unwrap_err();
            assert!(matches!(
                error.cause(),
                PackageTreeSourceAcquisitionErrorCause::Limit(
                    PackageTreeAcquisitionLimitError::Exceeded {
                        resource: actual,
                        ..
                    }
                ) if *actual == resource
            ));
        }

        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                "trees/preview/example/1.2.3/",
                1,
                [ListStep::page([ListEntry::file(
                    "trees/preview/example/1.2.3/a.typ",
                )])],
            )
            .unwrap()],
            [ReadScript::new(
                "trees/preview/example/1.2.3/a.typ",
                1,
                [ReadStep::chunk(b"four")],
            )
            .unwrap()],
            8,
        );
        let bindings = configured(&service);
        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 3,
                total_bytes: 8,
                ..reference
            })
            .unwrap(),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::ObjectBytes,
                    ceiling: 3,
                    observed_at_least: 4,
                }
            )
        ));

        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                "trees/preview/example/1.2.3/",
                2,
                [ListStep::page([
                    ListEntry::file("trees/preview/example/1.2.3/a.typ"),
                    ListEntry::file("trees/preview/example/1.2.3/b.typ"),
                ])],
            )
            .unwrap()],
            [
                ReadScript::new(
                    "trees/preview/example/1.2.3/a.typ",
                    1,
                    [ReadStep::chunk(b"12")],
                )
                .unwrap(),
                ReadScript::new(
                    "trees/preview/example/1.2.3/b.typ",
                    1,
                    [ReadStep::chunk(b"34")],
                )
                .unwrap(),
            ],
            12,
        );
        let bindings = configured(&service);
        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
                object_bytes: 3,
                total_bytes: 3,
                ..reference
            })
            .unwrap(),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::TotalBytes,
                    ceiling: 3,
                    observed_at_least: 4,
                }
            )
        ));
    }

    #[test]
    fn listing_limits_are_shared_across_absent_candidates() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(
                    "first/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "first/preview/example/1.2.3/dir/",
                    )])],
                )
                .unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "second/preview/example/1.2.3/long-directory/",
                    )])],
                )
                .unwrap(),
            ],
            [],
            8,
        );
        let bindings = configured(&service);
        let limits = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: 1,
            ..PackageTreeAcquisitionCeilings::reference_v1()
        })
        .unwrap();

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            limits,
        )))
        .unwrap_err();

        assert_eq!(error.source_index(), 1);
        assert!(matches!(
            error.cause(),
            PackageTreeSourceAcquisitionErrorCause::Limit(
                PackageTreeAcquisitionLimitError::Exceeded {
                    resource: PackageTreeAcquisitionResource::ListedEntries,
                    ceiling: 1,
                    observed_at_least: 2,
                }
            )
        ));
    }

    #[test]
    fn listing_permutations_preserve_canonical_order_and_exact_boundaries() {
        let candidate = "trees/preview/example/1.2.3/";
        let paths = [format!("{candidate}a"), format!("{candidate}b")];
        for entries in [
            [ListEntry::file(&paths[0]), ListEntry::file(&paths[1])],
            [ListEntry::file(&paths[1]), ListEntry::file(&paths[0])],
        ] {
            let service = ScriptedService::new(
                Capabilities::all(),
                [ListScript::new(candidate, 2, [ListStep::page(entries)]).unwrap()],
                [
                    ReadScript::new(&paths[0], 1, [ReadStep::chunk(b"a")]).unwrap(),
                    ReadScript::new(&paths[1], 1, [ReadStep::chunk(b"b")]).unwrap(),
                ],
                12,
            );
            let bindings = configured(&service);
            let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeAcquisitionLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();
            assert_eq!(
                acquisition
                    .entries()
                    .iter()
                    .map(|entry| entry.relative_path())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
        }

        let object = format!("{candidate}a");
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])])
                    .unwrap(),
            ],
            [ReadScript::new(&object, 1, [ReadStep::chunk(b"a")]).unwrap()],
            8,
        );
        let exact = PackageTreeAcquisitionLimits::new(PackageTreeAcquisitionCeilings {
            listed_entries: 1,
            listed_path_bytes: 29,
            total_listed_path_bytes: 33,
            selected_files: 1,
            object_bytes: 1,
            total_bytes: 1,
        })
        .unwrap();
        let bindings = configured(&service);
        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            exact,
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.entries()[0].bytes(), b"a");
    }

    #[test]
    fn completed_empty_observations_exhaust_to_absence() {
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new("first/preview/example/1.2.3/", 0, []).unwrap(),
                ListScript::new(
                    "second/preview/example/1.2.3/",
                    1,
                    [ListStep::page([ListEntry::directory(
                        "second/preview/example/1.2.3/empty/",
                    )])],
                )
                .unwrap(),
            ],
            [],
            8,
        );
        let bindings = configured(&service);

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap();

        assert!(acquisition.is_none());
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn core_preflight_canonicalizes_before_reads_and_owned_entries_build_the_final_tree() {
        let candidate = "trees/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                2,
                [ListStep::page([
                    ListEntry::file(format!("{candidate}./lib.typ")),
                    ListEntry::file(format!("{candidate}empty.typ")),
                ])],
            )
            .unwrap()],
            [
                ReadScript::new(
                    format!("{candidate}./lib.typ"),
                    1,
                    [ReadStep::chunk(b"library")],
                )
                .unwrap(),
                ReadScript::new(format!("{candidate}empty.typ"), 0, []).unwrap(),
            ],
            12,
        );
        let bindings = configured(&service);

        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.spec(), &spec());
        assert_eq!(acquisition.entries()[0].relative_path(), "empty.typ");
        assert_eq!(acquisition.entries()[0].len(), 0);
        assert!(acquisition.entries()[0].is_empty());
        assert_eq!(acquisition.entries()[1].relative_path(), "lib.typ");

        let (actual_spec, index, configured, candidate, entries) = acquisition.into_parts();
        assert_eq!(actual_spec, spec());
        assert_eq!(index, 0);
        assert_eq!(configured.operation_path(), "trees/");
        assert_eq!(candidate.operation_path(), "trees/preview/example/1.2.3/");
        let tree = PackageTree::from_owned_entries(
            entries
                .into_iter()
                .map(super::PackageTreeAcquisitionEntry::into_parts),
        )
        .unwrap();
        assert_eq!(tree.file("empty.typ"), Some(b"".as_slice()));
        assert_eq!(tree.file("lib.typ"), Some(b"library".as_slice()));
    }

    #[test]
    fn core_package_tree_conflicts_are_typed_and_terminal_before_reads() {
        let first = "first/preview/example/1.2.3/";
        let second = "second/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(
                    first,
                    2,
                    [ListStep::page([
                        ListEntry::file(format!("{first}assets")),
                        ListEntry::file(format!("{first}assets/logo.svg")),
                    ])],
                )
                .unwrap(),
                ListScript::new(
                    second,
                    1,
                    [ListStep::page([ListEntry::file(format!(
                        "{second}unreached.typ"
                    ))])],
                )
                .unwrap(),
            ],
            [],
            12,
        );
        let bindings = configured(&service);

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();

        let PackageTreeSourceAcquisitionErrorCause::InvalidPackageTree(source) = error.cause()
        else {
            panic!("unexpected cause: {:?}", error.cause());
        };
        assert_eq!(
            source.issues(),
            [PackageTreeIssue::PathTreeConflict {
                ancestor: "assets".to_owned(),
                descendant: "assets/logo.svg".to_owned(),
            }]
        );
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
        assert!(
            service
                .log()
                .entries()
                .iter()
                .all(|entry| !matches!(entry, OperationLogEntry::ReadInvoked { .. }))
        );
    }

    #[test]
    fn envelope_issues_are_aggregated_and_do_not_reach_lower_candidates() {
        let candidate = "trees/preview/example/1.2.3/";
        let service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                2,
                [ListStep::page([
                    ListEntry::unknown(format!("{candidate}unknown")),
                    ListEntry::file("outside/file.typ"),
                ])],
            )
            .unwrap()],
            [],
            8,
        );
        let bindings = configured(&service);

        let error = expect_ready(pin!(acquire_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        let PackageTreeSourceAcquisitionErrorCause::Structural(survey) = error.cause() else {
            panic!("unexpected cause: {:?}", error.cause());
        };
        assert_eq!(survey.issues().len(), 2);
        assert!(matches!(
            &survey.issues()[0],
            super::PackageTreeAcquisitionIssue::ListedPathOutsidePrefix { operation_path }
                if operation_path == "outside/file.typ"
        ));
        assert_eq!(
            service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn mutation_is_observed_but_disappearance_and_list_not_found_are_terminal() {
        let candidate = "trees/preview/example/1.2.3/";
        let changing = format!("{candidate}changing.typ");
        let replacement =
            ReadScript::new(&changing, 1, [ReadStep::chunk(b"bytes after listing")]).unwrap();
        let mutation_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                1,
                [
                    ListStep::page([ListEntry::file(&changing)]),
                    ListStep::replace_read(replacement),
                ],
            )
            .unwrap()],
            [ReadScript::new(&changing, 1, [ReadStep::chunk(b"bytes during listing")]).unwrap()],
            8,
        );
        let mutation_bindings = configured(&mutation_service);
        let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
            &mutation_bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(acquisition.entries()[0].bytes(), b"bytes after listing");

        let absent_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(
                candidate,
                1,
                [ListStep::page([ListEntry::file(format!(
                    "{candidate}gone.typ"
                ))])],
            )
            .unwrap()],
            [],
            8,
        );
        let absent_bindings = configured(&absent_service);
        let absent = expect_ready(pin!(acquire_package_tree_candidates(
            &absent_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        assert_eq!(
            absent.failed_path(),
            Some("trees/preview/example/1.2.3/gone.typ")
        );
        assert!(matches!(
            absent.cause(),
            PackageTreeSourceAcquisitionErrorCause::ListedObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let list_failure_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(candidate, 0, [ListStep::failure(ErrorKind::NotFound)]).unwrap()],
            [],
            4,
        );
        let list_failure_bindings = configured(&list_failure_service);
        let list_failure = expect_ready(pin!(acquire_package_tree_candidates(
            &list_failure_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeAcquisitionLimits::reference_v1(),
        )))
        .unwrap_err();
        assert!(matches!(
            list_failure.cause(),
            PackageTreeSourceAcquisitionErrorCause::List(source)
                if source.kind() == ErrorKind::NotFound
        ));
    }

    #[test]
    fn cancellation_drops_the_reached_operation_without_reaching_fallback() {
        let candidate = "trees/preview/example/1.2.3/";
        let list_pending = PendingPoint::new();
        let list_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(candidate, 0, [ListStep::pending(list_pending.clone())]).unwrap()],
            [],
            4,
        );
        let list_bindings = configured(&list_service);
        let sources = [source("trees/"), source("unreached/")];
        {
            let requested_spec = spec();
            let mut acquisition = pin!(acquire_package_tree_candidates(
                &list_bindings,
                &requested_spec,
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(list_pending.was_observed());
        }
        assert_eq!(
            list_service.cancellations(),
            [DroppedOperation::List {
                id: 0,
                path: candidate.to_owned(),
            }]
        );

        let read_pending = PendingPoint::new();
        let object = format!("{candidate}pending.typ");
        let read_service = ScriptedService::new(
            Capabilities::all(),
            [
                ListScript::new(candidate, 1, [ListStep::page([ListEntry::file(&object)])])
                    .unwrap(),
            ],
            [ReadScript::new(&object, 0, [ReadStep::pending(read_pending.clone())]).unwrap()],
            8,
        );
        let read_bindings = configured(&read_service);
        {
            let requested_spec = spec();
            let mut acquisition = pin!(acquire_package_tree_candidates(
                &read_bindings,
                &requested_spec,
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(acquisition.as_mut()), Poll::Pending));
            assert!(read_pending.was_observed());
        }
        assert_eq!(
            read_service.cancellations(),
            [DroppedOperation::Read {
                id: 1,
                path: object,
            }]
        );
        assert_eq!(
            read_service
                .log()
                .entries()
                .iter()
                .filter(|entry| matches!(entry, OperationLogEntry::ListInvoked { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn memory_acquires_candidates_below_root_and_non_root_configured_prefixes() {
        for (configured, object, expected_candidate) in [
            (
                "",
                "preview/example/1.2.3/lib.typ",
                "preview/example/1.2.3/",
            ),
            (
                "packages/",
                "packages/preview/example/1.2.3/lib.typ",
                "packages/preview/example/1.2.3/",
            ),
        ] {
            let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
            expect_ready(pin!(operator.write(object, b"memory package".to_vec()))).unwrap();
            let binding = OperatorBinding::new("trees").unwrap();
            let bindings = OperatorBindings::new([(binding.clone(), operator)]).unwrap();
            let sources = [PackageTreeSource::new(
                Location::from_operation_path(binding, configured).unwrap(),
            )];

            let acquisition = expect_ready(pin!(acquire_package_tree_candidates(
                &bindings,
                &spec(),
                &sources,
                PackageTreeAcquisitionLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();

            assert_eq!(
                acquisition.candidate_location().operation_path(),
                expected_candidate
            );
            assert_eq!(acquisition.entries()[0].relative_path(), "lib.typ");
            assert_eq!(acquisition.entries()[0].bytes(), b"memory package");
        }
    }

    fn expect_ready<F: Future>(mut future: std::pin::Pin<&mut F>) -> F::Output {
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    fn spec() -> PackageSpec {
        "@preview/example:1.2.3".parse().unwrap()
    }

    fn source(path: &str) -> PackageTreeSource {
        PackageTreeSource::new(
            Location::from_operation_path(OperatorBinding::new("trees").unwrap(), path).unwrap(),
        )
    }

    fn configured(service: &ScriptedService) -> OperatorBindings {
        OperatorBindings::new([(OperatorBinding::new("trees").unwrap(), service.operator())])
            .unwrap()
    }

    struct CountingResolver {
        calls: Cell<usize>,
        operator: opendal::Operator,
    }

    impl CountingResolver {
        fn new(operator: opendal::Operator) -> Self {
            Self {
                calls: Cell::new(0),
                operator,
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl OperatorResolver for CountingResolver {
        type Error = Infallible;

        fn resolve(&self, _: &OperatorBinding) -> Result<opendal::Operator, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.operator.clone())
        }
    }
}
