// Settings → Writing style as the core draws it (docs/ai.md rules 8 to 10; docs/waiting.md): Learn
// or the run in its place, why Learn cannot be pressed with its one remedy, the library's rows
// opening the reveal or the detail as each says, and each account's style. Driven through the real
// composables; every change is an intent.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleLearnBlock
import uniffi.mailcal_bindings.WritingStyleOpens
import uniffi.mailcal_bindings.WritingStyleOutcome
import uniffi.mailcal_bindings.WritingStylePage
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleResult
import uniffi.mailcal_bindings.WritingStyleRun

@RunWith(RobolectricTestRunner::class)
class SettingsWritingStyleTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<WritingStyleIntent>()
    private val signIns = mutableListOf<Unit>()
    private var endpointsOpened = 0
    private var page by mutableStateOf<WritingStylePage?>(stylePage())

    private fun show() {
        compose.setContent {
            AppTheme {
                val ui = testWritingStyleUi(
                    page,
                    null,
                    sent,
                    reveal = { styleReveal(it) },
                    detail = { styleDetail(it) },
                    signIns = signIns,
                )
                Column { WritingStyleCategory(ui) { endpointsOpened++ } }
            }
        }
        compose.waitForIdle()
    }

    /** The category is in the hub exactly while the core offers the feature (docs/ai.md rule 4). */
    @Test
    fun the_category_follows_signatures_and_only_while_offered() {
        assertEquals(false, SettingsCategory.WRITING_STYLE in SettingsCategory.shown(allodiaAvailable = false))
        val shown = SettingsCategory.shown(allodiaAvailable = false, writingStyleOffered = true)
        assertEquals(shown.indexOf(SettingsCategory.SIGNATURES) + 1, shown.indexOf(SettingsCategory.WRITING_STYLE))
    }

    @Test
    fun nothing_is_drawn_while_the_feature_is_not_offered() {
        page = null
        show()
        compose.onNodeWithText(L10n.writing_style_learn(ctx())).assertDoesNotExist()
    }

    @Test
    fun an_empty_library_invites_learning_and_learn_opens_the_flow() {
        show()
        compose.onNodeWithTag(WRITING_STYLE_EMPTY_TAG).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_category_writing_style_summary(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.writing_style_library_heading(ctx())).assertDoesNotExist()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertIsEnabled().performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Open), learningSent(sent))
    }

    @Test
    fun learn_says_why_it_cannot_be_pressed_with_its_one_remedy() {
        page = stylePage(learn = HELD).copy(
            learnBlocked = WritingStyleLearnBlock.CannotSend(AiFailure.NowhereToSend, offered(AiRemedy.OPEN_AI_ENDPOINT)),
        )
        show()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertIsNotEnabled()
        compose.onNodeWithText(L10n.ai_failure_nowhere_to_send(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(WRITING_STYLE_REMEDY_TAG).assert(hasText(L10n.ai_remedy_open_endpoint(ctx()))).performClick()
        assertEquals(1, endpointsOpened)

        page = stylePage(learn = HELD).copy(
            learnBlocked = WritingStyleLearnBlock.CannotSend(AiFailure.RelaySignIn, offered(AiRemedy.SIGN_IN_TO_ALLODIA)),
        )
        compose.waitForIdle()
        compose.onNodeWithTag(WRITING_STYLE_REMEDY_TAG).performClick()
        assertEquals(1, signIns.size)
    }

    @Test
    fun no_mail_account_is_said_without_a_remedy() {
        page = stylePage(learn = HELD).copy(learnBlocked = WritingStyleLearnBlock.NoMailAccount)
        show()
        compose.onNodeWithText(L10n.writing_style_learn_no_account(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(WRITING_STYLE_REMEDY_TAG).assertDoesNotExist()
    }

    @Test
    fun a_run_is_drawn_in_learn_s_place_with_stop() {
        page = stylePage().copy(run = WritingStyleRun.Learning("alice@test.local", 1u, 4u, FREE))
        show()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertDoesNotExist()
        compose.onNodeWithText(L10n.learn_progress(ctx(), "1", "4")).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_progress_account(ctx(), "alice@test.local")).assertIsDisplayed()
        compose.onNodeWithTag(WRITING_STYLE_STOP_TAG).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Stop), learningSent(sent))
    }

    @Test
    fun a_run_that_failed_with_its_host_closed_is_said_above_learn() {
        page = stylePage().copy(run = WritingStyleRun.Failed(AiFailure.Unreachable, offered(AiRemedy.TRY_AGAIN)))
        show()
        compose.onNodeWithText(L10n.writing_style_run_failed(ctx()), substring = true).assertIsDisplayed()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Open), learningSent(sent))

        // Its remedy goes to the flow, as it would in the host.
        sent.clear()
        compose.onNodeWithTag(WRITING_STYLE_REMEDY_TAG).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Remedy), learningSent(sent))
    }

    @Test
    fun a_row_opens_the_reveal_once_and_the_detail_ever_after() {
        page = stylePage(
            styles = listOf(
                styleEntry("style-1", "New one", opens = WritingStyleOpens.REVEAL),
                styleEntry("style-2", "Old one", opens = WritingStyleOpens.DETAIL),
            ),
        )
        show()
        compose.onNodeWithTag(writingStyleRowTag("style-1")).performClick()
        compose.waitForIdle()
        compose.onNodeWithTag(FLOW_HOST_TAG).assertExists()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_CLOSE_TAG).performClick()
        compose.waitForIdle()
        compose.onNodeWithTag(FLOW_HOST_TAG).assertDoesNotExist()

        compose.onNodeWithTag(writingStyleRowTag("style-2")).performClick()
        compose.waitForIdle()
        compose.onNodeWithTag(DETAIL_RENAME_TAG).assertExists()
    }

    @Test
    fun a_held_row_cannot_be_opened() {
        page = stylePage(styles = listOf(styleEntry("style-1", "Work", open = HELD)))
        show()
        compose.onNodeWithTag(writingStyleRowTag("style-1")).assertIsNotEnabled()
    }

    @Test
    fun each_account_s_style_is_picked_from_none_and_the_library() {
        page = stylePage(
            styles = listOf(styleEntry("style-1", "Work")),
            accounts = listOf(styleSlot("alice", selected = "style-1"), styleSlot("bob", selected = "style-1", picker = STYLE_RUNNING)),
        )
        show()
        compose.onNodeWithTag(writingStyleSlotTag("alice")).assert(hasText("Work"))
        compose.onNodeWithTag(writingStyleSlotTag("bob")).assertIsNotEnabled()
        compose.onNodeWithTag("${writingStyleSlotTag("bob")}-running").assertExists()

        compose.onNodeWithTag(writingStyleSlotTag("alice")).performClick()
        compose.onNodeWithText(L10n.writing_style_none(ctx())).performClick()
        assertEquals(listOf<WritingStyleIntent>(WritingStyleIntent.Assign("alice", null)), sent)
    }

    @Test
    fun a_refused_slot_is_said_once_its_outcome_is_new() {
        val refused = WritingStyleOutcome(1uL, WritingStyleChangeKind.ASSIGN, null, "alice", WritingStyleResult.Refused(WritingStyleRefusal.GONE))
        page = stylePage().copy(outcome = refused)
        show()
        compose.onNodeWithTag(WRITING_STYLE_OUTCOME_TAG).assertDoesNotExist()

        page = stylePage().copy(outcome = refused.copy(serial = 2uL))
        compose.waitForIdle()
        compose.onNodeWithTag(WRITING_STYLE_OUTCOME_TAG).assert(hasText(L10n.writing_style_refused_gone(ctx())))
    }

    @Test
    fun a_remedy_is_held_while_its_control_is() {
        page = stylePage(learn = HELD).copy(
            learnBlocked = WritingStyleLearnBlock.CannotSend(AiFailure.NowhereToSend, offered(AiRemedy.OPEN_AI_ENDPOINT, HELD)),
        )
        show()
        compose.onNodeWithTag(WRITING_STYLE_REMEDY_TAG).assertIsNotEnabled()
    }
}
