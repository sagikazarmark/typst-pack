//! Versioned Pack Archive encoding and decoding.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::str::FromStr;

use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;

use crate::manifest::PackManifest;
pub use crate::manifest::{FORMAT_VERSION, MANIFEST_PATH, PackManifestError as ManifestError};
use crate::pack::{
    PackConstructionInput, PackFontInput, PackInvariantError, PackageFileInput,
    PackageRequirementInput, ProjectFileInput,
};
use crate::payload::SharedBytes;
use crate::{Pack, PackArchiveBytes};

/// A resource bounded during Pack Archive Decoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeResource {
    ArchiveBytes,
    Members,
    RawMemberNameBytes,
    ManifestBytes,
    MemberBytes,
    TotalContentBytes,
}

/// A supplied decode ceiling that cannot support bounded accounting.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: DecodeResource,
        ceiling: u64,
    },
}

/// A Pack Archive exceeded a mandatory decode ceiling.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeLimitError {
    #[error(
        "Pack Archive Decode {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: DecodeResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("Pack Archive Decode {resource:?} accounting overflowed")]
    AccountingOverflow { resource: DecodeResource },
}

/// A failure in one phase of Pack Archive Decoding.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    #[error(transparent)]
    Limit(#[from] DecodeLimitError),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("decoded declarations and content do not form a valid Pack: {0}")]
    InvalidPack(#[from] PackInvariantError),
}

/// A malformed, unsafe, ambiguous, or unsupported raw Pack Archive.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ArchiveError {
    #[error("failed to read ZIP structure: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("failed to read ZIP structure: {0}")]
    Io(#[from] std::io::Error),
    #[error("the archive contains no {MANIFEST_PATH} manifest (is this a Typst Pack?)")]
    MissingManifest,
    #[error("the archive contains more than one {MANIFEST_PATH} manifest")]
    DuplicateManifest,
    #[error("the archive contains a duplicate raw member named {0:?}")]
    DuplicateMember(Vec<u8>),
    #[error("the archive contains members with ambiguous effective names")]
    AmbiguousMemberNames,
    #[error("the archive contains a malformed UTF-8 member name {0:?}")]
    InvalidUtf8MemberName(Vec<u8>),
    #[error("the {MANIFEST_PATH} manifest is not a regular file")]
    ManifestNotFile,
    #[error("archive member {member:?} could not be read: {source}")]
    MemberUnreadable {
        member: String,
        #[source]
        source: std::io::Error,
    },
    #[error("archive member {0:?} has an unsafe path")]
    UnsafeMemberName(String),
    #[error("package archive member {0:?} does not name a package file")]
    MalformedPackageMember(String),
    #[error(
        "package archive member {member:?} contains invalid specification {spec:?}: {message:?}"
    )]
    InvalidPackageSpec {
        member: String,
        spec: String,
        message: String,
    },
    #[error("font declaration path {0:?} is not a safe archive member name")]
    InvalidFontPath(String),
    #[error("font declaration path {path:?} conflicts with the {role:?} archive role")]
    FontPathRoleConflict {
        path: String,
        role: ReservedMemberRole,
    },
    #[error("font declaration path {descendant:?} has file ancestor {ancestor:?}")]
    FontPathTreeConflict {
        ancestor: String,
        descendant: String,
    },
    #[error("archive member {0:?} is not a regular file or directory")]
    UnsupportedMemberKind(String),
}

/// A reserved version-1 archive role that font data cannot occupy.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReservedMemberRole {
    Manifest,
    Project,
    Package,
}

impl From<zip::result::ZipError> for DecodeError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Archive(ArchiveError::Zip(error))
    }
}

impl From<std::io::Error> for DecodeError {
    fn from(error: std::io::Error) -> Self {
        Self::Archive(ArchiveError::Io(error))
    }
}

/// Mandatory finite resource ceilings for Pack Archive Decoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DecodeLimits {
    archive_bytes: u64,
    members: u64,
    raw_member_name_bytes: u64,
    manifest_bytes: u64,
    member_bytes: u64,
    total_content_bytes: u64,
}

impl DecodeLimits {
    /// Constructs a validated set of mandatory finite decode ceilings.
    pub fn new(
        archive_bytes: u64,
        members: u64,
        raw_member_name_bytes: u64,
        manifest_bytes: u64,
        member_bytes: u64,
        total_content_bytes: u64,
    ) -> Result<Self, DecodeLimitsError> {
        let ceilings = [
            (DecodeResource::ArchiveBytes, archive_bytes),
            (DecodeResource::Members, members),
            (DecodeResource::RawMemberNameBytes, raw_member_name_bytes),
            (DecodeResource::ManifestBytes, manifest_bytes),
            (DecodeResource::MemberBytes, member_bytes),
            (DecodeResource::TotalContentBytes, total_content_bytes),
        ];
        if let Some((resource, ceiling)) = ceilings
            .into_iter()
            .find(|(_, ceiling)| *ceiling == u64::MAX)
        {
            return Err(DecodeLimitsError::CannotProbe { resource, ceiling });
        }
        Ok(Self {
            archive_bytes,
            members,
            raw_member_name_bytes,
            manifest_bytes,
            member_bytes,
            total_content_bytes,
        })
    }

    /// The first-party limits for version-1 Pack Archives.
    pub const fn reference_v1() -> Self {
        Self {
            archive_bytes: 512 * 1024 * 1024,
            members: 100_000,
            raw_member_name_bytes: 16 * 1024 * 1024,
            manifest_bytes: 4 * 1024 * 1024,
            member_bytes: 256 * 1024 * 1024,
            total_content_bytes: 2 * 1024 * 1024 * 1024,
        }
    }

    pub const fn archive_bytes(&self) -> u64 {
        self.archive_bytes
    }

    pub const fn members(&self) -> u64 {
        self.members
    }

    pub const fn raw_member_name_bytes(&self) -> u64 {
        self.raw_member_name_bytes
    }

    pub const fn manifest_bytes(&self) -> u64 {
        self.manifest_bytes
    }

    pub const fn member_bytes(&self) -> u64 {
        self.member_bytes
    }

    pub const fn total_content_bytes(&self) -> u64 {
        self.total_content_bytes
    }
}

/// Decodes one borrowed exact Pack Archive into an authoritative [`Pack`].
pub fn decode(archive: &PackArchiveBytes, limits: DecodeLimits) -> Result<Pack, DecodeError> {
    if archive.len() > limits.archive_bytes {
        return Err(DecodeLimitError::Exceeded {
            resource: DecodeResource::ArchiveBytes,
            ceiling: limits.archive_bytes,
            observed_at_least: archive.len(),
        }
        .into());
    }
    let central_directory = locate_central_directory(archive.as_slice())?;
    let mut reader = Cursor::new(archive.as_slice());
    let raw_entries =
        raw_central_entries(&mut reader, archive.as_slice(), central_directory, limits)?;
    for entry in &raw_entries {
        if entry.utf8 && std::str::from_utf8(&entry.name).is_err() {
            return Err(ArchiveError::InvalidUtf8MemberName(entry.name.clone()).into());
        }
    }
    let mut raw_names = BTreeSet::new();
    for entry in &raw_entries {
        if !raw_names.insert(entry.name.clone()) {
            return Err(if entry.name == MANIFEST_PATH.as_bytes() {
                ArchiveError::DuplicateManifest
            } else {
                ArchiveError::DuplicateMember(entry.name.clone())
            }
            .into());
        }
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(archive.as_slice()))?;
    if raw_entries.len() != archive.len() {
        return Err(ArchiveError::AmbiguousMemberNames.into());
    }
    let mut manifest_index = None;
    let mut project_entries = Vec::new();
    let mut package_entries = Vec::new();
    let mut unknown_entries = Vec::new();
    let mut effective_names = BTreeMap::new();
    let mut accepted_members = Vec::new();

    for (index, raw_entry) in raw_entries.iter().enumerate() {
        let entry = archive.by_index_raw(index)?;
        let archive_name = entry.name().to_owned();
        let prefix_normalized_name = strip_current_directory_prefix(&archive_name);
        let canonical_name = canonical_archive_name(&archive_name)?;
        register_archive_identity(
            &mut effective_names,
            canonical_name.clone(),
            &raw_entry.name,
        )?;

        let regular_file = is_regular_file(&entry);
        let directory = is_directory(&entry);
        let role_name = if prefix_normalized_name == MANIFEST_PATH
            || prefix_normalized_name.starts_with(PROJECT_PREFIX)
            || prefix_normalized_name.starts_with(PACKAGES_PREFIX)
        {
            prefix_normalized_name
        } else {
            canonical_name.as_str()
        }
        .to_owned();

        if role_name == MANIFEST_PATH && !regular_file {
            return Err(ArchiveError::ManifestNotFile.into());
        }
        if !regular_file && !directory {
            return Err(ArchiveError::UnsupportedMemberKind(archive_name).into());
        }
        accepted_members.push(AcceptedMember {
            index,
            archive_name,
            role_name,
            canonical_name,
            directory,
        });
    }

    for member in accepted_members {
        if member.role_name == MANIFEST_PATH {
            manifest_index = Some(member.index);
        } else if member.directory {
            continue;
        } else if let Some(path) = member.role_name.strip_prefix(PROJECT_PREFIX) {
            project_entries.push(ProjectEntry {
                index: member.index,
                path: path.trim_start_matches('/').to_owned(),
            });
        } else if let Some(rest) = member.role_name.strip_prefix(PACKAGES_PREFIX) {
            let (spec, path) = split_package_entry(rest, &member.archive_name)?;
            package_entries.push(PackageEntry {
                index: member.index,
                spec,
                path,
            });
        } else {
            unknown_entries.push(UnknownEntry {
                index: member.index,
                canonical_name: member.canonical_name,
            });
        }
    }

    let manifest_index = manifest_index.ok_or(ArchiveError::MissingManifest)?;
    let manifest_bytes = read_manifest(&mut archive, manifest_index, limits)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(ManifestError::NotUtf8)?;
    let manifest = PackManifest::from_toml(manifest_text)?;

    let mut font_paths = BTreeSet::new();
    let mut font_path_values = Vec::new();
    for font in manifest.fonts() {
        let path = canonical_archive_name(font.path())
            .map_err(|_| ArchiveError::InvalidFontPath(font.path().to_owned()))?;
        if let Some(role) = reserved_font_archive_role(&path) {
            return Err(ArchiveError::FontPathRoleConflict {
                path: font.path().to_owned(),
                role,
            }
            .into());
        }
        font_paths.insert(path.clone());
        font_path_values.push(path);
    }
    if let Some((ancestor, descendant)) = find_path_tree_conflict(font_path_values) {
        return Err(ArchiveError::FontPathTreeConflict {
            ancestor,
            descendant,
        }
        .into());
    }

    let font_entries = unknown_entries
        .into_iter()
        .filter(|entry| font_paths.contains(&entry.canonical_name))
        .map(|entry| (entry.index, entry.canonical_name))
        .collect::<Vec<_>>();
    let content_indices = project_entries
        .iter()
        .map(|entry| entry.index)
        .chain(package_entries.iter().map(|entry| entry.index))
        .chain(font_entries.iter().map(|(index, _)| *index))
        .collect::<Vec<_>>();
    preflight_content(&mut archive, &content_indices, limits)?;

    let mut total_content_bytes = 0;
    let mut files = Vec::new();
    for project in project_entries {
        files.push(ProjectFileInput {
            path: project.path,
            data: SharedBytes::new(read_content(
                &mut archive,
                project.index,
                limits,
                &mut total_content_bytes,
            )?),
        });
    }
    let mut package_files = Vec::new();
    for package in package_entries {
        package_files.push(PackageFileInput {
            spec: package.spec,
            path: package.path,
            data: SharedBytes::new(read_content(
                &mut archive,
                package.index,
                limits,
                &mut total_content_bytes,
            )?),
            embedded: true,
        });
    }
    let mut fonts_by_path = BTreeMap::new();
    for (index, path) in font_entries {
        fonts_by_path.insert(
            path,
            SharedBytes::new(read_content(
                &mut archive,
                index,
                limits,
                &mut total_content_bytes,
            )?),
        );
    }

    let package_requirements = manifest
        .packages()
        .vendored()
        .iter()
        .cloned()
        .map(|entry| PackageRequirementInput {
            entry,
            embedded: true,
        })
        .chain(
            manifest
                .packages()
                .unvendored()
                .iter()
                .cloned()
                .map(|entry| PackageRequirementInput {
                    entry,
                    embedded: false,
                }),
        )
        .collect();
    let fonts = manifest
        .fonts()
        .iter()
        .map(|entry| {
            let canonical = canonical_archive_name(entry.path()).ok();
            PackFontInput {
                path: Some(entry.path().to_owned()),
                index: entry.index(),
                declared_container_digest: entry.container_digest().map(str::to_owned),
                declared_container_identity_kind: entry
                    .container_identity_kind()
                    .map(str::to_owned),
                declared_container_identity_schema: entry
                    .container_identity_schema()
                    .map(str::to_owned),
                declared_container_identity_algorithm: entry
                    .container_identity_algorithm()
                    .map(str::to_owned),
                declared_container_length: entry.container_length(),
                data: canonical.and_then(|path| fonts_by_path.get(&path).cloned()),
                embedded: !entry.is_external(),
            }
        })
        .collect();

    Pack::construct(PackConstructionInput {
        entrypoint: manifest.project().entrypoint().to_owned(),
        metadata: manifest.metadata().cloned(),
        files,
        package_files,
        package_requirements,
        package_requirements_are_declared: true,
        fonts,
    })
    .map_err(DecodeError::InvalidPack)
}

#[derive(Clone, Copy)]
struct CentralDirectory {
    start: u64,
    archive_offset: u64,
}

fn locate_central_directory(bytes: &[u8]) -> Result<CentralDirectory, ArchiveError> {
    const EOCD_LEN: usize = 22;
    const MAX_COMMENT_LEN: usize = u16::MAX as usize;
    let search_start = bytes.len().saturating_sub(EOCD_LEN + MAX_COMMENT_LEN);
    let eocd = bytes[search_start..]
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(relative, signature)| {
            if signature != b"PK\x05\x06" {
                return None;
            }
            let position = search_start + relative;
            let comment_length = read_u16(bytes, position + 20)? as usize;
            (position.checked_add(EOCD_LEN + comment_length) == Some(bytes.len()))
                .then_some(position)
        })
        .ok_or_else(|| invalid_zip("could not find end of central directory"))?;

    let entries =
        read_u16(bytes, eocd + 10).ok_or_else(|| invalid_zip("truncated ZIP end record"))?;
    let central_size =
        read_u32(bytes, eocd + 12).ok_or_else(|| invalid_zip("truncated ZIP end record"))?;
    let central_offset =
        read_u32(bytes, eocd + 16).ok_or_else(|| invalid_zip("truncated ZIP end record"))?;
    if entries != u16::MAX && central_size != u32::MAX && central_offset != u32::MAX {
        let start = eocd
            .checked_sub(central_size as usize)
            .and_then(|start| u64::try_from(start).ok())
            .ok_or_else(|| invalid_zip("invalid central directory size"))?;
        let archive_offset = start
            .checked_sub(u64::from(central_offset))
            .ok_or_else(|| invalid_zip("invalid central directory offset"))?;
        return Ok(CentralDirectory {
            start,
            archive_offset,
        });
    }

    let locator = eocd
        .checked_sub(20)
        .filter(|position| bytes.get(*position..*position + 4) == Some(b"PK\x06\x07"))
        .ok_or_else(|| invalid_zip("missing ZIP64 end locator"))?;
    let zip64_eocd = bytes[..locator]
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(position, signature)| {
            if signature != b"PK\x06\x06" {
                return None;
            }
            let record_size = read_u64(bytes, position + 4)?;
            let record_end = u64::try_from(position)
                .ok()?
                .checked_add(12)?
                .checked_add(record_size)?;
            (record_end == locator as u64).then_some(position)
        })
        .ok_or_else(|| invalid_zip("missing ZIP64 end record"))?;
    let central_size = read_u64(bytes, zip64_eocd + 40)
        .ok_or_else(|| invalid_zip("truncated ZIP64 end record"))?;
    let central_offset = read_u64(bytes, zip64_eocd + 48)
        .ok_or_else(|| invalid_zip("truncated ZIP64 end record"))?;
    let start = u64::try_from(zip64_eocd)
        .ok()
        .and_then(|end| end.checked_sub(central_size))
        .ok_or_else(|| invalid_zip("invalid ZIP64 central directory size"))?;
    let archive_offset = start
        .checked_sub(central_offset)
        .ok_or_else(|| invalid_zip("invalid ZIP64 central directory offset"))?;
    Ok(CentralDirectory {
        start,
        archive_offset,
    })
}

fn invalid_zip(message: &'static str) -> ArchiveError {
    ArchiveError::Zip(zip::result::ZipError::InvalidArchive(message.into()))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

struct RawCentralEntry {
    name: Vec<u8>,
    utf8: bool,
}

fn raw_central_entries<R: Read + Seek>(
    reader: &mut R,
    archive: &[u8],
    central_directory: CentralDirectory,
    limits: DecodeLimits,
) -> Result<Vec<RawCentralEntry>, DecodeError> {
    reader.seek(SeekFrom::Start(central_directory.start))?;
    let mut entries = Vec::new();
    let mut total_name_bytes = 0u64;
    loop {
        let header_start = reader.stream_position()?;
        let mut signature = [0; 4];
        reader.read_exact(&mut signature)?;
        if signature != *b"PK\x01\x02" {
            reader.seek(SeekFrom::Start(header_start))?;
            break;
        }

        let mut fixed = [0; 42];
        reader.read_exact(&mut fixed)?;
        let flags = u16::from_le_bytes([fixed[4], fixed[5]]);
        let name_len = u16::from_le_bytes([fixed[24], fixed[25]]) as usize;
        let extra_len = u16::from_le_bytes([fixed[26], fixed[27]]) as i64;
        let comment_len = u16::from_le_bytes([fixed[28], fixed[29]]) as i64;
        let observed_members = u64::try_from(entries.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(DecodeLimitError::AccountingOverflow {
                resource: DecodeResource::Members,
            })?;
        if observed_members > limits.members {
            return Err(DecodeLimitError::Exceeded {
                resource: DecodeResource::Members,
                ceiling: limits.members,
                observed_at_least: observed_members,
            }
            .into());
        }
        total_name_bytes = total_name_bytes
            .checked_add(u64::try_from(name_len).map_err(|_| {
                DecodeLimitError::AccountingOverflow {
                    resource: DecodeResource::RawMemberNameBytes,
                }
            })?)
            .ok_or(DecodeLimitError::AccountingOverflow {
                resource: DecodeResource::RawMemberNameBytes,
            })?;
        if total_name_bytes > limits.raw_member_name_bytes {
            return Err(DecodeLimitError::Exceeded {
                resource: DecodeResource::RawMemberNameBytes,
                ceiling: limits.raw_member_name_bytes,
                observed_at_least: total_name_bytes,
            }
            .into());
        }
        let mut name = vec![0; name_len];
        reader.read_exact(&mut name)?;
        let mut extra = vec![0; extra_len as usize];
        reader.read_exact(&mut extra)?;
        reader.seek(SeekFrom::Current(comment_len))?;
        let local_offset = zip64_local_offset(&fixed, &extra)?
            .checked_add(central_directory.archive_offset)
            .ok_or_else(|| invalid_zip("local header offset overflow"))?;
        let central_unicode_name = unicode_path(&extra, &name)?;
        validate_local_name(archive, local_offset, &name, flags, central_unicode_name)?;
        entries.push(RawCentralEntry {
            name,
            utf8: flags & (1 << 11) != 0,
        });
    }
    Ok(entries)
}

fn zip64_local_offset(fixed: &[u8; 42], extra: &[u8]) -> Result<u64, ArchiveError> {
    let offset = u32::from_le_bytes([fixed[38], fixed[39], fixed[40], fixed[41]]);
    if offset != u32::MAX {
        return Ok(u64::from(offset));
    }

    for field in ExtraFields::new(extra) {
        let (id, data) = field?;
        if id != 0x0001 {
            continue;
        }

        let mut offset_cursor = 0usize;
        if u32::from_le_bytes([fixed[20], fixed[21], fixed[22], fixed[23]]) == u32::MAX {
            offset_cursor += 8;
        }
        if u32::from_le_bytes([fixed[16], fixed[17], fixed[18], fixed[19]]) == u32::MAX {
            offset_cursor += 8;
        }
        return read_u64(data, offset_cursor)
            .ok_or_else(|| invalid_zip("ZIP64 local header offset is missing"));
    }
    Err(invalid_zip("ZIP64 local header offset is missing"))
}

fn validate_local_name(
    archive: &[u8],
    local_offset: u64,
    central_name: &[u8],
    central_flags: u16,
    central_unicode_name: Option<&[u8]>,
) -> Result<(), ArchiveError> {
    let start =
        usize::try_from(local_offset).map_err(|_| invalid_zip("invalid local header offset"))?;
    let fixed_end = start
        .checked_add(30)
        .ok_or_else(|| invalid_zip("local header offset overflow"))?;
    let fixed = archive
        .get(start..fixed_end)
        .ok_or_else(|| invalid_zip("truncated local header"))?;
    if &fixed[..4] != b"PK\x03\x04" {
        return Err(invalid_zip("invalid local header signature"));
    }
    let local_flags = u16::from_le_bytes([fixed[6], fixed[7]]);
    let name_len = usize::from(u16::from_le_bytes([fixed[26], fixed[27]]));
    let extra_len = usize::from(u16::from_le_bytes([fixed[28], fixed[29]]));
    let name_start = fixed_end;
    let name_end = name_start
        .checked_add(name_len)
        .ok_or_else(|| invalid_zip("local member name length overflow"))?;
    let extra_end = name_end
        .checked_add(extra_len)
        .ok_or_else(|| invalid_zip("local extra field length overflow"))?;
    let local_name = archive
        .get(name_start..name_end)
        .ok_or_else(|| invalid_zip("truncated local member name"))?;
    let local_extra = archive
        .get(name_end..extra_end)
        .ok_or_else(|| invalid_zip("truncated local extra fields"))?;
    if local_name != central_name || (local_flags ^ central_flags) & (1 << 11) != 0 {
        return Err(ArchiveError::AmbiguousMemberNames);
    }
    if local_flags & (1 << 11) != 0 && std::str::from_utf8(local_name).is_err() {
        return Err(ArchiveError::InvalidUtf8MemberName(local_name.to_vec()));
    }
    if let Some(local_unicode_name) = unicode_path(local_extra, local_name)?
        && local_unicode_name != central_unicode_name.unwrap_or(central_name)
    {
        return Err(ArchiveError::AmbiguousMemberNames);
    }
    Ok(())
}

fn unicode_path<'a>(extra: &'a [u8], raw_name: &[u8]) -> Result<Option<&'a [u8]>, ArchiveError> {
    let mut unicode_name = None;
    for field in ExtraFields::new(extra) {
        let (id, data) = field?;
        if id != 0x7075 {
            continue;
        }
        if unicode_name.is_some() {
            return Err(ArchiveError::AmbiguousMemberNames);
        }
        let crc = data
            .get(1..5)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| invalid_zip("Unicode path extra field is too small"))?;
        if crc != crc32(raw_name) {
            return Err(invalid_zip(
                "Unicode path extra field has an invalid checksum",
            ));
        }
        let name = &data[5..];
        std::str::from_utf8(name)
            .map_err(|_| invalid_zip("Unicode path extra field is not valid UTF-8"))?;
        unicode_name = Some(name);
    }
    Ok(unicode_name)
}

struct ExtraFields<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ExtraFields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
}

impl<'a> Iterator for ExtraFields<'a> {
    type Item = Result<(u16, &'a [u8]), ArchiveError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor == self.bytes.len() {
            return None;
        }
        let result = (|| {
            let header_end = self
                .cursor
                .checked_add(4)
                .ok_or_else(|| invalid_zip("extra field header overflow"))?;
            let header = self
                .bytes
                .get(self.cursor..header_end)
                .ok_or_else(|| invalid_zip("truncated extra field header"))?;
            let id = u16::from_le_bytes([header[0], header[1]]);
            let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
            let field_end = header_end
                .checked_add(length)
                .ok_or_else(|| invalid_zip("extra field length overflow"))?;
            let data = self
                .bytes
                .get(header_end..field_end)
                .ok_or_else(|| invalid_zip("truncated extra field"))?;
            self.cursor = field_end;
            Ok((id, data))
        })();
        if result.is_err() {
            self.cursor = self.bytes.len();
        }
        Some(result)
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

const PROJECT_PREFIX: &str = "project/";
const PACKAGES_PREFIX: &str = "packages/";
const FILE_TYPE_MASK: u32 = 0o170000;
const REGULAR_FILE: u32 = 0o100000;
const DIRECTORY: u32 = 0o040000;

struct ProjectEntry {
    index: usize,
    path: String,
}

struct PackageEntry {
    index: usize,
    spec: PackageSpec,
    path: String,
}

struct UnknownEntry {
    index: usize,
    canonical_name: String,
}

struct AcceptedMember {
    index: usize,
    archive_name: String,
    role_name: String,
    canonical_name: String,
    directory: bool,
}

fn is_regular_file<R: Read>(entry: &zip::read::ZipFile<'_, R>) -> bool {
    entry.is_file()
        && entry
            .unix_mode()
            .is_none_or(|mode| matches!(mode & FILE_TYPE_MASK, 0 | REGULAR_FILE))
}

fn is_directory<R: Read>(entry: &zip::read::ZipFile<'_, R>) -> bool {
    entry.is_dir()
        && entry
            .unix_mode()
            .is_none_or(|mode| matches!(mode & FILE_TYPE_MASK, 0 | DIRECTORY))
}

fn read_manifest<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    limits: DecodeLimits,
) -> Result<Vec<u8>, DecodeError> {
    let mut entry = archive.by_index(index)?;
    let name = entry.name().to_owned();
    let size = entry.size();
    read_bounded(
        &mut entry,
        size,
        limits.manifest_bytes,
        DecodeResource::ManifestBytes,
        name,
    )
}

fn preflight_content<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    indices: &[usize],
    limits: DecodeLimits,
) -> Result<(), DecodeError> {
    let mut total = 0u64;
    for &index in indices {
        let size = archive.by_index_raw(index)?.size();
        check_exceeded(DecodeResource::MemberBytes, limits.member_bytes, size)?;
        total = total
            .checked_add(size)
            .ok_or(DecodeLimitError::AccountingOverflow {
                resource: DecodeResource::TotalContentBytes,
            })?;
        check_exceeded(
            DecodeResource::TotalContentBytes,
            limits.total_content_bytes,
            total,
        )?;
    }
    Ok(())
}

fn read_content<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    limits: DecodeLimits,
    total: &mut u64,
) -> Result<Vec<u8>, DecodeError> {
    let entry = archive.by_index(index)?;
    let name = entry.name().to_owned();
    let size = entry.size();
    let total_remaining = limits.total_content_bytes.checked_sub(*total).ok_or(
        DecodeLimitError::AccountingOverflow {
            resource: DecodeResource::TotalContentBytes,
        },
    )?;
    let probe_ceiling = limits.member_bytes.min(total_remaining);
    let capacity = usize::try_from(size.min(probe_ceiling).min(64 * 1024)).unwrap();
    let mut data = Vec::with_capacity(capacity);
    entry
        .take(probe_ceiling + 1)
        .read_to_end(&mut data)
        .map_err(|source| ArchiveError::MemberUnreadable {
            member: name,
            source,
        })?;
    let actual_member_bytes =
        u64::try_from(data.len()).map_err(|_| DecodeLimitError::AccountingOverflow {
            resource: DecodeResource::MemberBytes,
        })?;
    check_exceeded(
        DecodeResource::MemberBytes,
        limits.member_bytes,
        actual_member_bytes,
    )?;
    let actual_total =
        total
            .checked_add(actual_member_bytes)
            .ok_or(DecodeLimitError::AccountingOverflow {
                resource: DecodeResource::TotalContentBytes,
            })?;
    check_exceeded(
        DecodeResource::TotalContentBytes,
        limits.total_content_bytes,
        actual_total,
    )?;
    *total = actual_total;
    Ok(data)
}

fn read_bounded(
    reader: &mut impl Read,
    declared_size: u64,
    ceiling: u64,
    resource: DecodeResource,
    member: String,
) -> Result<Vec<u8>, DecodeError> {
    check_exceeded(resource, ceiling, declared_size)?;
    let capacity = usize::try_from(declared_size.min(ceiling).min(64 * 1024)).unwrap();
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .take(ceiling + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| ArchiveError::MemberUnreadable { member, source })?;
    check_exceeded(
        resource,
        ceiling,
        u64::try_from(bytes.len())
            .map_err(|_| DecodeLimitError::AccountingOverflow { resource })?,
    )?;
    Ok(bytes)
}

fn check_exceeded(
    resource: DecodeResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), DecodeLimitError> {
    if observed > ceiling {
        return Err(DecodeLimitError::Exceeded {
            resource,
            ceiling,
            observed_at_least: observed,
        });
    }
    Ok(())
}

fn split_package_entry(rest: &str, member: &str) -> Result<(PackageSpec, String), ArchiveError> {
    let mut parts = rest.splitn(4, '/');
    let (Some(namespace), Some(name), Some(version), Some(path)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ArchiveError::MalformedPackageMember(member.to_owned()));
    };
    let serialized = format!("@{namespace}/{name}:{version}");
    let spec =
        PackageSpec::from_str(&serialized).map_err(|error| ArchiveError::InvalidPackageSpec {
            member: member.to_owned(),
            spec: serialized,
            message: error.to_string(),
        })?;
    Ok((spec, path.trim_start_matches('/').to_owned()))
}

fn canonical_archive_name(path: &str) -> Result<String, ArchiveError> {
    let prefix_normalized_path = strip_current_directory_prefix(path);
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains('\0')
        || has_windows_drive_prefix(prefix_normalized_path)
    {
        return Err(ArchiveError::UnsafeMemberName(path.to_owned()));
    }
    let canonical = VirtualPath::new(path)
        .map_err(|_| ArchiveError::UnsafeMemberName(path.to_owned()))?
        .get_without_slash()
        .to_owned();
    if has_windows_drive_prefix(&canonical) {
        return Err(ArchiveError::UnsafeMemberName(path.to_owned()));
    }
    Ok(canonical)
}

fn register_archive_identity(
    entries: &mut BTreeMap<String, Vec<u8>>,
    canonical: String,
    raw_name: &[u8],
) -> Result<(), ArchiveError> {
    if let Some(first_entry) = entries.get(&canonical) {
        if first_entry == raw_name {
            return Ok(());
        }
        return Err(ArchiveError::AmbiguousMemberNames);
    }
    entries.insert(canonical, raw_name.to_owned());
    Ok(())
}

fn strip_current_directory_prefix(mut path: &str) -> &str {
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    path
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn reserved_font_archive_role(path: &str) -> Option<ReservedMemberRole> {
    if is_same_or_descendant(path, MANIFEST_PATH) {
        Some(ReservedMemberRole::Manifest)
    } else if is_same_or_descendant(path, PROJECT_PREFIX.trim_end_matches('/')) {
        Some(ReservedMemberRole::Project)
    } else if is_same_or_descendant(path, PACKAGES_PREFIX.trim_end_matches('/')) {
        Some(ReservedMemberRole::Package)
    } else {
        None
    }
}

fn is_same_or_descendant(path: &str, ancestor: &str) -> bool {
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn find_path_tree_conflict(mut paths: Vec<String>) -> Option<(String, String)> {
    paths.sort();
    for ancestor in &paths {
        let prefix = format!("{ancestor}/");
        if let Some(descendant) = paths.iter().find(|path| path.starts_with(&prefix)) {
            return Some((ancestor.clone(), descendant.clone()));
        }
    }
    None
}
