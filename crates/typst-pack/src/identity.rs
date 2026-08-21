//! Shared canonical identity values.

use std::hash::{Hash, Hasher};

/// The semantic role of a [`CanonicalIdentity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalIdentityRole {
    Pack,
    PackageTree,
    FontContainer,
    Compilation,
    CompilationResult,
}

impl CanonicalIdentityRole {
    /// The stable role string used by serialized identity declarations.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pack => "pack",
            Self::PackageTree => "complete-package-tree",
            Self::FontContainer => "font-container",
            Self::Compilation => "compilation",
            Self::CompilationResult => "compilation-result",
        }
    }

    pub(crate) const fn schema(self) -> &'static str {
        match self {
            Self::Pack => "typst-pack-identity-v1",
            Self::PackageTree => "typst-pack-complete-package-tree-v1",
            Self::FontContainer => "typst-pack-font-container-identity-v1",
            Self::Compilation => "typst-pack-compilation-v1",
            Self::CompilationResult => "typst-pack-compilation-result-v1",
        }
    }
}

/// A role-separated canonical semantic identity.
///
/// Equality includes the role, schema, algorithm, and digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalIdentity {
    role: CanonicalIdentityRole,
    digest: u128,
}

impl Hash for CanonicalIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Schema-v1 projections hashed the digest-only identity newtypes.
        self.digest.hash(state);
    }
}

impl CanonicalIdentity {
    pub(crate) const fn from_digest(role: CanonicalIdentityRole, digest: u128) -> Self {
        Self { role, digest }
    }

    /// Derives the Font Container identity of exact container bytes.
    pub fn for_font_container_bytes(data: &[u8]) -> Self {
        Self::from_digest(
            CanonicalIdentityRole::FontContainer,
            typst::utils::hash128(&data),
        )
    }

    /// The semantic role separated by this identity.
    pub const fn role(self) -> CanonicalIdentityRole {
        self.role
    }

    /// The identity schema used by this release.
    pub const fn schema(self) -> &'static str {
        self.role.schema()
    }

    /// The deterministic digest algorithm used by the schema.
    pub const fn algorithm(self) -> &'static str {
        "typst-hash128-0.15"
    }

    /// The deterministic 128-bit digest in big-endian order.
    pub const fn digest(self) -> [u8; 16] {
        self.digest.to_be_bytes()
    }

    pub(crate) const fn digest_value(self) -> u128 {
        self.digest
    }

    pub(crate) fn encode(self) -> String {
        format!("{:032x}", self.digest)
    }

    pub(crate) fn decode(role: CanonicalIdentityRole, value: &str) -> Option<Self> {
        (value.len() == 32)
            .then(|| u128::from_str_radix(value, 16).ok())
            .flatten()
            .map(|digest| Self::from_digest(role, digest))
    }
}
