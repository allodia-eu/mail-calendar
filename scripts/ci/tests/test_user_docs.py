#!/usr/bin/env python3
"""Unit tests for the user-documentation check.

The check's real risk is the same as the store-copy check's beside it: **scraping nothing**. It
walks a directory tree and reads hand-written frontmatter, so a renamed directory or a reworded
key could turn it into a program that audits zero pages and prints success. So most of what is
tested here is that it *fails*.

The fixtures are miniature trees in a temp directory rather than the real `docs/user/`, so writing
a new guide never breaks these tests, and a change to the tree's *shape* always does.
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path, PureWindowsPath

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_user_docs as subject


PAGE_TEMPLATE = """---
title: {title}
description: A page in the fixture tree.
platforms: [{platforms}]
order: {order}
updated_for: {updated_for}
---

# {title}

{body}
"""

SHOT_BLOCK = """```screenshot
id: shot-a
alt: The thing, shown
```"""


def page(
    title="A guide",
    platforms="macos, android",
    order=1,
    updated_for="0.4.0",
    body="Some prose.",
):
    """One fixture page, valid unless a caller bends a field."""
    return PAGE_TEMPLATE.format(
        title=title, platforms=platforms, order=order, updated_for=updated_for, body=body
    )


def capture(**overrides):
    """One manifest leaf; a single (id, platform, locale) capture."""
    entry = {"sha256": "a" * 64, "width": 1400, "height": 900, "bytes": 84213}
    entry.update(overrides)
    return entry


def manifest(images=None):
    """A manifest carrying `shot-a` on both fixture platforms, in both fixture locales."""
    if images is None:
        images = {
            "shot-a": {
                platform: {locale: capture() for locale in subject.DOC_LOCALES}
                for platform in ("macos", "android")
            }
        }
    return {"version": 1, "generator": "scripts/dev/docs_images.py", "images": images}


DEFAULT_NAV = {
    "version": 1,
    "home": "index",
    "sections": [
        {
            "id": "guides",
            "title": {"en": "Guides", "nl": "Handleidingen"},
            "pages": ["guides/setup"],
        }
    ],
}


def default_pages():
    """A landing page with no screenshots, and one guide that shows `shot-a`."""
    pages = {}
    for locale in subject.DOC_LOCALES:
        pages[(locale, "index")] = page(title="Help", platforms="", order=0, body="Welcome.")
        pages[(locale, "guides/setup")] = page(title="Set up", body=SHOT_BLOCK)
    return pages


class DocsTreeCase(unittest.TestCase):
    """Builds a valid miniature docs tree, which each test then bends in exactly one way."""

    def setUp(self):
        holder = tempfile.TemporaryDirectory()
        self.addCleanup(holder.cleanup)
        self.root = Path(holder.name)

    def build(self, pages=None, nav=None, images=None, version="0.4.0", catalog=None):
        """Write a tree to `self.root` and return it."""
        (self.root / "VERSION").write_text(version + "\n", encoding="utf-8")
        inlang = self.root / "project.inlang"
        inlang.mkdir(parents=True, exist_ok=True)
        inlang.joinpath("settings.json").write_text(
            json.dumps({"locales": catalog if catalog is not None else ["en", "nl", "de"]}),
            encoding="utf-8",
        )
        user = self.root / "docs" / "user"
        # Every locale directory exists even when it holds no pages, so `pages={}` exercises the
        # "found no pages" branch rather than tripping the missing-directory one before it.
        for locale in subject.DOC_LOCALES:
            user.joinpath(locale).mkdir(parents=True, exist_ok=True)
        for (locale, slug), text in (default_pages() if pages is None else pages).items():
            path = user / locale / (slug + ".md")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        user.joinpath("nav.json").write_text(
            json.dumps(DEFAULT_NAV if nav is None else nav), encoding="utf-8"
        )
        user.joinpath("screenshots.json").write_text(json.dumps(manifest(images)), encoding="utf-8")
        return self.root

    def audit(self, released=False, **kwargs):
        return subject.audit(self.build(**kwargs), released=released)

    def assertFailsWith(self, needle, **kwargs):
        """The audit reports at least one problem, and one of them mentions `needle`."""
        problems = self.audit(**kwargs)
        self.assertTrue(problems, "expected a failure mentioning %r, got a clean audit" % needle)
        joined = "\n".join(problems)
        self.assertIn(needle, joined)


class ValidTree(DocsTreeCase):
    def test_a_correct_tree_passes(self):
        self.assertEqual(self.audit(), [])

    def test_external_links_are_not_resolved(self):
        pages = default_pages()
        body = "See [the forum](https://support.allodia.eu/) and [mail](mailto:a@b.c) and [#top](#top)."
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(title="Set up", body=body)
        self.assertEqual(self.audit(pages=pages, images={}), [])

    def test_a_relative_link_between_pages_resolves(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(title="Set up", body="Back to [help](../index.md).")
        self.assertEqual(self.audit(pages=pages, images={}), [])

    def test_a_screenshot_example_in_a_wider_fence_is_not_a_reference(self):
        """`docs/user-docs.md` shows the block inside a four-backtick fence; that is documentation."""
        body = "````markdown\n```screenshot\nid: not-real\nalt: an example\n```\n````"
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(title="Set up", body=body)
        self.assertEqual(self.audit(pages=pages, images={}), [])


class Parity(DocsTreeCase):
    def test_a_page_missing_in_one_locale_fails(self):
        pages = default_pages()
        del pages[("nl", "guides/setup")]
        self.assertFailsWith("docs/user/nl/guides/setup.md is missing", pages=pages)

    def test_a_docs_locale_outside_the_app_catalog_fails(self):
        self.assertFailsWith("not in the app's catalog", catalog=["en", "de"])


class Navigation(DocsTreeCase):
    def test_a_page_in_no_section_fails(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/orphan")] = page(title="Orphan", body="Nothing links here.")
        self.assertFailsWith("is in no nav.json section", pages=pages, images={})

    def test_nav_pointing_at_a_missing_page_fails(self):
        nav = json.loads(json.dumps(DEFAULT_NAV))
        nav["sections"][0]["pages"].append("guides/ghost")
        self.assertFailsWith("does not exist", nav=nav)

    def test_a_page_listed_twice_fails(self):
        nav = json.loads(json.dumps(DEFAULT_NAV))
        nav["sections"][0]["pages"].append("guides/setup")
        self.assertFailsWith("more than once", nav=nav)

    def test_a_section_title_missing_a_locale_fails(self):
        nav = json.loads(json.dumps(DEFAULT_NAV))
        nav["sections"][0]["title"] = {"en": "Guides"}
        self.assertFailsWith("must name every docs locale", nav=nav)

    def test_a_nav_without_sections_is_a_shape_error(self):
        with self.assertRaises(subject.DocumentShapeError):
            self.audit(nav={"version": 1, "home": "index"})


class Frontmatter(DocsTreeCase):
    def test_an_unknown_platform_fails(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up", platforms="macos, blackberry")
        self.assertFailsWith("unknown platform `blackberry`", pages=pages, images={})

    def test_updated_for_ahead_of_version_fails(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up", updated_for="0.9.0")
        self.assertFailsWith("is ahead of /VERSION", pages=pages, images={})

    def test_a_non_version_updated_for_fails(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up", updated_for="next")
        self.assertFailsWith("not a MAJOR.MINOR.PATCH", pages=pages, images={})

    def test_an_empty_title_fails(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="", body="Prose.")
        self.assertFailsWith("`title` is empty", pages=pages, images={})

    def test_an_unknown_frontmatter_key_fails(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up").replace(
            "order: 1", "order: 1\nauthor: someone"
        )
        self.assertFailsWith("unknown frontmatter key(s): author", pages=pages)

    def test_a_page_without_frontmatter_is_a_shape_error(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = "# No frontmatter here\n"
        with self.assertRaises(subject.DocumentShapeError):
            self.audit(pages=pages)


class ReleaseCurrency(DocsTreeCase):
    """`--released`: at a release the pages must describe *this* build, not merely a shipped one.

    The distinction is the whole reason for the flag. Between releases a lagging `updated_for` is
    the normal, correct state; the page was written against the shipped app and the app has not
    shipped since. At a release it means the version moved and the screenshots did not, which no
    reader of the page could ever detect.
    """

    def test_a_lagging_page_passes_the_gate_and_fails_at_release(self):
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up", updated_for="0.3.0", body=SHOT_BLOCK)
        self.assertEqual(self.audit(pages=pages, version="0.4.0"), [])
        self.assertFailsWith("lags /VERSION", released=True, pages=pages, version="0.4.0")

    def test_a_current_tree_passes_both(self):
        self.assertEqual(self.audit(version="0.4.0"), [])
        self.assertEqual(self.audit(released=True, version="0.4.0"), [])

    def test_the_failure_names_the_command_that_fixes_it(self):
        """A release is a bad moment to go looking for which script re-photographs the set."""
        pages = default_pages()
        pages[("nl", "guides/setup")] = page(title="Set up", updated_for="0.3.0", body=SHOT_BLOCK)
        self.assertFailsWith("docs_release.py --apply", released=True, pages=pages, version="0.4.0")

    def test_a_malformed_updated_for_is_left_to_the_frontmatter_check(self):
        """One violation, one message: `next` is not a version, which is already reported."""
        pages = default_pages()
        pages[("en", "guides/setup")] = page(title="Set up", updated_for="next", body=SHOT_BLOCK)
        problems = self.audit(released=True, pages=pages, version="0.4.0")
        self.assertEqual([problem for problem in problems if "lags /VERSION" in problem], [])


class Typography(DocsTreeCase):
    """House style, which is the kind of rule a machine has to hold or nobody does."""

    def test_an_em_dash_in_the_prose_fails(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body="Type your address — then continue.")
        self.assertFailsWith("em dash", pages=pages, images={})

    def test_an_em_dash_in_the_frontmatter_fails(self):
        """A `description:` is copy a search result shows, so it is held to the same rule."""
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(
                title="Set up — quickly", body="Some prose."
            )
        self.assertFailsWith("em dash", pages=pages, images={})

    def test_hyphens_and_en_dashes_are_left_alone(self):
        """Only U+2014 is banned; its neighbours have jobs it does not."""
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body="Open 9–17, app-specific, well-known.")
        self.assertEqual(self.audit(pages=pages, images={}), [])


class Links(DocsTreeCase):
    def test_a_dangling_relative_link_fails(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body="See [gone](./nowhere.md).")
        self.assertFailsWith("resolves to no page", pages=pages, images={})

    def test_a_site_absolute_link_fails(self):
        """`/docs/mail-calendar/x` would break the same link when read on GitHub."""
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body="See [x](/docs/mail-calendar/guides/setup).")
        self.assertFailsWith("must be a relative path to a `.md` page", pages=pages, images={})

    def test_a_link_escaping_the_locale_root_fails(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body="See [x](../../en/index.md).")
        self.assertFailsWith("resolves to no page", pages=pages, images={})


class Screenshots(DocsTreeCase):
    def test_an_id_absent_from_the_manifest_fails(self):
        self.assertFailsWith("is not in docs/user/screenshots.json", images={})

    def test_a_platform_with_no_capture_fails(self):
        """The page claims macOS and Android; the manifest only has macOS."""
        images = {"shot-a": {"macos": {locale: capture() for locale in subject.DOC_LOCALES}}}
        self.assertFailsWith("has no android capture", images=images)

    def test_narrowing_the_block_makes_the_omission_deliberate(self):
        body = "```screenshot\nid: shot-a\nalt: The thing\nplatforms: macos\n```"
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body=body)
        images = {"shot-a": {"macos": {locale: capture() for locale in subject.DOC_LOCALES}}}
        self.assertEqual(self.audit(pages=pages, images=images), [])

    def test_narrowing_to_a_platform_the_page_does_not_declare_fails(self):
        body = "```screenshot\nid: shot-a\nalt: The thing\nplatforms: windows\n```"
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body=body)
        self.assertFailsWith("narrows to platform(s) the page does not declare", pages=pages)

    def test_a_capture_missing_one_locale_fails(self):
        images = manifest()["images"]
        del images["shot-a"]["android"]["nl"]
        self.assertFailsWith("but not in `nl`", images=images)

    def test_a_manifest_entry_missing_its_hash_fails(self):
        images = manifest()["images"]
        del images["shot-a"]["macos"]["en"]["sha256"]
        self.assertFailsWith("is missing sha256", images=images)

    def test_a_block_without_alt_text_fails(self):
        body = "```screenshot\nid: shot-a\n```"
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body=body)
        self.assertFailsWith("has no `alt`", pages=pages)

    def test_a_bare_string_platforms_field_reports_once_not_per_letter(self):
        """`platforms: macos` without brackets is a list error, not five unknown platforms."""
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(body=SHOT_BLOCK).replace(
                "platforms: [macos, android]", "platforms: macos"
            )
        problems = self.audit(pages=pages)
        self.assertTrue(any("must be a list" in problem for problem in problems))
        self.assertFalse(any("unknown platform `m`" in problem for problem in problems))

    def test_a_screenshot_on_a_page_declaring_no_platforms_fails(self):
        pages = default_pages()
        for locale in subject.DOC_LOCALES:
            pages[(locale, "guides/setup")] = page(platforms="", body=SHOT_BLOCK)
        self.assertFailsWith("needs at least one platform", pages=pages)

    def test_an_unreferenced_manifest_entry_fails(self):
        images = manifest()["images"]
        images["shot-orphan"] = {"macos": {locale: capture() for locale in subject.DOC_LOCALES}}
        self.assertFailsWith("which no page references", images=images)


class ScrapingNothing(DocsTreeCase):
    """The failure this checker is most likely to have, and least likely to report."""

    def test_an_empty_tree_is_a_shape_error(self):
        with self.assertRaises(subject.DocumentShapeError) as caught:
            self.audit(pages={})
        self.assertIn("found no pages", str(caught.exception))

    def test_a_renamed_docs_directory_is_a_shape_error(self):
        root = self.build()
        (root / "docs" / "user").rename(root / "docs" / "user-guides")
        with self.assertRaises(subject.DocumentShapeError):
            subject.audit(root)

    def test_a_missing_locale_directory_is_a_shape_error(self):
        root = self.build()
        shutil.rmtree(root / "docs" / "user" / subject.DOC_LOCALES[-1])
        with self.assertRaises(subject.DocumentShapeError) as caught:
            subject.audit(root)
        self.assertIn("has no directory", str(caught.exception))


class PathsReadTheSameEverywhere(unittest.TestCase):
    """A page's `where` is slash-separated on every host, including Windows.

    Written with `PureWindowsPath` **on purpose**. The obvious test; call `relative()` with the
    ordinary `Path` and assert there is no backslash; cannot fail on macOS or Linux, where `str()`
    and `as_posix()` return the same characters whether or not the fix is present. That is exactly
    how the bug survived: six tests in `scripts/dev/tests/` asserted slash-separated paths, passed
    for everyone who ran them, and turned `scripts/dev/gate.sh` red on every Windows checkout. So
    the separator is pinned against a path type that *has* a different one, which any host can run.
    """

    def test_a_windows_flavoured_path_still_comes_back_with_slashes(self):
        root = PureWindowsPath(r"D:\repos\mailcal")
        page = root / "docs" / "user" / "en" / "setup.md"
        self.assertEqual(subject.relative(page, root), "docs/user/en/setup.md")

    def test_a_path_outside_the_root_is_returned_whole_rather_than_raising(self):
        root = PureWindowsPath(r"D:\repos\mailcal")
        self.assertEqual(
            subject.relative(PureWindowsPath(r"C:\elsewhere\page.md"), root),
            str(PureWindowsPath(r"C:\elsewhere\page.md")),
        )


class RealTree(unittest.TestCase):
    """The checker runs clean against the repository it ships in."""

    # `audit` is called directly here, so the skip `main` performs for a tree with no help pages
    # does not apply. The public copy is such a tree: Allodia's pages travel with the brand.
    @unittest.skipUnless((subject.REPO_ROOT / "docs" / "user").is_dir(),
                         "no docs/user/ in this tree, so there are no help pages to audit")
    def test_the_repository_passes_its_own_check(self):
        self.assertEqual(subject.audit(subject.REPO_ROOT), [])


if __name__ == "__main__":
    unittest.main()
