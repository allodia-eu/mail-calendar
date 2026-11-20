// Draft a reply in the composer as the core's record draws it (docs/ai.md rules 17 to 24): the
// control, each state among the notes, the instruction's bottom sheet with its suggestions, the
// replace question before a draft and when one arrives, the Send question asked once, and the
// editor handed each draft once. Driven through the real composables; every press is an intent.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.hasProgressBarRangeInfo
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.ComposerReplyDraft
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.ReplyDraftHold
import uniffi.mailcal_bindings.ReplyDraftIntent
import uniffi.mailcal_bindings.ReplyDraftItem
import uniffi.mailcal_bindings.ReplyDraftScript
import uniffi.mailcal_bindings.ReplyDraftSendQuestion
import uniffi.mailcal_bindings.ReplyDraftState
import uniffi.mailcal_bindings.ReplyDraftSuggestion
import uniffi.mailcal_bindings.ReplyDraftTaskKind

private const val COMPOSITION = "composition-1"

internal fun replyDraft(
    state: ReplyDraftState = ReplyDraftState.Ready,
    control: ControlState = FREE,
    draft: ControlState = FREE,
    stop: ControlState = FREE,
    heldBy: ReplyDraftHold? = null,
    instruction: String = "",
    askBeforeReplacing: Boolean = false,
    replaceArrived: Boolean = false,
    editorScript: ReplyDraftScript? = null,
    sendQuestion: ReplyDraftSendQuestion? = null,
) = ComposerReplyDraft(
    state = state,
    control = control,
    draft = draft,
    stop = stop,
    heldBy = heldBy,
    suggestions = ReplyDraftSuggestion.entries.toList(),
    instruction = instruction,
    askBeforeReplacing = askBeforeReplacing,
    replaceArrived = replaceArrived,
    editorScript = editorScript,
    sendQuestion = sendQuestion,
)

private val DRAFTED = ReplyDraftState.Drafted(
    summary = "Asks whether Thursday still works.",
    checklist = listOf(
        ReplyDraftItem(0u, ReplyDraftTaskKind.FILL_IN, "[delivery date]", false, null),
        ReplyDraftItem(1u, ReplyDraftTaskKind.ATTACH, "Attach the quote", false, FREE),
        ReplyDraftItem(2u, ReplyDraftTaskKind.DO, "Book the room", true, FREE),
        ReplyDraftItem(3u, ReplyDraftTaskKind.FILL_IN, "[name]", true, null),
    ),
)

@RunWith(RobolectricTestRunner::class)
class ComposerReplyDraftTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<ReplyDraftIntent>()
    private val remedies = mutableListOf<AiRemedy>()
    private val reports = mutableListOf<String>()
    private var record by mutableStateOf<ComposerReplyDraft?>(replyDraft())
    private var version by mutableStateOf(0)
    private var submitted = 0

    private val ui
        get() = ComposerReplyDraftUi(
            read = { _, _ -> record },
            report = { _, message -> reports += message },
            onChange = { sent += it },
            onRemedy = { remedies += it },
            version = version,
            language = "en",
        )

    private fun show() {
        compose.setContent {
            AppTheme {
                val host = rememberReplyDraft(ui, COMPOSITION, from = "acct-1", editor = null)
                if (host != null) {
                    Column {
                        ComposerReplyDraftNotes(host)
                        ComposerReplyDraftDialogs(host) { submitted++ }
                        Button(onClick = { host.send { submitted++ } }) {
                            Text("send")
                        }
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun reread(next: ComposerReplyDraft?) {
        record = next
        version += 1
        compose.waitForIdle()
    }

    @Test
    fun nothing_is_drawn_while_the_core_offers_none() {
        record = null
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).assertDoesNotExist()
    }

    @Test
    fun the_sheet_opens_holding_the_last_line_and_a_suggestion_fills_it() {
        record = replyDraft(instruction = "Say yes")
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assert(hasText("Say yes"))
        compose.onNodeWithTag(replyDraftSuggestionTag(ReplyDraftSuggestion.NO)).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assert(hasText(L10n.composer_draft_chip_no(ctx())))

        compose.onNodeWithTag(REPLY_DRAFT_DRAFT_TAG).performClick()
        assertEquals(
            listOf<ReplyDraftIntent>(ReplyDraftIntent.Draft(COMPOSITION, "acct-1", L10n.composer_draft_chip_no(ctx()), "en", false)),
            sent,
        )
    }

    @Test
    fun the_keyboard_s_action_in_the_line_drafts_too() {
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).performTextInput("Decline politely")
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).performImeAction()
        assertEquals(
            listOf<ReplyDraftIntent>(ReplyDraftIntent.Draft(COMPOSITION, "acct-1", "Decline politely", "en", false)),
            sent,
        )
    }

    // At a large text size the sheet is taller than what the keyboard leaves, and Draft must still
    // be reachable by scrolling it rather than cut off at the foot.
    @Test
    @Config(qualifiers = "w360dp-h420dp")
    fun the_sheet_scrolls_to_draft_when_it_is_taller_than_the_room() {
        RuntimeEnvironment.setFontScale(2f)
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFT_TAG).performScrollTo().assertIsDisplayed().performClick()
        assertEquals(1, sent.size)
    }

    @Test
    fun draft_asks_first_while_the_record_says_so_and_cancel_sends_nothing() {
        record = replyDraft(askBeforeReplacing = true)
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFT_TAG).performClick()
        compose.onNodeWithTag("$REPLY_DRAFT_REPLACE_TAG-cancel").performClick()
        assertEquals(emptyList<ReplyDraftIntent>(), sent)

        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFT_TAG).performClick()
        compose.onNodeWithTag("$REPLY_DRAFT_REPLACE_TAG-confirm").performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.Draft(COMPOSITION, "acct-1", "", "en", true)), sent)
    }

    @Test
    fun a_held_draft_says_why_in_the_line_s_place_with_its_remedy() {
        record = replyDraft(heldBy = ReplyDraftHold(AiFailure.NoStyleForAccount, offered(AiRemedy.CHOOSE_STYLE)), draft = HELD)
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithText(L10n.ai_failure_no_style_for_account(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assertDoesNotExist()
        compose.onNodeWithTag(REPLY_DRAFT_REMEDY_TAG).performClick()
        assertEquals(listOf(AiRemedy.CHOOSE_STYLE), remedies)
        // The input closes, the client's own; the composer stays as it is (docs/ai.md rule 27).
        compose.onNodeWithText(L10n.ai_failure_no_style_for_account(ctx())).assertDoesNotExist()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).assertIsDisplayed()
        assertEquals(emptyList<ReplyDraftIntent>(), sent)
    }

    @Test
    fun drafting_is_drawn_among_the_notes_with_stop() {
        record = replyDraft(state = ReplyDraftState.Drafting, control = STYLE_RUNNING, draft = HELD)
        show()
        compose.onNodeWithTag("$REPLY_DRAFT_CONTROL_TAG-running").assertExists()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFTING_TAG).assertIsDisplayed()
        compose.onAllNodes(hasProgressBarRangeInfo(ProgressBarRangeInfo.Indeterminate)).assertCountEquals(1)
        compose.onNodeWithTag(REPLY_DRAFT_STOP_TAG).performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.Stop(COMPOSITION)), sent)
    }

    @Test
    fun the_card_says_what_is_asked_and_ticks_only_what_the_person_ticks() {
        record = replyDraft(state = DRAFTED)
        show()
        compose.onNodeWithText("Asks whether Thursday still works.").assertIsDisplayed()
        compose.onNodeWithText(L10n.composer_task_fill_in(ctx(), "[delivery date]")).assertIsDisplayed()
        // A gap follows the reply: its state is read, with no action and no checkbox to hold.
        for ((index, done) in listOf(0u to false, 3u to true)) {
            val gap = compose.onNodeWithTag(replyDraftItemTag(index))
            if (done) gap.assertIsOn() else gap.assertIsOff()
            gap.assert(SemanticsMatcher.keyNotDefined(SemanticsActions.OnClick))
            gap.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Disabled))
            gap.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Role))
        }
        compose.onNodeWithTag(replyDraftItemTag(2u)).assertIsOn()

        compose.onNodeWithTag(replyDraftItemTag(1u)).performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.Tick(COMPOSITION, 1u, true)), sent)
    }

    @Test
    fun a_gap_and_a_tick_line_up_in_one_column_at_one_height() {
        record = replyDraft(state = DRAFTED)
        show()
        val texts = DRAFTED.checklist.map { item ->
            compose.onNodeWithText(replyDraftItemText(ctx(), item), useUnmergedTree = true)
                .fetchSemanticsNode().boundsInRoot.left
        }
        assertEquals("every item's words start where the others' do: $texts", 1, texts.distinct().size)
        for (item in DRAFTED.checklist) {
            compose.onNodeWithTag(replyDraftItemTag(item.index)).assertHeightIsAtLeast(48.dp)
        }
    }

    @Test
    fun a_failure_offers_its_remedy_and_try_again_opens_the_sheet() {
        record = replyDraft(state = ReplyDraftState.Failed(AiFailure.Unreachable, offered(AiRemedy.TRY_AGAIN)))
        show()
        compose.onNodeWithText(L10n.ai_failure_unreachable(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(REPLY_DRAFT_REMEDY_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assertIsDisplayed()
        assertEquals(emptyList<AiRemedy>(), remedies)
    }

    @Test
    fun a_message_gone_is_a_line_among_the_notes_and_the_control_held() {
        record = replyDraft(state = ReplyDraftState.Unavailable(AiFailure.MessageGone, null), control = HELD)
        show()
        compose.onNodeWithText(L10n.ai_failure_message_gone(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).assertIsNotEnabled()
    }

    @Test
    fun a_draft_that_arrived_after_writing_is_asked_about() {
        record = replyDraft(state = ReplyDraftState.Drafting, replaceArrived = true)
        show()
        compose.onNodeWithTag("$REPLY_DRAFT_ARRIVED_TAG-confirm").performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.AnswerArrived(COMPOSITION, true)), sent)
    }

    @Test
    fun send_asks_once_while_the_question_is_due_and_send_anyway_then_sends() {
        record = replyDraft(state = DRAFTED, sendQuestion = ReplyDraftSendQuestion(2u))
        show()
        compose.onNodeWithText("send").performClick()
        assertEquals("nothing is sent while the question is open", 0, submitted)
        compose.onNodeWithText(L10n.composer_send_open_message(ctx(), 2)).assertIsDisplayed()
        compose.onNodeWithTag("$REPLY_DRAFT_SEND_TAG-anyway").performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.SendQuestionAnswered(COMPOSITION)), sent)
        assertEquals(1, submitted)

        // Answered, the core no longer says it is due, and Send sends.
        reread(replyDraft(state = DRAFTED))
        compose.onNodeWithText("send").performClick()
        assertEquals(2, submitted)
    }

    @Test
    fun keep_editing_answers_the_question_and_sends_nothing() {
        record = replyDraft(sendQuestion = ReplyDraftSendQuestion(1u))
        show()
        compose.onNodeWithText("send").performClick()
        compose.onNodeWithTag("$REPLY_DRAFT_SEND_TAG-keep").performClick()
        assertEquals(listOf<ReplyDraftIntent>(ReplyDraftIntent.SendQuestionAnswered(COMPOSITION)), sent)
        assertEquals(0, submitted)
    }

    @Test
    fun each_draft_is_handed_to_the_editor_once() {
        val host = ReplyDraftHost(ui, COMPOSITION, replyDraft(editorScript = ReplyDraftScript(4uL, "window.setComposerDraftText(1)")))
        assertEquals("window.setComposerDraftText(1)", host.scriptToHand())
        assertNull("the same serial is not handed again", host.scriptToHand())
        host.record = replyDraft(editorScript = ReplyDraftScript(5uL, "window.setComposerDraftText(2)"))
        assertEquals("window.setComposerDraftText(2)", host.scriptToHand())
    }

    @Test
    fun the_record_is_read_again_when_from_changes() {
        val asked = mutableListOf<String?>()
        var from by mutableStateOf<String?>("acct-1")
        compose.setContent {
            val reading = ComposerReplyDraftUi({ _, f -> asked += f; replyDraft() }, { _, _ -> }, {}, {}, 0, "en")
            rememberReplyDraft(reading, COMPOSITION, from, null)
        }
        compose.waitForIdle()
        from = "acct-2"
        compose.waitForIdle()
        assertEquals(listOf<String?>("acct-1", "acct-2"), asked)
    }
}
