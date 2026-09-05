#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 WarpCoreDev
# SPDX-License-Identifier: GPL-3.0-or-later

"""Regenerates src-tauri/icons/ from the master logo in assets/logo.png.

Everything Tauri bundles comes out of here: the PNG sizes listed in
tauri.conf.json, a multi-size Windows .ico, and a macOS .icns.

The .icns is assembled by hand rather than through Pillow's writer, which is
unreliable off macOS. The format is simple enough: a header, then one entry per
size — a 4-byte OSType, a big-endian length covering the whole entry, and a
complete PNG.

Requires Pillow (`python3 -c "import PIL"`). Run from anywhere:

    python3 scripts/generate-icons.py
"""

import io
import struct
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("Pillow is required: pip install Pillow (inside a venv)")

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "assets" / "logo.png"
OUT = REPO / "src-tauri" / "icons"

# name -> pixel size, matching the bundle.icon list in tauri.conf.json
PNG_SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}

ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

# macOS icon types and the pixel size each one holds.
ICNS_ENTRIES = [
    (b"ic07", 128),   # 128x128
    (b"ic08", 256),   # 256x256
    (b"ic09", 512),   # 512x512
    (b"ic10", 1024),  # 512x512@2x
    (b"ic11", 32),    # 16x16@2x
    (b"ic12", 64),    # 32x32@2x
    (b"ic13", 256),   # 128x128@2x
    (b"ic14", 512),   # 256x256@2x
]


def main() -> None:
    if not SOURCE.exists():
        sys.exit(f"missing master logo: {SOURCE}")

    src = Image.open(SOURCE).convert("RGBA")
    if src.width != src.height:
        print(f"warning: master is {src.width}x{src.height}, not square", file=sys.stderr)
    OUT.mkdir(parents=True, exist_ok=True)

    # Lanczos holds up in both directions; flat artwork upscales acceptably.
    def at(size: int) -> Image.Image:
        return src.resize((size, size), Image.LANCZOS)

    for name, size in PNG_SIZES.items():
        at(size).save(OUT / name)

    at(max(ICO_SIZES)).save(
        OUT / "icon.ico", format="ICO", sizes=[(s, s) for s in ICO_SIZES]
    )

    entries = b""
    for ostype, size in ICNS_ENTRIES:
        buf = io.BytesIO()
        at(size).save(buf, format="PNG")
        png = buf.getvalue()
        entries += ostype + struct.pack(">I", len(png) + 8) + png
    (OUT / "icon.icns").write_bytes(b"icns" + struct.pack(">I", len(entries) + 8) + entries)

    for f in sorted(OUT.iterdir()):
        print(f"  {f.stat().st_size:>8}  {f.name}")


if __name__ == "__main__":
    main()
