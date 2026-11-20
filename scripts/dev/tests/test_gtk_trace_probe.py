#!/usr/bin/env python3
"""Tests `gtk-trace.sh probe`, which patches the Linux client's log handler for one run.

The probe writes the text it waits for into Rust source, so the text has to arrive as a string
literal whatever it contains, and the handler has to be put back however the run ends. Both run
against a copy of the scripts over a stand-in `crash.rs`, so the real tree is never patched.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import bashtools  # noqa: E402  (after the path insert, deliberately)

DEV = Path(__file__).resolve().parents[1]
ROOT = DEV.parents[1]
NO_BASH = bashtools.bash_problem()

CRASH_RS = """fn handler(domain: &str, level: glib::LogLevel, message: &str) {
        glib::log_default_handler(domain, level, Some(message));
}
"""


@unittest.skipIf(NO_BASH, NO_BASH)
class Probe(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        (self.root / "scripts/dev").mkdir(parents=True)
        for name in ("gtk-trace.sh", "lib.sh", "brand.sh"):
            shutil.copy(DEV / name, self.root / "scripts/dev" / name)
        (self.root / "branding").mkdir()
        for env in (ROOT / "branding").glob("*.env"):
            shutil.copy(env, self.root / "branding" / env.name)
        self.crash = self.root / "clients/linux/src/crash.rs"
        self.crash.parent.mkdir(parents=True)
        self.crash.write_text(CRASH_RS)
        self.seen = self.root / "seen.rs"

    def probe(self, wanted: str, command: str = "true") -> subprocess.CompletedProcess[str]:
        """Runs the probe for `wanted`; the command it wraps copies the patched handler aside first."""
        script = bashtools.bash_path(self.root / "scripts/dev") + "/gtk-trace.sh"
        crash = bashtools.bash_path(self.crash.parent) + "/crash.rs"
        seen = bashtools.bash_path(self.root) + "/seen.rs"
        return subprocess.run(
            bashtools.bash_argv(script, "probe", wanted, "--", "bash", "-c", f"cp {crash} {seen}; {command}"),
            capture_output=True,
            text=True,
            check=False,
        )

    def test_the_text_is_written_as_a_string_literal_whatever_it_contains(self) -> None:
        for wanted, literal in (
            ("gtk_widget_grab_focus", 'r#"gtk_widget_grab_focus"#'),
            ("it's", 'r#"it\'s"#'),
            ('box != NULL', 'r#"box != NULL"#'),
            ('say "#hi"', 'r##"say "#hi""##'),
        ):
            with self.subTest(wanted=wanted):
                run = self.probe(wanted)
                self.assertEqual(run.returncode, 0, run.stderr)
                self.assertIn(f"if message.contains({literal}) {{", self.seen.read_text())

    def test_the_handler_is_put_back_after_the_run(self) -> None:
        run = self.probe("assertion")
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn("GTKTRACE", self.seen.read_text())
        self.assertEqual(self.crash.read_text(), CRASH_RS)
        self.assertIn("probe removed", run.stderr + run.stdout)

    def test_the_handler_is_put_back_when_the_command_fails(self) -> None:
        run = self.probe("assertion", command="exit 3")
        self.assertNotEqual(run.returncode, 0)
        self.assertEqual(self.crash.read_text(), CRASH_RS)


if __name__ == "__main__":
    unittest.main()
