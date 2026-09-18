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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposeRequest
import uniffi.mailcal_bindings.QueuedRow
import uniffi.mailcal_bindings.QueuedState

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun queued(
    op: ULong,
    to: String = "ada@example.test",
    subject: String = "Lunch",
    state: QueuedState = QueuedState.WAITING,
    account: String = "work",
) = QueuedRow(
    account = account,
    op = op,
    to = to,
    subject = subject,
    state = state,
    attempts = 1u,
    detail = null,
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

    /**
     * **The rule the whole surface turns on.** Only a message still waiting may be acted on: one
     * in flight cannot be called back, and one whose delivery could not be confirmed may already
     * be in front of its recipients.
     */
    @Test
    fun `only a message still waiting may be acted on`() {
        assertTrue(isActionable(QueuedState.WAITING))
        assertFalse(isActionable(QueuedState.SENDING))
        assertFalse(isActionable(QueuedState.UNCONFIRMED))
    }

    @Test
    fun `every queued state is written as itself`() {
        val words = listOf(QueuedState.WAITING, QueuedState.SENDING, QueuedState.UNCONFIRMED)
            .map { queuedStateText(it, ctx()) }
        assertEquals("two states read as the same sentence", words.size, words.toSet().size)
        assertEquals(L10n.outbox_waiting(ctx()), queuedStateText(QueuedState.WAITING, ctx()))
        assertEquals(L10n.outbox_sending(ctx()), queuedStateText(QueuedState.SENDING, ctx()))
        assertEquals(
            L10n.outbox_unconfirmed(ctx()),
            queuedStateText(QueuedState.UNCONFIRMED, ctx()),
        )
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

    /**
     * A message that may already have been delivered carries no menu at all: asking to send it
     * again is how it arrives twice, and a menu that opens onto nothing is a worse offer than no
     * menu.
     */
    @Test
    fun `a message that cannot be called back is offered nothing`() {
        screen(
            listOf(
                queued(1u, subject = "Waiting"),
                queued(2u, subject = "Gone out", state = QueuedState.UNCONFIRMED),
            ),
        )
        compose.onNodeWithText("Gone out").assertIsDisplayed()
        compose.onNodeWithText(L10n.outbox_unconfirmed(ctx()), substring = true).assertIsDisplayed()
        // One menu on screen, belonging to the row that is still waiting.
        compose.onAllNodesWithContentDescription(L10n.a11y_more_actions(ctx()))
            .assertCountEquals(1)
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
     * Every field the core hands back reaches the composer, and From is the account the message
     * was **queued on**: on a device with two accounts, the selected mailbox's would send it as
     * an identity the recipient has never seen it from.
     */
    @Test
    fun `a withdrawn message opens holding everything it was queued with`() {
        val seed = withdrawnSeed(
            ComposeRequest(
                account = "home",
                to = "ada@example.test",
                cc = "copy@example.test",
                bcc = "audit@example.test",
                subject = "Lunch",
                bodyText = "One o'clock?",
            ),
        )
        assertEquals(
            WithdrawnSeed(
                from = "home",
                to = "ada@example.test",
                cc = "copy@example.test",
                bcc = "audit@example.test",
                subject = "Lunch",
                body = "One o'clock?",
            ),
            seed,
        )
    }
}
