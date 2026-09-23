// Draft a reply in the composer (docs/ai.md, "Drafting a reply"), driven without a WebView: which
// composers offer it, what is asked of the core, when the person is asked before their text is
// replaced, and what the editor is handed. Nothing on this path sends, so nothing here can.
package eu.allodia.mailcal

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
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
    private var answer: () -> DraftReply = { DraftReply("draft-1", "Hi Anna,\n\nYes.", emptyList(), "en", null) }

    private val editor = object : DraftEditor {
        override fun leadHasText(answer: (Boolean) -> Unit) = answer(written)

        override fun insert(text: String, draftId: String) {
            inserted += text to draftId
        }
    }

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
        answer = { DraftReply("draft-2", "See you on [date].", listOf("[date]"), "en", null) }
        control.create(from = "acct-work")
        assertTrue(control.checkBrackets)
        control.sending()
        assertFalse(control.checkBrackets)
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
