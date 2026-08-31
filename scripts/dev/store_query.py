#!/usr/bin/env python3
"""Run one read-only query against a store snapshot and print it as a table.

The fallback for `scripts/dev/store.sh sql` when the sqlite3 CLI isn't on PATH; which is the
normal case on Windows, where neither the OS nor Git for Windows ships sqlite3.exe. Python's
bundled sqlite3 module needs no install.

Reads DB (snapshot path) and SQL (the query) from the environment, so no quoting round-trip.
Opened `mode=ro` on a snapshot, so it cannot touch the app's live store.
"""

import os
import sqlite3
import sys


def main() -> int:
    db = os.environ["DB"]
    sql = os.environ["SQL"]
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        cursor = conn.execute(sql)
    except sqlite3.Error as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    rows = cursor.fetchall()
    if cursor.description is None:
        return 0
    headers = [d[0] for d in cursor.description]
    widths = [
        max(len(h), *(len(str(r[i])) for r in rows)) if rows else len(h)
        for i, h in enumerate(headers)
    ]
    line = "  ".join(h.ljust(w) for h, w in zip(headers, widths))
    print(line)
    print("  ".join("-" * w for w in widths))
    for row in rows:
        print("  ".join(str(v).ljust(w) for v, w in zip(row, widths)))
    print(f"\n({len(rows)} row(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
