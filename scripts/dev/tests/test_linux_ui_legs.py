#!/usr/bin/env python3
"""Tests for the Linux UI suite's leg list (`scripts/dev/linux-ui/session.sh`).

`--leg` runs one leg after the ones it needs, so the list, the functions and the needs have to
agree: a leg listed without a function fails only when a whole run reaches it, and a need that
comes after its leg would run the leg on a screen it does not expect.
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
SUITE = REPO_ROOT / "scripts" / "dev" / "linux-ui"
PARTS = ["agent", "app", "fixtures", "session", "legs_start", "legs_mail", "legs_calendar",
         "legs_people", "legs_desktop"]


def suite(body: str, *args: str) -> list[str]:
    sources = "; ".join(f'source "{SUITE / part}.sh"' for part in PARTS)
    script = f'source "{REPO_ROOT / "scripts/dev/lib.sh"}"; {sources}; {body}'
    done = subprocess.run(
        bash_argv("-c", script, "suite", *args), capture_output=True, text=True, timeout=30
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.split()


class LinuxUiLegs(unittest.TestCase):
    def setUp(self) -> None:
        self.legs = suite('printf "%s\\n" "${LEGS[@]}"')

    def test_every_leg_has_a_function_and_says_what_it_covers(self) -> None:
        for leg in self.legs:
            with self.subTest(leg=leg):
                self.assertEqual(suite(f'declare -F "leg_{leg.replace("-", "_")}" >/dev/null && '
                                       f'leg_known "{leg}" && echo yes'), ["yes"])

    def test_a_need_is_a_leg_that_runs_before_the_one_needing_it(self) -> None:
        needs = suite('for leg in "${!LEG_NEEDS[@]}"; do echo "$leg=${LEG_NEEDS[$leg]}"; done')
        for pair in needs:
            leg, need = pair.split("=")
            with self.subTest(leg=leg):
                self.assertIn(need, self.legs)
                self.assertLess(self.legs.index(need), self.legs.index(leg))

    def test_one_leg_runs_after_the_first_screens_and_its_needs_in_order(self) -> None:
        self.assertEqual(
            suite('legs_to_run "$@"', "mailto-warm", "search"),
            ["welcome", "zone-question", "notifications", "search", "signatures", "mailto-warm"],
        )

    def test_no_leg_named_runs_them_all(self) -> None:
        self.assertEqual(suite("legs_to_run"), self.legs)


if __name__ == "__main__":
    unittest.main()
