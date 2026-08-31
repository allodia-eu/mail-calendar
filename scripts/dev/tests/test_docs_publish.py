"""Tests for `scripts/dev/docs_publish.py`.

Every case runs against a fake store rather than the network, so the suite is deterministic and
needs no site to be up. The interesting half is the failures: a manifest naming an image nobody
captured, a blob whose name and content disagree, a store that renames what it was handed. Each is
a way the docs could ship with a broken image, and each is asserted to stop the run.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import docs_publish  # noqa: E402
import envfile  # noqa: E402


def webp(payload: bytes) -> bytes:
    """A RIFF/WEBP container with `payload` inside; enough to be distinct bytes."""
    return b"RIFF" + len(payload).to_bytes(4, "little") + b"WEBP" + payload


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def manifest_for(entries) -> dict:
    """`{(screen, platform, locale): sha256}` → a manifest document."""
    images = {}
    for (screen, platform, locale), digest in entries.items():
        images.setdefault(screen, {}).setdefault(platform, {})[locale] = {
            "sha256": digest,
            "width": 1400,
            "height": 875,
            "bytes": 4096,
        }
    return {"version": 1, "generator": "scripts/dev/docs_images.py", "images": images}


class FakeStore:
    """A `HttpTransport` stand-in that records what it was asked to do."""

    def __init__(self, already=(), rename_to=None, fail_upload=None):
        self.base_url = "https://example.test"
        self.blobs = {digest: b"" for digest in already}
        self.uploads = []
        self.head_calls = []
        self._rename_to = rename_to
        self._fail_upload = fail_upload

    def published(self, digest):
        self.head_calls.append(digest)
        return digest in self.blobs

    def upload(self, data):
        if self._fail_upload is not None:
            raise docs_publish.PublishError(self._fail_upload)
        self.uploads.append(data)
        digest = self._rename_to or sha(data)
        stored = digest not in self.blobs
        self.blobs[digest] = data
        return {"hash": digest, "size": len(data), "stored": stored}


def args(**overrides):
    defaults = dict(manifest=None, blobs=None, apply=False, check=False)
    defaults.update(overrides)
    return argparse.Namespace(**defaults)


class Harness(unittest.TestCase):
    """A temp dir holding a manifest and a blob dir, plus a runner that captures output."""

    def setUp(self):
        self._tmp = TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.blobs = self.root / "docs-web"
        self.blobs.mkdir()
        self.manifest = self.root / "screenshots.json"
        self.addCleanup(self._tmp.cleanup)

    def write_manifest(self, entries):
        self.manifest.write_text(json.dumps(manifest_for(entries)), encoding="utf-8")

    def write_blob(self, data, name=None):
        digest = sha(data)
        (self.blobs / ("%s.webp" % (name or digest))).write_bytes(data)
        return digest

    def run_publish(self, store, **overrides):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = docs_publish.execute(
                args(manifest=self.manifest, blobs=self.blobs, **overrides), store
            )
        return code, out.getvalue(), err.getvalue()


class ReferencedHashes(Harness):
    def test_every_platform_and_locale_is_collected(self):
        first, second = sha(webp(b"a")), sha(webp(b"b"))
        manifest = manifest_for(
            {
                ("setup-email", "macos", "en"): first,
                ("setup-email", "android", "nl"): second,
            }
        )
        self.assertEqual(
            sorted(digest for digest, _ in docs_publish.referenced_hashes(manifest)),
            sorted([first, second]),
        )

    def test_one_image_referenced_twice_is_uploaded_once(self):
        shared = sha(webp(b"same on both"))
        manifest = manifest_for(
            {
                ("mcp-off", "macos", "en"): shared,
                ("mcp-off", "macos", "nl"): shared,
            }
        )
        found = docs_publish.referenced_hashes(manifest)
        self.assertEqual(len(found), 1)
        # The location kept is the first one, so the error message still names a real page.
        self.assertIn("mcp-off", found[0][1])

    def test_a_manifest_with_no_images_object_is_refused(self):
        with self.assertRaises(docs_publish.PublishError) as raised:
            docs_publish.referenced_hashes({"version": 1})
        self.assertIn("docs_images.py", str(raised.exception))

    def test_an_entry_without_a_usable_hash_is_refused(self):
        for broken in [{}, {"sha256": "short"}, {"sha256": 42}, "not an object"]:
            manifest = {"images": {"s": {"macos": {"en": broken}}}}
            with self.assertRaises(docs_publish.PublishError):
                docs_publish.referenced_hashes(manifest)


class Planning(Harness):
    def test_the_default_run_uploads_nothing(self):
        data = webp(b"one")
        digest = self.write_blob(data)
        self.write_manifest({("setup-email", "macos", "en"): digest})
        store = FakeStore()

        code, out, _ = self.run_publish(store)

        self.assertEqual(code, 0)
        self.assertEqual(store.uploads, [], "plan mode must not send anything")
        self.assertIn("Would publish 1 image", out)
        self.assertIn("--apply", out)

    def test_an_empty_manifest_says_so_instead_of_claiming_success(self):
        self.manifest.write_text(json.dumps(manifest_for({})), encoding="utf-8")
        store = FakeStore()

        code, out, _ = self.run_publish(store)

        self.assertEqual(code, 0)
        self.assertIn("nothing to publish", out)
        self.assertEqual(store.head_calls, [])

    def test_a_fully_published_set_is_a_no_op(self):
        digest = self.write_blob(webp(b"already up"))
        self.write_manifest({("setup-email", "macos", "en"): digest})

        code, out, _ = self.run_publish(FakeStore(already=[digest]))

        self.assertEqual(code, 0)
        self.assertIn("Nothing to do", out)


class Applying(Harness):
    def test_it_uploads_exactly_the_missing_blobs(self):
        here, there = webp(b"present"), webp(b"absent")
        self.write_blob(here)
        self.write_blob(there)
        self.write_manifest(
            {
                ("setup-email", "macos", "en"): sha(here),
                ("setup-detected", "macos", "en"): sha(there),
            }
        )
        store = FakeStore(already=[sha(here)])

        code, out, _ = self.run_publish(store, apply=True)

        self.assertEqual(code, 0)
        self.assertEqual(store.uploads, [there])
        self.assertIn("1 newly stored", out)

    def test_re_publishing_reports_zero_new_rather_than_failing(self):
        data = webp(b"idempotent")
        self.write_blob(data)
        self.write_manifest({("setup-email", "macos", "en"): sha(data)})
        # The store answers 404 to HEAD but already holds the bytes; the window a
        # re-run after a partial upload lands in.
        store = FakeStore()
        store.blobs[sha(data)] = data
        store.published = lambda digest: False  # type: ignore[method-assign]

        code, out, _ = self.run_publish(store, apply=True)

        self.assertEqual(code, 0)
        self.assertIn("0 newly stored", out)

    def test_a_manifest_naming_an_image_this_machine_lacks_stops_the_run(self):
        self.write_manifest({("setup-email", "macos", "en"): sha(webp(b"never captured"))})

        code, _, err = self.run_publish(FakeStore(), apply=True)

        self.assertEqual(code, 1)
        self.assertIn("showcase.sh", err)
        self.assertIn("docs_images.py", err)

    def test_a_blob_whose_name_lies_about_its_contents_is_not_uploaded(self):
        # The filename claims one hash, the bytes are another. Uploading it would store the bytes
        # under their *real* hash and leave the manifest pointing at nothing.
        liar = sha(webp(b"claimed"))
        (self.blobs / ("%s.webp" % liar)).write_bytes(webp(b"actual"))
        self.write_manifest({("setup-email", "macos", "en"): liar})
        store = FakeStore()

        code, _, err = self.run_publish(store, apply=True)

        self.assertEqual(code, 1)
        self.assertIn("does not hash to its own name", err)
        self.assertEqual(store.uploads, [])

    def test_a_store_that_names_the_upload_differently_is_a_hard_error(self):
        data = webp(b"mangled in flight")
        self.write_blob(data)
        self.write_manifest({("setup-email", "macos", "en"): sha(data)})

        code, _, err = self.run_publish(
            FakeStore(rename_to="f" * 64), apply=True
        )

        self.assertEqual(code, 1)
        self.assertIn("not the bytes the manifest describes", err)

    def test_a_transport_failure_surfaces_its_message(self):
        data = webp(b"unauthorized")
        self.write_blob(data)
        self.write_manifest({("setup-email", "macos", "en"): sha(data)})

        code, _, err = self.run_publish(
            FakeStore(fail_upload="the website rejected the upload token (401)"), apply=True
        )

        self.assertEqual(code, 1)
        self.assertIn("401", err)


class Checking(Harness):
    def test_it_passes_when_every_referenced_image_is_served(self):
        digest = sha(webp(b"published"))
        self.write_manifest({("setup-email", "macos", "en"): digest})

        code, out, _ = self.run_publish(FakeStore(already=[digest]), check=True)

        self.assertEqual(code, 0)
        self.assertIn("every image the docs reference is published", out)

    def test_it_fails_and_names_the_page_when_one_is_missing(self):
        published, absent = sha(webp(b"up")), sha(webp(b"down"))
        self.write_manifest(
            {
                ("setup-email", "macos", "en"): published,
                ("setup-untrusted", "android", "nl"): absent,
            }
        )

        code, _, err = self.run_publish(FakeStore(already=[published]), check=True)

        self.assertEqual(code, 1)
        self.assertIn("setup-untrusted", err)
        self.assertIn("android", err)
        self.assertIn("nl", err)

    def test_checking_needs_no_local_blobs(self):
        # The point of --check: it runs anywhere, including a machine that has never captured
        # anything, so CI and a release script can both ask the question.
        digest = sha(webp(b"remote only"))
        self.write_manifest({("setup-email", "macos", "en"): digest})
        self.assertEqual(list(self.blobs.iterdir()), [])

        code, _, _ = self.run_publish(FakeStore(already=[digest]), check=True)

        self.assertEqual(code, 0)

    def test_check_never_uploads_even_when_the_blobs_are_right_there(self):
        data = webp(b"could have been uploaded")
        self.write_blob(data)
        self.write_manifest({("setup-email", "macos", "en"): sha(data)})
        store = FakeStore()

        code, _, _ = self.run_publish(store, check=True)

        self.assertEqual(code, 1)
        self.assertEqual(store.uploads, [])


class ManifestLoading(Harness):
    def test_a_missing_manifest_points_at_the_generator(self):
        with self.assertRaises(docs_publish.PublishError) as raised:
            docs_publish.load_manifest(self.root / "nope.json")
        self.assertIn("docs_images.py", str(raised.exception))

    def test_a_corrupt_manifest_says_it_is_not_json(self):
        self.manifest.write_text("{oops", encoding="utf-8")
        with self.assertRaises(docs_publish.PublishError) as raised:
            docs_publish.load_manifest(self.manifest)
        self.assertIn("not valid JSON", str(raised.exception))


class EnvIsolated(unittest.TestCase):
    """A temp dir standing in for both credentials locations, with the real ones neutralized."""

    def setUp(self):
        self._tmp = TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.addCleanup(self._tmp.cleanup)
        # Neither the environment nor a real credentials file on this machine may decide the
        # outcome of these tests, so both are neutralized and restored.
        # BASE_URL_ENV belongs here too: without it these tests pass or fail depending on
        # whether the developer exports ALLODIA_DOCS_BASE_URL, which made the suite green in
        # CI and red on the machine of the person who publishes the docs.
        for name in (docs_publish.TOKEN_ENV, docs_publish.ENV_FILE_VAR, docs_publish.BASE_URL_ENV):
            self.addCleanup(os.environ.pop, name, None)
            os.environ.pop(name, None)
        original = docs_publish.ENV_FILE_LOCATIONS
        self.addCleanup(setattr, docs_publish, "ENV_FILE_LOCATIONS", original)
        docs_publish.ENV_FILE_LOCATIONS = (self.root / "absent.env", self.root / "also-absent.env")

    def write(self, body="ALLODIA_DOCS_UPLOAD_TOKEN=from-the-file\n", name="docs.env"):
        path = self.root / name
        path.write_text(body, encoding="utf-8")
        return path


class TokenHandling(EnvIsolated):
    def test_uploading_without_a_token_says_where_to_put_one(self):
        transport = docs_publish.HttpTransport("https://example.test", token=None)
        with self.assertRaises(docs_publish.PublishError) as raised:
            transport.upload(webp(b"x"))
        message = str(raised.exception)
        self.assertIn(docs_publish.TOKEN_ENV, message)
        # It must name the file, not only the variable: "set an env var" is advice you have to
        # follow again in every new shell.
        self.assertIn(str(docs_publish.ENV_FILE_LOCATIONS[1]), message)
        # And it must name the website's variable, since the two have to match and nothing else
        # says so at the moment the upload is refused.
        self.assertIn("DOCS_ASSET_UPLOAD_TOKEN", message)

    def test_the_token_is_not_a_command_line_flag(self):
        # A secret on argv lands in shell history and in every `ps` on the box. Assert the flag's
        # absence so nobody adds one for convenience later.
        out = io.StringIO()
        with redirect_stdout(out):
            with self.assertRaises(SystemExit):
                docs_publish.main(["--help"])
        self.assertNotIn("--token", out.getvalue())

    def test_no_token_anywhere_resolves_to_none(self):
        self.assertIsNone(docs_publish.resolve_token())

    def test_it_reads_the_credentials_file(self):
        docs_publish.ENV_FILE_LOCATIONS = (self.root / "absent.env", self.write())
        self.assertEqual(docs_publish.resolve_token(), "from-the-file")

    def test_the_environment_beats_the_file(self):
        # A file is a convenience, not an override of what the person just typed.
        docs_publish.ENV_FILE_LOCATIONS = (self.write(),)
        os.environ[docs_publish.TOKEN_ENV] = "from-the-environment"
        self.assertEqual(docs_publish.resolve_token(), "from-the-environment")

    def test_the_first_location_wins_over_the_second(self):
        first = self.write("ALLODIA_DOCS_UPLOAD_TOKEN=repo-local\n", name="first.env")
        second = self.write("ALLODIA_DOCS_UPLOAD_TOKEN=home-config\n", name="second.env")
        docs_publish.ENV_FILE_LOCATIONS = (first, second)
        self.assertEqual(docs_publish.resolve_token(), "repo-local")

    def test_a_shared_env_does_not_hide_a_key_only_a_later_file_has(self):
        # The reason `.env` can be put in front of files that already work. A `.env` holding
        # someone else's keys must not shadow `.docs.env` next to it; and "first file found wins"
        # would do exactly that, silently, looking like a token that had gone missing.
        shared = self.write("MSSTORE_APP_ID=9\n", name="shared.env")
        per_tool = self.write("ALLODIA_DOCS_UPLOAD_TOKEN=still-found\n", name="per-tool.env")
        docs_publish.ENV_FILE_LOCATIONS = (shared, per_tool)
        self.assertEqual(docs_publish.resolve_token(), "still-found")

    def test_a_named_file_is_read_alone_rather_than_merged(self):
        # Asking for one file and being handed a merge of three is how you publish with the wrong
        # credentials while believing you pinned them.
        self.write("ALLODIA_DOCS_UPLOAD_TOKEN=from-the-search-path\n", name="searched.env")
        docs_publish.ENV_FILE_LOCATIONS = (self.root / "searched.env",)
        named = self.write("ALLODIA_DOCS_BASE_URL=https://named.example\n", name="named.env")
        settings = docs_publish.resolve_settings(env_file=str(named))
        self.assertEqual(settings.base_url, "https://named.example")
        self.assertIsNone(settings.token)

    def test_the_env_file_variable_is_honoured(self):
        os.environ[docs_publish.ENV_FILE_VAR] = str(self.write())
        self.assertEqual(docs_publish.resolve_token(), "from-the-file")

    def test_a_named_file_that_is_not_there_is_an_error(self):
        # Asking for a file and being handed the environment instead is how you publish to the
        # wrong place. Both ways of naming one must fail loudly.
        with self.assertRaises(envfile.EnvFileError):
            docs_publish.resolve_token(str(self.root / "nope.env"))
        os.environ[docs_publish.ENV_FILE_VAR] = str(self.root / "nope.env")
        with self.assertRaises(envfile.EnvFileError):
            docs_publish.resolve_token()

    def test_a_missing_file_is_reported_rather_than_crashing_the_run(self):
        code = docs_publish.main(["--env-file", str(self.root / "nope.env")])
        self.assertEqual(code, 1)

    def test_a_file_holding_only_other_keys_resolves_to_none(self):
        docs_publish.ENV_FILE_LOCATIONS = (self.write("MSSTORE_APP_ID=9\n"),)
        self.assertIsNone(docs_publish.resolve_token())


class TargetResolution(EnvIsolated):
    """The base URL follows the same flag → environment → file → default order as the token."""

    def test_nothing_configured_means_the_live_site(self):
        settings = docs_publish.resolve_settings()
        self.assertEqual(settings.base_url, docs_publish.DEFAULT_BASE_URL)
        self.assertTrue(settings.is_default_target)

    def test_the_file_can_point_the_run_somewhere_else(self):
        docs_publish.ENV_FILE_LOCATIONS = (
            self.write("ALLODIA_DOCS_BASE_URL=https://staging.example\n"),
        )
        settings = docs_publish.resolve_settings()
        self.assertEqual(settings.base_url, "https://staging.example")
        self.assertFalse(settings.is_default_target)

    def test_the_environment_beats_the_file_for_the_target_too(self):
        docs_publish.ENV_FILE_LOCATIONS = (
            self.write("ALLODIA_DOCS_BASE_URL=https://from-file.example\n"),
        )
        os.environ[docs_publish.BASE_URL_ENV] = "https://from-env.example"
        self.addCleanup(os.environ.pop, docs_publish.BASE_URL_ENV, None)
        self.assertEqual(docs_publish.resolve_settings().base_url, "https://from-env.example")

    def test_the_flag_beats_everything(self):
        docs_publish.ENV_FILE_LOCATIONS = (
            self.write("ALLODIA_DOCS_BASE_URL=https://from-file.example\n"),
        )
        os.environ[docs_publish.BASE_URL_ENV] = "https://from-env.example"
        self.addCleanup(os.environ.pop, docs_publish.BASE_URL_ENV, None)
        settings = docs_publish.resolve_settings(base_url="http://localhost:3999")
        self.assertEqual(settings.base_url, "http://localhost:3999")
        self.assertEqual(settings.base_url_source, "--base-url")

    def test_one_file_can_carry_both_the_token_and_the_target(self):
        docs_publish.ENV_FILE_LOCATIONS = (
            self.write(
                "ALLODIA_DOCS_UPLOAD_TOKEN=tok\nALLODIA_DOCS_BASE_URL=https://staging.example\n"
            ),
        )
        settings = docs_publish.resolve_settings()
        self.assertEqual((settings.token, settings.base_url), ("tok", "https://staging.example"))

    def test_the_source_of_a_non_default_target_is_recorded(self):
        path = self.write("ALLODIA_DOCS_BASE_URL=https://staging.example\n")
        docs_publish.ENV_FILE_LOCATIONS = (path,)
        # Naming the *file* is the point: "not the default" is not actionable, "this file said so"
        # is the thing you go and fix.
        self.assertEqual(docs_publish.resolve_settings().base_url_source, str(path))


class TargetAnnouncement(Harness):
    def test_a_non_default_target_is_announced_with_its_source(self):
        digest = sha(webp(b"x"))
        self.write_manifest({("setup-email", "macos", "en"): digest})
        store = FakeStore(already=[digest])
        store.base_url_source = "~/.config/allodia/docs.env"

        _, out, _ = self.run_publish(store, check=True)

        self.assertIn("not the default", out)
        self.assertIn("https://example.test", out)
        self.assertIn("docs.env", out)

    def test_the_live_site_is_not_announced(self):
        digest = sha(webp(b"x"))
        self.write_manifest({("setup-email", "macos", "en"): digest})
        store = FakeStore(already=[digest])
        store.base_url = docs_publish.DEFAULT_BASE_URL
        store.base_url_source = "the default"

        _, out, _ = self.run_publish(store, check=True)

        # Every normal run would carry this line otherwise, and a warning that fires always is a
        # warning nobody reads.
        self.assertNotIn("not the default", out)


class CredentialsFileIsIgnored(unittest.TestCase):
    def test_the_repo_local_location_is_gitignored(self):
        # The script offers `.docs.env` *because* .gitignore covers it. If that line ever goes, the
        # script starts recommending a path that `git add -A` will commit a publish token from.
        shared, per_tool, home = docs_publish.ENV_FILE_LOCATIONS
        self.assertEqual(shared.name, ".env")
        self.assertEqual(per_tool.name, ".docs.env")
        # The third is outside every checkout, for a secret you would rather not keep in one.
        self.assertEqual(home.parent.name, "allodia")
        self.assertNotIn(str(docs_publish.REPO_ROOT), str(home))
        for path in (shared, per_tool):
            ignored = subprocess.run(
                ["git", "check-ignore", str(path)],
                cwd=str(docs_publish.REPO_ROOT),
                capture_output=True,
                text=True,
            )
            self.assertEqual(ignored.returncode, 0, "%s is NOT gitignored" % path.name)

    def test_the_shared_location_is_carried_into_every_worktree(self):
        # `.worktreeinclude` copies only files that are BOTH listed there and gitignored, so this
        # pairs with the assertion above. Break either half and a new worktree comes up with no
        # credentials; which looks exactly like a token that stopped working.
        listed = (docs_publish.REPO_ROOT / ".worktreeinclude").read_text(encoding="utf-8")
        entries = [
            line.strip()
            for line in listed.splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        self.assertIn(".env", entries)


if __name__ == "__main__":
    unittest.main()
