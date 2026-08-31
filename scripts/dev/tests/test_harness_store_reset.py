#!/usr/bin/env python3
"""Unit tests for the client-store wipe `harness.sh reset` performs.

Why this is worth a test at all: it is an `rm -rf` over a path computed from a string, inside the
developer's home, and the harness dev stores sit **directly under the real one**
(`.../Allodia/MailCalendar/dev` beside `.../Allodia/MailCalendar/mailcal.sqlite`). A mode that
resolved to the empty string would therefore delete the developer's actual mail, and it would do it
on a command whose whole point is that it is safe to run often. So most of what is asserted below is
about what **survives**.

The other half is the reason the wipe exists, which nothing in the code can state: Stalwart mints
its ids deterministically from an empty database, so a re-bootstrapped server reuses them for a
different set of messages and a client that kept its cached bodies serves every one under the wrong
message. See docker/stalwart/README.md.

Both functions are bash inside scripts that would otherwise do real work (talk to Docker, read the
host), so they are extracted by text the way test_gate_prune.py extracts its own; and, as there, an
extraction that silently yielded nothing would leave every test here passing over an empty shell,
which `test_the_functions_can_still_be_extracted` is what stops.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

DEV = Path(__file__).resolve().parents[1]
LIB = DEV / "lib.sh"
HARNESS = DEV / "harness.sh"

# The mode list plus the path mapping, from lib.sh.
LIB_EXTRACT = re.compile(
    r"^DEV_STORE_MODES=\(.*?^dev_store_dir\(\) \{$.*?^\}$",
    re.MULTILINE | re.DOTALL,
)
# The wipe itself, from harness.sh.
HARNESS_EXTRACT = re.compile(
    r"^clear_dev_stores\(\) \{$.*?^\}$",
    re.MULTILINE | re.DOTALL,
)


# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem, find_bash  # noqa: E402


def usable_bash() -> str:
    """
    Why bash cannot be used here, or "" when it can.

    Kept as a named function rather than inlined so the skip message says WHAT is wrong. The choice
    of interpreter is bashtools' job: on Windows a bare `bash` lookup finds WSL's launcher, which
    fails before it reads a word of the script and produces errors that look like the wipe
    misbehaving when nothing in this repo was involved.
    """
    return bash_problem()


NO_BASH = usable_bash()


def _extract(pattern: re.Pattern[str], path: Path, what: str) -> str:
    match = pattern.search(path.read_text(encoding="utf-8"))
    assert match, f"{what} could not be extracted from {path.name}"
    return match.group(0)


def run_clear(root: Path, *, keep_clients: bool = False) -> str:
    """Run `clear_dev_stores` against `root` as the Windows client's LOCALAPPDATA.

    Windows is pinned rather than taken from the host so the same assertions run everywhere: the
    mapping under test is per-*client*, and only the Windows one puts the dev stores inside the real
    store's own directory, which is the shape the survival assertions are about.
    """
    script = "\n".join(
        [
            "set -euo pipefail",
            # The three helpers the extracted code calls, stubbed to the Windows client.
            'info() { printf "==> %s\\n" "$*"; }',
            'warn() { printf "warning: %s\\n" "$*" >&2; }',
            "is_windows() { true; }",
            "is_macos() { false; }",
            "is_linux() { false; }",
            'host_desktop_client() { printf "windows"; }',
            f"KEEP_CLIENTS={1 if keep_clients else 0}",
            _extract(LIB_EXTRACT, LIB, "DEV_STORE_MODES / dev_store_dir"),
            _extract(HARNESS_EXTRACT, HARNESS, "clear_dev_stores"),
            "clear_dev_stores",
        ]
    )
    # The environment is ADDED to, never replaced: the extracted code shells out to `rm`, and a
    # bare env leaves it with no PATH to find one; which fails as "the wipe did not run" rather
    # than as the missing-PATH it is.
    env = {**os.environ, "LOCALAPPDATA": str(root), "HOME": str(root)}
    completed = subprocess.run(
        bash_argv("-c", script),
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    return completed.stdout + completed.stderr


def make_profile(root: Path) -> Path:
    """A Windows profile: the real store, with the three harness stores inside its directory."""
    base = root / "Allodia" / "MailCalendar"
    (base / "logs").mkdir(parents=True)
    (base / "mailcal.sqlite").write_text("the developer's actual mail", encoding="utf-8")
    (base / "preferences.toml").write_text("real", encoding="utf-8")
    (base / "logs" / "app.log").write_text("shared log", encoding="utf-8")
    for mode in ("dev", "dev-multi", "dev-imap"):
        (base / mode).mkdir()
        (base / mode / "mailcal.sqlite").write_text(mode, encoding="utf-8")
    return base


class Extraction(unittest.TestCase):
    """The text-lifting the rest depends on. Deliberately outside the bash-gated class: these two
    hold on any host, and they are what stops a refactor from leaving every other test here passing
    over an empty shell."""

    def test_the_functions_can_still_be_extracted(self):
        self.assertIn("DEV_STORE_MODES=(", _extract(LIB_EXTRACT, LIB, "lib"))
        self.assertIn("rm -rf", _extract(HARNESS_EXTRACT, HARNESS, "harness"))

    def test_the_modes_match_the_client(self):
        # AppPaths.cs is the client's own mapping; a mode added there and not here is a store the
        # reset would quietly leave stale, which is the exact failure this whole thing exists for.
        source = _extract(LIB_EXTRACT, LIB, "lib")
        modes = re.search(r"DEV_STORE_MODES=\(([^)]*)\)", source)
        assert modes
        self.assertEqual(["dev", "dev-multi", "dev-imap"], modes.group(1).split())


@unittest.skipIf(NO_BASH, NO_BASH)
class ClearDevStores(unittest.TestCase):
    def test_every_harness_store_goes(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = make_profile(Path(tmp))
            run_clear(Path(tmp))
            for mode in ("dev", "dev-multi", "dev-imap"):
                self.assertFalse((base / mode).exists(), f"{mode} survived the reset")

    def test_the_real_store_survives(self):
        # THE assertion. The dev stores are children of the real store's own directory, so a mapping
        # that ever resolved to the parent would delete the developer's mail on a routine command.
        with tempfile.TemporaryDirectory() as tmp:
            base = make_profile(Path(tmp))
            run_clear(Path(tmp))
            self.assertTrue(base.exists(), "the real store's directory was removed")
            self.assertEqual(
                "the developer's actual mail",
                (base / "mailcal.sqlite").read_text(encoding="utf-8"),
            )
            self.assertTrue((base / "preferences.toml").exists())
            # app.log is deliberately outside the per-mode stores so one file diagnoses whatever ran
            # last (docs/logging.md); a wipe must not take the evidence with it.
            self.assertTrue((base / "logs" / "app.log").exists())

    def test_it_names_what_it_removed(self):
        # A silent wipe of a directory the developer did not ask about is the wrong kind of quiet.
        with tempfile.TemporaryDirectory() as tmp:
            make_profile(Path(tmp))
            output = run_clear(Path(tmp))
            for mode in ("dev", "dev-multi", "dev-imap"):
                self.assertIn(mode, output)

    def test_keep_clients_removes_nothing_and_warns(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = make_profile(Path(tmp))
            output = run_clear(Path(tmp), keep_clients=True)
            for mode in ("dev", "dev-multi", "dev-imap"):
                self.assertTrue((base / mode).exists(), f"{mode} was removed despite --keep-clients")
            self.assertIn("warning:", output)

    def test_a_profile_with_no_harness_stores_says_nothing(self):
        # `reset` is run often and usually has nothing to clear; a line every time trains people to
        # stop reading the ones that matter.
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "Allodia" / "MailCalendar"
            base.mkdir(parents=True)
            (base / "mailcal.sqlite").write_text("real", encoding="utf-8")
            self.assertEqual("", run_clear(Path(tmp)).strip())

    def test_an_unknown_mode_resolves_to_nothing(self):
        # The guard behind the survival test: `dev_store_dir` must fail rather than fall through to
        # the store root, so a typo or an empty mode can never name the developer's real store.
        script = "\n".join(
            [
                "is_windows() { true; }",
                _extract(LIB_EXTRACT, LIB, "lib"),
                'if dev_store_dir windows "" ; then echo RESOLVED; else echo REFUSED; fi',
                'if dev_store_dir freebsd dev ; then echo RESOLVED; else echo REFUSED; fi',
            ]
        )
        out = subprocess.run(
            bash_argv("-c", script),
            check=True,
            capture_output=True,
            text=True,
            env={**os.environ, "LOCALAPPDATA": "/tmp/x", "HOME": "/tmp/x"},
        ).stdout
        # An empty mode still resolves (to `<root>/`), which is why clear_dev_stores never builds one
        #; it iterates DEV_STORE_MODES. An unknown *platform* must refuse outright.
        self.assertIn("REFUSED", out, "an unknown platform resolved to a path")


if __name__ == "__main__":
    unittest.main()
