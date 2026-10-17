//! Shared compilation and creation domain vocabulary.

use typst::foundations::Datetime;

/// The Typst document model selected for creation or compilation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypstTarget {
    /// A paged document used by PDF and image formats.
    #[default]
    Paged,
    /// An HTML document.
    Html,
}

/// The exact or explicitly absent time used by Typst document-time requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Hash)]
pub enum DocumentTime {
    /// Document-time requests have no value.
    #[default]
    Absent,
    /// Return one fixed Typst date or datetime.
    Fixed(Datetime),
    /// Resolve one fixed UTC instant under each requested timezone offset.
    UnixTimestamp(i64),
}
