"""Tests the MSIX identity rewrite in `scripts/dev/msix_manifest.py`.

The manifest is committed unbranded and rewritten for a Store build (docs/branding.md), which puts
the app's whole Store identity; package name, publisher GUID, listing name; behind one text
transformation that runs on Windows, inside a packaging script, minutes before an upload. Nothing
downstream checks it: a manifest that kept a default builds, signs and packages, and is rejected at
ingestion days later having burned a submission.

So the rewrite is exercised here against the **real committed manifest**, on any host.
"""

from __future__ import annotations

import sys
import unittest
import xml.dom.minidom
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

import brand  # noqa: E402
from msix_manifest import ManifestError, rebrand  # noqa: E402

MANIFEST = REPO_ROOT / "clients" / "windows" / "Mailcal" / "Package.appxmanifest"

NEUTRAL = {
    "MAILCAL_APP_ID": "org.mailcal.client",
    "MAILCAL_APP_NAME": "MailCal",
    "MAILCAL_MSIX_IDENTITY_NAME": "org.mailcal.client",
    "MAILCAL_MSIX_PUBLISHER": "CN=MailCal",
    "MAILCAL_MSIX_PUBLISHER_DISPLAY_NAME": "MailCal",
}
BRANDED = {
    "MAILCAL_APP_ID": "eu.example.mail",
    "MAILCAL_APP_NAME": "Example Mail & Calendar",
    "MAILCAL_MSIX_IDENTITY_NAME": "ExampleEU.ExampleMailCalendar",
    "MAILCAL_MSIX_PUBLISHER": "CN=00000000-1111-2222-3333-444444444444",
    "MAILCAL_MSIX_PUBLISHER_DISPLAY_NAME": "Example EU",
}


class MsixIdentityRewrite(unittest.TestCase):
    def setUp(self) -> None:
        self.committed = MANIFEST.read_text(encoding="utf-8")

    def rebranded(self) -> str:
        return rebrand(self.committed, NEUTRAL, BRANDED)

    def test_the_committed_manifest_carries_the_unbranded_identity(self) -> None:
        """The default has to be a real, buildable identity; the public repo ships this file."""
        self.assertIn('Name="%s"' % NEUTRAL["MAILCAL_MSIX_IDENTITY_NAME"], self.committed)
        self.assertIn('Publisher="%s"' % NEUTRAL["MAILCAL_MSIX_PUBLISHER"], self.committed)
        self.assertIn("<DisplayName>%s</DisplayName>" % NEUTRAL["MAILCAL_APP_NAME"], self.committed)

    def test_the_committed_protocol_is_the_neutral_application_id(self) -> None:
        """The other half of the check `OAuthSchemeTests` cannot make.

        C# pins that the code's scheme is `Brand.AppId`; this build's id. Nothing there can see
        that the committed manifest declares the *neutral* id, which is what makes the rewrite the
        thing that joins them. Break this and a packaged build registers one scheme while the app
        listens for another, and every browser sign-in dies on delivery.
        """
        self.assertIn(
            '<uap:Protocol Name="%s">' % NEUTRAL["MAILCAL_APP_ID"], self.committed
        )

    def test_the_store_identity_replaces_every_reserved_field(self) -> None:
        """These three are what Partner Center matches on; a default in any one is a rejection."""
        rewritten = self.rebranded()
        self.assertIn('Name="ExampleEU.ExampleMailCalendar"', rewritten)
        self.assertIn('Publisher="CN=00000000-1111-2222-3333-444444444444"', rewritten)
        self.assertIn("<PublisherDisplayName>Example EU</PublisherDisplayName>", rewritten)

    def test_the_name_a_person_reads_is_escaped_for_xml(self) -> None:
        """A product name with an ampersand is the ordinary case, not an edge one."""
        rewritten = self.rebranded()
        self.assertIn("<DisplayName>Example Mail &amp; Calendar</DisplayName>", rewritten)
        self.assertIn('DisplayName="Example Mail &amp; Calendar"', rewritten)
        self.assertNotIn("Example Mail & Calendar", rewritten)
        xml.dom.minidom.parseString(rewritten)  # raises if the escaping broke the document

    def test_our_scheme_moves_with_the_app_id_and_mailto_does_not(self) -> None:
        """`mailto` is the OS's scheme, not ours; rebranding it would unregister the app as a
        mail client on every branded build."""
        rewritten = self.rebranded()
        self.assertIn('<uap:Protocol Name="eu.example.mail">', rewritten)
        self.assertIn('<uap:Protocol Name="mailto">', rewritten)
        self.assertNotIn(NEUTRAL["MAILCAL_APP_ID"], rewritten)

    def test_both_protocol_labels_name_the_branded_product(self) -> None:
        rewritten = self.rebranded()
        self.assertIn("<uap:DisplayName>Example Mail &amp; Calendar sign-in</uap:DisplayName>", rewritten)
        self.assertIn("<uap:DisplayName>Example Mail &amp; Calendar</uap:DisplayName>", rewritten)

    def test_the_load_bearing_comments_survive(self) -> None:
        """A DOM round trip would drop them, and they are why the alias and the dev-only scheme
        exist at all."""
        rewritten = self.rebranded()
        self.assertIn("%LOCALAPPDATA%", rewritten)
        self.assertIn("RegisterProtocolForUnpackaged", rewritten)

    def test_a_manifest_that_lost_a_field_is_an_error_not_a_silent_default(self) -> None:
        without_identity = self.committed.replace("<Identity", "<Removed", 1)

        with self.assertRaises(ManifestError) as raised:
            rebrand(without_identity, NEUTRAL, BRANDED)

        self.assertIn("Identity/@Name", str(raised.exception))

    def test_rebranding_to_the_default_leaves_the_committed_file_alone(self) -> None:
        """The unbranded case is a no-op, so a from-source packaged build needs no special path."""
        self.assertEqual(rebrand(self.committed, NEUTRAL, NEUTRAL), self.committed)

    def test_the_neutral_values_are_the_ones_the_brand_file_declares(self) -> None:
        """The rewrite keys on the committed defaults, so branding/default.env and the manifest
        cannot be edited apart."""
        defaults = brand.defaults()
        for key, expected in NEUTRAL.items():
            self.assertEqual(defaults[key], expected, key)


if __name__ == "__main__":
    unittest.main()
