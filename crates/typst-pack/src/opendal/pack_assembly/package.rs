use std::fmt;

use typst::syntax::package::PackageSpec;

use super::super::BoxError;
use super::super::acquisition::recursive::{
    PackageTreeRecursiveAcquisitionOperation, RecursiveAcquisitionLimits,
    RecursiveAcquisitionOperation, RecursiveAcquisitionResource, RecursiveSurveyIssue,
    RecursiveSurveyIssueKind, acquire_first_present_package_tree_prefix_with_resolved,
};
use super::super::acquisition::{
    ExactPathAcquisitionOperation, ResolvedOperators, acquire_exact_path,
};
use super::super::{Location, LocationRoleError, OperatorResolver};
use crate::acquisition_layout;
use crate::limits::{LimitError, Limits, LimitsError, ResourceKind};
use crate::package_catalog::PackageTreeError;
use crate::redacted_error::RedactedError;

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
pub type PackageTreeAcquisitionResource = ResourceKind<11>;

#[allow(non_upper_case_globals)]
impl ResourceKind<11> {
    /// Entries yielded by recursive listings.
    pub const ListedEntries: Self = Self::new(0);
    /// Bytes in one yielded operation path.
    pub const ListedPathBytes: Self = Self::new(1);
    /// Bytes retained for paths and structural evidence.
    pub const TotalListedPathBytes: Self = Self::new(2);
    /// Selected file objects.
    pub const SelectedFiles: Self = Self::new(3);
    /// Bytes retained for one selected file.
    pub const ObjectBytes: Self = Self::new(4);
    /// Bytes retained across the selected tree.
    pub const TotalBytes: Self = Self::new(5);
}

/// A supplied Package Tree Acquisition ceiling is internally inconsistent.
pub type PackageTreeAcquisitionLimitsError = LimitsError<PackageTreeAcquisitionResource>;

/// Mandatory finite limits for OpenDAL Package Tree Acquisition.
pub type PackageTreeAcquisitionLimits = Limits<PackageTreeAcquisitionResource>;

impl Limits<PackageTreeAcquisitionResource> {
    /// Validates every named Package Tree Acquisition ceiling.
    pub fn new(
        ceilings: PackageTreeAcquisitionCeilings,
    ) -> Result<Self, PackageTreeAcquisitionLimitsError> {
        let limits = Self::from_ceilings([
            ceilings.listed_entries,
            ceilings.listed_path_bytes,
            ceilings.total_listed_path_bytes,
            ceilings.selected_files,
            ceilings.object_bytes,
            ceilings.total_bytes,
            0,
        ])
        .validate_probe_resources([
            PackageTreeAcquisitionResource::ObjectBytes,
            PackageTreeAcquisitionResource::TotalBytes,
        ])?;
        if ceilings.object_bytes > ceilings.total_bytes {
            return Err(
                PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
                    object_bytes: ceilings.object_bytes,
                    total_bytes: ceilings.total_bytes,
                },
            );
        }
        Ok(limits)
    }

    /// The validated first-party version-1 limits.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            100_000,
            64 * 1024,
            64 * 1024 * 1024,
            50_000,
            64 * 1024 * 1024,
            512 * 1024 * 1024,
            0,
        ])
    }

    /// The maximum number of entries yielded across attempted tree sources.
    pub const fn listed_entries(&self) -> u64 {
        self.ceilings[0]
    }

    /// The maximum byte length of one yielded operation path.
    pub const fn listed_path_bytes(&self) -> u64 {
        self.ceilings[1]
    }

    /// The maximum retained bytes for paths and structural evidence.
    pub const fn total_listed_path_bytes(&self) -> u64 {
        self.ceilings[2]
    }

    /// The maximum selected file count.
    pub const fn selected_files(&self) -> u64 {
        self.ceilings[3]
    }

    /// The maximum exact bytes retained for one selected file.
    pub const fn object_bytes(&self) -> u64 {
        self.ceilings[4]
    }

    /// The maximum exact bytes retained for the selected Package Tree.
    pub const fn total_bytes(&self) -> u64 {
        self.ceilings[5]
    }
}

/// Package Tree Acquisition exceeded or could not account for a mandatory limit.
pub type PackageTreeAcquisitionLimitError = LimitError<PackageTreeAcquisitionResource>;

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
pub type PackageArchiveAcquisitionResource = ResourceKind<12>;

#[allow(non_upper_case_globals)]
impl ResourceKind<12> {
    /// Exact raw Package Archive bytes.
    pub const ArchiveBytes: Self = Self::new(0);
}

/// A supplied Package Archive Acquisition ceiling is invalid.
pub type PackageArchiveAcquisitionLimitsError = LimitsError<PackageArchiveAcquisitionResource>;

/// Mandatory finite limits for one raw Package Archive Acquisition.
pub type PackageArchiveAcquisitionLimits = Limits<PackageArchiveAcquisitionResource>;

impl Limits<PackageArchiveAcquisitionResource> {
    /// Validates the named raw archive ceiling.
    pub fn new(
        ceilings: PackageArchiveAcquisitionCeilings,
    ) -> Result<Self, PackageArchiveAcquisitionLimitsError> {
        Self::from_ceilings([ceilings.archive_bytes, 0, 0, 0, 0, 0, 0])
            .validate_probe_resources([PackageArchiveAcquisitionResource::ArchiveBytes])
    }

    /// The validated first-party version-1 limits.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([128 * 1024 * 1024, 0, 0, 0, 0, 0, 0])
    }

    /// The maximum exact raw archive bytes retained from one candidate.
    pub const fn archive_bytes(&self) -> u64 {
        self.ceilings[0]
    }
}

/// A raw Package Archive exceeded or could not account for its limit.
pub type PackageArchiveAcquisitionLimitError = LimitError<PackageArchiveAcquisitionResource>;

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

/// A resource bounded across Package Acquisition fallback.
pub type PackageAcquisitionResource = ResourceKind<13>;

#[allow(non_upper_case_globals)]
impl ResourceKind<13> {
    pub const TreeListedEntries: Self = Self::new(0);
    pub const TreeListedPathBytes: Self = Self::new(1);
    pub const TreeTotalListedPathBytes: Self = Self::new(2);
    pub const TreeSelectedFiles: Self = Self::new(3);
    pub const TreeObjectBytes: Self = Self::new(4);
    pub const TreeTotalBytes: Self = Self::new(5);
    pub const ArchiveBytes: Self = Self::new(6);
}

/// A supplied Package Acquisition limit family is invalid.
pub type PackageAcquisitionLimitsError = LimitsError<PackageAcquisitionResource>;

/// Mandatory finite limits for Package Acquisition fallback.
pub type PackageAcquisitionLimits = Limits<PackageAcquisitionResource>;

impl Limits<PackageAcquisitionResource> {
    /// Validates both Package Acquisition limit families.
    pub fn new(
        ceilings: PackageAcquisitionCeilings,
    ) -> Result<Self, PackageAcquisitionLimitsError> {
        let trees =
            PackageTreeAcquisitionLimits::new(ceilings.trees).map_err(map_tree_limits_error)?;
        let archives = PackageArchiveAcquisitionLimits::new(ceilings.archives)
            .map_err(map_archive_limits_error)?;
        Ok(Self::from_ceilings([
            trees.listed_entries(),
            trees.listed_path_bytes(),
            trees.total_listed_path_bytes(),
            trees.selected_files(),
            trees.object_bytes(),
            trees.total_bytes(),
            archives.archive_bytes(),
        ]))
    }

    /// Limits shared across ordered Package Tree candidates.
    pub const fn trees(&self) -> PackageTreeAcquisitionLimits {
        PackageTreeAcquisitionLimits::from_ceilings([
            self.ceilings[0],
            self.ceilings[1],
            self.ceilings[2],
            self.ceilings[3],
            self.ceilings[4],
            self.ceilings[5],
            0,
        ])
    }

    /// Limits applied independently to cache and registry candidates.
    pub const fn archives(&self) -> PackageArchiveAcquisitionLimits {
        PackageArchiveAcquisitionLimits::from_ceilings([self.ceilings[6], 0, 0, 0, 0, 0, 0])
    }

    /// The validated first-party version-1 composite limits.
    pub const fn reference_v1() -> Self {
        Self::from_ceilings([
            100_000,
            64 * 1024,
            64 * 1024 * 1024,
            50_000,
            64 * 1024 * 1024,
            512 * 1024 * 1024,
            128 * 1024 * 1024,
        ])
    }
}

fn map_tree_limits_error(
    error: PackageTreeAcquisitionLimitsError,
) -> PackageAcquisitionLimitsError {
    match error {
        PackageTreeAcquisitionLimitsError::CannotProbe { resource, ceiling } => {
            PackageAcquisitionLimitsError::CannotProbe {
                resource: match resource {
                    PackageTreeAcquisitionResource::ListedEntries => {
                        PackageAcquisitionResource::TreeListedEntries
                    }
                    PackageTreeAcquisitionResource::ListedPathBytes => {
                        PackageAcquisitionResource::TreeListedPathBytes
                    }
                    PackageTreeAcquisitionResource::TotalListedPathBytes => {
                        PackageAcquisitionResource::TreeTotalListedPathBytes
                    }
                    PackageTreeAcquisitionResource::SelectedFiles => {
                        PackageAcquisitionResource::TreeSelectedFiles
                    }
                    PackageTreeAcquisitionResource::ObjectBytes => {
                        PackageAcquisitionResource::TreeObjectBytes
                    }
                    PackageTreeAcquisitionResource::TotalBytes => {
                        PackageAcquisitionResource::TreeTotalBytes
                    }
                    _ => unreachable!("unknown Package Tree Acquisition resource"),
                },
                ceiling,
            }
        }
        PackageTreeAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
            object_bytes,
            total_bytes,
        } => PackageAcquisitionLimitsError::ObjectBytesExceedTotalBytes {
            object_bytes,
            total_bytes,
        },
        _ => unreachable!("unrelated Package Tree Acquisition limits error"),
    }
}

fn map_archive_limits_error(
    error: PackageArchiveAcquisitionLimitsError,
) -> PackageAcquisitionLimitsError {
    match error {
        PackageArchiveAcquisitionLimitsError::CannotProbe {
            resource: PackageArchiveAcquisitionResource::ArchiveBytes,
            ceiling,
        } => PackageAcquisitionLimitsError::CannotProbe {
            resource: PackageAcquisitionResource::ArchiveBytes,
            ceiling,
        },
        _ => unreachable!("unrelated Package Archive Acquisition limits error"),
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
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "Package Acquisition request for {spec} was rejected with {issue_count} issue(s)",
    issue_count = .issues.len()
)]
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
) -> Result<PackageAcquisition, PackageAcquisitionError> {
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
        Err(error) => return Err(error),
    }

    if let Some(configured_source) = request.archive_cache() {
        let candidate_location = compose_candidate(
            configured_source,
            &acquisition_layout::package_archive_cache_key(request.spec()),
        );
        match acquire_archive_candidate(
            &mut resolved,
            request.spec(),
            configured_source,
            &candidate_location,
            ArchiveSource::Cache,
            request.limits().archives().archive_bytes(),
        )
        .await
        {
            Ok(Some(bytes)) => {
                return Ok(PackageAcquisition::CachedArchive(
                    CachedPackageArchiveAcquisition {
                        spec: request.spec().clone(),
                        configured_source: configured_source.clone(),
                        candidate_location,
                        bytes,
                    },
                ));
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if let (Some(configured_source), Some(registry_key)) = (
        request.registry(),
        acquisition_layout::official_registry_archive_key(request.spec()),
    ) {
        let candidate_location = compose_candidate(configured_source, &registry_key);
        match acquire_archive_candidate(
            &mut resolved,
            request.spec(),
            configured_source,
            &candidate_location,
            ArchiveSource::Registry,
            request.limits().archives().archive_bytes(),
        )
        .await
        {
            Ok(Some(bytes)) => {
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
            Ok(None) => {}
            Err(error) => return Err(error),
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

#[derive(Clone, Copy)]
enum ArchiveSource {
    Cache,
    Registry,
}

async fn acquire_archive_candidate<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    spec: &PackageSpec,
    configured_source: &Location,
    candidate_location: &Location,
    archive_source: ArchiveSource,
    ceiling: u64,
) -> Result<Option<Vec<u8>>, PackageAcquisitionError> {
    debug_assert!(candidate_location.require_object().is_ok());
    let operator = resolved
        .resolve(candidate_location.binding())
        .map_err(|source| {
            PackageAcquisitionError::from_archive(
                spec,
                configured_source.clone(),
                candidate_location.clone(),
                PackageAcquisitionErrorCause::ResolveOperator(Box::new(source)),
            )
        })?;
    if !operator.read {
        return Err(PackageAcquisitionError::from_archive(
            spec,
            configured_source.clone(),
            candidate_location.clone(),
            PackageAcquisitionErrorCause::UnsupportedArchiveRead,
        ));
    }

    acquire_exact_path(
        &operator.operator,
        candidate_location.dispatch_path(),
        ceiling,
        ceiling,
        &PackageArchiveExactPathOperation {
            spec,
            configured_source,
            candidate_location,
            archive_source,
        },
    )
    .await
}

struct PackageArchiveExactPathOperation<'a> {
    spec: &'a PackageSpec,
    configured_source: &'a Location,
    candidate_location: &'a Location,
    archive_source: ArchiveSource,
}

impl PackageArchiveExactPathOperation<'_> {
    fn error(&self, cause: PackageAcquisitionErrorCause) -> PackageAcquisitionError {
        PackageAcquisitionError::from_archive(
            self.spec,
            self.configured_source.clone(),
            self.candidate_location.clone(),
            cause,
        )
    }
}

impl ExactPathAcquisitionOperation for PackageArchiveExactPathOperation<'_> {
    type Error = PackageAcquisitionError;

    fn read(&self, source: ::opendal::Error) -> PackageAcquisitionError {
        self.error(match self.archive_source {
            ArchiveSource::Cache => PackageAcquisitionErrorCause::CacheRead(source),
            ArchiveSource::Registry => PackageAcquisitionErrorCause::RegistryRead(source),
        })
    }

    fn limit_exceeded(&self, ceiling: u64, _: u64) -> PackageAcquisitionError {
        self.error(PackageAcquisitionErrorCause::ArchiveLimit(
            PackageArchiveAcquisitionLimitError::exceeded(
                PackageArchiveAcquisitionResource::ArchiveBytes,
                ceiling,
            ),
        ))
    }

    fn accounting_overflow(&self) -> PackageAcquisitionError {
        self.error(PackageAcquisitionErrorCause::ArchiveLimit(
            PackageArchiveAcquisitionLimitError::AccountingOverflow {
                resource: PackageArchiveAcquisitionResource::ArchiveBytes,
            },
        ))
    }
}

/// A terminal failure while acquiring one package through OpenDAL.
#[derive(Debug, thiserror::Error)]
#[error(
    "Package Acquisition failed for {spec}{tree_source}{candidate}: {cause}",
    tree_source = package_tree_source_context(.source_index),
    candidate = package_candidate_context(.candidate_location.as_ref())
)]
pub struct PackageAcquisitionError {
    spec: PackageSpec,
    source_index: Option<usize>,
    configured_source: Option<Location>,
    candidate_location: Option<Location>,
    failed_path: Option<String>,
    failure: crate::PackageAcquisitionFailure,
    #[source]
    cause: RedactedError<PackageAcquisitionErrorCause>,
}

impl PackageAcquisitionError {
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
    pub fn cause(&self) -> &PackageAcquisitionErrorCause {
        self.cause.inner()
    }

    fn from_tree(
        spec: &PackageSpec,
        sources: &[PackageTreeSource],
        child: &str,
        source_index: usize,
        failed_path: Option<String>,
        cause: PackageAcquisitionErrorCause,
    ) -> Self {
        let configured_source = sources[source_index].source.clone();
        let candidate_location = configured_source
            .require_prefix()
            .is_ok()
            .then(|| compose_candidate(&configured_source, child));
        Self {
            spec: spec.clone(),
            source_index: Some(source_index),
            configured_source: Some(configured_source),
            candidate_location,
            failed_path,
            failure: other_failure(spec),
            cause: RedactedError::new(cause),
        }
    }

    fn from_archive(
        spec: &PackageSpec,
        configured_source: Location,
        candidate_location: Location,
        cause: PackageAcquisitionErrorCause,
    ) -> Self {
        Self {
            spec: spec.clone(),
            source_index: None,
            configured_source: Some(configured_source),
            candidate_location: Some(candidate_location),
            failed_path: None,
            failure: other_failure(spec),
            cause: RedactedError::new(cause),
        }
    }
}

fn package_tree_source_context(source_index: &Option<usize>) -> String {
    source_index
        .map(|source_index| format!(" at tree source {source_index}"))
        .unwrap_or_default()
}

fn package_candidate_context(candidate: Option<&Location>) -> String {
    candidate
        .map(|candidate| format!(" at candidate {candidate}"))
        .unwrap_or_default()
}

/// The typed cause of a terminal OpenDAL Package Acquisition failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionErrorCause {
    /// The reached binding could not be resolved.
    #[error("operator resolution failed")]
    ResolveOperator(#[source] BoxError),
    /// A reached tree binding cannot recursively list or read selected files.
    #[error("required Package Tree capabilities are unsupported")]
    UnsupportedTreeCapabilities {
        list: bool,
        list_with_recursive: bool,
        read: bool,
    },
    /// A reached raw archive binding cannot read objects.
    #[error("Package Archive read capability is unsupported")]
    UnsupportedArchiveRead,
    /// A Package Tree recursive listing failed.
    #[error("the Package Tree listing failed")]
    TreeList(#[source] ::opendal::Error),
    /// A listed Package Tree object read failed.
    #[error("a Package Tree object read failed")]
    TreeRead(#[source] ::opendal::Error),
    /// A listed Package Tree object became absent when read.
    #[error("a listed Package Tree object became absent")]
    ListedTreeObjectAbsent(#[source] ::opendal::Error),
    /// The raw Package Archive cache read failed.
    #[error("the Package Archive cache read failed")]
    CacheRead(#[source] ::opendal::Error),
    /// The official Package Registry read failed.
    #[error("the Package Registry read failed")]
    RegistryRead(#[source] ::opendal::Error),
    /// A completed Package Tree listing had envelope issues.
    #[error("the Package Tree listing had structural issues")]
    TreeStructural(#[source] PackageTreeAcquisitionSurveyError),
    /// Listed paths do not form a valid Package Tree.
    #[error("the listed objects do not form a Package Tree")]
    InvalidPackageTree(#[source] PackageTreeError),
    /// Package Tree Acquisition exceeded a mandatory limit.
    #[error("a Package Tree Acquisition limit failed")]
    TreeLimit(#[source] PackageTreeAcquisitionLimitError),
    /// Raw Package Archive Acquisition exceeded a mandatory limit.
    #[error("a Package Archive Acquisition limit failed")]
    ArchiveLimit(#[source] PackageArchiveAcquisitionLimitError),
}

struct PackageTreeAcquisitionOperation<'a> {
    spec: &'a PackageSpec,
    sources: &'a [PackageTreeSource],
    child: &'a str,
}

impl PackageTreeAcquisitionOperation<'_> {
    fn error(
        &self,
        source_index: usize,
        failed_path: Option<String>,
        cause: PackageAcquisitionErrorCause,
    ) -> PackageAcquisitionError {
        PackageAcquisitionError::from_tree(
            self.spec,
            self.sources,
            self.child,
            source_index,
            failed_path,
            cause,
        )
    }
}

impl RecursiveAcquisitionOperation for PackageTreeAcquisitionOperation<'_> {
    type Error = PackageAcquisitionError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> PackageAcquisitionError {
        unreachable!("PackageAcquisitionRequest validates every tree prefix")
    }

    fn resolve_operator(&self, source_index: usize, source: BoxError) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        source_index: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::UnsupportedTreeCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, source_index: usize, source: ::opendal::Error) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::TreeList(source),
        )
    }

    fn read(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            Some(operation_path),
            PackageAcquisitionErrorCause::TreeRead(source),
        )
    }

    fn listed_object_absent(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            Some(operation_path),
            PackageAcquisitionErrorCause::ListedTreeObjectAbsent(source),
        )
    }

    fn structural(
        &self,
        source_index: usize,
        issues: Vec<RecursiveSurveyIssue>,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::TreeStructural(PackageTreeAcquisitionSurveyError {
                issues: issues.into_iter().map(map_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        source_index: usize,
        resource: RecursiveAcquisitionResource,
        ceiling: u64,
        _: u64,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::TreeLimit(PackageTreeAcquisitionLimitError::exceeded(
                map_resource(resource),
                ceiling,
            )),
        )
    }

    fn accounting_overflow(
        &self,
        source_index: usize,
        resource: RecursiveAcquisitionResource,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::TreeLimit(
                PackageTreeAcquisitionLimitError::AccountingOverflow {
                    resource: map_resource(resource),
                },
            ),
        )
    }
}

impl PackageTreeRecursiveAcquisitionOperation for PackageTreeAcquisitionOperation<'_> {
    fn invalid_package_tree(
        &self,
        source_index: usize,
        source: PackageTreeError,
    ) -> PackageAcquisitionError {
        self.error(
            source_index,
            None,
            PackageAcquisitionErrorCause::InvalidPackageTree(source),
        )
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
#[derive(Debug, thiserror::Error)]
#[error(
    "failed to insert acquired package {spec} at {target:?}",
    spec = .failure.spec()
)]
pub struct AcquiredPackageInsertionError {
    failure: Box<crate::PackageAcquisitionFailure>,
    target: AcquiredPackageInsertionTarget,
    #[source]
    cause: Box<AcquiredPackageInsertionErrorCause>,
}

#[cfg(feature = "package-acquisition")]
impl AcquiredPackageInsertionError {
    /// The exact package specification that could not be inserted.
    pub fn spec(&self) -> &PackageSpec {
        self.failure.spec()
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

/// The typed cause of an acquired-package insertion failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[cfg(feature = "package-acquisition")]
pub enum AcquiredPackageInsertionErrorCause {
    /// Acquired entries could not construct a Package Tree.
    #[error("acquired entries could not construct a Package Tree")]
    PackageTree(#[source] crate::PackageTreeError),
    /// Raw archive bytes could not expand into a Package Tree.
    #[error("raw archive bytes could not expand into a Package Tree")]
    ArchiveExpansion(#[source] Box<crate::PackageAcquisitionError>),
    /// The Package Catalog rejected the constructed tree.
    #[error("the Package Catalog rejected the constructed tree")]
    PackageCatalog(#[source] crate::PackageCatalogError),
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
            AcquiredPackageInsertionErrorCause::ArchiveExpansion(Box::new(source)),
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
        failure: Box::new(failure),
        target,
        cause: Box::new(cause),
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
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}", message = package_tree_survey_message(.issues.as_slice()))]
pub struct PackageTreeAcquisitionSurveyError {
    issues: Vec<PackageTreeAcquisitionIssue>,
}

impl PackageTreeAcquisitionSurveyError {
    /// Every independently detectable issue in canonical path and kind order.
    pub fn issues(&self) -> &[PackageTreeAcquisitionIssue] {
        &self.issues
    }
}

fn package_tree_survey_message(issues: &[PackageTreeAcquisitionIssue]) -> String {
    if let [issue] = issues {
        issue.to_string()
    } else {
        format!("Package Tree survey failed with {} issue(s)", issues.len())
    }
}

#[cfg(test)]
pub(crate) async fn acquire_package_tree_candidates<R: OperatorResolver + ?Sized>(
    resolver: &R,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeAcquisitionLimits,
) -> Result<Option<PackageTreeAcquisition>, PackageAcquisitionError> {
    let mut resolved = ResolvedOperators::new(resolver);
    acquire_package_tree_candidates_with_resolved(&mut resolved, spec, sources, limits).await
}

async fn acquire_package_tree_candidates_with_resolved<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeAcquisitionLimits,
) -> Result<Option<PackageTreeAcquisition>, PackageAcquisitionError> {
    let child = format!("{}/", acquisition_layout::package_tree_key(spec));
    let candidates = sources
        .iter()
        .map(|source| {
            source.source.require_prefix()?;
            Ok(compose_candidate(&source.source, &child))
        })
        .collect::<Vec<_>>();
    let Some((source_index, candidate_location, objects)) =
        acquire_first_present_package_tree_prefix_with_resolved(
            resolved,
            candidates,
            limits.into(),
            &PackageTreeAcquisitionOperation {
                spec,
                sources,
                child: &child,
            },
        )
        .await?
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
        Self::new(
            limits.listed_entries(),
            limits.listed_path_bytes(),
            limits.total_listed_path_bytes(),
            limits.selected_files(),
            limits.object_bytes(),
            limits.total_bytes(),
        )
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
        _ => unreachable!("unknown recursive acquisition resource"),
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
        PackageAcquisitionErrorCause, PackageTreeAcquisitionCeilings,
        PackageTreeAcquisitionLimitError, PackageTreeAcquisitionLimits,
        PackageTreeAcquisitionLimitsError, PackageTreeAcquisitionResource, PackageTreeSource,
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
                PackageAcquisitionErrorCause::TreeLimit(
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
            PackageAcquisitionErrorCause::TreeLimit(PackageTreeAcquisitionLimitError::Exceeded {
                resource: PackageTreeAcquisitionResource::ObjectBytes,
                ceiling: 3,
                observed_at_least: 4,
            })
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
            PackageAcquisitionErrorCause::TreeLimit(PackageTreeAcquisitionLimitError::Exceeded {
                resource: PackageTreeAcquisitionResource::TotalBytes,
                ceiling: 3,
                observed_at_least: 4,
            })
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

        assert_eq!(error.source_index(), Some(1));
        assert!(matches!(
            error.cause(),
            PackageAcquisitionErrorCause::TreeLimit(PackageTreeAcquisitionLimitError::Exceeded {
                resource: PackageTreeAcquisitionResource::ListedEntries,
                ceiling: 1,
                observed_at_least: 2,
            })
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

        let PackageAcquisitionErrorCause::InvalidPackageTree(source) = error.cause() else {
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
        let PackageAcquisitionErrorCause::TreeStructural(survey) = error.cause() else {
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
            PackageAcquisitionErrorCause::ListedTreeObjectAbsent(source)
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
            PackageAcquisitionErrorCause::TreeList(source)
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
