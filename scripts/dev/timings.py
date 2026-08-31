#!/usr/bin/env python3
"""Reduce a client's diagnostic log to the timing table the engine benchmarks print.

    scripts/dev/timings.py                                  # the macOS log + its rotations
    scripts/dev/timings.py ~/Downloads/app.log              # a log a user handed over
    scripts/dev/timings.py --since 2026-08-12 --top 5       # one day, and the slowest runs

The engine's `cargo bench -p mailbox-fixture` measures a fixture of known size and prints
`Operation | n | p50 | p90 | p99 | max`. The app already logs a duration for the same
operations, against whatever mail the user actually has; but it logs them one line at a
time, thousands of them, where a p99 is invisible and only the last line is ever read. This
turns that stream into the same table, so the two can be read side by side: what an
operation costs on a fixture, and what it costs on a real mailbox.

Which is the point. A benchmark that improves while the daily-driver numbers do not has
measured the wrong thing, and the only way to notice is to keep both tables in the same
shape.

Percentiles are nearest-rank, matching the engine's reporter: every number printed is a
duration some run actually took, not an interpolation between two that did.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

# One log record: `2026-08-12 16:00:08.517 INFO [mailcal_app::snapshot] rebuild_snapshot: ...`
RECORD = re.compile(
    r"^(?P<stamp>\d{4}-\d{2}-\d{2}) \d{2}:\d{2}:\d{2}\.\d+ \w+ \[(?P<target>[\w:]+)\] (?P<body>.*)$"
)

# Every timing line the core emits, mapped onto the operation it measures. Each pattern
# names its milliseconds `ms`; a line carrying two durations (a sync and the rebuild that
# follows it) is two operations, because they are two different costs with two different
# fixes.
#
# Keep this in step with the `log::info!` calls it reads. A renamed line silently stops
# being counted; which is a table that quietly shrinks rather than one that fails; so
# `--unmatched` exists to name any timing line no pattern claimed.
#
# The trailing `(?:[;(].*)?$` on several patterns is not decoration: those lines carry an
# optional qualifier (`; unchanged`, `; no redraw`, `(cache miss)`), and a pattern anchored
# hard at `ms$` would drop exactly the samples the qualifier marks as interesting.
OPERATIONS: list[tuple[str, re.Pattern[str]]] = [
    ("rebuild_snapshot", re.compile(r"^rebuild_snapshot: .* in (?P<ms>\d+)ms$")),
    ("refresh_mail/sync", re.compile(r"^refresh_mail: sync (?P<ms>\d+)ms \+ rebuild \d+ms$")),
    ("refresh_mail/rebuild", re.compile(r"^refresh_mail: sync \d+ms \+ rebuild (?P<ms>\d+)ms$")),
    ("sync_account", re.compile(r"^sync\[\w+\]: \d+ folder\(s\), .* in (?P<ms>\d+)ms$")),
    ("sync/folder", re.compile(r"^sync\[\w+\]: folder\[\d+\] \+\d+ -\d+ in (?P<ms>\d+)ms$")),
    ("sync/folder_list", re.compile(r"^sync\[\w+\]: folder list (?P<ms>\d+)ms$")),
    # The whole-account threading pass that used to run after every sync. The engine now
    # threads inside the apply and emits no line, so this matches archived logs only; kept for
    # the same reason as `cached_messages` below.
    ("sync/derive_threads", re.compile(r"^sync\[\w+\]: derive threads (?P<ms>\d+)ms$")),
    # `cached_messages` is the older line: one account's newest messages read out of the
    # store and deserialized. `list_rows` is what replaced it; the projected rows for the
    # accounts in view, read from an index. They are kept as **two** operations rather than one
    # renamed one because they measure two different costs, and an archived log holds the first.
    ("cached_messages", re.compile(r"^cached_messages: .* in (?P<ms>\d+)ms(?: \(.*\))?$")),
    ("list_rows", re.compile(r"^list rows: read .* in (?P<ms>\d+)ms(?: \(.*\))?$")),
    ("thread_completion", re.compile(r"^thread completion: .* in (?P<ms>\d+)ms$")),
    ("folder_on_demand", re.compile(r"^on-demand: folder synced in (?P<ms>\d+)ms$")),
    ("prefetch_bodies", re.compile(r"^prefetch: warmed .* in (?P<ms>\d+)ms$")),
    (
        "rebuild_calendar_cache",
        re.compile(r"^rebuild_calendar_cache: .* in (?P<ms>\d+)ms(?:;.*)?$"),
    ),
    (
        "refresh_calendar/sync",
        re.compile(r"^refresh_calendar: sync\+expand (?P<ms>\d+)ms \+ rebuild \d+ms(?:;.*)?$"),
    ),
    (
        "refresh_calendar/rebuild",
        re.compile(r"^refresh_calendar: sync\+expand \d+ms \+ rebuild (?P<ms>\d+)ms(?:;.*)?$"),
    ),
    ("rebuild_contacts", re.compile(r"^rebuild_contacts: .* in (?P<ms>\d+)ms$")),
    ("boot/open+migrate", re.compile(r"^boot: engine open\+migrate in (?P<ms>\d+)ms$")),
    (
        "boot/abandon_leases",
        re.compile(r"^boot: abandoned \d+ interrupted sync scope lease\(s\) in (?P<ms>\d+)ms$"),
    ),
    (
        "boot/prime_snapshot",
        re.compile(r"^boot: primed cached snapshot in (?P<ms>\d+)ms; NewAccounts total \d+ms$"),
    ),
    (
        "boot/new_accounts",
        re.compile(r"^boot: primed cached snapshot in \d+ms; NewAccounts total (?P<ms>\d+)ms$"),
    ),
    ("event_detail", re.compile(r"^event_detail: resolved in (?P<ms>\d+)ms$")),
]

# Lines that carry a duration this table is deliberately not about, so `--unmatched`
# reports only genuinely unmapped ones. Three reasons, all "not engine work":
#
#   - a round trip to somebody else's server (a connect, an OAuth refresh, a contact-source
#     bind, an account registration, a provider failure); that is network latency, and
#     nothing in the data plane can make it faster;
#   - an MCP tool call, which is a client's round trip through the same surface a user
#     drives, so its cost is already counted on the line underneath it;
#   - a *delay* rather than a duration (a retry backoff), which would read as slowness.
IGNORED = re.compile(
    r"^(oauth|carddav|jmap|imap|caldav|graph|google|microsoft|mcp|reconnect|connect\[|"
    r"contacts\[|contact_detail|refresh_contacts|add.account|on-demand: connected|"
    r"boot: (account\[|\d+ reachable))"
    # Matched anywhere in the line, not only at its start: a scope that failed or was
    # already held reports how long that took, and counting it as throughput would read
    # as a fast sync.
    r"|failed in \d+ms|busy in \d+ms|retrying in \d+ms"
)

# Where each platform keeps its log (docs/logging.md). Only the ones readable from this
# machine without a device are listed; for Android and iOS use `scripts/dev/logs.sh
# <platform> --dump > app.log` and pass the file.
DEFAULT_LOGS = [
    Path.home() / ".local/share/mailcal/mailcal.log",
    Path(os.environ.get("LOCALAPPDATA", "")) / "Allodia/MailCalendar/logs/app.log",
]


@dataclass
class Samples:
    """Every duration observed for one operation, with where the slowest ones happened."""

    durations: list[int] = field(default_factory=list)
    worst: list[tuple[int, str]] = field(default_factory=list)

    def add(self, ms: int, line: str) -> None:
        self.durations.append(ms)
        self.worst.append((ms, line))


def percentile(ascending: list[int], p: int) -> int:
    """The nearest-rank percentile: the smallest sample at or above `p` percent through."""
    if not ascending:
        return 0
    rank = max(1, -(-len(ascending) * p // 100))
    return ascending[rank - 1]


def classify(body: str) -> list[tuple[str, int]]:
    """The operations one log body reports, as `(operation, milliseconds)` pairs.

    A body may match more than one pattern; `refresh_mail` carries both a sync and a
    rebuild; so every pattern is tried rather than stopping at the first hit.
    """
    hits = []
    for name, pattern in OPERATIONS:
        match = pattern.match(body)
        if match:
            hits.append((name, int(match.group("ms"))))
    return hits


def collect(
    lines: list[str], since: str | None = None, until: str | None = None
) -> tuple[dict[str, Samples], list[str]]:
    """Reduces log lines to per-operation samples, plus the timing lines nothing matched."""
    samples: dict[str, Samples] = {}
    unmatched: list[str] = []
    for line in lines:
        record = RECORD.match(line.rstrip("\n"))
        if not record:
            continue
        if since and record["stamp"] < since:
            continue
        if until and record["stamp"] > until:
            continue
        body = record["body"]
        hits = classify(body)
        if not hits:
            if "ms" in body and re.search(r"\d+ms", body) and not IGNORED.search(body):
                unmatched.append(body)
            continue
        for name, ms in hits:
            samples.setdefault(name, Samples()).add(ms, line.rstrip("\n"))
    return samples, unmatched


def render(samples: dict[str, Samples], label: str) -> str:
    """Renders the table, widest column first so it lines up with the engine's."""
    width = max([len("Operation")] + [len(name) for name in samples])
    out = [f"\n{label}\n"]
    out.append(
        f"| {'Operation':<{width}} | {'n':>7} | {'p50':>10} | {'p90':>10} | {'p99':>10} | {'max':>10} |"
    )
    out.append(f"|{'-' * (width + 2)}|{'-' * 9}|{'-' * 12}|{'-' * 12}|{'-' * 12}|{'-' * 12}|")
    for name in sorted(samples):
        ascending = sorted(samples[name].durations)
        cells = [
            f"{percentile(ascending, p)} ms" for p in (50, 90, 99)
        ] + [f"{ascending[-1]} ms"]
        out.append(
            f"| {name:<{width}} | {len(ascending):>7} | "
            + " | ".join(f"{cell:>10}" for cell in cells)
            + " |"
        )
    return "\n".join(out)


def render_worst(samples: dict[str, Samples], top: int) -> str:
    """The slowest individual runs, with their timestamps; where to start reading the log."""
    out = ["\nSlowest runs"]
    for name in sorted(samples):
        worst = sorted(samples[name].worst, reverse=True)[:top]
        if not worst:
            continue
        out.append(f"\n  {name}")
        for ms, line in worst:
            out.append(f"    {ms:>7} ms  {line[:100]}")
    return "\n".join(out)


def read_logs(paths: list[Path]) -> tuple[list[str], list[Path]]:
    """Reads every given log, and for a default path its `.1`..`.3` rotations too."""
    lines: list[str] = []
    read: list[Path] = []
    for path in paths:
        for candidate in [path] + [Path(f"{path}.{n}") for n in (1, 2, 3)]:
            if candidate.is_file():
                lines.extend(candidate.read_text(errors="replace").splitlines())
                read.append(candidate)
    return lines, read


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "logs",
        nargs="*",
        type=Path,
        help="log files to read; defaults to this machine's own client log and its rotations",
    )
    parser.add_argument("--since", metavar="YYYY-MM-DD", help="ignore records before this date")
    parser.add_argument("--until", metavar="YYYY-MM-DD", help="ignore records after this date")
    parser.add_argument(
        "--top",
        type=int,
        default=0,
        metavar="N",
        help="also list the N slowest runs of each operation, with their timestamps",
    )
    parser.add_argument(
        "--unmatched",
        action="store_true",
        help="list timing lines no pattern claimed: a renamed log line stops being counted",
    )
    args = parser.parse_args(argv)

    paths = args.logs or [path for path in DEFAULT_LOGS if path.is_file()]
    if not paths:
        parser.error(
            "no log found on this machine: pass one, or dump a device's with "
            "`scripts/dev/logs.sh <platform> --dump > app.log`"
        )
    lines, read = read_logs(paths)
    if not read:
        parser.error(f"none of these exist: {', '.join(str(p) for p in paths)}")

    samples, unmatched = collect(lines, args.since, args.until)
    if not samples:
        print("no timing lines in range", file=sys.stderr)
        return 1

    span = " · ".join(str(path.name) for path in read)
    print(render(samples, f"{span} ({len(lines)} records)"))
    if args.top:
        print(render_worst(samples, args.top))
    if args.unmatched:
        print("\nUnmatched timing lines")
        # Collapsed to line *shapes*: the raw bodies differ only in their numbers, and a
        # truncated alphabetical list of them hides whole kinds behind one noisy one.
        for shape in sorted({re.sub(r"\d+", "N", body)[:110] for body in unmatched}):
            print(f"  {shape}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
