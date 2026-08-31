"""Tests the credential hand-off in `clients/linux/package.sh`.

The Flatpak is the one shipping artifact whose build cannot see the environment it was started
from: `flatpak-builder` runs cargo inside `flatpak build`, which forwards no host variables. So the
OAuth client registrations reach it only by riding along in the source tree flatpak-builder copies,
and the requirement that they be there travels separately, in the manifest (`BUILDING.md`). Get
either wrong and a tagged build hands users a bundle with Google and Microsoft sign-in silently
missing; every other gate stays green, because a credential-free binary is supposed to behave
exactly like that.

Nothing else can catch it. The block runs before `flatpak-builder`, only on Linux, and only with a
~2 GB runtime installed, so it is invisible to the workspace suite and to every developer on a Mac.
Here the real script runs with a stub `flatpak-builder` on PATH, which exercises the block itself
rather than a copy of it; on any host, in milliseconds.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

# An absolute path to Git Bash on Windows: a bare "bash" resolves through CreateProcess, which
# searches System32 -- WSL's launcher -- before PATH. See bashtools.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bashtools import bash_argv, bash_path, bash_problem  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
PACKAGE_SH = REPO_ROOT / "clients" / "linux" / "package.sh"
BRAND_SH = REPO_ROOT / "scripts" / "dev" / "brand.sh"
LIB_SH = REPO_ROOT / "scripts" / "dev" / "lib.sh"

# The unbranded identity the committed manifest carries, and what a brand turns it into.
NEUTRAL_APP_ID = "org.mailcal.client"
BRANDED_APP_ID = "eu.example.mail"


def credential_vars() -> frozenset[str]:
    """The names `package.sh` treats as credentials, read from the script itself.

    Not a prefix: two of them (`ALLODIA_TELEMETRY_*`) do not begin with `MAILCAL_`, and a name this
    misses is one the host silently adds to a case that states its credentials in full.
    """
    block = re.search(
        r"^CREDENTIAL_VARS=\(\n(.*?)^\)$", PACKAGE_SH.read_text(encoding="utf-8"), re.M | re.S
    )
    if block is None:
        raise RuntimeError("package.sh no longer declares CREDENTIAL_VARS=( ... )")
    names = re.findall(r"^\s*([A-Z][A-Z0-9_]*)\s*$", block.group(1), re.M)
    if not names:
        raise RuntimeError("package.sh's CREDENTIAL_VARS holds no names")
    return frozenset(names)


CREDENTIAL_VARS = credential_vars()

# The names the block carries across, and one it must not invent.
DESKTOP_ID = "MAILCAL_GOOGLE_DESKTOP_CLIENT_ID"
MS_ID = "MAILCAL_MS_CLIENT_ID"
IOS_ID = "MAILCAL_GOOGLE_IOS_CLIENT_ID"
ALLODIA_ID = "MAILCAL_ALLODIA_CLIENT_ID"
REQUIRE = "MAILCAL_REQUIRE_INJECTED_CONFIG"

# The feature the Allodia registration turns on, which has to reach the cargo line itself: cargo
# reads features from no environment variable, so the manifest is the only way in.
ALLODIA_FEATURE = "allodia-license"

# Only the shape the script edits: the app id it rewrites, and the build-options env block it
# inserts a sibling into.
MANIFEST = """\
app-id: org.mailcal.client
modules:
  - name: mailcal
    build-options:
      env:
        CARGO_HOME: /run/build/mailcal/cargo
    build-commands:
      - cargo build --release --locked -p mailcal-linux
      - cargo build --release --locked -p mailcal-mcp-shim --bin allodia-mcp
"""


@unittest.skipIf(bash_problem() != "", bash_problem())
class FlatpakCredentialHandoff(unittest.TestCase):
    """Runs the real `package.sh` against a scratch checkout and a stub flatpak-builder."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.addCleanup(self._tmp.cleanup)

        # The layout package.sh needs: it resolves ROOT as clients/linux/../.. .
        (self.root / "clients" / "linux" / "flatpak").mkdir(parents=True)
        self.script = self.root / "clients" / "linux" / "package.sh"
        self.script.write_text(PACKAGE_SH.read_text(encoding="utf-8"), encoding="utf-8")
        self.script.chmod(0o755)
        self.manifest = self.root / "clients" / "linux" / "flatpak" / f"{NEUTRAL_APP_ID}.yml"
        self.manifest.write_text(MANIFEST, encoding="utf-8")

        # package.sh resolves the app id through the real brand reader, so the scratch checkout
        # carries the real one and an unbranded `branding/` (docs/branding.md). Tests that want a
        # branded build write `allodia.env` themselves.
        (self.root / "scripts" / "dev").mkdir(parents=True)
        brand_sh = self.root / "scripts" / "dev" / "brand.sh"
        brand_sh.write_text(BRAND_SH.read_text(encoding="utf-8"), encoding="utf-8")
        # The cargo feature is derived by `core_cargo_features`, the one resolver every build front
        # door shares (BUILDING.md), so the real lib.sh is carried across rather than stubbed.
        lib_sh = self.root / "scripts" / "dev" / "lib.sh"
        lib_sh.write_text(LIB_SH.read_text(encoding="utf-8"), encoding="utf-8")
        self.branding = self.root / "branding"
        self.branding.mkdir()
        (self.branding / "default.env").write_text(
            'MAILCAL_APP_NAME="MailCal"\nMAILCAL_APP_ID="%s"\n' % NEUTRAL_APP_ID,
            encoding="utf-8",
        )

        # The stub records the `.env` and the manifest as they stand *during* the build. The script
        # removes both on the way out; itself the behaviour under test; so neither can be
        # inspected afterwards.
        self.stub_dir = self.root / "stub-bin"
        self.stub_dir.mkdir()
        self.captured_env_file = self.root / "captured-env"
        self.captured_manifest = self.root / "captured-manifest"
        stub = self.stub_dir / "flatpak-builder"
        stub.write_text(
            "#!/usr/bin/env bash\n"
            'if [[ -f "%s/.env" ]]; then cp "%s/.env" "%s"; fi\n'
            'cp "${@: -1}" "%s"\n'  # the manifest is the last argument
            "exit 0\n"
            % (self.root, self.root, self.captured_env_file, self.captured_manifest),
            encoding="utf-8",
        )
        stub.chmod(0o755)

        # `--install` shells out to `flatpak`. The stub records every invocation so the test can
        # read which repository the remote was pointed at.
        self.flatpak_calls = self.root / "flatpak-calls"
        flatpak = self.stub_dir / "flatpak"
        flatpak.write_text(
            "#!/usr/bin/env bash\n"
            + 'printf "%s\\n" "$*" >> "' + str(self.flatpak_calls) + '"\n'
            + "exit 0\n",
            encoding="utf-8",
        )
        flatpak.chmod(0o755)

    def brand_it(self) -> None:
        """Give the scratch checkout a brand, the way a private checkout has one."""
        (self.branding / "allodia.env").write_text(
            'MAILCAL_APP_NAME="Example Mail"\nMAILCAL_APP_ID="%s"\n' % BRANDED_APP_ID,
            encoding="utf-8",
        )

    def package(self, _args: list[str] | None = None, **credentials: str) -> subprocess.CompletedProcess:
        """Runs package.sh with exactly `credentials` in its environment."""
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith("MAILCAL_") and k not in CREDENTIAL_VARS
        }
        env["PATH"] = os.pathsep.join([str(self.stub_dir), env.get("PATH", "")])
        env.update(credentials)
        return subprocess.run(
            bash_argv(str(self.script), *(_args or [])),
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )

    def run_package(self, _args: list[str] | None = None, **credentials: str) -> str:
        done = self.package(_args, **credentials)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return done.stdout + done.stderr

    def sandbox_env(self) -> str:
        self.assertTrue(self.captured_env_file.is_file(), "the build saw no .env at all")
        return self.captured_env_file.read_text(encoding="utf-8")

    def sandbox_manifest(self) -> str:
        self.assertTrue(self.captured_manifest.is_file(), "the build got no manifest")
        return self.captured_manifest.read_text(encoding="utf-8")

    # --- the credentials ---------------------------------------------------

    def test_credentials_in_the_environment_reach_the_build(self) -> None:
        """The CI case: variables are set, no `.env` exists, and the sandbox must still get them."""
        self.run_package(**{DESKTOP_ID: "desktop-id", MS_ID: "ms-id"})
        written = self.sandbox_env()
        self.assertIn("%s=desktop-id" % DESKTOP_ID, written)
        self.assertIn("%s=ms-id" % MS_ID, written)
        # A variable that was not set must not be invented as an empty one: blank counts as absent
        # everywhere else, and a name with no value only makes the file harder to read.
        self.assertNotIn(IOS_ID, written)

    def test_the_temporary_file_does_not_outlive_the_build(self) -> None:
        """Credentials written for the sandbox are removed again, however the build exits."""
        self.run_package(**{MS_ID: "ms-id"})
        self.assertFalse(
            (self.root / ".env").exists(), "package.sh left the credentials it wrote on disk"
        )

    def test_an_existing_env_file_is_used_and_left_alone(self) -> None:
        """A developer's own file is what gets copied, and the script must not touch it."""
        env_file = self.root / ".env"
        env_file.write_text("%s=from-file\n" % MS_ID, encoding="utf-8")
        self.run_package()
        self.assertIn("%s=from-file" % MS_ID, self.sandbox_env())
        self.assertEqual(env_file.read_text(encoding="utf-8"), "%s=from-file\n" % MS_ID)

    def test_an_override_that_cannot_win_is_reported(self) -> None:
        """The sandbox reads only the file, so an exported override is silently lost; say so."""
        (self.root / ".env").write_text("%s=from-file\n" % MS_ID, encoding="utf-8")
        output = self.run_package(**{MS_ID: "from-env"})
        self.assertIn("%s=from-file" % MS_ID, self.sandbox_env())
        self.assertIn("IGNORED", output)

    def test_a_build_with_no_credentials_says_so(self) -> None:
        """Not an error; but never silent, or a release engineer cannot tell from the log."""
        output = self.run_package()
        self.assertIn("will not offer Google or Microsoft sign-in", output)
        self.assertFalse((self.root / ".env").exists())

    def test_a_name_exported_on_the_host_does_not_join_the_build(self) -> None:
        """Every case here states its credentials in full. CI exports several from repository
        secrets, so one that survived into the environment would quietly make this a build *with*
        credentials; and the case that proves a credential-free bundle announces itself would
        instead be proving nothing."""
        for var in sorted(CREDENTIAL_VARS):
            with self.subTest(var=var), mock.patch.dict(os.environ, {var: "from-the-host"}):
                self.assertIn(
                    "will not offer Google or Microsoft sign-in", self.run_package()
                )

    # --- the requirement, which must travel by a different road ------------

    def test_the_requirement_reaches_the_sandbox_through_the_manifest(self) -> None:
        """Not through `.env`: a file that failed to cross would take the requirement with it, and
        the build inside would see nothing to enforce; the guard unable to fire in exactly the
        case it exists for."""
        self.run_package(**{MS_ID: "ms-id", REQUIRE: "1"})
        # A sibling of CARGO_HOME, at its indentation, or the block is not valid YAML.
        self.assertIn(
            "        CARGO_HOME: /run/build/mailcal/cargo\n"
            '        MAILCAL_REQUIRE_INJECTED_CONFIG: "1"\n',
            self.sandbox_manifest(),
        )

    def test_the_committed_manifest_is_never_edited_in_place(self) -> None:
        self.run_package(**{MS_ID: "ms-id", REQUIRE: "1"})
        self.assertEqual(self.manifest.read_text(encoding="utf-8"), MANIFEST)
        # And the generated one does not outlive the build, or the next `git status` shows it.
        self.assertEqual(
            sorted(p.name for p in self.manifest.parent.iterdir()),
            ["%s.yml" % NEUTRAL_APP_ID],
        )

    def test_a_build_without_the_requirement_uses_the_committed_manifest(self) -> None:
        """Nothing is generated when nothing asked for it; a from-source build is untouched."""
        self.run_package(**{MS_ID: "ms-id"})
        self.assertNotIn(REQUIRE, self.sandbox_manifest())

    # --- the identity, which decides what the desktop shell can find --------

    def test_an_unbranded_build_uses_the_committed_manifest_untouched(self) -> None:
        """The default case, and the state of the public repository: nothing to generate."""
        self.run_package()

        self.assertIn("app-id: %s" % NEUTRAL_APP_ID, self.sandbox_manifest())
        self.assertEqual(
            sorted(p.name for p in self.manifest.parent.iterdir()),
            ["%s.yml" % NEUTRAL_APP_ID],
        )

    def test_a_branded_build_gets_a_manifest_carrying_its_own_id(self) -> None:
        """Every basename the shell matches on; desktop entry, icon, metainfo; is the app id, so
        a branded bundle built from the neutral manifest installs a launcher the window it opens
        cannot be tied to."""
        self.brand_it()

        output = self.run_package()

        self.assertIn("app-id: %s" % BRANDED_APP_ID, self.sandbox_manifest())
        self.assertNotIn(NEUTRAL_APP_ID, self.sandbox_manifest())
        self.assertIn(BRANDED_APP_ID, output)

    def test_the_generated_manifest_is_a_sibling_and_does_not_outlive_the_build(self) -> None:
        """A sibling because `sources: path: ../../..` resolves against the manifest's own
        directory; removed because the next `git status` would otherwise show it."""
        self.brand_it()

        self.run_package()

        self.assertEqual(
            sorted(p.name for p in self.manifest.parent.iterdir()),
            ["%s.yml" % NEUTRAL_APP_ID],
        )
        self.assertEqual(self.manifest.read_text(encoding="utf-8"), MANIFEST)

    def test_a_manifest_without_an_app_id_is_a_hard_error(self) -> None:
        self.brand_it()
        self.manifest.write_text("modules: []\n", encoding="utf-8")

        done = self.package()

        self.assertNotEqual(done.returncode, 0, "a manifest with no app id was accepted")
        self.assertIn("could not put the app id", done.stdout + done.stderr)

    def test_a_manifest_it_cannot_edit_is_a_hard_error(self) -> None:
        """Losing the anchor must fail loudly, not leave the requirement quietly unset."""
        self.manifest.write_text("id: eu.allodia.mailcal\n", encoding="utf-8")
        done = self.package(**{MS_ID: "ms-id", REQUIRE: "1"})
        self.assertNotEqual(done.returncode, 0, "a manifest it could not edit was accepted")
        self.assertIn("could not add the credential requirement", done.stdout + done.stderr)

    def test_the_allodia_registration_puts_its_feature_on_the_cargo_line(self) -> None:
        """A build given the registration must also be given the code that uses it.

        Cargo takes features from the command line and nowhere else; no environment variable, and
        nothing `build-options.env` can carry; so unlike every other credential, which rides in
        the copied tree, this one has to be written into the manifest the build runs. Without it
        the bundle compiles and installs and simply has no Allodia sign-in, which is a supported
        build and therefore invisible to every other gate.
        """
        output = self.run_package(**{ALLODIA_ID: "allodia-client-id"})

        built = self.captured_manifest.read_text(encoding="utf-8")
        self.assertIn(
            f"- cargo build --release --locked -p mailcal-linux --features {ALLODIA_FEATURE}",
            built,
        )
        # Only the client's own line: the MCP shim is a separate package with no such feature.
        self.assertIn("-p mailcal-mcp-shim --bin allodia-mcp\n", built)
        self.assertNotIn(f"mailcal-mcp-shim --bin allodia-mcp --features", built)
        self.assertIn(ALLODIA_FEATURE, output)
        # The registration still has to reach the sandbox, like every other one.
        self.assertIn("allodia-client-id", self.captured_env_file.read_text(encoding="utf-8"))

    def test_a_build_without_the_registration_asks_for_no_feature(self) -> None:
        """The ordinary build from source: no registration, no feature, no sign-in surface."""
        self.run_package(**{MS_ID: "ms-id"})

        built = self.captured_manifest.read_text(encoding="utf-8")
        self.assertIn("- cargo build --release --locked -p mailcal-linux\n", built)
        self.assertNotIn("--features", built)

    def test_installing_points_the_remote_at_this_checkout(self) -> None:
        """`--if-not-exists` alone installs another checkout's bundle and calls it success.

        A worktree builds into its own `target/`, so the second checkout on a machine finds
        `mailcal-local` already there, pointing at the first one's repository; and every later
        install silently serves that build. It cost a real "prod" install of a stale bundle, which
        looked exactly like a fresh one.
        """
        self.run_package(**{"MAILCAL_INSTALL": "1"}, _args=["--install"])

        calls = self.flatpak_calls.read_text(encoding="utf-8").splitlines()
        # A real URL, not a bare path: `remote-modify --url` stores what it is handed, and a bare
        # path leaves the remote unreadable; "Unable to load summary from remote".
        # `bash_path`, not `str(self.root)`: package.sh resolves ROOT with `cd … && pwd`, so on
        # Windows the URL it writes names `/tmp/…` while Python holds `C:\Users\…`.
        url = f"file://{bash_path(self.root)}/target/flatpak/repo"
        self.assertTrue(
            any(line.startswith("remote-modify") and f"--url={url}" in line for line in calls),
            f"the remote must be re-pointed at this checkout on every install: {calls}",
        )
        # And it still installs from that remote rather than whatever else is configured.
        self.assertTrue(
            any("install" in line and "mailcal-local" in line for line in calls),
            f"the install must come from the local remote: {calls}",
        )


if __name__ == "__main__":
    unittest.main()
