use std::fmt;
use std::marker::PhantomData;

const MAX_RESOURCES: usize = 7;

/// A resource identifier from one operation-specific profile.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ResourceKind<const PROFILE: u8>(u8);

impl<const PROFILE: u8> ResourceKind<PROFILE> {
    pub(crate) const fn new(index: u8) -> Self {
        Self(index)
    }
}

impl<const PROFILE: u8> fmt::Debug for ResourceKind<PROFILE> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(resource_name(PROFILE, self.0))
    }
}

const fn operation_name(profile: u8) -> &'static str {
    match profile {
        0 => "filesystem project gathering",
        1 => "filesystem Package Tree gathering",
        2 => "filesystem Font Catalog gathering",
        3 => "compilation",
        4 => "Pack Archive Encode",
        5 => "Pack Archive Decode",
        6 => "Package Archive Expansion",
        7 => "Pack Archive Acquisition",
        8 => "OpenDAL recursive acquisition",
        9 => "OpenDAL Project Acquisition",
        10 => "OpenDAL Font Acquisition",
        11 => "OpenDAL Package Tree Acquisition",
        12 => "OpenDAL Package Archive Acquisition",
        13 => "OpenDAL Package Acquisition",
        _ => "resource-limited operation",
    }
}

const fn resource_name(profile: u8, index: u8) -> &'static str {
    match (profile, index) {
        (0, 0) | (1, 0) | (2, 0) => "VisitedEntries",
        (0, 1) | (1, 1) => "SelectedFiles",
        (0, 2) => "RootPolicyBytes",
        (0, 3) | (1, 2) => "SelectedFileBytes",
        (0, 4) => "TotalSelectedBytes",
        (1, 3) => "PackageTreeBytes",
        (2, 1) => "AcceptedContainers",
        (2, 2) | (10, 4) => "ContainerBytes",
        (2, 3) => "TotalAcceptedBytes",
        (3, 0) => "SourcePages",
        (3, 1) => "Artifacts",
        (3, 2) => "PixelsPerArtifact",
        (3, 3) => "TotalPixels",
        (3, 4) => "ArtifactBytes",
        (3, 5) => "RetainedArtifactBytes",
        (3, 6) => "ExportWorkers",
        (4, 0) | (5, 0) | (7, 0) | (12, 0) => "ArchiveBytes",
        (4, 1) | (5, 1) | (6, 1) => "Members",
        (4, 2) => "GeneratedMemberNameBytes",
        (4, 3) | (5, 3) => "ManifestBytes",
        (4, 4) | (5, 4) | (6, 3) => "MemberBytes",
        (4, 5) | (5, 5) => "TotalContentBytes",
        (5, 2) => "RawMemberNameBytes",
        (6, 0) => "CompressedArchiveBytes",
        (6, 2) => "MemberNameBytes",
        (6, 4) => "TotalExpandedBytes",
        (8, 0) | (9, 0) | (10, 0) | (11, 0) => "ListedEntries",
        (8, 1) | (9, 1) | (10, 1) | (11, 1) => "ListedPathBytes",
        (8, 2) | (9, 2) | (10, 2) | (11, 2) => "TotalListedPathBytes",
        (8, 3) => "SelectedObjects",
        (8, 4) | (9, 4) | (11, 4) => "ObjectBytes",
        (8, 5) | (9, 5) | (10, 5) | (11, 5) => "TotalBytes",
        (9, 3) | (11, 3) => "SelectedFiles",
        (10, 3) => "SelectedContainers",
        (13, 0) => "TreeListedEntries",
        (13, 1) => "TreeListedPathBytes",
        (13, 2) => "TreeTotalListedPathBytes",
        (13, 3) => "TreeSelectedFiles",
        (13, 4) => "TreeObjectBytes",
        (13, 5) => "TreeTotalBytes",
        (13, 6) => "ArchiveBytes",
        _ => "UnknownResource",
    }
}

/// One kind of resource governed by a finite ceiling.
pub trait Resource: Copy + fmt::Debug + Eq + 'static {
    /// The operation whose work this resource describes.
    const OPERATION: &'static str;

    #[doc(hidden)]
    fn index(self) -> usize;
}

impl<const PROFILE: u8> Resource for ResourceKind<PROFILE> {
    const OPERATION: &'static str = operation_name(PROFILE);

    fn index(self) -> usize {
        self.0 as usize
    }
}

/// Validated finite ceilings for the resources used by one operation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Limits<R: Resource> {
    pub(crate) ceilings: [u64; MAX_RESOURCES],
    resource: PhantomData<R>,
}

impl<R: Resource> Limits<R> {
    pub(crate) const fn from_ceilings(ceilings: [u64; MAX_RESOURCES]) -> Self {
        Self {
            ceilings,
            resource: PhantomData,
        }
    }

    pub(crate) fn validate_probe_resources(
        self,
        resources: impl IntoIterator<Item = R>,
    ) -> Result<Self, LimitsError<R>> {
        for resource in resources {
            let ceiling = self.ceiling(resource);
            if ceiling == u64::MAX {
                return Err(LimitsError::CannotProbe { resource, ceiling });
            }
        }
        Ok(self)
    }

    /// Returns the finite ceiling for one resource.
    pub fn ceiling(&self, resource: R) -> u64 {
        self.ceilings[resource.index()]
    }
}

impl<R: Resource> fmt::Debug for Limits<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Limits")
            .field("ceilings", &self.ceilings)
            .finish()
    }
}

/// A supplied ceiling family that cannot support bounded accounting.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum LimitsError<R: Resource> {
    CannotProbe {
        resource: R,
        ceiling: u64,
    },
    ZeroWorkers,
    ObjectBytesExceedTotalBytes {
        object_bytes: u64,
        total_bytes: u64,
    },
    ContainerBytesExceedTotalBytes {
        container_bytes: u64,
        total_bytes: u64,
    },
}

impl<R: Resource> fmt::Display for LimitsError<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CannotProbe {
                resource,
                ceiling: _,
            } => write!(
                formatter,
                "the {resource:?} ceiling must leave room for a plus-one probe"
            ),
            Self::ZeroWorkers => {
                formatter.write_str("the ExportWorkers ceiling must be greater than zero")
            }
            Self::ObjectBytesExceedTotalBytes {
                object_bytes,
                total_bytes,
            } => write!(
                formatter,
                "the ObjectBytes ceiling {object_bytes} exceeds the TotalBytes ceiling {total_bytes}"
            ),
            Self::ContainerBytesExceedTotalBytes {
                container_bytes,
                total_bytes,
            } => write!(
                formatter,
                "the ContainerBytes ceiling {container_bytes} exceeds the TotalBytes ceiling {total_bytes}"
            ),
        }
    }
}

impl<R: Resource> std::error::Error for LimitsError<R> {}

/// A mandatory resource ceiling was exceeded or could not be accounted.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum LimitError<R: Resource> {
    Exceeded {
        resource: R,
        ceiling: u64,
        observed_at_least: u64,
    },
    AccountingOverflow {
        resource: R,
    },
}

impl<R: Resource> LimitError<R> {
    pub(crate) fn exceeded(resource: R, ceiling: u64) -> Self {
        match ceiling.checked_add(1) {
            Some(observed_at_least) => Self::Exceeded {
                resource,
                ceiling,
                observed_at_least,
            },
            None => Self::AccountingOverflow { resource },
        }
    }
}

impl<R: Resource> fmt::Display for LimitError<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exceeded {
                resource,
                ceiling,
                observed_at_least,
            } => write!(
                formatter,
                "{} {resource:?} limit exceeded: ceiling {ceiling}, observed at least {observed_at_least}",
                R::OPERATION
            ),
            Self::AccountingOverflow { resource } => {
                write!(
                    formatter,
                    "{} {resource:?} accounting overflowed",
                    R::OPERATION
                )
            }
        }
    }
}

impl<R: Resource> std::error::Error for LimitError<R> {}
