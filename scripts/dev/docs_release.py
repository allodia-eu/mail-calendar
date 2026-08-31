#!/usr/bin/env python3
"""Bring the documentation screenshots up to the release in `/VERSION`, and publish them.

The user guides are illustrated, and an illustration is the part of a document that goes stale
without anyone noticing: prose that no longer matches the app reads oddly, while a screenshot of
last year's interface reads like a screenshot. So the set is regenerated **at every release**
(`docs/user-docs.md`), and this is the command that does it.

    python3 scripts/dev/docs_release.py               # plan; what it would photograph and publish
    python3 scripts/dev/docs_release.py --recapture    # photograph every screen, re-encode, verify
    python3 scripts/dev/docs_release.py --publish      # stamp `updated_for`, upload, prove it
    python3 scripts/dev/docs_release.py --apply        # both, in order

`scripts/dev/release.py` runs the two phases separately, and the split is the whole point of having
them. **Recapture happens before the release is written**: it builds a client per platform and
photographs it, which is by far the longest and most failure-prone step in a release, and the writes
it precedes delete the changelog fragments the release is assembled from. **Publishing happens
after**, because a page's `updated_for` may never exceed `/VERSION`; so the pages can only claim
this release once `bump-version.sh` has moved it.

That ordering has one consequence worth stating: the captured build carries the *previous* version
string. No screen in the documentation set shows a version today (they are account setup and agent
access), and a screen that does may not join the set without moving recapture after the bump, at the
cost of the atomicity above.

What gets photographed is derived from the pages themselves; every platform any page offers a tab
for, in every docs locale. Nothing is configured here, so a guide that starts covering Windows
starts demanding a Windows capture pass, on a host that may not have one. That is the multi-host
gap in `docs/user-docs.md` arriving as an error rather than as silence.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Callable, Dict, List, Optional, Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))

import check_user_docs  # noqa: E402  (path set above so this runs as a script)
import docs_publish  # noqa: E402  (a sibling in scripts/dev)

SHOWCASE = REPO_ROOT / "scripts" / "dev" / "showcase.sh"
DOCS_IMAGES = REPO_ROOT / "scripts" / "dev" / "docs_images.py"
DOCS_PUBLISH = REPO_ROOT / "scripts" / "dev" / "docs_publish.py"
USER_DOCS_CHECK = REPO_ROOT / "scripts" / "ci" / "check_user_docs.py"
VERSION_PATH = REPO_ROOT / "VERSION"

# A page's platform tab -> the `showcase.sh` target that photographs it. The two vocabularies are
# deliberately separate (`docs/user-docs.md`): a page offers a reader `iphone`, while showcase.sh
# also knows capture slots like `android-tablet-7` that no reader ever picks.
#
# `linux` is **absent rather than mapped**, because there is no showcase mode on Linux at all. A
# Linux page would therefore raise here instead of quietly photographing nothing.
SHOWCASE_PLATFORM = {
    "macos": "macos",
    "windows": "windows",
    "iphone": "iphone",
    "android": "android",
}


class DocsReleaseError(Exception):
    """The documentation cannot be refreshed for this release, with a message meant for a human."""


def load_pages(root: Path) -> Dict[str, Dict[str, object]]:
    """The docs tree, with a scraper failure turned into this module's own error.

    One exception type crosses this boundary on purpose. `check_user_docs` raises a
    `DocumentShapeError`, and so does `changelog_fragments`; a different class of the same name,
    which `release.py` already catches. Letting both through would mean an `except` clause naming
    two identical words and catching only one of them, and the one it missed would surface as a
    traceback in the middle of cutting a release.
    """
    try:
        return check_user_docs.load_pages(root)
    except check_user_docs.DocumentShapeError as error:
        raise DocsReleaseError(
            "the documentation tree cannot be read, so a release cannot check it: %s" % error
        )


# ---- what the pages ask for ----------------------------------------------------------------------


def declared_platforms(pages: Dict[str, Dict[str, object]]) -> List[str]:
    """Every platform the pages offer a tab for, in the contract's own order.

    Read off the pages rather than off the manifest. The manifest describes what was captured last
    time, so deriving from it would make a newly documented platform invisible to exactly the step
    that exists to photograph it.
    """
    wanted = set()
    for locale in sorted(pages):
        for _, page in sorted(pages[locale].items()):
            declared = page.meta.get("platforms")  # type: ignore[attr-defined]
            if isinstance(declared, list):
                wanted.update(str(item) for item in declared)
    return [platform for platform in check_user_docs.PLATFORMS if platform in wanted]


def capture_commands(platforms: Sequence[str], locales: Sequence[str]) -> List[List[str]]:
    """One `showcase.sh` run per (platform, locale); the shape that script takes.

    Per locale rather than `--locale all`, because "all" means the app's seven catalog languages
    and the documentation ships two. Photographing the other five would cost four fifths of the
    time for images no page can reference.
    """
    commands = []  # type: List[List[str]]
    for platform in platforms:
        target = SHOWCASE_PLATFORM.get(platform)
        if target is None:
            raise DocsReleaseError(
                "a page declares `%s`, which has no showcase mode to photograph it. Either the "
                "platform gained a client and this table needs the entry, or the page is claiming "
                "a tab it cannot illustrate." % platform
            )
        for locale in locales:
            commands.append([str(SHOWCASE), target, "--set", "docs", "--locale", locale])
    return commands


def ahead_of(root: Path, version: str) -> List[str]:
    """Pages whose `updated_for` claims a release above the one being cut.

    The gate already refuses an `updated_for` above `/VERSION`, so this can only fire when the gate
    has not run. It is here because a release is the wrong moment to be assuming a check ran: a page
    describing a build nobody can install is the failure this whole field exists to prevent.
    """
    target = check_user_docs.version_tuple(version)
    if target is None:
        raise DocsReleaseError("%r is not a MAJOR.MINOR.PATCH version" % version)
    problems = []  # type: List[str]
    pages = load_pages(root)
    for locale in sorted(pages):
        for _, page in sorted(pages[locale].items()):
            raw = str(page.meta.get("updated_for", ""))  # type: ignore[attr-defined]
            parsed = check_user_docs.version_tuple(raw)
            if parsed is not None and parsed > target:
                problems.append(
                    "%s: `updated_for: %s` is above the release being cut (%s)"
                    % (page.where, raw, version)  # type: ignore[attr-defined]
                )
    return problems


# ---- stamping ------------------------------------------------------------------------------------


def stamp_text(text: str, version: str, where: str) -> Optional[str]:
    """One page with its `updated_for` set to `version`; `None` when it already says so.

    Rewrites the frontmatter line in place rather than the file, so a body that happens to contain
    the words `updated_for:`; a guide explaining this very field would; is left alone. Exactly one
    such line must exist: zero means the page changed shape, and two means the checker and the
    renderer are reading different values from the same file.
    """
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        raise DocsReleaseError("%s: no `---` frontmatter block to stamp" % where)
    try:
        end = lines.index("---", 1)
    except ValueError:
        raise DocsReleaseError("%s: the frontmatter block is never closed with `---`" % where)

    at = [
        index
        for index in range(1, end)
        if lines[index].partition(":")[0].strip() == "updated_for" and ":" in lines[index]
    ]
    if len(at) != 1:
        raise DocsReleaseError(
            "%s: expected exactly one `updated_for:` line in the frontmatter, found %d"
            % (where, len(at))
        )
    if lines[at[0]].partition(":")[2].strip() == version:
        return None
    lines[at[0]] = "updated_for: %s" % version
    return "\n".join(lines)


def stamp(root: Path, version: str) -> List[str]:
    """Set every page's `updated_for` to `version`. Returns the pages it changed.

    Called only after the captures have been retaken, and only from the phase that runs after the
    bump. Stamping a page whose screenshots were not retaken is precisely the lie the field exists
    to prevent, so the two are one step here rather than two things a person does in order.
    """
    changed = []  # type: List[str]
    pages = load_pages(root)
    for locale in sorted(pages):
        for _, page in sorted(pages[locale].items()):
            where = page.where  # type: ignore[attr-defined]
            updated = stamp_text(page.text, version, where)  # type: ignore[attr-defined]
            if updated is None:
                continue
            (root / where).write_text(updated, encoding="utf-8")
            changed.append(where)
    return changed


# ---- the phases ----------------------------------------------------------------------------------


def shown(command: Sequence[str]) -> str:
    """A command as a person would type it: repo-relative, and `python3` rather than its full path.

    The commands themselves stay absolute, because they are handed to a subprocess. This is only
    what gets printed; and a plan whose lines are too long to read is a plan nobody reads.

    Slash-separated on every host, because what is printed is a line a person may retype, and
    these are `bash` scripts run through Git Bash even on Windows; `scripts\\dev\\showcase.sh` is
    not a command anyone can paste.
    """
    parts = []  # type: List[str]
    for part in command:
        if part == sys.executable:
            parts.append("python3")
            continue
        try:
            parts.append(Path(part).relative_to(REPO_ROOT).as_posix())
        except ValueError:
            parts.append(part)
    return " ".join(parts)


def announce(command: Sequence[str], log: Callable[[str], None]) -> None:
    log("\n==> %s" % shown(command))


def recapture(
    root: Path, run: Callable[[Sequence[str]], int], log: Callable[[str], None] = print
) -> int:
    """Photograph every screen the pages need, re-encode, and prove the manifest still fits.

    Also resolves the upload token, which this phase does not use. That is deliberate: the phase
    that *does* need it runs after the release has been written, and discovering a missing
    credential there leaves a half-cut release behind. Cheap here, expensive later.
    """
    pages = load_pages(root)
    platforms = declared_platforms(pages)
    if not platforms:
        raise DocsReleaseError(
            "no page declares a platform, so there is nothing to photograph. A documentation tree "
            "with no illustrated page is either mid-edit or the scrape found the wrong directory."
        )
    if not docs_publish.resolve_settings().token:
        raise DocsReleaseError(
            "no docs upload token, so the images could be recaptured and never published. Put "
            "`%s=…` in %s (chmod 600) before cutting the release."
            % (docs_publish.TOKEN_ENV, docs_publish.ENV_FILE_LOCATIONS[1])
        )

    commands = capture_commands(platforms, check_user_docs.DOC_LOCALES)
    commands.append([sys.executable, str(DOCS_IMAGES)])
    # The manifest has just been rewritten from the new captures; this proves the *pages* still
    # resolve against it. A screen that stopped being reachable comes back as a capture missing
    # from the manifest, which is a check failing rather than a figure quietly disappearing.
    commands.append([sys.executable, str(USER_DOCS_CHECK)])

    log(
        "Recapturing the documentation set: %s x %s"
        % (", ".join(platforms), ", ".join(check_user_docs.DOC_LOCALES))
    )
    for command in commands:
        announce(command, log)
        code = run(command)
        if code:
            return code
    return 0


def publish(
    root: Path,
    version: str,
    run: Callable[[Sequence[str]], int],
    log: Callable[[str], None] = print,
) -> int:
    """Stamp `updated_for`, upload the blobs, and ask the live site whether they are really there."""
    changed = stamp(root, version)
    log(
        "Stamped `updated_for: %s` onto %d page(s)%s"
        % (version, len(changed), ":" if changed else " (all of them already said so)")
    )
    for where in changed:
        log("  %s" % where)

    for command in (
        [sys.executable, str(DOCS_PUBLISH), "--apply"],
        # Not a courtesy re-run. Uploading reports what this machine sent; only asking the site
        # reports what a reader will get, and a blob store on a volume that did not persist answers
        # the two questions differently.
        [sys.executable, str(DOCS_PUBLISH), "--check"],
        [sys.executable, str(USER_DOCS_CHECK), "--released"],
    ):
        announce(command, log)
        code = run(command)
        if code:
            return code
    return 0


def plan(root: Path, version: str, log: Callable[[str], None] = print) -> int:
    """Print what `--apply` would do, and touch nothing."""
    pages = load_pages(root)
    platforms = declared_platforms(pages)
    commands = capture_commands(platforms, check_user_docs.DOC_LOCALES)
    log("Would recapture for /VERSION %s:" % version)
    for command in commands:
        log("  %s" % shown(command))
    log("  %s" % shown([sys.executable, str(DOCS_IMAGES)]))

    stale = []  # type: List[str]
    for locale in sorted(pages):
        for _, page in sorted(pages[locale].items()):
            if str(page.meta.get("updated_for", "")) != version:  # type: ignore[attr-defined]
                stale.append(
                    "%s (%s)"
                    % (page.where, page.meta.get("updated_for"))  # type: ignore[attr-defined]
                )
    log(
        "\nWould stamp `updated_for: %s` onto %d page(s)%s"
        % (version, len(stale), ":" if stale else " (every page already says so)")
    )
    for where in stale:
        log("  %s" % where)
    log("\nWould then publish the blobs and verify them. Re-run with --apply.")
    return 0


# ---- CLI -------------------------------------------------------------------------------------------


def shell(command: Sequence[str]) -> int:
    return subprocess.call(list(command), cwd=str(REPO_ROOT))


def main(argv=None) -> int:
    # No `--root`: the functions below take one so the tests can drive a fixture tree, but the
    # commands they run are anchored at this checkout, so a flag offering another root would only be
    # half true. The tests call them directly instead.
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--recapture", action="store_true", help="photograph and re-encode, publishing nothing"
    )
    mode.add_argument(
        "--publish", action="store_true", help="stamp `updated_for`, upload the blobs, verify them"
    )
    mode.add_argument("--apply", action="store_true", help="both phases, in order")
    args = parser.parse_args(argv)

    version = VERSION_PATH.read_text(encoding="utf-8").strip()
    try:
        if args.recapture:
            return recapture(REPO_ROOT, shell)
        if args.publish:
            return publish(REPO_ROOT, version, shell)
        if args.apply:
            return recapture(REPO_ROOT, shell) or publish(REPO_ROOT, version, shell)
        return plan(REPO_ROOT, version)
    except DocsReleaseError as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
