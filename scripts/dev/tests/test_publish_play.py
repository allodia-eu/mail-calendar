#!/usr/bin/env python3
"""Unit tests for the Google Play publisher.

Two kinds of thing are tested here, and neither is "does the SDK work".

**The document -> payload mapping**, because its failure mode is silent. A locale that falls out of
the mapping does not crash: Play simply leaves that language showing the previous release's copy,
and the run reports success. So the tests pin that every catalog locale reaches a payload, that the
`{KEYSTORE}` token is really substituted, and that a missing translation raises.

**The track selection**, because it is the only real logic in the file and the only way to find out
you got it wrong is to read a live Play console; after a staged rollout's `userFraction` has been
overwritten. `select_release` and `with_release_notes` are pure for exactly that reason, and this is
where they are exercised.

The fixtures are a miniature `store-listing.md`, not the real one, so editing store copy never
breaks these tests and changing the document's *shape* always does.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ci"))

import brand  # noqa: E402
import play_listing as listing_module  # noqa: E402
import publish_play as subject  # noqa: E402
from changelog_fragments import DocumentShapeError  # noqa: E402
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

LOCALES = ("en", "nl")

LISTING = """# Listing

## Field limits

| Store | Name/Title | Subtitle | Short/Promo | Description | Feature list | Search terms | What's new |
|---|---|---|---|---|---|---|---|
| Microsoft Store | 256 | — | — | 10,000 | up to 20 × ~200 | up to 7 × 30 (21 words) | 1,500 |
| App Store Connect | 30 | 30 | 170 (Promo) | 4,000 | — | Keywords: 100 | 4,000 |
| Google Play | 30 | — | 80 (Short desc) | 4,000 | — | — | 500 (release notes) |

## Shared description — English

```
Mail, with credentials in {KEYSTORE}.
```

## Shared description — Nederlands

```
Mail, met inloggegevens in {KEYSTORE}.
```

## Per-store fields

### `{KEYSTORE}` token — the one substitution in the shared body

| Store (platform) | English | Nederlands |
|---|---|---|
| Microsoft Store (Windows) | the Credential Manager | Referentiebeheer |
| App Store Connect (Apple) | the Keychain | de Keychain |
| Google Play (Android) | the Android Keystore | de Android Keystore |

### Product name / title (every store, every language)

```
Allodia Mail & Calendar
```

### Google Play — Short description (≤80)

**English**
```
Sovereign email.
```

**Nederlands**
```
Soevereine e-mail.
```
"""

RELEASE_NOTE = """# 1.2.3 — 2026-08-03

## macos, ios, windows, android

Paste into: App Store Connect (macOS)

**English**

```
Signatures, and a Contacts tab.
```

**Nederlands**

```
Handtekeningen, en een tabblad Contacten.
```
"""

WINDOWS_ONLY_NOTE = """# 1.2.3 — 2026-08-03

## windows

Paste into: Microsoft Store

**English**

```
A screen-reader fix.
```

**Nederlands**

```
Een schermlezer-fix.
```
"""


class LocaleTests(unittest.TestCase):
    """Every catalog locale must reach Play, or be a loud error."""

    def test_maps_every_catalog_locale(self):
        with unittest.mock.patch.object(listing_module, "catalog_locales", return_value=LOCALES):
            self.assertEqual(
                listing_module.play_locales(),
                (("en", "English", "en-GB"), ("nl", "Nederlands", "nl-NL")),
            )

    def test_unmapped_locale_is_an_error_not_a_skip(self):
        """A catalog locale with no Play tag must stop the run, not quietly publish the rest.

        The fixture removes a tag from a locale that *does* have a display name; using an
        entirely unknown code instead would trip `LOCALE_NAMES` first and pass this test without
        ever reaching the branch it names.
        """
        with unittest.mock.patch.object(listing_module, "catalog_locales", return_value=LOCALES):
            with unittest.mock.patch.object(listing_module, "PLAY_LOCALES", {"en": "en-GB"}):
                with self.assertRaises(listing_module.PublishError) as caught:
                    listing_module.play_locales()
        self.assertIn("PLAY_LOCALES", str(caught.exception))

    def test_a_locale_with_no_display_name_is_also_an_error(self):
        with unittest.mock.patch.object(listing_module, "catalog_locales", return_value=("en", "sv")):
            with self.assertRaises(DocumentShapeError) as caught:
                listing_module.play_locales()
        self.assertIn("LOCALE_NAMES", str(caught.exception))

    def test_every_shipped_locale_has_a_play_tag(self):
        """The real catalog, not a fixture; this is the check that catches adding a language."""
        for code in listing_module.catalog_locales():
            self.assertIn(code, listing_module.PLAY_LOCALES, f"locale '{code}' has no Google Play tag")

    def test_play_tags_are_distinct(self):
        self.assertEqual(
            len(set(listing_module.PLAY_LOCALES.values())), len(listing_module.PLAY_LOCALES),
            "two locales share a Play tag: one would overwrite the other's listing",
        )


class ListingTests(unittest.TestCase):
    def setUp(self):
        patch = unittest.mock.patch.object(listing_module, "catalog_locales", return_value=LOCALES)
        patch.start()
        self.addCleanup(patch.stop)

    def test_substitutes_the_keystore_token_with_plays_value(self):
        payloads = listing_module.listing_payloads(LISTING)
        self.assertEqual(payloads["en-GB"]["fullDescription"], "Mail, with credentials in the Android Keystore.")
        self.assertEqual(payloads["nl-NL"]["fullDescription"], "Mail, met inloggegevens in de Android Keystore.")

    def test_no_payload_carries_an_unsubstituted_token(self):
        for tag, payload in listing_module.listing_payloads(LISTING).items():
            self.assertNotIn(listing_module.KEYSTORE_TOKEN, payload["fullDescription"], tag)

    def test_carries_title_and_short_description(self):
        payload = listing_module.listing_payloads(LISTING)["nl-NL"]
        self.assertEqual(payload["title"], brand.value("MAILCAL_APP_NAME"))
        self.assertEqual(payload["shortDescription"], "Soevereine e-mail.")

    def test_missing_translation_raises_rather_than_publishing_a_partial_set(self):
        stripped = LISTING.replace("## Shared description — Nederlands", "## Something else")
        with self.assertRaises(DocumentShapeError):
            listing_module.listing_payloads(stripped)


class ReleaseNoteTests(unittest.TestCase):
    def setUp(self):
        patch = unittest.mock.patch.object(listing_module, "catalog_locales", return_value=LOCALES)
        patch.start()
        self.addCleanup(patch.stop)
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.directory, ignore_errors=True))

    def write(self, text):
        (self.directory / "1.2.3.md").write_text(text, encoding="utf-8")
        return self.directory

    def test_reads_the_android_section_under_play_tags(self):
        notes = listing_module.android_release_notes("1.2.3", self.write(RELEASE_NOTE))
        self.assertEqual(notes["en-GB"], "Signatures, and a Contacts tab.")
        self.assertEqual(notes["nl-NL"], "Handtekeningen, en een tabblad Contacten.")

    def test_a_windows_only_release_gives_play_nothing(self):
        """Rule 4 of store-listing.md: a fix Android never got is not advertised on Play."""
        self.assertEqual(listing_module.android_release_notes("1.2.3", self.write(WINDOWS_ONLY_NOTE)), {})

    def test_a_missing_release_file_names_the_fix(self):
        with self.assertRaises(listing_module.PublishError) as caught:
            listing_module.android_release_notes("9.9.9", self.directory)
        self.assertIn("release.py", str(caught.exception))


class VersionTests(unittest.TestCase):
    def test_version_code_matches_the_documented_formula(self):
        self.assertEqual(listing_module.android_version_code("0.3.0"), 300000)
        self.assertEqual(listing_module.android_version_code("1.2.3"), 10203000)

    def test_version_codes_only_climb(self):
        versions = ["0.2.0", "0.2.1", "0.3.0", "0.13.5", "1.0.0"]
        codes = [listing_module.android_version_code(version) for version in versions]
        self.assertEqual(codes, sorted(codes))

    def test_package_name_is_the_id_the_build_is_branded_with(self):
        # Not a literal: an unbranded checkout publishes nowhere, but it must not fail here for
        # the wrong reason, and the point of the function is that it never disagrees with Gradle.
        self.assertEqual(listing_module.package_name(), brand.value("MAILCAL_APP_ID"))

    def test_package_name_without_an_application_id_in_gradle_raises(self):
        path = Path(tempfile.mkdtemp()) / "build.gradle.kts"
        path.write_text("android { }", encoding="utf-8")
        with self.assertRaises(listing_module.PublishError):
            listing_module.package_name(path)


class TrackSelectionTests(unittest.TestCase):
    TRACKS = [
        {"track": "internal", "releases": [{"versionCodes": ["300000"], "status": "draft"}]},
        {
            "track": "production",
            "releases": [
                {"versionCodes": ["200000"], "status": "completed"},
                {"versionCodes": ["300000"], "status": "inProgress", "userFraction": 0.2},
            ],
        },
    ]

    def test_finds_the_release_carrying_the_version_code(self):
        tracks = [self.TRACKS[0]]
        self.assertEqual(subject.select_release(tracks, "300000"), ("internal", 0))

    def test_matches_an_int_version_code_against_plays_strings(self):
        """Regression: the two sides of this comparison have different types.

        `android_version_code()` returns an int; Play returns `versionCodes` as strings, because
        `int64` is not JSON-safe. An unnormalised `in` test never matches, and the error it
        produces; "no release contains versionCode 300000"; reads exactly like a build that was
        never uploaded. This is the real call path: the CLI passes the int.
        """
        code = listing_module.android_version_code("0.3.0")
        self.assertIsInstance(code, int)
        self.assertEqual(subject.select_release([self.TRACKS[0]], code), ("internal", 0))

    def test_finds_a_release_that_is_not_the_first_in_its_track(self):
        tracks = [self.TRACKS[1]]
        self.assertEqual(subject.select_release(tracks, "300000"), ("production", 1))

    def test_ambiguous_version_code_refuses_rather_than_guessing(self):
        with self.assertRaises(listing_module.PublishError) as caught:
            subject.select_release(self.TRACKS, "300000")
        self.assertIn("--track", str(caught.exception))

    def test_an_explicit_track_disambiguates(self):
        self.assertEqual(
            subject.select_release(self.TRACKS, "300000", requested="production"), ("production", 1)
        )

    def test_a_build_that_is_not_uploaded_yet_says_so(self):
        with self.assertRaises(listing_module.PublishError) as caught:
            subject.select_release(self.TRACKS, "400000")
        self.assertIn("--skip-notes", str(caught.exception))

    def test_an_empty_track_list_is_not_a_silent_success(self):
        with self.assertRaises(listing_module.PublishError):
            subject.select_release([], "300000")


class ReleaseNotePatchTests(unittest.TestCase):
    TRACK = {
        "track": "production",
        "releases": [
            {"versionCodes": ["200000"], "status": "completed", "releaseNotes": [{"language": "en-GB", "text": "old"}]},
            {"versionCodes": ["300000"], "status": "inProgress", "userFraction": 0.2},
        ],
    }

    def test_writes_notes_onto_the_named_release(self):
        updated = subject.with_release_notes(self.TRACK, 1, {"en-GB": "new", "nl-NL": "nieuw"})
        self.assertEqual(
            updated["releases"][1]["releaseNotes"],
            [{"language": "en-GB", "text": "new"}, {"language": "nl-NL", "text": "nieuw"}],
        )

    def test_preserves_a_staged_rollout(self):
        """The whole track goes back to Play, so losing `userFraction` would resume a rollout."""
        updated = subject.with_release_notes(self.TRACK, 1, {"en-GB": "new"})
        self.assertEqual(updated["releases"][1]["userFraction"], 0.2)
        self.assertEqual(updated["releases"][1]["status"], "inProgress")

    def test_leaves_other_releases_alone(self):
        updated = subject.with_release_notes(self.TRACK, 1, {"en-GB": "new"})
        self.assertEqual(updated["releases"][0]["releaseNotes"], [{"language": "en-GB", "text": "old"}])

    def test_does_not_mutate_the_track_it_was_given(self):
        subject.with_release_notes(self.TRACK, 1, {"en-GB": "new"})
        self.assertNotIn("releaseNotes", self.TRACK["releases"][1])


class UploadTests(unittest.TestCase):
    """The call sequence, and the safety model that hangs off it.

    `upload` is the one function no local run can exercise for real, so it is exercised against a
    mock Play. What matters is not that the SDK is called but *that the edit is thrown away* unless
    `--commit` is passed; a rehearsal that silently published would be the worst possible bug in
    this file, and it is invisible until a user sees the copy.
    """

    TRACKS = {"tracks": [{"track": "internal", "releases": [{"versionCodes": ["300000"]}]}]}

    def service(self):
        service = unittest.mock.MagicMock()
        edits = service.edits.return_value
        edits.insert.return_value.execute.return_value = {"id": "EDIT-1"}
        edits.tracks.return_value.list.return_value.execute.return_value = self.TRACKS
        return service, edits

    def run_upload(self, commit=False, notes=None):
        service, edits = self.service()
        listings = {"en-GB": {"title": "T", "shortDescription": "s", "fullDescription": "f"}}
        notes = {"en-GB": "note"} if notes is None else notes
        committed = subject.upload(
            service, "eu.allodia.mailcal", listings, notes, 300000, None, commit, lambda _: None
        )
        return edits, committed

    def test_a_rehearsal_validates_and_discards(self):
        edits, committed = self.run_upload(commit=False)
        self.assertFalse(committed)
        edits.validate.assert_called_once()
        edits.delete.assert_called_once_with(packageName="eu.allodia.mailcal", editId="EDIT-1")
        edits.commit.assert_not_called()

    def test_commit_publishes_and_does_not_delete(self):
        edits, committed = self.run_upload(commit=True)
        self.assertTrue(committed)
        edits.commit.assert_called_once_with(packageName="eu.allodia.mailcal", editId="EDIT-1")
        edits.delete.assert_not_called()

    def test_writes_one_listing_per_language(self):
        edits, _ = self.run_upload()
        update = edits.listings.return_value.update
        update.assert_called_once()
        self.assertEqual(update.call_args.kwargs["language"], "en-GB")
        self.assertEqual(update.call_args.kwargs["body"]["title"], "T")

    def test_writes_the_note_onto_the_located_track(self):
        edits, _ = self.run_upload()
        update = edits.tracks.return_value.update
        self.assertEqual(update.call_args.kwargs["track"], "internal")
        release = update.call_args.kwargs["body"]["releases"][0]
        self.assertEqual(release["releaseNotes"], [{"language": "en-GB", "text": "note"}])

    def test_no_notes_means_the_track_is_never_touched(self):
        edits, _ = self.run_upload(notes={})
        edits.tracks.return_value.update.assert_not_called()

    def test_a_failure_midway_still_discards_the_edit(self):
        """A crash must not leave an open edit holding half the copy."""
        service, edits = self.service()
        edits.listings.return_value.update.return_value.execute.side_effect = RuntimeError("boom")
        with self.assertRaises(RuntimeError):
            subject.upload(
                service, "pkg", {"en-GB": {"title": "T"}}, {}, 300000, None, False, lambda _: None
            )
        edits.delete.assert_called_once()
        edits.commit.assert_not_called()


def write_png(path, width, height):
    """A file with a valid PNG header of the given size. Nothing reads past IHDR."""
    import struct, zlib  # noqa: E401; local to the fixture, not to the module under test

    ihdr = struct.pack(">II", width, height) + bytes([8, 6, 0, 0, 0])
    chunk = struct.pack(">I", len(ihdr)) + b"IHDR" + ihdr
    chunk += struct.pack(">I", zlib.crc32(b"IHDR" + ihdr))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk)


class ImageCollectionTests(unittest.TestCase):
    """Reading `showcase-screenshots/android/` into Play's four slots.

    The flat directory is deliberate (docs/store-screenshots.md): Play's picker is a flat
    list, so the form factor lives in the file name. Which means the *name* is load-bearing, and a
    typo has no symptom; Play leaves an unwritten slot showing the old gallery and returns 200.
    """

    LOCALES = ("en", "nl")

    def setUp(self):
        patch = unittest.mock.patch.object(listing_module, "catalog_locales", return_value=self.LOCALES)
        patch.start()
        self.addCleanup(patch.stop)
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.directory, ignore_errors=True))

    def populate(self, locales=("en", "nl"), screens=("list", "calendar")):
        for locale in locales:
            write_png(self.directory / f"feature-graphic-{locale}.png", 1024, 500)
            for screen in screens:
                write_png(self.directory / f"phone-{locale}-{screen}.png", 1080, 2400)
                write_png(self.directory / f"tablet-7-{locale}-{screen}.png", 1200, 1920)
                write_png(self.directory / f"tablet-10-{locale}-{screen}.png", 1600, 2560)
        return self.directory

    def test_each_form_factor_lands_in_its_own_play_slot(self):
        """Three galleries, not one; an empty tablet slot marks the app phone-only on Play."""
        payloads = listing_module.image_payloads(self.populate())
        self.assertEqual(
            sorted(payloads["en-GB"]),
            ["featureGraphic", "phoneScreenshots", "sevenInchScreenshots", "tenInchScreenshots"],
        )

    def test_every_catalog_locale_gets_its_own_images(self):
        payloads = listing_module.image_payloads(self.populate())
        self.assertEqual(sorted(payloads), ["en-GB", "nl-NL"])

    def test_screenshots_are_ordered_by_the_shared_gallery_order(self):
        """`SCREEN_ORDER`, the same one the other two publishers apply; not alphabetical.

        Alphabetical would open every Play listing on `add-account`, which is mostly empty space.
        """
        payloads = listing_module.image_payloads(
            self.populate(screens=("add-account", "list", "calendar"))
        )
        self.assertEqual(
            [path.name for path in payloads["en-GB"]["phoneScreenshots"]],
            ["phone-en-list.png", "phone-en-calendar.png", "phone-en-add-account.png"],
        )

    def test_the_dark_inbox_is_a_screen_of_its_own_and_lands_third(self):
        """`list-dark` is a capture, not a variant the publishers have to know how to pair.

        It reaches them as an ordinary `<form>-<locale>-<screen>.png`, so the only thing that has to
        be decided is where it sits; and that decision is `SCREEN_ORDER`, not the file name. Third,
        behind the two screens that say what the product *is*.
        """
        payloads = listing_module.image_payloads(
            self.populate(screens=("list-dark", "add-account", "list", "calendar"))
        )
        self.assertEqual(
            [path.name for path in payloads["en-GB"]["phoneScreenshots"]],
            [
                "phone-en-list.png",
                "phone-en-calendar.png",
                "phone-en-list-dark.png",
                "phone-en-add-account.png",
            ],
        )

    def test_a_file_nobody_can_place_is_an_error_not_a_silent_skip(self):
        self.populate()
        write_png(self.directory / "phone-en-list-final-FINAL.PNG", 1080, 2400)
        with self.assertRaises(listing_module.ListingError) as caught:
            listing_module.image_payloads(self.directory)
        self.assertIn("match no naming convention", str(caught.exception))

    def test_a_missing_directory_names_itself(self):
        with self.assertRaises(listing_module.ListingError):
            listing_module.image_payloads(self.directory / "nope")

    def test_a_locale_with_no_captures_is_simply_absent(self):
        """Not an error: a re-capture of one language is a legitimate partial push."""
        payloads = listing_module.image_payloads(self.populate(locales=("en",)))
        self.assertEqual(sorted(payloads), ["en-GB"])

    # -- what Play would reject, caught before an edit is opened ---------------------------------

    def test_a_slot_with_one_screenshot_is_refused(self):
        with self.assertRaises(listing_module.ListingError) as caught:
            listing_module.image_payloads(self.populate(locales=("en",), screens=("list",)))
        self.assertIn("Play takes 2-8", str(caught.exception))

    def test_a_feature_graphic_of_the_wrong_size_is_refused(self):
        self.populate(locales=("en",))
        write_png(self.directory / "feature-graphic-en.png", 1024, 512)
        with self.assertRaises(listing_module.ListingError) as caught:
            listing_module.image_payloads(self.directory)
        self.assertIn("must be exactly 1024x500", str(caught.exception))

    def test_a_screenshot_side_outside_plays_bounds_is_refused(self):
        self.populate(locales=("en",))
        write_png(self.directory / "phone-en-list.png", 200, 400)
        with self.assertRaises(listing_module.ListingError) as caught:
            listing_module.image_payloads(self.directory)
        self.assertIn("between 320 and 3840", str(caught.exception))

    def test_a_file_that_is_not_a_png_is_refused(self):
        self.populate(locales=("en",))
        (self.directory / "phone-en-list.png").write_bytes(b"not a png at all")
        with self.assertRaises(Exception):
            listing_module.image_payloads(self.directory)


class ImageUploadTests(unittest.TestCase):
    """The call sequence against a mock Play. What matters is `deleteall` before `upload`.

    Play **appends** to a gallery. Uploading six screenshots over a gallery that already holds six
    leaves twelve, of which the store shows the first eight; so the second run of this script
    would publish a gallery half made of last release's captures, and report success.
    """

    def setUp(self):
        # The SDK is not installed on the machines this suite runs on, and `upload_images` imports
        # it INSIDE the function precisely so --dry-run and these tests never need it. Standing in
        # a fake module is what proves that deferral is real.
        import types

        module = types.ModuleType("googleapiclient.http")
        module.MediaFileUpload = lambda path, mimetype=None: f"media:{path}"
        for name, value in (("googleapiclient", types.ModuleType("googleapiclient")),
                            ("googleapiclient.http", module)):
            if name not in sys.modules:
                sys.modules[name] = value
                self.addCleanup(sys.modules.pop, name, None)

    def run_upload(self, images):
        edits = unittest.mock.MagicMock()
        subject.upload_images(edits, "eu.allodia.mailcal", "EDIT-1", images, lambda _: None)
        return edits.images.return_value

    def test_every_slot_is_cleared_before_it_is_filled(self):
        images = self.run_upload(
            {"en-GB": {"phoneScreenshots": (Path("a.png"), Path("b.png"))}}
        )
        images.deleteall.assert_called_once_with(
            packageName="eu.allodia.mailcal", editId="EDIT-1",
            language="en-GB", imageType="phoneScreenshots",
        )
        self.assertEqual(images.upload.call_count, 2)

    def test_uploads_in_gallery_order(self):
        images = self.run_upload(
            {"en-GB": {"phoneScreenshots": (Path("list.png"), Path("calendar.png"))}}
        )
        self.assertEqual(
            [call.kwargs["media_body"] for call in images.upload.call_args_list],
            ["media:list.png", "media:calendar.png"],
        )

    def test_a_slot_that_was_not_captured_is_never_deleted(self):
        """The tablet gallery must survive a phone-only run; see the phone-only trap above."""
        images = self.run_upload({"en-GB": {"phoneScreenshots": (Path("a.png"),)}})
        self.assertEqual(
            [call.kwargs["imageType"] for call in images.deleteall.call_args_list],
            ["phoneScreenshots"],
        )

    def test_each_language_is_written_separately(self):
        images = self.run_upload({
            "en-GB": {"featureGraphic": (Path("en.png"),)},
            "nl-NL": {"featureGraphic": (Path("nl.png"),)},
        })
        self.assertEqual(
            [call.kwargs["language"] for call in images.upload.call_args_list], ["en-GB", "nl-NL"]
        )

    def test_the_edit_flow_skips_images_entirely_when_none_were_given(self):
        service = unittest.mock.MagicMock()
        edits = service.edits.return_value
        edits.insert.return_value.execute.return_value = {"id": "EDIT-1"}
        subject.upload(
            service, "pkg", {"en-GB": {"title": "T"}}, {}, 300000, None, False, lambda _: None
        )
        edits.images.return_value.deleteall.assert_not_called()


class MeasurementTests(unittest.TestCase):
    def setUp(self):
        patch = unittest.mock.patch.object(listing_module, "catalog_locales", return_value=LOCALES)
        patch.start()
        self.addCleanup(patch.stop)
        self.limits = listing_module.parse_limits(LISTING)

    def test_copy_that_fits_reports_no_violation(self):
        measured = listing_module.measure(listing_module.listing_payloads(LISTING), {}, self.limits)
        self.assertTrue(all(item.fits for item in measured))

    def test_an_over_long_note_is_caught_before_upload(self):
        measured = listing_module.measure({}, {"en-GB": "x" * 501}, self.limits)
        self.assertEqual([item.fits for item in measured], [False])

    def test_an_over_long_description_is_caught_before_upload(self):
        payloads = listing_module.listing_payloads(LISTING)
        payloads["en-GB"]["fullDescription"] = "x" * 4001
        over = [item for item in listing_module.measure(payloads, {}, self.limits) if not item.fits]
        self.assertEqual([item.where for item in over], ["fullDescription / en-GB"])


class LiveComparisonTests(unittest.TestCase):
    """The pure half of `--show`: live state in, report and drift out. No network, no fakes."""

    LISTINGS = {
        "en-GB": {"title": "T", "shortDescription": "S", "fullDescription": "F"},
        "nl-NL": {"title": "T", "shortDescription": "S", "fullDescription": "F"},
    }

    @staticmethod
    def entry(listing, feature=1, phone=6, seven=6, ten=6):
        return {
            "missing": False,
            "listing": listing,
            "images": {
                listing_module.FEATURE_GRAPHIC: feature,
                listing_module.PHONE: phone,
                listing_module.SEVEN_INCH: seven,
                listing_module.TEN_INCH: ten,
            },
        }

    def matching_images(self):
        return {
            tag: {
                listing_module.FEATURE_GRAPHIC: ["f"],
                listing_module.PHONE: ["p"] * 6,
                listing_module.SEVEN_INCH: ["s"] * 6,
                listing_module.TEN_INCH: ["t"] * 6,
            }
            for tag in self.LISTINGS
        }

    def test_a_listing_that_matches_reports_no_drift(self):
        state = {tag: self.entry(dict(payload)) for tag, payload in self.LISTINGS.items()}
        lines, drift = listing_module.compare_live(state, self.LISTINGS, self.matching_images())
        self.assertEqual(drift, [])
        self.assertTrue(all("copy matches" in line for line in lines))

    def test_hand_edited_copy_in_the_console_is_caught(self):
        state = {tag: self.entry(dict(payload)) for tag, payload in self.LISTINGS.items()}
        state["nl-NL"]["listing"]["fullDescription"] = "someone edited this in the console"
        _, drift = listing_module.compare_live(state, self.LISTINGS, self.matching_images())
        self.assertEqual(drift, ["nl-NL: live fullDescription differs from the resolved listing"])

    def test_a_missing_listing_is_not_reported_as_an_empty_gallery(self):
        state = {
            "en-GB": self.entry(dict(self.LISTINGS["en-GB"])),
            "nl-NL": {"missing": True, "listing": None, "images": {}},
        }
        lines, drift = listing_module.compare_live(state, self.LISTINGS, self.matching_images())
        self.assertIn("NO LISTING", lines[1])
        self.assertEqual(drift, ["nl-NL: no listing (nothing has been published for it yet)"])

    def test_a_language_play_never_heard_of_counts_as_missing(self):
        _, drift = listing_module.compare_live({}, self.LISTINGS, None)
        self.assertEqual(len(drift), 2)

    def test_a_half_uploaded_gallery_is_drift(self):
        state = {tag: self.entry(dict(payload)) for tag, payload in self.LISTINGS.items()}
        state["en-GB"]["images"][listing_module.TEN_INCH] = 3
        _, drift = listing_module.compare_live(state, self.LISTINGS, self.matching_images())
        self.assertEqual(drift, ["en-GB · 10-inch tablet: 3 live, 6 in the capture directory"])

    def test_without_a_capture_directory_galleries_are_reported_not_judged(self):
        state = {tag: self.entry(dict(payload)) for tag, payload in self.LISTINGS.items()}
        state["en-GB"]["images"][listing_module.PHONE] = 2
        lines, drift = listing_module.compare_live(state, self.LISTINGS, None)
        self.assertEqual(drift, [])
        self.assertIn("phone 2", lines[0])
        self.assertNotIn("/", lines[0].split("phone")[1][:6])


class ShowTests(unittest.TestCase):
    """The network half: `--show` opens an edit to read through, and must never write."""

    LISTINGS = {"en-GB": {"title": "T", "shortDescription": "S", "fullDescription": "F"}}

    def setUp(self):
        # Same deferral as `upload_images`: `live_state` imports `HttpError` inside the function so
        # --dry-run and this suite never need the SDK. Faking it is what proves that is still true.
        import types

        errors = types.ModuleType("googleapiclient.errors")

        class HttpError(Exception):
            def __init__(self, status):
                super().__init__(f"HTTP {status}")
                self.resp = types.SimpleNamespace(status=status)

        errors.HttpError = HttpError
        self.HttpError = HttpError
        for name, value in (
            ("googleapiclient", types.ModuleType("googleapiclient")),
            ("googleapiclient.errors", errors),
        ):
            if name not in sys.modules:
                sys.modules[name] = value
                self.addCleanup(sys.modules.pop, name, None)

    def fake_service(self, listing=None, counts=6):
        edits = unittest.mock.MagicMock()
        edits.insert.return_value.execute.return_value = {"id": "edit-1"}
        edits.listings.return_value.get.return_value.execute.return_value = (
            listing if listing is not None else dict(self.LISTINGS["en-GB"])
        )
        edits.images.return_value.list.return_value.execute.return_value = {
            "images": [{"id": str(index)} for index in range(counts)]
        }
        service = unittest.mock.MagicMock()
        service.edits.return_value = edits
        return service, edits

    def test_it_never_commits_and_always_deletes_the_edit(self):
        service, edits = self.fake_service()
        subject.show(service, "eu.allodia.mailcal", self.LISTINGS, {}, lambda _line: None)
        edits.commit.assert_not_called()
        edits.listings.return_value.update.assert_not_called()
        edits.images.return_value.upload.assert_not_called()
        edits.images.return_value.deleteall.assert_not_called()
        edits.delete.assert_called_once()

    def test_the_edit_is_deleted_even_when_the_read_fails(self):
        service, edits = self.fake_service()
        edits.listings.return_value.get.return_value.execute.side_effect = RuntimeError("boom")
        with self.assertRaises(RuntimeError):
            subject.show(service, "eu.allodia.mailcal", self.LISTINGS, {}, lambda _line: None)
        edits.delete.assert_called_once()

    def test_matching_copy_and_galleries_report_no_drift(self):
        service, _ = self.fake_service()
        images = {
            "en-GB": {
                listing_module.FEATURE_GRAPHIC: ["f"] * 6,
                listing_module.PHONE: ["p"] * 6,
                listing_module.SEVEN_INCH: ["s"] * 6,
                listing_module.TEN_INCH: ["t"] * 6,
            }
        }
        drift = subject.show(
            service, "eu.allodia.mailcal", self.LISTINGS, images, lambda _line: None
        )
        self.assertEqual(drift, [])


class ApiErrorTests(unittest.TestCase):
    """Play's `403 The caller does not have permission` names neither the caller nor the fix."""

    KEY = {
        "type": "service_account",
        "project_id": "allodia-play-123",
        "client_email": "play-publisher@allodia-play-123.iam.gserviceaccount.com",
        "private_key": "-----BEGIN PRIVATE KEY-----\nSHOULD-NEVER-BE-PRINTED\n-----END-----\n",
    }

    def key_file(self, payload=None):
        path = Path(tempfile.mkdtemp()) / "key.json"
        path.write_text(json.dumps(self.KEY if payload is None else payload), encoding="utf-8")
        return str(path)

    @staticmethod
    def http_error(status):
        return unittest.mock.Mock(resp=unittest.mock.Mock(status=status))

    def test_a_403_names_the_identity_and_the_console_step(self):
        message = subject.explain_api_error(
            self.http_error(403), "eu.allodia.mailcal", self.key_file()
        )
        self.assertIn(self.KEY["client_email"], message)
        self.assertIn("eu.allodia.mailcal", message)
        self.assertIn("Play Console", message)
        self.assertIn("Manage store presence", message)

    def test_the_private_key_is_never_in_the_message(self):
        message = subject.explain_api_error(
            self.http_error(403), "eu.allodia.mailcal", self.key_file()
        )
        self.assertNotIn("SHOULD-NEVER-BE-PRINTED", message)
        self.assertNotIn("PRIVATE KEY", message)

    def test_a_404_is_about_the_app_not_the_grant(self):
        message = subject.explain_api_error(
            self.http_error(404), "eu.allodia.mailcal", self.key_file()
        )
        self.assertIn("no app 'eu.allodia.mailcal'", message)
        self.assertNotIn("Manage store presence", message)

    def test_an_unrelated_status_gets_no_invented_advice(self):
        for status in (400, 409, 500):
            self.assertIsNone(
                subject.explain_api_error(self.http_error(status), "eu.allodia.mailcal", None)
            )

    def test_a_non_http_error_gets_no_advice(self):
        self.assertIsNone(subject.explain_api_error(ValueError("boom"), "eu.allodia.mailcal", None))

    def test_an_unreadable_key_still_explains_the_grant(self):
        for key in (None, "/nonexistent/key.json", self.key_file({"type": "service_account"})):
            message = subject.explain_api_error(self.http_error(403), "eu.allodia.mailcal", key)
            self.assertIn("this service account", message)
            self.assertIn("Play Console", message)


@unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
class RealDocumentTests(unittest.TestCase):
    """The actual repo, with the real catalog; deliberately no `catalog_locales` patch.

    These read the resolved store listing and `docs/changelog/released/` as they are, so a reshaped
    heading or a locale added without a Play tag fails here rather than at Google. They sit in
    their own class because the fixture-based tests above patch the catalog down to two locales,
    and inheriting that patch would quietly narrow these to en/nl; a test that reads the real
    document but only checks two sevenths of it.
    """

    def test_every_shipped_locale_gets_a_payload(self):
        payloads = listing_module.listing_payloads(
            listing_module.listing_path().read_text(encoding="utf-8")
        )
        self.assertEqual(
            sorted(payloads), sorted(listing_module.PLAY_LOCALES[code]
                                     for code in listing_module.catalog_locales())
        )

    def test_the_real_copy_fits_google_play_today(self):
        """Same numbers the store-copy CI job enforces, read off the real documents."""
        listing = listing_module.listing_path().read_text(encoding="utf-8")
        measured = listing_module.measure(
            listing_module.listing_payloads(listing),
            listing_module.android_release_notes(listing_module.current_version()),
            listing_module.parse_limits(
                listing_module.LIMITS_PATH.read_text(encoding="utf-8")
            ),
        )
        self.assertEqual([str(item) for item in measured if not item.fits], [])

    def test_the_shipped_release_note_reaches_every_locale(self):
        notes = listing_module.android_release_notes(listing_module.current_version())
        if notes:
            self.assertEqual(len(notes), len(listing_module.catalog_locales()))


@unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
class CliTests(unittest.TestCase):
    """Run the script the way a human does: as a program, in a fresh interpreter.

    Every other test imports the module, and the test file puts `scripts/ci` on `sys.path` itself,
    which silently supplied an import the script was not setting up for itself. The suite was green
    while `scripts/dev/publish_play.py --dry-run` died on `ModuleNotFoundError`. A subprocess is
    the only thing that sees that, so the smoke test is a subprocess.
    """

    SCRIPT = Path(__file__).resolve().parents[1] / "publish_play.py"

    def run_cli(self, *args, cwd=None):
        return subprocess.run(
            [sys.executable, str(self.SCRIPT), *args],
            capture_output=True, text=True, cwd=cwd or tempfile.gettempdir(),
        )

    def test_dry_run_works_as_a_program_from_any_directory(self):
        result = self.run_cli("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("nothing was sent to Google", result.stdout)

    def test_dry_run_needs_no_credentials(self):
        self.assertNotIn("service-account", self.run_cli("--dry-run").stdout)

    def test_a_real_run_without_a_key_refuses_before_any_network(self):
        environment = dict(os.environ)
        environment.pop(subject.KEY_ENV, None)
        result = subprocess.run(
            [sys.executable, str(self.SCRIPT)],
            capture_output=True, text=True, env=environment, cwd=tempfile.gettempdir(),
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn(subject.KEY_ENV, result.stderr)

    def test_help_works_without_the_sdk(self):
        self.assertEqual(self.run_cli("--help").returncode, 0)


if __name__ == "__main__":
    unittest.main()
