// Draft a reply exists in a composer exactly while the core's record does (docs/ai.md rules 4 and
// 24): not offered, the composer has no control, no notes, no card and no Send question, and Send
// sends; withdrawn while the composer is open, everything Draft a reply drew goes with it, an open
// sheet or question included, and none of it comes back open with the offer. Drawn as
// RichComposeScreen draws it: each piece from the host, Send through `sendThrough`.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.ComposerReplyDraft
import uniffi.mailcal_bindings.ReplyDraftIntent
import uniffi.mailcal_bindings.ReplyDraftItem
import uniffi.mailcal_bindings.ReplyDraftSendQuestion
import uniffi.mailcal_bindings.ReplyDraftState
import uniffi.mailcal_bindings.ReplyDraftTaskKind

private const val COMPOSITION = "composition-1"

private val DRAFTED = ReplyDraftState.Drafted(
    summary = "Asks whether Thursday still works.",
    checklist = listOf(ReplyDraftItem(0u, ReplyDraftTaskKind.DO, "Book the room", false, FREE)),
)

@RunWith(RobolectricTestRunner::class)
class ComposerReplyDraftOfferTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<ReplyDraftIntent>()
    private var record by mutableStateOf<ComposerReplyDraft?>(replyDraft())
    private var version by mutableStateOf(0)
    private var submitted = 0

    private fun show() {
        compose.setContent {
            AppTheme {
                val ui = ComposerReplyDraftUi({ _, _ -> record }, { _, _ -> }, { sent += it }, {}, version, "en")
                val draft = rememberReplyDraft(ui, COMPOSITION, from = "acct-1", editor = null)
                val submit: () -> Unit = { submitted++ }
                Column {
                    ComposerTopBar(
                        title = "Re: Thursday",
                        onClose = {},
                        onAttach = {},
                        signatureMenu = null,
                        onSaveDraft = {},
                        onDiscard = {},
                        sendEnabled = true,
                        onSend = { sendThrough(draft, submit) },
                        replyDraft = draft?.let { host -> { ComposerReplyDraftControl(host) } },
                    )
                    draft?.let { ComposerReplyDraftNotes(it) }
                }
                draft?.let { ComposerReplyDraftDialogs(it, submit) }
            }
        }
        compose.waitForIdle()
    }

    private fun reread(next: ComposerReplyDraft?) {
        record = next
        version += 1
        compose.waitForIdle()
    }

    private fun send() = compose.onNodeWithContentDescription(L10n.action_send(ctx())).performClick()

    private fun assertNothingDrawn() {
        for (tag in listOf(
            REPLY_DRAFT_CONTROL_TAG,
            REPLY_DRAFT_CARD_TAG,
            REPLY_DRAFT_DRAFTING_TAG,
            REPLY_DRAFT_SHEET_TAG,
            REPLY_DRAFT_LINE_TAG,
            "$REPLY_DRAFT_REPLACE_TAG-confirm",
            "$REPLY_DRAFT_ARRIVED_TAG-confirm",
            "$REPLY_DRAFT_SEND_TAG-message",
        )) {
            compose.onNodeWithTag(tag).assertDoesNotExist()
        }
    }

    @Test
    fun not_offered_the_composer_has_none_of_it_and_send_sends() {
        record = null
        show()
        assertNothingDrawn()
        send()
        compose.waitForIdle()
        assertNothingDrawn()
        assertEquals(1, submitted)
        assertEquals(emptyList<ReplyDraftIntent>(), sent)
    }

    @Test
    fun an_open_sheet_goes_with_the_offer_and_does_not_come_back_open() {
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assertIsDisplayed()

        reread(null)
        assertNothingDrawn()

        reread(replyDraft())
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).assertIsDisplayed()
        compose.onNodeWithTag(REPLY_DRAFT_LINE_TAG).assertDoesNotExist()
    }

    @Test
    fun an_open_replace_question_goes_with_the_offer() {
        record = replyDraft(askBeforeReplacing = true)
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CONTROL_TAG).performClick()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFT_TAG).performClick()
        compose.onNodeWithTag("$REPLY_DRAFT_REPLACE_TAG-confirm").assertIsDisplayed()

        reread(null)
        assertNothingDrawn()
        reread(replyDraft(askBeforeReplacing = true))
        compose.onNodeWithTag("$REPLY_DRAFT_REPLACE_TAG-confirm").assertDoesNotExist()
        assertEquals(emptyList<ReplyDraftIntent>(), sent)
    }

    @Test
    fun a_draft_drafting_and_the_question_it_arrived_with_go_with_the_offer() {
        record = replyDraft(state = ReplyDraftState.Drafting, replaceArrived = true)
        show()
        compose.onNodeWithTag(REPLY_DRAFT_DRAFTING_TAG).assertIsDisplayed()
        compose.onNodeWithTag("$REPLY_DRAFT_ARRIVED_TAG-confirm").assertIsDisplayed()

        reread(null)
        assertNothingDrawn()
    }

    @Test
    fun an_open_send_question_and_the_card_go_with_the_offer_and_send_then_sends() {
        record = replyDraft(state = DRAFTED, sendQuestion = ReplyDraftSendQuestion(1u))
        show()
        compose.onNodeWithTag(REPLY_DRAFT_CARD_TAG).assertIsDisplayed()
        send()
        compose.onNodeWithTag("$REPLY_DRAFT_SEND_TAG-message").assertIsDisplayed()
        assertEquals(0, submitted)

        reread(null)
        assertNothingDrawn()
        assertEquals("withdrawing answers nothing and sends nothing", 0, submitted)
        assertEquals(emptyList<ReplyDraftIntent>(), sent)

        send()
        compose.waitForIdle()
        assertEquals(1, submitted)
        assertNothingDrawn()
    }
}
