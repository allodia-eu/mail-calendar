#!/usr/bin/env python3
"""Tests how `control.sh linux key` turns a chord into the keystrokes `wtype` sends.

A shortcut such as Ctrl+K or Ctrl+Return is most of what `key` exists for, and a chord parsed
wrongly sends a bare letter into whatever has focus: the composer gains a "k", and the test that
meant to open the link dialog reports a dialog that never appeared.
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"
SESSION_SH = REPO_ROOT / "scripts" / "dev" / "linux_session.sh"


def chord(value: str) -> list[str]:
    script = f'source "{LIB_SH}"; source "{SESSION_SH}"; linux_session_chord_args "$1"'
    done = subprocess.run(
        bash_argv("-c", script, "chord", value), capture_output=True, text=True, timeout=30
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.split()


class LinuxSessionChord(unittest.TestCase):
    def test_a_bare_key_is_a_chord_of_one(self) -> None:
        self.assertEqual(chord("Escape"), ["-k", "Escape"])

    def test_modifiers_are_held_around_the_key_and_released(self) -> None:
        self.assertEqual(
            chord("ctrl+shift+Left"),
            ["-M", "ctrl", "-M", "shift", "-k", "Left", "-m", "ctrl", "-m", "shift"],
        )

    def test_a_modifier_is_named_in_wtype_s_lower_case(self) -> None:
        self.assertEqual(chord("Ctrl+k"), ["-M", "ctrl", "-k", "k", "-m", "ctrl"])


if __name__ == "__main__":
    unittest.main()
