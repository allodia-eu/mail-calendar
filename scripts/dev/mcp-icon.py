#!/usr/bin/env python3
"""Regenerate the icon the MCP server advertises to a connected client.

`crates/mailcal-mcp/assets/icon-128.png` is a binary blob in the repository, so its
provenance has to live somewhere or it becomes a file nobody dares touch. This is that
somewhere: run it after any change to the app icon.

    /usr/bin/python3 scripts/dev/mcp-icon.py

Why these numbers. The icon is inlined into every `initialize` response as a base64 `data:`
URI (see `crates/mailcal-mcp/src/branding.rs` for why a data URI rather than a URL), so its
byte count is a wire cost, paid once per session. 128x128 is the smallest size that stays
crisp where clients render it; a list avatar at @2x; and a 128-colour octree quantization
roughly halves the file without visible banding in the icon's blue gradient. Full-colour is
~31 KB; 64 colours bands the gradient. This is the middle.

Requires Pillow (`/usr/bin/python3 -m pip install --user Pillow`).
"""

from __future__ import annotations

import pathlib
import sys

try:
    from PIL import Image
except ImportError:  # pragma: no cover - a developer-tooling path
    sys.exit("Pillow is required: /usr/bin/python3 -m pip install --user Pillow")

ROOT = pathlib.Path(__file__).resolve().parents[2]
# The macOS app icon is the highest-fidelity master in the tree; every other platform's icon
# is generated from the same artwork, so there is one source of truth here too.
SOURCE = ROOT / "clients/apple/App/Assets.xcassets/AppIcon.appiconset/icon_512x512.png"
TARGET = ROOT / "crates/mailcal-mcp/assets/icon-128.png"
EDGE = 128
COLOURS = 128


def main() -> None:
    if not SOURCE.exists():
        sys.exit(f"source icon not found: {SOURCE}")
    icon = Image.open(SOURCE).convert("RGBA").resize((EDGE, EDGE), Image.LANCZOS)
    # FASTOCTREE is the only quantizer Pillow offers for RGBA, and it is the one that matters:
    # the icon has a transparent rounded corner, and a quantizer that flattened alpha would
    # put a grey box behind the robot on a dark background.
    quantized = icon.quantize(colors=COLOURS, method=Image.FASTOCTREE).convert("RGBA")
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    quantized.save(TARGET, "PNG", optimize=True)
    size = TARGET.stat().st_size
    print(f"wrote {TARGET.relative_to(ROOT).as_posix()}: {size} bytes ({size * 4 // 3} as base64)")


if __name__ == "__main__":
    main()
