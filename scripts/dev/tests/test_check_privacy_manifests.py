#!/usr/bin/env python3
"""Unit tests for the Apple privacy-manifest gate (`clients/apple/Scripts/check-privacy-manifests.sh`).

App Store Connect runs the same check on upload (ITMS-91053), and a rejection there costs a build
number and a round trip. The gate runs only inside a packaging run, so the shapes it judges are
built here as ordinary directories: an executable is a file holding the NUL-delimited import names
a Mach-O carries, which is exactly what the gate reads. No Xcode, no signing, no network.
"""

from __future__ import annotations

import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[3] / "clients" / "apple" / "Scripts" / "check-privacy-manifests.sh"
APPLE = SCRIPT.parents[1]

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 (WSL's launcher) before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_problem, find_bash  # noqa: E402

BASH = find_bash()

TIMESTAMP = "NSPrivacyAccessedAPICategoryFileTimestamp"
DISK = "NSPrivacyAccessedAPICategoryDiskSpace"
DEFAULTS = "NSPrivacyAccessedAPICategoryUserDefaults"


def write_bundle(path, imports=(), layout="macos", manifest=None):
    """A bundle at `path` whose executable imports `imports`; `manifest` maps category -> reasons."""
    contents = path / "Contents" if layout == "macos" else path
    exedir = contents / "MacOS" if layout == "macos" else contents
    exedir.mkdir(parents=True, exist_ok=True)
    (contents / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "bin"}))
    (exedir / "bin").write_bytes(b"\xcf\xfa\xed\xfe\0" + b"\0".join(n.encode() for n in imports) + b"\0")
    if manifest is not None:
        where = contents / "Resources" if layout == "macos" else contents
        where.mkdir(parents=True, exist_ok=True)
        (where / "PrivacyInfo.xcprivacy").write_bytes(plistlib.dumps({
            "NSPrivacyTracking": False,
            "NSPrivacyAccessedAPITypes": [
                {"NSPrivacyAccessedAPIType": c, "NSPrivacyAccessedAPITypeReasons": r}
                for c, r in manifest.items()
            ],
        }))


@unittest.skipIf(bash_problem() != "", bash_problem())
class PrivacyManifestTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(self.root, ignore_errors=True))
        self.app = self.root / "AllodiaMail.app"

    def run_check(self):
        return subprocess.run([BASH, SCRIPT.as_posix(), self.app.as_posix()], capture_output=True, text=True)

    def test_a_manifest_covering_every_import_passes(self):
        write_bundle(self.app, ["_fstat", "_statfs", "_OBJC_CLASS_$_NSUserDefaults"],
                     manifest={TIMESTAMP: ["C617.1"], DISK: ["E174.1"], DEFAULTS: ["CA92.1"]})
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("FileTimestamp, DiskSpace, UserDefaults", result.stdout)

    def test_an_ios_layout_reads_the_manifest_from_the_bundle_root(self):
        write_bundle(self.app, ["_stat"], layout="ios", manifest={TIMESTAMP: ["C617.1"]})
        self.assertEqual(self.run_check().returncode, 0)

    def test_an_undeclared_category_fails_naming_the_import(self):
        """The ITMS-91053 case: a dependency starts measuring time and nobody declares it."""
        write_bundle(self.app, ["_fstat", "_mach_absolute_time"], manifest={TIMESTAMP: ["C617.1"]})
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("_mach_absolute_time", result.stderr)
        self.assertIn("NSPrivacyAccessedAPICategorySystemBootTime", result.stderr)

    def test_a_name_only_inside_a_longer_symbol_is_not_an_import(self):
        write_bundle(self.app, ["_fstat_wrapper", "x_statfs"], manifest={})
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_selector_counts_as_a_use(self):
        """`ProcessInfo.systemUptime` reaches the binary as a selector name, not a C symbol."""
        write_bundle(self.app, ["systemUptime"], manifest={})
        self.assertIn("SystemBootTime", self.run_check().stderr)

    def test_a_nested_bundle_without_a_manifest_fails(self):
        write_bundle(self.app, [], manifest={})
        write_bundle(self.app / "Contents/PlugIns/Share.appex", ["_NSURLContentModificationDateKey"])
        result = self.run_check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("Contents/PlugIns/Share.appex: no Contents/Resources/PrivacyInfo.xcprivacy", result.stderr)

    def test_a_reason_from_another_category_fails(self):
        write_bundle(self.app, ["_statfs"], manifest={DISK: ["C617.1"]})
        self.assertIn("C617.1 is not a reason", self.run_check().stderr)

    def test_a_reason_reserved_for_third_party_sdks_fails(self):
        write_bundle(self.app, ["_OBJC_CLASS_$_NSUserDefaults"], manifest={DEFAULTS: ["C56D.1"]})
        self.assertIn("C56D.1 is not a reason", self.run_check().stderr)

    def test_a_category_with_no_reason_fails(self):
        write_bundle(self.app, ["_stat"], manifest={TIMESTAMP: []})
        self.assertIn("declared with no reason", self.run_check().stderr)

    def test_the_committed_manifests_parse_and_give_only_allowed_reasons(self):
        """The three files the build copies in parse, and give only reasons the gate allows."""
        for manifest in ("App", "ShareExtension", "McpHelper"):
            with self.subTest(manifest=manifest):
                shutil.rmtree(self.app, ignore_errors=True)
                write_bundle(self.app, [], manifest={})
                shutil.copy(APPLE / manifest / "PrivacyInfo.xcprivacy",
                            self.app / "Contents/Resources/PrivacyInfo.xcprivacy")
                result = self.run_check()
                self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
