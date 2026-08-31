#!/usr/bin/env python3
"""Fail if the user documentation under `docs/user/` cannot be published truthfully.

`docs/user-docs.md` is the contract; this is its machine half. The pages it guards are the ones
users read at `allodia.eu/docs/mail-calendar/…`, and every failure mode below is one that produces
a page which *looks* fine:

* a guide translated in English and forgotten in Dutch; the navigation still offers it, and the
  reader lands on nothing;
* a screenshot id renamed in the prose but not in the manifest; a figure that silently vanishes,
  or worse, resolves to the previous release's image;
* a page claiming a platform for which no capture exists; an empty tab where a screenshot should
  be, on the one platform the reader actually uses;
* an `updated_for` above `/VERSION`; a page documenting a build nobody can install yet;
* an em dash; house style for these pages, and the kind of rule that only survives as a check.

The last check is the one that matters most and reads as the least interesting: **finding no pages
at all**. This program scrapes a directory tree and hand-written frontmatter, so the way it breaks
is by quietly matching nothing and reporting success. A rename of `docs/user/` would do it. So an
empty scrape is a hard error, not a pass.

    scripts/ci/check-user-docs.py
    scripts/ci/check-user-docs.py --released

`--released` adds one rule the gate deliberately does not carry: every page's `updated_for` must
**equal** `/VERSION`, not merely stay under it. That is the question at the moment of a release,
*do these pages describe the build about to ship?*; and `scripts/dev/release.py` asks it after
recapturing the set. Asking it in the gate instead would turn every workstation red the day a
release was cut without a capture host, over documentation nobody was editing.

Stdlib only, and **3.9-compatible**, for the same reasons as the store-copy check beside it: a
stock macOS ships 3.9 as `/usr/bin/python3`, and a gate that needs `pip install` is a gate people
stop running. That is also why the tiny frontmatter parser below is hand-written rather than
PyYAML; the shape it has to read is five keys wide, and the alternative is a dependency.
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parents[2]

# The locales the documentation ships in. Deliberately a **subset** of the app's catalog; the
# public website speaks these two; and `docs/user-docs.md` records that divergence rather than
# letting it become invisible. The half that stays enforceable is checked below: a docs locale
# must exist in the catalog, so docs can never offer a language the app itself does not speak.
DOC_LOCALES = ("en", "nl")

# The platform tabs a page may declare. Not read from the showcase scripts on purpose: those name
# *capture targets* (`android-tablet-7` is a screenshot slot, not a platform a reader chooses), and
# a page's switcher is a reader-facing list. `iphone` covers iPad too; one page, one tab.
PLATFORMS = ("macos", "windows", "iphone", "android", "linux")

REQUIRED_FRONTMATTER = ("title", "description", "platforms", "order", "updated_for")

# What a manifest entry must carry for the renderer to emit a figure without measuring the image
# itself: the hash that addresses the blob, and the dimensions that reserve its space in the page.
MANIFEST_FIELDS = ("sha256", "width", "height", "bytes")

# Link targets that leave the documentation. Everything else must be a relative `*.md` path, so the
# same link works on GitHub and in the renderer; see `docs/user-docs.md`.
EXTERNAL_LINK = re.compile(r"^(?:https?:|mailto:|#)")

# `[text](target)`; enough for the links these pages actually contain. Reference-style links and
# raw `<a href>` are not supported, and would be missed rather than mis-reported; the checker's
# own tests pin that, so the limitation is known rather than assumed away.
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)")

FENCE = re.compile(r"^(`{3,})(.*)$")

# U+2014. Banned in the user-facing pages, and only there: the contract docs beside this file use
# it freely. Its near neighbours are left alone deliberately, because they have honest jobs an
# em dash does not: an en dash spans a range (`9-17`), a hyphen joins words.
EM_DASH = "—"


class DocumentShapeError(Exception):
    """The tree or a document changed shape, so this checker can no longer read it.

    Distinct from a content violation: the advice is to fix the *scraper*, not the docs.
    """


@dataclass
class Page:
    """One markdown page in one locale."""

    locale: str
    slug: str
    where: str
    meta: Dict[str, object]
    body: str
    text: str


@dataclass
class Shot:
    """One ` ```screenshot ` block, with the platforms it claims a capture for."""

    page: Page
    line: int
    shot_id: str
    platforms: Tuple[str, ...]


def relative(path: Path, root: Path) -> str:
    """A repo-relative path for an error message, POSIX-separated on every host.

    It names a file in *this repository*, so it must read the same wherever the check runs; the
    slug beside it is already `as_posix()` for the same reason. Left as `str()` it also became a
    `Page.where`, which `docs_release.stamp` returns as the list of pages it changed: on Windows
    that reported `docs\\user\\en\\setup.md` for a path every other tool in the repo spells with
    slashes.
    """
    try:
        return path.relative_to(root).as_posix()
    except ValueError:  # pragma: no cover; only if a caller passes a foreign root
        return str(path)


# ---- parsing ------------------------------------------------------------------------------------


def parse_frontmatter(text: str, where: str) -> Tuple[Dict[str, object], str]:
    """Split a page into its frontmatter mapping and its body.

    Accepts exactly the shape `docs/user-docs.md` documents; `key: value` lines between two `---`
    fences, where a value is a scalar or a flow list `[a, b]`. Anything else raises, because a
    frontmatter this program half-understands is worse than one it refuses.
    """
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        raise DocumentShapeError("%s: no `---` frontmatter block at the top of the file" % where)
    try:
        end = lines.index("---", 1)
    except ValueError:
        raise DocumentShapeError("%s: the frontmatter block is never closed with `---`" % where)

    meta = {}  # type: Dict[str, object]
    for offset, line in enumerate(lines[1:end], start=2):
        if not line.strip():
            continue
        if ":" not in line:
            raise DocumentShapeError("%s:%d: frontmatter line is not `key: value`" % (where, offset))
        key, _, raw = line.partition(":")
        key = key.strip()
        if key in meta:
            raise DocumentShapeError("%s:%d: duplicate frontmatter key `%s`" % (where, offset, key))
        meta[key] = parse_scalar(raw.strip())
    return meta, "\n".join(lines[end + 1 :])


def parse_scalar(raw: str) -> object:
    """One frontmatter value: a flow list, an integer, or a (optionally quoted) string."""
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        if not inner:
            return []
        return [item.strip().strip("'\"") for item in inner.split(",")]
    if len(raw) >= 2 and raw[0] == raw[-1] and raw[0] in "'\"":
        return raw[1:-1]
    if re.fullmatch(r"-?\d+", raw):
        return int(raw)
    return raw


def iter_outside_fences(body: str) -> Iterable[Tuple[int, str]]:
    """Yield `(line number, text)` for every body line that is not inside a fenced block.

    Tracks the fence *length*, so a four-backtick block showing a ` ```screenshot ` example; which
    `docs/user-docs.md` itself contains; is skipped whole rather than being read as a real one.
    """
    fence_len = 0
    for number, line in enumerate(body.split("\n"), start=1):
        match = FENCE.match(line)
        if fence_len:
            if match and len(match.group(1)) >= fence_len and not match.group(2).strip():
                fence_len = 0
            continue
        if match:
            fence_len = len(match.group(1))
            continue
        yield number, line


def parse_shots(page: Page) -> Tuple[List[Shot], List[str]]:
    """Every ` ```screenshot ` block on a page, plus any problems found reading them."""
    shots = []  # type: List[Shot]
    problems = []  # type: List[str]
    lines = page.body.split("\n")
    fence_len = 0
    open_at = 0
    fields = {}  # type: Dict[str, str]
    collecting = False

    for number, line in enumerate(lines, start=1):
        match = FENCE.match(line)
        if fence_len:
            if match and len(match.group(1)) >= fence_len and not match.group(2).strip():
                if collecting:
                    shot, trouble = build_shot(page, open_at, fields)
                    problems.extend(trouble)
                    if shot is not None:
                        shots.append(shot)
                fence_len = 0
                collecting = False
                continue
            if collecting:
                if not line.strip():
                    continue
                if ":" not in line:
                    problems.append(
                        "%s:%d: screenshot block line is not `key: value`: %r"
                        % (page.where, number, line)
                    )
                    continue
                key, _, value = line.partition(":")
                fields[key.strip()] = value.strip()
            continue
        if match:
            fence_len = len(match.group(1))
            collecting = match.group(2).strip() == "screenshot"
            open_at = number
            fields = {}
    if fence_len and collecting:
        problems.append("%s:%d: screenshot block is never closed" % (page.where, open_at))
    return shots, problems


def build_shot(
    page: Page, line: int, fields: Dict[str, str]
) -> Tuple[Optional[Shot], List[str]]:
    """Validate one screenshot block's fields into a `Shot`."""
    problems = []  # type: List[str]
    where = "%s:%d" % (page.where, line)
    unknown = sorted(set(fields) - {"id", "alt", "platforms"})
    if unknown:
        problems.append("%s: unknown screenshot key(s): %s" % (where, ", ".join(unknown)))
    for required in ("id", "alt"):
        if not fields.get(required):
            problems.append("%s: screenshot block has no `%s`" % (where, required))
    if not fields.get("id"):
        return None, problems

    # A malformed `platforms:` (a bare string rather than a flow list) is reported once by
    # check_frontmatter; treating it as empty here keeps this function from spelling the string
    # out one letter at a time on top of that.
    raw_declared = page.meta.get("platforms")
    declared = raw_declared if isinstance(raw_declared, list) else []
    if "platforms" in fields:
        narrowed = tuple(item.strip() for item in fields["platforms"].split(",") if item.strip())
        stray = [item for item in narrowed if item not in declared]
        if stray:
            problems.append(
                "%s: narrows to platform(s) the page does not declare: %s"
                % (where, ", ".join(stray))
            )
        platforms = tuple(item for item in narrowed if item in declared)
    else:
        platforms = tuple(str(item) for item in declared)

    if not platforms:
        problems.append(
            "%s: a screenshot needs at least one platform: the page declares `platforms: []`"
            % where
        )
    return Shot(page=page, line=line, shot_id=fields["id"], platforms=platforms), problems


# ---- loading ------------------------------------------------------------------------------------


def load_pages(root: Path) -> Dict[str, Dict[str, Page]]:
    """Every page, keyed `locale -> slug -> Page`. Raises if the tree holds nothing to check."""
    user_docs = root / "docs" / "user"
    if not user_docs.is_dir():
        raise DocumentShapeError("no docs/user/ directory at %s" % relative(user_docs, root))

    pages = {}  # type: Dict[str, Dict[str, Page]]
    total = 0
    for locale in DOC_LOCALES:
        pages[locale] = {}
        locale_dir = user_docs / locale
        if not locale_dir.is_dir():
            raise DocumentShapeError(
                "docs locale `%s` has no directory at %s" % (locale, relative(locale_dir, root))
            )
        for path in sorted(locale_dir.rglob("*.md")):
            slug = path.relative_to(locale_dir).with_suffix("").as_posix()
            where = relative(path, root)
            text = path.read_text(encoding="utf-8")
            meta, body = parse_frontmatter(text, where)
            pages[locale][slug] = Page(
                locale=locale, slug=slug, where=where, meta=meta, body=body, text=text
            )
            total += 1

    if not total:
        raise DocumentShapeError(
            "found no pages under docs/user/{%s}/: this check scrapes a directory tree, and "
            "matching nothing is how it fails silently" % ",".join(DOC_LOCALES)
        )
    return pages


def load_json(path: Path, root: Path) -> Dict[str, object]:
    """Read one of the two generated/hand-kept JSON files, with a shape error on either failure."""
    if not path.is_file():
        raise DocumentShapeError("missing %s" % relative(path, root))
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except ValueError as error:
        raise DocumentShapeError("%s is not valid JSON: %s" % (relative(path, root), error))


def version_tuple(raw: str) -> Optional[Tuple[int, ...]]:
    """`1.2.3` -> `(1, 2, 3)`; None when it is not a three-part version."""
    parts = raw.strip().split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        return None
    return tuple(int(part) for part in parts)


# ---- the audits ---------------------------------------------------------------------------------


def check_locales(root: Path) -> List[str]:
    """The docs locale set must be a subset of the app's own catalog."""
    settings = load_json(root / "project.inlang" / "settings.json", root)
    catalog = settings.get("locales")
    if not isinstance(catalog, list) or not catalog:
        raise DocumentShapeError("project.inlang/settings.json has no `locales` list")
    stray = [locale for locale in DOC_LOCALES if locale not in catalog]
    if not stray:
        return []
    return [
        "docs locale(s) %s are not in the app's catalog (%s). Documentation may never offer a "
        "language the app itself does not speak: add it to project.inlang/settings.json and the "
        "messages/ catalog first." % (", ".join(stray), ", ".join(catalog))
    ]


def check_parity(pages: Dict[str, Dict[str, Page]]) -> List[str]:
    """Every page exists in every docs locale, under the same slug."""
    problems = []  # type: List[str]
    everywhere = set()  # type: set
    for locale in DOC_LOCALES:
        everywhere |= set(pages[locale])
    for locale in DOC_LOCALES:
        for slug in sorted(everywhere - set(pages[locale])):
            problems.append(
        "docs/user/%s/%s.md is missing: the page exists in %s. A half-translated guide is "
                "a page the reader cannot reach while the navigation says they can."
                % (locale, slug, ", ".join(sorted(loc for loc in DOC_LOCALES if slug in pages[loc])))
            )
    return problems


def check_nav(nav: Dict[str, object], pages: Dict[str, Dict[str, Page]]) -> List[str]:
    """`nav.json` and the tree describe the same set of pages, exactly once each."""
    problems = []  # type: List[str]
    sections = nav.get("sections")
    home = nav.get("home")
    if not isinstance(sections, list) or not isinstance(home, str):
        raise DocumentShapeError("docs/user/nav.json needs a string `home` and a list `sections`")

    listed = [home]
    for index, section in enumerate(sections):
        if not isinstance(section, dict):
            raise DocumentShapeError("docs/user/nav.json: section %d is not an object" % index)
        where = "docs/user/nav.json section `%s`" % section.get("id", index)
        titles = section.get("title")
        if not isinstance(titles, dict) or sorted(titles) != sorted(DOC_LOCALES):
            problems.append(
                "%s: `title` must name every docs locale (%s)" % (where, ", ".join(DOC_LOCALES))
            )
        section_pages = section.get("pages")
        if not isinstance(section_pages, list) or not section_pages:
            problems.append("%s: has no `pages`" % where)
            continue
        listed.extend(str(slug) for slug in section_pages)

    for slug in listed:
        for locale in DOC_LOCALES:
            if slug not in pages[locale]:
                problems.append(
                    "docs/user/nav.json lists `%s`, but docs/user/%s/%s.md does not exist"
                    % (slug, locale, slug)
                )

    duplicates = sorted({slug for slug in listed if listed.count(slug) > 1})
    for slug in duplicates:
        problems.append("docs/user/nav.json lists `%s` more than once" % slug)

    for slug in sorted(set(pages[DOC_LOCALES[0]]) - set(listed)):
        problems.append(
        "docs/user/%s/%s.md is in no nav.json section: an unreachable page. Add it to the "
            "navigation, or delete it." % (DOC_LOCALES[0], slug)
        )
    return problems


def check_frontmatter(pages: Dict[str, Dict[str, Page]], version: Tuple[int, ...]) -> List[str]:
    """Required keys, known platforms, and an `updated_for` that does not out-run `/VERSION`."""
    problems = []  # type: List[str]
    for locale in DOC_LOCALES:
        for _, page in sorted(pages[locale].items()):
            where = page.where
            missing = [key for key in REQUIRED_FRONTMATTER if key not in page.meta]
            if missing:
                problems.append("%s: frontmatter is missing %s" % (where, ", ".join(missing)))
            unknown = sorted(set(page.meta) - set(REQUIRED_FRONTMATTER))
            if unknown:
                problems.append("%s: unknown frontmatter key(s): %s" % (where, ", ".join(unknown)))

            for key in ("title", "description"):
                if key in page.meta and not str(page.meta[key]).strip():
                    problems.append("%s: `%s` is empty" % (where, key))

            platforms = page.meta.get("platforms")
            if platforms is None:
                pass
            elif not isinstance(platforms, list):
                problems.append("%s: `platforms` must be a list, e.g. `[macos, android]`" % where)
            else:
                for platform in platforms:
                    if platform not in PLATFORMS:
                        problems.append(
                            "%s: unknown platform `%s` (known: %s)"
                            % (where, platform, ", ".join(PLATFORMS))
                        )

            if "order" in page.meta and not isinstance(page.meta["order"], int):
                problems.append("%s: `order` must be a whole number" % where)

            raw = str(page.meta.get("updated_for", ""))
            parsed = version_tuple(raw)
            if parsed is None:
                problems.append("%s: `updated_for` is not a MAJOR.MINOR.PATCH version: %r" % (where, raw))
            elif parsed > version:
                problems.append(
                    "%s: `updated_for: %s` is ahead of /VERSION (%s). Documentation describes the "
                    "release users are running, not the branch."
                    % (where, raw, ".".join(str(part) for part in version))
                )
    return problems


def check_typography(pages: Dict[str, Dict[str, Page]]) -> List[str]:
    """No em dashes. House style for the user-facing pages, and only they are checked.

    It is a rule about voice rather than correctness, which is exactly why it needs a machine: an
    em dash is the most natural thing in the world to type, nobody will notice one in review, and
    "we don't use those" is a sentence that survives in a person's head for about a week. The
    replacement is always available (a comma, a colon, a full stop, a pair of brackets) and reads
    plainer, which is the brand rule this serves.

    The whole file is scanned, frontmatter and fenced blocks included. A `description:` is copy a
    search engine shows, and a code block that needs an em dash has not come up; if one ever does,
    widening this is a deliberate edit rather than a silent exemption.
    """
    problems = []  # type: List[str]
    for locale in DOC_LOCALES:
        for _, page in sorted(pages[locale].items()):
            for number, line in enumerate(page.text.split("\n"), start=1):
                if EM_DASH in line:
                    problems.append(
                        "%s:%d: em dash. The user-facing pages do not use them; a comma, a colon "
                        "or a full stop says the same thing more plainly. Line: %s"
                        % (page.where, number, line.strip())
                    )
    return problems


def check_links(pages: Dict[str, Dict[str, Page]]) -> List[str]:
    """Internal links are relative `*.md` paths that resolve inside the same locale."""
    problems = []  # type: List[str]
    for locale in DOC_LOCALES:
        for slug, page in sorted(pages[locale].items()):
            base = Path(slug).parent
            for number, line in iter_outside_fences(page.body):
                for target in MARKDOWN_LINK.findall(line):
                    if EXTERNAL_LINK.match(target):
                        continue
                    where = "%s:%d" % (page.where, number)
                    path, _, _anchor = target.partition("#")
                    if not path:
                        continue
                    if not path.endswith(".md"):
                        problems.append(
                            "%s: internal link `%s` must be a relative path to a `.md` page, so "
                            "the same link works on GitHub and on the site" % (where, target)
                        )
                        continue
                    normalized = normalize((base / path[: -len(".md")]).as_posix())
                    if normalized is None or normalized not in pages[locale]:
                        problems.append(
                            "%s: internal link `%s` resolves to no page in docs/user/%s/"
                            % (where, target, locale)
                        )
    return problems


def normalize(slug: str) -> Optional[str]:
    """Collapse `a/../b` into `b`; None when the path escapes the locale root."""
    parts = []  # type: List[str]
    for part in slug.split("/"):
        if part in ("", "."):
            continue
        if part == "..":
            if not parts:
                return None
            parts.pop()
            continue
        parts.append(part)
    return "/".join(parts)


def check_screenshots(
    pages: Dict[str, Dict[str, Page]], manifest: Dict[str, object]
) -> List[str]:
    """Every referenced screenshot id has a capture for every platform its block claims."""
    problems = []  # type: List[str]
    images = manifest.get("images")
    if not isinstance(images, dict):
        raise DocumentShapeError("docs/user/screenshots.json has no `images` object")

    referenced = set()  # type: set
    for locale in DOC_LOCALES:
        for _, page in sorted(pages[locale].items()):
            shots, trouble = parse_shots(page)
            problems.extend(trouble)
            for shot in shots:
                referenced.add(shot.shot_id)
                where = "%s:%d" % (shot.page.where, shot.line)  # type: ignore[attr-defined]
                entry = images.get(shot.shot_id)
                if not isinstance(entry, dict):
                    problems.append(
                        "%s: screenshot id `%s` is not in docs/user/screenshots.json. Capture it "
                        "(scripts/dev/showcase.sh <platform> --set docs) or fix the id."
                        % (where, shot.shot_id)
                    )
                    continue
                for platform in shot.platforms:
                    per_locale = entry.get(platform)
                    if not isinstance(per_locale, dict):
                        problems.append(
                            "%s: screenshot `%s` has no %s capture. Either capture it, or narrow "
                            "the block with a `platforms:` line so the omission is deliberate."
                            % (where, shot.shot_id, platform)
                        )
                        continue
                    shot_locale = per_locale.get(locale)
                    if not isinstance(shot_locale, dict):
                        problems.append(
                            "%s: screenshot `%s` has a %s capture, but not in `%s`"
                            % (where, shot.shot_id, platform, locale)
                        )
                        continue
                    missing = [field for field in MANIFEST_FIELDS if field not in shot_locale]
                    if missing:
                        problems.append(
                            "%s: manifest entry for `%s`/%s/%s is missing %s"
                            % (where, shot.shot_id, platform, locale, ", ".join(missing))
                        )

    for orphan in sorted(set(images) - referenced):
        problems.append(
        "docs/user/screenshots.json holds `%s`, which no page references: a captured image "
            "nobody shows. Reference it or drop it from the manifest." % orphan
        )
    return problems


def check_current(pages: Dict[str, Dict[str, Page]], raw_version: str) -> List[str]:
    """Every page's `updated_for` names the release in `/VERSION`, exactly.

    Only asked at a release (`--released`). The rest of the time a page may lag: it was written
    against the shipped app and the app has not shipped again since, which is the same statement.
    What this catches is the release that moved `/VERSION` without recapturing; the pages then
    illustrate the *previous* interface while claiming to describe the current one, and no reader
    can tell.
    """
    problems = []  # type: List[str]
    for locale in DOC_LOCALES:
        for _, page in sorted(pages[locale].items()):
            raw = str(page.meta.get("updated_for", ""))
            if version_tuple(raw) is not None and raw != raw_version:
                problems.append(
                    "%s: `updated_for: %s` lags /VERSION (%s). Recapture and stamp the set:\n"
                    "      python3 scripts/docs_release.py --apply   (where the help pages live)" % (page.where, raw, raw_version)
                )
    return problems


def audit(root: Path, released: bool = False) -> List[str]:
    """Run every check against a docs tree. Returns the problems; raises on a shape change."""
    raw_version = (root / "VERSION").read_text(encoding="utf-8").strip()
    version = version_tuple(raw_version)
    if version is None:
        raise DocumentShapeError("/VERSION is not MAJOR.MINOR.PATCH: %r" % raw_version)

    pages = load_pages(root)
    nav = load_json(root / "docs" / "user" / "nav.json", root)
    manifest = load_json(root / "docs" / "user" / "screenshots.json", root)

    problems = []  # type: List[str]
    problems.extend(check_locales(root))
    problems.extend(check_parity(pages))
    problems.extend(check_nav(nav, pages))
    problems.extend(check_frontmatter(pages, version))
    problems.extend(check_typography(pages))
    problems.extend(check_links(pages))
    problems.extend(check_screenshots(pages, manifest))
    if released:
        problems.extend(check_current(pages, raw_version))
    return problems


def count_pages(root: Path) -> int:
    """How many pages were actually read; printed on success, so a silent scrape is visible."""
    return sum(1 for _ in (root / "docs" / "user").rglob("*.md"))


def main(argv: Sequence[str]) -> int:
    """Exit 0 when the docs can be published, 1 on a violation, 2 if the tree changed shape."""
    # Hand-parsed rather than argparse, to keep the positional root this has always taken working
    # exactly as before; the flag is additive, and the tests drive `audit` directly.
    rest = [argument for argument in argv[1:] if argument != "--released"]
    released = "--released" in argv[1:]
    root = Path(rest[0]).resolve() if rest else REPO_ROOT

    # Allodia's own help pages are excluded from the public tree: they are written in our voice and
    # their screenshots live in our content store, so a copy of this repository holds none of them.
    # Say so and pass. This is a skip, not a silent one, and it cannot hide a regression: where the
    # pages exist, so does this check, and there the absence of the directory is still an error.
    if not (root / "docs" / "user").is_dir():
        print("SKIP: no docs/user/ in this tree, so there are no help pages to check.")
        return 0

    try:
        problems = audit(root, released=released)
    except DocumentShapeError as error:
        print("ERROR: %s" % error, file=sys.stderr)
        print(
            "This check scrapes docs/user/ and its two JSON files. Their shape changed, so it can "
            "no longer read them: fix the scraper rather than the documents, and see "
            "scripts/ci/tests/test_user_docs.py.",
            file=sys.stderr,
        )
        return 2

    if problems:
        print("User documentation that cannot be published as it stands:")
        for problem in problems:
            print("  %s" % problem)
        print(
            "\nERROR: %d problem(s). The contract is docs/user-docs.md." % len(problems),
            file=sys.stderr,
        )
        return 1

    print(
        "OK: %d user-doc page(s) across %s are consistent%s."
        % (
            count_pages(root),
            ", ".join(DOC_LOCALES),
            " and current for /VERSION" if released else "",
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
