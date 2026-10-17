//! The filesystem adapter: reading projects and packages, packing, resolving
//! external requirements, and writing extractions, artifacts, and files.

#![cfg(feature = "fs")]

#[cfg(feature = "embedded-fonts")]
#[path = "support/fonts.rs"]
mod font_bytes;

use std::path::{Path, PathBuf};

use typst::syntax::package::PackageSpec;
use typst_pack::compile::{Artifact, DocumentTime, Input, Output, OutputFormat, Unfulfilled};
use typst_pack::{LimitExceeded, Pack, ProjectSnapshot, ReadLimits};
use typst_pack::{compile, create, extract, fs};

/// Writes `files` beneath `root`, creating every parent directory.
fn write_tree(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

fn manifest(name: &str, version: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nentrypoint = \"lib.typ\"\n")
}

/// Writes a `@local/{name}:0.1.0` package beneath a package path.
fn write_package(packages: &Path, name: &str, body: &str) -> PathBuf {
    let root = packages.join(format!("local/{name}/0.1.0"));
    write_tree(
        &root,
        &[("typst.toml", &manifest(name, "0.1.0")), ("lib.typ", body)],
    );
    root
}

fn read(root: &Path) -> Result<ProjectSnapshot, fs::Error> {
    fs::read_project(root, "main.typ", ReadLimits::default())
}

fn paths(snapshot: &ProjectSnapshot) -> Vec<&str> {
    snapshot.files().map(|(path, _)| path).collect()
}

fn pack_paths(pack: &Pack) -> Vec<&str> {
    pack.files().map(|(path, _)| path).collect()
}

/// A packer that offers no host fonts.
fn packer() -> fs::Packer {
    fs::Packer {
        system_fonts: false,
        ..fs::Packer::default()
    }
}

// --- Project reading -------------------------------------------------------

#[test]
fn the_root_ignore_policy_selects_files_and_travels_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let policy = "drop/**\n!drop/keep/\n!drop/keep/kept.txt\n*.secret\n";
    write_tree(
        dir.path(),
        &[
            ("main.typ", "main"),
            (".typkignore", policy),
            ("drop/gone.txt", "gone"),
            ("drop/keep/kept.txt", "kept"),
            // Nested ignore files are ordinary files.
            ("nested/.typkignore", "*.txt\n"),
            ("nested/ordinary.txt", "ordinary"),
            ("private.secret", "private"),
            // Packs are never project files.
            ("old.typk", "pack"),
        ],
    );

    let snapshot = read(dir.path()).unwrap();

    assert_eq!(
        paths(&snapshot),
        [
            ".typkignore",
            "drop/keep/kept.txt",
            "main.typ",
            "nested/.typkignore",
            "nested/ordinary.txt"
        ]
    );
    assert_eq!(snapshot.file(".typkignore"), Some(policy.as_bytes()));
}

#[test]
fn files_beneath_an_ignored_directory_are_not_reincluded() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            ("main.typ", "Hello"),
            (".typkignore", "ignored/\n!ignored/keep.txt\n"),
            ("ignored/keep.txt", "still ignored"),
        ],
    );

    assert_eq!(
        paths(&read(dir.path()).unwrap()),
        [".typkignore", "main.typ"]
    );
}

#[test]
fn the_ignore_policy_supports_gitignore_pattern_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let policy = "# Project policy\n/root-only.secret\n\\!important.txt\n\\#notes.txt\n\
                  cache/\n*.tmp\n!keep.tmp\n leading.txt\ntrailing.txt   \n";
    write_tree(
        dir.path(),
        &[
            ("main.typ", "Hello"),
            (".typkignore", policy),
            ("# Project policy", "packed"),
            ("root-only.secret", "ignored"),
            ("nested/root-only.secret", "packed"),
            ("!important.txt", "ignored"),
            ("#notes.txt", "ignored"),
            ("nested/cache/data.txt", "ignored"),
            ("drop.tmp", "ignored"),
            ("nested/keep.tmp", "packed"),
            (" leading.txt", "ignored"),
            ("trailing.txt", "ignored"),
        ],
    );

    assert_eq!(
        paths(&read(dir.path()).unwrap()),
        [
            "# Project policy",
            ".typkignore",
            "main.typ",
            "nested/keep.tmp",
            "nested/root-only.secret"
        ]
    );
}

// Windows drops a trailing space from a file name.
#[cfg(unix)]
#[test]
fn an_escaped_trailing_space_is_matched() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            ("main.typ", "Hello"),
            (".typkignore", "literal\\ \n"),
            ("literal ", "ignored"),
            ("literal", "packed"),
        ],
    );

    assert_eq!(
        paths(&read(dir.path()).unwrap()),
        [".typkignore", "literal", "main.typ"]
    );
}

#[test]
fn a_malformed_ignore_file_and_an_ignored_entrypoint_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.typ"), b"main").unwrap();
    std::fs::write(dir.path().join(".typkignore"), [0xff]).unwrap();
    assert!(matches!(
        read(dir.path()),
        Err(fs::Error::InvalidIgnoreFile { ref path, .. }) if path.ends_with(".typkignore")
    ));

    std::fs::write(dir.path().join(".typkignore"), "main.typ\n").unwrap();
    assert!(matches!(
        packer().pack(dir.path(), "main.typ"),
        Err(fs::Error::EntrypointIgnored(path)) if path == "main.typ"
    ));
}

#[test]
fn an_entrypoint_outside_the_root_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[("project/main.typ", "inside"), ("outside.typ", "outside")],
    );
    let root = dir.path().join("project");

    // Relative to the root, or absolute.
    for entrypoint in [
        PathBuf::from("../outside.typ"),
        dir.path().join("outside.typ"),
    ] {
        let error = packer().pack(&root, &entrypoint).unwrap_err();
        assert!(
            matches!(&error, fs::Error::OutsideRoot(path) if path.ends_with("outside.typ")),
            "{error:?}"
        );
    }
}

#[test]
fn only_an_excluded_file_is_reported_as_an_ignored_entrypoint() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[("main.typ", "main"), ("chapters/one.typ", "one")],
    );

    let error = packer().pack(dir.path(), "chapters").unwrap_err();
    assert!(
        matches!(
            &error,
            fs::Error::Invalid(invalid) if matches!(
                invalid.issues.as_slice(),
                [typst_pack::PackIssue::MissingEntrypoint { path }] if path == "chapters"
            )
        ),
        "{error}"
    );

    let error = packer().pack(dir.path(), "missing.typ").unwrap_err();
    assert!(
        matches!(
            &error,
            fs::Error::Io { path, source }
                if path.ends_with("missing.typ")
                    && source.kind() == std::io::ErrorKind::NotFound
        ),
        "{error}"
    );
}

#[test]
fn project_and_package_reading_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            ("main.typ", "0123456789"),
            ("chapters/one.typ", "0123456789"),
        ],
    );
    let fits = ReadLimits {
        max_entries: 3,
        max_file_bytes: 10,
        max_total_bytes: 20,
    };
    let exceeded = |change: fn(&mut ReadLimits)| {
        let mut limits = fits;
        change(&mut limits);
        match fs::read_project(dir.path(), "main.typ", limits) {
            Err(fs::Error::Limit { source, .. }) => source,
            other => panic!("expected a limit, got {other:?}"),
        }
    };

    let snapshot = fs::read_project(dir.path(), "main.typ", fits).unwrap();
    assert_eq!(paths(&snapshot), ["chapters/one.typ", "main.typ"]);
    let limit = |resource, limit| LimitExceeded { resource, limit };
    assert_eq!(exceeded(|l| l.max_entries = 2), limit("entries", 2));
    assert_eq!(exceeded(|l| l.max_file_bytes = 9), limit("file bytes", 9));
    assert_eq!(
        exceeded(|l| l.max_total_bytes = 19),
        limit("total bytes", 19)
    );

    let package = fs::read_package(
        dir.path(),
        ReadLimits {
            max_file_bytes: 9,
            ..fits
        },
    );
    assert!(matches!(
        package,
        Err(fs::Error::Limit { source, .. }) if source == limit("file bytes", 9)
    ));
}

#[test]
fn a_missing_root_names_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");

    let error = read(&missing).unwrap_err();

    assert!(matches!(error, fs::Error::Io { ref path, .. } if path == &missing));
    // The I/O error is the source, not part of the message.
    assert_eq!(
        error.to_string(),
        format!("cannot read `{}`", missing.display())
    );
    let source = std::error::Error::source(&error).unwrap();
    assert_eq!(
        source.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
}

#[test]
fn the_default_packer_reads_today_in_the_local_timezone() {
    let packer = fs::Packer::default();
    assert!(matches!(
        packer.options.document_time,
        typst_pack::compile::DocumentTime::Local { .. }
    ));
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn links_resolving_beneath_the_root_are_followed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        write_tree(
            &root,
            &[
                ("main.typ", "main"),
                ("shared/data.csv", "a,b"),
                ("shared/ignore", "*.csv\n!data.csv\n"),
            ],
        );
        symlink("shared/data.csv", root.join("data.csv")).unwrap();
        symlink(root.join("shared"), root.join("linked")).unwrap();
        symlink("shared/ignore", root.join(".typkignore")).unwrap();

        let snapshot = read(&root).unwrap();

        assert_eq!(
            paths(&snapshot),
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
    fn links_escaping_the_root_are_refused_unless_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        write_tree(
            dir.path(),
            &[("secret.txt", "secret"), ("policy", "*.txt\n")],
        );
        write_tree(
            &root,
            &[
                ("main.typ", "main"),
                (".typkignore", "ignored/\nskipped/\n"),
            ],
        );
        // Ignored subtrees are pruned before their links are resolved.
        std::fs::create_dir(root.join("ignored")).unwrap();
        symlink(dir.path(), root.join("ignored/outside")).unwrap();
        symlink(dir.path(), root.join("skipped")).unwrap();
        assert_eq!(paths(&read(&root).unwrap()), [".typkignore", "main.typ"]);

        symlink(dir.path().join("secret.txt"), root.join("leak.txt")).unwrap();
        assert!(matches!(
            read(&root),
            Err(fs::Error::EscapingSymlink { ref path, .. }) if path == &root.join("leak.txt")
        ));

        // The root ignore file itself may not come from outside.
        let root = dir.path().join("linked-policy");
        write_tree(&root, &[("main.typ", "main")]);
        symlink(dir.path().join("policy"), root.join(".typkignore")).unwrap();
        let policy = root.canonicalize().unwrap().join(".typkignore");
        assert!(matches!(
            packer().pack(&root, "main.typ"),
            Err(fs::Error::EscapingSymlink { ref path, .. }) if path == &policy
        ));
    }

    #[test]
    fn cycles_broken_links_sockets_and_non_unicode_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cycle");
        write_tree(&root, &[("main.typ", "main")]);
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        symlink("../b", root.join("a/to-b")).unwrap();
        symlink("../a", root.join("b/to-a")).unwrap();
        assert!(matches!(read(&root), Err(fs::Error::SymlinkCycle { .. })));

        let root = dir.path().join("broken");
        write_tree(&root, &[("main.typ", "main")]);
        symlink("missing", root.join("broken.log")).unwrap();
        let error = read(&root).unwrap_err();
        assert!(matches!(
            error,
            fs::Error::BrokenSymlink { ref path, .. } if path.ends_with("broken.log")
        ));
        assert!(error.to_string().starts_with("broken symbolic link"));
        std::fs::write(root.join(".typkignore"), b"*.log\n").unwrap();
        assert!(read(&root).is_ok());

        let root = dir.path().join("socket");
        write_tree(&root, &[("main.typ", "main")]);
        let _socket = std::os::unix::net::UnixListener::bind(root.join("project.sock")).unwrap();
        assert!(matches!(
            read(&root),
            Err(fs::Error::Unsupported { ref path }) if path.ends_with("project.sock")
        ));

        use std::os::unix::ffi::OsStringExt;
        let root = dir.path().join("bytes");
        write_tree(&root, &[("main.typ", "main")]);
        // APFS and HFS+ refuse names that are not UTF-8.
        let name = std::ffi::OsString::from_vec(vec![0xff]);
        if std::fs::write(root.join(name), b"bytes").is_ok() {
            assert!(matches!(read(&root), Err(fs::Error::NonUtf8Path { .. })));
        }
    }

    #[test]
    fn a_symlinked_package_root_and_internal_links_are_read_but_escapes_refused() {
        let dir = tempfile::tempdir().unwrap();
        let development = dir.path().join("dev/example");
        write_tree(&development, &[("src/lib.typ", "library")]);
        symlink("src/lib.typ", development.join("lib.typ")).unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(data.join("local/example")).unwrap();
        symlink(&development, data.join("local/example/1.0.0")).unwrap();
        let spec: PackageSpec = "@local/example:1.0.0".parse().unwrap();

        let resolved = resolver(&data, &data).resolve(&requiring(&spec)).unwrap();
        let tree = &resolved.packages[0].1;
        assert_eq!(tree.file("lib.typ"), Some(&b"library"[..]));
        assert_eq!(tree.file("src/lib.typ"), Some(&b"library"[..]));

        std::fs::write(dir.path().join("outside.txt"), b"outside").unwrap();
        symlink(
            dir.path().join("outside.txt"),
            development.join("alias.txt"),
        )
        .unwrap();
        assert!(matches!(
            fs::read_package(&development, ReadLimits::default()),
            Err(fs::Error::EscapingSymlink { ref path, .. }) if path.ends_with("alias.txt")
        ));
        let error = resolver(&data, &data)
            .resolve(&requiring(&spec))
            .unwrap_err();
        assert!(matches!(error, fs::Error::Package { spec: ref failed, .. } if failed == &spec));
    }
}

// --- Package reading and resolution ----------------------------------------

#[test]
fn a_package_is_read_as_one_complete_tree_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            ("typst.toml", "declaration"),
            ("lib.typ", "library"),
            ("assets/unused.txt", "unused"),
            // Version-control entries are never package files, whether
            // directories or files (a submodule's `.git` is a file).
            (".git/HEAD", "ref: refs/heads/main"),
            (".hg/store", "store"),
            ("assets/.svn/entries", "entries"),
            ("vendor/.git", "gitdir: ../.git/modules/vendor"),
            ("vendor/lib.typ", "vendored"),
        ],
    );

    let tree = fs::read_package(dir.path(), ReadLimits::default()).unwrap();

    let files = tree.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(
        files,
        [
            "assets/unused.txt",
            "lib.typ",
            "typst.toml",
            "vendor/lib.typ"
        ]
    );
}

/// A pack that requires `spec` externally.
fn requiring(spec: &PackageSpec) -> Pack {
    Pack::builder("main.typ")
        .file("main.typ", Vec::new())
        .external_package_file(spec.clone(), "lib.typ", Vec::new())
        .build()
        .unwrap()
}

fn resolver(data: &Path, cache: &Path) -> fs::Packer {
    fs::Packer {
        package_path: Some(data.into()),
        package_cache_path: Some(cache.into()),
        ..packer()
    }
}

#[test]
fn the_package_directory_precedes_the_cache_by_namespace_name_and_version() {
    let dir = tempfile::tempdir().unwrap();
    let (data, cache) = (dir.path().join("data"), dir.path().join("cache"));
    for key in [
        "local/example/1.0.0",
        "local/example/2.3.4",
        "preview/nested-name/1.0.0",
    ] {
        write_tree(&data.join(key), &[("marker.txt", key)]);
        write_tree(&cache.join(key), &[("marker.txt", "cache")]);
    }
    let marker = |text: &str| {
        let spec: PackageSpec = text.parse().unwrap();
        let resolved = resolver(&data, &cache).resolve(&requiring(&spec)).unwrap();
        let tree = &resolved.packages.first()?.1;
        let directory = resolved.package_dirs[&spec].clone();
        Some((tree.file("marker.txt").unwrap().to_vec(), directory))
    };

    for (text, key) in [
        ("@local/example:1.0.0", "local/example/1.0.0"),
        ("@local/example:2.3.4", "local/example/2.3.4"),
        ("@preview/nested-name:1.0.0", "preview/nested-name/1.0.0"),
    ] {
        assert_eq!(
            marker(text),
            Some((key.as_bytes().to_vec(), data.join(key)))
        );
    }
    std::fs::remove_dir_all(data.join("local/example/1.0.0")).unwrap();
    assert_eq!(
        marker("@local/example:1.0.0"),
        Some((b"cache".to_vec(), cache.join("local/example/1.0.0")))
    );
    // A package found nowhere is left for compilation to report.
    assert_eq!(marker("@local/missing:1.0.0"), None);
}

#[test]
fn external_packages_resolve_from_the_package_path_for_compilation() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    write_tree(
        &project,
        &[("main.typ", "#import \"@local/kept:0.1.0\": mark\n#mark")],
    );
    let packages = dir.path().join("packages");
    write_package(&packages, "kept", "#let mark = rect(width: 1pt)");
    let packer = fs::Packer {
        package_path: Some(packages.clone()),
        embed_packages: false,
        ..packer()
    };
    let pack = packer.pack(&project, "main.typ").unwrap().pack;
    assert!(!pack.package_requirements()[0].embedded);

    let resolved = packer.resolve(&pack).unwrap();
    let mut request = compile::Request::new(pack.clone(), Output::new(OutputFormat::Svg));
    request.packages = resolved.packages;
    request.fonts = resolved.fonts;
    compile(&request).unwrap();

    std::fs::remove_dir_all(&packages).unwrap();
    request.packages = packer.resolve(&pack).unwrap().packages;
    assert!(matches!(
        compile(&request),
        Err(compile::Error::Unfulfilled(issues))
            if matches!(issues.as_slice(), [Unfulfilled::MissingPackage { .. }])
    ));
}

// --- Packing ---------------------------------------------------------------

#[test]
fn packing_contains_every_project_file_and_complete_packages() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    let png = tiny_skia::Pixmap::new(4, 4).unwrap().encode_png().unwrap();
    write_tree(
        &project,
        &[
            (
                "main.typ",
                "#import \"@local/greet:0.1.0\": greet\n\
                 #include \"chapters/intro.typ\"\n\
                 #image(\"logo.png\", width: 8pt)\n\
                 #greet(\"World\")\nRows: #csv(\"data.csv\").len()",
            ),
            ("chapters/intro.typ", "= Introduction\n"),
            ("data.csv", "a,b\n1,2\n"),
            // A file the compile never reads.
            ("notes.txt", "extra"),
        ],
    );
    std::fs::write(project.join("logo.png"), png).unwrap();
    let packages = dir.path().join("packages");
    let greet = write_package(&packages, "greet", "#let greet(name) = [Hello #name!]");
    std::fs::write(greet.join("unused.txt"), "complete package").unwrap();

    let packed = fs::Packer {
        package_path: Some(packages),
        ..packer()
    }
    .pack(&project, "main.typ")
    .unwrap();

    assert_eq!(
        pack_paths(&packed.pack),
        [
            "chapters/intro.typ",
            "data.csv",
            "logo.png",
            "main.typ",
            "notes.txt"
        ]
    );
    let requirement = &packed.pack.package_requirements()[0];
    assert!(requirement.embedded);
    assert_eq!(requirement.spec.to_string(), "@local/greet:0.1.0");
    // The whole package tree travels, not only what was read.
    let (_, tree) = packed.pack.packages().next().unwrap();
    assert_eq!(tree.digest(), requirement.digest);
    let files = tree.files().map(|(path, _)| path).collect::<Vec<_>>();
    assert_eq!(files, ["lib.typ", "typst.toml", "unused.txt"]);
}

#[test]
fn one_packer_packs_several_projects_on_the_host_clock_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let packer = packer();
    for name in ["first", "second"] {
        let project = dir.path().join(name);
        write_tree(
            &project,
            &[(
                "main.typ",
                &format!("#assert(datetime.today().year() >= 2024)\n{name}"),
            )],
        );
        let packed = packer.pack(&project, "main.typ").unwrap();
        assert!(
            packed
                .pack
                .file("main.typ")
                .unwrap()
                .ends_with(name.as_bytes())
        );
    }
}

#[test]
fn the_target_inputs_and_document_time_drive_discovery_but_not_selection() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            (
                "main.typ",
                "#context if target() == \"html\" { read(\"html.txt\") }\n\
                 #if sys.inputs.at(\"pick\") == \"yes\" { read(\"input.txt\") }\n\
                 #if datetime.today().year() == 2024 { read(\"time.txt\") }",
            ),
            ("html.txt", "html"),
            ("paged.txt", "paged"),
            ("input.txt", "input"),
            ("time.txt", "time"),
        ],
    );

    let packed = fs::Packer {
        target: create::Target::Html,
        options: compile::Options {
            inputs: vec![("pick".into(), Input::from("yes"))],
            document_time: DocumentTime::UnixTimestamp(1_704_067_200),
            ..compile::Options::default()
        },
        ..packer()
    }
    .pack(dir.path(), "main.typ")
    .unwrap();

    assert_eq!(
        pack_paths(&packed.pack),
        ["html.txt", "input.txt", "main.typ", "paged.txt", "time.txt"]
    );
}

#[test]
fn the_package_cache_resolves_preview_packages_online_and_offline() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[
            (
                "project/main.typ",
                "#import \"@preview/cached:0.1.0\": mark\n#mark",
            ),
            (
                "cache/preview/cached/0.1.0/typst.toml",
                &manifest("cached", "0.1.0"),
            ),
            (
                "cache/preview/cached/0.1.0/lib.typ",
                "#let mark = rect(width: 1pt, height: 1pt)",
            ),
        ],
    );
    std::fs::create_dir(dir.path().join("data")).unwrap();
    let offline = resolver(&dir.path().join("data"), &dir.path().join("cache"));
    #[cfg(feature = "egress")]
    let online = fs::Packer {
        download_packages: true,
        ..offline.clone()
    };
    #[cfg(not(feature = "egress"))]
    let online = offline.clone();

    for packer in [offline, online] {
        let packed = packer.pack(dir.path().join("project"), "main.typ").unwrap();
        let (spec, tree) = packed.pack.packages().next().unwrap();
        assert_eq!(spec.to_string(), "@preview/cached:0.1.0");
        assert!(tree.file("lib.typ").is_some());
    }
}

/// Offline (or built without egress), a Universe package no local source holds
/// fails the import that needed it.
#[test]
fn an_unavailable_universe_package_fails_at_its_import() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(
        dir.path(),
        &[(
            "main.typ",
            "#import \"@preview/typst-pack-no-such-package:0.0.1\": x\n",
        )],
    );
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();

    let result = resolver(&empty, &empty).pack(dir.path(), "main.typ");

    let Err(fs::Error::Create(error)) = result else {
        panic!("an unresolvable package did not fail discovery");
    };
    let create::Error::Failed { errors, .. } = *error else {
        panic!("an unresolvable package did not fail discovery");
    };
    assert!(errors.iter().all(|error| !error.span.is_detached()));
    assert_eq!(
        errors[0].message.as_str(),
        "package not found (searched for @preview/typst-pack-no-such-package:0.0.1)"
    );
}

#[cfg(feature = "embedded-fonts")]
mod fonts {
    use typst_pack::Digest;

    use super::*;
    use crate::font_bytes::{family_of, typst_container};

    fn embeds_any(pack: &Pack) -> bool {
        let mut requirements = pack.font_requirements().iter();
        requirements.any(|requirement| requirement.embedded)
    }

    #[test]
    fn typst_embedded_fonts_are_embedded_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), &[("main.typ", "Hello")]);
        let pack = |include_typst_embedded_fonts| {
            let packer = fs::Packer {
                embed_fonts: true,
                include_typst_embedded_fonts,
                ..packer()
            };
            packer.pack(dir.path(), "main.typ").unwrap().pack
        };

        assert!(!embeds_any(&pack(false)));
        assert!(embeds_any(&pack(true)));
    }

    #[test]
    fn html_creation_embeds_fonts_used_inside_frames() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), &[("main.typ", "#html.frame[Hello]")]);

        let packed = fs::Packer {
            target: create::Target::Html,
            embed_fonts: true,
            include_typst_embedded_fonts: true,
            ..packer()
        }
        .pack(dir.path(), "main.typ")
        .unwrap();

        assert!(embeds_any(&packed.pack));
    }

    #[test]
    fn a_scanned_container_is_embedded_even_when_typst_ships_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let data = typst_container();
        let family = family_of(&data);
        write_tree(
            dir.path(),
            &[(
                "project/main.typ",
                &format!("#set text(font: \"{family}\")\nSelected"),
            )],
        );
        std::fs::create_dir(dir.path().join("fonts")).unwrap();
        std::fs::write(dir.path().join("fonts/font.ttf"), &data).unwrap();

        let packed = fs::Packer {
            font_paths: vec![dir.path().join("fonts")],
            typst_embedded_fonts: false,
            embed_fonts: true,
            ..packer()
        }
        .pack(dir.path().join("project"), "main.typ")
        .unwrap();

        let requirements = packed.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].digest, Digest::of(&data));
        assert!(requirements[0].embedded);
    }

    #[test]
    fn a_container_offered_twice_is_used_once_and_embedded_if_either_embeds_it() {
        let dir = tempfile::tempdir().unwrap();
        let data = typst_container();
        let family = family_of(&data);
        write_tree(
            dir.path(),
            &[(
                "project/main.typ",
                &format!("#set text(font: \"{family}\")\nSelected"),
            )],
        );
        std::fs::create_dir(dir.path().join("fonts")).unwrap();
        std::fs::write(dir.path().join("fonts/font.ttf"), &data).unwrap();

        // Typst ships the bytes too, unembedded, and the path is listed twice.
        let packed = fs::Packer {
            font_paths: vec![dir.path().join("fonts"), dir.path().join("fonts")],
            embed_fonts: true,
            ..packer()
        }
        .pack(dir.path().join("project"), "main.typ")
        .unwrap();

        let requirements = packed.pack.font_requirements();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].digest, Digest::of(&data));
        assert!(requirements[0].embedded);
    }

    #[cfg(unix)]
    #[test]
    fn resolving_typst_fonts_scans_no_font_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), &[("main.typ", "Hello")]);
        let pack = packer().pack(dir.path(), "main.typ").unwrap().pack;
        let fonts = dir.path().join("fonts");
        std::fs::create_dir(&fonts).unwrap();
        // A scan would report this broken font as skipped.
        std::os::unix::fs::symlink("missing.ttf", fonts.join("broken.ttf")).unwrap();

        let resolved = fs::Packer {
            font_paths: vec![fonts],
            ..packer()
        }
        .resolve(&pack)
        .unwrap();

        assert!(!resolved.fonts.is_empty());
        assert!(resolved.skipped_fonts.is_empty());
    }

    #[test]
    fn font_paths_are_resolved_before_system_fonts_can_exhaust_the_limits() {
        let dir = tempfile::tempdir().unwrap();
        let data = typst_container();
        let pack = Pack::builder("main.typ")
            .file("main.typ", Vec::new())
            .external_font(data.clone())
            .build()
            .unwrap();
        let fonts = dir.path().join("fonts");
        std::fs::create_dir(&fonts).unwrap();
        std::fs::write(fonts.join("font.ttf"), &data).unwrap();

        // Room for the explicit path's one file but not for a system font
        // directory: whatever the host has is read after the explicit path.
        let resolved = fs::Packer {
            font_paths: vec![fonts],
            system_fonts: true,
            typst_embedded_fonts: false,
            font_limits: ReadLimits {
                max_entries: 1,
                ..ReadLimits::default()
            },
            ..packer()
        }
        .resolve(&pack)
        .unwrap();

        let digests = resolved.fonts.iter().map(|font| font.digest());
        assert_eq!(digests.collect::<Vec<_>>(), [Digest::of(&data)]);
    }

    #[test]
    fn a_created_pack_compiles_with_its_resolved_external_fonts() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), &[("main.typ", "Hello")]);
        let packer = packer();
        let pack = packer.pack(dir.path(), "main.typ").unwrap().pack;
        let mut request = compile::Request::new(pack.clone(), Output::new(OutputFormat::Pdf));
        assert!(matches!(
            compile(&request),
            Err(compile::Error::Unfulfilled(_))
        ));

        // The fonts the packer offered were Typst's own, declared externally.
        let resolved = packer.resolve(&pack).unwrap();
        assert!(resolved.packages.is_empty());
        let digests = resolved.fonts.iter().map(|font| font.digest());
        let required = pack.font_requirements().iter().map(|r| r.digest);
        assert_eq!(
            digests.collect::<std::collections::HashSet<_>>(),
            required.collect()
        );
        request.fonts = resolved.fonts;
        assert!(
            compile(&request).unwrap().artifacts[0]
                .bytes
                .starts_with(b"%PDF")
        );
    }
}

// --- Writing ---------------------------------------------------------------

fn plan_of(files: &[(&str, &[u8])]) -> extract::Plan {
    let mut builder = Pack::builder("main.typ").file("main.typ", b"new main".to_vec());
    for (path, bytes) in files {
        builder = builder.file(*path, bytes.to_vec());
    }
    extract::plan(&builder.build().unwrap(), extract::Selection::default()).unwrap()
}

fn extraction_plan() -> extract::Plan {
    plan_of(&[("assets/data.txt", b"new data")])
}

/// A canonical temporary directory; macOS temp paths start with a symlink.
fn temp_root() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    (directory, root)
}

/// Every name in a directory tree, relative to it and sorted.
fn tree(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.strip_prefix(root).unwrap().to_string_lossy();
            names.push(name.replace('\\', "/"));
            if path.is_dir() && !path.is_symlink() {
                pending.push(path);
            }
        }
    }
    names.sort();
    names
}

fn rejected(result: Result<(), fs::WriteError>) -> Vec<fs::WriteIssue> {
    match result {
        Err(fs::WriteError::Rejected { issues }) => issues,
        other => panic!("expected a rejected write, got {other:?}"),
    }
}

fn compiled(source: &str, format: OutputFormat) -> Vec<Artifact> {
    let pack = Pack::builder("main.typ")
        .file("main.typ", source.as_bytes().to_vec())
        .build()
        .unwrap();
    compile(&compile::Request::new(pack, Output::new(format)))
        .unwrap()
        .artifacts
}

#[test]
fn extraction_creates_the_destination_and_writes_exact_bytes() {
    let (_directory, root) = temp_root();
    for destination in [root.join("new/nested/written"), root.clone()] {
        fs::write_extraction(&destination, &extraction_plan(), fs::WritePolicy::CreateNew).unwrap();

        assert_eq!(
            std::fs::read(destination.join("main.typ")).unwrap(),
            b"new main"
        );
        assert_eq!(
            std::fs::read(destination.join("assets/data.txt")).unwrap(),
            b"new data"
        );
    }
}

#[test]
fn create_new_refuses_every_existing_target_and_overwrite_replaces_them() {
    let (_directory, root) = temp_root();
    write_tree(
        &root,
        &[
            ("main.typ", "old main"),
            ("assets/data.txt", "old data"),
            ("unrelated.txt", "unrelated"),
        ],
    );

    let issues = rejected(fs::write_extraction(
        &root,
        &extraction_plan(),
        fs::WritePolicy::CreateNew,
    ));
    let existing = |path: &str| fs::WriteIssue::ExistingTarget { path: path.into() };
    assert_eq!(issues, [existing("assets/data.txt"), existing("main.typ")]);
    assert_eq!(std::fs::read(root.join("main.typ")).unwrap(), b"old main");

    fs::write_extraction(&root, &extraction_plan(), fs::WritePolicy::Overwrite).unwrap();
    assert_eq!(std::fs::read(root.join("main.typ")).unwrap(), b"new main");
    assert_eq!(
        std::fs::read(root.join("unrelated.txt")).unwrap(),
        b"unrelated"
    );
    assert_eq!(
        tree(&root),
        ["assets", "assets/data.txt", "main.typ", "unrelated.txt"]
    );
}

#[test]
fn conflicting_files_and_directories_are_refused_under_every_policy() {
    for policy in [fs::WritePolicy::CreateNew, fs::WritePolicy::Overwrite] {
        let (_directory, root) = temp_root();
        std::fs::create_dir_all(root.join("main.typ")).unwrap();
        std::fs::write(root.join("assets"), b"a file").unwrap();

        let issues = rejected(fs::write_extraction(&root, &extraction_plan(), policy));

        assert_eq!(
            issues,
            [
                fs::WriteIssue::ConflictingAncestor {
                    path: "assets/data.txt".into(),
                    ancestor: root.join("assets"),
                },
                fs::WriteIssue::ConflictingTarget {
                    path: "main.typ".into()
                },
            ]
        );
        assert_eq!(tree(&root), ["assets", "main.typ"]);

        let file = root.join("assets");
        let issues = rejected(fs::write_extraction(&file, &extraction_plan(), policy));
        assert!(matches!(
            issues.as_slice(),
            [fs::WriteIssue::ConflictingAncestor { ancestor, .. }] if ancestor == &file
        ));
    }
}

#[test]
fn unportable_and_case_colliding_plan_paths_are_refused_everywhere() {
    let plan = plan_of(&[
        ("CON.txt", b"device"),
        ("notes.", b"trailing dot"),
        ("Data.txt", b"upper"),
        ("data.txt", b"lower"),
        ("caf\u{e9}.txt", b"composed"),
        ("cafe\u{301}.txt", b"decomposed"),
        ("a\u{3a3}", b"capital sigma"),
        ("a\u{3c3}", b"small sigma"),
    ]);
    let (_directory, root) = temp_root();
    let destination = root.join("written");

    let issues = rejected(fs::write_extraction(
        &destination,
        &plan,
        fs::WritePolicy::Overwrite,
    ));

    for name in ["CON.txt", "notes."] {
        assert!(issues.contains(&fs::WriteIssue::ReservedName {
            path: name.into(),
            component: name.to_owned(),
        }));
    }
    for (first, second) in [
        ("Data.txt", "data.txt"),
        // Plans are in byte order, so the decomposed form comes first.
        ("cafe\u{301}.txt", "caf\u{e9}.txt"),
        ("a\u{3a3}", "a\u{3c3}"),
    ] {
        assert!(
            issues.contains(&fs::WriteIssue::PathConflict {
                first: first.into(),
                second: second.into(),
            }),
            "{first} and {second} collide"
        );
    }
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn symlinks_beneath_the_destination_are_refused_but_above_it_followed() {
    use std::os::unix::fs::symlink;

    let (_directory, root) = temp_root();
    let destination = root.join("written");
    let outside = root.join("outside");
    std::fs::create_dir(&destination).unwrap();
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, destination.join("assets")).unwrap();
    symlink(outside.join("main.typ"), destination.join("main.typ")).unwrap();

    let issues = rejected(fs::write_extraction(
        &destination,
        &extraction_plan(),
        fs::WritePolicy::Overwrite,
    ));

    assert_eq!(
        issues,
        [
            fs::WriteIssue::ConflictingAncestor {
                path: "assets/data.txt".into(),
                ancestor: destination.join("assets"),
            },
            fs::WriteIssue::ConflictingTarget {
                path: "main.typ".into()
            },
        ]
    );
    assert!(tree(&outside).is_empty());

    // The destination itself is a link, then lies beneath one.
    symlink(&outside, root.join("linked")).unwrap();
    let plan = extraction_plan();
    fs::write_extraction(root.join("linked"), &plan, fs::WritePolicy::CreateNew).unwrap();
    fs::write_extraction(
        root.join("linked/nested"),
        &plan,
        fs::WritePolicy::CreateNew,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(outside.join("assets/data.txt")).unwrap(),
        b"new data"
    );
    assert_eq!(
        std::fs::read(outside.join("nested/main.typ")).unwrap(),
        b"new main"
    );
}

#[test]
fn artifacts_are_written_to_their_paths_and_replaced_on_overwrite() {
    let result = compiled("first#pagebreak()second", OutputFormat::Svg);
    let (_directory, root) = temp_root();
    std::fs::create_dir(root.join("pages")).unwrap();
    let paths = [root.join("page-1.svg"), root.join("pages/page-2.svg")];
    std::fs::write(&paths[0], b"stale").unwrap();

    let error = fs::write_artifacts(&paths, &result, fs::WritePolicy::CreateNew).unwrap_err();
    assert!(matches!(
        error,
        fs::WriteError::Io { ref path, ref completed, .. }
            if path == &paths[0] && completed.is_empty()
    ));
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"stale");

    fs::write_artifacts(&paths, &result, fs::WritePolicy::Overwrite).unwrap();
    for (artifact, path) in result.iter().zip(&paths) {
        assert_eq!(std::fs::read(path).unwrap(), artifact.bytes);
    }
    assert_eq!(tree(&root), ["page-1.svg", "pages", "pages/page-2.svg"]);
}

#[test]
fn artifact_writes_reject_count_and_path_conflicts_before_writing() {
    let result = compiled("first#pagebreak()second", OutputFormat::Svg);
    let (_directory, root) = temp_root();
    let same = root.join("same.svg");

    let nested = same.join("nested.svg");

    let issues = rejected(fs::write_artifacts(
        &[same.clone(), nested.clone(), same.clone()],
        &result,
        fs::WritePolicy::Overwrite,
    ));

    assert_eq!(
        issues,
        [
            fs::WriteIssue::PathCountMismatch {
                artifacts: 2,
                paths: 3
            },
            fs::WriteIssue::PathConflict {
                first: same.clone(),
                second: same.clone(),
            },
            fs::WriteIssue::PathConflict {
                first: same,
                second: nested,
            },
        ]
    );
    assert!(tree(&root).is_empty());
}

#[test]
fn a_failed_artifact_write_leaves_no_temporary_file() {
    let result = compiled("= Written", OutputFormat::Pdf);
    let (_directory, root) = temp_root();
    let output = root.join("output.pdf");
    std::fs::create_dir(&output).unwrap();

    let error = fs::write_artifacts(
        std::slice::from_ref(&output),
        &result,
        fs::WritePolicy::Overwrite,
    )
    .unwrap_err();

    assert!(matches!(error, fs::WriteError::Io { ref path, .. } if path == &output));
    assert_eq!(tree(&root), ["output.pdf"]);
}

#[cfg(unix)]
#[test]
fn artifact_paths_resolve_natively_and_need_not_be_unicode() {
    use std::os::unix::ffi::OsStringExt as _;
    use std::os::unix::fs::symlink;

    let result = compiled("= Written", OutputFormat::Pdf);
    let (_directory, root) = temp_root();
    std::fs::create_dir_all(root.join("actual/nested")).unwrap();
    symlink(root.join("actual/nested"), root.join("linked")).unwrap();

    let parent = [root.join("linked/../output.pdf")];
    fs::write_artifacts(&parent, &result, fs::WritePolicy::Overwrite).unwrap();
    assert_eq!(
        std::fs::read(root.join("actual/output.pdf")).unwrap(),
        result[0].bytes
    );

    let path = root.join(std::ffi::OsString::from_vec(b"report-\xff.pdf".to_vec()));
    // APFS and HFS+ refuse names that are not UTF-8.
    if std::fs::write(&path, b"probe").is_ok() {
        let paths = std::slice::from_ref(&path);
        fs::write_artifacts(paths, &result, fs::WritePolicy::Overwrite).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), result[0].bytes);
    }
}

#[test]
fn files_are_written_without_replacing_unless_asked() {
    let (_directory, root) = temp_root();
    let destination = root.join("archive.typk");
    fs::write_file(&destination, b"first", fs::WritePolicy::CreateNew).unwrap();
    let error = fs::write_file(&destination, b"second", fs::WritePolicy::CreateNew).unwrap_err();
    assert!(matches!(
        error,
        fs::WriteError::Io { ref source, .. }
            if source.kind() == std::io::ErrorKind::AlreadyExists
    ));
    assert_eq!(std::fs::read(&destination).unwrap(), b"first");

    fs::write_file(&destination, b"second", fs::WritePolicy::Overwrite).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), b"second");
    assert_eq!(
        tree(&root),
        ["archive.typk"],
        "no temporary file is left behind"
    );
}

#[cfg(unix)]
#[test]
fn overwrite_writes_through_a_final_symbolic_link_and_keeps_the_mode() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let result = compiled("= Written", OutputFormat::Pdf);
    let (_directory, root) = temp_root();
    std::fs::create_dir(root.join("real")).unwrap();
    let real = root.join("real/output.pdf");
    std::fs::write(&real, b"old").unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink("real/output.pdf", root.join("output.pdf")).unwrap();
    symlink("real/created.pdf", root.join("dangling.pdf")).unwrap();
    let linked = [root.join("output.pdf"), root.join("dangling.pdf")];

    for path in &linked {
        let error = fs::write_file(path, b"new", fs::WritePolicy::CreateNew).unwrap_err();
        assert!(matches!(
            error,
            fs::WriteError::Io { ref source, .. }
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ));
    }
    assert_eq!(std::fs::read(&real).unwrap(), b"old");
    assert!(!root.join("real/created.pdf").exists());

    fs::write_artifacts(&linked[..1], &result, fs::WritePolicy::Overwrite).unwrap();
    fs::write_file(&linked[1], b"created", fs::WritePolicy::Overwrite).unwrap();

    for path in &linked {
        assert!(path.symlink_metadata().unwrap().is_symlink());
    }
    assert_eq!(std::fs::read(&real).unwrap(), result[0].bytes);
    let mode = std::fs::metadata(&real).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o640);
    assert_eq!(
        std::fs::read(root.join("real/created.pdf")).unwrap(),
        b"created"
    );
    assert_eq!(
        tree(&root),
        [
            "dangling.pdf",
            "output.pdf",
            "real",
            "real/created.pdf",
            "real/output.pdf"
        ]
    );
}

#[cfg(unix)]
#[test]
fn overwriting_an_extracted_file_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;

    let (_directory, root) = temp_root();
    write_tree(&root, &[("main.typ", "old main")]);
    let main = root.join("main.typ");
    std::fs::set_permissions(&main, std::fs::Permissions::from_mode(0o600)).unwrap();

    fs::write_extraction(&root, &extraction_plan(), fs::WritePolicy::Overwrite).unwrap();

    assert_eq!(std::fs::read(&main).unwrap(), b"new main");
    let mode = std::fs::metadata(&main).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn overwriting_keeps_the_mode_but_not_setuid_or_setgid() {
    use std::os::unix::fs::PermissionsExt;

    let (_directory, root) = temp_root();
    let target = root.join("tool");
    std::fs::write(&target, b"old").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o6755)).unwrap();

    fs::write_file(&target, b"new", fs::WritePolicy::Overwrite).unwrap();

    let mode = std::fs::metadata(&target).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o755);
}

/// The differences from Typst's in-place write that the docs name: a
/// read-only target is replaced, and a writable target in a directory that
/// admits no new entry is rewritten in place.
#[cfg(unix)]
#[test]
fn overwriting_replaces_read_only_files_and_rewrites_in_locked_directories() {
    use std::os::unix::fs::PermissionsExt;

    let (_directory, root) = temp_root();
    let read_only = root.join("read-only.pdf");
    std::fs::write(&read_only, b"old").unwrap();
    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o444)).unwrap();
    fs::write_file(&read_only, b"new", fs::WritePolicy::Overwrite).unwrap();
    assert_eq!(std::fs::read(&read_only).unwrap(), b"new");

    let locked = root.join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join("output.pdf"), b"old").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let written = fs::write_file(
        locked.join("output.pdf"),
        b"new",
        fs::WritePolicy::Overwrite,
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    written.unwrap();
    assert_eq!(std::fs::read(locked.join("output.pdf")).unwrap(), b"new");
    assert_eq!(tree(&locked), ["output.pdf"]);
}
