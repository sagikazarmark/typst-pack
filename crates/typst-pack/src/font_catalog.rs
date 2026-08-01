//! The Candidate Font Catalog Pack Creation selects faces from.

#[cfg(feature = "embedded-fonts")]
use typst::foundations::Bytes;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::FontStore;

use crate::pack::{FontContainerIdentity, FontFaceIdentity};
use crate::payload::SharedBytes;

/// Whether a Font Container's bytes travel inside the Pack or must be
/// fulfilled externally when the Pack is compiled.
///
/// A caller declares the disposition of every candidate container; it is never
/// inferred from container bytes, so identical inputs produce identical Packs
/// across build configurations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontDisposition {
    /// The container's exact bytes are stored in the Pack.
    Embedded,
    /// The container is declared and must be supplied at compilation.
    External,
}

impl FontDisposition {
    /// [`Embedded`](Self::Embedded) when `embed` is set, otherwise
    /// [`External`](Self::External).
    pub fn embedded_if(embed: bool) -> Self {
        if embed {
            Self::Embedded
        } else {
            Self::External
        }
    }

    /// Whether the container's exact bytes are stored in the Pack.
    pub fn is_embedded(self) -> bool {
        matches!(self, Self::Embedded)
    }
}

/// One candidate Font Container: the exact bytes of one standalone font file
/// or multi-face collection, and the disposition it carries into the Pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateFontContainer {
    data: SharedBytes,
    disposition: FontDisposition,
}

impl CandidateFontContainer {
    /// Offers the exact container bytes under the given disposition.
    pub fn new(data: impl Into<Vec<u8>>, disposition: FontDisposition) -> Self {
        Self::from_shared(SharedBytes::new(data.into()), disposition)
    }

    #[cfg(feature = "embedded-fonts")]
    pub(crate) fn from_bytes(data: Bytes, disposition: FontDisposition) -> Self {
        Self::from_shared(SharedBytes::from_typst(data), disposition)
    }

    pub(crate) fn from_shared(data: SharedBytes, disposition: FontDisposition) -> Self {
        Self { data, disposition }
    }

    /// Offers a container whose bytes are stored in the Pack.
    pub fn embedded(data: impl Into<Vec<u8>>) -> Self {
        Self::new(data, FontDisposition::Embedded)
    }

    /// Offers a container that must be fulfilled externally.
    ///
    /// Mind font licenses: embedding redistributes the container bytes, while
    /// an external requirement only declares them.
    pub fn external(data: impl Into<Vec<u8>>) -> Self {
        Self::new(data, FontDisposition::External)
    }

    /// The exact container bytes.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// The Canonical Identity of the container bytes.
    pub fn identity(&self) -> FontContainerIdentity {
        FontContainerIdentity::from_bytes(&self.data)
    }

    /// Whether the container travels inside the Pack.
    pub fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// The Candidate Font Catalog: exactly the Font Containers Pack Creation may
/// select faces from, in the order the caller chose.
///
/// Face selection is attributable to that order alone. Faces are expanded in
/// container-local index order, and nothing joins a supplied catalog
/// implicitly, so Pack contents are not a function of which crate features a
/// build enabled or which font sources a host offers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateFontCatalog {
    containers: Vec<CandidateFontContainer>,
}

impl CandidateFontCatalog {
    /// An empty catalog, offering no candidate face at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one candidate container after every container already offered.
    pub fn push(&mut self, container: CandidateFontContainer) {
        self.containers.push(container);
    }

    /// The candidate containers, in catalog order.
    pub fn containers(&self) -> &[CandidateFontContainer] {
        &self.containers
    }

    /// The candidate faces this catalog offers, in catalog order: every face
    /// of the first container in container-local index order, then those of
    /// the second, and so on.
    ///
    /// Faces a container's bytes do not yield are not offered, and a container
    /// that holds no readable face offers nothing.
    pub fn faces(&self) -> Vec<CandidateFontFace> {
        self.expand().faces
    }

    /// Expands the catalog into the faces creation compiles against.
    pub(crate) fn expand(&self) -> CandidateFonts {
        let mut store = FontStore::new();
        let mut faces = Vec::new();
        for container in &self.containers {
            let identity = container.identity();
            for font in Font::iter(container.data.to_typst()) {
                let info = font.info().clone();
                faces.push(CandidateFontFace {
                    identity: FontFaceIdentity::new(identity, font.index()),
                    disposition: container.disposition,
                });
                store.push((font, info));
            }
        }
        CandidateFonts { store, faces }
    }
}

impl Extend<CandidateFontContainer> for CandidateFontCatalog {
    fn extend<T: IntoIterator<Item = CandidateFontContainer>>(&mut self, containers: T) {
        self.containers.extend(containers);
    }
}

impl FromIterator<CandidateFontContainer> for CandidateFontCatalog {
    fn from_iter<T: IntoIterator<Item = CandidateFontContainer>>(containers: T) -> Self {
        Self {
            containers: containers.into_iter().collect(),
        }
    }
}

/// One candidate face a catalog offers to Pack Creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateFontFace {
    identity: FontFaceIdentity,
    disposition: FontDisposition,
}

impl CandidateFontFace {
    /// The exact container and container-local face index.
    pub fn identity(&self) -> FontFaceIdentity {
        self.identity
    }

    /// The disposition the face's container carries.
    pub fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// The compile-time projection of one Candidate Font Catalog: the candidate
/// faces in catalog order, indexed exactly as the representative compile sees
/// them.
pub(crate) struct CandidateFonts {
    store: FontStore,
    faces: Vec<CandidateFontFace>,
}

// Only creation compiles against candidate faces.
impl CandidateFonts {
    /// The selection metadata official Typst chooses faces from.
    pub(crate) fn book(&self) -> &LazyHash<FontBook> {
        self.store.book()
    }

    /// The face at the given catalog position.
    pub(crate) fn font(&self, index: usize) -> Option<Font> {
        self.store.font(index)
    }

    /// The disposition carried by the container of the face at the given
    /// catalog position.
    pub(crate) fn disposition(&self, index: usize) -> Option<FontDisposition> {
        self.faces.get(index).map(CandidateFontFace::disposition)
    }
}

/// Typst's embedded fonts as candidate containers, in Typst's own order.
///
/// A caller splices them into its catalog at the position it wants; they never
/// join a catalog implicitly. Their disposition is the caller's choice, like
/// that of any other container.
#[cfg(feature = "embedded-fonts")]
pub fn typst_embedded_font_containers(
    disposition: FontDisposition,
) -> impl Iterator<Item = CandidateFontContainer> {
    // Typst exposes its embedded fonts one face at a time. Every face of one
    // container carries that container's exact bytes, so first-seen order over
    // the faces recovers the containers Typst ships, in Typst's own order.
    let mut containers: Vec<Bytes> = Vec::new();
    for (font, _) in typst_kit::fonts::embedded() {
        if !containers.iter().any(|data| data == font.data()) {
            containers.push(font.data().clone());
        }
    }
    containers
        .into_iter()
        .map(move |data| CandidateFontContainer::from_bytes(data, disposition))
}
