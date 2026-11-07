#!/usr/bin/env python3
"""Tests that `sdk_installation` finds a `--user` SDK from inside a test session.

`flatpak` locates the user installation through `XDG_DATA_HOME`, and the UI acceptance suite gives
the app a private one, so a probe that inherits it reports the SDK missing and the client never
starts. The stand-in `flatpak` here answers for the user installation only when it is handed the
real data directory, the one `flatpak run` already restores.
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
from bashtools import bash_argv, bash_problem  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"
SDK_SH = REPO_ROOT / "scripts" / "dev" / "sdk.sh"

FAKE_FLATPAK = textwrap.dedent(
    """\
    #!/usr/bin/env bash
    [[ "$1" == info ]] || exit 2
    case "$2" in
      --user) [[ "${XDG_DATA_HOME:-}" == "$HOME/.local/share" && -n "${FAKE_USER_SDK:-}" ]] ;;
      --system) [[ -n "${FAKE_SYSTEM_SDK:-}" ]] ;;
      *) exit 2 ;;
    esac
    """
)


@unittest.skipIf(bash_problem() != "", bash_problem())
@unittest.skipUnless(sys.platform == "linux", "the GNOME SDK is Linux's")
class SdkInstallation(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.home = root / "home"
        self.home.mkdir()
        self.bin = root / "bin"
        self.bin.mkdir()
        flatpak = self.bin / "flatpak"
        flatpak.write_text(FAKE_FLATPAK, encoding="utf-8")
        flatpak.chmod(0o755)
        self.session_data = root / "session-data"
        self.session_data.mkdir()

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def installation(self, **sdks: str) -> subprocess.CompletedProcess[str]:
        script = f'source "{LIB_SH}"; source "{SDK_SH}"; sdk_installation 50'
        env = {k: v for k, v in os.environ.items() if not k.startswith("FAKE_")}
        env.update(
            HOME=str(self.home),
            PATH=f"{self.bin}:{os.environ['PATH']}",
            XDG_DATA_HOME=str(self.session_data),
            **sdks,
        )
        return subprocess.run(
            bash_argv("-c", script), env=env, capture_output=True, text=True, check=False
        )

    def test_a_user_sdk_is_found_under_a_redirected_data_dir(self) -> None:
        result = self.installation(FAKE_USER_SDK="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "--user\n")

    def test_a_system_sdk_is_found_when_the_user_has_none(self) -> None:
        result = self.installation(FAKE_SYSTEM_SDK="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "--system\n")

    def test_no_sdk_anywhere_is_a_failure(self) -> None:
        result = self.installation()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
