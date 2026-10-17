//! Project reading for the reference filesystem source.

use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::fs_read::{FilesystemReadError, Kind, Root, Walker, utf8_path};
use crate::limits::ReadLimits;
use crate::pack::names_pack_path;
use crate::project_snapshot::{ProjectSnapshot, ProjectSnapshotAssembly};

/// The root-relative path of the filesystem Project Ignore Policy file.
pub const IGNORE_FILE: &str = ".typkignore";

/// Reads one Project Snapshot from a project directory.
///
/// The root [`IGNORE_FILE`] is read first and applied with gitignore syntax;
/// its bytes join the snapshot. Excluded directories are not descended.
/// Symbolic links are followed when they resolve beneath `root`; an escaping
/// link, a FIFO, socket, or device that is not excluded fails the read.
pub fn read_filesystem_project(
    root: impl AsRef<Path>,
    entrypoint: impl Into<String>,
    limits: ReadLimits,
) -> Result<ProjectSnapshot, FilesystemReadError> {
    let root_path = root.as_ref();
    let root = Root::open(root_path)?;
    let mut walker = Walker::strict(limits);
    let mut entries = Vec::new();

    let policy = match walker.read_root_file(&root, IGNORE_FILE)? {
        Some(bytes) => {
            let policy = IgnorePolicy::parse(&bytes).map_err(|message| {
                FilesystemReadError::InvalidIgnoreFile {
                    path: root_path.join(IGNORE_FILE),
                    message,
                }
            })?;
            entries.push((IGNORE_FILE.to_owned(), bytes));
            policy
        }
        None => IgnorePolicy::default(),
    };

    walker.walk(
        &root,
        &mut |relative, kind| {
            let path = utf8_path(root_path, relative)?;
            Ok(path != IGNORE_FILE && !policy.excludes(&path, kind == Kind::Dir))
        },
        &mut |relative, bytes| {
            entries.push((utf8_path(root_path, &relative)?, bytes));
            Ok(())
        },
    )?;

    ProjectSnapshotAssembly::new(entrypoint)
        .assemble(entries)
        .map_err(FilesystemReadError::Snapshot)
}

/// The parsed root ignore file.
struct IgnorePolicy {
    rules: Gitignore,
}

impl Default for IgnorePolicy {
    fn default() -> Self {
        Self {
            rules: Gitignore::empty(),
        }
    }
}

impl IgnorePolicy {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let contents =
            std::str::from_utf8(bytes).map_err(|_| "the file is not valid UTF-8".to_owned())?;
        let mut builder = GitignoreBuilder::new(".");
        for (index, line) in contents.lines().enumerate() {
            let line = if index == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                line
            };
            builder
                .add_line(None, line)
                .map_err(|error| format!("line {}: {error}", index + 1))?;
        }
        let rules = builder.build().map_err(|error| error.to_string())?;
        Ok(Self { rules })
    }

    /// Whether a root-relative path is excluded. A path naming a Pack is
    /// always excluded; any excluded ancestor excludes its descendants.
    fn excludes(&self, path: &str, is_directory: bool) -> bool {
        if names_pack_path(path) {
            return true;
        }
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
