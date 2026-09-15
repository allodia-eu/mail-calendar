#!/usr/bin/env python3
"""Tests the build `showcase.sh` puts in front of the Linux camera.

Every other platform is photographed from a binary its own build script produced, and each of those
derives the Allodia sign-in from the injected registration (`core_cargo_features`, BUILDING.md).
Linux is the one arm that calls cargo directly, so it is the one that can lose the sign-in without
anything else noticing: a build without it drops the offer from the first-account screen and the
account category from Settings, and what reaches a store is then a picture of a screen the shipped
bundle never shows. Nothing downstream can tell. The capture is a clean, correctly sized,
showcase-mode PNG of the right screen in the right language, and it passes the byte floor, the
pixel-size assertion and the showcase-launch proof alike.

`build_once` is bash inside a script that would otherwise take screenshots, so it is lifted out by
text and run against a stub cargo, the way the freshness helpers are in `test_showcase_freshness`.
`test_the_function_can_still_be_extracted` fails loudly if a refactor breaks that, since an
extraction that silently yielded nothing would leave the other tests here passing over an empty
shell.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
SHOWCASE = REPO_ROOT / "scripts" / "dev" / "showcase.sh"
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"

EXTRACT = re.compile(r"^build_once\(\) \{.*?^\}$", re.MULTILINE | re.DOTALL)


def function_source() -> str:
    """`build_once`, lifted out of `showcase.sh`."""
    match = EXTRACT.search(SHOWCASE.read_text(encoding="utf-8"))
    assert match, "build_once could not be extracted from showcase.sh"
    return match.group(0)


def cargo_argv(client_id: str) -> list[str]:
    """The arguments the Linux arm hands cargo, for a checkout given `client_id` as its registration.

    The real `core_cargo_features` answers rather than a stub of it: it is the resolver under test,
    and one stubbed out would agree with itself whatever the script then did with the answer. It is
    pointed at an empty scratch root, so the developer's own `.env` cannot decide the result: with
    the real root, the case that asks what a build from source produces would pass or fail by whose
    machine it ran on.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        recorded = root / "cargo-argv"
        snippet = "\n".join(
            (
                "set -euo pipefail",
                f'source "{LIB_SH}"',
                f'REPO_ROOT="{root}"',
                "info() { :; }",
                "BUILD=1",
                "platform=linux",
                # A shell function rather than a stub on PATH: the arm calls cargo from a subshell
                # that has `cd`ed elsewhere, and a subshell inherits functions.
                f'cargo() {{ printf "%s\\n" "$@" >"{recorded}"; }}',
                function_source(),
                "build_once",
            )
        )
        env = dict(os.environ)
        env["MAILCAL_ALLODIA_CLIENT_ID"] = client_id
        done = subprocess.run(
            bash_argv("-c", snippet),
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            env=env,
        )
        assert done.returncode == 0, f"build_once failed: {done.stderr}"
        return recorded.read_text(encoding="utf-8").split()


class LinuxShowcaseBuild(unittest.TestCase):
    def test_the_function_can_still_be_extracted(self) -> None:
        source = function_source()
        self.assertIn("build_once", source)
        self.assertIn("core_cargo_features", source)

    def test_a_registration_puts_the_allodia_sign_in_in_the_photographed_build(self) -> None:
        argv = cargo_argv("an-allodia-client-id")
        self.assertIn("--features", argv)
        self.assertIn("allodia-license", argv)

    def test_a_build_from_source_asks_for_no_feature_at_all(self) -> None:
        self.assertNotIn("--features", cargo_argv(""))

    def test_the_package_photographed_is_the_linux_client(self) -> None:
        self.assertEqual(cargo_argv("")[:3], ["build", "-p", "mailcal-linux"])

    def test_the_harness_trust_path_stays_out_of_a_screenshot_build(self) -> None:
        """`dev-harness` reaches a real server, and this dataset is offline and in memory."""
        self.assertNotIn("dev-harness", " ".join(cargo_argv("an-allodia-client-id")))


if __name__ == "__main__":
    unittest.main()
