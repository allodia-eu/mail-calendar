#!/usr/bin/env python3
"""Fail when swept prose contains a dash used as punctuation."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXTENSIONS = {".cs", ".html", ".js", ".kt", ".kts", ".md", ".ps1", ".py", ".rs", ".sh",
              ".swift", ".toml", ".ts", ".xml", ".yml", ".yaml"}
EXEMPT_PARTS = (
    # The checker and its tests contain intentional dash literals to prove detection.
    "scripts/ci/check_dash_hygiene.py",
    "scripts/ci/tests/test_dash_hygiene.py",
    "LICENSES/",
    "clients/android/gradle/",
    "clients/composer/dist/",
    "docs/changelog/released/",
    "docs/changelog/announcements/",
    "docs/privacy-policy",
    "crates/mailcal-bindings/src/showcase_data/",
    "crates/mailcal-bindings/src/showcase_bodies/",
    # These values are coupled to assertions in the same test files.
    "clients/windows/uitests/SyncHint.Tests.ps1",
    "clients/windows/uitests/SyncHintBodies.Tests.ps1",
    "crates/mailcal-app/tests/fixtures/imip/README.md",
)
# Every path the sweep cleaned. Leaving one out does not fail the check; it silently stops
# guarding that half of the tree, which is how the reader-facing docs went unwatched once already.
# `:(glob)` keeps the last entry to root-level markdown, since a bare `*.md` matches at any depth.
SWEPT_ROOTS = ("crates", "clients", "scripts", "docs", ".agents", "messages", "branding",
               "allodia_license", "docker", ":(glob)*.md")

COMMENT = re.compile(r"^\s*(?:/\*\*|/\*|///|//!|//|#|\*|--|<!--)\s?(.*)")
FENCE = re.compile(r"^\s*```")
QUOTED_LINE = re.compile(r'''^\s*(?:>|\$ |>>> |(?:OK|ERROR|WARNING):|#{1,6}\s).*[—–]''')
QUOTED_TEXT = re.compile(r'''(?P<quote>["']).*?(?P=quote)''')
SYMBOL_REFERENCE = re.compile(
    r"`[^`]*`|\[[^\]]+\]|<(?:see|paramref|typeparamref)\s+[^>]+>"
)


# `--others --exclude-standard` alongside `--cached` is not optional: without it `git ls-files`
# reads the index, so a file added but not yet staged is invisible and this passes on the very
# change that introduces what it forbids. `check-public-hygiene.sh` already says so about
# `git grep --untracked`, and AGENTS.md says it about `check-file-length.sh`; this checker had the
# same hole and neither. Ignored paths (target/, .env) stay ignored either way.
def tracked() -> list[tuple[str, Path]]:
    names = subprocess.run(["git", "-C", str(ROOT), "ls-files", "--cached", "--others",
                            "--exclude-standard", *SWEPT_ROOTS],
                           capture_output=True, text=True, check=True).stdout.splitlines()
    return [(name, ROOT / name) for name in names
            if Path(name).suffix in EXTENSIONS and not any(part in name for part in EXEMPT_PARTS)]


def is_range(text: str, index: int) -> bool:
    """Ranges and minus signs do not use a dash as sentence punctuation."""
    char = text[index]
    if char == "–":
        before = text[index - 1:index]
        after = text[index + 1:index + 2]
        if before and after and not before.isspace() and not after.isspace():
            return True
        return bool(re.search(r"\d\s–\s\d", text[max(0, index - 2):index + 3]))
    return False


def is_matrix_cell(text: str, index: int) -> bool:
    if "|" not in text:
        return False
    offset = 0
    for cell in text.split("|"):
        end = offset + len(cell)
        if offset <= index < end:
            return cell.strip() == "—"
        offset = end + 1
    return False


def is_quoted_line(text: str) -> bool:
    """Quoted headings and captured output are data, not prose authored here."""
    return bool(QUOTED_LINE.match(text))


def is_quoted_text(text: str, index: int) -> bool:
    """Quoted headings and captured output remain exact data, including their typography."""
    return any(match.start() <= index < match.end()
               for match in QUOTED_TEXT.finditer(text))


MATRIX_GLYPH = ("✅", "🚧", "⬜", "❌")


def is_matrix_marker(text: str) -> bool:
    """A capability matrix spends the em dash as its "not applicable" cell.

    True for the legend that declares it, for a cell that qualifies it (`— no worker`) and for
    prose naming it, all of which carry the matrix's own glyphs. `is_matrix_cell` sees only a cell
    holding nothing else, so without this the legend declaring the convention fails the rule the
    convention is exempt from.
    """
    return any(glyph in text for glyph in MATRIX_GLYPH)


def is_named_character(text: str, index: int) -> bool:
    """`(—)` names the character rather than using it, which is how the rule states itself."""
    return text[index - 1:index] == "(" and text[index + 1:index + 2] == ")"


def is_after_unclosed_backtick(text: str, index: int) -> bool:
    """A quoted span that wraps to the next line, such as a server error string."""
    return text.count("`", 0, index) % 2 == 1


def is_symbol_reference(text: str, index: int) -> bool:
    """A dash inside a documented symbol name must remain an exact identifier."""
    return any(match.start() <= index < match.end()
               for match in SYMBOL_REFERENCE.finditer(text))


def dash_hits(name: str, path: Path):
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return
    markdown = path.suffix == ".md"
    fenced = False
    for number, raw in enumerate(text.splitlines(), 1):
        if markdown and FENCE.match(raw):
            fenced = not fenced
            continue
        if fenced:
            continue
        comment = COMMENT.match(raw)
        if not markdown and not comment:
            continue
        line = raw if markdown else comment.group(1)
        if is_quoted_line(line):
            continue
        for index, char in enumerate(line):
            if (char not in "—–" or is_range(line, index) or is_symbol_reference(line, index)
                    or is_quoted_text(line, index) or is_named_character(line, index)
                    or is_after_unclosed_backtick(line, index)):
                continue
            if char == "—" and (is_matrix_marker(line)
                                or ("|" in line and is_matrix_cell(line, index))):
                continue
            yield number, raw.strip(), char


def main() -> int:
    found = [(name, number, line, char)
             for name, path in tracked()
             for number, line, char in dash_hits(name, path)]
    if found:
        print("Dash punctuation remains in swept prose:", file=sys.stderr)
        for name, number, line, char in found:
            print(f"  {name}:{number}: {char} {line}", file=sys.stderr)
        print(f"\nERROR: {len(found)} dash occurrence(s).", file=sys.stderr)
        return 1
    print(f"OK: swept prose in {len(tracked())} file(s) contains no dash punctuation.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
