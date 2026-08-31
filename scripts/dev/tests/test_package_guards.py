#!/usr/bin/env python3
"""Unit tests for the pre-upload guards in `clients/apple/Scripts/package.sh`.

Two of them, both standing between a finished archive and an upload to Apple:

`assert_entitlements_resolved` refuses an entitlements plist that still carries an unexpanded build
setting such as `$(PRODUCT_BUNDLE_IDENTIFIER)`; codesign signs those in as literal text, and the app
then matches no profile.

`assert_symbols_kept` refuses a binary Apple's default `STRIP_STYLE` has emptied, because a crash
stack in a user's log would then name no function.

Neither is reachable without a multi-minute archive on a machine holding a distribution certificate
and a provisioning profile, so both are lifted out of the script and run directly, under the same
shell options the script sets. No Xcode, no certificate, no archive.

They share a failure mode worth naming, because it is invisible: a command substitution whose
pipeline ends non-zero takes `pipefail` and `set -e` with it, and the run stops with **no message**.
`grep -o` matching nothing and `find` given a path that does not exist both do that, and in each case
the non-zero status is the ordinary, correct situation rather than an error. So the assertion
throughout is that the guard is *reached and answers*; silence is the defect.

`assert_symbols_kept` has a second reason to be here: it inspects a macOS bundle
(`Contents/MacOS/<exe>`) and an iOS one (the executable at the top level) with one `find` over both
paths, so one of the two is always absent whichever platform is being packaged.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path
from shlex import quote as shell_quote

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_problem, find_bash  # noqa: E402

PACKAGE_SH = Path(__file__).resolve().parents[3] / "clients" / "apple" / "Scripts" / "package.sh"
BASH = find_bash()

# The marker the harness prints after the call. Reaching it is the whole assertion for the passing
# case: the function returning and the script continuing are the same event here.
CONTINUED = "REACHED-THE-NEXT-STEP"


def extract_function(source, name):
    """A top-level function definition, from `name() {` in column 1 to the `}` that closes it."""
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
class EntitlementsGuard(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.function = extract_function(
            PACKAGE_SH.read_text(encoding="utf-8"), "assert_entitlements_resolved"
        )

    def check(self, plist_body):
        """Run the guard over `plist_body` under the script's own shell options."""
        path = Path(self._tmp) / "entitlements.plist"
        path.write_text(plist_body, encoding="utf-8")
        script = "\n".join(
            [
                # The script's options, which are what turn a no-match into an exit.
                "set -euo pipefail",
                # `fail` as package.sh defines it: a named error on stderr, then a non-zero exit.
                'fail() { echo "error: $*" >&2; exit 1; }',
                self.function,
                "assert_entitlements_resolved {}".format(shell_quote(str(path))),
                'echo "{}"'.format(CONTINUED),
            ]
        )
        return subprocess.run(
            [BASH, "-c", script], capture_output=True, text=True, check=False
        )

    def setUp(self):
        import tempfile

        holder = tempfile.TemporaryDirectory()
        self.addCleanup(holder.cleanup)
        self._tmp = holder.name

    def test_a_fully_resolved_file_lets_the_run_continue(self) -> None:
        """The passing case, which is the one that silently ended the run."""
        done = self.check(
            "<plist><dict>\n"
            "<key>com.apple.security.app-sandbox</key><true/>\n"
            # A stand-in team id, never the real one: check-public-hygiene.sh denies Allodia's
            # reservations by name, and a fixture is exactly where one gets copied in by habit.
            "<key>com.apple.application-identifier</key><string>ABCDE12345.eu.allodia.mailcal</string>\n"
            "</dict></plist>\n"
        )
        self.assertEqual(
            done.returncode,
            0,
            "the guard ended the run on a correct file.\nstdout: {!r}\nstderr: {!r}".format(
                done.stdout, done.stderr
            ),
        )
        self.assertIn(CONTINUED, done.stdout, "the guard returned but the script did not continue")

    def test_an_unexpanded_build_setting_is_refused_by_name(self) -> None:
        """The case the guard exists for; and it has to say which token, or the fix is a hunt."""
        done = self.check(
            "<plist><dict>\n"
            "<key>com.apple.application-identifier</key>\n"
            "<string>$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)</string>\n"
            "</dict></plist>\n"
        )
        self.assertEqual(done.returncode, 1)
        self.assertNotIn(CONTINUED, done.stdout)
        self.assertIn("$(AppIdentifierPrefix)", done.stderr)
        self.assertIn("$(PRODUCT_BUNDLE_IDENTIFIER)", done.stderr)

    def test_a_token_shaped_like_a_shell_variable_is_not_mistaken_for_one(self) -> None:
        """`${FOO}` and a bare `$FOO` are not build settings; only `$(NAME)` is the shape codesign
        would sign in as text, and a guard that flagged the others would refuse valid files."""
        done = self.check(
            "<plist><dict>\n"
            "<key>keychain-access-groups</key><string>group.eu.allodia.mailcal</string>\n"
            "</dict></plist>\n"
        )
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn(CONTINUED, done.stdout)


@unittest.skipIf(bash_problem() != "", bash_problem())
class SymbolsKept(unittest.TestCase):
    """`assert_symbols_kept`, against both bundle shapes it is given."""

    @classmethod
    def setUpClass(cls):
        cls.function = extract_function(
            PACKAGE_SH.read_text(encoding="utf-8"), "assert_symbols_kept"
        )

    def setUp(self):
        import tempfile

        holder = tempfile.TemporaryDirectory()
        self.addCleanup(holder.cleanup)
        self.root = Path(holder.name)

    def bundle(self, shape):
        """An app of `shape` carrying a real executable, so `nm` has something to read."""
        app = self.root / "AllodiaMail.app"
        exe = app / "Contents" / "MacOS" / "AllodiaMail" if shape == "macos" else app / "AllodiaMail"
        exe.parent.mkdir(parents=True, exist_ok=True)
        # A real Mach-O rather than a script: the guard counts what `nm` reports, and a text file
        # would make the count zero for a reason that has nothing to do with stripping.
        shutil.copy("/bin/ls", exe)
        return app

    def run_guard(self, app):
        script = "\n".join(
            [
                "set -euo pipefail",
                'fail() { echo "error: $*" >&2; exit 1; }',
                self.function,
                "assert_symbols_kept {}".format(shell_quote(str(app))),
                'echo "{}"'.format(CONTINUED),
            ]
        )
        return subprocess.run([BASH, "-c", script], capture_output=True, text=True, check=False)

    def assert_answered(self, done, shape):
        """The guard must say something either way; a silent exit is the defect."""
        spoke = CONTINUED in done.stdout or "symbols" in (done.stdout + done.stderr)
        self.assertTrue(
            spoke,
            "the {} bundle ended the run with no diagnostic (exit {}).\nstdout: {!r}\n"
            "stderr: {!r}".format(shape, done.returncode, done.stdout, done.stderr),
        )

    def test_a_macos_bundle_is_inspected_rather_than_ending_the_run(self) -> None:
        self.assert_answered(self.run_guard(self.bundle("macos")), "macOS")

    def test_an_ios_bundle_is_inspected_rather_than_ending_the_run(self) -> None:
        """iOS has no `Contents/MacOS`, so the guard's `find` is handed a path that does not
        exist; the ordinary case for this shape, and not a reason to stop."""
        self.assert_answered(self.run_guard(self.bundle("ios")), "iOS")

    def test_a_bundle_with_no_executable_is_refused_by_name(self) -> None:
        """The miss the guard exists to report, which must not read like the silent exit."""
        empty = self.root / "Empty.app"
        (empty / "Contents" / "MacOS").mkdir(parents=True)
        done = self.run_guard(empty)
        self.assertEqual(done.returncode, 1)
        self.assertIn("no executable found", done.stderr)
        self.assertNotIn(CONTINUED, done.stdout)


if __name__ == "__main__":
    unittest.main()
