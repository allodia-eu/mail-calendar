// What the core's row menu looks like on Android, and what picking an item does
// (docs/row-actions.md). The core decides which items a menu holds, which respond and which message
// each is about; this file gives each action its words and says what a pick runs, and keeps no
// rule of its own about either. Plain, so the JVM suite tests it without composing a screen.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MenuMessage
import uniffi.mailcal_bindings.MessageKind
import uniffi.mailcal_bindings.RowAction
import uniffi.mailcal_bindings.RowMenu
import uniffi.mailcal_bindings.RowMenuItem
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.ThreadRow

/** The item's label, in the user's language: the selection bar's words for what the two share. */
internal fun RowAction.label(ctx: Context): String = when (this) {
    RowAction.OPEN -> L10n.action_open(ctx)
    RowAction.OPEN_IN_WINDOW -> L10n.action_open_in_window(ctx)
    RowAction.REPLY -> L10n.action_reply(ctx)
    RowAction.REPLY_ALL -> L10n.action_reply_all(ctx)
    RowAction.FORWARD -> L10n.action_forward(ctx)
    RowAction.MARK_READ -> L10n.action_mark_read(ctx)
    RowAction.MARK_UNREAD -> L10n.action_mark_unread(ctx)
    RowAction.FLAG -> L10n.action_flag(ctx)
    RowAction.UNFLAG -> L10n.action_unflag(ctx)
    RowAction.MARK_AS_SPAM -> L10n.action_mark_as_spam(ctx)
    RowAction.MARK_AS_NOT_SPAM -> L10n.action_mark_as_not_spam(ctx)
    RowAction.MOVE_TO_FOLDER -> L10n.action_move_to_folder(ctx)
    RowAction.ARCHIVE -> L10n.action_archive(ctx)
    RowAction.DELETE -> L10n.action_move_to_trash(ctx)
    RowAction.PERMANENTLY_DELETE -> L10n.action_delete_permanently(ctx)
}

/** What picking a row menu's item does. */
internal sealed interface RowPick {
    /** Open the one message, resuming it in a composer when it is a draft. */
    data class Open(val message: MenuMessage) : RowPick

    /** Reply, reply all or forward the one message. */
    data class Answer(val origin: ComposeOrigin) : RowPick

    /** A write on the one message. */
    data class Write(val intent: Intent) : RowPick

    /** The batch over the rows the menu was asked about. */
    data class Rows(val rows: List<SelectedRow>, val action: BulkAction) : RowPick

    /** Ask for a folder, then move the rows the menu was asked about. */
    data class Move(val rows: List<SelectedRow>) : RowPick
}

/**
 * What picking [item] does, on the one message it names or else on [rows]. Null for an item the
 * core never enables here: one naming no message with no batch form, or a window, which a phone
 * does not open.
 */
internal fun rowPick(item: RowMenuItem, rows: List<SelectedRow>): RowPick? {
    if (item.action == RowAction.MOVE_TO_FOLDER) return RowPick.Move(rows)
    val message = item.target ?: return item.action.batch()?.let { RowPick.Rows(rows, it) }
    val (account, key) = message.account to message.key
    return when (item.action) {
        RowAction.OPEN -> RowPick.Open(message)
        RowAction.OPEN_IN_WINDOW -> null
        RowAction.REPLY -> RowPick.Answer(ComposeOrigin.Reply(account, key, false))
        RowAction.REPLY_ALL -> RowPick.Answer(ComposeOrigin.Reply(account, key, true))
        RowAction.FORWARD -> RowPick.Answer(ComposeOrigin.Forward(account, key))
        RowAction.MARK_READ -> RowPick.Write(Intent.MarkRead(account, key, true))
        RowAction.MARK_UNREAD -> RowPick.Write(Intent.MarkRead(account, key, false))
        RowAction.FLAG -> RowPick.Write(Intent.SetFlagged(account, key, true))
        RowAction.UNFLAG -> RowPick.Write(Intent.SetFlagged(account, key, false))
        RowAction.MARK_AS_SPAM -> RowPick.Write(Intent.MarkAsSpam(account, key))
        RowAction.MARK_AS_NOT_SPAM -> RowPick.Write(Intent.MarkAsNotSpam(account, key))
        RowAction.ARCHIVE -> RowPick.Write(Intent.Archive(account, key))
        RowAction.DELETE -> RowPick.Write(Intent.Delete(account, key))
        RowAction.PERMANENTLY_DELETE -> RowPick.Write(Intent.PermanentlyDelete(account, key))
        RowAction.MOVE_TO_FOLDER -> RowPick.Move(rows)
    }
}

/** The batch an item without a target runs, or null for an action that always names a message. */
private fun RowAction.batch(): BulkAction? = when (this) {
    RowAction.MARK_READ -> BulkAction.MARK_READ
    RowAction.MARK_UNREAD -> BulkAction.MARK_UNREAD
    RowAction.FLAG -> BulkAction.FLAG
    RowAction.UNFLAG -> BulkAction.UNFLAG
    RowAction.ARCHIVE -> BulkAction.ARCHIVE
    RowAction.DELETE -> BulkAction.DELETE
    RowAction.PERMANENTLY_DELETE -> BulkAction.PERMANENTLY_DELETE
    RowAction.OPEN, RowAction.OPEN_IN_WINDOW, RowAction.REPLY, RowAction.REPLY_ALL,
    RowAction.FORWARD, RowAction.MARK_AS_SPAM, RowAction.MARK_AS_NOT_SPAM,
    RowAction.MOVE_TO_FOLDER,
    -> null
}

/**
 * The selection bar's actions, read off the core's menu over the selection
 * (docs/list-selection.md, rule 5): each toggle names the half that changes something, and an
 * action the menu would draw disabled is disabled on the bar too.
 *
 * With nothing selected the core answers no menu, and the bar names the affirmative halves, all
 * disabled: "Mark as unread" over nothing describes an action nobody asked for.
 */
internal data class SelectionBarActions(
    val read: BulkAction,
    val readEnabled: Boolean,
    val flag: BulkAction,
    val flagEnabled: Boolean,
    val archive: Boolean,
    val delete: Boolean,
    val permanentlyDelete: Boolean,
) {
    companion object {
        fun of(menu: RowMenu): SelectionBarActions {
            val items = menu.sections.flatMap { it.items }
            // The first item doing one of `actions`, which for a toggle is the half on offer.
            fun find(vararg actions: RowAction): Pair<RowAction, Boolean> =
                items.firstOrNull { it.action in actions }?.let { it.action to it.enabled }
                    ?: (actions[0] to false)
            val (read, readEnabled) = find(RowAction.MARK_READ, RowAction.MARK_UNREAD)
            val (flag, flagEnabled) = find(RowAction.FLAG, RowAction.UNFLAG)
            return SelectionBarActions(
                read = read.batch() ?: BulkAction.MARK_READ,
                readEnabled = readEnabled,
                flag = flag.batch() ?: BulkAction.FLAG,
                flagEnabled = flagEnabled,
                archive = find(RowAction.ARCHIVE).second,
                delete = find(RowAction.DELETE).second,
                permanentlyDelete = find(RowAction.PERMANENTLY_DELETE).second,
            )
        }
    }
}

/**
 * Which message a listed row opens, and whether it resumes in a composer (docs/drafts.md). Told by
 * the **row**, never by the folder, so a draft met in a search result or inside a conversation
 * shown from another folder resumes as one in Drafts does.
 */
internal object ListedOpening {
    /**
     * The message tapping [thread] opens: its newest, whatever its kind, a Sent reply filed
     * elsewhere or a draft included (docs/row-actions.md, rule 2). In Drafts it is the newest
     * draft, the one the row summarises.
     */
    fun head(thread: ThreadRow, showingDrafts: Boolean): ThreadMessage? {
        val summarised = thread.messages.firstOrNull { it.key == thread.latestKey }
        return (if (showingDrafts) summarised else null) ?: thread.messages.firstOrNull()
    }

    /** Whether the message an item names opens in a composer. */
    fun resumes(message: MenuMessage): Boolean = message.kind == MessageKind.DRAFT
}
