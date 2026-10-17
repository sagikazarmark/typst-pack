//! Canonical portable relative paths shared by the content constructors.

use std::borrow::Borrow;

use typst::syntax::VirtualPath;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalPath(String);

impl CanonicalPath {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl Borrow<str> for CanonicalPath {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for CanonicalPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Canonicalizes a root-relative path, or explains why it cannot name a
/// portable file.
pub(crate) fn canonical_relative_path(path: &str) -> Result<CanonicalPath, &'static str> {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err("path must name a root-relative file");
    }
    if path.contains('\\') {
        return Err("backslashes are not portable path separators");
    }
    if path.contains('\0') {
        return Err("path must not contain NUL bytes");
    }
    if has_windows_drive_prefix(path) {
        return Err("path must not contain a platform root prefix");
    }
    let virtual_path =
        VirtualPath::new(path).map_err(|_| "path cannot be represented canonically")?;
    let canonical = virtual_path.get_without_slash();
    if canonical.is_empty() {
        return Err("path must name a file");
    }
    if has_windows_drive_prefix(canonical) {
        return Err("path must not contain a platform root prefix");
    }
    Ok(CanonicalPath(canonical.to_owned()))
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Every pair of paths where the first is a directory ancestor of the second.
pub(crate) fn path_tree_conflicts<'a>(
    paths: impl IntoIterator<Item = &'a CanonicalPath>,
) -> Vec<(&'a CanonicalPath, &'a CanonicalPath)> {
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort();
    let mut conflicts = Vec::new();
    for ancestor in &paths {
        let prefix = format!("{ancestor}/");
        for descendant in paths
            .iter()
            .skip(paths.partition_point(|path| path.as_str() < prefix.as_str()))
            .take_while(|path| path.as_str().starts_with(&prefix))
        {
            conflicts.push((*ancestor, *descendant));
        }
    }
    conflicts
}
