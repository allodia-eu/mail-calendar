package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MenuMessage
import uniffi.mailcal_bindings.MessageKind
import uniffi.mailcal_bindings.RowAction
import uniffi.mailcal_bindings.RowMenu
import uniffi.mailcal_bindings.RowMenuItem
import uniffi.mailcal_bindings.RowMenuSection
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.ThreadRow

// What a row menu's items do, what the selection bar reads off the same answer, and which message a
// listed conversation opens (docs/row-actions.md, docs/drafts.md).
class RowMenuPresentationTest {
    private val conversation = listOf(SelectedRow.Thread("acct-1", "thread-1"))
    private val received = MenuMessage(account = "acct-1", key = "newest", kind = MessageKind.RECEIVED)

    private fun item(action: RowAction, enabled: Boolean = true, target: MenuMessage? = null) =
        RowMenuItem(action = action, enabled = enabled, target = target)

    @Test
    fun an_item_naming_a_message_acts_on_that_message_alone() {
        fun on(action: RowAction) = rowPick(item(action, target = received), conversation)

        assertEquals(RowPick.Open(received), on(RowAction.OPEN))
        assertEquals(
            RowPick.Answer(ComposeOrigin.Reply(account = "acct-1", key = "newest", all = true)),
            on(RowAction.REPLY_ALL),
        )
        assertEquals(
            RowPick.Write(Intent.SetFlagged(account = "acct-1", key = "newest", flagged = true)),
            on(RowAction.FLAG),
        )
        assertEquals(
            RowPick.Write(Intent.MarkAsSpam(account = "acct-1", key = "newest")),
            on(RowAction.MARK_AS_SPAM),
        )
        // A phone opens no window, whatever it is offered.
        assertNull(on(RowAction.OPEN_IN_WINDOW))
    }

    // A conversation's move, read state and unflag reach members the core works out when the write
    // lands, so the item names no message and the batch names the row.
    @Test
    fun an_item_naming_no_message_acts_on_the_rows_the_menu_was_asked_about() {
        for ((action, batch) in listOf(
            RowAction.MARK_READ to BulkAction.MARK_READ,
            RowAction.UNFLAG to BulkAction.UNFLAG,
            RowAction.ARCHIVE to BulkAction.ARCHIVE,
            RowAction.DELETE to BulkAction.DELETE,
            RowAction.PERMANENTLY_DELETE to BulkAction.PERMANENTLY_DELETE,
        )) {
            assertEquals(RowPick.Rows(conversation, batch), rowPick(item(action), conversation))
        }
        assertEquals(RowPick.Move(conversation), rowPick(item(RowAction.MOVE_TO_FOLDER), conversation))
        // Report as junk with nothing received to report: drawn disabled, and it does nothing.
        assertNull(rowPick(item(RowAction.MARK_AS_SPAM, enabled = false), conversation))
    }

    @Test
    fun the_bar_reads_its_toggles_and_what_responds_off_the_selections_menu() {
        val menu = RowMenu(
            sections = listOf(
                RowMenuSection(items = listOf(item(RowAction.MARK_UNREAD), item(RowAction.UNFLAG))),
                RowMenuSection(
                    items = listOf(
                        item(RowAction.ARCHIVE, enabled = false),
                        item(RowAction.DELETE),
                        item(RowAction.PERMANENTLY_DELETE),
                    ),
                ),
            ),
        )
        assertEquals(
            SelectionBarActions(
                read = BulkAction.MARK_UNREAD,
                readEnabled = true,
                flag = BulkAction.UNFLAG,
                flagEnabled = true,
                archive = false,
                delete = true,
                permanentlyDelete = true,
            ),
            SelectionBarActions.of(menu),
        )
    }

    // "Mark as unread" over nothing describes an action nobody asked for.
    @Test
    fun an_empty_menu_names_the_affirmative_actions_and_enables_none() {
        assertEquals(
            SelectionBarActions(
                read = BulkAction.MARK_READ,
                readEnabled = false,
                flag = BulkAction.FLAG,
                flagEnabled = false,
                archive = false,
                delete = false,
                permanentlyDelete = false,
            ),
            SelectionBarActions.of(RowMenu(sections = emptyList())),
        )
    }

    // Tapping a conversation opens its newest message whatever its kind, so a draft newest on a
    // conversation in the Inbox resumes in a composer; in Drafts it is the draft the row summarises.
    @Test
    fun a_conversation_opens_its_newest_message_and_a_draft_resumes_wherever_it_is_listed() {
        val inbox = thread(
            latestKey = "received",
            messages = listOf(message(key = "draft", draft = true), message(key = "received")),
        )
        val head = ListedOpening.head(inbox, showingDrafts = false)
        assertEquals("draft", head?.key)
        assertEquals(true, head?.draft)

        val drafts = thread(
            latestKey = "older-draft",
            messages = listOf(message(key = "received"), message(key = "older-draft", draft = true)),
        )
        assertEquals("received", ListedOpening.head(drafts, showingDrafts = false)?.key)
        assertEquals("older-draft", ListedOpening.head(drafts, showingDrafts = true)?.key)

        assertEquals(true, ListedOpening.resumes(received.copy(kind = MessageKind.DRAFT)))
        assertEquals(false, ListedOpening.resumes(received))
    }

    private fun message(key: String, draft: Boolean = false) = ThreadMessage(
        account = "acct-1",
        key = key,
        from = "Ada Lovelace",
        avatar = stubAvatar("AL"),
        date = "2026-07-10T11:34:41Z",
        preview = "",
        unread = false,
        flagged = false,
        outgoing = false,
        draft = draft,
        hasAttachment = false,
    )

    private fun thread(latestKey: String, messages: List<ThreadMessage>) = ThreadRow(
        account = "acct-1",
        threadId = "thread-1",
        latestKey = latestKey,
        subject = "Contribution for next year",
        latestFrom = "Ada Lovelace",
        avatar = stubAvatar("AL"),
        latestDate = "2026-07-10T11:34:41Z",
        messageCount = messages.size.toUInt(),
        unreadCount = 0u,
        flagged = false,
        hasAttachment = false,
        preview = "",
        messages = messages,
    )
}
