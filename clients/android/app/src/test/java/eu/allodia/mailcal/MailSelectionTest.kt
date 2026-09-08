// The message list's selection rules (docs/list-selection.md), without composing a screen: what a
// tap does in selection mode, what survives a snapshot rebuild, which of the paired actions the
// bar offers, and what a conversation row becomes when the batch goes to the core.
//
// The last two fail silently in the running app: a button offering the action that changes nothing
// does nothing when tapped, and a conversation sent as its latest message archives one reply while
// the rest of the thread stays in the inbox.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.ThreadRow

class MailSelectionTest {
    private fun avatar() = Avatar(
        initials = "S",
        light = Swatch(background = "#4C6EF5", text = "#FFFFFF", border = "#3B5BDB"),
        dark = Swatch(background = "#4C6EF5", text = "#FFFFFF", border = "#3B5BDB"),
        imagePath = null,
    )

    private fun flat(key: String, unread: Boolean = false, flagged: Boolean = false) =
        SnapshotRow.Flat(
            FlatRow(
                account = "acct-1",
                key = key,
                subject = "Subject $key",
                from = "sender",
                avatar = avatar(),
                date = "2026-07-20",
                unread = unread,
                flagged = flagged,
                hasAttachment = false,
                preview = "",
            ),
        )

    private fun thread(id: String, unreadCount: UInt = 0u) = SnapshotRow.Thread(
        ThreadRow(
            account = "acct-1",
            threadId = id,
            latestKey = "$id-latest",
            subject = "Conversation",
            latestFrom = "sender",
            avatar = avatar(),
            latestDate = "2026-07-20",
            messageCount = 3u,
            unreadCount = unreadCount,
            hasAttachment = false,
            preview = "",
            messages = emptyList(),
        ),
    )

    private fun keys(selection: MailSelectionState) = selection.selectedRows().map { row ->
        when (row) {
            is SelectedRow.Message -> row.key
            is SelectedRow.Thread -> row.threadId
        }
    }

    @Test
    fun the_mode_is_on_only_while_something_is_selected() {
        val selection = MailSelectionState()
        assertFalse(selection.active)

        val row = flat("m1")
        selection.toggle(row)
        assertTrue(selection.active)

        selection.toggle(row)
        assertFalse("deselecting the last row leaves the mode", selection.active)
    }

    @Test
    fun a_tap_adds_then_removes_the_same_row() {
        val rows = listOf(flat("m1"), flat("m2"))
        val selection = MailSelectionState()

        selection.toggle(rows[0])
        selection.toggle(rows[1])
        assertEquals(listOf("m1", "m2"), keys(selection))

        selection.toggle(rows[1])
        assertEquals(listOf("m1"), keys(selection))
    }

    @Test
    fun a_row_that_leaves_the_list_leaves_the_selection_with_it() {
        // The archived rows are gone from the next snapshot. A selection that kept them would act
        // on messages nobody can see, and the first sign of it would be mail leaving the mailbox.
        val rows = listOf(flat("m1"), flat("m2"), flat("m3"))
        val selection = MailSelectionState()
        selection.selectAll(rows)

        selection.retainListed(listOf(flat("m2")))
        assertEquals(listOf("m2"), keys(selection))

        selection.retainListed(emptyList())
        assertFalse("an emptied list empties the selection", selection.active)
    }

    @Test
    fun the_bar_offers_mark_read_while_anything_selected_is_unread() {
        val unread = listOf(flat("m1", unread = true), flat("m2"))
        val selection = MailSelectionState()
        selection.selectAll(unread)
        assertEquals(BulkAction.MARK_READ, selection.readAction(unread))

        val read = listOf(flat("m1"), flat("m2"))
        assertEquals(BulkAction.MARK_UNREAD, selection.readAction(read))
    }

    @Test
    fun the_bar_offers_flag_while_anything_selected_is_unflagged() {
        val mixed = listOf(flat("m1", flagged = true), flat("m2"))
        val selection = MailSelectionState()
        selection.selectAll(mixed)
        assertEquals(BulkAction.FLAG, selection.flagAction(mixed))

        val flagged = listOf(flat("m1", flagged = true), flat("m2", flagged = true))
        assertEquals(BulkAction.UNFLAG, selection.flagAction(flagged))
    }

    @Test
    fun a_conversation_is_selected_as_a_thread_not_as_its_latest_message() {
        // The core expands a conversation itself, from the store's thread index; naming its latest
        // message here would archive one reply and leave the rest of the thread in the inbox.
        val rows = listOf(thread("t1", unreadCount = 2u))
        val selection = MailSelectionState()
        selection.selectAll(rows)

        assertEquals(
            listOf(SelectedRow.Thread("acct-1", "t1")),
            selection.selectedRows(),
        )
        assertEquals(BulkAction.MARK_READ, selection.readAction(rows))
        assertEquals(
            "a conversation carries no flag of its own, so flagging is what it can be asked for",
            BulkAction.FLAG,
            selection.flagAction(rows),
        )
    }

    @Test
    fun only_the_actions_that_empty_the_row_clear_the_selection() {
        assertTrue(BulkAction.ARCHIVE.removesRows())
        assertTrue(BulkAction.DELETE.removesRows())
        assertTrue(BulkAction.PERMANENTLY_DELETE.removesRows())
        assertFalse(BulkAction.MARK_READ.removesRows())
        assertFalse(BulkAction.MARK_UNREAD.removesRows())
        assertFalse(BulkAction.FLAG.removesRows())
        assertFalse(BulkAction.UNFLAG.removesRows())
    }
}
