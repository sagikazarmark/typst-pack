//! Package Tree reading from the reference filesystem source.

#![cfg(feature = "fs")]

use std::fs;
use std::path::{Path, PathBuf};

use typst::syntax::package::PackageSpec;
use typst_pack::{
    FilesystemPackageAuthority, FilesystemPackageAuthorityReadError, FilesystemReadError,
    LimitExceeded, PackageReadFailureReason, ReadLimits, read_filesystem_package,
};

fn write_package(base: &Path, marker: &[u8]) -> PathBuf {
    let root = base.join("local/example/1.0.0");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("marker.txt"), marker).unwrap();
    root
}

#[test]
fn reading_returns_one_validated_complete_package_tree_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("assets")).unwrap();
    fs::write(dir.path().join("typst.toml"), b"declaration").unwrap();
    fs::write(dir.path().join("lib.typ"), b"library").unwrap();
    fs::write(dir.path().join("assets/unused.txt"), b"unused").unwrap();

    let tree = read_filesystem_package(dir.path(), ReadLimits::default()).unwrap();

    assert_eq!(
        tree.files().collect::<Vec<_>>(),
        [
            ("assets/unused.txt", &b"unused"[..]),
            ("lib.typ", &b"library"[..]),
            ("typst.toml", &b"declaration"[..]),
        ]
    );
}

#[test]
fn package_reading_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.typ"), b"library").unwrap();

    let error = read_filesystem_package(
        dir.path(),
        ReadLimits {
            max_file_bytes: 6,
            ..ReadLimits::default()
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FilesystemReadError::Limit { source, .. }
            if source == LimitExceeded { resource: "file bytes", limit: 6 }
    ));
}

#[cfg(unix)]
#[test]
fn a_symlinked_package_root_and_internal_links_are_read() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let development = dir.path().join("dev/example");
    fs::create_dir_all(development.join("src")).unwrap();
    fs::write(development.join("src/lib.typ"), b"library").unwrap();
    symlink("src/lib.typ", development.join("lib.typ")).unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(data.join("local/example")).unwrap();
    symlink(&development, data.join("local/example/1.0.0")).unwrap();
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let read = FilesystemPackageAuthority::new(Some(&data), None, true)
        .read(&spec)
        .unwrap();

    assert_eq!(read.tree().file("lib.typ"), Some(&b"library"[..]));
    assert_eq!(read.tree().file("src/lib.typ"), Some(&b"library"[..]));
}

#[cfg(unix)]
#[test]
fn links_escaping_the_package_root_are_refused() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let root = write_package(dir.path(), b"local");
    fs::write(dir.path().join("outside.txt"), b"outside").unwrap();
    symlink(dir.path().join("outside.txt"), root.join("alias.txt")).unwrap();

    let error = read_filesystem_package(&root, ReadLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        FilesystemReadError::EscapingSymlink { ref path, .. } if path == &root.join("alias.txt")
    ));

    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let error = FilesystemPackageAuthority::new(Some(dir.path()), None, true)
        .read(&spec)
        .unwrap_err();
    let FilesystemPackageAuthorityReadError::Filesystem { failure, source } = error else {
        panic!("expected a typed filesystem read cause");
    };
    assert_eq!(failure.spec(), &spec);
    assert!(matches!(
        source.as_ref(),
        FilesystemReadError::EscapingSymlink { .. }
    ));
}

#[test]
fn concrete_authority_makes_local_cache_and_offline_precedence_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let cache = dir.path().join("cache");
    let data_root = write_package(&data, b"local");
    write_package(&cache, b"cache");
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let read = FilesystemPackageAuthority::new(Some(&data), Some(&cache), true)
        .read(&spec)
        .unwrap();

    assert_eq!(read.tree().file("marker.txt"), Some(&b"local"[..]));
    assert_eq!(read.root(), Some(data_root.as_path()));

    fs::remove_dir_all(&data_root).unwrap();
    let read = FilesystemPackageAuthority::new(Some(&data), Some(&cache), true)
        .read(&spec)
        .unwrap();
    assert_eq!(read.tree().file("marker.txt"), Some(&b"cache"[..]));
}

#[test]
fn concrete_authority_finds_a_tree_under_its_namespace_name_and_version() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let layouts = [
        ("@local/example:1.0.0", "local/example/1.0.0"),
        ("@local/example:2.3.4", "local/example/2.3.4"),
        ("@local/nested-name:1.0.0", "local/nested-name/1.0.0"),
        ("@preview/example:1.0.0", "preview/example/1.0.0"),
    ];

    for (_, key) in layouts {
        let root = data.join(key);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("marker.txt"), key.as_bytes()).unwrap();
    }

    for (text, key) in layouts {
        let spec: PackageSpec = text.parse().unwrap();

        let read = FilesystemPackageAuthority::new(Some(&data), Some(&data), true)
            .read(&spec)
            .unwrap();

        assert_eq!(
            read.tree().file("marker.txt"),
            Some(key.as_bytes()),
            "{text}"
        );
        assert_eq!(read.root(), Some(data.join(key).as_path()), "{text}");
    }
}

#[test]
fn concrete_authority_failure_retains_the_exact_specification() {
    let dir = tempfile::tempdir().unwrap();
    let spec: PackageSpec = "@local/missing:1.0.0".parse().unwrap();

    let error = FilesystemPackageAuthority::new(Some(dir.path()), Some(dir.path()), true)
        .read(&spec)
        .unwrap_err();
    let failure = error.failure();

    assert_eq!(failure.spec(), &spec);
    assert_eq!(failure.reason(), &PackageReadFailureReason::NotFound);
}
