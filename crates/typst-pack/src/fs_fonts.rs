//! Font Catalog reading for the reference filesystem Font Authority.
//!
//! Font directories are host configuration, so reading them is lenient:
//! symbolic links are followed wherever they lead, and an unreadable
//! directory, unreadable file, or unparseable font is skipped and reported
//! rather than failing the whole catalog. Only an explicitly configured root
//! that cannot be opened, or an exceeded limit, fails the read.

use std::path::{Path, PathBuf};

#[cfg(feature = "embedded-fonts")]
use crate::font_catalog::typst_embedded_font_containers;
use crate::font_catalog::{FontCatalog, FontCatalogEntry, FontContainer, FontDisposition};
use crate::fs_read::{FilesystemReadError, Kind, Root, Walker};
use crate::limits::ReadLimits;

/// One explicitly configured source of Font Containers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemFontSource {
    kind: SourceKind,
    disposition: FontDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceKind {
    System,
    #[cfg(feature = "embedded-fonts")]
    TypstEmbedded,
    Directory(PathBuf),
}

impl FilesystemFontSource {
    /// Selects the host's standard system font directories at this position.
    pub fn system(disposition: FontDisposition) -> Self {
        Self {
            kind: SourceKind::System,
            disposition,
        }
    }

    /// Selects Typst's compiled-in Font Containers at this position.
    #[cfg(feature = "embedded-fonts")]
    pub fn typst_embedded(disposition: FontDisposition) -> Self {
        Self {
            kind: SourceKind::TypstEmbedded,
            disposition,
        }
    }

    /// Selects every eligible Font Container beneath one directory.
    pub fn directory(path: impl Into<PathBuf>, disposition: FontDisposition) -> Self {
        Self {
            kind: SourceKind::Directory(path.into()),
            disposition,
        }
    }

    /// The disposition every catalog position from this source carries.
    pub fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// A Font Catalog read from filesystem sources.
#[derive(Debug)]
pub struct FilesystemFonts {
    /// The catalog of every readable Font Container, in source order.
    pub catalog: FontCatalog,
    /// Font files and directories that could not be read and were left out.
    pub skipped: Vec<FilesystemReadError>,
}

/// Reads one ordered Font Catalog from explicitly configured sources.
///
/// Sources compose in iterator order. Paths within one scanned root compose in
/// lexical order, and no system or embedded source is added implicitly. Only
/// files with a font extension are considered.
pub fn read_filesystem_fonts(
    sources: impl IntoIterator<Item = FilesystemFontSource>,
    limits: ReadLimits,
) -> Result<FilesystemFonts, FilesystemReadError> {
    let mut walker = Walker::lenient(limits);
    let mut catalog = FontCatalog::new();
    let mut invalid = Vec::new();

    for source in sources {
        let disposition = source.disposition;
        let roots = match source.kind {
            SourceKind::System => system_font_roots(),
            #[cfg(feature = "embedded-fonts")]
            SourceKind::TypstEmbedded => {
                for container in typst_embedded_font_containers() {
                    catalog.push(FontCatalogEntry::new(container, disposition));
                }
                continue;
            }
            SourceKind::Directory(root) => {
                // An explicitly configured root must exist.
                let root = Root::open(&root)?;
                read_root(&mut walker, &root, disposition, &mut catalog, &mut invalid)?;
                continue;
            }
        };
        for root in roots {
            let root = match Root::open(&root) {
                Ok(root) => root,
                Err(FilesystemReadError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    continue;
                }
                Err(error) => {
                    walker.skipped.push(error);
                    continue;
                }
            };
            read_root(&mut walker, &root, disposition, &mut catalog, &mut invalid)?;
        }
    }

    let mut skipped = walker.skipped;
    skipped.extend(invalid);
    Ok(FilesystemFonts { catalog, skipped })
}

fn read_root(
    walker: &mut Walker,
    root: &Root,
    disposition: FontDisposition,
    catalog: &mut FontCatalog,
    invalid: &mut Vec<FilesystemReadError>,
) -> Result<(), FilesystemReadError> {
    let base = root.path().to_owned();
    walker.walk(
        root,
        &mut |relative, kind| Ok(kind == Kind::Dir || font_eligible(relative)),
        &mut |relative, bytes| {
            match FontContainer::new(bytes) {
                Ok(container) => catalog.push(FontCatalogEntry::new(container, disposition)),
                Err(source) => invalid.push(FilesystemReadError::InvalidFont {
                    path: base.join(relative),
                    source,
                }),
            }
            Ok(())
        },
    )
}

fn font_eligible(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(crate::read_layout::is_font_container_extension)
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
