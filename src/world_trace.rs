use std::collections::BTreeSet;
use std::sync::Mutex;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapturedAccessKind {
    Source,
    File,
    Font,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapturedAccessOutcome {
    Read {
        byte_length: usize,
        digest: [u8; 16],
    },
    Missing,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CapturedObservation {
    pub(crate) kind: CapturedAccessKind,
    pub(crate) logical_path: String,
    pub(crate) font_index: Option<usize>,
    pub(crate) outcome: CapturedAccessOutcome,
}

pub(crate) struct WorldTrace<'a, W: ?Sized> {
    world: &'a W,
    observations: Mutex<BTreeSet<CapturedObservation>>,
}

impl<'a, W: World + ?Sized> WorldTrace<'a, W> {
    pub(crate) fn new(world: &'a W) -> Self {
        Self {
            world,
            observations: Mutex::new(BTreeSet::new()),
        }
    }

    pub(crate) fn snapshot(&self) -> BTreeSet<CapturedObservation> {
        self.observations
            .lock()
            .expect("world trace lock poisoned")
            .clone()
    }

    fn record_file<T>(
        &self,
        id: FileId,
        kind: CapturedAccessKind,
        result: &FileResult<T>,
        bytes: impl FnOnce(&T) -> &[u8],
    ) {
        let outcome = match result {
            Ok(value) => read_outcome(bytes(value)),
            Err(FileError::NotFound(_)) => CapturedAccessOutcome::Missing,
            Err(_) => CapturedAccessOutcome::Failed,
        };
        self.record(CapturedObservation {
            kind,
            logical_path: logical_path(id),
            font_index: None,
            outcome,
        });
    }

    fn record(&self, observation: CapturedObservation) {
        self.observations
            .lock()
            .expect("world trace lock poisoned")
            .insert(observation);
    }
}

impl<W: World + ?Sized> World for WorldTrace<'_, W> {
    fn library(&self) -> &LazyHash<Library> {
        self.world.library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.world.book()
    }

    fn main(&self) -> FileId {
        self.world.main()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let result = self.world.source(id);
        self.record_file(id, CapturedAccessKind::Source, &result, |source| {
            source.text().as_bytes()
        });
        result
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let result = self.world.file(id);
        self.record_file(id, CapturedAccessKind::File, &result, Bytes::as_slice);
        result
    }

    fn font(&self, requested_index: usize) -> Option<Font> {
        let font = self.world.font(requested_index);
        let observation = match &font {
            Some(font) => CapturedObservation {
                kind: CapturedAccessKind::Font,
                logical_path: format!(
                    "font:{:032x}",
                    u128::from_be_bytes(
                        crate::FontContainerIdentity::from_bytes(font.data().as_slice()).digest()
                    )
                ),
                font_index: Some(font.index() as usize),
                outcome: read_outcome(font.data().as_slice()),
            },
            None => CapturedObservation {
                kind: CapturedAccessKind::Font,
                logical_path: format!("font-index:{requested_index}"),
                font_index: Some(requested_index),
                outcome: CapturedAccessOutcome::Missing,
            },
        };
        self.record(observation);
        font
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.world.today(offset)
    }
}

fn read_outcome(data: &[u8]) -> CapturedAccessOutcome {
    CapturedAccessOutcome::Read {
        byte_length: data.len(),
        digest: typst::utils::hash128(&data).to_be_bytes(),
    }
}

pub(crate) fn logical_path(id: FileId) -> String {
    let path = id.vpath().get_without_slash();
    match id.root() {
        VirtualRoot::Project => format!("project:{path}"),
        VirtualRoot::Package(spec) => format!("package:{spec}/{path}"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use typst::syntax::{RootedPath, VirtualPath};

    use super::*;
    use crate::{Pack, world::PackWorld};

    #[test]
    fn captures_exact_file_outcomes_and_missing_font_requests() {
        let pack = Pack::builder("main.typ")
            .file("main.typ", b"Hello".to_vec())
            .unwrap()
            .file("bad.typ", vec![0xff])
            .unwrap()
            .build()
            .unwrap();
        let world = PackWorld::builder(pack).build().unwrap();
        let trace = WorldTrace::new(&world);
        let bad =
            RootedPath::new(VirtualRoot::Project, VirtualPath::new("bad.typ").unwrap()).intern();
        let missing = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("missing.bin").unwrap(),
        )
        .intern();

        trace.source(world.main()).unwrap();
        trace.source(world.main()).unwrap();
        assert!(matches!(trace.source(bad), Err(FileError::InvalidUtf8)));
        assert_eq!(
            trace.file(missing),
            Err(FileError::NotFound(PathBuf::from("missing.bin")))
        );
        assert!(trace.font(41).is_none());

        let observations = trace.snapshot();
        assert_eq!(observations.len(), 4);
        assert!(observations.contains(&CapturedObservation {
            kind: CapturedAccessKind::Source,
            logical_path: "project:main.typ".to_owned(),
            font_index: None,
            outcome: CapturedAccessOutcome::Read {
                byte_length: 5,
                digest: typst::utils::hash128(&b"Hello".as_slice()).to_be_bytes(),
            },
        }));
        assert!(observations.contains(&CapturedObservation {
            kind: CapturedAccessKind::Source,
            logical_path: "project:bad.typ".to_owned(),
            font_index: None,
            outcome: CapturedAccessOutcome::Failed,
        }));
        assert!(observations.contains(&CapturedObservation {
            kind: CapturedAccessKind::File,
            logical_path: "project:missing.bin".to_owned(),
            font_index: None,
            outcome: CapturedAccessOutcome::Missing,
        }));
        assert!(observations.contains(&CapturedObservation {
            kind: CapturedAccessKind::Font,
            logical_path: "font-index:41".to_owned(),
            font_index: Some(41),
            outcome: CapturedAccessOutcome::Missing,
        }));
    }
}
