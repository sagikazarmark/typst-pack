//! Font Catalog reading from the reference filesystem sources.

#![cfg(feature = "fs")]

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use std::fs;
use std::path::{Path, PathBuf};

use typst_pack::{
    FilesystemFontSource, FilesystemFonts, FilesystemReadError, FontDisposition, ReadLimits,
    read_filesystem_fonts,
};

#[cfg(feature = "embedded-fonts")]
use crate::font_bytes::typst_container;
#[cfg(feature = "embedded-fonts")]
use typst_pack::{Digest, LimitExceeded};

fn read_directory(root: &Path, limits: ReadLimits) -> Result<FilesystemFonts, FilesystemReadError> {
    read_filesystem_fonts(
        [FilesystemFontSource::directory(
            root,
            FontDisposition::External,
        )],
        limits,
    )
}

fn skipped_paths(fonts: &FilesystemFonts) -> Vec<PathBuf> {
    fonts
        .skipped
        .iter()
        .map(|skipped| match skipped {
            FilesystemReadError::Io { path, .. }
            | FilesystemReadError::Unsupported { path }
            | FilesystemReadError::InvalidFont { path, .. } => path.clone(),
            other => panic!("unexpected skipped entry {other}"),
        })
        .collect()
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn configured_sources_and_paths_compose_in_order_with_explicit_dispositions() {
    let dir = tempfile::tempdir().unwrap();
    let first_root = dir.path().join("first");
    let second_root = dir.path().join("second");
    fs::create_dir_all(&first_root).unwrap();
    fs::create_dir_all(&second_root).unwrap();
    let first = typst_container();
    let mut second = first.clone();
    second.push(0);
    fs::write(first_root.join("b.ttf"), &second).unwrap();
    fs::write(first_root.join("a.ttf"), &first).unwrap();
    fs::write(second_root.join("same.ttf"), &first).unwrap();

    let fonts = read_filesystem_fonts(
        [
            FilesystemFontSource::directory(&first_root, FontDisposition::External),
            FilesystemFontSource::directory(&second_root, FontDisposition::Embedded),
        ],
        ReadLimits::default(),
    )
    .unwrap();

    assert_eq!(
        fonts
            .catalog
            .entries()
            .iter()
            .map(|entry| (entry.container().digest(), entry.disposition()))
            .collect::<Vec<_>>(),
        [
            (Digest::of(&first), FontDisposition::External,),
            (Digest::of(&second), FontDisposition::External,),
            (Digest::of(&first), FontDisposition::Embedded,),
        ]
    );
    assert!(fonts.skipped.is_empty());
}

#[test]
fn no_font_source_is_added_implicitly() {
    let fonts = read_filesystem_fonts(
        std::iter::empty::<FilesystemFontSource>(),
        ReadLimits::default(),
    )
    .unwrap();

    assert!(fonts.catalog.entries().is_empty());
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn typst_embedded_fonts_join_only_at_the_explicit_source_position() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fonts");
    fs::create_dir_all(&root).unwrap();
    let data = typst_container();
    fs::write(root.join("font.ttf"), &data).unwrap();

    let fonts = read_filesystem_fonts(
        [
            FilesystemFontSource::directory(&root, FontDisposition::External),
            FilesystemFontSource::typst_embedded(FontDisposition::Embedded),
            FilesystemFontSource::directory(&root, FontDisposition::External),
        ],
        ReadLimits::default(),
    )
    .unwrap();
    let entries = fonts.catalog.entries();

    let embedded_count = typst_pack::typst_embedded_font_containers().count();
    assert_eq!(entries.len(), embedded_count + 2);
    assert_eq!(entries[0].disposition(), FontDisposition::External);
    assert!(
        entries[1..=embedded_count]
            .iter()
            .all(|entry| entry.disposition() == FontDisposition::Embedded)
    );
    assert_eq!(
        entries[embedded_count + 1].disposition(),
        FontDisposition::External
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn font_reading_is_bounded_but_embedded_fonts_are_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let data = typst_container();
    fs::write(dir.path().join("font.ttf"), &data).unwrap();
    let size = data.len() as u64;

    let error = read_directory(
        dir.path(),
        ReadLimits {
            max_file_bytes: size - 1,
            ..ReadLimits::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FilesystemReadError::Limit { source, .. }
            if source == LimitExceeded { resource: "file bytes", limit: size - 1 }
    ));

    let empty = ReadLimits {
        max_entries: 0,
        max_file_bytes: 0,
        max_total_bytes: 0,
    };
    let fonts = read_filesystem_fonts(
        [FilesystemFontSource::typst_embedded(
            FontDisposition::External,
        )],
        empty,
    )
    .unwrap();
    assert!(!fonts.catalog.entries().is_empty());
}

#[test]
fn unavailable_explicit_roots_retain_the_native_failure() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");

    let error = read_directory(&missing, ReadLimits::default()).unwrap_err();

    assert!(matches!(
        error,
        FilesystemReadError::Io { path, source }
            if path == missing && source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn only_font_suffixes_are_read_and_unparseable_fonts_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let candidates = [
        ("lower-ttf.ttf", true),
        ("upper-ttf.TTF", true),
        ("lower-ttc.ttc", true),
        ("mixed-ttc.Ttc", true),
        ("lower-otf.otf", true),
        ("upper-otf.OTF", true),
        ("lower-otc.otc", true),
        ("mixed-otc.oTc", true),
        ("ignored.woff", false),
        ("ignored.woff2", false),
        ("ignored.txt", false),
        ("ignored.ttf.txt", false),
        ("ignored", false),
        (".ttf", false),
    ];
    for (name, _) in candidates {
        fs::write(dir.path().join(name), b"not a font").unwrap();
    }
    let mut selected = candidates
        .iter()
        .filter(|(_, eligible)| *eligible)
        .map(|(name, _)| dir.path().join(name))
        .collect::<Vec<_>>();
    selected.sort();

    let fonts = read_directory(dir.path(), ReadLimits::default()).unwrap();

    // Every selected file holds bytes no Font Container accepts, so the
    // skipped entries name exactly what selection accepted.
    assert!(fonts.catalog.entries().is_empty());
    assert!(
        fonts
            .skipped
            .iter()
            .all(|skipped| matches!(skipped, FilesystemReadError::InvalidFont { .. }))
    );
    assert_eq!(skipped_paths(&fonts), selected);
}

#[cfg(unix)]
mod links {
    use super::*;

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn links_are_followed_anywhere_and_bad_entries_never_fail_the_catalog() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fonts");
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(root.join("unreadable")).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        let data = typst_container();
        fs::write(elsewhere.join("linked.ttf"), &data).unwrap();
        fs::write(elsewhere.join("README"), b"not a font").unwrap();
        symlink(elsewhere.join("linked.ttf"), root.join("a-linked.ttf")).unwrap();
        symlink(&elsewhere, root.join("b-directory")).unwrap();
        symlink(elsewhere.join("missing.ttf"), root.join("c-broken.ttf")).unwrap();
        symlink(elsewhere.join("missing"), root.join("d-broken-other")).unwrap();
        fs::write(root.join("e-broken.ttf"), b"not a font").unwrap();
        let _socket = UnixListener::bind(root.join("f-socket.ttf")).unwrap();
        fs::write(root.join("unreadable/hidden.ttf"), &data).unwrap();
        let mut permissions = fs::metadata(root.join("unreadable")).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o000);
        fs::set_permissions(root.join("unreadable"), permissions).unwrap();
        // Privileged users read the directory anyway.
        let unreadable = fs::read_dir(root.join("unreadable")).is_err();

        let fonts = read_directory(&root, ReadLimits::default()).unwrap();

        let mut permissions = fs::metadata(root.join("unreadable")).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        fs::set_permissions(root.join("unreadable"), permissions).unwrap();

        let identity = Digest::of(&data);
        let found = fonts
            .catalog
            .entries()
            .iter()
            .filter(|entry| entry.container().digest() == identity)
            .count();
        assert_eq!(found, if unreadable { 2 } else { 3 });
        let mut expected = vec![
            root.join("c-broken.ttf"),
            root.join("f-socket.ttf"),
            root.join("e-broken.ttf"),
        ];
        if unreadable {
            expected.insert(2, root.join("unreadable"));
        }
        assert_eq!(skipped_paths(&fonts), expected);
    }

    #[test]
    fn fifos_with_font_names_are_skipped_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("blocked.ttf");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(status.success());

        let fonts = read_directory(dir.path(), ReadLimits::default()).unwrap();

        assert!(matches!(
            fonts.skipped.as_slice(),
            [FilesystemReadError::Unsupported { path }] if path == &fifo
        ));
    }
}
