use std::io::{self, Cursor, Read};

use typst_pack::pack_archive::{
    ReadError, ReadLimitError, ReadLimits, ReadLimitsError, ReadPackError, ReadResource, read,
    read_pack,
};

#[test]
fn reference_v1_profile_bounds_exact_archive_bytes() {
    assert_eq!(
        ReadLimits::reference_v1().archive_bytes(),
        512 * 1024 * 1024
    );
}

#[test]
fn read_limits_reject_an_unprobeable_ceiling() {
    assert!(matches!(
        ReadLimits::new(u64::MAX),
        Err(ReadLimitsError::CannotProbe {
            resource: ReadResource::ArchiveBytes,
            ceiling: u64::MAX,
        })
    ));
}

#[test]
fn stream_read_handles_short_reads_and_preserves_exact_bytes() {
    let mut reader = ChunkedReader::new(b"exact archive bytes", 2);

    let archive = read(&mut reader, ReadLimits::new(19).unwrap()).unwrap();

    assert_eq!(archive.as_slice(), b"exact archive bytes");
    assert!(reader.reads > 1);
}

#[test]
fn read_archive_debug_excludes_payload_bytes() {
    let archive = read(
        Cursor::new(b"secret archive bytes"),
        ReadLimits::reference_v1(),
    )
    .unwrap();

    assert_eq!(format!("{archive:?}"), "PackArchiveBytes(20)");
}

#[test]
fn stream_read_accepts_the_boundary_and_probes_only_one_byte_past_it() {
    let bytes = b"12345";
    for ceiling in [bytes.len() as u64 + 1, bytes.len() as u64] {
        let archive = read(Cursor::new(bytes), ReadLimits::new(ceiling).unwrap()).unwrap();
        assert_eq!(archive.as_slice(), bytes);
    }

    let mut one_over = ChunkedReader::new(b"123456789", 9);
    let error = read(&mut one_over, ReadLimits::new(5).unwrap()).unwrap_err();

    assert!(matches!(
        error,
        ReadError::Limit(ReadLimitError::Exceeded {
            resource: ReadResource::ArchiveBytes,
            ceiling: 5,
            observed_at_least: 6,
        })
    ));
    assert_eq!(one_over.position, 6);
}

#[test]
fn stream_read_returns_a_typed_read_error_without_partial_bytes() {
    let error = read(FailinReader, ReadLimits::new(10).unwrap()).unwrap_err();

    assert!(matches!(error, ReadError::Read(_)));
}

#[test]
fn read_pack_returns_exact_read_bytes_when_decoding_fails() {
    let expected = b"not a Pack Archive";
    let error = read_pack(
        Cursor::new(expected),
        ReadLimits::new(expected.len() as u64).unwrap(),
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
fn file_read_uses_known_size_preflight_and_exact_reads() {
    use typst_pack::pack_archive::{
        FileReadError, FileReadPhase, OpenPackError, open_pack, read_file,
    };

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("archive.typk");
    std::fs::write(&path, b"12345").unwrap();

    let archive = read_file(&path, ReadLimits::new(5).unwrap()).unwrap();
    assert_eq!(archive.as_slice(), b"12345");

    let error = read_file(&path, ReadLimits::new(4).unwrap()).unwrap_err();
    assert_eq!(error.phase(), FileReadPhase::Metadata);
    assert!(matches!(
        error,
        FileReadError::Limit {
            source: ReadLimitError::Exceeded {
                resource: ReadResource::ArchiveBytes,
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
        ReadLimits::new(invalid.len() as u64).unwrap(),
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

struct FailinReader;

impl Read for FailinReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("scripted read failure"))
    }
}
