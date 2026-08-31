#!/usr/bin/env python3
"""Fails when a diagnostic log line names this repository instead of the user's mail.

The log is a file the user opens and attaches to a support request, so every line is product
surface (`docs/logging.md` -> "A log line describes the user's mail, never our source tree").
This checks only the part of that rule a machine can decide without guessing:

  * a path inside this repo, or any `.md` reference   -- `docs/provider-oauth.md rule 5`
  * an issue or PR number                             -- `#1234`
  * a raw Rust account-id interpolation               -- `{account_id}` / `{account}`

Deliberately NOT checked: internal jargon ("the registry", "re-serialize", a type name). That is
the larger half of the rule, and it needs judgement -- a checker that guesses at prose produces
false positives, and a checker people learn to skip protects nothing. The narrow half is worth
having because it is exact, and because it is the half that actually shipped: an `error!` citing
`docs/provider-oauth.md rule 5` reached a production build.

Scans Rust, Kotlin, Swift and C# for logging calls and inspects only their *string literals*, so a
comment above a log line -- which is where our reasoning is supposed to live -- is never flagged.

Usage: check_log_hygiene.py [paths...]      (default: crates/ clients/)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

# The logging calls each language spells differently. Matched at the call, then the argument text
# that follows is scanned up to a blank line or the end of the statement.
LOG_CALL = re.compile(
    r"""
    (?:
        log:: (?: error | warn | info | debug | trace ) ! \s*\(   # Rust, the `log` crate
      | logger\. (?: error | warn | info | debug ) \s*\(          # Kotlin/Swift/C# instance loggers
      | Log\. (?: Error | Warn | Info | Debug ) \s*\(             # C# static helper
      | (?: NSLog | os_log ) \s*\(                               # Apple
    )
    """,
    re.VERBOSE,
)

# What may never appear inside a logged string.
FORBIDDEN = (
    (
        re.compile(r"\b(?:docs|crates|clients|scripts)/[A-Za-z0-9._/-]+"),
        "names a path in this repository",
    ),
    (re.compile(r"[A-Za-z0-9_-]+\.md\b"), "cites a design document"),
    (re.compile(r"(?<![A-Za-z0-9_])#\d{2,}"), "cites an issue or PR number"),
    (
        re.compile(r"\{(?:account_id|account)(?::[^}]*)?\}"),
        "interpolates a raw account id (which embeds an address)",
    ),
)

# A Rust string literal, including the `\`-continued multi-line form the wide log lines use.
STRING_LITERAL = re.compile(r'"(?:[^"\\]|\\.)*"', re.DOTALL)

SUFFIXES = {".rs", ".kt", ".swift", ".cs"}


def tracked_sources(roots: list[str]) -> list[Path]:
    """Every tracked source file under `roots`, plus untracked ones.

    `git ls-files --others --exclude-standard` is included deliberately: `check-file-length.sh`
    reads only the index, so a brand-new file it has never seen is invisible to it until staged.
    A checker that cannot see the file you just wrote fails at the one moment it matters.
    """
    listed: set[Path] = set()
    for extra in ([], ["--others", "--exclude-standard"]):
        result = subprocess.run(
            ["git", "ls-files", "-z", *extra, "--", *roots],
            capture_output=True,
            text=True,
            check=True,
        )
        for name in result.stdout.split("\0"):
            path = Path(name)
            # `is_file()` is not belt-and-braces: `git ls-files` reads the *index*, so a source file
            # deleted in the working tree but not yet staged is listed and cannot be opened. Without
            # this the checker crashes; and a checker that crashes fails the build for a reason that
            # has nothing to do with what it checks, which is how a gate gets disabled.
            if name and path.suffix in SUFFIXES and path.is_file():
                listed.add(path)
    return sorted(listed)


def violations(path: Path) -> list[tuple[int, str, str]]:
    """Every `(line, reason, snippet)` in `path`, scanning only logged string literals."""
    text = path.read_text(encoding="utf-8", errors="replace")
    found: list[tuple[int, str, str]] = []
    for call in LOG_CALL.finditer(text):
        # The argument text: from the opening paren to the end of the statement. `);` at the start
        # of a line ends the widest formatted call; a blank line bounds a malformed one.
        tail = text[call.end() : call.end() + 4000]
        end = re.search(r"\n\s*\)\s*[;,]?|\n\s*\n", tail)
        args = tail[: end.start()] if end else tail
        for literal in STRING_LITERAL.finditer(args):
            for pattern, reason in FORBIDDEN:
                hit = pattern.search(literal.group())
                if hit:
                    line = text.count("\n", 0, call.start()) + 1
                    found.append((line, reason, hit.group()))
    return found


def main(argv: list[str]) -> int:
    roots = argv[1:] or ["crates", "clients"]
    failures = 0
    for path in tracked_sources(roots):
        for line, reason, snippet in violations(path):
            print(f"{path}:{line}: a log line {reason}: {snippet!r}")
            failures += 1
    if failures:
        print(
            f"\n{failures} log line(s) name this repository. The log is a file the user reads and "
            "attaches to a support request, so it says what happened to their mail and what it "
            "means for them; our reasoning goes in a comment beside the code, where a refactor "
            "keeps it true. See docs/logging.md."
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
