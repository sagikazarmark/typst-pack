use std::io::{self, Write};

use typst_pack::PackArchiveBytes;
use typst_pack::pack_archive::{
    CommitCertainty, EncodeLimits, StreamWritePhase, WritePackError, write, write_pack_with_limits,
};

#[test]
fn stream_write_handles_short_writes_and_reports_complete_evidence() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(2, None, false);

    let receipt = write(&mut writer, &archive).unwrap();

    assert_eq!(writer.bytes, archive.as_slice());
    assert_eq!(receipt.visible_prefix(), archive.len());
    assert!(writer.flush_called);
}

#[test]
fn stream_write_failure_reports_the_exact_visible_prefix() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(3, Some(6), false);

    let error = write(&mut writer, &archive).unwrap_err();

    assert_eq!(writer.bytes, b"comple");
    assert_eq!(error.phase(), StreamWritePhase::Write);
    assert_eq!(error.visible_prefix(), 6);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(!writer.flush_called);
}

#[test]
fn stream_flush_failure_reports_the_complete_visible_prefix_as_indeterminate() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(usize::MAX, None, true);

    let error = write(&mut writer, &archive).unwrap_err();

    assert_eq!(writer.bytes, archive.as_slice());
    assert_eq!(error.phase(), StreamWritePhase::Flush);
    assert_eq!(error.visible_prefix(), archive.len());
    assert_eq!(error.commit_certainty(), CommitCertainty::Indeterminate);
}

#[test]
fn write_pack_returns_exact_encoded_bytes_for_retry_without_reencoding() {
    let pack = simple_pack();
    let mut failing = ScriptedWriter::new(4, Some(8), false);
    let error =
        write_pack_with_limits(&mut failing, &pack, EncodeLimits::reference_v1()).unwrap_err();

    let WritePackError::Write { archive, source } = error else {
        panic!("expected a write failure");
    };
    assert_eq!(source.visible_prefix(), 8);

    let mut retry = Vec::new();
    write(&mut retry, &archive).unwrap();
    let decoded = typst_pack::pack_archive::decode(
        &archive,
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap();
    assert_eq!(decoded.identity(), pack.identity());
    assert_eq!(retry, archive.as_slice());
}

#[cfg(feature = "fs")]
#[test]
fn file_create_new_writes_complete_bytes_and_never_replaces() {
    use typst_pack::pack_archive::StagingResidueStatus;
    use typst_pack::pack_archive::{FileWritePhase, FileWritePolicy, write_file};

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

    let receipt = write_file(&destination, &archive, FileWritePolicy::CreateNew).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());
    assert_eq!(receipt.destination(), destination);

    let replacement = PackArchiveBytes::from_vec(b"replacement".to_vec());
    let error = write_file(&destination, &replacement, FileWritePolicy::CreateNew).unwrap_err();
    assert_eq!(error.phase(), FileWritePhase::Commit);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());
    let staging = error.staging_residue().expect("retry material on disk");
    assert_eq!(
        error.staging_residue_status(),
        StagingResidueStatus::Present
    );
    assert_eq!(staging.parent(), destination.parent());
    assert_eq!(std::fs::read(staging).unwrap(), replacement.as_slice());
}

#[cfg(feature = "fs")]
#[test]
fn file_replace_requires_an_existing_destination_and_commits_atomically() {
    use typst_pack::pack_archive::{
        FileWritePhase, FileWritePolicy, SavePackError, StagingResidueStatus,
        save_pack_with_limits, write_file,
    };

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

    if cfg!(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))) {
        let error =
            write_file(&destination, &archive, FileWritePolicy::ReplaceExisting).unwrap_err();
        assert_eq!(error.phase(), FileWritePhase::Policy);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert_eq!(error.staging_residue(), None);
        return;
    }

    let missing = write_file(&destination, &archive, FileWritePolicy::ReplaceExisting).unwrap_err();
    assert_eq!(missing.phase(), FileWritePhase::Commit);
    assert_eq!(missing.commit_certainty(), CommitCertainty::NotCommitted);
    assert_eq!(
        missing.staging_residue_status(),
        StagingResidueStatus::Present
    );
    assert!(!destination.exists());
    assert_eq!(
        std::fs::read(missing.staging_residue().unwrap()).unwrap(),
        archive.as_slice()
    );

    std::fs::write(&destination, b"old archive").unwrap();
    write_file(&destination, &archive, FileWritePolicy::ReplaceExisting).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());

    let pack = simple_pack();
    let error = save_pack_with_limits(
        &destination,
        &pack,
        EncodeLimits::reference_v1(),
        FileWritePolicy::CreateNew,
    )
    .unwrap_err();
    let SavePackError::Write { archive, source } = error else {
        panic!("expected a write failure");
    };
    assert_eq!(source.phase(), FileWritePhase::Commit);
    let decoded = typst_pack::pack_archive::decode(
        &archive,
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap();
    assert_eq!(decoded.identity(), pack.identity());
}

fn simple_pack() -> typst_pack::Pack {
    typst_pack::Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap()
}

struct ScriptedWriter {
    bytes: Vec<u8>,
    maximum_write: usize,
    fail_after: Option<usize>,
    fail_flush: bool,
    flush_called: bool,
}

impl ScriptedWriter {
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

impl Write for ScriptedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.fail_after == Some(self.bytes.len()) {
            return Err(io::Error::other("scripted write failure"));
        }
        let before_failure = self
            .fail_after
            .map_or(usize::MAX, |limit| limit - self.bytes.len());
        let written = buffer.len().min(self.maximum_write).min(before_failure);
        if written == 0 && !buffer.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "scripted zero write",
            ));
        }
        self.bytes.extend_from_slice(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_called = true;
        if self.fail_flush {
            return Err(io::Error::other("scripted flush failure"));
        }
        Ok(())
    }
}
