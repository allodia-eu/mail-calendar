// The folder drawer's dialogs (name, Move to…, delete) and the standing notice a refused change
// leaves on the mailbox (docs/folder-pane.md, rules 24 to 28).
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.FolderIntent
import uniffi.mailcal_bindings.FolderNameCheck
import uniffi.mailcal_bindings.FolderNotice

// Shows whichever folder dialog is open, and closes it on an answer either way.
@Composable
internal fun FolderDialogHost(dialog: FolderDialog?, editing: FolderEditing, onClose: () -> Unit) {
    when (dialog) {
        null -> {}
        is FolderDialog.Create -> FolderNameDialog(
            title = L10n.folder_new_title(LocalContext.current),
            confirm = L10n.action_create(LocalContext.current),
            initial = "",
            check = { editing.checkName(dialog.account, dialog.parent, it, null) },
            onConfirm = { name ->
                editing.dispatch(FolderIntent.Create(dialog.account, dialog.parent, name))
                onClose()
            },
            onDismiss = onClose,
        )
        is FolderDialog.Rename -> FolderNameDialog(
            title = L10n.folder_rename_title(LocalContext.current),
            confirm = L10n.action_save(LocalContext.current),
            initial = dialog.folder.name,
            check = {
                editing.checkName(dialog.account, dialog.folder.parent, it, dialog.folder.key)
            },
            onConfirm = { name ->
                editing.dispatch(FolderIntent.Rename(dialog.account, dialog.folder.key, name))
                onClose()
            },
            onDismiss = onClose,
        )
        is FolderDialog.Move -> FolderMoveDialog(dialog, editing, onClose)
        is FolderDialog.Delete -> FolderDeleteDialog(dialog, editing, onClose)
    }
}

// Rule 25: the name is checked as it is typed, and only a `VALID` name can be confirmed.
@Composable
private fun FolderNameDialog(
    title: String,
    confirm: String,
    initial: String,
    check: (String) -> FolderNameCheck,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    var name by rememberSaveable { mutableStateOf(initial) }
    // A local store read per keystroke; the core answers from the folders it already holds.
    val result = remember(name) { check(name) }
    val problem = nameProblem(result, name, ctx)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(L10n.folder_name_label(ctx)) },
                singleLine = true,
                isError = problem != null,
                supportingText = problem?.let { { Text(it) } },
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(name) },
                // A rename to the name it already has is not a change.
                enabled = result == FolderNameCheck.VALID && name != initial,
            ) { Text(confirm) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_cancel(ctx)) }
        },
    )
}

// Rule 24: the route to moving a folder on a phone, where the drawer covers any drag.
@Composable
private fun FolderMoveDialog(dialog: FolderDialog.Move, editing: FolderEditing, onClose: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onClose,
        title = {
            Text(L10n.folder_move_title(ctx, folderLabel(dialog.folder.role, dialog.folder.name, ctx)))
        },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 360.dp)) {
                items(dialog.targets, key = { it.key ?: "" }) { target ->
                    Text(
                        text = target.label,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(role = Role.Button) {
                                editing.dispatch(
                                    FolderIntent.Move(dialog.account, dialog.folder.key, target.key),
                                )
                                onClose()
                            }
                            .padding(vertical = 12.dp),
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onClose) { Text(L10n.action_cancel(ctx)) }
        },
    )
}

// Rule 26: confirmed both ways, and worded by whether the folder is already in Trash.
@Composable
private fun FolderDeleteDialog(dialog: FolderDialog.Delete, editing: FolderEditing, onClose: () -> Unit) {
    val ctx = LocalContext.current
    val copy = deleteCopy(dialog.folder, ctx)
    AlertDialog(
        onDismissRequest = onClose,
        title = { Text(copy.title) },
        text = { Text(copy.message) },
        confirmButton = {
            TextButton(onClick = {
                editing.dispatch(FolderIntent.Delete(dialog.account, dialog.folder.key))
                onClose()
            }) { Text(copy.confirm) }
        },
        dismissButton = {
            TextButton(onClick = onClose) { Text(L10n.action_cancel(ctx)) }
        },
    )
}

// Rule 28: a refused change stands above the list until the user closes it. Not a snackbar: one
// that timed out would let a change the user asked for disappear unread.
@Composable
internal fun FolderNoticeCard(notice: FolderNotice, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    Surface(
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(start = 16.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        ) {
            Text(
                text = noticeText(notice, ctx),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
        }
    }
}
