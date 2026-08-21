//! Validated external compilation fulfillment inputs.

use std::collections::{BTreeMap, BTreeSet};

use typst::syntax::package::PackageSpec;

use crate::{CanonicalIdentity, FontContainer, Pack, PackageTree};

#[derive(Debug, Clone)]
pub struct PackageTreeFulfillment {
    pub(super) spec: PackageSpec,
    pub(super) tree: PackageTree,
    pub(super) provenance: Option<String>,
    pub(super) cache_hit: bool,
}

impl PackageTreeFulfillment {
    pub fn new(spec: PackageSpec, tree: PackageTree) -> Self {
        Self {
            spec,
            tree,
            provenance: None,
            cache_hit: false,
        }
    }

    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub fn tree(&self) -> &PackageTree {
        &self.tree
    }

    pub fn provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub fn cache_hit(mut self, cache_hit: bool) -> Self {
        self.cache_hit = cache_hit;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FontContainerFulfillment {
    pub(super) expected_identity: CanonicalIdentity,
    pub(super) container: FontContainer,
    pub(super) provenance: Option<String>,
    pub(super) licensing: Option<String>,
}

impl FontContainerFulfillment {
    pub fn new(expected_identity: CanonicalIdentity, container: FontContainer) -> Self {
        Self {
            expected_identity,
            container,
            provenance: None,
            licensing: None,
        }
    }

    pub fn expected_identity(&self) -> CanonicalIdentity {
        self.expected_identity
    }

    pub fn container(&self) -> &FontContainer {
        &self.container
    }

    pub fn provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub fn licensing(mut self, licensing: impl Into<String>) -> Self {
        self.licensing = Some(licensing.into());
        self
    }
}

/// Resolves the Pack's external Font Requirements from exact source-container bytes.
///
/// Each matching container produces at most one fulfillment. Unrelated and
/// duplicate sources are ignored, and requirements with no matching source
/// remain absent for compilation's exact fulfillment verification to report.
pub fn resolve_external_font_requirements<I, S>(
    pack: &Pack,
    sources: I,
) -> Result<Vec<FontContainerFulfillment>, crate::FontContainerError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<[u8]>,
{
    let required = pack
        .font_requirements()
        .iter()
        .filter(|requirement| !requirement.is_embedded())
        .map(|requirement| requirement.container_identity())
        .collect::<BTreeSet<_>>();
    let mut fulfillments = BTreeMap::new();
    for source in sources {
        let data = source.as_ref();
        let identity = CanonicalIdentity::for_font_container_bytes(data);
        if required.contains(&identity) && !fulfillments.contains_key(&identity) {
            let container = FontContainer::new(data.to_vec())?;
            fulfillments.insert(identity, FontContainerFulfillment::new(identity, container));
        }
    }
    Ok(fulfillments.into_values().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationFulfillmentSetIssue {
    #[error("package specification {spec} is fulfilled more than once")]
    DuplicatePackageSpecification { spec: PackageSpec },
    #[error("Font Container Identity {identity:?} is fulfilled more than once")]
    DuplicateFontContainerIdentity { identity: CanonicalIdentity },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Compilation Fulfillment Set construction failed with {} issue(s)", .issues.len())]
pub struct CompilationFulfillmentSetError {
    issues: Vec<CompilationFulfillmentSetIssue>,
}

impl CompilationFulfillmentSetError {
    pub fn issues(&self) -> &[CompilationFulfillmentSetIssue] {
        &self.issues
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompilationFulfillmentSet {
    pub(super) packages: BTreeMap<String, PackageTreeFulfillment>,
    pub(super) fonts: BTreeMap<CanonicalIdentity, FontContainerFulfillment>,
}

impl CompilationFulfillmentSet {
    pub fn new(
        packages: impl IntoIterator<Item = PackageTreeFulfillment>,
        fonts: impl IntoIterator<Item = FontContainerFulfillment>,
    ) -> Result<Self, CompilationFulfillmentSetError> {
        let packages = packages.into_iter().collect::<Vec<_>>();
        let fonts = fonts.into_iter().collect::<Vec<_>>();
        let mut package_keys = BTreeSet::new();
        let mut duplicate_packages = BTreeSet::new();
        for fulfillment in &packages {
            let key = fulfillment.spec.to_string();
            if !package_keys.insert(key.clone()) {
                duplicate_packages.insert(key);
            }
        }
        let mut font_keys = BTreeSet::new();
        let mut duplicate_fonts = BTreeSet::new();
        for fulfillment in &fonts {
            if !font_keys.insert(fulfillment.expected_identity) {
                duplicate_fonts.insert(fulfillment.expected_identity);
            }
        }
        let mut issues = duplicate_packages
            .into_iter()
            .map(|key| {
                let spec = packages
                    .iter()
                    .find(|fulfillment| fulfillment.spec.to_string() == key)
                    .expect("duplicate key came from a package fulfillment")
                    .spec
                    .clone();
                CompilationFulfillmentSetIssue::DuplicatePackageSpecification { spec }
            })
            .collect::<Vec<_>>();
        issues.extend(duplicate_fonts.into_iter().map(|identity| {
            CompilationFulfillmentSetIssue::DuplicateFontContainerIdentity { identity }
        }));
        if !issues.is_empty() {
            return Err(CompilationFulfillmentSetError { issues });
        }
        Ok(Self {
            packages: packages
                .into_iter()
                .map(|fulfillment| (fulfillment.spec.to_string(), fulfillment))
                .collect(),
            fonts: fonts
                .into_iter()
                .map(|fulfillment| (fulfillment.expected_identity, fulfillment))
                .collect(),
        })
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn packages(&self) -> impl ExactSizeIterator<Item = &PackageTreeFulfillment> {
        self.packages.values()
    }

    pub fn fonts(&self) -> impl ExactSizeIterator<Item = &FontContainerFulfillment> {
        self.fonts.values()
    }
}

/// Operational evidence retained for one exact package fulfillment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFulfillmentReport {
    pub(super) spec: PackageSpec,
    pub(super) required_tree_identity: Option<CanonicalIdentity>,
    pub(super) supplied_tree_identity: Option<CanonicalIdentity>,
    pub(super) declared: bool,
    pub(super) embedded: bool,
    pub(super) provenance: Option<String>,
    pub(super) cache_hit: bool,
}

impl PackageFulfillmentReport {
    pub fn spec(&self) -> &PackageSpec {
        &self.spec
    }

    pub fn required_tree_identity(&self) -> Option<CanonicalIdentity> {
        self.required_tree_identity
    }

    pub fn supplied_tree_identity(&self) -> Option<CanonicalIdentity> {
        self.supplied_tree_identity
    }

    pub fn declared(&self) -> bool {
        self.declared
    }

    pub fn embedded(&self) -> bool {
        self.embedded
    }

    pub fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub fn cache_hit(&self) -> bool {
        self.cache_hit
    }
}

/// Operational evidence retained for one exact font fulfillment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFulfillmentReport {
    pub(super) container_identity: CanonicalIdentity,
    pub(super) supplied_container_identity: Option<CanonicalIdentity>,
    pub(super) declared: bool,
    pub(super) embedded: bool,
    pub(super) provenance: Option<String>,
    pub(super) licensing: Option<String>,
}

impl FontFulfillmentReport {
    pub fn container_identity(&self) -> CanonicalIdentity {
        self.container_identity
    }

    pub fn supplied_container_identity(&self) -> Option<CanonicalIdentity> {
        self.supplied_container_identity
    }

    pub fn declared(&self) -> bool {
        self.declared
    }

    pub fn embedded(&self) -> bool {
        self.embedded
    }

    pub fn provenance(&self) -> Option<&str> {
        self.provenance.as_deref()
    }

    pub fn licensing(&self) -> Option<&str> {
        self.licensing.as_deref()
    }
}

/// Operational dependency evidence surrounding one official semantic result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompilationFulfillmentReport {
    pub(super) packages: Vec<PackageFulfillmentReport>,
    pub(super) fonts: Vec<FontFulfillmentReport>,
}

impl CompilationFulfillmentReport {
    pub fn packages(&self) -> &[PackageFulfillmentReport] {
        &self.packages
    }

    pub fn fonts(&self) -> &[FontFulfillmentReport] {
        &self.fonts
    }
}

/// One exact-set deviation detected before private World materialization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompilationFulfillmentIssue {
    #[error("external package fulfillment for {spec} is missing")]
    MissingExternalPackage { spec: PackageSpec },
    #[error("package fulfillment for undeclared specification {spec} supplied {actual:?}")]
    UndeclaredPackage {
        spec: PackageSpec,
        actual: CanonicalIdentity,
    },
    #[error("embedded package {spec} was unexpectedly fulfilled externally")]
    UnexpectedEmbeddedPackage { spec: PackageSpec },
    #[error("package fulfillment for {spec} supplied {actual:?}, expected {expected:?}")]
    MismatchedPackageTree {
        spec: PackageSpec,
        expected: CanonicalIdentity,
        actual: CanonicalIdentity,
        expected_file_count: u64,
        actual_file_count: u64,
        expected_byte_length: u64,
        actual_byte_length: u64,
    },
    #[error("external Font Container fulfillment for {identity:?} is missing")]
    MissingExternalFont { identity: CanonicalIdentity },
    #[error("Font Container fulfillment for undeclared identity {identity:?} supplied {actual:?}")]
    UndeclaredFont {
        identity: CanonicalIdentity,
        actual: CanonicalIdentity,
    },
    #[error("embedded Font Container {identity:?} was unexpectedly fulfilled externally")]
    UnexpectedEmbeddedFont { identity: CanonicalIdentity },
    #[error("Font Container fulfillment for {expected:?} supplied {actual:?}")]
    MismatchedFontContainer {
        expected: CanonicalIdentity,
        actual: CanonicalIdentity,
        expected_length: u64,
        actual_length: u64,
    },
    #[error("Font Container {identity:?} has no required face at index {index}")]
    MissingFontFace {
        identity: CanonicalIdentity,
        index: u32,
    },
}

/// Complete canonical evidence that a fulfillment set is not exact.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Compilation Fulfillment Set has {} deviation(s)", .issues.len())]
pub struct InvalidCompilationFulfillmentSet {
    pub(super) issues: Vec<CompilationFulfillmentIssue>,
}

impl InvalidCompilationFulfillmentSet {
    pub fn issues(&self) -> &[CompilationFulfillmentIssue] {
        &self.issues
    }
}

pub(super) fn verify_compilation_fulfillment_set(
    pack: &Pack,
    package_fulfillments: &BTreeMap<String, PackageTreeFulfillment>,
    font_fulfillments: &BTreeMap<CanonicalIdentity, FontContainerFulfillment>,
) -> Vec<CompilationFulfillmentIssue> {
    let package_requirements = pack
        .package_requirements()
        .iter()
        .map(|requirement| (requirement.spec().to_string(), requirement))
        .collect::<BTreeMap<_, _>>();
    let package_keys = package_requirements
        .keys()
        .chain(package_fulfillments.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    for key in package_keys {
        match (
            package_requirements.get(&key),
            package_fulfillments.get(&key),
        ) {
            (Some(requirement), None) if !requirement.is_embedded() => {
                issues.push(CompilationFulfillmentIssue::MissingExternalPackage {
                    spec: requirement.spec().clone(),
                });
            }
            (None, Some(fulfillment)) => {
                issues.push(CompilationFulfillmentIssue::UndeclaredPackage {
                    spec: fulfillment.spec.clone(),
                    actual: fulfillment.tree.identity(),
                });
            }
            (Some(requirement), Some(fulfillment)) => {
                if requirement.is_embedded() {
                    issues.push(CompilationFulfillmentIssue::UnexpectedEmbeddedPackage {
                        spec: requirement.spec().clone(),
                    });
                }
                if fulfillment.tree.identity() != requirement.tree_identity()
                    || fulfillment.tree.file_count() != requirement.file_count()
                    || fulfillment.tree.byte_length() != requirement.byte_length()
                {
                    issues.push(CompilationFulfillmentIssue::MismatchedPackageTree {
                        spec: requirement.spec().clone(),
                        expected: requirement.tree_identity(),
                        actual: fulfillment.tree.identity(),
                        expected_file_count: requirement.file_count(),
                        actual_file_count: fulfillment.tree.file_count(),
                        expected_byte_length: requirement.byte_length(),
                        actual_byte_length: fulfillment.tree.byte_length(),
                    });
                }
            }
            _ => {}
        }
    }

    let font_requirements = pack
        .font_requirements()
        .iter()
        .map(|requirement| (requirement.container_identity(), requirement))
        .collect::<BTreeMap<_, _>>();
    let font_keys = font_requirements
        .keys()
        .chain(font_fulfillments.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for identity in font_keys {
        match (
            font_requirements.get(&identity),
            font_fulfillments.get(&identity),
        ) {
            (Some(requirement), None) if !requirement.is_embedded() => {
                issues.push(CompilationFulfillmentIssue::MissingExternalFont { identity });
            }
            (None, Some(fulfillment)) => {
                issues.push(CompilationFulfillmentIssue::UndeclaredFont {
                    identity,
                    actual: fulfillment.container.identity(),
                });
            }
            (Some(requirement), Some(fulfillment)) => {
                if requirement.is_embedded() {
                    issues.push(CompilationFulfillmentIssue::UnexpectedEmbeddedFont { identity });
                }
                let actual = fulfillment.container.identity();
                let actual_length = fulfillment.container.data().len() as u64;
                if actual != identity || actual_length != requirement.container_length() {
                    issues.push(CompilationFulfillmentIssue::MismatchedFontContainer {
                        expected: identity,
                        actual,
                        expected_length: requirement.container_length(),
                        actual_length,
                    });
                }
                for index in requirement
                    .face_indices()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                {
                    if !fulfillment
                        .container
                        .faces()
                        .iter()
                        .any(|face| face.identity().index() == index)
                    {
                        issues
                            .push(CompilationFulfillmentIssue::MissingFontFace { identity, index });
                    }
                }
            }
            _ => {}
        }
    }
    issues
}
