#!/usr/bin/env python3
"""Tests how `linux_session_start` starts sway: the config is still there when sway reads it, sway
runs without its file capabilities and without the developer's environment, and a sway that ends
early is reported with how it ended.

sway opens its Wayland socket before it reads the config it was given, so a start that deletes the
file as soon as the socket appears races the compositor: a loaded runner loses, sway exits with
"Unable to open … for reading", and every GTK test after it fails for want of a display. The
stand-in `sway` here does what sway does, only slower: socket first, config a second later, and
then it answers `swaymsg`, as sway does once its event loop runs.

sway is installed with `cap_sys_nice` and takes realtime scheduling with it; under a hard
`RLIMIT_RTTIME` of 0 the kernel then SIGKILLs it during startup. `no_new_privs` is what keeps the
capability from applying, and the stand-in records whether it was set. A process the kernel
SIGKILLs prints nothing, so the report names the signal.

The stand-in reads its instructions from a file in the runtime directory, because sway starts with
an environment of its own.
"""

from __future__ import annotations

import json
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
    import json, os, signal, socket, sys, time
    runtime = os.environ["XDG_RUNTIME_DIR"]
    fake = json.load(open(os.path.join(runtime, "fake-sway.json")))
    with open(fake["env"], "w") as record:
        json.dump(dict(os.environ), record)
    config = sys.argv[sys.argv.index("-c") + 1]
    server = socket.socket(socket.AF_UNIX)
    server.bind(os.path.join(runtime, "wayland-1"))
    if fake["mode"] == "exit":
        sys.exit(3)
    if fake["mode"] == "killed":
        os.kill(os.getpid(), signal.SIGKILL)
    with open(fake["nnp"], "w") as record:
        record.write(next(line.split()[1] for line in open("/proc/self/status")
                          if line.startswith("NoNewPrivs:")))
    time.sleep(1)
    found = os.path.exists(config)
    with open(fake["record"], "w") as record:
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
        self.nnp = root / "nnp"
        self.env = root / "env.json"
        for name, body in (("sway", FAKE_SWAY), ("swaymsg", FAKE_SWAYMSG), ("grim", "#!/bin/sh\n")):
            path = self.bin / name
            path.write_text(body)
            path.chmod(0o755)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def start(self, mode: str, **extra_env: str) -> subprocess.CompletedProcess[str]:
        (self.runtime / "fake-sway.json").write_text(
            json.dumps(
                {"mode": mode, "record": str(self.record), "nnp": str(self.nnp), "env": str(self.env)}
            )
        )
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
            **extra_env,
        }
        return subprocess.run(
            bash_argv("-c", script), capture_output=True, text=True, timeout=60, env=env
        )

    def test_the_config_is_still_there_when_sway_reads_it(self) -> None:
        done = self.start("slow")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.record.read_text(), "read")
        self.assertEqual(done.stdout.strip(), "wayland-1")

    def test_sway_runs_without_the_privileges_its_file_capabilities_grant(self) -> None:
        done = self.start("slow")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.nnp.read_text(), "1")

    def test_a_sway_that_exits_after_opening_its_socket_is_reported(self) -> None:
        done = self.start("exit")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("did not finish starting", done.stderr)
        self.assertIn("exited with status 3", done.stderr)

    def test_a_sway_the_kernel_kills_is_reported_with_its_signal(self) -> None:
        done = self.start("killed")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("was killed by SIGKILL", done.stderr)
        self.assertIn("ulimit -R", done.stderr)

    def test_sway_gets_none_of_the_developers_session(self) -> None:
        done = self.start(
            "slow",
            DISPLAY=":0",
            WAYLAND_DISPLAY="wayland-0",
            SWAYSOCK="/run/developer/sway.sock",
            XKB_DEFAULT_LAYOUT="de",
            WLR_RENDERER="pixman",
        )
        self.assertEqual(done.returncode, 0, done.stderr)
        env = json.loads(self.env.read_text())
        for name in ("DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK", "XKB_DEFAULT_LAYOUT"):
            self.assertNotIn(name, env)
        self.assertEqual(env["WLR_RENDERER"], "pixman")
        self.assertEqual(env["WLR_BACKENDS"], "headless")


if __name__ == "__main__":
    unittest.main()
