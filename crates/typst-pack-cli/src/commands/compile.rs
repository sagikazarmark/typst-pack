//! `typst-pack compile`: compile a pack.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use typst::foundations::Smart;
use typst::syntax::VirtualRoot;
use typst_pack::Sources;
use typst_pack::archive::{self, DecodeLimits};
use typst_pack::compile::{
    self, CreationTimestamp, DocumentTime, Error, Limits, Output, OutputFormat, PageSelection,
    Request,
};

use super::{
    CommandResult, Failure, Terminal, Timings, check_timestamp, compile_options, initialize_jobs,
    now, packer, write_to,
};
use crate::cli::CompileArgs;
use crate::output;

pub fn run(args: CompileArgs, terminal: Terminal, cert: Option<PathBuf>) -> CommandResult {
    // Arguments are checked before any I/O, in Typst's order.
    let stdin = args.pack == Path::new("-");
    if stdin && args.output.is_none() {
        return Err("an explicit output is required when the Pack is read from stdin".into());
    }
    let format = format(&args)?;
    let standards = pdf_standards(&args)?;
    let stdout = Some(Path::new("-"));
    if args.output.as_deref() == stdout && args.deps.as_deref() == stdout {
        return Err("cannot write both output and dependencies to stdout".into());
    }
    check_timestamp(args.automation.creation_timestamp)?;
    initialize_jobs(args.automation.jobs);

    let pack = if stdin {
        archive::read(std::io::stdin().lock(), DecodeLimits::default())
            .map_err(|error| format!("cannot read Pack from stdin: {error}"))?
    } else {
        archive::open(&args.pack, DecodeLimits::default())
            .map_err(|error| format!("cannot open pack `{}`: {error}", args.pack.display()))?
    };
    // An explicit timestamp is recorded in UTC, the current time locally.
    let (unix, timestamp) = match args.automation.creation_timestamp {
        Some(unix) => (unix, CreationTimestamp::Utc(unix)),
        None => {
            let (unix, offset_minutes) = now();
            (
                unix,
                CreationTimestamp::Local {
                    unix,
                    offset_minutes,
                },
            )
        }
    };
    let (options, input_files) =
        compile_options(&args.compilation, DocumentTime::UnixTimestamp(unix))?;
    let resolved = packer(&args.fonts, &args.packages, cert).resolve(&pack)?;
    let defaults = Limits::default();
    let workers = u64::try_from(rayon::current_num_threads()).unwrap_or(defaults.export_workers);
    let request = Request {
        output: Output {
            pages: PageSelection {
                ranges: args
                    .pages
                    .iter()
                    .flat_map(|pages| pages.ranges.clone())
                    .collect(),
            },
            pretty: args.pretty,
            ppi: Some(args.ppi),
            pdf_standards: standards,
            pdf_tags: if args.no_pdf_tags {
                Smart::Custom(false)
            } else {
                Smart::Auto
            },
            pdf_timestamp: timestamp,
            ..Output::new(format)
        },
        options,
        packages: resolved.packages,
        fonts: resolved.fonts,
        limits: Limits {
            export_workers: workers.clamp(1, defaults.export_workers),
            ..defaults
        },
        pack,
    };

    let timings = Timings::start(args.automation.timings.clone());
    let result = compile::compile(&request);
    let sources = match &result {
        Ok(compiled) => Some(&compiled.sources),
        Err(Error::Failed { sources, .. } | Error::Export { sources, .. }) => Some(sources),
        Err(_) => None,
    };
    let timing_error = timings.finish(sources);

    // The pack, the input files, and the files read from external packages.
    let mut inputs = input_files.into_iter().collect::<BTreeSet<_>>();
    inputs.extend((!stdin).then(|| args.pack.clone()));
    inputs.extend(
        sources
            .map_or(&[][..], Sources::files)
            .iter()
            .filter_map(|id| {
                let VirtualRoot::Package(spec) = id.root() else {
                    return None;
                };
                let root = resolved.package_dirs.get(&spec.to_string())?;
                Some(root.join(id.vpath().get_without_slash()))
            }),
    );
    let write_dependencies = |outputs: Option<&[PathBuf]>| -> CommandResult {
        let Some(destination) = &args.deps else {
            return Ok(());
        };
        let bytes = output::dependencies(args.deps_format, &inputs, outputs)?;
        Ok(write_to(destination, &bytes, "dependencies")?)
    };
    let failed = || write_dependencies(None).and(Err(Failure::Reported));

    let outcome = match result {
        Ok(compiled) => {
            let output = args.output.as_deref();
            let written = output::targets(output, &args.pack, format, &compiled)
                .and_then(|targets| output::write(&targets, &compiled).map(|()| targets));
            match written {
                Ok(targets) => {
                    terminal.diagnostics(&compiled.sources, &compiled.warnings);
                    open(&args, &targets).and_then(|()| write_dependencies(Some(&targets)))
                }
                Err(error) => {
                    terminal.error(&error);
                    terminal.diagnostics(&compiled.sources, &compiled.warnings);
                    failed()
                }
            }
        }
        Err(Error::Failed {
            errors,
            warnings,
            sources,
        }) => {
            terminal.diagnostics(&sources, warnings.iter().chain(&errors));
            failed()
        }
        Err(
            ref error @ Error::Export {
                ref warnings,
                ref sources,
                ..
            },
        ) => {
            terminal.error(&error.to_string());
            terminal.diagnostics(sources, warnings);
            failed()
        }
        Err(error) => {
            let mut hints = error.hints();
            if matches!(error, Error::Unfulfilled(_)) {
                hints.push(format!(
                    "`typst-pack inspect {}` lists every requirement this pack expects to be \
                     supplied",
                    args.pack.display()
                ));
            }
            let message = error.to_string();
            Err(Failure::Error { message, hints })
        }
    };
    // As in Typst's CLI, a timing failure replaces a failure not yet printed.
    match timing_error {
        Some(error) => Err(error.into()),
        None => outcome,
    }
}

/// The output format: explicit, inferred from the output's extension, or PDF.
fn format(args: &CompileArgs) -> Result<OutputFormat, Failure> {
    let path = match (args.format, args.output.as_deref()) {
        (Some(format), _) => return Ok(format),
        (None, Some(path)) if path != Path::new("-") => path,
        (None, _) => return Ok(OutputFormat::Pdf),
    };
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension {
        Some(extension) => OutputFormat::from_extension(extension).ok_or_else(|| {
            format!("cannot infer output format from extension `{extension}`; pass --format")
        }),
        None => Err("cannot infer output format; pass --format".to_owned()),
    }
    .map_err(Failure::from)
}

/// The PDF standards, checked as Typst's CLI checks them before compiling.
fn pdf_standards(args: &CompileArgs) -> Result<Vec<typst_pdf::PdfStandard>, Failure> {
    let (standards, tagged): (Vec<_>, Vec<_>) = args.pdf_standards.iter().copied().unzip();
    if let Err(error) = typst_pdf::PdfStandards::new(&standards) {
        let hints = error.hints().iter().map(ToString::to_string).collect();
        let message = error.message().to_string();
        return Err(Failure::Error { message, hints });
    }
    let untagged = args.no_pdf_tags || !args.pages.is_empty();
    if let Some(name) = tagged.into_iter().flatten().next().filter(|_| untagged) {
        let message = format!("cannot disable PDF tags when exporting a {name} document");
        let hint = (!args.no_pdf_tags).then(|| "using --pages implies --no-pdf-tags".to_owned());
        let hints = hint.into_iter().collect();
        return Err(Failure::Error { message, hints });
    }
    Ok(standards)
}

/// Opens the first written artifact in a viewer, unless it went to stdout.
fn open(args: &CompileArgs, targets: &[PathBuf]) -> CommandResult {
    let (Some(viewer), Some(first)) = (&args.open, targets.first()) else {
        return Ok(());
    };
    if first == Path::new("-") {
        return Ok(());
    }
    let first = first
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize path ({error})"))?;
    match viewer {
        Some(viewer) => open::with_detached(&first, viewer),
        None => open::that_detached(&first),
    }
    .map_err(Failure::from)
}
