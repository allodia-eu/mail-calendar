// The writing assistant's records in words, and the host's controls each stage is given
// (docs/ai.md rules 12, 15 and 27): plain functions, so each is pinned without composing a screen.
package eu.allodia.mailcal

import java.time.ZoneId
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AiEndpointDeclarationGap
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.JurisdictionLimit
import uniffi.mailcal_bindings.JurisdictionRefusal
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningLanguage
import uniffi.mailcal_bindings.StyleLearningReport
import uniffi.mailcal_bindings.StyleLearningStage
import uniffi.mailcal_bindings.WritingStyleRevealPage

@RunWith(RobolectricTestRunner::class)
class WritingStyleWordsTest {
    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun a_refusal_is_worded_by_the_mode_in_force() {
        assertEquals(
            L10n.ai_failure_refused_eu_hosted(ctx()),
            aiFailureText(ctx(), AiFailure.Refused(JurisdictionRefusal.NonEu(JurisdictionLimit.EU_HOSTED))),
        )
        assertEquals(
            L10n.ai_failure_refused_eu_native(ctx()),
            aiFailureText(ctx(), AiFailure.Refused(JurisdictionRefusal.EuHosted)),
        )
        assertEquals(
            "an endpoint that does not say where it runs has words of its own",
            L10n.ai_failure_refused_undeclared(ctx()),
            aiFailureText(ctx(), AiFailure.Refused(JurisdictionRefusal.Unknown(JurisdictionLimit.EU_NATIVE))),
        )
    }

    @Test
    fun the_letter_is_spoken_as_one_sentence_in_order_leaving_out_what_is_empty() {
        val letter = checkNotNull(styleLanguage("en").letter)
        assertEquals(
            listOf(
                // The comma joining the parts stands in for the one the greeting and sign-off end on.
                L10n.a11y_letter_opening(ctx(), "Hi Name"),
                L10n.a11y_letter_words(ctx(), 80),
                L10n.a11y_letter_paragraphs(ctx(), 3),
                L10n.a11y_letter_closing(ctx(), "Best"),
                L10n.a11y_letter_name(ctx(), "Alice"),
            ).joinToString(", "),
            letterSpoken(ctx(), letter),
        )
        val bare = letter.copy(opening = emptyList(), words = 0u, paragraphCount = 0u, signsAs = "")
        assertEquals(
            "the paragraphs are the record's count, never the lines drawn",
            L10n.a11y_letter_closing(ctx(), "Best"),
            letterSpoken(ctx(), bare),
        )
    }

    @Test
    fun the_core_moves_the_flow_for_three_remedies_and_the_client_opens_the_rest() {
        assertEquals(RemedyAction.MOVE_FLOW, remedyAction(AiRemedy.CHOOSE_ACCOUNT))
        assertEquals(RemedyAction.MOVE_FLOW, remedyAction(AiRemedy.CHOOSE_RANGE))
        assertEquals(RemedyAction.MOVE_FLOW, remedyAction(AiRemedy.TRY_AGAIN))
        assertEquals(RemedyAction.SIGN_IN, remedyAction(AiRemedy.SIGN_IN_TO_ALLODIA))
        assertEquals(RemedyAction.OPEN_ENDPOINT, remedyAction(AiRemedy.OPEN_AI_ENDPOINT))
        for (remedy in AiRemedy.entries) assertTrue(aiRemedyText(ctx(), remedy).isNotBlank())
    }

    @Test
    fun progress_counts_only_once_the_run_knows_its_total() {
        assertEquals(L10n.learn_progress_starting(ctx()), learnProgressText(ctx(), 0u, 0u))
        assertNull(learnProgressFraction(0u, 0u))
        assertEquals(L10n.learn_progress(ctx(), "3", "4"), learnProgressText(ctx(), 3u, 4u))
        assertEquals(0.75f, learnProgressFraction(3u, 4u)!!, 0.001f)
    }

    @Test
    fun a_day_is_worded_whole_without_a_zone_moving_it() {
        assertEquals("1 March 2025", dayText("2025-03-01", Locale.UK))
        assertEquals("2025-03-01", millisDay(dayMillis("2025-03-01")!!))
        assertNull(dayMillis("not a day"))
    }

    @Test
    fun the_report_says_each_count_and_leaves_out_what_is_not_there() {
        val report = StyleLearningReport(10u, 4u, listOf(StyleLearningLanguage("en", 4u)), 0u, null, null)
        val lines = learnReportLines(ctx(), report, ZoneId.of("UTC"), Locale.UK)
        assertEquals(
            listOf(
                L10n.learn_report_found(ctx(), 10),
                L10n.learn_report_usable(ctx(), 4),
                L10n.learn_report_languages(ctx(), "English"),
            ),
            lines,
        )
    }

    @Test
    fun a_language_is_named_in_the_reader_s_language() {
        assertEquals("Dutch", languageText("nl", Locale.UK))
        assertEquals("Niederländisch", languageText("nl", Locale.GERMANY))
    }

    @Test
    fun beside_the_form_a_refusal_nobody_declared_is_worded_by_its_mode() {
        assertEquals(
            L10n.ai_endpoint_gap_refused_eu_hosted(ctx()),
            endpointDeclarationGapText(ctx(), AiEndpointDeclarationGap.Refused(JurisdictionRefusal.Unknown(JurisdictionLimit.EU_HOSTED))),
        )
    }

    @Test
    fun dates_follow_the_catalog_s_region_not_the_device_s() {
        assertEquals(Locale.UK, catalogLocale(ctx()))
        assertEquals("19 November 2026", dayText("2026-11-19", catalogLocale(ctx())))
    }

    @Test
    fun a_stage_gets_exactly_the_controls_its_record_gives() {
        val sent = mutableListOf<StyleLearningIntent>()
        val review = learningChrome(ctx(), learning(chooseAccount(), next = HELD), { sent += it }, {})
        assertNull(review.back)
        assertEquals(L10n.wizard_next(ctx()), review.primary?.label)
        assertFalse(review.primary!!.control.enabled)
        assertNull(review.pages)

        val stopping = learningChrome(ctx(), learning(StyleLearningStage.Learning(0u, 0u), stop = FREE), { sent += it }, {})
        assertTrue("Stop is the lesser action", stopping.primary!!.outlined)
        assertEquals(stopping.close, stopping.systemBack())

        val failed = learningChrome(ctx(), learning(StyleLearningStage.Failed(AiFailure.OutOfCredits, null), back = FREE), { sent += it }, {})
        assertNull("no remedy, no primary", failed.primary)
        assertEquals(failed.back, failed.systemBack())
    }

    @Test
    fun the_reveal_s_last_page_saves_and_every_other_moves_on() {
        val reveal = styleReveal()
        val first = revealChrome(ctx(), reveal, 0, "Work", ::fakeStyleNameRefusal, {}, {}, {})
        assertNull(first.back)
        assertEquals(L10n.wizard_next(ctx()), first.primary?.label)
        assertEquals(FlowPages(WritingStyleRevealPage.entries.size, 0), first.pages)

        val last = revealChrome(ctx(), reveal, reveal.pages.size - 1, "", ::fakeStyleNameRefusal, {}, {}, {})
        assertEquals(L10n.reveal_save(ctx()), last.primary?.label)
        assertFalse("a refused name holds Save", last.primary!!.control.enabled)
        assertNotEquals(null, last.back)
    }

    @Test
    fun an_outcome_is_acted_on_once_and_never_the_one_in_force_at_opening() {
        val serials = OutcomeSerials(3uL)
        assertFalse(serials.fresh(3uL))
        assertTrue(serials.fresh(4uL))
        assertFalse(serials.fresh(4uL))
        assertFalse(serials.fresh(null))
    }
}
