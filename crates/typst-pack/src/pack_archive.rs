//! The pack archive: a ZIP file holding a `typst-pack.toml` manifest and the
//! pack's content.
//!
//! See the README's "Pack format" section for the layout and verification
//! rules.

mod read;
mod write;

pub use read::*;
pub use write::*;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};

use typst::syntax::package::PackageSpec;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub use crate::manifest::{FORMAT_VERSION, MANIFEST_PATH, ManifestError};
use crate::manifest::{FileEntry, FontEntry, Manifest, PackageEntry};
use crate::pack::{FontInput, PackInput, PackInvariantError, PackageContent, PackageInput};
use crate::paths::canonical_relative_path;
use crate::payload::SharedBytes;
use crate::{Digest, LimitExceeded, Pack};

const PROJECT_PREFIX: &str = "project/";
const PACKAGES_PREFIX: &str = "packages/";
const FONTS_PREFIX: &str = "fonts/";

/// The archive member name of an embedded font container.
pub(crate) fn font_member_name(digest: &Digest, data: &[u8]) -> String {
    let extension = match data.get(..4) {
        Some(b"OTTO") => "otf",
        Some(b"ttcf") => "ttc",
        Some(b"wOFF") => "woff",
        Some(b"wOF2") => "woff2",
        _ => "ttf",
    };
    format!("{FONTS_PREFIX}{}.{extension}", digest.to_hex())
}

fn package_member_prefix(spec: &PackageSpec) -> String {
    format!(
        "{PACKAGES_PREFIX}{}/{}/{}/",
        spec.namespace, spec.name, spec.version
    )
}

/// A Pack that cannot be represented as a ZIP archive.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    #[error("archive member name {0:?} exceeds the 65535-byte ZIP limit")]
    MemberNameTooLong(String),
    #[error("failed to write ZIP structure: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Encodes a [`Pack`] as archive bytes.
///
/// The output is deterministic for a given build: members are written in a
/// fixed order with a fixed timestamp (1980-01-01), permissions, and
/// compression level.
pub fn encode(pack: &Pack) -> Result<Vec<u8>, EncodeError> {
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        typst_version: env!("TYPST_PACK_ENGINE_VERSION").to_owned(),
        generator: concat!("typst-pack ", env!("CARGO_PKG_VERSION")).to_owned(),
        entrypoint: pack.entrypoint().to_owned(),
        files: pack
            .file_digests()
            .map(|(path, digest)| FileEntry {
                path: path.to_owned(),
                digest,
            })
            .collect(),
        packages: pack
            .package_requirements()
            .iter()
            .map(|requirement| PackageEntry {
                spec: requirement.spec.clone(),
                digest: requirement.digest,
                embedded: requirement.embedded,
            })
            .collect(),
        fonts: pack
            .font_requirements()
            .iter()
            .map(|requirement| FontEntry {
                digest: requirement.digest,
                embedded: requirement.embedded,
                faces: requirement.faces.clone(),
            })
            .collect(),
        metadata: pack.metadata().cloned(),
    };

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    add_member(
        &mut zip,
        MANIFEST_PATH.to_owned(),
        manifest.to_toml().as_bytes(),
    )?;
    for (path, data) in pack.files() {
        add_member(&mut zip, format!("{PROJECT_PREFIX}{path}"), data)?;
    }
    for (spec, files) in pack.packages() {
        let prefix = package_member_prefix(spec);
        for (path, data) in files {
            add_member(&mut zip, format!("{prefix}{path}"), data)?;
        }
    }
    for requirement in pack.font_requirements() {
        if let Some(data) = pack.font_data(&requirement.digest) {
            add_member(&mut zip, font_member_name(&requirement.digest, data), data)?;
        }
    }
    Ok(zip.finish()?.into_inner())
}

fn add_member(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: String,
    data: &[u8],
) -> Result<(), EncodeError> {
    if name.len() > usize::from(u16::MAX) {
        return Err(EncodeError::MemberNameTooLong(name));
    }
    // Deflate may expand incompressible input; nine bits per byte plus framing
    // bounds the compressed size.
    let bound = data.len() as u64 + (data.len() as u64).div_ceil(8) + 16;
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::DEFAULT)
        .system(zip::System::Unix)
        .unix_permissions(0o644)
        .large_file(bound > zip::ZIP64_BYTES_THR);
    zip.start_file(name, options)?;
    zip.write_all(data).map_err(zip::result::ZipError::Io)?;
    Ok(())
}

/// Ceilings for decoding an untrusted pack archive.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DecodeLimits {
    /// Maximum size of the archive itself.
    pub max_archive_bytes: u64,
    /// Maximum number of archive members.
    pub max_members: u64,
    /// Maximum decompressed size of one member.
    pub max_member_bytes: u64,
    /// Maximum combined decompressed size of all members.
    pub max_total_bytes: u64,
}

impl DecodeLimits {
    /// Ceilings suitable for ordinary packs.
    pub const DEFAULT: Self = Self {
        max_archive_bytes: 512 * 1024 * 1024,
        max_members: 100_000,
        max_member_bytes: 256 * 1024 * 1024,
        max_total_bytes: 2 * 1024 * 1024 * 1024,
    };
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A failure while decoding archive bytes into a [`Pack`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    #[error(transparent)]
    Limit(#[from] LimitExceeded),
    #[error("not a valid ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive member {name:?} could not be read: {source}")]
    UnreadableMember {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("archive member {0:?} has an unsafe name")]
    UnsafeMemberName(String),
    #[error("archive member {0:?} appears more than once")]
    DuplicateMember(String),
    #[error("archive member {0:?} is not a regular file or directory")]
    UnsupportedMemberKind(String),
    #[error("the archive contains no {MANIFEST_PATH} manifest (is this a Typst Pack?)")]
    MissingManifest,
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("archive member {0:?} is not declared by the manifest")]
    UndeclaredMember(String),
    #[error("{0} is declared by the manifest but missing from the archive")]
    MissingMember(String),
    #[error("{name} does not match its declared digest {declared} (actual {actual})")]
    DigestMismatch {
        name: String,
        declared: Digest,
        actual: Digest,
    },
    #[error(transparent)]
    InvalidPack(#[from] PackInvariantError),
}

/// Decodes archive bytes into a [`Pack`], verifying every member against the
/// digest the manifest declares for it.
pub fn decode(archive: &[u8], limits: DecodeLimits) -> Result<Pack, DecodeError> {
    LimitExceeded::check(
        archive.len() as u64,
        limits.max_archive_bytes,
        "archive bytes",
    )?;
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
    let raw_names = raw_member_names(archive, zip.central_directory_start());
    LimitExceeded::check(
        raw_names.len().max(zip.len()) as u64,
        limits.max_members,
        "archive members",
    )?;
    if raw_names.len() != zip.len() {
        let mut seen = BTreeSet::new();
        let duplicate = raw_names.iter().find(|name| !seen.insert(**name));
        return Err(DecodeError::DuplicateMember(
            String::from_utf8_lossy(duplicate.copied().unwrap_or_default()).into_owned(),
        ));
    }

    // Classify every member by canonical name before reading any content.
    let mut members = BTreeMap::new();
    let mut declared_total = 0;
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index)?;
        let name = entry.name().to_owned();
        let mode = entry.unix_mode().map(|mode| mode & 0o170000);
        let directory = entry.is_dir();
        let canonical = canonical_relative_path(name.strip_suffix('/').unwrap_or(&name))
            .map_err(|_| DecodeError::UnsafeMemberName(name.clone()))?
            .into_string();
        match (directory, mode) {
            (true, None | Some(0 | 0o040000)) => continue,
            (false, None | Some(0 | 0o100000)) => {}
            _ => return Err(DecodeError::UnsupportedMemberKind(name)),
        }
        LimitExceeded::check(entry.size(), limits.max_member_bytes, "member bytes")?;
        LimitExceeded::add(
            &mut declared_total,
            entry.size(),
            limits.max_total_bytes,
            "total member bytes",
        )?;
        if members.insert(canonical.clone(), index).is_some() {
            return Err(DecodeError::DuplicateMember(canonical));
        }
    }

    let mut reader = MemberReader {
        zip,
        limits,
        total: 0,
    };
    let manifest_index = members
        .remove(MANIFEST_PATH)
        .ok_or(DecodeError::MissingManifest)?;
    let manifest = Manifest::parse(&reader.read(manifest_index, MANIFEST_PATH)?)?;

    let declared_files = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry.digest))
        .collect::<BTreeMap<_, _>>();
    let embedded_packages = manifest
        .packages
        .iter()
        .filter(|entry| entry.embedded)
        .map(|entry| (package_member_prefix(&entry.spec), entry))
        .collect::<BTreeMap<_, _>>();
    let embedded_fonts = manifest
        .fonts
        .iter()
        .filter(|entry| entry.embedded)
        .map(|entry| entry.digest)
        .collect::<BTreeSet<_>>();

    let mut files = Vec::new();
    let mut package_files = BTreeMap::<String, Vec<(String, SharedBytes)>>::new();
    let mut font_data = BTreeMap::new();
    for (name, index) in members {
        if let Some(path) = name
            .strip_prefix(PROJECT_PREFIX)
            .filter(|path| declared_files.contains_key(path))
        {
            files.push((path.to_owned(), reader.read_shared(index, &name)?));
        } else if let Some((prefix, path)) = embedded_packages
            .keys()
            .find_map(|prefix| Some((prefix, name.strip_prefix(prefix.as_str())?)))
        {
            let data = reader.read_shared(index, &name)?;
            package_files
                .entry(prefix.clone())
                .or_default()
                .push((path.to_owned(), data));
        } else if let Some(digest) = name
            .strip_prefix(FONTS_PREFIX)
            .and_then(|file| Digest::from_hex(file.split('.').next().unwrap_or_default()))
            .filter(|digest| embedded_fonts.contains(digest))
        {
            font_data.insert(digest, reader.read_shared(index, &name)?);
        } else {
            return Err(DecodeError::UndeclaredMember(name));
        }
    }

    if let Some(path) = declared_files
        .keys()
        .find(|path| !files.iter().any(|(file, _)| file == *path))
    {
        return Err(DecodeError::MissingMember(format!("project file {path:?}")));
    }
    let packages = manifest
        .packages
        .iter()
        .map(|entry| {
            let content = if entry.embedded {
                PackageContent::Files(
                    package_files
                        .remove(&package_member_prefix(&entry.spec))
                        .ok_or_else(|| {
                            DecodeError::MissingMember(format!("package {}", entry.spec))
                        })?,
                )
            } else {
                PackageContent::Declared(entry.digest)
            };
            Ok(PackageInput {
                spec: entry.spec.clone(),
                embedded: entry.embedded,
                content,
            })
        })
        .collect::<Result<_, DecodeError>>()?;
    let fonts = manifest
        .fonts
        .iter()
        .map(|entry| {
            Ok(if entry.embedded {
                FontInput::Bytes {
                    data: font_data.remove(&entry.digest).ok_or_else(|| {
                        DecodeError::MissingMember(format!("font container {}", entry.digest))
                    })?,
                    embedded: true,
                }
            } else {
                FontInput::Declared {
                    digest: entry.digest,
                    faces: entry.faces.clone(),
                }
            })
        })
        .collect::<Result<_, DecodeError>>()?;

    let pack = Pack::construct(PackInput {
        entrypoint: manifest.entrypoint.clone(),
        metadata: manifest.metadata.clone(),
        files,
        packages,
        fonts,
    })?;
    verify_digests(&pack, &manifest)?;
    Ok(pack)
}

/// Compares the digests derived from the decoded content with the declared
/// ones.
fn verify_digests(pack: &Pack, manifest: &Manifest) -> Result<(), DecodeError> {
    let mismatch = |name: String, declared: Digest, actual: Digest| {
        (declared != actual).then_some(DecodeError::DigestMismatch {
            name,
            declared,
            actual,
        })
    };
    let declared_files = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry.digest))
        .collect::<BTreeMap<_, _>>();
    let files = pack.file_digests().filter_map(|(path, actual)| {
        mismatch(
            format!("project file {path:?}"),
            declared_files[path],
            actual,
        )
    });
    let mut declared_packages = manifest.packages.clone();
    declared_packages.sort_by_cached_key(|entry| entry.spec.to_string());
    let packages = declared_packages
        .iter()
        .zip(pack.package_requirements())
        .filter_map(|(declared, actual)| {
            mismatch(
                format!("package {}", declared.spec),
                declared.digest,
                actual.digest,
            )
        });
    let fonts = manifest
        .fonts
        .iter()
        .zip(pack.font_requirements())
        .filter_map(|(declared, actual)| {
            mismatch(
                format!("font container {}", declared.digest),
                declared.digest,
                actual.digest,
            )
        });
    match files.chain(packages).chain(fonts).next() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Reads member content under the decode limits, metering actual inflated
/// bytes rather than trusting declared sizes.
struct MemberReader<'a> {
    zip: zip::ZipArchive<Cursor<&'a [u8]>>,
    limits: DecodeLimits,
    total: u64,
}

impl MemberReader<'_> {
    fn read(&mut self, index: usize, name: &str) -> Result<Vec<u8>, DecodeError> {
        let entry = self.zip.by_index(index)?;
        let limit = self
            .limits
            .max_member_bytes
            .min(self.limits.max_total_bytes - self.total);
        let mut data = Vec::with_capacity(entry.size().min(limit).min(1 << 20) as usize);
        entry
            .take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|source| DecodeError::UnreadableMember {
                name: name.to_owned(),
                source,
            })?;
        LimitExceeded::check(
            data.len() as u64,
            self.limits.max_member_bytes,
            "member bytes",
        )?;
        LimitExceeded::add(
            &mut self.total,
            data.len() as u64,
            self.limits.max_total_bytes,
            "total member bytes",
        )?;
        Ok(data)
    }

    fn read_shared(&mut self, index: usize, name: &str) -> Result<SharedBytes, DecodeError> {
        self.read(index, name).map(SharedBytes::new)
    }
}

/// The raw member names recorded in the central directory. The ZIP reader
/// keeps only one member per raw name, so duplicates are detected here.
fn raw_member_names(archive: &[u8], start: u64) -> Vec<&[u8]> {
    let mut names = Vec::new();
    let mut at = usize::try_from(start).unwrap_or(usize::MAX);
    while let Some(header) = archive
        .get(at..at.saturating_add(46))
        .filter(|header| header.starts_with(b"PK\x01\x02"))
    {
        let field =
            |offset: usize| usize::from(u16::from_le_bytes([header[offset], header[offset + 1]]));
        let (name, extra, comment) = (field(28), field(30), field(32));
        let Some(raw) = archive.get(at + 46..at + 46 + name) else {
            break;
        };
        names.push(raw);
        at += 46 + name + extra + comment;
    }
    names
}
