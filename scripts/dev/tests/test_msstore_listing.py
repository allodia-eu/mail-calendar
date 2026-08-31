#!/usr/bin/env python3
"""Unit tests for the store-listing CLI; mostly for the things it must *not* do.

The payload tests next door prove the copy is read and merged correctly. What is left is the part
that touches a live Store listing, and there the property worth pinning is restraint: a plan run
writes nothing, `--apply` writes a draft but never submits it, and copy that a console would reject
never reaches the network at all. Each of those is one `if` away from being wrong, and none of them
would be noticed by a run that "worked".

Partner Center is replaced by a recorder, so the whole orchestration is exercised without an
account: what makes these tests meaningful is that the fake is dumb; it asserts nothing itself, it
just remembers what it was asked to do.
"""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import msstore_listing as subject  # noqa: E402
from test_msstore_payload import listing_doc, png  # noqa: E402

DRAFT_ID = "1152921504621243540"
SAS = "https://example.invalid/ingestion?sig=redacted"


class FakeClient:
    """Records what it was asked to do. Asserts nothing; that is the tests' job."""

    def __init__(self, credentials=None, pending=True, listings=None):
        self.credentials = credentials
        self.pending = pending
        self.listings = listings if listings is not None else {}
        self.calls = []

    def _draft(self):
        return {"id": DRAFT_ID, "fileUploadUrl": SAS, "listings": self.listings}

    def app(self):
        self.calls.append(("app", None))
        app = {"id": "9NBLGGH0000", "primaryName": "Allodia Mail & Calendar"}
        if self.pending:
            app["pendingApplicationSubmission"] = {"id": DRAFT_ID}
        else:
            app["lastPublishedApplicationSubmission"] = {"id": "9"}
        return app

    def submission(self, submission_id):
        self.calls.append(("submission", submission_id))
        return self._draft()

    def create_submission(self):
        self.calls.append(("create_submission", None))
        return self._draft()

    def update_submission(self, submission_id, submission):
        self.calls.append(("update_submission", (submission_id, submission)))
        return submission

    def commit_submission(self, submission_id):
        self.calls.append(("commit_submission", submission_id))
        return {}

    def submission_status(self, submission_id):
        self.calls.append(("submission_status", submission_id))
        return {"status": "PreProcessing", "statusDetails": {}}

    def upload(self, sas_url, archive):
        self.calls.append(("upload", (sas_url, Path(archive).read_bytes())))
        return 201

    @property
    def verbs(self):
        return [name for name, _ in self.calls]

    def payload(self, verb):
        return next(argument for name, argument in self.calls if name == verb)


class CommandLine(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.listing = self.root / "store-listing.md"
        self.listing.write_text(listing_doc(), encoding="utf-8")
        self.clients = []

        original_client = subject.PartnerCenter
        original_credentials = subject.Credentials.from_env
        self.addCleanup(setattr, subject, "PartnerCenter", original_client)
        self.addCleanup(setattr, subject.Credentials, "from_env", original_credentials)
        # Resolved credentials, not a resolver: where they come from is `test_msstore_api.py`'s
        # subject, and these tests are about what happens after.
        subject.Credentials.from_env = staticmethod(
            lambda app_id=None, env_file=None: subject.Credentials(
                tenant_id="t", client_id="c", client_secret="s", app_id="9", source="test"
            )
        )

    def use(self, **kwargs):
        """Install a fake Partner Center for the next run and return it."""
        client = FakeClient(**kwargs)
        subject.PartnerCenter = lambda credentials: client
        self.clients.append(client)
        return client

    def run_cli(self, *argv):
        return subject.run(subject.parse_args(["--listing", str(self.listing), *argv]))

    # -- restraint ---------------------------------------------------------------------------

    def test_a_plan_run_reads_and_writes_nothing(self) -> None:
        client = self.use()
        self.assertEqual(self.run_cli("-l", "en"), 0)
        self.assertEqual(client.verbs, ["app", "submission"])

    def test_apply_writes_the_draft_but_does_not_submit_it(self) -> None:
        client = self.use()
        self.assertEqual(self.run_cli("-l", "en", "--apply"), 0)
        self.assertIn("update_submission", client.verbs)
        self.assertNotIn("commit_submission", client.verbs)

    def test_apply_reuses_the_draft_already_in_progress(self) -> None:
        # Creating a second one would discard whatever is already staged in Partner Center.
        client = self.use(pending=True)
        self.run_cli("-l", "en", "--apply")
        self.assertNotIn("create_submission", client.verbs)
        self.assertEqual(client.payload("update_submission")[0], DRAFT_ID)

    def test_apply_opens_a_draft_when_there_is_none(self) -> None:
        client = self.use(pending=False)
        self.run_cli("-l", "en", "--apply")
        self.assertIn("create_submission", client.verbs)

    def test_a_plan_run_never_opens_a_draft_even_when_there_is_none(self) -> None:
        client = self.use(pending=False)
        self.run_cli("-l", "en")
        self.assertNotIn("create_submission", client.verbs)

    def test_commit_is_the_only_thing_that_submits(self) -> None:
        client = self.use()
        self.assertEqual(self.run_cli("-l", "en", "--commit"), 0)
        self.assertEqual(
            client.verbs,
            ["app", "submission", "update_submission", "commit_submission", "submission_status"],
        )

    def test_commit_submits_even_when_the_copy_already_matches(self) -> None:
        # A draft written by an earlier --apply has nothing left to write, but is exactly the
        # draft you then want to submit. Returning early here made `--commit` a no-op that
        # reported success; it printed "nothing to write" and never called commit at all.
        listings = {
            "en-us": {
                "baseListing": {
                    "description": "Sovereign mail.",
                    "features": ["One feature", "Another feature"],
                    "keywords": ["secure email app", "JMAP email client"],
                    "copyrightAndTrademarkInfo": "© 2026 Allodia",
                }
            }
        }
        client = self.use(listings=listings)
        self.assertEqual(self.run_cli("-l", "en", "--commit"), 0)
        self.assertNotIn("update_submission", client.verbs)
        self.assertIn("commit_submission", client.verbs)

    def test_apply_alone_still_stops_when_there_is_nothing_to_write(self) -> None:
        listings = {
            "en-us": {
                "baseListing": {
                    "description": "Sovereign mail.",
                    "features": ["One feature", "Another feature"],
                    "keywords": ["secure email app", "JMAP email client"],
                    "copyrightAndTrademarkInfo": "© 2026 Allodia",
                }
            }
        }
        client = self.use(listings=listings)
        self.assertEqual(self.run_cli("-l", "en", "--apply"), 0)
        self.assertNotIn("update_submission", client.verbs)
        self.assertNotIn("commit_submission", client.verbs)

    def test_offline_never_reaches_partner_center(self) -> None:
        def explode(credentials):
            raise AssertionError("--offline must not construct a client")

        subject.PartnerCenter = explode
        self.assertEqual(self.run_cli("-l", "en", "--offline", "--out", str(self.root / "out")), 0)
        self.assertTrue((self.root / "out" / "en-us" / "description.txt").is_file())

    # -- the local gate ----------------------------------------------------------------------

    def test_copy_the_console_would_reject_never_reaches_the_network(self) -> None:
        # Measured first, deliberately: the alternative is a draft written, a person waiting, and
        # a rejection that names a field they cannot see.
        self.listing.write_text(listing_doc(english_features="x" * 201), encoding="utf-8")

        def explode(credentials):
            raise AssertionError("an over-long field must fail before authentication")

        subject.PartnerCenter = explode
        self.assertEqual(self.run_cli("-l", "en"), 1)

    def test_a_reworded_document_exits_two_rather_than_pushing_less(self) -> None:
        self.listing.write_text(
            listing_doc().replace("## Shared description — English", "## The description"),
            encoding="utf-8",
        )
        self.use()
        self.assertEqual(subject.main(["--listing", str(self.listing), "-l", "en"]), 2)

    # -- screenshots -------------------------------------------------------------------------

    def test_without_the_flag_no_image_is_touched(self) -> None:
        client = self.use(
            listings={"en-us": {"baseListing": {"images": [{"fileName": "old.png"}]}}}
        )
        self.run_cli("-l", "en", "--apply")
        _, submission = client.payload("update_submission")
        self.assertEqual(
            submission["listings"]["en-us"]["baseListing"]["images"], [{"fileName": "old.png"}]
        )
        self.assertNotIn("upload", client.verbs)

    def test_with_the_flag_the_gallery_is_replaced_and_uploaded(self) -> None:
        shots = self.root / "shots"
        shots.mkdir()
        for name in ("en-list.png", "en-calendar.png"):
            (shots / name).write_bytes(png(2880, 1800))
        client = self.use()
        self.assertEqual(self.run_cli("-l", "en", "--apply", "--screenshots", str(shots)), 0)
        _, submission = client.payload("update_submission")
        images = submission["listings"]["en-us"]["baseListing"]["images"]
        self.assertEqual([image["fileName"] for image in images], ["en-list.png", "en-calendar.png"])
        url, archive = client.payload("upload")
        self.assertEqual(url, SAS)
        with zipfile.ZipFile(self.root / "archive.zip", "w") as _:
            pass
        (self.root / "archive.zip").write_bytes(archive)
        with zipfile.ZipFile(self.root / "archive.zip") as opened:
            self.assertEqual(sorted(opened.namelist()), ["en-calendar.png", "en-list.png"])

    def test_a_console_created_draft_is_explained_not_just_reported(self) -> None:
        # The raw 409 advises deleting the submission, which for a draft holding an unpublished
        # package or screenshot set destroys work. The tool must say so instead of relaying it.
        client = self.use()

        def refuse(submission_id, submission):
            client.calls.append(("update_submission", (submission_id, submission)))
            raise subject.PartnerCenterError(
                "PUT /applications/X/submissions/Y failed (409): InvalidState — Cannot update the "
                "submission because it is in the state 'None'. If you need to change the "
                "submission, delete the submission and create a new one."
            )

        client.update_submission = refuse
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(self.run_cli("-l", "en", "--apply"), 1)
        self.assertIn("Do NOT delete it", errors.getvalue())
        self.assertIn("--offline --out", errors.getvalue())
        self.assertNotIn("commit_submission", client.verbs)

    def test_an_unrelated_partner_center_failure_is_not_swallowed(self) -> None:
        client = self.use()

        def refuse(submission_id, submission):
            raise subject.PartnerCenterError("PUT failed (500): something else entirely")

        client.update_submission = refuse
        with self.assertRaises(subject.PartnerCenterError):
            self.run_cli("-l", "en", "--apply")

    def test_the_upload_happens_after_the_submission_names_the_files(self) -> None:
        # The other order uploads a blob nothing in the submission refers to, and the images are
        # dropped on ingestion with no error anyone reads.
        shots = self.root / "shots"
        shots.mkdir()
        (shots / "en-list.png").write_bytes(png(2880, 1800))
        client = self.use()
        self.run_cli("-l", "en", "--apply", "--screenshots", str(shots))
        self.assertLess(client.verbs.index("update_submission"), client.verbs.index("upload"))


if __name__ == "__main__":
    unittest.main()
