#!/usr/bin/env python3
"""Unit tests for `signing_team` in scripts/dev/lib.sh: which Apple team a device build signs with.

It decides for device.sh, `test-ui.sh --device` and `build-and-run.sh --iphone`, and the last of
those writes its answer into clients/apple/signing.local.xcconfig, where Xcode's own device builds
read it. A keychain with Apple Development certificates from more than one team is ordinary (a
personal team beside a company's), and the team that owns this app's identifier is then a question
the keychain's order cannot answer.

The function is extracted by text and run against stubbed `security` and `openssl`, as
test_harness_store_reset.py does with its own; `test_the_function_can_still_be_extracted` is what
stops an extraction that yields nothing from leaving every test here passing over an empty shell.
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
EXTRACT = re.compile(r"^signing_team\(\) \{$.*?^\}$", re.MULTILINE | re.DOTALL)

sys.path.insert(0, str(DEV))
from bashtools import bash_argv, bash_problem  # noqa: E402

NO_BASH = bash_problem()

# Two teams, in the order a keychain listed them on the machine that hit this: the first
# certificate's team does not own the app.
PERSONAL = ("Apple Development: info@example.com (AAAAAAAAAA)", "PERSONAL01")
COMPANY = ("Apple Development: Alice Tester (BBBBBBBBBB)", "COMPANY001")
COMPANY_TOO = ("Apple Development: Alice Tester (CCCCCCCCCC)", "COMPANY001")


def extracted() -> str:
    match = EXTRACT.search(LIB.read_text(encoding="utf-8"))
    assert match, "signing_team could not be extracted from lib.sh"
    return match.group(0)


def run(
    certificates: list[tuple[str, str]],
    *,
    xcconfig: str | None = None,
    environment_team: str | None = None,
) -> tuple[int, str, str]:
    """`signing_team`'s exit status, stdout and stderr, with these certificates in the keychain and
    this text (if any) in the checkout's signing.local.xcconfig."""
    identities = "\n".join(
        f'  {i}) {"0" * 40} "{name}"' for i, (name, _) in enumerate(certificates, 1)
    )
    teams = " ".join(f'["{name}"]="{team}"' for name, team in certificates)
    script = "\n".join(
        [
            "set -euo pipefail",
            'warn() { printf "warning: %s\\n" "$*" >&2; }',
            f"declare -A CERT_TEAM=({teams})",
            # `security find-identity` lists the identities; `find-certificate -c <name> -p` stands
            # in for the PEM by printing the name, which the `openssl` stub turns into a subject.
            "security() {",
            '  case "$1" in',
            '    find-identity) cat "$IDENTITIES" ;;',
            '    find-certificate) printf "%s\\n" "$3" ;;',
            "  esac",
            "}",
            'openssl() { local name; read -r name; printf "subject=CN=%s, OU=%s, O=Example\\n"'
            ' "$name" "${CERT_TEAM[$name]}"; }',
            extracted(),
            "signing_team",
        ]
    )
    with tempfile.TemporaryDirectory() as root:
        apple = Path(root) / "clients" / "apple"
        apple.mkdir(parents=True)
        (Path(root) / "identities").write_text(identities + "\n", encoding="utf-8")
        if xcconfig is not None:
            (apple / "signing.local.xcconfig").write_text(xcconfig, encoding="utf-8")
        env = {**os.environ, "REPO_ROOT": root, "IDENTITIES": str(Path(root) / "identities")}
        env.pop("DEVELOPMENT_TEAM", None)
        if environment_team is not None:
            env["DEVELOPMENT_TEAM"] = environment_team
        completed = subprocess.run(
            bash_argv("-c", script), capture_output=True, text=True, env=env, check=False
        )
    return completed.returncode, completed.stdout.strip(), completed.stderr


class Extraction(unittest.TestCase):
    def test_the_function_can_still_be_extracted(self) -> None:
        self.assertIn("signing_team", extracted())


@unittest.skipIf(NO_BASH, NO_BASH)
class SigningTeam(unittest.TestCase):
    def test_the_environment_decides_first(self) -> None:
        status, team, _ = run(
            [PERSONAL, COMPANY],
            xcconfig="MAILCAL_DEVELOPMENT_TEAM = COMPANY001\n",
            environment_team="OVERRIDE01",
        )
        self.assertEqual((status, team), (0, "OVERRIDE01"))

    def test_the_team_xcode_already_signs_with_beats_the_keychain_order(self) -> None:
        status, team, _ = run(
            [PERSONAL, COMPANY],
            xcconfig="// Written by Scripts/build-and-run.sh\nMAILCAL_DEVELOPMENT_TEAM = COMPANY001\n",
        )
        self.assertEqual((status, team), (0, "COMPANY001"))

    def test_an_empty_setting_falls_through_to_the_keychain(self) -> None:
        status, team, _ = run([COMPANY], xcconfig="MAILCAL_DEVELOPMENT_TEAM =\n")
        self.assertEqual((status, team), (0, "COMPANY001"))

    def test_certificates_of_one_team_name_it(self) -> None:
        status, team, _ = run([COMPANY, COMPANY_TOO])
        self.assertEqual((status, team), (0, "COMPANY001"))

    def test_certificates_of_several_teams_are_not_a_choice(self) -> None:
        status, team, stderr = run([PERSONAL, COMPANY])
        self.assertEqual((status, team), (1, ""))
        self.assertIn("PERSONAL01", stderr)
        self.assertIn("COMPANY001", stderr)
        self.assertIn("DEVELOPMENT_TEAM", stderr)

    def test_no_certificate_is_no_team(self) -> None:
        status, team, _ = run([])
        self.assertEqual((status, team), (1, ""))


if __name__ == "__main__":
    unittest.main()
