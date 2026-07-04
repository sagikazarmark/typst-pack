//! Extracting a pack back into a directory.

#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};

use crate::pack::Pack;

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
    #[error("`{0}` already exists (pass force to overwrite)")]
    Exists(PathBuf),
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
    let mut report = ExtractReport::default();

    for (path, data) in pack.files() {
        write_file(dir, Path::new(path), data, options, &mut report)?;
    }

    if options.packages {
        for (spec, files) in pack.packages() {
            let base = PathBuf::from("packages")
                .join(spec.namespace.as_str())
                .join(spec.name.as_str())
                .join(spec.version.to_string());
            for (path, data) in files {
                write_file(dir, &base.join(path), data, options, &mut report)?;
            }
        }
    }

    if options.fonts {
        for font in pack.fonts() {
            let path = Path::new(&font.entry.path);
            if report.written.iter().any(|written| written == path) {
                continue;
            }
            write_file(dir, path, &font.data, options, &mut report)?;
        }
    }

    Ok(report)
}

fn write_file(
    dir: &Path,
    relative: &Path,
    data: &[u8],
    options: &ExtractOptions,
    report: &mut ExtractReport,
) -> Result<(), ExtractError> {
    let target = dir.join(relative);
    if !options.force && target.exists() {
        return Err(ExtractError::Exists(target));
    }
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
