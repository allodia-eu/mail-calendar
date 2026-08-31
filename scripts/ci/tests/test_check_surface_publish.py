#!/usr/bin/env python3
"""Unit tests for the published-surface checker.

The checker's whole value is that it fails, so most of these drive a violation and assert it is
caught; including the two shapes a refactor is most likely to produce: a direct
`self.observer.surface_changed(...)` and a snapshot field taken back out of `Surfaced`.

Two tests read the **real** crate rather than a fixture: that the tree is currently clean, and that
every surface in `PUBLISHED` really has the field the error message tells people to call. A message
naming a field that does not exist would send the next person looking for something that was
renamed out from under it.
"""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MODULE = ROOT / "scripts/ci/check_surface_publish.py"

spec = importlib.util.spec_from_file_location("check_surface_publish", MODULE)
checker = importlib.util.module_from_spec(spec)
sys.modules["check_surface_publish"] = checker
spec.loader.exec_module(checker)


def write(root: Path, relative: str, body: str) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    return path


class SignalTests(unittest.TestCase):
    def setUp(self):
        self._dir = tempfile.TemporaryDirectory()
        self.root = Path(self._dir.name)
        self._real_root = checker.ROOT
        checker.ROOT = self.root

    def tearDown(self):
        checker.ROOT = self._real_root
        self._dir.cleanup()

    def test_a_direct_signal_of_a_published_surface_is_refused(self):
        path = write(
            self.root,
            "crates/mailcal-app/src/sync.rs",
            "fn f(&self) {\n    self.observer.surface_changed(Surface::MailboxList);\n}\n",
        )
        problems = checker.stray_signals([path])
        self.assertEqual(len(problems), 1)
        self.assertIn("Surface::MailboxList", problems[0])
        self.assertIn("sync.rs:2", problems[0])
        # The message must say what to do instead, naming the field.
        self.assertIn("self.mailbox_list.publish", problems[0])
        self.assertIn("resignal", problems[0])

    def test_the_crate_path_prefix_form_is_caught_too(self):
        # `crate::Surface::Reading` is what a file without the import writes, and it was the exact
        # form one of the real call sites used.
        path = write(
            self.root,
            "crates/mailcal-app/src/invitations.rs",
            "self.observer.surface_changed(crate::Surface::Reading);\n",
        )
        self.assertEqual(len(checker.stray_signals([path])), 1)

    def test_an_unpublished_surface_is_left_alone(self):
        # `Settings` and `Connectivity` recompute at pull time; they have no stored snapshot and
        # the rule must not spread to them by accident.
        path = write(
            self.root,
            "crates/mailcal-app/src/display_settings.rs",
            "self.observer.surface_changed(Surface::Settings);\n"
            "self.observer.surface_changed(Surface::Connectivity);\n",
        )
        self.assertEqual(checker.stray_signals([path]), [])

    def test_the_helper_itself_may_signal(self):
        path = write(
            self.root,
            "crates/mailcal-app/src/surfaced.rs",
            "self.observer.surface_changed(Surface::MailboxList);\n",
        )
        self.assertEqual(checker.stray_signals([path]), [])

    def test_a_snapshot_field_demoted_to_a_bare_mutex_is_refused(self):
        path = write(
            self.root,
            "crates/mailcal-app/src/lib.rs",
            "pub struct App {\n    reading: Mutex<ReadingSnapshot>,\n}\n",
        )
        problems = checker.bare_fields([path])
        self.assertEqual(len(problems), 1)
        self.assertIn("Surface::Reading", problems[0])
        self.assertIn("Surfaced<_>", problems[0])

    def test_a_surfaced_field_passes(self):
        path = write(
            self.root,
            "crates/mailcal-app/src/lib.rs",
            "pub struct App {\n    reading: Surfaced<ReadingSnapshot>,\n}\n",
        )
        self.assertEqual(checker.bare_fields([path]), [])

    def test_an_unrelated_mutex_field_is_not_flagged(self):
        path = write(
            self.root,
            "crates/mailcal-app/src/lib.rs",
            "pub struct App {\n    search_query: Mutex<Option<String>>,\n}\n",
        )
        self.assertEqual(checker.bare_fields([path]), [])


class RealTreeTests(unittest.TestCase):
    def test_the_tree_is_currently_clean(self):
        self.assertEqual(checker.main(), 0)

    def test_every_published_surface_names_a_field_that_exists(self):
        # The error message tells people to call `self.<field>.publish(...)`. If a field were
        # renamed, the checker would still pass while pointing at nothing.
        lib = (ROOT / "crates/mailcal-app/src/lib.rs").read_text(encoding="utf-8")
        for surface, field in checker.PUBLISHED.items():
            self.assertIn(
                f"{field}: Surfaced<",
                lib,
                f"Surface::{surface} claims field `{field}`, which is not a Surfaced field on App",
            )

    def test_every_published_surface_is_a_real_variant(self):
        protocol = (ROOT / "crates/mailcal-app/src/protocol.rs").read_text(encoding="utf-8")
        for surface in checker.PUBLISHED:
            self.assertIn(f"    {surface},", protocol, f"Surface::{surface} is not a variant")


if __name__ == "__main__":
    unittest.main()
