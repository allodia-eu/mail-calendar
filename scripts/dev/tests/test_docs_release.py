#!/usr/bin/env python3
"""Unit tests for the documentation half of cutting a release.

Two things here are worth testing and one is not. The commands are not: `showcase.sh` needs a Mac,
a simulator and fifteen minutes, so every test below hands `recapture`/`publish` a runner that
records what it was asked to do and returns a code. What that buys is the part that actually goes
wrong; the *order* (recapture before the writes, stamp after the bump), and stopping at the first
failure rather than uploading images for a capture pass that died halfway.

The other is the stamp. It edits frontmatter in place, in every page, immediately before a release
is tagged, so the ways it could go wrong are all silent: matching a body line, matching nothing,
matching twice. Those are fixtures, not review.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path, PureWindowsPath

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ci"))

import check_user_docs  # noqa: E402
import docs_release as subject  # noqa: E402

PAGE = """---
title: {title}
description: A page in the fixture tree.
platforms: [{platforms}]
order: 1
updated_for: {updated_for}
---

# {title}

{body}
"""


def page(title="A guide", platforms="macos, android", updated_for="0.4.0", body="Some prose."):
    return PAGE.format(title=title, platforms=platforms, updated_for=updated_for, body=body)


class Recorder:
    """A stand-in for `subprocess.call` that remembers, and can be told to fail on the Nth call."""

    def __init__(self, fail_at=None, code=3):
        self.commands = []
        self.fail_at = fail_at
        self.code = code

    def __call__(self, command):
        self.commands.append(list(command))
        if self.fail_at is not None and len(self.commands) == self.fail_at:
            return self.code
        return 0

    @property
    def shown(self):
        """Each command as a readable string, which is what the assertions are about."""
        return [subject.shown(command) for command in self.commands]


class TreeCase(unittest.TestCase):
    """A miniature docs tree, bent one way per test."""

    def setUp(self):
        holder = tempfile.TemporaryDirectory()
        self.addCleanup(holder.cleanup)
        self.root = Path(holder.name)
        # `recapture` resolves the upload token before it photographs anything; the fixture tree has
        # no credentials file and must not read the developer's.
        patch = unittest.mock.patch.object(
            subject.docs_publish,
            "resolve_settings",
            return_value=subject.docs_publish.Settings("https://example.test", "test", "a-token"),
        )
        patch.start()
        self.addCleanup(patch.stop)

    def build(self, pages=None, version="0.4.0"):
        (self.root / "VERSION").write_text(version + "\n", encoding="utf-8")
        user = self.root / "docs" / "user"
        for locale in check_user_docs.DOC_LOCALES:
            user.joinpath(locale).mkdir(parents=True, exist_ok=True)
        if pages is None:
            pages = {(locale, "setup"): page() for locale in check_user_docs.DOC_LOCALES}
        for (locale, slug), text in pages.items():
            path = user / locale / (slug + ".md")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        return self.root

    def pages(self, **kwargs):
        return check_user_docs.load_pages(self.build(**kwargs))


class DerivesWhatToPhotograph(TreeCase):
    def test_the_platforms_come_from_the_pages_not_from_the_manifest(self):
        pages = {
            (locale, "setup"): page(platforms="android") for locale in check_user_docs.DOC_LOCALES
        }
        pages.update(
            {(locale, "agents"): page(platforms="macos") for locale in check_user_docs.DOC_LOCALES}
        )
        self.assertEqual(subject.declared_platforms(self.pages(pages=pages)), ["macos", "android"])

    def test_the_order_is_the_contracts_not_the_pages(self):
        """So a release prints the same plan whichever page happened to be written first."""
        pages = {
            (locale, "setup"): page(platforms="android, macos")
            for locale in check_user_docs.DOC_LOCALES
        }
        self.assertEqual(subject.declared_platforms(self.pages(pages=pages)), ["macos", "android"])

    def test_one_run_per_platform_and_locale(self):
        commands = subject.capture_commands(["macos", "android"], ("en", "nl"))
        self.assertEqual(
            [subject.shown(command) for command in commands],
            [
                "scripts/dev/showcase.sh macos --set docs --locale en",
                "scripts/dev/showcase.sh macos --set docs --locale nl",
                "scripts/dev/showcase.sh android --set docs --locale en",
                "scripts/dev/showcase.sh android --set docs --locale nl",
            ],
        )

    def test_a_platform_with_no_showcase_mode_is_refused(self):
        """Linux has none, so a Linux page would otherwise photograph nothing and say nothing."""
        with self.assertRaises(subject.DocsReleaseError) as caught:
            subject.capture_commands(["linux"], ("en",))
        self.assertIn("no showcase mode", str(caught.exception))

    def test_a_printed_command_is_slash_separated_on_every_host(self):
        """The assertion above cannot fail off Windows; this one can, anywhere.

        `shown` prints a line a person may retype, and these are `bash` scripts; run through Git
        Bash even on Windows, where `scripts\\dev\\showcase.sh` is not a command anyone can paste.
        On macOS and Linux `str()` and `as_posix()` are the same characters, so the test above
        passed for everyone while the printed plan was wrong on Windows for a month.

        `shown` builds its own `Path`, so the only way to give it a path type whose separator
        differs is to hand it one: hence the two patches. Everything else about the function is
        exercised by the test above; this pins the separator alone.
        """
        root = PureWindowsPath(r"D:\repos\mailcal")
        with unittest.mock.patch.object(subject, "Path", PureWindowsPath):
            with unittest.mock.patch.object(subject, "REPO_ROOT", root):
                shown = subject.shown([str(root / "scripts" / "dev" / "showcase.sh"), "--set"])
        self.assertEqual(shown, "scripts/dev/showcase.sh --set")


class Stamps(TreeCase):
    def test_the_frontmatter_line_is_rewritten(self):
        stamped = subject.stamp_text(page(updated_for="0.3.0"), "0.4.0", "a.md")
        self.assertIn("updated_for: 0.4.0", stamped)
        self.assertNotIn("0.3.0", stamped)

    def test_a_page_that_already_says_so_is_not_touched(self):
        """Returning None rather than identical bytes is what keeps the release diff honest."""
        self.assertIsNone(subject.stamp_text(page(updated_for="0.4.0"), "0.4.0", "a.md"))

    def test_a_body_line_is_left_alone(self):
        """A guide explaining this very field would otherwise be edited by its own release."""
        text = page(updated_for="0.3.0", body="Set `updated_for: 0.1.0` when you capture.")
        stamped = subject.stamp_text(text, "0.4.0", "a.md")
        self.assertIn("updated_for: 0.1.0", stamped)
        self.assertIn("updated_for: 0.4.0", stamped)

    def test_two_updated_for_lines_are_refused(self):
        text = page().replace("order: 1", "order: 1\nupdated_for: 0.2.0")
        with self.assertRaises(subject.DocsReleaseError) as caught:
            subject.stamp_text(text, "0.4.0", "a.md")
        self.assertIn("found 2", str(caught.exception))

    def test_a_page_with_no_frontmatter_is_refused(self):
        with self.assertRaises(subject.DocsReleaseError):
            subject.stamp_text("# Just a heading\n", "0.4.0", "a.md")

    def test_stamping_writes_the_files_and_names_them(self):
        pages = {
            ("en", "setup"): page(updated_for="0.3.0"),
            ("nl", "setup"): page(updated_for="0.4.0"),
        }
        root = self.build(pages=pages)
        self.assertEqual(subject.stamp(root, "0.4.0"), ["docs/user/en/setup.md"])
        self.assertIn(
            "updated_for: 0.4.0", (root / "docs/user/en/setup.md").read_text(encoding="utf-8")
        )


class RefusesADocumentationAheadOfTheRelease(TreeCase):
    def test_a_page_above_the_version_being_cut_is_reported(self):
        pages = {
            (locale, "setup"): page(updated_for="0.9.0") for locale in check_user_docs.DOC_LOCALES
        }
        problems = subject.ahead_of(self.build(pages=pages), "0.5.0")
        self.assertEqual(len(problems), 2)
        self.assertIn("above the release being cut (0.5.0)", problems[0])

    def test_a_page_at_the_version_being_cut_is_fine(self):
        pages = {
            (locale, "setup"): page(updated_for="0.5.0") for locale in check_user_docs.DOC_LOCALES
        }
        self.assertEqual(subject.ahead_of(self.build(pages=pages), "0.5.0"), [])

    def test_a_lagging_page_is_not_this_checks_business(self):
        """Lagging is what `--released` catches, *after* the recapture that would fix it."""
        self.assertEqual(subject.ahead_of(self.build(), "0.5.0"), [])


class RunsThePhasesInOrder(TreeCase):
    def test_recapture_photographs_then_encodes_then_rechecks_the_pages(self):
        run = Recorder()
        self.assertEqual(subject.recapture(self.build(), run, log=lambda _: None), 0)
        self.assertEqual(
            run.shown,
            [
                "scripts/dev/showcase.sh macos --set docs --locale en",
                "scripts/dev/showcase.sh macos --set docs --locale nl",
                "scripts/dev/showcase.sh android --set docs --locale en",
                "scripts/dev/showcase.sh android --set docs --locale nl",
                "python3 scripts/dev/docs_images.py",
                "python3 scripts/ci/check_user_docs.py",
            ],
        )

    def test_a_failed_capture_stops_before_the_manifest_is_rewritten(self):
        """Half a capture pass encoded into the manifest is worse than none: it looks complete."""
        run = Recorder(fail_at=2)
        self.assertEqual(subject.recapture(self.build(), run, log=lambda _: None), 3)
        self.assertEqual(len(run.commands), 2)

    def test_recapture_refuses_without_an_upload_token(self):
        """The phase that needs it runs after the release is written, which is too late to find out."""
        with unittest.mock.patch.object(
            subject.docs_publish,
            "resolve_settings",
            return_value=subject.docs_publish.Settings("https://example.test", "test", None),
        ):
            with self.assertRaises(subject.DocsReleaseError) as caught:
                subject.recapture(self.build(), Recorder(), log=lambda _: None)
        self.assertIn("never published", str(caught.exception))

    def test_a_tree_where_no_page_declares_a_platform_is_refused(self):
        pages = {
            (locale, "index"): page(platforms="") for locale in check_user_docs.DOC_LOCALES
        }
        with self.assertRaises(subject.DocsReleaseError) as caught:
            subject.recapture(self.build(pages=pages), Recorder(), log=lambda _: None)
        self.assertIn("nothing to photograph", str(caught.exception))

    def test_publish_stamps_then_uploads_then_proves_it(self):
        run = Recorder()
        root = self.build(
            pages={
                (locale, "setup"): page(updated_for="0.3.0")
                for locale in check_user_docs.DOC_LOCALES
            }
        )
        self.assertEqual(subject.publish(root, "0.4.0", run, log=lambda _: None), 0)
        self.assertIn(
            "updated_for: 0.4.0", (root / "docs/user/en/setup.md").read_text(encoding="utf-8")
        )
        self.assertEqual(
            run.shown,
            [
                "python3 scripts/dev/docs_publish.py --apply",
                "python3 scripts/dev/docs_publish.py --check",
                "python3 scripts/ci/check_user_docs.py --released",
            ],
        )

    def test_publish_asks_the_site_after_uploading_rather_than_trusting_the_upload(self):
        """`--apply` reports what this machine sent; only `--check` reports what a reader gets."""
        run = Recorder(fail_at=2)
        self.assertEqual(subject.publish(self.build(), "0.4.0", run, log=lambda _: None), 3)
        self.assertEqual(len(run.commands), 2)


if __name__ == "__main__":
    unittest.main()
