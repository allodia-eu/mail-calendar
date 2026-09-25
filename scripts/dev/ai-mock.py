#!/usr/bin/env python3
"""A canned OpenAI-compatible endpoint for driving Writing style without spending credits.

Set it up in a debug build under Settings → Advanced → Own AI endpoint as
`http://127.0.0.1:28434/v1`, any model name, "In the EU, run by a European company", then learn a
style from the harness account's Sent Items and draft a reply. Every answer is fixed, so a UI test
can assert on it:

- a request whose `emit_json` tool asks for a `reply` (a draft) gets a reply with one `[date]` gap,
  in Dutch when the instructions ask for the reply in Dutch and in English otherwise, with a
  summary and two tasks, one to attach and one to do, in Dutch when the app is shown in Dutch;
- any other request that forces the tool (learning) gets one description, whichever language or
  part it asked about, with the messages numbered 1 to 3 as the passages.

An Android emulator reaches it after `adb reverse tcp:28434 tcp:28434`, because the app accepts
plain HTTP only to this device. It listens on loopback only and prints one line per request,
never what the request carried.

    scripts/dev/ai-mock.py [--port 28434] [--delay SECONDS] [--status CODE] [--key KEY]

`--delay` holds every answer so the progress states can be seen; `--status` answers every request
with that HTTP status instead (402 out of credits, 429 rate limited, 500); `--key` refuses any
other bearer with 401.
"""

import argparse
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STYLE = {
    "style": {
        "greetings": [{"text": "Hi Bob,", "share": 80}, {"text": "Hi,", "share": 20}],
        "sign_offs": [{"text": "Cheers,", "share": 90}],
        "signs_as": "Alice",
        "register": "Friendly and direct, on first-name terms.",
        "register_headline": "Friendly and direct",
        "typical_words": 70,
        "typical_paragraphs": 2,
        "shape": "Two or three short paragraphs.",
        "punctuation": "Plain; the odd exclamation mark when pleased.",
        "structure": "Thanks first, then the answer, then the next step.",
        "structure_headline": "Thanks, answer, next step",
        "moves": "Says no politely with a reason and offers an alternative.",
        "moves_headline": "A reasoned no, with options",
        "phrases": ["Thanks for", "I'll", "Let me know"],
        "avoid": ["Dear", "Kind regards"],
    },
    "exemplars": [1, 2, 3],
}

DRAFTS = {
    "en": "Hi,\n\nThanks for your message. That works for me; shall we say [date]? "
    "I'll put it in the calendar once you confirm.\n\nCheers,\nAlice",
    "nl": "Hoi,\n\nBedankt voor je bericht. Dat komt me goed uit; zullen we [date] afspreken? "
    "Ik zet het in de agenda zodra je het bevestigt.\n\nGroetjes,\nAlice",
}

SUMMARIES = {
    "en": "The sender asks whether a meeting suits you, and on which date.",
    "nl": "De afzender vraagt of een afspraak je uitkomt, en op welke datum.",
}

TASKS = {
    "en": [
        {"kind": "attach", "text": "Attach the agenda for the meeting"},
        {"kind": "do", "text": "Put the meeting in the calendar once it is confirmed"},
    ],
    "nl": [
        {"kind": "attach", "text": "Voeg de agenda voor de afspraak toe"},
        {"kind": "do", "text": "Zet de afspraak in de agenda zodra die bevestigd is"},
    ],
}

USAGE = {"prompt_tokens": 1000, "completion_tokens": 100, "total_tokens": 1100}


def answer(request):
    """The canned chat completion for one request body."""
    tool = (request.get("tools") or [{}])[0].get("function", {})
    if "reply" in tool.get("parameters", {}).get("properties", {}):
        system = next(
            (m.get("content") or "" for m in request.get("messages", []) if m.get("role") == "system"),
            "",
        )
        language = "nl" if "Write the reply in Dutch" in system else "en"
        interface = "nl" if "every task in Dutch" in system else "en"
        arguments = {
            "summary": SUMMARIES[interface],
            "reply": DRAFTS[language],
            "tasks": TASKS[interface],
        }
        purpose = f"draft ({language}, list in {interface})"
    else:
        arguments = STYLE
        purpose = "style"
    message = {
        "role": "assistant",
        "content": None,
        "tool_calls": [
            {
                "id": "call_1",
                "type": "function",
                "function": {"name": "emit_json", "arguments": json.dumps(arguments)},
            }
        ],
    }
    completion = {
        "id": "mock",
        "object": "chat.completion",
        "model": request.get("model", "mock"),
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
        "usage": USAGE,
    }
    return purpose, completion


class Handler(BaseHTTPRequestHandler):
    options = None

    def log_message(self, *_):
        pass

    def reply(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.endswith("/models"):
            self.reply(200, {"object": "list", "data": [{"id": "mock", "object": "model"}]})
        else:
            self.reply(404, {"error": {"message": "not found"}})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        if not self.path.endswith("/chat/completions"):
            self.reply(404, {"error": {"message": "not found"}})
            return
        options = Handler.options
        if options.key and self.headers.get("Authorization") != f"Bearer {options.key}":
            print("refused: wrong key", flush=True)
            self.reply(401, {"error": {"message": "invalid key"}})
            return
        if options.delay:
            time.sleep(options.delay)
        if options.status:
            print(f"answered {options.status} ({len(raw)} bytes asked)", flush=True)
            self.reply(options.status, {"error": {"message": f"mock status {options.status}"}})
            return
        try:
            request = json.loads(raw)
        except json.JSONDecodeError:
            self.reply(400, {"error": {"message": "not JSON"}})
            return
        purpose, completion = answer(request)
        print(f"{purpose}: {len(raw)} bytes asked", flush=True)
        self.reply(200, completion)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=28434)
    parser.add_argument("--delay", type=float, default=0.0)
    parser.add_argument("--status", type=int)
    parser.add_argument("--key")
    Handler.options = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", Handler.options.port), Handler)
    print(f"mock AI endpoint on http://127.0.0.1:{Handler.options.port}/v1", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
