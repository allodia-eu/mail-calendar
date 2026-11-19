// A style shown back once (docs/ai.md rule 15): the pages the core composed and no others, Back
// and Next between them, the language control only where there is a choice, Save held while the
// name is refused, and leaving it as the reveal's own intents. Driven through the real composables.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleOutcome
import uniffi.mailcal_bindings.WritingStylePage
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleResult
import uniffi.mailcal_bindings.WritingStyleReveal
import uniffi.mailcal_bindings.WritingStyleRevealPage

private fun savedReveal(serial: Int, result: WritingStyleResult) = WritingStyleOutcome(
    serial = serial.toULong(),
    kind = WritingStyleChangeKind.SAVE_REVEAL,
    style = "style-1",
    accountId = null,
    result = result,
)

@RunWith(RobolectricTestRunner::class)
class WritingStyleRevealTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<WritingStyleIntent>()
    private var left = 0
    private var reveal by mutableStateOf(styleReveal())
    private var page by mutableStateOf<WritingStylePage>(stylePage())

    private fun show() {
        compose.setContent {
            AppTheme {
                val current: WritingStyleReveal = reveal
                val ui = testWritingStyleUi(page, null, sent, reveal = { current })
                WritingStyleFlow(ui, revealing = "style-1", onRevealLeft = { left++ }, onOpenEndpoint = {})
            }
        }
        compose.waitForIdle()
    }

    private fun next() {
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).performClick()
        compose.waitForIdle()
    }

    private fun pageCount(count: Int) = SemanticsMatcher.expectValue(
        SemanticsProperties.ContentDescription,
        listOf(L10n.a11y_flow_page(ctx(), "1", count)),
    )

    @Test
    fun the_first_page_says_what_was_read_and_has_no_way_back() {
        show()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.reveal_based_on(ctx(), "alice@test.local")).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertDoesNotExist()
        compose.onNodeWithTag(FLOW_INDICATOR_TAG).assert(pageCount(6))
    }

    @Test
    fun only_the_pages_the_core_composed_are_counted() {
        reveal = styleReveal(pages = listOf(WritingStyleRevealPage.READ, WritingStyleRevealPage.LETTER, WritingStyleRevealPage.NAME))
        show()
        compose.onNodeWithTag(FLOW_INDICATOR_TAG).assert(pageCount(3))
    }

    @Test
    fun next_and_back_move_between_pages() {
        show()
        next()
        compose.onNodeWithTag(revealPageTag(WritingStyleRevealPage.LETTER)).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertIsDisplayed().performClick()
        compose.waitForIdle()
        compose.onNodeWithTag(revealPageTag(WritingStyleRevealPage.READ)).assertIsDisplayed()
        assertEquals("moving between pages sends nothing", emptyList<WritingStyleIntent>(), sent)
    }

    @Test
    fun a_per_language_page_offers_each_language_and_says_when_one_has_nothing() {
        show()
        next()
        compose.onNodeWithText("Two short paragraphs.").assertExists()
        // The letter is one element to TalkBack, spoken as one sentence; its runs are not read alone.
        compose.onNodeWithTag(REVEAL_LETTER_TAG)
            .assert(hasContentDescription(letterSpoken(ctx(), checkNotNull(styleLanguage("en").letter))))
        compose.onNodeWithText("Best,").assertDoesNotExist()
        compose.onNodeWithTag(revealLanguageTag("nl")).performClick()
        compose.waitForIdle()
        compose.onNodeWithText(L10n.reveal_nothing_in_language(ctx())).assertIsDisplayed()
    }

    @Test
    fun one_language_draws_no_language_control() {
        reveal = styleReveal(languages = listOf(styleLanguage("en")))
        show()
        next()
        compose.onNodeWithTag(revealLanguageTag("en")).assertDoesNotExist()
    }

    @Test
    fun save_is_on_the_last_page_held_while_the_name_is_refused_and_sends_what_was_typed() {
        reveal = styleReveal(pages = listOf(WritingStyleRevealPage.READ, WritingStyleRevealPage.NAME))
        show()
        next()
        compose.onNodeWithTag(REVEAL_NAME_TAG).performTextClearance()
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
        compose.onNodeWithText(L10n.writing_style_refused_blank_name(ctx())).assertIsDisplayed()

        compose.onNodeWithTag(REVEAL_NAME_TAG).performTextInput("Work")
        compose.onNodeWithTag(REVEAL_NOTES_TAG).performTextInput("Never use exclamation marks.")
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsEnabled().performClick()
        assertEquals(
            listOf<WritingStyleIntent>(WritingStyleIntent.SaveReveal("style-1", "Work", "Never use exclamation marks.")),
            sent,
        )
    }

    @Test
    fun a_running_save_holds_the_fields_and_shows_it_works() {
        reveal = styleReveal(pages = listOf(WritingStyleRevealPage.NAME), save = STYLE_RUNNING, close = HELD)
        show()
        compose.onNodeWithTag("$FLOW_PRIMARY_TAG-running").assertExists()
        compose.onNodeWithTag(REVEAL_NAME_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(FLOW_CLOSE_TAG).assertIsNotEnabled()
    }

    @Test
    fun a_row_s_reveal_closes_once_its_save_is_made_and_says_a_refusal() {
        reveal = styleReveal(pages = listOf(WritingStyleRevealPage.NAME))
        page = stylePage().copy(outcome = savedReveal(1, WritingStyleResult.Done))
        show()
        assertEquals("an outcome in force when it opened is not its own", 0, left)

        page = page.copy(outcome = savedReveal(2, WritingStyleResult.Refused(WritingStyleRefusal.GONE)))
        compose.waitForIdle()
        compose.onNodeWithText(L10n.writing_style_refused_gone(ctx())).assertIsDisplayed()
        assertEquals(0, left)

        page = page.copy(outcome = savedReveal(3, WritingStyleResult.Done))
        compose.waitForIdle()
        assertEquals(1, left)
    }

    @Test
    fun closing_a_row_s_reveal_counts_it_as_shown() {
        show()
        compose.onNodeWithTag(FLOW_CLOSE_TAG).performClick()
        assertEquals(listOf<WritingStyleIntent>(WritingStyleIntent.CloseReveal("style-1")), sent)
        assertEquals(1, left)
    }

    @Test
    fun moving_between_pages_is_held_while_the_record_holds_it() {
        reveal = styleReveal(paging = HELD)
        show()
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
    }
}
