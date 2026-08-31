#!/usr/bin/env python3
"""Turn `idb ui describe-all` output into the grep-able surface the macOS AX dump already gives.

`scripts/dev/control.sh iphone|ipad` is the entry point; see there for the idb half (which
simulator, which companion). This file reads the JSON on stdin and knows nothing about idb, so the
selection rules are testable without a simulator: `scripts/dev/tests/test_ios_ui_idb.py`.

Why it exists: driving the iOS simulator used to mean screenshot -> read the picture -> work out
where the control is -> convert device pixels to screen coordinates -> tap -> screenshot again, at
a vision round-trip each. `describe-all` reports every on-screen element's role, label and frame,
and those frames are in POINTS -- the same coordinate space `idb ui tap` takes -- so a label
resolves straight into a tap with no scaling and no window arithmetic.

The read half is the valuable one, and not only for tokens. A screenshot cannot tell you that a
card collapsed its children into a single accessibility node: the buttons are still right there in
the picture, and VoiceOver can no longer reach them. The tree says so. That is a real bug this repo
shipped (an invitation card's Accept/Maybe/Decline, fixed in 1636061) and it is why an Apple
verification run should assert on nodes, not pixels.

Output mirrors `scripts/dev/macos-ax.swift` line for line, so a grep written for one platform ports
to the other:

    AXButton [220,700] Accept this invitation

with one deliberate difference: `describe-all` returns a FLAT list of the elements currently on
screen rather than a hierarchy, so there is no indent to reproduce.

KNOWN GAP -- `describe-all` does not enumerate navigation-bar / toolbar items. The bar arrives as
one unlabelled container (`AXGroup [220,89]`) and its contents are simply absent, in both the flat
and `--nested` formats. They ARE in the accessibility tree and VoiceOver reaches them: probing the
same pixel with `describe-point` returns a fully labelled `AXButton 'Compose'`. It is idb's
enumeration that stops, not the app's tree, and it stops the same way in Apple's own Settings app
-- so an absent Compose/Send/Cancel in a dump says nothing about the app. Use `probe <x> <y>` (or a
`MAILCAL_*` launch hook, or a plain `tap`) for anything in the top bar, and never read a missing
toolbar item as a finding.

Exit codes: 0 ok - 1 usage / no match.
"""

from __future__ import annotations

import json
import sys
from typing import Any

# Keep a dump readable when a body paragraph is the label; the macOS dump truncates at the same
# width, and `find --all` at the same 80.
LABEL_WIDTH = 100
ALL_LABEL_WIDTH = 80


def label(node: dict[str, Any]) -> str:
    """Return a node's identifying text, on exactly one line.

    UIKit and SwiftUI spread it over several attributes: a button carries `AXLabel`, a text field
    carries its content in `AXValue` and its placeholder in `AXLabel`. Joined like the macOS dump
    joins title/value/description, so `find` matches on either half.

    Whitespace is collapsed because a mail list row's label is the message preview, newlines and
    all; and a node that prints across two lines silently breaks every grep written against this
    dump, which is the only reason the dump exists.
    """
    parts = (node.get("AXLabel"), node.get("AXValue"), node.get("title"))
    return " | ".join(" ".join(str(part).split()) for part in parts if part)


def center(node: dict[str, Any]) -> tuple[int, int]:
    """Return the centre of a node's frame, in points -- what `idb ui tap` consumes unchanged."""
    frame = node.get("frame") or {}
    x = float(frame.get("x", 0)) + float(frame.get("width", 0)) / 2
    y = float(frame.get("y", 0)) + float(frame.get("height", 0)) / 2
    return int(x), int(y)


def dump(nodes: list[dict[str, Any]] | dict[str, Any]) -> int:
    """Print every on-screen node as `Role [x,y] Label`.

    Accepts a bare object too, so `describe-point` (one node) formats through the same path as
    `describe-all` (a list) and `probe` reads like one line of a dump.
    """
    if isinstance(nodes, dict):
        nodes = [nodes]
    for node in nodes:
        x, y = center(node)
        print(f"{node.get('role') or '?'} [{x},{y}] {label(node)[:LABEL_WIDTH]}".rstrip())
    return 0


def find(nodes: list[dict[str, Any]], needle: str, show_all: bool) -> int:
    """Print `<x> <y>` for the node whose label best matches `needle`, so it pipes into a tap.

    Ranked, not first-or-last-wins: a plain substring search for "Reply" also hits "Reply all", and
    silently tapping the wrong one is the kind of bug that invalidates a whole verification run. An
    exact label wins, then the shortest label containing the needle -- the same tie-break
    `macos-ax.swift` uses, so a flow ported between the two platforms picks the same node.
    """
    wanted = needle.lower()
    hits = [(label(node), center(node)) for node in nodes]
    hits = [hit for hit in hits if wanted in hit[0].lower()]
    if not hits:
        sys.stderr.write(f"error: no element matching '{needle}'\n")
        return 1

    if show_all:
        for text, (x, y) in hits:
            print(f"{x} {y}  {text[:ALL_LABEL_WIDTH]}")
        return 0

    text, (x, y) = min(hits, key=lambda hit: (hit[0].lower() != wanted, len(hit[0])))
    print(f"{x} {y}")
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        sys.stderr.write("usage: ios_ui_idb.py <dump|find> [args...]  # describe-all JSON on stdin\n")
        return 1
    action, rest = argv[0], argv[1:]
    nodes = json.load(sys.stdin)
    if action == "dump":
        return dump(nodes)
    if action == "find":
        if not rest:
            sys.stderr.write("error: find <text> [--all]\n")
            return 1
        return find(nodes, rest[0], "--all" in rest)
    sys.stderr.write(f"error: unknown action '{action}' (dump|find)\n")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
