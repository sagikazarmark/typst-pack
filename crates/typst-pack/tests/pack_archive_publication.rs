use std::io::{self, Write};

use typst_pack::PackArchiveBytes;
use typst_pack::pack_archive::{
    CommitCertainty, EncodeLimits, StreamPublicationPhase, WritePackError, publish, write_pack,
};

#[test]
fn stream_publication_handles_short_writes_and_reports_complete_evidence() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(2, None, false);

    let receipt = publish(&mut writer, &archive).unwrap();

    assert_eq!(writer.bytes, archive.as_slice());
    assert_eq!(receipt.phase(), StreamPublicationPhase::Complete);
    assert_eq!(receipt.visible_prefix(), archive.len());
    assert_eq!(receipt.commit_certainty(), CommitCertainty::Committed);
    assert!(writer.flush_called);
}

#[test]
fn stream_write_failure_reports_the_exact_visible_prefix() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(3, Some(6), false);

    let error = publish(&mut writer, &archive).unwrap_err();

    assert_eq!(writer.bytes, b"comple");
    assert_eq!(error.phase(), StreamPublicationPhase::Write);
    assert_eq!(error.visible_prefix(), 6);
    assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
    assert!(!writer.flush_called);
}

#[test]
fn stream_flush_failure_reports_the_complete_visible_prefix_as_indeterminate() {
    let archive = PackArchiveBytes::from_vec(b"complete archive".to_vec());
    let mut writer = ScriptedWriter::new(usize::MAX, None, true);

    let error = publish(&mut writer, &archive).unwrap_err();

    assert_eq!(writer.bytes, archive.as_slice());
    assert_eq!(error.phase(), StreamPublicationPhase::Flush);
    assert_eq!(error.visible_prefix(), archive.len());
    assert_eq!(error.commit_certainty(), CommitCertainty::Indeterminate);
}

#[test]
fn write_pack_returns_exact_encoded_bytes_for_retry_without_reencoding() {
    let pack = simple_pack();
    let mut failing = ScriptedWriter::new(4, Some(8), false);
    let error = write_pack(&mut failing, &pack, EncodeLimits::reference_v1()).unwrap_err();

    let WritePackError::Publish { archive, source } = error else {
        panic!("expected a publication failure");
    };
    assert_eq!(source.visible_prefix(), 8);

    let mut retry = Vec::new();
    publish(&mut retry, &archive).unwrap();
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
fn file_create_new_publishes_complete_bytes_and_never_replaces() {
    use typst_pack::pack_archive::StagingResidueStatus;
    use typst_pack::pack_archive::{FilePublicationPhase, FilePublicationPolicy, publish_file};

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

    let receipt = publish_file(&destination, &archive, FilePublicationPolicy::CreateNew).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());
    assert_eq!(receipt.phase(), FilePublicationPhase::Complete);
    assert_eq!(receipt.destination(), destination);
    assert_eq!(receipt.commit_certainty(), CommitCertainty::Committed);
    assert_eq!(receipt.staging_residue(), None);

    let replacement = PackArchiveBytes::from_vec(b"replacement".to_vec());
    let error =
        publish_file(&destination, &replacement, FilePublicationPolicy::CreateNew).unwrap_err();
    assert_eq!(error.phase(), FilePublicationPhase::Commit);
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
        FilePublicationPhase, FilePublicationPolicy, SavePackError, StagingResidueStatus,
        publish_file, save_pack,
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
        let error = publish_file(
            &destination,
            &archive,
            FilePublicationPolicy::ReplaceExisting,
        )
        .unwrap_err();
        assert_eq!(error.phase(), FilePublicationPhase::Policy);
        assert_eq!(error.commit_certainty(), CommitCertainty::NotCommitted);
        assert_eq!(error.staging_residue_status(), StagingResidueStatus::Absent);
        assert_eq!(error.staging_residue(), None);
        return;
    }

    let missing = publish_file(
        &destination,
        &archive,
        FilePublicationPolicy::ReplaceExisting,
    )
    .unwrap_err();
    assert_eq!(missing.phase(), FilePublicationPhase::Commit);
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
    let receipt = publish_file(
        &destination,
        &archive,
        FilePublicationPolicy::ReplaceExisting,
    )
    .unwrap();
    assert_eq!(receipt.commit_certainty(), CommitCertainty::Committed);
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());

    let pack = simple_pack();
    let error = save_pack(
        &destination,
        &pack,
        EncodeLimits::reference_v1(),
        FilePublicationPolicy::CreateNew,
    )
    .unwrap_err();
    let SavePackError::Publish { archive, source } = error else {
        panic!("expected a publication failure");
    };
    assert_eq!(source.phase(), FilePublicationPhase::Commit);
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
