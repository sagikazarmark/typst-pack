//! Extracting a pack back into a directory.

#![cfg(feature = "fs")]

use std::collections::{BTreeMap, btree_map::Entry};
use std::path::{Path, PathBuf};

use crate::pack::{Pack, PackPathRole};

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
    /// Resource Slot paths omitted from the extracted project.
    pub resource_slots: Vec<PathBuf>,
}

/// A failure while extracting a pack.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(
        "extraction path `{first_path}` ({first_role}) conflicts with `{second_path}` ({second_role})"
    )]
    PlannedPathConflict {
        first_path: PathBuf,
        first_role: PackPathRole,
        second_path: PathBuf,
        second_role: PackPathRole,
    },
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
    let mut plan = BTreeMap::new();
    for (path, data) in pack.files() {
        add_to_plan(
            &mut plan,
            PathBuf::from(path),
            PackPathRole::ProjectFile,
            Some(data.as_slice()),
        )?;
    }

    if options.packages {
        for (spec, files) in pack.packages() {
            let base = PathBuf::from("packages")
                .join(spec.namespace.as_str())
                .join(spec.name.as_str())
                .join(spec.version.to_string());
            for (path, data) in files {
                add_to_plan(
                    &mut plan,
                    base.join(path),
                    PackPathRole::PackageFile,
                    Some(data.as_slice()),
                )?;
            }
        }
    }

    if options.fonts {
        for font in pack.fonts() {
            add_to_plan(
                &mut plan,
                PathBuf::from(font.manifest().path()),
                PackPathRole::FontData,
                Some(font.data().as_slice()),
            )?;
        }
    }

    for path in pack.resource_slots() {
        add_to_plan(
            &mut plan,
            PathBuf::from(path),
            PackPathRole::ResourceSlot,
            None,
        )?;
    }
    validate_plan(&plan)?;
    preflight_destination(&plan, dir, options.force)?;

    let mut report = ExtractReport {
        resource_slots: pack.resource_slots().map(PathBuf::from).collect(),
        ..ExtractReport::default()
    };

    for (relative, planned) in plan {
        if let Some(data) = planned.data {
            write_file(dir, &relative, data, &mut report)?;
        }
    }

    Ok(report)
}

struct PlannedPath<'a> {
    role: PackPathRole,
    data: Option<&'a [u8]>,
}

fn add_to_plan<'a>(
    plan: &mut BTreeMap<PathBuf, PlannedPath<'a>>,
    relative: PathBuf,
    role: PackPathRole,
    data: Option<&'a [u8]>,
) -> Result<(), ExtractError> {
    match plan.entry(relative) {
        Entry::Occupied(existing) => {
            if existing.get().role != role {
                return Err(ExtractError::PlannedPathConflict {
                    first_path: existing.key().clone(),
                    first_role: existing.get().role,
                    second_path: existing.key().clone(),
                    second_role: role,
                });
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(PlannedPath { role, data });
        }
    }
    Ok(())
}

fn validate_plan(plan: &BTreeMap<PathBuf, PlannedPath<'_>>) -> Result<(), ExtractError> {
    let mut ancestors = Vec::<(&Path, PackPathRole)>::new();
    let mut role_counts = [0usize; 6];

    for (relative, planned) in plan {
        while ancestors
            .last()
            .is_some_and(|(ancestor, _)| !relative.starts_with(ancestor))
        {
            let (_, role) = ancestors.pop().expect("an ancestor was present");
            role_counts[role_index(role)] -= 1;
        }

        if ancestors.len() != role_counts[role_index(planned.role)] {
            let (ancestor, ancestor_role) = ancestors
                .iter()
                .rev()
                .find(|(_, role)| *role != planned.role)
                .expect("a conflicting ancestor role was counted");
            return Err(ExtractError::PlannedPathConflict {
                first_path: ancestor.to_path_buf(),
                first_role: *ancestor_role,
                second_path: relative.clone(),
                second_role: planned.role,
            });
        }

        ancestors.push((relative, planned.role));
        role_counts[role_index(planned.role)] += 1;
    }
    Ok(())
}

fn role_index(role: PackPathRole) -> usize {
    match role {
        PackPathRole::PackManifest => 0,
        PackPathRole::Entrypoint => 1,
        PackPathRole::ProjectFile => 2,
        PackPathRole::ResourceSlot => 3,
        PackPathRole::PackageFile => 4,
        PackPathRole::FontData => 5,
    }
}

fn preflight_destination(
    plan: &BTreeMap<PathBuf, PlannedPath<'_>>,
    dir: &Path,
    force: bool,
) -> Result<(), ExtractError> {
    for (relative, planned) in plan {
        if planned.data.is_none() {
            continue;
        }

        let target = dir.join(relative);
        if target.exists() {
            if target.is_file() {
                if !force {
                    return Err(ExtractError::Exists(target));
                }
            } else {
                return Err(ExtractError::DestinationConflict(target));
            }
        }

        let mut parent = target.parent();
        while let Some(path) = parent {
            if path.exists() {
                if !path.is_dir() {
                    return Err(ExtractError::DestinationConflict(path.to_owned()));
                }
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
