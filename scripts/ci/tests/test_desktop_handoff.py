"""Tests for `check-desktop-handoff.sh`.

What it guards cannot be reached by any other test: reproducing the bug needs a Flatpak sandbox and
a live portal, and the same code is perfectly well behaved in a `--host` build, which is the one a
developer iterates in. So the check is the only thing standing between the tree and an app that
freezes on the shape that ships; and a check that stops matching would be silent about it.

The first version of this check passed while the banned call sat in the tree, because `git grep`
exited 128 on a bad pathspec and the script read a failure as "no matches". That is why the error
path is exercised here too, and not only the two happy ones.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess,
# which searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "dev"))
from bashtools import bash_argv, bash_problem  # noqa: E402

SCRIPT = Path(__file__).resolve().parents[1] / "check-desktop-handoff.sh"

PORTAL_SHAPED = """\
fn open_it(uri: &str) {
    gtk::UriLauncher::new(uri).launch(None::<&gtk::Window>, gio::Cancellable::NONE, |_| ());
}
fn open_file(file: &gio::File) {
    gtk::FileLauncher::new(Some(file)).launch(None::<&gtk::Window>, gio::Cancellable::NONE, |_| ());
}
"""


def run(sources: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Write `sources` into a throwaway git repo and run the check over it."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "scripts" / "ci").mkdir(parents=True)
        shutil.copy(SCRIPT, root / "scripts" / "ci" / SCRIPT.name)
        for name, text in sources.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        # The check searches with `git grep`, so it needs a repository to search.
        for args in (["init", "-q"], ["add", "-A"]):
            subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)
        return subprocess.run(
            bash_argv(str(root / "scripts" / "ci" / SCRIPT.name)),
            capture_output=True,
            text=True,
            check=False,
        )


@unittest.skipIf(bash_problem() != "", bash_problem())
class DesktopHandoff(unittest.TestCase):
    def test_the_portal_launchers_pass(self) -> None:
        done = run({"clients/linux/src/ui/open.rs": PORTAL_SHAPED})

        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)

    def test_the_call_that_froze_the_app_is_refused(self) -> None:
        """The exact call, and the exact file, that wedged a sandboxed build on every sign-in."""
        done = run(
            {
                "clients/linux/src/ui/oauth_loopback.rs": (
                    "fn launch_browser(uri: &str) {\n"
                    "    let _ = gio::AppInfo::launch_default_for_uri(uri, None);\n"
                    "}\n"
                )
            }
        )

        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("launch_default_for_uri", done.stdout)
        self.assertIn("oauth_loopback.rs", done.stdout)
        # The message has to say what to do instead, or it is a rule without a remedy.
        self.assertIn("UriLauncher", done.stderr)
        self.assertIn("FileLauncher", done.stderr)

    def test_an_untracked_file_is_searched_too(self) -> None:
        """A newly written file is exactly where this mistake gets made."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "scripts" / "ci").mkdir(parents=True)
            shutil.copy(SCRIPT, root / "scripts" / "ci" / SCRIPT.name)
            source = root / "clients" / "linux" / "src" / "new.rs"
            source.parent.mkdir(parents=True)
            source.write_text("gtk::show_uri(None, \"x\", 0);\n", encoding="utf-8")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True, capture_output=True)
            done = subprocess.run(
                bash_argv(str(root / "scripts" / "ci" / SCRIPT.name)),
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("show_uri", done.stdout)

    def test_a_search_that_cannot_run_is_an_error_and_not_a_pass(self) -> None:
        """The failure the first version of this check had.

        Outside a repository `git grep` exits non-zero for a reason that is not "no matches". That
        must be loud: a check that reports OK because it could not look is worse than no check.
        """
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "scripts" / "ci").mkdir(parents=True)
            shutil.copy(SCRIPT, root / "scripts" / "ci" / SCRIPT.name)
            done = subprocess.run(
                bash_argv(str(root / "scripts" / "ci" / SCRIPT.name)),
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertNotEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertNotIn("OK:", done.stdout)


if __name__ == "__main__":
    unittest.main()
