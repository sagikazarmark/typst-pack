//! `typst-pack extract`: write a pack's files into a directory.

use std::path::{Path, PathBuf};

use typst_pack::extract::{self, Selection};
use typst_pack::fs::{self, WriteError, WriteIssue, WritePolicy};

use super::{CommandResult, Failure, chain, escape, listed, read_pack};
use crate::cli::ExtractArgs;

pub fn run(args: ExtractArgs) -> CommandResult {
    let output = match (args.output, args.pack.file_stem()) {
        (Some(output), _) => output,
        (None, _) if args.pack == Path::new("-") => {
            return Err("an explicit --output is required when the pack is read from stdin".into());
        }
        (None, Some(stem)) => PathBuf::from(stem),
        (None, None) => PathBuf::from("extracted"),
    };
    let pack = read_pack(&args.pack)?;
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
    fs::write_extraction(&output, &plan, policy).map_err(|error| describe(&error, &output))?;
    println!(
        "extracted {} file(s) into `{}`",
        plan.entries().len(),
        output.display()
    );
    Ok(())
}

/// The most refusal reasons listed; the rest are counted.
const LISTED_ISSUES: usize = 20;

/// Why a write was refused (the first [`LISTED_ISSUES`] reasons, then how
/// many more), suggesting `--force` only when existing files were all that
/// stood in the way.
fn describe(error: &WriteError, destination: &Path) -> Failure {
    let WriteError::Rejected { issues } = error else {
        return escape(&chain(error)).into();
    };
    let conflict = |path: &Path| {
        format!(
            "existing destination entry `{}` conflicts with extraction",
            path.display()
        )
    };
    let lines = issues.iter().take(LISTED_ISSUES).map(|issue| {
        escape(&match issue {
            WriteIssue::ExistingTarget { path } => {
                format!("`{}` already exists", destination.join(path).display())
            }
            WriteIssue::ConflictingTarget { path } => conflict(&destination.join(path)),
            WriteIssue::ConflictingAncestor { ancestor, .. } => conflict(ancestor),
            issue => issue.to_string(),
        })
    });
    let more = issues.len().saturating_sub(LISTED_ISSUES);
    let more = (more > 0).then(|| format!("and {more} more"));
    let existing = |issue| matches!(issue, &WriteIssue::ExistingTarget { .. });
    let force = !issues.is_empty() && issues.iter().all(existing);
    Failure::Error {
        message: listed("nothing was extracted", lines.chain(more)),
        hints: force
            .then(|| "pass --force to overwrite existing files".to_owned())
            .into_iter()
            .collect(),
    }
}
