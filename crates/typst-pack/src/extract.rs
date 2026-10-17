//! Planning the extraction of a pack into a directory tree.
//!
//! A [`Plan`] lists every file an extraction writes, with destination-relative
//! paths and exact bytes, before any destination is touched; the filesystem
//! and OpenDAL adapters write it.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as Slot;
use std::fmt;

use crate::archive::font_member_name;
use crate::pack::package_member_prefix;
use crate::payload::SharedBytes;
use crate::{Pack, list};

/// The embedded dependencies an extraction includes besides the project
/// files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    /// Embedded package trees, under `packages/<namespace>/<name>/<version>/`.
    pub packages: bool,
    /// Embedded font containers, under `fonts/`.
    pub fonts: bool,
}

/// What one extracted file holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EntryKind {
    /// A project file, at its root-relative path.
    ProjectFile,
    /// A file of an embedded package, under
    /// `packages/<namespace>/<name>/<version>/`.
    PackageFile,
    /// An embedded font container, under `fonts/`.
    FontContainer,
}

impl fmt::Display for EntryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProjectFile => "project file",
            Self::PackageFile => "package file",
            Self::FontContainer => "font container",
        })
    }
}

/// One file of a [`Plan`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    path: String,
    kind: EntryKind,
    bytes: SharedBytes,
}

impl Entry {
    /// The canonical slash-separated path relative to the destination.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What the file holds.
    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    /// The exact file bytes.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// Every file one extraction of a pack writes, ordered by path and free of
/// conflicting paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    entries: Vec<Entry>,
}

impl Plan {
    /// The files in path order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

/// Two extracted files that cannot coexist: the same path, or a file where
/// another needs a directory.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("extraction path {path:?} ({kind}) conflicts with {other_path:?} ({other_kind})")]
pub struct Conflict {
    /// The first path, in path order.
    pub path: String,
    /// What the first path holds.
    pub kind: EntryKind,
    /// The conflicting path.
    pub other_path: String,
    /// What the conflicting path holds.
    pub other_kind: EntryKind,
}

/// A pack whose selected content cannot be extracted into one directory
/// tree.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{}", list("the pack cannot be extracted", .issues))]
pub struct Error {
    /// Every conflict, in path order.
    pub issues: Vec<Conflict>,
}

/// Plans the extraction of a pack's project files and the selected embedded
/// dependencies.
pub fn plan(pack: &Pack, selection: Selection) -> Result<Plan, Error> {
    let mut entries = BTreeMap::new();
    let mut issues = Vec::new();
    for (path, _) in pack.files() {
        let bytes = pack.project().shared_file(path).expect("a project file");
        add(
            &mut entries,
            path.to_owned(),
            EntryKind::ProjectFile,
            bytes,
            &mut issues,
        );
    }
    if selection.packages {
        for (spec, tree) in pack.packages() {
            let prefix = package_member_prefix(spec);
            for (path, bytes) in tree.shared_files() {
                add(
                    &mut entries,
                    format!("{prefix}{path}"),
                    EntryKind::PackageFile,
                    bytes,
                    &mut issues,
                );
            }
        }
    }
    if selection.fonts {
        for requirement in pack.font_requirements() {
            if let Some(data) = pack.shared_font_data(&requirement.digest) {
                let path = font_member_name(&requirement.digest, data);
                add(
                    &mut entries,
                    path,
                    EntryKind::FontContainer,
                    data,
                    &mut issues,
                );
            }
        }
    }

    // A file conflicts with every descendant path of another kind.
    let mut ancestors = Vec::<&Entry>::new();
    for (path, entry) in &entries {
        while ancestors.last().is_some_and(|ancestor| {
            !path
                .strip_prefix(ancestor.path.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
        }) {
            ancestors.pop();
        }
        for ancestor in ancestors
            .iter()
            .filter(|ancestor| ancestor.kind != entry.kind)
        {
            issues.push(Conflict {
                path: ancestor.path.clone(),
                kind: ancestor.kind,
                other_path: path.clone(),
                other_kind: entry.kind,
            });
        }
        ancestors.push(entry);
    }

    if !issues.is_empty() {
        issues.sort_by(|left, right| {
            (left.kind, &left.path, left.other_kind, &left.other_path).cmp(&(
                right.kind,
                &right.path,
                right.other_kind,
                &right.other_path,
            ))
        });
        return Err(Error { issues });
    }
    Ok(Plan {
        entries: entries.into_values().collect(),
    })
}

fn add(
    entries: &mut BTreeMap<String, Entry>,
    path: String,
    kind: EntryKind,
    bytes: &SharedBytes,
    issues: &mut Vec<Conflict>,
) {
    match entries.entry(path) {
        Slot::Occupied(existing) if existing.get().kind != kind => issues.push(Conflict {
            path: existing.key().clone(),
            kind: existing.get().kind,
            other_path: existing.key().clone(),
            other_kind: kind,
        }),
        Slot::Occupied(_) => {}
        Slot::Vacant(slot) => {
            let path = slot.key().clone();
            slot.insert(Entry {
                path,
                kind,
                bytes: bytes.clone(),
            });
        }
    }
}
