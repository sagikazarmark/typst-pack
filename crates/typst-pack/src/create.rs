//! Creating a pack by compiling a project to discover the packages and fonts
//! it needs.
//!
//! Creation reads nothing. The caller supplies the project, package trees,
//! and font containers as values it already holds, so creation runs wherever
//! the core runs, including a host with no filesystem and no clock.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use typst::World;
use typst::diag::{SourceDiagnostic, SourceResult, Warned};
use typst::syntax::VirtualRoot;
use typst::syntax::package::PackageSpec;

use crate::compile::{Environment, Input, Options, RequestIssue};
use crate::pack::{FontInput, PackageContent, PackageInput};
use crate::world::{Loader, SourceWorld};
use crate::{Metadata, Pack, PackError, ProjectSnapshot, Sources, font, list, package};

/// The Typst document model creation compiles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Target {
    /// A paged document, as PDF and image output use.
    #[default]
    Paged,
    /// An HTML document.
    Html,
}

/// Everything one creation compiles over.
#[derive(Debug, Clone)]
pub struct Request {
    /// The project files and entrypoint; all of them become pack files.
    pub project: ProjectSnapshot,
    /// Package trees available to discovery.
    pub packages: package::Catalog,
    /// Font containers available to discovery, in selection order.
    pub fonts: Vec<font::Entry>,
    /// Packages the caller could not supply. A compile that imports one fails
    /// at the import with the recorded reason; a cataloged tree takes
    /// precedence.
    pub package_failures: HashMap<PackageSpec, package::ReadFailure>,
    /// The document model discovery compiles. HTML enables
    /// [`Feature::Html`](typst::Feature::Html) itself.
    pub target: Target,
    /// Inputs, document time, and features of the discovery compile.
    pub options: Options,
    /// Input overrides for additional discovery compiles. Each variant
    /// compiles `options` with its inputs replacing or adding to the base
    /// inputs, so packages and fonts reached only under other inputs are
    /// discovered too.
    pub variants: Vec<Vec<(String, Input)>>,
    /// Descriptive metadata, excluded from the pack identity.
    pub metadata: Option<Metadata>,
}

impl Request {
    /// Creation of `project` with no packages, fonts, or variants, the paged
    /// target, and default options.
    pub fn new(project: ProjectSnapshot) -> Self {
        Self {
            project,
            packages: package::Catalog::default(),
            fonts: Vec::new(),
            package_failures: HashMap::new(),
            target: Target::Paged,
            options: Options::default(),
            variants: Vec::new(),
            metadata: None,
        }
    }
}

/// A created pack.
#[derive(Debug)]
pub struct Created {
    /// The pack.
    pub pack: Pack,
    /// Warnings of every discovery compile, without repeats.
    pub warnings: Vec<SourceDiagnostic>,
    /// The sources the discovery compiles read.
    pub sources: Sources,
}

/// What one creation produced.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome {
    /// Every discovery compile succeeded.
    Created(Created),
    /// Packages discovery reached that no supplied tree covers, deduplicated
    /// and in specification order. Add them to the request's package catalog
    /// (or its package failures) and create again.
    MissingPackages(Vec<PackageSpec>),
}

/// A creation that produced no pack.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The request holds invalid values; nothing was compiled.
    #[error("{}", list("invalid create request", .0))]
    InvalidRequest(Vec<RequestIssue>),
    /// A discovery compile failed.
    #[error("{}", list("dependency discovery failed", &.errors.iter().map(|error| &error.message).collect::<Vec<_>>()))]
    Failed {
        errors: Vec<SourceDiagnostic>,
        warnings: Vec<SourceDiagnostic>,
        sources: Sources,
    },
    /// The discovered contents do not form a valid pack.
    #[error(transparent)]
    InvalidPack(#[from] PackError),
}

/// Compiles the project once per variant and creates the pack whose
/// requirements are every package and font container those compiles used, or
/// reports the packages they needed and were not given.
///
/// Project files come from the snapshot alone; the compiles select only
/// package and font requirements. Whole package trees travel, not just the
/// files a compile read.
///
/// # Resuming until every package is supplied
///
/// A failed import ends evaluation, so one round reports what that round
/// reached, and a project needing several packages completes over repeated
/// calls. The caller owns the loop, so any source can back it:
///
/// ```no_run
/// use typst::syntax::package::PackageSpec;
/// use typst_pack::create::{Outcome, Request};
/// use typst_pack::{Pack, ProjectSnapshot, create, package};
///
/// fn assemble(
///     project: ProjectSnapshot,
///     mut read_tree: impl FnMut(&PackageSpec) -> Result<package::Tree, Box<dyn std::error::Error>>,
/// ) -> Result<Pack, Box<dyn std::error::Error>> {
///     let mut request = Request::new(project);
///     loop {
///         match create(&request)? {
///             Outcome::Created(created) => return Ok(created.pack),
///             Outcome::MissingPackages(missing) => {
///                 for spec in missing {
///                     let tree = read_tree(&spec)?;
///                     request.packages.insert(spec, tree, true)?;
///                 }
///             }
///             outcome => return Err(format!("unexpected outcome: {outcome:?}").into()),
///         }
///     }
/// }
/// ```
pub fn create(request: &Request) -> Result<Outcome, Error> {
    let html = request.target == Target::Html;
    let mut issues = Vec::new();
    let environments = std::iter::once(request.options.clone())
        .chain(request.variants.iter().map(|inputs| {
            let mut options = request.options.clone();
            options.inputs.extend(inputs.iter().cloned());
            options
        }))
        .map(|options| Environment::new(&options, html, &mut issues))
        .collect::<Option<Vec<_>>>();
    issues.dedup();
    let Some(environments) = environments.filter(|_| issues.is_empty()) else {
        return Err(Error::InvalidRequest(issues));
    };

    let loader = Loader {
        project: request.project.clone(),
        packages: Arc::new(
            request
                .packages
                .entries()
                .map(|entry| (entry.spec.to_string(), entry.tree.clone()))
                .collect(),
        ),
        failures: Arc::new(request.package_failures.clone()),
    };
    let mut used_packages = BTreeMap::new();
    let mut missing = BTreeMap::new();
    let mut used_fonts = BTreeSet::new();
    let mut warnings = Vec::<SourceDiagnostic>::new();
    let mut failure = None;
    let mut sources = None;
    for (index, environment) in environments.into_iter().enumerate() {
        let variant = index
            .checked_sub(1)
            .map(|variant| &request.variants[variant]);
        let fonts = request
            .fonts
            .iter()
            .map(|entry| entry.container.fonts().to_vec());
        let mut world = SourceWorld::new(
            loader.clone(),
            fonts,
            environment.library,
            environment.clock,
        );
        let Warned {
            output,
            warnings: run_warnings,
        } = compile_target(&world, request.target);

        // Observed from the requests the compiler made, not from what its
        // diagnostics said about them.
        for id in world.requested_files() {
            let VirtualRoot::Package(spec) = id.root() else {
                continue;
            };
            if request.packages.get(spec).is_some() {
                used_packages.insert(spec.to_string(), spec.clone());
            } else if !request.package_failures.contains_key(spec) {
                // A specification declared unreadable already failed the
                // compile with the caller's reason; asking for it again would
                // ask for what the caller said it cannot supply.
                missing.insert(spec.to_string(), spec.clone());
            }
        }
        used_fonts.extend(world.used_containers());
        for warning in &run_warnings {
            if !warnings.contains(warning) {
                warnings.push(warning.clone());
            }
        }
        let run_sources = world.into_sources();
        if let Err(errors) = output
            && failure.is_none()
        {
            let mut errors = errors.into_iter().collect::<Vec<_>>();
            if let Some(inputs) = variant {
                let hint = variant_hint(inputs);
                errors
                    .iter_mut()
                    .for_each(|error| error.hint(hint.as_str()));
            }
            failure = Some(Error::Failed {
                errors,
                warnings: run_warnings.into_iter().collect(),
                sources: run_sources.clone(),
            });
        }
        sources.get_or_insert(run_sources);
    }

    // Reported before failures, because the import that needed a tree is
    // exactly what failed the compile.
    if !missing.is_empty() {
        return Ok(Outcome::MissingPackages(missing.into_values().collect()));
    }
    if let Some(failure) = failure {
        return Err(failure);
    }

    let packages = used_packages
        .into_values()
        .map(|spec| {
            let entry = request
                .packages
                .get(&spec)
                .expect("a used package is cataloged");
            PackageInput {
                content: PackageContent::Tree {
                    tree: entry.tree.clone(),
                    embedded: entry.embedded,
                },
                spec,
            }
        })
        .collect();
    // One requirement per container, with the disposition of its first used
    // position: the same file may be supplied twice, for example from two
    // font directories.
    let mut digests = BTreeSet::new();
    let fonts = used_fonts
        .into_iter()
        .map(|position| &request.fonts[position])
        .filter(|entry| digests.insert(entry.container.digest()))
        .map(|entry| FontInput::Container {
            container: entry.container.clone(),
            embedded: entry.embedded,
        })
        .collect();
    let pack = Pack::with_dependencies(
        request.project.clone(),
        request.metadata.clone(),
        packages,
        fonts,
    )?;
    Ok(Outcome::Created(Created {
        pack,
        warnings,
        sources: sources.expect("the base options always compile"),
    }))
}

/// Names the variant inputs a failed discovery compile ran with.
fn variant_hint(inputs: &[(String, Input)]) -> String {
    let inputs = inputs.iter().map(|(key, value)| match value {
        Input::Str(text) => format!("{key}={text}"),
        Input::Bytes(data) => format!("{key}=<{} bytes>", data.len()),
    });
    let inputs = inputs.collect::<Vec<_>>().join(",");
    format!("this error occurred in the discovery compile of variant `{inputs}`")
}

/// Compiles the target's document model, keeping only whether it succeeded.
fn compile_target(world: &dyn World, target: Target) -> Warned<SourceResult<()>> {
    match target {
        Target::Paged => {
            let Warned { output, warnings } = typst::compile::<typst_layout::PagedDocument>(world);
            Warned {
                output: output.map(drop),
                warnings,
            }
        }
        Target::Html => {
            let Warned { output, warnings } = typst::compile::<typst_html::HtmlDocument>(world);
            Warned {
                output: output.map(drop),
                warnings,
            }
        }
    }
}
