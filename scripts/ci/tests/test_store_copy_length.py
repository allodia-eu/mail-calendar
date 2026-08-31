#!/usr/bin/env python3
"""Unit tests for the store-copy length check.

The check's real risk is not miscounting; it is **scraping nothing**. It reads two hand-written
markdown documents, and a heading someone rewords could turn it into a program that measures zero
fields and reports success. So most of what is tested here is that it *fails*: over-long copy,
a missing section, a reworded table, a locale with no keystore token.

The fixtures are miniature documents rather than the real ones, so a legitimate edit to the copy
never breaks these tests, and a change to the document's *shape* always does.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import changelog_fragments as fragments_module
import check_store_copy_length as subject


# The `{KEYSTORE}` substitution, as the document carried it while the copy named each platform's
# own secret store. Kept as a fixture constant rather than inlined, because the table is now
# **optional**; the copy says "stored securely on your device"; and the tests below have to be
# able to build a document both with and without it.
KEYSTORE_SECTION = """### `{KEYSTORE}` token — the one substitution in the shared body

| Store (platform) | English |
|---|---|
| Microsoft Store (Windows) | the Windows Credential Manager |
| App Store Connect (Apple) | your device's Keychain |
| Google Play (Android) | the Android Keystore |"""


def audit(document: str):
    """Measure a fixture, whose limits table and copy still share one document.

    The shipped pair is two files -- `docs/store-listing.md` holds the caps and the resolved
    `branding/<brand>-listing.md` holds the copy -- but a fixture that split them would need two
    strings to state one case, and every test below is about the copy.
    """
    return subject.audit_listing(document, subject.parse_limits(document))


def listing(
    *,
    keystore: str = KEYSTORE_SECTION,
    description: str = "Short body with {KEYSTORE} in it.",
    limits_row: str = "| App Store Connect | 30 | 30 | 170 (Promo) | 4,000 | — | Keywords: 100 | 4,000 |",
    search_terms: str = "one term\ntwo term",
    apple_block: str = "Subtitle:        Sovereign email\nPromotional:     Short promo.\nKeywords:        mail,calendar",
    features: str = "One feature line",
    short_description: str = "Sovereign, private email.",
) -> str:
    """A miniature store-listing.md with one language, shaped exactly like the real one."""
    return f"""# App-store listing

## Field limits

| Store | Name/Title | Subtitle | Short/Promo | Description | Feature list | Search terms | What's new |
|---|---|---|---|---|---|---|---|
| Microsoft Store | 256 | — | — | 10,000 | up to 20 × ~200 | up to 7 × 30 (21 words) | 1,500 |
{limits_row}
| Google Play | 30 | — | 80 (Short desc) | 4,000 | — | — | 500 (release notes) |

## Shared description — English

> Paste verbatim.

```
{description}
```

## Per-store fields

{keystore}

### Microsoft Store — Product features (one per field, ≤ ~200 chars, max 20)

**English**
```
{features}
```

### Microsoft Store — Search terms (up to 7 per language)

**English**
```
{search_terms}
```

### App Store Connect — Subtitle (≤30), Promotional text (≤170), Keywords (≤100)

**English**
```
{apple_block}
```

### Google Play — Short description (≤80)

**English**
```
{short_description}
```
"""


# A miniature catalog: the real one has seven locales, and none of these rules is about how many.
LANGUAGES = ("English",)


def fragment_text(
    *, platforms: str = "all", bump: str = "patch", note: str = "A short release note."
) -> str:
    """The markdown of one pending fragment, shaped exactly like a real one."""
    return f"""# A change a user can see

Platforms: {platforms}
Bump: {bump}

> Why it is shaped this way. Never measured.

**English**

```
{note}
```
"""


def fragment(slug: str = "a-change", **kwargs) -> fragments_module.Fragment:
    return fragments_module.parse_fragment(fragment_text(**kwargs), slug, LANGUAGES)


def release(*, platforms: str = "all", note: str = "A short release note.") -> str:
    """A miniature released/X.Y.Z.md."""
    return f"""# 0.1.0 — 2026-01-01

## {platforms}

Paste into: whatever

**English**

```
{note}
```

## {fragments_module.APPENDIX_HEADING}

### A change — `a-change` (all, patch)

> Commentary, which is not copy and must not be measured.
"""


class ParsesTheLimitsTable(unittest.TestCase):
    """The table is the source of truth, so reading it wrong is reading every limit wrong."""

    def test_reads_each_store_row(self) -> None:
        limits = subject.parse_limits(listing())
        self.assertEqual(limits["Microsoft Store"].description, 10_000)
        self.assertEqual(limits["App Store Connect"].description, 4_000)
        self.assertEqual(limits["Google Play"].whats_new, 500)

    def test_em_dash_means_no_such_field(self) -> None:
        self.assertIsNone(subject.parse_limits(listing())["Microsoft Store"].subtitle)

    def test_feature_cell_yields_a_count_and_a_size(self) -> None:
        microsoft = subject.parse_limits(listing())["Microsoft Store"]
        self.assertEqual((microsoft.feature_count, microsoft.feature_chars), (20, 200))

    def test_apple_keywords_share_the_search_terms_column(self) -> None:
        apple = subject.parse_limits(listing())["App Store Connect"]
        self.assertEqual(apple.keywords, 100)
        # The "100" must not be mistaken for a count of terms or a per-term size: Apple's keywords
        # are one 100-character field, not a list, and it has no feature list at all.
        self.assertIsNone(apple.feature_chars)
        self.assertIsNone(apple.search_count)
        self.assertIsNone(apple.search_chars)

    def test_the_search_cell_yields_a_count_a_size_and_a_word_budget(self) -> None:
        microsoft = subject.parse_limits(listing())["Microsoft Store"]
        self.assertEqual(
            (microsoft.search_count, microsoft.search_chars, microsoft.search_words), (7, 30, 21)
        )

    def test_a_reworded_table_is_an_error_not_a_pass(self) -> None:
        with self.assertRaises(subject.DocumentShapeError):
            subject.parse_limits(listing(limits_row="| App Store Connect | 30 | 4,000 |"))

    def test_a_missing_store_row_is_an_error(self) -> None:
        with self.assertRaises(subject.DocumentShapeError):
            subject.parse_limits(listing(limits_row=""))


class MeasuresTheDescription(unittest.TestCase):
    def test_a_body_within_the_cap_passes(self) -> None:
        self.assertEqual([m for m in audit(listing()) if not m.fits], [])

    def test_a_body_over_the_cap_fails_on_apple_and_play_but_not_microsoft(self) -> None:
        over = [m for m in audit(listing(description="x" * 4_500)) if not m.fits]
        stores = sorted(item.where.split(" / ")[-1] for item in over)
        self.assertEqual(stores, ["App Store Connect", "Google Play"])

    def test_the_keystore_token_is_counted_as_what_replaces_it(self) -> None:
        """The token is 10 characters and every substitution is longer, so a body that fits with
        the token in it can still be rejected by the console."""
        # 3,995 + "{KEYSTORE}" (10) = 4,005 raw, but the token is what shrinks: 3,995 + 20 = 4,015.
        body = "x" * 3_995 + subject.KEYSTORE_TOKEN
        measured = {m.where: m for m in audit(listing(description=body))}
        play = measured["Description / English / Google Play"]
        self.assertEqual(play.actual, 3_995 + len("the Android Keystore"))
        self.assertFalse(play.fits)

    def test_a_renamed_description_heading_is_an_error_not_a_pass(self) -> None:
        broken = listing().replace("## Shared description — English", "## The description")
        with self.assertRaises(subject.DocumentShapeError):
            audit(broken)

    def test_a_locale_with_no_keystore_token_is_an_error(self) -> None:
        broken = listing().replace("| App Store Connect (Apple) | your device's Keychain |", "")
        with self.assertRaises(subject.DocumentShapeError):
            audit(broken)

    def test_copy_that_uses_no_token_needs_no_table(self) -> None:
        """The table went away with the token: naming the platform's secret store told a shopper
        nothing, so the body says "stored securely on your device" and there is nothing to
        substitute. What must not change is that the body is still measured."""
        measured = {
            m.where: m
            for m in audit(listing(keystore="", description="A plain body."))
        }
        self.assertEqual(measured["Description / English / Google Play"].actual, len("A plain body."))

    def test_a_token_with_no_table_is_still_an_error(self) -> None:
        # The pairing is what is enforced, not the table's presence: a body carrying `{KEYSTORE}`
        # with nothing to replace it would be measured; and pasted; as a literal brace.
        with self.assertRaises(subject.DocumentShapeError):
            audit(listing(keystore=""))


class MeasuresTheSearchTerms(unittest.TestCase):
    """Three caps on one field, and the one that binds is the one nobody counts by hand."""

    def over(self, **kwargs):
        return {m.where: m for m in audit(listing(**kwargs)) if not m.fits}

    def test_terms_within_every_cap_pass(self) -> None:
        self.assertEqual(self.over(), {})

    def test_an_eighth_term_fails(self) -> None:
        terms = "\n".join(f"term{number}" for number in range(8))
        self.assertIn(
            "Search terms / English / Microsoft Store", self.over(search_terms=terms)
        )

    def test_an_over_long_term_names_its_position(self) -> None:
        self.assertIn(
            "Search term 2 / English / Microsoft Store",
            self.over(search_terms="short one\n" + "x" * 31),
        )

    def test_seven_short_terms_can_still_blow_the_word_budget(self) -> None:
        # This is the whole point of measuring words: no single term here is remotely long, and
        # the count is exactly seven, so every other check passes while the console refuses it.
        terms = "\n".join(f"alpha{n} beta{n} gamma{n} delta{n}" for n in range(7))
        measured = self.over(search_terms=terms)
        self.assertIn("Search words / English / Microsoft Store", measured)
        self.assertEqual(measured["Search words / English / Microsoft Store"].actual, 28)
        # Nothing else complains: seven terms, none of them long.
        self.assertEqual(sorted(measured), ["Search words / English / Microsoft Store"])

    def test_a_repeated_word_is_spent_again(self) -> None:
        # Measured against the live ingestion API on 2026-08-03: seven terms of five words are
        # refused at 35 even though they spend five *distinct* words. The budget is a total.
        # This file asserted the opposite until the Store rejected copy the gate had passed.
        terms = "\n".join("aa bb cc dd ee" for _ in range(7))
        over = self.over(search_terms=terms)
        self.assertEqual(list(over), ["Search words / English / Microsoft Store"])
        self.assertEqual(over["Search words / English / Microsoft Store"].actual, 35)

    def test_a_hyphen_splits_a_word(self) -> None:
        # `privacy-focused` costs two. Our English list read as 20 by whitespace and was refused
        # at 22 for exactly this reason.
        self.assertEqual(
            subject.search_term_words(["privacy-focused client"]), ["privacy", "focused", "client"]
        )

    def test_case_and_punctuation_do_not_change_a_word(self) -> None:
        self.assertEqual(
            subject.search_term_words(["Email client", "email, Client!"]),
            ["email", "client", "email", "client"],
        )

    def test_twenty_one_words_is_accepted_and_twenty_two_is_not(self) -> None:
        # The exact boundary the Store enforces, both sides of it.
        fits = "\n".join("aa bb cc" for _ in range(7))                    # 21
        over = fits + " dd"                                               # 22
        self.assertEqual(self.over(search_terms=fits), {})
        blown = self.over(search_terms=over)
        self.assertEqual(list(blown), ["Search words / English / Microsoft Store"])
        self.assertEqual(blown["Search words / English / Microsoft Store"].actual, 22)

    def test_a_renamed_search_heading_is_an_error_not_a_pass(self) -> None:
        broken = listing().replace("### Microsoft Store — Search terms", "### Keywords")
        with self.assertRaises(subject.DocumentShapeError):
            audit(broken)


class MeasuresTheShortFields(unittest.TestCase):
    def test_an_over_long_subtitle_fails(self) -> None:
        block = "Subtitle:        " + "x" * 31 + "\nPromotional:     ok\nKeywords:        ok"
        over = [m for m in audit(listing(apple_block=block)) if not m.fits]
        self.assertEqual([item.where for item in over], ["Subtitle / English / App Store Connect"])

    def test_an_over_long_play_short_description_fails(self) -> None:
        over = [m for m in audit(listing(short_description="x" * 81)) if not m.fits]
        self.assertEqual(
            [item.where for item in over], ["Short description / English / Google Play"]
        )

    def test_a_missing_apple_field_is_an_error(self) -> None:
        with self.assertRaises(subject.DocumentShapeError):
            audit(listing(apple_block="Subtitle:        Just the one"))

    def test_a_21st_feature_fails_on_count_not_length(self) -> None:
        over = [
            item
            for item in audit(listing(features="\n".join(["short"] * 21)))
            if not item.fits
        ]
        self.assertEqual([item.unit for item in over], ["features"])
        self.assertEqual(over[0].actual, 21)

    def test_an_over_long_feature_fails_on_length(self) -> None:
        over = [m for m in audit(listing(features="x" * 201)) if not m.fits]
        self.assertEqual([item.where for item in over], ["Feature 1 / English / Microsoft Store"])


class MeasuresTheFragments(unittest.TestCase):
    """A fragment's cap comes from the stores its own `Platforms:` reach; not from a constant.

    That is the rule with teeth in it. Holding a Mac-only note to Play's 500 would make authors trim
    copy no console was ever going to see, and holding an Android note to Apple's 4,000 would let a
    501-character note through to the one store that rejects it.
    """

    def limits(self):
        return subject.parse_limits(listing())

    def test_a_change_that_ships_everywhere_is_held_to_plays_500(self) -> None:
        measured = subject.audit_fragments(self.limits(), [fragment()])
        self.assertEqual([item.limit for item in measured], [500])
        self.assertTrue(measured[0].where.endswith("Google Play"))

    def test_an_android_note_over_500_fails(self) -> None:
        over = [
            item
            for item in subject.audit_fragments(
                self.limits(), [fragment(platforms="android", note="x" * 501)]
            )
            if not item.fits
        ]
        self.assertEqual(len(over), 1)
        self.assertEqual(over[0].actual, 501)

    def test_the_same_note_passes_when_it_only_ships_to_the_mac(self) -> None:
        measured = subject.audit_fragments(
            self.limits(), [fragment(platforms="macos", note="x" * 501)]
        )
        self.assertEqual([item.limit for item in measured], [4_000])
        self.assertEqual([item for item in measured if not item.fits], [])

    def test_a_linux_only_note_reaches_no_store_and_is_not_measured(self) -> None:
        self.assertEqual(
            subject.audit_fragments(self.limits(), [fragment(platforms="linux", note="x" * 9_000)]),
            [],
        )

    def test_nothing_pending_is_legal(self) -> None:
        """An empty `unreleased/` means no user-facing change is waiting; not a broken scrape."""
        self.assertEqual(subject.audit_fragments(self.limits(), []), [])

    def test_an_unknown_platform_tag_is_an_error_not_a_pass(self) -> None:
        with self.assertRaises(subject.FragmentError):
            fragment(platforms="macintosh")

    def test_a_bump_that_is_not_minor_or_patch_is_an_error(self) -> None:
        with self.assertRaises(subject.FragmentError):
            fragment(bump="major")

    def test_a_missing_catalog_locale_is_an_error(self) -> None:
        """Every locale ships in every release, so a fragment carries them all; enforced, because
        a translation that lags is a release some users read in a language nobody wrote."""
        with self.assertRaises(subject.FragmentError):
            fragments_module.parse_fragment(
                fragment_text(), "a-change", ("English", "Nederlands")
            )

    def test_a_locale_that_is_not_in_the_catalog_is_an_error(self) -> None:
        """Catches the typo that would otherwise pass: `**Portugues**` is not a missing locale to
        the eye, and without this it would be measured as if it were a language we ship."""
        with self.assertRaises(subject.FragmentError):
            fragments_module.parse_fragment(
                fragment_text().replace("**English**", "**Klingon**"), "a-change", LANGUAGES
            )

    def test_a_fragment_with_no_notes_at_all_is_an_error(self) -> None:
        """The `audit_changelog` discipline this replaced: finding nothing must not read as green."""
        with self.assertRaises(subject.FragmentError):
            fragments_module.parse_fragment(
                "# Headline\n\nPlatforms: all\nBump: patch\n", "a-change", LANGUAGES
            )


class MeasuresTheReleasedNotes(unittest.TestCase):
    def test_a_released_note_is_measured_against_its_own_sections_stores(self) -> None:
        limits = subject.parse_limits(listing())
        parsed = [("0.1.0", fragments_module.parse_release(release(), "0.1.0"))]
        measured = subject.audit_releases(limits, parsed)
        self.assertEqual([item.limit for item in measured], [500])

    def test_a_released_note_over_its_cap_fails(self) -> None:
        limits = subject.parse_limits(listing())
        parsed = [("0.1.0", fragments_module.parse_release(release(note="x" * 501), "0.1.0"))]
        self.assertEqual(len([m for m in subject.audit_releases(limits, parsed) if not m.fits]), 1)

    def test_the_commentary_appendix_is_not_measured(self) -> None:
        """It carries every consumed fragment's rationale; prose, sometimes pages of it, and never
        pasted into a store. Counting it would fail every release on text no console sees."""
        limits = subject.parse_limits(listing())
        with_long_appendix = release().replace(
            "> Commentary, which is not copy and must not be measured.",
            "> " + "x" * 5_000,
        )
        parsed = [("0.1.0", fragments_module.parse_release(with_long_appendix, "0.1.0"))]
        self.assertEqual([m for m in subject.audit_releases(limits, parsed) if not m.fits], [])

    def test_a_section_with_no_notes_is_an_error(self) -> None:
        limits = subject.parse_limits(listing())
        broken = release().replace("**English**", "")
        with self.assertRaises(subject.DocumentShapeError):
            subject.audit_releases(limits, [("0.1.0", fragments_module.parse_release(broken, "0.1.0"))])


class MeasuresAListingWithNoPerStoreFields(unittest.TestCase):
    """The unbranded shape: `branding/default-listing.md`, which promises nothing per store.

    Both halves matter. Dropping the per-store sections must not make the audit find nothing, and
    it must not make a *branded* listing's renamed heading pass either -- `has_per_store_fields`
    is what keeps those two answers apart.
    """

    def neutral(self) -> str:
        """The fixture minus everything under `## Per-store fields`, plus Play's short line.

        The short description comes **before** the shared body, which is the order
        `branding/default-listing.md` uses and not a detail: a `##` section runs to the next `##`,
        so a `###` block placed after the body would be read as a second fenced block inside it.
        """
        # No `{KEYSTORE}`: the token's substitution table lives under the heading being removed,
        # and the neutral listing carries neither. A body still using it there would be pasted as a
        # literal brace, which is why the pairing stays an error rather than a skipped check.
        document = listing(description="A plain body.")
        head, _, tail = document.partition("## Per-store fields")
        body_at = head.index("## Shared description — English")
        short = tail[tail.index("### Google Play — Short description"):]
        return head[:body_at] + short + "\n" + head[body_at:]

    def test_the_body_and_the_short_description_are_still_measured(self) -> None:
        measured = {m.where for m in audit(self.neutral())}
        self.assertIn("Description / English / Google Play", measured)
        self.assertIn("Short description / English / Google Play", measured)

    def test_no_per_store_field_is_measured(self) -> None:
        measured = " ".join(m.where for m in audit(self.neutral()))
        for absent in ("Feature", "Search", "Subtitle", "Keywords", "Promotional"):
            self.assertNotIn(absent, measured)

    def test_a_missing_shared_body_is_still_an_error(self) -> None:
        broken = self.neutral().replace("## Shared description — English", "## Body")
        with self.assertRaises(subject.DocumentShapeError):
            audit(broken)

    def test_a_missing_short_description_is_still_an_error(self) -> None:
        broken = self.neutral().replace("### Google Play — Short description", "### Play blurb")
        with self.assertRaises(subject.DocumentShapeError):
            audit(broken)


class TakesTheNameFromTheInjectedIdentity(unittest.TestCase):
    def test_the_name_measured_is_the_brand_value_not_a_line_of_copy(self) -> None:
        """A name written in the listing too could disagree with the launcher's."""
        name = subject.brand.value("MAILCAL_APP_NAME")
        measured = {m.where: m for m in audit(listing())}
        self.assertEqual(measured["Name / Google Play"].actual, len(name))
        self.assertNotIn("Product name / title", listing())


class ReadsTheRealDocuments(unittest.TestCase):
    """The fixtures above prove the rules; this proves they still point at the real files."""

    def test_the_shipped_copy_is_measured_and_fits(self) -> None:
        limits = subject.parse_limits(subject.LIMITS_PATH.read_text(encoding="utf-8"))
        listing_text = subject.listing_path().read_text(encoding="utf-8")
        measured = (
            subject.audit_listing(listing_text, limits)
            + subject.audit_fragments(limits)
            + subject.audit_releases(limits)
        )
        # A scrape that found nothing would otherwise pass this as loudly as a correct one.
        self.assertGreater(len(measured), 100)
        self.assertEqual([str(item) for item in measured if not item.fits], [])

    def test_the_real_releases_are_read_and_include_the_last_released_version(self) -> None:
        """`audit_releases` would happily measure zero files. /VERSION's own note must be among
        them; the same property `check-version-sync.sh` enforces, asserted where it can be seen."""
        version = (fragments_module.REPO_ROOT / "VERSION").read_text(encoding="utf-8").strip()
        self.assertIn(version, [name for name, _ in fragments_module.load_releases()])


if __name__ == "__main__":
    unittest.main()
