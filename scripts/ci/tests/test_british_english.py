#!/usr/bin/env python3
"""Unit tests for `check_british_english.py`.

A checker whose document-scraping can silently find nothing is not a checker, so each test states
one rule and the last class asserts against the real tree.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_british_english as subject  # noqa: E402


def scan(text: str, suffix: str = ".md"):
    """Every hit in one file's worth of `text`, as `(line, found, wanted)`."""
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / ("sample" + suffix)
        path.write_text(text, encoding="utf-8")
        return list(subject.hits(path))


class ReadsProseOnly(unittest.TestCase):
    def test_markdown_prose_is_read(self) -> None:
        self.assertEqual(scan("The behavior is wrong.\n"), [(1, "behavior", "behaviour")])

    def test_a_fenced_block_is_not_prose(self) -> None:
        """Store copy and shell transcripts are pasted verbatim; they are not ours to correct."""
        self.assertEqual(scan("```\nthe behavior\n```\n"), [])

    def test_an_inline_code_span_is_not_prose(self) -> None:
        self.assertEqual(scan("Call `normalizeColor` first.\n"), [])

    def test_source_outside_a_comment_is_not_prose(self) -> None:
        self.assertEqual(scan("let behavior = 1;\n", ".rs"), [])

    def test_a_source_comment_is_prose(self) -> None:
        self.assertEqual(scan("// The behavior here.\n", ".rs"), [(1, "behavior", "behaviour")])

    def test_a_whole_kdoc_on_one_line_is_prose(self) -> None:
        """`//` needs two slashes and `*` needs the line to start with one, so `/**` matches
        neither marker. Every miss the first sweep left behind was this shape."""
        self.assertEqual(scan("/** The behavior. */\n", ".kt"), [(1, "behavior", "behaviour")])


class LeavesIdentifiersAlone(unittest.TestCase):
    def test_a_snake_case_identifier_is_not_a_spelling(self) -> None:
        self.assertEqual(scan("// see authorization_url for the shape\n", ".rs"), [])

    def test_a_dotted_member_is_not_a_spelling(self) -> None:
        self.assertEqual(scan("// body.center.x is the anchor\n", ".rs"), [])

    def test_a_call_is_not_a_spelling(self) -> None:
        self.assertEqual(scan("// normalize() runs first\n", ".rs"), [])

    def test_a_sentence_ending_full_stop_is_not_an_identifier_boundary(self) -> None:
        """The guard above is why: `.` joins `body.center.x` far less often than it ends a
        sentence, and reading it as a boundary silently exempted every word at a line's end."""
        self.assertEqual(scan("// Confirm the behavior.\n", ".rs"), [(1, "behavior", "behaviour")])

    def test_a_hyphenated_word_is_still_prose(self) -> None:
        self.assertEqual(
            scan("// It re-authorizes the grant\n", ".rs"), [(1, "authorizes", "authorises")]
        )


class LeavesNamesAlone(unittest.TestCase):
    def test_a_spec_phrase_survives_any_punctuation(self) -> None:
        """One PHRASES entry covers the spaced, hyphenated and cased forms."""
        for written in ("the authorization server", "the Authorization-Server", "authorization-url"):
            self.assertEqual(scan("// %s\n" % written, ".rs"), [], written)

    def test_a_symbol_is_matched_exactly(self) -> None:
        self.assertEqual(scan("// NSLocalizedString is the API\n", ".swift"), [])

    def test_a_symbols_casing_does_not_excuse_the_ordinary_word(self) -> None:
        """`Color` is a type and `color` is a word. Flattening the case would lose that."""
        self.assertEqual(scan("// the color key\n", ".rs"), [(1, "color", "colour")])

    def test_a_csharp_doc_reference_is_not_prose(self) -> None:
        """Renaming a cref target compiles and resolves to nothing, which is why it is a symbol."""
        self.assertEqual(scan('/// <see cref="Maximized"/> is true\n', ".cs"), [])

    def test_icalendars_property_keeps_its_spelling(self) -> None:
        self.assertEqual(scan("// ORGANIZER is the property\n", ".rs"), [])

    def test_but_the_role_in_prose_does_not(self) -> None:
        self.assertEqual(scan("// the organizer is waiting\n", ".rs"), [(1, "organizer", "organiser")])


class ReadsTheRealTree(unittest.TestCase):
    """The tests above prove the rules; this proves they still point at the real files."""

    def test_the_tree_is_clean(self) -> None:
        self.assertEqual(subject.main([]), 0)

    def test_it_actually_reads_something(self) -> None:
        """A scan that found no files would pass as loudly as one that found them all."""
        self.assertGreater(len(list(subject.tracked(subject.REPO_ROOT))), 500)

    def test_an_exempt_path_is_skipped(self) -> None:
        names = {name for name, _ in subject.tracked(subject.REPO_ROOT)}
        self.assertNotIn("CODE_OF_CONDUCT.md", names)
        self.assertIn("AGENTS.md", names)


if __name__ == "__main__":
    unittest.main()
