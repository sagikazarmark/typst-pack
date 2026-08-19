//! Canonical portable relative paths shared by domain constructors.

use std::borrow::Borrow;

use typst::syntax::VirtualPath;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalPath(String);

impl CanonicalPath {
    pub(crate) fn from_canonical(path: String) -> Self {
        Self(path)
    }

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

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub(crate) struct CanonicalPathError {
    message: &'static str,
}

impl CanonicalPathError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

pub(crate) fn canonical_relative_path(path: &str) -> Result<CanonicalPath, CanonicalPathError> {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err(CanonicalPathError::new(
            "path must name a root-relative file",
        ));
    }
    if path.contains('\\') {
        return Err(CanonicalPathError::new(
            "backslashes are not portable path separators",
        ));
    }
    if path.contains('\0') {
        return Err(CanonicalPathError::new("path must not contain NUL bytes"));
    }
    if has_windows_drive_prefix(path) {
        return Err(CanonicalPathError::new(
            "path must not contain a platform root prefix",
        ));
    }
    let virtual_path = VirtualPath::new(path)
        .map_err(|_| CanonicalPathError::new("path cannot be represented canonically"))?;
    let canonical = virtual_path.get_without_slash();
    if canonical.is_empty() {
        return Err(CanonicalPathError::new("path must name a file"));
    }
    if has_windows_drive_prefix(canonical) {
        return Err(CanonicalPathError::new(
            "path must not contain a platform root prefix",
        ));
    }
    Ok(CanonicalPath(canonical.to_owned()))
}

pub(crate) fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathTreeConflict<'a, R> {
    pub(crate) ancestor: &'a CanonicalPath,
    pub(crate) ancestor_role: R,
    pub(crate) descendant: &'a CanonicalPath,
    pub(crate) descendant_role: R,
}

pub(crate) fn path_tree_conflicts<'a, R: Copy>(
    paths: impl IntoIterator<Item = (&'a CanonicalPath, R)>,
) -> Vec<PathTreeConflict<'a, R>> {
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort_by_key(|(path, _)| *path);
    let mut conflicts = Vec::new();
    for (ancestor, ancestor_role) in &paths {
        let prefix = format!("{ancestor}/");
        for (descendant, descendant_role) in paths
            .iter()
            .skip(paths.partition_point(|(path, _)| path.as_str() < prefix.as_str()))
            .take_while(|(path, _)| path.as_str().starts_with(&prefix))
        {
            conflicts.push(PathTreeConflict {
                ancestor,
                ancestor_role: *ancestor_role,
                descendant,
                descendant_role: *descendant_role,
            });
        }
    }
    conflicts
}
