"""Tests which brand file `brand.py` reads, the Python half of the rule in `docs/branding.md`.

`branding/brand.env` is the publisher's slot and `branding/allodia.env` the older name it is still
read under, so a pipeline copying either builds branded, and the new name answers when both are
present. The file slots (the icon and the listing) resolve by the same order.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import brand  # noqa: E402  (after the path insert, deliberately)


class BrandFileOrder(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp())
        (self.dir / "default.env").write_text(
            'MAILCAL_APP_NAME="Neutral"\nMAILCAL_APP_ID="org.neutral.client"\n', encoding="utf-8"
        )
        patches = [
            mock.patch.object(brand, "BRAND_DIR", self.dir),
            mock.patch.object(brand, "DEFAULT_FILE", self.dir / "default.env"),
            mock.patch.object(
                brand, "OVERRIDE_FILES", tuple(self.dir / f"{name}.env" for name in brand.BRANDS)
            ),
        ]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)

    def name(self) -> str:
        return brand.resolve({})["MAILCAL_APP_NAME"]

    def test_the_default_answers_alone(self) -> None:
        self.assertEqual(self.name(), "Neutral")

    def test_the_older_name_still_brands(self) -> None:
        (self.dir / "allodia.env").write_text('MAILCAL_APP_NAME="Older"\n', encoding="utf-8")
        self.assertEqual(self.name(), "Older")

    def test_the_brand_file_wins_over_the_older_name(self) -> None:
        (self.dir / "allodia.env").write_text('MAILCAL_APP_NAME="Older"\n', encoding="utf-8")
        (self.dir / "brand.env").write_text('MAILCAL_APP_NAME="Branded"\n', encoding="utf-8")
        self.assertEqual(self.name(), "Branded")
        self.assertEqual(
            brand.resolve({})["MAILCAL_APP_ID"],
            "org.neutral.client",
            "a brand file naming only some keys leaves the rest on the default",
        )

    def test_a_file_slot_follows_the_same_order(self) -> None:
        self.assertEqual(brand.icon_source(), self.dir / "default-icon.png")
        (self.dir / "allodia-icon.png").write_bytes(b"")
        self.assertEqual(brand.icon_source(), self.dir / "allodia-icon.png")
        (self.dir / "brand-icon.png").write_bytes(b"")
        self.assertEqual(brand.icon_source(), self.dir / "brand-icon.png")


if __name__ == "__main__":
    unittest.main()
