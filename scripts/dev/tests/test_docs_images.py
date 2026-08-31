#!/usr/bin/env python3
"""Unit tests for the documentation image pipeline.

`docs_images.py` needs `cwebp` and a directory of captures, neither of which CI has; so the parts
that can be tested anywhere are tested here, and the parts that cannot are named as such.

What is covered is the reading and the refusing: the image-header parsers whose numbers land in the
manifest a renderer reserves space with, the capture-name rule, and the refusal to overwrite a good
manifest with an empty one. What is not covered is the `cwebp` call itself, which is a subprocess
that either produces a WebP or fails loudly.
"""

from __future__ import annotations

import json
import shutil
import struct
import sys
import tempfile
import unittest
import unittest.mock
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import docs_images as subject  # noqa: E402


def png(width: int, height: int) -> bytes:
    """A minimal but structurally real PNG: signature, then an IHDR chunk with its CRC."""
    header = struct.pack(">II5B", width, height, 8, 6, 0, 0, 0)
    chunk = b"IHDR" + header
    return (
        b"\x89PNG\r\n\x1a\n"
        + struct.pack(">I", len(header))
        + chunk
        + struct.pack(">I", zlib.crc32(chunk))
    )


def riff(fourcc: bytes, payload: bytes) -> bytes:
    """A WebP container around one frame chunk."""
    body = b"WEBP" + fourcc + struct.pack("<I", len(payload)) + payload
    return b"RIFF" + struct.pack("<I", len(body)) + body


class PngHeader(unittest.TestCase):
    def test_reads_the_ihdr_dimensions(self):
        self.assertEqual((2880, 1800), subject.png_size(png(2880, 1800)))

    def test_a_file_that_is_not_a_png_is_an_error(self):
        with self.assertRaises(subject.ImageError):
            subject.png_size(b"GIF89a" + b"\x00" * 32)

    def test_a_truncated_png_is_an_error(self):
        with self.assertRaises(subject.ImageError):
            subject.png_size(png(100, 100)[:20])


class WebpHeader(unittest.TestCase):
    """All three frame types, because which one cwebp emits depends on the source.

    A capture with an alpha channel comes out VP8X, one without comes out VP8; so a parser that
    only knew one of them would work on macOS and raise on Android, or worse, silently read the
    wrong offsets and put a plausible-but-wrong size in the manifest.
    """

    def test_reads_an_extended_vp8x_canvas(self):
        payload = b"\x10\x00\x00\x00" + (1399).to_bytes(3, "little") + (874).to_bytes(3, "little")
        self.assertEqual((1400, 875), subject.webp_size(riff(b"VP8X", payload)))

    def test_reads_a_simple_lossy_vp8_frame(self):
        payload = b"\x00\x00\x00" + b"\x9d\x01\x2a" + struct.pack("<HH", 1400, 875)
        self.assertEqual((1400, 875), subject.webp_size(riff(b"VP8 ", payload)))

    def test_reads_a_lossless_vp8l_frame(self):
        bits = (1400 - 1) | ((875 - 1) << 14)
        payload = b"\x2f" + struct.pack("<I", bits)
        self.assertEqual((1400, 875), subject.webp_size(riff(b"VP8L", payload)))

    def test_a_file_that_is_not_a_webp_is_an_error(self):
        with self.assertRaises(subject.ImageError):
            subject.webp_size(png(10, 10))

    def test_an_unknown_frame_type_is_an_error(self):
        # Better to refuse than to guess offsets: a wrong width here would reach the manifest and
        # the page would reserve the wrong space for every reader.
        with self.assertRaises(subject.ImageError):
            subject.webp_size(riff(b"ANIM", b"\x00" * 16))


class CaptureNames(unittest.TestCase):
    def test_the_locale_and_screen_split_at_the_first_hyphen(self):
        match = subject.CAPTURE_NAME.match("nl-setup-untrusted.png")
        self.assertIsNotNone(match)
        self.assertEqual(("nl", "setup-untrusted"), match.groups())

    def test_names_showcase_sh_does_not_write_are_rejected(self):
        # Each of these would otherwise enter the manifest under a guessed id.
        for name in (
            "setup-email.png",  # no locale
            "en_setup_email.png",  # underscores
            "en-setup-email.webp",  # already encoded
            "eng-setup-email.png",  # not a catalog locale code
            "Screenshot 2026-08-05.png",  # dragged in by hand
        ):
            self.assertIsNone(subject.CAPTURE_NAME.match(name), name)


class EmptyManifest(unittest.TestCase):
    """The refusal that protects a good manifest from an empty capture directory.

    Without it, running this after a `rm -rf showcase-screenshots`; or in a fresh clone; would
    replace every entry with `{}`, and the next gate run would report eight broken pages whose
    actual fault was that nobody had captured anything.

    Every case here runs with **no `cwebp` on PATH**, pinned rather than inherited. An empty
    capture set encodes nothing, so the refusal must be reachable without an encoder; and it was
    not: `build` demanded one up front, so these two tests died on `SystemExit` on any machine
    that had not installed it. That is every CI runner (this file's own docstring says so) and
    every Windows checkout, where it turned `scripts/dev/gate.sh` red. Patching `which` rather
    than relying on the host is what makes the property checkable on a developer's Mac too, where
    `brew install webp` hides it.
    """

    def setUp(self):
        which = unittest.mock.patch.object(subject.shutil, "which", return_value=None)
        which.start()
        self.addCleanup(which.stop)
        self.work = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.work)
        self.captures = self.work / "captures"
        (self.captures / "macos").mkdir(parents=True)
        self.manifest = self.work / "screenshots.json"
        self.manifest.write_text(
            json.dumps({"version": 1, "images": {"setup-email": {}}}), encoding="utf-8"
        )

    def args(self, *extra):
        return [
            "--captures", str(self.captures),
            "--blobs", str(self.work / "blobs"),
            "--manifest", str(self.manifest),
        ] + list(extra)

    def test_an_empty_capture_set_refuses_and_leaves_the_manifest_alone(self):
        self.assertEqual(1, subject.main(self.args()))
        self.assertIn("setup-email", self.manifest.read_text(encoding="utf-8"))

    def test_clearing_the_set_is_possible_but_must_be_asked_for(self):
        self.assertEqual(0, subject.main(self.args("--allow-empty")))
        self.assertEqual({}, json.loads(self.manifest.read_text(encoding="utf-8"))["images"])

    def test_a_missing_capture_directory_is_reported_not_ignored(self):
        shutil.rmtree(self.captures)
        self.assertEqual(1, subject.main(self.args()))


class ManifestShape(unittest.TestCase):
    def test_it_carries_every_field_the_gate_requires(self):
        # scripts/ci/check_user_docs.py reads exactly these; a rename on either side would leave
        # the manifest passing its own writer and failing the gate that consumes it.
        sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ci"))
        import check_user_docs  # noqa: PLC0415

        document = subject.manifest_document(
            {"setup-email": {"macos": {"en": {"sha256": "x", "width": 1, "height": 2, "bytes": 3}}}}
        )
        entry = document["images"]["setup-email"]["macos"]["en"]
        for field in check_user_docs.MANIFEST_FIELDS:
            self.assertIn(field, entry)

    def test_it_is_written_deterministically(self):
        # Re-running on unchanged captures must produce a byte-identical file, or `--check` would
        # report drift that isn't there and the release gate would cry wolf.
        images = {"b": {"macos": {"en": {}}}, "a": {"macos": {"en": {}}}}
        first = subject.serialize(subject.manifest_document(images))
        second = subject.serialize(subject.manifest_document(dict(reversed(list(images.items())))))
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
