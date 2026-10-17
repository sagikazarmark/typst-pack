//! Package Tree reading from the reference filesystem source.

#![cfg(feature = "fs")]
use std::path::{Path, PathBuf};

use typst::syntax::package::PackageSpec;
use typst_pack::fs;
use typst_pack::{LimitExceeded, Pack, ReadLimits};

fn write_package(base: &Path, marker: &[u8]) -> PathBuf {
    let root = base.join("local/example/1.0.0");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("marker.txt"), marker).unwrap();
    root
}

#[test]
fn reading_returns_one_validated_complete_package_tree_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("typst.toml"), b"declaration").unwrap();
    std::fs::write(dir.path().join("lib.typ"), b"library").unwrap();
    std::fs::write(dir.path().join("assets/unused.txt"), b"unused").unwrap();

    let tree = fs::read_package(dir.path(), ReadLimits::default()).unwrap();

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
    std::fs::write(dir.path().join("lib.typ"), b"library").unwrap();

    let error = fs::read_package(
        dir.path(),
        ReadLimits {
            max_file_bytes: 6,
            ..ReadLimits::default()
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        fs::Error::Limit { source, .. }
            if source == LimitExceeded { resource: "file bytes", limit: 6 }
    ));
}

/// A pack that requires `spec` externally.
fn requiring(spec: &PackageSpec) -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .external_package_file(spec.clone(), "lib.typ", Vec::new())
        .build()
        .unwrap()
}

/// Resolves `spec` from a package directory and cache, as compilation does.
fn resolve(data: &Path, cache: &Path, spec: &PackageSpec) -> Result<fs::Resolved, fs::Error> {
    fs::Packer {
        system_fonts: false,
        package_path: Some(data.into()),
        package_cache_path: Some(cache.into()),
        ..fs::Packer::default()
    }
    .resolve(&requiring(spec))
}

#[cfg(unix)]
#[test]
fn a_symlinked_package_root_and_internal_links_are_read() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let development = dir.path().join("dev/example");
    std::fs::create_dir_all(development.join("src")).unwrap();
    std::fs::write(development.join("src/lib.typ"), b"library").unwrap();
    symlink("src/lib.typ", development.join("lib.typ")).unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(data.join("local/example")).unwrap();
    symlink(&development, data.join("local/example/1.0.0")).unwrap();
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let resolved = resolve(&data, &data, &spec).unwrap();
    let tree = &resolved.packages[0].1;

    assert_eq!(tree.file("lib.typ"), Some(&b"library"[..]));
    assert_eq!(tree.file("src/lib.typ"), Some(&b"library"[..]));
}

#[cfg(unix)]
#[test]
fn links_escaping_the_package_root_are_refused() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let root = write_package(dir.path(), b"local");
    std::fs::write(dir.path().join("outside.txt"), b"outside").unwrap();
    symlink(dir.path().join("outside.txt"), root.join("alias.txt")).unwrap();

    let error = fs::read_package(&root, ReadLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        fs::Error::EscapingSymlink { ref path, .. } if path == &root.join("alias.txt")
    ));

    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();
    let error = resolve(dir.path(), dir.path(), &spec).unwrap_err();
    let fs::Error::Package { spec: failed, .. } = &error else {
        panic!("expected a package read failure, got {error}");
    };
    assert_eq!(failed, &spec);
    assert!(error.to_string().contains("alias.txt"), "{error}");
}

#[test]
fn the_package_directory_precedes_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let cache = dir.path().join("cache");
    let data_root = write_package(&data, b"local");
    let cache_root = write_package(&cache, b"cache");
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let resolved = resolve(&data, &cache, &spec).unwrap();
    assert_eq!(
        resolved.packages[0].1.file("marker.txt"),
        Some(&b"local"[..])
    );
    assert_eq!(resolved.package_dirs[&spec.to_string()], data_root);

    std::fs::remove_dir_all(&data_root).unwrap();
    let resolved = resolve(&data, &cache, &spec).unwrap();
    assert_eq!(
        resolved.packages[0].1.file("marker.txt"),
        Some(&b"cache"[..])
    );
    assert_eq!(resolved.package_dirs[&spec.to_string()], cache_root);
}

#[test]
fn a_tree_is_found_under_its_namespace_name_and_version() {
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
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("marker.txt"), key.as_bytes()).unwrap();
    }

    for (text, key) in layouts {
        let spec: PackageSpec = text.parse().unwrap();
        let resolved = resolve(&data, &data, &spec).unwrap();
        assert_eq!(
            resolved.packages[0].1.file("marker.txt"),
            Some(key.as_bytes()),
            "{text}"
        );
        assert_eq!(resolved.package_dirs[text], data.join(key), "{text}");
    }
}

#[test]
fn a_package_found_nowhere_is_left_unresolved() {
    let dir = tempfile::tempdir().unwrap();
    let spec: PackageSpec = "@local/missing:1.0.0".parse().unwrap();

    let resolved = resolve(dir.path(), dir.path(), &spec).unwrap();
    assert!(resolved.packages.is_empty());
    assert!(resolved.package_dirs.is_empty());
}
