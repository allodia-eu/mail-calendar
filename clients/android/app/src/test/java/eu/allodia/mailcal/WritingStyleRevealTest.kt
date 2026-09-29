// The reveal's steps and the arithmetic behind its pictures (WritingStyleReveal.kt). Each rule is
// one a view gets wrong while looking right: a letter whose lines ignore the paragraph count, a card
// that repeats its heading as its first sentence, a placeholder drawn with its brackets.
package eu.allodia.mailcal

import java.time.Instant
import java.time.ZoneOffset
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.HabitFrequency

@RunWith(RobolectricTestRunner::class)
class WritingStyleRevealTest {
    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun the_language_control_is_on_the_four_pages_about_one_language() {
        assertEquals(6, RevealStep.entries.size)
        assertEquals(
            listOf(RevealStep.LETTER, RevealStep.HABITS, RevealStep.VOICE, RevealStep.PHRASES),
            RevealStep.entries.filter { it.isPerLanguage },
        )
    }

    @Test
    fun the_pager_stays_within_its_pages() {
        val pager = WizardPager(6)
        assertTrue(pager.isFirst && !pager.isLast)
        pager.go(-1)
        assertEquals(0, pager.index)
        pager.go(5)
        assertTrue(pager.isLast)
        pager.go(9)
        assertEquals(5, pager.index)
        pager.go(4)
        assertEquals(4, pager.index)
        assertTrue("a sheet always has a page", WizardPager(0).let { it.isFirst && it.isLast })
    }

    /**
     * Seventy words in two paragraphs is about five lines, the longer paragraph first, and each
     * paragraph ends on a short line.
     */
    @Test
    fun a_letter_draws_its_words_at_about_fifteen_to_a_line() {
        val lines = revealLetterLines(words = 70u, paragraphs = 2u)
        assertEquals(listOf(3, 2), lines.map { it.size })
        for (paragraph in lines) {
            assertTrue(paragraph.last() < 0.8)
            assertTrue(paragraph.dropLast(1).all { it > 0.9 })
        }
    }

    @Test
    fun a_letter_without_a_paragraph_count_draws_two() {
        assertEquals(2, revealLetterLines(words = 90u, paragraphs = 0u).size)
        assertEquals(listOf(2, 2), revealLetterLines(words = 0u, paragraphs = 0u).map { it.size })
    }

    @Test
    fun a_letter_has_a_line_for_every_paragraph_and_room_for_all() {
        assertEquals(listOf(1, 1, 1), revealLetterLines(words = 10u, paragraphs = 3u).map { it.size })
        assertEquals(12, revealLetterLines(words = 900u, paragraphs = 3u).sumOf { it.size })
        assertEquals(6, revealLetterLines(words = 200u, paragraphs = 40u).size)
    }

    @Test
    fun a_card_keeps_the_core_s_heading_over_the_whole_description() {
        val card = revealCardText(" Friendly and direct ", "On first-name terms. Brief.")
        assertEquals(RevealCardText("Friendly and direct", "On first-name terms. Brief."), card)
    }

    @Test
    fun a_card_without_a_heading_leads_with_its_first_sentence_once() {
        assertEquals(
            RevealCardText("Thanks first, then the answer", "Short lines."),
            revealCardText("", "Thanks first, then the answer. Short lines."),
        )
        assertEquals(RevealCardText("Says no politely", ""), revealCardText("", "Says no politely."))
        assertEquals(RevealCardText("", ""), revealCardText("", "  "))
    }

    @Test
    fun a_placeholder_is_a_run_of_its_own_without_its_brackets() {
        assertEquals(
            listOf(RevealRun("Hi ", false), RevealRun("Name", true), RevealRun(",", false)),
            revealRuns("Hi [Name],"),
        )
        assertEquals(listOf(RevealRun("Cheers,", false)), revealRuns("Cheers,"))
        assertEquals(listOf(RevealRun("Name", true)), revealRuns("[Name]"))
        assertEquals(listOf(RevealRun("Hi [] all", false)), revealRuns("Hi [] all"))
    }

    @Test
    fun each_frequency_reads_its_own_key() {
        assertEquals(L10n.reveal_frequency_mostly(ctx()), revealFrequencyText(ctx(), HabitFrequency.MOSTLY))
        assertEquals(L10n.reveal_frequency_often(ctx()), revealFrequencyText(ctx(), HabitFrequency.OFTEN))
        assertEquals(L10n.reveal_frequency_sometimes(ctx()), revealFrequencyText(ctx(), HabitFrequency.SOMETIMES))
    }

    @Test
    fun since_is_the_day_this_year_and_the_month_before_it() {
        // 2026-09-24 12:00 UTC.
        val now = Instant.ofEpochSecond(1_790_251_200)
        // 2026-06-24 and 2025-03-03.
        assertEquals("24 June", revealSinceText(1_782_302_400, now, ZoneOffset.UTC, Locale.UK))
        assertEquals("Mar 2025", revealSinceText(1_740_960_000, now, ZoneOffset.UTC, Locale.UK))
    }

    /** The account is looked up by its id; one from another device, or since removed, names none. */
    @Test
    fun the_source_account_is_named_only_while_it_is_here() {
        assertEquals(ALICE_ACCOUNT.email, revealSourceAddress(PLAIN_STYLE, listOf(BOB_ACCOUNT, ALICE_ACCOUNT)))
        assertNull(revealSourceAddress(PLAIN_STYLE, listOf(BOB_ACCOUNT)))
        assertNull(revealSourceAddress(PLAIN_STYLE.copy(sourceAccount = ""), listOf(ALICE_ACCOUNT)))
    }

    @Test
    fun the_learning_sheet_asks_for_an_account_only_when_there_is_a_choice() {
        assertEquals(
            listOf(LearnPage.RANGE, LearnPage.CONSENT, LearnPage.PROGRESS),
            learnPages(listOf(ALICE_ACCOUNT)),
        )
        assertEquals(LearnPage.ACCOUNT, learnPages(listOf(ALICE_ACCOUNT, BOB_ACCOUNT)).first())
        assertFalse(LearnPage.ACCOUNT in learnPages(emptyList()))
    }
}
