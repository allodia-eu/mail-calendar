#!/usr/bin/env python3
"""The changelog fragment format; one file per user-facing change, read by two tools.

`docs/changelog.md` used to be one 1,900-line file that every user-facing PR had to edit, at the
top, in seven languages. Two PRs in flight always conflicted, in the same place, every time; and
because AGENTS.md binds the note to a `/VERSION` bump, they conflicted in `/VERSION`, `Cargo.toml`
and `clients/apple/project.yml` too. The fix is structural rather than procedural: a change writes
**a new file nobody else is writing**, and the release assembles them.

    docs/changelog/unreleased/<slug>.md    one pending change, per-locale notes
    docs/changelog/unreleased/_summary.md  optional, release-time: sections a cap cannot enumerate
    docs/changelog/released/<X.Y.Z>.md     what a release actually shipped, per store group

This module owns that format so the two tools that read it cannot disagree about what it means:

    scripts/ci/check_store_copy_length.py   measures every note against the store it is pasted into
    scripts/dev/release.py                  assembles the fragments into a released note

It also owns the **platform -> store map**, deliberately as code rather than a table in a document.
A doc can drift from what is enforced; this cannot, and both `changelog.md` and `store-listing.md`
link here instead of restating it.

**Python 3.9-compatible on purpose.** `/usr/bin/python3` on a stock macOS is 3.9, and the gate in
AGENTS.md says `python3`; so a checker that needs 3.10 is a gate that some hosts silently cannot
run. No `match`, no `zip(strict=)`, no PEP 604 unions outside annotations.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHANGELOG_DIR = REPO_ROOT / "docs" / "changelog"
UNRELEASED_DIR = CHANGELOG_DIR / "unreleased"
RELEASED_DIR = CHANGELOG_DIR / "released"

# The release-time summary file. Underscore-prefixed because it is *not* a fragment; it carries no
# `Platforms:`/`Bump:`, nobody's feature PR writes one, and `load_fragments` must not read it as a
# malformed change. It is consumed and deleted by the release exactly as the fragments are.
SUMMARY_PATH = UNRELEASED_DIR / "_summary.md"
SETTINGS_PATH = REPO_ROOT / "project.inlang" / "settings.json"

# The stores, exactly as `store-listing.md`'s "Field limits" table names them. Order is the table's,
# and it is also the order a release note lists its paste targets in.
STORES = ("Microsoft Store", "App Store Connect", "Google Play")

# Which console a platform's note is pasted into. `ios` covers iPadOS: they share one App Store
# record, so they cannot be given different notes even in principle. Linux has no store yet, so a
# Linux-only change is written down and shipped, but measured against nothing.
PLATFORM_STORES = {
    "macos": ("App Store Connect",),
    "ios": ("App Store Connect",),
    "windows": ("Microsoft Store",),
    "android": ("Google Play",),
    "linux": (),
}

# Canonical order; used for section headings, so two runs never disagree about "macos, ios".
PLATFORM_ORDER = ("macos", "ios", "windows", "android", "linux")

# What a human is told to open. Both Apple platforms map to one store but to two *records*, so the
# paste line names them separately; the store map above is what decides the character limit.
PASTE_TARGETS = {
    "macos": "App Store Connect (macOS)",
    "ios": "App Store Connect (iOS/iPadOS)",
    "windows": "Microsoft Store",
    "android": "Google Play",
    "linux": "",
}

# `Platforms: all`; the common case, and the one a fragment gets by default.
ALL_PLATFORMS = "all"

# The one `##` heading in a released note that is not a platform group. It carries each consumed
# fragment's headline and engineering commentary, because releasing deletes the fragments and that
# rationale is the most expensive thing in them to reconstruct. Once, at the end of the file,
# repeating it under all five platform sections would triple the length of a release note.
APPENDIX_HEADING = "Changes in this release"

BUMPS = ("minor", "patch")

# Locale code -> the display name both documents already use. A language reaches a store listing
# under its own endonym, never an English exonym; the same rule `mailcal-l10n` enforces on the
# catalog's `settings_language_<loc>` key.
LOCALE_NAMES = {
    "en": "English",
    "nl": "Nederlands",
    "de": "Deutsch",
    "fr": "Français",
    "es": "Español",
    "it": "Italiano",
    "pt": "Português",
}


class DocumentShapeError(Exception):
    """A document no longer looks the way the tools read it.

    Raised rather than returning "nothing to measure", because a scraper that quietly finds nothing
    is indistinguishable from a green run and would hide exactly the drift it exists to catch.
    """


class FragmentError(DocumentShapeError):
    """A fragment is malformed; an authoring mistake, not a scraper that fell behind.

    A subclass because the "found nothing must not pass" property is the same one; a separate class
    because the *advice* differs. A reworded heading in `store-listing.md` means fix the checker; an
    unknown `Platforms:` tag means fix the file you just wrote.
    """


_HEADING = re.compile(r"^(#{1,6})\s+(.*)$")
_LABEL = re.compile(r"^\*\*([^*]+)\*\*$")
_KEY = re.compile(r"^([A-Za-z][A-Za-z ]*):\s*(.*?)\s*$")
_VERSION = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def labelled_blocks(body):
    """Every fenced block in `body` paired with the nearest `**Label**` line above it.

    The label is the language name as the documents write it ("Nederlands", not "nl"); the tools
    report what a reader would search for, and need no locale-code mapping to do it.

    This lives here rather than in the checker because the fragment format deliberately *is* the
    shape `store-listing.md` already uses. One implementation, so a fragment and a listing field can
    never be read by two subtly different parsers.
    """
    out = []
    label = None
    current = []
    inside = False
    for line in body:
        if line.startswith("```"):
            if inside:
                if label is not None:
                    out.append((label, "\n".join(current)))
                current = []
                label = None
            inside = not inside
            continue
        if inside:
            current.append(line)
            continue
        matched = _LABEL.match(line.strip())
        if matched:
            label = matched.group(1).strip()
    return out


def catalog_locales():
    """Every locale *code* the app ships, in the catalog's own order.

    Read from `project.inlang/settings.json` rather than listed here: the catalog is the single
    source of the language list (AGENTS.md), and a note set that lags it is not a stale document,
    it is a release some users get in a language the app speaks and the store does not.

    Codes rather than names because a store console is keyed on them: `msstore_listing.py` needs
    `nl` to reach Partner Center's `nl-nl`, while the documents these fragments live in are labelled
    with the endonym. Both readings come from this one list.
    """
    settings = json.loads(SETTINGS_PATH.read_text(encoding="utf-8"))
    locales = settings.get("locales")
    if not locales:
        raise DocumentShapeError(f"{SETTINGS_PATH} lists no locales")
    unknown = [code for code in locales if code not in LOCALE_NAMES]
    if unknown:
        raise DocumentShapeError(
            f"no display name for locale(s) {', '.join(unknown)}: add them to LOCALE_NAMES in "
            f"{Path(__file__).name}, under the language's own endonym."
        )
    return tuple(locales)


def catalog_languages():
    """The display names of every locale the app ships, in the catalog's own order."""
    return tuple(LOCALE_NAMES[code] for code in catalog_locales())


def store_targets(platforms):
    """The distinct stores a note for `platforms` is pasted into, in the "Field limits" order."""
    return tuple(
        store
        for store in STORES
        if any(store in PLATFORM_STORES[platform] for platform in platforms)
    )


def paste_targets(platforms):
    """The store *records* a note for `platforms` is pasted into; macOS and iOS listed apart."""
    out = []
    for store in STORES:
        for platform in PLATFORM_ORDER:
            if platform in platforms and store in PLATFORM_STORES[platform]:
                out.append(PASTE_TARGETS[platform])
    return tuple(out)


def parse_platforms(value, where):
    """`all` or a comma-separated subset, normalised to canonical order."""
    raw = [item.strip() for item in value.split(",") if item.strip()]
    if not raw:
        raise FragmentError(f"{where}: 'Platforms:' is empty: say `all` or list the platforms.")
    if raw == [ALL_PLATFORMS]:
        return PLATFORM_ORDER
    unknown = [item for item in raw if item not in PLATFORM_STORES]
    if unknown:
        raise FragmentError(
            f"{where}: unknown platform tag(s) {', '.join(unknown)}. "
            f"Use {', '.join(PLATFORM_ORDER)} or `all`."
        )
    return tuple(platform for platform in PLATFORM_ORDER if platform in raw)


@dataclass(frozen=True)
class Fragment:
    """One pending user-facing change, as `docs/changelog/unreleased/<slug>.md` states it."""

    slug: str
    headline: str
    platforms: tuple
    bump: str
    notes: dict
    commentary: str

    @property
    def sort_key(self):
        """`minor` before `patch`, alphabetical by slug within each; so assembly is repeatable.

        A reader scanning a release note wants the new capability before the fix, and a release
        assembled twice must produce the same bytes or the diff is noise.
        """
        return (BUMPS.index(self.bump), self.slug)


@dataclass(frozen=True)
class Section:
    """One bullet set, and every platform whose note is exactly that set."""

    platforms: tuple
    fragments: tuple

    @property
    def paste_line(self):
        targets = paste_targets(self.platforms)
        return " · ".join(targets) if targets else "(no store yet)"


@dataclass(frozen=True)
class ReleaseSection:
    """One `## <platforms>` section of a `docs/changelog/released/<X.Y.Z>.md` file."""

    platforms: tuple
    notes: dict


def parse_fragment(text, slug, languages):
    """Read one fragment. Every failure here is an authoring mistake with a named fix."""
    lines = text.splitlines()
    where = f"docs/changelog/unreleased/{slug}.md"

    headline = ""
    for line in lines:
        heading = _HEADING.match(line)
        if heading and len(heading.group(1)) == 1:
            headline = heading.group(2).strip()
            break
    if not headline:
        raise FragmentError(f"{where}: no `# Headline` line: the first line names the change.")

    # The header is everything above the first `**Language**` line. Scoped rather than scanned
    # document-wide, so a note whose own prose happens to open `Something: …` can never be read as
    # a `Platforms:` or `Bump:` declaration.
    header = lines
    for position, line in enumerate(lines):
        if _LABEL.match(line.strip()):
            header = lines[:position]
            break

    keys = {}
    commentary = []
    for line in header:
        if line.startswith(">"):
            commentary.append(line)
            continue
        matched = _KEY.match(line)
        if matched and matched.group(1) in ("Platforms", "Bump"):
            keys[matched.group(1)] = matched.group(2)

    if "Platforms" not in keys:
        raise FragmentError(f"{where}: no `Platforms:` line: say `all` or list the platforms.")
    if "Bump" not in keys:
        raise FragmentError(f"{where}: no `Bump:` line: `minor` for a feature, `patch` for a fix.")
    if keys["Bump"] not in BUMPS:
        raise FragmentError(
            f"{where}: Bump is '{keys['Bump']}', expected one of {', '.join(BUMPS)}."
        )

    platforms = parse_platforms(keys["Platforms"], where)

    notes = {}
    for language, block in labelled_blocks(lines):
        if language in notes:
            raise FragmentError(f"{where}: two **{language}** notes: there can be only one.")
        if language not in languages:
            raise FragmentError(
                f"{where}: **{language}** is not a catalog language. Expected one of "
                f"{', '.join(languages)}."
            )
        notes[language] = block
    if not notes:
        raise FragmentError(
            f"{where}: no localized notes found. Each is a `**Language**` line followed by a "
            "fenced block."
        )
    missing = [language for language in languages if language not in notes]
    if missing:
        raise FragmentError(
            f"{where}: no note for {', '.join(missing)}. Every catalog locale ships in every "
            "release, so a fragment carries them all."
        )

    return Fragment(
        slug=slug,
        headline=headline,
        platforms=platforms,
        bump=keys["Bump"],
        notes=notes,
        commentary="\n".join(commentary).strip(),
    )


def load_fragments(directory=None, languages=None):
    """Every pending fragment, sorted for assembly. An empty directory is legal; nothing pending."""
    directory = UNRELEASED_DIR if directory is None else Path(directory)
    languages = catalog_languages() if languages is None else languages
    if not directory.is_dir():
        return []
    fragments = [
        parse_fragment(path.read_text(encoding="utf-8"), path.stem, languages)
        for path in sorted(directory.glob("*.md"))
        if not path.name.startswith("_")
    ]
    return sorted(fragments, key=lambda fragment: fragment.sort_key)


def load_summaries(path=None, languages=None):
    """Authored replacements for sections whose assembled notes cannot fit their store.

    A release that consumes fourteen fragments cannot enumerate them inside Google Play's 500
    characters; 500 divided by fourteen is thirty-five characters a change, which no trim reaches.
    Such a section is written rather than assembled, and this is where that text lives.

    Keyed by the **exact platform tuple** of the section it replaces; the same heading assembly
    emits; so a summary written for `android` can never silently attach to a differently-grouped
    section in a later release. Absent file means no summary, which is the normal case: an ordinary
    release enumerates its changes and needs none.
    """
    path = SUMMARY_PATH if path is None else Path(path)
    languages = catalog_languages() if languages is None else languages
    if not path.is_file():
        return {}
    where = "docs/changelog/unreleased/{}".format(path.name)
    summaries = {}
    for platforms, body in platform_sections(path.read_text(encoding="utf-8").splitlines(), where):
        heading = ", ".join(platforms)
        if platforms in summaries:
            raise FragmentError(f"{where}: two '{heading}' sections: there can be only one.")
        notes = dict(labelled_blocks(body))
        unknown = [language for language in notes if language not in languages]
        if unknown:
            raise FragmentError(
                f"{where}: **{unknown[0]}** is not a catalog language. Expected one of "
                f"{', '.join(languages)}."
            )
        missing = [language for language in languages if language not in notes]
        if missing:
            raise FragmentError(
                f"{where}: the '{heading}' summary has no note for {', '.join(missing)}. A summary "
                "replaces the assembled notes wholesale, so it carries every catalog locale exactly "
                "as a fragment does."
            )
        summaries[platforms] = notes
    if not summaries:
        raise FragmentError(
            f"{where}: no `## <platforms>` sections found. Write one per section you are "
            "summarizing, or delete the file. An empty summary reads like a release that needed "
            "none."
        )
    return summaries


def section_notes(section, language, summaries=None):
    """What a section's `language` block says: its authored summary, else its fragments joined.

    Verbatim and blank-line separated; never re-wrapped into bullets. A fragment's note is already
    finished prose, and bulleting it would mangle the one part of the file a user actually reads.
    """
    if summaries and section.platforms in summaries:
        return summaries[section.platforms][language]
    return "\n\n".join(fragment.notes[language] for fragment in section.fragments)


def group_sections(fragments):
    """One section per **distinct bullet set**, not one per store.

    When every fragment is `all`; the common case; that is a single section carrying every note,
    which is exactly the workload the one-file changelog had. Per-platform sections appear only when
    the content genuinely differs, so a Mac-only fix never reaches the iPhone's release note.
    """
    ordered = sorted(fragments, key=lambda fragment: fragment.sort_key)
    buckets = {}
    for platform in PLATFORM_ORDER:
        chosen = tuple(f for f in ordered if platform in f.platforms)
        if not chosen:
            continue
        key = tuple(f.slug for f in chosen)
        if key in buckets:
            buckets[key][0].append(platform)
        else:
            buckets[key] = ([platform], chosen)
    return [Section(tuple(platforms), chosen) for platforms, chosen in buckets.values()]


def platform_sections(lines, where):
    """Yield `(platforms, body_lines)` for every `## <platforms>` heading, up to the appendix.

    Shared by the two documents that are keyed on a section's platform set; an assembled release
    note and the release-time `_summary.md`; so a summary can never key its sections by a slightly
    different reading of the same heading than the note it replaces.
    """
    index = 0
    while index < len(lines):
        heading = _HEADING.match(lines[index])
        if not (heading and len(heading.group(1)) == 2):
            index += 1
            continue
        if heading.group(2).strip() == APPENDIX_HEADING:
            # Commentary, not copy. Skipped by name rather than by shape, so a mistyped platform
            # heading below still fails loudly instead of quietly dropping a note nobody measures.
            break
        platforms = parse_platforms(heading.group(2), where)
        body = []
        index += 1
        while index < len(lines):
            nested = _HEADING.match(lines[index])
            if nested and len(nested.group(1)) <= 2:
                break
            body.append(lines[index])
            index += 1
        yield platforms, body


def parse_release(text, version):
    """Read an assembled `released/<X.Y.Z>.md` back; its sections and their notes.

    Released files are generated, so this is not a courtesy parser: it is what lets the store-copy
    check measure history against the same caps a fragment is measured against, and what lets
    `version-sync` prove `/VERSION` has a note.
    """
    where = f"docs/changelog/released/{version}.md"
    sections = []
    for platforms, body in platform_sections(text.splitlines(), where):
        notes = dict(labelled_blocks(body))
        if not notes:
            raise DocumentShapeError(
                f"{where}: the '{', '.join(platforms)}' section carries no localized notes."
            )
        sections.append(ReleaseSection(platforms=platforms, notes=notes))
    if not sections:
        raise DocumentShapeError(
            f"{where}: no `## <platforms>` sections found: a released note is written by "
            "scripts/dev/release.py and should not be hand-shaped."
        )
    return tuple(sections)


def load_releases(directory=None):
    """Every released note, keyed by version. Sorted oldest first."""
    directory = RELEASED_DIR if directory is None else Path(directory)
    if not directory.is_dir():
        return []
    out = []
    for path in sorted(directory.glob("*.md")):
        if not _VERSION.match(path.stem):
            raise DocumentShapeError(
                f"docs/changelog/released/{path.name} is not named X.Y.Z.md: the filename is how "
                "/VERSION finds its note."
            )
        out.append((path.stem, parse_release(path.read_text(encoding="utf-8"), path.stem)))
    return sorted(out, key=lambda item: version_tuple(item[0]))


def version_tuple(version):
    """`"0.13.5"` -> `(0, 13, 5)`, for comparison and for computing the next one."""
    matched = _VERSION.match(version.strip())
    if not matched:
        raise ValueError(f"not a MAJOR.MINOR.PATCH version: {version!r}")
    return (int(matched.group(1)), int(matched.group(2)), int(matched.group(3)))


def next_version(current, bump):
    """The version a `minor` or `patch` release moves `current` to."""
    major, minor, patch = version_tuple(current)
    if bump == "minor":
        return "{}.{}.0".format(major, minor + 1)
    return "{}.{}.{}".format(major, minor, patch + 1)
