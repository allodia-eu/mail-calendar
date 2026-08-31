#!/usr/bin/env python3
"""Tests for `bashtools`; which `bash` the suites spawn.

The rule worth pinning is an **ordering**, and it is invisible to every other test here: on Windows
the `bash.exe` in `System32` is WSL's launcher, it ships with the OS whether or not a working
distribution does, and it wins a bare `shutil.which("bash")`. When WSL is installed but broken it
then fails before reading a word of the script under test, and the errors read as the script
misbehaving. That turned `scripts/dev/gate.sh` permanently red on a Windows dev box; the exact
"a gate that cannot run is a gate people stop running" failure AGENTS.md warns about.

So `find_bash()` preferring Git Bash is not a nicety, and a test that only checked "some bash was
found" would have passed on the broken machine.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bashtools  # noqa: E402


class FindBash(unittest.TestCase):
    def setUp(self) -> None:
        bashtools.find_bash.cache_clear()
        self.addCleanup(bashtools.find_bash.cache_clear)

    def test_whatever_it_picks_can_actually_run_something(self) -> None:
        """Presence is not enough; the broken-WSL launcher exists and is on PATH."""
        chosen = bashtools.find_bash()
        if chosen is None:
            self.skipTest(bashtools.bash_problem())
        probe = subprocess.run(
            [chosen, "-c", "printf ok"], capture_output=True, text=True, timeout=30
        )
        self.assertEqual(probe.stdout.strip(), "ok", f"{chosen} does not run")

    def test_a_problem_is_reported_exactly_when_there_is_one(self) -> None:
        """`bash_problem()` is the skip message, so it must not disagree with `find_bash()`."""
        if bashtools.find_bash() is None:
            self.assertNotEqual(bashtools.bash_problem(), "")
        else:
            self.assertEqual(bashtools.bash_problem(), "")

    def test_an_override_wins(self) -> None:
        """A host that keeps bash somewhere unusual can say so without editing this repo."""
        chosen = bashtools.find_bash()
        if chosen is None:
            self.skipTest(bashtools.bash_problem())
        bashtools.find_bash.cache_clear()
        old = os.environ.get("MAILCAL_BASH")
        os.environ["MAILCAL_BASH"] = chosen
        try:
            self.assertEqual(bashtools._candidates(), [chosen])
        finally:
            if old is None:
                os.environ.pop("MAILCAL_BASH", None)
            else:
                os.environ["MAILCAL_BASH"] = old

    @unittest.skipUnless(sys.platform == "win32", "the WSL launcher only exists on Windows")
    def test_on_windows_git_bash_is_preferred_over_the_one_on_path(self) -> None:
        """
        THE regression. `System32\\bash.exe` is WSL's launcher and wins a bare PATH lookup, so it must
        come last; after every Git Bash candidate; or a broken WSL takes the whole suite down with
        errors that name the wrong culprit.
        """
        os.environ.pop("MAILCAL_BASH", None)
        candidates = [c for c in bashtools._candidates() if c]
        self.assertTrue(candidates, "some candidate must be offered on Windows")

        system32 = [
            i for i, c in enumerate(candidates) if "system32" in c.replace("/", "\\").lower()
        ]
        git_bash = [i for i, c in enumerate(candidates) if c.replace("/", "\\").lower().endswith(
            "\\git\\bin\\bash.exe"
        )]
        if not git_bash:
            self.skipTest("Git for Windows is not installed on this host")
        if system32:
            self.assertLess(
                min(git_bash),
                min(system32),
                "Git Bash must be tried before the System32 (WSL) launcher",
            )

    @unittest.skipUnless(sys.platform == "win32", "Git Bash is the Windows prerequisite")
    def test_on_windows_the_chosen_bash_is_not_wsl(self) -> None:
        chosen = bashtools.find_bash()
        if chosen is None:
            self.skipTest(bashtools.bash_problem())
        self.assertNotIn(
            "system32",
            chosen.replace("/", "\\").lower(),
            "the WSL launcher was chosen; Git Bash is the supported interpreter on Windows",
        )


class BashArgv(unittest.TestCase):
    def test_it_builds_an_argv_for_subprocess(self) -> None:
        if bashtools.find_bash() is None:
            self.skipTest(bashtools.bash_problem())
        argv = bashtools.bash_argv("-c", "printf ok")
        self.assertEqual(argv[1:], ["-c", "printf ok"])
        probe = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        self.assertEqual(probe.stdout.strip(), "ok")


class BashPath(unittest.TestCase):
    """The other half of the same trap: the path a script resolves is not the one Python holds."""

    def setUp(self) -> None:
        if bashtools.find_bash() is None:
            self.skipTest(bashtools.bash_problem())
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)

    def test_it_answers_what_the_script_itself_would_resolve(self) -> None:
        """The contract: `cd … && pwd` inside bash, which is how every script here finds its root."""
        argv = bashtools.bash_argv("-c", 'cd -- "$1" && pwd', "_", str(self.root))
        done = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        self.assertEqual(bashtools.bash_path(self.root), done.stdout.strip())

    def test_a_windows_path_does_not_survive_as_a_windows_path(self) -> None:
        """The assertion the flatpak suite needed: no drive letter, no backslashes, on any host."""
        resolved = bashtools.bash_path(self.root)
        self.assertTrue(resolved.startswith("/"), f"{resolved} is not a path bash would accept")
        self.assertNotIn("\\", resolved)

    def test_it_refuses_a_directory_that_is_not_there(self) -> None:
        """A silent wrong answer here would surface as an unrelated assertion failing."""
        with self.assertRaises(RuntimeError):
            bashtools.bash_path(self.root / "no-such-directory")


if __name__ == "__main__":
    unittest.main()
