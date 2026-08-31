#!/usr/bin/env python3
"""Cut a release: assemble the pending changelog fragments, then move `/VERSION` to match.

    scripts/dev/release.py                 # version computed from the fragments' `Bump:` lines
    scripts/dev/release.py 0.4.0           # ...or state it, if you mean something else
    scripts/dev/release.py --dry-run       # print what would be written, change nothing
    scripts/dev/release.py --skip-docs     # ...on a host that cannot photograph the clients

`/VERSION` holds the **last released** version; the number users currently have; so a feature PR
never touches it, `Cargo.toml` or `clients/apple/project.yml`, and a version conflict between two
PRs in flight stops being possible. This script is the one thing that moves it.

What it does, in order:

0. Recaptures the documentation screenshots (`scripts/dev/docs_release.py --recapture`), unless
   `--skip-docs`. First, because it builds a client per platform and photographs it: it is the
   longest step here, the most likely to fail for reasons about the *host* rather than the release,
   and everything below it deletes files. `--skip-docs` exists for a machine that cannot capture,
   and says at the end what is then owed.
1. Refuses if `docs/changelog/unreleased/` is empty. A release with no user-facing change is a
   version number describing nothing.
2. Computes the next version: **minor if any fragment declares `Bump: minor`, else patch**. An
   explicit `X.Y.Z` overrides, and warns if it is a smaller step than the fragments asked for.
3. Measures each section against the tightest store it is pasted into, and **refuses before writing
   anything** if one is over. A section a cap cannot enumerate is written by hand in
   `docs/changelog/unreleased/_summary.md`, keyed by the section's platform tuple, and used in place
   of the assembled notes. The error names the file, the heading, and the changes to summarize.
4. Writes `docs/changelog/released/X.Y.Z.md`; one section per *distinct bullet set*, so a Mac-only
   fix never appears in the iPhone's note, plus an appendix carrying each fragment's engineering
   commentary (the fragments are about to be deleted; that rationale is not).
5. Deletes the consumed fragments and the summary, and adds the release to the index table in
   `docs/changelog.md`.
6. Runs `scripts/dev/bump-version.sh`, which writes `/VERSION` and its two committed mirrors and
   proves they agree.
7. Publishes the documentation (`docs_release.py --publish`): stamps `updated_for` onto every page,
   uploads the new blobs, and asks the live site whether they arrived. After the bump, because a
   page's `updated_for` may never exceed `/VERSION`.
8. Runs the store-copy check, which measures the assembled file itself; the same numbers step 3
   enforced, now read back off what was actually written; and the user-doc check in `--released`
   mode, which is the one that refuses a release whose pages still describe the previous one.

**It does not commit and does not tag**, the same deliberate rule as `bump-version.sh`: a `vX.Y.Z`
tag triggers `windows-release.yml`, so you tag when you mean to release. The assembled note is a
**draft**; every word of it is still yours to edit. Deciding what fourteen changes say inside Play's
500 characters is editorial and no cap can automate it; step 3 is only the machine refusing to
pretend the arithmetic works.
"""

from __future__ import annotations

import argparse
import datetime
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))

from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    APPENDIX_HEADING,
    DocumentShapeError,
    RELEASED_DIR,
    SUMMARY_PATH,
    UNRELEASED_DIR,
    catalog_languages,
    load_fragments,
    load_summaries,
    group_sections,
    next_version,
    section_notes,
    store_targets,
    version_tuple,
)
from check_store_copy_length import (  # noqa: E402
    LIMITS_PATH,
    binding_store,
    parse_limits,
    whats_new_caps,
)

import announcement  # noqa: E402  (a sibling in scripts/dev)
import docs_release  # noqa: E402  (a sibling in scripts/dev)

VERSION_PATH = REPO_ROOT / "VERSION"
ANNOUNCEMENTS_DIR = REPO_ROOT / "docs" / "changelog" / "announcements"
CHANGELOG_PATH = REPO_ROOT / "docs" / "changelog.md"
BUMP_SCRIPT = REPO_ROOT / "scripts" / "dev" / "bump-version.sh"
STORE_COPY_CHECK = REPO_ROOT / "scripts" / "ci" / "check_store_copy_length.py"
DOCS_RELEASE = REPO_ROOT / "scripts" / "dev" / "docs_release.py"
USER_DOCS_CHECK = REPO_ROOT / "scripts" / "ci" / "check_user_docs.py"

# Where the release index table starts in docs/changelog.md. New rows go directly under it, so the
# table stays newest-first without the script needing to sort what is already there.
INDEX_MARKER = "| Version | Date | What shipped |"

# How many headlines a release's index row names before it says "…". A backlog release can consume
# fourteen fragments, and a table cell holding fourteen headlines is a table nobody reads.
INDEX_HEADLINES = 3


class ReleaseError(Exception):
    """Something about the working tree makes this release impossible to cut."""


def assemble(version, date, fragments, languages=None, summaries=None):
    """The full text of `docs/changelog/released/<version>.md`."""
    # The catalog's order, not whatever order a fragment happened to write its blocks in; a
    # release assembled twice must produce the same bytes or the diff is noise.
    languages = catalog_languages() if languages is None else languages
    lines = ["# {} — {}".format(version, date), ""]

    for section in group_sections(fragments):
        lines.append("## {}".format(", ".join(section.platforms)))
        lines.append("")
        lines.append("Paste into: {}".format(section.paste_line))
        lines.append("")
        if summaries and section.platforms in summaries:
            # Say so in the file. A reader comparing this section against the appendix would
            # otherwise find changes with no matching sentence and read it as an omission.
            lines.append(
                "Written, not assembled: {} changes do not fit this store's cap. Every one of "
                "them is in the appendix below.".format(len(section.fragments))
            )
            lines.append("")
        for language in languages:
            lines.append("**{}**".format(language))
            lines.append("")
            lines.append("```")
            lines.append(section_notes(section, language, summaries))
            lines.append("```")
            lines.append("")

    lines.append("## {}".format(APPENDIX_HEADING))
    lines.append("")
    lines.append(
        "Engineering commentary, carried over from each fragment. Never pasted into a store."
    )
    lines.append("")
    for fragment in sorted(fragments, key=lambda item: item.sort_key):
        lines.append(
            "### {} — `{}` ({}, {})".format(
                fragment.headline.rstrip("."),
                fragment.slug,
                ", ".join(fragment.platforms),
                fragment.bump,
            )
        )
        lines.append("")
        if fragment.commentary:
            lines.append(fragment.commentary)
            lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def over_cap(fragments, languages, summaries):
    """Every `(section, language, store, cap, length)` a console would reject, worst first.

    Measured **before** anything is written, unlike the store-copy run at the end. Assembly deletes
    the fragments and moves `/VERSION`, so discovering the overflow afterwards leaves a half-cut
    release to unpick by hand; and the fragments carrying the text you need are the files it just
    removed.
    """
    caps = whats_new_caps(parse_limits(LIMITS_PATH.read_text(encoding="utf-8")))
    found = []
    for section in group_sections(fragments):
        binding = binding_store(caps, store_targets(section.platforms))
        if binding is None:
            # A Linux-only section reaches no console, so measuring it would invent a limit rather
            # than enforce one.
            continue
        store, cap = binding
        for language in languages:
            length = len(section_notes(section, language, summaries))
            if length > cap:
                found.append((section, language, store, cap, length))
    return sorted(found, key=lambda item: item[4] - item[3], reverse=True)


def report_over_cap(problems, summaries):
    """Print what will not fit, and the exact file and heading that fixes it."""
    print("Sections the console would reject:", file=sys.stderr)
    for section, language, store, cap, length in problems:
        print(
            "  {} / {} / {}: {} characters, limit {} (over by {})".format(
                ", ".join(section.platforms), language, store, length, cap, length - cap
            ),
            file=sys.stderr,
        )

    unwritten = []
    for section, _, _, _, _ in problems:
        if section.platforms not in summaries and section not in unwritten:
            unwritten.append(section)
    if not unwritten:
        print(
            "\nERROR: the authored summaries in {} are themselves over cap. Trim them. Every "
            "locale, not only the ones named above, or the stores tell different "
            "stories.".format(SUMMARY_PATH.relative_to(REPO_ROOT).as_posix()),
            file=sys.stderr,
        )
        return

    print(
        "\nERROR: this release enumerates more changes than these consoles accept. Write those "
        "sections instead of assembling them, in {}:".format(
            SUMMARY_PATH.relative_to(REPO_ROOT).as_posix()
        ),
        file=sys.stderr,
    )
    for section in unwritten:
        print("\n    ## {}".format(", ".join(section.platforms)), file=sys.stderr)
        print(
            "    ({} changes to summarize, one fenced block per catalog locale)".format(
                len(section.fragments)
            ),
            file=sys.stderr,
        )
        for fragment in section.fragments:
            print("      - {}".format(fragment.headline.rstrip(".")), file=sys.stderr)
    print("\nNothing was written. Re-run when the summary is in place.", file=sys.stderr)


def index_row(version, date, fragments):
    """The `docs/changelog.md` release-index row for this release."""
    headlines = [
        fragment.headline.rstrip(".")
        for fragment in sorted(fragments, key=lambda item: item.sort_key)
    ]
    shown = " · ".join(headlines[:INDEX_HEADLINES])
    if len(headlines) > INDEX_HEADLINES:
        shown += " · …{} more".format(len(headlines) - INDEX_HEADLINES)
    return "| [{v}](changelog/released/{v}.md) | {d} | {s} |".format(v=version, d=date, s=shown)


def insert_index_row(changelog, row):
    """Put `row` directly under the index table's header rule; the table is newest first."""
    lines = changelog.splitlines()
    for position, line in enumerate(lines):
        if line.strip() == INDEX_MARKER:
            # The header row is followed by the `|---|` rule; the newest release goes after it.
            lines.insert(position + 2, row)
            return "\n".join(lines) + "\n"
    raise ReleaseError(
        "docs/changelog.md has no release index table (looked for the header row "
        "'{}'). Restore it, or the release would be unlisted.".format(INDEX_MARKER)
    )


def resolve_version(current, fragments, requested):
    """The version this release lands on, and any warning worth printing about it."""
    declared = "minor" if any(f.bump == "minor" for f in fragments) else "patch"
    computed = next_version(current, declared)
    if requested is None:
        return computed, None
    try:
        version_tuple(requested)
    except ValueError:
        raise ReleaseError("'{}' is not a MAJOR.MINOR.PATCH version.".format(requested))
    if version_tuple(requested) <= version_tuple(current):
        raise ReleaseError(
            "{} is not above the last released version ({}). A store rejects a version that does "
            "not climb.".format(requested, current)
        )
    if version_tuple(requested) < version_tuple(computed):
        return requested, (
            "WARNING: {req} is a smaller step than the fragments declare: one of them says "
            "`Bump: minor`, which would make this {auto}.".format(req=requested, auto=computed)
        )
    return requested, None


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", nargs="?", help="X.Y.Z: computed from the fragments if omitted")
    parser.add_argument(
        "--dry-run", action="store_true", help="print what would be written and change nothing"
    )
    parser.add_argument(
        "--date", help="ISO release date (defaults to today): the note is dated when it is cut"
    )
    parser.add_argument(
        "--skip-docs",
        action="store_true",
        help="do not recapture or publish the documentation screenshots (a host that cannot "
        "photograph the clients); the pages then still describe the previous release",
    )
    args = parser.parse_args(argv)

    try:
        fragments = load_fragments()
        if not fragments:
            raise ReleaseError(
                "docs/changelog/unreleased/ is empty: nothing is pending, so there is no release "
                "to cut. Internal-only work does not mint a version."
            )
        languages = catalog_languages()
        summaries = load_summaries(languages=languages)
        current = VERSION_PATH.read_text(encoding="utf-8").strip()
        version, warning = resolve_version(current, fragments, args.version)
        # A page claiming a release above the one being cut describes a build nobody can install.
        # The gate refuses that too, against `/VERSION`; this asks it against the version in hand,
        # and asks it here, where refusing still costs nothing.
        ahead = docs_release.ahead_of(REPO_ROOT, version)
        if ahead:
            raise ReleaseError(
                "the documentation is ahead of this release:\n  %s\nDocumentation describes the "
                "app users are running, not the branch." % "\n  ".join(ahead)
            )
    except (ReleaseError, DocumentShapeError, docs_release.DocsReleaseError) as error:
        # A malformed `_summary.md` is an authoring mistake with a named fix, not a crash; it
        # reads the same way a malformed fragment does.
        print("ERROR: {}".format(error), file=sys.stderr)
        return 1

    problems = over_cap(fragments, languages, summaries)
    if problems:
        report_over_cap(problems, summaries)
        return 1

    date = args.date or datetime.date.today().isoformat()
    note_path = RELEASED_DIR / "{}.md".format(version)
    announcement_path = ANNOUNCEMENTS_DIR / "{}.md".format(version)
    if note_path.exists():
        print(
            "ERROR: {} already exists. {} has been released.".format(note_path, version),
            file=sys.stderr,
        )
        return 1

    text = assemble(version, date, fragments, languages, summaries)
    row = index_row(version, date, fragments)

    if warning:
        print(warning, file=sys.stderr)
    print("{} -> {} ({} fragment(s), {})".format(current, version, len(fragments), date))

    if args.dry_run:
        print("\n--- {} ---".format(note_path.relative_to(REPO_ROOT).as_posix()))
        print(text, end="")
        print("--- {} ---".format(announcement_path.relative_to(REPO_ROOT).as_posix()))
        print(announcement.build(version, date, fragments))
        print("--- docs/changelog.md index row ---")
        print(row)
        print("\n(dry run: nothing written)")
        return 0

    # Before any write: the screenshots. It builds and photographs a client per platform, so it is
    # the step most likely to fail for reasons about this machine rather than about the release,
    # and every line below deletes a fragment or moves a version. Same reasoning as `over_cap`.
    if not args.skip_docs:
        step = [sys.executable, str(DOCS_RELEASE), "--recapture"]
        print("\n==> {}".format(" ".join(step)))
        if subprocess.call(step, cwd=str(REPO_ROOT)):
            print(
                "\nERROR: the documentation screenshots could not be recaptured, so nothing was "
                "written and this release is not cut. Fix the capture host, or re-run with "
                "--skip-docs and refresh them before the build reaches users.",
                file=sys.stderr,
            )
            return 1

    RELEASED_DIR.mkdir(parents=True, exist_ok=True)
    note_path.write_text(text, encoding="utf-8")
    # Before the fragments are deleted: the released note keeps each change's headline and its
    # engineering commentary, but the user-facing note survives only inside the per-platform
    # sections; already merged into an authored summary wherever a store's cap could not hold the
    # list. So the announcement can be assembled now or never.
    ANNOUNCEMENTS_DIR.mkdir(parents=True, exist_ok=True)
    announcement_path.write_text(
        announcement.build(version, date, fragments), encoding="utf-8"
    )
    try:
        CHANGELOG_PATH.write_text(
            insert_index_row(CHANGELOG_PATH.read_text(encoding="utf-8"), row), encoding="utf-8"
        )
    except ReleaseError as error:
        note_path.unlink()
        print("ERROR: {}".format(error), file=sys.stderr)
        return 1
    for fragment in fragments:
        (UNRELEASED_DIR / "{}.md".format(fragment.slug)).unlink()
    if summaries:
        # Consumed with the fragments it stood in for: a summary describes *this* release's change
        # set, so leaving it behind would silently reappear in the next one under a heading that no
        # longer means the same thing.
        SUMMARY_PATH.unlink()
    print("wrote {}".format(note_path.relative_to(REPO_ROOT).as_posix()))
    print("wrote {}".format(announcement_path.relative_to(REPO_ROOT).as_posix()))
    print("consumed {} fragment(s) from docs/changelog/unreleased/".format(len(fragments)))
    if summaries:
        print(
            "consumed {} authored section(s) from {}".format(
                len(summaries), SUMMARY_PATH.name
            )
        )

    steps = [[str(BUMP_SCRIPT), version]]
    if not args.skip_docs:
        # After the bump, not before: a page's `updated_for` may never exceed `/VERSION`, so the
        # pages can only claim this release once that file names it.
        steps.append([sys.executable, str(DOCS_RELEASE), "--publish"])
    steps.append([sys.executable, str(STORE_COPY_CHECK)])
    # `--released` is the rule the gate does not carry: every page's `updated_for` must *equal*
    # `/VERSION`. With `--skip-docs` it is exactly what fails, which is the point; the release is
    # cut and the pages are stale, and this says so instead of the run ending in a success line.
    steps.append([sys.executable, str(USER_DOCS_CHECK), "--released"])

    failed = 0
    for step in steps:
        print("\n==> {}".format(" ".join(step)))
        failed |= subprocess.call(step, cwd=str(REPO_ROOT))

    if args.skip_docs:
        print(
            "\n--skip-docs: the documentation still shows {}. Before this build reaches users, on "
            "a host that can photograph the clients:\n"
            "    python3 scripts/dev/docs_release.py --apply".format(current),
            file=sys.stderr,
        )

    print(
        "\nBoth assembled files are DRAFTS. Edit {} until every section fits, and replace the "
        "announcement's lead paragraph in {}. Which two or three changes a release is about is a "
        "judgement no generator makes from a fragment list. Then:\n"
        "    git add -A && git commit -m 'Release {v}' && git tag v{v}".format(
            note_path.relative_to(REPO_ROOT).as_posix(),
            announcement_path.relative_to(REPO_ROOT).as_posix(),
            v=version,
        )
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
