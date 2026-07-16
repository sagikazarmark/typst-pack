# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**Pack**:
A portable, reusable Typst project whose compilation contract consists of contained project files and may also include vendored packages, embedded fonts, and declared Resource Slots.

**Pack Manifest**:
The versioned declarative description carried by a Pack. It describes intended contents and metadata, while agreement between those declarations and contained files is a Pack invariant.

**Resource Slot**:
A project-root-relative location declared by a Pack for non-source bytes supplied to a compilation rather than stored in the Pack. A slot may remain unfilled when a compilation does not request it.
_Avoid_: External Project Resource, placeholder, Render Asset, asset, external input, customer file, dynamic pack file

**Resource Provider**:
An opaque, path-addressable provider that may supply bytes for one or more Resource Slots. Providers form an ordered fallback chain in which only a missing resource falls through to the next provider.
_Avoid_: External Resource Provider, External Resource Reference, Source Reference, resource loader

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
A compilation-scoped replacement for a project file contained in a Pack. Unlike a Resource Slot, it deliberately changes reusable project content for one compilation; vendored package files and embedded fonts are not eligible.
_Avoid_: Resource Slot, replacement asset
