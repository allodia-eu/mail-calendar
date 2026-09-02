#!/usr/bin/env python3
"""Turn the documents that own the store copy into Google Play payloads. Offline, and pure.

This is the half of the Play publisher that never touches a network:
the resolved store listing and `docs/changelog/released/` in, one `edits.listings.update` body per
language out. [`publish_play.py`](publish_play.py) is the half that talks to Play.

They are split because the boundary is real and the testing story follows it. Everything here is a
document read, so it is exercised against fixtures with no SDK installed and no credentials; which
matters, because the failure mode of a bad mapping is *silent*: Play leaves an unwritten locale
showing the previous release's copy and reports success either way.

It reads the **same parsers** `scripts/ci/check_store_copy_length.py` does. `changelog_fragments`
states the reason in its own docstring; "one implementation, so a fragment and a listing field can
never be read by two subtly different parsers"; and it binds harder here: this module decides what
is *uploaded*, so a second reading of the same document could ship a field the CI gate believed it
had cleared.

**Python 3.9-compatible**, like its siblings: `/usr/bin/python3` on a stock macOS is 3.9.
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

import brand  # noqa: E402  (path set above so this runs as a script)

from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    LOCALE_NAMES,
    RELEASED_DIR,
    SETTINGS_PATH,
    DocumentShapeError,
    labelled_blocks,
    parse_release,
    store_targets,
    version_tuple,
)

# The helpers are imported rather than reimplemented; see the note above about two parsers.
# `parse_limits` / `whats_new_caps` are re-exported for the CLI, which measures with them.
from check_store_copy_length import (  # noqa: E402
    KEYSTORE_TOKEN,
    LIMITS_PATH,
    listing_path,
    Measurement,
    fenced_blocks,
    one_section,
    parse_limits,
    whats_new_caps,
)

# The title and the description bodies are read by the SAME functions every other publisher uses;
# `descriptions(listing, store)` even applies the right `{KEYSTORE}` word per store. Writing a
# second reader here would be the exact failure `changelog_fragments` warns about: two parsers for
# one document, agreeing until the day the document is reshaped.
from store_payload import (  # noqa: E402
    SCREEN_ORDER,
    ListingError,
    descriptions,
    png_dimensions,
    product_name,
)

VERSION_PATH = REPO_ROOT / "VERSION"
GRADLE_PATH = REPO_ROOT / "clients" / "android" / "app" / "build.gradle.kts"

# The store, exactly as `store-listing.md`'s "Field limits" table names it.
STORE = "Google Play"

# Catalog locale -> the BCP-47 tag Play files a listing under. Play has no bare-`en`/`es`/`pt`
# listing, so this mapping is required rather than cosmetic, and it is **not** derivable: Spanish
# and Portuguese are the European variants by product decision (AGENTS.md; "pt-PT / es-ES"), and
# English is `en-GB` because the shared body is written in British English ("sanitised",
# "organisations"). A catalog locale missing here is a hard error, never a silent skip.
PLAY_LOCALES = {
    "en": "en-GB",
    "nl": "nl-NL",
    "de": "de-DE",
    "fr": "fr-FR",
    "es": "es-ES",
    "it": "it-IT",
    "pt": "pt-PT",
}

# Gradle takes the id from the brand, so what is checked is that it still does; a literal
# creeping back would make this script publish to one id while the build produced another.
_APPLICATION_ID = re.compile(r'applicationId\s*=\s*brandValue\("MAILCAL_APP_ID"\)')


class PublishError(Exception):
    """Something about the repo or the Play state stops the upload; with a named fix.

    Separate from `DocumentShapeError`, which the checker already defines for "a document no longer
    looks the way the tools read it". The advice differs: a reshaped heading means fix the scraper,
    an unmapped locale means fix the mapping, and a missing build means upload it.
    """


def catalog_locales():
    """The locale codes the app ships, in the catalog's own order.

    Read from `project.inlang/settings.json` for the same reason `catalog_languages()` is: the
    catalog is the single source of the language list, so adding `messages/sv.json` makes this
    demand a Swedish Play listing rather than quietly publishing six of seven.
    """
    settings = json.loads(SETTINGS_PATH.read_text(encoding="utf-8"))
    locales = settings.get("locales")
    if not locales:
        raise DocumentShapeError(f"{SETTINGS_PATH} lists no locales")
    return tuple(locales)


def play_locales():
    """`(code, display name, Play tag)` per catalog locale, in catalog order."""
    out = []
    for code in catalog_locales():
        if code not in LOCALE_NAMES:
            raise DocumentShapeError(
                f"no display name for locale '{code}': add it to LOCALE_NAMES in "
                "scripts/ci/changelog_fragments.py."
            )
        if code not in PLAY_LOCALES:
            raise PublishError(
                f"locale '{code}' has no Google Play tag: add it to PLAY_LOCALES in "
                f"{Path(__file__).name}. Play files listings under BCP-47 tags (e.g. 'sv-SE'), and "
                "a locale left out here would publish six languages while the catalog ships seven."
            )
        out.append((code, LOCALE_NAMES[code], PLAY_LOCALES[code]))
    return tuple(out)


def package_name(gradle_path=None):
    """The Android `applicationId`, read from the brand rather than mirrored here.

    A hard-coded package name is a mirror that can drift, and its failure mode is publishing this
    app's copy over a different app's listing; not a mistake a review step catches. Gradle takes
    its `applicationId` from exactly this (docs/branding.md), so the two cannot disagree.

    `gradle_path` is still accepted, and still read, for the one thing the brand cannot tell us:
    that Gradle has not gone back to a literal.
    """
    path = GRADLE_PATH if gradle_path is None else Path(gradle_path)
    if not _APPLICATION_ID.search(path.read_text(encoding="utf-8")):
        raise PublishError(
            f"no `applicationId` in {path}: this script publishes to the id the build uses, and "
            "cannot confirm the build still takes it from the brand."
        )
    name = brand.value("MAILCAL_APP_ID")
    if not name:
        raise PublishError(
            "no MAILCAL_APP_ID: branding/default.env names it and is not optional (docs/branding.md)."
        )
    return name


def current_version():
    """`/VERSION`; the last released version, which is the one a store is being told about."""
    return VERSION_PATH.read_text(encoding="utf-8").strip()


def android_version_code(version):
    """`docs/versioning.md`'s formula: `major*10^7 + minor*10^5 + patch*10^3`."""
    major, minor, patch = version_tuple(version)
    return major * 10**7 + minor * 10**5 + patch * 10**3


def short_descriptions(listing):
    """`language -> Play's 80-character short description`."""
    body = one_section(
        listing.splitlines(),
        3,
        r"^Google Play — Short description",
        "Google Play short description",
    )
    out = dict(labelled_blocks(body))
    if not out:
        raise DocumentShapeError("no Google Play short descriptions found")
    return out


def listing_payloads(listing):
    """`Play tag -> the body of an `edits.listings.update` call`, in catalog order.

    Every language the catalog ships must be present in all three fields; a missing one raises
    rather than publishing a partial set, because Play leaves an unwritten locale showing whatever
    was there before and the run would still report success.
    """
    title = product_name(listing)
    # Already `{KEYSTORE}`-substituted with Play's word; the substitution is what gets uploaded,
    # so measuring or reviewing anything else would be measuring a string nobody ever sees.
    bodies = descriptions(listing, STORE)
    shorts = short_descriptions(listing)

    payloads = {}
    for _code, language, tag in play_locales():
        for what, source in (("description body", bodies), ("short description", shorts)):
            if language not in source:
                raise DocumentShapeError(
                    f"{listing_path().name} has no {what} for {language}, which the catalog ships."
                )
        payloads[tag] = {
            "title": title,
            "shortDescription": shorts[language].strip(),
            "fullDescription": bodies[language],
        }
    return payloads


def android_release_notes(version, released_dir=None):
    """`Play tag -> the note` for `version`, or `{}` when the release did not reach Android.

    A release with no Android section is normal and correct; a Windows-only fix is deliberately
    *not* advertised on Play (`store-listing.md` rule 4, the anti-hype rule); so this returns
    nothing rather than raising, and the caller says so out loud instead of uploading another
    platform's note.
    """
    directory = RELEASED_DIR if released_dir is None else Path(released_dir)
    path = directory / f"{version}.md"
    if not path.is_file():
        raise PublishError(
            f"no release note at {path}: there is nothing to tell Play about {version}. A release "
            "is cut with scripts/dev/release.py, which writes that file and moves /VERSION; pass "
            "--version to name a different released version, or --skip-notes to publish only the "
            "listing."
        )
    chosen = [
        section
        for section in parse_release(path.read_text(encoding="utf-8"), version)
        if STORE in store_targets(section.platforms)
    ]
    if not chosen:
        return {}
    if len(chosen) > 1:
        raise DocumentShapeError(
            f"{path} has {len(chosen)} sections reaching {STORE}: a platform belongs to exactly "
            "one section, so this file was not written by scripts/dev/release.py."
        )
    notes = chosen[0].notes
    out = {}
    for _code, language, tag in play_locales():
        if language not in notes:
            raise DocumentShapeError(
                f"{path}: the {STORE} section has no {language} note, which the catalog ships."
            )
        out[tag] = notes[language].strip()
    return out


# ---------------------------------------------------------------------------------------------
# Screenshots and the feature graphic
# ---------------------------------------------------------------------------------------------

# Play keeps a SEPARATE gallery per form factor, and the difference is not cosmetic: an empty
# tablet slot is what makes Play file the app as phone-only on large screens, whatever the app can
# actually do. So the three slots are one submission rather than a phone set with extras.
#
# The prefixes are the ones `scripts/dev/showcase.sh` already writes into a single flat
# `showcase-screenshots/android/` (docs/store-screenshots.md explains why one directory).
PHONE, SEVEN_INCH, TEN_INCH = "phoneScreenshots", "sevenInchScreenshots", "tenInchScreenshots"
FEATURE_GRAPHIC = "featureGraphic"
SLOT_NAMES = {
    PHONE: "phone",
    SEVEN_INCH: "7-inch tablet",
    TEN_INCH: "10-inch tablet",
    FEATURE_GRAPHIC: "feature graphic",
}

SCREENSHOT_NAME = re.compile(
    r"^(?P<form>phone|tablet-7|tablet-10)-(?P<locale>[a-z]{2})-(?P<screen>[a-z0-9][a-z0-9-]*)\.png$"
)
FEATURE_GRAPHIC_NAME = re.compile(r"^feature-graphic-(?P<locale>[a-z]{2})\.png$")
FORM_FACTOR_SLOTS = {"phone": PHONE, "tablet-7": SEVEN_INCH, "tablet-10": TEN_INCH}

# What Play accepts, asserted here rather than learned from a rejected submission (AGENTS.md; "a
# store upload is a remote gate too"). A slot Play shows at all must hold at least two shots, and
# the feature graphic is the one exact size in the whole set: Play will not publish a listing
# without it, and it refuses anything that is not 1024x500.
MIN_SIDE, MAX_SIDE = 320, 3840
MIN_PER_SLOT, MAX_PER_SLOT = 2, 8
FEATURE_GRAPHIC_PIXELS = (1024, 500)


@dataclass(frozen=True)
class Image:
    """One PNG bound for one language's slot in one Play listing."""

    path: Path
    slot: str
    screen: str  # "" for the feature graphic, which is not a screen
    width: int
    height: int

    @property
    def order(self):
        """Gallery position. Unknown screens sort after the known ones rather than vanishing."""
        try:
            return (0, SCREEN_ORDER.index(self.screen), "")
        except ValueError:
            return (1, 0, self.screen)


def collect_images(directory, locales):
    """Read `showcase-screenshots/android/` into `{locale: {slot: (Image, ...)}}`, in gallery order.

    Returns `(by_locale, skipped)`. A file that matches no convention is **reported**, never
    ignored: Play leaves an unwritten slot showing whatever was there before and answers 200 either
    way, so a typo'd name would read as a successful upload of a gallery that never changed.
    """
    directory = Path(directory)
    if not directory.is_dir():
        raise ListingError(f"no such screenshot directory: {directory}")
    by_locale = {locale: {} for locale in locales}
    skipped = []
    for path in sorted(directory.iterdir()):
        if not path.is_file():
            continue
        shot = SCREENSHOT_NAME.match(path.name)
        graphic = FEATURE_GRAPHIC_NAME.match(path.name)
        matched = shot or graphic
        if not matched or matched.group("locale") not in by_locale:
            skipped.append(path.name)
            continue
        width, height = png_dimensions(path)
        slot = FORM_FACTOR_SLOTS[shot.group("form")] if shot else FEATURE_GRAPHIC
        by_locale[matched.group("locale")].setdefault(slot, []).append(
            Image(
                path=path,
                slot=slot,
                screen=shot.group("screen") if shot else "",
                width=width,
                height=height,
            )
        )
    for slots in by_locale.values():
        for slot, images in slots.items():
            slots[slot] = tuple(sorted(images, key=lambda image: image.order))
    return by_locale, skipped


def measure_images(by_locale, tags):
    """Every reason Play would reject these galleries, in plain words. Empty means it would not.

    `tags` maps catalog locale -> Play tag, so a complaint names the listing a human would open.
    A locale with no captures at all is **not** a problem here: `--screenshots` is opt-in, and a
    run that uploads copy alone is a legitimate one.
    """
    problems = []
    for locale, slots in sorted(by_locale.items()):
        if not slots:
            continue
        tag = tags.get(locale, locale)
        for slot, images in sorted(slots.items()):
            name = SLOT_NAMES[slot]
            if slot == FEATURE_GRAPHIC:
                if len(images) != 1:
                    problems.append(f"{tag}: {len(images)} feature graphics, Play takes exactly 1")
                for image in images:
                    if (image.width, image.height) != FEATURE_GRAPHIC_PIXELS:
                        problems.append(
                            f"{tag}: {image.path.name} is {image.width}x{image.height}, and Play's "
                            f"feature graphic must be exactly "
                            f"{FEATURE_GRAPHIC_PIXELS[0]}x{FEATURE_GRAPHIC_PIXELS[1]}"
                        )
                continue
            if not MIN_PER_SLOT <= len(images) <= MAX_PER_SLOT:
                problems.append(
                    f"{tag}: {len(images)} {name} screenshots, Play takes "
                    f"{MIN_PER_SLOT}-{MAX_PER_SLOT}"
                )
            for image in images:
                if not all(MIN_SIDE <= side <= MAX_SIDE for side in (image.width, image.height)):
                    problems.append(
                        f"{tag}: {image.path.name} is {image.width}x{image.height}, and every side "
                        f"must be between {MIN_SIDE} and {MAX_SIDE} pixels"
                    )
    return problems


def image_payloads(directory, locales=None):
    """`{Play tag: {slot: (Path, ...)}}` for `directory`, ordered as they should be uploaded.

    The transport half wants paths, not `Image`s, and it wants them keyed the way Play is called.
    Anything that would be rejected raises **here**, before an edit is opened.
    """
    mapped = play_locales() if locales is None else locales
    tags = {code: tag for code, _language, tag in mapped}
    by_locale, skipped = collect_images(directory, tuple(tags))
    if skipped:
        raise ListingError(
            f"{len(skipped)} file(s) in {directory} match no naming convention and would be "
            f"silently left out: {', '.join(skipped[:6])}"
            f"{'...' if len(skipped) > 6 else ''}\n"
            "Screenshots are <form>-<locale>-<screen>.png (form: phone, tablet-7, tablet-10) and "
            "the feature graphic is feature-graphic-<locale>.png: see "
            "docs/store-screenshots.md."
        )
    problems = measure_images(by_locale, tags)
    if problems:
        raise ListingError(
            "Google Play would reject these images:\n  " + "\n  ".join(problems)
        )
    return {
        tags[locale]: {
            slot: tuple(image.path for image in images) for slot, images in sorted(slots.items())
        }
        for locale, slots in by_locale.items()
        if slots
    }


def measure(listings, notes, limits):
    """Every field measured against the Play row of `store-listing.md`'s "Field limits" table.

    The point is to fail *here* rather than at Play. It duplicates no numbers: the caps come from
    the same parsed table the store-copy check in CI enforces, so this can only reject copy that check
    would also reject; and it catches the case that job cannot, which is a limit changing between
    the branch turning green and the upload.
    """
    play = limits[STORE]
    measured = []
    for tag, payload in listings.items():
        for field, cap in (
            ("title", play.name),
            ("shortDescription", play.short),
            ("fullDescription", play.description),
        ):
            if cap is not None:
                measured.append(Measurement(f"{field} / {tag}", cap, len(payload[field])))
    cap = whats_new_caps(limits).get(STORE)
    for tag, note in notes.items():
        if cap is not None:
            measured.append(Measurement(f"release note / {tag}", cap, len(note)))
    return measured


# ---------------------------------------------------------------------------------------------
# Reading back what Play actually holds
# ---------------------------------------------------------------------------------------------

SLOT_ORDER = (FEATURE_GRAPHIC, PHONE, SEVEN_INCH, TEN_INCH)

COPY_FIELDS = ("title", "shortDescription", "fullDescription")


def compare_live(state, listings, images=None):
    """`(report lines, drift)` for what Play holds against what this repo would push.

    Pure, so the comparison is testable without a network or a fake client; `publish_play.py` owns
    the reads that produce `state`.

    The drift list is the point. A store listing is the one part of the product with no compiler
    and no test suite behind it: it is edited in a console by a human, and a hand-tweak there is
    invisible here until someone reads both side by side. Comparing the live text against
    the resolved listing; the document that claims to be the single source of the copy; is
    what turns that claim into something that can fail.
    """
    lines, drift = [], []
    for tag in listings:
        entry = state.get(tag)
        if entry is None or entry.get("missing"):
            lines.append(f"  {tag}: NO LISTING: Play has never been given this language")
            drift.append(f"{tag}: no listing (nothing has been published for it yet)")
            continue
        parts = []
        live = entry.get("listing") or {}
        wanted = listings[tag]
        differing = [
            field
            for field in COPY_FIELDS
            if field in wanted and (live.get(field) or "") != wanted[field]
        ]
        parts.append("copy matches" if not differing else "copy DIFFERS (" + ", ".join(differing) + ")")
        drift.extend(f"{tag}: live {field} differs from the resolved listing" for field in differing)
        for slot in SLOT_ORDER:
            if slot not in entry.get("images", {}):
                continue
            count = entry["images"][slot]
            expected = None if images is None else len(images.get(tag, {}).get(slot, []))
            if expected is None:
                parts.append(f"{SLOT_NAMES[slot]} {count}")
                continue
            parts.append(f"{SLOT_NAMES[slot]} {count}/{expected}")
            if count != expected:
                drift.append(
                    f"{tag} · {SLOT_NAMES[slot]}: {count} live, {expected} in the capture directory"
                )
        lines.append(f"  {tag}: " + "  ·  ".join(parts))
    return lines, drift
