//! The package half of Creation Preparation for the reference Creation
//! Adapter, the filesystem one.
//!
//! Acquisition is resume-driven: the core reports the exact specifications its
//! representative request read and was not given, and the adapter obtains each
//! of them through the configured Package Authority — local package
//! directories, then the package cache, then a download unless creation is
//! offline or the build has no egress compiled in to download with.

use std::path::PathBuf;
use std::sync::Mutex;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst_kit::packages::SystemPackages;

use crate::creation::{PackageDisposition, ResolvedPackageTree};
use crate::packer::PackerError;
use crate::world::read_complete_package_tree;

/// The Complete Package Trees the adapter acquired for one creation, and the
/// Package Authority it acquires them from.
///
/// It is the adapter's own record of what it supplied: creation resumes over
/// it, the Creation Evidence Fence revalidates it against the filesystem, and
/// creation diagnostics render package sources from it rather than reading a
/// tree a second time.
pub(crate) struct AcquiredPackages {
    authority: SystemPackages,
    trees: Mutex<Vec<AcquiredPackageTree>>,
}

impl AcquiredPackages {
    /// Acquires from the given Package Authority, having acquired nothing yet.
    pub(crate) fn new(authority: SystemPackages) -> Self {
        Self {
            authority,
            trees: Mutex::new(Vec::new()),
        }
    }

    /// Obtains the Complete Package Tree for one reported specification and
    /// records it as creation evidence.
    ///
    /// The whole tree is read, not only the files the representative request
    /// went on to ask for, because the Pack contains the complete tree.
    pub(crate) fn acquire(
        &self,
        spec: &PackageSpec,
        disposition: PackageDisposition,
    ) -> Result<ResolvedPackageTree, PackerError> {
        let package = |message: String| PackerError::Package {
            spec: spec.clone(),
            message,
        };
        let root = self
            .authority
            .obtain(spec)
            .map_err(|error| package(error.to_string()))?;
        let files = read_complete_package_tree(root.path()).map_err(package)?;

        self.trees
            .lock()
            .expect("acquired package lock poisoned")
            .push(AcquiredPackageTree {
                spec: spec.clone(),
                root: root.path().to_owned(),
                files: files.clone(),
            });
        Ok(ResolvedPackageTree::from_entries(
            spec.clone(),
            files,
            disposition,
        ))
    }

    /// The exact bytes acquired for one package file, which is all creation
    /// diagnostics and timing spans may still resolve a package source from.
    pub(crate) fn file(&self, spec: &PackageSpec, path: &str) -> Option<Bytes> {
        self.trees
            .lock()
            .expect("acquired package lock poisoned")
            .iter()
            .find(|tree| &tree.spec == spec)?
            .files
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, data)| data.clone())
    }

    /// Fails when the trees backing the acquired packages no longer agree with
    /// the filesystem, which is the package half of the Creation Evidence
    /// Fence.
    pub(crate) fn revalidate(&self) -> Result<(), PackerError> {
        for tree in self
            .trees
            .lock()
            .expect("acquired package lock poisoned")
            .iter()
        {
            if read_complete_package_tree(&tree.root).as_ref().ok() != Some(&tree.files) {
                return Err(PackerError::CreationEvidenceChanged {
                    path: tree.spec.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// One Complete Package Tree the adapter read, and the root it read it from.
struct AcquiredPackageTree {
    spec: PackageSpec,
    root: PathBuf,
    files: Vec<(String, Bytes)>,
}
