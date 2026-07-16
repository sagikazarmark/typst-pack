//! Crate tests.

use crate::*;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use typst::World;
use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use typst_kit::files::FileLoader;

fn tiny_png() -> Vec<u8> {
    tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap()
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

struct ErrorProjectLoader {
    error: FileError,
    calls: Arc<AtomicUsize>,
}

#[cfg(feature = "fs")]
struct BlockingProjectFile {
    path: String,
    data: Bytes,
    entered: Arc<std::sync::Barrier>,
    release: Arc<std::sync::Barrier>,
}

#[cfg(feature = "fs")]
impl FileLoader for BlockingProjectFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().get_without_slash();
        if matches!(id.root(), VirtualRoot::Project) && path == self.path {
            self.entered.wait();
            self.release.wait();
            Ok(self.data.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(path)))
        }
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

#[test]
fn manifest_roundtrip() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"

        [packages]
        vendored = ["@preview/cetz:0.3.4"]
        external = ["@preview/tablex:0.0.9"]

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
    assert!(serialized.contains("external ="));
    assert!(!serialized.contains("unvendored ="));
    let reparsed = PackManifest::from_toml(&serialized).unwrap();
    assert_eq!(manifest, reparsed);
}

#[test]
fn manifest_declarations_are_exposed_read_only_through_accessors() {
    let manifest = PackManifest::from_toml(
        r#"
        format-version = 1

        [project]
        entrypoint = "main.typ"
        external-resources = ["logo.png"]

        [packages]
        vendored = ["@preview/cetz:0.3.4"]
        external = ["@preview/tablex:0.0.9"]

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
        manifest.project().external_resources().collect::<Vec<_>>(),
        ["logo.png"]
    );
    let vendored = "@preview/cetz:0.3.4"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    let unvendored = "@preview/tablex:0.0.9"
        .parse::<typst::syntax::package::PackageSpec>()
        .unwrap();
    assert_eq!(manifest.packages().vendored(), &[vendored]);
    assert_eq!(manifest.packages().unvendored(), &[unvendored]);
    assert_eq!(manifest.fonts()[0].path(), "fonts/test.ttf");
    assert_eq!(manifest.fonts()[0].index(), 2);
    assert_eq!(manifest.fonts()[0].families(), ["Test"]);
    assert_eq!(manifest.metadata().unwrap().name(), Some("Test pack"));
    assert_eq!(manifest.metadata().unwrap().authors(), ["A. U. Thor"]);
}

#[test]
fn old_manifest_has_no_external_project_resources() {
    let manifest =
        PackManifest::from_toml("format-version = 1\n[project]\nentrypoint = \"main.typ\"\n")
            .unwrap();

    assert!(manifest.project().external_resources().next().is_none());
}

#[test]
fn pack_rejects_unsafe_external_project_resource_declarations() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"../secret.png\"]\n";
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, manifest), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::ExternalProjectResource,
            ..
        }))
    ));
}

#[test]
fn pack_normalizes_external_project_resources_deterministically() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"z.png\", \"assets/../logo.png\", \"./logo.png\"]\n";
    let pack = Pack::from_bytes(raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
        ("project/main.typ", b"Hello"),
    ]))
    .unwrap();

    assert_eq!(
        pack.external_resources().collect::<Vec<_>>(),
        ["logo.png", "z.png"]
    );
    assert!(
        pack.manifest()
            .to_toml()
            .contains("external-resources = [\n    \"logo.png\",\n    \"z.png\",\n]")
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
        Pack::builder("main.typ").external_resource("."),
        Err(PackBuildError::Invariant(PackInvariantError::InvalidPath {
            role: PackPathRole::ExternalProjectResource,
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
        .external_resource("assets/logo.png")
        .unwrap()
        .build();
    assert!(matches!(
        built,
        Err(PackBuildError::Invariant(
            PackInvariantError::PathTreeConflict {
                ref ancestor,
                ref descendant,
                ancestor_role: PackPathRole::ProjectFile,
                descendant_role: PackPathRole::ExternalProjectResource,
            }
        )) if ancestor == "assets" && descendant == "assets/logo.png"
    ));

    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"assets/logo.png\"]\n";
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
                descendant_role: PackPathRole::ExternalProjectResource,
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
        .unvendored_package(spec)
        .build();
    assert!(matches!(
        built,
        Err(PackBuildError::Invariant(
            PackInvariantError::PackageRoleConflict(ref spec)
        )) if spec == "@local/example:1.0.0"
    ));

    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[packages]\nvendored = [\"@local/example:1.0.0\"]\nexternal = [\"@local/example:1.0.0\"]\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
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
    let missing_manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[packages]\nvendored = [\"@local/example:1.0.0\"]\n";
    let missing = raw_stored_zip(&[
        (MANIFEST_PATH, missing_manifest),
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
fn pack_construction_rejects_conflicting_package_file_tree_paths() {
    let manifest = b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\n[packages]\nvendored = [\"@local/example:1.0.0\"]\n";
    let bytes = raw_stored_zip(&[
        (MANIFEST_PATH, manifest),
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
fn pack_builder_reports_invalid_font_data_as_a_shared_invariant() {
    assert!(matches!(
        Pack::builder("main.typ").font(b"not a font".to_vec(), 2),
        Err(PackBuildError::Invariant(
            PackInvariantError::InvalidFontInput { index: 2 }
        ))
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
        descendant_role: PackPathRole::ExternalProjectResource,
    };
    assert_eq!(
        tree.to_string(),
        "project file path `assets` conflicts with External Project Resource descendant `assets/logo.png`"
    );

    let font = PackInvariantError::InvalidFontInput { index: 2 };
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

    let _: PackWorld = PackWorld::builder(pack).build();
}

#[test]
fn pack_world_accepts_an_external_resource_reference() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("resource.bin")
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack)
        .external_resource_reference(MemoryProjectFile::new("resource.bin", b"external".to_vec()))
        .build();

    assert_eq!(
        world
            .file(project_file_id("resource.bin"))
            .unwrap()
            .as_slice(),
        b"external"
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

#[test]
fn manually_declared_external_project_resource_survives_archive_roundtrip() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#image(\"assets/logo.png\")".to_vec())
        .unwrap()
        .external_resource("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    assert!(pack.file("assets/logo.png").is_none());
    let pack = Pack::from_bytes(pack.to_bytes().unwrap()).unwrap();
    assert_eq!(
        pack.external_resources().collect::<Vec<_>>(),
        ["assets/logo.png"]
    );
}

#[test]
fn pack_builder_rejects_external_project_resource_file_conflicts() {
    let packed_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("logo.png", tiny_png())
        .unwrap()
        .external_resource("logo.png")
        .unwrap()
        .build();
    let declared_first = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("logo.png")
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
fn read_reports_a_non_utf8_manifest_specifically() {
    let bytes = raw_stored_zip(&[(MANIFEST_PATH, &[0xff]), ("project/main.typ", b"Hello")]);

    assert!(matches!(
        Pack::from_bytes(bytes),
        Err(PackReadError::ManifestNotUtf8(_))
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
fn read_rejects_external_project_resource_file_conflicts() {
    use std::io::Write;

    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(MANIFEST_PATH, options).unwrap();
    zip.write_all(
        b"format-version = 1\n[project]\nentrypoint = \"main.typ\"\nexternal-resources = [\"logo.png\"]\n",
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
fn cli_accepts_source_references_without_a_resource_path_alias() {
    use clap::Parser as _;

    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "create",
            "project",
            "--source-reference",
            "resources/first",
            "--source-reference",
            "resources/second",
            "--external-resource",
            "assets/logo.png",
        ])
        .is_ok()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "compile",
            "project.typk",
            "--source-reference",
            "resources/first",
            "--source-reference",
            "resources/second",
        ])
        .is_ok()
    );
    assert!(
        crate::cli::Cli::try_parse_from([
            "typst-pack",
            "compile",
            "project.typk",
            "--resource-path",
            "resources/old",
        ])
        .is_err()
    );
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

    let world = PackWorld::builder(pack).build();

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
fn declared_external_project_resource_compiles_through_a_reference() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")"
                .to_vec(),
        )
        .unwrap()
        .external_resource("assets/logo.png")
        .unwrap()
        .build()
        .unwrap();

    let world = PackWorld::builder(pack.clone())
        .external_resource_reference(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .build();
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
        .external_resource_reference(MemoryProjectFile::new("assets/logo.png", tiny_png()))
        .feature(typst::Feature::Html)
        .build();
    let html = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    assert!(
        std::str::from_utf8(html.artifacts[0].bytes())
            .unwrap()
            .contains("<html")
    );
}

#[test]
fn external_resource_reference_cannot_supply_typst_source() {
    let pack = Pack::builder("main.typ")
        .file(
            "main.typ",
            b"#let _ = read(\"external.typ\")\n#import \"external.typ\": mark\n#mark".to_vec(),
        )
        .unwrap()
        .external_resource("external.typ")
        .unwrap()
        .build()
        .unwrap();
    let world = PackWorld::builder(pack)
        .external_resource_reference(MemoryProjectFile::new(
            "external.typ",
            b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
        ))
        .build();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
}

#[test]
fn external_resource_references_follow_registration_order() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("resource.bin")
        .unwrap()
        .build()
        .unwrap();
    let id = project_file_id("resource.bin");

    let (first, first_calls) = MemoryProjectFile::tracked("resource.bin", b"first".to_vec());
    let (second, second_calls) = MemoryProjectFile::tracked("resource.bin", b"second".to_vec());
    let world = PackWorld::builder(pack.clone())
        .external_resource_reference(first)
        .external_resource_reference(second)
        .build();
    assert_eq!(world.file(id).unwrap().as_slice(), b"first");
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);

    let (missing, missing_calls) = MemoryProjectFile::tracked("other.bin", Vec::new());
    let (fallback, fallback_calls) =
        MemoryProjectFile::tracked("resource.bin", b"fallback".to_vec());
    let world = PackWorld::builder(pack.clone())
        .external_resource_reference(missing)
        .external_resource_reference(fallback)
        .build();
    assert_eq!(world.file(id).unwrap().as_slice(), b"fallback");
    assert_eq!(missing_calls.load(Ordering::Relaxed), 1);
    assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);

    let (denied, denied_calls) = ErrorProjectLoader::tracked(FileError::AccessDenied);
    let (masked, masked_calls) = MemoryProjectFile::tracked("resource.bin", b"masked".to_vec());
    let world = PackWorld::builder(pack)
        .external_resource_reference(denied)
        .external_resource_reference(masked)
        .build();
    assert_eq!(world.file(id), Err(FileError::AccessDenied));
    assert_eq!(denied_calls.load(Ordering::Relaxed), 1);
    assert_eq!(masked_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn all_missing_external_resource_references_report_the_requested_project_path_lazily() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("requested.bin")
        .unwrap()
        .external_resource("unused.bin")
        .unwrap()
        .build()
        .unwrap();
    let (reference, calls) = ErrorProjectLoader::tracked(FileError::NotFound(PathBuf::from(
        "/host-specific/missing.bin",
    )));
    let world = PackWorld::builder(pack)
        .external_resource_reference(reference)
        .build();

    assert_eq!(
        world.file(project_file_id("requested.bin")),
        Err(FileError::NotFound(PathBuf::from("requested.bin")))
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn source_requests_do_not_consult_external_resource_references() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("external.typ")
        .unwrap()
        .build()
        .unwrap();
    let (reference, calls) = MemoryProjectFile::tracked("external.typ", b"injected".to_vec());
    let world = PackWorld::builder(pack)
        .external_resource_reference(reference)
        .build();

    assert!(matches!(
        world.source(project_file_id("external.typ")),
        Err(FileError::NotFound(_))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn packed_and_undeclared_project_paths_do_not_consult_external_resource_references() {
    let packed = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .file("resource.bin", b"packed".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("resource.bin", b"external".to_vec());
    let world = PackWorld::builder(packed)
        .external_resource_reference(loader)
        .build();
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
    let (loader, calls) = MemoryProjectFile::tracked("missing.bin", b"external".to_vec());
    let world = PackWorld::builder(undeclared)
        .external_resource_reference(loader)
        .build();
    assert!(matches!(
        world.file(project_file_id("missing.bin")),
        Err(FileError::NotFound(_))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn package_requests_do_not_consult_external_resource_references() {
    use std::str::FromStr as _;

    let pack = Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .unwrap()
        .external_resource("lib.typ")
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("lib.typ", b"external".to_vec());
    let world = PackWorld::builder(pack)
        .external_resource_reference(loader)
        .build();
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
fn project_requests_do_not_consult_package_loader() {
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#read(\"missing.txt\")".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let (loader, calls) = MemoryProjectFile::tracked("missing.txt", b"host file".to_vec());
    let world = PackWorld::builder(pack).package_loader(loader).build();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn vendored_package_compiles_without_consulting_package_loader() {
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
    let (loader, calls) = MemoryProjectFile::tracked("unused", Vec::new());
    let world = PackWorld::builder(pack).package_loader(loader).build();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_ok());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn missing_vendored_package_file_does_not_fall_through_to_package_loader() {
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
    let (loader, calls) = MemoryProjectFile::tracked(
        "missing.typ",
        b"#let mark = rect(width: 1pt, height: 1pt)".to_vec(),
    );
    let world = PackWorld::builder(pack).package_loader(loader).build();

    assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[cfg(feature = "fs")]
mod fs {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn system_package_loader_rejects_project_requests() {
        use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

        let dir = tempfile::tempdir().unwrap();
        let packages = SystemPackages::from_parts(
            Some(FsPackages::new(dir.path().join("packages"))),
            None,
            UniversePackages::new(OfflineDownloader),
        );
        let loader = SystemPackageLoader(packages);

        assert_eq!(
            loader.load(project_file_id("project.typ")),
            Err(FileError::NotFound(PathBuf::from("project.typ")))
        );
    }

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
    fn externally_loaded_project_resource_survives_the_pack_lifecycle() {
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
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .pack()
            .unwrap();

        assert_eq!(outcome.report.files, ["main.typ"]);
        assert_eq!(outcome.report.external_resources, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        assert_eq!(
            pack.external_resources().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );
        let world = PackWorld::builder(pack.clone()).build();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing External Project Resource did not produce a file diagnostic"),
        }

        let world = PackWorld::builder(pack)
            .external_resource_reference(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build();
        let output = compile(&world, OutputFormat::Svg, &CompileOptions::default()).unwrap();
        assert!(
            std::str::from_utf8(output.artifacts[0].bytes())
                .unwrap()
                .contains("<svg")
        );
    }

    #[test]
    fn explicitly_external_source_project_resource_is_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();
        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("assets/logo.png")
            .pack()
            .unwrap();

        assert_eq!(outcome.report.external_resources, ["assets/logo.png"]);
        assert!(outcome.pack.file("assets/logo.png").is_none());
        assert_eq!(
            outcome.pack.external_resources().collect::<Vec<_>>(),
            ["assets/logo.png"]
        );

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();
        let world = PackWorld::builder(pack.clone()).build();
        match compile(&world, OutputFormat::Svg, &CompileOptions::default()) {
            Err(CompileError::Diagnostics { errors, .. }) => assert!(
                errors
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("file not found"))
            ),
            _ => panic!("missing External Project Resource did not produce a file diagnostic"),
        }
        let world = PackWorld::builder(pack.clone())
            .external_resource_reference(MemoryProjectFile::new("assets/logo.png", tiny_png()))
            .build();
        assert!(compile(&world, OutputFormat::Svg, &CompileOptions::default()).is_ok());

        let target = dir.path().join("extracted");
        let report = extract(&pack, &target, &ExtractOptions::default()).unwrap();
        assert!(
            !report
                .written
                .iter()
                .any(|path| path == Path::new("assets/logo.png"))
        );
        assert!(!target.join("assets/logo.png").exists());
    }

    #[test]
    fn unrequested_external_project_resource_is_still_declared() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("conditional/logo.png")
            .pack()
            .unwrap();

        assert_eq!(
            outcome.pack.external_resources().collect::<Vec<_>>(),
            ["conditional/logo.png"]
        );
    }

    #[test]
    fn explicitly_included_file_cannot_be_an_external_project_resource() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();
        fs::write(project.join("conditional.txt"), "packed").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("conditional.txt")
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
    fn external_entrypoint_declaration_fails_before_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "Hello").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("main.typ")
            .external_resource("assets")
            .external_resource("assets/logo.png")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathRoleConflict {
                    ref path,
                    first: PackPathRole::ProjectFile,
                    second: PackPathRole::ExternalProjectResource,
                }
            ))) if path == "main.typ"
        ));
    }

    #[test]
    fn external_resource_tree_conflicts_fail_before_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#panic(\"discovery ran\")").unwrap();

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("assets")
            .external_resource("assets/logo.png")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathTreeConflict {
                    ancestor_role: PackPathRole::ExternalProjectResource,
                    descendant_role: PackPathRole::ExternalProjectResource,
                    ..
                }
            )))
        ));

        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("main.typ/child")
            .pack();

        assert!(matches!(
            result,
            Err(PackerError::Build(PackBuildError::Invariant(
                PackInvariantError::PathTreeConflict {
                    ancestor_role: PackPathRole::ProjectFile,
                    descendant_role: PackPathRole::ExternalProjectResource,
                    ..
                }
            )))
        ));
    }

    #[test]
    fn discovery_policy_keeps_source_project_files_authoritative() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(
            project.join("main.typ"),
            "#set page(width: 20pt, height: 20pt, margin: 0pt)\n#image(\"assets/logo.png\")",
        )
        .unwrap();

        let (strict_loader, strict_calls) =
            MemoryProjectFile::tracked("assets/logo.png", tiny_png());
        let strict = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource_reference(strict_loader)
            .pack();
        assert!(matches!(strict, Err(PackerError::Compile { .. })));
        assert_eq!(strict_calls.load(Ordering::Relaxed), 0);

        fs::write(project.join("assets/logo.png"), tiny_png()).unwrap();
        let (fallback, fallback_calls) =
            MemoryProjectFile::tracked("assets/logo.png", b"not the packed image".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(fallback)
            .pack()
            .unwrap();
        assert!(outcome.pack.file("assets/logo.png").is_some());
        assert!(outcome.report.external_resources.is_empty());
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn discovery_uses_external_resource_references_in_registration_order() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (missing, missing_calls) = MemoryProjectFile::tracked("other.bin", Vec::new());
        let (fallback, fallback_calls) =
            MemoryProjectFile::tracked("resource.bin", b"fallback".to_vec());
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(missing)
            .external_resource_reference(fallback)
            .pack()
            .unwrap();

        assert_eq!(missing_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);
        assert_eq!(outcome.report.external_resources, ["resource.bin"]);
        assert!(outcome.pack.file("resource.bin").is_none());
    }

    #[test]
    fn strict_discovery_does_not_resolve_an_explicit_missing_resource() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (reference, calls) = MemoryProjectFile::tracked("resource.bin", b"external".to_vec());
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .external_resource("resource.bin")
            .external_resource_reference(reference)
            .pack();

        assert!(matches!(result, Err(PackerError::Compile { .. })));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn discovery_does_not_mask_a_non_missing_primary_error() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("resource.bin")).unwrap();
        fs::write(project.join("main.typ"), "#read(\"resource.bin\")").unwrap();

        let (reference, calls) = MemoryProjectFile::tracked("resource.bin", b"external".to_vec());
        let result = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(reference)
            .pack();

        assert!(matches!(result, Err(PackerError::Compile { .. })));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[cfg(unix)]
    #[test]
    fn discovery_reads_project_sources_once_without_external_fallback() {
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

        let (loader, calls) = MemoryProjectFile::tracked(
            "chapter.typ",
            b"#let chapter = rect(width: 2pt, height: 2pt)".to_vec(),
        );
        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(loader)
            .pack()
            .unwrap();
        writer.join().unwrap();

        assert!(outcome.pack.file("chapter.typ").is_some());
        assert!(outcome.report.external_resources.is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn concurrent_external_file_load_cannot_satisfy_discovery_source_request() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#rect(width: 1pt, height: 1pt)").unwrap();

        let raw_entered = Arc::new(std::sync::Barrier::new(2));
        let raw_release = Arc::new(std::sync::Barrier::new(2));
        let mut outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource_reference(BlockingProjectFile {
                path: "external.typ".to_owned(),
                data: Bytes::new(b"#let injected = true".to_vec()),
                entered: Arc::clone(&raw_entered),
                release: Arc::clone(&raw_release),
            })
            .pack()
            .unwrap();
        let id = project_file_id("external.typ");
        let source_entered = Arc::new(std::sync::Barrier::new(2));
        let source_release = Arc::new(std::sync::Barrier::new(2));
        outcome.world.set_source_request_hook({
            let source_entered = Arc::clone(&source_entered);
            let source_release = Arc::clone(&source_release);
            move |source_id| {
                if source_id == id {
                    source_entered.wait();
                    source_release.wait();
                }
            }
        });

        std::thread::scope(|scope| {
            let file = scope.spawn(|| outcome.world.file(id));
            raw_entered.wait();
            let source = scope.spawn(|| outcome.world.source(id));
            source_entered.wait();

            raw_release.wait();
            assert!(file.join().unwrap().is_ok());
            source_release.wait();

            assert!(matches!(
                source.join().unwrap(),
                Err(FileError::NotFound(_))
            ));
        });
    }

    #[test]
    fn explicit_and_inferred_provenance_yield_one_packer_declaration() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.typ"), "#read(\"shared.bin\")").unwrap();
        let (reference, calls) = MemoryProjectFile::tracked("shared.bin", b"external".to_vec());

        let outcome = Packer::new(&project, "main.typ")
            .system_fonts(false)
            .project_resource_policy(ProjectResourcePolicy::AllowExternalFallback)
            .external_resource("shared.bin")
            .external_resource_reference(reference)
            .pack()
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(outcome.report.external_resources, ["shared.bin"]);
        assert_eq!(
            outcome.pack.external_resources().collect::<Vec<_>>(),
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
        let world = PackWorld::builder(pack).build();
        let output = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap();
        assert!(output.artifacts[0].bytes().starts_with(b"%PDF"));
    }

    #[cfg(feature = "embedded-fonts")]
    #[test]
    fn unvendored_packages_resolve_through_package_loader() {
        let dir = tempfile::tempdir().unwrap();
        let (project, packages) = fixture(dir.path());
        let outcome = Packer::new(&project, "main.typ")
            .package_path(&packages)
            .system_fonts(false)
            .vendor_packages(false)
            .pack()
            .unwrap();
        assert!(outcome.report.packages_vendored.is_empty());
        assert_eq!(outcome.report.packages_unvendored.len(), 1);

        let pack = Pack::from_bytes(outcome.pack.to_bytes().unwrap()).unwrap();

        // Without a loader, compilation must fail...
        let world = PackWorld::builder(pack.clone()).build();
        let error = compile(&world, OutputFormat::Pdf, &CompileOptions::default()).unwrap_err();
        let CompileError::Diagnostics { errors, .. } = error else {
            panic!("unvendored package did not produce a compilation diagnostic");
        };
        let messages = errors
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            messages.contains("no package loader is configured"),
            "{messages}"
        );

        // ...with a loader pointed at the package path, it succeeds.
        use typst_kit::downloader::SystemDownloader;
        use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};
        let loader = SystemPackageLoader(SystemPackages::from_parts(
            Some(FsPackages::new(&packages)),
            None,
            UniversePackages::new(SystemDownloader::new("typst-pack-test")),
        ));
        let world = PackWorld::builder(pack).package_loader(loader).build();
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
        PackWorld::builder(pack).build();
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
    let world = PackWorld::builder(pack.clone()).build();
    assert!(compile(&world, OutputFormat::Html, &CompileOptions::default()).is_err());

    // With the feature, it produces a document plus an "experimental" warning.
    let world = PackWorld::builder(pack)
        .feature(typst::Feature::Html)
        .build();
    let output = compile(&world, OutputFormat::Html, &CompileOptions::default()).unwrap();
    let html = std::str::from_utf8(output.artifacts[0].bytes()).unwrap();
    assert!(html.contains("<html"));
    assert!(html.contains("Hello from HTML"));
    assert!(!output.warnings.is_empty());
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
