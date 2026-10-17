//! The end-to-end OpenDAL Pack Assembler.

use std::collections::HashSet;

use ::opendal::Operator;
use typst::diag::SourceDiagnostic;

use super::{
    Error, FontSource, PackageSource, insert_package, read_fonts, read_package, read_project,
};
use crate::{
    CompileOptions, CreateOutcome, CreateRequest, Input, Pack, PackMetadata, PackageDisposition,
    ReadLimits, Sources, TypstTarget, create,
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
    /// The document model discovery compiles.
    pub target: TypstTarget,
    /// Inputs, document time, and features of the discovery compile.
    pub options: CompileOptions,
    /// Input overrides for additional discovery compiles.
    pub variants: Vec<Vec<(String, Input)>>,
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
    /// Discovery warnings.
    pub warnings: Vec<SourceDiagnostic>,
    /// The sources the discovery compiles read.
    pub sources: Sources,
}

impl OpenDalPackAssembler {
    /// An assembler with no fonts or package sources, embedded packages,
    /// default options, and default limits.
    pub fn new(
        operator: Operator,
        project: impl Into<String>,
        entrypoint: impl Into<String>,
    ) -> Self {
        Self {
            operator,
            project: project.into(),
            entrypoint: entrypoint.into(),
            fonts: Vec::new(),
            packages: Vec::new(),
            package_disposition: PackageDisposition::Embedded,
            target: TypstTarget::Paged,
            options: CompileOptions::default(),
            variants: Vec::new(),
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
        let mut request = CreateRequest::new(project);
        request.fonts = read_fonts(&self.operator, &self.fonts, &self.limits).await?;
        request.target = self.target;
        request.options = self.options.clone();
        request.variants = self.variants.clone();
        request.metadata = self.metadata.clone();
        let mut attempted = HashSet::new();
        loop {
            let outcome = create(&request).map_err(|error| Error::Creation(Box::new(error)))?;
            let missing = match outcome {
                CreateOutcome::Created {
                    pack,
                    warnings,
                    sources,
                } => {
                    return Ok(AssembledPack {
                        pack,
                        warnings,
                        sources,
                    });
                }
                CreateOutcome::MissingPackages(missing) => missing,
            };
            for spec in missing {
                if !attempted.insert(spec.to_string()) {
                    return Err(Error::RepeatedPackage { spec });
                }
                let read = read_package(&self.packages, &spec, &self.limits).await?;
                // A rejected package is recorded in `failures`; resuming
                // creation reports it at the importing source span.
                let _ = insert_package(
                    &mut request.packages,
                    &mut request.package_failures,
                    &spec,
                    read,
                    self.package_disposition,
                    self.limits,
                );
            }
        }
    }
}
