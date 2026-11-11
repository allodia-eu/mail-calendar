#!/usr/bin/env python3
"""Hold a running client's test agent to the protocol in docs/test-agent.md.

    scripts/dev/agent_conformance.py [--socket ENDPOINT] [unittest arguments]

The client runs on the in-memory demo account (`MAILCAL_DEV_ACCOUNT=demo`) with its agent on;
ENDPOINT defaults to `MAILCAL_TEST_AGENT`. The suite asks only what every agent answers and drives
only controls every client draws there, so it runs unchanged on each platform. It leaves the
app where it found it: on the mail view, the search field empty.
"""

from __future__ import annotations

import contextlib
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from agent import Agent, Refused, run  # noqa: E402

REQUIRED_VERBS = {
    "verbs", "ping", "windows", "query", "wait", "read", "dump", "activate", "set-text", "toggle",
    "select", "focus", "settle", "screenshot", "quit",
}
NODE_FIELDS = {"h", "type", "role", "showing", "enabled"}
# A handle no agent hands out within one run.
NEVER_ISSUED = 2**31 - 1
# The view switcher's two buttons, and a control only the calendar view draws.
MAIL, CALENDAR, CALENDAR_ONLY = "Mail", "Calendar", "New Event"
# A button every client draws on the mail view, a phone's included, which has no Sync.
NEW_MAIL = "New Mail"

ENDPOINT = os.environ.get("MAILCAL_TEST_AGENT", "")


class Conformance(unittest.TestCase):
    agent: Agent

    @classmethod
    def setUpClass(cls) -> None:
        if not ENDPOINT:
            raise unittest.SkipTest("no endpoint: pass --socket or set MAILCAL_TEST_AGENT")
        cls.agent = Agent(ENDPOINT)

    def ask(self, **request: Any) -> dict[str, Any]:
        return self.agent.ask(request)

    def refusal(self, **request: Any) -> dict[str, Any]:
        with self.assertRaises(Refused, msg=f"{request} was answered") as refused:
            if request.get("verb") in ("press", "fill"):
                run(self.agent, dict(request))
            else:
                self.agent.ask(request)
        reply = refused.exception.reply
        self.assertIs(reply.get("ok"), False, reply)
        self.assertIsInstance(reply.get("error"), str, reply)
        return reply

    def press(self, **filter_: Any) -> dict[str, Any]:
        return run(self.agent, {"verb": "press", **filter_})

    # The wire.

    def test_every_required_verb_is_listed_and_described(self) -> None:
        verbs = {entry["verb"]: entry.get("about", "") for entry in self.ask(verb="verbs")["verbs"]}
        self.assertLessEqual(REQUIRED_VERBS, set(verbs), "missing verbs")
        for verb in REQUIRED_VERBS:
            self.assertTrue(verbs[verb], f"{verb} has no description")

    def test_ping_names_the_process(self) -> None:
        reply = self.ask(verb="ping")
        self.assertIs(reply["ok"], True)
        self.assertIsInstance(reply["pid"], int)

    def test_a_request_that_names_no_verb_is_refused(self) -> None:
        self.assertEqual(self.refusal(text="Sync")["code"], "bad-request")

    def test_an_unknown_verb_is_refused_by_its_own_code(self) -> None:
        self.assertEqual(self.refusal(verb="fly")["code"], "unknown-verb")

    def test_a_misspelt_argument_is_refused_rather_than_ignored(self) -> None:
        self.assertEqual(self.refusal(verb="query", txt="Sync")["code"], "bad-request")
        self.assertEqual(self.refusal(verb="ping", text="Sync")["code"], "bad-request")

    # Finding.

    def test_a_window_is_listed(self) -> None:
        windows = self.ask(verb="windows")["windows"]
        self.assertTrue(windows)
        for window in windows:
            self.assertIn("h", window)
            self.assertIn("title", window)

    def test_an_element_is_described_the_same_way_everywhere(self) -> None:
        (button,) = self.ask(verb="wait", text=NEW_MAIL)["elements"]
        self.assertLessEqual(NODE_FIELDS, set(button), button)
        self.assertIn(NEW_MAIL, button.get("text", []) + [button.get("tooltip")])
        self.assertIs(button["showing"], True)
        bounds = button["bounds"]
        self.assertEqual(len(bounds), 4)
        self.assertGreater(bounds[2] * bounds[3], 0, "laid out with no area")
        self.assertEqual(self.ask(verb="read", h=button["h"])["h"], button["h"])

    def test_a_filter_that_matches_nothing_answers_an_empty_list(self) -> None:
        self.assertEqual(self.ask(verb="query", text="No such control anywhere")["elements"], [])

    def test_a_wait_that_runs_out_says_what_was_near(self) -> None:
        started = time.monotonic()
        reply = self.refusal(verb="wait", text="Synk", timeoutMs=300)
        self.assertEqual(reply["code"], "timeout")
        self.assertLess(time.monotonic() - started, 5, "the wait outlived its timeout")
        self.assertIn("near", reply)

    def test_a_wait_for_absence_returns_at_once_when_nothing_is_there(self) -> None:
        reply = self.ask(verb="wait", text="No such control anywhere", absent=True, timeoutMs=5000)
        self.assertEqual(reply["elements"], [])
        self.assertLess(reply["ms"], 1000)

    def test_a_handle_never_handed_out_is_refused(self) -> None:
        self.assertEqual(self.refusal(verb="read", h=NEVER_ISSUED)["code"], "no-element")
        self.assertEqual(self.refusal(verb="activate", h=NEVER_ISSUED)["code"], "no-element")

    def test_the_tree_is_dumped_as_text(self) -> None:
        text = self.ask(verb="dump")["text"]
        self.assertIn(NEW_MAIL, text)

    # Acting as a person would, and only as a person could.

    def test_the_view_switcher_changes_the_view(self) -> None:
        try:
            self.press(text=CALENDAR, **switcher())
            self.ask(verb="wait", text=CALENDAR_ONLY)
        finally:
            self.press(**back_to_mail(self.agent))
        self.ask(verb="wait", text=NEW_MAIL)
        self.ask(verb="wait", text=CALENDAR_ONLY, absent=True)

    def test_text_set_in_the_search_field_reads_back(self) -> None:
        field = search_field(self.agent)
        if field is None:
            # An iPhone folds its search field away under the title once the view has changed, and
            # only a pull on the list brings it back: a gesture, which no agent fakes.
            self.skipTest("no search field on screen")
        try:
            self.ask(verb="set-text", h=field, text="invoice")
            self.assertIn("invoice", str(self.ask(verb="read", h=field).get("value")))
        finally:
            self.ask(verb="set-text", h=field, text="")
            leave_the_field(self.agent, field)

    def test_text_cannot_be_set_on_something_that_is_not_a_field(self) -> None:
        (button,) = self.ask(verb="wait", text=NEW_MAIL)["elements"]
        self.assertEqual(self.refusal(verb="set-text", h=button["h"], text="x")["code"], "not-editable")

    def test_a_disabled_control_is_refused(self) -> None:
        disabled = [
            element
            for element in self.ask(verb="query", enabled=False)["elements"]
            if element.get("text") or element.get("tooltip") or element.get("icon")
        ]
        if not disabled:
            self.skipTest("nothing on screen is disabled")
        for element in disabled[:3]:
            self.assertIs(element["enabled"], False)
            self.assertEqual(self.refusal(verb="activate", h=element["h"])["code"], "disabled")

    # Waiting and looking.

    def test_settle_reports_how_long_it_took(self) -> None:
        reply = self.ask(verb="settle", quietMs=100, capMs=5000)
        self.assertIsInstance(reply["settled"], bool)
        self.assertLessEqual(reply["ms"], 6000)

    def test_a_screenshot_is_drawn_by_the_app(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = os.path.join(directory, "window.png")
            reply = self.ask(verb="screenshot", path=path)
            self.assertGreater(reply["width"], 0)
            self.assertGreater(reply["height"], 0)
            with open(path, "rb") as image:
                self.assertEqual(image.read(8), b"\x89PNG\r\n\x1a\n")


def switcher() -> dict[str, Any]:
    """What tells the view switcher's buttons apart from a heading with the same words."""
    return {"role": "toggle-button"} if sys.platform.startswith("linux") else {}


def back_to_mail(agent: Agent) -> dict[str, Any]:
    """What leaves the calendar for mail: the switcher's Mail button, or on macOS, whose sidebar is
    the switcher and names no Mail, the unified Inbox at the top of it."""
    for filter_ in ({"text": MAIL, **switcher()}, {"text": "Inbox", "index": 0}):
        if agent.ask({"verb": "query", **filter_})["elements"]:
            return filter_
    raise AssertionError("nothing on screen leads back to mail")


def leave_the_field(agent: Agent, field: int) -> None:
    """Ends the search that typing into the field began, where the platform makes one of it: an
    iPhone's search hides the bar it was opened from until it is dismissed. An agent offering
    `escape` dismisses it as VoiceOver does; one refusing has nothing to dismiss."""
    if any(entry["verb"] == "escape" for entry in agent.ask({"verb": "verbs"})["verbs"]):
        with contextlib.suppress(Refused):
            agent.ask({"verb": "escape", "h": field})
        agent.ask({"verb": "settle"})


def search_field(agent: Agent) -> int | None:
    for role in ("search-box", "edit", "AXTextField", "AXSearchField"):
        elements = agent.ask({"verb": "query", "role": role})["elements"]
        if elements:
            return int(elements[0]["h"])
    return None


def main(argv: list[str]) -> int:
    global ENDPOINT
    if argv[:1] == ["--socket"] and len(argv) > 1:
        ENDPOINT, argv = argv[1], argv[2:]
    program = unittest.main(module=__name__, argv=[sys.argv[0], *argv], exit=False)
    return 0 if program.result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
