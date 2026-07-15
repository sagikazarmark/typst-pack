# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**External Project Resource**:
A non-source resource addressed through Typst's project root whose path belongs to a pack's compilation contract, but whose bytes are supplied externally instead of being stored in the reusable pack. It may be required or conditional for a particular compilation.
_Avoid_: Render Asset, asset, external input, customer file, dynamic pack file

**Pack Override**:
A compilation-scoped replacement for a file contained in a pack. Unlike an External Project Resource, it deliberately changes reusable pack content for one compilation.
_Avoid_: External Project Resource, replacement asset
