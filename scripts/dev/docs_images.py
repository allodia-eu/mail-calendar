#!/usr/bin/env python3
"""Turn documentation captures into web images and the manifest that addresses them.

`scripts/dev/showcase.sh <platform> --set docs` writes full-resolution PNGs to
`showcase-screenshots/docs/<platform>/<locale>-<screen>.png`. This downscales each one, encodes it
as WebP, names the result by the SHA-256 of its own bytes, and writes
`docs/user/screenshots.json`; the one thing in this pipeline that git holds
(`docs/user-docs.md`).

Content addressing is what makes the manifest a *check* rather than a note: a page names a
screenshot id, the manifest resolves it to a hash, and the website serves the blob under that hash.
An image that drifted, or never got published, is a build failure at that step instead of a broken
`<img>` a reader discovers.

    python3 scripts/dev/docs_images.py                 # optimize + rewrite the manifest
    python3 scripts/dev/docs_images.py --check         # fail if the manifest is out of date

Needs `cwebp` (`brew install webp` / `apt install webp`), which both resizes and encodes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Dict, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parents[2]

CAPTURES = REPO_ROOT / "showcase-screenshots" / "docs"
BLOBS = REPO_ROOT / "showcase-screenshots" / "docs-web"
MANIFEST = REPO_ROOT / "docs" / "user" / "screenshots.json"

# Wide enough that a phone screenshot stays legible at full width on a desktop page, and small
# enough that a page with six of them is not a megabyte of images. Portrait captures are far taller
# than they are wide, so this is a cap on the *width* only; nothing is ever upscaled.
DEFAULT_WIDTH = 1400

# WebP quality. 82 is the knee of the curve for UI screenshots; flat fills and text, which is what
# WebP handles best; above it the file grows faster than the picture improves.
QUALITY = 82

# `<locale>-<screen>.png`, the name showcase.sh writes. The locale is a two-letter catalog code and
# a screen name is lower-case with hyphens, so the split is unambiguous at the first hyphen.
CAPTURE_NAME = re.compile(r"^([a-z]{2})-([a-z][a-z0-9-]*)\.png$")


class ImageError(Exception):
    """A capture or an encoded image could not be read."""


def shown(path: Path) -> str:
    """A repo-relative path for a message, falling back to the absolute one.

    `Path.relative_to` *raises* when the path is outside the root; so a `--manifest` pointing
    anywhere else crashed the run in its final success message, after every file had been written.

    POSIX-separated on every host: these paths name files in *this repository*, and a reader
    copying one back into a shell, a doc or a commit message needs the form the repo uses. A
    Windows run printing `docs\\user\\en\\setup.md` names the same file and is still the wrong
    string to hand anyone.
    """
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return str(path)


# ---- reading image headers ----------------------------------------------------------------------
#
# Read the dimensions out of the files themselves rather than computing them from the resize
# request. The manifest's width/height are what the renderer reserves space with, so a number that
# merely describes what we *asked* cwebp for would be an unchecked assumption in the one field a
# reader would notice being wrong.


def png_size(data: bytes) -> Tuple[int, int]:
    """The pixel size of a PNG, from its IHDR chunk."""
    if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ImageError("not a PNG (no signature or no leading IHDR chunk)")
    return (
        int.from_bytes(data[16:20], "big"),
        int.from_bytes(data[20:24], "big"),
    )


def webp_size(data: bytes) -> Tuple[int, int]:
    """The pixel size of a WebP, from whichever of the three frame headers it carries."""
    if len(data) < 16 or data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise ImageError("not a WebP (no RIFF/WEBP container)")
    kind, payload = data[12:16], data[20:]
    if kind == b"VP8X":  # extended format: canvas size, 24-bit, stored minus one
        if len(payload) < 10:
            raise ImageError("truncated VP8X header")
        width = int.from_bytes(payload[4:7], "little") + 1
        height = int.from_bytes(payload[7:10], "little") + 1
        return width, height
    if kind == b"VP8 ":  # simple lossy: 14-bit dimensions after the 3-byte start code
        if len(payload) < 10 or payload[3:6] != b"\x9d\x01\x2a":
            raise ImageError("truncated or unrecognized VP8 frame header")
        width = int.from_bytes(payload[6:8], "little") & 0x3FFF
        height = int.from_bytes(payload[8:10], "little") & 0x3FFF
        return width, height
    if kind == b"VP8L":  # lossless: 1-byte signature, then 14 bits each, stored minus one
        if len(payload) < 5 or payload[0] != 0x2F:
            raise ImageError("truncated or unrecognized VP8L frame header")
        bits = int.from_bytes(payload[1:5], "little")
        return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    raise ImageError("unsupported WebP frame type %r" % kind.decode("ascii", "replace"))


# ---- encoding -----------------------------------------------------------------------------------


def require_cwebp() -> str:
    """The `cwebp` binary, or a message naming how to get it."""
    found = shutil.which("cwebp")
    if found is None:
        raise SystemExit(
            "cwebp is not on PATH. It both resizes and encodes here, so there is no fallback.\n"
            "  macOS:  brew install webp\n"
            "  Debian: sudo apt install webp"
        )
    return found


def encode(cwebp: str, source: Path, width: int, out_dir: Path) -> Dict[str, object]:
    """Downscale and encode one capture; return its manifest entry.

    The blob is named by the SHA-256 of the *encoded* bytes, which is what the website serves it
    under; so re-running this on unchanged captures rewrites the same names and the manifest does
    not move.
    """
    original = source.read_bytes()
    source_width, _ = png_size(original)
    # `-resize <w> 0` scales height proportionally. Never upscale: a capture narrower than the cap
    # is already as good as it gets, and stretching it would only invent pixels.
    target = min(width, source_width)

    with tempfile.TemporaryDirectory() as work:
        encoded_path = Path(work) / "out.webp"
        result = subprocess.run(
            [cwebp, "-quiet", "-q", str(QUALITY), "-resize", str(target), "0",
             str(source), "-o", str(encoded_path)],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise ImageError("cwebp failed on %s: %s" % (source.name, result.stderr.strip()))
        encoded = encoded_path.read_bytes()

    digest = hashlib.sha256(encoded).hexdigest()
    out_width, out_height = webp_size(encoded)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / ("%s.webp" % digest)).write_bytes(encoded)
    return {
        "sha256": digest,
        "width": out_width,
        "height": out_height,
        "bytes": len(encoded),
    }


# ---- the manifest -------------------------------------------------------------------------------


def build(captures: Path, blobs: Path, width: int) -> Tuple[Dict[str, object], int]:
    """Encode every capture under `captures` and assemble the manifest's `images` map.

    The encoder is acquired on the *first* capture rather than up front. A run with nothing to
    encode needs no encoder, and demanding one anyway put the empty-set refusal below; the whole
    point of which is to protect a good manifest; out of reach of anyone without `cwebp`
    installed, including its own tests. Failing fast is preserved: the first file still asks.
    """
    cwebp = None  # type: Optional[str]
    images = {}  # type: Dict[str, Dict[str, Dict[str, object]]]
    count = 0
    for platform_dir in sorted(p for p in captures.iterdir() if p.is_dir()):
        for capture in sorted(platform_dir.glob("*.png")):
            match = CAPTURE_NAME.match(capture.name)
            if match is None:
                raise ImageError(
                    "%s is not named <locale>-<screen>.png. showcase.sh writes that shape; a file "
                    "that does not match is either hand-placed or from an older run. Remove it "
                    "rather than letting it into the manifest under a guessed id."
                    % shown(capture)
                )
            locale, screen = match.group(1), match.group(2)
            if cwebp is None:
                cwebp = require_cwebp()
            entry = encode(cwebp, capture, width, blobs)
            images.setdefault(screen, {}).setdefault(platform_dir.name, {})[locale] = entry
            count += 1
    return images, count


def manifest_document(images: Dict[str, object]) -> Dict[str, object]:
    """The whole file, so `--check` compares what would be written, not just the images."""
    return {
        "version": 1,
        "generator": "scripts/dev/docs_images.py",
        "images": images,
    }


def serialize(document: Dict[str, object]) -> str:
    return json.dumps(document, indent=2, sort_keys=True) + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--captures", type=Path, default=CAPTURES)
    parser.add_argument("--blobs", type=Path, default=BLOBS)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 if the manifest on disk differs from what a fresh run would write",
    )
    parser.add_argument(
        "--allow-empty",
        action="store_true",
        help="write an empty manifest (only ever right when deliberately clearing the set)",
    )
    args = parser.parse_args(argv)

    if not args.captures.is_dir():
        print(
            "No captures at %s. Take some first:\n"
            "  scripts/dev/showcase.sh macos --set docs\n"
            "  scripts/dev/showcase.sh android --set docs" % args.captures,
            file=sys.stderr,
        )
        return 1

    try:
        images, count = build(args.captures, args.blobs, args.width)
    except ImageError as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 1

    # An empty run would otherwise quietly replace a good manifest with `{}`, and every page would
    # then fail its screenshot check for a reason that has nothing to do with the pages. Refuse,
    # loudly, and make clearing the set an explicit act.
    if count == 0 and not args.allow_empty:
        print(
            "ERROR: found no captures under %s, so the manifest would be emptied. Capture first, "
            "or pass --allow-empty if clearing it is what you meant." % args.captures,
            file=sys.stderr,
        )
        return 1

    document = manifest_document(images)
    rendered = serialize(document)

    if args.check:
        current = args.manifest.read_text(encoding="utf-8") if args.manifest.exists() else ""
        if current != rendered:
            print(
                "ERROR: %s is out of date: re-run scripts/dev/docs_images.py and commit it."
                % shown(args.manifest),
                file=sys.stderr,
            )
            return 1
        print("OK: the screenshot manifest matches %d capture(s)." % count)
        return 0

    args.manifest.write_text(rendered, encoding="utf-8")
    print(
        "Wrote %s: %d image(s) across %d screenshot id(s); blobs in %s"
        % (
            shown(args.manifest),
            count,
            len(images),
            shown(args.blobs),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
