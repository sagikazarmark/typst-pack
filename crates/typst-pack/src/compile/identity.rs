//! Embedded implementation identities and diagnostic attribution.

use std::hash::{Hash, Hasher};

/// The role of an embedded implementation in compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplementationRole {
    Engine,
    Exporter,
}

/// The exact embedded implementation that participated in a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementationIdentity {
    pub(super) role: ImplementationRole,
    pub(super) implementation: &'static str,
    pub(super) version: &'static str,
    pub(super) source_checksum: &'static str,
    pub(super) target: &'static str,
    pub(super) target_features: &'static str,
    pub(super) feature_set: &'static str,
    pub(super) debug_assertions: bool,
}

impl Hash for ImplementationIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The role is represented by the engine/exporter position in schema v1.
        self.implementation.hash(state);
        self.version.hash(state);
        self.source_checksum.hash(state);
        self.target.hash(state);
        self.target_features.hash(state);
        let feature_set = if self.feature_set == env!("TYPST_PACK_FEATURE_SET") {
            env!("TYPST_PACK_IDENTITY_FEATURE_SET")
        } else {
            self.feature_set
        };
        feature_set.hash(state);
        self.debug_assertions.hash(state);
    }
}

impl ImplementationIdentity {
    pub(crate) const fn new(
        role: ImplementationRole,
        implementation: &'static str,
        version: &'static str,
        source_checksum: &'static str,
    ) -> Self {
        Self {
            role,
            implementation,
            version,
            source_checksum,
            target: env!("TYPST_PACK_TARGET"),
            target_features: env!("TYPST_PACK_CARGO_CFG_TARGET_FEATURE"),
            feature_set: env!("TYPST_PACK_FEATURE_SET"),
            debug_assertions: cfg!(debug_assertions),
        }
    }

    /// Whether this implementation is the engine or an exporter.
    pub fn role(self) -> ImplementationRole {
        self.role
    }

    pub fn implementation(self) -> &'static str {
        self.implementation
    }

    pub fn version(self) -> &'static str {
        self.version
    }

    pub fn source_checksum(self) -> &'static str {
        self.source_checksum
    }

    pub fn target(self) -> &'static str {
        self.target
    }

    pub fn target_features(self) -> &'static str {
        self.target_features
    }

    pub fn feature_set(self) -> &'static str {
        self.feature_set
    }

    pub fn debug_assertions(self) -> bool {
        self.debug_assertions
    }
}

/// The exact embedded implementation that emitted a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticProducer(DiagnosticProducerRole);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DiagnosticProducerRole {
    Engine(ImplementationIdentity),
    Exporter(ImplementationIdentity),
}

impl DiagnosticProducer {
    pub const fn new(identity: ImplementationIdentity) -> Self {
        match identity.role {
            ImplementationRole::Engine => Self(DiagnosticProducerRole::Engine(identity)),
            ImplementationRole::Exporter => Self(DiagnosticProducerRole::Exporter(identity)),
        }
    }

    pub fn implementation_identity(self) -> ImplementationIdentity {
        match self.0 {
            DiagnosticProducerRole::Engine(identity)
            | DiagnosticProducerRole::Exporter(identity) => identity,
        }
    }
}
