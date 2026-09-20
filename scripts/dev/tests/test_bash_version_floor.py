#!/usr/bin/env python3
"""Every shell script in this tree refuses a bash older than 5, and says so on its own first lines.

The floor is what lets the scripts use `mapfile`, `${x^^}` and a plain `"${a[@]}"` over an empty
array under `set -u` (AGENTS.md, "Building & verifying"). Without the guard a 3.2 gets as far as
whichever line first needs one of those, and reports "unbound variable" or "command not found" from
the middle of a build, which reads as the build being broken rather than the shell being old.

macOS is the only host where that happens, and it is also where a new script is most often written,
so the guard is easy to leave out and impossible to notice until someone without Homebrew's bash
runs it.
"""

from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bashtools  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]

# The container images run their own `#!/bin/sh` seeds under dash; they are not bash and are not
# ours to hold to this.
EXEMPT_PREFIXES = ("docker/",)

# Sourced, never run, and only ever from a file that has already checked. A guard of their own
# would be unreachable code. Each says so in its own header; a further one joining them is a
# decision, so it is named here rather than matched by a pattern.
SOURCED_ONLY = frozenset(
    {
        "scripts/dev/brand.sh",
        "scripts/dev/showcase-android.sh",
        "clients/apple/Scripts/provisioning.sh",
    }
)

GUARD = "${BASH_VERSION%%.*}"
SOURCES_LIB = re.compile(r"^\s*(\.|source)\s+.*\blib\.sh", re.MULTILINE)

NO_BASH = bashtools.bash_problem()


def tracked_bash_scripts() -> list[Path]:
    listed = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "*.sh"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    return [
        ROOT / p
        for p in listed
        if not p.startswith(EXEMPT_PREFIXES) and p not in SOURCED_ONLY
    ]


class TheGuard(unittest.TestCase):
    def test_every_bash_script_is_guarded(self) -> None:
        """Directly, or through `lib.sh`, which every script that has one sources before its work."""
        missing = []
        for script in tracked_bash_scripts():
            text = script.read_text(encoding="utf-8")
            if GUARD in text or SOURCES_LIB.search(text):
                continue
            missing.append(script.relative_to(ROOT).as_posix())
        self.assertEqual(
            missing,
            [],
            "these carry neither the bash 5 guard nor lib.sh, so an old bash reaches their work",
        )

    def test_the_guard_comes_before_the_work(self) -> None:
        """A guard under the code it protects fires second, which is not a guard.

        The shebang, comments, and the `set`/`shopt` preamble may precede it: none of them can
        reach a construct an old bash would choke on. Everything else is work.
        """
        late = []
        for script in tracked_bash_scripts():
            lines = script.read_text(encoding="utf-8").splitlines()
            guard = next((n for n, l in enumerate(lines) if GUARD in l), None)
            if guard is None:
                continue
            for n, line in enumerate(lines[1:guard], start=1):
                stripped = line.strip()
                if not stripped or stripped.startswith("#"):
                    continue
                if stripped.split()[0] in ("set", "shopt"):
                    continue
                late.append(f"{script.relative_to(ROOT).as_posix()}:{n + 1} {stripped[:40]}")
                break
        self.assertEqual(late, [], "the guard runs after code it was meant to protect")


@unittest.skipIf(NO_BASH, NO_BASH)
class TheInterpreter(unittest.TestCase):
    def test_the_suites_only_ever_spawn_a_bash_5(self) -> None:
        """`find_bash()` reads the version rather than trusting the name, like every other probe."""
        chosen = bashtools.find_bash()
        assert chosen is not None  # NO_BASH skips otherwise
        probe = subprocess.run(
            [chosen, "-c", 'printf %s "${BASH_VERSION%%.*}"'],
            capture_output=True,
            text=True,
            timeout=30,
        )
        self.assertGreaterEqual(int(probe.stdout.strip()), 5, f"{chosen} is too old")

    def test_an_old_bash_is_refused_even_though_it_runs(self) -> None:
        """The regression: `/bin/bash` on macOS runs perfectly and is still the wrong interpreter."""
        system_bash = Path("/bin/bash")
        if not system_bash.exists():
            self.skipTest("this host has no /bin/bash to ask")
        major = subprocess.run(
            [str(system_bash), "-c", 'printf %s "${BASH_VERSION%%.*}"'],
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout.strip()
        if int(major) >= 5:
            self.skipTest(f"/bin/bash here is already {major}.x, so it is not the case under test")
        self.assertFalse(
            bashtools._runs(str(system_bash)),
            "an interpreter that runs but is too old must be refused, not chosen",
        )


if __name__ == "__main__":
    unittest.main()
