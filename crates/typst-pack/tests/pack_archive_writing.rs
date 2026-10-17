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
fn directory_names(directory: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut names = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[cfg(feature = "fs")]
#[test]
fn file_create_new_writes_complete_bytes_and_never_replaces() {
    use typst_pack::pack_archive::write_file;
    use typst_pack::{FilesystemWriteError, WritePolicy};

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

    write_file(&destination, &archive, WritePolicy::CreateNew).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());

    let replacement = PackArchiveBytes::from_vec(b"replacement".to_vec());
    let error = write_file(&destination, &replacement, WritePolicy::CreateNew).unwrap_err();
    let FilesystemWriteError::Io { path, source, .. } = error else {
        panic!("expected an I/O failure");
    };
    assert_eq!(path, destination);
    assert_eq!(source.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());
    // No temporary file is left behind.
    assert_eq!(directory_names(directory.path()), ["archive.typk"]);
}

#[cfg(feature = "fs")]
#[test]
fn file_overwrite_creates_or_replaces_on_every_platform() {
    use typst_pack::pack_archive::{SavePackError, save_pack, save_pack_with_limits, write_file};
    use typst_pack::{FilesystemWriteError, WritePolicy};

    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("archive.typk");
    let archive = PackArchiveBytes::from_vec(b"new archive".to_vec());

    write_file(&destination, &archive, WritePolicy::Overwrite).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), archive.as_slice());

    std::fs::write(&destination, b"old archive").unwrap();
    let pack = simple_pack();
    save_pack(&destination, &pack, WritePolicy::Overwrite).unwrap();
    let saved = typst_pack::pack_archive::read_file(
        &destination,
        typst_pack::pack_archive::ReadLimits::reference_v1(),
    )
    .unwrap();
    let decoded = typst_pack::pack_archive::decode(
        &saved,
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap();
    assert_eq!(decoded.identity(), pack.identity());
    assert_eq!(directory_names(directory.path()), ["archive.typk"]);

    // A failed write keeps the encoded bytes for a retry.
    let error = save_pack_with_limits(
        &destination,
        &pack,
        EncodeLimits::reference_v1(),
        WritePolicy::CreateNew,
    )
    .unwrap_err();
    let SavePackError::Write { archive, source } = error else {
        panic!("expected a write failure");
    };
    assert!(matches!(source, FilesystemWriteError::Io { .. }));
    assert_eq!(archive.as_slice(), saved.as_slice());
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
