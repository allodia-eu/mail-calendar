#!/usr/bin/env python3
"""Tests the marker that lets `screenshot.sh` and `control.sh` find a headless Linux session.

Those calls drive whatever session the marker names, so a marker shared between runs hands one
run's clicks to another's app. A developer's `build-and-run.sh --headless` and an acceptance run
from a second checkout must each keep their own, and a run that stops its session may take away
only the marker it wrote. The functions are run for real against a scratch runtime directory; no
compositor is started, since a marker only records a pid and two socket names.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
SESSION_SH = REPO_ROOT / "scripts" / "dev" / "linux_session.sh"


def run(runtime: Path, checkout: str, *lines: str) -> str:
    """Runs `lines` with linux_session.sh sourced as `checkout`, returning what they print."""
    script = "\n".join(
        (
            "set -euo pipefail",
            f'export XDG_RUNTIME_DIR="{runtime}"',
            f'REPO_ROOT="{checkout}"',
            f'source "{SESSION_SH}"',
            *lines,
        )
    )
    done = subprocess.run(bash_argv("-c", script), capture_output=True, text=True, timeout=30)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def publish(pid: int) -> tuple[str, ...]:
    """Lines that publish a session whose compositor is `pid`."""
    return (
        f"LINUX_SESSION_PID={pid}; LINUX_SESSION_DISPLAY=wayland-9; LINUX_SESSION_SWAYSOCK=/nowhere",
        "linux_session_publish",
    )


@unittest.skipUnless(sys.platform == "linux", "the headless session is Linux's")
class LinuxSessionMarker(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.runtime = Path(self.tmp.name)
        # A live process standing in for the compositor, so the marker is not stale.
        self.compositor = subprocess.Popen(["sleep", "60"])

    def tearDown(self) -> None:
        self.compositor.kill()
        self.compositor.wait()
        self.tmp.cleanup()

    def markers(self) -> list[str]:
        return sorted(p.name for p in self.runtime.glob("mailcal-linux-session*"))

    def test_each_checkout_has_its_own_marker(self) -> None:
        first = run(self.runtime, "/checkout/a", "linux_session_marker")
        second = run(self.runtime, "/checkout/b", "linux_session_marker")
        self.assertNotEqual(first, second)
        self.assertEqual(first, run(self.runtime, "/checkout/a", "linux_session_marker"))

    def test_the_marker_follows_a_runtime_directory_set_after_sourcing(self) -> None:
        """The acceptance suite sources the file first and gives itself a private runtime after."""
        private = self.runtime / "private"
        private.mkdir()
        marker = run(
            self.runtime,
            "/checkout/a",
            f'export XDG_RUNTIME_DIR="{private}"',
            "linux_session_marker",
        )
        self.assertTrue(marker.startswith(str(private)), marker)

    def test_a_published_session_is_found_by_a_later_call_from_the_same_checkout(self) -> None:
        run(self.runtime, "/checkout/a", *publish(self.compositor.pid))
        found = run(
            self.runtime,
            "/checkout/a",
            'linux_session_attach && printf "%s\\n" "$LINUX_SESSION_DISPLAY"',
        )
        self.assertEqual(found, "wayland-9")

    def test_another_checkout_does_not_find_it(self) -> None:
        run(self.runtime, "/checkout/a", *publish(self.compositor.pid))
        self.assertEqual(
            run(self.runtime, "/checkout/b", "linux_session_attach || echo none"), "none"
        )

    def test_stopping_a_session_it_did_not_publish_leaves_the_marker(self) -> None:
        """A run that started and stopped its own compositor must not take a developer's away."""
        run(self.runtime, "/checkout/a", *publish(self.compositor.pid))
        before = self.markers()
        run(self.runtime, "/checkout/a", "LINUX_SESSION_PID=999999999", "linux_session_stop")
        self.assertEqual(self.markers(), before)

    def test_stopping_the_published_session_removes_its_marker(self) -> None:
        run(
            self.runtime,
            "/checkout/a",
            # A child that outlives nothing, so the stop's SIGKILL lands on it rather than the shell.
            "sleep 30 & LINUX_SESSION_PID=$!",
            "LINUX_SESSION_DISPLAY=wayland-9; LINUX_SESSION_SWAYSOCK=/nowhere",
            "linux_session_publish",
            "linux_session_stop",
        )
        self.assertEqual(self.markers(), [])

    def test_starting_a_session_publishes_nothing(self) -> None:
        """Only `build-and-run.sh --headless` means its session to be found."""
        source = SESSION_SH.read_text(encoding="utf-8")
        start = source[source.index("linux_session_start() {") :]
        start = start[: start.index("\n}\n")]
        self.assertNotIn("linux_session_marker", start)
        self.assertNotIn("linux_session_publish", start)


if __name__ == "__main__":
    unittest.main()
