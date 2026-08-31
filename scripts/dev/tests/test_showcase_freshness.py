#!/usr/bin/env python3
"""Unit tests for `showcase.sh`'s "am I photographing the right build" helpers.

The failure these guard is the one a screenshot cannot show you. A capture of a stale build; or of
a device you were not driving; is a clean, correctly-sized, showcase-mode PNG of the right screen
in the right language: it clears the byte floor, the pixel-size assertion and the showcase-launch
proof, and it looks right. It cost an afternoon here once, chasing a fix that was already working.

So the guard has to fire, and both halves of it can fail *silently*:

* `newer_source_than` is the comparison itself. One that never fires is indistinguishable from a
  fresh build, and is exactly as reassuring; so the cases here are mostly about it *firing*, and
  about the two ways it could stop: a missing root taking `find` down, and no roots at all leaving
  `find` to walk the working directory and call the first file it meets "newer".
* `file_mtime` only has to produce a date rather than the word "unknown"; its fallback is silent by
  design, and a report that says "app built unknown" is one nobody reads twice.

Both are bash inside a script that would otherwise capture screenshots, so they are extracted by
text; `test_the_functions_can_still_be_extracted` fails loudly if a refactor breaks that, since an
extraction that silently yielded nothing would leave every other test here passing over an empty
shell.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

SHOWCASE = Path(__file__).resolve().parents[1] / "showcase.sh"

EXTRACT = re.compile(
    r"^file_mtime\(\) \{.*?^\}$.*?^newer_source_than\(\) \{.*?^\}$",
    re.MULTILINE | re.DOTALL,
)


def function_source() -> str:
    """`file_mtime` and `newer_source_than`, lifted out of `showcase.sh`."""
    match = EXTRACT.search(SHOWCASE.read_text(encoding="utf-8"))
    assert match, "the freshness helpers could not be extracted from showcase.sh"
    return match.group(0)


def run(snippet: str, cwd: Path) -> str:
    """Runs `snippet` with the extracted helpers in scope."""
    done = subprocess.run(
        bash_argv("-c", f"set -euo pipefail\n{function_source()}\n{snippet}"),
        cwd=cwd,
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout.strip()


def touch(path: Path, when: float) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("x", encoding="utf-8")
    os.utime(path, (when, when))
    return path


class FreshnessHelperTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
        self.now = time.time()

    def test_the_functions_can_still_be_extracted(self):
        source = function_source()
        self.assertIn("file_mtime()", source)
        self.assertIn("newer_source_than()", source)

    def test_a_build_time_is_a_date_and_not_the_fallback(self):
        """`file_mtime` swallows its own errors, so the failure mode is the word "unknown"."""
        binary = touch(self.root / "app", self.now)
        printed = run(f'file_mtime "{binary}"', self.root)
        self.assertRegex(printed, r"^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d$")

    def test_a_source_newer_than_the_build_is_named(self):
        binary = touch(self.root / "app", self.now - 600)
        touch(self.root / "src" / "Model.swift", self.now)
        printed = run(f'newer_source_than "{binary}" "{self.root}/src"', self.root)
        self.assertTrue(printed.endswith("Model.swift"), printed)

    def test_a_build_newer_than_every_source_names_nothing(self):
        binary = touch(self.root / "app", self.now)
        touch(self.root / "src" / "Model.swift", self.now - 600)
        self.assertEqual(run(f'newer_source_than "{binary}" "{self.root}/src"', self.root), "")

    def test_a_missing_root_does_not_hide_a_newer_file_beside_it(self):
        """The roots are per platform and include paths a checkout may not have built yet.

        `find` walks the roots it can reach and only complains about the rest, so this holds with or
        without the filter above it; which is the point of pinning it: the filter exists for the
        empty-list case below, and someone removing it should not have to guess whether *this*
        depended on it.
        """
        binary = touch(self.root / "app", self.now - 600)
        touch(self.root / "src" / "Model.swift", self.now)
        printed = run(
            f'newer_source_than "{binary}" "{self.root}/nope" "{self.root}/src"', self.root
        )
        self.assertTrue(printed.endswith("Model.swift"), printed)

    def test_no_roots_at_all_is_quiet_rather_than_a_whole_tree_scan(self):
        """Every root missing must name nothing; never fall back to `find` with no path, which
        walks the working directory; here, the repo; and would name the first file it meets."""
        binary = touch(self.root / "app", self.now - 600)
        touch(self.root / "src" / "Model.swift", self.now)
        self.assertEqual(run(f'newer_source_than "{binary}" "{self.root}/nope"', self.root), "")


if __name__ == "__main__":
    unittest.main()
