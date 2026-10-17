//! `typst-pack inspect`: list what a pack contains.

use typst_pack::FontRequirement;
use typst_pack::pack_archive::{DecodeLimits, FORMAT_VERSION, open_pack};

use super::CommandResult;
use crate::cli::InspectArgs;

pub fn run(args: InspectArgs) -> CommandResult {
    let pack = open_pack(&args.pack, DecodeLimits::DEFAULT)?;

    println!("pack: {}", args.pack.display());
    println!("format version: {FORMAT_VERSION}");
    println!("entrypoint: {}", pack.entrypoint());
    if let Some(metadata) = pack.metadata() {
        if let Some(name) = &metadata.name {
            println!("name: {name}");
        }
        if let Some(description) = &metadata.description {
            println!("description: {description}");
        }
        if !metadata.authors.is_empty() {
            println!("authors: {}", metadata.authors.join(", "));
        }
    }

    println!("\npacked project files:");
    for (path, data) in pack.files() {
        println!("  {path} ({})", human_size(data.len()));
    }

    let vendored = pack.packages().collect::<Vec<_>>();
    if !vendored.is_empty() {
        println!("\nvendored packages:");
        for (spec, files) in vendored {
            let (count, size) = files.fold((0, 0), |(count, size), (_, data)| {
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

    let line = |requirement: &FontRequirement| {
        let faces = requirement.faces.iter().map(ToString::to_string);
        let faces = faces.collect::<Vec<_>>().join("; ");
        format!("  {} - {faces}", requirement.digest)
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
        println!("\nrequired fonts (supply with --font-path at compile time):");
        for requirement in required {
            println!("{}", line(requirement));
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
