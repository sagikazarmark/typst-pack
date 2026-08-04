//! Destination-independent Pack Extraction planning.

use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt;

use crate::PackIdentity;
use crate::pack::{Pack, font_container_path};
use crate::payload::SharedBytes;

/// The embedded dependency content selected for one Pack Extraction Plan.
///
/// Project files are always selected. Package Trees and Font Containers are
/// independent explicit choices.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PackExtractionSelection {
    packages: bool,
    fonts: bool,
}

impl PackExtractionSelection {
    /// Selects whether embedded Package Trees and Font Containers are included.
    pub const fn new(packages: bool, fonts: bool) -> Self {
        Self { packages, fonts }
    }

    /// Whether embedded Package Trees are included.
    pub const fn packages(self) -> bool {
        self.packages
    }

    /// Whether embedded Font Containers are included.
    pub const fn fonts(self) -> bool {
        self.fonts
    }
}

/// The semantic role of one Pack Extraction entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackExtractionEntryRole {
    /// A project file, included in every plan.
    ProjectFile,
    /// A file in an embedded Package Tree.
    PackageFile,
    /// One complete embedded Font Container.
    FontContainer,
}

impl fmt::Display for PackExtractionEntryRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProjectFile => "project file",
            Self::PackageFile => "package file",
            Self::FontContainer => "font container",
        })
    }
}

/// One canonical destination-relative entry in a Pack Extraction Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionEntry {
    relative_path: String,
    role: PackExtractionEntryRole,
    bytes: SharedBytes,
}

impl PackExtractionEntry {
    /// The canonical slash-separated path relative to a future destination.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The entry's role in the extracted project tree.
    pub fn role(&self) -> PackExtractionEntryRole {
        self.role
    }

    /// The exact payload length.
    pub fn len(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }

    /// Whether the exact payload contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The exact immutable payload bytes.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// An owned, immutable, destination-independent Pack Extraction Plan.
///
/// Entries are collision-checked and ordered by canonical relative path. The
/// plan contains no destination, platform path, conflict policy, or write state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionPlan {
    pack_identity: PackIdentity,
    selection: PackExtractionSelection,
    entries: Vec<PackExtractionEntry>,
}

impl PackExtractionPlan {
    /// The identity of the Pack projected by this plan.
    pub fn pack_identity(&self) -> &PackIdentity {
        &self.pack_identity
    }

    /// The embedded dependency choices used to construct this plan.
    pub fn selection(&self) -> PackExtractionSelection {
        self.selection
    }

    /// The canonical destination-relative entries in path order.
    pub fn entries(&self) -> &[PackExtractionEntry] {
        &self.entries
    }
}

/// One independently detectable issue in a Pack Extraction projection.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackExtractionPlanIssue {
    /// Two different semantic roles cannot coexist in one destination tree.
    #[error(
        "extraction path {first_path:?} ({first_role}) conflicts with {second_path:?} ({second_role})"
    )]
    PathConflict {
        first_path: String,
        first_role: PackExtractionEntryRole,
        second_path: String,
        second_role: PackExtractionEntryRole,
    },
}

impl PackExtractionPlanIssue {
    fn sort_key(&self) -> (u8, &str, u8, &str) {
        match self {
            Self::PathConflict {
                first_path,
                first_role,
                second_path,
                second_role,
            } => (
                role_index(*first_role),
                first_path,
                role_index(*second_role),
                second_path,
            ),
        }
    }
}

/// A failure while constructing a Pack Extraction Plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackExtractionPlanError {
    issues: Vec<PackExtractionPlanIssue>,
}

impl PackExtractionPlanError {
    /// Every independently detectable issue in role and canonical path order.
    pub fn issues(&self) -> &[PackExtractionPlanIssue] {
        &self.issues
    }
}

impl fmt::Display for PackExtractionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [issue] = self.issues.as_slice() {
            return issue.fmt(formatter);
        }
        write!(
            formatter,
            "Pack Extraction planning failed with {} issue(s)",
            self.issues.len()
        )?;
        for issue in &self.issues {
            write!(formatter, ": {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PackExtractionPlanError {}

/// Produces the complete semantic projection of one Pack before destination I/O.
pub fn plan_pack_extraction(
    pack: &Pack,
    selection: PackExtractionSelection,
) -> Result<PackExtractionPlan, PackExtractionPlanError> {
    let mut entries = BTreeMap::new();
    let mut issues = Vec::new();
    for (path, _) in pack.files() {
        add_entry(
            &mut entries,
            path.to_owned(),
            PackExtractionEntryRole::ProjectFile,
            pack.shared_file(path)
                .expect("a Pack project file has shared bytes")
                .clone(),
            &mut issues,
        );
    }

    if selection.packages() {
        for (spec, files) in pack.packages() {
            let base = format!("packages/{}/{}/{}", spec.namespace, spec.name, spec.version);
            for (path, _) in files {
                add_entry(
                    &mut entries,
                    format!("{base}/{path}"),
                    PackExtractionEntryRole::PackageFile,
                    pack.shared_package_file(spec, path)
                        .expect("an embedded Package Tree file has shared bytes")
                        .clone(),
                    &mut issues,
                );
            }
        }
    }

    if selection.fonts() {
        for font in pack.fonts() {
            add_entry(
                &mut entries,
                font_container_path(font.identity().container(), Some(font.data())),
                PackExtractionEntryRole::FontContainer,
                font.shared_data().clone(),
                &mut issues,
            );
        }
    }

    collect_tree_conflicts(&entries, &mut issues);
    if !issues.is_empty() {
        issues.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        return Err(PackExtractionPlanError { issues });
    }

    Ok(PackExtractionPlan {
        pack_identity: pack.identity(),
        selection,
        entries: entries.into_values().collect(),
    })
}

fn add_entry(
    entries: &mut BTreeMap<String, PackExtractionEntry>,
    relative_path: String,
    role: PackExtractionEntryRole,
    bytes: SharedBytes,
    issues: &mut Vec<PackExtractionPlanIssue>,
) {
    match entries.entry(relative_path) {
        Entry::Occupied(existing) if existing.get().role != role => {
            issues.push(PackExtractionPlanIssue::PathConflict {
                first_path: existing.key().clone(),
                first_role: existing.get().role,
                second_path: existing.key().clone(),
                second_role: role,
            });
        }
        Entry::Occupied(_) => {}
        Entry::Vacant(entry) => {
            let relative_path = entry.key().clone();
            entry.insert(PackExtractionEntry {
                relative_path,
                role,
                bytes,
            });
        }
    }
}

fn collect_tree_conflicts(
    entries: &BTreeMap<String, PackExtractionEntry>,
    issues: &mut Vec<PackExtractionPlanIssue>,
) {
    let mut ancestors = Vec::<(&str, PackExtractionEntryRole)>::new();

    for (relative_path, entry) in entries {
        while ancestors
            .last()
            .is_some_and(|(ancestor, _)| !is_ancestor(ancestor, relative_path))
        {
            ancestors.pop();
        }

        for (ancestor, ancestor_role) in ancestors.iter().filter(|(_, role)| *role != entry.role) {
            issues.push(PackExtractionPlanIssue::PathConflict {
                first_path: (*ancestor).to_owned(),
                first_role: *ancestor_role,
                second_path: relative_path.clone(),
                second_role: entry.role,
            });
        }

        ancestors.push((relative_path, entry.role));
    }
}

fn is_ancestor(ancestor: &str, descendant: &str) -> bool {
    descendant
        .strip_prefix(ancestor)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn role_index(role: PackExtractionEntryRole) -> u8 {
    match role {
        PackExtractionEntryRole::ProjectFile => 0,
        PackExtractionEntryRole::PackageFile => 1,
        PackExtractionEntryRole::FontContainer => 2,
    }
}
