// The "the organiser wasn't told" dialog, the client half of the reply-delivery verdict
// (docs/invitations.md → "Who delivers the answer").
//
// What is worth pinning here is not that a dialog draws. It is the four things a user's consent
// depends on and which a screenshot would not catch: that the recipient of the mail we are asking
// to send is actually named on screen; that the RFC status code is not; that "remember" rides
// whichever button was pressed, so a ticked box beside "Don't send" is a standing *no*; and that
// neither button can be reached without the core hearing about it.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ReplyPrompt

@RunWith(RobolectricTestRunner::class)
class InvitationReplyPromptTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    /** The shape `provider-caldav` reports after a Sabre/DAV server writes SCHEDULE-STATUS=5.2. */
    private fun prompt() = ReplyPrompt(
        account = "acct-a",
        summary = "Quarterly planning",
        organizer = "organizer@example.net",
        response = InvitationResponse.ACCEPT,
        statusCode = "5.2",
    )

    /** Renders the dialog, recording every `(send, remember)` pair it reports. */
    private fun show(answers: MutableList<Pair<Boolean, Boolean>>) {
        compose.setContent {
            AppTheme { InvitationReplyPrompt(prompt()) { send, remember -> answers += send to remember } }
        }
        compose.waitForIdle()
    }

    private fun tick() {
        compose.onNodeWithText(L10n.invitation_reply_undelivered_remember(ctx())).performClick()
        compose.waitForIdle()
    }

    /**
     * The user is authorising mail to be sent from their account to somebody they did not choose in
     * this moment. That consent is not informed unless the address is on screen, so the body must
     * carry the organiser's actual address rather than the words "the organiser".
     */
    @Test
    fun the_question_names_the_meeting_and_who_would_be_emailed() {
        show(mutableListOf())

        val body = L10n.invitation_reply_undelivered_body(
            ctx(),
            "Quarterly planning",
            "organizer@example.net",
        )
        assertTrue(
            "the organiser's address must reach the copy",
            body.contains("organizer@example.net"),
        )
        assertTrue("the meeting must be named", body.contains("Quarterly planning"))
        compose.onNodeWithText(body).assertExists()
    }

    /**
     * The RFC 6638 status rides the prompt for the diagnostics log. "5.2" in a modal explains
     * nothing to the person reading it, so it must not appear, a regression here would be someone
     * helpfully appending it to the sentence.
     */
    @Test
    fun the_protocol_status_code_is_not_shown_to_the_user() {
        show(mutableListOf())

        compose.onNodeWithText("5.2", substring = true).assertDoesNotExist()
    }

    /**
     * A standing choice for every future meeting on this account is a bigger decision than the one
     * being asked, so it starts off.
     */
    @Test
    fun remembering_is_off_until_the_user_asks_for_it() {
        val answers = mutableListOf<Pair<Boolean, Boolean>>()
        show(answers)

        compose.onNodeWithText(L10n.invitation_reply_undelivered_remember(ctx())).assertIsOff()
        compose.onNodeWithText(L10n.invitation_reply_undelivered_send(ctx())).performClick()

        assertEquals(listOf(true to false), answers)
    }

    @Test
    fun sending_with_the_box_ticked_is_a_standing_yes() {
        val answers = mutableListOf<Pair<Boolean, Boolean>>()
        show(answers)

        tick()
        compose.onNodeWithText(L10n.invitation_reply_undelivered_send(ctx())).performClick()

        assertEquals(listOf(true to true), answers)
    }

    /**
     * The one that would be easy to get wrong: the tick applies to whichever button was pressed.
     * Wiring it only to the confirm button would turn "never email for this account" into silence:
     * the user would be asked again at every meeting, on exactly the server that fails every time.
     */
    @Test
    fun declining_with_the_box_ticked_is_a_standing_no() {
        val answers = mutableListOf<Pair<Boolean, Boolean>>()
        show(answers)

        tick()
        compose.onNodeWithText(L10n.invitation_reply_undelivered_dismiss(ctx())).performClick()

        assertEquals(listOf(false to true), answers)
    }

    /** Null is how the core says *close this*, so nothing may be left on screen. */
    @Test
    fun no_question_draws_nothing() {
        compose.setContent { AppTheme { InvitationReplyPrompt(null) { _, _ -> } } }
        compose.waitForIdle()

        compose.onNodeWithText(L10n.invitation_reply_undelivered_title(ctx())).assertDoesNotExist()
    }
}
