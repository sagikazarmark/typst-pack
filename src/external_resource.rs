//! Runtime interpretation of External Resource References.

use std::path::PathBuf;

#[cfg(feature = "fs")]
use std::collections::BTreeSet;
#[cfg(feature = "fs")]
use std::sync::Mutex;

use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, VirtualRoot};
use typst_kit::files::FileLoader;

use crate::Pack;

pub(crate) type Reference = Box<dyn FileLoader + Send + Sync>;

struct ReferenceChain(Vec<Reference>);

impl ReferenceChain {
    fn resolve(&self, id: FileId) -> FileResult<Bytes> {
        for reference in &self.0 {
            match reference.load(id) {
                Err(FileError::NotFound(_)) => {}
                result => return result,
            }
        }
        missing(id)
    }
}

pub(crate) struct Compilation {
    references: ReferenceChain,
}

impl Compilation {
    pub(crate) fn new(references: Vec<Reference>) -> Self {
        Self {
            references: ReferenceChain(references),
        }
    }

    pub(crate) fn file(&self, pack: &Pack, id: FileId) -> Option<FileResult<Bytes>> {
        if !matches!(id.root(), VirtualRoot::Project) {
            return None;
        }
        let path = id.vpath().get_without_slash();
        Some(match pack.file(path) {
            Some(data) => Ok(data.clone()),
            None if pack.is_external_resource(path) => self.references.resolve(id),
            None => missing(id),
        })
    }

    pub(crate) fn source<T>(
        &self,
        pack: &Pack,
        id: FileId,
        authoritative: impl FnOnce() -> FileResult<T>,
    ) -> FileResult<T> {
        if matches!(id.root(), VirtualRoot::Project)
            && pack.file(id.vpath().get_without_slash()).is_none()
        {
            return missing(id);
        }
        authoritative()
    }
}

#[cfg(feature = "fs")]
pub(crate) struct Discovery {
    references: ReferenceChain,
    allow_fallback: bool,
    explicit: BTreeSet<String>,
    provenance: Mutex<BTreeSet<String>>,
}

#[cfg(feature = "fs")]
impl Discovery {
    pub(crate) fn new(
        references: Vec<Reference>,
        allow_fallback: bool,
        explicit: BTreeSet<String>,
    ) -> Self {
        Self {
            references: ReferenceChain(references),
            allow_fallback,
            provenance: Mutex::new(explicit.clone()),
            explicit,
        }
    }

    pub(crate) fn file(
        &self,
        id: FileId,
        authoritative: impl FnOnce() -> FileResult<Bytes>,
    ) -> FileResult<Bytes> {
        let result = authoritative();
        if !matches!(id.root(), VirtualRoot::Project) {
            return result;
        }
        match result {
            Ok(data) => Ok(data),
            Err(FileError::NotFound(_)) if self.allow_fallback => {
                let data = self.references.resolve(id)?;
                self.provenance
                    .lock()
                    .expect("External Project Resource provenance lock poisoned")
                    .insert(id.vpath().get_without_slash().to_owned());
                Ok(data)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn source<T>(
        &self,
        id: FileId,
        authoritative: impl FnOnce() -> FileResult<T>,
    ) -> FileResult<T> {
        if matches!(id.root(), VirtualRoot::Project)
            && self.explicit.contains(id.vpath().get_without_slash())
        {
            return missing(id);
        }
        authoritative()
    }

    pub(crate) fn is_external(&self, id: FileId) -> bool {
        self.provenance
            .lock()
            .expect("External Project Resource provenance lock poisoned")
            .contains(id.vpath().get_without_slash())
    }

    pub(crate) fn external_resources(&self) -> Vec<String> {
        self.provenance
            .lock()
            .expect("External Project Resource provenance lock poisoned")
            .iter()
            .cloned()
            .collect()
    }
}

fn missing<T>(id: FileId) -> FileResult<T> {
    Err(FileError::NotFound(PathBuf::from(
        id.vpath().get_without_slash(),
    )))
}
