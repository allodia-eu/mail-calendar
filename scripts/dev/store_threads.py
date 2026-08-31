#!/usr/bin/env python3
"""Report how the engine actually threaded a set of messages.

Answers "why is this conversation split into separate rows?" from the store rather than from the
UI. The clients group a list row by `(account, thread_id)` (see `mailcal-viewmodel::view_rows`),
so if two messages carry different `thread_id`s they *cannot* render as one conversation, no
matter what the view-model does. This prints, per matching message: its `thread_id`, its own
`Message-ID`, and the ids it references; which is exactly the input the engine's union-find
threading pass consumes (`engine-sync::threading`).

Read the output as: messages sharing a `thread_id` are one conversation. If two messages share a
referenced id but *not* a `thread_id`, the derivation pass never saw them together.

Prints headers only; never message bodies. Reads DB (snapshot path) and NEEDLE (a case-insensitive
subject substring; empty matches all) from the environment.
"""

import collections
import json
import os
import sqlite3
import sys

MAX_ROWS = 200


def as_list(value) -> list[str]:
    """Envelope id headers are lists; tolerate a bare string or a missing key."""
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    return [str(v) for v in value]


def main() -> int:
    db = os.environ["DB"]
    needle = os.environ.get("NEEDLE", "").lower()
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)

    threads = {
        (scope, key): thread_id
        for scope, key, thread_id in conn.execute(
            "SELECT scope_key, provider_key, thread_id FROM mail_index"
        )
    }
    dates = {
        (scope, key): date
        for scope, key, date in conn.execute(
            "SELECT scope_key, provider_key, date_utc FROM mail_index"
        )
    }

    found = []
    for scope, key, payload in conn.execute("SELECT scope_key, provider_key, payload FROM object"):
        if not payload:
            continue
        try:
            obj = json.loads(payload)
        except (ValueError, TypeError):
            continue
        envelope = obj.get("envelope")
        if not isinstance(envelope, dict):
            continue  # not a mail object (calendar events share this table)
        subject = envelope.get("subject") or ""
        if needle and needle not in subject.lower():
            continue
        # `In-Reply-To` is usually repeated in `References`; dedupe but keep order, so the
        # printed line shows the reference graph rather than the header layout.
        refs = list(
            dict.fromkeys(as_list(envelope.get("in_reply_to")) + as_list(envelope.get("references")))
        )
        found.append(
            {
                "date": dates.get((scope, key)) or "",
                "subject": subject,
                "thread_id": threads.get((scope, key)),
                "message_id": as_list(envelope.get("message_id")),
                "refs": refs,
            }
        )

    if not found:
        print("no matching messages" + (f" for subject ~ {needle!r}" if needle else ""))
        return 0

    found.sort(key=lambda m: m["date"])
    truncated = len(found) > MAX_ROWS
    for msg in found[:MAX_ROWS]:
        own = msg["message_id"][0] if msg["message_id"] else "(none)"
        print(f"{msg['date']}  {msg['subject'][:60]}")
        print(f"    thread_id : {msg['thread_id']}")
        print(f"    message_id: {own}")
        print(f"    references: {', '.join(msg['refs']) or '(none)'}")
        print()

    by_thread = collections.Counter(m["thread_id"] for m in found)
    print(f"{len(found)} message(s) in {len(by_thread)} thread(s):")
    for thread_id, count in by_thread.most_common():
        print(f"  {count:>3}  {thread_id}")

    # The tell-tale: messages that reference a common id but landed in different threads.
    shared: dict[str, set] = collections.defaultdict(set)
    for msg in found:
        for ref in msg["refs"]:
            shared[ref].add(msg["thread_id"])
    split = {ref: ts for ref, ts in shared.items() if len(ts) > 1}
    if split:
        # ASCII only: this runs in a Windows console whose code page mangles non-ASCII.
        print("\nSPLIT: these referenced ids appear in more than one thread -")
        print("the derivation pass never saw those messages together (engine-sync::threading).")
        for ref, thread_ids in split.items():
            print(f"  {ref}  ->  {len(thread_ids)} threads")
    if truncated:
        print(f"\n(showing the first {MAX_ROWS} of {len(found)}: narrow the subject substring)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
