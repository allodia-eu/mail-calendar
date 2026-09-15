#!/usr/bin/env python3
"""Put this build's identity into the MSIX manifest, and take it back out again.

`Package.appxmanifest` is committed carrying the **unbranded** identity (docs/branding.md), because
that is what a build of this repository is by default. A shipped build needs something else
entirely, and *which* something depends on where it is going (docs/windows-channels.md):

- `--channel store`, the default: the package name and the publisher GUID are a reservation held in
  Partner Center, and an upload whose manifest does not match it exactly is rejected on ingestion.
- `--channel direct`: the package we host ourselves, whose Publisher must be the subject of the
  certificate that signs it, because that is what Windows checks before it will install.

So `package.ps1` rewrites the manifest, builds, and restores the committed bytes; the same shape it
already uses to stamp the package version. The rewrite lives here rather than in PowerShell for one
reason: it can then be tested, on any host, by `scripts/dev/tests/test_msix_manifest.py`. Nothing
about an MSIX identity fails loudly; it fails at the Store, days later, having burned a submission,
or at a user's machine with "the publisher does not match", which nobody reports.

    python3 scripts/dev/msix_manifest.py --manifest clients/windows/Mailcal/Package.appxmanifest
    python3 scripts/dev/msix_manifest.py --manifest … --channel direct

The rewrite is textual and targeted rather than an XML round trip: the manifest carries load-bearing
comments (why the alias exists, why the dev loop registers a different scheme) that a DOM rewrite
would drop. Every substitution asserts it matched exactly once, so a manifest that has moved out
from under this fails here instead of silently keeping a default.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Dict

sys.path.insert(0, str(Path(__file__).resolve().parent))

import brand  # noqa: E402  (after the path insert, deliberately)


class ManifestError(RuntimeError):
    """The manifest did not have the shape this rewrite needs."""


# Which brand keys name the package identity, per channel. Only these two differ: the publisher
# *display* name is the account's own name and is the same wherever the package goes, and so is
# everything a person reads.
IDENTITY_KEYS = {
    "store": ("MAILCAL_MSIX_IDENTITY_NAME", "MAILCAL_MSIX_PUBLISHER"),
    "direct": ("MAILCAL_MSIX_DIRECT_IDENTITY_NAME", "MAILCAL_MSIX_DIRECT_PUBLISHER"),
}


def _one(pattern: str, replacement: str, text: str, what: str) -> str:
    rewritten, count = re.subn(pattern, replacement, text, flags=re.DOTALL)
    if count != 1:
        raise ManifestError(
            "expected exactly one %s in Package.appxmanifest, found %d: the manifest has moved "
            "and this rewrite would leave the default behind" % (what, count)
        )
    return rewritten


def rebrand(
    text: str, defaults: Dict[str, str], wanted: Dict[str, str], channel: str = "store"
) -> str:
    """The manifest as this build wants it, given what the committed one says now."""
    neutral_id = defaults["MAILCAL_APP_ID"]
    neutral_name = defaults["MAILCAL_APP_NAME"]
    try:
        name_key, publisher_key = IDENTITY_KEYS[channel]
    except KeyError:
        raise ManifestError(
            "unknown channel %r: the package goes to the Store or to our own downloads, and each "
            "has an identity of its own (docs/windows-channels.md)" % channel
        ) from None
    for key in (name_key, publisher_key, "MAILCAL_MSIX_PUBLISHER_DISPLAY_NAME"):
        value = wanted.get(key, "").strip()
        if not value:
            raise ManifestError(
                "the brand names no %s, so a %s package cannot be given an identity" % (key, channel)
            )
        # An X.500 distinguished name is what Windows compares, character for character, against
        # the signing certificate's subject; one carrying a quotation mark would close the
        # attribute it is written into and produce a manifest that is not the identity anyone meant.
        if '"' in value:
            raise ManifestError("%s carries a quotation mark, which no identity may" % key)

    # Every MSIX publisher is a distinguished name, so anything that is not one is a placeholder
    # somebody has yet to fill in. Refused here rather than at a store or at a signing service,
    # because both of those answers arrive after the build and name neither the file nor the key.
    publisher = wanted[publisher_key].strip()
    if not publisher.startswith("CN="):
        raise ManifestError(
            "%s is %r, which is not a distinguished name: an MSIX publisher begins CN=" \
            % (publisher_key, publisher)
        )

    # The three a store or a machine checks the package against. They do not follow the application
    # id: the Store issues its own package name and a publisher GUID, and a package we host is
    # matched against the certificate that signed it.
    text = _one(
        r'(<Identity\b[^>]*?\bName=")[^"]*(")',
        lambda m: m.group(1) + wanted[name_key] + m.group(2),
        text,
        "Identity/@Name",
    )
    text = _one(
        r'(<Identity\b[^>]*?\bPublisher=")[^"]*(")',
        lambda m: m.group(1) + _xml(wanted[publisher_key]) + m.group(2),
        text,
        "Identity/@Publisher",
    )
    text = _one(
        r"(<PublisherDisplayName>)[^<]*(</PublisherDisplayName>)",
        lambda m: m.group(1) + _xml(wanted["MAILCAL_MSIX_PUBLISHER_DISPLAY_NAME"]) + m.group(2),
        text,
        "PublisherDisplayName",
    )

    # The name a person reads: the Store listing, the Start menu tile, the Default-apps entry.
    text = _one(
        r"(<DisplayName>)[^<]*(</DisplayName>)",
        lambda m: m.group(1) + _xml(wanted["MAILCAL_APP_NAME"]) + m.group(2),
        text,
        "Properties/DisplayName",
    )
    text = _one(
        r'(<uap:VisualElements\b[^>]*?\bDisplayName=")[^"]*(")',
        lambda m: m.group(1) + _xml(wanted["MAILCAL_APP_NAME"]) + m.group(2),
        text,
        "VisualElements/@DisplayName",
    )

    # The custom URI scheme sign-ins come back to. Keyed on the neutral id so the `mailto`
    # declaration beside it; which is a real scheme, not ours; is left alone.
    text = _one(
        r'(<uap:Protocol Name=")%s(")' % re.escape(neutral_id),
        lambda m: m.group(1) + wanted["MAILCAL_APP_ID"] + m.group(2),
        text,
        "our own <uap:Protocol Name>",
    )

    # Both protocol labels name the product ("… sign-in", and the mailto entry). They are copy, so
    # the name is replaced inside them rather than over them.
    def relabel(match: "re.Match[str]") -> str:
        return (
            match.group(1)
            + match.group(2).replace(_xml(neutral_name), _xml(wanted["MAILCAL_APP_NAME"]))
            + match.group(3)
        )

    text, labels = re.subn(r"(<uap:DisplayName>)([^<]*)(</uap:DisplayName>)", relabel, text)
    if labels == 0:
        raise ManifestError("no <uap:DisplayName> in Package.appxmanifest: a protocol lost its label")
    return text


def _xml(value: str) -> str:
    """`&` is the only character in a product name that an attribute or element cannot carry raw."""
    return value.replace("&", "&amp;")


def main(argv) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--out", type=Path, help="where to write (default: over --manifest)")
    parser.add_argument(
        "--channel",
        choices=sorted(IDENTITY_KEYS),
        default="store",
        help="which package identity to write (default: store)",
    )
    args = parser.parse_args(argv)

    text = args.manifest.read_text(encoding="utf-8")
    try:
        rewritten = rebrand(text, brand.defaults(), brand.resolve(), args.channel)
    except ManifestError as problem:
        print("msix_manifest: %s" % problem, file=sys.stderr)
        return 1
    (args.out or args.manifest).write_text(rewritten, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
