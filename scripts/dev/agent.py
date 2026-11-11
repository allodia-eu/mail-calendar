#!/usr/bin/env python3
"""Drive a running client through its test agent (docs/test-agent.md).

    agent.py [--socket ENDPOINT] <verb> [key=value ...]
    agent.py [--socket ENDPOINT] --json '{"verb": "query", "text": "Archive"}'

ENDPOINT is what the app was launched with in MAILCAL_TEST_AGENT, and defaults to it: a Unix
socket's path, or on Windows a named pipe's name. `scripts/dev/control.sh linux agent <verb> ...`
passes the socket of this checkout's Linux app.

Each `key=value` becomes one field of the request. A value that reads as JSON is sent as JSON
(`h=12`, `active=true`, `has={"text":"Inbox"}`); anything else is sent as text (`text=Archive`).
A field that only ever holds text is sent as typed, so `text=2024` looks for the label "2024".

Besides the app's own verbs (`verbs` lists them), two do what a person means in one call:

    press <filter>              find exactly one element and activate it
    fill <filter> value=TEXT    find exactly one element and set its text

Both refuse to guess: no match, or more than one, is an error that lists what was there.

The reply is printed as JSON; `dump` prints its tree as text. The exit status is 0 for an answer
and 1 for a refusal, whose `code` is on the first line of stderr, so a script can branch on it.

    agent.py [--socket ENDPOINT] --lines

keeps one connection open for a script that asks many times: each line on stdin is one request
(JSON, `press` and `fill` included) and each answer is one line of JSON on stdout, a refusal
included, so the caller reads `ok` and `code` rather than an exit status. It connects at the first
request, waiting up to `--connect-ms` for an app that is still starting, and again after an app
that hung up: a script that relaunches the app keeps the same process. Two codes are its own:
`no-app` when nothing answered in time, `hung-up` when the app went before it answered.
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time
from typing import Any, BinaryIO, TextIO

TIMEOUT_SECONDS = 120
# How long `--lines` waits for an app to start answering, by default.
CONNECT_MS = 60_000

# The fields whose value is always text: read as JSON, `text=3` would be a number the app refuses
# and `value=true` would be typed in as "True".
TEXT_FIELDS = frozenset(
    {
        "text", "textLike", "type", "id", "name", "icon", "role", "window", "value", "script",
        "path", "target",
    }
)


class Refused(Exception):
    def __init__(self, reply: dict[str, Any]) -> None:
        super().__init__(reply.get("error", "refused"))
        self.reply = reply


def parse_value(text: str) -> Any:
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return text


def request_from(verb: str, pairs: list[str]) -> dict[str, Any]:
    request: dict[str, Any] = {"verb": verb}
    for pair in pairs:
        key, separator, value = pair.partition("=")
        if not separator or not key:
            raise SystemExit(f"error: '{pair}' is not key=value")
        request[key] = value if key in TEXT_FIELDS else parse_value(value)
    return request


def connect(endpoint: str) -> BinaryIO:
    """A byte stream to the agent: a named pipe on Windows, a Unix socket everywhere else."""
    if os.name == "nt":
        name = endpoint if endpoint.startswith("\\\\") else rf"\\.\pipe\{endpoint}"
        return open(name, "r+b", buffering=0)  # noqa: SIM115: closed with the Agent
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(TIMEOUT_SECONDS)
    connection.connect(endpoint)
    return connection.makefile("rwb", buffering=0)


class Agent:
    def __init__(self, endpoint: str) -> None:
        try:
            self.stream = connect(endpoint)
        except OSError as error:
            raise SystemExit(
                f"error: no app answers on {endpoint} ({error.strerror or error}).\n"
                "  Start one with its build-and-run script; on Linux: "
                "clients/linux/build-and-run.sh --headless"
            ) from error

    def ask(self, request: dict[str, Any]) -> dict[str, Any]:
        self.stream.write((json.dumps(request) + "\n").encode())
        line = self.stream.readline().decode("utf-8")
        if not line:
            raise SystemExit("error: the app hung up before it answered")
        reply = json.loads(line)
        if not reply.get("ok"):
            raise Refused(reply)
        return reply

    def one(self, filter_: dict[str, Any]) -> dict[str, Any]:
        """The single element a filter names, waiting for it as `wait` does."""
        reply = self.ask({"verb": "wait", **filter_})
        elements = reply["elements"]
        if len(elements) != 1:
            raise Refused(
                {
                    "ok": False,
                    "code": "ambiguous",
                    "error": f"{len(elements)} elements match: narrow the filter "
                    "(type=, role=, window=, under=, has=) or pick one with index=",
                    "elements": elements,
                }
            )
        return elements[0]


def run(agent: Agent, request: dict[str, Any]) -> dict[str, Any]:
    verb = request.pop("verb")
    if verb == "press":
        element = agent.one(request)
        return {**agent.ask({"verb": "activate", "h": element["h"]}), "element": element}
    if verb == "fill":
        if "value" not in request:
            raise SystemExit("error: fill needs value=TEXT")
        text = str(request.pop("value"))
        submit = bool(request.pop("submit", False))
        element = agent.one(request)
        reply = agent.ask({"verb": "set-text", "h": element["h"], "text": text, "activate": submit})
        return {**reply, "element": element}
    return agent.ask({"verb": verb, **request})


class HungUp(Exception):
    """The app closed the connection before it answered."""


def reconnecting(endpoint: str, connect_ms: int) -> BinaryIO | None:
    """A stream to the app, retried while it is not listening yet; None when it never was."""
    deadline = time.monotonic() + connect_ms / 1000
    while True:
        try:
            return connect(endpoint)
        except OSError:
            if time.monotonic() >= deadline:
                return None
            time.sleep(0.05)


class LineAgent(Agent):
    """An `Agent` whose connection a `serve_lines` loop can drop and make again."""

    def __init__(self, stream: BinaryIO) -> None:
        self.stream = stream

    def ask(self, request: dict[str, Any]) -> dict[str, Any]:
        try:
            self.stream.write((json.dumps(request) + "\n").encode())
            line = self.stream.readline()
        except OSError as error:
            raise HungUp from error
        if not line:
            raise HungUp
        reply = json.loads(line.decode("utf-8"))
        if not reply.get("ok"):
            raise Refused(reply)
        return reply


def serve_lines(endpoint: str, source: TextIO, sink: TextIO, connect_ms: int = CONNECT_MS) -> int:
    """Answers each request line from `source` with one reply line on `sink`, until `source` ends."""
    agent: LineAgent | None = None
    for line in source:
        if not line.strip():
            continue
        try:
            request = json.loads(line)
            if not isinstance(request, dict) or not isinstance(request.get("verb"), str):
                raise ValueError("a request is a JSON object with a verb")
        except ValueError as error:
            reply: dict[str, Any] = {"ok": False, "code": "bad-request", "error": str(error)}
        else:
            if agent is None:
                stream = reconnecting(endpoint, connect_ms)
                agent = None if stream is None else LineAgent(stream)
            if agent is None:
                reply = {
                    "ok": False,
                    "code": "no-app",
                    "error": f"no app answered on {endpoint} within {connect_ms} ms",
                }
            else:
                try:
                    reply = run(agent, request)
                except Refused as refusal:
                    reply = refusal.reply
                except HungUp:
                    agent = None
                    reply = {"ok": False, "code": "hung-up", "error": "the app went before it answered"}
                except SystemExit as error:
                    reply = {"ok": False, "code": "bad-request", "error": str(error)}
        # `ok` first, so a shell script can tell an answer from a refusal by its first bytes.
        reply = {"ok": bool(reply.get("ok")), **{k: v for k, v in reply.items() if k != "ok"}}
        sink.write(json.dumps(reply, ensure_ascii=False) + "\n")
        sink.flush()
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--socket",
        default=os.environ.get("MAILCAL_TEST_AGENT"),
        help="where the app listens (default: $MAILCAL_TEST_AGENT)",
    )
    parser.add_argument("--json", help="the whole request as one JSON object")
    parser.add_argument(
        "--lines", action="store_true", help="answer one JSON request per stdin line, until it ends"
    )
    parser.add_argument(
        "--connect-ms",
        type=int,
        default=CONNECT_MS,
        help="with --lines, how long to wait for an app to start answering",
    )
    parser.add_argument("verb", nargs="?")
    parser.add_argument("pairs", nargs="*", metavar="key=value")
    args = parser.parse_args(argv)
    if args.lines:
        if not args.socket:
            parser.error("name the app's endpoint with --socket, or set MAILCAL_TEST_AGENT")
        return serve_lines(args.socket, sys.stdin, sys.stdout, args.connect_ms)
    if args.json:
        request = json.loads(args.json)
    elif args.verb:
        request = request_from(args.verb, args.pairs)
    else:
        parser.error("name a verb, or pass --json")

    if not args.socket:
        parser.error("name the app's endpoint with --socket, or set MAILCAL_TEST_AGENT")
    verb = request.get("verb")
    agent = Agent(args.socket)
    try:
        reply = run(agent, request)
    except Refused as refusal:
        reply = refusal.reply
        print(reply.get("code", "refused"), file=sys.stderr)
        print(f"error: {reply.get('error')}", file=sys.stderr)
        for key in ("near", "elements", "element", "actions"):
            if reply.get(key):
                print(f"{key}: {json.dumps(reply[key], indent=2, ensure_ascii=False)}", file=sys.stderr)
        return 1
    if verb == "dump" and "text" in reply:
        sys.stdout.write(reply["text"])
    else:
        reply.pop("ok", None)
        print(json.dumps(reply, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
