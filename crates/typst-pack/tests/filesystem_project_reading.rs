//! Project reading from the reference filesystem source.

#![cfg(feature = "fs")]

use std::fs;
use std::path::Path;

use typst_pack::{FilesystemReadError, LimitExceeded, ReadLimits, read_filesystem_project};

fn files(snapshot: &typst_pack::ProjectSnapshot) -> Vec<&str> {
    snapshot.files().map(|(path, _)| path).collect()
}

#[test]
fn reading_applies_the_root_policy_once_and_preserves_its_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let policy = b"drop/**\n!drop/keep/\n!drop/keep/kept.txt\n*.secret\n";
    fs::create_dir_all(root.join("drop/keep")).unwrap();
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("main.typ"), b"main").unwrap();
    fs::write(root.join(".typkignore"), policy).unwrap();
    fs::write(root.join("drop/gone.txt"), b"gone").unwrap();
    fs::write(root.join("drop/keep/kept.txt"), b"kept").unwrap();
    fs::write(root.join("nested/.typkignore"), b"*.txt\n").unwrap();
    fs::write(root.join("nested/ordinary.txt"), b"ordinary").unwrap();
    fs::write(root.join("private.secret"), b"private").unwrap();

    let snapshot = read_filesystem_project(root, "main.typ", ReadLimits::default()).unwrap();

    assert_eq!(snapshot.file(".typkignore"), Some(policy.as_slice()));
    assert_eq!(snapshot.file("drop/keep/kept.txt"), Some(&b"kept"[..]));
    assert_eq!(snapshot.file("nested/.typkignore"), Some(&b"*.txt\n"[..]));
    assert_eq!(snapshot.file("nested/ordinary.txt"), Some(&b"ordinary"[..]));
    assert_eq!(snapshot.file("drop/gone.txt"), None);
    assert_eq!(snapshot.file("private.secret"), None);
}

#[test]
fn malformed_root_policy_bytes_are_a_typed_reading_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("main.typ"), b"main").unwrap();
    fs::write(dir.path().join(".typkignore"), [0xff]).unwrap();

    let error = read_filesystem_project(dir.path(), "main.typ", ReadLimits::default()).unwrap_err();

    assert!(matches!(
        error,
        FilesystemReadError::InvalidIgnoreFile { ref path, .. } if path.ends_with(".typkignore")
    ));
}

#[test]
fn each_limit_reports_the_resource_it_bounds() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("chapters")).unwrap();
    fs::write(dir.path().join("main.typ"), b"0123456789").unwrap();
    fs::write(dir.path().join("chapters/one.typ"), b"0123456789").unwrap();
    let read = |limits| read_filesystem_project(dir.path(), "main.typ", limits);

    let limits = ReadLimits {
        max_entries: 3,
        max_file_bytes: 10,
        max_total_bytes: 20,
    };
    assert_eq!(
        files(&read(limits).unwrap()),
        ["chapters/one.typ", "main.typ"]
    );

    for (limits, resource, limit) in [
        (
            ReadLimits {
                max_entries: 2,
                ..limits
            },
            "entries",
            2,
        ),
        (
            ReadLimits {
                max_file_bytes: 9,
                ..limits
            },
            "file bytes",
            9,
        ),
        (
            ReadLimits {
                max_total_bytes: 19,
                ..limits
            },
            "total bytes",
            19,
        ),
    ] {
        assert!(matches!(
            read(limits),
            Err(FilesystemReadError::Limit { source, .. })
                if source == LimitExceeded { resource, limit }
        ));
    }
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn links_resolving_beneath_the_root_are_followed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        fs::create_dir_all(root.join("shared")).unwrap();
        fs::write(root.join("main.typ"), b"main").unwrap();
        fs::write(root.join("shared/data.csv"), b"a,b").unwrap();
        symlink("shared/data.csv", root.join("data.csv")).unwrap();
        symlink(root.join("shared"), root.join("linked")).unwrap();
        fs::write(root.join("shared/ignore"), b"*.csv\n!data.csv\n").unwrap();
        symlink("shared/ignore", root.join(".typkignore")).unwrap();

        let snapshot = read_filesystem_project(&root, "main.typ", ReadLimits::default()).unwrap();

        assert_eq!(
            files(&snapshot),
            [
                ".typkignore",
                "data.csv",
                "linked/data.csv",
                "linked/ignore",
                "main.typ",
                "shared/data.csv",
                "shared/ignore",
            ]
        );
        assert_eq!(snapshot.file("linked/data.csv"), Some(&b"a,b"[..]));
    }

    #[test]
    fn links_escaping_the_root_are_refused_unless_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        fs::create_dir_all(root.join("ignored")).unwrap();
        fs::write(dir.path().join("secret.txt"), b"secret").unwrap();
        fs::write(root.join("main.typ"), b"main").unwrap();
        fs::write(root.join(".typkignore"), b"ignored/\nskipped/\n").unwrap();
        symlink(dir.path(), root.join("ignored/outside")).unwrap();
        symlink(dir.path(), root.join("skipped")).unwrap();

        let snapshot = read_filesystem_project(&root, "main.typ", ReadLimits::default()).unwrap();
        assert_eq!(files(&snapshot), [".typkignore", "main.typ"]);

        symlink(dir.path().join("secret.txt"), root.join("leak.txt")).unwrap();
        let error = read_filesystem_project(&root, "main.typ", ReadLimits::default()).unwrap_err();
        assert!(matches!(
            error,
            FilesystemReadError::EscapingSymlink { ref path, .. }
                if path == &root.join("leak.txt")
        ));
        assert!(error.to_string().contains("leak.txt"));
    }

    #[test]
    fn an_escaping_root_ignore_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.typ"), b"main").unwrap();
        fs::write(dir.path().join("policy"), b"*.txt\n").unwrap();
        symlink(dir.path().join("policy"), root.join(".typkignore")).unwrap();

        let error = read_filesystem_project(&root, "main.typ", ReadLimits::default()).unwrap_err();

        assert!(matches!(error, FilesystemReadError::EscapingSymlink { .. }));
    }

    #[test]
    fn directory_cycles_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("main.typ"), b"main").unwrap();
        symlink("../b", root.join("a/to-b")).unwrap();
        symlink("../a", root.join("b/to-a")).unwrap();

        let error = read_filesystem_project(&root, "main.typ", ReadLimits::default()).unwrap_err();

        assert!(matches!(error, FilesystemReadError::SymlinkCycle { .. }));
    }

    #[test]
    fn broken_links_fail_unless_excluded() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.typ"), b"main").unwrap();
        symlink("missing", dir.path().join("broken.log")).unwrap();

        let error =
            read_filesystem_project(dir.path(), "main.typ", ReadLimits::default()).unwrap_err();
        assert!(
            matches!(error, FilesystemReadError::Io { ref path, .. } if path.ends_with("broken.log"))
        );

        fs::write(dir.path().join(".typkignore"), b"*.log\n").unwrap();
        assert!(read_filesystem_project(dir.path(), "main.typ", ReadLimits::default()).is_ok());
    }

    #[test]
    fn sockets_are_unsupported_entries() {
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.typ"), b"main").unwrap();
        let _socket = UnixListener::bind(dir.path().join("project.sock")).unwrap();

        let error =
            read_filesystem_project(dir.path(), "main.typ", ReadLimits::default()).unwrap_err();

        assert!(matches!(
            error,
            FilesystemReadError::Unsupported { ref path } if path.ends_with("project.sock")
        ));
    }

    #[test]
    fn non_unicode_paths_are_refused() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir().unwrap();
        // APFS and HFS+ refuse names that are not UTF-8.
        if fs::write(dir.path().join(OsString::from_vec(vec![0xff])), b"bytes").is_err() {
            eprintln!("skipped: the temporary filesystem rejects non-UTF-8 names");
            return;
        }
        fs::write(dir.path().join("main.typ"), b"main").unwrap();

        let error =
            read_filesystem_project(dir.path(), "main.typ", ReadLimits::default()).unwrap_err();

        assert!(matches!(error, FilesystemReadError::NonUtf8Path { .. }));
    }
}

#[test]
fn a_missing_root_names_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");

    let error = read_filesystem_project(&missing, "main.typ", ReadLimits::default()).unwrap_err();

    assert!(
        matches!(error, FilesystemReadError::Io { ref path, .. } if path == Path::new(&missing))
    );
}
