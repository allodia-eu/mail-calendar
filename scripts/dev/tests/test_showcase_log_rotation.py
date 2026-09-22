#!/usr/bin/env python3
"""`log_slice_since` still sees a launch whose log rotated underneath it.

The showcase proof reads what a launch appended to the client log after an offset taken before it
started. The logger rotates at 1 MB by renaming `mailcal.log` to `mailcal.log.1`, so a launch that
crosses the limit writes its first lines, the showcase marker among them, into the file that becomes
`.1`. Reading only the fresh file refuses a launch that did enter showcase mode, partway through a
49-shot run.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

LIB = Path(__file__).resolve().parents[1] / "lib.sh"

EXTRACT = re.compile(r"^log_slice_since\(\) \{.*?^\}$", re.MULTILINE | re.DOTALL)


def slice_since(log: Path, offset: int) -> str:
    match = EXTRACT.search(LIB.read_text(encoding="utf-8"))
    assert match, "log_slice_since could not be extracted from lib.sh"
    done = subprocess.run(
        bash_argv("-c", f'set -euo pipefail\n{match.group(0)}\nlog_slice_since "$1" "$2"', "_",
                  str(log), str(offset)),
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout


class LogSliceSinceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = Path(tempfile.mkdtemp())
        self.log = self.dir / "mailcal.log"

    def test_returns_what_was_appended_after_the_offset(self):
        self.log.write_text("earlier\nlaunch marker\n", encoding="utf-8")
        self.assertEqual(slice_since(self.log, len("earlier\n")), "launch marker\n")

    def test_a_rotation_during_the_launch_keeps_the_lines_written_before_it(self):
        before = "earlier session\n"
        self.log.with_name("mailcal.log.1").write_text(before + "launch marker\n", encoding="utf-8")
        self.log.write_text("after rotation\n", encoding="utf-8")
        self.assertEqual(slice_since(self.log, len(before)), "launch marker\nafter rotation\n")

    def test_a_missing_log_is_an_empty_slice(self):
        self.assertEqual(slice_since(self.log, 0), "")


if __name__ == "__main__":
    unittest.main()
