#!/usr/bin/env python3
"""Unit tests for the iOS simulator accessibility-tree formatter and label matcher.

No simulator and no idb: the fixtures are real `idb ui describe-all` records captured from the
iPhone client, so the selection rules can fail here rather than mid-flow on a device.
"""

from __future__ import annotations

import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import ios_ui_idb as subject


def node(role: str, x: float, y: float, width: float, height: float, **attributes: object) -> dict:
    """A describe-all record. Real ones carry float frames and null-valued attributes."""
    record: dict = {
        "role": role,
        "frame": {"x": x, "y": y, "width": width, "height": height},
        "AXLabel": None,
        "AXValue": None,
        "title": None,
    }
    record.update(attributes)
    return record


# The reading view's reply row, which is where the "Reply" / "Reply all" tie-break bites.
REPLY = node("AXButton", 24, 205, 40, 32, AXLabel="Reply")
REPLY_ALL = node("AXButton", 96, 205, 40, 32, AXLabel="Reply all")


def run(action: str, *args: str, payload: object) -> tuple[int, str]:
    """Drive the module the way control.sh does: JSON on stdin, formatted lines on stdout."""
    buffer = io.StringIO()
    stdin, sys.stdin = sys.stdin, io.StringIO(json.dumps(payload))
    try:
        with redirect_stdout(buffer):
            code = subject.main([action, *args])
    finally:
        sys.stdin = stdin
    return code, buffer.getvalue()


class LabelTests(unittest.TestCase):
    def test_joins_the_attributes_a_label_is_spread_over(self) -> None:
        # A text field carries its content in AXValue and its placeholder in AXLabel; without both,
        # every field in the composer would be unlabelled and unreachable by `find`.
        field = node("AXTextField", 16, 164, 408, 22, AXLabel="To", AXValue="bob@test.local")
        self.assertEqual(subject.label(field), "To | bob@test.local")

    def test_skips_absent_attributes(self) -> None:
        self.assertEqual(subject.label(REPLY), "Reply")

    def test_collapses_whitespace_so_a_node_stays_on_one_line(self) -> None:
        # A mail row's label is the message preview, newlines and all. A node that printed across
        # two lines would silently break every grep written against the dump.
        row = node("AXStaticText", 12, 128, 400, 20, AXLabel="First line\nsecond line\n")
        self.assertEqual(subject.label(row), "First line second line")


class CenterTests(unittest.TestCase):
    def test_returns_the_frame_centre_in_points(self) -> None:
        # Points are also what `idb ui tap` consumes, which is why a label resolves to a tap with
        # no scaling: 24 + 40/2 = 44, 205 + 32/2 = 221.
        self.assertEqual(subject.center(REPLY), (44, 221))

    def test_tolerates_a_frame_missing_fields(self) -> None:
        self.assertEqual(subject.center({"role": "AXGroup"}), (0, 0))


class DumpTests(unittest.TestCase):
    def test_prints_role_point_and_label(self) -> None:
        code, out = run("dump", payload=[REPLY])
        self.assertEqual(code, 0)
        self.assertEqual(out, "AXButton [44,221] Reply\n")

    def test_prints_one_line_per_node(self) -> None:
        multiline = node("AXStaticText", 0, 0, 100, 20, AXLabel="a\nb\nc")
        code, out = run("dump", payload=[REPLY, multiline, REPLY_ALL])
        self.assertEqual(code, 0)
        self.assertEqual(len(out.splitlines()), 3)

    def test_keeps_an_unlabelled_container_without_a_trailing_space(self) -> None:
        # The navigation bar arrives like this; idb enumerates the container but not its items,
        # so the line has to stay clean enough to grep and match on.
        code, out = run("dump", payload=[node("AXGroup", 0, 62, 440, 54)])
        self.assertEqual(out, "AXGroup [220,89]\n")

    def test_accepts_a_single_record_so_describe_point_shares_the_path(self) -> None:
        code, out = run("dump", payload=REPLY)
        self.assertEqual(code, 0)
        self.assertEqual(out, "AXButton [44,221] Reply\n")


class FindTests(unittest.TestCase):
    def test_prints_a_point_that_pipes_into_tap(self) -> None:
        code, out = run("find", "Reply", payload=[REPLY])
        self.assertEqual(code, 0)
        self.assertEqual(out, "44 221\n")

    def test_an_exact_label_beats_a_longer_containing_one(self) -> None:
        # The whole reason find is ranked: "Reply" also matches "Reply all", and tapping the wrong
        # one invalidates a verification run without ever looking wrong.
        code, out = run("find", "Reply", payload=[REPLY_ALL, REPLY])
        self.assertEqual(out, "44 221\n")

    def test_the_shortest_containing_label_wins_when_none_is_exact(self) -> None:
        longer = node("AXStaticText", 0, 300, 200, 20, AXLabel="noreply@example.com wrote")
        code, out = run("find", "repl", payload=[longer, REPLY_ALL])
        self.assertEqual(out, "116 221\n")

    def test_matching_is_case_insensitive(self) -> None:
        code, out = run("find", "rEpLy AlL", payload=[REPLY, REPLY_ALL])
        self.assertEqual(out, "116 221\n")

    def test_matches_a_value_only_node(self) -> None:
        field = node("AXTextField", 16, 198, 408, 22, AXValue="Cc")
        code, out = run("find", "Cc", payload=[field])
        self.assertEqual(out, "220 209\n")

    def test_all_lists_every_hit_in_tree_order(self) -> None:
        code, out = run("find", "Reply", "--all", payload=[REPLY, REPLY_ALL])
        self.assertEqual(code, 0)
        self.assertEqual(out.splitlines(), ["44 221  Reply", "116 221  Reply all"])

    def test_no_match_fails_rather_than_returning_a_point(self) -> None:
        # press() turns this exit code into a hard stop, so a label that is not on screen can never
        # become a tap at a stale coordinate.
        code, out = run("find", "Compose", payload=[REPLY])
        self.assertEqual(code, 1)
        self.assertEqual(out, "")


class UsageTests(unittest.TestCase):
    def test_no_action_is_an_error(self) -> None:
        self.assertEqual(subject.main([]), 1)

    def test_unknown_action_is_an_error(self) -> None:
        code, _ = run("wiggle", payload=[])
        self.assertEqual(code, 1)

    def test_find_without_a_needle_is_an_error(self) -> None:
        code, _ = run("find", payload=[])
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
