#!/usr/bin/env python3
"""Tests the claim `sdk_cargo` takes on the SDK build directory every checkout shares.

Cargo cannot tell two checkouts apart in one build directory: a checkout whose sources predate
another checkout's build is handed that checkout's binary, and the Linux client then runs code this
tree does not contain. `sdk_cargo` holds a lock for as long as cargo runs and rebuilds this
repository's own crates when the directory's marker names another checkout. These tests run the
real function against a scratch build directory, with the cargo it would run inside the SDK
replaced by a recorder.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

if sys.platform == "linux":
    import fcntl

REPO_ROOT = Path(__file__).resolve().parents[3]
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"
SDK_SH = REPO_ROOT / "scripts" / "dev" / "sdk.sh"


def snippet(build_dir: Path, checkout: str, recorded: Path, clean_fails: bool = False) -> str:
    """A script that runs `sdk_cargo build` as `checkout`, recording each cargo call."""
    clean = "return 1" if clean_fails else ":"
    return "\n".join(
        (
            "set -euo pipefail",
            f'source "{LIB_SH}"',
            f'source "{SDK_SH}"',
            f'REPO_ROOT="{checkout}"',
            "info() { :; }",
            f'sdk_build_dir() {{ printf "%s\\n" "{build_dir}"; }}',
            "sdk_cargo_unclaimed() {",
            f'  printf "%s\\n" "$*" >>"{recorded}"',
            f'  if [[ "$1" == clean ]]; then {clean}; fi',
            "}",
            "sdk_cargo build -p mailcal-linux",
        )
    )


@unittest.skipUnless(sys.platform == "linux", "the GNOME SDK build, and flock, are Linux's")
class SdkClaim(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.build_dir = Path(self.tmp.name) / "build"
        self.recorded = Path(self.tmp.name) / "calls"

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_as(self, checkout: str, clean_fails: bool = False) -> subprocess.CompletedProcess:
        return subprocess.run(
            bash_argv("-c", snippet(self.build_dir, checkout, self.recorded, clean_fails)),
            capture_output=True,
            text=True,
            timeout=30,
        )

    def calls(self) -> list[str]:
        if not self.recorded.exists():
            return []
        calls = self.recorded.read_text(encoding="utf-8").splitlines()
        self.recorded.unlink()
        return calls

    def test_a_directory_nobody_has_claimed_is_cleaned_first(self) -> None:
        self.assertEqual(self.run_as("/checkout/a").returncode, 0)
        self.assertEqual(
            self.calls(), ["clean --workspace --quiet", "build -p mailcal-linux"]
        )

    def test_the_checkout_that_built_last_builds_again_without_a_clean(self) -> None:
        self.run_as("/checkout/a")
        self.calls()
        self.assertEqual(self.run_as("/checkout/a").returncode, 0)
        self.assertEqual(self.calls(), ["build -p mailcal-linux"])

    def test_another_checkout_rebuilds_the_members(self) -> None:
        self.run_as("/checkout/a")
        self.calls()
        self.run_as("/checkout/b")
        self.assertEqual(
            self.calls(), ["clean --workspace --quiet", "build -p mailcal-linux"]
        )

    def test_a_failed_clean_claims_nothing(self) -> None:
        """The next run must try again rather than trust what this one could not replace."""
        self.run_as("/checkout/a", clean_fails=True)
        self.calls()
        self.run_as("/checkout/a")
        self.assertEqual(self.calls()[0], "clean --workspace --quiet")

    def test_a_second_build_waits_for_the_one_holding_the_lock(self) -> None:
        self.build_dir.mkdir(parents=True)
        with open(self.build_dir / ".mailcal-sdk.lock", "w", encoding="utf-8") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            waiting = subprocess.Popen(
                bash_argv("-c", snippet(self.build_dir, "/checkout/a", self.recorded)),
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            time.sleep(1)
            self.assertIsNone(waiting.poll(), "it built while another checkout held the lock")
            self.assertEqual(self.calls(), [])
            fcntl.flock(lock, fcntl.LOCK_UN)
        self.assertEqual(waiting.wait(timeout=30), 0)
        self.assertIn("build -p mailcal-linux", self.calls())


if __name__ == "__main__":
    unittest.main()
