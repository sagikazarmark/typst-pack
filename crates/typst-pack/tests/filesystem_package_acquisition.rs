//! Package Tree gathering from the reference filesystem source.

#![cfg(feature = "fs")]

use std::fs;

use typst::syntax::package::PackageSpec;
use typst_pack::{
    FilesystemPackageAcquisitionError, FilesystemPackageAuthority, FilesystemPackageGatherError,
    FilesystemPackageLimitError, FilesystemPackageLimits, FilesystemPackageLimitsError,
    FilesystemPackageResource, PackageAcquisitionFailureReason, gather_filesystem_package,
};

fn limits(values: [u64; 4]) -> FilesystemPackageLimits {
    FilesystemPackageLimits::new(values[0], values[1], values[2], values[3]).unwrap()
}

fn write_package(base: &std::path::Path, marker: &[u8]) -> std::path::PathBuf {
    let root = base.join("local/example/1.0.0");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("marker.txt"), marker).unwrap();
    root
}

#[test]
fn the_reference_v1_profile_bounds_every_package_source_resource() {
    let limits = FilesystemPackageLimits::reference_v1();

    assert_eq!(limits.visited_entries(), 100_000);
    assert_eq!(limits.selected_files(), 50_000);
    assert_eq!(limits.selected_file_bytes(), 64 * 1024 * 1024);
    assert_eq!(limits.package_tree_bytes(), 512 * 1024 * 1024);
}

#[test]
fn every_package_source_ceiling_must_leave_room_for_a_plus_one_probe() {
    let resources = [
        FilesystemPackageResource::VisitedEntries,
        FilesystemPackageResource::SelectedFiles,
        FilesystemPackageResource::SelectedFileBytes,
        FilesystemPackageResource::PackageTreeBytes,
    ];

    for (index, resource) in resources.into_iter().enumerate() {
        let mut values = [1; 4];
        values[index] = u64::MAX;
        assert_eq!(
            FilesystemPackageLimits::new(values[0], values[1], values[2], values[3]),
            Err(FilesystemPackageLimitsError::CannotProbe {
                resource,
                ceiling: u64::MAX,
            })
        );
    }
}

#[test]
fn gathering_returns_one_validated_complete_package_tree_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("assets")).unwrap();
    fs::write(dir.path().join("typst.toml"), b"declaration").unwrap();
    fs::write(dir.path().join("lib.typ"), b"library").unwrap();
    fs::write(dir.path().join("assets/unused.txt"), b"unused").unwrap();

    let tree =
        gather_filesystem_package(dir.path(), FilesystemPackageLimits::reference_v1()).unwrap();

    assert_eq!(
        tree.files().collect::<Vec<_>>(),
        [
            ("assets/unused.txt", &b"unused"[..]),
            ("lib.typ", &b"library"[..]),
            ("typst.toml", &b"declaration"[..]),
        ]
    );
    assert_eq!(tree.file_count(), 3);
    assert_eq!(tree.byte_length(), 24);
}

fn assert_limit(
    root: &std::path::Path,
    values: [u64; 4],
    resource: FilesystemPackageResource,
    ceiling: u64,
    observed: u64,
) {
    let error = gather_filesystem_package(root, limits(values)).unwrap_err();
    assert!(
        matches!(
            error,
            FilesystemPackageGatherError::Limit {
                source: FilesystemPackageLimitError::Exceeded {
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
fn generated_boundaries_cover_every_package_source_resource() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("lib.typ"), b"1234").unwrap();
    fs::write(root.join("second.typ"), b"5").unwrap();
    let cases = [
        (FilesystemPackageResource::VisitedEntries, 2),
        (FilesystemPackageResource::SelectedFiles, 2),
        (FilesystemPackageResource::SelectedFileBytes, 4),
        (FilesystemPackageResource::PackageTreeBytes, 5),
    ];
    let exact = [2, 2, 4, 5];

    for (index, (resource, observed)) in cases.into_iter().enumerate() {
        for ceiling in [observed + 1, observed] {
            let mut values = exact;
            values[index] = ceiling;
            gather_filesystem_package(root, limits(values)).unwrap();
        }
        let mut values = exact;
        values[index] -= 1;
        assert_limit(root, values, resource, values[index], observed);
    }
}

#[cfg(unix)]
#[test]
fn filesystem_aliases_do_not_masquerade_as_package_files() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.typ"), b"library").unwrap();
    symlink(dir.path().join("lib.typ"), dir.path().join("alias.typ")).unwrap();

    let error =
        gather_filesystem_package(dir.path(), FilesystemPackageLimits::reference_v1()).unwrap_err();

    assert!(matches!(error, FilesystemPackageGatherError::Survey(_)));
}

#[cfg(unix)]
#[test]
fn survey_issues_take_precedence_over_deferred_selected_file_limits() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.typ"), b"library").unwrap();
    symlink(dir.path().join("lib.typ"), dir.path().join("alias.typ")).unwrap();

    let error = gather_filesystem_package(dir.path(), limits([10, 0, 100, 100])).unwrap_err();

    assert!(matches!(error, FilesystemPackageGatherError::Survey(_)));
}

#[test]
fn concrete_authority_makes_local_cache_and_offline_precedence_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let cache = dir.path().join("cache");
    let data_root = write_package(&data, b"local");
    write_package(&cache, b"cache");
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let acquired = FilesystemPackageAuthority::new(Some(&data), Some(&cache), true)
        .acquire(&spec)
        .unwrap();

    assert_eq!(acquired.tree().file("marker.txt"), Some(&b"local"[..]));
    assert_eq!(acquired.root(), Some(data_root.as_path()));

    fs::remove_dir_all(&data_root).unwrap();
    let acquired = FilesystemPackageAuthority::new(Some(&data), Some(&cache), true)
        .acquire(&spec)
        .unwrap();
    assert_eq!(acquired.tree().file("marker.txt"), Some(&b"cache"[..]));
}

#[test]
fn concrete_authority_failure_retains_the_exact_specification() {
    let dir = tempfile::tempdir().unwrap();
    let spec: PackageSpec = "@local/missing:1.0.0".parse().unwrap();

    let error = FilesystemPackageAuthority::new(Some(dir.path()), Some(dir.path()), true)
        .acquire(&spec)
        .unwrap_err();
    let failure = error.failure();

    assert_eq!(failure.spec(), &spec);
    assert_eq!(failure.reason(), &PackageAcquisitionFailureReason::NotFound);
}

#[cfg(unix)]
#[test]
fn concrete_authority_preserves_the_typed_filesystem_cause() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let root = write_package(dir.path(), b"local");
    symlink(root.join("marker.txt"), root.join("alias.txt")).unwrap();
    let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

    let error = FilesystemPackageAuthority::new(Some(dir.path()), None, true)
        .acquire(&spec)
        .unwrap_err();

    let FilesystemPackageAcquisitionError::Filesystem { failure, source } = error else {
        panic!("expected a typed filesystem acquisition cause");
    };
    assert_eq!(failure.spec(), &spec);
    assert!(matches!(
        source.as_ref(),
        FilesystemPackageGatherError::Survey(_)
    ));
}
