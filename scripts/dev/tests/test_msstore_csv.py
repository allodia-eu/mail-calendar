#!/usr/bin/env python3
"""Unit tests for filling a Partner Center listing export.

The failure this tool can cause is not a crash; it is a **CSV that imports cleanly and says the
wrong thing**. Three shapes of that, each with a test here: a row it does not own coming back
changed (the screenshot URLs Partner Center minted are in this file, and losing one silently drops
an image from a published listing); a shortened list leaving its old tail behind under the new
items; and a language written into the column next door. None of those would look like an error at
import time, so each is asserted rather than assumed.

The fixture document is `test_msstore_payload.py`'s, so an edit to the real store copy never turns
these red; and the fixture export is built here, small but shaped exactly like the real one.
"""

from __future__ import annotations

import csv
import io
import contextlib
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import msstore_csv as subject  # noqa: E402
from check_store_copy_length import listing_promises_per_store_fields  # noqa: E402
from test_msstore_payload import listing_doc  # noqa: E402

# Every "the real copy is complete" assertion below is a claim about a **branded** listing. An
# unbranded checkout -- the public repository's own shape -- resolves to
# `branding/default-listing.md`, which carries one language and no per-store fields by design, so
# tooling refusing it is that tooling working rather than a regression. Those tests skip, and say
# so: a skip that reads like a pass is the failure this repo keeps warning about.
BRANDED_LISTING = listing_promises_per_store_fields()
NEEDS_A_BRANDED_LISTING = (
    "the resolved listing carries no per-store fields (branding/default-listing.md): "
    "there is nothing for a console push to push"
)


# Shaped like the URLs Partner Center mints for an uploaded screenshot, with the ids replaced:
# a real product id in a fixture is one grep away from being mistaken for the id to use.
SHOT_URL = (
    "https://developer.microsoft.com/en-us/dashboard/apps/0PRODUCTID00/submissions/"
    "1000000000000000000/listings/1000000000000000001/listingassets/1000000000000000002"
)


def export_csv(*, languages=("en", "nl"), features=6, terms=4, extra_rows=()) -> str:
    """A miniature listingData export, in Partner Center's own dialect."""
    blank = "," * len(languages)
    rows = [
        "Field,ID,Type (Type),default," + ",".join(languages),
        f"Description,2,Text,{blank}",
        f"ReleaseNotes,3,Text,{blank}",
        "Title,4,Text," + "," + ",".join(["Allodia Mail & Calendar"] * len(languages)),
        f"ShortTitle,5,Text,{blank}",
        f"ShortDescription,8,Text,{blank}",
        f"CopyrightTrademarkInformation,12,Text,{blank}",
        "DesktopScreenshot1,100,Relative path (or URL to file in Partner Center),,"
        + ",".join([SHOT_URL] * len(languages)),
    ]
    rows.extend(
        f"Feature{number},{699 + number},Text,{blank}" for number in range(1, features + 1)
    )
    rows.extend(f"SearchTerm{number},{899 + number},Text,{blank}" for number in range(1, terms + 1))
    rows.extend(extra_rows)
    return "\r\n".join(rows) + "\r\n"


def write_raw(path: Path, text: str) -> None:
    """Write CSV text exactly as given, with the export's byte-order mark.

    Bytes, not `Path.write_text`: on Windows that translates every LF, so the fixture's CRLF row
    terminators would become blank rows and these tests would be exercising a dialect Partner
    Center never emits.
    """
    path.write_bytes(b"\xef\xbb\xbf" + text.encode("utf-8"))


class Filling(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.listing = self.root / "store-listing.md"
        self.listing.write_text(listing_doc(), encoding="utf-8")
        self.export = self.root / "listingData-X.csv"
        write_raw(self.export, export_csv())
        self.out = self.root / "filled.csv"

    def run_cli(self, *argv, export=None):
        return subject.main(
            [
                str(export or self.export),
                "--listing",
                str(self.listing),
                "-o",
                str(self.out),
                *argv,
            ]
        )

    def filled(self, path=None):
        """`{row name: {language: cell}}` from a written export."""
        with (path or self.out).open(newline="", encoding="utf-8-sig") as handle:
            rows = list(csv.reader(handle))
        header = rows[0]
        languages = {header[index]: index for index in range(4, len(header))}
        return {
            row[0]: {name: row[index] for name, index in languages.items()}
            for row in rows[1:]
            if row
        }

    # -- what it writes ------------------------------------------------------------------------

    def test_it_fills_every_owned_field_per_language(self) -> None:
        self.assertEqual(self.run_cli("-l", "en", "-l", "nl"), 0)
        filled = self.filled()
        self.assertEqual(filled["Description"]["en"], "Sovereign mail.")
        self.assertEqual(filled["Description"]["nl"], "Soevereine mail.")
        self.assertEqual(filled["Feature1"]["nl"], "Eén functie")
        self.assertEqual(filled["SearchTerm2"]["en"], "JMAP email client")
        self.assertEqual(filled["CopyrightTrademarkInformation"]["en"], "© 2026 Allodia")

    def test_a_language_not_asked_for_is_left_alone(self) -> None:
        self.run_cli("-l", "en")
        self.assertEqual(self.filled()["Description"]["nl"], "")

    def test_the_export_dialect_survives_the_round_trip(self) -> None:
        # BOM, CRLF between rows, bare LF inside a quoted cell: an import reads all three.
        self.listing.write_text(listing_doc(english="Two\nlines."), encoding="utf-8")
        self.run_cli("-l", "en")
        raw = self.out.read_bytes()
        self.assertTrue(raw.startswith(b"\xef\xbb\xbf"))
        self.assertIn(b'"Two\nlines."', raw)
        self.assertNotIn(b"\r\n\r\n", raw)

    # -- what it must not touch ----------------------------------------------------------------

    def test_rows_it_does_not_own_come_back_byte_identical(self) -> None:
        # The screenshot cells hold URLs Partner Center minted; re-authoring one drops an image
        # from a published listing, and the import would report nothing wrong.
        self.run_cli("-l", "en", "-l", "nl")
        before = self.export.read_text(encoding="utf-8-sig").splitlines()
        after = self.out.read_text(encoding="utf-8-sig").splitlines()
        owned = ("Description,", "ReleaseNotes,", "CopyrightTrademarkInformation,", "Feature",
                 "SearchTerm")
        for original, written in zip(before, after):
            if original.startswith(owned):
                continue
            self.assertEqual(original, written)

    def test_release_notes_are_only_written_when_asked(self) -> None:
        self.run_cli("-l", "en")
        self.assertEqual(self.filled()["ReleaseNotes"]["en"], "")

    def test_the_default_output_sits_beside_the_input_not_on_it(self) -> None:
        original = self.export.read_bytes()
        self.assertEqual(
            subject.main([str(self.export), "--listing", str(self.listing), "-l", "en"]), 0
        )
        self.assertEqual(self.export.read_bytes(), original)
        self.assertTrue((self.root / "listingData-X-filled.csv").is_file())

    # -- the tail ------------------------------------------------------------------------------

    def test_a_shortened_list_clears_the_slots_it_no_longer_fills(self) -> None:
        # Otherwise the export keeps a feature nobody wrote, sitting under the ones somebody did.
        write_raw(self.export, export_csv())
        stale = list(csv.reader(io.StringIO(self.export.read_text(encoding="utf-8-sig"))))
        for row in stale:
            if row and row[0] in ("Feature5", "SearchTerm4"):
                row[4] = "left over from last time"
        with self.export.open("w", newline="", encoding="utf-8-sig") as handle:
            csv.writer(handle, lineterminator="\r\n").writerows(stale)

        self.run_cli("-l", "en")
        filled = self.filled()
        self.assertEqual(filled["Feature5"]["en"], "")
        self.assertEqual(filled["SearchTerm4"]["en"], "")

    # -- refusals ------------------------------------------------------------------------------

    def test_copy_that_overruns_a_field_writes_no_file(self) -> None:
        self.listing.write_text(listing_doc(english_features="x" * 201), encoding="utf-8")
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(self.run_cli("-l", "en"), 1)
        self.assertFalse(self.out.exists())
        self.assertIn("does not fit", errors.getvalue())

    def test_a_language_the_export_has_no_column_for_is_named(self) -> None:
        write_raw(self.export, export_csv(languages=("en",)))
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(self.run_cli("-l", "en", "-l", "nl"), 1)
        self.assertIn("no column for nl", errors.getvalue())
        self.assertFalse(self.out.exists())

    def test_a_file_that_is_not_an_export_is_refused_before_it_is_edited(self) -> None:
        other = self.root / "something-else.csv"
        other.write_text("a,b,c\r\n1,2,3\r\n", encoding="utf-8")
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(self.run_cli("-l", "en", export=other), 1)
        self.assertIn("does not look like a Partner Center listing export", errors.getvalue())

    def test_more_items_than_slots_is_an_error_not_a_silent_truncation(self) -> None:
        write_raw(self.export, export_csv(features=1))
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(self.run_cli("-l", "en"), 1)
        self.assertIn("only 1 Feature rows", errors.getvalue())

    def test_a_reworded_document_exits_two_rather_than_filling_less(self) -> None:
        self.listing.write_text(
            listing_doc().replace("## Shared description — English", "## The description"),
            encoding="utf-8",
        )
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(self.run_cli("-l", "en"), 2)

    # -- column resolution ---------------------------------------------------------------------

    def test_a_regional_column_is_matched_by_its_language(self) -> None:
        # Partner Center labels a column `en` or `en-us` depending on when the listing was made.
        write_raw(self.export, export_csv(languages=("en-us", "nl-nl")))
        self.run_cli("-l", "en", "-l", "nl")
        filled = self.filled()
        self.assertEqual(filled["Description"]["en-us"], "Sovereign mail.")
        self.assertEqual(filled["Description"]["nl-nl"], "Soevereine mail.")

    def test_an_exact_column_beats_a_regional_one(self) -> None:
        write_raw(self.export, export_csv(languages=("en-gb", "en", "nl")))
        self.run_cli("-l", "en", "-l", "nl")
        filled = self.filled()
        self.assertEqual(filled["Description"]["en"], "Sovereign mail.")
        self.assertEqual(filled["Description"]["en-gb"], "")


@unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
class RealDocument(unittest.TestCase):
    """The one property a fixture cannot prove: the shipped copy fills the shipped export."""

    def test_every_catalog_locale_has_a_store_language(self) -> None:
        for locale in subject.resolve_locales():
            self.assertRegex(locale, r"^[a-z]{2}$")


if __name__ == "__main__":
    unittest.main()
