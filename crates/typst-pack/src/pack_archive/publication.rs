use std::io::{self, Write};

#[cfg(feature = "fs")]
use std::path::{Path, PathBuf};
#[cfg(feature = "fs")]
use std::sync::atomic::{AtomicU64, Ordering};

use super::{EncodeError, EncodeLimits, encode_with_limits};
use crate::{CommitCertainty, Pack, PackArchiveBytes};

/// The stream publication phase reached by an attempt.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StreamPublicationPhase {
    Write,
    Flush,
    Complete,
}

/// Evidence from successful exact stream publication.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StreamPublicationReceipt {
    visible_prefix: u64,
}

impl StreamPublicationReceipt {
    pub const fn visible_prefix(&self) -> u64 {
        self.visible_prefix
    }
}

/// Evidence and the concrete cause from failed exact stream publication.
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive stream publication failed during {phase:?} after a {visible_prefix}-byte visible prefix: {source}"
)]
pub struct StreamPublicationError {
    phase: StreamPublicationPhase,
    visible_prefix: u64,
    commit_certainty: CommitCertainty,
    #[source]
    source: io::Error,
}

impl StreamPublicationError {
    pub const fn phase(&self) -> StreamPublicationPhase {
        self.phase
    }

    pub const fn visible_prefix(&self) -> u64 {
        self.visible_prefix
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub const fn io_error(&self) -> &io::Error {
        &self.source
    }
}

/// Publishes exact Pack Archive bytes to a stream and flushes the writer.
pub fn publish(
    mut writer: impl Write,
    archive: &PackArchiveBytes,
) -> Result<StreamPublicationReceipt, StreamPublicationError> {
    let mut visible_prefix = 0usize;
    while visible_prefix < archive.as_slice().len() {
        match writer.write(&archive.as_slice()[visible_prefix..]) {
            Ok(0) => {
                return Err(StreamPublicationError {
                    phase: StreamPublicationPhase::Write,
                    visible_prefix: visible_prefix as u64,
                    commit_certainty: CommitCertainty::NotCommitted,
                    source: io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to publish the complete Pack Archive",
                    ),
                });
            }
            Ok(written) => visible_prefix += written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => {
                return Err(StreamPublicationError {
                    phase: StreamPublicationPhase::Write,
                    visible_prefix: visible_prefix as u64,
                    commit_certainty: CommitCertainty::NotCommitted,
                    source,
                });
            }
        }
    }
    if let Err(source) = writer.flush() {
        return Err(StreamPublicationError {
            phase: StreamPublicationPhase::Flush,
            visible_prefix: visible_prefix as u64,
            commit_certainty: CommitCertainty::Indeterminate,
            source,
        });
    }
    Ok(StreamPublicationReceipt {
        visible_prefix: visible_prefix as u64,
    })
}

/// A failure in Pack Archive Encoding followed by stream publication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WritePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("encoded Pack Archive could not be published: {source}")]
    Publish {
        archive: PackArchiveBytes,
        #[source]
        source: StreamPublicationError,
    },
}

/// Encodes and publishes one Pack while preserving exact bytes on publication failure.
pub fn write_pack(
    writer: impl Write,
    pack: &Pack,
) -> Result<StreamPublicationReceipt, WritePackError> {
    write_pack_with_limits(writer, pack, EncodeLimits::reference_v1())
}

/// Encodes under explicit resource ceilings and publishes one Pack.
pub fn write_pack_with_limits(
    writer: impl Write,
    pack: &Pack,
    encode_limits: EncodeLimits,
) -> Result<StreamPublicationReceipt, WritePackError> {
    let archive = encode_with_limits(pack, encode_limits)?;
    match publish(writer, &archive) {
        Ok(receipt) => Ok(receipt),
        Err(source) => Err(WritePackError::Publish { archive, source }),
    }
}

/// The strict atomic policy requested for filesystem publication.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilePublicationPolicy {
    CreateNew,
    ReplaceExisting,
}

/// The filesystem publication phase reached by an attempt.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilePublicationPhase {
    Policy,
    StagingCreate,
    StagingWrite,
    StagingFlush,
    Commit,
    StagingCleanup,
    Complete,
}

/// The observed state of same-directory staging when an attempt returns.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StagingResidueStatus {
    Absent,
    Present,
    Indeterminate,
}

/// The concrete cause of a filesystem publication failure.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilePublicationErrorCause {
    #[error("the platform cannot guarantee the requested {0:?} publication policy")]
    UnsupportedPolicy(FilePublicationPolicy),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Evidence from successful atomic filesystem publication.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FilePublicationReceipt {
    destination: PathBuf,
    policy: FilePublicationPolicy,
    byte_length: u64,
}

#[cfg(feature = "fs")]
impl FilePublicationReceipt {
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub const fn policy(&self) -> FilePublicationPolicy {
        self.policy
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

/// Evidence and the concrete cause from failed atomic filesystem publication.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[error(
    "Pack Archive publication to {destination:?} failed during {phase:?} with {commit_certainty:?} certainty: {source}"
)]
pub struct FilePublicationError {
    destination: PathBuf,
    policy: FilePublicationPolicy,
    byte_length: u64,
    phase: FilePublicationPhase,
    staging_residue: Option<PathBuf>,
    staging_residue_status: StagingResidueStatus,
    commit_certainty: CommitCertainty,
    #[source]
    source: FilePublicationErrorCause,
}

#[cfg(feature = "fs")]
impl FilePublicationError {
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub const fn policy(&self) -> FilePublicationPolicy {
        self.policy
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn phase(&self) -> FilePublicationPhase {
        self.phase
    }

    pub fn staging_residue(&self) -> Option<&Path> {
        self.staging_residue.as_deref()
    }

    pub const fn staging_residue_status(&self) -> StagingResidueStatus {
        self.staging_residue_status
    }

    pub const fn commit_certainty(&self) -> CommitCertainty {
        self.commit_certainty
    }

    pub const fn cause(&self) -> &FilePublicationErrorCause {
        &self.source
    }
}

#[cfg(feature = "fs")]
fn publication_error(
    destination: &Path,
    policy: FilePublicationPolicy,
    byte_length: u64,
    phase: FilePublicationPhase,
    staging_residue: Option<PathBuf>,
    commit_certainty: CommitCertainty,
    source: impl Into<FilePublicationErrorCause>,
) -> FilePublicationError {
    let staging_residue_status = staging_residue
        .as_deref()
        .map(observe_staging_residue)
        .unwrap_or(StagingResidueStatus::Absent);
    let staging_residue = if staging_residue_status == StagingResidueStatus::Absent {
        None
    } else {
        staging_residue
    };
    FilePublicationError {
        destination: destination.to_owned(),
        policy,
        byte_length,
        phase,
        staging_residue,
        staging_residue_status,
        commit_certainty,
        source: source.into(),
    }
}

/// Atomically publishes exact Pack Archive bytes from same-directory staging.
#[cfg(feature = "fs")]
pub fn publish_file(
    destination: impl AsRef<Path>,
    archive: &PackArchiveBytes,
    policy: FilePublicationPolicy,
) -> Result<FilePublicationReceipt, FilePublicationError> {
    publish_file_before_commit(destination.as_ref(), archive, policy, |_| {})
}

#[cfg(feature = "fs")]
fn publish_file_before_commit(
    destination: &Path,
    archive: &PackArchiveBytes,
    policy: FilePublicationPolicy,
    before_commit: impl FnOnce(&Path),
) -> Result<FilePublicationReceipt, FilePublicationError> {
    let byte_length = archive.len();
    if policy == FilePublicationPolicy::ReplaceExisting && !replace_existing_supported() {
        return Err(publication_error(
            destination,
            policy,
            byte_length,
            FilePublicationPhase::Policy,
            None,
            CommitCertainty::NotCommitted,
            FilePublicationErrorCause::UnsupportedPolicy(policy),
        ));
    }

    let (staging, mut file) = create_staging(destination).map_err(|source| {
        publication_error(
            destination,
            policy,
            byte_length,
            FilePublicationPhase::StagingCreate,
            None,
            CommitCertainty::NotCommitted,
            source,
        )
    })?;
    if let Err((phase, source)) = write_staging(&mut file, archive.as_slice()) {
        drop(file);
        return Err(publication_error(
            destination,
            policy,
            byte_length,
            phase,
            Some(staging),
            CommitCertainty::NotCommitted,
            source,
        ));
    }
    drop(file);
    before_commit(&staging);

    if let Err(source) = commit_staging(&staging, destination, policy) {
        return Err(publication_error(
            destination,
            policy,
            byte_length,
            FilePublicationPhase::Commit,
            Some(staging),
            CommitCertainty::NotCommitted,
            source,
        ));
    }
    if let Err(source) = std::fs::remove_file(&staging)
        && source.kind() != io::ErrorKind::NotFound
    {
        return Err(publication_error(
            destination,
            policy,
            byte_length,
            FilePublicationPhase::StagingCleanup,
            Some(staging),
            CommitCertainty::Committed,
            source,
        ));
    }

    Ok(FilePublicationReceipt {
        destination: destination.to_owned(),
        policy,
        byte_length,
    })
}

#[cfg(feature = "fs")]
fn create_staging(destination: &Path) -> io::Result<(PathBuf, std::fs::File)> {
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
    const ATTEMPTS: usize = 128;

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name")
    })?;
    for _ in 0..ATTEMPTS {
        let sequence = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let mut staging_name = file_name.to_os_string();
        staging_name.push(format!(
            ".typst-pack-stage-{}-{sequence}",
            std::process::id()
        ));
        let staging = parent.join(staging_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
        {
            Ok(file) => return Ok((staging, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique same-directory staging file",
    ))
}

#[cfg(feature = "fs")]
fn observe_staging_residue(staging: &Path) -> StagingResidueStatus {
    match std::fs::symlink_metadata(staging) {
        Ok(_) => StagingResidueStatus::Present,
        Err(error) if error.kind() == io::ErrorKind::NotFound => StagingResidueStatus::Absent,
        Err(_) => StagingResidueStatus::Indeterminate,
    }
}

#[cfg(feature = "fs")]
fn write_staging(
    writer: &mut impl Write,
    bytes: &[u8],
) -> Result<(), (FilePublicationPhase, io::Error)> {
    let mut written = 0;
    while written < bytes.len() {
        match writer.write(&bytes[written..]) {
            Ok(0) => {
                return Err((
                    FilePublicationPhase::StagingWrite,
                    io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write complete staging file",
                    ),
                ));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err((FilePublicationPhase::StagingWrite, error)),
        }
    }
    writer
        .flush()
        .map_err(|error| (FilePublicationPhase::StagingFlush, error))
}

#[cfg(feature = "fs")]
fn commit_staging(
    staging: &Path,
    destination: &Path,
    policy: FilePublicationPolicy,
) -> io::Result<()> {
    match policy {
        FilePublicationPolicy::CreateNew => std::fs::hard_link(staging, destination),
        FilePublicationPolicy::ReplaceExisting => replace_existing(staging, destination),
    }
}

#[cfg(feature = "fs")]
const fn replace_existing_supported() -> bool {
    cfg!(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
}

#[cfg(all(feature = "fs", any(target_os = "linux", target_os = "android")))]
fn replace_existing(staging: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let staging = CString::new(staging.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // RENAME_EXCHANGE atomically requires both paths and leaves the old target at staging.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            staging.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(all(feature = "fs", any(target_os = "macos", target_os = "ios")))]
fn replace_existing(staging: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let staging = CString::new(staging.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // RENAME_SWAP atomically requires both paths and leaves the old target at staging.
    let result =
        unsafe { libc::renamex_np(staging.as_ptr(), destination.as_ptr(), libc::RENAME_SWAP) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(all(
    feature = "fs",
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
))]
fn replace_existing(_staging: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "strict replace-existing publication is unsupported",
    ))
}

/// A failure in Pack Archive Encoding followed by atomic file publication.
#[cfg(feature = "fs")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SavePackError {
    #[error(transparent)]
    Encode(#[from] EncodeError),
    #[error("encoded Pack Archive could not be published: {source}")]
    Publish {
        archive: PackArchiveBytes,
        #[source]
        source: FilePublicationError,
    },
}

/// Encodes and atomically publishes one Pack while preserving bytes on failure.
#[cfg(feature = "fs")]
pub fn save_pack(
    destination: impl AsRef<Path>,
    pack: &Pack,
    policy: FilePublicationPolicy,
) -> Result<FilePublicationReceipt, SavePackError> {
    save_pack_with_limits(destination, pack, EncodeLimits::reference_v1(), policy)
}

/// Encodes under explicit resource ceilings and atomically publishes one Pack.
#[cfg(feature = "fs")]
pub fn save_pack_with_limits(
    destination: impl AsRef<Path>,
    pack: &Pack,
    encode_limits: EncodeLimits,
    policy: FilePublicationPolicy,
) -> Result<FilePublicationReceipt, SavePackError> {
    let archive = encode_with_limits(pack, encode_limits)?;
    match publish_file(destination, &archive, policy) {
        Ok(receipt) => Ok(receipt),
        Err(source) => Err(SavePackError::Publish { archive, source }),
    }
}

#[cfg(all(test, feature = "fs"))]
mod tests {
    use super::*;

    #[test]
    fn create_new_destination_race_does_not_replace_the_racing_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("archive.typk");
        let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

        let error = publish_file_before_commit(
            &destination,
            &archive,
            FilePublicationPolicy::CreateNew,
            |_| std::fs::write(&destination, b"racing archive").unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.phase(), FilePublicationPhase::Commit);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(std::fs::read(&destination).unwrap(), b"racing archive");
        assert_eq!(
            std::fs::read(error.staging_residue().unwrap()).unwrap(),
            archive.as_slice()
        );
    }

    #[test]
    fn replace_existing_destination_race_does_not_create_a_new_file() {
        if !replace_existing_supported() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("archive.typk");
        std::fs::write(&destination, b"old archive").unwrap();
        let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

        let error = publish_file_before_commit(
            &destination,
            &archive,
            FilePublicationPolicy::ReplaceExisting,
            |_| std::fs::remove_file(&destination).unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.phase(), FilePublicationPhase::Commit);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert!(!destination.exists());
        assert_eq!(
            std::fs::read(error.staging_residue().unwrap()).unwrap(),
            archive.as_slice()
        );
    }

    #[test]
    fn commit_failure_reports_staging_removed_by_a_race_as_absent() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("archive.typk");
        let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

        let error = publish_file_before_commit(
            &destination,
            &archive,
            FilePublicationPolicy::CreateNew,
            |staging| std::fs::remove_file(staging).unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.phase(), FilePublicationPhase::Commit);
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert_eq!(error.staging_residue(), None);
        assert!(!destination.exists());
    }

    #[test]
    fn staging_write_handles_short_writes() {
        let mut writer = FaultWriter::new(2, None, false);

        write_staging(&mut writer, b"complete staging bytes").unwrap();

        assert_eq!(writer.bytes, b"complete staging bytes");
        assert!(writer.flush_called);
    }

    #[test]
    fn staging_write_and_flush_faults_keep_their_phase() {
        let mut write_failure = FaultWriter::new(3, Some(6), false);
        let (phase, _) = write_staging(&mut write_failure, b"complete staging bytes").unwrap_err();
        assert_eq!(phase, FilePublicationPhase::StagingWrite);
        assert_eq!(write_failure.bytes, b"comple");

        let mut flush_failure = FaultWriter::new(usize::MAX, None, true);
        let (phase, _) = write_staging(&mut flush_failure, b"complete staging bytes").unwrap_err();
        assert_eq!(phase, FilePublicationPhase::StagingFlush);
        assert_eq!(flush_failure.bytes, b"complete staging bytes");
    }

    struct FaultWriter {
        bytes: Vec<u8>,
        maximum_write: usize,
        fail_after: Option<usize>,
        fail_flush: bool,
        flush_called: bool,
    }

    impl FaultWriter {
        fn new(maximum_write: usize, fail_after: Option<usize>, fail_flush: bool) -> Self {
            Self {
                bytes: Vec::new(),
                maximum_write,
                fail_after,
                fail_flush,
                flush_called: false,
            }
        }
    }

    impl Write for FaultWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.fail_after == Some(self.bytes.len()) {
                return Err(io::Error::other("scripted staging write failure"));
            }
            let before_failure = self
                .fail_after
                .map_or(usize::MAX, |limit| limit - self.bytes.len());
            let written = buffer.len().min(self.maximum_write).min(before_failure);
            self.bytes.extend_from_slice(&buffer[..written]);
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_called = true;
            if self.fail_flush {
                return Err(io::Error::other("scripted staging flush failure"));
            }
            Ok(())
        }
    }
}
