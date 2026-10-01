// What the folder drawer offers on a row, where a message row may file mail, and the copy each
// folder dialog shows (docs/folder-pane.md, "Changing the tree"). Plain functions over the core's
// rows, so the rules are tested without composing a drawer.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.FolderIntent
import uniffi.mailcal_bindings.FolderNameCheck
import uniffi.mailcal_bindings.FolderNotice
import uniffi.mailcal_bindings.FolderProblem
import uniffi.mailcal_bindings.FolderRole
import uniffi.mailcal_bindings.FolderRow
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.SelectedRow

// The core's two doors for the drawer: the change itself, and the name check a dialog runs as the
// user types. A class rather than the app object so a test can hand the drawer fakes.
internal class FolderEditing(
    val dispatch: (FolderIntent) -> Unit,
    val checkName: (account: String, parent: String?, name: String, renaming: String?) -> FolderNameCheck,
) {
    companion object {
        fun of(app: MailcalApp) = FolderEditing(
            dispatch = { app.dispatch(Intent.Folders(it)) },
            checkName = { account, parent, name, renaming ->
                app.checkFolderName(account, parent, name, renaming)
            },
        )
    }
}

internal enum class FolderMenuItem { NEW_FOLDER, RENAME, MOVE, DELETE }

// Rule 22: a row offers only what the core stamped on it. A pending row offers nothing, which the
// core's flags already say; the explicit check keeps that true if a flag ever drifts.
internal fun folderMenu(row: FolderRow): List<FolderMenuItem> {
    if (row.pending) return emptyList()
    return buildList {
        if (row.acceptsFolders) add(FolderMenuItem.NEW_FOLDER)
        if (row.editable) {
            add(FolderMenuItem.RENAME)
            add(FolderMenuItem.MOVE)
            add(FolderMenuItem.DELETE)
        }
    }
}

// An account row offers New folder alone, and only where its provider can change the tree.
internal fun accountMenu(folders: AccountFolderRow?): List<FolderMenuItem> =
    if (folders?.managesFolders == true) listOf(FolderMenuItem.NEW_FOLDER) else emptyList()

internal fun menuLabel(item: FolderMenuItem, ctx: Context): String =
    when (item) {
        FolderMenuItem.NEW_FOLDER -> L10n.folder_action_new(ctx)
        FolderMenuItem.RENAME -> L10n.folder_action_rename(ctx)
        FolderMenuItem.MOVE -> L10n.folder_action_move(ctx)
        FolderMenuItem.DELETE -> L10n.folder_action_delete(ctx)
    }

// Each row's name with the folders it sits inside ("Clients / Acme"), walking the rows' own
// parents the way the core's `folder_paths` does. A picker row's accessible name: an indent is not
// read aloud, and two folders called `2024` would otherwise sound alike.
internal fun folderPaths(rows: List<FolderRow>, ctx: Context): Map<String, String> {
    val byKey = rows.associateBy { it.key }
    return rows.associate { row ->
        val parts = mutableListOf(folderLabel(row.role, row.name, ctx))
        var parent = row.parent
        // Bounded by the number of rows, so a chain that loops cannot hang the walk.
        while (parent != null && parts.size <= rows.size) {
            val ancestor = byKey[parent] ?: break
            parts.add(folderLabel(ancestor.role, ancestor.name, ctx))
            parent = ancestor.parent
        }
        row.key to parts.asReversed().joinToString(" / ")
    }
}

// One row of a picker drawn as the drawer's tree. `key` null is the top of the account's tree,
// drawn with the account's icon; `name` is what the row shows and `label` what it is called to
// assistive technology. A row that is not `enabled` is a parent kept so its folders stay under it.
internal data class MoveTarget(
    val key: String?,
    val name: String,
    val role: FolderRole?,
    val indent: Int,
    val enabled: Boolean,
    val label: String,
)

// Rule 24: Top level first, then every folder of the account that takes folders, leaving out
// the folder itself and everything inside it. The folders sit one step inside Top level.
internal fun moveTargets(folder: FolderRow, rows: List<FolderRow>, ctx: Context): List<MoveTarget> {
    val inside = mutableSetOf(folder.key)
    // Rows arrive depth-first, so a folder's descendants follow it and one pass finds them.
    for (row in rows) {
        if (row.parent in inside) inside.add(row.key)
    }
    val top = L10n.folder_move_top_level(ctx)
    return listOf(MoveTarget(null, top, null, indent = 0, enabled = true, label = top)) +
        treeTargets(rows, ctx, shift = 1) { it.acceptsFolders && it.key !in inside }
}

// Rule 24, for mail: the account's folders that take it, leaving out the one the list is showing.
// No Top level, because mail is filed in a folder.
internal fun messageTargets(rows: List<FolderRow>, showing: String?, ctx: Context): List<MoveTarget> =
    treeTargets(rows, ctx, shift = 0) { it.acceptsMessages && it.key != showing }

// The destinations in the drawer's order, each with every folder it sits inside: a parent that is
// no destination itself is kept, disabled, so the tree keeps its shape. `shift` is the indent the
// whole tree starts at.
private fun treeTargets(
    rows: List<FolderRow>,
    ctx: Context,
    shift: Int,
    isDestination: (FolderRow) -> Boolean,
): List<MoveTarget> {
    val byKey = rows.associateBy { it.key }
    val kept = mutableSetOf<String>()
    for (row in rows.filter(isDestination)) {
        kept.add(row.key)
        // `add` answers false at a parent already kept, whose own parents are kept with it, which
        // also ends a chain that loops.
        var parent = row.parent
        while (parent != null && kept.add(parent)) {
            parent = byKey[parent]?.parent
        }
    }
    val paths = folderPaths(rows, ctx)
    return rows.filter { it.key in kept }.map { row ->
        MoveTarget(
            key = row.key,
            name = folderLabel(row.role, row.name, ctx),
            role = row.role,
            indent = row.depth.toInt() + shift,
            enabled = isDestination(row),
            label = paths.getValue(row.key),
        )
    }
}

// A message row's route to a named folder: where a message of `account` may go, and the move,
// which takes the path a drop onto a folder takes (`FolderIntent.MoveMessages`). A class rather
// than the app object so a test can hand the row fakes.
internal class MessageFiling(
    val targets: (account: String) -> List<MoveTarget>,
    val move: (account: String, key: String, folder: String) -> Unit,
) {
    companion object {
        // Reads the activity's folders when the menu opens, so the list is the one on screen.
        fun of(app: MailcalApp, activity: MainActivity) = MessageFiling(
            targets = { account ->
                val rows = activity.accountFolders.firstOrNull { it.accountId == account }?.folders
                val showing = activity.selectedFolder.takeIf { activity.selectedAccount == account }
                messageTargets(rows.orEmpty(), showing, activity)
            },
            move = { account, key, folder ->
                val rows = listOf(SelectedRow.Message(account, key))
                app.dispatch(Intent.Folders(FolderIntent.MoveMessages(rows, account, folder)))
            },
        )
    }
}

// The line under a name field, or null where there is nothing to say. An empty field says
// nothing until something was typed: the disabled button is enough, and a warning on a dialog
// that has just opened scolds the user for not having typed yet.
internal fun nameProblem(check: FolderNameCheck, typed: String, ctx: Context): String? =
    when (check) {
        FolderNameCheck.VALID -> null
        FolderNameCheck.EMPTY -> if (typed.isEmpty()) null else L10n.folder_name_empty(ctx)
        FolderNameCheck.SURROUNDED -> L10n.folder_name_surrounded(ctx)
        FolderNameCheck.CONTROL -> L10n.folder_name_control(ctx)
        FolderNameCheck.SEPARATOR -> L10n.folder_name_separator(ctx)
        FolderNameCheck.TAKEN -> L10n.folder_name_taken(ctx)
    }

internal data class DeleteCopy(val title: String, val message: String, val confirm: String)

// Rule 26: outside Trash a delete moves the folder there; inside Trash it is for good, and the
// question says which.
internal fun deleteCopy(folder: FolderRow, ctx: Context): DeleteCopy {
    val name = folderLabel(folder.role, folder.name, ctx)
    return if (folder.inTrash) {
        DeleteCopy(
            L10n.folder_delete_permanent_title(ctx, name),
            L10n.folder_delete_permanent_message(ctx),
            L10n.action_delete_permanently(ctx),
        )
    } else {
        DeleteCopy(
            L10n.folder_delete_title(ctx, name),
            L10n.folder_delete_message(ctx),
            L10n.action_move_to_trash(ctx),
        )
    }
}

// Rule 28: what the standing notice says.
internal fun noticeText(notice: FolderNotice, ctx: Context): String =
    when (notice.problem) {
        FolderProblem.CHANGED_ELSEWHERE -> L10n.folder_notice_changed(ctx, notice.folder)
        FolderProblem.REFUSED -> L10n.folder_notice_refused(ctx, notice.folder)
    }

// A folder dialog the drawer has open.
internal sealed interface FolderDialog {
    val account: String

    data class Create(override val account: String, val parent: String?) : FolderDialog

    data class Rename(override val account: String, val folder: FolderRow) : FolderDialog

    data class Move(override val account: String, val folder: FolderRow, val targets: List<MoveTarget>) :
        FolderDialog

    data class Delete(override val account: String, val folder: FolderRow) : FolderDialog
}

// The dialog a menu item opens on a folder row.
internal fun dialogFor(
    item: FolderMenuItem,
    account: String,
    folder: FolderRow,
    rows: List<FolderRow>,
    ctx: Context,
): FolderDialog =
    when (item) {
        FolderMenuItem.NEW_FOLDER -> FolderDialog.Create(account, folder.key)
        FolderMenuItem.RENAME -> FolderDialog.Rename(account, folder)
        FolderMenuItem.MOVE -> FolderDialog.Move(account, folder, moveTargets(folder, rows, ctx))
        FolderMenuItem.DELETE -> FolderDialog.Delete(account, folder)
    }
