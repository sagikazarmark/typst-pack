# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**Pack**:
A portable, reusable Typst project whose compilation contract consists of contained project files and may also include vendored packages, embedded fonts, and declared External Project Resources.

**Pack Manifest**:
The versioned declarative description carried by a Pack. It describes intended contents and metadata, while agreement between those declarations and contained files is a Pack invariant.

**External Project Resource**:
A non-source resource addressed through Typst's project root whose path belongs to a pack's compilation contract, but whose bytes are supplied externally instead of being stored in the reusable pack. It may be required or conditional for a particular compilation.
_Avoid_: Render Asset, asset, external input, customer file, dynamic pack file

**External Resource Reference**:
An opaque compilation input identifying a path-addressable source that may supply bytes for one or more External Project Resources. Source references form an ordered fallback chain in which only a missing resource falls through to the next source.
_Avoid_: Resource Path, resource directory, resource loader

**Document Format**:
A compilation format that produces one Compilation Output Artifact for a selected or unpaged document. PDF and HTML are Document Formats.

**Page Format**:
A compilation format that produces one Compilation Output Artifact for each selected source page. PNG and SVG are Page Formats.

**Source Page Number**:
The one-based physical position of a page in the source document before page selection, distinct from its emission order or printed page label.

**Compilation Output Artifact**:
One file produced by compiling a Pack. It carries its Document Format or Page Format. A Page Format artifact corresponds to one Source Page Number, while a Document Format artifact has no single Source Page Number.
_Avoid_: Output buffer, result file

**Pack Override**:
A compilation-scoped replacement for a file contained in a pack. Unlike an External Project Resource, it deliberately changes reusable pack content for one compilation.
_Avoid_: External Project Resource, replacement asset
