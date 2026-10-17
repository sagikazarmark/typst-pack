//! Creating a pack from OpenDAL storage.

use std::collections::HashSet;

use ::opendal::Operator;

use super::{
    Error, FontSource, PackageSource, insert_package, read_fonts, read_package, read_project,
};
use crate::compile::{Input, Options};
use crate::create::{self, Created, Outcome, Target};
use crate::package::ReadFailure;
use crate::{Metadata, ReadLimits};

/// Creates a pack from a project, fonts, and packages held in OpenDAL
/// storage.
///
/// [`assemble`](Self::assemble) reads the project and fonts, then runs
/// creation. Whenever creation reports missing packages, each one is read
/// from [`packages`](Self::packages) in order, inserted, and creation is
/// resumed. A package that is unavailable, exceeds a read limit (while being
/// read or expanded), or fails to expand or validate is recorded as a package
/// read failure, so creation reports it at the import that needs it. Storage
/// failures, and read limits on the project or fonts, end assembly.
///
/// The future is caller-polled and needs no particular async runtime.
/// Between storage reads it runs [`create()`](crate::create()) synchronously,
/// which compiles the document and can take a while: the polling thread is
/// blocked meanwhile. On a shared async executor, poll the future from a
/// blocking-capable task (for example Tokio's `spawn_blocking` with
/// `Handle::block_on`), or drive the building blocks
/// ([`read_project`](super::read_project), [`read_package`],
/// [`insert_package`]) yourself and run `create()` on a blocking thread.
#[derive(Clone, Debug)]
pub struct Assembler {
    /// The operator holding the project and font prefixes.
    pub operator: Operator,
    /// The project prefix (`""` or ending in `/`).
    pub project: String,
    /// The entrypoint path relative to the project prefix.
    pub entrypoint: String,
    /// Font prefixes, read in order.
    pub fonts: Vec<FontSource>,
    /// Package sources, tried in order for each missing package.
    pub packages: Vec<PackageSource>,
    /// Whether created packs embed the packages read.
    pub embed_packages: bool,
    /// The document model discovery compiles.
    pub target: Target,
    /// Inputs, document time, and features of the discovery compile.
    pub options: Options,
    /// Input overrides for additional discovery compiles.
    pub variants: Vec<Vec<(String, Input)>>,
    /// Optional pack metadata.
    pub metadata: Option<Metadata>,
    /// Limits for reading the project and the fonts.
    pub limits: ReadLimits,
    /// Limits for reading each package and expanding each package archive.
    pub package_limits: ReadLimits,
}

impl Assembler {
    /// An assembler with no fonts or package sources, embedded packages,
    /// default options, default limits, and [`ReadLimits::PACKAGE`] for
    /// packages.
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
            embed_packages: true,
            target: Target::Paged,
            options: Options::default(),
            variants: Vec::new(),
            metadata: None,
            limits: ReadLimits::default(),
            package_limits: ReadLimits::PACKAGE,
        }
    }

    /// Reads every input and runs creation to completion.
    pub async fn assemble(&self) -> Result<Created, Error> {
        let project =
            read_project(&self.operator, &self.project, &self.entrypoint, self.limits).await?;
        let mut request = create::Request::new(project);
        request.fonts = read_fonts(&self.operator, &self.fonts, self.limits).await?;
        request.target = self.target;
        request.options = self.options.clone();
        request.variants = self.variants.clone();
        request.metadata = self.metadata.clone();
        let mut attempted = HashSet::new();
        loop {
            let outcome =
                create::create(&request).map_err(|error| Error::Create(Box::new(error)))?;
            let missing = match outcome {
                Outcome::Created(created) => return Ok(created),
                Outcome::MissingPackages(missing) => missing,
            };
            for spec in missing {
                if !attempted.insert(spec.to_string()) {
                    return Err(Error::RepeatedPackage { spec });
                }
                // A package over a read limit, like one that fails to expand
                // or validate, is recorded as a failure; resumed creation
                // reports it at the importing source span.
                let read = match read_package(&self.packages, &spec, self.package_limits).await {
                    Ok(read) => read,
                    Err(error @ Error::Limit { .. }) => {
                        let failure = ReadFailure::Other(Some(crate::chain(&error)));
                        request.package_failures.insert(spec, failure);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let _ = insert_package(
                    &mut request,
                    &spec,
                    read,
                    self.embed_packages,
                    self.package_limits,
                );
            }
        }
    }
}
