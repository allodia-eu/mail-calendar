#!/usr/bin/env python3
"""Tests for the PKGBUILD renderer, and that the committed template renders to valid bash."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import aur_pkgbuild  # noqa: E402
from bashtools import bash_argv, bash_problem  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[3]
TEMPLATE = REPO_ROOT / "clients" / "linux" / "aur" / "PKGBUILD.in"

# What package-aur.sh hands over for an unbranded build with no brand files.
FIELDS = {
    "PUBLISHER_NAME": "MailCal",
    "REPO_URL": "https://github.com/allodia-eu/mail-calendar",
    "PKGNAME": "mailcal",
    "VERSION": "1.2.3",
    "PKGDESC": "A user's mail and calendar",
    "URL": "https://example.org/a?b=1&c=$HOME",
    "LICENSE": "GPL-3.0-only",
    "SOURCES": '"$pkgname-$pkgver.tar.gz"',
    "SHA256SUMS": "'0000'",
    "SOURCE_DIR": '"mail-calendar-$pkgver"',
    "BRAND_FILES": "",
    "APP_ID": "org.mailcal.client",
    "MCP_RELAY_NAME": "mailcal-mcp",
}


class Render(unittest.TestCase):
    def test_the_template_comment_block_is_left_out(self) -> None:
        rendered = aur_pkgbuild.render("# about the template\n\n# Maintainer: @N@\n", {"N": "x"})
        self.assertEqual(rendered, "# Maintainer: x\n")

    def test_a_quoted_placeholder_takes_any_text_safely(self) -> None:
        rendered = aur_pkgbuild.render("head\n\nv=@'V'@\n", {"V": "it's $HOME `x`"})
        self.assertEqual(rendered, "v='it'\"'\"'s $HOME `x`'\n")

    def test_a_placeholder_with_no_value_is_refused_by_name(self) -> None:
        with self.assertRaisesRegex(aur_pkgbuild.RenderError, "LATER, NOPE"):
            aur_pkgbuild.render("head\n\n@NOPE@ @'LATER'@\n", {})

    def test_a_half_quoted_placeholder_is_refused(self) -> None:
        with self.assertRaisesRegex(aur_pkgbuild.RenderError, "one side"):
            aur_pkgbuild.render("head\n\n@'V@\n", {"V": "x"})


@unittest.skipIf(bash_problem() != "", bash_problem())
class CommittedTemplate(unittest.TestCase):
    def setUp(self) -> None:
        self.pkgbuild = aur_pkgbuild.render(TEMPLATE.read_text(encoding="utf-8"), FIELDS)

    def variables(self, *names: str) -> list[str]:
        script = self.pkgbuild + "".join(f'\nprintf "%s\\n" "${{{name}[*]}}"' for name in names)
        result = subprocess.run(
            bash_argv("-c", script), capture_output=True, text=True, check=True
        )
        return result.stdout.splitlines()

    def test_it_opens_with_the_maintainer_line(self) -> None:
        self.assertTrue(self.pkgbuild.startswith("# Maintainer: MailCal\n"))

    def test_free_text_reaches_bash_unchanged(self) -> None:
        self.assertEqual(
            self.variables("pkgdesc", "url", "license"),
            [FIELDS["PKGDESC"], FIELDS["URL"], FIELDS["LICENSE"]],
        )

    def test_the_installed_command_is_the_package_name(self) -> None:
        self.assertIn('"$pkgdir/usr/bin/mailcal"', self.pkgbuild)
        self.assertIn('"$pkgdir/usr/bin/mailcal-mcp"', self.pkgbuild)


if __name__ == "__main__":
    unittest.main()
