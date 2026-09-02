#!/usr/bin/env python3
"""Unit tests for the readers every store publisher shares.

What is asserted here is that one document yields one answer: the same description body, the same
product name, the same gallery order, whichever console is about to be handed it. The failure this
guards is not a crash but a **divergence**, and a divergence between two publishers is invisible
until someone opens two store pages side by side and reads them.

The `{KEYSTORE}` substitution is the exception that proves the rule, and so it is the one thing
tested per store: the same sentence has to say "the Windows Credential Manager" on one console and
"your device's Keychain" on another, so `descriptions` takes the store rather than defaulting to
one. A default here would paste one store's word into another's listing, silently.

The fixtures are miniature documents shaped like the real one; the same discipline as
`scripts/ci/tests/test_store_copy_length.py`; so an edit to the actual store copy never turns these
red, while a change to the document's *shape* always does.
"""

from __future__ import annotations

import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

from changelog_fragments import DocumentShapeError  # noqa: E402

import brand  # noqa: E402
import store_payload as subject  # noqa: E402

KEYSTORE_TABLE = """
### `{KEYSTORE}` token — the one substitution in the shared body

| Store (platform) | English | Nederlands |
|---|---|---|
| Microsoft Store (Windows) | the Windows Credential Manager | Windows Referentiebeheer |
| App Store Connect (Apple) | your device's Keychain | de Keychain van je apparaat |
| Google Play (Android) | the Android Keystore | de Android Keystore |
"""


def listing_doc(
    *,
    english="Sovereign mail.",
    dutch="Soevereine mail.",
    keystore_table="",
    copyright_row="| Copyright | `© 2026 Allodia` |",
) -> str:
    """A miniature store-listing.md carrying only what every publisher reads."""
    return f"""# App-store listing

## Shared description — English

```
{english}
```

## Shared description — Nederlands

```
{dutch}
```

## Per-store fields
{keystore_table}
### Product name / title (every store, every language)

```
Allodia Mail & Calendar
```

## Console-side metadata (per store)

### Shared fields (identical on every store)

| Field | Value |
|---|---|
| Category | **Productivity** |
{copyright_row}
"""


def png(width: int, height: int) -> bytes:
    """The smallest valid-enough PNG: a real signature and IHDR, which is all we parse."""
    ihdr = struct.pack(">II", width, height) + bytes([8, 6, 0, 0, 0])
    chunk = struct.pack(">I", len(ihdr)) + b"IHDR" + ihdr
    chunk += struct.pack(">I", zlib.crc32(b"IHDR" + ihdr) & 0xFFFFFFFF)
    return subject.PNG_MAGIC + chunk


class DocumentScraping(unittest.TestCase):
    def test_a_body_is_keyed_to_the_language_that_wrote_it(self) -> None:
        bodies = subject.descriptions(listing_doc(), "Microsoft Store")
        self.assertEqual(bodies["English"], "Sovereign mail.")
        self.assertEqual(bodies["Nederlands"], "Soevereine mail.")

    def test_a_document_with_no_description_section_is_an_error(self) -> None:
        with self.assertRaises(DocumentShapeError):
            subject.descriptions("# Nothing here\n", "Microsoft Store")

    def test_the_product_name_comes_from_the_brand_not_the_copy(self) -> None:
        # The launcher, the installer and every listing must agree about what the app is called,
        # and a name written twice is a name that can disagree with the one the OS shows.
        self.assertEqual(subject.product_name(listing_doc()), brand.value("MAILCAL_APP_NAME"))

    def test_the_copyright_row_is_read_out_of_the_shared_table(self) -> None:
        self.assertEqual(subject.copyright_line(listing_doc()), "© 2026 Allodia")

    def test_a_missing_copyright_row_is_an_error(self) -> None:
        with self.assertRaises(DocumentShapeError):
            subject.copyright_line(listing_doc(copyright_row=""))


class TheKeystoreToken(unittest.TestCase):
    def test_each_store_is_handed_its_own_word(self) -> None:
        # The whole reason `descriptions` takes a store: one body, three consoles, three words.
        document = listing_doc(
            english="Credentials live in {KEYSTORE}.", keystore_table=KEYSTORE_TABLE
        )
        for store, expected in (
            ("Microsoft Store", "the Windows Credential Manager"),
            ("App Store Connect", "your device's Keychain"),
            ("Google Play", "the Android Keystore"),
        ):
            with self.subTest(store=store):
                bodies = subject.descriptions(document, store)
                self.assertEqual(bodies["English"], f"Credentials live in {expected}.")

    def test_a_token_with_no_table_is_an_error_rather_than_a_literal_paste(self) -> None:
        # A console showing the characters "{KEYSTORE}" to a shopper is the failure being refused.
        with self.assertRaises(DocumentShapeError):
            subject.descriptions(listing_doc(english="Credentials live in {KEYSTORE}."), "Google Play")

    def test_a_store_the_table_does_not_cover_is_named(self) -> None:
        document = listing_doc(
            english="Credentials live in {KEYSTORE}.", keystore_table=KEYSTORE_TABLE
        )
        with self.assertRaises(DocumentShapeError) as caught:
            subject.descriptions(document, "Amazon Appstore")
        self.assertIn("{KEYSTORE}", str(caught.exception))


class Screenshots(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)

    def put(self, name: str, size=(2880, 1800)) -> None:
        (self.dir / name).write_bytes(png(*size))

    def test_captures_are_grouped_by_locale_and_ordered_hero_first(self) -> None:
        for name in ("en-add-account.png", "en-list.png", "en-calendar.png", "nl-list.png"):
            self.put(name)
        found, skipped = subject.collect_screenshots(self.dir, ("en", "nl"))
        self.assertEqual([shot.screen for shot in found["en"]], ["list", "calendar", "add-account"])
        self.assertEqual([shot.screen for shot in found["nl"]], ["list"])
        self.assertEqual(skipped, [])

    def test_an_unknown_screen_sorts_after_the_known_ones_rather_than_vanishing(self) -> None:
        self.put("en-list.png")
        self.put("en-zebra.png")
        self.put("en-aardvark.png")
        found, _ = subject.collect_screenshots(self.dir, ("en",))
        self.assertEqual([shot.screen for shot in found["en"]], ["list", "aardvark", "zebra"])

    def test_a_file_that_does_not_match_the_convention_is_reported_not_ignored(self) -> None:
        self.put("en-list.png")
        self.put("screenshot final FINAL.png")
        _, skipped = subject.collect_screenshots(self.dir, ("en",))
        self.assertEqual(skipped, ["screenshot final FINAL.png"])

    def test_a_locale_outside_the_run_is_skipped_rather_than_pushed(self) -> None:
        self.put("nl-list.png")
        found, skipped = subject.collect_screenshots(self.dir, ("en",))
        self.assertEqual(found["en"], ())
        self.assertEqual(skipped, ["nl-list.png"])

    def test_the_dimensions_come_from_the_header(self) -> None:
        # Every store states its own pixel bounds, so the reader is shared and the bound is not.
        self.put("en-list.png", size=(1366, 768))
        self.assertEqual(subject.png_dimensions(self.dir / "en-list.png"), (1366, 768))

    def test_a_non_png_is_refused_by_its_header_not_its_extension(self) -> None:
        (self.dir / "en-list.png").write_bytes(b"GIF89a not a png at all")
        with self.assertRaises(subject.ListingError):
            subject.collect_screenshots(self.dir, ("en",))

    def test_a_missing_directory_is_a_listing_error_not_a_document_error(self) -> None:
        # Different exit code, different advice: "fix the path", never "fix the scraper".
        with self.assertRaises(subject.ListingError):
            subject.collect_screenshots(self.dir / "nope", ("en",))
        self.assertNotIsInstance(subject.ListingError("x"), DocumentShapeError)


if __name__ == "__main__":
    unittest.main()
