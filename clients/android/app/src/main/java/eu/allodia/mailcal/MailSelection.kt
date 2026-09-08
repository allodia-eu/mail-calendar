// Selecting several messages in the list and acting on the lot at once (docs/list-selection.md).
//
// A phone has no modifier keys, so this is a mode: a long press enters it, a tap then toggles a
// row instead of opening it, and Back leaves it. The rules live in [MailSelectionState], a plain
// class rather than a tangle of `remember`s in MailboxScreen, so what a selection survives and
// which action its bar offers are unit-testable without composing a screen.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.SnapshotRow

/**
 * One selected row's identity: the account, plus a message key or a thread id.
 *
 * Account-scoped, because a provider key is unique only within its account and the unified list
 * shows several at once. [thread] decides which shape the row travels to the core in: a
 * conversation must go as a thread, since the core expands it from the store's thread index, which
 * holds messages the list never showed.
 */
internal data class SelectionKey(val account: String, val id: String, val thread: Boolean) {
    fun selectedRow(): SelectedRow =
        if (thread) {
            SelectedRow.Thread(account, id)
        } else {
            SelectedRow.Message(account, id)
        }

    companion object {
        fun of(row: SnapshotRow): SelectionKey = when (row) {
            is SnapshotRow.Flat -> SelectionKey(row.row.account, row.row.key, thread = false)
            is SnapshotRow.Thread -> SelectionKey(row.row.account, row.row.threadId, thread = true)
        }
    }
}

/**
 * The message list's selection mode: which rows are picked, and what the bar over them offers.
 *
 * Holds Compose snapshot state so a screen reads [keys] directly, but composes nothing itself.
 */
internal class MailSelectionState {
    /** The picked rows, in the order they were picked. Empty means the mode is off. */
    var keys by mutableStateOf<List<SelectionKey>>(emptyList())
        private set

    /** Whether the list is in selection mode: a tap toggles rather than opens. */
    val active: Boolean get() = keys.isNotEmpty()

    val count: Int get() = keys.size

    fun contains(row: SnapshotRow): Boolean = keys.contains(SelectionKey.of(row))

    /** Adds or removes one row. Removing the last one leaves selection mode. */
    fun toggle(row: SnapshotRow) {
        val key = SelectionKey.of(row)
        keys = if (keys.contains(key)) keys - key else keys + key
    }

    /**
     * Selects every row the list is showing, which is the loaded window rather than the whole
     * folder (docs/list-selection.md, rule 10).
     */
    fun selectAll(rows: List<SnapshotRow>) {
        keys = rows.map { SelectionKey.of(it) }
    }

    fun clear() {
        keys = emptyList()
    }

    /**
     * Drops anything [rows] no longer holds: a message that was archived, a folder the user has
     * left, a search that replaced the list. A selection outliving its list acts on rows nobody
     * can see (docs/list-selection.md, rule 4).
     */
    fun retainListed(rows: List<SnapshotRow>) {
        val listed = rows.map { SelectionKey.of(it) }.toSet()
        val kept = keys.filter(listed::contains)
        if (kept.size != keys.size) {
            keys = kept
        }
    }

    /** The selected rows in the shape the core's batched intent takes. */
    fun selectedRows(): List<SelectedRow> = keys.map(SelectionKey::selectedRow)

    /**
     * The action the bar's single read button runs: the one that changes something, so any unread
     * row makes it "mark read" (docs/list-selection.md, rule 5).
     */
    fun readAction(rows: List<SnapshotRow>): BulkAction =
        if (selected(rows).any(::isUnread)) BulkAction.MARK_READ else BulkAction.MARK_UNREAD

    /**
     * The action the bar's single flag button runs, on the same terms. A conversation carries no
     * flag of its own, so it counts as unflagged: flagging is what its rows can be asked for.
     */
    fun flagAction(rows: List<SnapshotRow>): BulkAction =
        if (selected(rows).any(::isUnflagged)) BulkAction.FLAG else BulkAction.UNFLAG

    private fun selected(rows: List<SnapshotRow>): List<SnapshotRow> = rows.filter(::contains)

    private fun isUnread(row: SnapshotRow): Boolean = when (row) {
        is SnapshotRow.Flat -> row.row.unread
        is SnapshotRow.Thread -> row.row.unreadCount > 0u
    }

    private fun isUnflagged(row: SnapshotRow): Boolean = when (row) {
        is SnapshotRow.Flat -> !row.row.flagged
        is SnapshotRow.Thread -> true
    }
}

/**
 * Whether the action takes its rows out of the folder, so the selection has nothing left to name
 * afterwards. Read and flag change a message in place and leave the set alone, because the user is
 * usually part-way through working through it.
 */
internal fun BulkAction.removesRows(): Boolean = when (this) {
    BulkAction.ARCHIVE, BulkAction.DELETE, BulkAction.PERMANENTLY_DELETE -> true
    BulkAction.MARK_READ, BulkAction.MARK_UNREAD, BulkAction.FLAG, BulkAction.UNFLAG -> false
}
