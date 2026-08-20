#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{FilesystemProjectLimits, read_filesystem_project};

fuzz_target!(|data: &[u8]| {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let selector = data.first().copied().unwrap_or_default();
    std::fs::write(root.join("main.typ"), b"main").unwrap();
    std::fs::create_dir_all(root.join("ignored/nested")).unwrap();
    std::fs::write(root.join("ignored/nested/file.typ"), b"ignored").unwrap();
    std::fs::create_dir_all(root.join("nested")).unwrap();
    let stem: String = data
        .iter()
        .skip(1)
        .take(16)
        .map(|byte| char::from(b'a' + byte % 26))
        .collect();
    let stem = if stem.is_empty() { "empty" } else { &stem };
    std::fs::write(root.join(format!("nested/{stem}.typ")), data).unwrap();

    let policy = if selector & 1 == 0 {
        data
    } else {
        &b"ignored/\n!nested/keep.typ\n"[..]
    };
    std::fs::write(root.join(".typkignore"), policy).unwrap();

    #[cfg(unix)]
    let _socket = {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        if selector & 2 != 0 {
            let _ = symlink(root.join("main.typ"), root.join("alias.typ"));
        }
        let socket = (selector & 4 != 0)
            .then(|| UnixListener::bind(root.join("project.sock")).ok())
            .flatten();
        if selector & 8 != 0 {
            let name = data
                .iter()
                .skip(1)
                .take(16)
                .copied()
                .filter(|byte| *byte != 0 && *byte != b'/')
                .collect::<Vec<_>>();
            if !name.is_empty() {
                let _ = std::fs::write(root.join(OsString::from_vec(name)), b"path");
            }
        }
        socket
    };

    let limits = if selector & 16 == 0 {
        FilesystemProjectLimits::reference_v1()
    } else {
        let value = |index: usize| data.get(index).copied().unwrap_or_default() as u64;
        FilesystemProjectLimits::new(value(1), value(2), value(3), value(4), value(5)).unwrap()
    };

    let _ = read_filesystem_project(root, "main.typ", limits);
});
