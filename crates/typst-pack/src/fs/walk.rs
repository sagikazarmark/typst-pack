//! The directory walker shared by the project, package, and font readers.
//!
//! Traversal is handle-based: each directory is listed and its entries opened
//! through the `cap-std` handle of the directory that contains them. Symbolic
//! links are followed. A strict walk (projects and packages) accepts a link
//! only when its target resolves beneath the root, and opens that target
//! through the root handle; a lenient walk (fonts, which are host
//! configuration) follows links anywhere and records failing entries instead
//! of stopping. Directory cycles are detected by comparing canonical
//! directory paths on the current descent.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::{Dir, File};

use super::Error;
use crate::limits::{LimitExceeded, ReadLimits};

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
    pub(crate) path: PathBuf,
    canonical: PathBuf,
    dir: Dir,
}

impl Root {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        let io = |source| Error::Io {
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
    pub(crate) skipped: Vec<Error>,
}

/// One walk beneath a root: decides with `select`, from the root-relative
/// path and resolved kind, whether a directory is descended or a file is
/// read, and hands each read file to `take`.
struct Walk<'a, S, T> {
    walker: &'a mut Walker,
    root: &'a Root,
    /// The canonical directories on the current descent.
    stack: Vec<PathBuf>,
    select: S,
    take: T,
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
    ) -> Result<Option<Vec<u8>>, Error> {
        let file_type = match root.dir.symlink_metadata(name) {
            Ok(metadata) => shape(metadata.file_type()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(Error::Io {
                    path: root.path.join(name),
                    source,
                });
            }
        };
        let entry = classify(file_type, &root.canonical, OsStr::new(name));
        if entry.kind != Kind::File {
            return Err(Error::Unsupported {
                path: root.path.join(name),
            });
        }
        let mut bytes = None;
        let mut walk = Walk {
            walker: self,
            root,
            stack: vec![root.canonical.clone()],
            select: |_: &Path, _| Ok(true),
            take: |_, read| {
                bytes = Some(read);
                Ok(())
            },
        };
        walk.visit(&root.dir, OsStr::new(name), Path::new(name), entry)?;
        Ok(bytes)
    }

    /// Walks every entry beneath `root` in lexical order; see [`Walk`].
    pub(crate) fn walk(
        &mut self,
        root: &Root,
        select: impl FnMut(&Path, Kind) -> Result<bool, Error>,
        take: impl FnMut(PathBuf, Vec<u8>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut walk = Walk {
            walker: self,
            root,
            stack: vec![root.canonical.clone()],
            select,
            take,
        };
        walk.dir(&root.dir, Path::new(""))
    }

    fn read_bounded(&mut self, file: File, path: &Path) -> Result<Vec<u8>, Error> {
        let limits = self.limits;
        let allowance = limits
            .max_file_bytes
            .min(limits.max_total_bytes.saturating_sub(self.total_bytes));
        let mut bytes = Vec::new();
        file.take(allowance.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| Error::Io {
                path: path.to_owned(),
                source,
            })?;
        let length = bytes.len() as u64;
        let limit = |source| Error::Limit {
            path: path.to_owned(),
            source,
        };
        LimitExceeded::check(length, limits.max_file_bytes, "file bytes").map_err(limit)?;
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
    fn skip(&mut self, error: Error) -> Result<(), Error> {
        if self.lenient && !matches!(error, Error::Limit { .. }) {
            self.skipped.push(error);
            Ok(())
        } else {
            Err(error)
        }
    }
}

impl<S, T> Walk<'_, S, T>
where
    S: FnMut(&Path, Kind) -> Result<bool, Error>,
    T: FnMut(PathBuf, Vec<u8>) -> Result<(), Error>,
{
    fn dir(&mut self, dir: &Dir, relative: &Path) -> Result<(), Error> {
        let listed = match list(dir) {
            Ok(listed) => listed,
            Err(source) => {
                let path = self.root.path.join(relative);
                return self.walker.skip(Error::Io { path, source });
            }
        };
        for (name, file_type) in listed {
            let relative = relative.join(&name);
            let limits = self.walker.limits;
            LimitExceeded::add(&mut self.walker.entries, 1, limits.max_entries, "entries")
                .map_err(|source| Error::Limit {
                    path: self.root.path.join(&relative),
                    source,
                })?;
            let entry = match file_type {
                Ok(file_type) => classify(file_type, self.stack.last().expect("a root"), &name),
                Err(error) => Entry {
                    kind: Kind::File,
                    target: Target::Broken(error),
                },
            };
            if !(self.select)(&relative, entry.kind)? {
                continue;
            }
            if let Err(error) = self.visit(dir, &name, &relative, entry) {
                self.walker.skip(error)?;
            }
        }
        Ok(())
    }

    fn visit(
        &mut self,
        dir: &Dir,
        name: &OsStr,
        relative: &Path,
        entry: Entry,
    ) -> Result<(), Error> {
        let root = self.root;
        let path = root.path.join(relative);
        let (canonical, linked) = match entry.target {
            Target::Broken(source) => return Err(Error::Io { path, source }),
            Target::Child => (self.stack.last().expect("a root").join(name), false),
            Target::Link(target) => {
                if !self.walker.lenient && !target.starts_with(&root.canonical) {
                    return Err(Error::EscapingSymlink {
                        path,
                        root: root.path.clone(),
                    });
                }
                (target, true)
            }
        };
        let io = |source| Error::Io {
            path: path.clone(),
            source,
        };
        match entry.kind {
            Kind::Other => Err(Error::Unsupported { path }),
            Kind::Dir => {
                if self.stack.contains(&canonical) {
                    return Err(Error::SymlinkCycle { path });
                }
                let child = if !linked {
                    dir.open_dir(name)
                } else if let Ok(beneath) = canonical.strip_prefix(&root.canonical) {
                    root.dir.open_dir(beneath)
                } else {
                    Dir::open_ambient_dir(&canonical, ambient_authority())
                }
                .map_err(io)?;
                self.stack.push(canonical);
                let walked = self.dir(&child, relative);
                self.stack.pop();
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
                let bytes = self.walker.read_bounded(file, &path)?;
                (self.take)(relative.to_owned(), bytes)
            }
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

/// The `/`-separated form of a root-relative path, failing with the full
/// path when it is not UTF-8.
pub(crate) fn utf8_path(root: &Path, relative: &Path) -> Result<String, Error> {
    relative
        .iter()
        .map(OsStr::to_str)
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
        .ok_or_else(|| Error::NonUtf8Path {
            path: root.join(relative),
        })
}
