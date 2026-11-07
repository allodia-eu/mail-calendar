#!/usr/bin/env python3
"""Tests that `atspi_registryd_bin` finds the AT-SPI registry where the distribution put it.

Each distribution installs `at-spi2-registryd` into its own libexec: `/usr/libexec` on Ubuntu,
`/usr/lib` on Arch. The one place every one of them names it is the D-Bus service file at-spi2-core
ships, so that is what the resolver reads, and these tests hand it service files of their own
through `XDG_DATA_DIRS` rather than relying on whichever distribution runs them.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem  # noqa: E402

LIB = Path(__file__).resolve().parents[1] / "lib.sh"
SERVICE = Path("dbus-1") / "accessibility-services" / "org.a11y.atspi.Registry.service"


def extract_function(name: str) -> str:
    match = re.search(
        r"^{}\(\) \{{\n(?:.*?\n)*?^\}}\n".format(re.escape(name)),
        LIB.read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"{name}() is no longer a top-level function in lib.sh")
    return match.group(0)


@unittest.skipIf(bash_problem() != "", bash_problem())
@unittest.skipUnless(sys.platform == "linux", "AT-SPI's registry is the Linux suite's")
class AtspiRegistrydBin(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.function = extract_function("atspi_registryd_bin")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def executable(self, relative: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("#!/bin/sh\n", encoding="utf-8")
        path.chmod(0o755)
        return path

    def data_dir(self, name: str, exec_line: str) -> Path:
        directory = self.root / name
        service = directory / SERVICE
        service.parent.mkdir(parents=True)
        service.write_text(
            f"[D-BUS Service]\nName=org.a11y.atspi.Registry\nExec={exec_line}\n",
            encoding="utf-8",
        )
        return directory

    def resolve(self, *data_dirs: Path) -> subprocess.CompletedProcess[str]:
        env = dict(os.environ, XDG_DATA_DIRS=":".join(str(d) for d in data_dirs))
        return subprocess.run(
            bash_argv("-c", self.function + "atspi_registryd_bin"),
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_the_path_the_service_file_names_is_returned_without_its_arguments(self) -> None:
        daemon = self.executable("usr/lib/at-spi2-registryd")
        result = self.resolve(self.data_dir("share", f"{daemon} --use-gnome-session"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"{daemon}\n")

    def test_a_service_file_naming_a_missing_binary_gives_way_to_the_next_data_dir(self) -> None:
        stale = self.data_dir("stale", str(self.root / "usr/libexec/at-spi2-registryd"))
        daemon = self.executable("usr/lib/at-spi2-registryd")
        current = self.data_dir("current", str(daemon))
        result = self.resolve(stale, current)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"{daemon}\n")

    def test_no_service_file_is_a_failure_rather_than_an_empty_path(self) -> None:
        result = self.resolve(self.root / "nothing-here")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
