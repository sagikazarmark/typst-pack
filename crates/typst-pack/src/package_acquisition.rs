//! Package acquisition for a Pack Assembler that supplies its own transport.
//!
//! Creation reports the exact specifications its representative request needs
//! and was not given. These helpers cover the two transformations between that
//! report and a resolved tree — constructing the registry URL of a
//! specification, and expanding the archive bytes fetched from it into a
//! Package Tree — so a caller reimplements neither the registry layout
//! nor the archive encoding. Fetching those bytes is the adapter's own
//! obligation and stays outside: nothing here needs an HTTP client, so the
//! helpers are usable on a host whose only network access is asynchronous.

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;

use crate::Pack;
use crate::package_catalog::{PackageTree, PackageTreeError};

/// A failure while acquiring exact Package Archive bytes from a stream.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PackageArchiveAcquisitionError {
    #[error(transparent)]
    Limit(#[from] PackageExpansionLimitError),
    #[error("failed to read Package Archive bytes: {0}")]
    Read(#[source] std::io::Error),
}

/// The URL of the package registry these helpers describe the layout of, the
/// official Typst Universe registry. There is no standardized registry
/// protocol, so the layout is this registry's own.
pub const PACKAGE_REGISTRY_URL: &str = "https://packages.typst.org";

/// The one package namespace the registry serves. A specification in any other
/// namespace is resolved from wherever its namespace lives, which the registry
/// layout says nothing about.
pub const PACKAGE_REGISTRY_NAMESPACE: &str = "preview";

/// A resource bounded during Package Archive Expansion.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageExpansionResource {
    CompressedArchiveBytes,
    Members,
    MemberNameBytes,
    MemberBytes,
    TotalExpandedBytes,
}

/// A supplied expansion ceiling that cannot support bounded accounting.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageExpansionLimitsError {
    #[error("the {resource:?} ceiling must leave room for a plus-one probe")]
    CannotProbe {
        resource: PackageExpansionResource,
        ceiling: u64,
    },
}

/// A package archive exceeded a mandatory expansion ceiling.
#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageExpansionLimitError {
    #[error(
        "Package Archive Expansion {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}"
    )]
    Exceeded {
        resource: PackageExpansionResource,
        ceiling: u64,
        observed_at_least: u64,
    },
    #[error("Package Archive Expansion {resource:?} accounting overflowed")]
    AccountingOverflow { resource: PackageExpansionResource },
}

/// Mandatory finite resource ceilings for Package Archive Expansion.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PackageExpansionLimits {
    compressed_archive_bytes: u64,
    members: u64,
    member_name_bytes: u64,
    member_bytes: u64,
    total_expanded_bytes: u64,
}

impl PackageExpansionLimits {
    /// Constructs a validated set of mandatory finite expansion ceilings.
    pub fn new(
        compressed_archive_bytes: u64,
        members: u64,
        member_name_bytes: u64,
        member_bytes: u64,
        total_expanded_bytes: u64,
    ) -> Result<Self, PackageExpansionLimitsError> {
        let ceilings = [
            (
                PackageExpansionResource::CompressedArchiveBytes,
                compressed_archive_bytes,
            ),
            (PackageExpansionResource::Members, members),
            (PackageExpansionResource::MemberNameBytes, member_name_bytes),
            (PackageExpansionResource::MemberBytes, member_bytes),
            (
                PackageExpansionResource::TotalExpandedBytes,
                total_expanded_bytes,
            ),
        ];
        if let Some((resource, ceiling)) = ceilings
            .into_iter()
            .find(|(_, ceiling)| *ceiling == u64::MAX)
        {
            return Err(PackageExpansionLimitsError::CannotProbe { resource, ceiling });
        }
        Ok(Self {
            compressed_archive_bytes,
            members,
            member_name_bytes,
            member_bytes,
            total_expanded_bytes,
        })
    }

    /// The first-party limits for registry package archives.
    pub const fn reference_v1() -> Self {
        Self {
            compressed_archive_bytes: 128 * 1024 * 1024,
            members: 50_000,
            member_name_bytes: 8 * 1024 * 1024,
            member_bytes: 64 * 1024 * 1024,
            total_expanded_bytes: 512 * 1024 * 1024,
        }
    }

    pub const fn compressed_archive_bytes(&self) -> u64 {
        self.compressed_archive_bytes
    }

    pub const fn members(&self) -> u64 {
        self.members
    }

    pub const fn member_name_bytes(&self) -> u64 {
        self.member_name_bytes
    }

    pub const fn member_bytes(&self) -> u64 {
        self.member_bytes
    }

    pub const fn total_expanded_bytes(&self) -> u64 {
        self.total_expanded_bytes
    }
}

/// Acquires exact Package Archive bytes under the expansion profile's
/// compressed-byte ceiling.
///
/// A known size is checked before the reader is touched. The stream is still
/// incrementally metered with a plus-one probe because a size declaration is
/// only a hint.
pub fn acquire_package_archive(
    mut reader: impl Read,
    known_size: Option<u64>,
    limits: PackageExpansionLimits,
) -> Result<Vec<u8>, PackageArchiveAcquisitionError> {
    let resource = PackageExpansionResource::CompressedArchiveBytes;
    if let Some(size) = known_size {
        check_expansion_limit(resource, limits.compressed_archive_bytes, size)?;
    }

    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limits.compressed_archive_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(PackageArchiveAcquisitionError::Read)?;
    let observed = u64::try_from(bytes.len())
        .map_err(|_| PackageExpansionLimitError::AccountingOverflow { resource })?;
    check_expansion_limit(resource, limits.compressed_archive_bytes, observed)?;
    Ok(bytes)
}

/// The URL of the archive holding one exact package specification's Package
/// Tree.
///
/// No index lookup is involved: creation only ever reports fully versioned
/// specifications, because a Typst import specification always carries an exact
/// version.
pub fn package_archive_url(spec: &PackageSpec) -> Result<String, PackageAcquisitionError> {
    if spec.namespace != PACKAGE_REGISTRY_NAMESPACE {
        return Err(PackageAcquisitionError::UnservedNamespace { spec: spec.clone() });
    }
    Ok(format!(
        "{PACKAGE_REGISTRY_URL}/{}/{}-{}.tar.gz",
        spec.namespace, spec.name, spec.version
    ))
}

/// Expands the archive bytes served for one exact package specification into
/// the Package Tree creation accepts as a resolved tree for it.
///
/// The bytes are the gzip-compressed tar a registry serves, which the caller
/// fetched with whatever primitive its host provides; expansion itself needs no
/// transport. Only addressable regular files become tree entries: directories,
/// links, and other archive members are not package files. An entry whose path
/// cannot name one is rejected here rather than carried into a tree, because
/// expansion is where a hostile archive is met.
///
/// Expansion stops at `limits` and fails with
/// [`PackageAcquisitionError::ExpansionLimit`] rather than materializing what
/// lies past it. Every raw member and its payload is charged before semantic
/// interpretation, whether or not that member becomes a package file.
///
pub fn expand_package_archive(
    spec: PackageSpec,
    archive: &[u8],
    limits: PackageExpansionLimits,
) -> Result<PackageTree, PackageAcquisitionError> {
    let malformed = |message: String| PackageAcquisitionError::MalformedArchive {
        spec: spec.clone(),
        message,
    };
    let limited = |source| PackageAcquisitionError::ExpansionLimit {
        spec: spec.clone(),
        source,
    };

    let compressed_archive_bytes = u64::try_from(archive.len()).map_err(|_| {
        limited(PackageExpansionLimitError::AccountingOverflow {
            resource: PackageExpansionResource::CompressedArchiveBytes,
        })
    })?;
    check_expansion_limit(
        PackageExpansionResource::CompressedArchiveBytes,
        limits.compressed_archive_bytes,
        compressed_archive_bytes,
    )
    .map_err(limited)?;

    let tar_state = Rc::new(RefCell::new(TarReadState::default()));
    let reader = ObservedTarReader {
        inner: flate2::read::GzDecoder::new(archive),
        state: Rc::clone(&tar_state),
    };
    let mut archive = tar::Archive::new(reader);
    let mut files: Vec<(String, Bytes)> = Vec::new();
    let mut members = 0u64;
    let mut member_name_bytes = 0u64;
    let mut total_expanded_bytes = 0u64;
    let mut gnu_long_name: Option<Vec<u8>> = None;
    let mut gnu_long_link = false;
    let mut pax_local = false;
    let mut pax_path: Option<Vec<u8>> = None;
    let mut pax_size: Option<u64> = None;

    for entry in archive
        .entries()
        .map_err(|error| malformed(error.to_string()))?
        .raw(true)
    {
        let mut entry = entry.map_err(|error| malformed(error.to_string()))?;
        check_tar_padding(&tar_state).map_err(&malformed)?;

        members = checked_add(members, 1, PackageExpansionResource::Members).map_err(limited)?;
        check_expansion_limit(PackageExpansionResource::Members, limits.members, members)
            .map_err(limited)?;

        let raw_name = entry.header().path_bytes().into_owned();
        member_name_bytes = checked_add_usize(
            member_name_bytes,
            raw_name.len(),
            PackageExpansionResource::MemberNameBytes,
        )
        .map_err(limited)?;
        check_expansion_limit(
            PackageExpansionResource::MemberNameBytes,
            limits.member_name_bytes,
            member_name_bytes,
        )
        .map_err(limited)?;

        let entry_type = entry.header().entry_type();
        let extension = entry_type.is_gnu_longname()
            || entry_type.is_gnu_longlink()
            || entry_type.is_pax_local_extensions()
            || entry_type.is_pax_global_extensions();
        if !extension
            && let Some(pax_size) = pax_size.take()
            && pax_size != entry.size()
        {
            return Err(malformed(format!(
                "PAX size {pax_size} conflicts with member size {}",
                entry.size()
            )));
        }
        if entry_type.is_gnu_longname() {
            let observed = checked_add(
                member_name_bytes,
                entry.size(),
                PackageExpansionResource::MemberNameBytes,
            )
            .map_err(limited)?;
            check_expansion_limit(
                PackageExpansionResource::MemberNameBytes,
                limits.member_name_bytes,
                observed,
            )
            .map_err(limited)?;
        }

        check_expansion_limit(
            PackageExpansionResource::MemberBytes,
            limits.member_bytes,
            entry.size(),
        )
        .map_err(limited)?;
        let observed_total = checked_add(
            total_expanded_bytes,
            entry.size(),
            PackageExpansionResource::TotalExpandedBytes,
        )
        .map_err(limited)?;
        check_expansion_limit(
            PackageExpansionResource::TotalExpandedBytes,
            limits.total_expanded_bytes,
            observed_total,
        )
        .map_err(limited)?;

        let declared_size = entry.size();
        if entry_type.is_pax_global_extensions() {
            return Err(malformed(
                "global PAX extensions make package member names ambiguous".to_owned(),
            ));
        }
        if entry_type.is_pax_local_extensions() {
            if pax_local {
                return Err(malformed(
                    "multiple local PAX extensions describe one archive member".to_owned(),
                ));
            }
            let name_remaining = limits.member_name_bytes - member_name_bytes;
            let parsed = match read_pax(&mut entry, declared_size, name_remaining) {
                Ok(parsed) => parsed,
                Err(PaxReadError::Io(error) | PaxReadError::Malformed(error)) => {
                    return Err(malformed(error));
                }
                Err(PaxReadError::NameLimit { observed }) => {
                    return Err(limited(PackageExpansionLimitError::Exceeded {
                        resource: PackageExpansionResource::MemberNameBytes,
                        ceiling: limits.member_name_bytes,
                        observed_at_least: checked_add(
                            member_name_bytes,
                            observed,
                            PackageExpansionResource::MemberNameBytes,
                        )
                        .map_err(limited)?,
                    }));
                }
            };
            register_tar_padding(&tar_state, entry.raw_file_position(), declared_size)
                .map_err(&malformed)?;
            total_expanded_bytes = checked_add(
                total_expanded_bytes,
                declared_size,
                PackageExpansionResource::TotalExpandedBytes,
            )
            .map_err(limited)?;
            if let Some(path) = &parsed.path {
                member_name_bytes = checked_add_usize(
                    member_name_bytes,
                    path.len(),
                    PackageExpansionResource::MemberNameBytes,
                )
                .map_err(limited)?;
            }
            if let Some(size) = parsed.size {
                check_expansion_limit(
                    PackageExpansionResource::MemberBytes,
                    limits.member_bytes,
                    size,
                )
                .map_err(limited)?;
                let observed_total = checked_add(
                    total_expanded_bytes,
                    size,
                    PackageExpansionResource::TotalExpandedBytes,
                )
                .map_err(limited)?;
                check_expansion_limit(
                    PackageExpansionResource::TotalExpandedBytes,
                    limits.total_expanded_bytes,
                    observed_total,
                )
                .map_err(limited)?;
            }
            pax_path = parsed.path;
            pax_size = parsed.size;
            pax_local = true;
            continue;
        }

        let probe_ceiling = limits
            .member_bytes
            .min(limits.total_expanded_bytes - total_expanded_bytes);
        let mut reader = entry.by_ref().take(probe_ceiling + 1);
        let retain = entry_type.is_file() || entry_type.is_gnu_longname();
        let capacity = usize::try_from(declared_size.min(64 * 1024)).unwrap();
        let mut data = Vec::with_capacity(if retain { capacity } else { 0 });
        let mut observed_size = 0u64;
        let mut buffer = [0; 16 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| malformed(error.to_string()))?;
            if read == 0 {
                break;
            }
            observed_size =
                checked_add_usize(observed_size, read, PackageExpansionResource::MemberBytes)
                    .map_err(limited)?;
            check_expansion_limit(
                PackageExpansionResource::MemberBytes,
                limits.member_bytes,
                observed_size,
            )
            .map_err(limited)?;
            let observed_total = checked_add(
                total_expanded_bytes,
                observed_size,
                PackageExpansionResource::TotalExpandedBytes,
            )
            .map_err(limited)?;
            check_expansion_limit(
                PackageExpansionResource::TotalExpandedBytes,
                limits.total_expanded_bytes,
                observed_total,
            )
            .map_err(limited)?;
            if retain {
                data.extend_from_slice(&buffer[..read]);
            }
        }
        if observed_size != declared_size {
            return Err(malformed(format!(
                "archive member declared {declared_size} byte(s) but yielded {observed_size}"
            )));
        }
        register_tar_padding(&tar_state, entry.raw_file_position(), declared_size)
            .map_err(&malformed)?;
        total_expanded_bytes = checked_add(
            total_expanded_bytes,
            observed_size,
            PackageExpansionResource::TotalExpandedBytes,
        )
        .map_err(limited)?;

        if entry_type.is_gnu_longname() {
            if gnu_long_name.is_some() {
                return Err(malformed(
                    "multiple GNU long names describe one archive member".to_owned(),
                ));
            }
            member_name_bytes = checked_add(
                member_name_bytes,
                observed_size,
                PackageExpansionResource::MemberNameBytes,
            )
            .map_err(limited)?;
            gnu_long_name = Some(strip_gnu_terminator(data));
            continue;
        }
        if entry_type.is_gnu_longlink() {
            if gnu_long_link {
                return Err(malformed(
                    "multiple GNU long links describe one archive member".to_owned(),
                ));
            }
            gnu_long_link = true;
            continue;
        }
        let effective_name = match (gnu_long_name.take(), pax_path.take()) {
            (Some(_), Some(_)) => {
                return Err(malformed(
                    "GNU and PAX extensions provide ambiguous member names".to_owned(),
                ));
            }
            (Some(name), None) | (None, Some(name)) => name,
            (None, None) => raw_name,
        };
        gnu_long_link = false;
        pax_local = false;
        let path = std::str::from_utf8(&effective_name)
            .map_err(|_| malformed(format!("member name {effective_name:?} is not valid UTF-8")))?
            .to_owned();

        if entry_type.is_file() {
            files.push((path, Bytes::new(data)));
        } else {
            Pack::canonical_package_path(&path).map_err(|message| {
                malformed(format!(
                    "archive member {path:?} does not name a package file: {message}"
                ))
            })?;
        }
    }

    check_tar_padding(&tar_state).map_err(&malformed)?;

    if gnu_long_name.is_some() || gnu_long_link || pax_local {
        return Err(malformed(
            "archive metadata describes a future member that is missing".to_owned(),
        ));
    }

    PackageTree::from_typst_entries(files)
        .map_err(|source| PackageAcquisitionError::InvalidPackageTree { spec, source })
}

#[derive(Default)]
struct TarReadState {
    position: u64,
    padding: Option<(u64, u64)>,
    nonzero_padding: bool,
}

struct ObservedTarReader<R> {
    inner: R,
    state: Rc<RefCell<TarReadState>>,
}

impl<R: Read> Read for ObservedTarReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        let mut state = self.state.borrow_mut();
        let start = state.position;
        state.position = state
            .position
            .checked_add(read as u64)
            .ok_or_else(|| std::io::Error::other("tar stream position overflowed"))?;
        if let Some((padding_start, padding_end)) = state.padding {
            let overlap_start = start.max(padding_start);
            let overlap_end = state.position.min(padding_end);
            if overlap_start < overlap_end {
                let buffer_start = usize::try_from(overlap_start - start).unwrap();
                let buffer_end = usize::try_from(overlap_end - start).unwrap();
                state.nonzero_padding |= buffer[buffer_start..buffer_end]
                    .iter()
                    .any(|byte| *byte != 0);
            }
            if state.position >= padding_end {
                state.padding = None;
            }
        }
        Ok(read)
    }
}

fn register_tar_padding(
    state: &Rc<RefCell<TarReadState>>,
    file_position: u64,
    declared_size: u64,
) -> Result<(), String> {
    let padding_start = file_position
        .checked_add(declared_size)
        .ok_or_else(|| "tar member position overflowed".to_owned())?;
    let padding_end = padding_start
        .checked_add(511)
        .map(|end| end & !511)
        .ok_or_else(|| "tar member padding position overflowed".to_owned())?;
    state.borrow_mut().padding =
        (padding_start < padding_end).then_some((padding_start, padding_end));
    Ok(())
}

fn check_tar_padding(state: &Rc<RefCell<TarReadState>>) -> Result<(), String> {
    if state.borrow().nonzero_padding {
        return Err("archive member contains non-zero bytes past its declared size".to_owned());
    }
    Ok(())
}

fn strip_gnu_terminator(mut name: Vec<u8>) -> Vec<u8> {
    if name.last() == Some(&0) {
        name.pop();
    }
    name
}

struct PaxMetadata {
    path: Option<Vec<u8>>,
    size: Option<u64>,
}

enum PaxReadError {
    Io(String),
    Malformed(String),
    NameLimit { observed: u64 },
}

fn read_pax(
    reader: &mut impl Read,
    declared_size: u64,
    name_remaining: u64,
) -> Result<PaxMetadata, PaxReadError> {
    let mut remaining = declared_size;
    let mut path = None;
    let mut size = None;
    while remaining > 0 {
        let mut prefix = Vec::with_capacity(21);
        loop {
            let byte = read_pax_byte(reader)?;
            prefix.push(byte);
            if byte == b' ' {
                break;
            }
            if !byte.is_ascii_digit() || prefix.len() > 20 {
                return Err(PaxReadError::Malformed(
                    "malformed PAX record length".to_owned(),
                ));
            }
        }
        let prefix_length = u64::try_from(prefix.len()).unwrap();
        let record_length = std::str::from_utf8(&prefix[..prefix.len() - 1])
            .ok()
            .and_then(|length| length.parse::<u64>().ok())
            .filter(|length| *length > prefix_length + 2 && *length <= remaining)
            .ok_or_else(|| PaxReadError::Malformed("invalid PAX record length".to_owned()))?;

        let mut key = Vec::with_capacity(16);
        let mut key_length = 0u64;
        loop {
            let byte = read_pax_byte(reader)?;
            key_length += 1;
            if byte == b'=' {
                break;
            }
            if key.len() < 16 {
                key.push(byte);
            }
            if prefix_length + key_length + 1 >= record_length {
                return Err(PaxReadError::Malformed(
                    "malformed PAX key-value record".to_owned(),
                ));
            }
        }
        let value_length = record_length - prefix_length - key_length - 1;
        match key.as_slice() {
            b"path" if path.is_some() => {
                return Err(PaxReadError::Malformed(
                    "multiple PAX paths describe one archive member".to_owned(),
                ));
            }
            b"path" => {
                if value_length > name_remaining {
                    return Err(PaxReadError::NameLimit {
                        observed: value_length,
                    });
                }
                let length = usize::try_from(value_length).map_err(|_| {
                    PaxReadError::Malformed("PAX path length is not addressable".to_owned())
                })?;
                let mut value = vec![0; length];
                read_pax_exact(reader, &mut value)?;
                path = Some(value);
            }
            b"size" if size.is_some() => {
                return Err(PaxReadError::Malformed(
                    "multiple PAX sizes describe one archive member".to_owned(),
                ));
            }
            b"size" => {
                if value_length > 20 {
                    return Err(PaxReadError::Malformed(
                        "PAX size is not an unsigned integer".to_owned(),
                    ));
                }
                let mut value = vec![0; value_length as usize];
                read_pax_exact(reader, &mut value)?;
                size = Some(
                    std::str::from_utf8(&value)
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .ok_or_else(|| {
                            PaxReadError::Malformed(
                                "PAX size is not an unsigned integer".to_owned(),
                            )
                        })?,
                );
            }
            _ => discard_pax_bytes(reader, value_length)?,
        }
        if read_pax_byte(reader)? != b'\n' {
            return Err(PaxReadError::Malformed(
                "PAX record does not end with a newline".to_owned(),
            ));
        }
        remaining -= record_length;
    }
    Ok(PaxMetadata { path, size })
}

fn read_pax_byte(reader: &mut impl Read) -> Result<u8, PaxReadError> {
    let mut byte = [0];
    read_pax_exact(reader, &mut byte)?;
    Ok(byte[0])
}

fn read_pax_exact(reader: &mut impl Read, buffer: &mut [u8]) -> Result<(), PaxReadError> {
    reader
        .read_exact(buffer)
        .map_err(|error| PaxReadError::Io(error.to_string()))
}

fn discard_pax_bytes(reader: &mut impl Read, mut remaining: u64) -> Result<(), PaxReadError> {
    let mut buffer = [0; 8 * 1024];
    while remaining > 0 {
        let length = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        read_pax_exact(reader, &mut buffer[..length])?;
        remaining -= length as u64;
    }
    Ok(())
}

fn checked_add(
    total: u64,
    value: u64,
    resource: PackageExpansionResource,
) -> Result<u64, PackageExpansionLimitError> {
    total
        .checked_add(value)
        .ok_or(PackageExpansionLimitError::AccountingOverflow { resource })
}

fn checked_add_usize(
    total: u64,
    value: usize,
    resource: PackageExpansionResource,
) -> Result<u64, PackageExpansionLimitError> {
    let value = u64::try_from(value)
        .map_err(|_| PackageExpansionLimitError::AccountingOverflow { resource })?;
    checked_add(total, value, resource)
}

fn check_expansion_limit(
    resource: PackageExpansionResource,
    ceiling: u64,
    observed: u64,
) -> Result<(), PackageExpansionLimitError> {
    if observed > ceiling {
        return Err(PackageExpansionLimitError::Exceeded {
            resource,
            ceiling,
            observed_at_least: observed,
        });
    }
    Ok(())
}

/// A failure while acquiring one Package Tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PackageAcquisitionError {
    /// The registry does not serve the specification's namespace, so it has no
    /// URL there.
    #[error(
        "the registry serves only the `{PACKAGE_REGISTRY_NAMESPACE}` namespace, and {spec} is not in it"
    )]
    UnservedNamespace { spec: PackageSpec },
    /// The archive exceeds one expansion resource ceiling.
    #[error("the archive for {spec} exceeded an expansion limit: {source}")]
    ExpansionLimit {
        spec: PackageSpec,
        source: PackageExpansionLimitError,
    },
    /// The bytes are not the archive a registry serves for the specification.
    #[error("the archive for {spec} is malformed: {message:?}")]
    MalformedArchive { spec: PackageSpec, message: String },
    /// The archive does not expand into a valid Package Tree.
    #[error("the archive for {spec} does not contain a valid package tree: {source}")]
    InvalidPackageTree {
        spec: PackageSpec,
        source: PackageTreeError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_expansion_accounting_overflow_is_typed() {
        assert_eq!(
            checked_add(u64::MAX, 1, PackageExpansionResource::TotalExpandedBytes),
            Err(PackageExpansionLimitError::AccountingOverflow {
                resource: PackageExpansionResource::TotalExpandedBytes,
            })
        );
    }
}
