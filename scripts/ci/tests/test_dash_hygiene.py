#!/usr/bin/env python3
"""Tests for the no-dash punctuation checker."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_dash_hygiene as subject  # noqa: E402


class DashRules(unittest.TestCase):
    def scan(self, text: str, suffix: str = ".md"):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / ("sample" + suffix)
            path.write_text(text, encoding="utf-8")
            return list(subject.dash_hits("sample" + suffix, path))

    def test_em_dash_is_found(self) -> None:
        self.assertEqual(len(self.scan("A sentence — with a break.")), 1)

    def test_spaced_en_dash_is_found(self) -> None:
        self.assertEqual(len(self.scan("A sentence – with a break.")), 1)

    def test_range_is_allowed(self) -> None:
        self.assertEqual(self.scan("A–Z and 15–120 min."), [])

    def test_fenced_text_is_quoted(self) -> None:
        self.assertEqual(self.scan("```\nA sentence — with a break.\n```"), [])

    def test_quoted_output_is_left_alone(self) -> None:
        self.assertEqual(self.scan('/// The output is "Jun – Jul 2026".', ".rs"), [])
        self.assertEqual(self.scan("> Captured output – unchanged", ".md"), [])

    def test_matrix_cell_is_left_alone(self) -> None:
        self.assertEqual(self.scan("| Windows | — |", ".md"), [])

    def test_doc_symbol_reference_is_left_alone(self) -> None:
        self.assertEqual(self.scan("/// [`Type–Name`] is documented here.", ".rs"), [])

    def test_code_is_not_scanned(self) -> None:
        self.assertEqual(self.scan('let text = "A sentence — with a break.";\n', ".rs"), [])

    def test_source_only_checks_comments(self) -> None:
        self.assertEqual(len(self.scan('// A sentence — with a break.\n', ".rs")), 1)


class RealTree(unittest.TestCase):
    def test_tree_is_clean(self) -> None:
        self.assertEqual(subject.main(), 0)

    def test_every_swept_area_is_actually_scanned(self) -> None:
        """A root missing from `SWEPT_ROOTS` does not fail; it silently stops guarding that area.

        This checker shipped scanning only `crates`, `clients` and `scripts`, so the whole
        reader-facing half of the sweep (the root markdown, `docs/`, `.agents/skills`, `messages/`
        and `branding/`) was unwatched while reporting OK. Naming a file per area is what makes
        that shape fail here instead of in review.
        """
        scanned = {name for name, _ in subject.tracked()}
        for expected in ("AGENTS.md", "README.md", "docs/calendar.md",
                         ".agents/skills/mail-harness/SKILL.md", "branding/default-listing.md",
                         "crates/mailcal-app/src/lib.rs", "clients/apple/README.md",
                         "scripts/dev/gate.sh"):
            self.assertIn(expected, scanned, f"{expected} is in a swept area but is not scanned")

    def test_an_exemption_names_a_path_that_exists(self) -> None:
        """A stale exempt entry is invisible: it matches nothing and quietly protects nothing."""
        import subprocess
        every = subprocess.run(["git", "-C", str(subject.ROOT), "ls-files"],
                               capture_output=True, text=True, check=True).stdout.splitlines()
        for part in subject.EXEMPT_PARTS:
            self.assertTrue(any(part in name for name in every),
                            f"EXEMPT_PARTS entry {part!r} matches no tracked path")


if __name__ == "__main__":
    unittest.main()
