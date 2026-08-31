#!/usr/bin/env python3
"""Unit tests for the Microsoft Store listing payload.

The risk this tool carries is not a crash; it is a **quiet wrong paste**: a language whose copy
silently didn't change, a gallery that replaced the wrong listing, a field that overran a console
limit and was written anyway. So most of what is asserted here is that the tool *refuses*, and that
what it builds is keyed to the language a reader would expect.

The fixtures are miniature documents shaped like the real ones; the same discipline as
`scripts/ci/tests/test_store_copy_length.py`; so an edit to the actual store copy never turns
these red, while a change to the document's *shape* always does. Two tests deliberately read the
real resolved listing: they assert that the scraper finds every catalog language, which is
the one property a fixture cannot prove.
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
from check_store_copy_length import parse_limits  # noqa: E402

import msstore_payload as subject  # noqa: E402
from check_store_copy_length import listing_promises_per_store_fields  # noqa: E402
import brand  # noqa: E402

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


def listing_doc(
    *,
    english="Sovereign mail.",
    dutch="Soevereine mail.",
    english_features="One feature\nAnother feature",
    dutch_features="Eén functie\nNog een functie",
    english_search="secure email app\nJMAP email client",
    dutch_search="veilige e-mail app\nJMAP e-mailclient",
    keystore_table="",
    copyright_row="| Copyright | `© 2026 Allodia` |",
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

### Microsoft Store — Product features (one per field, ≤ ~200 chars, max 20)

**English**
```
{english_features}
```

**Nederlands**
```
{dutch_features}
```

### Microsoft Store — Search terms (up to 7 per language)

**English**
```
{english_search}
```

**Nederlands**
```
{dutch_search}
```

## Console-side metadata (per store)

### Shared fields (identical on every store)

| Field | Value |
|---|---|
| Category | **Productivity** |
{copyright_row}
"""


KEYSTORE_TABLE = """
### `{KEYSTORE}` token — the one substitution in the shared body

| Store (platform) | English | Nederlands |
|---|---|---|
| Microsoft Store (Windows) | the Windows Credential Manager | Windows Referentiebeheer |
| App Store Connect (Apple) | your device's Keychain | de Keychain van je apparaat |
| Google Play (Android) | the Android Keystore | de Android Keystore |
"""


def png(width: int, height: int) -> bytes:
    """The smallest valid-enough PNG: a real signature and IHDR, which is all we parse."""
    ihdr = struct.pack(">II", width, height) + bytes([8, 6, 0, 0, 0])
    chunk = struct.pack(">I", len(ihdr)) + b"IHDR" + ihdr
    chunk += struct.pack(">I", zlib.crc32(b"IHDR" + ihdr) & 0xFFFFFFFF)
    return subject.PNG_MAGIC + chunk


class DocumentScraping(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "store-listing.md"

    def write(self, **kwargs) -> str:
        self.path.write_text(listing_doc(**kwargs), encoding="utf-8")
        return str(self.path)

    def test_a_language_becomes_its_partner_center_listing_code(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("en", "nl"))
        self.assertEqual([item.store_language for item in built], ["en-us", "nl-nl"])
        self.assertEqual([item.language for item in built], ["English", "Nederlands"])

    def test_the_body_and_features_come_from_the_matching_language(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("nl",))
        self.assertEqual(built[0].description, "Soevereine mail.")
        self.assertEqual(built[0].features, ("Eén functie", "Nog een functie"))
        self.assertEqual(built[0].title, brand.value("MAILCAL_APP_NAME"))
        self.assertEqual(built[0].copyright, "© 2026 Allodia")

    def test_search_terms_come_from_the_matching_language(self) -> None:
        built = subject.load_listings(listing_md=self.write(), locales=("nl",))
        self.assertEqual(built[0].search_terms, ("veilige e-mail app", "JMAP e-mailclient"))

    def test_a_missing_search_section_is_an_error_not_an_empty_list(self) -> None:
        # An empty list would *clear* the console's search terms, which is a silent SEO regression
        # rather than a no-op; so a document that has lost the section must stop the push.
        broken = listing_doc().replace("### Microsoft Store — Search terms", "### Keywords")
        self.path.write_text(broken, encoding="utf-8")
        with self.assertRaises(DocumentShapeError):
            subject.load_listings(listing_md=str(self.path), locales=("en",))

    def test_a_language_the_catalog_ships_but_the_doc_lacks_is_an_error(self) -> None:
        # The failure this replaces is a listing pushed in six languages out of seven, reported
        # as a success.
        with self.assertRaises(DocumentShapeError) as caught:
            subject.load_listings(listing_md=self.write(), locales=("en", "de"))
        self.assertIn("Deutsch", str(caught.exception))

    def test_the_keystore_token_is_substituted_when_the_copy_still_uses_it(self) -> None:
        path = self.write(
            english="Credentials live in {KEYSTORE}.", keystore_table=KEYSTORE_TABLE
        )
        built = subject.load_listings(listing_md=path, locales=("en",))
        self.assertEqual(built[0].description, "Credentials live in the Windows Credential Manager.")

    def test_a_token_with_no_table_is_an_error_rather_than_a_literal_paste(self) -> None:
        path = self.write(english="Credentials live in {KEYSTORE}.")
        with self.assertRaises(DocumentShapeError):
            subject.load_listings(listing_md=path, locales=("en",))

    def test_a_missing_copyright_row_is_an_error(self) -> None:
        with self.assertRaises(DocumentShapeError):
            subject.load_listings(listing_md=self.write(copyright_row=""), locales=("en",))

    def test_a_locale_with_no_store_language_is_named(self) -> None:
        with self.assertRaises(DocumentShapeError) as caught:
            subject.load_listings(listing_md=self.write(), locales=("xx",))
        self.assertIn("STORE_LANGUAGES", str(caught.exception))


@unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
class RealDocument(unittest.TestCase):
    """The one property a fixture cannot prove: the scraper still fits the document we ship."""

    def test_every_catalog_locale_yields_a_complete_listing(self) -> None:
        from changelog_fragments import catalog_locales

        built = subject.load_listings()
        self.assertEqual(tuple(item.locale for item in built), catalog_locales())
        for item in built:
            self.assertTrue(item.description.strip(), item.locale)
            self.assertTrue(item.features, item.locale)
            self.assertTrue(item.title.strip(), item.locale)

    def test_the_real_copy_fits_the_microsoft_store(self) -> None:
        limits = parse_limits(subject.LIMITS_PATH.read_text(encoding="utf-8"))
        for item in subject.load_listings():
            self.assertEqual(subject.measure(item, limits), [], item.locale)


class Measuring(unittest.TestCase):
    def setUp(self) -> None:
        self.limits = parse_limits(LIMITS_TABLE)
        self.listing = subject.Listing(
            locale="en",
            language="English",
            store_language="en-us",
            title="Allodia Mail & Calendar",
            description="Body.",
            features=("A feature",),
            copyright="© 2026 Allodia",
        )

    def replace(self, **kwargs):
        from dataclasses import replace

        return replace(self.listing, **kwargs)

    def test_a_clean_listing_has_nothing_to_report(self) -> None:
        self.assertEqual(subject.measure(self.listing, self.limits), [])

    def test_too_many_features_is_reported(self) -> None:
        problems = subject.measure(self.replace(features=tuple(f"f{n}" for n in range(21))), self.limits)
        self.assertTrue(any("21 product features" in problem for problem in problems))

    def test_an_over_long_feature_names_its_position(self) -> None:
        problems = subject.measure(self.replace(features=("x" * 201,)), self.limits)
        self.assertTrue(any("feature 1" in problem for problem in problems))

    def test_an_eighth_search_term_is_reported(self) -> None:
        problems = subject.measure(
            self.replace(search_terms=tuple(f"term{n}" for n in range(8))), self.limits
        )
        self.assertTrue(any("8 search terms" in problem for problem in problems))

    def test_the_word_budget_is_measured_not_only_the_character_count(self) -> None:
        # Seven short terms, none over 30 characters, 28 words; refused by the console and by
        # nothing else here.
        terms = tuple(f"alpha{n} beta{n} gamma{n} delta{n}" for n in range(7))
        problems = subject.measure(self.replace(search_terms=terms), self.limits)
        self.assertEqual(len(problems), 1)
        self.assertIn("spend 28 words", problems[0])

    def test_a_hyphen_spends_two_words(self) -> None:
        # 14 words by whitespace, 21 the way the Store counts; so the same seven terms with one
        # more hyphen are refused. Measured against the live API; see search_term_words.
        fits = tuple("aa-bb cc" for _ in range(7))            # 21
        over = tuple("aa-bb cc-dd" for _ in range(7))         # 28
        self.assertEqual(subject.measure(self.replace(search_terms=fits), self.limits), [])
        problems = subject.measure(self.replace(search_terms=over), self.limits)
        self.assertIn("spend 28 words", problems[0])

    def test_reusing_a_word_costs_nothing(self) -> None:
        terms = tuple(f"email client {word}" for word in "abcdefg")
        self.assertEqual(subject.measure(self.replace(search_terms=terms), self.limits), [])

    def test_an_over_long_search_term_names_its_position(self) -> None:
        problems = subject.measure(self.replace(search_terms=("ok", "x" * 31)), self.limits)
        self.assertTrue(any("search term 2" in problem for problem in problems))

    def test_an_over_long_release_note_is_measured_against_the_stores_own_cap(self) -> None:
        # 1,500 here, not Google Play's 500: this pushes one console, so it is measured against
        # that console. The cross-store cap stays the store-copy check's job.
        self.assertEqual(subject.measure(self.replace(release_notes="x" * 1_500), self.limits), [])
        self.assertTrue(subject.measure(self.replace(release_notes="x" * 1_501), self.limits))


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

    def test_a_capture_outside_the_stores_pixel_bounds_is_refused(self) -> None:
        self.put("en-list.png", size=(800, 600))
        found, _ = subject.collect_screenshots(self.dir, ("en",))
        listing = subject.Listing(
            locale="en",
            language="English",
            store_language="en-us",
            title="t",
            description="d",
            features=("f",),
            copyright="c",
            screenshots=found["en"],
        )
        problems = subject.measure_screenshots(listing)
        self.assertTrue(any("800x600" in problem for problem in problems))

    def test_more_than_ten_screenshots_is_refused(self) -> None:
        for index in range(11):
            self.put(f"en-screen{index}.png")
        found, _ = subject.collect_screenshots(self.dir, ("en",))
        listing = subject.Listing(
            locale="en",
            language="English",
            store_language="en-us",
            title="t",
            description="d",
            features=("f",),
            copyright="c",
            screenshots=found["en"],
        )
        self.assertTrue(any("at most 10" in problem for problem in subject.measure_screenshots(listing)))

    def test_a_non_png_is_refused_by_its_header_not_its_extension(self) -> None:
        (self.dir / "en-list.png").write_bytes(b"GIF89a not a png at all")
        with self.assertRaises(subject.ListingError):
            subject.collect_screenshots(self.dir, ("en",))

    def test_a_missing_directory_is_a_listing_error_not_a_document_error(self) -> None:
        # Different exit code, different advice: "fix the path", never "fix the scraper".
        with self.assertRaises(subject.ListingError):
            subject.collect_screenshots(self.dir / "nope", ("en",))
        self.assertNotIsInstance(subject.ListingError("x"), DocumentShapeError)


def a_listing(locale="nl", **kwargs):
    defaults = dict(
        locale=locale,
        language=subject.LOCALE_NAMES[locale],
        store_language=subject.STORE_LANGUAGES[locale],
        title="Allodia Mail & Calendar",
        description="New body.",
        features=("New feature",),
        copyright="© 2026 Allodia",
    )
    defaults.update(kwargs)
    return subject.Listing(**defaults)


class Merging(unittest.TestCase):
    def submission(self, listings=None):
        return {
            "id": "1152921504621243540",
            "fileUploadUrl": "https://example.invalid/sas",
            "listings": listings if listings is not None else {},
        }

    def test_an_existing_language_is_updated_in_place(self) -> None:
        current = self.submission(
            {"nl-nl": {"baseListing": {"title": "Old", "description": "Old body.", "features": ["Old"]}}}
        )
        merged, plans, uploads = subject.merge(current, [a_listing()])
        base = merged["listings"]["nl-nl"]["baseListing"]
        self.assertEqual(base["description"], "New body.")
        self.assertEqual(base["features"], ["New feature"])
        self.assertEqual(base["copyrightAndTrademarkInfo"], "© 2026 Allodia")
        self.assertFalse(plans[0].is_new)
        self.assertEqual(uploads, ())

    def test_the_source_submission_is_not_mutated(self) -> None:
        # The plan is a diff of before against after; mutating in place would make every diff empty.
        current = self.submission({"nl-nl": {"baseListing": {"description": "Old body."}}})
        subject.merge(current, [a_listing()])
        self.assertEqual(current["listings"]["nl-nl"]["baseListing"]["description"], "Old body.")

    def test_a_language_the_console_holds_under_another_code_is_reused(self) -> None:
        # Writing `nl-nl` beside an existing `nl` would publish a second Dutch listing rather than
        # update the first; and nothing in the response would say so.
        current = self.submission({"nl": {"baseListing": {"description": "Old body."}}})
        merged, plans, _ = subject.merge(current, [a_listing()])
        self.assertEqual(sorted(merged["listings"]), ["nl"])
        self.assertEqual(plans[0].key, "nl")
        self.assertFalse(plans[0].is_new)

    def test_a_language_the_console_does_not_have_is_flagged_as_new(self) -> None:
        merged, plans, _ = subject.merge(self.submission(), [a_listing()])
        self.assertTrue(plans[0].is_new)
        self.assertIn("nl-nl", merged["listings"])

    def test_search_terms_replace_the_consoles_keywords(self) -> None:
        # The API calls the field `keywords`; Partner Center's form calls it Search terms. They are
        # the same field, and the document owns it; so the console's list is replaced, not merged.
        current = self.submission({"nl-nl": {"baseListing": {"keywords": ["oud", "stale"]}}})
        merged, plans, _ = subject.merge(
            current, [a_listing(search_terms=("veilige e-mail app", "JMAP e-mailclient"))]
        )
        self.assertEqual(
            merged["listings"]["nl-nl"]["baseListing"]["keywords"],
            ["veilige e-mail app", "JMAP e-mailclient"],
        )
        change = next(c for c in plans[0].changes if c.field == "search terms")
        self.assertEqual(change.before, ("oud", "stale"))

    def test_fields_this_tool_does_not_own_are_left_exactly_as_they_were(self) -> None:
        current = self.submission(
            {
                "nl-nl": {
                    "baseListing": {
                        "description": "Old body.",
                        "licenseTerms": "Terms.",
                        "images": [{"fileName": "old.png", "imageType": "Screenshot"}],
                    },
                    "platformOverrides": {"Windows81": {"description": "Legacy."}},
                }
            }
        )
        merged, _, _ = subject.merge(current, [a_listing()])
        entry = merged["listings"]["nl-nl"]
        self.assertEqual(entry["baseListing"]["licenseTerms"], "Terms.")
        self.assertEqual(entry["platformOverrides"], {"Windows81": {"description": "Legacy."}})
        # No --screenshots, so the gallery is untouched rather than emptied.
        self.assertEqual(entry["baseListing"]["images"], [{"fileName": "old.png", "imageType": "Screenshot"}])

    def test_release_notes_are_written_only_when_asked_for(self) -> None:
        current = self.submission({"nl-nl": {"baseListing": {"releaseNotes": "Kept."}}})
        merged, _, _ = subject.merge(current, [a_listing()])
        self.assertEqual(merged["listings"]["nl-nl"]["baseListing"]["releaseNotes"], "Kept.")
        merged, _, _ = subject.merge(current, [a_listing(release_notes="Nieuw.")])
        self.assertEqual(merged["listings"]["nl-nl"]["baseListing"]["releaseNotes"], "Nieuw.")

    def test_screenshots_replace_the_gallery_and_are_queued_for_upload(self) -> None:
        shots = (
            subject.Screenshot(path=Path("nl-list.png"), screen="list", width=2880, height=1800),
            subject.Screenshot(path=Path("nl-calendar.png"), screen="calendar", width=2880, height=1800),
        )
        current = self.submission({"nl-nl": {"baseListing": {"images": [{"fileName": "old.png"}]}}})
        merged, _, uploads = subject.merge(current, [a_listing(screenshots=shots)])
        images = merged["listings"]["nl-nl"]["baseListing"]["images"]
        self.assertEqual([image["fileName"] for image in images], ["nl-list.png", "nl-calendar.png"])
        self.assertTrue(all(image["fileStatus"] == "PendingUpload" for image in images))
        self.assertEqual([shot.zip_name for shot in uploads], ["nl-list.png", "nl-calendar.png"])

    def test_a_title_the_console_does_not_set_is_not_introduced(self) -> None:
        # Every real listing came back without `title`: Partner Center leaves the per-listing
        # reserved-name override unset when there is one reserved name. Writing it would be the
        # only field in this push the Store can refuse, for a value it already derives.
        current = self.submission({"nl-nl": {"baseListing": {"description": "Old body."}}})
        merged, plans, _ = subject.merge(current, [a_listing()])
        self.assertNotIn("title", merged["listings"]["nl-nl"]["baseListing"])
        title = next(change for change in plans[0].changes if change.field == "title")
        self.assertFalse(title.changed)
        self.assertIn("reserved app name", title.note)
        self.assertIn("reserved app name", subject.render_plan(plans, images_pushed=False))

    def test_a_title_the_console_does_set_is_kept_in_sync(self) -> None:
        current = self.submission(
            {"nl-nl": {"baseListing": {"title": "Allodia Mail", "description": "Old body."}}}
        )
        merged, plans, _ = subject.merge(current, [a_listing()])
        self.assertEqual(
            merged["listings"]["nl-nl"]["baseListing"]["title"], "Allodia Mail & Calendar"
        )

    def test_an_unchanged_language_reports_no_change(self) -> None:
        current = self.submission(
            {
                "nl-nl": {
                    "baseListing": {
                        "title": "Allodia Mail & Calendar",
                        "description": "New body.",
                        "features": ["New feature"],
                        "copyrightAndTrademarkInfo": "© 2026 Allodia",
                    }
                }
            }
        )
        _, plans, _ = subject.merge(current, [a_listing()])
        self.assertEqual(plans[0].changed, ())


class Reporting(unittest.TestCase):
    def test_the_plan_names_the_language_and_the_changed_fields(self) -> None:
        current = {"listings": {"nl-nl": {"baseListing": {"description": "Old body."}}}}
        _, plans, _ = subject.merge(current, [a_listing()])
        report = subject.render_plan(plans, images_pushed=False)
        self.assertIn("nl-nl", report)
        self.assertIn("Nederlands", report)
        self.assertIn("description", report)
        self.assertIn("- Old body.", report)
        self.assertIn("+ New body.", report)
        self.assertIn("untouched", report)

    def test_a_feature_list_change_shows_only_what_moved(self) -> None:
        current = {"listings": {"nl-nl": {"baseListing": {"features": ["Kept", "Dropped"]}}}}
        _, plans, _ = subject.merge(current, [a_listing(features=("Kept", "Added"))])
        report = subject.render_plan(plans, images_pushed=False)
        self.assertIn("- Dropped", report)
        self.assertIn("+ Added", report)
        self.assertNotIn("- Kept", report)


class Packages(unittest.TestCase):
    """The `.msixupload` half.

    Uploading the package by hand in Partner Center is what put 0.5.0's listing back to the
    previous release's copy: the console had loaded the listing before the API wrote it, and
    saving the package saved that stale copy over the top. So the package travels in the same
    submission write as the copy, and these assert the two refusals that keep a wrong one out.
    """

    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp())

    def _package(self, name, body=b"PK not really a bundle"):
        path = self.dir / name
        path.write_bytes(body)
        return path

    def test_a_package_is_added_as_pending_upload(self) -> None:
        path = self._package("Mailcal_0.5.0.0_x64_arm64_bundle.msixupload")
        package = subject.load_package(path, expected_version="0.5.0")
        submission = {"applicationPackages": []}
        _, change = subject.merge_package(submission, package)
        self.assertEqual(submission["applicationPackages"][0]["fileName"], path.name)
        self.assertEqual(submission["applicationPackages"][0]["fileStatus"], "PendingUpload")
        self.assertIsNotNone(change)

    def test_packages_already_in_the_submission_are_kept(self) -> None:
        # The Store serves older packages to downlevel Windows, so a new one is added beside
        # them rather than replacing them; the console does the same.
        path = self._package("Mailcal_0.5.0.0_x64_arm64_bundle.msixupload")
        package = subject.load_package(path, expected_version="0.5.0")
        submission = {"applicationPackages": [
            {"fileName": "Mailcal_0.3.0.0_x64_arm64_bundle.msixupload", "fileStatus": "Uploaded"},
        ]}
        subject.merge_package(submission, package)
        names = [p["fileName"] for p in submission["applicationPackages"]]
        self.assertIn("Mailcal_0.3.0.0_x64_arm64_bundle.msixupload", names)
        self.assertIn(path.name, names)

    def test_re_running_does_not_queue_the_same_package_twice(self) -> None:
        path = self._package("Mailcal_0.5.0.0_x64_arm64_bundle.msixupload")
        package = subject.load_package(path, expected_version="0.5.0")
        submission = {"applicationPackages": []}
        subject.merge_package(submission, package)
        subject.merge_package(submission, package)
        self.assertEqual(len(submission["applicationPackages"]), 1)

    def test_a_package_whose_version_is_not_this_release_is_refused(self) -> None:
        # The failure this exists for: 0.5.0 was nearly packaged from a branch whose /VERSION
        # still said 0.4.0, which the Store would have taken as a re-upload of a shipped version.
        path = self._package("Mailcal_0.4.0.0_x64_arm64_bundle.msixupload")
        with self.assertRaises(subject.ListingError) as caught:
            subject.load_package(path, expected_version="0.5.0")
        self.assertIn("0.4.0.0", str(caught.exception))
        self.assertIn("0.5.0", str(caught.exception))

    def test_a_non_zero_revision_is_refused_before_the_upload(self) -> None:
        # The Store rejects it only after ingesting the whole bundle, which is a slow way to
        # learn it; clients/windows/package.ps1 refuses the same thing at build time.
        path = self._package("Mailcal_0.5.0.1_x64_arm64_bundle.msixupload")
        with self.assertRaises(subject.ListingError):
            subject.load_package(path, expected_version="0.5.0")

    def test_a_file_that_is_not_a_msixupload_is_refused(self) -> None:
        path = self._package("Mailcal_0.5.0.0_x64.msix")
        with self.assertRaises(subject.ListingError):
            subject.load_package(path, expected_version="0.5.0")

    def test_a_missing_package_names_the_path(self) -> None:
        missing = self.dir / "Mailcal_0.5.0.0_x64_arm64_bundle.msixupload"
        with self.assertRaises(subject.ListingError) as caught:
            subject.load_package(missing, expected_version="0.5.0")
        self.assertIn(str(missing), str(caught.exception))

    def test_an_empty_package_is_refused(self) -> None:
        path = self._package("Mailcal_0.5.0.0_x64_arm64_bundle.msixupload", body=b"")
        with self.assertRaises(subject.ListingError):
            subject.load_package(path, expected_version="0.5.0")


if __name__ == "__main__":
    unittest.main()
