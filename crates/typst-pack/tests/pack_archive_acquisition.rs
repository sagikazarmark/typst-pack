use std::io::{self, Cursor, Read};

use typst_pack::pack_archive::{
    AcquisitionError, AcquisitionLimitError, AcquisitionLimits, AcquisitionLimitsError,
    AcquisitionResource, ReadPackError, acquire, read_pack,
};

#[test]
fn reference_v1_profile_bounds_exact_archive_bytes() {
    assert_eq!(
        AcquisitionLimits::reference_v1().archive_bytes(),
        512 * 1024 * 1024
    );
}

#[test]
fn acquisition_limits_reject_an_unprobeable_ceiling() {
    assert!(matches!(
        AcquisitionLimits::new(u64::MAX),
        Err(AcquisitionLimitsError::CannotProbe {
            resource: AcquisitionResource::ArchiveBytes,
            ceiling: u64::MAX,
        })
    ));
}

#[test]
fn stream_acquisition_handles_short_reads_and_preserves_exact_bytes() {
    let mut reader = ChunkedReader::new(b"exact archive bytes", 2);

    let archive = acquire(&mut reader, AcquisitionLimits::new(19).unwrap()).unwrap();

    assert_eq!(archive.as_slice(), b"exact archive bytes");
    assert!(reader.reads > 1);
}

#[test]
fn stream_acquisition_accepts_the_boundary_and_probes_only_one_byte_past_it() {
    let bytes = b"12345";
    for ceiling in [bytes.len() as u64 + 1, bytes.len() as u64] {
        let archive =
            acquire(Cursor::new(bytes), AcquisitionLimits::new(ceiling).unwrap()).unwrap();
        assert_eq!(archive.as_slice(), bytes);
    }

    let mut one_over = ChunkedReader::new(b"123456789", 9);
    let error = acquire(&mut one_over, AcquisitionLimits::new(5).unwrap()).unwrap_err();

    assert!(matches!(
        error,
        AcquisitionError::Limit(AcquisitionLimitError::Exceeded {
            resource: AcquisitionResource::ArchiveBytes,
            ceiling: 5,
            observed_at_least: 6,
        })
    ));
    assert_eq!(one_over.position, 6);
}

#[test]
fn stream_acquisition_returns_a_typed_read_error_without_partial_bytes() {
    let error = acquire(FailingReader, AcquisitionLimits::new(10).unwrap()).unwrap_err();

    assert!(matches!(error, AcquisitionError::Read(_)));
}

#[test]
fn read_pack_returns_exact_acquired_bytes_when_decoding_fails() {
    let expected = b"not a Pack Archive";
    let error = read_pack(
        Cursor::new(expected),
        AcquisitionLimits::new(expected.len() as u64).unwrap(),
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap_err();

    let ReadPackError::Decode { archive, .. } = error else {
        panic!("expected a decode failure");
    };
    assert_eq!(archive.as_slice(), expected);
}

#[cfg(feature = "fs")]
#[test]
fn file_acquisition_uses_known_size_preflight_and_exact_reads() {
    use typst_pack::pack_archive::{
        FileAcquisitionError, FileAcquisitionPhase, OpenPackError, acquire_file, open_pack,
    };

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("archive.typk");
    std::fs::write(&path, b"12345").unwrap();

    let archive = acquire_file(&path, AcquisitionLimits::new(5).unwrap()).unwrap();
    assert_eq!(archive.as_slice(), b"12345");

    let error = acquire_file(&path, AcquisitionLimits::new(4).unwrap()).unwrap_err();
    assert_eq!(error.phase(), FileAcquisitionPhase::Metadata);
    assert!(matches!(
        error,
        FileAcquisitionError::Limit {
            source: AcquisitionLimitError::Exceeded {
                resource: AcquisitionResource::ArchiveBytes,
                ceiling: 4,
                observed_at_least: 5,
            },
            ..
        }
    ));

    let invalid = b"not a Pack Archive";
    std::fs::write(&path, invalid).unwrap();
    let error = open_pack(
        &path,
        AcquisitionLimits::new(invalid.len() as u64).unwrap(),
        typst_pack::pack_archive::DecodeLimits::reference_v1(),
    )
    .unwrap_err();
    let OpenPackError::Decode { archive, .. } = error else {
        panic!("expected a decode failure");
    };
    assert_eq!(archive.as_slice(), invalid);
}

struct ChunkedReader<'a> {
    bytes: &'a [u8],
    position: usize,
    chunk: usize,
    reads: usize,
}

impl<'a> ChunkedReader<'a> {
    fn new(bytes: &'a [u8], chunk: usize) -> Self {
        Self {
            bytes,
            position: 0,
            chunk,
            reads: 0,
        }
    }
}

impl Read for ChunkedReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        let length = self
            .chunk
            .min(buffer.len())
            .min(self.bytes.len() - self.position);
        buffer[..length].copy_from_slice(&self.bytes[self.position..self.position + length]);
        self.position += length;
        Ok(length)
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted read failure"))
    }
}
