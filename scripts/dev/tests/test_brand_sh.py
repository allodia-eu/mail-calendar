#!/usr/bin/env python3
"""Tests how `brand.sh` resolves a brand value, the shell half of the rule in `docs/branding.md`.

Nearly every script in the tree sources it through `lib.sh`, so it also has to stay cheap: under
Git Bash each process costs tens of milliseconds, and a pipeline per key once made sourcing
`lib.sh` take eleven seconds on Windows. Reading the brand therefore runs no external command,
which `NoProcess` holds by taking `PATH` away.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bashtools  # noqa: E402  (after the path insert, deliberately)

BRAND_SH = Path(__file__).resolve().parents[1] / "brand.sh"
NO_BASH = bashtools.bash_problem()

DEFAULT = "\n".join(
    [
        "# a comment",
        "",
        'MAILCAL_APP_NAME="Neutral & Co"',
        "  export   MAILCAL_APP_ID = org.neutral.client  ",
        "SINGLE='quoted value'",
        "DUP=first",
        'DUP="second"',
        "ONLY_DEFAULT=d",
        "EMPTY_IN_BRAND=default-has-it",
        'SPACES="  inner  "   ',
        "not a pair",
        "9STARTS_WITH_A_DIGIT=x",
        'HALF="open',
        "TAIL=no-newline",
    ]
)
BRAND = 'MAILCAL_APP_ID="eu.brand.app"\r\nEMPTY_IN_BRAND=\r\nCRLF="crlf value"\r\n'
ALLODIA = 'MAILCAL_APP_ID=allodia.older\nONLY_ALLODIA="a"\n'


@unittest.skipIf(NO_BASH, NO_BASH)
class Fixture(unittest.TestCase):
    """The three brand files, written for each test, and a shell that sources `brand.sh` over them."""

    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp())
        for name, text in (("default.env", DEFAULT), ("brand.env", BRAND), ("allodia.env", ALLODIA)):
            (self.dir / name).write_bytes(text.encode("utf-8"))

    def shell(self, body: str, *, brand_file: bool = True, path: str | None = None, **env: str) -> str:
        """`body` after sourcing `brand.sh` against this test's files."""
        files = bashtools.bash_path(self.dir)
        first = f"{files}/brand.env" if brand_file else f"{files}/absent.env"
        script = (
            f'source "{bashtools.bash_path(BRAND_SH.parent)}/brand.sh"\n'
            f'BRAND_DEFAULT_FILE="{files}/default.env"\n'
            f'BRAND_OVERRIDE_FILES=("{first}" "{files}/allodia.env")\n'
            f'{"PATH=" + path if path is not None else ""}\n'
            f"{body}\n"
        )
        # The developer's own brand variables would answer first, so only the test's own reach it.
        environment = {k: v for k, v in os.environ.items() if not k.startswith("MAILCAL_")} | env
        done = subprocess.run(
            bashtools.bash_argv("-c", script), capture_output=True, text=True, env=environment, timeout=60
        )
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def value(self, key: str, **kwargs) -> str:
        out = self.shell(f'brand_value "{key}"', **kwargs)
        self.assertTrue(out.endswith("\n"), "a value is printed as one line")
        return out[:-1]


class BrandSh(Fixture):
    def test_the_publishers_file_answers_before_the_older_name_and_the_default(self) -> None:
        self.assertEqual(self.value("MAILCAL_APP_ID"), "eu.brand.app")
        self.assertEqual(self.value("MAILCAL_APP_ID", brand_file=False), "allodia.older")
        self.assertEqual(self.value("ONLY_ALLODIA"), "a")
        self.assertEqual(self.value("ONLY_DEFAULT"), "d")

    def test_a_key_given_empty_in_an_earlier_file_is_empty(self) -> None:
        self.assertEqual(self.value("EMPTY_IN_BRAND"), "")
        self.assertEqual(self.value("EMPTY_IN_BRAND", brand_file=False), "default-has-it")

    def test_the_environment_answers_first_unless_it_is_empty(self) -> None:
        self.assertEqual(self.value("MAILCAL_APP_ID", MAILCAL_APP_ID="from.env"), "from.env")
        self.assertEqual(self.value("MAILCAL_APP_ID", MAILCAL_APP_ID=""), "eu.brand.app")

    def test_a_value_loses_one_pair_of_quotes_and_its_trailing_space(self) -> None:
        self.assertEqual(self.value("MAILCAL_APP_NAME"), "Neutral & Co")
        self.assertEqual(self.value("SINGLE"), "quoted value")
        self.assertEqual(self.value("SPACES"), "  inner  ")
        self.assertEqual(self.value("CRLF"), "crlf value")
        self.assertEqual(self.value("HALF"), '"open')

    def test_the_last_line_for_a_key_in_a_file_wins(self) -> None:
        self.assertEqual(self.value("DUP"), "second")

    def test_a_key_no_file_names_is_an_empty_line(self) -> None:
        self.assertEqual(self.value("NOT_THERE"), "")

    def test_the_keys_are_every_name_any_file_gives_and_nothing_else(self) -> None:
        self.assertEqual(
            self.shell("brand_keys").split(),
            sorted(
                [
                    "CRLF", "DUP", "EMPTY_IN_BRAND", "HALF", "MAILCAL_APP_ID", "MAILCAL_APP_NAME",
                    "ONLY_ALLODIA", "ONLY_DEFAULT", "SINGLE", "SPACES", "TAIL",
                ]
            ),
        )

    def test_load_exports_what_value_answers(self) -> None:
        out = self.shell(
            'brand_load; for k in MAILCAL_APP_ID MAILCAL_APP_NAME DUP TAIL; do printf "%s=%s\\n" "$k" "${!k}"; done'
        )
        self.assertEqual(
            out.splitlines(),
            ["MAILCAL_APP_ID=eu.brand.app", "MAILCAL_APP_NAME=Neutral & Co", "DUP=second", "TAIL=no-newline"],
        )


class NoProcess(Fixture):
    """With `PATH` empty, any command that is not a builtin fails, and so would the test."""

    def test_load_and_value_run_no_external_command(self) -> None:
        out = self.shell(
            'set -e; brand_load; brand_value MAILCAL_APP_NAME; printf "%s\\n" "$MAILCAL_APP_ID"', path=""
        )
        self.assertEqual(out.splitlines(), ["Neutral & Co", "eu.brand.app"])


if __name__ == "__main__":
    unittest.main()
