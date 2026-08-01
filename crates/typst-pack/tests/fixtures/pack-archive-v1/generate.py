#!/usr/bin/env python3
"""Regenerate the version-1 Pack Archive interoperability fixtures."""

from __future__ import annotations

import binascii
import stat
import struct
import zipfile
from pathlib import Path


ROOT = Path(__file__).parent
TIMESTAMP = (2020, 1, 1, 0, 0, 0)
MANIFEST = b'''format-version = 1

[project]
entrypoint = "main.typ"

[[packages.unvendored]]
spec = "@preview/example:1.0.0"
tree-digest = "00000000000000000000000000000001"
tree-identity-kind = "complete-package-tree"
tree-identity-schema = "typst-pack-complete-package-tree-v1"
tree-identity-algorithm = "typst-hash128-0.15"
file-count = 1
byte-length = 7

[metadata]
name = "Python fixture"
authors = ["Independent producer"]
'''
MINIMAL_MANIFEST = b'format-version = 1\n[project]\nentrypoint = "main.typ"\n'


def member(name: str, data: bytes, mode: int = stat.S_IFREG | 0o644) -> tuple[zipfile.ZipInfo, bytes]:
    info = zipfile.ZipInfo(name, TIMESTAMP)
    info.create_system = 3
    info.external_attr = mode << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    return info, data


def directory(name: str) -> tuple[zipfile.ZipInfo, bytes]:
    info, data = member(name, b'', stat.S_IFDIR | 0o755)
    info.external_attr |= 0x10
    return info, data


def write_zip(name: str, entries: list[tuple[zipfile.ZipInfo, bytes]]) -> None:
    with zipfile.ZipFile(ROOT / name, 'w') as archive:
        for info, data in entries:
            archive.writestr(info, data)


def raw_stored_zip(entries: list[tuple[bytes, bool, bytes]]) -> bytes:
    output = bytearray()
    central = []
    for name, utf8, data in entries:
        offset = len(output)
        flags = 1 << 11 if utf8 else 0
        crc = binascii.crc32(data)
        output += struct.pack(
            '<IHHHHHIIIHH',
            0x04034B50,
            20,
            flags,
            0,
            0,
            0,
            crc,
            len(data),
            len(data),
            len(name),
            0,
        )
        output += name + data
        central.append((name, flags, data, crc, offset))

    central_offset = len(output)
    for name, flags, data, crc, offset in central:
        output += struct.pack(
            '<IHHHHHHIIIHHHHHII',
            0x02014B50,
            (3 << 8) | 20,
            20,
            flags,
            0,
            0,
            0,
            crc,
            len(data),
            len(data),
            len(name),
            0,
            0,
            0,
            0,
            (stat.S_IFREG | 0o644) << 16,
            offset,
        )
        output += name

    central_size = len(output) - central_offset
    output += struct.pack(
        '<IHHHHIIH',
        0x06054B50,
        0,
        0,
        len(entries),
        len(entries),
        central_size,
        central_offset,
        0,
    )
    return bytes(output)


write_zip(
    'accepted-python.typk',
    [
        directory('project/'),
        member('project/main.typ', b'Hello from Python\n'),
        member('project/notes/caf\N{LATIN SMALL LETTER E WITH ACUTE}.typ', b'Unicode path\n'),
        directory('future/'),
        member('future/data.bin', b'ignored extension data'),
        member('typst-pack.toml', MANIFEST),
    ],
)
write_zip('missing-manifest.typk', [member('project/main.typ', b'Hello')])
write_zip(
    'malformed-manifest.typk',
    [member('typst-pack.toml', b'not valid TOML = ['), member('project/main.typ', b'Hello')],
)
write_zip(
    'unsupported-version.typk',
    [
        member('typst-pack.toml', b'format-version = 99\n[project]\nentrypoint = "main.typ"\n'),
        member('project/main.typ', b'Hello'),
    ],
)
write_zip(
    'invalid-pack.typk',
    [
        member('typst-pack.toml', b'format-version = 1\n[project]\nentrypoint = "missing.typ"\n'),
        member('project/main.typ', b'Hello'),
    ],
)
write_zip(
    'unsafe-path.typk',
    [
        member('typst-pack.toml', MINIMAL_MANIFEST),
        member('project/main.typ', b'Hello'),
        member('../escape', b'unsafe'),
    ],
)
write_zip(
    'unsupported-kind.typk',
    [
        member('typst-pack.toml', MINIMAL_MANIFEST),
        member('project/main.typ', b'Hello'),
        member('future/link', b'target', stat.S_IFLNK | 0o777),
    ],
)
write_zip(
    'unsupported-directory-kind.typk',
    [
        member('typst-pack.toml', MINIMAL_MANIFEST),
        member('project/main.typ', b'Hello'),
        member('future/link/', b'target', stat.S_IFLNK | 0o777),
    ],
)
write_zip(
    'canonical-collision.typk',
    [
        member('typst-pack.toml', MINIMAL_MANIFEST),
        member('project/main.typ', b'first'),
        member('project/./main.typ', b'second'),
    ],
)
(ROOT / 'invalid-utf8-name.typk').write_bytes(
    raw_stored_zip(
        [
            (b'typst-pack.toml', False, MINIMAL_MANIFEST),
            (b'project/main.typ', False, b'Hello'),
            (b'future/\xff.bin', True, b'invalid name'),
        ]
    )
)
(ROOT / 'duplicate-member.typk').write_bytes(
    raw_stored_zip(
        [
            (b'typst-pack.toml', False, MINIMAL_MANIFEST),
            (b'project/main.typ', False, b'Hello'),
            (b'future/data', False, b'first'),
            (b'future/data', False, b'second'),
        ]
    )
)
(ROOT / 'invalid-manifest-utf8.typk').write_bytes(
    raw_stored_zip(
        [
            (b'typst-pack.toml', False, b'\xff'),
            (b'project/main.typ', False, b'Hello'),
        ]
    )
)
