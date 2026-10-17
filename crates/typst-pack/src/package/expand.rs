//! Bounded expansion of the gzip-compressed tar archives the package
//! registry serves.

use std::io::Read;

use super::Tree;
use crate::payload::SharedBytes;
use crate::{LimitExceeded, PackError, ReadLimits};

/// The longest member path accepted, in bytes: Linux's `PATH_MAX`.
const MAX_PATH_BYTES: usize = 4096;

/// The largest GNU long-name or PAX extended-header record accepted, in
/// bytes. Such a record holds a path and a few attributes.
const MAX_EXTENSION_BYTES: u64 = 64 * 1024;

/// Expands the gzip-compressed tar the registry serves into a package tree.
///
/// Regular files become tree entries; directories and PAX global headers
/// (such as `git archive` writes) are skipped; links and every other member
/// kind are rejected, as are paths that cannot name a package file, contain
/// a `..` component, or are longer than 4096 bytes (a leading `./` is
/// accepted). `limits` bounds the member count, each file, and
/// the total expanded size, including the GNU long-name and PAX records that
/// describe members; the decompressed stream itself is bounded too, so a
/// compression bomb stops at the limit.
pub fn expand_archive(archive: &[u8], limits: ReadLimits) -> Result<Tree, ExpandError> {
    let malformed = |error: std::io::Error| ExpandError::Malformed(error.to_string());

    // Every tar record counts toward `max_entries` and its data toward
    // `max_total_bytes`, so the stream holds at most that data, one header
    // block and under one block of padding per record, and the block that
    // ends the archive.
    let stream_limit = limits
        .max_total_bytes
        .saturating_add(limits.max_entries.saturating_mul(1024))
        .saturating_add(512);
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive).take(stream_limit));
    let mut files = Vec::new();
    let mut entries = 0;
    let mut total = 0;
    // Records that describe the next member. They are read here, from raw
    // entries, rather than by the tar reader: it would read a GNU long-name or
    // PAX record whole into memory, whatever its declared size and before
    // yielding the member, outside every limit. Reading them by hand bounds
    // each record and charges it to `max_total_bytes`.
    let mut long_name = None;
    let mut extensions = None;
    for entry in tar.entries().map_err(malformed)?.raw(true) {
        let mut entry = entry.map_err(malformed)?;
        LimitExceeded::add(&mut entries, 1, limits.max_entries, "archive members")?;
        let size = entry.size();
        let kind = entry.header().entry_type();
        if kind.is_gnu_longname()
            || kind.is_gnu_longlink()
            || kind.is_pax_local_extensions()
            || kind.is_pax_global_extensions()
        {
            if size > MAX_EXTENSION_BYTES {
                return Err(ExpandError::Malformed(format!(
                    "a {size}-byte extension record exceeds the {MAX_EXTENSION_BYTES}-byte maximum"
                )));
            }
            LimitExceeded::add(&mut total, size, limits.max_total_bytes, "total bytes")?;
            // Global headers describe the archive, and long link names only
            // links, which are rejected below.
            let slot = if kind.is_gnu_longname() {
                &mut long_name
            } else if kind.is_pax_local_extensions() {
                &mut extensions
            } else {
                continue;
            };
            if slot.is_some() {
                return Err(ExpandError::Malformed(
                    "two extension records of one kind describe one member".to_owned(),
                ));
            }
            *slot = Some(read_record(&mut entry, size)?);
            continue;
        }

        let path = member_path(&entry, long_name.take(), extensions.take(), size)?;
        if kind.is_dir() {
            LimitExceeded::add(&mut total, size, limits.max_total_bytes, "total bytes")?;
            continue;
        }
        if !kind.is_file() {
            return Err(ExpandError::Malformed(format!(
                "member {path:?} is not a regular file or directory"
            )));
        }
        LimitExceeded::check(size, limits.max_file_bytes, "file bytes")?;
        LimitExceeded::add(&mut total, size, limits.max_total_bytes, "total bytes")?;
        let data = read_record(&mut entry, size)?;
        files.push((path, SharedBytes::new(data)));
    }
    if long_name.is_some() || extensions.is_some() {
        return Err(ExpandError::Malformed(
            "an extension record describes no member".to_owned(),
        ));
    }

    Ok(Tree::from_shared(files)?)
}

/// The path of a member: its GNU long name, else its PAX `path`, else the
/// name in its header, as the tar reader would choose. A PAX `size` must
/// agree with the header, so no two readers see different contents.
fn member_path(
    entry: &tar::Entry<'_, impl Read>,
    long_name: Option<Vec<u8>>,
    extensions: Option<Vec<u8>>,
    size: u64,
) -> Result<String, ExpandError> {
    let mut path = long_name.map(|mut name| {
        if name.last() == Some(&0) {
            name.pop();
        }
        name
    });
    for extension in tar::PaxExtensions::new(extensions.as_deref().unwrap_or_default()) {
        let extension = extension.map_err(|error| ExpandError::Malformed(error.to_string()))?;
        match extension.key_bytes() {
            b"path" => {
                path.get_or_insert_with(|| extension.value_bytes().to_vec());
            }
            b"size"
                if extension.value().ok().and_then(|value| value.parse().ok()) != Some(size) =>
            {
                return Err(ExpandError::Malformed(
                    "a member's extended header disagrees with its size".to_owned(),
                ));
            }
            _ => {}
        }
    }
    let path = path.unwrap_or_else(|| entry.path_bytes().into_owned());
    if path.len() > MAX_PATH_BYTES {
        return Err(ExpandError::Malformed(format!(
            "a {}-byte member name exceeds the {MAX_PATH_BYTES}-byte maximum",
            path.len()
        )));
    }
    let path = String::from_utf8(path).map_err(|error| {
        ExpandError::Malformed(format!(
            "member name {:?} is not valid UTF-8",
            error.as_bytes()
        ))
    })?;
    // Typst's own unpacking skips such members; normalizing `a/../b` to `b`
    // would read a different tree than Typst does.
    if path.split(['/', '\\']).any(|segment| segment == "..") {
        return Err(ExpandError::Malformed(format!(
            "member {path:?} has a `..` component"
        )));
    }
    Ok(path)
}

/// Reads a record's data, which must be complete.
fn read_record(entry: &mut impl Read, size: u64) -> Result<Vec<u8>, ExpandError> {
    let mut data = Vec::with_capacity(size.min(1 << 20) as usize);
    entry
        .read_to_end(&mut data)
        .map_err(|error| ExpandError::Malformed(error.to_string()))?;
    if data.len() as u64 != size {
        return Err(ExpandError::Malformed(
            "the archive ends inside a member".to_owned(),
        ));
    }
    Ok(data)
}

/// A package archive that could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExpandError {
    /// The archive exceeds a read limit.
    #[error("the package archive exceeded a limit")]
    Limit(#[from] LimitExceeded),
    /// The bytes are not a gzip-compressed tar of regular files.
    #[error("the package archive is malformed: {0}")]
    Malformed(String),
    /// The files do not form a valid package tree.
    #[error("the package archive does not hold a valid package tree")]
    Invalid(#[from] PackError),
}
