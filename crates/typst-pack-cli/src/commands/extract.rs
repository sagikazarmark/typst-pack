//! `typst-pack extract`: write a pack's files into a directory.

use std::path::{Path, PathBuf};

use typst_pack::archive::{self, DecodeLimits};
use typst_pack::extract::{self, Selection};
use typst_pack::fs::{self, WriteError, WriteIssue, WritePolicy};

use super::CommandResult;
use crate::cli::ExtractArgs;

pub fn run(args: ExtractArgs) -> CommandResult {
    let pack = archive::open(&args.pack, DecodeLimits::default())
        .map_err(|error| format!("cannot open pack `{}`: {error}", args.pack.display()))?;
    let output = args.output.unwrap_or_else(|| match args.pack.file_stem() {
        Some(stem) => PathBuf::from(stem),
        None => PathBuf::from("extracted"),
    });
    let selection = Selection {
        packages: args.packages || args.all,
        fonts: args.fonts || args.all,
    };
    let plan = extract::plan(&pack, selection)?;
    let policy = if args.force {
        WritePolicy::Overwrite
    } else {
        WritePolicy::CreateNew
    };
    fs::write_extraction(&plan, &output, policy).map_err(|error| describe(&error, &output))?;
    println!(
        "extracted {} file(s) into `{}`",
        plan.entries().len(),
        output.display()
    );
    Ok(())
}

fn describe(error: &WriteError, destination: &Path) -> String {
    let WriteError::Rejected { issues } = error else {
        return error.to_string();
    };
    let conflict = |path: &Path| {
        format!(
            "existing destination entry `{}` conflicts with extraction",
            path.display()
        )
    };
    match issues.first() {
        Some(WriteIssue::ExistingTarget { path }) => format!(
            "`{}` already exists (pass --force to overwrite)",
            destination.join(path).display()
        ),
        Some(WriteIssue::ConflictingTarget { path }) => conflict(&destination.join(path)),
        Some(WriteIssue::ConflictingAncestor { ancestor, .. }) => conflict(ancestor),
        _ => error.to_string(),
    }
}
