//! The directory walker shared by the filesystem project, package, and font
//! readers.
//!
//! Traversal is handle-based: each directory is listed and its entries opened
//! through the `cap-std` handle of the directory that contains them. Symbolic
//! links are followed. A strict walk (projects and packages) accepts a link
//! only when its target resolves beneath the root, and opens that target
//! through the root handle; a lenient walk (fonts, which are host
//! configuration) follows links anywhere and records failing entries instead
//! of stopping. Directory cycles are detected by comparing canonical
//! directory paths on the current descent.
//!
//! These checks are validation, not confinement against a concurrent local
//! peer that rewrites the tree while it is read.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::{Dir, File};

use crate::font_catalog::FontContainerError;
use crate::limits::{LimitExceeded, ReadLimits};
use crate::package_catalog::PackageTreeError;
use crate::project_snapshot::ProjectSnapshotError;

/// A failure while reading a project, package, or font directory.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilesystemReadError {
    #[error("cannot read `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("symbolic link `{}` resolves outside `{}`", path.display(), root.display())]
    EscapingSymlink { path: PathBuf, root: PathBuf },
    #[error("symbolic link `{}` leads back to a directory it is inside of", path.display())]
    SymlinkCycle { path: PathBuf },
    #[error(
        "unsupported filesystem entry `{}`: only regular files and directories can be read",
        path.display()
    )]
    Unsupported { path: PathBuf },
    #[error("path `{}` is not valid UTF-8", path.display())]
    NonUtf8Path { path: PathBuf },
    #[error("cannot read `{}`: {source}", path.display())]
    Limit {
        path: PathBuf,
        #[source]
        source: LimitExceeded,
    },
    #[error("invalid Project Ignore Policy `{}`: {message}", path.display())]
    InvalidIgnoreFile { path: PathBuf, message: String },
    #[error("invalid Font Container `{}`: {source}", path.display())]
    InvalidFont {
        path: PathBuf,
        #[source]
        source: FontContainerError,
    },
    #[error("read files do not form a Project Snapshot: {0}")]
    Snapshot(#[source] ProjectSnapshotError),
    #[error("read files do not form a Package Tree: {0}")]
    PackageTree(#[source] PackageTreeError),
}

/// The resolved kind of one directory entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    File,
    Dir,
    /// A FIFO, socket, device, or other non-regular entry.
    Other,
}

/// One opened traversal root.
pub(crate) struct Root {
    /// The root as the caller named it, used in messages.
    path: PathBuf,
    canonical: PathBuf,
    dir: Dir,
}

impl Root {
    pub(crate) fn open(path: &Path) -> Result<Self, FilesystemReadError> {
        let io = |source| FilesystemReadError::Io {
            path: path.to_owned(),
            source,
        };
        let canonical = std::fs::canonicalize(path).map_err(io)?;
        let dir = Dir::open_ambient_dir(&canonical, ambient_authority()).map_err(io)?;
        Ok(Self {
            path: path.to_owned(),
            canonical,
            dir,
        })
    }

    /// The root as the caller named it.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Where one entry's content lives.
enum Target {
    /// A real entry inside the directory being listed.
    Child,
    /// A symbolic link with this canonical target.
    Link(PathBuf),
    /// A symbolic link that cannot be resolved.
    Broken(io::Error),
}

struct Entry {
    kind: Kind,
    target: Target,
}

/// Bounded traversal over one or more roots.
pub(crate) struct Walker {
    limits: ReadLimits,
    lenient: bool,
    entries: u64,
    total_bytes: u64,
    /// Entries a lenient walk could not read.
    pub(crate) skipped: Vec<FilesystemReadError>,
}

impl Walker {
    /// A strict walker refuses escaping links and stops at the first failure.
    pub(crate) fn strict(limits: ReadLimits) -> Self {
        Self::new(limits, false)
    }

    /// A lenient walker follows links anywhere and records failing entries in
    /// [`Self::skipped`]. Exceeded limits still stop it.
    pub(crate) fn lenient(limits: ReadLimits) -> Self {
        Self::new(limits, true)
    }

    fn new(limits: ReadLimits, lenient: bool) -> Self {
        Self {
            limits,
            lenient,
            entries: 0,
            total_bytes: 0,
            skipped: Vec::new(),
        }
    }

    /// Reads one regular file directly beneath the root, if it exists.
    pub(crate) fn read_root_file(
        &mut self,
        root: &Root,
        name: &str,
    ) -> Result<Option<Vec<u8>>, FilesystemReadError> {
        let file_type = match root.dir.symlink_metadata(name) {
            Ok(metadata) => shape(metadata.file_type()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(FilesystemReadError::Io {
                    path: root.path.join(name),
                    source,
                });
            }
        };
        let entry = classify(file_type, &root.canonical, OsStr::new(name));
        if entry.kind != Kind::File {
            return Err(FilesystemReadError::Unsupported {
                path: root.path.join(name),
            });
        }
        let mut bytes = None;
        self.visit(
            root,
            &root.dir,
            OsStr::new(name),
            Path::new(name),
            entry,
            &mut vec![root.canonical.clone()],
            &mut |_, _| Ok(true),
            &mut |_, read| {
                bytes = Some(read);
                Ok(())
            },
        )?;
        Ok(bytes)
    }

    /// Walks every entry beneath `root` in lexical order.
    ///
    /// `select` decides, from the root-relative path and resolved kind,
    /// whether a directory is descended or a file is read; `take` receives each
    /// selected file's exact bytes.
    pub(crate) fn walk(
        &mut self,
        root: &Root,
        select: &mut impl FnMut(&Path, Kind) -> Result<bool, FilesystemReadError>,
        take: &mut impl FnMut(PathBuf, Vec<u8>) -> Result<(), FilesystemReadError>,
    ) -> Result<(), FilesystemReadError> {
        let mut stack = vec![root.canonical.clone()];
        self.walk_dir(root, &root.dir, Path::new(""), &mut stack, select, take)
    }

    fn walk_dir(
        &mut self,
        root: &Root,
        dir: &Dir,
        relative: &Path,
        stack: &mut Vec<PathBuf>,
        select: &mut impl FnMut(&Path, Kind) -> Result<bool, FilesystemReadError>,
        take: &mut impl FnMut(PathBuf, Vec<u8>) -> Result<(), FilesystemReadError>,
    ) -> Result<(), FilesystemReadError> {
        let listed = list(dir).map_err(|source| FilesystemReadError::Io {
            path: root.path.join(relative),
            source,
        });
        let listed = match listed {
            Ok(listed) => listed,
            Err(error) => return self.skip(error),
        };
        for (name, file_type) in listed {
            let relative = relative.join(&name);
            LimitExceeded::add(&mut self.entries, 1, self.limits.max_entries, "entries").map_err(
                |source| FilesystemReadError::Limit {
                    path: root.path.join(&relative),
                    source,
                },
            )?;
            let entry = match file_type {
                Ok(file_type) => classify(file_type, stack.last().expect("a root"), &name),
                Err(error) => Entry {
                    kind: Kind::File,
                    target: Target::Broken(error),
                },
            };
            if !select(&relative, entry.kind)? {
                continue;
            }
            if let Err(error) = self.visit(root, dir, &name, &relative, entry, stack, select, take)
            {
                self.skip(error)?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        root: &Root,
        dir: &Dir,
        name: &OsStr,
        relative: &Path,
        entry: Entry,
        stack: &mut Vec<PathBuf>,
        select: &mut impl FnMut(&Path, Kind) -> Result<bool, FilesystemReadError>,
        take: &mut impl FnMut(PathBuf, Vec<u8>) -> Result<(), FilesystemReadError>,
    ) -> Result<(), FilesystemReadError> {
        let path = root.path.join(relative);
        let (canonical, linked) = match entry.target {
            Target::Broken(source) => return Err(FilesystemReadError::Io { path, source }),
            Target::Child => (stack.last().expect("a root").join(name), false),
            Target::Link(target) => {
                if !self.lenient && !target.starts_with(&root.canonical) {
                    return Err(FilesystemReadError::EscapingSymlink {
                        path,
                        root: root.path.clone(),
                    });
                }
                (target, true)
            }
        };
        let io = |source| FilesystemReadError::Io {
            path: path.clone(),
            source,
        };
        match entry.kind {
            Kind::Other => Err(FilesystemReadError::Unsupported { path }),
            Kind::Dir => {
                if stack.contains(&canonical) {
                    return Err(FilesystemReadError::SymlinkCycle { path });
                }
                let child = if !linked {
                    dir.open_dir(name)
                } else if let Ok(beneath) = canonical.strip_prefix(&root.canonical) {
                    root.dir.open_dir(beneath)
                } else {
                    Dir::open_ambient_dir(&canonical, ambient_authority())
                }
                .map_err(io)?;
                stack.push(canonical);
                let walked = self.walk_dir(root, &child, relative, stack, select, take);
                stack.pop();
                walked
            }
            Kind::File => {
                let file = if !linked {
                    dir.open(name)
                } else if let Ok(beneath) = canonical.strip_prefix(&root.canonical) {
                    root.dir.open(beneath)
                } else {
                    File::open_ambient(&canonical, ambient_authority())
                }
                .map_err(io)?;
                let bytes = self.read_bounded(file, &path)?;
                take(relative.to_owned(), bytes)
            }
        }
    }

    fn read_bounded(&mut self, file: File, path: &Path) -> Result<Vec<u8>, FilesystemReadError> {
        let limits = self.limits;
        let allowance = limits
            .max_file_bytes
            .min(limits.max_total_bytes.saturating_sub(self.total_bytes));
        let mut bytes = Vec::new();
        file.take(allowance.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| FilesystemReadError::Io {
                path: path.to_owned(),
                source,
            })?;
        let length = bytes.len() as u64;
        let limit = |source| FilesystemReadError::Limit {
            path: path.to_owned(),
            source,
        };
        if length > limits.max_file_bytes {
            return Err(limit(LimitExceeded {
                resource: "file bytes",
                limit: limits.max_file_bytes,
            }));
        }
        LimitExceeded::add(
            &mut self.total_bytes,
            length,
            limits.max_total_bytes,
            "total bytes",
        )
        .map_err(limit)?;
        Ok(bytes)
    }

    /// Records a failing entry in a lenient walk; anything else stops.
    fn skip(&mut self, error: FilesystemReadError) -> Result<(), FilesystemReadError> {
        if self.lenient && !matches!(error, FilesystemReadError::Limit { .. }) {
            self.skipped.push(error);
            Ok(())
        } else {
            Err(error)
        }
    }
}

/// An entry's own kind, and whether it is a symbolic link.
type Shape = (Kind, bool);

/// Lists one directory sorted by name.
fn list(dir: &Dir) -> io::Result<Vec<(OsString, io::Result<Shape>)>> {
    let mut listed = dir
        .entries()?
        .map(|entry| entry.map(|entry| (entry.file_name(), entry.file_type().map(shape))))
        .collect::<io::Result<Vec<_>>>()?;
    listed.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(listed)
}

fn shape(file_type: cap_std::fs::FileType) -> Shape {
    let kind = if file_type.is_dir() {
        Kind::Dir
    } else if file_type.is_file() {
        Kind::File
    } else {
        Kind::Other
    };
    (kind, file_type.is_symlink())
}

/// Classifies an entry, resolving a symbolic link to its canonical target.
fn classify((kind, symlink): Shape, directory: &Path, name: &OsStr) -> Entry {
    if !symlink {
        return Entry {
            kind,
            target: Target::Child,
        };
    }
    match std::fs::canonicalize(directory.join(name))
        .and_then(|target| std::fs::metadata(&target).map(|metadata| (target, metadata)))
    {
        Ok((target, metadata)) => Entry {
            kind: if metadata.is_dir() {
                Kind::Dir
            } else if metadata.is_file() {
                Kind::File
            } else {
                Kind::Other
            },
            target: Target::Link(target),
        },
        Err(error) => Entry {
            kind: Kind::File,
            target: Target::Broken(error),
        },
    }
}

/// The `/`-separated form of a root-relative path, if it is valid UTF-8.
pub(crate) fn slash_path(relative: &Path) -> Option<String> {
    relative
        .iter()
        .map(OsStr::to_str)
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
}

/// Like [`slash_path`], failing with the full path when it is not UTF-8.
pub(crate) fn utf8_path(root: &Path, relative: &Path) -> Result<String, FilesystemReadError> {
    slash_path(relative).ok_or_else(|| FilesystemReadError::NonUtf8Path {
        path: root.join(relative),
    })
}
