#!/usr/bin/env python3
"""Tests for `scripts/dev/linux-ui/agent.sh`, the Linux UI suite's shell client of the test agent.

It builds each request from `key=value` words in bash and reads the reply off one long-lived
`agent.py --lines`, so what is held here is what a leg depends on and cannot see: that a value
reaches the app as typed, JSON-escaped, and that a refusal stops the step.
"""

from __future__ import annotations

import shlex
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from bashtools import bash_argv  # noqa: E402
from test_agent import ARCHIVE, Server, stand_in  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"
AGENT_SH = REPO_ROOT / "scripts" / "dev" / "linux-ui" / "agent.sh"


def suite_shell(endpoint: str, body: str) -> subprocess.CompletedProcess[str]:
    """Runs `body` with agent.sh sourced and its client connected to `endpoint`."""
    with tempfile.TemporaryDirectory() as artifacts:
        script = (
            f'set -euo pipefail; source "{LIB_SH}"; PYTHON="{sys.executable}"; '
            f'ARTIFACT_DIR="{artifacts}"; source "{AGENT_SH}"; ui_start "$1"; {body}; ui_stop'
        )
        return subprocess.run(
            bash_argv("-c", script, "suite", endpoint), capture_output=True, text=True, timeout=60
        )


@unittest.skipUnless(hasattr(socket, "AF_UNIX"), "no Unix sockets on this host")
class SuiteAgentClient(unittest.TestCase):
    def setUp(self) -> None:
        self.server = Server(stand_in)

    def tearDown(self) -> None:
        self.server.close()

    def test_text_reaches_the_app_as_typed_and_escaped(self) -> None:
        typed = 'Don’t "quote" me\\ on\tthis'
        done = suite_shell(
            self.server.path,
            f"ui query {shlex.quote('text=' + typed)} 'textLike=2024' index=1 "
            "'has={\"text\":\"Inbox\"}' absent=true",
        )
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(
            self.server.requests[-1],
            {
                "verb": "query",
                "text": typed,
                "textLike": "2024",
                "index": 1,
                "has": {"text": "Inbox"},
                "absent": True,
            },
        )

    def test_a_step_without_a_timeout_waits_as_long_as_the_atspi_driver_did(self) -> None:
        done = suite_shell(self.server.path, "ui_press text=Archive")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(
            self.server.requests[0],
            {"verb": "wait", "enabled": True, "text": "Archive", "timeoutMs": 20000},
        )
        self.assertEqual(self.server.requests[-1], {"verb": "activate", "h": ARCHIVE["h"]})

    def test_a_refusal_fails_the_step_and_says_why(self) -> None:
        done = suite_shell(self.server.path, "ui_press role=button")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('"code": "ambiguous"', done.stderr)

    def test_one_names_the_handle_of_a_single_match_and_refuses_two(self) -> None:
        done = suite_shell(self.server.path, 'ui_one text=Archive; printf "%s" "$UI_H"')
        self.assertEqual((done.returncode, done.stdout), (0, str(ARCHIVE["h"])), done.stderr)
        done = suite_shell(self.server.path, "ui_one role=button")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("found 2 elements", done.stderr)

    def test_the_first_text_of_a_match_is_read_back(self) -> None:
        done = suite_shell(self.server.path, "ui_wait text=Archive; ui_first_text")
        self.assertEqual((done.returncode, done.stdout), (0, "Archive"), done.stderr)


if __name__ == "__main__":
    unittest.main()
