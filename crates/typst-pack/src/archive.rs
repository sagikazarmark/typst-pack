//! The pack archive: a ZIP file holding a `typst-pack.toml` manifest and the
//! pack's content.
//!
//! See the
//! [pack format](https://github.com/sagikazarmark/typst-pack/blob/main/docs/reference/pack-format.md)
//! for the layout and verification rules.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read, Write};

use zip::ZipWriter;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;

use crate::font::Face;
pub use crate::manifest::FORMAT_VERSION;
use crate::manifest::{
    FaceEntry, FileEntry, FontEntry, GENERATOR, MANIFEST_PATH, Manifest, PackageEntry,
};
use crate::pack::{
    FontInput, PackInput, PackageContent, PackageInput, Producer, package_member_prefix,
};
use crate::paths::canonical_relative_path;
use crate::payload::SharedBytes;
use crate::project::PROJECT_PREFIX;
use crate::{Digest, LimitExceeded, Pack, PackError};

const FONTS_PREFIX: &str = "fonts/";

/// Every extension [`font_member_name`] chooses.
const FONT_EXTENSIONS: [&str; 5] = ["ttf", "otf", "ttc", "woff", "woff2"];

/// The archive member name of an embedded font container: its digest and an
/// extension chosen by the container's leading bytes.
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

/// Encodes a pack as archive bytes, recording this crate as the generator.
///
/// The recorded Typst version is the pack's own
/// ([`Pack::typst_version`]), so re-encoding a decoded pack keeps the Typst
/// version it was created with, or records none if it had none. A pack built
/// by this crate records the embedded Typst version.
///
/// The output is deterministic for a given build: members are written in a
/// fixed order with a fixed timestamp (1980-01-01), permissions, and
/// compression level.
pub fn encode(pack: &Pack) -> Vec<u8> {
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        typst_version: pack.typst_version().unwrap_or_default().to_owned(),
        generator: GENERATOR.to_owned(),
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
                faces: requirement.faces.iter().map(FaceEntry::from).collect(),
            })
            .collect(),
        metadata: pack.metadata().cloned(),
    };

    // Pack validation keeps every member name within the ZIP limit, and
    // writing to memory cannot fail.
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut add = |name: String, data: &[u8]| {
        // Deflate may expand incompressible input; nine bits per byte plus
        // framing bounds the compressed size.
        let bound = data.len() as u64 + (data.len() as u64).div_ceil(8) + 16;
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(zip::DateTime::DEFAULT)
            .system(zip::System::Unix)
            .unix_permissions(0o644)
            .large_file(bound > zip::ZIP64_BYTES_THR);
        zip.start_file(name, options)
            .expect("a validated pack has unique, short member names");
        zip.write_all(data).expect("writing to memory cannot fail");
    };
    add(MANIFEST_PATH.to_owned(), manifest.to_toml().as_bytes());
    for (path, data) in pack.files() {
        add(format!("{PROJECT_PREFIX}{path}"), data);
    }
    for (spec, tree) in pack.packages() {
        let prefix = package_member_prefix(spec);
        for (path, data) in tree.files() {
            add(format!("{prefix}{path}"), data);
        }
    }
    for requirement in pack.font_requirements() {
        if let Some(data) = pack.font_data(&requirement.digest) {
            add(font_member_name(&requirement.digest, data), data);
        }
    }
    zip.finish()
        .expect("writing to memory cannot fail")
        .into_inner()
}

/// Encodes a pack and writes it to a stream, then flushes the stream.
pub fn write(mut writer: impl Write, pack: &Pack) -> io::Result<()> {
    writer.write_all(&encode(pack))?;
    writer.flush()
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

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_archive_bytes: 512 * 1024 * 1024,
            max_members: 100_000,
            max_member_bytes: 256 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Archive bytes that could not be read or decoded into a pack.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The archive could not be read from its stream.
    #[error("failed to read pack archive bytes")]
    Io(#[from] io::Error),
    /// A decode limit was exceeded.
    #[error(transparent)]
    Limit(#[from] LimitExceeded),
    /// The bytes are not a valid ZIP archive. The ZIP reader's own error is
    /// the source.
    #[error("not a valid ZIP archive")]
    Zip(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// A member could not be decompressed.
    #[error("archive member {name:?} could not be read")]
    UnreadableMember {
        name: String,
        #[source]
        source: io::Error,
    },
    /// A member name is not a canonical, safe relative path.
    #[error("archive member {0:?} has an unsafe name")]
    UnsafeMemberName(String),
    /// Two members share one name.
    #[error("archive member {0:?} appears more than once")]
    DuplicateMember(String),
    /// A member is a link or another special entry.
    #[error("archive member {0:?} is not a regular file or directory")]
    UnsupportedMemberKind(String),
    /// The archive holds no manifest.
    #[error("the archive contains no {MANIFEST_PATH} manifest (is this a Typst Pack?)")]
    MissingManifest,
    /// The manifest is not UTF-8.
    #[error("the {MANIFEST_PATH} manifest is not valid UTF-8")]
    ManifestNotUtf8,
    /// The manifest is not valid TOML of the expected shape. The TOML
    /// parser's own error is the source.
    #[error("failed to parse the {MANIFEST_PATH} manifest")]
    InvalidManifest(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The manifest admits more than one reading of the archive: a path
    /// that is not canonical, or a path, package, or font container declared
    /// twice.
    #[error("the {MANIFEST_PATH} manifest is ambiguous: {0}")]
    AmbiguousManifest(String),
    /// The manifest declares no format version.
    #[error("the {MANIFEST_PATH} manifest has no integer `format-version`")]
    MissingFormatVersion,
    /// The manifest declares a format version this reader does not implement.
    #[error("unsupported pack format version {0} (this reader supports version {FORMAT_VERSION})")]
    UnsupportedVersion(i64),
    /// A member is not declared by the manifest.
    #[error("archive member {0:?} is not declared by the manifest")]
    UndeclaredMember(String),
    /// A declared file, package, or font is missing.
    #[error("{0} is declared by the manifest but missing from the archive")]
    MissingMember(String),
    /// Content does not match the digest the manifest declares.
    #[error("{name} does not match its declared digest {declared} (actual {actual})")]
    DigestMismatch {
        name: String,
        declared: Digest,
        actual: Digest,
    },
    /// The content does not form a valid pack.
    #[error(transparent)]
    InvalidPack(#[from] PackError),
}

impl DecodeError {
    fn zip(error: ZipError) -> Self {
        Self::Zip(Box::new(error))
    }

    pub(crate) fn manifest(error: toml::de::Error) -> Self {
        Self::InvalidManifest(Box::new(error))
    }
}

/// Reads and decodes a pack from a stream, reading at most
/// [`DecodeLimits::max_archive_bytes`].
pub fn read(reader: impl Read, limits: DecodeLimits) -> Result<Pack, DecodeError> {
    let mut archive = Vec::new();
    reader
        .take(limits.max_archive_bytes.saturating_add(1))
        .read_to_end(&mut archive)?;
    decode(&archive, limits)
}

/// Decodes archive bytes into a pack, verifying every member against the
/// digest the manifest declares for it.
pub fn decode(archive: &[u8], limits: DecodeLimits) -> Result<Pack, DecodeError> {
    LimitExceeded::check(
        archive.len() as u64,
        limits.max_archive_bytes,
        "archive bytes",
    )?;
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(DecodeError::zip)?;
    let raw_names = raw_member_names(archive, zip.central_directory_start());
    LimitExceeded::check(
        raw_names.len().max(zip.len()) as u64,
        limits.max_members,
        "archive members",
    )?;
    if raw_names.len() != zip.len() {
        let mut seen = BTreeSet::new();
        return Err(match raw_names.iter().find(|name| !seen.insert(**name)) {
            Some(name) => DecodeError::DuplicateMember(String::from_utf8_lossy(name).into_owned()),
            None => DecodeError::zip(ZipError::InvalidArchive(
                "the central directory entry count does not match its entries".into(),
            )),
        });
    }

    let mut members = classify_members(&mut zip, limits)?;
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
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
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
            .filter(|path| declared_files.contains(path))
        {
            files.push((path.to_owned(), reader.read_shared(index, &name)?));
        } else if let Some((prefix, path)) = split_package_member(&name)
            .filter(|(prefix, _)| embedded_packages.contains_key(*prefix))
        {
            let data = reader.read_shared(index, &name)?;
            package_files
                .entry(prefix.to_owned())
                .or_default()
                .push((path.to_owned(), data));
        } else if let Some(digest) =
            font_member_digest(&name).filter(|digest| embedded_fonts.contains(digest))
        {
            if font_data.contains_key(&digest) {
                return Err(DecodeError::DuplicateMember(name));
            }
            let data = reader.read_shared(index, &name)?;
            // The name follows from the content, so a container has exactly
            // one name.
            if font_member_name(&digest, &data) != name {
                return Err(DecodeError::UndeclaredMember(name));
            }
            font_data.insert(digest, data);
        } else {
            return Err(DecodeError::UndeclaredMember(name));
        }
    }

    let present = files
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(path) = declared_files.difference(&present).next() {
        return Err(DecodeError::MissingMember(format!("project file {path:?}")));
    }
    let packages = manifest
        .packages
        .iter()
        .map(|entry| {
            let content = if entry.embedded {
                PackageContent::Files {
                    files: package_files
                        .remove(&package_member_prefix(&entry.spec))
                        .ok_or_else(|| {
                            DecodeError::MissingMember(format!("package {}", entry.spec))
                        })?,
                    embedded: true,
                }
            } else {
                PackageContent::Declared(entry.digest)
            };
            Ok(PackageInput {
                spec: entry.spec.clone(),
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
                    faces: entry.faces.iter().map(Face::from).collect(),
                }
            })
        })
        .collect::<Result<_, DecodeError>>()?;

    let recorded = |value: &str| (!value.is_empty()).then(|| value.to_owned());
    let pack = Pack::construct(PackInput {
        entrypoint: manifest.entrypoint.clone(),
        metadata: manifest.metadata.clone(),
        producer: Producer {
            typst_version: recorded(&manifest.typst_version),
            generator: recorded(&manifest.generator),
        },
        files,
        packages,
        fonts,
    })?;
    verify_digests(&pack, &manifest)?;
    Ok(pack)
}

/// Classifies every member by name and declared size before any content is
/// read, returning the files' indices by name; directories are skipped. Only
/// canonical names of regular files and directories are accepted, so every
/// extractor sees the same tree.
fn classify_members(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    limits: DecodeLimits,
) -> Result<BTreeMap<String, usize>, DecodeError> {
    let mut members = BTreeMap::new();
    let mut declared_total = 0;
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(DecodeError::zip)?;
        let name = entry.name().map_err(DecodeError::zip)?.into_owned();
        let mode = entry.unix_mode().map(|mode| mode & 0o170000);
        let directory = entry.is_dir();
        let path = if directory {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        };
        if canonical_relative_path(path).map_or(true, |canonical| canonical.as_str() != path) {
            return Err(DecodeError::UnsafeMemberName(name));
        }
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
        if members.contains_key(&name) {
            return Err(DecodeError::DuplicateMember(name));
        }
        members.insert(name, index);
    }
    Ok(members)
}

/// Splits `packages/<namespace>/<name>/<version>/<path>` into its package
/// prefix, with the trailing slash, and the package-relative path.
fn split_package_member(name: &str) -> Option<(&str, &str)> {
    let end = name.match_indices('/').nth(3)?.0 + 1;
    name.starts_with("packages/").then(|| name.split_at(end))
}

/// The digest a `fonts/<hex digest>.<extension>` member name declares.
fn font_member_digest(name: &str) -> Option<Digest> {
    let (hex, extension) = name.strip_prefix(FONTS_PREFIX)?.split_once('.')?;
    FONT_EXTENSIONS
        .contains(&extension)
        .then(|| Digest::from_hex(hex))?
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
        let entry = self.zip.by_index(index).map_err(DecodeError::zip)?;
        let limit = self
            .limits
            .max_member_bytes
            .min(self.limits.max_total_bytes - self.total);
        let mut data = Vec::with_capacity(entry.size().min(limit).min(1 << 20) as usize);
        entry
            .take(limit.saturating_add(1))
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
