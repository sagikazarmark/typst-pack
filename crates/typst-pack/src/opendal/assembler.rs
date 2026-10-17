//! The end-to-end OpenDAL Pack Assembler.

use std::collections::HashSet;

use ::opendal::Operator;
use ecow::EcoVec;
use typst::diag::SourceDiagnostic;

use super::{
    Error, FontSource, PackageSource, insert_package, read_fonts, read_package, read_project,
};
use crate::{
    DiscoverySpecification, Pack, PackCreationInput, PackCreationOutcome, PackMetadata,
    PackageCatalog, PackageDisposition, PackageReadFailures, ReadLimits, create,
};

/// Assembles a Pack from a project, fonts, and packages held in OpenDAL
/// storage.
///
/// [`assemble`](Self::assemble) reads the project and fonts, then runs Pack
/// Creation. Whenever creation reports missing packages, each one is read
/// from [`packages`](Self::packages) in order, inserted, and creation is
/// resumed. A package that is unavailable or fails to expand or validate is
/// recorded as a Package Read Failure, so creation reports it at the import
/// that needs it. Storage failures end assembly.
///
/// The future is caller-polled and needs no particular async runtime.
#[derive(Clone, Debug)]
pub struct OpenDalPackAssembler {
    /// The operator holding the project and font prefixes.
    pub operator: Operator,
    /// The project prefix (`""` or ending in `/`).
    pub project: String,
    /// The entrypoint path relative to the project prefix.
    pub entrypoint: String,
    /// Font prefixes, cataloged in order.
    pub fonts: Vec<FontSource>,
    /// Package sources, tried in order for each missing package.
    pub packages: Vec<PackageSource>,
    /// The disposition of every read package.
    pub package_disposition: PackageDisposition,
    /// The Dependency Discovery controls.
    pub discovery: DiscoverySpecification,
    /// Optional Pack metadata.
    pub metadata: Option<PackMetadata>,
    /// Limits for reading the project, the fonts, and each package, and for
    /// expanding each package archive.
    pub limits: ReadLimits,
}

/// A Pack produced by [`OpenDalPackAssembler::assemble`].
#[derive(Debug)]
pub struct AssembledPack {
    pub pack: Pack,
    /// Dependency Discovery warnings.
    pub warnings: EcoVec<SourceDiagnostic>,
}

impl OpenDalPackAssembler {
    /// An assembler with no fonts or package sources, embedded packages,
    /// and default limits.
    pub fn new(
        operator: Operator,
        project: impl Into<String>,
        entrypoint: impl Into<String>,
        discovery: DiscoverySpecification,
    ) -> Self {
        Self {
            operator,
            project: project.into(),
            entrypoint: entrypoint.into(),
            fonts: Vec::new(),
            packages: Vec::new(),
            package_disposition: PackageDisposition::Embedded,
            discovery,
            metadata: None,
            limits: ReadLimits::default(),
        }
    }

    /// Reads every input and runs Pack Creation to completion.
    pub async fn assemble(&self) -> Result<AssembledPack, Error> {
        let project = read_project(
            &self.operator,
            &self.project,
            &self.entrypoint,
            &self.limits,
        )
        .await?;
        let fonts = read_fonts(&self.operator, &self.fonts, &self.limits).await?;
        let mut packages = PackageCatalog::new();
        let mut failures = PackageReadFailures::new();
        let mut attempted = HashSet::new();
        loop {
            let outcome = create(PackCreationInput {
                project: &project,
                packages: &packages,
                fonts: &fonts,
                package_failures: &failures,
                discovery: &self.discovery,
                metadata: self.metadata.as_ref(),
            })
            .map_err(|error| Error::Creation(Box::new(error)))?;
            let missing = match outcome {
                PackCreationOutcome::Created { pack, warnings } => {
                    return Ok(AssembledPack { pack, warnings });
                }
                PackCreationOutcome::MissingPackageSpecifications(missing) => missing,
            };
            for spec in missing {
                if !attempted.insert(spec.to_string()) {
                    return Err(Error::RepeatedPackage { spec });
                }
                let read = read_package(&self.packages, &spec, &self.limits).await?;
                // A rejected package is recorded in `failures`; resuming
                // creation reports it at the importing source span.
                let _ = insert_package(
                    &mut packages,
                    &mut failures,
                    &spec,
                    read,
                    self.package_disposition,
                    self.limits,
                );
            }
        }
    }
}
