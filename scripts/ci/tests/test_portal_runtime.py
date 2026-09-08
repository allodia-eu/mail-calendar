"""Tests for `check-portal-runtime.sh`.

The bug it guards is invisible to every run-time gate, because the **first** portal call of the
process succeeds: a manual check passes, a screenshot proves nothing, and only the second call
hangs, with no error and no timeout. So this grep is the only thing standing between the tree and
a client that stops noticing mail partway through a session, and a grep that stopped matching
would be silent about it.

`OnceLock` is why the rule cannot be a unit test: two calls to `shared()` hand back the same
runtime whatever any other module does, so the assertion would hold over the broken tree too.
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

SCRIPT = Path(__file__).resolve().parents[1] / "check-portal-runtime.sh"

OWNER = """\
pub(crate) fn shared() -> Option<&'static Runtime> {
    static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Builder::new_multi_thread().build().ok())
        .as_ref()
}
"""

SHARED_CALLER = """\
pub(super) fn post(outcome: BackgroundSyncOutcome) {
    let Some(runtime) = host_runtime::shared() else { return; };
    let _ = runtime.block_on(post_all(outcome.accounts));
}
"""

# The exact shape that shipped, and hung: a runtime per call, dropped on return.
OWN_RUNTIME = """\
pub(super) fn post(outcome: BackgroundSyncOutcome) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
        return;
    };
    let _ = runtime.block_on(post_all(outcome.accounts));
}
"""


def run(sources: dict[str, str], *, commit: bool = True) -> subprocess.CompletedProcess[str]:
    """Write `sources` into a throwaway git repo and run the check over it."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "scripts" / "ci").mkdir(parents=True)
        shutil.copy(SCRIPT, root / "scripts" / "ci" / SCRIPT.name)
        for name, text in sources.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        subprocess.run(["git", "init", "-q"], cwd=root, check=True, capture_output=True)
        if commit:
            subprocess.run(["git", "add", "-A"], cwd=root, check=True, capture_output=True)
        return subprocess.run(
            bash_argv(str(root / "scripts" / "ci" / SCRIPT.name)),
            capture_output=True,
            text=True,
            check=False,
        )


@unittest.skipIf(bash_problem() != "", bash_problem())
class PortalRuntime(unittest.TestCase):
    def test_the_owner_may_build_the_one_runtime(self) -> None:
        done = run(
            {
                "clients/linux/src/host_runtime.rs": OWNER,
                "clients/linux/src/ui/notifications.rs": SHARED_CALLER,
            }
        )

        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)

    def test_a_second_runtime_is_refused(self) -> None:
        """The exact shape, and the exact file, whose second notification hung for good."""
        done = run(
            {
                "clients/linux/src/host_runtime.rs": OWNER,
                "clients/linux/src/ui/notifications.rs": OWN_RUNTIME,
            }
        )

        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("notifications.rs", done.stdout)
        # The message has to say what to do instead, or it is a rule without a remedy.
        self.assertIn("host_runtime::shared()", done.stderr)

    def test_test_code_may_build_its_own(self) -> None:
        """The secure store's nesting guard needs two distinct runtimes to prove anything."""
        done = run(
            {
                "clients/linux/src/host_runtime.rs": OWNER,
                "clients/linux/src/secrets.rs": (
                    "fn open() {}\n"
                    "\n"
                    "#[cfg(test)]\n"
                    "mod tests {\n"
                    "    use tokio::runtime::Builder;\n"
                    "    fn core() { Builder::new_multi_thread().build().unwrap(); }\n"
                    "}\n"
                ),
                "clients/linux/src/ui/mailbox_tests.rs": (
                    "fn helper() { tokio::runtime::Builder::new_current_thread(); }\n"
                ),
            }
        )

        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)

    def test_a_runtime_above_the_test_module_is_still_refused(self) -> None:
        """A file having tests must not excuse the production half of the same file."""
        done = run(
            {
                "clients/linux/src/host_runtime.rs": OWNER,
                "clients/linux/src/secrets.rs": (
                    "fn open() { tokio::runtime::Builder::new_multi_thread(); }\n"
                    "\n"
                    "#[cfg(test)]\n"
                    "mod tests {}\n"
                ),
            }
        )

        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("secrets.rs", done.stdout)

    def test_an_untracked_file_is_searched_too(self) -> None:
        """A newly written file is exactly where this mistake gets made."""
        done = run(
            {
                "clients/linux/src/host_runtime.rs": OWNER,
                "clients/linux/src/ui/new.rs": OWN_RUNTIME,
            },
            commit=False,
        )

        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("new.rs", done.stdout)

    def test_a_search_that_cannot_run_is_an_error_and_not_a_pass(self) -> None:
        """A check that reports OK because it could not look is worse than no check."""
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

        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)


if __name__ == "__main__":
    unittest.main()
