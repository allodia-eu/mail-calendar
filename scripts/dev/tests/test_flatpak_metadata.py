"""Tests for the Linux client's desktop-entry and AppStream generator.

Two kinds of assertion here, and the second is the one that earns its keep. The pure functions are
checked against small fixtures; then the whole thing is run against the **real** resolved listing,
`/VERSION` and `docs/changelog/released/`. That second half is what makes
this an always-run gate on the documents rather than on the code: the `dev-scripts` CI job is not
area-gated, so a store-listing edit that moves a heading; a docs-only PR, which the `linux` area
would never turn on; fails here instead of at a `flatpak-builder` run nobody does on that branch.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from xml.etree import ElementTree

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

import flatpak_metadata as meta  # noqa: E402
from changelog_fragments import DocumentShapeError  # noqa: E402
from check_store_copy_length import listing_promises_per_store_fields  # noqa: E402

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


LISTING = """\
## Shared description — English

```
First framing paragraph.

Second framing paragraph.

Feature bullets that must not reach the metainfo.
```

## Shared description — Nederlands

```
Eerste alinea.

Tweede alinea.

Opsomming die niet meegaat.
```

## Per-store fields

### Product name / title (every store, every language)

```
Allodia Mail & Calendar
```

### Google Play — Short description (≤80)

**English**
```
Sovereign, private email & calendar.
```

**Nederlands**
```
Soevereine, private e-mail en agenda.
```
"""


class ListingScraping(unittest.TestCase):
    def test_only_the_framing_paragraphs_are_taken(self):
        paragraphs = meta.descriptions(LISTING)
        self.assertEqual(len(paragraphs["en"]), 2)
        self.assertEqual(paragraphs["en"][0], "First framing paragraph.")
        self.assertEqual(paragraphs["nl"][1], "Tweede alinea.")
        # Rule 3 of docs/store-listing.md: the feature bullets may not out-run the matrix, and
        # Linux is 🚧 on nearly all of them. They are left out entirely.
        self.assertNotIn(
            "Feature bullets", " ".join(paragraphs["en"]), "a feature bullet reached the metainfo"
        )

    def test_a_summary_loses_only_its_trailing_stop(self):
        summaries = meta.summaries(LISTING)
        self.assertEqual(summaries["en"], "Sovereign, private email & calendar")
        self.assertEqual(summaries["nl"], "Soevereine, private e-mail en agenda")

    def test_a_body_with_too_few_paragraphs_is_a_shape_error(self):
        thin = LISTING.replace(
            "First framing paragraph.\n\nSecond framing paragraph.\n\n"
            "Feature bullets that must not reach the metainfo.",
            "Only one paragraph.",
        )
        with self.assertRaises(DocumentShapeError):
            meta.descriptions(thin)


class Releases(unittest.TestCase):
    def _write(self, directory: Path, name: str, heading: str) -> None:
        (directory / name).write_text(f"{heading}\n\n## linux\n", encoding="utf-8")

    def test_dates_come_from_the_notes_and_sort_newest_first(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            self._write(directory, "0.2.0.md", "# 0.2.0 — 2026-07-20")
            self._write(directory, "0.10.0.md", "# 0.10.0 — 2026-08-04")
            self._write(directory, "0.3.0.md", "# 0.3.0 — 2026-08-03")
            # Newest first, and 0.10.0 above 0.3.0; compared as integers, not as text.
            self.assertEqual(
                meta.releases(directory),
                [("0.10.0", "2026-08-04"), ("0.3.0", "2026-08-03"), ("0.2.0", "2026-07-20")],
            )

    def test_a_note_whose_heading_disagrees_with_its_filename_is_refused(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            self._write(directory, "0.4.0.md", "# 0.5.0 — 2026-08-04")
            with self.assertRaises(DocumentShapeError):
                meta.releases(directory)

    def test_a_version_with_no_released_note_is_refused(self):
        # `/VERSION` means "the version users currently have", so a metainfo whose newest release is
        # not it would advertise a build that was never released.
        with self.assertRaises(meta.MetadataError):
            meta._releases_element([("0.4.0", "2026-08-04")], "0.5.0")


class GeneratedFiles(unittest.TestCase):
    """The real documents, generated and parsed back."""

    @classmethod
    def setUpClass(cls):
        cls._dir = tempfile.TemporaryDirectory()
        cls.written = meta.build(Path(cls._dir.name))
        cls.listing = meta.brand.listing_source().read_text(encoding="utf-8")
        # Named for the app id this checkout is branded as (docs/branding.md), not for a literal:
        # the unbranded default is as correct a build as the branded one.
        cls.desktop = (Path(cls._dir.name) / f"{meta.APP_ID}.desktop").read_text(encoding="utf-8")
        cls.metainfo = (Path(cls._dir.name) / f"{meta.APP_ID}.metainfo.xml").read_text(
            encoding="utf-8"
        )

    @classmethod
    def tearDownClass(cls):
        cls._dir.cleanup()

    def test_the_metainfo_is_well_formed_and_carries_the_released_version(self):
        root = ElementTree.fromstring(self.metainfo)
        self.assertEqual(root.findtext("id"), meta.APP_ID)
        version = (REPO_ROOT / "VERSION").read_text(encoding="utf-8").strip()
        newest = root.find("releases/release")
        self.assertIsNotNone(newest, "the metainfo carries no <release>")
        self.assertEqual(newest.get("version"), version)
        self.assertRegex(newest.get("date"), r"^\d{4}-\d{2}-\d{2}$")

    def test_every_catalog_locale_reaches_both_files(self):
        root = ElementTree.fromstring(self.metainfo)
        tagged = {
            element.get("{http://www.w3.org/XML/1998/namespace}lang")
            for element in root.findall("summary")
        }
        for locale in meta.summaries(self.listing):
            if locale == "en":
                continue
            with self.subTest(locale=locale):
                self.assertIn(locale, tagged, "a language ships chrome with no summary")
                self.assertIn(f"Comment[{locale}]=", self.desktop)

    @unittest.skipUnless(BRANDED_LISTING, NEEDS_A_BRANDED_LISTING)
    def test_a_branded_listing_covers_every_catalog_locale(self):
        """Adding a language to the app without translating the store copy is caught here.

        Split from the test above deliberately. That one asserts every locale the listing *carries*
        reaches both files, which is true of any listing; this one asserts a branded listing carries
        them all, which is the rule and is false of the neutral default by design -- English only,
        because AppStream falls a reader back to the untagged paragraph and nobody has reviewed
        unbranded copy in six more languages.
        """
        self.assertEqual(
            set(meta.summaries(self.listing)), set(meta.catalog_locales()),
            "a catalog language has no store translation",
        )

    def test_both_files_carry_the_injected_name_not_a_line_of_copy(self):
        """The software centre and the launcher must agree about what the app is called.

        The name is `MAILCAL_APP_NAME` (docs/branding.md), so a listing does not carry one: two
        sources for one fact means the name a reader sees is whichever the generator happened to
        read, and an unbranded build would announce the brand it was stripped of.
        """
        name = meta.brand.value("MAILCAL_APP_NAME")
        root = ElementTree.fromstring(self.metainfo)
        self.assertEqual(root.findtext("name"), name)
        self.assertIn("Name=%s" % name, self.desktop)

    def test_the_desktop_entry_claims_nothing_the_client_cannot_do(self):
        self.assertIn("MimeType=x-scheme-handler/mailto;", self.desktop)
        self.assertIn("Exec=mailcal %u", self.desktop)
        self.assertIn(f"Icon={meta.APP_ID}", self.desktop)

    def test_no_unsubstituted_placeholder_survives(self):
        self.assertNotIn("@", self.metainfo.split("<description>")[0].replace("allodia.eu", ""))


class Gallery(unittest.TestCase):
    """The screenshots block, which is absent until the captures are published."""

    SHOT = {
        "screenshots": [
            {
                "caption": {"en": "Your inbox", "nl": "Je postvak IN"},
                "width": 2732,
                "height": 1844,
                "url": "https://allodia.eu/docs-assets/abc123",
            }
        ]
    }

    def test_a_gallery_renders_the_first_shot_as_the_default(self):
        rendered = meta._screenshots_element(self.SHOT)
        self.assertIn('<screenshot type="default">', rendered)
        self.assertIn("<caption>Your inbox</caption>", rendered)
        self.assertIn('<caption xml:lang="nl">Je postvak IN</caption>', rendered)
        self.assertIn("https://allodia.eu/docs-assets/abc123", rendered)

    def test_no_manifest_emits_a_comment_rather_than_an_empty_element(self):
        # An empty <screenshots/> is invalid AppStream, and invented URLs would validate and then
        # render as a broken gallery in front of a user.
        rendered = meta._screenshots_element(None)
        self.assertNotIn("<screenshots>", rendered)
        self.assertTrue(rendered.strip().startswith("<!--"))


if __name__ == "__main__":
    unittest.main()
