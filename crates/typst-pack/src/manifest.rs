//! The pack manifest stored as `typst-pack.toml` inside the archive.

use serde::{Deserialize, Serialize};
use typst::syntax::package::PackageSpec;

use crate::Digest;
use crate::pack::FontFace;

/// The archive member name of the manifest.
pub const MANIFEST_PATH: &str = "typst-pack.toml";

/// The pack format version this crate reads and writes.
///
/// It changes on every incompatible format change; readers reject every other
/// version. Unknown keys are ignored, so compatible additions keep the version.
pub const FORMAT_VERSION: i64 = 2;

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
    pub(crate) metadata: Option<PackMetadata>,
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
    /// The faces of the container; informational for embedded containers.
    #[serde(default)]
    pub(crate) faces: Vec<FontFace>,
}

/// Optional descriptive metadata, stored as `[metadata]` and excluded from
/// Pack Identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackMetadata {
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

/// A manifest that could not be accepted.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("the {MANIFEST_PATH} manifest is not valid UTF-8: {0}")]
    NotUtf8(#[source] std::str::Utf8Error),
    #[error("failed to parse the {MANIFEST_PATH} manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("the {MANIFEST_PATH} manifest has no integer `format-version`")]
    MissingFormatVersion,
    #[error("unsupported pack format version {0} (this reader supports version {FORMAT_VERSION})")]
    UnsupportedVersion(i64),
}

impl Manifest {
    /// Parses a manifest, rejecting every format version but [`FORMAT_VERSION`]
    /// before interpreting any other key.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        let text = std::str::from_utf8(bytes).map_err(ManifestError::NotUtf8)?;
        let table: toml::Table = toml::from_str(text)?;
        let version = table
            .get("format-version")
            .and_then(toml::Value::as_integer)
            .ok_or(ManifestError::MissingFormatVersion)?;
        if version != FORMAT_VERSION {
            return Err(ManifestError::UnsupportedVersion(version));
        }
        Ok(table.try_into()?)
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

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PackageSpec, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(|error| {
            serde::de::Error::custom(format!("invalid package spec {value:?}: {error}"))
        })
    }
}
