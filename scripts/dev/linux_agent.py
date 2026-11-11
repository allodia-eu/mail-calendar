#!/usr/bin/env python3
"""Drive the running Linux client through its test agent (`clients/linux/src/agent.rs`).

    linux_agent.py --socket PATH <verb> [key=value ...]
    linux_agent.py --socket PATH --json '{"verb": "query", "text": "Archive"}'

`scripts/dev/control.sh linux agent <verb> ...` passes the socket of this checkout's app; use that.

Each `key=value` becomes one field of the request. A value that reads as JSON is sent as JSON
(`h=12`, `active=true`, `has={"text":"Inbox"}`); anything else is sent as text (`text=Archive`).
A field that only ever holds text is sent as typed, so `text=2024` looks for the label "2024".

Besides the app's own verbs (`verbs` lists them), two do what a person means in one call:

    press <filter>              find exactly one element and activate it
    fill <filter> value=TEXT    find exactly one element and set its text

Both refuse to guess: no match, or more than one, is an error that lists what was there.

The reply is printed as JSON; `dump` prints its tree as text. The exit status is 0 for an answer
and 1 for a refusal, whose `code` is on the first line of stderr, so a script can branch on it.
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
from typing import Any

TIMEOUT_SECONDS = 120

# The fields whose value is always text: read as JSON, `text=3` would be a number the app refuses
# and `value=true` would be typed in as "True".
TEXT_FIELDS = frozenset(
    {"text", "textLike", "type", "name", "icon", "role", "window", "value", "script", "path", "target"}
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


class Agent:
    def __init__(self, path: str) -> None:
        self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.connection.settimeout(TIMEOUT_SECONDS)
        try:
            self.connection.connect(path)
        except OSError as error:
            raise SystemExit(
                f"error: no app answers on {path} ({error.strerror or error}).\n"
                "  Start one with: clients/linux/build-and-run.sh --headless --detach"
            ) from error
        self.replies = self.connection.makefile("r", encoding="utf-8")

    def ask(self, request: dict[str, Any]) -> dict[str, Any]:
        self.connection.sendall((json.dumps(request) + "\n").encode())
        line = self.replies.readline()
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


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--socket", required=True, help="the socket the app listens on")
    parser.add_argument("--json", help="the whole request as one JSON object")
    parser.add_argument("verb", nargs="?")
    parser.add_argument("pairs", nargs="*", metavar="key=value")
    args = parser.parse_args(argv)
    if args.json:
        request = json.loads(args.json)
    elif args.verb:
        request = request_from(args.verb, args.pairs)
    else:
        parser.error("name a verb, or pass --json")

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
