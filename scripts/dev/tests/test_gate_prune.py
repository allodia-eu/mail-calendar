#!/usr/bin/env python3
"""Unit tests for `gate.sh`'s incremental-cache prune.

This is the one place in the repo where a `rm -rf` is driven by a `find`, and the tree it runs
against is the developer's whole `target/` directory. The failure that matters is not "it forgot to
prune"; that only costs disk. It is a widened pattern taking `deps/` with it, or worse something
outside `target/`, which nobody would notice until a three-minute rebuild or a lost file.

So the fixtures are miniature `target/` trees and the assertions are mostly about what **survives**.
Because the function is bash inside a script that would otherwise run the whole gate, it is
extracted by text; `test_the_function_can_still_be_extracted` fails loudly if a refactor breaks that,
since an extraction that silently yields nothing would leave every other test here passing over an
empty shell.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_problem  # noqa: E402

GATE = Path(__file__).resolve().parents[1] / "gate.sh"

# From the cap declaration through the closing brace of the function below it.
EXTRACT = re.compile(
    r"^INCREMENTAL_CAP_GIB=\d+$.*?^prune_incremental\(\) \{$.*?^\}$",
    re.MULTILINE | re.DOTALL,
)


def function_source() -> str:
    """The cap and `prune_incremental`, lifted out of `gate.sh`."""
    match = EXTRACT.search(GATE.read_text(encoding="utf-8"))
    assert match, "prune_incremental could not be extracted from gate.sh"
    return match.group(0)


def run_prune(root: Path, cap_gib: int) -> str:
    """Runs the extracted function in `root` with the cap overridden."""
    script = f"""
        set -e
        yellow=""; reset=""
        {function_source()}
        INCREMENTAL_CAP_GIB={cap_gib}
        prune_incremental
    """
    done = subprocess.run(
        bash_argv("-c", script),
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout


def make_target(root: Path) -> None:
    """A miniature `target/`: two incremental caches, and the artifacts that must survive."""
    for path in (
        "target/debug/incremental/mailcal_app-abc/session/dep-graph.bin",
        "target/aarch64-apple-ios/debug/incremental/mailcal_app-def/session/dep-graph.bin",
        "target/debug/deps/libmailcal_app.rlib",
        "target/debug/.fingerprint/mailcal-app-1/lib-mailcal_app.json",
        "target/debug/build/ring-1/output",
        "target/release/deps/libmailcal_app.rlib",
        "keep-me-outside-target.txt",
    ):
        file = root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(b"x" * 1024)


class ExtractionTests(unittest.TestCase):
    def test_the_function_can_still_be_extracted(self):
        # Guards every other test in this file: a rename or a reshuffle in gate.sh that broke the
        # extraction would otherwise leave them asserting against an empty script, all green.
        source = function_source()
        self.assertIn("prune_incremental() {", source)
        self.assertIn("rm -rf", source)
        self.assertIn("-name incremental", source)

    def shipped_cap(self) -> int:
        cap = re.search(r"^INCREMENTAL_CAP_GIB=(\d+)$", GATE.read_text(), re.MULTILINE)
        self.assertIsNotNone(cap, "the cap declaration moved or was renamed")
        return int(cap.group(1))

    def test_the_shipped_cap_is_present_and_sane(self):
        cap = self.shipped_cap()
        # High enough that ordinary work never trips it, low enough to matter.
        self.assertGreaterEqual(cap, 1)
        self.assertLessEqual(cap, 32)

    def test_the_docs_quote_the_cap_the_script_actually_uses(self):
        # The number is in two places because a reader wants it inline; tuning it in the script and
        # not the doc is a silent lie in the file people read *instead of* the script. Build tuning
        # lives in docs/debugging.md alone; AGENTS.md deliberately carries none of it.
        cap = self.shipped_cap()
        root = GATE.resolve().parents[2]
        text = (root / "docs/debugging.md").read_text(encoding="utf-8")
        self.assertIn(
            f"{cap} GiB cap",
            text,
            f"docs/debugging.md does not quote the shipped cap of {cap} GiB",
        )


@unittest.skipIf(bash_problem() != "", bash_problem())
class PruneTests(unittest.TestCase):
    def test_under_the_cap_nothing_is_touched_and_nothing_is_said(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_target(root)
            output = run_prune(root, cap_gib=4096)
            self.assertEqual(output, "", "a no-op must be silent")
            self.assertTrue((root / "target/debug/incremental").is_dir())

    def test_over_the_cap_only_the_incremental_caches_go(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_target(root)
            output = run_prune(root, cap_gib=0)

            self.assertFalse((root / "target/debug/incremental").exists())
            self.assertFalse(
                (root / "target/aarch64-apple-ios/debug/incremental").exists(),
                "the per-triple caches are cache too",
            )
            # Everything that makes a rebuild ten seconds instead of three minutes.
            for survivor in (
                "target/debug/deps/libmailcal_app.rlib",
                "target/debug/.fingerprint/mailcal-app-1/lib-mailcal_app.json",
                "target/debug/build/ring-1/output",
                "target/release/deps/libmailcal_app.rlib",
                "keep-me-outside-target.txt",
            ):
                self.assertTrue((root / survivor).exists(), f"{survivor} must survive")
            self.assertIn("reclaimed", output, "a prune must say so")

    def test_it_is_a_noop_where_there_is_no_target_directory(self):
        # `gate.sh` is runnable from a fresh clone that has never built.
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(run_prune(Path(tmp), cap_gib=0), "")

    def test_a_target_directory_with_no_incremental_cache_is_left_alone(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target/debug/deps").mkdir(parents=True)
            (root / "target/debug/deps/lib.rlib").write_bytes(b"x")
            self.assertEqual(run_prune(root, cap_gib=0), "")
            self.assertTrue((root / "target/debug/deps/lib.rlib").exists())


if __name__ == "__main__":
    unittest.main()
