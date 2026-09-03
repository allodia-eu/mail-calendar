// What survives leaving the mailbox for a message and coming back.
//
// MainScreen REPLACES the list with the reading view rather than covering it, so everything the
// screen remembered is gone by the time the user returns: they came back to the top of the inbox,
// and out of a search whose query the core was still applying. Both are now held by the activity
// (MailListPosition, SearchBarState).
//
// These tests drive the real MailboxScreen and swap it out the way MainScreen does. A stub list
// would pass with the state remembered inside the screen, which is exactly the bug.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasScrollToIndexAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings

private const val ACCOUNT = "acct-1"
private const val ROWS = 60
private const val READING = "the message body"
private const val QUERY = "report"

// Comfortably past SearchField's typing debounce.
private const val SEARCH_SETTLED_MS = 400L

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun subjectOf(index: Int) = "Message $index"

private fun mailbox(): List<SnapshotRow> = (0 until ROWS).map { index ->
    SnapshotRow.Flat(
        FlatRow(
            account = ACCOUNT,
            key = "imap:v1:u$index@INBOX",
            subject = subjectOf(index),
            from = "someone@remote.test",
            avatar = stubAvatar(),
            date = "2026-07-10T11:34:41Z",
            unread = false,
            flagged = false,
            hasAttachment = false,
            preview = "",
        ),
    )
}

// The real screen, with everything these tests do not care about stubbed out.
@Composable
private fun TestMailbox(position: MailListPosition, search: SearchBarState) {
    MailboxScreen(
        rows = mailbox(),
        sendStatus = SendStatus.IDLE,
        accounts = listOf(AccountRow(id = ACCOUNT, email = "me@local.test", expanded = false)),
        selectedAccount = ACCOUNT,
        onSelectAccount = {},
        onAddAccount = {},
        onRemoveAccount = {},
        drawerState = rememberDrawerState(DrawerValue.Closed),
        position = position,
        search = search,
        currentScopeLabel = "Inbox",
        onRefresh = {},
        onShowMore = {},
        onOpen = {},
        onOpenThread = {},
        onSetRead = { _, _, _ -> },
        onSetFlagged = { _, _, _ -> },
        onDelete = { _, _ -> },
        onPermanentlyDelete = { _, _ -> },
        inJunkFolder = false,
        onMarkAsSpam = { _, _ -> },
        onMarkAsNotSpam = { _, _ -> },
        onActOnSelection = { _, _ -> },
        onReply = { _, _, _, _, _, _ -> true },
        onForward = { _, _, _, _, _, _ -> true },
        replyRecipients = { _, _, _ -> null },
        onSubmitRich = { _, _, _, _, _ -> true },
        swipe = SwipeSettings(SwipeActionKind.DELETE, SwipeActionKind.DELETE),
        onArchive = { _, _ -> },
        defaultSendAccount = null,
        timeZone = null,
        onAcceptTimeZoneChange = {},
        onDismissTimeZoneChange = {},
        syncProgress = null,
        offline = false,
        unreachableAccounts = emptyList(),
        connectionIssues = emptyList(),
        onOpenSettings = {},
    )
}

@RunWith(RobolectricTestRunner::class)
class MailboxReturnTest {

    @get:Rule
    val rule = createComposeRule()

    private val position = MailListPosition()

    /** Everything the search chrome dispatched at the core, in order. */
    private val queries = mutableListOf<String?>()
    private val search = SearchBarState(onSearch = { queries += it }, onSetScope = {})

    /** The mailbox, with a switch that opens a message the way MainScreen's `when` does. */
    private lateinit var openMessage: (Boolean) -> Unit

    private fun mailboxAndReadingView() {
        var reading by mutableStateOf(false)
        rule.setContent {
            AppTheme {
                if (reading) Text(READING) else TestMailbox(position, search)
            }
        }
        openMessage = { open ->
            rule.runOnUiThread { reading = open }
            rule.waitForIdle()
        }
    }

    @Test
    fun returning_from_a_message_lands_on_the_row_the_user_left() {
        mailboxAndReadingView()

        rule.onNode(hasScrollToIndexAction()).performScrollToIndex(40)
        rule.waitForIdle()
        val left = position.listState.firstVisibleItemIndex
        assertTrue("the list has to be scrolled away from the top to prove anything", left > 0)
        rule.onNodeWithText(subjectOf(left)).assertIsDisplayed()

        openMessage(true)
        rule.onNodeWithText(READING).assertIsDisplayed()
        openMessage(false)

        assertEquals(left, position.listState.firstVisibleItemIndex)
        rule.onNodeWithText(subjectOf(left)).assertIsDisplayed()
    }

    @Test
    fun returning_from_a_message_keeps_the_search_the_core_is_still_applying() {
        mailboxAndReadingView()

        rule.onNodeWithContentDescription(L10n.search_placeholder(ctx())).performClick()
        rule.onNode(hasSetTextAction()).performTextInput(QUERY)
        // Past the typing debounce, so the core has actually been asked.
        rule.mainClock.advanceTimeBy(SEARCH_SETTLED_MS)
        rule.waitForIdle()
        assertEquals(listOf<String?>(QUERY), queries)

        openMessage(true)
        openMessage(false)
        // Give the re-armed debounce every chance to fire, so the last assertion below is an
        // observation rather than a race this test happened to win.
        rule.mainClock.advanceTimeBy(SEARCH_SETTLED_MS)
        rule.waitForIdle()

        // The core clears its query on nothing but this client asking, so a field that closed
        // itself here would leave the list narrowed with nothing on screen saying so.
        assertTrue(search.open)
        rule.onNodeWithText(QUERY).assertIsDisplayed()
        // And the return is not itself a search: the debounce effect re-arms on re-entry.
        assertEquals(listOf<String?>(QUERY), queries)
    }
}
