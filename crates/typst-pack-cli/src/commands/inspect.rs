//! `typst-pack inspect`: list what a pack contains.

use std::collections::BTreeSet;
use std::path::Path;

use typst_pack::archive::FORMAT_VERSION;
use typst_pack::font;

use super::{CommandResult, Terminal, escape, read_pack, warn_typst_version};
use crate::cli::InspectArgs;

pub fn run(args: InspectArgs, terminal: Terminal) -> CommandResult {
    let pack = read_pack(&args.pack)?;
    warn_typst_version(&pack, terminal);

    // Names and paths come from the pack and are printed escaped.
    if args.pack == Path::new("-") {
        println!("pack: <stdin>");
    } else {
        println!("pack: {}", args.pack.display());
    }
    println!("format version: {FORMAT_VERSION}");
    if let Some(version) = pack.typst_version() {
        println!("typst version: {}", escape(version));
    }
    if let Some(generator) = pack.generator() {
        println!("generator: {}", escape(generator));
    }
    println!("identity: {}", pack.identity());
    println!("entrypoint: {}", escape(pack.entrypoint()));
    if let Some(metadata) = pack.metadata() {
        if let Some(name) = &metadata.name {
            println!("name: {}", escape(name));
        }
        if let Some(description) = &metadata.description {
            println!("description: {}", escape(description));
        }
        if !metadata.authors.is_empty() {
            println!("authors: {}", escape(&metadata.authors.join(", ")));
        }
    }

    println!("\npacked project files:");
    for (path, data) in pack.files() {
        println!("  {} ({})", escape(path), human_size(data.len()));
    }

    let vendored = pack.packages().collect::<Vec<_>>();
    if !vendored.is_empty() {
        println!("\nvendored packages:");
        for (spec, tree) in vendored {
            let (count, size) = tree.files().fold((0, 0), |(count, size), (_, data)| {
                (count + 1, size + data.len())
            });
            println!("  {spec} ({count} files, {})", human_size(size));
        }
    }
    let unvendored = pack.package_requirements().iter().filter(|r| !r.embedded);
    let unvendored = unvendored.collect::<Vec<_>>();
    if !unvendored.is_empty() {
        println!("\nunvendored packages:");
        for requirement in unvendored {
            println!("  {}", requirement.spec);
        }
    }

    let line = |requirement: &font::Requirement| {
        let faces = requirement.faces.iter().map(ToString::to_string);
        let faces = faces.collect::<Vec<_>>().join("; ");
        format!("  {} - {}", requirement.digest, escape(&faces))
    };
    let (embedded, required): (Vec<_>, Vec<_>) =
        pack.font_requirements().iter().partition(|r| r.embedded);
    if !embedded.is_empty() {
        println!("\nembedded fonts:");
        for requirement in embedded {
            let size = pack.font_data(&requirement.digest).map_or(0, <[u8]>::len);
            println!("{} ({})", line(requirement), human_size(size));
        }
    }
    // External fonts are identified by content digest, so this listing is the
    // only place a recipient learns which fonts a pack expects them to supply.
    if !required.is_empty() {
        let typst = font::embedded()
            .map(|container| container.digest())
            .collect::<BTreeSet<_>>();
        println!("\nrequired fonts:");
        for requirement in required {
            let note = if typst.contains(&requirement.digest) {
                "provided by Typst"
            } else {
                "install it or supply it with --font-path"
            };
            println!("{} ({note})", line(requirement));
        }
    }
    Ok(())
}

fn human_size(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
