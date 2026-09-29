// The learning sheet's steps (docs/ai.md, "Learning"), driven without a screen: what it asks,
// what it reads, when pressing Learn is allowed to send anything, and what happens to an answer
// that arrives after the person moved on.
package eu.allodia.mailcal

import java.time.LocalDate
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.WritingStyleFailure

private val AMSTERDAM: ZoneId = ZoneId.of("Europe/Amsterdam")

@RunWith(RobolectricTestRunner::class)
class WritingStyleLearnFlowTest {

    private val core = FakeWritingStyleActions()

    private fun flow(
        accounts: List<uniffi.mailcal_bindings.AccountWritingStyleRow> = listOf(ALICE_ACCOUNT),
        background: Background = ImmediateBackground,
    ) = LearnFlow(accounts, core, background, AMSTERDAM).also { it.start() }

    @Test
    fun one_account_is_not_asked_about_and_everything_on_the_device_is_read() {
        val flow = flow()
        assertEquals(listOf(FakeWritingStyleActions.Asked(ALICE_ACCOUNT.accountId, null, null)), core.reports)
        val step = flow.step as LearnStep.Consent
        assertEquals(LearnRange.Everything, step.range)
        assertEquals(CorpusState.Ready(corpusReport()), step.corpus)
    }

    @Test
    fun several_accounts_are_asked_about_first_and_nothing_is_read_before() {
        val flow = flow(accounts = listOf(ALICE_ACCOUNT, BOB_ACCOUNT))
        assertEquals(LearnStep.ChooseAccount(listOf(ALICE_ACCOUNT, BOB_ACCOUNT)), flow.step)
        assertTrue(core.reports.isEmpty())

        flow.chooseAccount(BOB_ACCOUNT.accountId)
        assertEquals(BOB_ACCOUNT.accountId, core.reports.single().account)
    }

    /** "Up to and including" the chosen day: the core's `until` is its last second, locally. */
    @Test
    fun up_to_a_date_reads_to_the_end_of_that_day_in_the_display_zone() {
        val flow = flow()
        flow.chooseRange(LearnRange.Until(LocalDate.of(2024, 12, 31)))
        assertEquals(FakeWritingStyleActions.Asked(ALICE_ACCOUNT.accountId, null, 1_735_685_999L), core.reports.last())
        assertEquals(LearnRange.Until(LocalDate.of(2024, 12, 31)), (flow.step as LearnStep.Consent).range)
    }

    @Test
    fun nothing_usable_offers_nothing_to_consent_to() {
        core.reportAnswer = { corpusReport(usable = 0u) }
        val flow = flow()
        assertFalse(flow.canLearn)
        flow.consent("My style", "en")
        assertTrue("nothing was sent", core.learns.isEmpty())
        assertTrue(flow.step is LearnStep.Consent)
    }

    /** Learn is offered only once the report is in and says enough; not while it is being read. */
    @Test
    fun learn_is_offered_only_over_a_report_with_something_to_learn_from() {
        val held = HeldBackground()
        val flow = flow(background = held)
        assertFalse("still reading", flow.canLearn)
        held.release()
        assertTrue(flow.canLearn)
        flow.consent("My style", "en")
        assertFalse("already learning", flow.canLearn)
    }

    /** Pressing Learn on the sheet is the consent, and the only thing that sends anything. */
    @Test
    fun consenting_learns_the_range_under_the_default_name_in_the_language_on_screen() {
        val flow = flow()
        flow.chooseRange(LearnRange.Until(LocalDate.of(2024, 12, 31)))
        assertTrue("reading sent nothing", core.learns.isEmpty())

        flow.consent("My style", "nl")
        assertEquals(
            listOf(FakeWritingStyleActions.Learned(ALICE_ACCOUNT.accountId, 1_735_685_999L, "My style", "nl")),
            core.learns,
        )
        assertEquals(LearnStep.Learned(PLAIN_STYLE.id), flow.step)
    }

    @Test
    fun a_failed_run_says_which_failure() {
        core.learnAnswer = { throw WritingStyleFailure.OutOfCredits() }
        val flow = flow()
        flow.consent("My style", "en")
        val failed = flow.step as LearnStep.Failed
        assertTrue(failed.failure is WritingStyleFailure.OutOfCredits)
    }

    @Test
    fun an_account_with_no_sent_folder_says_so_on_the_sheet() {
        core.reportAnswer = { throw WritingStyleFailure.NoSentFolder() }
        val flow = flow()
        val corpus = (flow.step as LearnStep.Consent).corpus as CorpusState.Failed
        assertTrue(corpus.failure is WritingStyleFailure.NoSentFolder)
    }

    /** Stop asks the core to stop; the run then answers `Cancelled`, which the sheet shows. */
    @Test
    fun stopping_a_run_cancels_it_and_shows_that_it_stopped() {
        val held = HeldBackground()
        val flow = flow(background = held)
        held.release()
        flow.stop()
        assertEquals("nothing is running yet", 0, core.cancels)

        core.learnAnswer = { throw WritingStyleFailure.Cancelled() }
        flow.consent("My style", "en")
        assertTrue(flow.step is LearnStep.Learning)
        flow.stop()
        assertEquals(1, core.cancels)
        held.release()
        assertTrue((flow.step as LearnStep.Failed).failure is WritingStyleFailure.Cancelled)
    }

    /** A report for a range the person has since changed is not drawn over the one they chose. */
    @Test
    fun a_report_for_an_earlier_range_is_dropped() {
        val held = HeldBackground()
        var answers = listOf(corpusReport(usable = 5u), corpusReport(usable = 9u))
        core.reportAnswer = { answers.first().also { answers = answers.drop(1) } }
        val flow = flow(background = held)
        flow.chooseRange(LearnRange.Until(LocalDate.of(2024, 12, 31)))
        held.release()

        val step = flow.step as LearnStep.Consent
        assertEquals(LearnRange.Until(LocalDate.of(2024, 12, 31)), step.range)
        assertEquals(9u, (step.corpus as CorpusState.Ready).report.usable)
    }

    /** Closing the sheet stops a run and leaves nothing to land on a sheet nobody can see. */
    @Test
    fun closing_during_a_run_cancels_it_and_ignores_its_answer() {
        val held = HeldBackground()
        val flow = flow(background = held)
        held.release()
        flow.consent("My style", "en")
        flow.close()
        assertEquals(1, core.cancels)

        held.release()
        assertTrue("the late answer was not drawn", flow.step is LearnStep.Learning)
    }
}
