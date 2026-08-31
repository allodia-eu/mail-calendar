"""Tests for `check_composer_labels.py`.

Three of these pin the three silent failures the checker exists to catch, and one pins the parsing
that everything else rests on. The fifth runs it against the real repo, which is what turns "the
clients agree today" from a claim into a check.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "check_composer_labels",
    Path(__file__).resolve().parents[1] / "check_composer_labels.py",
)
labels = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(labels)

BUNDLE = """
export interface Labels {
  placeholder: string;
  bold: string;
  indent: string;
}

export const DEFAULT_LABELS: Labels = {
  placeholder: "Write your message",
};
"""

EXPECTED = {"placeholder", "bold", "indent"}


def built(**clients: set[str]) -> dict[str, tuple[str, set[str]]]:
    return {name: (f"{name}/labels", keys) for name, keys in clients.items()}


class BundleKeysTests(unittest.TestCase):
    def test_reads_the_interface_and_not_the_defaults(self) -> None:
        # DEFAULT_LABELS repeats a subset of the same names; picking it up instead would make the
        # expected set depend on how many defaults happen to be listed.
        self.assertEqual(labels.bundle_keys(BUNDLE), EXPECTED)

    def test_a_renamed_interface_is_an_error_not_an_empty_set(self) -> None:
        # An empty expected set would make every client pass, which is the failure mode this whole
        # file is about: the check silently stops checking.
        with self.assertRaises(SystemExit):
            labels.bundle_keys("export interface Chrome {\n  bold: string;\n}\n")


class FailureTests(unittest.TestCase):
    def test_all_clients_in_sync_is_silent(self) -> None:
        found = built(apple=set(EXPECTED), windows=set(EXPECTED))
        self.assertEqual(labels.failures_for(EXPECTED, found, {"apple": True, "windows": True}), [])

    def test_a_client_omitting_one_label_is_named_with_the_key(self) -> None:
        found = built(apple=set(EXPECTED), windows={"placeholder", "bold"})
        failures = labels.failures_for(EXPECTED, found, {"apple": True, "windows": True})
        self.assertEqual(len(failures), 1)
        self.assertIn("windows", failures[0])
        self.assertIn("indent", failures[0])

    def test_a_key_the_bundle_does_not_know_is_reported(self) -> None:
        # Dropped by the bundle's mergeLabels, so the control keeps English while the client looks
        # like it translated it.
        found = built(apple=EXPECTED | {"strikethrough"})
        failures = labels.failures_for(EXPECTED, found, {"apple": True})
        self.assertEqual(len(failures), 1)
        self.assertIn("strikethrough", failures[0])

    def test_a_client_that_builds_the_map_but_never_sends_it_is_reported(self) -> None:
        # The shipped case exactly: nothing is missing, nothing is misspelled, and the toolbar is English.
        found = built(apple=set(EXPECTED))
        failures = labels.failures_for(EXPECTED, found, {"apple": False})
        self.assertEqual(len(failures), 1)
        self.assertIn("never calls", failures[0])


class RepositoryTests(unittest.TestCase):
    def test_the_real_clients_are_in_sync(self) -> None:
        self.assertEqual(labels.main(), 0)


if __name__ == "__main__":
    unittest.main()
