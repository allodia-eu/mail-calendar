"""Tests for `check-license-dir.sh`.

The promise it holds is that the open tree builds without the one directory that is not GPL. Both
of its rules fail silently if they stop matching; a workspace that no longer excludes the
directory, and a grep that no longer finds an import; so each is exercised against a tree that
should trip it and one that should not.
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

SCRIPT = Path(__file__).resolve().parents[1] / "check-license-dir.sh"

WORKSPACE = """\
[workspace]
members = ["crates/*", "allodia_license/crates/allodia-license"]
default-members = [
  "crates/mailcal-app",
]
"""


def run(files: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Write `files` into a throwaway repo and run the check over it."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "scripts" / "ci").mkdir(parents=True)
        shutil.copy(SCRIPT, root / "scripts" / "ci" / SCRIPT.name)
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        git = ["git", "-C", str(root), "-c", "user.email=t@example.com", "-c", "user.name=T"]
        for argv in (["init", "-q"], ["add", "-A"], ["commit", "-qm", "fixture"]):
            subprocess.run([*git, *argv], check=True)
        return subprocess.run(
            bash_argv(str(root / "scripts" / "ci" / SCRIPT.name)),
            capture_output=True,
            text=True,
        )


class LicenseDirTests(unittest.TestCase):
    def setUp(self) -> None:
        problem = bash_problem()
        if problem:
            self.skipTest(problem)

    def test_a_clean_tree_passes(self) -> None:
        result = run({"Cargo.toml": WORKSPACE, "crates/a/src/lib.rs": "pub fn a() {}\n"})
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_workspace_with_no_default_members_fails(self) -> None:
        """Without that list every member is in the default build, so the crate would ship."""
        result = run({"Cargo.toml": '[workspace]\nmembers = ["crates/*"]\n'})
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("no default-members", result.stderr)

    def test_the_crate_in_default_members_fails(self) -> None:
        """Membership is fine and deliberate; it shares the lockfile and the engine. Being in the
        default build is not: a bare `cargo build` would ship the one crate that is not GPL."""
        result = run(
            {
                "Cargo.toml": (
                    "[workspace]\n"
                    'members = ["crates/*", "allodia_license/crates/allodia-license"]\n'
                    "default-members = [\n"
                    '  "crates/mailcal-app",\n'
                    '  "allodia_license/crates/allodia-license",\n'
                    "]\n"
                )
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("in default-members", result.stderr)

    def test_a_source_file_reaching_into_it_fails(self) -> None:
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "crates/a/src/lib.rs": '#[path = "../../../allodia_license/login.rs"]\nmod login;\n',
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("references allodia_license/", result.stderr)

    def test_a_client_build_file_reaching_into_it_fails(self) -> None:
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "clients/android/app/build.gradle.kts": 'sourceSets["main"].java.srcDir("../../allodia_license/android")\n',
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("references allodia_license/", result.stderr)

    def test_a_drifted_licence_mirror_fails(self) -> None:
        """The text lives twice; `LICENSES/` for `reuse lint`, the directory for a person; and
        `reuse lint` checks that a licence text exists, never that the two say the same thing."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "allodia_license/LICENSE.md": "Version 1.0\n",
                "LICENSES/LicenseRef-Allodia-1.0.txt": "Version 1.1\n",
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("drifted", result.stderr)

    def test_matching_licence_copies_pass(self) -> None:
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "allodia_license/LICENSE.md": "Version 1.0\n",
                "LICENSES/LicenseRef-Allodia-1.0.txt": "Version 1.0\n",
            }
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_the_optional_dependency_and_its_feature_are_allowed(self) -> None:
        """The seam itself. The line has to exist for an Allodia build to turn the directory on,
        and `optional = true` is what keeps it out of everyone else's."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "crates/mailcal-app/Cargo.toml": (
                    "[features]\n"
                    'allodia-license = ["dep:allodia-license"]\n\n'
                    "[dependencies]\n"
                    "# Off by default.\n"
                    'allodia-license = { path = "../../allodia_license/crates/allodia-license", '
                    "optional = true }\n"
                ),
            }
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_non_optional_dependency_on_it_is_still_caught(self) -> None:
        """The allowance is the exact line, not the word: drop `optional = true` and the open tree
        stops standing alone, which is the whole thing the check exists for."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "crates/mailcal-app/Cargo.toml": (
                    "[dependencies]\n"
                    'allodia-license = { path = "../../allodia_license/crates/allodia-license" }\n'
                ),
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("references allodia_license/", result.stderr)

    def test_a_default_on_feature_is_caught(self) -> None:
        """`optional = true` is only half of it. A feature in `default = [...]` is on for everyone
        while the dependency line still reads as optional to anyone skimming the manifest."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "crates/mailcal-app/Cargo.toml": (
                    "[features]\n"
                    'default = ["allodia-license"]\n'
                    'allodia-license = ["dep:allodia-license"]\n'
                ),
            }
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("on by default", result.stderr)

    def test_using_the_crate_is_not_reaching_into_the_directory(self) -> None:
        """`allodia_license` is the directory *and* the crate's Rust identifier. Whether the crate
        is linked is the manifest's business, so a `use` is not what this rule is looking for."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "crates/mailcal-app/src/lib.rs": (
                    '#[cfg(feature = "allodia-license")]\n'
                    "pub mod allodia_account {\n"
                    "    pub use allodia_license::AccountService;\n"
                    "}\n"
                ),
            }
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_prose_may_name_the_directory(self) -> None:
        """The README explains the seam, the pledge promises it, and this script is about it. A
        rule that forbade the word would be a rule nobody could document the seam under."""
        result = run(
            {
                "Cargo.toml": WORKSPACE,
                "README.md": "Everything but `allodia_license/` is GPL-3.0-only.\n",
                "clients/linux/README.md": "Nothing here reads allodia_license/.\n",
            }
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
