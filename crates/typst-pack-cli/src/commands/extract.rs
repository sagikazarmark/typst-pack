//! `typst-pack extract`: write a pack's files into a directory.

use std::path::{Path, PathBuf};

use typst_pack::pack_archive::{DecodeLimits, open_pack};
use typst_pack::{
    FilesystemWriteError, PackExtractionSelection, WriteIssue, WritePolicy, plan_pack_extraction,
    write_pack_extraction_plan_to_filesystem,
};

use super::CommandResult;
use crate::cli::ExtractArgs;

pub fn run(args: ExtractArgs) -> CommandResult {
    let pack = open_pack(&args.pack, DecodeLimits::DEFAULT)?;
    let output = args.output.unwrap_or_else(|| match args.pack.file_stem() {
        Some(stem) => PathBuf::from(stem),
        None => PathBuf::from("extracted"),
    });
    let selection = PackExtractionSelection::new(args.packages || args.all, args.fonts || args.all);
    let plan = plan_pack_extraction(&pack, selection)?;
    let policy = if args.force {
        WritePolicy::Overwrite
    } else {
        WritePolicy::CreateNew
    };
    write_pack_extraction_plan_to_filesystem(&plan, &output, policy)
        .map_err(|error| describe(&error, &output))?;
    println!(
        "extracted {} file(s) into `{}`",
        plan.entries().len(),
        output.display()
    );
    Ok(())
}

fn describe(error: &FilesystemWriteError, destination: &Path) -> String {
    let FilesystemWriteError::Rejected { issues } = error else {
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
