// What the core's row menu and command bar look like on Android, and what picking an item does
// (docs/row-actions.md, docs/command-bar.md). The core decides which items a menu holds, which respond and which message
// each is about; this file gives each action its words and says what a pick runs, and keeps no
// rule of its own about either. Plain, so the JVM suite tests it without composing a screen.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.Command
import uniffi.mailcal_bindings.CommandBar
import uniffi.mailcal_bindings.CommandItem
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MenuMessage
import uniffi.mailcal_bindings.MessageKind
import uniffi.mailcal_bindings.RowAction
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
    RowAction.DELETE -> L10n.action_delete(ctx)
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

    /** Ask before [then], which cannot be undone; [rows] are what the question names. */
    data class Ask(val rows: List<SelectedRow>, val then: RowPick) : RowPick
}

/**
 * What picking [item] does, on the one message it names or else on [rows]. Null for an item the
 * core never enables here: one naming no message with no batch form, or a window, which a phone
 * does not open.
 */
internal fun rowPick(item: RowMenuItem, rows: List<SelectedRow>): RowPick? {
    val pick = plainPick(item, rows) ?: return null
    if (!item.confirm) return pick
    val named = item.target?.let { listOf(SelectedRow.Message(it.account, it.key)) } ?: rows
    return RowPick.Ask(named, pick)
}

private fun plainPick(item: RowMenuItem, rows: List<SelectedRow>): RowPick? {
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

/** The bar item as the row menu names it, so a pick runs the menu's own path; null for Sync. */
internal fun CommandItem.menuItem(): RowMenuItem? = when (val command = command) {
    is Command.Act -> RowMenuItem(command.action, enabled, target, confirm)
    Command.Sync -> null
}

/** The bar's Delete, which in Trash is Delete permanently (docs/command-bar.md, rule 6). */
internal fun CommandBar.deleteItem(): CommandItem? = sections.flatMap { it.items }.firstOrNull {
    val action = (it.command as? Command.Act)?.action
    action == RowAction.DELETE || action == RowAction.PERMANENTLY_DELETE
}

/** What the question before deleting for good says about [rows]: one, or how many, of which. */
internal fun purgeMessage(ctx: Context, rows: List<SelectedRow>): String {
    val threads = rows.count { it is SelectedRow.Thread }
    return when {
        rows.size == 1 && threads == 1 -> L10n.delete_permanently_conversation(ctx)
        rows.size == 1 -> L10n.delete_permanently_message(ctx)
        threads == 0 -> L10n.delete_permanently_message_many(ctx, rows.size.toLong())
        else -> L10n.delete_permanently_conversations(ctx, rows.size.toLong())
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
