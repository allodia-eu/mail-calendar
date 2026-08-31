#!/usr/bin/env python3
"""Ask a real IMAP server what it does when a client opens too many connections.

    scripts/dev/imap-probe.py --profile soverin-test [--host imap.soverin.net] [--max 16]
    scripts/dev/imap-probe.py --profile soverin-test --mode rate

Why this exists, and why it is not an engine-adapter test. The engine's `dav` tool is the
right instrument for "what would the core get from this server", because it drives the real
adapter. This asks a different question; *what bytes does the server actually send when it
refuses*; and every adapter in the world maps those bytes onto its own error type, which is
exactly the information being thrown away. A production log showed a Soverin account failing
with `[AUTHENTICATIONFAILED] Authentication failed.` seconds after the same credential
authenticated seven times successfully, during a burst of ten simultaneous connections. Only
the wire can say whether that is a connection cap wearing an auth failure's clothes.

IMAP has no 429. RFC 5530 gives servers `[UNAVAILABLE]`, `[LIMIT]` and `[CONTACTADMIN]`, but
nothing that means "you have too many connections, back off"; and Dovecot, whose
`mail_max_userip_connections` defaults to **10**, rejects at the login stage where its own
documentation notes the client cannot tell what went wrong. So the classification has to be
inferred from behaviour, and this is the tool that observes it.

Two modes, because two different limits produce the same error:

  concurrency (default)  Hold N connections open at once, authenticating each. Finds a cap on
                         *simultaneous* connections (Dovecot's `mail_max_userip_connections`,
                         Cyrus' `maxchild`, a proxy's pool).
  rate                   Open, authenticate and close N times in quick succession, never more
                         than one at a time. Finds a cap on *authentication rate* (fail2ban,
                         Dovecot's auth penalty, an anti-brute-force proxy).

If concurrency fails at some N and rate does not, it is a connection cap. If both fail at a
similar N, it is a rate limit. That distinction decides whether the fix is a connection
semaphore or a backoff, so it is worth the two runs.

SAFETY. This ramps up and **stops at the first rejection**, and it never retries. That matters
because a limit that presents as an auth failure can also be *counted* as one: hammering a
server whose protection is IP-based can ban the whole IP for every account on it, including
real mailboxes on other devices. Use a test account. Passwords are read from the profile and
never printed.
"""

from __future__ import annotations

import argparse
import os
import re
import socket
import ssl
import sys
import threading
import time
from pathlib import Path

# Where named server profiles live; outside every checkout, mode 600, so a profile written
# while debugging one repo works from another. Same convention as the engine's `dav` tool.
PROFILE_DIRS = (
    Path.home() / ".config" / "allodia" / "servers",
    Path.home() / ".config" / "allodia",
)
# Ramp, not a jump: each step is the smallest that still resolves a plausible cap, so the run
# that finds the limit overshoots it by as little as possible.
LADDER = (1, 2, 4, 6, 8, 10, 12, 14, 16)
READ_TIMEOUT = 20.0


def load_profile(name: str) -> dict[str, str]:
    """The `KEY=value` pairs of a named profile, from the first directory that has it."""
    for directory in PROFILE_DIRS:
        path = directory / f"{name}.env"
        if not path.is_file():
            continue
        values: dict[str, str] = {}
        for line in path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            values[key.strip()] = value.strip().strip("'\"")
        return values
    searched = ", ".join(str(d) for d in PROFILE_DIRS)
    sys.exit(f"no profile '{name}.env' in: {searched}")


def credentials(profile: dict[str, str]) -> tuple[str, str]:
    """The username and password, accepting the key spellings the profiles actually use."""

    def first(*keys: str) -> str | None:
        return next((profile[k] for k in keys if profile.get(k)), None)

    user = first("IMAP_USER", "SOVERIN_USER", "USER", "USERNAME", "CALDAV_USER")
    password = first("IMAP_PASS", "SOVERIN_PASS", "PASS", "PASSWORD", "CALDAV_PASS")
    if not user or not password:
        sys.exit("the profile names no usable username/password")
    return user, password


def quote(value: str) -> str:
    """An IMAP quoted-string, so a password with a space or a quote still logs in."""
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


class Session:
    """One IMAP connection, opened and authenticated far enough to see the server's verdict."""

    def __init__(self, host: str, port: int) -> None:
        self.host, self.port = host, port
        self.sock: ssl.SSLSocket | None = None
        self.greeting = ""
        self.login_response = ""
        self.error = ""
        self.connect_ms = 0.0
        self.login_ms = 0.0

    def open(self) -> None:
        started = time.monotonic()
        try:
            raw = socket.create_connection((self.host, self.port), timeout=READ_TIMEOUT)
            context = ssl.create_default_context()
            self.sock = context.wrap_socket(raw, server_hostname=self.host)
            self.sock.settimeout(READ_TIMEOUT)
            self.greeting = self._read_line()
        except Exception as err:  # noqa: BLE001; any failure is a datum here
            self.error = f"{type(err).__name__}: {err}"
        self.connect_ms = (time.monotonic() - started) * 1000

    def login(self, user: str, password: str) -> None:
        if self.sock is None:
            return
        started = time.monotonic()
        try:
            self.sock.sendall(f"a1 LOGIN {quote(user)} {quote(password)}\r\n".encode())
            # Skip untagged chatter; the tagged line is the verdict.
            while True:
                line = self._read_line()
                if not line or line.startswith("a1 "):
                    self.login_response = line
                    break
                if line.startswith("* BYE"):
                    self.login_response = line
                    break
        except Exception as err:  # noqa: BLE001
            self.error = f"{type(err).__name__}: {err}"
        self.login_ms = (time.monotonic() - started) * 1000

    def _read_line(self) -> str:
        assert self.sock is not None
        chunks = bytearray()
        while not chunks.endswith(b"\r\n"):
            byte = self.sock.recv(1)
            if not byte:
                break
            chunks.extend(byte)
        return chunks.decode("utf-8", "replace").rstrip("\r\n")

    def close(self) -> None:
        if self.sock is None:
            return
        try:
            self.sock.sendall(b"a2 LOGOUT\r\n")
            self.sock.close()
        except Exception:  # noqa: BLE001; a close that fails tells us nothing useful
            pass

    @property
    def ok(self) -> bool:
        return self.login_response.startswith("a1 OK")

    def verdict(self, password: str) -> str:
        """One line describing the outcome, with the password scrubbed just in case."""
        text = self.login_response or self.error or "(no response)"
        return text.replace(password, "<redacted>")


def probe_concurrency(host: str, port: int, user: str, password: str, cap: int) -> bool:
    """Hold `cap` connections open at once. True if every one authenticated."""
    sessions = [Session(host, port) for _ in range(cap)]
    threads = [threading.Thread(target=s.open) for s in sessions]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    threads = [threading.Thread(target=s.login, args=(user, password)) for s in sessions]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    good = sum(1 for s in sessions if s.ok)
    print(f"  {cap:>3} concurrent -> {good}/{cap} authenticated")
    for i, s in enumerate(sessions):
        if not s.ok:
            print(f"        conn[{i}] {s.verdict(password)}")
            if s.greeting:
                print(f"        conn[{i}] greeting was: {s.greeting}")
    for s in sessions:
        s.close()
    return good == cap


def probe_rate(host: str, port: int, user: str, password: str, count: int) -> bool:
    """Open, authenticate and close `count` times, one at a time. True if all succeeded."""
    for i in range(count):
        session = Session(host, port)
        session.open()
        session.login(user, password)
        session.close()
        if not session.ok:
            print(f"  sequential attempt {i + 1} REFUSED: {session.verdict(password)}")
            return False
    print(f"  {count} sequential logins, one at a time -> all authenticated")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, help="named profile under ~/.config/allodia")
    parser.add_argument("--host", default="imap.soverin.net")
    parser.add_argument("--port", type=int, default=993)
    parser.add_argument("--max", type=int, default=16, help="highest rung to attempt")
    parser.add_argument("--mode", choices=("concurrency", "rate"), default="concurrency")
    args = parser.parse_args()

    user, password = credentials(load_profile(args.profile))
    masked = re.sub(r"^(.).*?(@|$)", r"\1***\2", user)
    print(f"probing {args.host}:{args.port} as {masked}: mode={args.mode}\n")

    if args.mode == "rate":
        probe_rate(args.host, args.port, user, password, args.max)
        return 0

    limit: int | None = None
    for rung in LADDER:
        if rung > args.max:
            break
        if not probe_concurrency(args.host, args.port, user, password, rung):
            limit = rung
            break
        # Let the server release the sockets before the next rung, so the ladder measures
        # concurrency rather than accumulation.
        time.sleep(2.0)

    print()
    if limit is None:
        print(f"no cap found up to {args.max} simultaneous connections")
    else:
        print(f"REFUSED at {limit} simultaneous connections: stopping, not retrying")
    return 0


if __name__ == "__main__":
    sys.exit(main())
