#!/usr/bin/env python3
"""Every `require_showcase_launch` call site passes the screen.

`require_showcase_launch <locale> <log-offset> <screen>` reads `$3` to pick which launch marker
proves the app came up: the seeded dataset writes one line, the account-less first-run boot another.
The scripts run under `set -u`, so a caller that passes two arguments does not quietly get an empty
screen and the seeded marker; it dies with `$3: unbound variable` inside a command substitution, and
the run reports `no showcase marker for locale 'en'`, which reads as a bad locale rather than a
miscall.

That is a whole platform's capture set gone, and the arms are one per platform in two files, so
adding a parameter leaves the others compiling fine and failing at the shutter. The reach is what
earns a check: a caller here is reached only by booting a simulator or an emulator, so nothing in
the gate would otherwise observe it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

DEV = Path(__file__).resolve().parents[1]
SCRIPTS = (DEV / "showcase.sh", DEV / "showcase-android.sh")

# A call, not the definition: the definition is `require_showcase_launch() {`.
CALL = re.compile(r"^\s*require_showcase_launch((?: +(?:\"[^\"]*\"|\S+))*)\s*$", re.MULTILINE)


def call_sites() -> list[tuple[Path, int, str]]:
    found = []
    for script in SCRIPTS:
        for line_no, line in enumerate(script.read_text(encoding="utf-8").splitlines(), 1):
            match = CALL.match(line)
            if match:
                found.append((script, line_no, match.group(1).strip()))
    return found


class RequireShowcaseLaunchArityTests(unittest.TestCase):
    def test_the_call_sites_can_still_be_found(self):
        """An extraction that silently found nothing would leave the test below passing."""
        self.assertGreaterEqual(len(call_sites()), 4, call_sites())

    def test_every_call_passes_locale_offset_and_screen(self):
        for script, line_no, args in call_sites():
            with self.subTest(f"{script.name}:{line_no}"):
                self.assertEqual(
                    len(args.split()),
                    3,
                    f"{script.name}:{line_no} passes `{args}`; "
                    "`$3` is the screen and `set -u` makes a missing one fatal at the shutter",
                )


if __name__ == "__main__":
    unittest.main()
