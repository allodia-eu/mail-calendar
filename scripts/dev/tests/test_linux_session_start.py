#!/usr/bin/env python3
"""Tests that `linux_session_start` hands sway's config over before taking it away.

sway opens its Wayland socket before it reads the config it was given, so a start that deletes the
file as soon as the socket appears races the compositor: a loaded runner loses, sway exits with
"Unable to open … for reading", and every GTK test after it fails for want of a display. The
stand-in `sway` here does what sway does, only slower: socket first, config a second later, and
then it answers `swaymsg`, as sway does once its event loop runs.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
DEV = REPO_ROOT / "scripts" / "dev"

FAKE_SWAY = textwrap.dedent(
    """\
    #!/usr/bin/env python3
    import os, socket, sys, time
    runtime = os.environ["XDG_RUNTIME_DIR"]
    config = sys.argv[sys.argv.index("-c") + 1]
    server = socket.socket(socket.AF_UNIX)
    server.bind(os.path.join(runtime, "wayland-1"))
    if os.environ["FAKE_SWAY_MODE"] == "exit":
        sys.exit(1)
    time.sleep(1)
    found = os.path.exists(config)
    with open(os.environ["FAKE_SWAY_RECORD"], "w") as record:
        record.write("read" if found else "missing")
    if not found:
        sys.exit(1)
    open(os.path.join(runtime, "fake-sway-running"), "w").close()
    time.sleep(60)
    """
)

FAKE_SWAYMSG = '#!/usr/bin/env bash\n[[ -e "$XDG_RUNTIME_DIR/fake-sway-running" ]]\n'


@unittest.skipUnless(sys.platform == "linux", "the headless session is Linux's")
class LinuxSessionStart(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.runtime = root / "runtime"
        self.runtime.mkdir()
        self.bin = root / "bin"
        self.bin.mkdir()
        self.record = root / "record"
        for name, body in (("sway", FAKE_SWAY), ("swaymsg", FAKE_SWAYMSG), ("grim", "#!/bin/sh\n")):
            path = self.bin / name
            path.write_text(body)
            path.chmod(0o755)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def start(self, mode: str) -> subprocess.CompletedProcess[str]:
        script = "\n".join(
            (
                "set -euo pipefail",
                f'source "{DEV / "lib.sh"}"',
                f'source "{DEV / "linux_session.sh"}"',
                "linux_session_start 640x480 1",
                'printf "%s\\n" "$LINUX_SESSION_DISPLAY"',
                "linux_session_stop",
            )
        )
        env = {
            **os.environ,
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "XDG_RUNTIME_DIR": str(self.runtime),
            "FAKE_SWAY_MODE": mode,
            "FAKE_SWAY_RECORD": str(self.record),
        }
        return subprocess.run(
            bash_argv("-c", script), capture_output=True, text=True, timeout=60, env=env
        )

    def test_the_config_is_still_there_when_sway_reads_it(self) -> None:
        done = self.start("slow")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.record.read_text(), "read")
        self.assertEqual(done.stdout.strip(), "wayland-1")

    def test_a_sway_that_exits_after_opening_its_socket_is_reported(self) -> None:
        done = self.start("exit")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("did not finish starting", done.stderr)


if __name__ == "__main__":
    unittest.main()
