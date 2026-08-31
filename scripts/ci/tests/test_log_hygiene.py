"""Tests for `check_log_hygiene.py`.

The first two are the ones that matter: the checker must catch the line that actually shipped, and
must not flag the comment above it; because a comment is exactly where the reasoning it objects to
in the *string* is supposed to live. A checker that flagged both would push the explanation out of
the code entirely, which is worse than the bug.
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "check_log_hygiene",
    Path(__file__).resolve().parents[1] / "check_log_hygiene.py",
)
hygiene = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(hygiene)


def check(source: str, suffix: str = ".rs") -> list[tuple[int, str, str]]:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / f"sample{suffix}"
        path.write_text(source, encoding="utf-8")
        return hygiene.violations(path)


class LogHygiene(unittest.TestCase):
    def test_catches_the_line_that_shipped(self) -> None:
        """The real one, verbatim in shape: a design-doc citation inside an `error!`."""
        found = check(
            '''
            log::error!(
                "oauth: [{}] a refresh token ROTATED for an account that is NOT in the \\
                 registry. An account must be registered before it \\
                 connects (docs/provider-oauth.md rule 5)",
                handle,
            );
            '''
        )
        self.assertTrue(found, "the shipped violation was not caught")
        self.assertIn("path in this repository", found[0][1])

    def test_leaves_a_comment_above_a_log_line_alone(self) -> None:
        """Our reasoning belongs in comments; flagging those would defeat the purpose."""
        self.assertEqual(
            check(
                '''
            // See docs/provider-oauth.md rule 5 for why this is an error.
            log::error!("oauth: [{handle}] this account will ask to be signed in again", );
            '''
            ),
            [],
        )

    def test_catches_an_issue_number(self) -> None:
        found = check('log::warn!("queued for later delivery (#1234)");')
        self.assertTrue(found)
        self.assertIn("issue or PR number", found[0][1])

    def test_catches_a_raw_account_id_interpolation(self) -> None:
        found = check(
            'log::warn!("watch[{account_id}/{folder}] dropped; reconnecting");'
        )
        self.assertTrue(found)
        self.assertIn("raw account id", found[0][1])

    def test_catches_a_raw_account_interpolation(self) -> None:
        found = check(
            'log::info!("calendar write reconciled for account {account}");'
        )
        self.assertTrue(found)
        self.assertIn("raw account id", found[0][1])

    def test_allows_a_non_identifying_account_handle(self) -> None:
        self.assertEqual(
            check('log::info!("calendar write reconciled for [{account_handle}]");'),
            [],
        )

    def test_allows_a_protocol_token_that_looks_like_a_number(self) -> None:
        """`invalid_grant`, status codes and RFC-ish tokens are what the server said, not our code."""
        self.assertEqual(
            check(
                'log::warn!("oauth: the server refused the sign-in: invalid_grant, HTTP 400");'
            ),
            [],
        )
        self.assertEqual(check('log::info!("imap: server answered 421 and hung up");'), [])

    def test_checks_client_languages_too(self) -> None:
        """The rule is cross-platform; a Kotlin or C# line ships the same file to the same user."""
        self.assertTrue(check('logger.error("see docs/logging.md for the cap")', ".kt"))
        self.assertTrue(check('Log.Warn("retry pending, tracked in #142");', ".cs"))

    def test_a_clean_line_passes(self) -> None:
        self.assertEqual(
            check(
                '''
            log::error!(
                "oauth: [{}] the server renewed this account's sign-in, but this device's secure \\
                 store refused to save it ({err}). Mail keeps working until the app is restarted",
                handle,
            );
            '''
            ),
            [],
        )


class SourceListing(unittest.TestCase):
    def test_skips_a_file_that_is_in_the_index_but_gone_from_disk(self) -> None:
        """A deleted-but-unstaged source file must be skipped, not crash the run.

        `git ls-files` reads the index, so it lists a file `rm` has already removed. This crashed on
        the first branch that deleted a Rust file; a gate that fails for a reason unrelated to what
        it checks is a gate people turn off.
        """
        listed = hygiene.tracked_sources(["scripts"])
        self.assertTrue(all(path.is_file() for path in listed))


if __name__ == "__main__":
    unittest.main()
