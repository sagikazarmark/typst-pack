//! Pack Creation: one representative Typst request over supplied inputs.
//!
//! Creation acquires nothing. The caller supplies a [`ProjectSnapshot`], a
//! [`FontCatalog`], and the Package Trees resolved for the
//! document, all as bytes it already holds, so the operation runs wherever the
//! core runs — including a host with no filesystem and no clock. Obtaining
//! those inputs belongs to Pack Assembly and a Pack Assembler.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use ecow::EcoVec;
use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore};

use crate::domain::{DocumentTime, TypstTarget};
use crate::embedded::EmbeddedTypst;
use crate::font_catalog::{CatalogFonts, FontCatalog, FontDisposition};
use crate::manifest::PackMetadata;
use crate::pack::{Pack, PackBuildError, PackInvariantError};
use crate::package_catalog::PackageCatalog;
use crate::package_failure::{
    PackageAcquisitionFailure, PackageAcquisitionFailureReason, PackageAcquisitionFailures,
};
use crate::payload::SharedBytes;
use crate::project_snapshot::ProjectSnapshot;

/// The semantic controls for one Dependency Discovery run.
///
/// These values select dependencies for one Pack Creation invocation. They do
/// not become Pack state and do not restrict later Pack compilation requests.
#[derive(Clone, Debug)]
pub struct DiscoverySpecification {
    target: TypstTarget,
    inputs: Dict,
    document_time: DocumentTime,
    features: Vec<Feature>,
}

impl DiscoverySpecification {
    /// Validates and groups every semantic control for one discovery run.
    pub fn new(
        target: TypstTarget,
        inputs: Dict,
        document_time: DocumentTime,
        features: impl IntoIterator<Item = Feature>,
    ) -> Result<Self, DiscoverySpecificationError> {
        if let DocumentTime::UnixTimestamp(timestamp) = document_time
            && Time::fixed_timestamp(timestamp).is_err()
        {
            return Err(DiscoverySpecificationError::InvalidDocumentTimestamp);
        }
        Ok(Self {
            target,
            inputs,
            document_time,
            features: features.into_iter().collect(),
        })
    }

    /// The Typst document model selected for Dependency Discovery.
    pub fn target(&self) -> TypstTarget {
        self.target
    }

    /// Values exposed to document code through `sys.inputs`.
    pub fn inputs(&self) -> &Dict {
        &self.inputs
    }

    /// The exact or explicitly absent Document Time for this run.
    pub fn document_time(&self) -> DocumentTime {
        self.document_time
    }

    /// Typst engine features enabled for this run.
    pub fn features(&self) -> &[Feature] {
        &self.features
    }
}

/// A failure while constructing a [`DiscoverySpecification`].
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DiscoverySpecificationError {
    #[error("the discovery document-time UNIX timestamp is out of range")]
    InvalidDocumentTimestamp,
}

/// Every value borrowed by one stateless Pack Creation invocation.
#[derive(Clone, Copy, Debug)]
pub struct PackCreationInput<'a> {
    /// The complete, stabilized project tree and selected entrypoint.
    pub project: &'a ProjectSnapshot,
    /// Validated Package Trees available to Dependency Discovery.
    pub packages: &'a PackageCatalog,
    /// Ordered Font Containers available to Dependency Discovery.
    pub fonts: &'a FontCatalog,
    /// Failed package acquisitions to attach at importing source spans.
    pub package_failures: &'a PackageAcquisitionFailures,
    /// Semantic controls used only for this Dependency Discovery run.
    pub discovery: &'a DiscoverySpecification,
    /// Optional descriptive Pack metadata, excluded from Pack Identity.
    pub metadata: Option<&'a PackMetadata>,
}

/// What one Pack Creation invocation produced.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // The accepted Created outcome owns its validated Pack.
pub enum PackCreationOutcome {
    /// Dependency Discovery succeeded and authoritative validation produced one
    /// Pack. Discovery warnings remain separate from Pack state.
    Created {
        pack: Pack,
        warnings: EcoVec<SourceDiagnostic>,
    },
    /// Exact specifications the caller must add to the Package Catalog before
    /// invoking creation again. The list is nonempty, deduplicated, and in
    /// canonical specification order.
    MissingPackageSpecifications(Vec<PackageSpec>),
}

/// Complete compiler evidence from a rejected Dependency Discovery run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyDiscoveryRejection {
    diagnostics: EcoVec<SourceDiagnostic>,
    warnings: EcoVec<SourceDiagnostic>,
}

impl DependencyDiscoveryRejection {
    /// Every rejection diagnostic in compiler order.
    pub fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }

    /// Every discovery warning in compiler order.
    pub fn warnings(&self) -> &[SourceDiagnostic] {
        &self.warnings
    }

    /// Recovers the complete owned compiler evidence.
    pub fn into_parts(self) -> (EcoVec<SourceDiagnostic>, EcoVec<SourceDiagnostic>) {
        (self.diagnostics, self.warnings)
    }
}

/// A failure that creates no Pack.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackCreationError {
    /// Dependency Discovery did not compile. All diagnostics and warnings from
    /// the run are retained.
    #[error(
        "dependency discovery was rejected with {} diagnostic(s)",
        .0.diagnostics.len()
    )]
    DependencyDiscoveryRejected(DependencyDiscoveryRejection),
    /// The selected inputs do not satisfy authoritative whole-Pack invariants.
    #[error(transparent)]
    InvalidPack(#[from] PackInvariantError),
}

/// Runs one representative Typst request over the supplied inputs and issues
/// the Pack it selected, or reports the packages it needed and was not given.
///
/// Compiler observations select package and font requirements; project files
/// come from the Project Snapshot alone. Creation fails rather than issuing an
/// incomplete Pack when the representative request does not compile.
///
/// A request that read a package no supplied tree covers returns
/// [`PackCreationOutcome::MissingPackageSpecifications`] instead: resolve
/// those specifications, add their trees to the Package Catalog, and invoke
/// creation again. Because a
/// failed import ends module evaluation, one round reports what that round
/// reached, and a project needing several packages completes over repeated
/// invocation. A specification the caller cannot resolve is added to
/// [`PackageAcquisitionFailures`], which fails the next round's
/// Dependency Discovery at the import that needed it.
///
/// Creation borrows validated bytes and has nothing to re-read. A Pack represents the
/// exact values its source adapters acquired, without guaranteeing that values
/// from mutable sources all coexisted at one instant.
pub fn create(input: PackCreationInput<'_>) -> Result<PackCreationOutcome, PackCreationError> {
    let entrypoint = VirtualPath::new(input.project.entrypoint())
        .expect("Project Snapshot entrypoint invariant violated");

    let mut world = SuppliedWorld {
        library: LazyHash::new(
            Library::builder()
                .with_inputs(input.discovery.inputs.clone())
                .with_features(input.discovery.features.iter().copied().collect())
                .build(),
        ),
        main: RootedPath::new(VirtualRoot::Project, entrypoint).intern(),
        files: FileStore::new(SuppliedLoader {
            project: input.project,
            packages: input.packages,
            package_failures: input.package_failures,
        }),
        fonts: input.fonts.expand(),
        used_font_indices: Mutex::new(BTreeSet::new()),
        clock: DiscoveryClock::new(input.discovery.document_time),
    };

    let Warned { output, warnings } = compile_creation_target(&world, input.discovery.target);
    let observed = world.observed_packages();

    // Reported before the compile outcome is inspected, because the import that
    // needed a tree is exactly what failed the compile. The caller resolves
    // these and invokes creation again rather than reading diagnostics.
    if !observed.missing.is_empty() {
        return Ok(PackCreationOutcome::MissingPackageSpecifications(
            observed.missing,
        ));
    }

    if let Err(diagnostics) = output {
        return Err(PackCreationError::DependencyDiscoveryRejected(
            DependencyDiscoveryRejection {
                diagnostics,
                warnings,
            },
        ));
    }

    let mut builder = Pack::builder(input.project.entrypoint());
    for (path, data) in input.project.shared_files() {
        builder = map_build(builder.shared_file(path, data.clone()))?;
    }

    // Packages, in canonical specification order. The whole Package
    // Tree travels, not only the files the representative request read.
    let loader = world.files.loader();
    for spec in observed.supplied {
        let entry = loader
            .packages
            .get(&spec)
            .expect("observed package was partitioned as supplied");
        for (path, data) in entry.tree().shared_files() {
            builder = if entry.disposition().is_embedded() {
                map_build(builder.shared_package_file(spec.clone(), path, data.clone()))?
            } else {
                map_build(builder.shared_external_package_file(spec.clone(), path, data.clone()))?
            };
        }
    }

    // Selected faces in Font Catalog order, each under the disposition
    // its container carries.
    for (font, disposition) in world.used_fonts() {
        builder =
            if disposition.is_embedded() {
                map_build(
                    builder.shared_font(SharedBytes::from_typst(font.data().clone()), font.index()),
                )?
            } else {
                map_build(builder.shared_external_font(
                    SharedBytes::from_typst(font.data().clone()),
                    font.index(),
                ))?
            };
    }

    if let Some(metadata) = input.metadata {
        builder = builder.metadata(metadata.clone());
    }

    Ok(PackCreationOutcome::Created {
        pack: map_build(builder.build())?,
        warnings,
    })
}

fn map_build<T>(result: Result<T, PackBuildError>) -> Result<T, PackCreationError> {
    match result {
        Ok(value) => Ok(value),
        Err(PackBuildError::Invariant(error)) => Err(PackCreationError::InvalidPack(error)),
    }
}

/// The package specifications one representative request asked for, split by
/// whether the supplied trees covered them.
#[derive(Default)]
struct ObservedPackages {
    /// Specifications a supplied tree covers, which become Package
    /// Requirements.
    supplied: Vec<PackageSpec>,
    /// Specifications no supplied tree covers, which creation reports so the
    /// caller can resolve them and invoke creation again.
    missing: Vec<PackageSpec>,
}

/// The world the representative request compiles against: supplied bytes and
/// nothing else.
struct SuppliedWorld<'a> {
    library: LazyHash<Library>,
    main: FileId,
    files: FileStore<SuppliedLoader<'a>>,
    fonts: CatalogFonts,
    used_font_indices: Mutex<BTreeSet<usize>>,
    clock: DiscoveryClock,
}

impl SuppliedWorld<'_> {
    /// The packages the representative request asked for a file of, split by
    /// whether a supplied tree covers them, each in canonical specification
    /// order.
    ///
    /// Both halves come from the requests the compiler made, not from what its
    /// diagnostics said about them.
    fn observed_packages(&mut self) -> ObservedPackages {
        let mut specs: BTreeMap<String, PackageSpec> = BTreeMap::new();
        let (loader, dependencies) = self.files.dependencies();
        for id in dependencies {
            if let VirtualRoot::Package(spec) = id.root() {
                specs.insert(spec.to_string(), spec.clone());
            }
        }

        let mut observed = ObservedPackages::default();
        for spec in specs.into_values() {
            if loader.packages.get(&spec).is_some() {
                observed.supplied.push(spec);
            } else if loader.package_failures.get(&spec).is_none() {
                observed.missing.push(spec);
            }
            // A specification the caller declared unresolvable is neither. The
            // representative request already failed at it, carrying the
            // caller's own reason, and reporting it again would ask for what
            // the caller said it cannot supply.
        }
        observed
    }

    /// The selected faces in Font Catalog order, each with the
    /// disposition its container carries.
    fn used_fonts(&self) -> Vec<(Font, FontDisposition)> {
        self.used_font_indices
            .lock()
            .expect("used font index lock poisoned")
            .iter()
            .filter_map(|index| Some((self.fonts.font(*index)?, self.fonts.disposition(*index)?)))
            .collect()
    }
}

impl World for SuppliedWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        let font = self.fonts.font(index);
        if font.is_some() {
            self.used_font_indices
                .lock()
                .expect("used font index lock poisoned")
                .insert(index);
        }
        font
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.clock.today(offset)
    }
}

enum DiscoveryClock {
    None,
    Fixed(Datetime),
    Timestamp(Time),
}

impl DiscoveryClock {
    fn new(document_time: DocumentTime) -> Self {
        match document_time {
            DocumentTime::Absent => Self::None,
            DocumentTime::Fixed(datetime) => Self::Fixed(datetime),
            DocumentTime::UnixTimestamp(timestamp) => Self::Timestamp(
                Time::fixed_timestamp(timestamp)
                    .expect("Discovery Specification validated its Document Time"),
            ),
        }
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        match self {
            Self::None => None,
            Self::Fixed(datetime) => Some(*datetime),
            Self::Timestamp(time) => time.today(offset),
        }
    }
}

/// Serves file requests from the supplied Project Snapshot and package trees.
struct SuppliedLoader<'a> {
    project: &'a ProjectSnapshot,
    packages: &'a PackageCatalog,
    package_failures: &'a PackageAcquisitionFailures,
}

impl FileLoader for SuppliedLoader<'_> {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => self
                .project
                .shared_file(path)
                .map(|data| data.to_typst())
                .ok_or_else(|| FileError::NotFound(path.into())),
            VirtualRoot::Package(spec) => {
                let Some(entry) = self.packages.get(spec) else {
                    // A supplied tree first, so a caller that resolved a
                    // specification it had declared unresolvable is served it.
                    return Err(FileError::Package(
                        self.package_failures
                            .get(spec)
                            .map(package_failure_for_discovery)
                            .unwrap_or_else(|| PackageError::NotFound(spec.clone())),
                    ));
                };
                entry
                    .tree()
                    .shared_file(path)
                    .map(SharedBytes::to_typst)
                    .ok_or_else(|| FileError::NotFound(path.into()))
            }
        }
    }
}

fn package_failure_for_discovery(failure: &PackageAcquisitionFailure) -> PackageError {
    let spec = failure.spec().clone();
    match failure.reason() {
        PackageAcquisitionFailureReason::NotFound => PackageError::NotFound(spec),
        PackageAcquisitionFailureReason::VersionNotFound { latest } => {
            PackageError::VersionNotFound(spec, *latest)
        }
        PackageAcquisitionFailureReason::NetworkFailed { detail } => {
            PackageError::NetworkFailed(detail.clone().map(Into::into))
        }
        PackageAcquisitionFailureReason::MalformedArchive { detail } => {
            PackageError::MalformedArchive(detail.clone().map(Into::into))
        }
        PackageAcquisitionFailureReason::Other { detail } => {
            PackageError::Other(detail.clone().map(Into::into))
        }
    }
}

/// Runs the representative request for the selected Typst Target, keeping only
/// what selects requirements: whether it compiled, and its warnings.
fn compile_creation_target(
    world: &dyn World,
    target: TypstTarget,
) -> Warned<Result<(), EcoVec<SourceDiagnostic>>> {
    match target {
        TypstTarget::Paged => {
            let Warned { output, warnings } = EmbeddedTypst::compile_paged(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
        TypstTarget::Html => {
            let Warned { output, warnings } = EmbeddedTypst::compile_html(world);
            Warned {
                output: output.map(|_| ()),
                warnings,
            }
        }
    }
}
