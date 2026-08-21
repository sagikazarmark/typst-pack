use std::fmt;

use typst::syntax::package::PackageSpec;

use super::super::BoxError;
use super::super::read::recursive::{
    PackageTreeRecursiveReadOperation, RecursiveReadLimits, RecursiveReadOperation,
    RecursiveReadResource, RecursiveSurveyIssue, RecursiveSurveyIssueKind,
    read_first_present_package_tree_prefix_with_resolved,
};
use super::super::read::{ExactPathReadOperation, ResolvedOperators, read_exact_path};
use super::super::{Location, LocationRoleError, OperatorResolver};
use crate::limits::{LimitError, Limits, ResourceKind};
use crate::package_catalog::PackageTreeError;
use crate::read_layout;
use crate::redacted_error::RedactedError;

/// Named finite ceilings for one OpenDAL Package Tree Read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTreeReadCeilings {
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

impl PackageTreeReadCeilings {
    /// The first-party version-1 Package Tree Read profile.
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

/// A resource bounded during OpenDAL Package Tree Read.
pub type PackageTreeReadResource = ResourceKind<11>;

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

/// Mandatory finite limits for OpenDAL Package Tree Read.
pub type PackageTreeReadLimits = Limits<PackageTreeReadResource>;

impl Limits<PackageTreeReadResource> {
    /// Validates every named Package Tree Read ceiling.
    #[track_caller]
    pub fn new(ceilings: PackageTreeReadCeilings) -> Self {
        let limits = Self::from_ceilings([
            ceilings.listed_entries,
            ceilings.listed_path_bytes,
            ceilings.total_listed_path_bytes,
            ceilings.selected_files,
            ceilings.object_bytes,
            ceilings.total_bytes,
            0,
        ])
        .assert_probe_resources([
            PackageTreeReadResource::ListedEntries,
            PackageTreeReadResource::ListedPathBytes,
            PackageTreeReadResource::TotalListedPathBytes,
            PackageTreeReadResource::SelectedFiles,
            PackageTreeReadResource::ObjectBytes,
            PackageTreeReadResource::TotalBytes,
        ]);
        assert!(
            ceilings.object_bytes <= ceilings.total_bytes,
            "the ObjectBytes ceiling {} exceeds the TotalBytes ceiling {}",
            ceilings.object_bytes,
            ceilings.total_bytes
        );
        limits
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

/// Package Tree Read exceeded or could not account for a mandatory limit.
pub type PackageTreeReadLimitError = LimitError<PackageTreeReadResource>;

/// Named finite ceilings for one raw Package Archive Read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageArchiveReadCeilings {
    /// Exact raw archive bytes retained from one candidate.
    pub archive_bytes: u64,
}

impl PackageArchiveReadCeilings {
    /// The first-party 128 MiB archive profile.
    pub const fn reference_v1() -> Self {
        Self {
            archive_bytes: 128 * 1024 * 1024,
        }
    }
}

/// A resource bounded while reading a raw Package Archive.
pub type PackageArchiveReadResource = ResourceKind<12>;

#[allow(non_upper_case_globals)]
impl ResourceKind<12> {
    /// Exact raw Package Archive bytes.
    pub const ArchiveBytes: Self = Self::new(0);
}

/// Mandatory finite limits for one raw Package Archive Read.
pub type PackageArchiveReadLimits = Limits<PackageArchiveReadResource>;

impl Limits<PackageArchiveReadResource> {
    /// Validates the named raw archive ceiling.
    #[track_caller]
    pub fn new(ceilings: PackageArchiveReadCeilings) -> Self {
        Self::from_ceilings([ceilings.archive_bytes, 0, 0, 0, 0, 0, 0])
            .assert_probe_resources([PackageArchiveReadResource::ArchiveBytes])
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
pub type PackageArchiveReadLimitError = LimitError<PackageArchiveReadResource>;

/// Named finite ceilings for Package Read fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageReadCeilings {
    /// Ceilings shared across ordered Package Tree candidates.
    pub trees: PackageTreeReadCeilings,
    /// Ceilings applied independently to cache and registry candidates.
    pub archives: PackageArchiveReadCeilings,
}

impl PackageReadCeilings {
    /// The first-party version-1 composite profile.
    pub const fn reference_v1() -> Self {
        Self {
            trees: PackageTreeReadCeilings::reference_v1(),
            archives: PackageArchiveReadCeilings::reference_v1(),
        }
    }
}

/// A resource bounded across Package Read fallback.
pub type PackageReadResource = ResourceKind<13>;

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

/// Mandatory finite limits for Package Read fallback.
pub type PackageReadLimits = Limits<PackageReadResource>;

impl Limits<PackageReadResource> {
    /// Validates both Package Read limit families.
    #[track_caller]
    pub fn new(ceilings: PackageReadCeilings) -> Self {
        let trees = PackageTreeReadLimits::new(ceilings.trees);
        let archives = PackageArchiveReadLimits::new(ceilings.archives);
        Self::from_ceilings([
            trees.listed_entries(),
            trees.listed_path_bytes(),
            trees.total_listed_path_bytes(),
            trees.selected_files(),
            trees.object_bytes(),
            trees.total_bytes(),
            archives.archive_bytes(),
        ])
    }

    /// Limits shared across ordered Package Tree candidates.
    pub const fn trees(&self) -> PackageTreeReadLimits {
        PackageTreeReadLimits::from_ceilings([
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
    pub const fn archives(&self) -> PackageArchiveReadLimits {
        PackageArchiveReadLimits::from_ceilings([self.ceilings[6], 0, 0, 0, 0, 0, 0])
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

/// A validated request to read one exact package from ordered sources.
#[derive(Clone, Debug)]
pub struct PackageReadRequest {
    spec: PackageSpec,
    tree_sources: Vec<PackageTreeSource>,
    archive_cache: Option<Location>,
    registry: Option<Location>,
    limits: PackageReadLimits,
}

impl PackageReadRequest {
    /// Validates every configured prefix role before accepting the request.
    pub fn new(
        spec: PackageSpec,
        tree_sources: impl IntoIterator<Item = PackageTreeSource>,
        archive_cache: Option<Location>,
        registry: Option<Location>,
        limits: PackageReadLimits,
    ) -> Result<Self, PackageReadRequestRejection> {
        let tree_sources = tree_sources.into_iter().collect::<Vec<_>>();
        let mut issues = tree_sources
            .iter()
            .enumerate()
            .filter_map(|(source_index, configured)| {
                configured.source.require_prefix().err().map(|source| {
                    PackageReadRequestIssue::InvalidTreeSourceRole {
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
            issues.push(PackageReadRequestIssue::InvalidArchiveCacheRole {
                location: location.clone(),
                source,
            });
        }
        if let Some(location) = &registry
            && let Err(source) = location.require_prefix()
        {
            issues.push(PackageReadRequestIssue::InvalidRegistryRole {
                location: location.clone(),
                source,
            });
        }
        if !issues.is_empty() {
            return Err(PackageReadRequestRejection { spec, issues });
        }
        Ok(Self {
            spec,
            tree_sources,
            archive_cache,
            registry,
            limits,
        })
    }

    /// The exact package specification being read.
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
    pub const fn limits(&self) -> PackageReadLimits {
        self.limits
    }
}

/// Every invalid source role in a rejected Package Read request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "Package Read request for {spec} was rejected with {issue_count} issue(s)",
    issue_count = .issues.len()
)]
pub struct PackageReadRequestRejection {
    spec: PackageSpec,
    issues: Vec<PackageReadRequestIssue>,
}

impl PackageReadRequestRejection {
    /// The exact package specification from the rejected request.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// Invalid source roles in request-field and caller-source order.
    pub fn issues(&self) -> &[PackageReadRequestIssue] {
        &self.issues
    }
}

/// One invalid source role in a Package Read request.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageReadRequestIssue {
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

/// One exact file read below a Package Tree candidate prefix.
pub struct PackageTreeReadEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

impl PackageTreeReadEntry {
    /// The canonical package-relative file path.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The exact bytes observed by the completed object read.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The exact read byte length.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Whether the read file was empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Recovers the owned path and exact bytes.
    pub fn into_parts(self) -> (String, Vec<u8>) {
        (self.relative_path, self.bytes)
    }
}

impl fmt::Debug for PackageTreeReadEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeReadEntry")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact files read from the first present Package Tree candidate.
pub struct PackageTreeRead {
    spec: PackageSpec,
    source_index: usize,
    configured_source: Location,
    candidate_location: Location,
    entries: Vec<PackageTreeReadEntry>,
}

impl PackageTreeRead {
    /// The exact package specification read.
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

    /// Read entries in canonical package-relative path order.
    pub fn entries(&self) -> &[PackageTreeReadEntry] {
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
        Vec<PackageTreeReadEntry>,
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

impl fmt::Debug for PackageTreeRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PackageTreeRead")
            .field("spec", &self.spec)
            .field("source_index", &self.source_index)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("entries", &self.entries)
            .finish()
    }
}

/// Exact raw Package Archive bytes read from a configured cache.
pub struct CachedPackageArchiveRead {
    spec: PackageSpec,
    configured_source: Location,
    candidate_location: Location,
    bytes: Vec<u8>,
}

impl CachedPackageArchiveRead {
    /// The exact package specification read.
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

impl fmt::Debug for CachedPackageArchiveRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CachedPackageArchiveRead")
            .field("spec", &self.spec)
            .field("configured_source", &self.configured_source)
            .field("candidate_location", &self.candidate_location)
            .field("byte_length", &self.bytes.len())
            .finish()
    }
}

/// Exact raw Package Archive bytes read from the official registry.
pub struct RegistryPackageArchiveRead {
    spec: PackageSpec,
    configured_source: Location,
    candidate_location: Location,
    cache_destination: Option<Location>,
    bytes: Vec<u8>,
}

impl RegistryPackageArchiveRead {
    /// The exact package specification read.
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

    /// The derived cache object available for later optional write.
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

impl fmt::Debug for RegistryPackageArchiveRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryPackageArchiveRead")
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
pub struct UnavailablePackageRead {
    spec: PackageSpec,
    failure: crate::PackageReadFailure,
}

impl UnavailablePackageRead {
    /// The exact package specification that was unavailable.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The stable failure to carry into resumed Pack Creation.
    pub fn failure(&self) -> &crate::PackageReadFailure {
        &self.failure
    }

    /// The stable reason the package was unavailable.
    pub fn reason(&self) -> &crate::PackageReadFailureReason {
        self.failure.reason()
    }

    /// Recovers the exact specification and owned failure.
    pub fn into_parts(self) -> (PackageSpec, crate::PackageReadFailure) {
        (self.spec, self.failure)
    }
}

/// The raw result of reading one package through configured OpenDAL sources.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackageRead {
    /// A present Package Tree candidate and its exact files.
    Tree(PackageTreeRead),
    /// A present raw Package Archive cache object.
    CachedArchive(CachedPackageArchiveRead),
    /// A present official Package Registry object.
    RegistryArchive(RegistryPackageArchiveRead),
    /// Every applicable configured candidate was definitely absent.
    Unavailable(UnavailablePackageRead),
}

impl PackageRead {
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

/// Reads one package from ordered trees, an optional cache, then an optional registry.
///
/// Only definite absence advances fallback. Registry lookup is skipped when the
/// official registry does not serve the requested namespace. This operation
/// returns exact raw values and performs no archive expansion or cache write.
pub async fn read_package<R: OperatorResolver + ?Sized>(
    resolver: &R,
    request: &PackageReadRequest,
) -> Result<PackageRead, PackageReadError> {
    let mut resolved = ResolvedOperators::new(resolver);
    match read_package_tree_candidates_with_resolved(
        &mut resolved,
        request.spec(),
        request.tree_sources(),
        request.limits().trees(),
    )
    .await
    {
        Ok(Some(tree)) => return Ok(PackageRead::Tree(tree)),
        Ok(None) => {}
        Err(error) => return Err(error),
    }

    if let Some(configured_source) = request.archive_cache() {
        let candidate_location = compose_candidate(
            configured_source,
            &read_layout::package_archive_cache_key(request.spec()),
        );
        match read_archive_candidate(
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
                return Ok(PackageRead::CachedArchive(CachedPackageArchiveRead {
                    spec: request.spec().clone(),
                    configured_source: configured_source.clone(),
                    candidate_location,
                    bytes,
                }));
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if let (Some(configured_source), Some(registry_key)) = (
        request.registry(),
        read_layout::official_registry_archive_key(request.spec()),
    ) {
        let candidate_location = compose_candidate(configured_source, &registry_key);
        match read_archive_candidate(
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
                        &read_layout::package_archive_cache_key(request.spec()),
                    )
                });
                return Ok(PackageRead::RegistryArchive(RegistryPackageArchiveRead {
                    spec: request.spec().clone(),
                    configured_source: configured_source.clone(),
                    candidate_location,
                    cache_destination,
                    bytes,
                }));
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    let failure = crate::PackageReadFailure::new(
        request.spec().clone(),
        crate::PackageReadFailureReason::NotFound,
    );
    Ok(PackageRead::Unavailable(UnavailablePackageRead {
        spec: request.spec().clone(),
        failure,
    }))
}

#[derive(Clone, Copy)]
enum ArchiveSource {
    Cache,
    Registry,
}

async fn read_archive_candidate<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    spec: &PackageSpec,
    configured_source: &Location,
    candidate_location: &Location,
    archive_source: ArchiveSource,
    ceiling: u64,
) -> Result<Option<Vec<u8>>, PackageReadError> {
    debug_assert!(candidate_location.require_object().is_ok());
    let operator = resolved
        .resolve(candidate_location.binding())
        .map_err(|source| {
            PackageReadError::from_archive(
                spec,
                configured_source.clone(),
                candidate_location.clone(),
                PackageReadErrorCause::ResolveOperator(Box::new(source)),
            )
        })?;
    if !operator.read {
        return Err(PackageReadError::from_archive(
            spec,
            configured_source.clone(),
            candidate_location.clone(),
            PackageReadErrorCause::UnsupportedArchiveRead,
        ));
    }

    read_exact_path(
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
    fn error(&self, cause: PackageReadErrorCause) -> PackageReadError {
        PackageReadError::from_archive(
            self.spec,
            self.configured_source.clone(),
            self.candidate_location.clone(),
            cause,
        )
    }
}

impl ExactPathReadOperation for PackageArchiveExactPathOperation<'_> {
    type Error = PackageReadError;

    fn read(&self, source: ::opendal::Error) -> PackageReadError {
        self.error(match self.archive_source {
            ArchiveSource::Cache => PackageReadErrorCause::CacheRead(source),
            ArchiveSource::Registry => PackageReadErrorCause::RegistryRead(source),
        })
    }

    fn limit_exceeded(&self, ceiling: u64, _: u64) -> PackageReadError {
        self.error(PackageReadErrorCause::ArchiveLimit(
            PackageArchiveReadLimitError::exceeded(
                PackageArchiveReadResource::ArchiveBytes,
                ceiling,
            ),
        ))
    }

    fn accounting_overflow(&self) -> PackageReadError {
        self.error(PackageReadErrorCause::ArchiveLimit(
            PackageArchiveReadLimitError::AccountingOverflow {
                resource: PackageArchiveReadResource::ArchiveBytes,
            },
        ))
    }
}

/// A terminal failure while reading one package through OpenDAL.
#[derive(Debug, thiserror::Error)]
#[error(
    "Package Read failed for {spec}{tree_source}{candidate}: {cause}",
    tree_source = package_tree_source_context(.source_index),
    candidate = package_candidate_context(.candidate_location.as_ref())
)]
pub struct PackageReadError {
    spec: PackageSpec,
    source_index: Option<usize>,
    configured_source: Option<Location>,
    candidate_location: Option<Location>,
    failed_path: Option<String>,
    failure: crate::PackageReadFailure,
    #[source]
    cause: RedactedError<PackageReadErrorCause>,
}

impl PackageReadError {
    /// The exact package specification whose read failed.
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
    pub fn failure(&self) -> &crate::PackageReadFailure {
        &self.failure
    }

    /// The stable Package Read Failure reason.
    pub fn reason(&self) -> &crate::PackageReadFailureReason {
        self.failure.reason()
    }

    /// The typed adapter cause retained by this failure.
    pub fn cause(&self) -> &PackageReadErrorCause {
        self.cause.inner()
    }

    fn from_tree(
        spec: &PackageSpec,
        sources: &[PackageTreeSource],
        child: &str,
        source_index: usize,
        failed_path: Option<String>,
        cause: PackageReadErrorCause,
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
        cause: PackageReadErrorCause,
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

/// The typed cause of a terminal OpenDAL Package Read failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageReadErrorCause {
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
    TreeStructural(#[source] PackageTreeReadSurveyError),
    /// Listed paths do not form a valid Package Tree.
    #[error("the listed objects do not form a Package Tree")]
    InvalidPackageTree(#[source] PackageTreeError),
    /// Package Tree Read exceeded a mandatory limit.
    #[error("a Package Tree Read limit failed")]
    TreeLimit(#[source] PackageTreeReadLimitError),
    /// Raw Package Archive Read exceeded a mandatory limit.
    #[error("a Package Archive Read limit failed")]
    ArchiveLimit(#[source] PackageArchiveReadLimitError),
}

struct PackageTreeReadOperation<'a> {
    spec: &'a PackageSpec,
    sources: &'a [PackageTreeSource],
    child: &'a str,
}

impl PackageTreeReadOperation<'_> {
    fn error(
        &self,
        source_index: usize,
        failed_path: Option<String>,
        cause: PackageReadErrorCause,
    ) -> PackageReadError {
        PackageReadError::from_tree(
            self.spec,
            self.sources,
            self.child,
            source_index,
            failed_path,
            cause,
        )
    }
}

impl RecursiveReadOperation for PackageTreeReadOperation<'_> {
    type Error = PackageReadError;

    fn invalid_location_role(&self, _: usize, _: LocationRoleError) -> PackageReadError {
        unreachable!("PackageReadRequest validates every tree prefix")
    }

    fn resolve_operator(&self, source_index: usize, source: BoxError) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::ResolveOperator(source),
        )
    }

    fn unsupported_capabilities(
        &self,
        source_index: usize,
        list: bool,
        list_with_recursive: bool,
        read: bool,
    ) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::UnsupportedTreeCapabilities {
                list,
                list_with_recursive,
                read,
            },
        )
    }

    fn list(&self, source_index: usize, source: ::opendal::Error) -> PackageReadError {
        self.error(source_index, None, PackageReadErrorCause::TreeList(source))
    }

    fn read(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> PackageReadError {
        self.error(
            source_index,
            Some(operation_path),
            PackageReadErrorCause::TreeRead(source),
        )
    }

    fn listed_object_absent(
        &self,
        source_index: usize,
        operation_path: String,
        source: ::opendal::Error,
    ) -> PackageReadError {
        self.error(
            source_index,
            Some(operation_path),
            PackageReadErrorCause::ListedTreeObjectAbsent(source),
        )
    }

    fn structural(
        &self,
        source_index: usize,
        issues: Vec<RecursiveSurveyIssue>,
    ) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::TreeStructural(PackageTreeReadSurveyError {
                issues: issues.into_iter().map(map_issue).collect(),
            }),
        )
    }

    fn limit(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
        ceiling: u64,
        _: u64,
    ) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::TreeLimit(PackageTreeReadLimitError::exceeded(
                map_resource(resource),
                ceiling,
            )),
        )
    }

    fn accounting_overflow(
        &self,
        source_index: usize,
        resource: RecursiveReadResource,
    ) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::TreeLimit(PackageTreeReadLimitError::AccountingOverflow {
                resource: map_resource(resource),
            }),
        )
    }
}

impl PackageTreeRecursiveReadOperation for PackageTreeReadOperation<'_> {
    fn invalid_package_tree(
        &self,
        source_index: usize,
        source: PackageTreeError,
    ) -> PackageReadError {
        self.error(
            source_index,
            None,
            PackageReadErrorCause::InvalidPackageTree(source),
        )
    }
}

fn other_failure(spec: &PackageSpec) -> crate::PackageReadFailure {
    crate::PackageReadFailure::new(
        spec.clone(),
        crate::PackageReadFailureReason::Other { detail: None },
    )
}

/// Exact registry bytes retained after successful validation and insertion.
#[cfg(feature = "package-reading")]
pub struct RegistryArchiveResidue {
    spec: PackageSpec,
    destination: Location,
    bytes: Vec<u8>,
}

#[cfg(feature = "package-reading")]
impl RegistryArchiveResidue {
    /// The exact package specification validated and inserted.
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    /// The derived exact cache object for optional write.
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

#[cfg(feature = "package-reading")]
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

/// The stage at which an read package could not be inserted.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[cfg(feature = "package-reading")]
pub enum ReadPackageInsertionTarget {
    /// Construction of an read Package Tree failed.
    PackageTree,
    /// Expansion of cached raw archive bytes failed.
    CachedArchive,
    /// Expansion of registry raw archive bytes failed.
    RegistryArchive,
    /// Package Catalog validation or insertion failed.
    PackageCatalog,
}

/// A failure while converting raw read into Pack Creation inputs.
#[cfg(feature = "package-reading")]
#[derive(Debug, thiserror::Error)]
#[error(
    "failed to insert read package {spec} at {target:?}",
    spec = .failure.spec()
)]
pub struct ReadPackageInsertionError {
    failure: Box<crate::PackageReadFailure>,
    target: ReadPackageInsertionTarget,
    #[source]
    cause: Box<ReadPackageInsertionErrorCause>,
}

#[cfg(feature = "package-reading")]
impl ReadPackageInsertionError {
    /// The exact package specification that could not be inserted.
    pub fn spec(&self) -> &PackageSpec {
        self.failure.spec()
    }

    /// The stable failure recorded for resumed Pack Creation.
    pub fn failure(&self) -> &crate::PackageReadFailure {
        &self.failure
    }

    /// The stable Package Read Failure reason.
    pub fn reason(&self) -> &crate::PackageReadFailureReason {
        self.failure.reason()
    }

    /// The insertion stage that failed.
    pub fn target(&self) -> &ReadPackageInsertionTarget {
        &self.target
    }

    /// The typed core cause retained by this failure.
    pub fn cause(&self) -> &ReadPackageInsertionErrorCause {
        &self.cause
    }
}

/// The typed cause of an read-package insertion failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[cfg(feature = "package-reading")]
pub enum ReadPackageInsertionErrorCause {
    /// Read entries could not construct a Package Tree.
    #[error("read entries could not construct a Package Tree")]
    PackageTree(#[source] crate::PackageTreeError),
    /// Raw archive bytes could not expand into a Package Tree.
    #[error("raw archive bytes could not expand into a Package Tree")]
    ArchiveExpansion(#[source] Box<crate::PackageReadError>),
    /// The Package Catalog rejected the constructed tree.
    #[error("the Package Catalog rejected the constructed tree")]
    PackageCatalog(#[source] crate::PackageCatalogError),
}

/// Expands or constructs an read package, inserts it, and updates failures.
///
/// Registry residue is returned only after expansion, validation, and catalog
/// insertion succeed. Writing those exact bytes is a separate low-level
/// operation; writing before this function succeeds can poison a cache with
/// terminal malformed bytes.
#[cfg(feature = "package-reading")]
#[allow(unreachable_patterns)]
pub fn insert_read_package(
    catalog: &mut crate::PackageCatalog,
    failures: &mut crate::PackageReadFailures,
    read: PackageRead,
    disposition: crate::PackageDisposition,
    expansion_limits: crate::PackageExpansionLimits,
) -> Result<Option<RegistryArchiveResidue>, ReadPackageInsertionError> {
    let (spec, tree, residue) = match read {
        PackageRead::Tree(read) => {
            let (spec, _, _, _, entries) = read.into_parts();
            let tree = crate::PackageTree::from_owned_entries(
                entries.into_iter().map(PackageTreeReadEntry::into_parts),
            )
            .map_err(|source| {
                insertion_error(
                    &spec,
                    ReadPackageInsertionTarget::PackageTree,
                    ReadPackageInsertionErrorCause::PackageTree(source),
                    crate::PackageReadFailureReason::Other { detail: None },
                    failures,
                )
            })?;
            (spec, tree, None)
        }
        PackageRead::CachedArchive(read) => {
            let (spec, _, _, bytes) = read.into_parts();
            let tree = expand_read_archive(
                &spec,
                &bytes,
                ReadPackageInsertionTarget::CachedArchive,
                expansion_limits,
                failures,
            )?;
            (spec, tree, None)
        }
        PackageRead::RegistryArchive(read) => {
            let (spec, _, _, destination, bytes) = read.into_parts();
            let tree = expand_read_archive(
                &spec,
                &bytes,
                ReadPackageInsertionTarget::RegistryArchive,
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
        PackageRead::Unavailable(read) => {
            let (_, failure) = read.into_parts();
            failures.insert(failure);
            return Ok(None);
        }
        _ => unreachable!("future Package Read outcomes require explicit composition"),
    };

    catalog
        .insert(spec.clone(), tree, disposition)
        .map_err(|source| {
            insertion_error(
                &spec,
                ReadPackageInsertionTarget::PackageCatalog,
                ReadPackageInsertionErrorCause::PackageCatalog(source),
                crate::PackageReadFailureReason::Other { detail: None },
                failures,
            )
        })?;
    failures.remove(&spec);
    Ok(residue)
}

#[cfg(feature = "package-reading")]
fn expand_read_archive(
    spec: &PackageSpec,
    bytes: &[u8],
    target: ReadPackageInsertionTarget,
    limits: crate::PackageExpansionLimits,
    failures: &mut crate::PackageReadFailures,
) -> Result<crate::PackageTree, ReadPackageInsertionError> {
    crate::expand_package_archive(spec.clone(), bytes, limits).map_err(|source| {
        let reason = match &source {
            crate::PackageReadError::MalformedArchive { .. }
            | crate::PackageReadError::InvalidPackageTree { .. } => {
                crate::PackageReadFailureReason::MalformedArchive { detail: None }
            }
            crate::PackageReadError::UnservedNamespace { .. }
            | crate::PackageReadError::ExpansionLimit { .. } => {
                crate::PackageReadFailureReason::Other { detail: None }
            }
        };
        insertion_error(
            spec,
            target,
            ReadPackageInsertionErrorCause::ArchiveExpansion(Box::new(source)),
            reason,
            failures,
        )
    })
}

#[cfg(feature = "package-reading")]
fn insertion_error(
    spec: &PackageSpec,
    target: ReadPackageInsertionTarget,
    cause: ReadPackageInsertionErrorCause,
    reason: crate::PackageReadFailureReason,
    failures: &mut crate::PackageReadFailures,
) -> ReadPackageInsertionError {
    let failure = crate::PackageReadFailure::new(spec.clone(), reason);
    failures.insert(failure.clone());
    ReadPackageInsertionError {
        failure: Box::new(failure),
        target,
        cause: Box::new(cause),
    }
}

/// An unsupported entry kind yielded by a Package Tree listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageTreeReadEntryKind {
    /// OpenDAL did not classify the entry as a file or directory.
    Unknown,
}

/// One storage-envelope issue found during a Package Tree survey.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageTreeReadIssue {
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
        kind: PackageTreeReadEntryKind,
    },
}

/// The nonempty canonical set of Package Tree survey envelope issues.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}", message = package_tree_survey_message(.issues.as_slice()))]
pub struct PackageTreeReadSurveyError {
    issues: Vec<PackageTreeReadIssue>,
}

impl PackageTreeReadSurveyError {
    /// Every independently detectable issue in canonical path and kind order.
    pub fn issues(&self) -> &[PackageTreeReadIssue] {
        &self.issues
    }
}

fn package_tree_survey_message(issues: &[PackageTreeReadIssue]) -> String {
    if let [issue] = issues {
        issue.to_string()
    } else {
        format!("Package Tree survey failed with {} issue(s)", issues.len())
    }
}

#[cfg(test)]
pub(crate) async fn read_package_tree_candidates<R: OperatorResolver + ?Sized>(
    resolver: &R,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeReadLimits,
) -> Result<Option<PackageTreeRead>, PackageReadError> {
    let mut resolved = ResolvedOperators::new(resolver);
    read_package_tree_candidates_with_resolved(&mut resolved, spec, sources, limits).await
}

async fn read_package_tree_candidates_with_resolved<R: OperatorResolver + ?Sized>(
    resolved: &mut ResolvedOperators<'_, R>,
    spec: &PackageSpec,
    sources: &[PackageTreeSource],
    limits: PackageTreeReadLimits,
) -> Result<Option<PackageTreeRead>, PackageReadError> {
    let child = format!("{}/", read_layout::package_tree_key(spec));
    let candidates = sources
        .iter()
        .map(|source| {
            source.source.require_prefix()?;
            Ok(compose_candidate(&source.source, &child))
        })
        .collect::<Vec<_>>();
    let Some((source_index, candidate_location, objects)) =
        read_first_present_package_tree_prefix_with_resolved(
            resolved,
            candidates,
            limits.into(),
            &PackageTreeReadOperation {
                spec,
                sources,
                child: &child,
            },
        )
        .await?
    else {
        return Ok(None);
    };

    Ok(Some(PackageTreeRead {
        spec: spec.clone(),
        source_index,
        configured_source: sources[source_index].source.clone(),
        candidate_location,
        entries: objects
            .into_iter()
            .map(|object| PackageTreeReadEntry {
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

impl From<PackageTreeReadLimits> for RecursiveReadLimits {
    fn from(limits: PackageTreeReadLimits) -> Self {
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

fn map_resource(resource: RecursiveReadResource) -> PackageTreeReadResource {
    match resource {
        RecursiveReadResource::ListedEntries => PackageTreeReadResource::ListedEntries,
        RecursiveReadResource::ListedPathBytes => PackageTreeReadResource::ListedPathBytes,
        RecursiveReadResource::TotalListedPathBytes => {
            PackageTreeReadResource::TotalListedPathBytes
        }
        RecursiveReadResource::SelectedObjects => PackageTreeReadResource::SelectedFiles,
        RecursiveReadResource::ObjectBytes => PackageTreeReadResource::ObjectBytes,
        RecursiveReadResource::TotalBytes => PackageTreeReadResource::TotalBytes,
        _ => unreachable!("unknown recursive read resource"),
    }
}

fn map_issue(issue: RecursiveSurveyIssue) -> PackageTreeReadIssue {
    let operation_path = issue.operation_path;
    match issue.kind {
        RecursiveSurveyIssueKind::ListedPathOutsidePrefix => {
            PackageTreeReadIssue::ListedPathOutsidePrefix { operation_path }
        }
        RecursiveSurveyIssueKind::PrefixMarkerWhereFileRequired => {
            PackageTreeReadIssue::PrefixMarkerWhereFileRequired { operation_path }
        }
        RecursiveSurveyIssueKind::EmptyRelativeOperationPath => {
            PackageTreeReadIssue::EmptyRelativeOperationPath { operation_path }
        }
        RecursiveSurveyIssueKind::UnsupportedEntryKind => {
            PackageTreeReadIssue::UnsupportedEntryKind {
                operation_path,
                kind: PackageTreeReadEntryKind::Unknown,
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
        PackageReadErrorCause, PackageTreeReadCeilings, PackageTreeReadLimitError,
        PackageTreeReadLimits, PackageTreeReadResource, PackageTreeSource,
        read_package_tree_candidates,
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

        let read = expect_ready(pin!(read_package_tree_candidates(
            &resolver,
            &"@preview/example:1.2.3".parse().unwrap(),
            &sources,
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();

        assert_eq!(read.source_index(), 1);
        assert_eq!(read.configured_source(), sources[1].source());
        assert_eq!(
            read.candidate_location().operation_path(),
            "second/preview/example/1.2.3/"
        );
        assert_eq!(
            read.entries()
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
        let reference = PackageTreeReadCeilings::reference_v1();
        assert_eq!(reference.listed_entries, 100_000);
        assert_eq!(reference.listed_path_bytes, 64 * 1024);
        assert_eq!(reference.total_listed_path_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.selected_files, 50_000);
        assert_eq!(reference.object_bytes, 64 * 1024 * 1024);
        assert_eq!(reference.total_bytes, 512 * 1024 * 1024);

        let narrowed = PackageTreeReadLimits::new(PackageTreeReadCeilings {
            listed_entries: u64::MAX - 1,
            listed_path_bytes: u64::MAX - 1,
            total_listed_path_bytes: u64::MAX - 1,
            total_bytes: reference.object_bytes,
            ..reference
        });
        assert_eq!(narrowed.listed_entries(), u64::MAX - 1);
        assert_eq!(narrowed.listed_path_bytes(), u64::MAX - 1);
        assert_eq!(narrowed.total_listed_path_bytes(), u64::MAX - 1);
        assert_eq!(narrowed.selected_files(), reference.selected_files);
        assert_eq!(narrowed.object_bytes(), reference.object_bytes);
        assert_eq!(narrowed.total_bytes(), reference.object_bytes);

        for ceilings in [
            PackageTreeReadCeilings {
                listed_entries: u64::MAX,
                ..reference
            },
            PackageTreeReadCeilings {
                listed_path_bytes: u64::MAX,
                ..reference
            },
            PackageTreeReadCeilings {
                total_listed_path_bytes: u64::MAX,
                ..reference
            },
            PackageTreeReadCeilings {
                selected_files: u64::MAX,
                ..reference
            },
            PackageTreeReadCeilings {
                object_bytes: u64::MAX,
                total_bytes: u64::MAX,
                ..reference
            },
            PackageTreeReadCeilings {
                total_bytes: u64::MAX,
                ..reference
            },
        ] {
            assert!(std::panic::catch_unwind(|| PackageTreeReadLimits::new(ceilings)).is_err());
        }
        assert!(
            std::panic::catch_unwind(|| PackageTreeReadLimits::new(PackageTreeReadCeilings {
                object_bytes: 2,
                total_bytes: 1,
                ..reference
            }))
            .is_err()
        );
    }

    #[test]
    fn tree_resources_map_shared_survey_and_payload_boundaries() {
        let reference = PackageTreeReadCeilings::reference_v1();
        let survey_cases = [
            (
                PackageTreeReadResource::ListedEntries,
                PackageTreeReadCeilings {
                    listed_entries: 0,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeReadResource::ListedPathBytes,
                PackageTreeReadCeilings {
                    listed_path_bytes: 1,
                    ..reference
                },
                ListEntry::directory("trees/preview/example/1.2.3/dir/"),
            ),
            (
                PackageTreeReadResource::TotalListedPathBytes,
                PackageTreeReadCeilings {
                    total_listed_path_bytes: 0,
                    ..reference
                },
                ListEntry::file("trees/preview/example/1.2.3/a.typ"),
            ),
            (
                PackageTreeReadResource::SelectedFiles,
                PackageTreeReadCeilings {
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
            let error = expect_ready(pin!(read_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeReadLimits::new(ceilings),
            )))
            .unwrap_err();
            assert!(matches!(
                error.cause(),
                PackageReadErrorCause::TreeLimit(
                    PackageTreeReadLimitError::Exceeded {
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
        let error = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeReadLimits::new(PackageTreeReadCeilings {
                object_bytes: 3,
                total_bytes: 8,
                ..reference
            }),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageReadErrorCause::TreeLimit(PackageTreeReadLimitError::Exceeded {
                resource: PackageTreeReadResource::ObjectBytes,
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
        let error = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeReadLimits::new(PackageTreeReadCeilings {
                object_bytes: 3,
                total_bytes: 3,
                ..reference
            }),
        )))
        .unwrap_err();
        assert!(matches!(
            error.cause(),
            PackageReadErrorCause::TreeLimit(PackageTreeReadLimitError::Exceeded {
                resource: PackageTreeReadResource::TotalBytes,
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
        let limits = PackageTreeReadLimits::new(PackageTreeReadCeilings {
            listed_entries: 1,
            ..PackageTreeReadCeilings::reference_v1()
        });

        let error = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            limits,
        )))
        .unwrap_err();

        assert_eq!(error.source_index(), Some(1));
        assert!(matches!(
            error.cause(),
            PackageReadErrorCause::TreeLimit(PackageTreeReadLimitError::Exceeded {
                resource: PackageTreeReadResource::ListedEntries,
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
            let read = expect_ready(pin!(read_package_tree_candidates(
                &bindings,
                &spec(),
                &[source("trees/")],
                PackageTreeReadLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();
            assert_eq!(
                read.entries()
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
        let exact = PackageTreeReadLimits::new(PackageTreeReadCeilings {
            listed_entries: 1,
            listed_path_bytes: 29,
            total_listed_path_bytes: 33,
            selected_files: 1,
            object_bytes: 1,
            total_bytes: 1,
        });
        let bindings = configured(&service);
        let read = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            exact,
        )))
        .unwrap()
        .unwrap();
        assert_eq!(read.entries()[0].bytes(), b"a");
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

        let read = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap();

        assert!(read.is_none());
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

        let read = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(read.spec(), &spec());
        assert_eq!(read.entries()[0].relative_path(), "empty.typ");
        assert_eq!(read.entries()[0].len(), 0);
        assert!(read.entries()[0].is_empty());
        assert_eq!(read.entries()[1].relative_path(), "lib.typ");

        let (actual_spec, index, configured, candidate, entries) = read.into_parts();
        assert_eq!(actual_spec, spec());
        assert_eq!(index, 0);
        assert_eq!(configured.operation_path(), "trees/");
        assert_eq!(candidate.operation_path(), "trees/preview/example/1.2.3/");
        let tree = PackageTree::from_owned_entries(
            entries
                .into_iter()
                .map(super::PackageTreeReadEntry::into_parts),
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

        let error = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("first/"), source("second/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap_err();

        let PackageReadErrorCause::InvalidPackageTree(source) = error.cause() else {
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

        let error = expect_ready(pin!(read_package_tree_candidates(
            &bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap_err();
        let PackageReadErrorCause::TreeStructural(survey) = error.cause() else {
            panic!("unexpected cause: {:?}", error.cause());
        };
        assert_eq!(survey.issues().len(), 2);
        assert!(matches!(
            &survey.issues()[0],
            super::PackageTreeReadIssue::ListedPathOutsidePrefix { operation_path }
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
        let read = expect_ready(pin!(read_package_tree_candidates(
            &mutation_bindings,
            &spec(),
            &[source("trees/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap()
        .unwrap();
        assert_eq!(read.entries()[0].bytes(), b"bytes after listing");

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
        let absent = expect_ready(pin!(read_package_tree_candidates(
            &absent_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap_err();
        assert_eq!(
            absent.failed_path(),
            Some("trees/preview/example/1.2.3/gone.typ")
        );
        assert!(matches!(
            absent.cause(),
            PackageReadErrorCause::ListedTreeObjectAbsent(source)
                if source.kind() == ErrorKind::NotFound
        ));

        let list_failure_service = ScriptedService::new(
            Capabilities::all(),
            [ListScript::new(candidate, 0, [ListStep::failure(ErrorKind::NotFound)]).unwrap()],
            [],
            4,
        );
        let list_failure_bindings = configured(&list_failure_service);
        let list_failure = expect_ready(pin!(read_package_tree_candidates(
            &list_failure_bindings,
            &spec(),
            &[source("trees/"), source("unreached/")],
            PackageTreeReadLimits::reference_v1(),
        )))
        .unwrap_err();
        assert!(matches!(
            list_failure.cause(),
            PackageReadErrorCause::TreeList(source)
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
            let mut read = pin!(read_package_tree_candidates(
                &list_bindings,
                &requested_spec,
                &sources,
                PackageTreeReadLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(read.as_mut()), Poll::Pending));
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
            let mut read = pin!(read_package_tree_candidates(
                &read_bindings,
                &requested_spec,
                &sources,
                PackageTreeReadLimits::reference_v1(),
            ));
            assert!(matches!(poll_once(read.as_mut()), Poll::Pending));
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
    fn memory_reads_candidates_below_root_and_non_root_configured_prefixes() {
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

            let read = expect_ready(pin!(read_package_tree_candidates(
                &bindings,
                &spec(),
                &sources,
                PackageTreeReadLimits::reference_v1(),
            )))
            .unwrap()
            .unwrap();

            assert_eq!(
                read.candidate_location().operation_path(),
                expected_candidate
            );
            assert_eq!(read.entries()[0].relative_path(), "lib.typ");
            assert_eq!(read.entries()[0].bytes(), b"memory package");
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
