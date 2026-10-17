//! The pack manifest stored as `typst-pack.toml` inside the archive.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use typst::layout::Ratio;
use typst::syntax::package::PackageSpec;
use typst::text::{FontStretch, FontStyle, FontWeight};

use crate::Digest;
use crate::archive::DecodeError;
use crate::font::Face;
use crate::paths::canonical_relative_path;

/// The archive member name of the manifest.
pub(crate) const MANIFEST_PATH: &str = "typst-pack.toml";

/// The pack format version this crate reads and writes.
///
/// It changes on every incompatible format change; readers reject every other
/// version. Unknown keys are ignored, so compatible additions keep the version.
pub const FORMAT_VERSION: i64 = 2;

/// The `generator` this crate records.
pub(crate) const GENERATOR: &str = concat!("typst-pack ", env!("CARGO_PKG_VERSION"));

/// The contents of `typst-pack.toml`.
///
/// One struct is both written and read. Unknown keys are ignored so that newer
/// producers can add compatible fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Manifest {
    pub(crate) format_version: i64,
    /// The Typst version of the producer; informational.
    #[serde(default)]
    pub(crate) typst_version: String,
    /// The producing tool and its version; informational.
    #[serde(default)]
    pub(crate) generator: String,
    pub(crate) entrypoint: String,
    #[serde(default)]
    pub(crate) files: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) packages: Vec<PackageEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) fonts: Vec<FontEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<Metadata>,
}

/// One `[[files]]` entry: a project file and the SHA-256 of its bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) digest: Digest,
}

/// One `[[packages]]` entry: an exact package and its tree digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PackageEntry {
    #[serde(with = "package_spec")]
    pub(crate) spec: PackageSpec,
    pub(crate) digest: Digest,
    pub(crate) embedded: bool,
}

/// One `[[fonts]]` entry: a font container, in font book order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FontEntry {
    pub(crate) digest: Digest,
    pub(crate) embedded: bool,
    /// The faces of the container; informational.
    #[serde(default)]
    pub(crate) faces: Vec<FaceEntry>,
}

/// One `[[fonts.faces]]` entry.
///
/// Faces are informational, so reading them is lenient: every field is
/// optional, an unknown style reads as `normal`, and out-of-range numbers are
/// clamped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FaceEntry {
    /// The face index within the container.
    pub(crate) index: u32,
    /// The typographic family name.
    pub(crate) family: String,
    /// `normal`, `italic`, or `oblique`.
    pub(crate) style: String,
    /// The weight on the CSS scale from 100 to 900; 400 is normal.
    pub(crate) weight: u16,
    /// The width in per mille of the normal width, from 500 to 2000; 1000 is
    /// normal.
    pub(crate) stretch: u16,
}

impl Default for FaceEntry {
    fn default() -> Self {
        Self {
            index: 0,
            family: String::new(),
            style: "normal".to_owned(),
            weight: 400,
            stretch: 1000,
        }
    }
}

impl From<&Face> for FaceEntry {
    fn from(face: &Face) -> Self {
        let style = match face.style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        };
        Self {
            index: face.index,
            family: face.family.clone(),
            style: style.to_owned(),
            weight: face.weight.to_number(),
            stretch: (face.stretch.to_ratio().get() * 1000.0).round() as u16,
        }
    }
}

impl From<&FaceEntry> for Face {
    fn from(entry: &FaceEntry) -> Self {
        let style = match entry.style.as_str() {
            "italic" => FontStyle::Italic,
            "oblique" => FontStyle::Oblique,
            _ => FontStyle::Normal,
        };
        Self {
            index: entry.index,
            family: entry.family.clone(),
            style,
            weight: FontWeight::from_number(entry.weight),
            // Half a unit up keeps the per-mille value exact through the
            // ratio, which Typst truncates back to per mille.
            stretch: FontStretch::from_ratio(Ratio::new((f64::from(entry.stretch) + 0.5) / 1000.0)),
        }
    }
}

/// Optional descriptive metadata of a pack, stored as `[metadata]` and
/// excluded from its identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// A human-readable pack name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A description of the packed document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The pack authors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

impl Manifest {
    /// Parses a manifest, rejecting every format version but [`FORMAT_VERSION`]
    /// before interpreting any other key.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, DecodeError> {
        let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::ManifestNotUtf8)?;
        let table: toml::Table = toml::from_str(text).map_err(DecodeError::manifest)?;
        let version = table
            .get("format-version")
            .and_then(toml::Value::as_integer)
            .ok_or(DecodeError::MissingFormatVersion)?;
        if version != FORMAT_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let manifest: Self = table.try_into().map_err(DecodeError::manifest)?;
        manifest.check().map_err(DecodeError::AmbiguousManifest)?;
        Ok(manifest)
    }

    /// Rejects declarations that would give one archive several readings:
    /// paths that are not already canonical, and a path, specification, or
    /// font digest declared twice. Paths that cannot be canonicalized at all
    /// are left to pack validation.
    fn check(&self) -> Result<(), String> {
        let canonical = |path: &str| {
            canonical_relative_path(path).map_or(true, |canonical| canonical.as_str() == path)
        };
        if !canonical(&self.entrypoint) {
            return Err(format!(
                "entrypoint {:?} is not a canonical path",
                self.entrypoint
            ));
        }
        let mut paths = BTreeSet::new();
        for entry in &self.files {
            if !canonical(&entry.path) {
                return Err(format!(
                    "project file {:?} is not a canonical path",
                    entry.path
                ));
            }
            if !paths.insert(entry.path.as_str()) {
                return Err(format!(
                    "project file {:?} is declared more than once",
                    entry.path
                ));
            }
        }
        let mut specs = BTreeSet::new();
        for entry in &self.packages {
            if !specs.insert(entry.spec.to_string()) {
                return Err(format!("package {} is declared more than once", entry.spec));
            }
        }
        let mut digests = BTreeSet::new();
        for entry in &self.fonts {
            if !digests.insert(entry.digest) {
                return Err(format!(
                    "font container {} is declared more than once",
                    entry.digest
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn to_toml(&self) -> String {
        toml::to_string(self).expect("a manifest is always representable as TOML")
    }
}

mod package_spec {
    use serde::{Deserialize, Deserializer, Serializer};
    use typst::syntax::package::PackageSpec;

    pub(super) fn serialize<S: Serializer>(
        spec: &PackageSpec,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(spec)
    }

    /// Parses a specification written in canonical form only, so one
    /// manifest text names one package.
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PackageSpec, D::Error> {
        let value = String::deserialize(deserializer)?;
        let spec: PackageSpec = value.parse().map_err(|error| {
            serde::de::Error::custom(format!("invalid package spec {value:?}: {error}"))
        })?;
        if spec.to_string() != value {
            return Err(serde::de::Error::custom(format!(
                "package spec {value:?} is not in canonical form `{spec}`"
            )));
        }
        Ok(spec)
    }
}
