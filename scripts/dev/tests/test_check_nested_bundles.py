#!/usr/bin/env python3
"""Unit tests for the Apple payload-shape gate (`clients/apple/Scripts/check-nested-bundles.sh`).

The gate itself only ever runs inside a release packaging run, against an archive that takes
minutes to produce and a signing identity most machines do not have. That is precisely the shape of
check that stops being run; and the bug it exists for was found by App Store Connect, not by us,
after a rejection had already burned a build number.

So the shapes it must judge are built here as ordinary directories: an empty nested `.app` (the
0.3.0 iOS rejection, ITMS-90207 + ITMS-90036), a bundle whose Info.plist names an executable that
is not there, a macOS-layout bundle, an iOS-layout one, and a payload carrying a name that platform
must not ship. No Xcode, no signing, no network.
"""

from __future__ import annotations

import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[3] / "clients" / "apple" / "Scripts" / "check-nested-bundles.sh"

# Invoke the gate through bash by name rather than executing it by path. Windows cannot execute a
# `.sh` at all (`OSError: [WinError 193]`), which failed all eleven of these before the shebang was
# ever consulted; so the suite that exists to be runnable anywhere ran on macOS only. bash is not
# a new prerequisite: `scripts/dev/gate.sh`, which runs this suite, is itself a bash script.
# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem, find_bash  # noqa: E402

BASH = find_bash()


def write_bundle(path, executable="helper", layout="macos", plist=True, binary=True):
    """A nested bundle at `path`, complete or broken in one specific way."""
    contents = path / "Contents" if layout == "macos" else path
    exedir = contents / "MacOS" if layout == "macos" else contents
    exedir.mkdir(parents=True, exist_ok=True)
    if plist:
        body = {"CFBundleIdentifier": "eu.allodia.test"}
        if executable is not None:
            body["CFBundleExecutable"] = executable
        (contents / "Info.plist").write_bytes(plistlib.dumps(body))
    if binary and executable is not None:
        (exedir / executable).write_text("#!/bin/sh\n", encoding="utf-8")


@unittest.skipIf(bash_problem() != "", bash_problem())
class NestedBundleTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(self.root, ignore_errors=True))
        self.app = self.root / "AllodiaMail.app"
        write_bundle(self.app, executable="AllodiaMail")

    def run_check(self, *args, app=None):
        # The one place that knows how to invoke the gate; `app` overrides the fixture so a case
        # about a *missing* directory cannot drift into calling it a second way.
        # POSIX-style paths: bash on Windows is MSYS, which reads `C:/…` but treats the
        # backslashes of a native path as escapes.
        return subprocess.run(
            [BASH, SCRIPT.as_posix(), (app or self.app).as_posix(), *args],
            capture_output=True,
            text=True,
        )

    # -- the shapes that must pass -------------------------------------------------------------

    def test_an_app_with_no_nested_bundles_passes(self):
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_complete_nested_bundle_passes(self):
        write_bundle(self.app / "Contents/Library/Helpers/allodia-mcp.app", executable="allodia-mcp")
        self.assertEqual(self.run_check().returncode, 0)

    def test_an_ios_layout_bundle_passes(self):
        """iOS puts Info.plist and the binary at the bundle root, not under Contents/."""
        write_bundle(self.app / "PlugIns/Share.appex", layout="ios")
        self.assertEqual(self.run_check().returncode, 0)

    # -- the shapes App Store delivery rejects --------------------------------------------------

    def test_an_empty_nested_app_fails(self):
        """The 0.3.0 iOS rejection, exactly: four directories and nothing else.

        Xcode created it from the copy phase's declared output path, on a platform where the phase
        copies nothing. `codesign --verify --deep --strict` walked straight past it, because empty
        directories are not code.
        """
        (self.app / "Library/Helpers/allodia-mcp.app/Contents/MacOS").mkdir(parents=True)
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("NO Info.plist", result.stderr)
        self.assertIn("ITMS-90207", result.stderr)

    def test_a_bundle_whose_executable_is_missing_fails(self):
        """ITMS-90207 on its own: the Info.plist is fine and names a binary that is not there."""
        write_bundle(self.app / "Contents/Library/Helpers/allodia-mcp.app",
                     executable="allodia-mcp", binary=False)
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("MISSING executable 'allodia-mcp'", result.stderr)

    def test_a_bundle_with_no_cfbundleexecutable_fails(self):
        write_bundle(self.app / "Contents/Library/Helpers/relay.app", executable=None)
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("NO CFBundleExecutable", result.stderr)

    def test_the_app_itself_is_not_judged_as_its_own_nested_bundle(self):
        """`find` matches the app we were handed unless it is excluded; it is not nested in itself."""
        shutil.rmtree(self.app / "Contents")
        (self.app / "Contents").mkdir()
        self.assertEqual(self.run_check().returncode, 0)

    # -- --forbid: the relay is macOS-only ------------------------------------------------------

    def test_a_forbidden_name_in_the_payload_fails(self):
        write_bundle(self.app / "Library/Helpers/allodia-mcp.app", executable="allodia-mcp")
        result = self.run_check("--forbid", "allodia-mcp")
        self.assertEqual(result.returncode, 1)
        self.assertIn("must not be in this payload", result.stderr)

    def test_a_forbidden_name_that_is_absent_passes(self):
        result = self.run_check("--forbid", "allodia-mcp")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no allodia-mcp", result.stdout)

    def test_a_forbidden_bare_binary_is_caught_too(self):
        """Not only the bundle: the earlier layout dropped a bare Mach-O beside the main executable."""
        (self.app / "Contents/MacOS/allodia-mcp").write_text("x", encoding="utf-8")
        self.assertEqual(self.run_check("--forbid", "allodia-mcp").returncode, 1)

    # -- the CLI ---------------------------------------------------------------------------------

    def test_a_missing_directory_is_an_error_not_a_pass(self):
        result = self.run_check(app=self.root / "nope.app")
        self.assertEqual(result.returncode, 1)
        self.assertIn("not a directory", result.stderr)


if __name__ == "__main__":
    unittest.main()
