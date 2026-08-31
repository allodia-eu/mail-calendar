"""The forum announcement `release.py` assembles beside the release note."""

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "ci"))
sys.path.insert(0, str(ROOT / "scripts" / "dev"))

import announcement as subject  # noqa: E402
from changelog_fragments import PLATFORM_STORES, Fragment  # noqa: E402


def fragment(slug, platforms, bump="minor", headline=None, note=None):
    return Fragment(
        slug=slug,
        headline=headline or slug.replace("-", " ").capitalize(),
        platforms=tuple(platforms),
        bump=bump,
        notes={"English": note or f"What {slug} does."},
        commentary="",
    )


EVERYWHERE = ("macos", "ios", "windows", "android", "linux")


class Grouping(unittest.TestCase):
    def test_a_change_on_every_shipping_platform_leads_the_page(self) -> None:
        out = subject.build("1.0.0", "2027-01-01", [fragment("everywhere", EVERYWHERE)])
        self.assertIn("## 🌟 Every app: macOS, iPhone & iPad, Windows and Android", out)
        self.assertNotIn("## 🖥️ macOS", out)

    def test_a_change_reaching_two_apps_is_listed_under_both(self) -> None:
        # The alternative; a section per distinct platform tuple; produces headings like
        # "macOS, iPhone & iPad and Android", which is a spec, not something a reader scans.
        out = subject.build("1.0.0", "2027-01-01", [fragment("drag", ("macos", "android"))])
        self.assertEqual(out.count("- **Drag.**"), 2)
        self.assertIn("## 🖥️ macOS", out)
        self.assertIn("## 🤖 Android", out)

    def test_linux_is_reported_as_unreleased_rather_than_announced(self) -> None:
        out = subject.build("1.0.0", "2027-01-01", [fragment("penguin", ("linux",))])
        self.assertIn("## 🐧 Linux (in development, not yet released)", out)
        self.assertIn("cannot be installed yet", out)

    def test_a_platform_with_no_change_gets_no_heading(self) -> None:
        out = subject.build("1.0.0", "2027-01-01", [fragment("winonly", ("windows",))])
        self.assertIn("## 🪟 Windows", out)
        for absent in ("## 🖥️ macOS", "## 🤖 Android", "## 🐧 Linux"):
            self.assertNotIn(absent, out)


class NewBeforeFixed(unittest.TestCase):
    def test_both_halves_are_labelled_when_both_are_present(self) -> None:
        out = subject.build(
            "1.0.0",
            "2027-01-01",
            [fragment("gain", ("windows",)), fragment("mend", ("windows",), bump="patch")],
        )
        self.assertIn("### ✨ New", out)
        self.assertIn("### 🔧 Fixed", out)
        self.assertLess(out.index("### ✨ New"), out.index("### 🔧 Fixed"))

    def test_a_section_of_only_fixes_carries_no_labels(self) -> None:
        # A "Fixed" heading with no "New" beside it only announces the absence of the other kind.
        out = subject.build(
            "1.0.0", "2027-01-01", [fragment("mend", ("windows",), bump="patch")]
        )
        self.assertNotIn("### 🔧 Fixed", out)
        self.assertIn("- **Mend.**", out)


class Shipping(unittest.TestCase):
    def test_shipping_is_derived_from_the_store_map(self) -> None:
        # The one property that keeps this honest as the product grows: the day Linux gets a store,
        # it stops being described as unreleased without anyone remembering to edit this file.
        self.assertEqual(
            subject.shipping_platforms(),
            tuple(p for p in subject.PLATFORM_NAMES if PLATFORM_STORES[p]),
        )
        self.assertNotIn("linux", subject.shipping_platforms())


class Bullets(unittest.TestCase):
    def test_a_wrapped_note_becomes_one_line(self) -> None:
        # Fragments are wrapped to the repo's column width; a forum post is not.
        out = subject.build(
            "1.0.0",
            "2027-01-01",
            [fragment("wrapped", ("windows",), note="One sentence\nsplit over\nthree lines.")],
        )
        self.assertIn("- **Wrapped.** One sentence split over three lines.", out)

    def test_the_date_is_written_for_a_reader_not_a_field(self) -> None:
        out = subject.build("1.0.0", "2027-01-01", [fragment("a", EVERYWHERE)])
        self.assertIn("Released 1 January 2027.", out)

    def test_the_lead_paragraph_says_it_must_be_replaced(self) -> None:
        # It is a draft, and a placeholder that does not admit it is one gets posted.
        out = subject.build("1.0.0", "2027-01-01", [fragment("a", EVERYWHERE)])
        self.assertIn("REPLACE THIS PARAGRAPH", out)


if __name__ == "__main__":
    unittest.main()
