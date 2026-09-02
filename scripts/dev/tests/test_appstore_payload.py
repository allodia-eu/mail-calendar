#!/usr/bin/env python3
"""Unit tests for the macOS App Store listing payload.

The risk here is the same one `test_store_payload.py` opens with, sharpened by how Apple fails: a
push to a locale the store does not offer, or a gallery uploaded in the wrong order, **reports
success**. So most of what is asserted is that the tool refuses, that a language's copy is keyed to
that language, and that the two orderings a human cannot see; the locale codes and the gallery
sequence; come out exactly as intended.

The fixtures are miniature documents shaped like the real one, so editing the actual store copy
never turns these red while a change to the document's *shape* always does. Three tests deliberately
read the real resolved listing and `docs/changelog/released/`: that every catalog language is
carried, that the copyright agrees with the Microsoft Store's reading of the same table, and that a
Mac release note is the macOS section rather than the iOS one; none of which a fixture can prove.
"""

from __future__ import annotations

import json
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

from changelog_fragments import DocumentShapeError, catalog_locales, parse_release  # noqa: E402
from check_store_copy_length import LIMITS_PATH, listing_path, parse_limits  # noqa: E402

import appstore_listing as cli  # noqa: E402
import brand  # noqa: E402
import appstore_payload as subject  # noqa: E402
import store_payload  # noqa: E402
from check_store_copy_length import listing_promises_per_store_fields  # noqa: E402

# Every "the real copy is complete" assertion below is a claim about a **branded** listing. An
# unbranded checkout -- the public repository's own shape -- resolves to
# `branding/default-listing.md`, which carries one language and no per-store fields by design, so a
# push refusing it is the tooling working rather than a regression. Those tests skip, and say so:
# a skip that reads like a pass is the failure this repo keeps warning about.
BRANDED_LISTING = listing_promises_per_store_fields()
NEEDS_A_BRANDED_LISTING = (
    "the resolved listing carries no per-store fields (branding/default-listing.md): "
    "there is nothing for a console push to push"
)

LIMITS_TABLE = """## Field limits

| Store | Name/Title | Subtitle | Short/Promo | Description | Feature list | Search terms | What's new |
|---|---|---|---|---|---|---|---|
| Microsoft Store | 256 | — | — | 10,000 | up to 20 × ~200 | up to 7 × 30 (21 words) | 1,500 |
| App Store Connect | 30 | 30 | 170 (Promo) | 4,000 | — | Keywords: 100 | 4,000 |
| Google Play | 30 | — | 80 (Short desc) | 4,000 | — | — | 500 (release notes) |
"""

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
    english="Sovereign mail, keys in {KEYSTORE}.",
    dutch="Soevereine mail, sleutels in {KEYSTORE}.",
    english_fields="Subtitle:        Sovereign email & calendar\n"
    "Promotional:     Your mailbox, on your terms.\n"
    "Keywords:        email,mail,calendar,IMAP",
    dutch_fields="Subtitle:        Soevereine mail en agenda\n"
    "Promotional:     Jouw mailbox, op jouw voorwaarden.\n"
    "Keywords:        e-mail,mail,agenda,IMAP",
    keystore_table=KEYSTORE_TABLE,
    support_row="| Support URL | `https://allodia.eu/contact` |",
) -> str:
    """A miniature store-listing.md carrying two languages, shaped exactly like the real one."""
    return f"""# App-store listing

{LIMITS_TABLE}

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

### App Store Connect — Subtitle (≤30), Promotional text (≤170), Keywords (≤100)

**English**
```
{english_fields}
```

**Nederlands**
```
{dutch_fields}
```

## Console-side metadata (per store)

### Shared fields (identical on every store)

| Field | Value |
|---|---|
| Category | **Productivity** |
{support_row}
| Marketing / website URL | `https://allodia.eu` |
| Privacy policy URL | `https://allodia.eu/privacy/mail-calendar` |
| Copyright | `© 2026 Allodia` |
"""


def png(width: int, height: int) -> bytes:
    """The smallest valid-enough PNG: a real signature and IHDR, which is all we parse."""
    ihdr = struct.pack(">II", width, height) + bytes([8, 6, 0, 0, 0])
    chunk = struct.pack(">I", len(ihdr)) + b"IHDR" + ihdr
    chunk += struct.pack(">I", zlib.crc32(b"IHDR" + ihdr) & 0xFFFFFFFF)
    return store_payload.PNG_MAGIC + chunk


class Fixture(unittest.TestCase):
    """A temp directory holding a miniature contract, plus a place to write captures."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.path = self.root / "store-listing.md"

    def write(self, **kwargs) -> str:
        self.path.write_text(listing_doc(**kwargs), encoding="utf-8")
        return str(self.path)

    def captures(self, names, size=(2880, 1800), device="macos") -> Path:
        directory = self.root / device
        directory.mkdir(exist_ok=True)
        for name in names:
            (directory / name).write_bytes(png(*size))
        return directory


class DocumentScraping(Fixture):
    def test_a_language_becomes_its_app_store_locale(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("en", "nl"))
        self.assertEqual([item.store_locale for item in built], ["en-US", "nl-NL"])
        self.assertEqual([item.language for item in built], ["English", "Nederlands"])

    def test_italian_is_it_because_app_store_connect_has_no_it_IT(self) -> None:
        # The whole reason the map is written down. A push to a locale the store does not offer
        # writes nothing and raises nothing, so `it-IT` would leave Italy on the previous copy while
        # the run reported seven languages covered.
        self.assertEqual(subject.APP_STORE_LOCALES["it"], "it")

    def test_the_short_fields_come_from_the_matching_language(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("nl",))[0]
        self.assertEqual(built.subtitle, "Soevereine mail en agenda")
        self.assertEqual(built.promotional, "Jouw mailbox, op jouw voorwaarden.")
        self.assertEqual(built.keywords, "e-mail,mail,agenda,IMAP")

    def test_a_block_missing_a_field_is_refused_rather_than_pushed_short(self) -> None:
        with self.assertRaises(DocumentShapeError) as raised:
            subject.load_listings(
                listing_md=self.write(dutch_fields="Subtitle:  Soevereine mail en agenda"),
                locales=("nl",),
            )
        self.assertIn("Promotional", str(raised.exception))

    def test_the_keystore_token_is_substituted_with_apples_word_not_windows(self) -> None:
        # The one thing about a description that is NOT shared between stores. Reusing the
        # Microsoft Store's reader without saying which store is asking put "Windows
        # Referentiebeheer" into the App Store's Dutch body; caught here, before a push.
        built = subject.load_listings(listing_md=self.write(), locales=("nl",))[0]
        self.assertIn("de Keychain van je apparaat", built.description)
        self.assertNotIn("Referentiebeheer", built.description)
        self.assertNotIn("{KEYSTORE}", built.description)

    def test_a_language_with_no_apple_token_is_refused_rather_than_pushed_with_the_token(
        self,
    ) -> None:
        with self.assertRaises(DocumentShapeError) as raised:
            subject.load_listings(listing_md=self.write(keystore_table=""), locales=("en",))
        self.assertIn("App Store Connect", str(raised.exception))

    def test_the_urls_come_from_the_shared_fields_table(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("en",))[0]
        self.assertEqual(built.support_url, "https://allodia.eu/contact")
        self.assertEqual(built.marketing_url, "https://allodia.eu")
        self.assertEqual(built.privacy_policy_url, "https://allodia.eu/privacy/mail-calendar")

    def test_a_locale_with_its_own_privacy_page_keeps_it(self) -> None:
        # App Store Connect stores this per locale, and Dutch already had a Dutch page. A push that
        # wrote the shared URL everywhere would send those readers to English text; a downgrade
        # that looks like a successful run.
        doc = listing_doc().replace(
            "| Copyright | `© 2026 Allodia` |",
            "| Copyright | `© 2026 Allodia` |\n\n| Catalog locale | URL |\n|---|---|\n"
            "| `nl` | `https://allodia.eu/nl/privacy/mail-calendar` |",
        )
        self.path.write_text(doc, encoding="utf-8")
        built = {
            item.locale: item.privacy_policy_url
            for item in subject.load_listings(listing_md=str(self.path), locales=("en", "nl"))
        }
        self.assertEqual(built["nl"], "https://allodia.eu/nl/privacy/mail-calendar")
        self.assertEqual(built["en"], "https://allodia.eu/privacy/mail-calendar")

    def test_a_missing_url_row_is_refused(self) -> None:
        with self.assertRaises(DocumentShapeError) as raised:
            subject.shared_fields(listing_doc(support_row=""))
        self.assertIn("support url", str(raised.exception))

    def test_a_locale_with_no_app_store_code_is_a_hard_error(self) -> None:
        with self.assertRaises(DocumentShapeError) as raised:
            subject.resolve_locales(("en", "sv"))
        self.assertIn("sv", str(raised.exception))


class Copyright(Fixture):
    def test_apple_gets_the_copyright_without_its_symbol(self) -> None:
        # Apple renders the © itself; handing it the symbol reads "© © 2026 Allodia" once live.
        self.assertEqual(
            cli.apple_copyright(subject.shared_fields(listing_doc())), "2026 Allodia"
        )

    @unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
    def test_both_stores_read_the_same_copyright_row(self) -> None:
        # The one row two modules parse. This turns the overlap into a check: if either scraper
        # drifts, they stop agreeing here rather than in two different consoles.
        text = listing_path().read_text(encoding="utf-8")
        self.assertEqual(
            subject.shared_fields(text)["copyright"], store_payload.copyright_line(text)
        )


MAC_SLOT = subject.SCREENSHOT_SLOTS["MAC_OS"][0]
IPHONE_SLOT, IPAD_SLOT = subject.SCREENSHOT_SLOTS["IOS"]


def _slot_sizes(device_type):
    """The sizes Apple takes for `device_type`, for a test asserting one is *absent*."""
    return {
        (1242, 2688), (1284, 2778), (2688, 1242), (2778, 1284)
    } if device_type == "APP_IPHONE_65" else ()


class Measuring(Fixture):
    def setUp(self) -> None:
        super().setUp()
        self.limits = parse_limits(listing_doc())

    def test_an_over_long_subtitle_is_refused(self) -> None:
        built = subject.load_listings(
            listing_md=self.write(
                english_fields="Subtitle:        A subtitle far past Apple's thirty characters\n"
                "Promotional:     Fine.\nKeywords:        email"
            ),
            locales=("en",),
        )[0]
        problems = subject.measure(built, self.limits)
        self.assertTrue(any("subtitle is 45 characters, limit 30" in item for item in problems))

    def gallery(self, names, size=(2880, 1800), device="macos"):
        """One built listing carrying `names` as its gallery."""
        collected, _ = subject.collect_screenshots(self.captures(names, size, device), ("en",))
        return subject.load_listings(
            listing_md=self.write(), locales=("en",), screenshots=collected
        )[0]

    def test_an_off_size_screenshot_is_refused_before_any_upload_starts(self) -> None:
        # Apple takes a fixed set of sizes per slot, not a range: an off-size PNG fails partway
        # through a locale and leaves that gallery half replaced.
        built = self.gallery(["en-list.png"], (1920, 1080))
        problems = subject.measure_screenshots(built, MAC_SLOT)
        self.assertTrue(any("1920x1080" in item and "APP_DESKTOP" in item for item in problems))

    def test_a_store_valid_size_passes(self) -> None:
        built = self.gallery(["en-list.png"])
        self.assertEqual(subject.measure_screenshots(built, MAC_SLOT), [])

    def test_more_screenshots_than_apple_takes_is_refused(self) -> None:
        names = [f"en-screen{number}.png" for number in range(subject.MAX_SCREENSHOTS + 1)]
        built = self.gallery(names)
        self.assertTrue(
            any("at most 10" in item for item in subject.measure_screenshots(built, MAC_SLOT))
        )

    def test_a_69_inch_iphone_capture_belongs_to_the_67_slot(self) -> None:
        # The trap this table exists for. A 6.9" capture is 1320x2868, which APP_IPHONE_65 does not
        # take at any size; pointing the iPhone gallery at 65 fails on every file, at upload.
        built = self.gallery(["en-list.png"], (1320, 2868), "iphone")
        self.assertEqual(subject.measure_screenshots(built, IPHONE_SLOT), [])
        self.assertNotIn((1320, 2868), dict.fromkeys(_slot_sizes("APP_IPHONE_65")))

    def test_a_13_inch_ipad_capture_belongs_to_the_129_slot(self) -> None:
        built = self.gallery(["en-list.png"], (2064, 2752), "ipad")
        self.assertEqual(subject.measure_screenshots(built, IPAD_SLOT), [])

    def test_an_iphone_capture_is_refused_by_the_ipad_slot(self) -> None:
        # Each slot is measured against its own sizes, so a directory handed to the wrong gallery
        # is caught here rather than partway through an upload.
        built = self.gallery(["en-list.png"], (1320, 2868), "iphone")
        problems = subject.measure_screenshots(built, IPAD_SLOT)
        self.assertTrue(any("APP_IPAD_PRO_3GEN_129" in item for item in problems))


class WritingWhatAscReads(Fixture):
    def test_the_canonical_tree_splits_app_fields_from_version_fields(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("en", "nl"))
        subject.write_metadata(built, self.root / "metadata", "0.3.0")
        app_info = json.loads(
            (self.root / "metadata" / "app-info" / "nl-NL.json").read_text(encoding="utf-8")
        )
        version = json.loads(
            (self.root / "metadata" / "version" / "0.3.0" / "nl-NL.json").read_text(encoding="utf-8")
        )
        self.assertEqual(app_info["subtitle"], "Soevereine mail en agenda")
        self.assertEqual(app_info["privacyPolicyUrl"], "https://allodia.eu/privacy/mail-calendar")
        self.assertNotIn("description", app_info)
        self.assertIn("de Keychain van je apparaat", version["description"])
        self.assertEqual(version["keywords"], "e-mail,mail,agenda,IMAP")

    def test_whats_new_is_omitted_rather_than_written_empty(self) -> None:
        # An empty string is a value: pushing it would erase whatever note is already there.
        built = subject.load_listings(listing_md=self.write(), locales=("en",))
        subject.write_metadata(built, self.root / "metadata", "0.3.0")
        version = json.loads(
            (self.root / "metadata" / "version" / "0.3.0" / "en-US.json").read_text(encoding="utf-8")
        )
        self.assertNotIn("whatsNew", version)

    def test_only_the_locales_in_this_run_get_a_file(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("nl",))
        subject.write_metadata(built, self.root / "metadata", "0.3.0")
        written = sorted(path.name for path in (self.root / "metadata" / "app-info").iterdir())
        self.assertEqual(written, ["nl-NL.json"])


class StagingTheGallery(Fixture):
    def test_the_staged_names_sort_into_the_gallery_order(self) -> None:
        # Apple orders a gallery by upload order, and `asc` uploads a locale's files sorted by name.
        # Without the numeric prefix the listing would open on `add-account`.
        names = ["en-add-account.png", "en-calendar.png", "en-list.png"]
        galleries, _ = subject.collect_screenshots(self.captures(names), ("en",))
        built = subject.load_listings(
            listing_md=self.write(), locales=("en",), screenshots=galleries
        )
        staged = subject.stage_screenshots(built, self.root / "staged")
        ordered = sorted(path.name for path in staged["en-US"].iterdir())
        self.assertEqual(
            [name.split("-", 1)[1] for name in ordered],
            ["en-list.png", "en-calendar.png", "en-add-account.png"],
        )

    def test_a_locale_becomes_a_directory_named_for_the_store(self) -> None:
        galleries, _ = subject.collect_screenshots(self.captures(["nl-list.png"]), ("nl",))
        built = subject.load_listings(
            listing_md=self.write(), locales=("nl",), screenshots=galleries
        )
        staged = subject.stage_screenshots(built, self.root / "staged")
        self.assertEqual(list(staged), ["nl-NL"])
        # A real file, not a link: `asc` refuses to read a symlink it was pointed at.
        staged_file = self.root / "staged" / "nl-NL" / "01-nl-list.png"
        self.assertTrue(staged_file.is_file())
        self.assertFalse(staged_file.is_symlink())


@unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
class AgainstTheRealDocuments(unittest.TestCase):
    def test_every_catalog_language_has_app_store_copy(self) -> None:
        # The property a fixture cannot prove: adding a language to the catalog must make this fail
        # until its store translation is written.
        built = subject.load_listings()
        self.assertEqual(len(built), len(catalog_locales()))
        for listing in built:
            self.assertTrue(listing.subtitle and listing.promotional and listing.keywords)

    def test_a_mac_release_note_is_the_macos_section_not_the_ios_one(self) -> None:
        # Both platforms are pasted into the same store, but they are separate *versions* there,
        # `store_targets` would hand the Mac an iOS note describing a build it is not shipping.
        version = (REPO_ROOT / "VERSION").read_text(encoding="utf-8").strip()
        path = REPO_ROOT / "docs" / "changelog" / "released" / f"{version}.md"
        sections = {
            platform: section
            for section in parse_release(path.read_text(encoding="utf-8"), version)
            for platform in section.platforms
        }
        notes = subject.release_notes(version)
        self.assertEqual(notes["English"], sections["macos"].notes["English"])
        if "ios" in sections:
            self.assertNotEqual(notes["English"], sections["ios"].notes["English"])

    def test_the_bundle_id_is_the_one_the_build_is_branded_with(self) -> None:
        # Not a literal: pushing this app's copy onto a different app's listing is not a mistake
        # review would catch, so what matters is that this and project.yml read one source.
        self.assertEqual(cli.bundle_id(), brand.value("MAILCAL_APP_ID"))


if __name__ == "__main__":
    unittest.main()
