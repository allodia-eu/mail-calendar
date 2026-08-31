#!/usr/bin/env python3
"""Unit tests for the Partner Center client's credential resolution.

The HTTP half of `msstore_api.py` needs a real account and stays untested by construction. What is
testable; and worth testing, because it decides *which Store account a push reaches*; is where the
four credentials come from: an explicit file, the environment, or one of the default locations, and
which of those wins when more than one has an answer.

The failure being guarded is quiet rather than loud: a typo'd key in a credentials file reads
exactly like a value nobody set, and a `--env-file` that is not there would otherwise fall back to
whatever the environment happens to hold; which on a machine that publishes two products is a push
to the wrong one.
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

import msstore_api as subject  # noqa: E402

COMPLETE = """# Partner Center, Allodia Mail & Calendar
MSSTORE_TENANT_ID=tenant-from-file
export MSSTORE_CLIENT_ID = client-from-file

MSSTORE_CLIENT_SECRET="secret from file"
MSSTORE_APP_ID='9NBLGGH000000'
"""


class ParsesTheFile(unittest.TestCase):
    def test_comments_blanks_export_and_quotes_are_all_tolerated(self) -> None:
        values = subject.parse_env_file(COMPLETE)
        self.assertEqual(
            values,
            {
                "MSSTORE_TENANT_ID": "tenant-from-file",
                "MSSTORE_CLIENT_ID": "client-from-file",
                "MSSTORE_CLIENT_SECRET": "secret from file",
                "MSSTORE_APP_ID": "9NBLGGH000000",
            },
        )

    def test_it_is_not_a_shell(self) -> None:
        # No interpolation and no substitution: the value is the literal text after the `=`.
        values = subject.parse_env_file("MSSTORE_APP_ID=$OTHER\nMSSTORE_CLIENT_ID=`id`\n")
        self.assertEqual(values["MSSTORE_APP_ID"], "$OTHER")
        self.assertEqual(values["MSSTORE_CLIENT_ID"], "`id`")

    def test_a_line_that_is_not_an_assignment_is_skipped_not_fatal(self) -> None:
        self.assertEqual(subject.parse_env_file("nonsense\nMSSTORE_APP_ID=9\n"), {"MSSTORE_APP_ID": "9"})


class ResolvesCredentials(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        # A clean environment, and no default file within reach, so each test states its own world.
        for name in (*subject.CREDENTIAL_NAMES, subject.ENV_FILE_VAR):
            self.addCleanup(os.environ.pop, name, None)
            os.environ.pop(name, None)
        original = subject.ENV_FILE_LOCATIONS
        subject.ENV_FILE_LOCATIONS = (self.root / "absent.env",)
        self.addCleanup(setattr, subject, "ENV_FILE_LOCATIONS", original)

    def write(self, text=COMPLETE, name="msstore.env") -> Path:
        path = self.root / name
        path.write_text(text, encoding="utf-8")
        return path

    def test_a_file_supplies_all_four(self) -> None:
        credentials = subject.Credentials.from_env(env_file=str(self.write()))
        self.assertEqual(credentials.tenant_id, "tenant-from-file")
        self.assertEqual(credentials.client_secret, "secret from file")
        self.assertEqual(credentials.app_id, "9NBLGGH000000")

    def test_the_environment_beats_the_file(self) -> None:
        # A file is a convenience, not an override of what the person just typed on the command.
        os.environ["MSSTORE_APP_ID"] = "9FROMENV"
        credentials = subject.Credentials.from_env(env_file=str(self.write()))
        self.assertEqual(credentials.app_id, "9FROMENV")
        self.assertEqual(credentials.tenant_id, "tenant-from-file")

    def test_the_app_id_argument_beats_both(self) -> None:
        os.environ["MSSTORE_APP_ID"] = "9FROMENV"
        credentials = subject.Credentials.from_env("9FROMFLAG", str(self.write()))
        self.assertEqual(credentials.app_id, "9FROMFLAG")

    def test_a_default_location_is_found_without_being_named(self) -> None:
        subject.ENV_FILE_LOCATIONS = (self.root / "absent.env", self.write())
        credentials = subject.Credentials.from_env()
        self.assertEqual(credentials.tenant_id, "tenant-from-file")
        self.assertTrue(credentials.source.endswith("msstore.env"))

    def test_the_env_file_variable_is_honoured(self) -> None:
        os.environ[subject.ENV_FILE_VAR] = str(self.write())
        self.assertEqual(subject.Credentials.from_env().client_id, "client-from-file")

    def test_a_named_file_that_is_absent_is_an_error_not_a_fallback(self) -> None:
        # Falling back would push with whatever account the environment holds; on a machine that
        # publishes two products, that is a push to the wrong listing, reported as a success.
        for name in subject.CREDENTIAL_NAMES:
            os.environ[name] = "from-environment"
        with self.assertRaises(subject.PartnerCenterError):
            subject.Credentials.from_env(env_file=str(self.root / "nope.env"))

    def test_a_missing_value_names_what_is_missing_and_where_it_looked(self) -> None:
        path = self.write("MSSTORE_TENANT_ID=t\n")
        with self.assertRaises(subject.PartnerCenterError) as caught:
            subject.Credentials.from_env(env_file=str(path))
        message = str(caught.exception)
        self.assertIn("MSSTORE_CLIENT_SECRET", message)
        self.assertIn(str(path), message)
        self.assertNotIn("MSSTORE_TENANT_ID,", message)

    def test_a_typod_key_is_pointed_at_rather_than_read_as_absent(self) -> None:
        path = self.write("MSSTORE_TENANT_ID=t\nMSSTORE_CLIENT_ID=c\nMSSTORE_SECRET=s\nMSSTORE_APP_ID=9\n")
        with self.assertRaises(subject.PartnerCenterError) as caught:
            subject.Credentials.from_env(env_file=str(path))
        self.assertIn("MSSTORE_SECRET", str(caught.exception))

    def test_with_nothing_anywhere_the_error_says_where_to_put_a_file(self) -> None:
        with self.assertRaises(subject.PartnerCenterError) as caught:
            subject.Credentials.from_env()
        self.assertIn("No credentials file found", str(caught.exception))
        self.assertIn(subject.ENV_FILE_VAR, str(caught.exception))

    def test_the_secret_is_never_part_of_the_source_label(self) -> None:
        # `source` is printed on every run; it names the file, never what is in it.
        credentials = subject.Credentials.from_env(env_file=str(self.write()))
        self.assertNotIn("secret from file", credentials.source)


class TheGitignore(unittest.TestCase):
    def test_the_repo_local_credentials_file_is_ignored(self) -> None:
        # The script offers `<repo>/.msstore.env` *because* it is ignored. If that line ever goes,
        # the convenient location becomes a committed key.
        ignored = (REPO_ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()
        self.assertIn(subject.ENV_FILE_LOCATIONS[0].name, [line.strip() for line in ignored])


if __name__ == "__main__":
    unittest.main()
