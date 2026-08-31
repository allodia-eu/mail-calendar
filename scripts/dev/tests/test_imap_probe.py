"""Unit tests for the pure helpers in `scripts/dev/imap-probe.py`.

The probe itself needs a real server, so it cannot be tested here; but the two parts that
decide whether a run is *valid* can be, and both fail silently if they are wrong. A password
containing a quote would be sent unescaped and rejected, and the probe would then report a
connection limit that does not exist; a profile key spelled differently in one file would send
an empty password and produce the same false result. Those are the failures that look like
findings, which is exactly why they are worth pinning.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

# The script is a hyphenated executable, not an importable module name.
_SPEC = importlib.util.spec_from_file_location(
    "imap_probe", Path(__file__).resolve().parents[1] / "imap-probe.py"
)
assert _SPEC and _SPEC.loader
imap_probe = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(imap_probe)


class QuoteTests(unittest.TestCase):
    def test_a_plain_password_is_quoted(self):
        self.assertEqual(imap_probe.quote("hunter2"), '"hunter2"')

    def test_a_space_survives(self):
        # Unquoted, IMAP would read this as two arguments and the LOGIN would fail as a
        # syntax error that reads exactly like a refused credential.
        self.assertEqual(imap_probe.quote("two words"), '"two words"')

    def test_a_quote_and_a_backslash_are_escaped(self):
        self.assertEqual(imap_probe.quote('a"b'), '"a\\"b"')
        self.assertEqual(imap_probe.quote("a\\b"), '"a\\\\b"')


class CredentialTests(unittest.TestCase):
    def test_the_imap_spelling_wins_over_the_caldav_one(self):
        # A profile carrying both must not probe IMAP with the CalDAV login.
        profile = {
            "IMAP_USER": "imap@example.com",
            "IMAP_PASS": "imap-secret",
            "CALDAV_USER": "dav@example.com",
            "CALDAV_PASS": "dav-secret",
        }
        self.assertEqual(
            imap_probe.credentials(profile), ("imap@example.com", "imap-secret")
        )

    def test_a_prefixed_profile_is_accepted(self):
        profile = {"SOVERIN_USER": "a@example.com", "SOVERIN_PASS": "s"}
        self.assertEqual(imap_probe.credentials(profile), ("a@example.com", "s"))

    def test_a_profile_with_no_usable_keys_exits_rather_than_probing_blind(self):
        with self.assertRaises(SystemExit):
            imap_probe.credentials({"URL": "https://example.com"})

    def test_an_empty_password_is_not_accepted_as_a_password(self):
        # Otherwise the probe sends `""` and reports every connection refused.
        with self.assertRaises(SystemExit):
            imap_probe.credentials({"USER": "a@example.com", "PASS": ""})


if __name__ == "__main__":
    unittest.main()
