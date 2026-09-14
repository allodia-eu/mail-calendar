#!/usr/bin/env python3
"""Unit tests for the cross-platform release-note check.

Two risks, and they pull in opposite directions. The check can **scrape nothing**, the way every
checker over these hand-written documents can, and then report success over an empty list; so the
release path is exercised against a miniature document whose shape is the real one. And it can be
**too eager**: the notes are seven languages of prose about windows, mail and calendars, so a rule
that fired on the common noun, or on a platform naming itself, would be turned off within a week.
Most of what is asserted below is therefore what the rule deliberately permits.

The fixtures are miniature documents rather than the real ones, so a legitimate edit to a shipped
note never breaks these tests, and a change to the document's *shape* always does.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import changelog_fragments as fragments_module
import check_platform_mentions as subject

LANGUAGES = ("English",)


def fragment_text(*, platforms: str = "all", note: str = "A short release note.") -> str:
    """The markdown of one pending fragment, shaped exactly like a real one."""
    return f"""# A change a user can see

Platforms: {platforms}
Bump: patch

> Why it is shaped this way. Never pasted into a store.

**English**

```
{note}
```
"""


def fragment(slug: str = "a-change", **kwargs) -> fragments_module.Fragment:
    return fragments_module.parse_fragment(fragment_text(**kwargs), slug, LANGUAGES)


def release(*, platforms: str = "all", note: str = "A short release note.") -> str:
    """A miniature released/X.Y.Z.md."""
    return f"""# 0.1.0 — 2026-01-01

## {platforms}

Paste into: whatever

**English**

```
{note}
```

## {fragments_module.APPENDIX_HEADING}

### A change — `a-change` (all, patch)

> Commentary naming Windows, Linux, the Mac and an iPhone, which is not copy and must not be read.
"""


def leaks_in(platforms: str, note: str):
    """The names one note leaks, addressed the way a section's heading spells its platforms."""
    return subject.leaked_names(fragments_module.parse_platforms(platforms, "test"), note)


class TheAppleBoundary(unittest.TestCase):
    """A note may not name the other side of the boundary it is pasted over."""

    def test_an_apple_note_naming_windows_leaks(self) -> None:
        self.assertEqual(leaks_in("macos", "Search moved, on Mac and Windows."), ("Windows",))

    def test_the_0_9_0_sentence_that_reached_app_store_connect_leaks(self) -> None:
        note = "Search is now centred at the top of the window on Mac, Windows and Linux."
        self.assertEqual(leaks_in("macos", note), ("Linux", "Windows"))

    def test_an_ios_note_is_held_to_the_same_rule_as_macos(self) -> None:
        self.assertEqual(leaks_in("ios", "Also on Android."), ("Android",))

    def test_a_windows_note_naming_a_mac_leaks_the_other_way(self) -> None:
        self.assertEqual(leaks_in("windows", "On Mac and Windows the calendar scrolls."), ("Mac",))

    def test_an_android_note_naming_an_iphone_leaks(self) -> None:
        self.assertEqual(leaks_in("android", "Pinch to zoom on iPhone."), ("iPhone",))

    def test_a_linux_note_is_held_to_it_too(self) -> None:
        self.assertEqual(leaks_in("linux", "You pull the list down on iPad."), ("iPad",))


class WhatTheRuleDeliberatelyAllows(unittest.TestCase):
    """The narrowness is the point: a rule that fires on prose gets switched off."""

    def test_an_apple_note_may_name_apple_platforms(self) -> None:
        note = "On iPhone and iPad you pull the list down; the Mac remembers its pane widths."
        self.assertEqual(leaks_in("macos", note), ())

    def test_a_windows_note_may_name_windows(self) -> None:
        self.assertEqual(leaks_in("windows", "On Windows, a screen reader now reads the list."), ())

    def test_windows_naming_linux_is_not_a_leak(self) -> None:
        """Only the Apple boundary is a rejection risk; no other store objects."""
        self.assertEqual(leaks_in("windows", "Also on Linux and Android."), ())

    def test_the_common_noun_is_not_the_brand(self) -> None:
        """Matching is case-sensitive, so a note may talk about the windows it draws."""
        self.assertEqual(leaks_in("macos", "Two windows now sit side by side."), ())

    def test_a_longer_word_is_not_a_platform(self) -> None:
        """Word boundaries: `Mac` must not fire inside `Macintosh`, nor `iPad` inside `iPadOS`."""
        self.assertEqual(leaks_in("windows", "Machine translation and Macintosh-era fonts."), ())
        self.assertEqual(leaks_in("windows", "Sold for the iPadOS era."), ("iPadOS",))


class ANoteReachingBothSides(unittest.TestCase):
    """One body, two consoles: there is no wording that is safe in one and honest in the other."""

    def test_it_may_name_no_platform_at_all(self) -> None:
        self.assertEqual(leaks_in("all", "Now on Mac and Windows."), ("Mac", "Windows"))

    def test_scoped_to_neither_it_passes(self) -> None:
        self.assertEqual(leaks_in("all", "Archiving or deleting opens the next message."), ())


class ReadsPendingFragments(unittest.TestCase):
    """The author's own file is where this has to fail, because that is where the words are."""

    def test_a_leaking_fragment_is_reported_with_its_slug(self) -> None:
        leaks = subject.audit_fragments([fragment(platforms="macos", note="Also on Android.")])
        self.assertEqual(len(leaks), 1)
        self.assertIn("a-change", leaks[0].where)
        self.assertEqual(leaks[0].named, ("Android",))

    def test_a_clean_fragment_reports_nothing(self) -> None:
        self.assertEqual(subject.audit_fragments([fragment(note="A short note.")]), [])


class ReadsAssembledReleases(unittest.TestCase):
    """A release may hand-write a section, so the assembled file is read as well."""

    def audit(self, document: str):
        sections = fragments_module.parse_release(document, "0.1.0")
        return subject.audit_releases([("0.1.0", sections)])

    def test_a_leaking_section_is_reported_with_its_platforms(self) -> None:
        leaks = self.audit(release(platforms="macos", note="Search moved, on Windows too."))
        self.assertEqual(len(leaks), 1)
        self.assertIn("macos", leaks[0].where)
        self.assertEqual(leaks[0].named, ("Windows",))

    def test_the_appendix_is_commentary_and_is_not_read(self) -> None:
        """It names every platform on purpose, and reaches no store."""
        self.assertEqual(self.audit(release(platforms="macos", note="A short note.")), [])

    def test_a_reshaped_document_is_an_error_not_a_pass(self) -> None:
        """The failure this check is most likely to have is finding nothing to object to."""
        with self.assertRaises(fragments_module.DocumentShapeError):
            self.audit("# 0.1.0 — 2026-01-01\n\nNo sections at all.\n")


if __name__ == "__main__":
    unittest.main()
