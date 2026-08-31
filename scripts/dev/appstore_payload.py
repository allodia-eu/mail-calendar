#!/usr/bin/env python3
"""Turn the resolved store listing (`branding/<brand>-listing.md`) into App Store Connect metadata
files. Offline, and pure.

This is the half of the App Store push that never touches the network: it reads the contract,
measures every field against the caps the contract itself states, collects the screenshots, and
writes the **canonical metadata tree** the `asc` CLI applies
([`appstore_listing.py`](appstore_listing.py) is the half that runs `asc`). All the tests live here
([`tests/test_appstore_payload.py`](tests/test_appstore_payload.py)), because everything that can be
decided without an App Store Connect account is decided here.

**It reads the document through the checker's own scraper**, exactly as
[`msstore_payload.py`](msstore_payload.py) does and for the same reason: `store-copy` already parses
this file to measure it, and a second parser would be a second reading of one document; with the
one that reaches a store console being the one nobody tested. The screenshot layout and the gallery
order come from `msstore_payload` rather than being restated, since which capture leads a listing is
a product decision and not a per-store one.

**The canonical tree is `asc`'s, not ours.** `asc metadata push` reads
`app-info/<locale>.json` (the fields that belong to the *app*: name, subtitle, privacy-policy URL)
and `version/<X.Y.Z>/<locale>.json` (the fields that belong to a *version*: description, keywords,
promotional text, the URLs, What's new). Writing those files rather than calling the API ourselves
means the transport, its retries and its auth are the CLI's problem, and what this repo owns is the
mapping from the contract to the fields.

**Python 3.9-compatible**, like its siblings: `/usr/bin/python3` on a stock macOS is 3.9.
"""

from __future__ import annotations

import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    LOCALE_NAMES,
    DocumentShapeError,
    catalog_locales,
    labelled_blocks,
    load_releases,
)
from check_store_copy_length import (  # noqa: E402
    KEYSTORE_TOKEN,
    listing_path,
    one_section,
    parse_keystore_tokens,
)

# The capture naming convention, the gallery order and the PNG reader, imported rather than
# restated. `showcase-screenshots/<platform>/` uses one layout for every store, and "which
# screen leads the listing" is a decision about the product, not about Apple.
from msstore_payload import (  # noqa: E402
    ListingError,
    collect_screenshots,
    descriptions,
    png_dimensions,
    product_name,
)

STORE = "App Store Connect"

# App Store Connect's platform key -> the changelog `Platforms:` tag whose section becomes that
# version's What's new. The Apple app record spans macOS, iOS and iPadOS, but a *version* is per
# platform; and `docs/changelog/released/` already separates the two, so each submission is told
# about itself. Not `store_targets`, which would hand the Mac an iOS section too.
SECTION_FOR = {"MAC_OS": "macos", "IOS": "ios"}

# Catalog locale -> the App Store Connect locale code.
#
# **Italian is `it`, not `it-IT`**; App Store Connect has no `it-IT`, and the difference is not
# cosmetic: a push to a locale the store does not offer creates nothing and reports nothing, so
# Italy would quietly keep the previous release's copy. The rest are the regional codes Apple lists
# (`en-US` because that is the app's primary locale; Spanish and Portuguese are the **European**
# variants, per AGENTS.md).
APP_STORE_LOCALES = {
    "en": "en-US",
    "nl": "nl-NL",
    "de": "de-DE",
    "fr": "fr-FR",
    "es": "es-ES",
    "it": "it",
    "pt": "pt-PT",
}

# The three short fields the doc keeps one block per language, one `Field: value` per line.
SHORT_FIELDS = ("Subtitle", "Promotional", "Keywords")

MAX_SCREENSHOTS = 10


@dataclass(frozen=True)
class Slot:
    """One App Store gallery: its display type, the capture directory feeding it, and the sizes
    Apple accepts for it."""

    device_type: str
    capture: str
    sizes: tuple


# The galleries each App Store platform carries. `asc screenshots sizes --all` is the authority for
# both the display-type names and the pixel sizes, and Apple refuses an off-size PNG at upload,
# partway through a locale, leaving that gallery half replaced; rather than at review.
#
# A 6.9" iPhone capture is 1320x2868, which belongs to APP_IPHONE_67; APP_IPHONE_65 takes only
# 1242x2688 and 1284x2778, so pointing the iPhone gallery at it fails on every file.
SCREENSHOT_SLOTS = {
    "MAC_OS": (
        Slot("APP_DESKTOP", "macos", ((1280, 800), (1440, 900), (2560, 1600), (2880, 1800))),
    ),
    "IOS": (
        Slot(
            "APP_IPHONE_67",
            "iphone",
            ((1260, 2736), (1290, 2796), (1320, 2868), (2736, 1260), (2796, 1290), (2868, 1320)),
        ),
        Slot(
            "APP_IPAD_PRO_3GEN_129",
            "ipad",
            ((2048, 2732), (2064, 2752), (2732, 2048), (2752, 2064)),
        ),
    ),
}

ASC_PLATFORMS = tuple(SCREENSHOT_SLOTS)


@dataclass(frozen=True)
class Listing:
    """Everything this repo writes down about one language's App Store listing."""

    locale: str
    language: str  # the endonym, as the documents label it ("Nederlands")
    store_locale: str  # the App Store Connect code ("nl-NL")
    name: str
    subtitle: str
    promotional: str
    keywords: str
    description: str
    marketing_url: str
    support_url: str
    privacy_policy_url: str
    whats_new: str = None
    screenshots: tuple = ()

    def app_info(self) -> dict:
        """The `app-info/<locale>.json` body; the fields that belong to the app, not a version."""
        return {
            "name": self.name,
            "subtitle": self.subtitle,
            "privacyPolicyUrl": self.privacy_policy_url,
        }

    def version(self) -> dict:
        """The `version/<X.Y.Z>/<locale>.json` body.

        `whatsNew` is omitted rather than written empty when this run carries no release note: an
        empty string is a *value*, and pushing it would erase a note somebody else put there.
        """
        body = {
            "description": self.description,
            "keywords": self.keywords,
            "promotionalText": self.promotional,
            "marketingUrl": self.marketing_url,
            "supportUrl": self.support_url,
        }
        if self.whats_new is not None:
            body["whatsNew"] = self.whats_new
        return body


# -------------------------------------------------------------------------------------------
# Reading the contract
# -------------------------------------------------------------------------------------------


def short_fields(listing: str) -> dict:
    """`language -> {"Subtitle": ..., "Promotional": ..., "Keywords": ...}`.

    Read the way `store-copy` reads it; one labelled block per language, `Field: value` per line,
    so a field this refuses to find is one that job is not measuring either.
    """
    body = one_section(
        listing.splitlines(), 3, r"^App Store Connect — Subtitle", "App Store Connect short fields"
    )
    found = {}
    for language, block in labelled_blocks(body):
        fields = {}
        for line in block.splitlines():
            label, _, value = line.partition(":")
            if label.strip() in SHORT_FIELDS:
                fields[label.strip()] = value.strip()
        missing = [field for field in SHORT_FIELDS if field not in fields]
        if missing:
            raise DocumentShapeError(
                f"the App Store Connect block for {language} is missing: {', '.join(missing)}"
            )
        found[language] = fields
    if not found:
        raise DocumentShapeError("no App Store Connect field blocks found")
    return found


def shared_fields(listing: str) -> dict:
    """The "Console-side metadata" -> "Shared fields" table, as `lowercased label -> value`.

    The same table `msstore_payload.copyright_line` reads one row of; a test asserts the two agree
    on the copyright, so the overlap is a check rather than a second source of truth.
    """
    found = {}
    for line in one_section(listing.splitlines(), 3, r"^Shared fields", "shared console fields"):
        # A backticked first cell is a locale row of the localised-URL table below, not a field of
        # this one; `localized_privacy_urls` reads those.
        if not line.startswith("| ") or line.startswith("| `"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 2:
            found[cells[0].strip("*").lower()] = cells[1].strip("`").strip("*").strip()
    for required in ("support url", "marketing / website url", "privacy policy url"):
        if required not in found:
            raise DocumentShapeError(f"the 'Shared fields' table has no '{required}' row")
    return found


def localized_privacy_urls(listing: str) -> dict:
    """`catalog locale -> its own privacy-policy URL`, for the locales the website has a page for.

    Optional by construction: the table lists only the locales with a localized page, and a locale
    absent from it takes the shared URL. Reading it matters because App Store Connect stores this
    per locale; a push that wrote the shared URL everywhere would quietly send Dutch readers to the
    English policy, which is a *downgrade nobody would notice* rather than an error.
    """
    found = {}
    for line in one_section(
        listing.splitlines(), 3, r"^Shared fields", "shared console fields"
    ):
        if not line.startswith("| `"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 2 and cells[1].startswith("`http"):
            found[cells[0].strip("`")] = cells[1].strip("`")
    return found


def release_notes(version: str, platform: str = "MAC_OS") -> dict:
    """`language -> the note` for the sections of `version` that reached `platform`."""
    tag = SECTION_FOR[platform]
    for released, parsed in load_releases():
        if released != version:
            continue
        notes = {}
        for section in parsed:
            if tag not in section.platforms:
                continue
            for language, note in section.notes.items():
                notes[language] = f"{notes[language]}\n\n{note}" if language in notes else note
        if not notes:
            raise DocumentShapeError(
                f"docs/changelog/released/{version}.md has no section reaching {tag}: that "
                f"release shipped no {tag} build, so there is no What's new to push."
            )
        return notes
    raise DocumentShapeError(f"no docs/changelog/released/{version}.md")


# -------------------------------------------------------------------------------------------
# Screenshots
# -------------------------------------------------------------------------------------------


def measure_screenshots(listing: Listing, slot: Slot) -> list:
    """Every reason App Store Connect would refuse this language's gallery, in plain words.

    Apple takes a fixed set of sizes per slot rather than a range, so this is stricter than its
    Microsoft Store counterpart; and it has to be: an off-size PNG fails at upload, halfway
    through a locale, leaving a gallery half replaced.
    """
    problems = []
    if len(listing.screenshots) > MAX_SCREENSHOTS:
        problems.append(
            f"{listing.store_locale}: {len(listing.screenshots)} screenshots, App Store Connect "
            f"takes at most {MAX_SCREENSHOTS}"
        )
    for shot in listing.screenshots:
        if (shot.width, shot.height) not in slot.sizes:
            sizes = ", ".join(f"{width}x{height}" for width, height in slot.sizes)
            problems.append(
                f"{listing.store_locale}: {shot.path.name} is {shot.width}x{shot.height}; the "
                f"{slot.capture} slot ({slot.device_type}) takes only {sizes}"
            )
    return problems


# -------------------------------------------------------------------------------------------
# Building the listings
# -------------------------------------------------------------------------------------------


def resolve_locales(locales=None) -> tuple:
    """The catalog locales this run covers, refusing any App Store Connect has no code for."""
    wanted = tuple(locales) if locales else catalog_locales()
    unknown = [locale for locale in wanted if locale not in APP_STORE_LOCALES]
    if unknown:
        raise DocumentShapeError(
            f"no App Store Connect locale for {', '.join(unknown)}: add it to APP_STORE_LOCALES "
            "in appstore_payload.py, matching the codes the console offers (note Italian is 'it', "
            "not 'it-IT')."
        )
    return wanted


def load_listings(listing_md=None, locales=None, version=None, screenshots=None,
                  platform="MAC_OS") -> tuple:
    """Every language's listing, exactly as the resolved store listing states it.

    `version` adds that release's What's new; `screenshots` is `collect_screenshots`' per-locale
    result. Both are opt-in: a run that silently replaced a gallery with whatever happened to be on
    disk, or wrote a What's new nobody asked for, would be a worse tool than the console it saves.
    """
    text = (Path(listing_md) if listing_md else listing_path()).read_text(encoding="utf-8")
    wanted = resolve_locales(locales)

    name = product_name(text)
    # `STORE`, not the reader's default: the same body says "the Windows Credential Manager" on the
    # Microsoft Store and "your device's Keychain" here, and the substitution is what gets pushed.
    bodies = descriptions(text, STORE)
    shorts = short_fields(text)
    urls = shared_fields(text)
    localized = localized_privacy_urls(text)
    notes = release_notes(version, platform) if version else {}
    galleries = screenshots or {}

    built = []
    for locale in wanted:
        language = LOCALE_NAMES[locale]
        for what, source in (("description", bodies), ("short fields", shorts)):
            if language not in source:
                raise DocumentShapeError(
                    f"{listing_path().name} has no {what} for {language}: the catalog ships "
                    f"{locale}, so the listing must carry it (docs/store-listing.md: languages "
                    "are the localisation catalog)."
                )
        built.append(
            Listing(
                locale=locale,
                language=language,
                store_locale=APP_STORE_LOCALES[locale],
                name=name,
                subtitle=shorts[language]["Subtitle"],
                promotional=shorts[language]["Promotional"],
                keywords=shorts[language]["Keywords"],
                description=bodies[language],
                marketing_url=urls["marketing / website url"],
                support_url=urls["support url"],
                privacy_policy_url=localized.get(locale, urls["privacy policy url"]),
                whats_new=notes.get(language),
                screenshots=tuple(galleries.get(locale, ())),
            )
        )
    if notes:
        missing = [item.language for item in built if item.whats_new is None]
        if missing:
            raise DocumentShapeError(
                f"the release note has no text for {', '.join(missing)}: a note carries every "
                "catalog locale (docs/changelog.md)."
            )
    return tuple(built)


def measure(listing: Listing, limits) -> list:
    """Every field App Store Connect would refuse, measured against the contract's own table.

    The `store-copy` CI job measures the same copy against the *tightest* store across all three;
    this measures it against the one console it is about to be handed to. Same table, read the same
    way, so the two cannot disagree; and this one also catches the case that job cannot, which is
    the table changing between a branch turning green and the push.
    """
    caps = limits[STORE]
    problems = []
    checks = (
        ("name", caps.name, len(listing.name)),
        ("subtitle", caps.subtitle, len(listing.subtitle)),
        ("promotional text", caps.short, len(listing.promotional)),
        ("keywords", caps.keywords, len(listing.keywords)),
        ("description", caps.description, len(listing.description)),
    )
    if listing.whats_new is not None:
        checks += (("What's new", caps.whats_new, len(listing.whats_new)),)
    for what, cap, actual in checks:
        if cap is not None and actual > cap:
            problems.append(f"{listing.store_locale}: {what} is {actual} characters, limit {cap}")
    # No check for a leftover `{KEYSTORE}` here: `descriptions` raises when a language has no value
    # for this store, so one could never reach this point; and a check that cannot fail is not a
    # check (AGENTS.md). The refusal is tested where it happens, in `load_listings`.
    return problems


# -------------------------------------------------------------------------------------------
# Writing what `asc` reads
# -------------------------------------------------------------------------------------------


def write_metadata(listings, directory, version: str) -> list:
    """Write the canonical tree `asc metadata push --dir` reads. Returns the paths written.

    Only the locales in `listings` get a file. `asc` pushes what it finds, so a narrowed run
    (`--language nl`) touches one locale and leaves the other six exactly as the console has them.
    """
    directory = Path(directory)
    written = []
    for listing in listings:
        for relative, body in (
            (Path("app-info") / f"{listing.store_locale}.json", listing.app_info()),
            (Path("version") / version / f"{listing.store_locale}.json", listing.version()),
        ):
            path = directory / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(body, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            written.append(path)
    return written


def stage_screenshots(listings, directory) -> dict:
    """Lay the captures out the way `asc screenshots upload` fans out: `<locale>/<file>`.

    Two things happen here that the flat capture directory cannot express. The locale becomes a
    *directory*, which is what lets one run cover seven languages; and the gallery order becomes a
    numeric **file-name prefix**, because Apple orders a gallery by upload order and `asc` uploads
    a locale's files sorted by name; so `add-account.png` would otherwise lead the listing.

    Returns `locale -> the staged directory`. The captures are **copied**, not linked: `asc` refuses
    to read a symlink (measured 2026-08-03; `refusing to read symlink …`), which is a sensible rule
    for a tool that uploads whatever it is pointed at.
    """
    directory = Path(directory)
    staged = {}
    for listing in listings:
        if not listing.screenshots:
            continue
        target = directory / listing.store_locale
        target.mkdir(parents=True, exist_ok=True)
        for number, shot in enumerate(listing.screenshots, start=1):
            shutil.copyfile(shot.path, target / f"{number:02d}-{shot.path.name}")
        staged[listing.store_locale] = target
    return staged


__all__ = [
    "APP_STORE_LOCALES",
    "ASC_PLATFORMS",
    "SECTION_FOR",
    "SCREENSHOT_SLOTS",
    "Slot",
    "Listing",
    "ListingError",
    "collect_screenshots",
    "load_listings",
    "measure",
    "measure_screenshots",
    "png_dimensions",
    "release_notes",
    "resolve_locales",
    "shared_fields",
    "short_fields",
    "stage_screenshots",
    "write_metadata",
]
