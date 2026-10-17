//! `typst-pack create`: pack a project directory.

use std::path::{Path, PathBuf};

use typst_pack::compile::{DocumentTime, Input};
use typst_pack::{FILE_EXTENSION, Metadata, archive, create, fs};

use super::{
    CommandResult, Failure, Terminal, Timings, check_timestamp, compile_options, initialize_jobs,
    now, packer,
};
use crate::cli::CreateArgs;

pub fn run(args: CreateArgs, terminal: Terminal, cert: Option<PathBuf>) -> CommandResult {
    check_timestamp(args.automation.creation_timestamp)?;
    initialize_jobs(args.automation.jobs);
    if args.input == Path::new("-") {
        return Err("create input must be a named Typst source file, not stdin".into());
    }
    let input = args
        .input
        .canonicalize()
        .map_err(|error| format!("cannot access `{}`: {error}", args.input.display()))?;
    if !input.is_file() {
        let message = "create input must be a Typst source file";
        return Err(format!("{message}: `{}`", args.input.display()).into());
    }
    let root = match args.root {
        Some(root) => root,
        None => input
            .parent()
            .ok_or("cannot determine project root")?
            .into(),
    };
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension(FILE_EXTENSION));

    let timestamp = args
        .automation
        .creation_timestamp
        .unwrap_or_else(|| now().0);
    let (options, _) = compile_options(&args.compilation, DocumentTime::UnixTimestamp(timestamp))?;
    let metadata = Metadata {
        name: args.name,
        description: args.description,
        authors: args.authors,
    };
    let packer = fs::Packer {
        vendor_packages: !args.no_vendor_packages,
        embed_fonts: args.embed_fonts,
        include_typst_embedded_fonts: args.include_typst_embedded_fonts,
        options,
        target: args.target.unwrap_or_default(),
        variants: args
            .variants
            .into_iter()
            .map(|variant| {
                let inputs = variant.0.into_iter();
                inputs
                    .map(|(key, value)| (key, Input::Str(value)))
                    .collect()
            })
            .collect(),
        metadata: (metadata != Metadata::default()).then_some(metadata),
        ..packer(&args.fonts, &args.packages, cert)
    };

    let timings = Timings::start(args.automation.timings);
    let packed = packer.pack(&root, &input);
    let sources = match &packed {
        Ok(packed) => Some(&packed.sources),
        Err(fs::Error::Create(create::Error::Failed { sources, .. })) => Some(sources),
        Err(_) => None,
    };
    let timing_error = timings.finish(sources);
    let packed = match packed {
        Ok(packed) => packed,
        Err(fs::Error::Create(create::Error::Failed {
            errors,
            warnings,
            sources,
        })) => {
            terminal.diagnostics(&sources, errors.iter().chain(&warnings));
            return Err(timing_error.map_or(Failure::Reported, Failure::from));
        }
        Err(error) => {
            if let Some(timing_error) = timing_error {
                terminal.error(&error.to_string());
                return Err(timing_error.into());
            }
            return Err(error.into());
        }
    };

    terminal.diagnostics(&packed.sources, &packed.warnings);
    for skipped in &packed.skipped_fonts {
        terminal.warning(&format!("skipped font: {skipped}"));
    }
    if let Some(error) = timing_error {
        return Err(error.into());
    }

    let pack = &packed.pack;
    if output == Path::new("-") {
        archive::write(std::io::stdout().lock(), pack)
            .map_err(|error| format!("cannot write Pack to stdout: {error}"))?;
        return Ok(());
    }
    archive::save(&output, pack, fs::WritePolicy::Overwrite)?;

    let requirements = pack.package_requirements();
    let (vendored, unvendored): (Vec<_>, Vec<_>) = requirements
        .iter()
        .partition(|requirement| requirement.embedded);
    println!(
        "packed {} project file(s), {} package(s), {} font(s) into `{}`",
        pack.files().count(),
        vendored.len(),
        pack.font_requirements().len(),
        output.display(),
    );
    if !unvendored.is_empty() {
        println!(
            "note: {} package(s) were not vendored and must be available when compiling:",
            unvendored.len()
        );
        for requirement in unvendored {
            println!("  {}", requirement.spec);
        }
    }
    Ok(())
}
