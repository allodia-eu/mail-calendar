#!/usr/bin/env python3
"""Turn the resolved store listing (`branding/<brand>-listing.md`) into a Microsoft Store submission
payload, and say what changed.

This is the half of the store-listing push that never touches the network: it reads the contract,
measures it against the caps the contract itself states, collects the screenshots, merges the
result into a submission resource, and renders a diff a human can approve. All the tests live here
([`tests/test_msstore_payload.py`](tests/test_msstore_payload.py)); the transport half
([`msstore_api.py`](msstore_api.py)) needs a Partner Center account to exercise, so everything that
can be decided without one is decided here.

**It reads the document through the checker's own scraper.** `scripts/ci/check_store_copy_length.py`
already parses this file to measure it; a second parser would be a second reading of one document,
and the one that pastes into a store console would be the one nobody tested. So `sections`,
`fenced_blocks`, `one_section` and `parse_keystore_tokens` are imported from there; as is the
"Field limits" table itself, which `measure` is handed; and `labelled_blocks` comes from the
fragment format both documents share.

**What it owns, and what it deliberately leaves alone.** A listing carries more than the copy this
repo writes down. Pushed: **title**, **description**, the **product features** list, the
**copyright** line, and; only when asked; **release notes**, **screenshots** and the
**`.msixupload`**. Left untouched: Store search terms, price and availability, age rating, and the
properties Partner Center owns outright (privacy-policy URL, support contact, website; the API
documents those as ignored).

**The package is here so a release needs no visit to the console.** Partner Center's Submit button
commits the listing the page loaded, not the one the API staged: on 0.5.0 that silently reverted the
copy and gallery to the previous release's, twice, with no error anywhere. Uploading the bundle was
the only step that *forced* a console visit, so it is the step that made the trap unavoidable,
`--package` plus `--commit` closes it. The first version of this note blamed the package upload
itself; the second reversion happened with no package upload at all, which is what corrected it.
Nothing outside the resolved listing is invented here; a field this repo does not write down is
a field a human still owns.
"""

from __future__ import annotations

import copy
import difflib
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))

import brand  # noqa: E402  (path set above so this runs as a script)

from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    LOCALE_NAMES,
    DocumentShapeError,
    catalog_locales,
    labelled_blocks,
    load_releases,
    store_targets,
)
from check_store_copy_length import (  # noqa: E402
    KEYSTORE_TOKEN,
    LIMITS_PATH,
    listing_path,
    fenced_blocks,
    one_section,
    parse_keystore_tokens,
    search_term_words,
    sections,
)

STORE = "Microsoft Store"


class ListingError(RuntimeError):
    """An input this run cannot use; a missing directory, a file that is not a PNG.

    Deliberately not a `DocumentShapeError`: that one means "the tool has fallen behind the
    document, go fix the scraper", which is the wrong advice entirely for a mistyped path.
    """

# Catalog locale -> the Partner Center listing language it is published under. Spanish and
# Portuguese are the **European** variants (AGENTS.md: "Portuguese and Spanish are the European
# variants, pt-PT / es-ES"), which is why they are not `es-mx`/`pt-br`. This map is only a
# *preference*: if the submission already carries a listing for the same language under another
# code, `resolve_store_language` reuses that one rather than opening a second Dutch listing.
STORE_LANGUAGES = {
    "en": "en-us",
    "nl": "nl-nl",
    "de": "de-de",
    "fr": "fr-fr",
    "es": "es-es",
    "it": "it-it",
    "pt": "pt-pt",
}

# The gallery order, hero first. Not alphabetical on purpose: alphabetical opens the listing on
# `add-account`, which store-listing.md's own "Known gaps" calls out as mostly empty space, and the
# first screenshot is the one a shopper sees without scrolling. Screens not named here follow, in
# alphabetical order, so an unknown capture is never silently dropped.
#
# `list-dark` is the mailbox list again in the dark appearance, and it sits third rather than second
# on purpose: the first two slots go to the two things the product *is*; mail, then calendar; and
# the dark shot then answers the question a shopper has already formed looking at them. Second would
# spend the most valuable slot in the gallery on a screen they just saw.
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

# `<locale>-<screen>.png`, the layout `showcase-screenshots/windows/` already uses.
SCREENSHOT_NAME = re.compile(r"^(?P<locale>[a-z]{2})-(?P<screen>[a-z0-9][a-z0-9-]*)\.png$")

# What the Store accepts for a desktop screenshot. Asserted here because the alternative is
# learning it from a rejected submission; AGENTS.md's "a store upload is a remote gate too".
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
MIN_PIXELS = (1366, 768)
MAX_PIXELS = (3840, 2160)
MAX_SCREENSHOTS = 10
IMAGE_TYPE = "Screenshot"


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


# A `.msixupload` names its version, and the name is the only thing about the bundle this tool can
# read without unzipping 150 MB. It is enough for the two refusals that matter; see `load_package`.
PACKAGE_SUFFIX = ".msixupload"
PACKAGE_VERSION = re.compile(r"_(?P<version>\d+\.\d+\.\d+\.\d+)_")


@dataclass(frozen=True)
class Package:
    """One `.msixupload`, bound for the submission's package list."""

    path: Path
    version: str

    @property
    def zip_name(self) -> str:
        """Its path inside the upload archive; the same name the package entry refers to."""
        return self.path.name


def load_package(path, expected_version: str) -> Package:
    """The bundle at `path`, refused unless it is this release's.

    Two refusals, both of which are otherwise learned from a slow round-trip:

    * **The wrong version.** 0.5.0 was nearly packaged from a branch whose `/VERSION` still said
      `0.4.0`; a bundle the Store would have taken as a re-upload of a version it has already
      ingested, and rejected. The name carries the version, so it costs nothing to check here.
    * **A non-zero revision.** The Store reserves the fourth field for its own repackaging and says
      so only after the whole bundle is ingested. `clients/windows/package.ps1` refuses to *build*
      one; this refuses to *upload* one, because the two are separate doors into the same mistake.
    """
    path = Path(path)
    if not path.is_file():
        raise ListingError(f"no package at {path}")
    if path.suffix != PACKAGE_SUFFIX:
        raise ListingError(
            f"{path.name} is not a {PACKAGE_SUFFIX}: the Store takes the bundle that "
            f"clients/windows/package.ps1 writes into AppPackages/, not a bare .msix"
        )
    if path.stat().st_size == 0:
        raise ListingError(f"{path.name} is empty")
    found = PACKAGE_VERSION.search(path.name)
    if not found:
        raise ListingError(
            f"{path.name} does not carry a four-field version, so this cannot tell whether it is "
            f"{expected_version}'s bundle. Expected a name like "
            f"Mailcal_{expected_version}.0_x64_arm64_bundle{PACKAGE_SUFFIX}"
        )
    version = found.group("version")
    fields = version.split(".")
    if ".".join(fields[:3]) != expected_version:
        raise ListingError(
            f"{path.name} is version {version}, but this release is {expected_version}. "
            f"Rebuild with clients/windows/package.ps1 from the tagged commit."
        )
    if fields[3] != "0":
        raise ListingError(
            f"{path.name} has a non-zero revision ({version}). The Store refuses that on "
            f"ingestion: bump the build field instead."
        )
    return Package(path=path, version=version)


def merge_package(submission, package: Package):
    """Queue `package` on the submission, returning `(submission, change_or_None)`.

    Added beside whatever is already there rather than replacing it: the Store serves an older
    package to a downlevel Windows, so the list accumulates, and the console does the same.
    Re-running is idempotent; a second call finds the entry it wrote and leaves one.
    """
    packages = list(submission.get("applicationPackages") or ())
    before = tuple(entry.get("fileName") for entry in packages)
    if package.zip_name in before:
        return submission, None
    packages.append({"fileName": package.zip_name, "fileStatus": "PendingUpload"})
    submission["applicationPackages"] = packages
    return submission, FieldChange(
        "package", before, tuple(entry.get("fileName") for entry in packages)
    )


@dataclass(frozen=True)
class Listing:
    """Everything this repo writes down about one language's Microsoft Store listing."""

    locale: str
    language: str  # the endonym, as the documents label it ("Nederlands")
    store_language: str  # the Partner Center listing code ("nl-nl")
    title: str
    description: str
    features: tuple
    copyright: str
    search_terms: tuple = ()
    release_notes: str = None
    screenshots: tuple = ()


# -------------------------------------------------------------------------------------------
# Reading the contract
# -------------------------------------------------------------------------------------------


def _language_of(title: str) -> str:
    """"Shared description; Nederlands" -> "Nederlands"."""
    return title.split("—")[-1].strip()


def product_name(_listing: str = "") -> str:
    """The one title every store and every language uses.

    It is the injected `MAILCAL_APP_NAME` (docs/branding.md), not a line of copy: the launcher, the
    installer and every store listing must agree about what the app is called, and a name written
    in a second file is a name that can disagree with the one the OS shows. The unused argument is
    kept so the three publishers still read as "everything about a listing comes from one call".
    """
    return brand.value("MAILCAL_APP_NAME")


def descriptions(listing: str, store: str = STORE) -> dict:
    """`language -> the description body as `store` would receive it`.

    The `{KEYSTORE}` substitution is applied when the copy still uses it: the token was retired in
    favour of "stored securely on your device", but the substitution is what a console is handed,
    so this pastes what would be pasted rather than what is written.

    `store` exists because the substitution is the one thing about a description that is **not**
    shared: the same body says "the Windows Credential Manager" here and "your device's Keychain" on
    the App Store. It defaults to this module's store, and `appstore_payload` passes its own,
    reusing the reader rather than writing a second one that could drift from it.
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


def feature_lists(listing: str) -> dict:
    """`language -> the Microsoft Store product features`, one per line, blanks dropped."""
    body = one_section(
        listing.splitlines(), 3, r"^Microsoft Store — Product features", "Microsoft Store features"
    )
    found = {
        language: tuple(line for line in block.splitlines() if line.strip())
        for language, block in labelled_blocks(body)
    }
    if not found:
        raise DocumentShapeError("no Microsoft Store feature lists found")
    return found


def search_terms(listing: str) -> dict:
    """`language -> the Microsoft Store search terms`, one per line.

    Not shown to anyone; they only feed the Store's search; but they are listing copy all the
    same, which is why they live in the contract rather than in the console where nobody would
    diff them.
    """
    body = one_section(
        listing.splitlines(), 3, r"^Microsoft Store — Search terms", "Microsoft Store search terms"
    )
    found = {
        language: tuple(line.strip() for line in block.splitlines() if line.strip())
        for language, block in labelled_blocks(body)
    }
    if not found:
        raise DocumentShapeError("no Microsoft Store search-term lists found")
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


def release_notes(version: str) -> dict:
    """`language -> the released note`, taking only the sections the Microsoft Store is pasted.

    A release note is its own contract (`docs/changelog.md`) and is assembled per *platform group*,
    so a macOS-only section is not this store's business. Sections that do reach it are joined in
    file order, which is the order `release.py` wrote them.
    """
    for released, parsed in load_releases():
        if released != version:
            continue
        notes = {}
        for section in parsed:
            if STORE not in store_targets(section.platforms):
                continue
            for language, note in section.notes.items():
                notes[language] = f"{notes[language]}\n\n{note}" if language in notes else note
        if not notes:
            raise DocumentShapeError(
                f"docs/changelog/released/{version}.md has no section the {STORE} is pasted into "
                ": it shipped to no Windows platform, so there is no What's new to push."
            )
        return notes
    raise DocumentShapeError(f"no docs/changelog/released/{version}.md")


# -------------------------------------------------------------------------------------------
# Screenshots
# -------------------------------------------------------------------------------------------


def png_dimensions(path: Path):
    """`(width, height)` from the PNG header; no image library, and none needed for IHDR."""
    with Path(path).open("rb") as handle:
        header = handle.read(24)
    if len(header) < 24 or header[:8] != PNG_MAGIC or header[12:16] != b"IHDR":
        raise ListingError(f"{path} is not a PNG (the Store takes PNG screenshots)")
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


def measure_screenshots(listing: Listing):
    """Every reason the Store would reject this language's gallery, in plain words."""
    problems = []
    if len(listing.screenshots) > MAX_SCREENSHOTS:
        problems.append(
            f"{listing.store_language}: {len(listing.screenshots)} screenshots, the Store takes "
            f"at most {MAX_SCREENSHOTS}"
        )
    for shot in listing.screenshots:
        if not (MIN_PIXELS[0] <= shot.width <= MAX_PIXELS[0]) or not (
            MIN_PIXELS[1] <= shot.height <= MAX_PIXELS[1]
        ):
            problems.append(
                f"{listing.store_language}: {shot.path.name} is {shot.width}x{shot.height}, "
                f"outside the Store's {MIN_PIXELS[0]}x{MIN_PIXELS[1]}...{MAX_PIXELS[0]}x"
                f"{MAX_PIXELS[1]} bounds"
            )
    return problems


# -------------------------------------------------------------------------------------------
# Building the listings
# -------------------------------------------------------------------------------------------


def resolve_locales(locales=None):
    """The catalog locales this run covers, refusing any the console has no language for.

    Shared with the caller so a screenshot sweep and the copy it accompanies can never disagree
    about which languages are in play.
    """
    wanted = tuple(locales) if locales else catalog_locales()
    unknown = [locale for locale in wanted if locale not in STORE_LANGUAGES]
    if unknown:
        raise DocumentShapeError(
            f"no Partner Center listing language for {', '.join(unknown)}: add it to "
            "STORE_LANGUAGES in msstore_payload.py, matching the console's language picker."
        )
    return wanted


def load_listings(listing_md=None, locales=None, version=None, screenshots=None):
    """Every language's listing, exactly as the resolved store listing states it.

    `locales` narrows the run (a single language's screenshots, say); `version` adds that release's
    What's new; `screenshots` is `collect_screenshots`' per-locale result. The latter two are
    opt-in, because a run that silently replaced a gallery with whatever happened to be on disk
    would be a worse tool than the console it replaces.
    """
    text = (Path(listing_md) if listing_md else listing_path()).read_text(encoding="utf-8")
    wanted = resolve_locales(locales)

    title = product_name(text)
    bodies = descriptions(text)
    features = feature_lists(text)
    terms = search_terms(text)
    rights = copyright_line(text)
    notes = release_notes(version) if version else {}
    galleries = screenshots or {}

    built = []
    for locale in wanted:
        language = LOCALE_NAMES[locale]
        for what, source in (
            ("description", bodies),
            ("feature list", features),
            ("search terms", terms),
        ):
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
                store_language=STORE_LANGUAGES[locale],
                title=title,
                description=bodies[language],
                features=tuple(features[language]),
                copyright=rights,
                search_terms=tuple(terms[language]),
                release_notes=notes.get(language),
                screenshots=tuple(galleries.get(locale, ())),
            )
        )
    if notes:
        missing = [item.language for item in built if item.release_notes is None]
        if missing:
            raise DocumentShapeError(
                f"the release note has no text for {', '.join(missing)}: a note carries every "
                "catalog locale (docs/changelog.md)."
            )
    return tuple(built)


def measure(listing: Listing, limits) -> list:
    """Every field of this listing the Microsoft Store would refuse, measured against its own table.

    The store-copy check in CI measures the same copy against the *tightest* store across all three;
    this measures it against the one console it is about to be handed to, which is what decides
    whether this push can succeed. Same table, read the same way, so the two cannot disagree.
    """
    caps = limits[STORE]
    problems = []
    checks = (
        ("title", caps.name, len(listing.title)),
        ("description", caps.description, len(listing.description)),
        ("What's new", caps.whats_new, len(listing.release_notes or "")),
    )
    for what, cap, actual in checks:
        if cap is not None and actual > cap:
            problems.append(
                f"{listing.store_language}: {what} is {actual} characters, limit {cap}"
            )
    if caps.feature_count is not None and len(listing.features) > caps.feature_count:
        problems.append(
            f"{listing.store_language}: {len(listing.features)} product features, limit "
            f"{caps.feature_count}"
        )
    if caps.feature_chars is not None:
        for number, feature in enumerate(listing.features, start=1):
            if len(feature) > caps.feature_chars:
                problems.append(
                    f"{listing.store_language}: feature {number} is {len(feature)} characters, "
                    f"limit {caps.feature_chars}"
                )
    if caps.search_count is not None and len(listing.search_terms) > caps.search_count:
        problems.append(
            f"{listing.store_language}: {len(listing.search_terms)} search terms, limit "
            f"{caps.search_count}"
        )
    if caps.search_words is not None:
        # The budget that actually binds, and the one nobody counts by hand: seven short terms can
        # sail past 21 words without any single term looking long, and a hyphen costs a whole word.
        spent = search_term_words(listing.search_terms)
        if len(spent) > caps.search_words:
            problems.append(
                f"{listing.store_language}: search terms spend {len(spent)} words, limit "
                f"{caps.search_words} ({' '.join(spent)})"
            )
    if caps.search_chars is not None:
        for number, term in enumerate(listing.search_terms, start=1):
            if len(term) > caps.search_chars:
                problems.append(
                    f"{listing.store_language}: search term {number} is {len(term)} characters, "
                    f"limit {caps.search_chars}"
                )
    return problems + measure_screenshots(listing)


# -------------------------------------------------------------------------------------------
# Merging into a submission
# -------------------------------------------------------------------------------------------


def resolve_store_language(existing, listing: Listing) -> str:
    """The key this listing belongs under in the submission's `listings` map.

    Prefers a key the submission already has for the same language; Partner Center may hold Dutch
    as `nl` or `nl-nl` depending on when the listing was created, and writing the other one would
    publish a second Dutch listing rather than update the first.
    """
    lowered = {key.lower(): key for key in existing}
    preferred = listing.store_language.lower()
    if preferred in lowered:
        return lowered[preferred]
    for key in sorted(lowered):
        if key.split("-")[0] == listing.locale:
            return lowered[key]
    return listing.store_language


@dataclass(frozen=True)
class FieldChange:
    """One field of one language, before and after. `note` explains a deliberate non-change."""

    field: str
    before: object
    after: object
    note: str = ""

    @property
    def changed(self) -> bool:
        return self.before != self.after


@dataclass(frozen=True)
class LanguagePlan:
    """What a push would do to one language's listing."""

    listing: Listing
    key: str
    is_new: bool
    changes: tuple

    @property
    def changed(self):
        return tuple(change for change in self.changes if change.changed)


def merge(submission, listings):
    """Fold the listings into a copy of `submission`. Returns `(submission, plans, uploads)`.

    The submission is deep-copied rather than mutated: the caller diffs the original against the
    result to show a human what it is about to do, and a diff against something already changed
    would always be empty.
    """
    updated = copy.deepcopy(submission)
    existing = updated.setdefault("listings", {})
    plans = []
    uploads = []
    for listing in listings:
        key = resolve_store_language(existing, listing)
        is_new = key not in existing
        entry = existing.setdefault(key, {})
        base = entry.setdefault("baseListing", {})
        # The per-listing title is a **reserved-name override**, and Partner Center leaves it unset
        # when there is only one reserved name; which is our case, and why every listing came back
        # without the field. Setting it would be the one write in this push that the Store can
        # reject (a title that is not a reserved name is refused), in exchange for a value the
        # console already derives. So it is kept in sync where it exists and never introduced where
        # it does not.
        has_title = bool(base.get("title"))
        changes = [
            FieldChange("title", base.get("title"), listing.title)
            if has_title
            else FieldChange(
                "title", None, None, note="not set here: the Store uses the reserved app name"
            ),
            FieldChange("description", base.get("description"), listing.description),
            FieldChange("features", tuple(base.get("features") or ()), listing.features),
            FieldChange("search terms", tuple(base.get("keywords") or ()), listing.search_terms),
            FieldChange(
                "copyright", base.get("copyrightAndTrademarkInfo"), listing.copyright
            ),
        ]
        if has_title:
            base["title"] = listing.title
        base["description"] = listing.description
        base["features"] = list(listing.features)
        # The API calls them `keywords`; Partner Center's form calls them Search terms. The
        # document uses the console's word, because that is what someone comparing the two reads.
        base["keywords"] = list(listing.search_terms)
        base["copyrightAndTrademarkInfo"] = listing.copyright
        if listing.release_notes is not None:
            changes.append(
                FieldChange("release notes", base.get("releaseNotes"), listing.release_notes)
            )
            base["releaseNotes"] = listing.release_notes
        if listing.screenshots:
            before = tuple(
                image.get("fileName") for image in base.get("images") or ()
            )
            images = [
                {
                    "fileName": shot.zip_name,
                    "fileStatus": "PendingUpload",
                    "description": "",
                    "imageType": IMAGE_TYPE,
                }
                for shot in listing.screenshots
            ]
            changes.append(
                FieldChange("screenshots", before, tuple(shot.zip_name for shot in listing.screenshots))
            )
            base["images"] = images
            uploads.extend(listing.screenshots)
        plans.append(LanguagePlan(listing=listing, key=key, is_new=is_new, changes=tuple(changes)))
    return updated, tuple(plans), tuple(uploads)


# -------------------------------------------------------------------------------------------
# Reporting
# -------------------------------------------------------------------------------------------

# How many diff lines to print for a changed description. Enough to see which sentence moved
# without reprinting four thousand characters seven times.
DIFF_CONTEXT_LINES = 12


def _describe(change: FieldChange) -> list:
    """The human-readable body of one changed field."""
    if change.field == "description":
        before = (change.before or "").splitlines()
        after = (change.after or "").splitlines()
        diff = [
            # `-x` / `+x` re-spaced to `- x` / `+ x`, so a description diff reads the same way as
            # the feature-list diff two lines below it.
            f"{line[0]} {line[1:]}"
            for line in difflib.unified_diff(before, after, lineterm="", n=0)
            if not line.startswith(("---", "+++", "@@"))
        ]
        head = diff[:DIFF_CONTEXT_LINES]
        if len(diff) > DIFF_CONTEXT_LINES:
            head.append(f"... {len(diff) - DIFF_CONTEXT_LINES} more changed line(s)")
        return head
    if isinstance(change.after, tuple):
        before = set(change.before or ())
        after = set(change.after)
        return [f"- {item}" for item in change.before or () if item not in after] + [
            f"+ {item}" for item in change.after if item not in before
        ]
    return [f"- {change.before!r}", f"+ {change.after!r}"]


def render_plan(plans, *, images_pushed: bool) -> str:
    """The report a human approves before anything is written."""
    out = []
    for plan in plans:
        marker = " (NEW listing language)" if plan.is_new else ""
        out.append(f"  {plan.key}  {plan.listing.language}{marker}")
        for change in plan.changes:
            if not change.changed:
                out.append(f"    {change.field:<14} = {change.note or 'unchanged'}")
                continue
            out.append(f"    {change.field:<14} ~ changed")
            for line in _describe(change):
                out.append(f"      {line}")
        if not images_pushed:
            out.append("    screenshots    - untouched (pass --screenshots to replace them)")
    return "\n".join(out)
