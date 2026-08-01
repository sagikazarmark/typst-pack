# Version-1 Pack Archive fixtures

These checked-in archives freeze semantic interoperability and representative
malformed-input observations. They do not define byte-for-byte encoder output.

`accepted-python.typk` and the malformed ZIP fixtures are produced with Python's
standard-library `zipfile` implementation. The raw-name and duplicate fixtures
use the small independent stored-ZIP producer in `generate.py` because Python's
high-level ZIP API cannot express those names unambiguously.

Regenerate the corpus from the repository root with:

```console
python3 crates/typst-pack/tests/fixtures/pack-archive-v1/generate.py
```

The contract suite decodes these files through typst-pack's public API. It also
consumes typst-pack encoder output with a test-only ZIP reader built directly on
the ZIP records and Deflate stream, independently of the production `zip` crate.
