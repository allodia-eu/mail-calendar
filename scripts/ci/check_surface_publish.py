#!/usr/bin/env python3
"""A published surface may only be signalled by publishing it.

`Surfaced<T>` (crates/mailcal-app/src/surfaced.rs) makes the write and the signal one operation,
so a host that pulls the instant it is told can never read the previous snapshot. The type closes
the door for anyone who uses it; but `App` still owns an `observer`, and
`self.observer.surface_changed(Surface::MailboxList)` would compile fine and silently reopen the
bug, in a form no test would catch: a stale paint is not a panic, and there is no second signal
coming to correct it.

So this refuses two things across the crate:

1. Signalling a published surface anywhere but `surfaced.rs`. Announce it by publishing a value,
   or; when what went stale is not this snapshot; by `resignal()`, which cannot run ahead of a
   write because it performs none.
2. Declaring one of those snapshot fields as a bare `Mutex`, which would take it back out of the
   type's hands entirely.

`git grep --untracked`, so a newly added file is covered before it is staged. Adding a surface to
`PUBLISHED` is the deliberate act of putting it under the rule; a surface whose pull recomputes
from live state (`Settings`, `Connectivity`) has no stored snapshot and does not belong here.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = "crates/mailcal-app/src"
HOME = f"{CRATE}/surfaced.rs"

# Surface -> the `App` field that holds its snapshot.
PUBLISHED = {
    "MailboxList": "mailbox_list",
    "Calendar": "calendar",
    "Reading": "reading",
    "Sending": "send_status",
}

SIGNAL = re.compile(r"surface_changed\(\s*(?:crate::)?Surface::([A-Za-z]+)\s*\)")


def tracked_rust_files() -> list[Path]:
    """Every `.rs` file under the app crate, untracked ones included."""
    out = subprocess.run(
        ["git", "grep", "--untracked", "-l", "", "--", f"{CRATE}/*.rs"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    return [ROOT / line for line in out.stdout.splitlines() if line]


def stray_signals(files: list[Path]) -> list[str]:
    """Signals of a published surface raised outside `surfaced.rs`."""
    problems = []
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        if relative == HOME:
            continue
        text = path.read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), start=1):
            for surface in SIGNAL.findall(line):
                if surface in PUBLISHED:
                    field = PUBLISHED[surface]
                    problems.append(
                        f"{relative}:{number}: Surface::{surface} is published, so it may not be "
                        f"signalled directly.\n    Use self.{field}.publish(value), or "
                        f"self.{field}.resignal() if there is no new value.\n    {line.strip()}"
                    )
    return problems


def bare_fields(files: list[Path]) -> list[str]:
    """Snapshot fields declared as a plain `Mutex` instead of a `Surfaced`."""
    problems = []
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        text = path.read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), start=1):
            for surface, field in PUBLISHED.items():
                if re.match(rf"\s*{field}:\s*Mutex<", line):
                    problems.append(
                        f"{relative}:{number}: `{field}` backs Surface::{surface} and must stay a "
                        f"`Surfaced<_>`; a bare Mutex separates the write from the signal again."
                        f"\n    {line.strip()}"
                    )
    return problems


def main() -> int:
    files = tracked_rust_files()
    if not files:
        print("ERROR: found no app-crate sources to check: is the path still right?")
        return 1
    problems = stray_signals(files) + bare_fields(files)
    if problems:
        print("Published surfaces must be announced by publishing them:\n")
        for problem in problems:
            print(f"  {problem}\n")
        print(f"See {HOME} for why.")
        return 1
    print(
        f"OK: {len(PUBLISHED)} published surface(s) are only signalled by publishing, "
        f"across {len(files)} file(s)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
