//! Crate tests.

use crate::compile::{CompileError, compile as compile_request, compile_world as compile};
#[cfg(feature = "fs")]
use crate::packer::DiscoveryWorld;
use crate::world::PackWorld;
use crate::*;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use typst::World;
use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use typst_kit::files::FileLoader;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
}

fn test_package_declaration(files: &[(&str, &[u8])]) -> PackageManifest {
    let spec: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let mut builder = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap();
    for (path, data) in files {
        builder = builder
            .package_file(spec.clone(), path, data.to_vec())
            .unwrap();
    }
    builder.build().unwrap().manifest().packages().vendored()[0].clone()
}

fn test_package_manifest(
    vendored: Vec<PackageManifest>,
    unvendored: Vec<PackageManifest>,
) -> Vec<u8> {
    PackManifest::new(
        "main.typ".to_owned(),
        BTreeSet::new(),
        vendored,
        unvendored,
        vec![],
        vec![],
        None,
    )
    .to_toml()
    .into_bytes()
}

#[test]
fn png_export_error_preserves_the_failing_source_page() {
    let error = CompileError::PngExport {
        message: "encoding failed".to_owned(),
        warnings: ecow::EcoVec::new(),
        pack_warnings: ecow::EcoVec::new(),
        source_page_count: 3,
        source_page_number: std::num::NonZeroUsize::new(2).unwrap(),
    };

    assert_eq!(
        error.to_string(),
        "PNG export failed for source page 2: encoding failed"
    );
    let CompileError::PngExport {
        source_page_count,
        source_page_number,
        ..
    } = error
    else {
        panic!("expected a PNG export error");
    };
    assert_eq!(source_page_count, 3);
    assert_eq!(source_page_number.get(), 2);
}

#[cfg(feature = "embedded-fonts")]
fn embedded_font_data() -> Vec<u8> {
    typst_kit::fonts::embedded()
        .next()
        .expect("Typst embedded fonts are available")
        .0
        .data()
        .to_vec()
}

#[cfg(feature = "embedded-fonts")]
fn two_face_collection(font: &[u8]) -> Vec<u8> {
    fn adjusted_font(font: &[u8], base: usize) -> Vec<u8> {
        let mut adjusted = font.to_vec();
        let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        for table in 0..table_count {
            let offset = 12 + table * 16 + 8;
            let original = u32::from_be_bytes(font[offset..offset + 4].try_into().unwrap());
            let adjusted_offset = original + u32::try_from(base).unwrap();
            adjusted[offset..offset + 4].copy_from_slice(&adjusted_offset.to_be_bytes());
        }
        adjusted
    }

    let first_offset = 20;
    let second_offset = (first_offset + font.len() + 3) & !3;
    let mut collection = Vec::with_capacity(second_offset + font.len());
    collection.extend_from_slice(b"ttcf");
    collection.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    collection.extend_from_slice(&2u32.to_be_bytes());
    collection.extend_from_slice(&u32::try_from(first_offset).unwrap().to_be_bytes());
    collection.extend_from_slice(&u32::try_from(second_offset).unwrap().to_be_bytes());
    collection.extend_from_slice(&adjusted_font(font, first_offset));
    collection.resize(second_offset, 0);
    collection.extend_from_slice(&adjusted_font(font, second_offset));
    collection
}

struct MemoryProjectFile {
    path: String,
    data: Bytes,
    calls: Arc<AtomicUsize>,
}

impl MemoryProjectFile {
    fn new(path: &str, data: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.to_owned(),
            data: Bytes::new(data.into()),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn tracked(path: &str, data: impl Into<Vec<u8>>) -> (Self, Arc<AtomicUsize>) {
        let loader = Self::new(path, data);
        let calls = Arc::clone(&loader.calls);
        (loader, calls)
    }
}

impl FileLoader for MemoryProjectFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project) && path == self.path {
            Ok(self.data.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
    }
}

struct MutableProjectFile {
    path: String,
    data: Arc<Mutex<Bytes>>,
}

impl FileLoader for MutableProjectFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project) && path == self.path {
            Ok(self
                .data
                .lock()
                .expect("mutable provider lock poisoned")
                .clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
    }
}

struct ErrorProjectLoader {
    error: FileError,
    calls: Arc<AtomicUsize>,
}

struct BlockingProjectFiles {
    paths: Vec<String>,
    data: Bytes,
    entered: mpsc::Sender<String>,
    release: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl FileLoader for BlockingProjectFiles {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project)
            && self.paths.iter().any(|candidate| candidate == path)
        {
            self.entered
                .send(path.to_owned())
                .map_err(|_| FileError::Other(Some("test entry receiver was dropped".into())))?;
            self.release
                .lock()
                .expect("test release lock poisoned")
                .recv_timeout(TEST_SYNC_TIMEOUT)
                .map_err(|_| FileError::Other(Some("timed out waiting for test release".into())))?;
            Ok(self.data.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
    }
}

const TEST_SYNC_TIMEOUT: Duration = Duration::from_secs(5);

struct ReleaseGuard {
    sender: mpsc::Sender<()>,
    remaining: usize,
}

impl ReleaseGuard {
    fn new(sender: mpsc::Sender<()>, remaining: usize) -> Self {
        Self { sender, remaining }
    }

    fn release_all(&mut self) {
        while self.remaining > 0 {
            let _ = self.sender.send(());
            self.remaining -= 1;
        }
    }
}

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.release_all();
    }
}

impl ErrorProjectLoader {
    fn tracked(error: FileError) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Self {
                error,
                calls: Arc::clone(&calls),
            },
            calls,
        )
    }
}

impl FileLoader for ErrorProjectLoader {
    fn load(&self, _id: FileId) -> FileResult<Bytes> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(self.error.clone())
    }
}

fn project_file_id(path: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).unwrap()).intern()
}

fn raw_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let entries = entries
        .iter()
        .map(|(name, data)| (name.as_bytes(), false, *data))
        .collect::<Vec<_>>();
    raw_stored_zip_with_raw_names(&entries)
}

fn raw_stored_zip_with_raw_names(entries: &[(&[u8], bool, &[u8])]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    fn u16_bytes(value: usize) -> [u8; 2] {
        u16::try_from(value).unwrap().to_le_bytes()
    }

    fn u32_bytes(value: usize) -> [u8; 4] {
        u32::try_from(value).unwrap().to_le_bytes()
    }

    let mut archive = Vec::new();
    let mut central_entries = Vec::new();
    for &(name, utf8, data) in entries {
        let offset = archive.len();
        let crc = crc32(data);
        let flags: u16 = if utf8 { 1 << 11 } else { 0 };
        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&flags.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(data.len()));
        archive.extend_from_slice(&u32_bytes(data.len()));
        archive.extend_from_slice(&u16_bytes(name.len()));
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name);
        archive.extend_from_slice(data);
        central_entries.push((name, flags, data.len(), crc, offset));
    }

    let central_start = archive.len();
    for (name, flags, size, crc, offset) in central_entries {
        archive.extend_from_slice(b"PK\x01\x02");
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&flags.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(size));
        archive.extend_from_slice(&u32_bytes(size));
        archive.extend_from_slice(&u16_bytes(name.len()));
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u32.to_le_bytes());
        archive.extend_from_slice(&u32_bytes(offset));
        archive.extend_from_slice(name);
    }
    let central_size = archive.len() - central_start;
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&u16_bytes(entries.len()));
    archive.extend_from_slice(&u16_bytes(entries.len()));
    archive.extend_from_slice(&u32_bytes(central_size));
    archive.extend_from_slice(&u32_bytes(central_start));
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive
}

fn with_first_zip_entry_unix_mode(mut archive: Vec<u8>, mode: u32) -> Vec<u8> {
    let eocd = archive.len() - 22;
    let central_start =
        u32::from_le_bytes(archive[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    archive[central_start + 4..central_start + 6]
        .copy_from_slice(&((3u16 << 8) | 20).to_le_bytes());
    archive[central_start + 38..central_start + 42].copy_from_slice(&(mode << 16).to_le_bytes());
    archive
}

fn with_first_zip_entry_corrupt_data(mut archive: Vec<u8>) -> Vec<u8> {
    let name_len = u16::from_le_bytes(archive[26..28].try_into().unwrap()) as usize;
    let extra_len = u16::from_le_bytes(archive[28..30].try_into().unwrap()) as usize;
    archive[30 + name_len + extra_len] ^= 1;
    archive
}

#[test]
fn manifest_roundtrip() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"
        resource-slots = ["logo.png"]

        [packages]
        vendored = [{ spec = "@preview/cetz:0.3.4", tree-digest = "00000000000000000000000000000001", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 }]
        unvendored = [{ spec = "@preview/tablex:0.0.9", tree-digest = "00000000000000000000000000000002", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 }]

        [[fonts]]
        path = "fonts/test.ttf"
        families = ["Test"]

        [metadata]
        name = "Test pack"
        "#,
    )
    .unwrap();
    assert_eq!(manifest.project().entrypoint(), "main.typ");
    assert_eq!(manifest.vendored_packages().len(), 1);
    assert_eq!(manifest.unvendored_packages().len(), 1);

    let serialized = manifest.to_toml();
    assert!(serialized.contains("resource-slots ="));
    assert!(serialized.contains("tree-digest ="));
    let reparsed = PackManifest::from_toml(&serialized).unwrap();
    assert_eq!(manifest, reparsed);
}

#[test]
fn manifest_rejects_conflicting_package_requirements_for_one_spec() {
    let manifest = r#"
        format-version = 1
        [project]
        entrypoint = "main.typ"
        [packages]
        unvendored = [
          { spec = "@local/example:1.0.0", tree-digest = "00000000000000000000000000000001", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 },
          { spec = "@local/example:1.0.0", tree-digest = "00000000000000000000000000000002", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 },
        ]
    "#;

    assert!(matches!(
        PackManifest::from_toml(manifest),
        Err(PackManifestError::ConflictingPackageRequirements { ref spec })
            if spec == "@local/example:1.0.0"
    ));
}

#[test]
fn manifest_rejects_legacy_version_one_field_names() {
    for manifest in [
        "format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"logo.png\"]\n",
        "format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[packages]\nexternal = [\"@preview/tablex:0.0.9\"]\n",
    ] {
        assert!(matches!(
            PackManifest::from_toml(manifest),
            Err(PackManifestError::Parse(_))
        ));
    }
}

#[test]
fn manifest_declarations_are_exposed_read_only_through_accessors() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"
        resource-slots = ["logo.png"]

        [packages]
        vendored = [{ spec = "@preview/cetz:0.3.4", tree-digest = "00000000000000000000000000000001", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 }]
        unvendored = [{ spec = "@preview/tablex:0.0.9", tree-digest = "00000000000000000000000000000002", tree-identity-kind = "complete-package-tree", tree-identity-schema = "typst-pack-complete-package-tree-v1", tree-identity-algorithm = "typst-hash128-0.15", file-count = 1, byte-length = 1 }]

        [[fonts]]
        path = "fonts/test.ttf"
        index = 2
        families = ["Test"]

        [metadata]
        name = "Test pack"
        authors = ["A. U. Thor"]
        "#,
    )
    .unwrap();

    assert_eq!(manifest.format_version(), 1);
    assert_eq!(manifest.project().entrypoint(), "main.typ");
    assert_eq!(
        manifest.project().resource_slots().collect::<Vec<_>>(),
        ["logo.png"]
    );
    let vendored = "@preview/cetz:0.3.4"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let unvendored = "@preview/tablex:0.0.9"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    assert_eq!(manifest.packages().vendored()[0].spec().unwrap(), vendored);
    assert_eq!(
        manifest.packages().unvendored()[0].spec().unwrap(),
        unvendored
    );
    assert_eq!(manifest.fonts()[0].path(), "fonts/test.ttf");
    assert_eq!(manifest.fonts()[0].index(), 2);
    assert_eq!(manifest.fonts()[0].families(), ["Test"]);
    assert_eq!(manifest.metadata().unwrap().name(), Some("Test pack"));
    assert_eq!(manifest.metadata().unwrap().authors(), ["A. U. Thor"]);
}

#[test]
fn manifest_defaults_to_no_resource_slots() {
    let manifest =
        PackManifest::from_toml("format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
            .unwrap();

    assert!(manifest.project().resource_slots().next().is_none());
}

#[test]
fn resource_slot_invariants_have_archive_and_builder_error_wrappers() {
    let builder_error = Pack::builder("main.typ")
        .resource_slot("../secret.png")
        .unwrap_err();
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nresource-slots = [\"../secret.png\"]\n";
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]);
    let archive_error = Pack::from_bytes(bytes).unwrap_err();

    let PackBuildError::Invariant(builder_invariant) = builder_error else {
        panic!("builder did not report a Pack invariant: {builder_error}");
    };
    let PackReadError::Invariant(archive_invariant) = archive_error else {
        panic!("archive did not report a Pack invariant: {archive_error}");
    };
    assert_eq!(archive_invariant, builder_invariant);
    assert!(matches!(
        archive_invariant,
        PackInvariantError::InvalidPath {
            role: PackPathRole::ResourceSlot,
            ref path,
            ..
        } if path == "../secret.png"
    ));
}

#[test]
fn pack_normalizes_resource_slots_deterministically() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nresource-slots = [\"z.png\", \"assets/../logo.png\", \"./logo.png\"]\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
    ]))
    .unwrap();

    assert_eq!(
        pack.resource_slots().collect::<Vec<_>>(),
        ["logo.png", "z.png"]
    );
    assert!(
        pack.manifest()
            .to_toml()
            .contains("resource-slots = [\n    \"logo.png\",\n    \"z.png\",\n]")
    );

    let built = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .resource_slot("z.png")
        .unwrap()
        .resource_slot("assets/../logo.png")
        .unwrap()
        .resource_slot("./logo.png")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        built.resource_slots().collect::<Vec<_>>(),
        ["logo.png", "z.png"]
    );
}

#[test]
fn manifest_rejects_future_version() {
    let result =
        PackManifest::from_toml("format-version = 99\n[project]\nentrypoint = \"main.typ\"\n");
    assert!(matches!(
        result,
        Err(PackManifestError::UnsupportedVersion(99))
    ));
}

#[test]
fn manifest_rejects_version_zero_and_unknown_version_one_fields() {
    assert!(matches!(
        PackManifest::from_toml("format-version = 0\n[project]\nentrypoint = \"main.typ\"\n"),
        Err(PackManifestError::UnsupportedVersion(0))
    ));
    assert!(matches!(
        PackManifest::from_toml(
            "format-version = 1\nunknown = true\n[project]\nentrypoint = \"main.typ\"\n"
        ),
        Err(PackManifestError::Parse(_))
    ));
}

#[test]
fn manifest_dispatches_version_before_interpreting_version_specific_fields() {
    let text = "format-version = 99\nfuture-field = true\n[project]\nentrypoint = \"main.typ\"\n";

    assert!(matches!(
        PackManifest::from_toml(text),
        Err(PackManifestError::UnsupportedVersion(99))
    ));
    assert!(toml::from_str::<PackManifest>(text).is_err());
}

#[test]
fn pack_construction_requires_a_contained_entrypoint() {
    let built = Pack::builder("main.typ").build();
    assert!(matches!(
        built,
        Err(PackBuildError::Invariant(
            PackInvariantError::MissingEntrypoint(ref path)
        )) if path == "main.typ"
    ));

    use std::io::Write;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    zip.start_file(MANIFEST_PATH, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
        .unwrap();
    zip.finish().unwrap();

    let read = Pack::from_bytes(buffer.into_inner());
    assert!(matches!(
        read,
        Err(PackReadError::Invariant(
            PackInvariantError::MissingEntrypoint(ref path)
        )) if path == "main.typ"
    ));
}

#[test]
fn pack_builder_rejects_paths_that_cannot_name_root_relative_files() {
    assert!(matches!(
        Pack::builder("").build(),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::Entrypoint,
            ..
        }))
    ));
    assert!(matches!(
        Pack::builder("main.typ").file("/main.typ", Vec::new()),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::ProjectFile,
            ..
        }))
    ));
    assert!(matches!(
        Pack::builder("main.typ").resource_slot("."),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::ResourceSlot,
            ..
        }))
    ));
    for path in ["C:outside.txt", "./C:/outside.txt"] {
        assert!(matches!(
            Pack::builder("main.typ").file(path, Vec::new()),
            Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
                role: PackPathRole::ProjectFile,
                ..
            }))
        ));
    }
    assert!(matches!(
        Pack::builder("main\0.typ").build(),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::Entrypoint,
            ..
        }))
    ));
    assert!(matches!(
        Pack::builder("main.typ").file("main\0.typ", Vec::new()),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::ProjectFile,
            ..
        }))
    ));
}

#[test]
fn pack_construction_rejects_conflicting_project_tree_roles() {
    let built = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("assets", b"packed".to_vec())
        .unwrap()
        .file("assets-foo", b"interleaved".to_vec())
        .unwrap()
        .resource_slot("assets/logo.png")
        .unwrap()
        .build();
    assert!(matches!(
        built,
        Err(PackBuildError::Invariant(
            PackInvariantError::PathTreeConflict {
                ref ancestor,
                ref descendant,
                ancestor_role: PackPathRole::ProjectFile,
                descendant_role: PackPathRole::ResourceSlot,
            }
        )) if ancestor == "assets" && descendant == "assets/logo.png"
    ));

    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nresource-slots = [\"assets/logo.png\"]\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("project/assets", b"packed"),
        ("project/assets-foo", b"interleaved"),
    ]);
    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::PathTreeConflict {
                ref ancestor,
                ref descendant,
                ancestor_role: PackPathRole::ProjectFile,
                descendant_role: PackPathRole::ResourceSlot,
            }
        )) if ancestor == "assets" && descendant == "assets/logo.png"
    ));
}

#[test]
fn pack_construction_rejects_conflicting_package_roles() {
    use std::str::FromStr as _;

    let spec = typst::syntax::package::PackageSpec::from_str("@local/example:1.0.0").unwrap();
    let built = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .package_file(spec.clone(), "lib.typ", b"Hello".to_vec())
        .unwrap()
        .external_package_file(spec, "lib.typ", b"Hello".to_vec())
        .unwrap()
        .build();
    assert!(matches!(
        built,
        Err(PackBuildError::Invariant(
            PackInvariantError::PackageRoleConflict(ref spec)
        )) if spec == "@local/example:1.0.0"
    ));

    let declaration = test_package_declaration(&[("lib.typ", b"Hello")]);
    let manifest = test_package_manifest(vec![declaration.clone()], vec![declaration]);
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, &manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0/lib.typ", b"Hello"),
    ]);
    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::PackageRoleConflict(ref spec)
        )) if spec == "@local/example:1.0.0"
    ));
}

#[test]
fn pack_construction_rejects_package_declaration_data_disagreement() {
    let missing_manifest = test_package_manifest(
        vec![test_package_declaration(&[("lib.typ", b"Hello")])],
        vec![],
    );
    let missing = raw_stored_zip(&[
        (MANIFEST_PATH, &missing_manifest),
        ("project/main.typ", b"Hello"),
    ]);
    assert!(matches!(
        Pack::from_bytes(missing),
        Err(PackReadError::Invariant(
            PackInvariantError::MissingVendoredPackageData(ref spec)
        )) if spec == "@local/example:1.0.0"
    ));

    let undeclared_manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let undeclared = raw_stored_zip(&[
        (MANIFEST_PATH, undeclared_manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0/lib.typ", b"Hello"),
    ]);
    assert!(matches!(
        Pack::from_bytes(undeclared),
        Err(PackReadError::Invariant(
            PackInvariantError::UndeclaredPackageData(ref spec)
        )) if spec == "@local/example:1.0.0"
    ));
}

#[test]
fn complete_package_tree_identity_binds_paths_bytes_and_fulfillment_role() {
    let spec: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let external = |path: &str, data: &[u8]| {
        Pack::builder("main.typ")
            .file("main.typ", b"Hello".to_vec())
            .unwrap()
            .external_package_file(spec.clone(), path, data.to_vec())
            .unwrap()
            .build()
            .unwrap()
    };
    let first = external("lib.typ", b"first");
    let changed_bytes = external("lib.typ", b"second");
    let changed_path = external("other.typ", b"first");
    let embedded = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .package_file(spec, "lib.typ", b"first".to_vec())
        .unwrap()
        .build()
        .unwrap();

    assert_ne!(first.identity(), changed_bytes.identity());
    assert_ne!(first.identity(), changed_path.identity());
    assert_ne!(first.identity(), embedded.identity());
    assert_eq!(first.package_requirements()[0].file_count(), 1);
    assert_eq!(first.package_requirements()[0].byte_length(), 5);
    assert!(!first.package_requirements()[0].is_embedded());
    assert!(embedded.package_requirements()[0].is_embedded());
}

#[test]
fn pack_construction_rejects_package_specs_that_do_not_roundtrip() {
    let mut invalid = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    invalid.name = "bad/name".into();

    let vendored = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .package_file(invalid.clone(), "lib.typ", b"Hello".to_vec())
        .unwrap()
        .build();
    let unvendored = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .external_package_file(invalid, "lib.typ", b"Hello".to_vec())
        .unwrap()
        .build();

    assert!(matches!(
        vendored,
        Err(PackBuildError::Invariant(
            PackInvariantError::InvalidPackageSpec { .. }
        ))
    ));
    assert!(matches!(
        unvendored,
        Err(PackBuildError::Invariant(
            PackInvariantError::InvalidPackageSpec { .. }
        ))
    ));
}

#[test]
fn pack_construction_rejects_archive_entry_names_too_long_for_zip() {
    let maximum_path = "a".repeat(65_535 - "project/".len());
    let pack = Pack::builder(&maximum_path)
        .file(&maximum_path, b"Hello".to_vec())
        .unwrap()
        .build()
        .unwrap();
    assert!(pack.to_bytes().is_ok());

    let path = format!("{maximum_path}a");
    let project = Pack::builder(&path)
        .file(&path, b"Hello".to_vec())
        .unwrap()
        .build();
    assert!(matches!(
        project,
        Err(PackBuildError::Invariant(
            PackInvariantError::ArchiveEntryNameTooLong {
                role: PackPathRole::ProjectFile,
                ..
            }
        ))
    ));

    let spec = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let package_path = "a".repeat(65_535);
    let package = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .package_file(spec, package_path, b"Package".to_vec())
        .unwrap()
        .build();
    assert!(matches!(
        package,
        Err(PackBuildError::Invariant(
            PackInvariantError::ArchiveEntryNameTooLong {
                role: PackPathRole::PackageFile,
                ..
            }
        ))
    ));
}

#[test]
fn archive_path_failures_precede_package_role_failures() {
    let path = "a".repeat(65_535 - "project/".len() + 1);
    let spec = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let pack = Pack::builder(&path)
        .file(&path, b"Hello".to_vec())
        .unwrap()
        .package_file(spec.clone(), "lib.typ", b"Package".to_vec())
        .unwrap()
        .external_package_file(spec, "lib.typ", b"Package".to_vec())
        .unwrap()
        .build();

    assert!(matches!(
        pack,
        Err(PackBuildError::Invariant(
            PackInvariantError::ArchiveEntryNameTooLong {
                role: PackPathRole::ProjectFile,
                ..
            }
        ))
    ));
}

#[test]
fn pack_construction_rejects_conflicting_package_file_tree_paths() {
    let manifest =
        test_package_manifest(vec![test_package_declaration(&[("lib", b"file")])], vec![]);
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, &manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0/lib", b"file"),
        ("packages/local/example/1.0.0/lib/child.typ", b"child"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::PackagePathTreeConflict {
                ref package,
                ref ancestor,
                ref descendant,
                ..
            }
        )) if package == "@local/example:1.0.0"
            && ancestor == "lib"
            && descendant == "lib/child.typ"
    ));
}

#[test]
fn pack_construction_rejects_invalid_contained_font_data() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"custom-font.bin\"\nindex = 3\nfamilies = [\"Informational\"]\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("custom-font.bin", b"not a font"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::InvalidFontData {
            ref path,
            index: 3,
        })) if path == "custom-font.bin"
    ));
}

#[test]
fn pack_construction_rejects_missing_font_data() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"fonts/vendor/font.ttf\"\n";
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::MissingFontData(ref path)))
            if path == "fonts/vendor/font.ttf"
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn read_preserves_nested_portable_font_paths() {
    let font = embedded_font_data();
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"fonts/vendor/font.ttf\"\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("fonts/vendor/font.ttf", &font),
    ]))
    .unwrap();

    assert_eq!(pack.fonts()[0].manifest().path(), "fonts/vendor/font.ttf");
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_construction_rejects_a_missing_face_in_valid_font_data() {
    let font = embedded_font_data();
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"fonts/vendor/font.ttf\"\nindex = 99\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("fonts/vendor/font.ttf", &font),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::InvalidFontData {
            ref path,
            index: 99,
        })) if path == "fonts/vendor/font.ttf"
    ));
}

#[test]
fn pack_builder_reports_invalid_font_data_as_an_ingestion_error() {
    assert!(matches!(
        Pack::builder("main.typ").font(b"not a font".to_vec(), 2),
        Err(PackBuildError::InvalidFontInput { index: 2 })
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_accepts_shared_multi_face_custom_font_data_and_informational_families() {
    let collection = two_face_collection(&embedded_font_data());
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"custom-font.data\"\nindex = 0\nfamilies = [\"Not the parsed family\"]\n[[fonts]]\npath = \"custom-font.data\"\nindex = 1\nfamilies = [\"Also informational\"]\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("custom-font.data", &collection),
    ]))
    .unwrap();

    assert_eq!(pack.fonts().len(), 2);
    assert_eq!(pack.fonts()[0].manifest().path(), "custom-font.data");
    assert_eq!(
        pack.fonts()[0].manifest().families(),
        ["Not the parsed family"]
    );
    assert_eq!(pack.fonts()[1].manifest().index(), 1);
    assert_eq!(
        pack.fonts()[1].manifest().families(),
        ["Also informational"]
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_font_catalog_preserves_declared_faces_and_container_disposition() {
    let collection = two_face_collection(&embedded_font_data());
    let mut embedded_collection = collection.clone();
    embedded_collection.push(0);
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"catalog".to_vec())
        .unwrap()
        .external_font(collection.clone(), 1)
        .unwrap()
        .font(embedded_collection, 0)
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(
        pack.font_catalog()
            .iter()
            .map(|face| (face.identity().index(), face.is_embedded()))
            .collect::<Vec<_>>(),
        [(1, false), (0, true)]
    );
    assert_eq!(pack.font_requirements().len(), 2);
    let external = pack
        .font_requirements()
        .iter()
        .find(|requirement| !requirement.is_embedded())
        .unwrap();
    assert_eq!(external.face_indices(), &[1]);

    let reread = Pack::from_bytes(pack.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reread
            .font_catalog()
            .iter()
            .map(|face| (face.identity().index(), face.is_embedded()))
            .collect::<Vec<_>>(),
        [(1, false), (0, true)]
    );
}

#[test]
fn malformed_external_font_is_a_pack_owned_pre_compilation_outcome() {
    let data = b"not a font";
    let identity = FontContainerIdentity::from_bytes(data);
    let digest = identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let manifest = format!(
        "format-version = 1\n[project]\nentrypoint = \"main.typ\"\n\
         [[fonts]]\npath = \"fonts/external.ttf\"\nexternal = true\n\
         container-digest = \"{digest}\"\n\
         container-identity-kind = \"font-container\"\n\
         container-identity-schema = \"typst-pack-font-container-identity-v1\"\n\
         container-identity-algorithm = \"typst-hash128-0.15\"\n\
         container-length = {}\n",
        data.len()
    );
    let archive = raw_stored_zip(&[
        (MANIFEST_PATH, manifest.as_bytes()),
        ("project/main.typ", b"unreached"),
    ]);
    let pack = Pack::from_bytes(archive).unwrap();

    let result = compile_request(
        PackCompilationRequest::new(pack, OutputFormat::Svg)
            .font_fulfillment(identity, FontContainerFulfillment::new(data.to_vec())),
    );

    assert!(matches!(
        result,
        Err(PackCompileError::Operation {
            outcome: CompilationOperationOutcome::MalformedExternalFontContainer {
                container,
                index: 0,
            },
            ..
        }) if container == identity
    ));
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_rejects_duplicate_font_faces() {
    let font = embedded_font_data();
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"font.data\"\nfamilies = [\"A\"]\n[[fonts]]\npath = \"font.data\"\nfamilies = [\"B\"]\n";

    assert!(matches!(
        Pack::from_bytes(raw_stored_zip(&[
            (MANIFEST_PATH, manifest),
            ("project/main.typ", b"Hello"),
            ("font.data", &font),
        ])),
        Err(PackReadError::Invariant(
            PackInvariantError::DuplicateFontFace {
                ref path,
                index: 0,
            }
        )) if path == "font.data"
    ));
}

#[test]
fn pack_construction_rejects_font_paths_reserved_for_project_files() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"project/main.typ\"\nfamilies = [\"Informational\"]\n";
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::ReservedFontPath {
            ref path,
            conflicting_role: PackPathRole::ProjectFile,
        })) if path == "project/main.typ"
    ));
}

#[test]
fn pack_construction_rejects_font_path_at_the_manifest() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"typst-pack.toml\"\nfamilies = [\"Informational\"]\n";
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::ReservedFontPath {
            ref path,
            conflicting_role: PackPathRole::PackManifest,
        })) if path == MANIFEST_PATH
    ));
}

#[test]
fn pack_construction_rejects_font_paths_at_reserved_namespace_roots() {
    for (path, conflicting_role) in [
        ("project", PackPathRole::ProjectFile),
        ("packages", PackPathRole::PackageFile),
    ] {
        let manifest = format!(
            "format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"{path}\"\nfamilies = [\"Informational\"]\n"
        );
        let bytes = raw_stored_zip(&[
            (MANIFEST_PATH, manifest.as_bytes()),
            ("project/main.typ", b"Hello"),
            (path, b"not a font"),
        ]);

        assert!(matches!(
            Pack::from_bytes(bytes),
            Err(PackReadError::Invariant(PackInvariantError::ReservedFontPath {
                path: ref actual,
                conflicting_role: actual_role,
            })) if actual == path && actual_role == conflicting_role
        ));
    }
}

#[test]
fn pack_construction_rejects_conflicting_font_data_tree_paths() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"fonts/a\"\nfamilies = [\"A\"]\n[[fonts]]\npath = \"fonts/a/face.ttf\"\nfamilies = [\"B\"]\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("fonts/a", b"not a font"),
        ("fonts/a/face.ttf", b"not a font"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::PathTreeConflict {
                ref ancestor,
                ancestor_role: PackPathRole::FontData,
                ref descendant,
                descendant_role: PackPathRole::FontData,
            }
        )) if ancestor == "fonts/a" && descendant == "fonts/a/face.ttf"
    ));
}

#[test]
fn font_path_failures_precede_entrypoint_failures() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"missing.typ\"\n[[fonts]]\npath = \"../font.ttf\"\nfamilies = [\"Invalid\"]\n";

    assert!(matches!(
        Pack::from_bytes(raw_stored_zip(&[(MANIFEST_PATH, manifest)])),
        Err(PackReadError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::FontData,
            ..
        }))
    ));
}

#[test]
fn invariant_diagnostics_do_not_expose_optional_field_formatting() {
    let tree = PackInvariantError::PathTreeConflict {
        ancestor: "assets".to_owned(),
        ancestor_role: PackPathRole::ProjectFile,
        descendant: "assets/logo.png".to_owned(),
        descendant_role: PackPathRole::ResourceSlot,
    };
    assert_eq!(
        tree.to_string(),
        "project file path `assets` conflicts with Resource Slot descendant `assets/logo.png`"
    );

    let font = PackBuildError::InvalidFontInput { index: 2 };
    assert_eq!(
        font.to_string(),
        "font input does not contain a valid face at index 2"
    );
}

#[test]
fn a_constructed_pack_builds_a_world_without_revalidation() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .build()
        .unwrap();

    let _: PackWorld = PackWorld::builder(pack).build().unwrap();
}

#[test]
fn pack_world_accepts_a_resource_provider() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("resource.bin")
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack)
        .resource_provider(MemoryProjectFile::new("resource.bin", b"provided".to_vec()))
        .build()
        .unwrap();

    assert_eq!(
        world
            .file(project_file_id("resource.bin"))
            .unwrap()
            .as_slice(),
        b"provided"
    );
}

#[test]
fn pack_roundtrip_in_memory() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", "#include \"note.typ\"".as_bytes().to_vec())
        .unwrap()
        .file("note.typ", "Hello".as_bytes().to_vec())
        .unwrap()
        .file("assets/logo.png", tiny_png())
        .unwrap()
        .build()
        .unwrap();

    let bytes = pack.to_bytes().unwrap();
    let reread = Pack::from_bytes(bytes).unwrap();

    assert_eq!(reread.entrypoint(), "main.typ");
    assert_eq!(reread.files().count(), 3);
    assert_eq!(
        reread.file("note.typ").unwrap().as_slice(),
        "Hello".as_bytes()
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn full_unicode_pack_roundtrip_is_equivalent_and_idempotent() {
    let vendored = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let unvendored = "@local/remote:2.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let pack = Pack::builder("文档.typ")
        .file("文档.typ", b"Hello".to_vec())
        .unwrap()
        .file("资料/说明.txt", b"Notes".to_vec())
        .unwrap()
        .resource_slot("品牌/图.png")
        .unwrap()
        .package_file(vendored, "章节.typ", b"Package".to_vec())
        .unwrap()
        .external_package_file(unvendored, "lib.typ", b"Remote".to_vec())
        .unwrap()
        .font(embedded_font_data(), 0)
        .unwrap()
        .metadata(PackMetadata::new().with_name("完整 Pack"))
        .build()
        .unwrap();

    let bytes = pack.to_bytes().unwrap();
    let reread = Pack::from_bytes(bytes.clone()).unwrap();

    assert_eq!(reread.manifest(), pack.manifest());
    assert_eq!(reread.file("资料/说明.txt").unwrap().as_slice(), b"Notes");
    assert_eq!(reread.resource_slots().collect::<Vec<_>>(), ["品牌/图.png"]);
    assert_eq!(reread.packages().count(), 1);
    assert_eq!(reread.fonts().len(), 1);
    assert_eq!(reread.to_bytes().unwrap(), bytes);
}

#[test]
fn manually_declared_resource_slot_survives_archive_roundtrip() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#image(\"assets/logo.png\")".to_vec())
        .unwrap()
        .resource_slot("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    assert!(pack.file("assets/logo.png").is_none());
    let pack = Pack::from_bytes(pack.to_bytes().unwrap()).unwrap();
    assert_eq!(
        pack.resource_slots().collect::<Vec<_>>(),
        ["assets/logo.png"]
    );
}

#[test]
fn pack_builder_rejects_resource_slot_file_conflicts() {
    let packed_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("logo.png", tiny_png())
        .unwrap()
        .resource_slot("logo.png")
        .unwrap()
        .build();
    let declared_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("logo.png")
        .unwrap()
        .file("logo.png", tiny_png())
        .unwrap()
        .build();

    assert!(matches!(
        packed_first,
        Err(PackBuildError::Invariant(PackInvariantError::PathRoleConflict { path, .. }))
            if path == "logo.png"
    ));
    assert!(matches!(
        declared_first,
        Err(PackBuildError::Invariant(PackInvariantError::PathRoleConflict { path, .. }))
            if path == "logo.png"
    ));
}

#[test]
fn repeated_builder_calls_replace_data_within_the_same_role() {
    let spec = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"first".to_vec())
        .unwrap()
        .file("main.typ", b"second".to_vec())
        .unwrap()
        .package_file(spec.clone(), "lib.typ", b"first".to_vec())
        .unwrap()
        .package_file(spec.clone(), "lib.typ", b"second".to_vec())
        .unwrap()
        .resource_slot("optional.bin")
        .unwrap()
        .resource_slot("optional.bin")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(pack.file("main.typ").unwrap().as_slice(), b"second");
    assert_eq!(
        pack.package_file(&spec, "lib.typ").unwrap().as_slice(),
        b"second"
    );
    assert_eq!(pack.resource_slots().collect::<Vec<_>>(), ["optional.bin"]);
}

#[test]
fn read_rejects_archives_without_manifest() {
    use std::io::Write;
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    zip.start_file("project/main.typ", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"hi").unwrap();
    zip.finish().unwrap();
    let result = Pack::from_bytes(buffer.into_inner());
    assert!(matches!(result, Err(PackReadError::MissingManifest)));
}

#[test]
fn read_reports_corrupt_zip_data_as_an_archive_error() {
    assert!(matches!(
        Pack::from_bytes(b"not a zip archive".to_vec()),
        Err(PackReadError::Zip(_))
    ));
}

#[test]
fn read_accepts_a_manifest_that_is_not_the_first_entry() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        ("project/main.typ", b"Hello"),
        (MANIFEST_PATH, manifest),
    ]))
    .unwrap();

    assert_eq!(pack.file("main.typ").unwrap().as_slice(), b"Hello");
}

#[test]
fn read_reports_a_non_utf8_manifest_specifically() {
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, &[0xff]), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::ManifestNotUtf8(_))
    ));
}

#[test]
fn read_reports_an_unreadable_manifest_payload_specifically() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = with_first_zip_entry_corrupt_data(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
    ]));

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::ManifestUnreadable(_))
    ));
}

#[test]
fn manifest_decoding_failures_precede_archive_path_failures() {
    let non_utf8 = raw_stored_zip(&[(MANIFEST_PATH, &[0xff]), ("project/../bad.typ", b"bad")]);
    assert!(matches!(
        Pack::from_bytes(non_utf8),
        Err(PackReadError::ManifestNotUtf8(_))
    ));

    let malformed = raw_stored_zip(&[
        (MANIFEST_PATH, b"not valid TOML = ["),
        ("project/../bad.typ", b"bad"),
    ]);
    assert!(matches!(
        Pack::from_bytes(malformed),
        Err(PackReadError::Manifest(_))
    ));

    let duplicate_with_non_utf8_manifest = raw_stored_zip(&[
        (MANIFEST_PATH, &[0xff]),
        ("future/data", b"first"),
        ("future/data", b"second"),
    ]);
    assert!(matches!(
        Pack::from_bytes(duplicate_with_non_utf8_manifest),
        Err(PackReadError::ManifestNotUtf8(_))
    ));

    let duplicate_with_malformed_manifest = raw_stored_zip(&[
        (MANIFEST_PATH, b"not valid TOML = ["),
        ("future/data", b"first"),
        ("future/data", b"second"),
    ]);
    assert!(matches!(
        Pack::from_bytes(duplicate_with_malformed_manifest),
        Err(PackReadError::Manifest(_))
    ));

    let duplicate_with_unsupported_manifest = raw_stored_zip(&[
        (
            MANIFEST_PATH,
            b"format-version = 99\n[project]\nentrypoint = \"main.typ\"\n",
        ),
        ("future/data", b"first"),
        ("future/data", b"second"),
    ]);
    assert!(matches!(
        Pack::from_bytes(duplicate_with_unsupported_manifest),
        Err(PackReadError::DuplicateArchiveEntry(ref name)) if name == b"future/data"
    ));
}

#[test]
fn read_reports_a_non_file_manifest_specifically() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = with_first_zip_entry_unix_mode(
        raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]),
        0o120777,
    );

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::ManifestNotFile)
    ));
}

#[test]
fn read_rejects_unsafe_unknown_directories_before_ignoring_them() {
    use std::io::Write as _;

    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(MANIFEST_PATH, options).unwrap();
    zip.write_all(b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
        .unwrap();
    zip.start_file("project/main.typ", options).unwrap();
    zip.write_all(b"Hello").unwrap();
    zip.add_directory("../ignored/", options).unwrap();
    zip.finish().unwrap();

    assert!(matches!(
        Pack::from_bytes(buffer.into_inner()),
        Err(PackReadError::UnsafeEntry(_))
    ));
}

#[test]
fn read_accepts_safe_unknown_entries_and_rewrite_drops_them() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("future/data.bin", b"ignored"),
    ]))
    .unwrap();

    let mut rewritten =
        zip::ZipArchive::new(std::io::Cursor::new(pack.to_bytes().unwrap())).unwrap();
    assert!(rewritten.by_name("future/data.bin").is_err());
    assert_eq!(rewritten.by_name("project/main.typ").unwrap().size(), 5);
}

#[test]
fn read_accepts_safe_directory_entries() {
    use std::io::Write as _;

    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    zip.add_directory("project/", options).unwrap();
    zip.add_directory("future/nested/", options).unwrap();
    zip.start_file(MANIFEST_PATH, options).unwrap();
    zip.write_all(b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
        .unwrap();
    zip.start_file("project/main.typ", options).unwrap();
    zip.write_all(b"Hello").unwrap();
    zip.finish().unwrap();

    let pack = Pack::from_bytes(buffer.into_inner()).unwrap();
    assert_eq!(pack.file("main.typ").unwrap().as_slice(), b"Hello");
}

#[test]
fn read_rejects_a_windows_prefix_hidden_by_a_current_directory_alias() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("./C:/ignored", b"must not be ignored"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::UnsafeEntry(_))
    ));
}

#[test]
fn read_rejects_duplicate_manifest_entries() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::DuplicateManifest)
    ));
}

#[test]
fn read_rejects_exact_duplicate_unknown_entries() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("future/data", b"first"),
        ("future/data", b"second"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::DuplicateArchiveEntry(ref name)) if name == b"future/data"
    ));
}

#[test]
fn read_rejects_distinct_archive_entries_with_one_canonical_identity() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"first"),
        ("project/./main.typ", b"second"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision {
                ref canonical,
                ..
            }
        )) if canonical == "project/main.typ"
    ));
}

#[test]
fn read_rejects_canonical_collisions_for_package_and_font_entries() {
    let package_manifest = test_package_manifest(
        vec![test_package_declaration(&[("lib.typ", b"first")])],
        vec![],
    );
    let package = raw_stored_zip(&[
        (MANIFEST_PATH, &package_manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0/lib.typ", b"first"),
        ("packages/local/example/1.0.0/./lib.typ", b"second"),
    ]);
    assert!(matches!(
        Pack::from_bytes(package),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision { ref canonical, .. }
        )) if canonical == "packages/local/example/1.0.0/lib.typ"
    ));

    let font_manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[[fonts]]\npath = \"fonts/vendor/font.ttf\"\n";
    let font = raw_stored_zip(&[
        (MANIFEST_PATH, font_manifest),
        ("project/main.typ", b"Hello"),
        ("fonts/vendor/font.ttf", b"first"),
        ("fonts/vendor/./font.ttf", b"second"),
    ]);
    assert!(matches!(
        Pack::from_bytes(font),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision { ref canonical, .. }
        )) if canonical == "fonts/vendor/font.ttf"
    ));
}

#[test]
fn read_rejects_malformed_package_entry_layouts() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0", b"missing file path"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::InvalidEntry { ref entry, .. })
            if entry == "packages/local/example/1.0.0"
    ));
}

#[test]
fn read_rejects_distinct_raw_names_with_one_decoded_identity() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip_with_raw_names(&[
        (MANIFEST_PATH.as_bytes(), false, manifest),
        (b"project/main.typ", false, b"Hello"),
        ("project/é.txt".as_bytes(), true, b"UTF-8"),
        (b"project/\x82.txt", false, b"CP437"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision {
                ref canonical,
                ..
            }
        )) if canonical == "project/é.txt"
    ));
}

#[test]
fn read_rejects_canonical_collisions_between_unknown_entries() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
        ("future/data", b"first"),
        ("future/./data", b"second"),
    ]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision {
                ref canonical,
                ..
            }
        )) if canonical == "future/data"
    ));
}

#[test]
fn read_does_not_normalize_a_known_role_into_an_ignored_archive_entry() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    for invalid in ["project/../ignored", "./project/../ignored"] {
        let bytes = raw_stored_zip(&[
            (MANIFEST_PATH, manifest),
            ("project/main.typ", b"Hello"),
            (invalid, b"must not be ignored"),
        ]);

        assert!(matches!(
            Pack::from_bytes(bytes),
            Err(PackReadError::Invariant(PackInvariantError::InvalidPath {
                role: PackPathRole::ProjectFile,
                ..
            }))
        ));
    }
}

#[test]
fn read_classifies_safe_archive_prefix_aliases_by_their_canonical_role() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("./project/main.typ", b"Hello"),
    ]))
    .unwrap();

    assert_eq!(pack.file("main.typ").unwrap().as_slice(), b"Hello");
    assert_eq!(
        pack.files().map(|(path, _)| path).collect::<Vec<_>>(),
        ["main.typ"]
    );
}

#[test]
fn read_accepts_safe_aliases_at_archive_role_boundaries() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n";
    let project = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project//main.typ", b"Hello"),
    ]))
    .unwrap();
    assert_eq!(project.file("main.typ").unwrap().as_slice(), b"Hello");

    let package_manifest = test_package_manifest(
        vec![test_package_declaration(&[("lib.typ", b"Package")])],
        vec![],
    );
    let package = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, &package_manifest),
        ("project/main.typ", b"Hello"),
        ("packages/local/example/1.0.0//lib.typ", b"Package"),
    ]))
    .unwrap();
    let spec = "@local/example:1.0.0"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    assert_eq!(
        package.package_file(&spec, "lib.typ").unwrap().as_slice(),
        b"Package"
    );

    let aliased_manifest = Pack::from_bytes(raw_stored_zip(&[
        ("alias/../typst-pack.toml", manifest),
        ("project/main.typ", b"Hello"),
    ]))
    .unwrap();
    assert_eq!(
        aliased_manifest.file("main.typ").unwrap().as_slice(),
        b"Hello"
    );

    let colliding_manifest = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("alias/../typst-pack.toml", manifest),
        ("project/main.typ", b"Hello"),
    ]);
    assert!(matches!(
        Pack::from_bytes(colliding_manifest),
        Err(PackReadError::Invariant(
            PackInvariantError::CanonicalArchiveEntryCollision { ref canonical, .. }
        )) if canonical == MANIFEST_PATH
    ));
}

#[test]
fn read_rejects_resource_slot_file_conflicts() {
    use std::io::Write;

    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(MANIFEST_PATH, options).unwrap();
    zip.write_all(
        b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nresource-slots = [\"logo.png\"]\n",
    )
    .unwrap();
    zip.start_file("project/main.typ", options).unwrap();
    zip.write_all(b"Hello").unwrap();
    zip.start_file("project/logo.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();
    zip.finish().unwrap();

    let result = Pack::from_bytes(buffer.into_inner());
    assert!(matches!(
        result,
        Err(PackReadError::Invariant(PackInvariantError::PathRoleConflict { path, .. }))
            if path == "logo.png"
    ));
}

#[test]
fn parse_page_selection_understands_ranges() {
    use std::num::NonZeroUsize;
    let one = NonZeroUsize::new(1);
    let three = NonZeroUsize::new(3);
    let five = NonZeroUsize::new(5);
    let nine = NonZeroUsize::new(9);
    assert_eq!(
        parse_page_selection("1,3-5,9-").unwrap().ranges(),
        &[one..=one, three..=five, nine..=None]
    );
    assert!(parse_page_selection("nope").is_err());
}

#[cfg(feature = "cli")]
#[test]
fn cli_uses_typst_embedded_font_terminology() {
    use clap::Parser as _;

    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--embed-fonts",
            "--include-typst-embedded-fonts",
        ])
        .is_ok()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--embed-fonts",
            "--include-default-fonts",
        ])
        .is_err()
    );
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn compile_in_memory_pack_to_pdf_and_svg() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            "#set page(width: 10cm, height: 4cm)\nHello from a pack!"
                .as_bytes()
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack).build().unwrap();

    let pdf = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
    assert_eq!(pdf.artifacts.len(), 1);
    assert!(pdf.artifacts[0].bytes().starts_with(b"%PDF"));

    let svg = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
    assert_eq!(svg.artifacts.len(), 1);
    assert!(
        std::str::from_utf8(svg.artifacts[0].bytes())
            .unwrap()
            .contains("<svg")
    );

    let png = compile(&world, OutputFormat::Png, &CompileOptions::default()).unwrap();
    assert!(
        png.artifacts[0]
            .bytes()
            .starts_with(&[0x89, b'P', b'N', b'G'])
    );
}

#[test]
fn declared_resource_slot_compiles_through_a_provider() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")"
                .to_vec(),
        )
        .unwrap()
        .resource_slot("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack.clone())
        .resource_provider(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .build()
        .unwrap();
    let pdf = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
    assert!(pdf.artifacts[0].bytes().starts_with(b"%PDF"));
    let png = compile(&world, OutputFormat::Png, &CompileOptions::default()).unwrap();
    assert!(
        png.artifacts[0]
            .bytes()
            .starts_with(&[0x89, b'P', b'N', b'G'])
    );
    let svg = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
    assert!(
        std::str::from_utf8(svg.artifacts[0].bytes())
            .unwrap()
            .contains("<svg")
    );

    let world = PackWorld::builder(pack)
        .resource_provider(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .feature(typst::Feature::Html)
        .build()
        .unwrap();
    let html = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    assert!(
        std::str::from_utf8(html.artifacts[0].bytes())
            .unwrap()
            .contains("<html")
    );
}

#[test]
fn source_compilation_cannot_use_a_non_typ_resource_slot_provider() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#include \"provided.data\"".to_vec())
        .unwrap()
        .resource_slot("provided.data")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) =
        MemoryProjectFile::tracked("provided.data", b"#rect(width: 1pt, height: 1pt)".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn pdf_default_timestamp_is_resolved_after_compilation() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#read(\"timestamp-trigger.bin\")\n#rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .unwrap()
        .resource_slot("timestamp-trigger.bin")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("timestamp-trigger.bin", b"read".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();
    let timestamp = typst_pdf::Timestamp::new_utc(
        typst::foundations::Datetime::from_ymd_hms(2000, 1, 2, 3, 4, 5).unwrap(),
    );
    let default_resolutions = AtomicUsize::new(0);

    let default_output = crate::compile::compile_with_default_pdf_timestamp(
        &world,
        OutputFormat::Pdf,
        &CompileOptions::default(),
        || {
            assert_eq!(calls.load(Ordering::Acquire), 1);
            default_resolutions.fetch_add(1, Ordering::Relaxed);
            Some(timestamp)
        },
    )
    .unwrap();

    let explicit_output = crate::compile::compile_with_default_pdf_timestamp(
        &world,
        OutputFormat::Pdf,
        &CompileOptions {
            creation_timestamp: CreationTimestamp::Explicit(timestamp),
            ..CompileOptions::default()
        },
        || panic!("an explicit timestamp must not resolve the default"),
    )
    .unwrap();

    assert_eq!(default_resolutions.load(Ordering::Relaxed), 1);
    assert_eq!(
        default_output.artifacts[0].bytes(),
        explicit_output.artifacts[0].bytes()
    );
}

#[test]
fn resource_providers_follow_registration_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("resource.bin")
        .unwrap()
        .build()
        .unwrap();
    let id = project_file_id("resource.bin");

    let (first, first_calls) = MemoryProjectFile::tracked("resource.bin", b"first".to_vec());
    let (second, second_calls) = MemoryProjectFile::tracked("resource.bin", b"second".to_vec());
    let world = PackWorld::builder(pack.clone())
        .resource_provider(first)
        .resource_provider(second)
        .build()
        .unwrap();
    assert_eq!(world.file(id).unwrap().as_slice(), b"first");
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);

    let (missing, missing_calls) = MemoryProjectFile::tracked("other.bin", Vec::new());
    let (fallback, fallback_calls) =
        MemoryProjectFile::tracked("resource.bin", b"fallback".to_vec());
    let world = PackWorld::builder(pack.clone())
        .resource_provider(missing)
        .resource_provider(fallback)
        .build()
        .unwrap();
    assert_eq!(world.file(id).unwrap().as_slice(), b"fallback");
    assert_eq!(missing_calls.load(Ordering::Relaxed), 1);
    assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);

    let (denied, denied_calls) = ErrorProjectLoader::tracked(FileError::AccessDenied);
    let (masked, masked_calls) = MemoryProjectFile::tracked("resource.bin", b"masked".to_vec());
    let world = PackWorld::builder(pack.clone())
        .resource_provider(denied)
        .resource_provider(masked)
        .build()
        .unwrap();
    assert_eq!(world.file(id), Err(FileError::AccessDenied));
    assert_eq!(denied_calls.load(Ordering::Relaxed), 1);
    assert_eq!(masked_calls.load(Ordering::Relaxed), 0);

    let integrity_error = FileError::Other(Some("checksum mismatch".into()));
    let (corrupt, corrupt_calls) = ErrorProjectLoader::tracked(integrity_error.clone());
    let (masked, masked_calls) = MemoryProjectFile::tracked("resource.bin", b"masked".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(corrupt)
        .resource_provider(masked)
        .build()
        .unwrap();
    assert_eq!(world.file(id), Err(integrity_error));
    assert_eq!(corrupt_calls.load(Ordering::Relaxed), 1);
    assert_eq!(masked_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn all_missing_resource_providers_report_the_requested_project_path_lazily() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("requested.bin")
        .unwrap()
        .resource_slot("unused.bin")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = ErrorProjectLoader::tracked(FileError::NotFound(PathBuf::from(
        "/host-specific/missing.bin",
    )));
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();

    assert_eq!(
        world.file(project_file_id("requested.bin")),
        Err(FileError::NotFound(PathBuf::from("requested.bin")))
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn source_requests_do_not_consult_resource_providers() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("provided.typ")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("provided.typ", b"injected".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();

    assert!(matches!(
        world.source(project_file_id("provided.typ")),
        Err(FileError::NotFound(_))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn raw_reads_use_providers_even_when_a_resource_slot_has_a_typ_extension() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#assert(read(\"provided.typ\") == \"injected\")\n#rect(width: 1pt, height: 1pt)"
                .to_vec(),
        )
        .unwrap()
        .resource_slot("provided.typ")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("provided.typ", b"injected".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();

    let output = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();

    assert_eq!(output.artifacts.len(), 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn concurrent_world_file_and_source_requests_remain_isolated() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("external.typ")
        .unwrap()
        .build()
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let world = PackWorld::builder(pack)
        .resource_provider(BlockingProjectFiles {
            paths: vec!["external.typ".to_owned()],
            data: Bytes::new(b"provided".to_vec()),
            entered: entered_tx,
            release: Arc::new(Mutex::new(release_rx)),
        })
        .build()
        .unwrap();
    let id = project_file_id("external.typ");

    std::thread::scope(|scope| {
        let mut release = ReleaseGuard::new(release_tx, 1);
        let world = &world;
        let file = scope.spawn(|| world.file(id));
        assert_eq!(
            entered_rx.recv_timeout(TEST_SYNC_TIMEOUT).unwrap(),
            "external.typ"
        );

        let (source_finished_tx, source_finished_rx) = mpsc::channel();
        let source = scope.spawn(move || {
            let result = world.source(id);
            let _ = source_finished_tx.send(());
            result
        });
        source_finished_rx.recv_timeout(TEST_SYNC_TIMEOUT).unwrap();

        release.release_all();
        let file_result = file.join().unwrap();
        let source_result = source.join().unwrap();

        assert!(matches!(source_result, Err(FileError::NotFound(_))));
        assert_eq!(file_result.unwrap().as_slice(), b"provided");
    });
}

#[test]
fn packed_and_undeclared_project_paths_do_not_consult_resource_providers() {
    let packed = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("resource.bin", b"packed".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("resource.bin", b"provided".to_vec());
    let world = PackWorld::builder(packed)
        .resource_provider(provider)
        .build()
        .unwrap();
    assert_eq!(
        world
            .file(project_file_id("resource.bin"))
            .unwrap()
            .as_slice(),
        b"packed"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let undeclared = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("missing.bin", b"provided".to_vec());
    let world = PackWorld::builder(undeclared)
        .resource_provider(provider)
        .build()
        .unwrap();
    assert!(matches!(
        world.file(project_file_id("missing.bin")),
        Err(FileError::NotFound(_))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn package_requests_do_not_consult_resource_providers() {
    use std::str::FromStr as _;

    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .resource_slot("lib.typ")
        .unwrap()
        .build()
        .unwrap();
    let (provider, calls) = MemoryProjectFile::tracked("lib.typ", b"provided".to_vec());
    let world = PackWorld::builder(pack)
        .resource_provider(provider)
        .build()
        .unwrap();
    let spec = typst::syntax::package::PackageSpec::from_str("@local/example:1.0.0").unwrap();
    let id = RootedPath::new(
        VirtualRoot::Package(spec),
        VirtualPath::new("lib.typ").unwrap(),
    )
    .intern();

    assert!(world.file(id).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn undeclared_package_requests_have_no_ambient_fallback() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack).build().unwrap();
    let spec = "@local/undeclared:1.0.0".parse().unwrap();
    let id = RootedPath::new(
        VirtualRoot::Package(spec),
        VirtualPath::new("lib.typ").unwrap(),
    )
    .intern();

    assert!(world.file(id).is_err());
}

#[test]
fn vendored_package_compiles_from_the_pack() {
    use std::str::FromStr as _;

    let spec = typst::syntax::package::PackageSpec::from_str("@local/inside:1.0.0").unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/inside:1.0.0\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .package_file(
            spec.clone(),
            "typst.toml",
            b"[package]\nname = \"inside\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n"
                .to_vec(),
        )
        .unwrap()
        .package_file(
            spec,
            "lib.typ",
            b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack).build().unwrap();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_ok());
}

#[test]
fn missing_vendored_package_file_has_no_ambient_fallback() {
    use std::str::FromStr as _;

    let spec = typst::syntax::package::PackageSpec::from_str("@local/inside:1.0.0").unwrap();
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#import \"@local/inside:1.0.0\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .package_file(
            spec,
            "typst.toml",
            b"[package]\nname = \"inside\"\nversion = \"1.0.0\"\nentrypoint = \"missing.typ\"\n"
                .to_vec(),
        )
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack).build().unwrap();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
}

#[cfg(feature = "fs")]
mod fs {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Creates a project directory with an image, a data file, an included
    /// chapter, and an import from a local package, plus the package itself
    /// in a separate directory laid out like a package path.
    pub(crate) fn fixture(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let project = dir.join("project");
        fs::create_dir_all(project.join("chapters")).unwrap();
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            r#"#import "@local/greet:0.1.0": greet
#set page(width: 10cm, height: 8cm)
#include "chapters/intro.typ"
#image("assets/logo.png", width: 8pt)
#greet("World")
Rows: #csv("data.csv").len()
"#,
        )
        .unwrap();
        fs::write(project.join("chapters/intro.typ"), "= Introduction\n").unwrap();
        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        fs::write(project.join("data.csv"), "a,b\n1,2\n").unwrap();
        // A file the compile never reads:
        fs::write(project.join("notes.txt"), "extra").unwrap();

        let packages = dir.join("packages");
        let package = packages.join("local/greet/0.1.0");
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("typst.toml"),
            "[package]\nname = \"greet\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        )
        .unwrap();
        fs::write(
            package.join("lib.typ"),
            "#let greet(name) = [Hello #name!]\n",
        )
        .unwrap();
        fs::write(package.join("unused.txt"), "complete package").unwrap();

        (project, packages)
    }

    fn pack_fixture(dir: &Path) -> PackOutcome {
        let (project, packages) = fixture(dir);
        Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .include("notes.txt")
            .pack()
            .unwrap()
    }

    #[test]
    fn discovery_finds_all_used_files_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        let report = &outcome.report;
        for expected in [
            "main.typ",
            "chapters/intro.typ",
            "assets/logo.png",
            "data.csv",
            "notes.txt",
        ] {
            assert!(
                report.files.iter().any(|file| file == expected),
                "missing {expected} in {:?}",
                report.files
            );
        }
        assert_eq!(report.packages_vendored.len(), 1);
        assert_eq!(
            report.packages_vendored[0].to_string(),
            "@local/greet:0.1.0"
        );
        assert!(report.packages_unvendored.is_empty());

        let spec = &report.packages_vendored[0];
        assert!(outcome.pack.has_package(spec));
        assert!(outcome.pack.package_file(spec, "lib.typ").is_some());
        assert!(outcome.pack.package_file(spec, "typst.toml").is_some());
        assert!(outcome.pack.package_file(spec, "unused.txt").is_some());

        let reread = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        assert_eq!(outcome.pack.discovery().len(), 1);
        assert!(!outcome.pack.discovery()[0].observations().is_empty());
        let mut without_discovery = outcome.pack.clone();
        without_discovery.set_discovery(vec![]);
        assert_ne!(without_discovery.identity(), outcome.pack.identity());
        assert_eq!(reread.discovery(), outcome.pack.discovery());
        assert_eq!(reread.identity(), outcome.pack.identity());
    }

    #[test]
    fn packer_preserves_the_timestamp_range_error() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("main.typ"), "Hello").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .creation_timestamp(Some(i64::MAX))
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::InvalidTimestamp(ref message))
                if message == "timestamp is out of range"
        ));
    }

    #[test]
    fn discovery_targets_union_target_specific_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#context if target() == \"html\" { read(\"html.txt\") } else { read(\"paged.txt\") }",
        )
        .unwrap();
        fs::write(project.join("paged.txt"), "paged").unwrap();
        fs::write(project.join("html.txt"), "html").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .feature(typst::Feature::Html)
            .target(DiscoveryTarget::Html)
            .target(DiscoveryTarget::Paged)
            .pack()
            .unwrap();

        assert_eq!(outcome.report.files, ["html.txt", "main.typ", "paged.txt"]);
        assert_eq!(outcome.report.discovery_variants.len(), 2);
        for variant in &outcome.report.discovery_variants {
            assert_eq!(variant.trace(), variant.replay_trace());
        }
        let traces = outcome
            .report
            .discovery_variants
            .iter()
            .map(|variant| {
                variant
                    .trace()
                    .observations()
                    .map(|observation| observation.logical_path())
                    .collect::<BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            outcome.report.discovery_variants[0].request().target(),
            DiscoveryTarget::Html
        );
        assert!(traces[0].contains("project:html.txt"));
        assert!(!traces[0].contains("project:paged.txt"));
        assert!(traces[1].contains("project:paged.txt"));
        assert!(!traces[1].contains("project:html.txt"));
    }

    #[test]
    fn discovery_targets_union_shared_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set text(font: \"Definitely Missing\")\nHello",
        )
        .unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .typst_embedded_fonts(false)
            .feature(typst::Feature::Html)
            .target(DiscoveryTarget::Paged)
            .target(DiscoveryTarget::Html)
            .pack()
            .unwrap();

        let missing_font_warnings = outcome
            .report
            .compile_warnings
            .iter()
            .filter(|warning| warning.message.contains("unknown font family"))
            .count();
        assert_eq!(
            missing_font_warnings, 2,
            "{:?}",
            outcome.report.compile_warnings
        );
        assert_eq!(outcome.report.discovery_variants.len(), 2);
        assert!(outcome.report.discovery_variants.iter().all(|variant| {
            variant
                .warnings()
                .iter()
                .filter(|warning| warning.message.contains("unknown font family"))
                .count()
                == 1
                && variant.warnings() == variant.replay_warnings()
        }));
    }

    #[test]
    fn changed_project_evidence_prevents_pack_issuance() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let main = project.join("main.typ");
        fs::write(&main, "original").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .after_discovery_hook({
                let main = main.clone();
                move || fs::write(&main, "changed").unwrap()
            })
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::CreationEvidenceChanged { ref path }) if path == &main.display().to_string()
        ));
    }

    #[test]
    fn exact_inputs_and_document_time_drive_discovery_and_replay_traces() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            r#"#if sys.inputs.at("pick") == "yes" { read("input.txt") }
#if datetime.today().year() == 2024 { read("time.txt") }"#,
        )
        .unwrap();
        fs::write(project.join("input.txt"), "input").unwrap();
        fs::write(project.join("time.txt"), "time").unwrap();
        let mut inputs = typst::foundations::Dict::new();
        inputs.insert("pick".into(), typst::foundations::Value::Str("yes".into()));

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .inputs(inputs)
            .creation_timestamp(Some(1_704_067_200))
            .pack()
            .unwrap();

        assert_eq!(outcome.report.files, ["input.txt", "main.typ", "time.txt"]);
        let variant = &outcome.report.discovery_variants[0];
        assert_eq!(variant.request().document_timestamp(), 1_704_067_200);
        assert_eq!(variant.request().inputs().entry_count(), 1);
        assert_eq!(variant.trace(), variant.replay_trace());
    }

    #[test]
    fn discovery_only_overrides_drive_trace_without_replacing_pack_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#include \"choice.typ\"").unwrap();
        fs::write(project.join("choice.typ"), "baseline").unwrap();
        fs::write(project.join("override.txt"), "selected").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .discovery_override("choice.typ", "#read(\"override.txt\")".as_bytes())
            .pack()
            .unwrap();

        assert_eq!(
            outcome.pack.file("choice.typ").unwrap().as_slice(),
            b"baseline"
        );
        let variant = &outcome.report.discovery_variants[0];
        assert_eq!(variant.request().overrides().len(), 1);
        assert_eq!(variant.trace(), variant.replay_trace());
        assert!(
            variant
                .trace()
                .observations()
                .any(|observation| observation.logical_path() == "project:override.txt")
        );
        let persisted = outcome.pack.discovery()[0]
            .observations()
            .iter()
            .find(|observation| observation.logical_path() == "project:choice.typ")
            .unwrap();
        assert_eq!(persisted.authority(), "project");
        assert_eq!(persisted.project_provenance(), Some("override"));
        assert!(persisted.digest().is_none());
        assert!(persisted.commitment().is_some());
    }

    #[test]
    fn package_discovery_does_not_consult_resource_providers() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let (provider, calls) = MemoryProjectFile::tracked("lib.typ", b"injected".to_vec());

        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .resource_provider(provider)
            .pack()
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(outcome.report.packages_vendored.len(), 1);
    }

    #[test]
    fn package_data_precedes_package_cache_during_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"@local/chosen:0.1.0\": mark\n#mark",
        )
        .unwrap();

        let data_package = dir.path().join("data/local/chosen/0.1.0");
        fs::create_dir_all(&data_package).unwrap();
        fs::write(
            data_package.join("typst.toml"),
            "[package]\nname = \"chosen\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        )
        .unwrap();
        fs::write(
            data_package.join("lib.typ"),
            "#let mark = rect(width: 1pt, height: 1pt)",
        )
        .unwrap();

        let cache_package = dir.path().join("cache/local/chosen/0.1.0");
        fs::create_dir_all(&cache_package).unwrap();
        fs::write(cache_package.join("lib.typ"), "this is not valid Typst: {").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .package_path(dir.path().join("data"))
            .package_cache_path(dir.path().join("cache"))
            .system_fonts(false)
            .pack()
            .unwrap();

        assert_eq!(outcome.report.packages_vendored.len(), 1);
    }

    #[test]
    fn package_cache_resolves_during_online_and_offline_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"@preview/cached:0.1.0\": mark\n#mark",
        )
        .unwrap();

        let data = dir.path().join("data");
        fs::create_dir_all(&data).unwrap();
        let cache = dir.path().join("cache/preview/cached/0.1.0");
        fs::create_dir_all(&cache).unwrap();
        fs::write(
            cache.join("typst.toml"),
            "[package]\nname = \"cached\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        )
        .unwrap();
        fs::write(
            cache.join("lib.typ"),
            "#let mark = rect(width: 1pt, height: 1pt)",
        )
        .unwrap();

        for offline in [false, true] {
            let outcome = Packer::new(&project, "main.typ")
                .package_path(&data)
                .package_cache_path(dir.path().join("cache"))
                .offline(offline)
                .system_fonts(false)
                .pack()
                .unwrap();

            let spec = &outcome.report.packages_vendored[0];
            assert_eq!(spec.to_string(), "@preview/cached:0.1.0");
            assert!(outcome.pack.package_file(spec, "lib.typ").is_some());
        }
    }

    #[test]
    fn provider_supplied_resource_survives_the_pack_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .pack()
            .unwrap();

        assert_eq!(outcome.report.files, ["main.typ"]);
        assert_eq!(outcome.report.resource_slots, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        assert_eq!(
            pack.resource_slots().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );
        let world = PackWorld::builder(pack.clone()).build().unwrap();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing Resource Slot did not produce a file diagnostic"),
        }

        let world = PackWorld::builder(pack)
            .resource_provider(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build()
            .unwrap();
        let output = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
        assert!(
            std::str::from_utf8(output.artifacts[0].bytes())
                .unwrap()
                .contains("<svg")
        );
    }

    #[test]
    fn explicit_resource_slot_source_project_bytes_are_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();
        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        let (provider, provider_calls) =
            MemoryProjectFile::tracked("assets/logo.png", b"provider bytes".to_vec());

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("assets/logo.png")
            .resource_provider(provider)
            .pack()
            .unwrap();

        assert_eq!(provider_calls.load(Ordering::Relaxed), 0);
        assert_eq!(outcome.report.resource_slots, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());
        assert_eq!(
            outcome.pack.resource_slots().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        let world = PackWorld::builder(pack.clone()).build().unwrap();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing Resource Slot did not produce a file diagnostic"),
        }
        let world = PackWorld::builder(pack.clone())
            .resource_provider(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build()
            .unwrap();
        assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_ok());

        let target = dir.path().join("extracted");
        let report = extract(&pack, &target, &ExtractOptions::default()).unwrap();
        assert_eq!(report.resource_slots, [PathBuf::from("assets/logo.png")]);
        assert!(
            !report
                .written
                .iter()
                .any(|path| path == Path::new("assets/logo.png"))
        );
        assert!(!target.join("assets/logo.png").exists());
    }

    #[test]
    fn explicit_typst_manifest_resource_remains_a_slot_after_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"typst.toml\")").unwrap();
        fs::write(
            project.join("typst.toml"),
            "[package]\nname = \"representative\"\nversion = \"1.0.0\"\nentrypoint = \"main.typ\"\n",
        )
        .unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("typst.toml")
            .pack()
            .unwrap();

        assert_eq!(outcome.report.resource_slots, ["typst.toml"]);
        assert!(!outcome.report.files.iter().any(|path| path == "typst.toml"));
        assert!(outcome.pack.file("typst.toml").is_none());
        assert_eq!(
            outcome.pack.resource_slots().collect::<Vec<_>>(),
            ["typst.toml"]
        );
    }

    #[test]
    fn unrequested_resource_slot_is_still_declared() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("conditional/logo.png")
            .pack()
            .unwrap();

        assert_eq!(
            outcome.pack.resource_slots().collect::<Vec<_>>(),
            ["conditional/logo.png"]
        );
        assert!(outcome.report.warnings.is_empty());
        assert!(outcome.report.compile_warnings.is_empty());
    }

    #[test]
    fn explicitly_included_file_cannot_be_a_resource_slot() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();
        fs::write(project.join("conditional.txt"), "packed").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("conditional.txt")
            .include("conditional.txt")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathRoleConflict { path, .. }
            ))) if path == "conditional.txt"
        ));
    }

    #[test]
    fn resource_slot_entrypoint_declaration_fails_before_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "Hello").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("main.typ")
            .resource_slot("assets")
            .resource_slot("assets/logo.png")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathRoleConflict {
                    ref path,
                    first: PackPathRole::ProjectFile,
                    second: PackPathRole::ResourceSlot,
                }
            ))) if path == "main.typ"
        ));
    }

    #[test]
    fn invalid_explicit_resource_slot_fails_before_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#panic(\"discovery ran\")").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("../outside.bin")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::InvalidPath {
                    role: PackPathRole::ResourceSlot,
                    ..
                }
            )))
        ));
    }

    #[test]
    fn resource_slot_tree_conflicts_fail_before_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#panic(\"discovery ran\")").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("assets")
            .resource_slot("assets/logo.png")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathTreeConflict {
                    ancestor_role: PackPathRole::ResourceSlot,
                    descendant_role: PackPathRole::ResourceSlot,
                    ..
                }
            )))
        ));

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("main.typ/child")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathTreeConflict {
                    ancestor_role: PackPathRole::ProjectFile,
                    descendant_role: PackPathRole::ResourceSlot,
                    ..
                }
            )))
        ));
    }

    #[test]
    fn resource_providers_keep_source_project_files_authoritative() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();

        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        let (fallback, fallback_calls) =
            MemoryProjectFile::tracked("assets/logo.png", b"not the packed image".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(fallback)
            .pack()
            .unwrap();
        assert!(outcome.pack.file("assets/logo.png").is_some());
        assert!(outcome.report.resource_slots.is_empty());
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn discovery_uses_resource_providers_in_registration_order() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (missing, missing_calls) = MemoryProjectFile::tracked("other.bin", Vec::new());
        let (fallback, fallback_calls) =
            MemoryProjectFile::tracked("resource.bin", b"fallback".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(missing)
            .resource_provider(fallback)
            .pack()
            .unwrap();

        // Discovery plus the pre-replay and issuance evidence fences.
        assert_eq!(missing_calls.load(Ordering::Relaxed), 3);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 3);
        assert_eq!(outcome.report.resource_slots, ["resource.bin"]);
        assert!(outcome.pack.file("resource.bin").is_none());
    }

    #[test]
    fn discovery_all_missing_providers_report_the_requested_project_path() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"requested.bin\")").unwrap();

        let (provider, calls) = ErrorProjectLoader::tracked(FileError::NotFound(PathBuf::from(
            "/host-specific/missing.bin",
        )));
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(provider)
            .pack();
        let Err(PackerError::Compile { world, .. }) = result else {
            panic!("missing provider unexpectedly satisfied discovery")
        };

        assert_eq!(
            world.world().file(project_file_id("requested.bin")),
            Err(FileError::NotFound(PathBuf::from("requested.bin")))
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn discovery_propagates_provider_errors_without_falling_through() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (denied, denied_calls) = ErrorProjectLoader::tracked(FileError::AccessDenied);
        let (masked, masked_calls) = MemoryProjectFile::tracked("resource.bin", b"masked".to_vec());
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(denied)
            .resource_provider(masked)
            .pack();
        let Err(PackerError::Compile { world, .. }) = result else {
            panic!("provider error was unexpectedly masked")
        };

        assert_eq!(
            world.world().file(project_file_id("resource.bin")),
            Err(FileError::AccessDenied)
        );
        assert_eq!(denied_calls.load(Ordering::Relaxed), 1);
        assert_eq!(masked_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn registering_a_resource_provider_enables_discovery_inference() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (provider, calls) = MemoryProjectFile::tracked("resource.bin", b"provided".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(provider)
            .pack()
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert_eq!(outcome.report.resource_slots, ["resource.bin"]);
        assert_eq!(
            outcome.pack.resource_slots().collect::<Vec<_>>(),
            ["resource.bin"]
        );
        assert!(outcome.pack.file("resource.bin").is_none());
    }

    #[test]
    fn changed_resource_provider_evidence_prevents_pack_issuance() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();
        let data = Arc::new(Mutex::new(Bytes::new(b"first".to_vec())));

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(MutableProjectFile {
                path: "resource.bin".to_owned(),
                data: Arc::clone(&data),
            })
            .after_discovery_hook(move || {
                *data.lock().expect("mutable provider lock poisoned") =
                    Bytes::new(b"second".to_vec());
            })
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::CreationEvidenceChanged { ref path }) if path == "resource.bin"
        ));
    }

    #[test]
    fn newly_authoritative_project_resource_prevents_pack_issuance() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();
        let appeared = project.join("resource.bin");

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(MemoryProjectFile::new(
                "resource.bin",
                b"representative".to_vec(),
            ))
            .after_discovery_hook(move || fs::write(&appeared, b"representative").unwrap())
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::CreationEvidenceChanged { ref path }) if path == "resource.bin"
        ));
    }

    #[test]
    fn changed_selected_font_evidence_prevents_pack_issuance() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let fonts = dir.path().join("fonts");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&fonts).unwrap();
        let data = embedded_font_data();
        let family = typst::text::FontInfo::new(&data, 0)
            .unwrap()
            .family
            .to_string();
        let font_path = fonts.join("selected.ttf");
        fs::write(&font_path, &data).unwrap();
        fs::write(
            project.join("main.typ"),
            format!("#set text(font: \"{family}\")\nselected"),
        )
        .unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .typst_embedded_fonts(false)
            .font_path(&fonts)
            .after_discovery_hook(move || fs::write(&font_path, b"changed").unwrap())
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::CreationEvidenceChanged { ref path })
                if path.starts_with("font catalog")
        ));
    }

    #[test]
    fn requested_unavailable_resource_slot_has_discovery_guidance() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"branding/logo.bin\")").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("branding/logo.bin")
            .pack();
        let Err(error) = result else {
            panic!("missing representative bytes unexpectedly satisfied discovery")
        };

        assert!(matches!(
            error,
            PackerError::ResourceSlotUnavailable { ref path }
                if path == "branding/logo.bin"
        ));
        let message = error.to_string();
        assert!(message.contains("Resource Provider"), "{message}");
        assert!(message.contains("source project"), "{message}");
        assert!(!message.contains("--resource-path"), "{message}");
        assert!(message.contains("not stored in the Pack"), "{message}");
    }

    #[test]
    fn tolerated_missing_resource_slot_request_still_fails_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#image(\"outer.svg\")").unwrap();
        fs::write(
            project.join("outer.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
<image href="missing.png" width="10" height="10"/>
</svg>"#,
        )
        .unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("missing.png")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::ResourceSlotUnavailable { ref path }) if path == "missing.png"
        ));
    }

    #[test]
    fn timing_export_errors_take_precedence_over_discovery_errors() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"branding/logo.bin\")").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("branding/logo.bin")
            .timings(Some(dir.path().to_path_buf()))
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Timings(ref message)) if message.contains("failed to create file")
        ));
    }

    #[test]
    fn discovery_does_not_mask_a_non_missing_primary_error() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("resource.bin")).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (provider, calls) = MemoryProjectFile::tracked("resource.bin", b"provided".to_vec());
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(provider)
            .pack();

        assert!(matches!(result, Err(PackerError::Compile { .. })));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[cfg(unix)]
    #[test]
    fn unavailable_project_source_evidence_prevents_issuance_without_provider_fallback() {
        use std::io::Write as _;
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"chapter.typ\": chapter\n#chapter",
        )
        .unwrap();

        let chapter = project.join("chapter.typ");
        assert!(
            Command::new("mkfifo")
                .arg(&chapter)
                .status()
                .unwrap()
                .success()
        );
        let writer = std::thread::spawn({
            let chapter = chapter.clone();
            move || {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&chapter)
                    .unwrap();
                fs::remove_file(&chapter).unwrap();
                file.write_all(b"#let chapter = rect(width: 1pt, height: 1pt)")
                    .unwrap();
            }
        });

        let (provider, calls) = MemoryProjectFile::tracked(
            "chapter.typ",
            b"#let chapter = rect(width: 2pt, height: 2pt)".to_vec(),
        );
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_provider(provider)
            .pack();
        writer.join().unwrap();

        assert!(matches!(
            result,
            Err(PackerError::CreationEvidenceChanged { ref path })
                if path == &chapter.display().to_string()
        ));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn concurrent_resource_provenance_is_complete_deduplicated_and_source_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let (raw_entered_tx, raw_entered_rx) = mpsc::channel();
        let (raw_release_tx, raw_release_rx) = mpsc::channel();
        let first = project_file_id("external-a.typ");
        let second = project_file_id("external-b.typ");
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("z.bin")
            .resource_slot("a.bin")
            .resource_slot("z.bin")
            .resource_provider(BlockingProjectFiles {
                paths: vec!["external-a.typ".to_owned(), "external-b.typ".to_owned()],
                data: Bytes::new(b"#let injected = true".to_vec()),
                entered: raw_entered_tx,
                release: Arc::new(Mutex::new(raw_release_rx)),
            })
            .discovery_hook(move |world| {
                let (source_entered_tx, source_entered_rx) = mpsc::channel();
                let (source_release_tx, source_release_rx) = mpsc::channel();
                let source_release_rx = Arc::new(Mutex::new(source_release_rx));
                world.set_source_request_hook(move |source_id| {
                    if source_id == first {
                        let _ = source_entered_tx.send(());
                        source_release_rx
                            .lock()
                            .expect("source release lock poisoned")
                            .recv_timeout(TEST_SYNC_TIMEOUT)
                            .expect("timed out waiting to release source request");
                    }
                });
                let world: &DiscoveryWorld = world;

                std::thread::scope(|scope| {
                    let mut raw_release = ReleaseGuard::new(raw_release_tx.clone(), 2);
                    let mut source_release = ReleaseGuard::new(source_release_tx, 1);
                    let first_file = scope.spawn(|| world.resolve_resource(first));
                    let second_file = scope.spawn(|| world.resolve_resource(second));
                    let entered = BTreeSet::from([
                        raw_entered_rx.recv_timeout(TEST_SYNC_TIMEOUT).unwrap(),
                        raw_entered_rx.recv_timeout(TEST_SYNC_TIMEOUT).unwrap(),
                    ]);
                    assert_eq!(
                        entered,
                        BTreeSet::from(["external-a.typ".to_owned(), "external-b.typ".to_owned(),])
                    );

                    let source = scope.spawn(|| world.source(first));
                    source_entered_rx.recv_timeout(TEST_SYNC_TIMEOUT).unwrap();
                    source_release.release_all();
                    assert!(matches!(
                        source.join().unwrap(),
                        Err(FileError::NotFound(_))
                    ));

                    raw_release.release_all();
                    assert!(first_file.join().unwrap().is_ok());
                    assert!(second_file.join().unwrap().is_ok());
                });
            })
            .pack()
            .unwrap();
        let expected = ["a.bin", "external-a.typ", "external-b.typ", "z.bin"];
        let reread = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();

        assert_eq!(outcome.report.resource_slots, expected);
        assert_eq!(outcome.pack.resource_slots().collect::<Vec<_>>(), expected);
        assert_eq!(reread.resource_slots().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn explicit_and_inferred_provenance_yield_one_packer_declaration() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"shared.bin\")").unwrap();
        let (provider, calls) = MemoryProjectFile::tracked("shared.bin", b"provided".to_vec());

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .resource_slot("shared.bin")
            .resource_provider(provider)
            .pack()
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert_eq!(outcome.report.resource_slots, ["shared.bin"]);
        assert_eq!(
            outcome.pack.resource_slots().collect::<Vec<_>>(),
            ["shared.bin"]
        );
        assert!(outcome.pack.file("shared.bin").is_none());
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn packed_project_compiles_offline() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        // Round-trip through bytes: nothing may depend on the file system.
        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        let world = PackWorld::builder(pack)
            .embedded_fonts(true)
            .build()
            .unwrap();
        let output = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
        assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn font_embedding_skips_typst_embedded_fonts_unless_asked() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());

        let slim = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .embed_fonts(true)
            .pack()
            .unwrap();
        assert!(
            slim.pack.fonts().is_empty(),
            "only Typst embedded fonts are used, so nothing should be embedded"
        );

        let full = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .embed_fonts(true)
            .include_typst_embedded_fonts(true)
            .pack()
            .unwrap();
        assert!(!full.pack.fonts().is_empty());
        // The embedded fonts must load again from the pack.
        let pack = Pack::from_bytes(full.pack.to_bytes().unwrap()).unwrap();
        PackWorld::builder(pack).build().unwrap();
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn html_discovery_embeds_fonts_used_inside_frames() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#html.frame[Hello]").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .target(DiscoveryTarget::Html)
            .feature(typst::Feature::Html)
            .system_fonts(false)
            .embed_fonts(true)
            .include_typst_embedded_fonts(true)
            .pack()
            .unwrap();

        assert!(!outcome.pack.fonts().is_empty());
    }

    #[test]
    fn extract_writes_project_and_packages() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = pack_fixture(dir.path());

        let target = dir.path().join("extracted");
        let report = extract(
            &outcome.pack,
            &target,
            &ExtractOptions {
                packages: true,
                fonts: true,
                force: false,
            },
        )
        .unwrap();
        assert!(!report.written.is_empty());
        assert!(target.join("main.typ").exists());
        assert!(target.join("assets/logo.png").exists());
        assert!(target.join("packages/local/greet/0.1.0/lib.typ").exists());

        // Refuses to overwrite without force.
        let result = extract(&outcome.pack, &target, &ExtractOptions::default());
        assert!(matches!(result, Err(ExtractError::Exists(_))));
    }

    #[test]
    fn extraction_rejects_project_package_conflicts_before_writing() {
        let spec: typst::syntax::package::PackageSpec = "@local/example:1.0.0".parse().unwrap();
        let projected_package = "packages/local/example/1.0.0/lib.typ";
        for project_path in [projected_package, "packages/local/example/1.0.0"] {
            let pack = Pack::builder("main.typ")
                .file("main.typ", b"main".to_vec())
                .unwrap()
                .file(project_path, b"project".to_vec())
                .unwrap()
                .package_file(spec.clone(), "lib.typ", b"package".to_vec())
                .unwrap()
                .build()
                .unwrap();
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("extracted");

            let result = extract(
                &pack,
                &target,
                &ExtractOptions {
                    packages: true,
                    fonts: false,
                    force: true,
                },
            );

            assert!(matches!(
                result,
                Err(ExtractError::PlannedPathConflict { .. })
            ));
            assert!(!target.exists());
        }
    }

    #[test]
    fn extraction_reserves_resource_slot_paths_before_writing() {
        let spec = "@local/example:1.0.0".parse().unwrap();
        let slot = "packages/local/example/1.0.0/lib.typ";
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .package_file(spec, "lib.typ", b"package".to_vec())
            .unwrap()
            .resource_slot(slot)
            .unwrap()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("extracted");

        let result = extract(
            &pack,
            &target,
            &ExtractOptions {
                packages: true,
                fonts: false,
                force: true,
            },
        );

        assert!(matches!(
            result,
            Err(ExtractError::PlannedPathConflict {
                first_role: PackPathRole::PackageFile,
                second_role: PackPathRole::ResourceSlot,
                ..
            }) | Err(ExtractError::PlannedPathConflict {
                first_role: PackPathRole::ResourceSlot,
                second_role: PackPathRole::PackageFile,
                ..
            })
        ));
        assert!(!target.exists());
        assert!(!target.join(slot).exists());
    }

    #[test]
    fn extraction_preflights_existing_destination_conflicts() {
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .file("z.txt", b"packed".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("extracted");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("z.txt"), b"external").unwrap();

        let result = extract(&pack, &target, &ExtractOptions::default());

        assert!(matches!(result, Err(ExtractError::Exists(_))));
        assert!(!target.join("main.typ").exists());
        assert_eq!(fs::read(target.join("z.txt")).unwrap(), b"external");

        let report = extract(
            &pack,
            &target,
            &ExtractOptions {
                force: true,
                ..ExtractOptions::default()
            },
        )
        .unwrap();
        assert_eq!(report.written.len(), 2);
        assert_eq!(fs::read(target.join("z.txt")).unwrap(), b"packed");

        let blocked_target = dir.path().join("blocked");
        fs::create_dir(&blocked_target).unwrap();
        fs::write(blocked_target.join("tree"), b"external").unwrap();
        let nested_pack = Pack::builder("main.typ")
            .file("main.typ", b"main".to_vec())
            .unwrap()
            .file("tree/nested.txt", b"nested".to_vec())
            .unwrap()
            .build()
            .unwrap();

        let result = extract(
            &nested_pack,
            &blocked_target,
            &ExtractOptions {
                force: true,
                ..ExtractOptions::default()
            },
        );

        assert!(matches!(result, Err(ExtractError::DestinationConflict(_))));
        assert!(!blocked_target.join("main.typ").exists());
        assert_eq!(fs::read(blocked_target.join("tree")).unwrap(), b"external");
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn extraction_rejects_project_font_conflicts_before_writing() {
        let data = embedded_font_data();
        let font_pack = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .unwrap()
            .font(data.clone(), 0)
            .unwrap()
            .build()
            .unwrap();
        let font_path = font_pack.fonts()[0].manifest().path().to_owned();
        let pack = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .unwrap()
            .file(&font_path, b"project".to_vec())
            .unwrap()
            .font(data, 0)
            .unwrap()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("extracted");

        let result = extract(
            &pack,
            &target,
            &ExtractOptions {
                packages: false,
                fonts: true,
                force: true,
            },
        );

        assert!(matches!(
            result,
            Err(ExtractError::PlannedPathConflict { .. })
        ));
        assert!(!target.exists());
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn extraction_coalesces_font_faces_sharing_one_data_path() {
        let data = two_face_collection(&embedded_font_data());
        let pack = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .unwrap()
            .font(data.clone(), 0)
            .unwrap()
            .font(data, 1)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(pack.fonts().len(), 2);
        assert_eq!(
            pack.fonts()[0].manifest().path(),
            pack.fonts()[1].manifest().path()
        );
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("extracted");

        let report = extract(
            &pack,
            &target,
            &ExtractOptions {
                packages: false,
                fonts: true,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.written.len(), 2);
        assert!(target.join(pack.fonts()[0].manifest().path()).is_file());
    }

    #[test]
    fn packer_reports_compile_errors() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("broken");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#import \"missing.typ\": x\n").unwrap();

        let result = Packer::new(&project, "main.typ").system_fonts(false).pack();
        assert!(matches!(result, Err(PackerError::Compile { .. })));
    }
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn html_output_is_gated_by_the_html_feature() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", "Hello from HTML".as_bytes().to_vec())
        .unwrap()
        .build()
        .unwrap();

    // Without the feature, Typst itself rejects HTML export.
    let world = PackWorld::builder(pack.clone()).build().unwrap();
    assert!(compile(&world, OutputFormat::Html, &CompileOptions::default()).is_err());

    // With the feature, it produces a document plus an "experimental" warning.
    let world = PackWorld::builder(pack)
        .feature(typst::Feature::Html)
        .build()
        .unwrap();
    let output = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    let html = std::str::from_utf8(output.artifacts[0].bytes()).unwrap();
    assert!(html.contains("<html"));
    assert!(html.contains("Hello from HTML"));
    assert!(!output.warnings.is_empty());
}

#[cfg(feature = "embedded-fonts")]
#[test]
fn pack_font_faces_remain_authoritative_over_host_and_typst_embedded_fonts() {
    let embedded_data = embedded_font_data();
    let mut pack_data = embedded_data.clone();
    pack_data.push(0);
    let pack_font = typst::text::Font::new(Bytes::new(pack_data.clone()), 0).unwrap();
    let mut host_data = embedded_data.clone();
    host_data.push(1);
    let host_font = typst::text::Font::new(Bytes::new(host_data.clone()), 0).unwrap();
    assert_eq!(pack_font.info().family, host_font.info().family);
    assert_eq!(pack_font.info().variant, host_font.info().variant);
    let family = pack_font.info().family.to_lowercase();
    let variant = pack_font.info().variant;
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"Hello".to_vec())
        .unwrap()
        .font(pack_data.clone(), 0)
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack)
        .embedded_fonts(true)
        .extra_fonts([(host_font.clone(), host_font.info().clone())])
        .build()
        .unwrap();
    let selected = world.book().select(&family, variant).unwrap();
    let selected = world.font(selected).unwrap();

    assert_ne!(pack_data, embedded_data);
    assert_ne!(pack_data, host_data);
    assert_eq!(selected.data().as_slice(), pack_data);
}

#[cfg(feature = "fs")]
mod offline {
    use super::*;
    use std::fs;

    #[test]
    fn offline_packing_works_with_local_packages() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = super::fs::fixture(dir.path());
        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .offline(true)
            .pack()
            .unwrap();
        assert_eq!(outcome.report.packages_vendored.len(), 1);
    }

    #[test]
    fn offline_packing_fails_on_uncached_universe_package() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("main.typ"),
            "#import \"@preview/typst-pack-no-such-package:0.0.1\": x\n",
        )
        .unwrap();
        let empty = dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .offline(true)
            .package_path(&empty)
            .package_cache_path(&empty)
            .pack();
        let Err(PackerError::Compile { errors, .. }) = result else {
            panic!("uncached package did not produce a compilation error");
        };
        let messages = errors
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(messages.contains("package not found"), "{messages}");
        assert!(!messages.contains("network"), "{messages}");
    }
}
