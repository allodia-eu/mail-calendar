#!/usr/bin/env python3
"""Unit tests for release assembly.

Assembly is the one piece of this workflow that is real logic rather than a document read: it picks
the next version, decides which platforms share a bullet set, and orders the bullets. All three are
things you only find out you got wrong by reading a store console; so they are tested here rather
than discovered at release, which is the same reasoning as the store-copy check itself.

The fixtures are miniature fragments, not the real ones, so writing a genuine release note never
breaks these tests and changing the *format* always does.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "ci"))

import changelog_fragments as fragments_module  # noqa: E402
import release as subject  # noqa: E402

LANGUAGES = ("English", "Nederlands")


def fragment(
    slug: str,
    *,
    platforms: str = "all",
    bump: str = "patch",
    note: str = "",
    languages=LANGUAGES,
) -> object:
    body = note or f"What {slug} does."
    blocks = "\n".join(f"**{language}**\n\n```\n{body}\n```\n" for language in languages)
    text = f"""# Headline for {slug}

Platforms: {platforms}
Bump: {bump}

> Why {slug} is shaped this way.

{blocks}"""
    return fragments_module.parse_fragment(text, slug, languages)


class PicksTheNextVersion(unittest.TestCase):
    """`/VERSION` holds the last released version, so the next one is derived, never typed."""

    def test_any_minor_fragment_makes_it_a_minor_release(self) -> None:
        pending = [fragment("fix", bump="patch"), fragment("feature", bump="minor")]
        self.assertEqual(subject.resolve_version("0.2.2", pending, None), ("0.3.0", None))

    def test_only_fixes_makes_it_a_patch_release(self) -> None:
        pending = [fragment("fix", bump="patch"), fragment("other-fix", bump="patch")]
        self.assertEqual(subject.resolve_version("0.2.2", pending, None), ("0.2.3", None))

    def test_a_minor_bump_resets_the_patch_field(self) -> None:
        version, _ = subject.resolve_version("0.13.5", [fragment("f", bump="minor")], None)
        self.assertEqual(version, "0.14.0")

    def test_an_explicit_version_wins_but_warns_when_it_undersells_the_work(self) -> None:
        version, warning = subject.resolve_version("0.2.2", [fragment("f", bump="minor")], "0.2.3")
        self.assertEqual(version, "0.2.3")
        self.assertIn("smaller step", warning)

    def test_an_explicit_version_above_the_computed_one_is_silent(self) -> None:
        self.assertEqual(
            subject.resolve_version("0.2.2", [fragment("f", bump="minor")], "1.0.0"), ("1.0.0", None)
        )

    def test_a_version_that_does_not_climb_is_refused(self) -> None:
        """Every store rejects a version that is not above the last one, so this fails here."""
        with self.assertRaises(subject.ReleaseError):
            subject.resolve_version("0.2.2", [fragment("f")], "0.2.2")

    def test_something_that_is_not_a_version_is_refused(self) -> None:
        with self.assertRaises(subject.ReleaseError):
            subject.resolve_version("0.2.2", [fragment("f")], "v0.3")


class GroupsByBulletSetNotByStore(unittest.TestCase):
    """The property that stops a Mac-only fix appearing in the iPhone's release note."""

    def test_everything_shipping_everywhere_is_one_section(self) -> None:
        sections = fragments_module.group_sections([fragment("a"), fragment("b")])
        self.assertEqual(len(sections), 1)
        self.assertEqual(sections[0].platforms, fragments_module.PLATFORM_ORDER)

    def test_a_mac_only_fragment_splits_the_mac_off(self) -> None:
        sections = fragments_module.group_sections(
            [fragment("everywhere"), fragment("mac-only", platforms="macos")]
        )
        by_platforms = {section.platforms: [f.slug for f in section.fragments] for section in sections}
        self.assertEqual(by_platforms[("macos",)], ["everywhere", "mac-only"])
        self.assertEqual(by_platforms[("ios", "windows", "android", "linux")], ["everywhere"])

    def test_platforms_with_identical_bullets_share_one_section(self) -> None:
        sections = fragments_module.group_sections([fragment("apple", platforms="macos, ios")])
        self.assertEqual([section.platforms for section in sections], [("macos", "ios")])

    def test_a_platform_with_no_bullets_gets_no_section(self) -> None:
        sections = fragments_module.group_sections([fragment("only-android", platforms="android")])
        self.assertEqual([section.platforms for section in sections], [("android",)])

    def test_the_paste_line_names_the_apple_records_separately(self) -> None:
        section = fragments_module.group_sections([fragment("a")])[0]
        self.assertEqual(
            section.paste_line,
            "Microsoft Store · App Store Connect (macOS) · App Store Connect (iOS/iPadOS) "
            "· Google Play",
        )

    def test_a_linux_only_section_says_it_has_no_store(self) -> None:
        section = fragments_module.group_sections([fragment("gtk", platforms="linux")])[0]
        self.assertEqual(section.paste_line, "(no store yet)")


class AssemblesADeterministicDraft(unittest.TestCase):
    def draft(self, pending):
        return subject.assemble("0.3.0", "2026-08-05", pending, LANGUAGES)

    def test_features_come_before_fixes_and_slugs_order_within_each(self) -> None:
        pending = [
            fragment("zeta-fix", bump="patch"),
            fragment("alpha-fix", bump="patch"),
            fragment("zeta-feature", bump="minor"),
            fragment("alpha-feature", bump="minor"),
        ]
        body = self.draft(pending)
        order = [line for line in body.splitlines() if line.startswith("What ")]
        self.assertEqual(
            order[:4],
            [
                "What alpha-feature does.",
                "What zeta-feature does.",
                "What alpha-fix does.",
                "What zeta-fix does.",
            ],
        )

    def test_assembling_twice_produces_the_same_bytes(self) -> None:
        """Otherwise a re-run shows up as a diff and nobody can tell it apart from a real edit."""
        pending = [fragment("b"), fragment("a", bump="minor")]
        self.assertEqual(self.draft(pending), self.draft(list(reversed(pending))))

    def test_every_catalog_locale_gets_a_block(self) -> None:
        body = self.draft([fragment("a")])
        for language in LANGUAGES:
            self.assertIn(f"**{language}**", body)

    def test_the_commentary_survives_the_release_in_the_appendix(self) -> None:
        """The fragments are deleted on release, and their rationale is the expensive half."""
        body = self.draft([fragment("a")])
        self.assertIn(fragments_module.APPENDIX_HEADING, body)
        self.assertIn("Why a is shaped this way.", body)

    def test_the_draft_reads_back_as_a_release(self) -> None:
        """Assembly and the store-copy check must agree about the file, or a note ships unmeasured."""
        pending = [fragment("everywhere"), fragment("mac-only", platforms="macos")]
        sections = fragments_module.parse_release(self.draft(pending), "0.3.0")
        self.assertEqual(
            [section.platforms for section in sections],
            [("macos",), ("ios", "windows", "android", "linux")],
        )
        self.assertEqual(sorted(sections[0].notes), sorted(LANGUAGES))


def summary_file(directory, sections, languages=LANGUAGES):
    """Write a `_summary.md` fixture: `{platform_heading: note}` -> the file, and its path."""
    lines = ["# Release summary", ""]
    for heading, note in sections.items():
        lines.append(f"## {heading}")
        lines.append("")
        for language in languages:
            lines.append(f"**{language}**")
            lines.append("")
            lines.append("```")
            lines.append(note)
            lines.append("```")
            lines.append("")
    path = Path(directory) / "_summary.md"
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


class RefusesToEnumerateMoreThanAStoreAccepts(unittest.TestCase):
    """Play caps "What's new" at 500 characters, and a big release has more changes than that.

    500 divided by fourteen is thirty-five characters a change, which no trim reaches; so the
    arithmetic does not work and the machine must say so rather than write a file the console will
    reject. The escape is an authored section, not a looser cap.
    """

    def crowded(self, count=14, note="A change that took about ninety characters to describe."):
        return [fragment(f"change-{n:02d}", platforms="android", note=note) for n in range(count)]

    def test_a_section_that_cannot_fit_its_store_is_reported(self) -> None:
        problems = subject.over_cap(self.crowded(), LANGUAGES, {})
        self.assertTrue(problems)
        section, _, store, cap, length = problems[0]
        self.assertEqual(section.platforms, ("android",))
        self.assertEqual((store, cap), ("Google Play", 500))
        self.assertGreater(length, cap)

    def test_the_worst_overflow_is_reported_first(self) -> None:
        """The binding language is what an editor has to write to, so it leads the report."""
        pending = self.crowded()
        pending[0].notes["Nederlands"] += "x" * 400
        problems = subject.over_cap(pending, LANGUAGES, {})
        self.assertEqual(problems[0][1], "Nederlands")

    def test_an_authored_summary_replaces_the_section_and_clears_it(self) -> None:
        pending = self.crowded()
        summaries = {("android",): {language: "Lots is new." for language in LANGUAGES}}
        self.assertEqual(subject.over_cap(pending, LANGUAGES, summaries), [])
        body = subject.assemble("0.3.0", "2026-08-05", pending, LANGUAGES, summaries)
        self.assertIn("Lots is new.", body)
        self.assertNotIn("A change that took", fragments_module.parse_release(body, "0.3.0")[0].notes["English"])

    def test_a_summary_that_is_itself_over_cap_still_fails(self) -> None:
        """Otherwise the escape hatch becomes a way to switch the check off."""
        summaries = {("android",): {language: "x" * 600 for language in LANGUAGES}}
        self.assertTrue(subject.over_cap(self.crowded(), LANGUAGES, summaries))

    def test_a_summarized_section_says_it_was_written_rather_than_assembled(self) -> None:
        """A reader comparing it against the appendix would otherwise read it as an omission."""
        summaries = {("android",): {language: "Lots is new." for language in LANGUAGES}}
        body = subject.assemble("0.3.0", "2026-08-05", self.crowded(), LANGUAGES, summaries)
        self.assertIn("Written, not assembled", body)

    def test_a_linux_only_section_is_not_measured(self) -> None:
        """It reaches no console, so measuring it would invent a limit rather than enforce one."""
        pending = [fragment(f"c{n}", platforms="linux", note="x" * 400) for n in range(9)]
        self.assertEqual(subject.over_cap(pending, LANGUAGES, {}), [])

    def test_nothing_is_written_when_a_section_does_not_fit(self) -> None:
        """The half-cut release is the failure mode: assembly deletes the fragments carrying the
        text you would need to write the summary from."""
        listing = lambda where: sorted(path.name for path in where.glob("*.md"))  # noqa: E731
        before = (
            listing(fragments_module.UNRELEASED_DIR),
            listing(fragments_module.RELEASED_DIR),
        )
        # The real catalog, because `main` measures against it rather than a fixture's two locales.
        pending = [
            fragment(
                f"change-{n:02d}",
                platforms="android",
                note="Iets nieuws, in ongeveer negentig tekens verteld.",
                languages=fragments_module.catalog_languages(),
            )
            for n in range(14)
        ]
        with unittest.mock.patch.object(subject, "load_fragments", return_value=pending):
            with unittest.mock.patch.object(subject, "load_summaries", return_value={}):
                self.assertEqual(subject.main(["--date", "2026-08-05"]), 1)
        self.assertEqual(
            before,
            (listing(fragments_module.UNRELEASED_DIR), listing(fragments_module.RELEASED_DIR)),
        )


class ReadsTheSummaryFile(unittest.TestCase):
    """`_summary.md` is authored at release time, so its mistakes need named fixes."""

    def setUp(self) -> None:
        self.directory = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.directory)

    def test_no_file_means_no_summary(self) -> None:
        """The normal case: a release that enumerates its changes needs none."""
        missing = Path(self.directory) / "_summary.md"
        self.assertEqual(fragments_module.load_summaries(missing, LANGUAGES), {})

    def test_a_section_is_keyed_by_its_platform_tuple(self) -> None:
        path = summary_file(self.directory, {"macos, ios": "Lots is new."})
        summaries = fragments_module.load_summaries(path, LANGUAGES)
        self.assertEqual(list(summaries), [("macos", "ios")])
        self.assertEqual(summaries[("macos", "ios")]["English"], "Lots is new.")

    def test_the_heading_is_read_the_same_way_a_release_note_is(self) -> None:
        """`all` in a summary must mean what `## macos, ios, windows, android, linux` means."""
        path = summary_file(self.directory, {"all": "Lots is new."})
        self.assertEqual(
            list(fragments_module.load_summaries(path, LANGUAGES)),
            [fragments_module.PLATFORM_ORDER],
        )

    def test_a_missing_locale_is_refused(self) -> None:
        """A summary replaces the notes wholesale, so a gap is a language that ships nothing."""
        path = summary_file(self.directory, {"android": "Lots is new."}, languages=("English",))
        with self.assertRaises(fragments_module.FragmentError):
            fragments_module.load_summaries(path, LANGUAGES)

    def test_an_unknown_platform_tag_is_refused(self) -> None:
        path = summary_file(self.directory, {"blackberry": "Lots is new."})
        with self.assertRaises(fragments_module.FragmentError):
            fragments_module.load_summaries(path, LANGUAGES)

    def test_a_file_with_no_sections_is_an_error_not_a_silent_none(self) -> None:
        """An empty summary reads exactly like a release that needed none; the one thing it isn't."""
        path = Path(self.directory) / "_summary.md"
        path.write_text("# Release summary\n\nNothing here yet.\n", encoding="utf-8")
        with self.assertRaises(fragments_module.FragmentError):
            fragments_module.load_summaries(path, LANGUAGES)

    def test_the_summary_is_not_read_as_a_fragment(self) -> None:
        """It carries no `Platforms:`/`Bump:`, so a glob that swept it in would fail every release."""
        summary_file(self.directory, {"android": "Lots is new."})
        Path(self.directory, "real.md").write_text(
            "# A change\n\nPlatforms: all\nBump: patch\n\n**English**\n\n```\nx\n```\n\n"
            "**Nederlands**\n\n```\nx\n```\n",
            encoding="utf-8",
        )
        loaded = fragments_module.load_fragments(self.directory, LANGUAGES)
        self.assertEqual([item.slug for item in loaded], ["real"])


class WritesTheIndexRow(unittest.TestCase):
    def test_the_row_links_the_note_and_names_what_shipped(self) -> None:
        row = subject.index_row("0.3.0", "2026-08-05", [fragment("a", bump="minor")])
        self.assertIn("[0.3.0](changelog/released/0.3.0.md)", row)
        self.assertIn("2026-08-05", row)
        self.assertIn("Headline for a", row)

    def test_a_backlog_release_is_summarized_rather_than_listed(self) -> None:
        pending = [fragment(f"change-{n}") for n in range(9)]
        row = subject.index_row("0.3.0", "2026-08-05", pending)
        self.assertIn("…6 more", row)

    def test_the_newest_release_goes_on_top(self) -> None:
        changelog = f"# Notes\n\n{subject.INDEX_MARKER}\n|---|---|---|\n| old | row | here |\n"
        updated = subject.insert_index_row(changelog, "| new |")
        self.assertLess(updated.index("| new |"), updated.index("| old |"))

    def test_a_missing_index_table_is_an_error_not_a_silent_skip(self) -> None:
        """The row is how a release is findable at all; dropping it quietly would hide the release."""
        with self.assertRaises(subject.ReleaseError):
            subject.insert_index_row("# Notes\n\nNo table here.\n", "| new |")


class RefusesWhenThereIsNothingToRelease(unittest.TestCase):
    def test_an_empty_unreleased_directory_stops_the_release(self) -> None:
        """A version number describing no user-facing change is a release nobody can write a note
        for; and `store-copy` passes happily on an empty directory, so nothing else would say so."""
        with unittest.mock.patch.object(subject, "load_fragments", return_value=[]):
            self.assertEqual(subject.main(["--dry-run", "--date", "2026-08-05"]), 1)

    def test_a_dry_run_writes_nothing(self) -> None:
        listing = lambda where: sorted(path.name for path in where.glob("*.md"))  # noqa: E731
        before = (
            listing(fragments_module.UNRELEASED_DIR),
            listing(fragments_module.RELEASED_DIR),
        )
        # The real catalog, because `main` assembles against it rather than a fixture's two locales.
        pending = [fragment("a", bump="minor", languages=fragments_module.catalog_languages())]
        with unittest.mock.patch.object(subject, "load_fragments", return_value=pending):
            self.assertEqual(subject.main(["--dry-run", "--date", "2026-08-05"]), 0)
        self.assertEqual(
            before,
            (listing(fragments_module.UNRELEASED_DIR), listing(fragments_module.RELEASED_DIR)),
        )


class ReadsTheRealFragments(unittest.TestCase):
    """The fixtures prove the rules; this proves they still point at the real directory."""

    def test_the_pending_fragments_assemble(self) -> None:
        pending = fragments_module.load_fragments()
        if not pending:
            self.skipTest("nothing pending: the last release consumed every fragment")
        body = subject.assemble("9.9.9", "2026-01-01", pending)
        self.assertEqual(
            [section.platforms for section in fragments_module.parse_release(body, "9.9.9")],
            [section.platforms for section in fragments_module.group_sections(pending)],
        )


if __name__ == "__main__":
    unittest.main()
