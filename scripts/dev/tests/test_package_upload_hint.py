#!/usr/bin/env python3
"""Unit tests for the upload hint in `clients/apple/Scripts/package.sh`.

The hint is the last thing a release run prints, and the only place the App Store Connect key id and
issuer id are ever assembled into a command. Both come from the git-ignored `signing.local.sh`, so
the interesting cases are about a file that is only *partly* filled in; and none of them can be
reached without a signing identity and a multi-minute archive, which is exactly the shape of check
that stops being run.

So the two functions are lifted out of the script and exercised directly, with the config variables
set by hand. No Xcode, no certificate, no network.

The case that matters is the placeholder one: `signing.local.sh.example` ships the ids commented out
with `REPLACE_WITH_*` text, and someone who uncomments without editing would otherwise be handed a
command line that looks ready to paste and fails against Apple talking about credentials rather than
about the line they just pasted.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

PACKAGE_SH = Path(__file__).resolve().parents[3] / "clients" / "apple" / "Scripts" / "package.sh"

# Invoke through bash by name rather than executing the script by path; the same reason
# test_check_nested_bundles.py does: Windows cannot execute a `.sh` at all, and this suite is part of
# the workspace gate, which runs everywhere.
# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem, find_bash  # noqa: E402

BASH = find_bash()

# A top-level function definition, from `name() {` in column 1 to the `}` in column 1 that closes it.
# The file's style puts every function at column 1 and closes it the same way, so this needs no
# sentinel comments in the script itself.
def extract_function(source, name):
    match = re.search(
        r"^{}\(\) \{{\n(?:.*?\n)*?^\}}\n".format(re.escape(name)), source, re.MULTILINE
    )
    if match is None:
        raise AssertionError(
            "{}() is no longer a top-level function in {}: this test lifts it out by name, so a "
            "rename means updating the test too.".format(name, PACKAGE_SH.name)
        )
    return match.group(0)


@unittest.skipIf(bash_problem() != "", bash_problem())
class UploadHintTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = PACKAGE_SH.read_text(encoding="utf-8")
        cls.functions = extract_function(source, "asc_ids_ready") + extract_function(
            source, "upload_hint"
        )

    def hint(self, artifact="/tmp/AllodiaMail.ipa", platform="ios", key_id="", issuer=""):
        """What package.sh would print for these config values."""
        script = "\n".join(
            [
                'CONFIG="/repo/clients/apple/signing.local.sh"',
                "ASC_API_KEY_ID={}".format(shell_quote(key_id)),
                "ASC_API_ISSUER_ID={}".format(shell_quote(issuer)),
                self.functions,
                "upload_hint {} {}".format(shell_quote(artifact), shell_quote(platform)),
            ]
        )
        done = subprocess.run(
            [BASH, "-c", script], capture_output=True, text=True, check=True
        )
        return done.stdout

    def test_both_ids_set_prints_a_pasteable_command(self):
        out = self.hint(key_id="FAU7K825T3", issuer="6053b7fe-68a3-47e6-a0d3-example")
        self.assertIn("--apiKey FAU7K825T3", out)
        self.assertIn("--apiIssuer 6053b7fe-68a3-47e6-a0d3-example", out)
        self.assertNotIn("<KEY_ID>", out)
        self.assertNotIn("<ISSUER_UUID>", out)
        # Nothing tells the reader to go set what they have already set.
        self.assertNotIn("ASC_API_KEY_ID", out)

    def test_neither_id_set_keeps_the_placeholders(self):
        out = self.hint()
        self.assertIn("--apiKey <KEY_ID>", out)
        self.assertIn("--apiIssuer <ISSUER_UUID>", out)
        # ...and says where to put them, naming the config file the script actually reads.
        self.assertIn("ASC_API_KEY_ID", out)
        self.assertIn("signing.local.sh", out)

    def test_one_id_alone_is_not_enough(self):
        # A half-filled config must not produce a command with one real id and one placeholder,
        # which would read as complete at a glance.
        key_only = self.hint(key_id="FAU7K825T3")
        self.assertIn("--apiKey <KEY_ID>", key_only)
        self.assertNotIn("FAU7K825T3", key_only)

        issuer_only = self.hint(issuer="6053b7fe-68a3-47e6-a0d3-example")
        self.assertIn("--apiIssuer <ISSUER_UUID>", issuer_only)
        self.assertNotIn("6053b7fe", issuer_only)

    def test_template_placeholders_do_not_count_as_configured(self):
        # The exact strings signing.local.sh.example ships, uncommented but not edited.
        out = self.hint(key_id="REPLACE_WITH_KEY_ID", issuer="REPLACE_WITH_ISSUER_UUID")
        self.assertIn("--apiKey <KEY_ID>", out)
        self.assertNotIn("REPLACE", out)

    def test_a_single_edited_id_beside_a_placeholder_still_does_not_count(self):
        out = self.hint(key_id="FAU7K825T3", issuer="REPLACE_WITH_ISSUER_UUID")
        self.assertIn("--apiKey <KEY_ID>", out)
        self.assertNotIn("REPLACE", out)
        self.assertNotIn("FAU7K825T3", out)

    def test_the_artifact_path_is_quoted_and_the_platform_carried_through(self):
        # Release artifacts live under a path with no spaces today, but the quoting is what keeps
        # that from being load-bearing.
        out = self.hint(
            artifact="/Users/me/Library/Mobile Documents/AllodiaMail.pkg",
            platform="macos",
            key_id="FAU7K825T3",
            issuer="6053b7fe-68a3-47e6-a0d3-example",
        )
        self.assertIn('-f "/Users/me/Library/Mobile Documents/AllodiaMail.pkg"', out)
        self.assertIn("-t macos", out)


def shell_quote(value):
    """A single-quoted shell word; the values under test are literals, never expressions."""
    return "'" + value.replace("'", "'\\''") + "'"


if __name__ == "__main__":
    unittest.main()
