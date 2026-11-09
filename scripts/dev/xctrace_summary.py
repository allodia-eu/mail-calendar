#!/usr/bin/env python3
"""Reduce a Time Profiler recording of the calendar's scripted motion to what the main thread did.

    xcrun xctrace export --input run.trace --toc > toc.xml
    xcrun xctrace export --input run.trace \\
      --xpath '//trace-toc[1]/run[1]/data[1]/table[@schema="time-profile"]' > profile.xml
    scripts/dev/xctrace_summary.py profile.xml --toc toc.xml --log run.log [--top 12]

`scripts/dev/calendar-perf-apple.sh mac-profile` runs all three. It prints the main thread's busy
samples for each second of the recording, then, for each phase of the drive (hours, days, zoom),
the share of that window the main thread was busy and the symbols it was busy in.

The export compresses itself: an element seen once carries an `id`, and every later occurrence is
an empty element with a `ref` to it, threads, backtraces and frames alike. Every lookup below goes
through `resolve`, or a repeated backtrace reads as an empty one.

A sample whose innermost frame is `mach_msg2_trap` is the run loop waiting for work, so it is not
busy, and busy time is the busy samples' weight, never their share of the thread's samples (see
`busy_per_second`). The phases come from the trace's own log lines: each "calendar frames" line is written as
its burst of motion ends, and names the axis the drive moved by its input counts.
"""

from __future__ import annotations

import argparse
import re
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

IDLE_LEAF = "mach_msg2_trap"
MAIN_THREAD = "Main Thread"
# CalendarFrameDrive.swift moves the grid for four seconds per axis; the line closing a burst is
# written a couple of frames after the last move.
BURST_SECONDS = 4.0
# Symbols present in nearly every busy sample are the run loop and the app's entry point: true of
# every phase, so they say nothing about this one.
SCAFFOLDING_SHARE = 0.9

FRAMES_LINE = re.compile(
    r"^(?P<stamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}) .*calendar frames: \d+ in motion"
    r".*inputs: days (?P<days>\d+), hours (?P<hours>\d+), zoom (?P<zoom>\d+)"
)


@dataclass(frozen=True)
class Sample:
    seconds: float
    frames: tuple[str, ...]
    """Innermost first, as the export lists them."""
    in_app: tuple[bool, ...]
    """Per frame: whether it is in the app's own executable."""
    weight_ms: float = 1.0
    """The time the sample stands for: the profiler's sampling interval."""

    @property
    def busy(self) -> bool:
        return bool(self.frames) and self.frames[0] != IDLE_LEAF


@dataclass(frozen=True)
class Phase:
    name: str
    start: float
    end: float


def load_samples(path: Path | str, thread: str = MAIN_THREAD) -> list[Sample]:
    """Every sample `thread` took, in order; a row with no backtrace is skipped."""
    root = ET.parse(path).getroot()
    by_id = {e.get("id"): e for e in root.iter() if e.get("id") is not None}

    def resolve(element: ET.Element) -> ET.Element:
        ref = element.get("ref")
        return by_id[ref] if ref is not None else element

    app_binary = _app_binary(root, resolve)
    samples = []
    for row in root.iter("row"):
        thread_el = row.find("thread")
        time_el = row.find("sample-time")
        trace_el = row.find("tagged-backtrace")
        weight_el = row.find("weight")
        if thread_el is None or time_el is None or trace_el is None:
            continue
        if not resolve(thread_el).get("fmt", "").startswith(thread):
            continue
        frames, in_app = [], []
        for frame in resolve(trace_el).findall("frame"):
            frame = resolve(frame)
            frames.append(frame.get("name") or frame.get("addr") or "?")
            binary = frame.find("binary")
            in_app.append(binary is not None and resolve(binary).get("name") == app_binary)
        seconds = int(resolve(time_el).text or 0) / 1e9
        weight = int(resolve(weight_el).text or 0) / 1e6 if weight_el is not None else 1.0
        samples.append(Sample(seconds, tuple(frames), tuple(in_app), weight))
    return samples


def _app_binary(root: ET.Element, resolve) -> str | None:
    """The recorded process's executable: the binary named after the process in its first row."""
    for process in root.iter("process"):
        name = resolve(process).get("fmt", "")
        if name:
            return name.split(" (")[0]
    return None


def trace_start(toc_text: str) -> datetime:
    """When the recording began, from `xctrace export --toc`."""
    match = re.search(r"<start-date>([^<]+)</start-date>", toc_text)
    if not match:
        raise ValueError("the table of contents carries no start-date")
    return datetime.fromisoformat(match.group(1))


def phases_from_log(lines: list[str], start: datetime) -> list[Phase]:
    """One phase per "calendar frames" line that a scripted axis drove, in trace seconds.

    A line whose inputs are all zero is the grid settling on its own (the calendar opening, a
    page landing): no axis was driven, so it is not a phase. One axis can close more than one
    line, when the drive holds the grid still for longer than the trace waits (the hours clamp at
    the top of the day), so consecutive lines on one axis are one phase ending at the last. The
    log is written in local time, which is the recording's zone on the same machine.
    """
    phases = []
    for line in lines:
        match = FRAMES_LINE.match(line)
        if not match:
            continue
        inputs = {axis: int(match.group(axis)) for axis in ("days", "hours", "zoom")}
        axis, count = max(inputs.items(), key=lambda item: item[1])
        if count == 0:
            continue
        stamp = datetime.strptime(match.group("stamp"), "%Y-%m-%d %H:%M:%S.%f")
        end = (stamp.replace(tzinfo=start.tzinfo) - start).total_seconds()
        if phases and phases[-1].name == axis:
            phases.pop()
        phases.append(Phase(axis, max(end - BURST_SECONDS, 0.0), end))
    return phases


def busy_ms(samples: list[Sample]) -> float:
    return sum(s.weight_ms for s in samples if s.busy)


def busy_per_second(samples: list[Sample]) -> dict[int, float]:
    """Milliseconds of each second of the recording the main thread spent busy.

    Counted from the busy samples' weights, never from the share of samples: a thread blocked in
    the run loop is mostly not sampled at all, so busy samples are nearly all of its samples
    whether it worked for a tenth of the second or all of it.
    """
    if not samples:
        return {}
    per: Counter[int] = Counter()
    for s in samples:
        if s.busy:
            per[int(s.seconds)] += s.weight_ms
    return {second: per.get(second, 0.0) for second in range(int(samples[-1].seconds) + 1)}


def top_symbols(samples: list[Sample], top: int) -> tuple[list[tuple[str, int]], list[tuple[str, int]]]:
    """The busy samples' innermost frames, and the app's own frames counted once per sample.

    The second list leaves out what nearly every sample carries (`main`, the run loop's entry):
    it is the same in every phase and would push the frames that differ off the list.
    """
    busy = [s for s in samples if s.busy]
    leaf = Counter(s.frames[0] for s in busy)
    inclusive: Counter[str] = Counter()
    for s in busy:
        inclusive.update({name for name, own in zip(s.frames, s.in_app) if own})
    floor = len(busy) * SCAFFOLDING_SHARE
    ranked = [(name, n) for name, n in inclusive.most_common() if n < floor or len(busy) < 10]
    return leaf.most_common(top), ranked[:top]


def summarise(samples: list[Sample], phases: list[Phase], top: int) -> str:
    out = []
    per_second = busy_per_second(samples)
    out.append("main thread busy, ms of each second of the recording:")
    out.append("  " + " ".join(f"{second}s:{ms:.0f}" for second, ms in per_second.items()))
    if not phases:
        out.append("no scripted phase in the log slice: was the run started in drive mode?")
    for phase in phases:
        window = [s for s in samples if phase.start <= s.seconds < phase.end]
        length_ms = (phase.end - phase.start) * 1000
        busy = busy_ms(window)
        out.append("")
        out.append(
            f"{phase.name}  {phase.start:.1f}s to {phase.end:.1f}s: main thread busy "
            f"{busy:.0f} ms of {length_ms:.0f} ({100 * busy / length_ms:.0f}%)"
        )
        leaf, inclusive = top_symbols(window, top)
        out.append("  innermost frame:")
        out.extend(f"    {n:6d}  {_short(name)}" for name, n in leaf)
        out.append("  app frames on the stack:")
        out.extend(f"    {n:6d}  {_short(name)}" for name, n in inclusive)
    return "\n".join(out)


def _short(name: str, width: int = 140) -> str:
    return name if len(name) <= width else name[: width - 1] + "…"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("profile", type=Path, help="the exported time-profile table")
    parser.add_argument("--toc", type=Path, required=True, help="`xctrace export --toc` output")
    parser.add_argument("--log", type=Path, required=True, help="the run's slice of mailcal.log")
    parser.add_argument("--top", type=int, default=12)
    args = parser.parse_args(argv)
    start = trace_start(args.toc.read_text(encoding="utf-8"))
    phases = phases_from_log(args.log.read_text(encoding="utf-8").splitlines(), start)
    samples = load_samples(args.profile)
    if not samples:
        print(f"no {MAIN_THREAD} samples in {args.profile}", file=sys.stderr)
        return 1
    print(summarise(samples, phases, args.top))
    return 0


if __name__ == "__main__":
    sys.exit(main())
