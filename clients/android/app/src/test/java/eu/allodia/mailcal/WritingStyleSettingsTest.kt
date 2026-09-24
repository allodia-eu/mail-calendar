// Settings → Writing style and Advanced → Own AI endpoint, driven through the real composables
// against a core that answers from memory (docs/ai.md, docs/settings.md rows 7 and 11).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CreditBalance
import uniffi.mailcal_bindings.GateRefusal
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.JurisdictionMode
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleSnapshot

private val BALANCE = CreditBalance(credits = 12.5, asOf = 1_788_256_800)

private val LOCAL_ENDPOINT = OwnAiEndpoint("http://127.0.0.1:28434/v1", "mock", JurisdictionClass.EU_NATIVE, hasKey = true)

@RunWith(RobolectricTestRunner::class)
class WritingStyleSettingsTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeWritingStyleActions()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private fun show(snapshot: WritingStyleSnapshot, endpoint: OwnAiEndpoint? = LOCAL_ENDPOINT) {
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                WritingStyleCategory(
                    WritingStyleSettings(snapshot, endpoint, core, ImmediateBackground),
                    activeZoneId = "Europe/Amsterdam",
                )
            }
        }
    }

    @Test
    fun an_empty_library_says_so_and_offers_to_learn() {
        show(writingStyleSnapshot())
        compose.onNodeWithText(L10n.writing_style_intro(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.writing_style_empty(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.writing_style_accounts_heading(ctx())).assertIsDisplayed()
    }

    /** A refusal is known before anything is read, so Learn is not offered to meet it. */
    @Test
    fun a_refusal_takes_the_place_of_the_learn_button() {
        show(writingStyleSnapshot(refused = GateRefusal(JurisdictionMode.EU_NATIVE, JurisdictionClass.NON_EU)))
        compose.onNodeWithText(L10n.writing_style_refused_eu_native(ctx())).assertIsDisplayed()
        compose.onAllNodesWithText(L10n.writing_style_learn(ctx())).assertCountEquals(0)
    }

    @Test
    fun the_relay_s_credits_are_shown() {
        show(writingStyleSnapshot(route = AiRoute.RELAY, balance = BALANCE))
        compose.onNodeWithText("12.5 credits left, as of", substring = true).assertIsDisplayed()
    }

    /** An own endpoint is not metered by Allodia, so a stored balance says nothing about it. */
    @Test
    fun an_own_endpoint_shows_no_credits() {
        show(writingStyleSnapshot(route = AiRoute.OWN_ENDPOINT, balance = BALANCE))
        compose.onAllNodesWithText("credits left", substring = true).assertCountEquals(0)
    }

    @Test
    fun an_account_is_given_a_style_from_its_picker() {
        show(writingStyleSnapshot(styles = listOf(PLAIN_STYLE)))
        compose.onNodeWithText(ALICE_ACCOUNT.email).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(L10n.writing_style_none(ctx())).performScrollTo().performClick()
        compose.onAllNodesWithText(PLAIN_STYLE.name)[1].performClick()
        assertEquals(listOf(ALICE_ACCOUNT.accountId to PLAIN_STYLE.id), core.assigned)
    }

    private fun next() = compose.onNodeWithText(L10n.wizard_next(ctx())).performClick()

    /**
     * From the library row through the reveal's pages, the account named by its address, and Save
     * on the last page renames and keeps the notes in one go.
     */
    @Test
    fun a_style_opens_its_reveal_and_saves_name_and_notes_together() {
        show(writingStyleSnapshot(styles = listOf(PLAIN_STYLE)))
        compose.onNodeWithText("Learned from 12 messages", substring = true).performClick()

        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.reveal_based_on(ctx(), address = ALICE_ACCOUNT.email)).assertIsDisplayed()
        compose.onNodeWithContentDescription(L10n.a11y_wizard_step(ctx(), step = "1", total = "6")).assertIsDisplayed()
        next()
        compose.onNodeWithText(L10n.reveal_step_letter(ctx())).assertIsDisplayed()
        compose.onNodeWithText("Hi Anna,").performScrollTo().assertIsDisplayed()
        compose.onNodeWithContentDescription("Usually about 80 words").performScrollTo().assertIsDisplayed()
        next()
        compose.onNodeWithText(L10n.reveal_step_habits(ctx())).assertIsDisplayed()
        // The greeting and the sign-off, each worded by its own share.
        compose.onAllNodesWithText(L10n.reveal_frequency_mostly(ctx())).assertCountEquals(2)
        repeat(3) { next() }
        compose.onNodeWithText(L10n.reveal_step_name(ctx())).assertIsDisplayed()
        compose.onAllNodesWithText(L10n.wizard_next(ctx())).assertCountEquals(0)
        compose.onNodeWithText(L10n.reveal_save(ctx())).performClick()

        assertEquals(listOf(Triple(PLAIN_STYLE.id, PLAIN_STYLE.name, PLAIN_DETAIL.notes)), core.saved)
    }

    /** The counts are read out as one sentence, with the full date the "Since" figure shortens. */
    @Test
    fun the_first_page_reads_its_counts_as_one_sentence() {
        val oldest = 1_782_302_400L
        core.detailAnswer = PLAIN_DETAIL.copy(row = PLAIN_STYLE.copy(oldest = oldest))
        show(writingStyleSnapshot(styles = listOf(PLAIN_STYLE)))
        compose.onNodeWithText("Learned from 12 messages", substring = true).performClick()

        val locale = ctx().resources.configuration.locales[0]
        val date = epochDate(oldest, displayZone("Europe/Amsterdam"), locale)
        val languages = writingStyleLanguages(ctx(), listOf("en"))
        compose.onNodeWithContentDescription(L10n.reveal_stats_a11y(ctx(), count = 12, date = date, languages = languages))
            .assertIsDisplayed()
    }

    /** Back steps back a page, and is not offered on the first. */
    @Test
    fun back_steps_back_through_the_reveal() {
        show(writingStyleSnapshot(styles = listOf(PLAIN_STYLE)))
        compose.onNodeWithText("Learned from 12 messages", substring = true).performClick()
        compose.onAllNodesWithText(L10n.wizard_back(ctx())).assertCountEquals(0)
        next()
        compose.onNodeWithText(L10n.wizard_back(ctx())).assertIsEnabled().performClick()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
    }

    @Test
    fun forgetting_a_style_asks_first() {
        show(writingStyleSnapshot(styles = listOf(PLAIN_STYLE)))
        compose.onNodeWithText("Learned from 12 messages", substring = true).performClick()
        repeat(5) { next() }
        compose.onNodeWithText(L10n.writing_style_forget(ctx())).performScrollTo().performClick()
        assertTrue("nothing forgotten before the answer", core.forgotten.isEmpty())

        compose.onNodeWithText(L10n.writing_style_forget_title(ctx(), PLAIN_STYLE.name)).assertIsDisplayed()
        // The dialog's confirm, not the button that opened it.
        compose.onAllNodesWithText(L10n.writing_style_forget(ctx()))[1].performClick()
        assertEquals(listOf(PLAIN_STYLE.id), core.forgotten)
    }

    /**
     * The sheet reads what the device holds, says what will be sent where (naming the own
     * endpoint's host), and only Learn sends; the style it learned opens straight afterwards.
     */
    @Test
    fun learning_reports_asks_consent_then_shows_the_style() {
        show(writingStyleSnapshot())
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).performClick()

        compose.onNodeWithText(L10n.learn_range_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_range_all(ctx())).assertIsDisplayed()
        next()
        compose.onNodeWithText("Long enough to learn from: 30").assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_consent_own(ctx(), host = "127.0.0.1")).performScrollTo()
            .assertIsDisplayed()
        assertTrue("reading sent nothing", core.learns.isEmpty())

        compose.onNodeWithText(L10n.learn_consent_confirm(ctx())).performClick()
        assertEquals(L10n.writing_style_default_name(ctx()), core.learns.single().name)
        assertEquals("en", core.learns.single().uiLanguage)
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
    }

    /** With a choice of account, the sheet asks first, and Next waits for the answer. */
    @Test
    fun several_accounts_are_asked_about_on_a_page_of_their_own() {
        show(writingStyleSnapshot(accounts = listOf(ALICE_ACCOUNT, BOB_ACCOUNT)))
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).performClick()
        compose.onNodeWithText(L10n.learn_account_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.wizard_next(ctx())).assertIsNotEnabled()
        // The sheet's row, not the account's style picker behind the sheet.
        compose.onNode(hasText(BOB_ACCOUNT.email) and hasClickAction()).performClick()
        next()
        compose.onNodeWithText(L10n.learn_range_title(ctx())).assertIsDisplayed()
        assertEquals(BOB_ACCOUNT.accountId, core.reports.single().account)
    }

    @Test
    fun a_run_that_fails_says_no_style_was_learned_and_why() {
        core.learnAnswer = { throw uniffi.mailcal_bindings.WritingStyleFailure.OutOfCredits() }
        show(writingStyleSnapshot(route = AiRoute.RELAY))
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).performClick()
        next()
        compose.onNodeWithText(L10n.learn_consent_relay(ctx())).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_consent_confirm(ctx())).performClick()

        compose.onNodeWithText(L10n.learn_failed_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.ai_error_out_of_credits(ctx())).assertIsDisplayed()
        compose.onAllNodesWithText(L10n.wizard_back(ctx())).assertCountEquals(0)
    }

    @Test
    fun nothing_usable_says_so_and_offers_no_learn_button() {
        core.reportAnswer = { corpusReport(usable = 0u) }
        show(writingStyleSnapshot())
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).performClick()
        next()
        compose.onNodeWithText(L10n.learn_report_nothing(ctx())).performScrollTo().assertIsDisplayed()
        compose.onAllNodesWithText(L10n.learn_consent_confirm(ctx())).assertCountEquals(0)
    }

    /** An empty key field keeps the stored key; a refusal names the field to fix. */
    @Test
    fun saving_the_endpoint_keeps_the_stored_key_and_words_a_refusal() {
        core.endpointRefusal = OwnEndpointException.NotHttps()
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                OwnAiEndpointCard(LOCAL_ENDPOINT, core::saveEndpoint, core::removeEndpoint)
            }
        }
        compose.onNodeWithText(L10n.ai_endpoint_key_stored(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.ai_endpoint_where_eu_hosted(ctx())).performScrollTo().performClick()
        compose.onNodeWithText(L10n.ai_endpoint_save(ctx())).performScrollTo().performClick()

        assertEquals(
            FakeWritingStyleActions.EndpointSaved(LOCAL_ENDPOINT.baseUrl, "mock", JurisdictionClass.EU_HOSTED, null),
            core.endpoints.single(),
        )
        compose.onNodeWithText(L10n.ai_endpoint_error_https(ctx())).performScrollTo().assertIsDisplayed()

        compose.onNodeWithText(L10n.ai_endpoint_key(ctx())).performTextInput("sk-new")
        compose.onNodeWithText(L10n.ai_endpoint_save(ctx())).performScrollTo().performClick()
        assertEquals("sk-new", core.endpoints.last().key)

        compose.onNodeWithText(L10n.ai_endpoint_remove(ctx())).performScrollTo().performClick()
        assertEquals(1, core.removals)
    }
}
