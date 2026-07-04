//! The pack manifest stored as `typst-pack.toml` inside the archive.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typst::syntax::VirtualPath;
use typst::syntax::package::PackageSpec;

/// The archive entry name of the manifest.
pub const MANIFEST_PATH: &str = "typst-pack.toml";

/// The pack format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The parsed contents of `typst-pack.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Manifest {
    /// The pack format version. Readers must reject versions they don't know.
    pub format_version: u32,
    /// The packed Typst project.
    pub project: ProjectManifest,
    /// Package dependencies observed while creating the pack.
    #[serde(default, skip_serializing_if = "PackagesManifest::is_empty")]
    pub packages: PackagesManifest,
    /// Fonts embedded in the pack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontManifest>,
    /// Optional descriptive metadata about the packed project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

/// The `[project]` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProjectManifest {
    /// The root-relative path of the entrypoint file, e.g. `main.typ`.
    pub entrypoint: String,
}

/// The `[packages]` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PackagesManifest {
    /// Exact specs of packages whose files are stored inside the pack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendored: Vec<String>,
    /// Exact specs of observed dependencies that are *not* stored inside the
    /// pack and must be resolved from a package directory, cache, or registry
    /// when compiling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external: Vec<String>,
}

impl PackagesManifest {
    fn is_empty(&self) -> bool {
        self.vendored.is_empty() && self.external.is_empty()
    }
}

/// One `[[fonts]]` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FontManifest {
    /// The archive entry holding the font data, e.g. `fonts/dejavu-sans.ttf`.
    pub path: String,
    /// The face index inside the font file (non-zero for collections).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub index: u32,
    /// Family names provided by this face, informational only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub families: Vec<String>,
}

fn is_zero(index: &u32) -> bool {
    *index == 0
}

/// The optional `[metadata]` section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

/// A manifest that could not be accepted.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("unsupported pack format version {0} (this reader supports up to {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid entrypoint path `{path}`: {message}")]
    InvalidEntrypoint { path: String, message: String },
    #[error("invalid package spec `{spec}`: {message}")]
    InvalidPackageSpec { spec: String, message: String },
    #[error("invalid font path `{path}`: {message}")]
    InvalidFontPath { path: String, message: String },
}

impl Manifest {
    /// Parses and validates a manifest from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, ManifestError> {
        let manifest: Manifest = toml::from_str(text)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Serializes the manifest to TOML text.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("manifest is always serializable")
    }

    /// Checks internal consistency of the manifest.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.format_version > FORMAT_VERSION {
            return Err(ManifestError::UnsupportedVersion(self.format_version));
        }
        self.entrypoint()?;
        for spec in self.packages.vendored.iter().chain(&self.packages.external) {
            parse_spec(spec)?;
        }
        for font in &self.fonts {
            if let Err(err) = VirtualPath::new(&font.path) {
                return Err(ManifestError::InvalidFontPath {
                    path: font.path.clone(),
                    message: err.to_string(),
                });
            }
        }
        Ok(())
    }

    /// The entrypoint as a validated virtual path.
    pub fn entrypoint(&self) -> Result<VirtualPath, ManifestError> {
        VirtualPath::new(&self.project.entrypoint).map_err(|err| ManifestError::InvalidEntrypoint {
            path: self.project.entrypoint.clone(),
            message: err.to_string(),
        })
    }

    /// The vendored package specs, parsed.
    pub fn vendored_packages(&self) -> Result<Vec<PackageSpec>, ManifestError> {
        self.packages
            .vendored
            .iter()
            .map(|spec| parse_spec(spec))
            .collect()
    }

    /// The external (non-vendored) package specs, parsed.
    pub fn external_packages(&self) -> Result<Vec<PackageSpec>, ManifestError> {
        self.packages
            .external
            .iter()
            .map(|spec| parse_spec(spec))
            .collect()
    }
}

fn parse_spec(spec: &str) -> Result<PackageSpec, ManifestError> {
    PackageSpec::from_str(spec).map_err(|err| ManifestError::InvalidPackageSpec {
        spec: spec.to_owned(),
        message: err.to_string(),
    })
}
