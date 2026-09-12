#!/usr/bin/env python3
"""Unit tests for the Linux launch barrier: the readiness marker, and the wait built on it.

Two things here can fail silently, and both turn a working client into a launch that reports
itself broken.

* **The marker is a contract between a Rust constant and a shell variable.** Nothing links them
  at build time, so renaming one leaves the other waiting for a line the app will never write:
  every healthy launch then times out, and the report blames the app. `test_the_marker_matches_
  the_client` is the link.
* **`wait_for_log_marker` is the barrier itself.** A wait that returns success without the marker
  would hand every caller an app that is not on screen yet, and one that cannot tell a dead
  process from a slow one spends the whole timeout before misreporting a crash as a slow boot.
  Both are asserted here, because neither shows up as a failure where it happens; it shows up
  later, as a driver acting on a window that is not there.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import textwrap
import unittest
from pathlib import Path

DEV = Path(__file__).resolve().parents[1]
LIB = DEV / "lib.sh"
LOGGER_RS = DEV.parents[1] / "clients" / "linux" / "src" / "logger.rs"

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(DEV))
from bashtools import bash_argv, bash_problem  # noqa: E402

NO_BASH = bash_problem()

MARKER_IN_LIB = re.compile(r"^LINUX_READY_LOG_MARKER='([^']*)'$", re.MULTILINE)
MARKER_IN_RUST = re.compile(
    r'^pub\(crate\) const WINDOW_ON_SCREEN: &str = "([^"]*)";$', re.MULTILINE
)


def marker_in_lib() -> str:
    match = MARKER_IN_LIB.search(LIB.read_text(encoding="utf-8"))
    assert match, "LINUX_READY_LOG_MARKER could not be read from lib.sh"
    return match.group(1)


def marker_in_rust() -> str:
    match = MARKER_IN_RUST.search(LOGGER_RS.read_text(encoding="utf-8"))
    assert match, "WINDOW_ON_SCREEN could not be read from clients/linux/src/logger.rs"
    return match.group(1)


class MarkerAgreement(unittest.TestCase):
    def test_the_marker_matches_the_client(self) -> None:
        """The text the launcher waits for is the text the client writes."""
        self.assertEqual(marker_in_rust(), marker_in_lib())

    def test_the_marker_is_not_empty(self) -> None:
        """An empty marker is in every log slice, so the wait would pass over a dead client."""
        self.assertTrue(marker_in_lib())


@unittest.skipIf(NO_BASH, NO_BASH)
class WaitForLogMarker(unittest.TestCase):
    """The barrier, driven against a real log file and a real process."""

    def run_wait(self, script: str) -> subprocess.CompletedProcess[str]:
        body = textwrap.dedent(
            f"""
            set -uo pipefail
            source "{LIB.as_posix()}"
            {textwrap.dedent(script)}
            """
        )
        return subprocess.run(
            [*bash_argv(), "-c", body],
            capture_output=True,
            text=True,
            timeout=120,
            env={**os.environ, "MAILCAL_BRAND": os.environ.get("MAILCAL_BRAND", "")},
        )

    def test_a_marker_already_there_returns_at_once(self) -> None:
        result = self.run_wait(
            """
            log="$(mktemp)"
            printf 'window on screen\\n' >"$log"
            sleep 600 & pid=$!
            wait_for_log_marker "$log" 0 "$LINUX_READY_LOG_MARKER" 5 "$pid"
            echo "outcome=$?"
            kill "$pid" 2>/dev/null
            """
        )
        self.assertIn("outcome=0", result.stdout, result.stderr)

    def test_a_marker_from_an_earlier_run_does_not_count(self) -> None:
        """The offset is what makes this launch's evidence this launch's own."""
        result = self.run_wait(
            """
            log="$(mktemp)"
            printf 'window on screen\\n' >"$log"
            offset="$(log_size "$log")"
            sleep 600 & pid=$!
            wait_for_log_marker "$log" "$offset" "$LINUX_READY_LOG_MARKER" 1 "$pid"
            echo "outcome=$?"
            kill "$pid" 2>/dev/null
            """
        )
        self.assertIn("outcome=1", result.stdout, result.stderr)

    def test_a_marker_that_arrives_late_is_still_caught(self) -> None:
        result = self.run_wait(
            """
            log="$(mktemp)"
            : >"$log"
            ( sleep 2; printf 'window on screen\\n' >>"$log"; sleep 600 ) & pid=$!
            wait_for_log_marker "$log" 0 "$LINUX_READY_LOG_MARKER" 30 "$pid"
            echo "outcome=$?"
            kill "$pid" 2>/dev/null
            """
        )
        self.assertIn("outcome=0", result.stdout, result.stderr)

    def test_a_client_that_dies_is_reported_before_the_timeout(self) -> None:
        """The answer that stops a crash being read as a slow machine, one timeout later."""
        result = self.run_wait(
            """
            log="$(mktemp)"
            : >"$log"
            sleep 1 & pid=$!
            start="$SECONDS"
            wait_for_log_marker "$log" 0 "$LINUX_READY_LOG_MARKER" 120 "$pid"
            echo "outcome=$?"
            echo "waited=$((SECONDS - start))"
            """
        )
        self.assertIn("outcome=2", result.stdout, result.stderr)
        waited = int(re.search(r"waited=(\d+)", result.stdout).group(1))
        self.assertLess(waited, 30, "a dead client must not cost the full timeout")

    def test_a_client_that_writes_the_marker_then_exits_is_ready(self) -> None:
        """Not a crash: it came up. Racing the two checks the other way round loses this."""
        result = self.run_wait(
            """
            log="$(mktemp)"
            ( printf 'window on screen\\n' >"$log" ) & pid=$!
            wait "$pid"
            wait_for_log_marker "$log" 0 "$LINUX_READY_LOG_MARKER" 5 "$pid"
            echo "outcome=$?"
            """
        )
        self.assertIn("outcome=0", result.stdout, result.stderr)

    def test_a_missing_log_is_a_timeout_not_an_error(self) -> None:
        """The client creates the log itself, so it is routinely absent when the wait starts."""
        result = self.run_wait(
            """
            log="$(mktemp -u)"
            sleep 600 & pid=$!
            wait_for_log_marker "$log" 0 "$LINUX_READY_LOG_MARKER" 1 "$pid"
            echo "outcome=$?"
            kill "$pid" 2>/dev/null
            """
        )
        self.assertIn("outcome=1", result.stdout, result.stderr)


if __name__ == "__main__":
    unittest.main()
