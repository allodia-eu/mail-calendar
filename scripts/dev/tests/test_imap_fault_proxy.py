"""Unit tests for `scripts/dev/imap-fault-proxy.py`.

Every failure this tool can have looks like a finding. A refusal sent with the wrong tag hangs the
client, which reads as a timeout; a *transport* class, the opposite of what the tool exists to
produce. A pass-through that mangles a byte makes a working folder look broken. And a policy that
refuses the wrong connection turns the mixed pass (some scopes reached, one refused) into the
all-refused one, which is the case the core is *supposed* to raise a prompt for. So the injector is
worth more scepticism than the code it tests.

The session tests run over a real `socketpair`, without TLS: TLS is the transport, not the behaviour.
"""

from __future__ import annotations

import importlib.util
import socket
import threading
import unittest
from pathlib import Path

# The script is a hyphenated executable, not an importable module name.
_SPEC = importlib.util.spec_from_file_location(
    "imap_fault_proxy", Path(__file__).resolve().parents[1] / "imap-fault-proxy.py"
)
assert _SPEC and _SPEC.loader
proxy = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(proxy)


class TagTests(unittest.TestCase):
    def test_the_clients_own_tag_comes_back(self):
        self.assertEqual(proxy.command_tag(b"a001 LOGIN user pass\r\n"), b"a001")

    def test_an_unusual_tag_is_not_normalised(self):
        # Engines number their own way; assuming `a001` would answer a tag no client is waiting on.
        self.assertEqual(proxy.command_tag(b"A7 LOGIN user pass\r\n"), b"A7")
        self.assertEqual(proxy.command_tag(b"x-42 AUTHENTICATE PLAIN\r\n"), b"x-42")

    def test_a_line_with_no_tag_does_not_crash(self):
        self.assertEqual(proxy.command_tag(b"\r\n"), b"*")


class AuthCommandTests(unittest.TestCase):
    def test_login_and_authenticate_both_carry_a_credential(self):
        self.assertTrue(proxy.is_auth_command(b"a1 LOGIN user pass\r\n"))
        self.assertTrue(proxy.is_auth_command(b"a1 AUTHENTICATE PLAIN\r\n"))

    def test_the_command_word_is_case_insensitive(self):
        # IMAP commands are case-insensitive; a lowercase login that slipped through would be
        # forwarded and the connection would silently succeed.
        self.assertTrue(proxy.is_auth_command(b"a1 login user pass\r\n"))

    def test_other_commands_are_not_intercepted(self):
        for line in (b"a1 CAPABILITY\r\n", b"a1 SELECT INBOX\r\n", b"a1 LOGOUT\r\n", b"\r\n"):
            self.assertFalse(proxy.is_auth_command(line), line)

    def test_a_command_merely_containing_login_is_not_one(self):
        self.assertFalse(proxy.is_auth_command(b"a1 SELECT LOGIN-notes\r\n"))


class PolicyTests(unittest.TestCase):
    def test_every_fifth_connection_is_refused(self):
        policy = proxy.FaultPolicy(refuse_every=5)
        verdicts = [policy.next_connection()[1] for _ in range(10)]
        self.assertEqual(
            verdicts,
            [False, False, False, False, True, False, False, False, False, True],
        )

    def test_the_first_connection_is_never_the_refused_one(self):
        # An account's first connection is its *connect*; refusing that raises the prompt through
        # the connect path and the mixed pass is never reached.
        policy = proxy.FaultPolicy(refuse_every=5)
        self.assertFalse(policy.next_connection()[1])

    def test_refuse_all_refuses_the_first(self):
        policy = proxy.FaultPolicy(refuse_all=True)
        self.assertTrue(policy.next_connection()[1])
        self.assertTrue(policy.next_connection()[1])

    def test_the_default_policy_refuses_nothing(self):
        policy = proxy.FaultPolicy()
        self.assertEqual([policy.next_connection()[1] for _ in range(4)], [False] * 4)

    def test_ordinals_count_from_one_and_refusals_are_counted(self):
        policy = proxy.FaultPolicy(refuse_every=2)
        self.assertEqual(policy.next_connection()[0], 1)
        self.assertEqual(policy.next_connection(), (2, True))
        self.assertEqual(policy.refused, 1)


class SessionTests(unittest.TestCase):
    """The relay itself, over a real socketpair on both sides."""

    def _session(self, refuse: bool):
        client_app, client_proxy = socket.socketpair()
        server_proxy, server_app = socket.socketpair()
        outcome: list[str] = []
        thread = threading.Thread(
            target=lambda: outcome.append(
                proxy.run_session(client_proxy, server_proxy, refuse=refuse, delay_ms=0)
            ),
            daemon=True,
        )
        thread.start()
        for sock in (client_app, server_app):
            sock.settimeout(5)
        return client_app, server_app, thread, outcome

    def test_a_refused_login_gets_dovecots_answer_with_its_own_tag(self):
        client, server, thread, outcome = self._session(refuse=True)
        client.sendall(b"A7 LOGIN alice secret\r\n")

        answer = client.recv(200)
        self.assertEqual(answer, b"A7 NO [AUTHENTICATIONFAILED] Authentication failed.\r\n")
        thread.join(timeout=5)
        self.assertEqual(outcome, ["login REFUSED"])
        client.close()
        server.close()

    def test_a_refused_login_never_reaches_the_server(self):
        # The credential is valid; if it were forwarded the server would accept it, and the
        # connection would work while the proxy claimed to have refused it.
        client, server, thread, _ = self._session(refuse=True)
        client.sendall(b"a1 LOGIN alice secret\r\n")
        client.recv(200)
        thread.join(timeout=5)

        # Silence, EOF and a reset all mean "nothing was relayed", and which one arrives is the
        # platform's choice: closing a socket another thread is blocked reading on wakes the peer
        # with EOF on macOS, leaves it waiting on Linux, and resets it on Windows
        # (ConnectionResetError / WinError 10054). Asserting any one of the three specifically has
        # now failed on two of the three hosts; first EOF, which passed on a Mac and failed in CI.
        server.settimeout(0.3)
        try:
            relayed = server.recv(200)
        except (TimeoutError, ConnectionResetError):
            relayed = b""
        self.assertEqual(relayed, b"", "the login was relayed to the server")
        client.close()
        server.close()

    def test_everything_before_the_login_is_relayed_both_ways(self):
        client, server, thread, _ = self._session(refuse=True)

        server.sendall(b"* OK [CAPABILITY IMAP4rev1] harness ready\r\n")
        self.assertEqual(client.recv(200), b"* OK [CAPABILITY IMAP4rev1] harness ready\r\n")
        client.sendall(b"a1 CAPABILITY\r\n")
        self.assertEqual(server.recv(200), b"a1 CAPABILITY\r\n")
        server.sendall(b"* CAPABILITY IMAP4rev1 AUTH=PLAIN\r\na1 OK done\r\n")
        self.assertEqual(client.recv(200), b"* CAPABILITY IMAP4rev1 AUTH=PLAIN\r\na1 OK done\r\n")

        # Only now the login, and only it is answered by the proxy.
        client.sendall(b"a2 LOGIN alice secret\r\n")
        self.assertEqual(client.recv(200), b"a2 NO [AUTHENTICATIONFAILED] Authentication failed.\r\n")
        thread.join(timeout=5)
        client.close()
        server.close()

    def test_a_forwarded_login_leaves_the_session_transparent(self):
        client, server, thread, outcome = self._session(refuse=False)
        client.sendall(b"a1 LOGIN alice secret\r\n")
        self.assertEqual(server.recv(200), b"a1 LOGIN alice secret\r\n")
        server.sendall(b"a1 OK logged in\r\n")
        self.assertEqual(client.recv(200), b"a1 OK logged in\r\n")

        # And past the login it is a plain pipe; including a literal whose payload contains a line
        # that looks like a command, which inspection must no longer be reading.
        client.sendall(b"a2 APPEND INBOX {24}\r\na3 LOGIN not-a-command\r\n")
        self.assertEqual(server.recv(200), b"a2 APPEND INBOX {24}\r\na3 LOGIN not-a-command\r\n")
        server.sendall(b"a2 OK appended\r\n")
        self.assertEqual(client.recv(200), b"a2 OK appended\r\n")

        client.close()
        server.close()
        thread.join(timeout=5)
        self.assertEqual(outcome, ["login forwarded"])

    def test_a_client_that_closes_without_logging_in_is_not_an_error(self):
        client, server, thread, outcome = self._session(refuse=True)
        client.close()
        thread.join(timeout=5)
        self.assertEqual(outcome, ["closed before login"])
        server.close()


if __name__ == "__main__":
    unittest.main()
