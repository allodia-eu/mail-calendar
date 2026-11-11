#!/usr/bin/env python3
"""Tests for `agent.py`, the client of every app's test agent, against a stand-in agent.

The stand-in answers the way `clients/linux/src/agent.rs` does: one JSON object per line, `ok`
true or false, and a `code` on a refusal. The app's own behaviour is tested in the client crate;
these hold the client's: how a request is built, that `press` and `fill` refuse to guess, and that
a refusal exits 1 with its code first on stderr.
"""

from __future__ import annotations

import io
import json
import os
import socket
import sys
import tempfile
import threading
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from typing import Any, Callable
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import agent  # noqa: E402

ARCHIVE = {"h": 4, "type": "GtkButton", "text": ["Archive"]}
SEARCH = {"h": 9, "type": "GtkEntry", "id": "search"}


def stand_in(request: dict[str, Any]) -> dict[str, Any]:
    verb = request["verb"]
    if verb == "wait":
        if request.get("text") == "Archive":
            return {"ok": True, "elements": [ARCHIVE]}
        if request.get("id") == "search":
            return {"ok": True, "elements": [SEARCH]}
        if request.get("role") == "button":
            return {"ok": True, "elements": [ARCHIVE, {"h": 5, "type": "GtkButton"}]}
        return {"ok": False, "code": "timeout", "error": "nothing matched", "near": ["[4] GtkButton"]}
    if verb in ("activate", "set-text"):
        return {"ok": True, "how": "clicked", "seen": request}
    if verb == "dump":
        return {"ok": True, "text": "[1] GtkWindow \"Mail\" window\n"}
    return {"ok": True, "seen": request}


class Server:
    def __init__(self, answer: Callable[[dict[str, Any]], dict[str, Any]]) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, "agent.sock")
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(self.path)
        self.listener.listen()
        self.requests: list[dict[str, Any]] = []
        self.answer = answer
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self) -> None:
        try:
            connection, _ = self.listener.accept()
        except OSError:
            return  # closed by a test that never connected
        with connection, connection.makefile("rw", encoding="utf-8") as stream:
            for line in stream:
                request = json.loads(line)
                self.requests.append(request)
                stream.write(json.dumps(self.answer(request)) + "\n")
                stream.flush()

    def close(self) -> None:
        self.listener.close()
        self.dir.cleanup()


# The stand-in listens on a Unix socket, which Windows' Python does not offer; the pipe half of
# `connect` is exercised by `agent_conformance.py` against the running Windows app.
@unittest.skipUnless(hasattr(socket, "AF_UNIX"), "no Unix sockets on this host")
class AgentClient(unittest.TestCase):
    def setUp(self) -> None:
        self.server = Server(stand_in)

    def tearDown(self) -> None:
        self.server.close()

    def call(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            status = agent.main(["--socket", self.server.path, *argv])
        return status, out.getvalue(), err.getvalue()

    def test_a_value_that_reads_as_json_is_sent_as_json(self) -> None:
        self.assertEqual(
            agent.request_from("query", ["text=Archive", "h=12", "has={\"text\":\"x\"}"]),
            {"verb": "query", "text": "Archive", "h": 12, "has": {"text": "x"}},
        )

    def test_a_text_field_is_sent_as_typed(self) -> None:
        self.assertEqual(
            agent.request_from("action", ["text=2024", "value=true", "target=3"]),
            {"verb": "action", "text": "2024", "value": "true", "target": "3"},
        )

    def test_press_activates_the_one_element_it_found(self) -> None:
        status, out, _ = self.call("press", "text=Archive")
        self.assertEqual(status, 0)
        self.assertEqual(self.server.requests[-1], {"verb": "activate", "h": 4})
        self.assertEqual(json.loads(out)["element"]["h"], 4)

    def test_press_refuses_to_pick_between_two(self) -> None:
        status, _, err = self.call("press", "role=button")
        self.assertEqual(status, 1)
        self.assertEqual(err.splitlines()[0], "ambiguous")
        self.assertNotIn("activate", [request["verb"] for request in self.server.requests])

    def test_a_refusal_exits_1_with_its_code_first_and_what_was_near(self) -> None:
        status, _, err = self.call("press", "text=Nope")
        self.assertEqual(status, 1)
        self.assertEqual(err.splitlines()[0], "timeout")
        self.assertIn("[4] GtkButton", err)

    def test_fill_sets_the_text_and_can_submit(self) -> None:
        status, _, _ = self.call("fill", "id=search", "value=invoice", "submit=true")
        self.assertEqual(status, 0)
        self.assertEqual(
            self.server.requests[-1],
            {"verb": "set-text", "h": 9, "text": "invoice", "activate": True},
        )

    def test_a_dump_prints_its_tree_as_text(self) -> None:
        status, out, _ = self.call("dump")
        self.assertEqual(status, 0)
        self.assertEqual(out, "[1] GtkWindow \"Mail\" window\n")

    def test_the_endpoint_defaults_to_the_one_the_app_was_given(self) -> None:
        out = io.StringIO()
        with mock.patch.dict(os.environ, {"MAILCAL_TEST_AGENT": self.server.path}), redirect_stdout(out):
            self.assertEqual(agent.main(["ping"]), 0)
        self.assertEqual(self.server.requests[-1], {"verb": "ping"})

    def test_a_missing_app_says_how_to_start_one(self) -> None:
        with self.assertRaises(SystemExit) as raised:
            agent.main(["--socket", self.server.path + ".gone", "ping"])
        self.assertIn("build-and-run.sh", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
