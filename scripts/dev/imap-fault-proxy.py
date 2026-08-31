#!/usr/bin/env python3
"""Refuse a chosen share of IMAP logins, so "the server said no to a credential that works" is
reproducible on a device instead of only in a unit test.

    scripts/dev/harness.sh up
    scripts/dev/imap-fault-proxy.py --refuse-every 5        # prints the line to export
    export MAILCAL_EXTRA_CA_PEM="$(base64 < docker/stalwart/tls/fault-proxy-cert.pem | tr -d '\\n')"
    scripts/dev/boot.sh android --account stalwart-imap
    adb reverse tcp:12993 tcp:12994                         # or pass --adb-reverse

A server can refuse a credential that is perfectly valid: Dovecot answers `[AUTHENTICATIONFAILED]`
after its two-second `auth_failure_delay` while sibling folders on the same account authenticate in
the same second. The core's rule for that; a refusal raises the sign-in prompt only when nothing on
the account reached; cannot be exercised against a real test server, because no real server refuses
one folder on demand. Stopping the harness refuses *everything* (a transport failure, the wrong
class), and changing a password server-side reaches neither an authenticated IMAP session nor one
folder in isolation.

So this sits between the client and the harness, passes every byte through, and answers **only** the
auth command itself; refusing a chosen share of connections and forwarding the rest untouched. The
greeting, the capabilities and every response after login come from the real server, so a refused
connection differs from a working one in exactly one respect.

`--refuse-every 5` is the interesting setting: an account opens one connection to connect and one per
folder, so every fifth refusal lands on a *folder* while the account and its siblings authenticate,
the mixed pass. `--refuse-all` gives the other case, an account where nothing works.

TLS. The client speaks implicit TLS, so this terminates it with a self-signed certificate of its own
(generated on first run, `DNS:localhost`) and the client must be told to trust it; that is what the
`MAILCAL_EXTRA_CA_PEM` line above is for. It reaches the harness over TLS **without** verification,
deliberately: the target is a local test server whose certificate is self-signed by design.

Local harness only. It terminates TLS and reads the auth command, so pointing it at a real mail
server would put a real credential in this process.
"""

from __future__ import annotations

import argparse
import shutil
import socket
import ssl
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
TLS_DIR = REPO_ROOT / "docker" / "stalwart" / "tls"
CERT = TLS_DIR / "fault-proxy-cert.pem"
KEY = TLS_DIR / "fault-proxy-key.pem"

# The commands that carry a credential. `AUTHENTICATE` is refused outright rather than answered with
# a `+` continuation: a server may reject the mechanism itself, and the client's error is the same.
AUTH_COMMANDS = (b"LOGIN", b"AUTHENTICATE")
# Verbatim what Dovecot sends, so the engine classifies this the way it classifies production.
REFUSAL = b"NO [AUTHENTICATIONFAILED] Authentication failed."
# Dovecot's `auth_failure_delay` default. The delay is the tell that a refusal is deliberate rather
# than a timeout, so it is reproduced by default.
DEFAULT_DELAY_MS = 2000
CLIENT_TIMEOUT = 60.0


def command_tag(line: bytes) -> bytes:
    """The tag of an IMAP client command, or `*` when the line carries none.

    Echoing the client's own tag is what makes a refusal *arrive*: answer a different tag and the
    client waits for a response that never comes, which reads as a hang and not as a refusal.
    """
    tag = line.strip().split(b" ", 1)[0]
    return tag or b"*"


def is_auth_command(line: bytes) -> bool:
    """Whether this client line is the one that carries the credential."""
    parts = line.strip().split(b" ", 2)
    return len(parts) >= 2 and parts[1].upper() in AUTH_COMMANDS


class FaultPolicy:
    """Decides, per connection, whether its login is refused. Counts from one."""

    def __init__(self, *, refuse_every: int = 0, refuse_all: bool = False) -> None:
        self.refuse_every = refuse_every
        self.refuse_all = refuse_all
        self.connections = 0
        self.refused = 0
        self._lock = threading.Lock()

    def next_connection(self) -> tuple[int, bool]:
        """The ordinal of this connection and whether to refuse its login."""
        with self._lock:
            self.connections += 1
            ordinal = self.connections
            refuse = self.refuse_all or (
                self.refuse_every > 0 and ordinal % self.refuse_every == 0
            )
            if refuse:
                self.refused += 1
            return ordinal, refuse


def run_session(
    client: socket.socket,
    server: socket.socket,
    *,
    refuse: bool,
    delay_ms: int = 0,
) -> str:
    """Relays one connection, answering the auth command itself when `refuse`.

    Returns what happened, for the operator's log. Byte-transparent in both directions apart from
    that one line; inspection of the client's stream stops at the auth command, so a literal in a
    later `APPEND` can never be mistaken for a second login.
    """
    reader = client.makefile("rb")
    upstream = threading.Thread(target=_pump, args=(server, client), daemon=True)
    upstream.start()
    try:
        while True:
            line = reader.readline()
            if not line:
                return "closed before login"
            if is_auth_command(line):
                if refuse:
                    time.sleep(delay_ms / 1000)
                    client.sendall(command_tag(line) + b" " + REFUSAL + b"\r\n")
                    return "login REFUSED"
                server.sendall(line)
                _pump(client, server)
                return "login forwarded"
            server.sendall(line)
    finally:
        _close(server)
        _close(client)


def _pump(src: socket.socket, dst: socket.socket) -> None:
    """Copies until either end closes. A closed socket is the normal end of a session, not an error."""
    try:
        while chunk := src.recv(65536):
            dst.sendall(chunk)
    except OSError:
        pass
    finally:
        try:
            dst.shutdown(socket.SHUT_WR)
        except OSError:
            pass


def _close(sock: socket.socket) -> None:
    try:
        sock.close()
    except OSError:
        pass


def ensure_cert() -> None:
    """Generates the proxy's certificate on first run. `DNS:localhost`; the name the client dials.

    `CA:FALSE` is not cosmetic. The client trusts this certificate directly, so the same certificate
    is both the trust anchor and the server's own leaf; and rustls rejects a leaf marked as a CA,
    which `openssl req -x509` sets by default. A `CA:TRUE` certificate here fails the handshake with
    `certificate unknown`, which looks like the client never received the trust root at all.
    """
    if CERT.is_file() and KEY.is_file():
        return
    if not shutil.which("openssl"):
        sys.exit("openssl is needed once, to generate the proxy's certificate")
    TLS_DIR.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "30",
            "-keyout", str(KEY), "-out", str(CERT), "-subj", "/CN=localhost",
            "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
            "-addext", "basicConstraints=CA:FALSE",
        ],
        check=True,
        capture_output=True,
    )
    print(f"==> generated {CERT.relative_to(REPO_ROOT)}")


def serve(args: argparse.Namespace, policy: FaultPolicy) -> None:
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(CERT, KEY)
    # We are the client to the harness, whose certificate is self-signed by design.
    upstream = ssl._create_unverified_context()  # noqa: SLF001; the only way to say "do not verify"
    listener = socket.socket()
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(("127.0.0.1", args.port))
    listener.listen(32)
    print(
        f"==> listening on 127.0.0.1:{args.port} -> {args.target_host}:{args.target_port}; "
        f"{_policy_description(args)}"
    )
    while True:
        raw, _ = listener.accept()
        threading.Thread(
            target=_accept, args=(raw, context, upstream, args, policy), daemon=True
        ).start()


def _accept(
    raw: socket.socket,
    context: ssl.SSLContext,
    upstream: ssl.SSLContext,
    args: argparse.Namespace,
    policy: FaultPolicy,
) -> None:
    ordinal, refuse = policy.next_connection()
    try:
        client = context.wrap_socket(raw, server_side=True)
        client.settimeout(CLIENT_TIMEOUT)
        target = socket.create_connection((args.target_host, args.target_port), timeout=20)
        server = upstream.wrap_socket(target, server_hostname=args.target_host)
    except (OSError, ssl.SSLError) as err:
        print(f"    conn[{ordinal}] failed before login: {type(err).__name__}: {err}")
        _close(raw)
        return
    outcome = run_session(client, server, refuse=refuse, delay_ms=args.delay_ms)
    print(f"    conn[{ordinal}] {outcome} ({policy.refused}/{policy.connections} refused so far)")


def _policy_description(args: argparse.Namespace) -> str:
    if args.refuse_all:
        return "refusing EVERY login"
    if args.refuse_every:
        return f"refusing every {args.refuse_every}th login after a {args.delay_ms}ms delay"
    return "refusing nothing (a plain pass-through)"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=12994, help="listen port (default 12994)")
    parser.add_argument("--target-host", default="127.0.0.1")
    parser.add_argument(
        "--target-port", type=int, default=12993, help="the harness's implicit-TLS IMAP port"
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--refuse-every",
        type=int,
        default=0,
        metavar="N",
        help="refuse every Nth connection's login (5 lands on a folder, not the connect)",
    )
    group.add_argument("--refuse-all", action="store_true", help="refuse every login")
    parser.add_argument("--delay-ms", type=int, default=DEFAULT_DELAY_MS)
    parser.add_argument(
        "--adb-reverse",
        action="store_true",
        help="point the device's own 12993 at this proxy: run AFTER boot.sh, which re-points it "
        "at the harness itself and would otherwise leave the client trusting this cert while "
        "talking to the harness (a handshake failure that reads as a missing trust root)",
    )
    args = parser.parse_args()

    ensure_cert()
    print(
        "==> trust it with:\n"
        f'    export MAILCAL_EXTRA_CA_PEM="$(base64 < {CERT.relative_to(REPO_ROOT)} | tr -d \'\\n\')"'
    )
    if args.adb_reverse:
        subprocess.run(
            ["adb", "reverse", f"tcp:{args.target_port}", f"tcp:{args.port}"], check=True
        )
        print(f"==> adb reverse tcp:{args.target_port} -> host {args.port}")
    try:
        serve(args, FaultPolicy(refuse_every=args.refuse_every, refuse_all=args.refuse_all))
    except KeyboardInterrupt:
        print("\n==> stopped")


if __name__ == "__main__":
    main()
