// An existing style's detail (docs/ai.md rule 16): its facts and sections on one page, Rename held
// while the name is refused or unchanged, the notes sent on Done, and Forget asked first, naming the
// accounts that draft in the style. Driven through the real composables.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleOutcome
import uniffi.mailcal_bindings.WritingStyleResult
import uniffi.mailcal_bindings.WritingStyleSection

@RunWith(RobolectricTestRunner::class)
class WritingStyleDetailTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<WritingStyleIntent>()
    private var closed = 0
    private var detail by mutableStateOf<WritingStyleDetail?>(styleDetail())
    private var page by mutableStateOf(stylePage())
    private var version by mutableStateOf(0)

    private fun show() {
        compose.setContent {
            AppTheme {
                val current = detail
                val ui = testWritingStyleUi(page, null, sent, detail = { current }, version = version)
                WritingStyleDetailDialog("style-1", ui) { closed++ }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_detail_draws_the_facts_and_every_section_on_one_page() {
        show()
        compose.onNodeWithText(L10n.reveal_based_on(ctx(), "alice@test.local")).assertExists()
        for (title in listOf(L10n.reveal_step_letter(ctx()), L10n.reveal_step_habits(ctx()), L10n.reveal_step_voice(ctx()), L10n.reveal_phrases(ctx()))) {
            compose.onNodeWithText(title).performScrollTo()
        }
        compose.onNodeWithText("Warm and direct").performScrollTo()
        compose.onNodeWithTag(FLOW_INDICATOR_TAG).assertDoesNotExist()
    }

    @Test
    fun rename_is_held_until_the_name_changes_and_while_it_is_refused() {
        show()
        compose.onNodeWithTag(DETAIL_RENAME_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(DETAIL_NAME_TAG).performTextClearance()
        compose.onNodeWithTag(DETAIL_RENAME_TAG).assertIsNotEnabled()
        compose.onNodeWithText(L10n.writing_style_refused_blank_name(ctx())).assertExists()

        compose.onNodeWithTag(DETAIL_NAME_TAG).performTextInput("Office")
        compose.onNodeWithTag(DETAIL_RENAME_TAG).assertIsEnabled().performClick()
        assertEquals(listOf<WritingStyleIntent>(WritingStyleIntent.Rename("style-1", "Office")), sent)
    }

    @Test
    fun a_running_rename_shows_it_works_and_holds_the_field() {
        detail = styleDetail(rename = STYLE_RUNNING)
        show()
        compose.onNodeWithTag("$DETAIL_RENAME_TAG-running").assertExists()
        compose.onNodeWithTag(DETAIL_NAME_TAG).assertIsNotEnabled()
    }

    @Test
    fun the_notes_are_sent_on_done_once() {
        show()
        compose.onNodeWithTag(DETAIL_NOTES_TAG).performScrollTo().performTextClearance()
        compose.onNodeWithTag(DETAIL_NOTES_TAG).performTextInput("Sign as Al.")
        compose.onNodeWithTag(DETAIL_NOTES_TAG).performImeAction()
        assertEquals(listOf<WritingStyleIntent>(WritingStyleIntent.SetNotes("style-1", "Sign as Al.")), sent)
    }

    @Test
    fun forget_asks_first_naming_the_accounts_that_draft_in_it() {
        detail = styleDetail(drafting = listOf("alice@test.local", "bob@test.local"))
        show()
        compose.onNodeWithTag(DETAIL_FORGET_TAG).performScrollTo().performClick()
        val message = compose.onNodeWithTag("$DETAIL_FORGET_CONFIRM_TAG-message").fetchSemanticsNode()
            .config[androidx.compose.ui.semantics.SemanticsProperties.Text].joinToString { it.text }
        assertTrue(message.contains("alice@test.local, bob@test.local"))
        assertTrue(message.contains(L10n.writing_style_forget_message(ctx())))
        compose.onNodeWithText(L10n.action_cancel(ctx())).performClick()
        assertEquals("cancelling sends nothing", emptyList<WritingStyleIntent>(), sent)

        compose.onNodeWithTag(DETAIL_FORGET_TAG).performScrollTo().performClick()
        compose.onNodeWithTag("$DETAIL_FORGET_CONFIRM_TAG-confirm").performClick()
        assertEquals(listOf<WritingStyleIntent>(WritingStyleIntent.Forget("style-1")), sent)
    }

    @Test
    fun the_detail_closes_once_forgetting_is_written_or_the_style_is_gone() {
        show()
        page = page.copy(
            outcome = WritingStyleOutcome(1uL, WritingStyleChangeKind.FORGET, "style-1", null, WritingStyleResult.Done),
        )
        compose.waitForIdle()
        assertEquals(1, closed)

        detail = null
        version += 1
        compose.waitForIdle()
        assertEquals(2, closed)
    }

    @Test
    fun only_the_sections_the_record_lists_are_drawn() {
        detail = styleDetail(sections = listOf(WritingStyleSection.LETTER))
        show()
        compose.onNodeWithText(L10n.reveal_step_letter(ctx())).assertExists()
        compose.onNodeWithText(L10n.reveal_step_habits(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.reveal_phrases(ctx())).assertDoesNotExist()
    }
}
