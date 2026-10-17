//! Validated Font Containers and the ordered Font Catalog.

#[cfg(feature = "embedded-fonts")]
use typst::foundations::Bytes;
use typst::text::Font;

use crate::Digest;
use crate::pack::FontFace;
use crate::payload::SharedBytes;

/// Whether a Font Container's bytes travel inside the Pack or must be
/// fulfilled externally when the Pack is compiled.
///
/// A caller declares the disposition of every catalog position; it is never
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

/// A failure to construct a validated Font Container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FontContainerError {
    /// The exact bytes contain no face the embedded Typst engine can read.
    #[error("font container has no readable face")]
    NoReadableFace,
}

/// The exact validated bytes of one standalone font file or multi-face
/// collection.
#[derive(Clone, Debug)]
pub struct FontContainer {
    data: SharedBytes,
    digest: Digest,
    fonts: Vec<Font>,
    faces: Vec<FontFace>,
}

impl FontContainer {
    /// Validates exact owned container bytes.
    pub fn new(data: impl Into<Vec<u8>>) -> Result<Self, FontContainerError> {
        Self::from_shared(SharedBytes::new(data.into()))
    }

    #[cfg(feature = "embedded-fonts")]
    fn from_bytes(data: Bytes) -> Result<Self, FontContainerError> {
        Self::from_shared(SharedBytes::from_typst(data))
    }

    pub(crate) fn from_shared(data: SharedBytes) -> Result<Self, FontContainerError> {
        let fonts = Font::iter(data.to_typst()).collect::<Vec<_>>();
        if fonts.is_empty() {
            return Err(FontContainerError::NoReadableFace);
        }
        Ok(Self {
            digest: Digest::of(&data),
            faces: fonts.iter().map(FontFace::of).collect(),
            data,
            fonts,
        })
    }

    /// The exact container bytes.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// The SHA-256 of the container bytes.
    pub fn digest(&self) -> Digest {
        self.digest
    }

    /// The readable faces in container-local index order.
    pub fn faces(&self) -> &[FontFace] {
        &self.faces
    }

    pub(crate) fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub(crate) fn shared_data(&self) -> &SharedBytes {
        &self.data
    }
}

impl PartialEq for FontContainer {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for FontContainer {}

/// One position in a Font Catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontCatalogEntry {
    container: FontContainer,
    disposition: FontDisposition,
}

impl FontCatalogEntry {
    /// Pairs one validated container with its explicit disposition.
    pub fn new(container: FontContainer, disposition: FontDisposition) -> Self {
        Self {
            container,
            disposition,
        }
    }

    /// The validated Font Container at this position.
    pub fn container(&self) -> &FontContainer {
        &self.container
    }

    /// Whether this position embeds or externally fulfills its container.
    pub fn disposition(&self) -> FontDisposition {
        self.disposition
    }
}

/// Exactly the Font Containers Pack Creation may select from, in the order the
/// caller chose.
///
/// Font selection is attributable to that order alone. Faces are expanded in
/// container-local index order, and nothing joins a supplied catalog
/// implicitly, so Pack contents are not a function of which crate features a
/// build enabled or which font sources a host offers. A Pack records every
/// container the representative compile used, in catalog order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontCatalog {
    entries: Vec<FontCatalogEntry>,
}

impl FontCatalog {
    /// An empty catalog, offering no face at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one explicitly disposed container after every existing entry.
    pub fn push(&mut self, entry: FontCatalogEntry) {
        self.entries.push(entry);
    }

    /// The entries in insertion order.
    pub fn entries(&self) -> &[FontCatalogEntry] {
        &self.entries
    }
}

impl Extend<FontCatalogEntry> for FontCatalog {
    fn extend<T: IntoIterator<Item = FontCatalogEntry>>(&mut self, entries: T) {
        self.entries.extend(entries);
    }
}

impl FromIterator<FontCatalogEntry> for FontCatalog {
    fn from_iter<T: IntoIterator<Item = FontCatalogEntry>>(entries: T) -> Self {
        let mut catalog = Self::new();
        catalog.extend(entries);
        catalog
    }
}

/// Typst's embedded fonts as validated containers, in Typst's own order.
///
/// A caller splices them into its catalog at the position it wants; they never
/// join a catalog implicitly. Their disposition is the caller's choice, like
/// that of any other container.
#[cfg(feature = "embedded-fonts")]
pub fn typst_embedded_font_containers() -> impl Iterator<Item = FontContainer> {
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
        .map(|data| FontContainer::from_bytes(data).expect("embedded Font Container is readable"))
}
