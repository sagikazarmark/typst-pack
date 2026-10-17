//! Reading fonts from the host.
//!
//! Font directories are host configuration, so reading them is lenient:
//! symbolic links are followed wherever they lead, and an unreadable
//! directory, unreadable file, file over the per-file limit, or unparseable
//! font is skipped and reported rather than failing the whole read. When a
//! directory exceeds the entry or total byte limit, the rest of it is skipped
//! and reported. Only an explicitly configured directory that cannot be
//! opened fails the read.

use std::path::{Path, PathBuf};

use super::Error;
use super::walk::{Kind, Root, Walker};
use crate::{ReadLimits, font};

/// Where fonts are read from, and whether a created pack embeds them.
pub(crate) enum FontSource {
    /// The host's standard system font directories.
    System { embedded: bool },
    /// Typst's embedded fonts.
    #[cfg(feature = "embedded-fonts")]
    Typst { embedded: bool },
    /// Every font file beneath one directory, which must exist.
    Directory { path: PathBuf, embedded: bool },
}

/// Reads fonts from the sources in order, and from each directory in
/// lexical path order. Returns the containers read and the files and
/// directories that were skipped.
pub(crate) fn read_fonts(
    sources: Vec<FontSource>,
    limits: ReadLimits,
) -> Result<(Vec<font::Entry>, Vec<Error>), Error> {
    let mut walker = Walker::lenient(limits);
    let mut entries = Vec::new();
    let mut invalid = Vec::new();
    for source in sources {
        let (roots, embedded) = match source {
            FontSource::System { embedded } => (system_font_roots(), embedded),
            #[cfg(feature = "embedded-fonts")]
            FontSource::Typst { embedded } => {
                entries.extend(font::embedded().map(|container| font::Entry {
                    container,
                    embedded,
                }));
                continue;
            }
            FontSource::Directory { path, embedded } => {
                let root = Root::open(&path)?;
                read_root(&mut walker, &root, embedded, &mut entries, &mut invalid);
                continue;
            }
        };
        for root in roots {
            let root = match Root::open(&root) {
                Ok(root) => root,
                Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                    continue;
                }
                Err(error) => {
                    walker.skipped.push(error);
                    continue;
                }
            };
            read_root(&mut walker, &root, embedded, &mut entries, &mut invalid);
        }
    }
    let mut skipped = walker.skipped;
    skipped.extend(invalid);
    Ok((entries, skipped))
}

/// Reads the fonts beneath one root. A lenient walk stops only at the entry or
/// total byte limit, which skips the rest of this root.
fn read_root(
    walker: &mut Walker,
    root: &Root,
    embedded: bool,
    entries: &mut Vec<font::Entry>,
    invalid: &mut Vec<Error>,
) {
    let walked = walker.walk(
        root,
        |relative, kind| Ok(kind == Kind::Dir || is_font_file(relative)),
        |relative, bytes| {
            match font::Container::new(bytes) {
                Some(container) => entries.push(font::Entry {
                    container,
                    embedded,
                }),
                None => invalid.push(Error::InvalidFont {
                    path: root.path.join(relative),
                }),
            }
            Ok(())
        },
    );
    if let Err(error) = walked {
        walker.skipped.push(error);
    }
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(font::is_container_extension)
}

fn system_font_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let system_root = std::env::var_os("SYSTEMROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        roots.push(system_root.join("Fonts"));
        if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            roots.push(home.join(r"AppData\Local\Microsoft\Windows\Fonts"));
            roots.push(home.join(r"AppData\Roaming\Microsoft\Windows\Fonts"));
        }
        if let Some(data) = std::env::var_os("APPDATA").map(PathBuf::from) {
            roots.push(data.join("Adobe/CoreSync/plugins/livetype/r"));
            roots.push(data.join("Adobe/User Owned Fonts"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        roots.extend([
            PathBuf::from("/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts"),
        ]);
        // Downloadable system fonts live in versioned asset directories.
        if let Ok(entries) = std::fs::read_dir("/System/Library/AssetsV2") {
            let mut assets = entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("com_apple_MobileAsset_Font")
                })
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            assets.sort();
            roots.extend(assets);
        }
        roots.push(PathBuf::from("/Network/Library/Fonts"));
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            roots.push(home.join("Library/Fonts"));
            let data = home.join("Library/Application Support/Adobe");
            roots.push(data.join("CoreSync/plugins/livetype/.r"));
            roots.push(data.join(".User Owned Fonts"));
        }
    }

    #[cfg(target_os = "redox")]
    roots.push(PathBuf::from("/ui/fonts"));

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    roots.extend(fontconfig_roots());

    roots
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn fontconfig_roots() -> Vec<PathBuf> {
    let mut config = fontconfig_parser::FontConfig::default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(config_file) = std::env::var_os("FONTCONFIG_FILE") {
        let _ = config.merge_config(Path::new(&config_file));
    } else {
        let user_config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".config")));
        let read_global = user_config.is_none_or(|path| {
            config
                .merge_config(&path.join("fontconfig/fonts.conf"))
                .is_err()
        });
        if read_global {
            let _ = config.merge_config(Path::new("/etc/fonts/local.conf"));
        }
        let _ = config.merge_config(Path::new("/etc/fonts/fonts.conf"));
    }

    let mut roots = config
        .dirs
        .into_iter()
        .filter_map(|directory| {
            if directory.path.starts_with("~") {
                home.as_ref()
                    .map(|home| home.join(directory.path.strip_prefix("~").unwrap()))
            } else {
                Some(directory.path)
            }
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.extend([
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ]);
        if let Some(home) = home {
            roots.push(home.join(".fonts"));
            roots.push(home.join(".local/share/fonts"));
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "embedded-fonts")]
    use crate::Digest;

    fn read_directory(
        root: &Path,
        limits: ReadLimits,
    ) -> Result<(Vec<font::Entry>, Vec<Error>), Error> {
        let path = root.to_owned();
        read_fonts(
            vec![FontSource::Directory {
                path,
                embedded: false,
            }],
            limits,
        )
    }

    fn skipped_paths(skipped: &[Error]) -> Vec<PathBuf> {
        skipped
            .iter()
            .map(|skipped| match skipped {
                Error::Io { path, .. }
                | Error::BrokenSymlink { path, .. }
                | Error::Limit { path, .. }
                | Error::Unsupported { path }
                | Error::InvalidFont { path } => path.clone(),
                other => panic!("unexpected skipped entry {other}"),
            })
            .collect()
    }

    #[cfg(feature = "embedded-fonts")]
    fn typst_container() -> Vec<u8> {
        font::embedded().next().unwrap().data().to_vec()
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn sources_compose_in_order_with_their_dispositions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fonts");
        std::fs::create_dir_all(&root).unwrap();
        let first = typst_container();
        let mut second = first.clone();
        second.push(0);
        std::fs::write(root.join("b.ttf"), &second).unwrap();
        std::fs::write(root.join("a.ttf"), &first).unwrap();

        let directory = |embedded| FontSource::Directory {
            path: root.clone(),
            embedded,
        };
        let (entries, skipped) = read_fonts(
            vec![
                directory(false),
                FontSource::Typst { embedded: true },
                directory(true),
            ],
            ReadLimits::default(),
        )
        .unwrap();

        let typst = font::embedded().count();
        let summary = entries
            .iter()
            .map(|entry| (entry.container.digest(), entry.embedded))
            .collect::<Vec<_>>();
        assert_eq!(summary.len(), typst + 4);
        assert_eq!(
            summary[..2],
            [(Digest::of(&first), false), (Digest::of(&second), false)]
        );
        assert!(summary[2..2 + typst].iter().all(|(_, embedded)| *embedded));
        assert_eq!(
            summary[2 + typst..],
            [(Digest::of(&first), true), (Digest::of(&second), true)]
        );
        assert!(skipped.is_empty());
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn exceeded_font_limits_skip_files_and_roots_but_typst_fonts_are_not_counted() {
        let dir = tempfile::tempdir().unwrap();
        let data = typst_container();
        let size = data.len() as u64;
        let (first, second) = (dir.path().join("first"), dir.path().join("second"));
        for root in [&first, &second] {
            std::fs::create_dir(root).unwrap();
            // Files that are not fonts count toward no limit.
            for index in 0..4 {
                std::fs::write(root.join(format!("{index}.txt")), b"not a font").unwrap();
            }
        }
        std::fs::write(first.join("a.ttf"), &data).unwrap();
        std::fs::write(first.join("b.ttf"), &data).unwrap();
        let mut larger = data.clone();
        larger.push(0);
        std::fs::write(second.join("a.ttf"), &larger).unwrap();
        std::fs::write(second.join("b.ttf"), &data).unwrap();
        let read = |limits| {
            let directory = |path: &PathBuf| FontSource::Directory {
                path: path.clone(),
                embedded: false,
            };
            let (entries, skipped) =
                read_fonts(vec![directory(&first), directory(&second)], limits).unwrap();
            let skipped = skipped
                .iter()
                .map(|error| match error {
                    Error::Limit { path, source } => (path.clone(), source.resource),
                    other => panic!("unexpected skipped entry {other}"),
                })
                .collect::<Vec<_>>();
            (entries.len(), skipped)
        };

        // A file over the per-file limit is skipped alone.
        let (found, skipped) = read(ReadLimits {
            max_file_bytes: size,
            ..ReadLimits::default()
        });
        assert_eq!(found, 3);
        assert_eq!(skipped, [(second.join("a.ttf"), "file bytes")]);

        // An exceeded entry or total limit skips the rest of a root.
        let (found, skipped) = read(ReadLimits {
            max_entries: 3,
            ..ReadLimits::default()
        });
        assert_eq!(found, 3);
        assert_eq!(skipped, [(second.join("b.ttf"), "entries")]);
        let (found, skipped) = read(ReadLimits {
            max_total_bytes: 2 * size,
            ..ReadLimits::default()
        });
        assert_eq!(found, 2);
        assert_eq!(skipped, [(second.join("a.ttf"), "total bytes")]);

        let empty = ReadLimits {
            max_entries: 0,
            max_file_bytes: 0,
            max_total_bytes: 0,
        };
        let (entries, _) = read_fonts(vec![FontSource::Typst { embedded: false }], empty).unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn a_missing_configured_directory_fails_the_read() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");

        let error = read_directory(&missing, ReadLimits::default()).unwrap_err();

        assert!(matches!(
            error,
            Error::Io { path, source }
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
            std::fs::write(dir.path().join(name), b"not a font").unwrap();
        }
        let mut selected = candidates
            .iter()
            .filter(|(_, eligible)| *eligible)
            .map(|(name, _)| dir.path().join(name))
            .collect::<Vec<_>>();
        selected.sort();

        let (entries, skipped) = read_directory(dir.path(), ReadLimits::default()).unwrap();

        // Every selected file is unreadable as a font, so the skipped entries
        // name exactly what selection accepted.
        assert!(entries.is_empty());
        assert!(
            skipped
                .iter()
                .all(|skipped| matches!(skipped, Error::InvalidFont { .. }))
        );
        assert_eq!(skipped_paths(&skipped), selected);
    }

    #[test]
    #[cfg(all(unix, feature = "embedded-fonts"))]
    fn links_are_followed_anywhere_and_bad_entries_are_skipped() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fonts");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(root.join("unreadable")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let data = typst_container();
        std::fs::write(elsewhere.join("linked.ttf"), &data).unwrap();
        std::fs::write(elsewhere.join("README"), b"not a font").unwrap();
        symlink(elsewhere.join("linked.ttf"), root.join("a-linked.ttf")).unwrap();
        symlink(&elsewhere, root.join("b-directory")).unwrap();
        symlink(elsewhere.join("missing.ttf"), root.join("c-broken.ttf")).unwrap();
        symlink(elsewhere.join("missing"), root.join("d-broken-other")).unwrap();
        std::fs::write(root.join("e-broken.ttf"), b"not a font").unwrap();
        let _socket = UnixListener::bind(root.join("f-socket.ttf")).unwrap();
        std::fs::write(root.join("unreadable/hidden.ttf"), &data).unwrap();
        let set_mode = |mode| {
            let mut permissions = std::fs::metadata(root.join("unreadable"))
                .unwrap()
                .permissions();
            std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, mode);
            std::fs::set_permissions(root.join("unreadable"), permissions).unwrap();
        };
        set_mode(0o000);
        // Privileged users read the directory anyway.
        let unreadable = std::fs::read_dir(root.join("unreadable")).is_err();

        let (entries, skipped) = read_directory(&root, ReadLimits::default()).unwrap();
        set_mode(0o755);

        let identity = Digest::of(&data);
        let found = entries
            .iter()
            .filter(|entry| entry.container.digest() == identity)
            .count();
        assert_eq!(found, if unreadable { 2 } else { 3 });
        assert!(matches!(&skipped[0], Error::BrokenSymlink { .. }));
        let mut expected = vec![
            root.join("c-broken.ttf"),
            root.join("f-socket.ttf"),
            root.join("e-broken.ttf"),
        ];
        if unreadable {
            expected.insert(2, root.join("unreadable"));
        }
        assert_eq!(skipped_paths(&skipped), expected);
    }

    #[test]
    #[cfg(unix)]
    fn fifos_with_font_names_are_skipped_without_blocking() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("blocked.ttf");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(status.success());

        let (_, skipped) = read_directory(dir.path(), ReadLimits::default()).unwrap();

        assert!(matches!(
            skipped.as_slice(),
            [Error::Unsupported { path }] if path == &fifo
        ));
    }
}
