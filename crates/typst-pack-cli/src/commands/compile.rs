//! `typst-pack compile`: compile a pack.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use typst::foundations::Smart;
use typst::syntax::VirtualRoot;
use typst_pack::Sources;
use typst_pack::archive::{self, DecodeLimits};
use typst_pack::compile::{self, CreationTimestamp, DocumentTime, Error, Limits, Output};
use typst_pack::compile::{OutputFormat, PageSelection, Request};
use typst_pdf::PdfStandard;

use super::{CommandResult, Failure, Terminal, Timings, compile_options, now, packer, prepare};
use crate::cli::CompileArgs;
use crate::output;

pub fn run(args: CompileArgs, terminal: Terminal, cert: Option<PathBuf>) -> CommandResult {
    // Arguments are checked before any I/O, in Typst's order.
    let stdin = args.pack == Path::new("-");
    if stdin && args.output.is_none() {
        return Err("an explicit output is required when the Pack is read from stdin".into());
    }
    let format = format(&args)?;
    check_pdf_standards(&args)?;
    let stdout = Some(Path::new("-"));
    if args.output.as_deref() == stdout && args.deps.as_deref() == stdout {
        return Err("cannot write both output and dependencies to stdout".into());
    }
    prepare(&args.automation)?;

    let pack = if stdin {
        archive::read(std::io::stdin().lock(), DecodeLimits::default())
            .map_err(|error| format!("cannot read Pack from stdin: {error}"))?
    } else {
        archive::open(&args.pack, DecodeLimits::default())
            .map_err(|error| format!("cannot open pack `{}`: {error}", args.pack.display()))?
    };
    // An explicit timestamp is recorded in UTC, the current time locally.
    let (unix, pdf_timestamp) = match args.automation.creation_timestamp {
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
    let ranges = args.pages.iter().flat_map(|pages| pages.ranges.clone());
    let request = Request {
        output: Output {
            pages: PageSelection {
                ranges: ranges.collect(),
            },
            pretty: args.pretty,
            ppi: Some(args.ppi),
            pdf_standards: args.pdf_standards.clone(),
            pdf_tags: if args.no_pdf_tags {
                Smart::Custom(false)
            } else {
                Smart::Auto
            },
            pdf_timestamp,
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
    let files = sources.map_or(&[][..], Sources::files);
    inputs.extend(files.iter().filter_map(|id| {
        let VirtualRoot::Package(spec) = id.root() else {
            return None;
        };
        let root = resolved.package_dirs.get(&spec.to_string())?;
        Some(root.join(id.vpath().get_without_slash()))
    }));
    let write_dependencies = |outputs: Option<&[PathBuf]>| -> CommandResult {
        let Some(destination) = &args.deps else {
            return Ok(());
        };
        let bytes = output::dependencies(args.deps_format, &inputs, outputs)?;
        let written = if destination == Path::new("-") {
            std::io::stdout().lock().write_all(&bytes)
        } else {
            std::fs::write(destination, bytes)
        };
        written.map_err(|error| {
            let to = if destination == Path::new("-") {
                "stdout".to_owned()
            } else {
                format!("`{}`", destination.display())
            };
            format!("cannot write dependencies to {to}: {error}").into()
        })
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
            Err(Failure::Error {
                message: error.to_string(),
                hints,
            })
        }
    };
    // As in Typst's CLI, a timing failure replaces a failure not yet printed.
    timing_error.map_or(outcome, |error| Err(error.into()))
}

/// The output format: explicit, inferred from the output's extension, or PDF.
fn format(args: &CompileArgs) -> Result<OutputFormat, Failure> {
    let path = match (args.format, args.output.as_deref()) {
        (Some(format), _) => return Ok(format),
        (None, Some(path)) if path != Path::new("-") => path,
        (None, _) => return Ok(OutputFormat::Pdf),
    };
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => OutputFormat::from_extension(extension).ok_or_else(|| {
            format!("cannot infer output format from extension `{extension}`; pass --format").into()
        }),
        None => Err("cannot infer output format; pass --format".into()),
    }
}

/// Checks the PDF standards as Typst's CLI does before compiling.
fn check_pdf_standards(args: &CompileArgs) -> CommandResult {
    if let Err(error) = typst_pdf::PdfStandards::new(&args.pdf_standards) {
        let hints = error.hints().iter().map(ToString::to_string).collect();
        return Err(Failure::Error {
            message: error.message().to_string(),
            hints,
        });
    }
    // The first standard that requires tags.
    let tagged = args
        .pdf_standards
        .iter()
        .find_map(|standard| match standard {
            PdfStandard::A_1a => Some("PDF/A-1a"),
            PdfStandard::A_2a => Some("PDF/A-2a"),
            PdfStandard::A_3a => Some("PDF/A-3a"),
            PdfStandard::Ua_1 => Some("PDF/UA-1"),
            _ => None,
        });
    match tagged {
        Some(name) if args.no_pdf_tags || !args.pages.is_empty() => {
            let message = format!("cannot disable PDF tags when exporting a {name} document");
            let hint = (!args.no_pdf_tags).then(|| "using --pages implies --no-pdf-tags".into());
            Err(Failure::Error {
                message,
                hints: hint.into_iter().collect(),
            })
        }
        _ => Ok(()),
    }
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
