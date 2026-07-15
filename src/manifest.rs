//! The pack manifest stored as `typst-pack.toml` inside the archive.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use typst::syntax::package::PackageSpec;

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
        let version = value
            .get("format-version")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| serde::de::Error::custom("missing or invalid `format-version`"))?;
        if version != i64::from(FORMAT_VERSION) {
            return Err(serde::de::Error::custom(format!(
                "unsupported pack format version {version}"
            )));
        }
        let wire: Version1Manifest = value.try_into().map_err(serde::de::Error::custom)?;
        let manifest = Self::try_from(wire).map_err(serde::de::Error::custom)?;
        manifest.validate().map_err(serde::de::Error::custom)?;
        Ok(manifest)
    }
}

/// The `[project]` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProjectManifest {
    /// The root-relative path of the entrypoint file, e.g. `main.typ`.
    entrypoint: String,
    /// Non-source project resources supplied externally at compilation time.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    external_resources: BTreeSet<String>,
}

/// The `[packages]` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PackagesManifest {
    /// Exact specs of packages whose files are stored inside the pack.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_package_specs"
    )]
    vendored: Vec<PackageSpec>,
    /// Exact specs of observed dependencies that are *not* stored inside the
    /// pack and must be resolved from a package directory, cache, or registry
    /// when compiling.
    #[serde(
        default,
        rename = "external",
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_package_specs"
    )]
    unvendored: Vec<PackageSpec>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Version1PackagesManifest {
    #[serde(default)]
    vendored: Vec<String>,
    #[serde(default, rename = "external")]
    unvendored: Vec<String>,
}

impl TryFrom<Version1PackagesManifest> for PackagesManifest {
    type Error = PackManifestError;

    fn try_from(packages: Version1PackagesManifest) -> Result<Self, Self::Error> {
        Ok(Self {
            vendored: parse_specs(packages.vendored)?,
            unvendored: parse_specs(packages.unvendored)?,
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

fn serialize_package_specs<S>(specs: &[PackageSpec], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_seq(specs.iter().map(ToString::to_string))
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
}

fn is_zero(index: &u32) -> bool {
    *index == 0
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

    /// The declared External Project Resource paths in deterministic order.
    pub fn external_resources(&self) -> impl Iterator<Item = &str> {
        self.external_resources.iter().map(String::as_str)
    }

    pub(crate) fn contains_external_resource(&self, path: &str) -> bool {
        self.external_resources.contains(path)
    }
}

impl PackagesManifest {
    /// Exact specifications of packages stored inside the Pack.
    pub fn vendored(&self) -> &[PackageSpec] {
        &self.vendored
    }

    /// Exact specifications of packages resolved outside the Pack.
    pub fn unvendored(&self) -> &[PackageSpec] {
        &self.unvendored
    }

    fn is_empty(&self) -> bool {
        self.vendored.is_empty() && self.unvendored.is_empty()
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

    pub(crate) fn new(path: String, index: u32, families: Vec<String>) -> Self {
        Self {
            path,
            index,
            families,
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
    #[error("unsupported pack format version {0} (this reader supports version {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid package spec `{spec}`: {message}")]
    InvalidPackageSpec { spec: String, message: String },
}

impl PackManifest {
    pub(crate) fn new(
        entrypoint: String,
        external_resources: BTreeSet<String>,
        vendored_packages: Vec<PackageSpec>,
        external_packages: Vec<PackageSpec>,
        fonts: Vec<FontManifest>,
        metadata: Option<PackMetadata>,
    ) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            project: ProjectManifest {
                entrypoint,
                external_resources,
            },
            packages: PackagesManifest {
                vendored: canonical_specs(vendored_packages),
                unvendored: canonical_specs(external_packages),
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
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct VersionProbe {
            format_version: u32,
        }

        let probe: VersionProbe = toml::from_str(text)?;
        if probe.format_version != FORMAT_VERSION {
            return Err(PackManifestError::UnsupportedVersion(probe.format_version));
        }
        let wire: Version1Manifest = toml::from_str(text)?;
        let manifest = Self::try_from(wire)?;
        manifest.validate()?;
        Ok(manifest)
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

    /// The vendored package specs.
    pub fn vendored_packages(&self) -> &[PackageSpec] {
        &self.packages.vendored
    }

    /// The unvendored package specs.
    pub fn unvendored_packages(&self) -> &[PackageSpec] {
        &self.packages.unvendored
    }
}

fn parse_specs(specs: Vec<String>) -> Result<Vec<PackageSpec>, PackManifestError> {
    specs
        .into_iter()
        .map(|spec| {
            PackageSpec::from_str(&spec).map_err(|err| PackManifestError::InvalidPackageSpec {
                spec,
                message: err.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(canonical_specs)
}

fn canonical_specs(specs: Vec<PackageSpec>) -> Vec<PackageSpec> {
    specs
        .into_iter()
        .map(|spec| (spec.to_string(), spec))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}
