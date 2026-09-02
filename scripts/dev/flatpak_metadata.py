#!/usr/bin/env python3
"""Emit the Linux client's desktop entry and AppStream metainfo from the documents that own them.

Neither file carries copy or a version of its own. A `.desktop` `Comment` is what GNOME's app grid
and search show, and an AppStream `<summary>`/`<description>` is what a software centre shows; so
both are store copy, and store copy is written in the listing this build resolves to,
`branding/<brand>-listing.md` ([`docs/store-listing.md`](../../docs/store-listing.md) rule 1: the
body is edited in one place and used verbatim). The name is not taken from there at all: it is the
injected `MAILCAL_APP_NAME`, so the software centre and the launcher cannot disagree about what the
app is called. The version and release dates come from `/VERSION` and the assembled notes under
`docs/changelog/released/`, because [`docs/versioning.md`](../../docs/versioning.md) allows no
hand-edited literal.

    scripts/dev/flatpak_metadata.py --out-dir <dir>

**What it takes from the listing, and what it deliberately leaves out.** The `<summary>` and
`Comment` are the Google Play short description; one line per locale, already capped at 80
characters and, unlike the feature bullets, free of any capability claim. The `<description>` is
the **first two paragraphs** of the shared body: the ones rule 5 names as never trimmed ("the
sovereignty framing is the point"), which describe the product rather than what a given client can
do today. Every remaining paragraph is a feature list, and store-listing rule 3 forbids copy that
out-runs the capability matrix. Linux ships every bullet but configurable swipe actions, which is
still ⬜ there, and the body is one block reused verbatim, so the bullets stay out rather than being
filtered down to a subset nobody has reviewed. Closing that is a deliberate edit here, not a silent
consequence of a matrix cell flipping.

The documents are read through the checker's own scrapers (`scripts/ci/check_store_copy_length.py`,
`scripts/ci/changelog_fragments.py`) for the reason `store_payload.py` gives: a second reading of
one document would be the reading nobody tested.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from xml.sax.saxutils import escape

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

import brand  # noqa: E402  (path set above so this runs as a script)

from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    LOCALE_NAMES,
    DocumentShapeError,
    catalog_locales,
    labelled_blocks,
)
from check_store_copy_length import (  # noqa: E402
    KEYSTORE_TOKEN,
    fenced_blocks,
    one_section,
    sections,
)

# The application id, and so the basename of every file installed into the sandbox. It is the id
# the client registers with GTK (`RelmApp::new` in clients/linux/src/main.rs) and stores secrets
# under (clients/linux/src/secrets.rs); the desktop entry, the icon and the metainfo must all
# carry it or the shell cannot tie a window to its launcher, and the notification portal has
# nothing to attribute a notification to.
#
# Injected, so it is the same id the binary was compiled with (docs/branding.md). This runs INSIDE
# the Flatpak sandbox, where the brand files arrive with the copied source tree; the same road the
# OAuth credentials take, for the same reason: flatpak-builder forwards no host environment.
APP_ID = brand.value("MAILCAL_APP_ID")

# The paragraphs of the shared body this file is allowed to use. See the module docstring: two, and
# which two is a rule `docs/store-listing.md` already states.
FRAMING_PARAGRAPHS = 2

# `# 0.4.0; 2026-08-04`, the first line of an assembled release note. `release.py` writes it; this
# is the only place the date of a release is recorded, so it is where AppStream's `date` comes from.
RELEASE_HEADING = re.compile(r"^#\s+(?P<version>\d+\.\d+\.\d+)\s+—\s+(?P<date>\d{4}-\d{2}-\d{2})\s*$")


class MetadataError(RuntimeError):
    """An input this run cannot use. Distinct from `DocumentShapeError`, which means "the scraper
    has fallen behind the document"; the wrong advice for a missing output directory."""


def _localized_blocks(body: list[str]) -> dict[str, str]:
    """`{locale: text}` for a section whose fenced blocks are each labelled by a `**Language**`.

    The scraping is `labelled_blocks`, which is deliberately the *one* implementation of this shape:
    the changelog fragment format and this listing share it, and two parsers would eventually read
    one document two ways. All this adds is the language-name -> locale-code mapping the XML needs
    and a reader does not.
    """
    by_language = {name: code for code, name in LOCALE_NAMES.items()}
    return {
        by_language[label]: text.strip()
        for label, text in labelled_blocks(body)
        if label in by_language
    }


# Flathub's quality guidelines: 35 characters, and 10 to 25 preferred. Enforced here rather than
# trusted, because the field is generated and nothing downstream measures it, and a listing held up
# on a guideline costs a volunteer reviewer a comment and a round trip.
SUMMARY_LIMIT = 35


def summaries(listing: str) -> dict[str, str]:
    """The one-line summary per locale, from the listing's own Flathub section.

    Deliberately not Play's short description, which the other stores share: Flathub caps this at a
    third of Play's 80 characters and asks for something a non-technical reader understands, so one
    line cannot serve both. `store-listing.md` records that this is the one field Flathub does not
    share with another store.

    AppStream renders a summary as a sentence fragment beside the name and its validator objects to
    a trailing full stop, so one is trimmed here. That is a presentation rule of the format, not an
    edit to the copy.
    """
    body = one_section(listing.splitlines(), 3, r"^Flathub — Summary", "'Flathub — Summary'")
    out = {}
    for locale, text in _localized_blocks(body).items():
        collapsed = " ".join(text.split())
        out[locale] = collapsed.rstrip(".")
    if not out:
        raise DocumentShapeError("no localized summaries found under 'Flathub: Summary'")
    for locale, text in sorted(out.items()):
        if len(text) > SUMMARY_LIMIT:
            raise MetadataError(
                f"the {locale} Flathub summary is {len(text)} characters, over the "
                f"{SUMMARY_LIMIT} Flathub's quality guidelines allow: {text!r}"
            )
    return out


def descriptions(listing: str) -> dict[str, list[str]]:
    """The framing paragraphs of the shared body, per locale."""
    lines = listing.splitlines()
    out: dict[str, list[str]] = {}
    for title, body in sections(lines, 2, re.compile(r"^Shared description — ")):
        language = title.split("—", 1)[1].strip()
        locale = {name: code for code, name in LOCALE_NAMES.items()}.get(language)
        if locale is None:
            raise DocumentShapeError(
                f"'{title}' names a language that is not a catalog locale: {language!r}"
            )
        blocks = fenced_blocks(body)
        if len(blocks) != 1:
            raise DocumentShapeError(f"expected exactly one fenced body under '{title}', found {len(blocks)}")
        paragraphs = [" ".join(part.split()) for part in blocks[0].split("\n\n") if part.strip()]
        if len(paragraphs) < FRAMING_PARAGRAPHS:
            raise DocumentShapeError(
                f"'{title}' has {len(paragraphs)} paragraphs; the framing this file uses needs "
                f"{FRAMING_PARAGRAPHS}."
            )
        out[locale] = paragraphs[:FRAMING_PARAGRAPHS]
    if not out:
        raise DocumentShapeError("no 'Shared description: <Language>' sections found")
    return out


def releases(released_dir: Path) -> list[tuple[str, str]]:
    """`[(version, date)]`, newest first; read from each assembled note's own heading."""
    out = []
    for path in sorted(released_dir.glob("*.md")):
        first = path.read_text(encoding="utf-8").splitlines()[:1]
        matched = RELEASE_HEADING.match(first[0]) if first else None
        if matched is None:
            raise DocumentShapeError(
                f"docs/changelog/released/{path.name} does not open with `# X.Y.Z: YYYY-MM-DD`; "
                "the metainfo takes every release date from that line."
            )
        if matched.group("version") != path.stem:
            raise DocumentShapeError(
                f"docs/changelog/released/{path.name} is headed {matched.group('version')}: the "
                "filename and the heading must name the same release."
            )
        out.append((path.stem, matched.group("date")))
    if not out:
        raise DocumentShapeError("docs/changelog/released/ holds no assembled notes")
    return sorted(out, key=lambda item: tuple(int(n) for n in item[0].split(".")), reverse=True)


def _screenshots_element(manifest: dict | None) -> str:
    """The gallery, from the manifest `--screenshots` names.

    Each image is addressed by its own hash on the publisher's host, so a stale one cannot be
    served under a fresh name. A metainfo naming an unpublished URL renders as a broken gallery,
    so publishing the images precedes shipping the listing.

    No manifest means no gallery. That is the honest answer while the client is unshipped: an empty
    `<screenshots>` element is invalid AppStream, and inventing URLs for images nobody has uploaded
    would produce a listing that validates and then breaks in front of a user.
    """
    if not manifest:
        return (
            "  <!-- No gallery: the generator was given no screenshots manifest. Capture with\n"
            "       scripts/dev/showcase.sh linux, publish, then name the manifest. -->"
        )
    out = ["  <screenshots>"]
    for index, shot in enumerate(manifest["screenshots"]):
        kind = ' type="default"' if index == 0 else ""
        out.append(f"    <screenshot{kind}>")
        out.append(f"      <caption>{escape(shot['caption']['en'])}</caption>")
        out += [
            f'      <caption xml:lang="{locale}">{escape(text)}</caption>'
            for locale, text in sorted(shot["caption"].items())
            if locale != "en"
        ]
        out.append(
            f'      <image type="source" width="{shot["width"]}" height="{shot["height"]}">'
            f'{escape(shot["url"])}</image>'
        )
        out.append("    </screenshot>")
    out.append("  </screenshots>")
    return "\n".join(out)


def desktop_entry(name: str, summary_by_locale: dict[str, str], locales: list[str]) -> str:
    """The desktop entry. Its basename is the app id, which is what ties a window to this launcher.

    No `StartupWMClass`: GTK4 reports the GApplication id as the Wayland `app_id`, and the shell
    matches that against the desktop file's own basename. Setting it to anything is how that
    association gets broken, not kept.

    `%U` passes the raw URIs through GApplication's command-line signal: `%u` (one) would have
    done for a `mailto:` link, but an "Open With" on several files is one launch carrying all of
    them, and the plural form costs a mail link nothing.

    The `MimeType` set is what a desktop reads to offer this app at all. `x-scheme-handler/mailto`
    is the mail-link half; the file types are the share half, since Linux has no share portal and
    "Open With" is what a desktop offers instead (`docs/os-integration.md`). The shared core still
    decides what a URI or a file means.
    """
    lines = [
        "[Desktop Entry]",
        "Type=Application",
        f"Name={name}",
        f"Comment={summary_by_locale['en']}",
    ]
    lines += [
        f"Comment[{locale}]={summary_by_locale[locale]}"
        for locale in locales
        if locale != "en" and locale in summary_by_locale
    ]
    lines += [
        "Exec=mailcal %U",
        f"Icon={APP_ID}",
        "Terminal=false",
        # One MAIN category. `Network` and `Office` are both main ones, and an entry naming two is
        # filed under both, so the app appears twice in the application menu. `desktop-file-validate`
        # says so as a hint rather than an error, which is how it survived. `Email` and `Calendar`
        # are additional categories and `Office` satisfies each, so this says the same thing about
        # the app and places it once.
        "Categories=Office;Email;Calendar;",
        # Deliberately a short list of what people actually email, not everything openable: a
        # desktop reads this as "can open", so every entry is also this app offering itself as a
        # handler for that type. Kept to documents, pictures, archives and the three mail-native
        # types, and never widened without asking what a user picking us for that type expects.
        "MimeType="
        + "".join(
            f"{mime};"
            for mime in (
                "x-scheme-handler/mailto",
                "application/pdf",
                "image/png",
                "image/jpeg",
                "image/gif",
                "image/webp",
                "text/plain",
                "text/csv",
                "application/zip",
                "application/vnd.oasis.opendocument.text",
                "application/vnd.oasis.opendocument.spreadsheet",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "message/rfc822",
                "text/calendar",
                "text/vcard",
            )
        ),
        "Keywords=email;mail;calendar;imap;jmap;caldav;smtp;",
        "StartupNotify=true",
        "",
    ]
    return "\n".join(lines)


def _summary_elements(summary_by_locale: dict[str, str], locales: list[str]) -> str:
    out = [f"  <summary>{escape(summary_by_locale['en'])}</summary>"]
    out += [
        f'  <summary xml:lang="{locale}">{escape(summary_by_locale[locale])}</summary>'
        for locale in locales
        if locale != "en" and locale in summary_by_locale
    ]
    return "\n".join(out)


def _description_element(paragraphs: dict[str, list[str]], locales: list[str]) -> str:
    """The description block, each translated paragraph following its English original.

    That interleaving is the AppStream convention rather than a preference: a reader whose locale is
    missing a paragraph falls back to the untagged one directly above it.
    """
    out = ["  <description>"]
    for index in range(FRAMING_PARAGRAPHS):
        out.append(f"    <p>{escape(paragraphs['en'][index])}</p>")
        out += [
            f'    <p xml:lang="{locale}">{escape(paragraphs[locale][index])}</p>'
            for locale in locales
            if locale != "en" and locale in paragraphs
        ]
    out.append("  </description>")
    return "\n".join(out)


def _releases_element(history: list[tuple[str, str]], version: str) -> str:
    """Every assembled release, newest first.

    `/VERSION` must be among them: it means "the version users currently have"
    (`docs/changelog.md`), so a metainfo whose newest release is not it would advertise a build that
    was never released; the same invariant `check-version-sync.sh` enforces from the other side.
    """
    if version not in {entry[0] for entry in history}:
        raise MetadataError(
            f"/VERSION is {version}, which has no note under docs/changelog/released/. Cut the "
            "release with scripts/dev/release.py rather than editing /VERSION by hand."
        )
    out = ["  <releases>"]
    out += [f'    <release version="{ver}" date="{date}"/>' for ver, date in history]
    out.append("  </releases>")
    return "\n".join(out)


def metainfo(
    template: str, *, name: str, summary_by_locale, paragraphs, history, version, locales, gallery=None
) -> str:
    rendered = (
        template.replace("@APP_ID@", APP_ID)
        .replace("@NAME@", escape(name))
        .replace("@SUMMARIES@", _summary_elements(summary_by_locale, locales))
        .replace("@DESCRIPTION@", _description_element(paragraphs, locales))
        .replace("@SCREENSHOTS@", _screenshots_element(gallery))
        .replace("@RELEASES@", _releases_element(history, version))
    )
    left = re.findall(r"@[A-Z_]+@", rendered)
    if left:
        raise MetadataError(f"the metainfo template has placeholders nothing filled: {sorted(set(left))}")
    return rendered


def build(
    out_dir: Path, *, repo_root: Path = REPO_ROOT, screenshots: Path | None = None
) -> list[Path]:
    """Write both files into `out_dir`, and return what was written."""
    listing_file = brand.listing_source()
    listing = listing_file.read_text(encoding="utf-8")
    version = (repo_root / "VERSION").read_text(encoding="utf-8").strip()
    template_path = repo_root / "clients" / "linux" / "flatpak" / "metainfo.xml.in"
    locales = list(catalog_locales())

    name = brand.value("MAILCAL_APP_NAME")
    # Name the file a missing section is missing FROM. A listing resolves as a whole file, the
    # branded one over the neutral default (`docs/store-listing.md`), so a section the default
    # carries and a brand does not fails here with a shape error naming neither, on the one
    # machine that has a brand. Which reads as the tooling being broken rather than as the copy
    # being short a section.
    try:
        summary_by_locale = summaries(listing)
        paragraphs = descriptions(listing)
    except DocumentShapeError as missing:
        # `relative_to` only when it is: a caller may pass a `repo_root` the resolved brand
        # directory is not under, and a ValueError raised inside this handler would replace the
        # shape error with a path error, which is the opposite of the point.
        try:
            where = listing_file.relative_to(repo_root).as_posix()
        except ValueError:
            where = listing_file.as_posix()
        raise DocumentShapeError(f"{where}: {missing}") from missing
    history = releases(repo_root / "docs" / "changelog" / "released")
    # Named by the caller, and out of this tree by default. A gallery is a set of URLs to images
    # of a *branded* build on the publisher's own host, so a copy committed here would survive the
    # one deletion that is supposed to un-brand a fork, and point its listing at our screenshots.
    # A fork that wants one passes its own.
    gallery_path = screenshots or (repo_root / "clients" / "linux" / "flatpak" / "screenshots.json")
    gallery = json.loads(gallery_path.read_text(encoding="utf-8")) if gallery_path.is_file() else None

    # The body no longer uses the token (see `parse_keystore_tokens`), but an unsubstituted one
    # reaching a software centre would be a literal `{KEYSTORE}` on screen. Refuse rather than ship.
    for source in (summary_by_locale, {k: " ".join(v) for k, v in paragraphs.items()}):
        for locale, text in source.items():
            if KEYSTORE_TOKEN in text:
                raise MetadataError(
                    f"the {locale} copy contains {KEYSTORE_TOKEN}, which this generator does not "
                    "substitute. Either resolve it in the listing or teach this script the table."
                )

    out_dir.mkdir(parents=True, exist_ok=True)
    desktop_path = out_dir / f"{APP_ID}.desktop"
    metainfo_path = out_dir / f"{APP_ID}.metainfo.xml"
    desktop_path.write_text(desktop_entry(name, summary_by_locale, locales), encoding="utf-8")
    metainfo_path.write_text(
        metainfo(
            template_path.read_text(encoding="utf-8"),
            name=name,
            summary_by_locale=summary_by_locale,
            paragraphs=paragraphs,
            history=history,
            version=version,
            locales=locales,
            gallery=gallery,
        ),
        encoding="utf-8",
    )
    return [desktop_path, metainfo_path]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out-dir", required=True, type=Path, help="where the two files are written")
    parser.add_argument(
        "--screenshots",
        type=Path,
        help="the gallery manifest, which lives with whoever publishes the listing",
    )
    args = parser.parse_args(argv)
    try:
        written = build(args.out_dir, screenshots=args.screenshots)
    except (DocumentShapeError, MetadataError) as error:
        print(f"flatpak-metadata: {error}", file=sys.stderr)
        return 1
    for path in written:
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
