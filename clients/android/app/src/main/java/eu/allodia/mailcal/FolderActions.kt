// What the folder drawer offers on a row, and the copy each folder dialog shows
// (docs/folder-pane.md, "Changing the tree"). Plain functions over the core's rows, so the
// rules are tested without composing a drawer.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.FolderIntent
import uniffi.mailcal_bindings.FolderNameCheck
import uniffi.mailcal_bindings.FolderNotice
import uniffi.mailcal_bindings.FolderProblem
import uniffi.mailcal_bindings.FolderRow
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp

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
// parents the way the core's `folder_paths` does. A flat list has no indent to say where a folder
// is, and two folders called `2024` would otherwise read alike.
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

// One place a folder can move to: `key` null is the top of the account's tree.
internal data class MoveTarget(val key: String?, val label: String)

// Rule 24: Top level first, then every folder of the account that takes folders, leaving out
// the folder itself and everything inside it.
internal fun moveTargets(folder: FolderRow, rows: List<FolderRow>, ctx: Context): List<MoveTarget> {
    val inside = mutableSetOf(folder.key)
    // Rows arrive depth-first, so a folder's descendants follow it and one pass finds them.
    for (row in rows) {
        if (row.parent in inside) inside.add(row.key)
    }
    val paths = folderPaths(rows, ctx)
    return listOf(MoveTarget(null, L10n.folder_move_top_level(ctx))) +
        rows.filter { it.acceptsFolders && it.key !in inside }
            .map { MoveTarget(it.key, paths.getValue(it.key)) }
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
