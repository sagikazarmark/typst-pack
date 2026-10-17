//! Font containers, their faces, and the requirements a pack records.
//!
//! A container is one font file or multi-face collection. Creation selects
//! containers from an ordered list of [`Entry`] values, each either embedded
//! in the pack or left for external fulfillment; nothing joins that list
//! implicitly, so a pack does not depend on which fonts a host happens to
//! offer.

use std::fmt;

use serde::{Deserialize, Serialize};
#[cfg(feature = "embedded-fonts")]
use typst::foundations::Bytes;
use typst::text::{Font, FontStretch, FontStyle, FontWeight};

use crate::Digest;
use crate::payload::SharedBytes;

/// The exact validated bytes of one font file or multi-face collection.
#[derive(Clone, Debug)]
pub struct Container {
    data: SharedBytes,
    digest: Digest,
    fonts: Vec<Font>,
    faces: Vec<Face>,
}

impl Container {
    /// Validates font bytes, or returns `None` when they hold no face the
    /// embedded Typst engine can read.
    pub fn new(data: impl Into<Vec<u8>>) -> Option<Self> {
        Self::from_shared(SharedBytes::new(data.into()))
    }

    pub(crate) fn from_shared(data: SharedBytes) -> Option<Self> {
        let fonts = Font::iter(data.to_typst()).collect::<Vec<_>>();
        if fonts.is_empty() {
            return None;
        }
        Some(Self {
            digest: Digest::of(&data),
            faces: fonts.iter().map(Face::of).collect(),
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

    /// The readable faces in container order.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    pub(crate) fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub(crate) fn shared_data(&self) -> &SharedBytes {
        &self.data
    }
}

impl PartialEq for Container {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for Container {}

/// The selection metadata of one face of a font container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Face {
    /// The face index within the container.
    pub index: u32,
    /// The typographic family name.
    pub family: String,
    /// The face's style.
    pub style: FontStyle,
    /// The face's weight.
    pub weight: FontWeight,
    /// The face's stretch.
    pub stretch: FontStretch,
}

impl Face {
    pub(crate) fn of(font: &Font) -> Self {
        let info = font.info();
        Self {
            index: font.index(),
            family: info.family.clone(),
            style: info.variant.style,
            weight: info.variant.weight,
            stretch: info.variant.stretch,
        }
    }
}

impl fmt::Display for Face {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = match self.style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        };
        write!(
            formatter,
            "{} ({style}, weight {}, stretch {:?})",
            self.family,
            self.weight.to_number(),
            self.stretch.to_ratio()
        )
    }
}

/// One exact font container a pack needs. Every face of the container is
/// available to the document at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// The SHA-256 of the container bytes.
    pub digest: Digest,
    /// Whether the container's bytes are stored in the pack.
    pub embedded: bool,
    /// The faces of the container, in container order.
    pub faces: Vec<Face>,
}

impl Requirement {
    /// The family names of the container's faces, deduplicated, for messages.
    pub(crate) fn families(&self) -> String {
        let mut families = Vec::<&str>::new();
        for face in &self.faces {
            if !families.contains(&face.family.as_str()) {
                families.push(&face.family);
            }
        }
        if families.is_empty() {
            "unknown family".to_owned()
        } else {
            families.join(", ")
        }
    }
}

/// A font container creation may select, embedded in the pack or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The container.
    pub container: Container,
    /// Whether a created pack stores the container's bytes.
    pub embedded: bool,
}

/// Typst's embedded fonts as containers, in Typst's own order.
///
/// They never join creation's fonts implicitly; add them where they should
/// rank, embedded or not, like any other container.
#[cfg(feature = "embedded-fonts")]
pub fn embedded() -> impl Iterator<Item = Container> {
    // Typst exposes its embedded fonts one face at a time. Every face of one
    // container carries that container's exact bytes, so first-seen order over
    // the faces recovers the containers Typst ships, in Typst's own order.
    let mut containers: Vec<Bytes> = Vec::new();
    for (font, _) in typst_kit::fonts::embedded() {
        if !containers.iter().any(|data| data == font.data()) {
            containers.push(font.data().clone());
        }
    }
    containers.into_iter().map(|data| {
        Container::from_shared(SharedBytes::from_typst(data))
            .expect("Typst's embedded fonts are readable")
    })
}

/// Whether a file extension names a font container, compared
/// case-insensitively.
#[cfg(any(feature = "fs", feature = "opendal"))]
pub(crate) fn is_container_extension(extension: &str) -> bool {
    ["ttf", "ttc", "otf", "otc"]
        .iter()
        .any(|supported| extension.eq_ignore_ascii_case(supported))
}

/// Whether a storage key ends in a font container extension. Unlike a path,
/// a key that is nothing but the extension, such as `.ttf`, qualifies.
#[cfg(feature = "opendal")]
pub(crate) fn is_container_key(key: &str) -> bool {
    key.rsplit_once('.')
        .is_some_and(|(_, suffix)| is_container_extension(suffix))
}

#[cfg(all(test, feature = "opendal"))]
mod tests {
    use super::*;

    #[test]
    fn font_container_names_are_matched_case_insensitively() {
        for (key, supported) in [
            ("container.ttf", true),
            ("container.TTF", true),
            ("fonts/nested/container.otc", true),
            ("container.Ttc", true),
            ("container.oTf", true),
            (".ttf", true),
            ("container.woff", false),
            ("container.ttf.txt", false),
            ("container", false),
            ("ttf", false),
            ("fonts.ttf/container", false),
            ("", false),
        ] {
            assert_eq!(is_container_key(key), supported, "{key:?}");
        }
    }
}
