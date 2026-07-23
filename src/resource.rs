//! Runtime resolution of Resource Slots through Resource Providers.

use std::path::PathBuf;

#[cfg(feature = "fs")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "fs")]
use std::sync::Mutex;

use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, VirtualRoot};
use typst_kit::files::FileLoader;

use crate::Pack;

pub(crate) type Provider = Box<dyn FileLoader + Send + Sync>;

struct ProviderChain(Vec<Provider>);

impl ProviderChain {
    fn resolve(&self, id: FileId) -> FileResult<Bytes> {
        self.resolve_with_index(id).map(|(_, data)| data)
    }

    fn resolve_with_index(&self, id: FileId) -> FileResult<(usize, Bytes)> {
        for (index, provider) in self.0.iter().enumerate() {
            let _timing = typst_timing::TimingScope::new("Resource Provider");
            match provider.load(id) {
                Err(FileError::NotFound(_)) => {}
                Ok(data) => return Ok((index, data)),
                Err(error) => return Err(error),
            }
        }
        missing(id)
    }
}

pub(crate) struct CompilationResources {
    providers: ProviderChain,
}

impl CompilationResources {
    pub(crate) fn new(providers: Vec<Provider>) -> Self {
        Self {
            providers: ProviderChain(providers),
        }
    }

    pub(crate) fn file(&self, pack: &Pack, id: FileId) -> Option<FileResult<Bytes>> {
        if !matches!(id.root(), VirtualRoot::Project) {
            return None;
        }
        let path = id.vpath().get_without_slash();
        Some(match pack.file(path) {
            Some(data) => Ok(data.clone()),
            None if pack.is_resource_slot(path) => self.providers.resolve(id),
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
pub(crate) struct DiscoveryResources {
    providers: ProviderChain,
    explicit_slots: BTreeSet<String>,
    provenance: Mutex<BTreeSet<String>>,
    unavailable: Mutex<BTreeSet<String>>,
    selected_providers: Mutex<BTreeMap<String, usize>>,
}

#[cfg(feature = "fs")]
impl DiscoveryResources {
    pub(crate) fn new(providers: Vec<Provider>, explicit_slots: BTreeSet<String>) -> Self {
        Self {
            providers: ProviderChain(providers),
            provenance: Mutex::new(explicit_slots.clone()),
            explicit_slots,
            unavailable: Mutex::new(BTreeSet::new()),
            selected_providers: Mutex::new(BTreeMap::new()),
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
            Err(FileError::NotFound(_)) => match self.providers.resolve_with_index(id) {
                Ok((provider, data)) => {
                    self.selected_providers
                        .lock()
                        .expect("Resource Provider selection lock poisoned")
                        .insert(id.vpath().get_without_slash().to_owned(), provider);
                    self.provenance
                        .lock()
                        .expect("Resource Slot provenance lock poisoned")
                        .insert(id.vpath().get_without_slash().to_owned());
                    Ok(data)
                }
                Err(FileError::NotFound(_)) => {
                    self.record_unavailable_explicit(id);
                    missing(id)
                }
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        }
    }

    pub(crate) fn source<T>(
        &self,
        id: FileId,
        authoritative: impl FnOnce() -> FileResult<T>,
    ) -> FileResult<T> {
        if matches!(id.root(), VirtualRoot::Project)
            && self.explicit_slots.contains(id.vpath().get_without_slash())
        {
            return missing(id);
        }
        authoritative()
    }

    pub(crate) fn is_resource_slot(&self, id: FileId) -> bool {
        self.provenance
            .lock()
            .expect("Resource Slot provenance lock poisoned")
            .contains(id.vpath().get_without_slash())
    }

    pub(crate) fn resource_slots(&self) -> Vec<String> {
        self.provenance
            .lock()
            .expect("Resource Slot provenance lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    pub(crate) fn unavailable_resource_slots(&self) -> Vec<String> {
        self.unavailable
            .lock()
            .expect("Resource Slot availability lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    pub(crate) fn selected_provider(&self, path: &str) -> Option<usize> {
        self.selected_providers
            .lock()
            .expect("Resource Provider selection lock poisoned")
            .get(path)
            .copied()
    }

    pub(crate) fn resolve_provider(&self, id: FileId) -> FileResult<(usize, Bytes)> {
        self.providers.resolve_with_index(id)
    }

    fn record_unavailable_explicit(&self, id: FileId) {
        let path = id.vpath().get_without_slash();
        if self.explicit_slots.contains(path) {
            self.unavailable
                .lock()
                .expect("Resource Slot availability lock poisoned")
                .insert(path.to_owned());
        }
    }
}

fn missing<T>(id: FileId) -> FileResult<T> {
    Err(FileError::NotFound(PathBuf::from(
        id.vpath().get_without_slash(),
    )))
}
