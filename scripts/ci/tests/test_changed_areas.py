"""Tests for `changed-areas.sh`.

The tag case is the one that matters. The Android and Linux **release** builds are gated on a `v*`
tag, so they only ever run if a tag push turns those areas on. Nothing else in CI would notice if
that stopped being true: the jobs would skip, a skipped job reports no status, `ci-ok` counts
`skipped` as success, and the tag would go out green having built no release variant at all.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "dev"))
from bashtools import bash_argv, bash_problem  # noqa: E402

SCRIPT = Path(__file__).resolve().parents[1] / "changed-areas.sh"

AREAS = ("rust", "apple", "windows", "android", "linux")
ZERO_SHA = "0" * 40


def run(event: dict[str, object], event_name: str = "push") -> dict[str, str]:
    """Run the script against a synthetic event payload; return its `GITHUB_OUTPUT` map."""
    with tempfile.TemporaryDirectory() as tmp:
        event_path = Path(tmp) / "event.json"
        event_path.write_text(json.dumps(event), encoding="utf-8")
        output_path = Path(tmp) / "output.txt"
        output_path.touch()
        subprocess.run(
            bash_argv(str(SCRIPT)),
            check=True,
            capture_output=True,
            env={
                **os.environ,
                "GITHUB_EVENT_NAME": event_name,
                "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_REPOSITORY": "example-org/example-repo",
                "GITHUB_SHA": "deadbeef",
                "GITHUB_OUTPUT": str(output_path),
                # Never inherit a forced set from the surrounding CI run.
                "FORCE_AREAS": "",
            },
        )
        text = output_path.read_text(encoding="utf-8")
    return dict(line.split("=", 1) for line in text.splitlines() if "=" in line)


@unittest.skipIf(bash_problem() != "", bash_problem())
class ChangedAreas(unittest.TestCase):
    def test_a_tag_push_builds_every_area(self) -> None:
        """A newly created ref has an all-zero `before`, so there is no diff base to resolve."""
        areas = run({"before": ZERO_SHA})
        for area in AREAS:
            self.assertEqual(
                areas.get(area),
                "true",
                f"a tag push must build {area}; the release builds are gated on it",
            )

    def test_forced_areas_still_win(self) -> None:
        """The manual-dispatch escape hatch bypasses detection entirely."""
        with tempfile.TemporaryDirectory() as tmp:
            event_path = Path(tmp) / "event.json"
            event_path.write_text("{}", encoding="utf-8")
            output_path = Path(tmp) / "output.txt"
            output_path.touch()
            subprocess.run(
                bash_argv(str(SCRIPT)),
                check=True,
                capture_output=True,
                env={
                    **os.environ,
                    "GITHUB_EVENT_NAME": "workflow_dispatch",
                    "GITHUB_EVENT_PATH": str(event_path),
                    "GITHUB_OUTPUT": str(output_path),
                    "FORCE_AREAS": "linux",
                },
            )
            areas = dict(
                line.split("=", 1)
                for line in output_path.read_text(encoding="utf-8").splitlines()
                if "=" in line
            )
        self.assertEqual(areas.get("linux"), "true")
        self.assertEqual(areas.get("apple"), "false")

    def test_a_docs_only_change_builds_nothing(self) -> None:
        """The property the whole `changes` job exists for, via the `--files` entry point."""
        result = subprocess.run(
            bash_argv(str(SCRIPT), "--files", "-"),
            check=True,
            capture_output=True,
            text=True,
            input="docs/mcp.md\nREADME.md\n",
            env={**os.environ, "FORCE_AREAS": ""},
        )
        for area in AREAS:
            self.assertRegex(result.stdout, rf"{area}\s+false")

    def test_the_renovate_config_builds_nothing(self) -> None:
        """A root file the fail-open arm would otherwise send through all six jobs, macOS at 10x."""
        result = subprocess.run(
            bash_argv(str(SCRIPT), "--files", "-"),
            check=True,
            capture_output=True,
            text=True,
            input="renovate.json\n",
            env={**os.environ, "FORCE_AREAS": ""},
        )
        for area in AREAS:
            self.assertRegex(result.stdout, rf"{area}\s+false")

    def test_the_nightly_rustfmt_pin_builds_only_rust(self) -> None:
        """Renovate bumps this monthly; the fail-open arm would spend all six jobs on a fmt pin."""
        result = subprocess.run(
            bash_argv(str(SCRIPT), "--files", "-"),
            check=True,
            capture_output=True,
            text=True,
            input="rust-nightly.toml\n",
            env={**os.environ, "FORCE_AREAS": ""},
        )
        self.assertRegex(result.stdout, r"rust\s+true")
        for area in AREAS:
            if area != "rust":
                self.assertRegex(result.stdout, rf"{area}\s+false")


if __name__ == "__main__":
    unittest.main()
