//! The pack manifest stored as `typst-pack.toml` inside the archive.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use typst::syntax::package::PackageSpec;

use crate::pack::{
    PACKAGE_TREE_IDENTITY_ALGORITHM, PACKAGE_TREE_IDENTITY_KIND, PACKAGE_TREE_IDENTITY_SCHEMA,
};

/// The archive entry name of the manifest.
pub const MANIFEST_PATH: &str = "typst-pack.toml";

/// The pack format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The parsed contents of `typst-pack.toml`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PackManifest {
    /// The pack format version. Readers must reject versions they don't know.
    format_version: u32,
    /// The packed Typst project.
    project: ProjectManifest,
    /// Package dependencies observed while creating the pack.
    #[serde(default, skip_serializing_if = "PackagesManifest::is_empty")]
    packages: PackagesManifest,
    /// Fonts embedded in the pack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fonts: Vec<FontManifest>,
    /// Optional descriptive metadata about the packed project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<PackMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Version1Manifest {
    format_version: u32,
    project: ProjectManifest,
    #[serde(default)]
    packages: Version1PackagesManifest,
    #[serde(default)]
    fonts: Vec<FontManifest>,
    #[serde(default)]
    metadata: Option<PackMetadata>,
}

impl TryFrom<Version1Manifest> for PackManifest {
    type Error = PackManifestError;

    fn try_from(manifest: Version1Manifest) -> Result<Self, Self::Error> {
        Ok(Self {
            format_version: manifest.format_version,
            project: manifest.project,
            packages: manifest.packages.try_into()?,
            fonts: manifest.fonts,
            metadata: manifest.metadata,
        })
    }
}

impl<'de> Deserialize<'de> for PackManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = toml::Value::deserialize(deserializer)?;
        parse_manifest_value(value).map_err(serde::de::Error::custom)
    }
}

/// The `[project]` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProjectManifest {
    /// The root-relative path of the entrypoint file, e.g. `main.typ`.
    entrypoint: String,
    /// Non-source project locations supplied at compilation time.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    resource_slots: BTreeSet<String>,
}

/// The `[packages]` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PackagesManifest {
    /// Exact package trees whose files are stored inside the Pack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    vendored: Vec<PackageManifest>,
    /// Exact package trees that must be externally fulfilled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unvendored: Vec<PackageManifest>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Version1PackagesManifest {
    #[serde(default)]
    vendored: Vec<PackageManifest>,
    #[serde(default)]
    unvendored: Vec<PackageManifest>,
}

impl TryFrom<Version1PackagesManifest> for PackagesManifest {
    type Error = PackManifestError;

    fn try_from(packages: Version1PackagesManifest) -> Result<Self, Self::Error> {
        Ok(Self {
            vendored: canonical_packages(packages.vendored)?,
            unvendored: canonical_packages(packages.unvendored)?,
        })
    }
}

impl<'de> Deserialize<'de> for PackagesManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Version1PackagesManifest::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

/// One exact Complete Package Tree declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PackageManifest {
    spec: String,
    tree_digest: String,
    tree_identity_kind: String,
    tree_identity_schema: String,
    tree_identity_algorithm: String,
    file_count: u64,
    byte_length: u64,
}

/// One `[[fonts]]` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FontManifest {
    /// The archive entry holding the font data, e.g. `fonts/dejavu-sans.ttf`.
    path: String,
    /// The face index inside the font file (non-zero for collections).
    #[serde(default, skip_serializing_if = "is_zero")]
    index: u32,
    /// Family names provided by this face, informational only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    families: Vec<String>,
    /// Whether the exact container must be supplied when compiling.
    #[serde(default, skip_serializing_if = "is_false")]
    external: bool,
    /// The canonical container digest, encoded as 32 lowercase hexadecimal digits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_digest: Option<String>,
    /// Canonical identity kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_identity_kind: Option<String>,
    /// Canonical identity schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_identity_schema: Option<String>,
    /// Canonical identity digest algorithm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_identity_algorithm: Option<String>,
    /// The exact container byte length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    container_length: Option<u64>,
}

fn is_zero(index: &u32) -> bool {
    *index == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The optional `[metadata]` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PackMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authors: Vec<String>,
}

impl ProjectManifest {
    /// The root-relative entrypoint path.
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// The declared Resource Slot paths in deterministic order.
    pub fn resource_slots(&self) -> impl Iterator<Item = &str> {
        self.resource_slots.iter().map(String::as_str)
    }

    pub(crate) fn contains_resource_slot(&self, path: &str) -> bool {
        self.resource_slots.contains(path)
    }
}

impl PackagesManifest {
    /// Exact package trees stored inside the Pack.
    pub fn vendored(&self) -> &[PackageManifest] {
        &self.vendored
    }

    /// Exact package trees fulfilled outside the Pack.
    pub fn unvendored(&self) -> &[PackageManifest] {
        &self.unvendored
    }

    fn is_empty(&self) -> bool {
        self.vendored.is_empty() && self.unvendored.is_empty()
    }
}

impl PackageManifest {
    pub(crate) fn new(
        spec: PackageSpec,
        tree_digest: String,
        file_count: u64,
        byte_length: u64,
    ) -> Self {
        Self {
            spec: spec.to_string(),
            tree_digest,
            tree_identity_kind: PACKAGE_TREE_IDENTITY_KIND.to_owned(),
            tree_identity_schema: PACKAGE_TREE_IDENTITY_SCHEMA.to_owned(),
            tree_identity_algorithm: PACKAGE_TREE_IDENTITY_ALGORITHM.to_owned(),
            file_count,
            byte_length,
        }
    }

    pub fn spec(&self) -> Result<PackageSpec, PackManifestError> {
        PackageSpec::from_str(&self.spec).map_err(|error| PackManifestError::InvalidPackageSpec {
            spec: self.spec.clone(),
            message: error.to_string(),
        })
    }

    pub fn tree_digest(&self) -> &str {
        &self.tree_digest
    }
    pub fn tree_identity_kind(&self) -> &str {
        &self.tree_identity_kind
    }
    pub fn tree_identity_schema(&self) -> &str {
        &self.tree_identity_schema
    }
    pub fn tree_identity_algorithm(&self) -> &str {
        &self.tree_identity_algorithm
    }
    pub fn file_count(&self) -> u64 {
        self.file_count
    }
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

impl std::fmt::Display for PackageManifest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.spec)
    }
}

impl FontManifest {
    /// The archive path containing this font's bytes.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The face index within the font data.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Informational family names declared for this face.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// Whether this face's container is externally fulfilled.
    pub fn is_external(&self) -> bool {
        self.external
    }

    pub(crate) fn container_digest(&self) -> Option<&str> {
        self.container_digest.as_deref()
    }

    pub(crate) fn container_length(&self) -> Option<u64> {
        self.container_length
    }

    pub(crate) fn container_identity_kind(&self) -> Option<&str> {
        self.container_identity_kind.as_deref()
    }

    pub(crate) fn container_identity_schema(&self) -> Option<&str> {
        self.container_identity_schema.as_deref()
    }

    pub(crate) fn container_identity_algorithm(&self) -> Option<&str> {
        self.container_identity_algorithm.as_deref()
    }

    pub(crate) fn new(
        path: String,
        index: u32,
        families: Vec<String>,
        external: bool,
        container_digest: String,
        container_length: u64,
    ) -> Self {
        Self {
            path,
            index,
            families,
            external,
            container_digest: Some(container_digest),
            container_identity_kind: Some("font-container".to_owned()),
            container_identity_schema: Some("typst-pack-font-container-identity-v1".to_owned()),
            container_identity_algorithm: Some("typst-hash128-0.15".to_owned()),
            container_length: Some(container_length),
        }
    }
}

impl PackMetadata {
    /// Creates empty Pack metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the human-readable Pack name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the Pack description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a Pack author.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.authors.push(author.into());
        self
    }

    /// The human-readable Pack name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The Pack description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The Pack authors.
    pub fn authors(&self) -> &[String] {
        &self.authors
    }
}

/// A manifest that could not be accepted.
#[derive(Debug, thiserror::Error)]
pub enum PackManifestError {
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("missing or invalid `format-version`")]
    InvalidFormatVersion,
    #[error("unsupported pack format version {0} (this reader supports version {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid package spec `{spec}`: {message}")]
    InvalidPackageSpec { spec: String, message: String },
    #[error("package requirement `{spec}` is declared more than once with conflicting values")]
    ConflictingPackageRequirements { spec: String },
}

impl PackManifest {
    pub(crate) fn new(
        entrypoint: String,
        resource_slots: BTreeSet<String>,
        vendored_packages: Vec<PackageManifest>,
        unvendored_packages: Vec<PackageManifest>,
        fonts: Vec<FontManifest>,
        metadata: Option<PackMetadata>,
    ) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            project: ProjectManifest {
                entrypoint,
                resource_slots,
            },
            packages: PackagesManifest {
                vendored: vendored_packages,
                unvendored: unvendored_packages,
            },
            fonts,
            metadata,
        }
    }

    /// The Pack format version.
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// The project declarations.
    pub fn project(&self) -> &ProjectManifest {
        &self.project
    }

    /// The package declarations.
    pub fn packages(&self) -> &PackagesManifest {
        &self.packages
    }

    /// The embedded font declarations.
    pub fn fonts(&self) -> &[FontManifest] {
        &self.fonts
    }

    /// Optional descriptive Pack metadata.
    pub fn metadata(&self) -> Option<&PackMetadata> {
        self.metadata.as_ref()
    }

    /// Parses and validates a manifest from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, PackManifestError> {
        Self::from_toml_value(toml::from_str(text)?)
    }

    pub(crate) fn from_toml_value(value: toml::Value) -> Result<Self, PackManifestError> {
        parse_manifest_value(value)
    }

    /// Serializes the manifest to TOML text.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("manifest is always serializable")
    }

    /// Checks internal consistency of the manifest.
    fn validate(&self) -> Result<(), PackManifestError> {
        if self.format_version != FORMAT_VERSION {
            return Err(PackManifestError::UnsupportedVersion(self.format_version));
        }
        Ok(())
    }

    /// The vendored package requirements.
    pub fn vendored_packages(&self) -> &[PackageManifest] {
        &self.packages.vendored
    }

    /// The externally fulfilled package requirements.
    pub fn unvendored_packages(&self) -> &[PackageManifest] {
        &self.packages.unvendored
    }
}

fn parse_manifest_value(value: toml::Value) -> Result<PackManifest, PackManifestError> {
    let version = value
        .get("format-version")
        .and_then(toml::Value::as_integer)
        .ok_or(PackManifestError::InvalidFormatVersion)?;
    let version = u32::try_from(version).map_err(|_| PackManifestError::InvalidFormatVersion)?;
    if version != FORMAT_VERSION {
        return Err(PackManifestError::UnsupportedVersion(version));
    }
    let wire: Version1Manifest = value.try_into()?;
    let manifest = PackManifest::try_from(wire)?;
    manifest.validate()?;
    Ok(manifest)
}

fn canonical_packages(
    packages: Vec<PackageManifest>,
) -> Result<Vec<PackageManifest>, PackManifestError> {
    let mut canonical = BTreeMap::new();
    for package in packages {
        let spec = package.spec()?.to_string();
        if let Some(existing) = canonical.insert(spec.clone(), package.clone())
            && existing != package
        {
            return Err(PackManifestError::ConflictingPackageRequirements { spec });
        }
    }
    Ok(canonical.into_values().collect())
}
