#!/usr/bin/env python3
"""Unit tests for the development-core gate (`scripts/dev/check-core-build.sh`).

The gate runs inside packaging, against a core that takes minutes to build in release. So the
shapes it must judge are built here as files: a library holding the production marker, one holding
the development marker, one holding neither, and a bundle with the core somewhere inside it. And
the two strings it reads are held against the ones the core compiles in and the ones the Android
and Windows packagers read, since a copy that drifts refuses every production core.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts" / "dev" / "check-core-build.sh"
PRODUCTION = b"mailcal-core-build:production"
DEVELOPMENT = b"mailcal-core-build:development"

# An absolute path to Git Bash on Windows, never System32's WSL launcher (see bashtools).
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem  # noqa: E402


@unittest.skipIf(bash_problem() != "", bash_problem())
class CoreBuildTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(self.root, ignore_errors=True))

    def library(self, name: str, *markers: bytes) -> Path:
        """A binary-looking file holding `markers` between bytes no text tool would expect."""
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"\x7fELF\x00\x01" + b"\x00".join(markers) + b"\x00\xff\xfe")
        return path

    def check(self, *targets: Path):
        return subprocess.run(
            bash_argv(SCRIPT.as_posix(), *(target.as_posix() for target in targets)),
            capture_output=True,
            text=True,
        )

    def test_a_production_core_passes(self):
        result = self.check(self.library("libmailcal_bindings.so", PRODUCTION))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_development_core_is_refused(self):
        result = self.check(self.library("libmailcal_bindings.so", DEVELOPMENT))
        self.assertEqual(result.returncode, 1)
        self.assertIn("DEVELOPMENT core", result.stderr)

    def test_a_core_without_a_marker_is_refused(self):
        result = self.check(self.library("libmailcal_bindings.so"))
        self.assertEqual(result.returncode, 1)
        self.assertIn("no production core marker", result.stderr)

    def test_a_development_marker_anywhere_is_refused(self):
        result = self.check(self.library("mixed.so", PRODUCTION, DEVELOPMENT))
        self.assertEqual(result.returncode, 1)

    def test_a_bundle_is_read_whole(self):
        self.library("App.app/Contents/MacOS/App", PRODUCTION)
        self.library("App.app/Contents/Helpers/relay")
        self.assertEqual(self.check(self.root / "App.app").returncode, 0)
        self.library("App.app/Contents/Frameworks/Core", DEVELOPMENT)
        self.assertEqual(self.check(self.root / "App.app").returncode, 1)

    def test_every_argument_is_judged(self):
        good = self.library("x86_64/libmailcal_bindings.so", PRODUCTION)
        bad = self.library("arm64/libmailcal_bindings.so", DEVELOPMENT)
        result = self.check(good, bad)
        self.assertEqual(result.returncode, 1)
        self.assertIn("arm64", result.stderr)

    def test_a_missing_path_is_refused_rather_than_passed(self):
        self.assertEqual(self.check(self.root / "nowhere.so").returncode, 1)


class MarkerAgreementTests(unittest.TestCase):
    """Every reader looks for what the core compiles in."""

    def test_the_core_and_every_reader_name_the_same_two_markers(self):
        for path in [
            "crates/mailcal-bindings/src/core_build.rs",
            "scripts/dev/check-core-build.sh",
            "clients/android/app/build.gradle.kts",
            "clients/windows/core-build.ps1",
        ]:
            text = (ROOT / path).read_bytes()
            for marker in (PRODUCTION, DEVELOPMENT):
                self.assertIn(marker, text, f"{path} does not name {marker.decode()}")


if __name__ == "__main__":
    unittest.main()
