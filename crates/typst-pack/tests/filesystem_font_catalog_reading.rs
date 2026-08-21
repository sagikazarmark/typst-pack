//! Font Catalog reading from the reference filesystem sources.

#![cfg(feature = "fs")]

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use std::fs;

use typst_pack::{
    FilesystemFontEntryKind, FilesystemFontIssue, FilesystemFontLimits, FilesystemFontOperation,
    FilesystemFontReadError, FilesystemFontResource, FilesystemFontSource, FontDisposition,
    read_filesystem_fonts,
};

#[cfg(feature = "embedded-fonts")]
use crate::font_bytes::typst_container;
#[cfg(feature = "embedded-fonts")]
use typst_pack::{CanonicalIdentity, FilesystemFontLimitError};

#[cfg(feature = "embedded-fonts")]
fn limits(values: [u64; 4]) -> FilesystemFontLimits {
    FilesystemFontLimits::new(values[0], values[1], values[2], values[3])
}

#[test]
fn the_reference_v1_profile_bounds_every_font_source_resource() {
    let limits = FilesystemFontLimits::reference_v1();

    assert_eq!(limits.visited_entries(), 100_000);
    assert_eq!(limits.accepted_containers(), 16_384);
    assert_eq!(limits.container_bytes(), 256 * 1024 * 1024);
    assert_eq!(limits.total_accepted_bytes(), 2 * 1024 * 1024 * 1024);
}

#[test]
fn every_font_source_ceiling_must_leave_room_for_a_plus_one_probe() {
    let resources = [
        FilesystemFontResource::VisitedEntries,
        FilesystemFontResource::AcceptedContainers,
        FilesystemFontResource::ContainerBytes,
        FilesystemFontResource::TotalAcceptedBytes,
    ];

    for (index, _resource) in resources.into_iter().enumerate() {
        let mut values = [1; 4];
        values[index] = u64::MAX;
        assert!(
            std::panic::catch_unwind(|| {
                FilesystemFontLimits::new(values[0], values[1], values[2], values[3])
            })
            .is_err()
        );
    }
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

    let catalog = read_filesystem_fonts(
        [
            FilesystemFontSource::directory(&first_root, FontDisposition::External),
            FilesystemFontSource::directory(&second_root, FontDisposition::Embedded),
        ],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap();

    assert_eq!(
        catalog
            .entries()
            .iter()
            .map(|entry| (entry.container().identity(), entry.disposition()))
            .collect::<Vec<_>>(),
        [
            (
                CanonicalIdentity::for_font_container_bytes(&first),
                FontDisposition::External,
            ),
            (
                CanonicalIdentity::for_font_container_bytes(&second),
                FontDisposition::External,
            ),
            (
                CanonicalIdentity::for_font_container_bytes(&first),
                FontDisposition::Embedded,
            ),
        ]
    );
}

#[test]
fn no_font_source_is_added_implicitly() {
    let catalog = read_filesystem_fonts(
        std::iter::empty::<FilesystemFontSource>(),
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap();

    assert!(catalog.entries().is_empty());
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn typst_embedded_fonts_join_only_at_the_explicit_source_position() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fonts");
    fs::create_dir_all(&root).unwrap();
    let data = typst_container();
    fs::write(root.join("font.ttf"), &data).unwrap();

    let catalog = read_filesystem_fonts(
        [
            FilesystemFontSource::directory(&root, FontDisposition::External),
            FilesystemFontSource::typst_embedded(FontDisposition::Embedded),
            FilesystemFontSource::directory(&root, FontDisposition::External),
        ],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap();

    let embedded_count = typst_pack::typst_embedded_font_containers().count();
    assert_eq!(catalog.entries().len(), embedded_count + 2);
    assert_eq!(
        catalog.entries()[0].disposition(),
        FontDisposition::External
    );
    assert!(
        catalog.entries()[1..=embedded_count]
            .iter()
            .all(|entry| entry.disposition() == FontDisposition::Embedded)
    );
    assert_eq!(
        catalog.entries()[embedded_count + 1].disposition(),
        FontDisposition::External
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn already_materialized_embedded_containers_do_not_consume_filesystem_limits() {
    let catalog = read_filesystem_fonts(
        [FilesystemFontSource::typst_embedded(
            FontDisposition::External,
        )],
        limits([0, 0, 0, 0]),
    )
    .unwrap();

    assert!(!catalog.entries().is_empty());
}

#[test]
fn unavailable_explicit_roots_retain_the_native_inspection_failure() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");

    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            &missing,
            FontDisposition::External,
        )],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FilesystemFontReadError::Io {
            operation: FilesystemFontOperation::InspectRoot,
            path,
            source,
        } if path == missing && source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[cfg(feature = "embedded-fonts")]
fn assert_limit(
    root: &std::path::Path,
    values: [u64; 4],
    resource: FilesystemFontResource,
    ceiling: u64,
    observed: u64,
) {
    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            root,
            FontDisposition::External,
        )],
        limits(values),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FilesystemFontReadError::Limit {
                source: FilesystemFontLimitError::Exceeded {
                    resource: reported,
                    ceiling: reported_ceiling,
                    observed_at_least,
                },
                ..
            } if reported == resource
                && reported_ceiling == ceiling
                && observed_at_least == observed
        ),
        "unexpected {resource:?} limit error: {error}"
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn generated_boundaries_cover_every_font_source_resource() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let data = typst_container();
    fs::write(root.join("font.ttf"), &data).unwrap();
    let mut second = data;
    second.push(0);
    fs::write(root.join("second.ttf"), &second).unwrap();
    let size = second.len() as u64;
    let cases = [
        (FilesystemFontResource::VisitedEntries, 2),
        (FilesystemFontResource::AcceptedContainers, 2),
        (FilesystemFontResource::ContainerBytes, size),
        (FilesystemFontResource::TotalAcceptedBytes, size * 2 - 1),
    ];
    let exact = [2, 2, size, size * 2 - 1];

    for (index, (resource, observed)) in cases.into_iter().enumerate() {
        for ceiling in [observed + 1, observed] {
            let mut values = exact;
            values[index] = ceiling;
            read_filesystem_fonts(
                [FilesystemFontSource::directory(
                    root,
                    FontDisposition::External,
                )],
                limits(values),
            )
            .unwrap();
        }
        let mut values = exact;
        values[index] -= 1;
        assert_limit(root, values, resource, values[index], observed);
    }
}

#[test]
fn malformed_eligible_font_bytes_retain_the_container_validation_cause() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("a-broken.otf");
    let second = dir.path().join("b-broken.ttf");
    fs::write(&first, b"not a font").unwrap();
    fs::write(&second, b"also not a font").unwrap();

    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            dir.path(),
            FontDisposition::External,
        )],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FilesystemFontReadError::InvalidContainers(ref validation)
            if validation
                .issues()
                .iter()
                .map(|issue| issue.path())
                .collect::<Vec<_>>() == [&first, &second]
    ));
}

#[test]
fn only_supported_font_container_suffixes_are_selected_case_insensitively() {
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

    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            dir.path(),
            FontDisposition::External,
        )],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap_err();

    // Every selected container holds bytes no Font Container accepts, so the
    // validation issues name exactly what selection accepted.
    let FilesystemFontReadError::InvalidContainers(validation) = error else {
        panic!("expected the selected containers to fail validation");
    };
    assert_eq!(
        validation
            .issues()
            .iter()
            .map(|issue| issue.path().to_owned())
            .collect::<Vec<_>>(),
        selected
    );
}

#[cfg(unix)]
#[test]
fn aliases_remain_typed_survey_failures() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("font.ttf");
    let alias = dir.path().join("alias.ttf");
    fs::write(&target, b"font").unwrap();
    symlink(&target, &alias).unwrap();

    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            dir.path(),
            FontDisposition::External,
        )],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FilesystemFontReadError::Survey(ref survey)
            if matches!(survey.issues(), [FilesystemFontIssue::Alias { path }] if path == &alias)
    ));
}

#[cfg(unix)]
#[test]
fn unsupported_eligible_entries_remain_typed_survey_failures() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("blocked.ttf");
    let path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: `path` is a valid NUL-terminated filesystem path.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

    let error = read_filesystem_fonts(
        [FilesystemFontSource::directory(
            dir.path(),
            FontDisposition::External,
        )],
        FilesystemFontLimits::reference_v1(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FilesystemFontReadError::Survey(ref survey)
            if matches!(survey.issues(), [FilesystemFontIssue::UnsupportedEntry {
                path,
                kind: FilesystemFontEntryKind::Fifo,
            }] if path == &fifo)
    ));
}
