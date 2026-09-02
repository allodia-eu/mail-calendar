#!/usr/bin/env python3
"""Fail if any store-listing or release-note field exceeds the limit its console enforces.

`docs/store-listing.md` has always carried a "Field limits" table, and `docs/changelog.md` has
always said "fit the 500-character Play ceiling"; and nothing measured either. Six of the seven
shared description bodies were over Apple's and Play's 4,000-character cap before anyone counted,
five of them for several releases, because a translation runs 15-35% longer than its English source
and "the English fits" was read as evidence the others did.

Release notes now live in `docs/changelog/`; one fragment per pending change, one assembled file
per release ([`changelog_fragments`](changelog_fragments.py) owns that format). That sharpens what
is measured rather than loosening it: a fragment's cap is the **tightest store across the platforms
it ships to**, so an Android fix is held to Play's 500 while a Mac-only one gets Apple's 4,000
instead of being trimmed to a limit no console was ever going to apply to it.

That is the failure AGENTS.md's "a store/release upload is a remote gate too" rule is about: the
console rejects the paste at submission, after a build number has been burned, and the only local
signal was a number stated in a table. This makes the table load-bearing; it is **parsed**, not
mirrored, so editing a limit here changes what is enforced and the two cannot drift.

Why Python rather than a `*.sh` beside the other `scripts/ci` checks: the job is counting Unicode
characters in seven languages and substituting a per-store token before counting. `wc -m` is
locale-dependent and the token substitution would be an awk program nobody wants to read. Python
counts code points natively, and the parser has unit tests (`scripts/ci/tests/`); a checker whose
own document-scraping can silently find nothing is not a checker. It stays **3.9-compatible** for
the same reason: `/usr/bin/python3` on a stock macOS is 3.9, and a gate that crashes on the host it
is written for is a gate nobody runs.

    scripts/ci/check_store_copy_length.py

**What "a character" means here.** Python's `len()` counts code points. For this copy that equals
what a console counts: every character in it is BMP and precomposed, so code points, UTF-16 code
units and grapheme clusters all agree. An emoji or a combining accent would break that equivalence
(astral characters are 2 UTF-16 units), so keep them out of store copy; which the brand voice
already does.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dev"))

# The brand overlay, for the product name and for which listing file this build reads. It lives in
# `scripts/dev` because that is where the packaging front doors are; importing it here rather than
# re-parsing `branding/*.env` is the same rule the fragment parser follows below; one reader per
# document.
import brand  # noqa: E402  (path set above so this runs as a script)

# The fragment format, the platform -> store map, and the `**Label**` + fenced-block primitive both
# documents use. Imported rather than restated so a fragment and a listing field can never be read
# by two subtly different parsers.
from changelog_fragments import (  # noqa: E402  (path set above so this runs as a script)
    DocumentShapeError,
    FragmentError,
    STORES,
    labelled_blocks as _labelled_blocks,
    load_fragments,
    load_releases,
    store_targets,
)

REPO_ROOT = Path(__file__).resolve().parents[2]

# The contract: the stores' own published caps, and the rules the copy obeys. Always present, and
# the same file in every checkout; a contributor writing a changelog fragment is measured against
# it, so it cannot be the one that moves with a brand.
LIMITS_PATH = REPO_ROOT / "docs" / "store-listing.md"


def listing_path() -> Path:
    """The store copy this build describes itself with; `branding/<brand>-listing.md`.

    Copy resolves as a whole file rather than as injected values (docs/branding.md), so this is a
    function and not a constant: the answer depends on which brand files are present and on
    `MAILCAL_LISTING_SOURCE`, and a constant would freeze whichever was true at import.
    """
    return brand.listing_source()

# The one substitution the shared description body carries (store-listing.md → "{KEYSTORE} token").
KEYSTORE_TOKEN = "{KEYSTORE}"


@dataclass(frozen=True)
class Measurement:
    """One field, measured against the limit of the console it is pasted into.

    Every field is recorded, not only the ones that fail, so a green run can still say how much
    room is left. That matters more here than it looks: most of the short fields sit within a
    couple of characters of their cap, so "it passed" and "you may add a word" are different
    facts.
    """

    where: str
    limit: int
    actual: int
    unit: str = "characters"

    @property
    def margin(self) -> int:
        return self.limit - self.actual

    @property
    def fits(self) -> bool:
        return self.actual <= self.limit

    def __str__(self) -> str:
        state = f"over by {-self.margin}" if not self.fits else f"{self.margin} to spare"
        return f"  {self.where}: {self.actual} {self.unit}, limit {self.limit} ({state})"


@dataclass(frozen=True)
class StoreLimits:
    """One store's row of the "Field limits" table. `None` means the store has no such field."""

    store: str
    name: int | None
    subtitle: int | None
    short: int | None
    description: int | None
    feature_count: int | None
    feature_chars: int | None
    search_count: int | None
    search_chars: int | None
    search_words: int | None
    keywords: int | None
    whats_new: int | None


# ---------------------------------------------------------------------------------------------
# Markdown scraping
# ---------------------------------------------------------------------------------------------

_HEADING = re.compile(r"^(#{1,6})\s+(.*)$")
_NUMBER = re.compile(r"(\d[\d,]*)")


def sections(lines: list[str], level: int, pattern: re.Pattern[str]):
    """Yield `(title, body_lines)` for every heading at exactly `level` whose text matches.

    A section ends at the next heading of the same or a higher level, so a `##` section carries its
    `###` children with it.

    Public, along with `fenced_blocks` and `one_section`, because every store publisher reads the
    same document to *push* the copy that this check *measures*; including the Microsoft Store's,
    which is not built here and imports this module across two checkouts. Two scrapers would be two
    readings of one file, and the one that pastes into a store console would be the one nobody
    tested; so there is one, here, next to its tests.
    """
    prefix_len = level
    index = 0
    while index < len(lines):
        heading = _HEADING.match(lines[index])
        if heading and len(heading.group(1)) == prefix_len and pattern.search(heading.group(2)):
            title = heading.group(2).strip()
            body: list[str] = []
            index += 1
            while index < len(lines):
                nested = _HEADING.match(lines[index])
                if nested and len(nested.group(1)) <= level:
                    break
                body.append(lines[index])
                index += 1
            yield title, body
            continue
        index += 1


def fenced_blocks(body: list[str]) -> list[str]:
    """Every fenced code block in `body`, in order, without its fences."""
    blocks: list[str] = []
    current: list[str] = []
    inside = False
    for line in body:
        if line.startswith("```"):
            if inside:
                blocks.append("\n".join(current))
                current = []
            inside = not inside
            continue
        if inside:
            current.append(line)
    return blocks


def one_section(lines: list[str], level: int, pattern: str, what: str) -> list[str]:
    """The body of the single section matching `pattern`; a structural error if it is not there."""
    found = list(sections(lines, level, re.compile(pattern)))
    if len(found) != 1:
        raise DocumentShapeError(f"expected exactly one {what} section, found {len(found)}")
    return found[0][1]


def _cell_number(cell: str) -> int | None:
    """The first integer in a table cell, or `None` for an em dash / no number."""
    found = _NUMBER.search(cell)
    return int(found.group(1).replace(",", "")) if found else None


def parse_limits(listing: str) -> dict[str, StoreLimits]:
    """Read the "Field limits" table; the source of truth for every number this check enforces.

    Scoped to that one section rather than scanned document-wide: the store names head the rows of
    four other tables here (age rating, Play Data Safety, per-store status), and a document-wide
    sweep reads whichever came last.
    """
    limits: dict[str, StoreLimits] = {}
    for line in one_section(listing.splitlines(), 2, r"^Field limits", "'Field limits'"):
        if not line.startswith("| "):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if not cells or cells[0] not in STORES:
            continue
        if len(cells) != 8:
            raise DocumentShapeError(
                f"the 'Field limits' row for {cells[0]} has {len(cells)} columns, expected 8"
            )
        store, name, subtitle, short, description, features, search, whats_new = cells
        # Two cells are not plain numbers, and both are matched by shape rather than by "the first
        # number in the cell"; which would read Apple's keyword limit as a per-feature size.
        #   features: "up to 20 x ~200"                 -> a count and a per-item size
        #   search:   "up to 7 x 30 (21 words)"         -> a count, a per-term size, a word budget
        #             "Keywords: 100"                   -> Apple's single field, in the same column
        feature_pair = re.search(r"up to\s+(\d+)\s*\S\s*~?(\d[\d,]*)", features)
        search_pair = re.search(r"up to\s+(\d+)\s*\S\s*~?(\d[\d,]*)", search)
        search_words = re.search(r"\((\d+)\s+words?\)", search)
        keywords = re.search(r"Keywords:\s*(\d[\d,]*)", search)
        limits[store] = StoreLimits(
            store=store,
            name=_cell_number(name),
            subtitle=_cell_number(subtitle),
            short=_cell_number(short),
            description=_cell_number(description),
            feature_count=int(feature_pair.group(1)) if feature_pair else None,
            feature_chars=int(feature_pair.group(2).replace(",", "")) if feature_pair else None,
            search_count=int(search_pair.group(1)) if search_pair else None,
            search_chars=int(search_pair.group(2).replace(",", "")) if search_pair else None,
            search_words=int(search_words.group(1)) if search_words else None,
            keywords=int(keywords.group(1).replace(",", "")) if keywords else None,
            whats_new=_cell_number(whats_new),
        )
    missing = [store for store in STORES if store not in limits]
    if missing:
        raise DocumentShapeError(f"the 'Field limits' table has no row for: {', '.join(missing)}")
    return limits


def search_term_words(terms) -> list:
    """Every word a language's search terms spend, as the Store counts them.

    **Two rules here were wrong until they were measured against the live ingestion API**
    (2026-08-03, probing a throwaway draft; the `KeywordsTotalCount` error names the budget):

    - It is a **total**, not a count of distinct words. Seven terms of `"aa bb cc dd ee"` spend
      **35**, not 5, and are rejected. This function therefore returns a list with repeats, and
      the caller measures its length.
    - **Hyphens split.** `privacy-focused` is **two** words, not one. Our English list read as 20
      words by whitespace and was refused at **22**; the two hyphenated terms each cost double.

    Both readings had been asserted the other way, with a docstring claiming "which is how the
    Store counts". Neither had ever been checked against a store; the copy they passed was
    rejected on the first push that reached one.

    Case-folded and stripped of surrounding punctuation; that part is cosmetic now that nothing
    is de-duplicated, but it keeps the reported word list readable when the budget is blown.

    Lives here, next to the check that enforces the budget, and is imported by the Microsoft Store
    push, which is not built here: a push that counted words differently from the gate would pass
    CI and be rejected by the console, which is the exact failure this file exists to prevent.
    """
    words = []
    for term in terms:
        for word in re.split(r"[\s\-‐‑]+", term):
            cleaned = word.strip(".,;:!?()[]\"'").casefold()
            if cleaned:
                words.append(cleaned)
    return words


def parse_keystore_tokens(listing: str) -> dict[str, dict[str, str]]:
    """Read the `{KEYSTORE}` substitution table into `store -> language -> value`.

    The value is what actually gets pasted, so it is what has to be counted: several locales'
    tokens carry their own preposition and differ in length by 10+ characters between stores.

    **The table is optional, the token is not.** The body named the platform's own secret store
    ("the Windows Credential Manager", "your device's Keychain") until it was judged to tell a
    reader nothing they wanted; "stored securely on your device" says the same thing to the person
    deciding whether to install. With no token in the copy there is nothing to substitute, so an
    absent table is legal and yields `{}`. What stays enforced is the pairing: a body that *uses*
    `{KEYSTORE}` with no table, or a table missing a store, is still a shape error; the failure
    that matters is measuring a string no console will ever be handed.
    """
    languages: list[str] = []
    tokens: dict[str, dict[str, str]] = {}
    heading = re.escape(f"`{KEYSTORE_TOKEN}` token")
    if not list(sections(listing.splitlines(), 3, re.compile(f"^{heading}"))):
        return {}
    for line in one_section(listing.splitlines(), 3, f"^{heading}", "keystore token"):
        if not line.startswith("| "):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if cells and cells[0] == "Store (platform)":
            languages = cells[1:]
            continue
        if not languages:
            continue
        store = next((known for known in STORES if cells[0].startswith(known)), None)
        if store is None or len(cells) - 1 != len(languages):
            continue
        # The lengths are already known equal; the guard above rejects the row otherwise; so this
        # needs no `strict=`, which would also cost the check its Python 3.9 compatibility.
        tokens[store] = dict(zip(languages, cells[1:]))
    missing = [store for store in STORES if store not in tokens]
    if missing:
        raise DocumentShapeError(
            f"the '{KEYSTORE_TOKEN} token' table has no row for: {', '.join(missing)}"
        )
    return tokens


# ---------------------------------------------------------------------------------------------
# The audits
# ---------------------------------------------------------------------------------------------


def has_per_store_fields(lines: list[str]) -> bool:
    """Whether this listing promises the fields a console is pushed from.

    A listing that carries `## Per-store fields` is claiming the whole set, so every subsection
    under it is required and a renamed heading is an error rather than a field that silently stops
    being measured. A listing without it -- the neutral default, which carries only what the Linux
    metadata needs -- is not claiming them, and none is looked for.

    The alternative, treating each subsection as individually optional, would have made the
    unbranded case work by making the branded one unguarded: exactly the shape where a scrape that
    finds nothing passes as loudly as one that finds everything.
    """
    return bool(list(sections(lines, 2, re.compile(r"^Per-store fields"))))


def listing_promises_per_store_fields() -> bool:
    """Whether the *resolved* listing is a full one; the shape a console push needs.

    Public because the store publishers' tests are the ones that need it. Every "the real copy is
    complete" assertion is a claim about a **branded** listing: an unbranded checkout resolves to
    `branding/default-listing.md`, which deliberately carries no per-store fields and one language,
    so a push refusing it is the tooling working. Those tests skip on this, and say why.
    """
    return has_per_store_fields(listing_path().read_text(encoding="utf-8").splitlines())


def audit_listing(listing: str, limits: dict[str, StoreLimits]) -> list[Measurement]:
    """Measure every field the listing carries against the store that has to accept it.

    `limits` comes from `LIMITS_PATH` and the copy from `listing_path()`, which are two files now:
    the caps are the stores' and stay public, the copy travels with the brand.

    **The per-store fields are measured only if the listing promises them**, which it does by
    carrying `## Per-store fields`; see `has_per_store_fields`. The shared body and Play's short
    description are required of every listing, because those two are what the Linux metadata is
    generated from and a build with neither has no entry in a software centre.
    """
    lines = listing.splitlines()
    tokens = parse_keystore_tokens(listing)
    measured: list[Measurement] = []

    # The shared description body; the one field that is store-specific only through {KEYSTORE},
    # so it is measured once per store rather than once.
    descriptions = list(sections(lines, 2, re.compile(r"^Shared description")))
    if not descriptions:
        raise DocumentShapeError("no 'Shared description' sections found")
    for title, body in descriptions:
        language = title.split("—")[-1].strip()
        blocks = fenced_blocks(body)
        if len(blocks) != 1:
            raise DocumentShapeError(
                f"'{title}' has {len(blocks)} fenced blocks, expected exactly 1"
            )
        for store in STORES:
            cap = limits[store].description
            if cap is None:
                continue
            pasted = blocks[0]
            if KEYSTORE_TOKEN in pasted:
                token = tokens.get(store, {}).get(language)
                if token is None:
                    raise DocumentShapeError(
                        f"the {language} body uses {KEYSTORE_TOKEN} but the "
                        f"'{KEYSTORE_TOKEN} token' table has no {language} value for {store}"
                    )
                pasted = pasted.replace(KEYSTORE_TOKEN, token)
            measured.append(
                Measurement(f"Description / {language} / {store}", cap, len(pasted))
            )

    # The product name goes into every store's name field unchanged, and it is the injected
    # identity rather than a line of copy: the launcher, the installer and the store must agree, and
    # a name written in two files is a name that can disagree.
    app_name = brand.value("MAILCAL_APP_NAME")
    for store in STORES:
        cap = limits[store].name
        if cap is not None:
            measured.append(Measurement(f"Name / {store}", cap, len(app_name)))

    # Microsoft Store product features: a capped number of fields, each capped in length.
    per_store = has_per_store_fields(lines)
    ms = limits["Microsoft Store"]
    feature_blocks = []
    if per_store:
        features_body = one_section(
            lines, 3, r"^Microsoft Store — Product features", "Microsoft Store features"
        )
        feature_blocks = _labelled_blocks(features_body)
        if not feature_blocks:
            raise DocumentShapeError("no Microsoft Store feature lists found")
    for language, block in feature_blocks:
        entries = [line for line in block.splitlines() if line.strip()]
        if ms.feature_count is not None:
            measured.append(
                Measurement(
                    f"Feature list / {language} / Microsoft Store",
                    ms.feature_count,
                    len(entries),
                    unit="features",
                )
            )
        if ms.feature_chars is None:
            continue
        for number, entry in enumerate(entries, start=1):
            measured.append(
                Measurement(
                    f"Feature {number} / {language} / Microsoft Store",
                    ms.feature_chars,
                    len(entry),
                )
            )

    # Microsoft Store search terms: a capped number of terms, each capped in length, and; the one
    # that actually binds; a shared budget of distinct words across all of a language's terms.
    search_blocks = []
    if per_store:
        search_body = one_section(
            lines, 3, r"^Microsoft Store — Search terms", "Microsoft Store search terms"
        )
        search_blocks = _labelled_blocks(search_body)
        if not search_blocks:
            raise DocumentShapeError("no Microsoft Store search-term lists found")
    for language, block in search_blocks:
        terms = [line.strip() for line in block.splitlines() if line.strip()]
        if ms.search_count is not None:
            measured.append(
                Measurement(
                    f"Search terms / {language} / Microsoft Store",
                    ms.search_count,
                    len(terms),
                    unit="terms",
                )
            )
        if ms.search_words is not None:
            measured.append(
                Measurement(
                    f"Search words / {language} / Microsoft Store",
                    ms.search_words,
                    len(search_term_words(terms)),
                    unit="words",
                )
            )
        if ms.search_chars is None:
            continue
        for number, term in enumerate(terms, start=1):
            measured.append(
                Measurement(
                    f"Search term {number} / {language} / Microsoft Store",
                    ms.search_chars,
                    len(term),
                )
            )

    # App Store Connect's three short fields share one block per language, one per line.
    apple = limits["App Store Connect"]
    apple_caps = {
        "Subtitle": apple.subtitle,
        "Promotional": apple.short,
        "Keywords": apple.keywords,
    }
    apple_blocks = []
    if per_store:
        apple_body = one_section(
            lines, 3, r"^App Store Connect — Subtitle", "App Store Connect short fields"
        )
        apple_blocks = _labelled_blocks(apple_body)
        if not apple_blocks:
            raise DocumentShapeError("no App Store Connect field blocks found")
    for language, block in apple_blocks:
        seen: set[str] = set()
        for line in block.splitlines():
            field, _, value = line.partition(":")
            cap = apple_caps.get(field.strip())
            if cap is None:
                continue
            seen.add(field.strip())
            measured.append(
                Measurement(
                    f"{field.strip()} / {language} / App Store Connect",
                    cap,
                    len(value.strip()),
                )
            )
        if seen != set(apple_caps):
            raise DocumentShapeError(
                f"the App Store Connect block for {language} is missing: "
                f"{', '.join(sorted(set(apple_caps) - seen))}"
            )

    # Google Play's short description is a whole block.
    short_cap = limits["Google Play"].short
    short_body = one_section(
        lines, 3, r"^Google Play — Short description", "'Google Play — Short description'"
    )
    short_blocks = _labelled_blocks(short_body)
    if not short_blocks:
        raise DocumentShapeError("no Google Play short descriptions found")
    for language, block in short_blocks:
        if short_cap is not None:
            measured.append(
                Measurement(f"Short description / {language} / Google Play", short_cap, len(block))
            )

    return measured


def whats_new_caps(limits: dict[str, StoreLimits]) -> dict[str, int]:
    """Each store's "What's new" limit, read from the table rather than hard-coded.

    Play's 500 is the binding one today and the changelog contract says so; but taking it from the
    table means that if Play ever raises its ceiling, the next store's number takes over on its own.
    """
    caps = {store: limits[store].whats_new for store in STORES if limits[store].whats_new}
    if not caps:
        raise DocumentShapeError("the 'Field limits' table states no 'What's new' limit")
    return caps


def binding_store(caps: dict[str, int], stores) -> tuple[str, int] | None:
    """The tightest of `stores` and its cap; `None` when a note reaches no store at all.

    A Linux-only change is written down and shipped like any other; it simply has no console to be
    rejected by, so measuring it would invent a limit rather than enforce one.
    """
    applicable = {store: caps[store] for store in stores if store in caps}
    if not applicable:
        return None
    store = min(applicable, key=lambda name: applicable[name])
    return store, applicable[store]


def audit_fragments(limits: dict[str, StoreLimits], fragments=None) -> list[Measurement]:
    """Measure every pending fragment against the tightest store *it* is pasted into.

    An empty `unreleased/` is legal; it means nothing is pending; but a fragment that yields no
    notes, or names a platform that is not a platform, is an error rather than a quiet pass. That is
    the same discipline the rest of this file is built on: a scrape that finds nothing must not
    read as a green run.
    """
    caps = whats_new_caps(limits)
    fragments = load_fragments() if fragments is None else fragments
    measured: list[Measurement] = []
    for fragment in fragments:
        binding = binding_store(caps, store_targets(fragment.platforms))
        if binding is None:
            continue
        store, cap = binding
        for language, note in fragment.notes.items():
            measured.append(
                Measurement(f"Fragment {fragment.slug} / {language} / {store}", cap, len(note))
            )
    return measured


def audit_releases(limits: dict[str, StoreLimits], releases=None) -> list[Measurement]:
    """Measure every assembled release note, section by section, against its own stores.

    History is measured too, not just what is pending: a released note is the text that was pasted,
    so if a limit in the table ever drops, the check says which shipped notes would no longer fit
    rather than only guarding the next one.
    """
    caps = whats_new_caps(limits)
    releases = load_releases() if releases is None else releases
    measured: list[Measurement] = []
    for version, sections in releases:
        for section in sections:
            binding = binding_store(caps, store_targets(section.platforms))
            if binding is None:
                continue
            store, cap = binding
            for language, note in section.notes.items():
                measured.append(
                    Measurement(f"Release note {version} / {language} / {store}", cap, len(note))
                )
    return measured


# How many of the tightest fields to name on a green run. Enough to see the shape of the problem
# without printing all ~200 measurements.
TIGHTEST_REPORTED = 6


def main() -> int:
    """Run every audit and report. Exit code 1 on a violation, 2 if a document changed shape."""
    try:
        limits = parse_limits(LIMITS_PATH.read_text(encoding="utf-8"))
        listing = listing_path().read_text(encoding="utf-8")
        measured = audit_listing(listing, limits) + audit_fragments(limits) + audit_releases(limits)
    except FragmentError as error:
        # An authoring mistake in a file someone just wrote; different advice from the shape error
        # below, which is about this checker having fallen behind a document.
        print(f"ERROR: {error}", file=sys.stderr)
        print(
            "See docs/changelog.md for the fragment format.",
            file=sys.stderr,
        )
        return 1
    except DocumentShapeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print(
            "This check scrapes docs/store-listing.md (the field limits), the resolved "
            "branding/<brand>-listing.md (the copy) and docs/changelog/. One of their shapes "
            "changed, so it can no longer measure them: fix the scraper rather than the document, "
            "and see scripts/ci/tests/test_store_copy_length.py.",
            file=sys.stderr,
        )
        return 2

    over = [item for item in measured if not item.fits]
    if over:
        print("Store copy that its console would reject:")
        for item in sorted(over, key=lambda item: item.where):
            print(item)
        print(
            f"\nERROR: {len(over)} field(s) exceed the limits in docs/store-listing.md "
            "-> 'Field limits'. Trim the copy (from the tail, never the opening paragraphs) or, "
            "if a console's limit really changed, change it in that table. A note is measured "
            "against the tightest store its `Platforms:` reach, so widening a fragment's platforms "
            "can tighten its cap.",
            file=sys.stderr,
        )
        return 1

    print(f"OK: all {len(measured)} store-listing fields and release notes fit their limits.")
    # Character fields only. The seven Microsoft feature lists are all at exactly 20/20; a fact
    # store-listing.md already states and explains; so ranking them alongside would fill the
    # report with the one thing nobody needs telling.
    text_fields = [item for item in measured if item.unit == "characters"]
    print(f"Tightest {TIGHTEST_REPORTED}: how much room a future edit has:")
    for item in sorted(text_fields, key=lambda item: (item.margin, item.where))[:TIGHTEST_REPORTED]:
        print(item)
    at_cap = [item for item in measured if item.unit != "characters" and item.margin == 0]
    if at_cap:
        print(f"({len(at_cap)} list field(s) sit at exactly their cap: adding one means dropping one.)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
