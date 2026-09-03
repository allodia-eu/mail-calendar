#!/usr/bin/env python3
"""What every store publisher reads out of the resolved store listing, read once.

Three consoles are pushed from one document, and some of what they ask for is the same question:
what is the app called, what does its description say, which capture leads the gallery. This module
answers those, so a publisher differs from its siblings only where the *store* differs.

**It reads the document through the checker's own scraper.**
`scripts/ci/check_store_copy_length.py` already parses this file to measure it; a second parser
would be a second reading of one document, and the one that pastes into a store console would be
the one nobody tested. So `sections`, `fenced_blocks`, `one_section` and `parse_keystore_tokens`
are imported from there rather than restated here.

**What is shared and what is not.** Shared: the title, the description bodies, the copyright line,
the capture naming convention and the gallery order. Not shared, and deliberately left to each
publisher: the field limits, the locale codes, the merge into a submission and every refusal a
console makes on its own terms. A limit that looks common today is one console's, and folding two
of them together here is how a cap gets applied to the store that never had it.

The Microsoft Store half of this used to live alongside it, in `msstore_payload.py`, because that
publisher was written first. It went with the rest of the Microsoft Store push, which is not built
here; this module is what stayed, and it stayed because the App Store and Play publishers are still
here.
"""

from __future__ import annotations

import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))

import brand  # noqa: E402  (path set above so this runs as a script)

from changelog_fragments import DocumentShapeError  # noqa: E402
from check_store_copy_length import (  # noqa: E402
    KEYSTORE_TOKEN,
    fenced_blocks,
    one_section,
    parse_keystore_tokens,
    sections,
)


class ListingError(RuntimeError):
    """An input this run cannot use; a missing directory, a file that is not a PNG.

    Deliberately not a `DocumentShapeError`: that one means "the tool has fallen behind the
    document, go fix the scraper", which is the wrong advice entirely for a mistyped path.
    """


# The gallery order, hero first. Not alphabetical on purpose: alphabetical opens the listing on
# `add-account`, which store-listing.md's own "Known gaps" calls out as mostly empty space, and the
# first screenshot is the one a shopper sees without scrolling. Screens not named here follow, in
# alphabetical order, so an unknown capture is never silently dropped.
#
# `list-dark` is the mailbox list again in the dark appearance, and it sits third rather than second
# on purpose: the first two slots go to the two things the product *is*; mail, then calendar; and
# the dark shot then answers the question a shopper has already formed looking at them. Second would
# spend the most valuable slot in the gallery on a screen they just saw.
#
# It is shared rather than per store because which capture leads a listing is a decision about the
# product, not about Apple or Microsoft or Google.
SCREEN_ORDER = (
    "list",
    "calendar",
    "list-dark",
    "invitation",
    "reply",
    "signatures",
    "settings",
    "add-account",
)

# `<locale>-<screen>.png`, the layout `showcase-screenshots/<platform>/` already uses.
SCREENSHOT_NAME = re.compile(r"^(?P<locale>[a-z]{2})-(?P<screen>[a-z0-9][a-z0-9-]*)\.png$")

# Every store takes PNG, and each states its own pixel bounds; those live with the publisher that
# enforces them, because a bound applied to the wrong console refuses a capture that console wanted.
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


@dataclass(frozen=True)
class Screenshot:
    """One capture bound for one language's gallery."""

    path: Path
    screen: str
    width: int
    height: int

    @property
    def zip_name(self) -> str:
        """Its path inside the upload archive; the file name, which is already locale-unique."""
        return self.path.name

    @property
    def order(self):
        try:
            return (0, SCREEN_ORDER.index(self.screen), "")
        except ValueError:
            return (1, 0, self.screen)


def png_dimensions(path: Path):
    """`(width, height)` from the PNG header; no image library, and none needed for IHDR."""
    with Path(path).open("rb") as handle:
        header = handle.read(24)
    if len(header) < 24 or header[:8] != PNG_MAGIC or header[12:16] != b"IHDR":
        raise ListingError(f"{path} is not a PNG (every store takes PNG screenshots)")
    return struct.unpack(">II", header[16:24])


def collect_screenshots(directory, locales):
    """Read `<locale>-<screen>.png` out of `directory`, per locale, in gallery order.

    Returns `(by_locale, skipped)`. Anything that does not match the naming convention is reported
    rather than ignored: a typo'd file name would otherwise mean a language silently keeps its old
    gallery, which reads exactly like a successful run.
    """
    directory = Path(directory)
    if not directory.is_dir():
        raise ListingError(f"no such screenshot directory: {directory}")
    by_locale = {locale: [] for locale in locales}
    skipped = []
    for path in sorted(directory.iterdir()):
        if not path.is_file():
            continue
        matched = SCREENSHOT_NAME.match(path.name)
        if not matched or matched.group("locale") not in by_locale:
            skipped.append(path.name)
            continue
        width, height = png_dimensions(path)
        by_locale[matched.group("locale")].append(
            Screenshot(path=path, screen=matched.group("screen"), width=width, height=height)
        )
    for locale in by_locale:
        by_locale[locale] = tuple(sorted(by_locale[locale], key=lambda shot: shot.order))
    return by_locale, skipped


def _language_of(title: str) -> str:
    """"Shared description; Nederlands" -> "Nederlands"."""
    return title.split("—")[-1].strip()


def product_name(_listing: str = "") -> str:
    """The one title every store and every language uses.

    It is the injected `MAILCAL_APP_NAME` (docs/branding.md), not a line of copy: the launcher, the
    installer and every store listing must agree about what the app is called, and a name written
    in a second file is a name that can disagree with the one the OS shows. The unused argument is
    kept so every publisher still reads as "everything about a listing comes from one call".
    """
    return brand.value("MAILCAL_APP_NAME")


def descriptions(listing: str, store: str) -> dict:
    """`language -> the description body as `store` would receive it`.

    The `{KEYSTORE}` substitution is applied when the copy still uses it: the token was retired in
    favour of "stored securely on your device", but the substitution is what a console is handed,
    so this pastes what would be pasted rather than what is written.

    `store` is required rather than defaulted because the substitution is the one thing about a
    description that is **not** shared: the same body says "the Windows Credential Manager" on the
    Microsoft Store and "your device's Keychain" on the App Store. A default here would be one
    store's word pasted into another's listing, and nothing downstream would question it.
    """
    tokens = parse_keystore_tokens(listing)
    found = {}
    for title, body in sections(listing.splitlines(), 2, re.compile(r"^Shared description")):
        language = _language_of(title)
        blocks = fenced_blocks(body)
        if len(blocks) != 1:
            raise DocumentShapeError(f"'{title}' has {len(blocks)} fenced blocks, expected exactly 1")
        body_text = blocks[0]
        if KEYSTORE_TOKEN in body_text:
            token = tokens.get(store, {}).get(language)
            if token is None:
                raise DocumentShapeError(
                    f"the {language} body uses {KEYSTORE_TOKEN} but the '{KEYSTORE_TOKEN} token' "
                    f"table has no {language} value for {store}"
                )
            body_text = body_text.replace(KEYSTORE_TOKEN, token)
        found[language] = body_text
    if not found:
        raise DocumentShapeError("no 'Shared description' sections found")
    return found


def copyright_line(listing: str) -> str:
    """The copyright, from "Console-side metadata" -> "Shared fields"."""
    for line in one_section(listing.splitlines(), 3, r"^Shared fields", "shared console fields"):
        if not line.startswith("| "):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 2 and cells[0].lower() == "copyright":
            return cells[1].strip("`").strip()
    raise DocumentShapeError("the 'Shared fields' table has no Copyright row")
