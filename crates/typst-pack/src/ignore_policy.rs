//! The root-scoped exclusion policy that decides project membership.

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::pack::names_pack_path;

/// The root-relative path of the root Project Ignore Policy file.
pub const IGNORE_FILE: &str = ".typkignore";

/// The root-scoped exclusion policy that decides project membership.
///
/// The policy is determined by ignore-file bytes alone and matches a path
/// without consulting a host, so the filesystem project gatherer can apply it
/// to a listing before reading content.
#[derive(Debug)]
pub struct ProjectIgnorePolicy {
    rules: Gitignore,
}

impl ProjectIgnorePolicy {
    /// The policy of a project without a root [`IGNORE_FILE`]: the built-in
    /// exclusion and nothing else.
    pub fn built_in() -> Self {
        Self {
            rules: Gitignore::empty(),
        }
    }

    /// Parses the root [`IGNORE_FILE`] bytes into a policy.
    pub fn from_ignore_file(bytes: &[u8]) -> Result<Self, ProjectIgnorePolicyError> {
        let contents = std::str::from_utf8(bytes).map_err(|_| ProjectIgnorePolicyError::NotUtf8)?;
        let mut builder = GitignoreBuilder::new(".");
        for (index, line) in contents.lines().enumerate() {
            // Match Git's handling of ignore files that begin with the Unicode BOM.
            let line = if index == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                line
            };
            builder.add_line(None, line).map_err(|error| {
                ProjectIgnorePolicyError::InvalidRule {
                    line: index + 1,
                    message: error.to_string(),
                }
            })?;
        }
        let rules = builder
            .build()
            .map_err(|error| ProjectIgnorePolicyError::Invalid(error.to_string()))?;
        Ok(Self { rules })
    }

    /// Whether the policy excludes the file at the given root-relative path.
    ///
    /// The path is matched as given, with `/` separators and no leading
    /// separator, so that the filesystem project gatherer can filter a listing
    /// before reading content. Project Snapshot assembly receives the entries
    /// already selected by that gatherer and does not apply this policy.
    pub fn excludes_file(&self, path: &str) -> bool {
        self.excludes(path, false)
    }

    /// Whether the policy excludes the directory at the given root-relative
    /// path, and with it every path beneath that directory.
    ///
    /// An adapter listing a hierarchy can stop descending here; one listing a
    /// flat set of paths does not need this, because [`Self::excludes_file`]
    /// answers for excluded ancestors too.
    pub fn excludes_directory(&self, path: &str) -> bool {
        self.excludes(path, true)
    }

    fn excludes(&self, path: &str, is_directory: bool) -> bool {
        if path == IGNORE_FILE {
            return false;
        }
        if names_pack_path(path) {
            return true;
        }
        // An excluded directory cannot be re-included from below, so ancestors
        // decide before the path itself does.
        let mut ancestor_end = 0;
        while let Some(offset) = path[ancestor_end..].find('/') {
            ancestor_end += offset;
            if self.rules.matched(&path[..ancestor_end], true).is_ignore() {
                return true;
            }
            ancestor_end += 1;
        }
        self.rules.matched(path, is_directory).is_ignore()
    }
}

/// A failure while parsing a Project Ignore Policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectIgnorePolicyError {
    /// The policy file is not valid UTF-8.
    #[error("invalid Project Ignore Policy: the policy file is not valid UTF-8")]
    NotUtf8,
    /// One policy rule cannot be parsed.
    #[error("invalid Project Ignore Policy: line {line}: {message}")]
    InvalidRule { line: usize, message: String },
    /// The parsed rules cannot be compiled into a matcher.
    #[error("invalid Project Ignore Policy: {0}")]
    Invalid(String),
}
