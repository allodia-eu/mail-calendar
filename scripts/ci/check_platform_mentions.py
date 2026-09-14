#!/usr/bin/env python3
"""Fail if a release note names a platform across the Apple boundary it is pasted over.

0.9.0's macOS "What's new" said, in all seven languages, that search "is now centred at the top of
the window on Mac, Windows and Linux". Apple has historically rejected an update whose copy names a
competing platform, and that is the text that went into App Store Connect. It was caught by hand
minutes before submission, which is not a gate: the iOS section of the same release named no other
platform, so nothing about the mistake was systematic and nothing would have caught the next one.

The copy is written by people and stays that way. What is added here is the gate, and it is narrow
on purpose:

  * a note reaching macOS or iOS may not name Windows, Linux or Android;
  * a note reaching Windows, Android or Linux may not name a piece of Apple hardware or an Apple
    operating system, because the reverse leak is the same mistake and no console enjoys it.

Nothing else is a leak. Windows naming Linux is not a rejection risk anywhere, and a note is always
free to name its own side: "On Windows, the message list now works with a screen reader" is exactly
what a Microsoft Store note should say, and a macOS note may talk about iPhone.

Both a **pending fragment** and an **assembled release note** are read, because they fail
differently. A fragment carries one body for every platform it lists, so a sentence naming a
sibling platform is a leak the moment it is written, and that is where the author is: the 0.9.0
text was a fragment reaching macos, ios, windows and linux at once, so it could name no platform at
all and named three. An assembled section is read as well because a release may hand-write one
(docs/changelog.md -> "When a store's cap cannot hold the release"), and hand-written prose belongs
to no fragment.

Why here rather than in the publisher: this repository is where the copy is written, and a gate in
the release tooling reports the problem after the build, with a submission waiting on it. The push
asserts it too, the way it already refuses over-long copy before anything reaches the network.

It stays **3.9-compatible** for the same reason as the length check beside it: `/usr/bin/python3`
on a stock macOS is 3.9, and a gate that crashes on the host it is meant to protect is not one.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

# One reader per document: the fragment format and the platform vocabulary are the same ones the
# length check and the store publishers use, so a note can never be split into sections by one
# parser and checked by another.
from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    DocumentShapeError,
    FragmentError,
    load_fragments,
    load_releases,
)

# The two sides of the boundary. `ios` covers iPadOS (one App Store record), and both Apple
# platforms are one side because a macOS note naming an iPhone is a copy judgement, not a rejection
# risk: Apple sells both.
APPLE_PLATFORMS = frozenset({"macos", "ios"})

# The names to refuse, per side. They are the brand names themselves, which is what makes one list
# serve all seven languages: a translation renders "the window" but never "Windows", so the Dutch,
# German and Portuguese notes all carried the same three words the English one did.
#
# Matched with word boundaries and **case-sensitively**, which is what keeps the common nouns out:
# a note is free to say two windows sit side by side. "Mac" and "iPadOS" are listed beside "macOS"
# and "iPad" because a boundary match on the longer name never covers the shorter one, and the
# reverse leak this has actually produced was spelled "on Mac and Windows".
APPLE_NAMES = ("macOS", "Mac", "iPadOS", "iPad", "iPhone", "iOS")
OTHER_NAMES = ("Windows", "Linux", "Android")


def _pattern(names) -> "re.Pattern[str]":
    return re.compile(r"\b(" + "|".join(re.escape(name) for name in names) + r")\b")


_APPLE = _pattern(APPLE_NAMES)
_OTHER = _pattern(OTHER_NAMES)


@dataclass(frozen=True)
class Leak:
    """One note naming platforms from the other side of the boundary it is pasted over."""

    where: str
    named: tuple
    allowed: str

    def __str__(self) -> str:
        return f"  {self.where}: names {', '.join(self.named)}, and is pasted into {self.allowed}"


def leaked_names(platforms, note: str) -> tuple:
    """Every forbidden name `note` carries, given the platforms it is pasted into.

    A note reaching both sides at once may name neither, which is not a trap: it is the same body
    going into App Store Connect and the Microsoft Store, and there is no wording that is safe in
    one and honest in the other. Split the fragment by platform, or leave the platform out.
    """
    apple = bool(APPLE_PLATFORMS.intersection(platforms))
    other = bool(set(platforms) - APPLE_PLATFORMS)
    found = []
    if apple:
        found += _OTHER.findall(note)
    if other:
        found += _APPLE.findall(note)
    # Sorted and de-duplicated: a name repeated in one note is one mistake, and a stable order
    # keeps the report diffable across runs.
    return tuple(sorted(set(found)))


def _side(platforms) -> str:
    """How the report names what a note is pasted into, for the advice to be actionable."""
    apple = bool(APPLE_PLATFORMS.intersection(platforms))
    other = bool(set(platforms) - APPLE_PLATFORMS)
    if apple and other:
        return "Apple's stores and others at once, so it may name no platform"
    return "an Apple store" if apple else "a non-Apple store"


def audit_fragments(fragments=None) -> list:
    """Every pending fragment, against the platforms its `Platforms:` line reaches."""
    fragments = load_fragments() if fragments is None else fragments
    leaks = []
    for fragment in fragments:
        for language, note in fragment.notes.items():
            named = leaked_names(fragment.platforms, note)
            if named:
                leaks.append(
                    Leak(f"Fragment {fragment.slug} / {language}", named, _side(fragment.platforms))
                )
    return leaks


def audit_releases(releases=None) -> list:
    """Every assembled release note, section by section.

    History is read too, not only what is pending. A shipped note cannot be un-pasted, but it is
    the copy the next release is written next to, and leaving a known leak in the tree makes it
    precedent rather than a mistake.
    """
    releases = load_releases() if releases is None else releases
    leaks = []
    for version, sections in releases:
        for section in sections:
            for language, note in section.notes.items():
                named = leaked_names(section.platforms, note)
                if named:
                    leaks.append(
                        Leak(
                            f"Release note {version} / {','.join(section.platforms)} / {language}",
                            named,
                            _side(section.platforms),
                        )
                    )
    return leaks


def main() -> int:
    """Read every note and report. Exit code 1 on a leak, 2 if a document changed shape."""
    try:
        fragments = load_fragments()
        releases = load_releases()
        leaks = audit_fragments(fragments) + audit_releases(releases)
    except FragmentError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("See docs/changelog.md for the fragment format.", file=sys.stderr)
        return 1
    except DocumentShapeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print(
            "This check reads docs/changelog/. Its shape changed, so it can no longer split the "
            "notes it is meant to read: fix the scraper rather than the document, and see "
            "scripts/ci/tests/test_platform_mentions.py.",
            file=sys.stderr,
        )
        return 2

    if leaks:
        print("Release copy naming a platform across the store boundary it is pasted over:")
        for leak in sorted(leaks, key=lambda item: item.where):
            print(leak)
        print(
            f"\nERROR: {len(leaks)} note(s) name a platform their store does not sell. Apple has "
            "historically rejected an update whose copy names a competing platform. Scope the "
            "sentence to the platforms the note reaches, drop the platform name, or split the "
            "fragment so each side gets its own body (docs/changelog.md -> 'The rule', 4).",
            file=sys.stderr,
        )
        return 1

    # Counted, and the count printed, because the way this check fails is by reading nothing: a
    # reworded heading would leave it splitting a release into no sections at all and reporting
    # success over an empty list.
    measured = sum(len(f.notes) for f in fragments)
    measured += sum(len(s.notes) for _, sections in releases for s in sections)
    print(f"OK: all {measured} release note(s) name only platforms their own store sells.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
