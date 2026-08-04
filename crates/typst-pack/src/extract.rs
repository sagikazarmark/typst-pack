//! Extracting a pack back into a directory.

#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};

use crate::{
    Pack, PackExtractionEntry, PackExtractionPlanError, PackExtractionSelection,
    plan_pack_extraction,
};

/// Options for [`extract`].
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    /// Also write vendored packages to `packages/<ns>/<name>/<version>/...`.
    pub packages: bool,
    /// Also write embedded fonts to their archive paths (`fonts/...`).
    pub fonts: bool,
    /// Overwrite existing files.
    pub force: bool,
}

/// A summary of an extraction.
#[derive(Debug, Clone, Default)]
pub struct ExtractReport {
    /// Paths written, relative to the target directory.
    pub written: Vec<PathBuf>,
}

/// A failure while extracting a pack.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Plan(#[from] PackExtractionPlanError),
    #[error("`{0}` already exists (pass force to overwrite)")]
    Exists(PathBuf),
    #[error("existing destination entry `{0}` conflicts with extraction")]
    DestinationConflict(PathBuf),
    #[error("failed to write `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Writes the project files of a pack into a directory.
///
/// Project files are written directly into `dir` so that the result is a
/// compilable project. With [`packages`](ExtractOptions::packages) and
/// [`fonts`](ExtractOptions::fonts), the vendored packages and embedded fonts
/// are additionally written to `packages/` and `fonts/` subdirectories. The
/// manifest itself is not recreated; it lives only inside the archive.
pub fn extract(
    pack: &Pack,
    dir: &Path,
    options: &ExtractOptions,
) -> Result<ExtractReport, ExtractError> {
    let plan = plan_pack_extraction(
        pack,
        PackExtractionSelection::new(options.packages, options.fonts),
    )?;
    preflight_destination(plan.entries(), dir, options.force)?;

    let mut report = ExtractReport::default();

    for entry in plan.entries() {
        write_file(
            dir,
            Path::new(entry.relative_path()),
            entry.bytes(),
            &mut report,
        )?;
    }

    Ok(report)
}

fn preflight_destination(
    entries: &[PackExtractionEntry],
    dir: &Path,
    force: bool,
) -> Result<(), ExtractError> {
    for entry in entries {
        let relative = Path::new(entry.relative_path());
        let target = dir.join(relative);
        match std::fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(ExtractError::DestinationConflict(target));
                }
                if metadata.is_file() {
                    if !force {
                        return Err(ExtractError::Exists(target));
                    }
                } else {
                    return Err(ExtractError::DestinationConflict(target));
                }
            }
            Err(source)
                if matches!(
                    source.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(source) => {
                return Err(ExtractError::Io {
                    path: target,
                    source,
                });
            }
        }

        let mut parent = target.parent();
        while let Some(path) = parent.filter(|path| path.starts_with(dir)) {
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(ExtractError::DestinationConflict(path.to_owned()));
                }
                Ok(_) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(ExtractError::Io {
                        path: path.to_owned(),
                        source,
                    });
                }
            }
            if path == dir {
                break;
            }
            parent = path.parent();
        }
    }
    Ok(())
}

fn write_file(
    dir: &Path,
    relative: &Path,
    data: &[u8],
    report: &mut ExtractReport,
) -> Result<(), ExtractError> {
    let target = dir.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ExtractError::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    std::fs::write(&target, data).map_err(|source| ExtractError::Io {
        path: target.clone(),
        source,
    })?;
    report.written.push(relative.to_owned());
    Ok(())
}
