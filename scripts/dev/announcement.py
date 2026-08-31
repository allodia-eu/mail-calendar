#!/usr/bin/env python3
"""Build the forum announcement for a release: one page covering every platform at once.

A store's release note only ever describes the platform of the store it sits in, and a reader has
to already own that app to find it. The announcement is the opposite surface; the whole release,
across every client, somewhere a person can be pointed at; so it is assembled from the same
fragments, at the same moment, rather than written again from memory later.

**It has to be built while the fragments still exist.** `docs/changelog/released/<X.Y.Z>.md` keeps
each change's headline and its engineering commentary, but not the user-facing note: that text is
distributed between the per-platform sections, already merged into an authored summary whenever a
store's cap could not hold the list. So `release.py` calls this before it deletes the fragments,
and the result is committed beside the release.

**What comes out is a draft**, the same rule as the assembled note. The lead paragraph in
particular is a placeholder: which two or three changes a release is *about* is an editorial
judgement no generator can make from a fragment list.
"""

from __future__ import annotations

import datetime
import re

import brand
from changelog_fragments import PLATFORM_ORDER, PLATFORM_STORES

# What a platform is called when a reader sees it, and the emoji its heading carries. `ios` is one
# App Store record covering both devices, so it is named for both.
PLATFORM_NAMES = {
    "macos": "macOS",
    "ios": "iPhone & iPad",
    "windows": "Windows",
    "android": "Android",
    "linux": "Linux",
}
PLATFORM_EMOJI = {
    "macos": "🖥️",
    "ios": "📱",
    "windows": "🪟",
    "android": "🤖",
    "linux": "🐧",
}
EVERYWHERE_EMOJI = "🌟"
NEW_EMOJI = "✨"
FIXED_EMOJI = "🔧"

# The announcement is written in English only: it is one forum post, not a per-locale store field.
LANGUAGE = "English"

# Spelled out rather than taken from `strftime("%B")`, which reads the process locale; so the same
# release cut on a Dutch-configured machine would be dated "augustus" in an English page. `%-d` is
# also unavailable on Windows, where this has to run too.
MONTHS = (
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
)  # fmt: skip


def reading_date(iso):
    """`2026-08-21` -> `21 August 2026`. A store field takes ISO; a person reads prose."""
    day = datetime.date.fromisoformat(iso)
    return f"{day.day} {MONTHS[day.month - 1]} {day.year}"


def shipping_platforms():
    """The platforms a user can actually install today; the ones that reach a store.

    Derived rather than listed, so the day Linux gets a store it moves out of "in development" on
    its own. A hand-kept list here would instead keep saying "not yet released" about a shipped
    client, in the one document written to be read by people who do not follow the repo.
    """
    return tuple(p for p in PLATFORM_ORDER if PLATFORM_STORES[p])


def _bullet(fragment):
    note = re.sub(r"\s+", " ", fragment.notes[LANGUAGE]).strip()
    return f"- **{fragment.headline.rstrip('.')}.** {note}"


def _group(fragments, heading):
    """One section: its heading, then New before Fixed, each alphabetical by headline."""
    if not fragments:
        return []
    lines = [heading, ""]
    new = [f for f in fragments if f.bump == "minor"]
    fixed = [f for f in fragments if f.bump != "minor"]
    for emoji, label, group in ((NEW_EMOJI, "New", new), (FIXED_EMOJI, "Fixed", fixed)):
        if not group:
            continue
        # Only label the halves when both are present: a section that is all fixes reads better
        # without a heading announcing the absence of the other kind.
        if new and fixed:
            lines += [f"### {emoji} {label}", ""]
        lines += [_bullet(f) for f in sorted(group, key=lambda item: item.headline)]
        lines.append("")
    return lines


def build(version, date, fragments):
    """The full text of `docs/changelog/announcements/<version>.md`. `date` is ISO, as cut."""
    shipping = set(shipping_platforms())
    everywhere = [f for f in fragments if shipping <= set(f.platforms)]
    rest = [f for f in fragments if not shipping <= set(f.platforms)]

    shipped_names = [PLATFORM_NAMES[p] for p in PLATFORM_ORDER if p in shipping]
    joined = ", ".join(shipped_names[:-1]) + f" and {shipped_names[-1]}"

    lines = [
        f"# {brand.value('MAILCAL_APP_NAME')} {version}",
        "",
        f"Released {reading_date(date)}.",
        "",
        f"This release carries {len(fragments)} "
        f"{'change' if len(fragments) == 1 else 'changes'}. "
        "REPLACE THIS PARAGRAPH: name the two or three changes the release is actually about, "
        "in a reader's terms.",
        "",
        "Changes are grouped by where they landed. The first section reached **every app**. The "
        "sections after it cover changes that did not reach all of them, so a change that landed "
        "on two apps is listed under both.",
        "",
        "---",
        "",
    ]
    lines += _group(everywhere, f"## {EVERYWHERE_EMOJI} Every app: {joined}")

    for platform in PLATFORM_ORDER:
        if platform not in shipping:
            continue
        reaching = [f for f in rest if platform in f.platforms]
        if not reaching:
            continue
        lines += ["---", ""]
        lines += _group(reaching, f"## {PLATFORM_EMOJI[platform]} {PLATFORM_NAMES[platform]}")

    # A platform with no store cannot be installed, so its changes are reported as such rather than
    # announced beside ones a reader can go and get. Saying nothing at all would be worse: the work
    # happened, and a reader comparing this against the repo would find it missing.
    for platform in PLATFORM_ORDER:
        if platform in shipping:
            continue
        reaching = [f for f in rest if platform in f.platforms]
        if not reaching:
            continue
        lines += ["---", ""]
        lines += _group(
            reaching,
            f"## {PLATFORM_EMOJI[platform]} {PLATFORM_NAMES[platform]} "
            "(not yet in a store)",
        )
        lines += [
            f"The {PLATFORM_NAMES[platform]} app is not in a store yet, so there is nowhere to "
            "update from. The above is what landed this cycle.",
            "",
        ]

    return "\n".join(lines)


__all__ = ["build", "reading_date", "shipping_platforms", "PLATFORM_NAMES"]
