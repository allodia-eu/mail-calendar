// The Outbox's rules (docs/sending.md), and the ones that fail silently on screen.
//
// A state mapped to the wrong word tells someone their message is waiting when it is already on
// its way; a row that offers "Send now" on a message whose delivery could not be confirmed is how
// it arrives twice; and a field dropped on the way back out of the queue looks like an empty
// composer and nothing else. None of the three looks wrong.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposeRequest
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.OutboxIntent
import uniffi.mailcal_bindings.QueuedRow
import uniffi.mailcal_bindings.QueuedState
import uniffi.mailcal_bindings.SendStatus

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun queued(
    op: ULong,
    to: String = "ada@example.test",
    subject: String = "Lunch",
    state: QueuedState = QueuedState.WAITING,
    account: String = "work",
    editable: Boolean = true,
) = QueuedRow(
    account = account,
    op = op,
    to = to,
    subject = subject,
    state = state,
    attempts = 1u,
    detail = null,
    editable = editable,
)

private val accounts = listOf(
    AccountRow(id = "work", email = "me@work.example", name = "", expanded = true),
)

@RunWith(RobolectricTestRunner::class)
class OutboxTest {
    @get:Rule val compose = createComposeRule()

    private val acted = mutableListOf<Triple<String, ULong, QueuedAction>>()

    private fun screen(rows: List<QueuedRow>) {
        compose.setContent {
            OutboxScreen(
                queued = rows,
                accounts = accounts,
                onOpenDrawer = {},
                onAct = { account, op, action -> acted.add(Triple(account, op, action)) },
            )
        }
    }

    private fun actions(state: QueuedState, editable: Boolean = true) =
        queuedActions(state, editable, ctx()).map { it.label to it.action }

    /**
     * **The rule the whole surface turns on.** A message in flight cannot be called back, and one
     * whose delivery could not be confirmed may already be in front of its recipients, so it is
     * offered only the two answers to that question: never Send Now, Edit or Cancel.
     */
    @Test
    fun `each state offers exactly its own actions`() {
        assertEquals(
            listOf(
                L10n.action_send_now(ctx()) to QueuedAction.SEND_NOW,
                L10n.action_edit_queued(ctx()) to QueuedAction.EDIT,
                L10n.action_cancel_send(ctx()) to QueuedAction.CANCEL,
            ),
            actions(QueuedState.WAITING),
        )
        assertEquals(emptyList<Pair<String, QueuedAction>>(), actions(QueuedState.SENDING))
        assertEquals(
            listOf(
                L10n.action_mark_sent(ctx()) to QueuedAction.CONFIRM_SENT,
                L10n.action_send_again(ctx()) to QueuedAction.CONFIRM_NOT_SENT,
            ),
            actions(QueuedState.UNCONFIRMED),
        )
        assertEquals(
            listOf(
                L10n.action_send_again(ctx()) to QueuedAction.SEND_NOW,
                L10n.action_edit_queued(ctx()) to QueuedAction.EDIT,
                L10n.action_discard(ctx()) to QueuedAction.CANCEL,
            ),
            actions(QueuedState.NOT_SENT),
        )
    }

    /**
     * A send no composer can hold (an invitation answer) offers no Edit, and nothing else about
     * its menu changes.
     */
    @Test
    fun `a send no composer can hold offers no edit`() {
        assertEquals(
            listOf(
                L10n.action_send_now(ctx()) to QueuedAction.SEND_NOW,
                L10n.action_cancel_send(ctx()) to QueuedAction.CANCEL,
            ),
            actions(QueuedState.WAITING, editable = false),
        )
        assertEquals(
            listOf(
                L10n.action_send_again(ctx()) to QueuedAction.SEND_NOW,
                L10n.action_discard(ctx()) to QueuedAction.CANCEL,
            ),
            actions(QueuedState.NOT_SENT, editable = false),
        )
        assertEquals(actions(QueuedState.UNCONFIRMED), actions(QueuedState.UNCONFIRMED, false))
        val row = outboxRows(listOf(queued(1u, editable = false)), accounts, ctx()).single()
        assertEquals(false, row.actions.any { it.action == QueuedAction.EDIT })
    }

    /** Only answering that an unconfirmed message did not arrive can deliver it twice. */
    @Test
    fun `only send again on an unconfirmed message asks first`() {
        assertEquals(
            setOf(QueuedAction.CONFIRM_NOT_SENT),
            QueuedAction.entries.filter(::queuedActionNeedsConfirming).toSet(),
        )
    }

    @Test
    fun `every queued state is written as itself`() {
        val words = QueuedState.entries.map { queuedStateText(it, ctx()) }
        assertEquals("two states read as the same sentence", words.size, words.toSet().size)
        assertEquals(L10n.outbox_waiting(ctx()), queuedStateText(QueuedState.WAITING, ctx()))
        assertEquals(L10n.outbox_sending(ctx()), queuedStateText(QueuedState.SENDING, ctx()))
        assertEquals(
            L10n.outbox_unconfirmed(ctx()),
            queuedStateText(QueuedState.UNCONFIRMED, ctx()),
        )
        assertEquals(L10n.outbox_not_sent(ctx()), queuedStateText(QueuedState.NOT_SENT, ctx()))
    }

    /** An unconfirmed delivery is a warning and a refused one an error; neither reads as waiting. */
    @Test
    fun `only the states that need the user draw a glyph`() {
        assertEquals(QueuedStateMark.NONE, queuedStateMark(QueuedState.WAITING))
        assertEquals(QueuedStateMark.NONE, queuedStateMark(QueuedState.SENDING))
        assertEquals(QueuedStateMark.WARNING, queuedStateMark(QueuedState.UNCONFIRMED))
        assertEquals(QueuedStateMark.ERROR, queuedStateMark(QueuedState.NOT_SENT))
    }

    @Test
    fun `each action is the intent of the same name`() {
        val expected = mapOf(
            QueuedAction.SEND_NOW to OutboxIntent.SendNow("work", 3uL),
            QueuedAction.EDIT to OutboxIntent.Edit("work", 3uL, "/cache/staging"),
            QueuedAction.CANCEL to OutboxIntent.Cancel("work", 3uL),
            QueuedAction.CONFIRM_SENT to OutboxIntent.ConfirmSent("work", 3uL),
            QueuedAction.CONFIRM_NOT_SENT to OutboxIntent.ConfirmNotSent("work", 3uL),
        )
        assertEquals(QueuedAction.entries.toSet(), expected.keys)
        expected.forEach { (action, intent) ->
            assertEquals(intent, outboxIntent("work", 3uL, action, "/cache/staging"))
        }
    }

    /**
     * Neither line is ever blank: an empty one reads as a rendering fault rather than as a fact,
     * and both genuinely come back empty (a message addressed by Bcc alone, and mail with no
     * subject).
     */
    @Test
    fun `a row with nothing to say still says something`() {
        val rows = outboxRows(listOf(queued(1u, to = "", subject = "")), accounts, ctx())
        assertEquals("me@work.example", rows.single().recipients)
        assertEquals(L10n.mail_no_subject(ctx()), rows.single().subject)
    }

    /** An account the user has since removed still names something rather than nothing. */
    @Test
    fun `every row names the account it would go out from`() {
        val rows = outboxRows(
            listOf(queued(1u), queued(2u, account = "gone")),
            accounts,
            ctx(),
        )
        assertEquals("me@work.example", rows[0].accountText)
        assertEquals("gone", rows[1].accountText)
    }

    @Test
    fun `the list names its recipients its subject its state and its account`() {
        screen(listOf(queued(1u)))
        compose.onNodeWithText("Lunch").assertIsDisplayed()
        compose.onNodeWithText("ada@example.test").assertIsDisplayed()
        compose.onNodeWithText(
            "${L10n.outbox_waiting(ctx())} · me@work.example",
        ).assertIsDisplayed()
    }

    /**
     * The three actions reach the core naming the send by its account **and** its op id: an op id
     * is unique only within its own account's queue, and this list holds every account's at once.
     */
    @Test
    fun `each action names the queued send by its account and its op`() {
        screen(listOf(queued(7u)))
        compose.onNodeWithContentDescription(L10n.a11y_more_actions(ctx())).performClick()
        compose.onNodeWithText(L10n.action_send_now(ctx())).performClick()
        assertEquals(listOf(Triple("work", 7uL, QueuedAction.SEND_NOW)), acted)
    }

    /** A message in flight carries no menu at all: one that opens onto nothing is a worse offer. */
    @Test
    fun `a message in flight is offered nothing`() {
        screen(
            listOf(
                queued(1u, subject = "Waiting"),
                queued(2u, subject = "On its way", state = QueuedState.SENDING),
            ),
        )
        compose.onNodeWithText("On its way").assertIsDisplayed()
        compose.onNodeWithText(L10n.outbox_sending(ctx()), substring = true).assertIsDisplayed()
        // One menu on screen, belonging to the row that is still waiting.
        compose.onAllNodesWithContentDescription(L10n.a11y_more_actions(ctx()))
            .assertCountEquals(1)
    }

    /**
     * Send Again on an unconfirmed message is the user saying it did not arrive, and only that
     * answer may send it again. It asks first, because if it did arrive the recipients receive it
     * twice, and nothing reaches the core until the user confirms.
     */
    @Test
    fun `send again on an unconfirmed message asks before it answers that it did not arrive`() {
        screen(listOf(queued(4u, subject = "Gone out", state = QueuedState.UNCONFIRMED)))
        compose.onNodeWithText(L10n.outbox_unconfirmed(ctx()), substring = true).assertIsDisplayed()
        compose.onNodeWithContentDescription(L10n.a11y_more_actions(ctx())).performClick()
        compose.onNodeWithText(L10n.action_send_now(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.action_cancel_send(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.action_send_again(ctx())).performClick()
        compose.onNodeWithText(L10n.outbox_send_again_title(ctx())).assertIsDisplayed()
        assertEquals(emptyList<Triple<String, ULong, QueuedAction>>(), acted)
        compose.onNodeWithText(L10n.action_send_again(ctx())).performClick()
        compose.onNodeWithText(L10n.outbox_send_again_title(ctx())).assertDoesNotExist()
        assertEquals(listOf(Triple("work", 4uL, QueuedAction.CONFIRM_NOT_SENT)), acted)
        assertEquals(
            OutboxIntent.ConfirmNotSent("work", 4uL),
            outboxIntent("work", 4uL, acted.single().third, "/cache/staging"),
        )
    }

    @Test
    fun `cancelling the question sends nothing`() {
        screen(listOf(queued(4u, state = QueuedState.UNCONFIRMED)))
        compose.onNodeWithContentDescription(L10n.a11y_more_actions(ctx())).performClick()
        compose.onNodeWithText(L10n.action_send_again(ctx())).performClick()
        compose.onNodeWithText(L10n.action_cancel(ctx())).performClick()
        compose.onNodeWithText(L10n.outbox_send_again_title(ctx())).assertDoesNotExist()
        assertEquals(emptyList<Triple<String, ULong, QueuedAction>>(), acted)
    }

    /** A refused message was never delivered, so sending it again is an ordinary Send Now. */
    @Test
    fun `send again on a refused message sends it now`() {
        screen(listOf(queued(5u, subject = "Refused", state = QueuedState.NOT_SENT)))
        compose.onNodeWithText(L10n.outbox_not_sent(ctx()), substring = true).assertIsDisplayed()
        compose.onNodeWithContentDescription(L10n.a11y_more_actions(ctx())).performClick()
        compose.onNodeWithText(L10n.action_discard(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.action_send_again(ctx())).performClick()
        compose.onNodeWithText(L10n.outbox_send_again_title(ctx())).assertDoesNotExist()
        assertEquals(listOf(Triple("work", 5uL, QueuedAction.SEND_NOW)), acted)
        assertEquals(
            OutboxIntent.SendNow("work", 5uL),
            outboxIntent("work", 5uL, acted.single().third, "/cache/staging"),
        )
    }

    /** Cancelling the last row empties the list under the user, and it has to say so once. */
    @Test
    fun `an emptied outbox says so rather than going blank`() {
        screen(emptyList())
        compose.onNodeWithText(L10n.outbox_empty(ctx())).assertIsDisplayed()
        // Not twice: "0 waiting to send" over "Nothing is waiting to be sent." is the same
        // sentence written out in both places.
        compose.onNodeWithText(L10n.a11y_outbox_count(ctx(), 0)).assertDoesNotExist()
    }

    /**
     * Every field the core hands back reaches the composer. The composition is the one the core
     * saved the draft under, so the composer saves over it rather than beside it, and every staged
     * file is held, because the first save replaces the draft and a file left out is taken off it.
     * From is the account the message was **queued on**: on a device with two accounts, the
     * selected mailbox's would send it as an identity the recipient has never seen it from.
     */
    @Test
    fun `a withdrawn message opens holding everything it was queued with`() {
        val files = listOf(
            ComposerFileAttachment(
                path = "/cache/resumed-drafts/x/agenda.pdf",
                fileName = "agenda.pdf",
                mediaType = "application/pdf",
            ),
            ComposerFileAttachment(
                path = "/cache/resumed-drafts/x/map.png",
                fileName = "map.png",
                mediaType = "image/png",
            ),
        )
        val seed = reopenedSeed(
            ComposeRequest(
                account = "home",
                composition = "draft-7",
                to = "ada@example.test",
                cc = "copy@example.test",
                bcc = "audit@example.test",
                subject = "Lunch",
                bodyHtml = "<p><strong>One</strong> o'clock?</p>",
                bodyText = "One o'clock?",
                attachments = files,
            ),
        )
        assertEquals(
            ReopenedSeed(
                composition = "draft-7",
                from = "home",
                to = "ada@example.test",
                cc = "copy@example.test",
                bcc = "audit@example.test",
                subject = "Lunch",
                html = "<p><strong>One</strong> o'clock?</p>",
                text = "One o'clock?",
                attachments = files,
            ),
            seed,
        )
    }

    /** A refused message says where it now is, rather than only that it failed. */
    @Test
    fun `a refused send says it is in the outbox`() {
        compose.setContent { SendStatusBanner(SendStatus.NOT_SENT, ctx()) }
        compose.onNodeWithText(L10n.send_status_not_sent(ctx())).assertIsDisplayed()
    }
}
