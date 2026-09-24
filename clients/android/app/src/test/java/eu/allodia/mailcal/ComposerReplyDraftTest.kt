// Draft a reply in the composer (docs/ai.md, "Drafting a reply"), driven without a WebView: which
// composers offer it, what is asked of the core, when the person is asked before their text is
// replaced, and what the editor is handed. Nothing on this path sends, so nothing here can.
package eu.allodia.mailcal

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.DraftReply
import uniffi.mailcal_bindings.WritingStyleFailure

private val ANSWERED = ReplyTarget("acct-work", "msg-1")

@RunWith(RobolectricTestRunner::class)
class ComposerReplyDraftTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private data class Asked(val account: String, val key: String, val from: String?, val intent: String?)

    private val asked = mutableListOf<Asked>()
    private val inserted = mutableListOf<Pair<String, String>>()
    private var written = false
    private var answer: () -> DraftReply = { DraftReply("draft-1", "Hi Anna,\n\nYes.", emptyList(), "", emptyList(), "en", null, null) }

    // What the editor says is left of the placeholders it is asked about.
    private var left: List<String>? = emptyList()
    private val placeholderReads = mutableListOf<List<String>>()

    private val editor = object : DraftEditor {
        override fun leadHasText(answer: (Boolean) -> Unit) = answer(written)

        override fun insert(text: String, draftId: String) {
            inserted += text to draftId
        }

        override fun placeholdersLeft(placeholders: List<String>, answer: (List<String>?) -> Unit) {
            placeholderReads += placeholders
            answer(left)
        }
    }

    private val withTasks = DraftReply(
        "draft-3", "See you on [date] at [time].", listOf("[date]", "[time]"),
        "Bob asks when you can meet.", DRAFT_TASKS, "en", null, null,
    )

    private val control = ReplyDraftControl(
        ANSWERED,
        ReplyDrafting(
            route = AiRoute.OWN_ENDPOINT,
            styleFor = { account -> "style-work".takeIf { account == "acct-work" } },
            draft = { account, key, from, intent ->
                asked += Asked(account, key, from, intent)
                answer()
            },
            background = ImmediateBackground,
        ),
        editor,
    )

    @Test
    fun only_a_reply_offers_a_draft() {
        assertTrue(replyDraftOffered(RichComposeMode.Reply))
        assertTrue(replyDraftOffered(RichComposeMode.ReplyAll))
        assertFalse(replyDraftOffered(RichComposeMode.Forward))
        assertFalse(replyDraftOffered(RichComposeMode.New))
    }

    @Test
    fun a_from_account_without_a_style_has_nothing_to_draft_in() {
        assertTrue(control.hasStyle("acct-work"))
        assertFalse(control.hasStyle("acct-home"))
        assertFalse(control.hasStyle(null))
    }

    /** `from` goes to the core only when the person moved the reply off the receiving account. */
    @Test
    fun the_draft_answers_the_message_and_names_the_sender_only_when_it_changed() {
        control.intent = "  Thanks, I will get back to you "
        control.create(from = "acct-work")
        control.create(from = "acct-home")
        assertEquals(
            listOf(
                Asked("acct-work", "msg-1", null, "Thanks, I will get back to you"),
                Asked("acct-work", "msg-1", "acct-home", "Thanks, I will get back to you"),
            ),
            asked,
        )
    }

    @Test
    fun an_empty_intent_leaves_the_draft_to_the_message() {
        control.create(from = "acct-work")
        assertNull(asked.single().intent)
    }

    /** The draft and its id go to the editor together, so a send from it is never learned from. */
    @Test
    fun a_draft_goes_into_the_editor_with_its_id() {
        control.open()
        control.create(from = "acct-work")
        assertFalse(control.sheetOpen)
        assertEquals(listOf("Hi Anna,\n\nYes." to "draft-1"), inserted)
        assertFalse(control.busy)
        assertFalse(control.checkBrackets)
    }

    @Test
    fun written_text_is_not_replaced_without_asking() {
        written = true
        control.create(from = "acct-work")
        assertTrue(control.confirmingReplace)
        assertTrue("nothing asked before the answer", asked.isEmpty())

        control.keep()
        assertFalse(control.confirmingReplace)
        assertTrue(asked.isEmpty())

        control.create(from = "acct-home")
        control.replace()
        assertEquals("acct-home", asked.single().from)
        assertEquals(1, inserted.size)
    }

    /** Gaps are pointed out until the next draft or a send. */
    @Test
    fun a_draft_with_gaps_says_to_check_the_brackets_until_it_is_sent() {
        answer = { DraftReply("draft-2", "See you on [date].", listOf("[date]"), "", emptyList(), "en", null, null) }
        control.create(from = "acct-work")
        assertTrue(control.checkBrackets)
        control.sending()
        assertFalse(control.checkBrackets)
    }

    /** The card names the gaps, so it takes the place of the line about brackets. */
    @Test
    fun a_draft_with_a_checklist_shows_its_card_in_place_of_the_brackets_line() {
        answer = { withTasks }
        control.create(from = "acct-work")
        assertFalse(control.checkBrackets)
        assertEquals("Bob asks when you can meet.", control.checklist.summary)
        assertEquals(DRAFT_TASKS.map { it.text }, control.checklist.items.map { it.text })
    }

    /** Files counted from the composer's opening, so only those added after the draft tick. */
    @Test
    fun an_attach_item_ticks_once_a_file_is_added_after_the_draft() {
        control.attachmentsChanged(1)
        answer = { withTasks }
        control.create(from = "acct-work")
        control.attachmentsChanged(1)
        assertFalse(control.checklist.items[2].ticked)
        control.attachmentsChanged(2)
        assertTrue(control.checklist.items[2].ticked)
    }

    /** Send reads the reply afresh, asks once, and a later Send goes straight through. */
    @Test
    fun send_asks_once_about_open_items_with_the_placeholders_read_afresh() {
        answer = { withTasks }
        control.create(from = "acct-work")
        var sends = 0
        left = listOf("[time]")
        control.requestSend { sends++ }
        assertEquals(listOf(listOf("[date]", "[time]")), placeholderReads)
        assertEquals(listOf(true, false, false, false), control.checklist.items.map { it.ticked })
        assertTrue(control.confirmingSend)
        assertEquals(0, sends)

        control.keepEditing()
        assertFalse(control.confirmingSend)
        assertEquals(0, sends)
        control.requestSend { sends++ }
        assertEquals(1, sends)
        assertFalse(control.confirmingSend)
    }

    @Test
    fun send_anyway_sends_as_it_stands() {
        answer = { withTasks }
        control.create(from = "acct-work")
        var sends = 0
        control.requestSend { sends++ }
        control.sendAnyway()
        assertEquals(1, sends)
        assertTrue(control.checklist.hasAsked)
    }

    /** Every placeholder filled and nothing else on the list: nothing to ask about. */
    @Test
    fun send_does_not_ask_once_the_reply_has_filled_everything() {
        answer = { withTasks.copy(tasks = DRAFT_TASKS.take(2)) }
        control.create(from = "acct-work")
        left = emptyList()
        var sends = 0
        control.requestSend { sends++ }
        assertEquals(1, sends)
        assertFalse(control.confirmingSend)
    }

    /** An editor that did not answer ticks nothing, so the question still comes. */
    @Test
    fun a_read_the_editor_did_not_answer_ticks_nothing() {
        answer = { withTasks }
        control.create(from = "acct-work")
        left = null
        control.readPlaceholders()
        assertTrue(control.checklist.items.none { it.ticked })
    }

    @Test
    fun the_card_shows_the_summary_and_only_the_items_the_editor_cannot_see_toggle() {
        answer = { withTasks.copy(tasks = DRAFT_TASKS.drop(1)) }
        control.create(from = "acct-work")
        left = listOf("[time]")
        compose.setContent { ComposerDraftStatus(control) }
        compose.onNodeWithText("Bob asks when you can meet.").assertIsDisplayed()
        compose.onNodeWithText(L10n.composer_task_fill_in(ctx(), placeholder = "[time]")).performClick()
        assertFalse(control.checklist.items[0].ticked)
        compose.onNodeWithText("Book the room").performClick()
        assertTrue(control.checklist.items[2].ticked)
        compose.onNodeWithContentDescription(L10n.a11y_task_attach(ctx())).assertIsDisplayed()

        // The card asks the editor about once a second, and ticks the placeholder once it is gone.
        left = emptyList()
        compose.mainClock.advanceTimeBy(1_100)
        compose.waitForIdle()
        assertTrue(control.checklist.items[0].ticked)
    }

    @Test
    fun the_placeholders_reach_the_editor_as_a_string_and_come_back_as_a_list() {
        assertEquals(
            "window.composerPlaceholdersLeft(\"[\\\"[date]\\\",\\\"a\\\\\\\"b\\\"]\")",
            composerPlaceholdersLeftScript(listOf("[date]", "a\"b")),
        )
        assertEquals(listOf("[date]"), placeholdersLeftAnswer("[\"[date]\"]"))
        assertEquals(emptyList<String>(), placeholdersLeftAnswer("[]"))
        assertNull(placeholdersLeftAnswer("null"))
        assertNull(placeholdersLeftAnswer(null))
    }

    @Test
    fun a_failure_is_kept_as_its_variant_and_inserts_nothing() {
        answer = { throw WritingStyleFailure.RateLimited() }
        control.create(from = "acct-work")
        assertTrue(control.failure is WritingStyleFailure.RateLimited)
        assertTrue(inserted.isEmpty())
        assertFalse(control.busy)
    }

    /** Disabled for an account with no style, and a screen reader is told why. */
    @Test
    fun the_button_is_disabled_and_says_why_without_a_style() {
        compose.setContent { ComposerDraftAction(control, from = "acct-home") }
        compose.onNodeWithContentDescription(L10n.composer_draft_reply(ctx()))
            .assertIsNotEnabled()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, L10n.ai_error_no_style(ctx())))
    }

    /** A chip puts its words in the intent field, and Draft asks the core with them. */
    @Test
    fun a_chip_fills_the_intent_and_draft_asks_with_it() {
        compose.setContent {
            ComposerDraftAction(control, from = "acct-work")
            ComposerDraftDialogs(control, from = "acct-work")
        }
        compose.onNodeWithContentDescription(L10n.composer_draft_reply(ctx())).assertIsEnabled().performClick()
        compose.onNodeWithText(L10n.composer_draft_chip_more_info(ctx())).performClick()
        compose.onNodeWithText(L10n.composer_draft_create(ctx())).performClick()

        assertEquals(L10n.composer_draft_chip_more_info(ctx()), asked.single().intent)
        assertEquals(1, inserted.size)
    }

    @Test
    fun the_draft_reaches_the_editor_as_string_literals() {
        assertEquals(
            "window.setComposerDraftText(\"Line \\\"one\\\"\\n<\\/div>\", \"d-1\")",
            composerDraftTextScript("Line \"one\"\n</div>", "d-1"),
        )
    }
}
