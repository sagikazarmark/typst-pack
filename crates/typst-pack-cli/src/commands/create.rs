//! `typst-pack create`: pack a project directory.

use std::path::{Path, PathBuf};

use typst_kit::diagnostics::termcolor::Color;
use typst_pack::compile::Input;
use typst_pack::{FILE_EXTENSION, Metadata, archive, create, fs};

use super::{
    CommandResult, Failure, Terminal, Timings, chain, compile_options, packer, prepare, time,
};
use crate::cli::CreateArgs;

pub fn run(args: CreateArgs, terminal: Terminal, cert: Option<PathBuf>) -> CommandResult {
    prepare(&args.automation)?;
    if args.input == Path::new("-") {
        return Err("create input must be a named Typst source file, not stdin".into());
    }
    let input = args
        .input
        .canonicalize()
        .map_err(|error| format!("cannot access `{}`: {error}", args.input.display()))?;
    if !input.is_file() {
        let input = args.input.display();
        return Err(format!("create input must be a Typst source file: `{input}`").into());
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

    let (options, _) = compile_options(&args.compilation, time(&args.automation).0)?;
    let metadata = Metadata {
        name: args.name,
        description: args.description,
        authors: args.authors,
    };
    let variants = args.variants.into_iter().map(|variant| {
        let inputs = variant.0.into_iter();
        inputs
            .map(|(key, value)| (key, Input::Str(value)))
            .collect()
    });
    let packer = fs::Packer {
        embed_packages: !args.no_vendor_packages,
        embed_fonts: args.embed_fonts,
        include_typst_embedded_fonts: args.include_typst_embedded_fonts,
        options,
        target: args.target.unwrap_or_default(),
        variants: variants.collect(),
        metadata: (metadata != Metadata::default()).then_some(metadata),
        ..packer(&args.fonts, &args.packages, cert)
    };

    let timings = Timings::start(args.automation.timings);
    let packed = packer.pack(&root, &input);
    let sources = match &packed {
        Ok(packed) => Some(&packed.sources),
        Err(fs::Error::Create(error)) => match &**error {
            create::Error::Failed { sources, .. } => Some(sources),
            _ => None,
        },
        Err(_) => None,
    };
    let timing_error = timings.finish(sources);
    let packed = match packed {
        Ok(packed) => packed,
        Err(error) => {
            if let fs::Error::Create(failure) = &error
                && let create::Error::Failed {
                    errors,
                    warnings,
                    sources,
                } = &**failure
            {
                terminal.diagnostics(sources, errors.iter().chain(warnings));
                return Err(timing_error.map_or(Failure::Reported, Failure::from));
            }
            let Some(timing_error) = timing_error else {
                return Err(error.into());
            };
            terminal.error(&chain(&error));
            return Err(timing_error.into());
        }
    };

    terminal.diagnostics(&packed.sources, &packed.warnings);
    for skipped in &packed.skipped_fonts {
        let message = format!("skipped font: {}", chain(skipped));
        terminal.label("warning", Color::Yellow, &message);
    }
    if let Some(error) = timing_error {
        return Err(error.into());
    }

    let pack = &packed.pack;
    if output == Path::new("-") {
        archive::write(std::io::stdout().lock(), pack)
            .map_err(|error| format!("cannot write pack to stdout: {error}"))?;
        return Ok(());
    }
    fs::write_archive(&output, pack, fs::WritePolicy::Overwrite)?;

    let (vendored, unvendored): (Vec<_>, Vec<_>) = pack
        .package_requirements()
        .iter()
        .partition(|requirement| requirement.embedded);
    let fonts = pack.font_requirements();
    println!(
        "packed {} project file(s), {} package(s), {} font(s) ({} embedded) into `{}`",
        pack.files().count(),
        vendored.len(),
        fonts.len(),
        fonts.iter().filter(|font| font.embedded).count(),
        output.display(),
    );
    if !unvendored.is_empty() {
        let count = unvendored.len();
        println!(
            "note: {count} package(s) were not vendored and must be available when compiling:"
        );
        for requirement in unvendored {
            println!("  {}", requirement.spec);
        }
    }
    Ok(())
}
