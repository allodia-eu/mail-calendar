// The Shared mailboxes section of an account's page (docs/shared-mailboxes.md rule 3): a switch per
// mailbox the account's sign-in opens besides its own, and on Microsoft a field for one's address.
// Switching one off is confirmed first, because it deletes what the device holds of it (rule 7).
// What each row and line says is SharedMailboxRules'; this file draws it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.SharedMailboxEntry

/**
 * The section, once the server has answered, for an account whose sign-in can open a shared
 * mailbox; nothing otherwise. The server is asked as the page opens.
 */
@Composable
internal fun SharedMailboxesGroup(entry: AccountEntry, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    LaunchedEffect(entry.id) { ui.onAskSharedMailboxes(entry.id) }
    val view = ui.state.shared.answers[entry.id]
    if (view == null || !SharedMailboxRules.shows(view)) return
    Spacer(modifier = Modifier.height(8.dp))
    SettingsGroupCard(
        L10n.settings_account_shared_heading(ctx),
        L10n.settings_account_shared_description(ctx),
    ) {
        view.mailboxes.forEach { mailbox -> SharedMailboxRow(entry.id, mailbox, ui) }
        if (SharedMailboxRules.takesAddress(view)) AddressField(entry.id, ui)
        sharedNoteText(ctx, SharedMailboxRules.note(view))?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp).testTag("account-shared-note"),
            )
        }
    }
}

internal fun sharedNoteText(ctx: android.content.Context, note: SharedNote): String? = when (note) {
    SharedNote.NO_MAILBOXES -> L10n.settings_account_shared_none(ctx)
    SharedNote.BY_ADDRESS -> L10n.settings_account_shared_by_address(ctx)
    SharedNote.NOT_CONNECTED -> L10n.settings_account_shared_not_connected(ctx)
    SharedNote.UNREACHABLE -> L10n.settings_account_shared_unreachable(ctx)
    SharedNote.NONE -> null
}

/** One mailbox's switch: the whole row is the target, as a use's is. */
@Composable
private fun SharedMailboxRow(accountId: String, mailbox: SharedMailboxEntry, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val shared = ui.state.shared
    val row = SharedMailboxRules.row(mailbox)
    val pending = shared.pending?.takeIf { it.accountId == accountId && it.handle == mailbox.handle }
    val checked = pending?.on ?: mailbox.selected
    val enabled = !shared.busy
    var confirmingOff by remember(accountId, mailbox.handle) { mutableStateOf(false) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(
                value = checked,
                enabled = enabled,
                role = Role.Switch,
                onValueChange = { on ->
                    if (on) ui.onSetSharedMailbox(accountId, mailbox, true) else confirmingOff = true
                },
            )
            .padding(vertical = 4.dp)
            .testTag("account-shared-${mailbox.handle}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(row.title, style = MaterialTheme.typography.bodyLarge)
            val subtitle = if (row.withdrawn) L10n.settings_account_shared_withdrawn(ctx) else row.label
            subtitle?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        Switch(checked = checked, onCheckedChange = null, enabled = enabled)
    }
    if (confirmingOff) {
        AlertDialog(
            onDismissRequest = { confirmingOff = false },
            title = { Text(L10n.settings_account_shared_off_title(ctx, row.title)) },
            text = { Text(L10n.settings_account_shared_off_message(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingOff = false
                        ui.onSetSharedMailbox(accountId, mailbox, false)
                    },
                    modifier = Modifier.testTag("account-shared-off-confirm"),
                ) { Text(L10n.action_switch_off(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmingOff = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

/**
 * A field for the address of a mailbox shared with the account, for a server that lists none.
 * Committed with Done or Add, never per keystroke: each commit asks the server. A refusal leaves
 * the address where it was typed, to correct; an address added empties it.
 */
@Composable
private fun AddressField(accountId: String, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val shared = ui.state.shared
    var typed by remember(accountId, shared.added) { mutableStateOf("") }
    fun add() {
        val address = typed.trim()
        if (address.isNotEmpty() && !shared.busy) ui.onAddSharedMailbox(accountId, address)
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = typed,
            onValueChange = { typed = it },
            singleLine = true,
            enabled = shared.adding == null,
            label = { Text(L10n.settings_account_shared_address(ctx)) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { add() }),
            modifier = Modifier.weight(1f).testTag("account-shared-address"),
        )
        Spacer(modifier = Modifier.width(8.dp))
        TextButton(
            onClick = { add() },
            enabled = typed.isNotBlank() && !shared.busy,
            modifier = Modifier.testTag("account-shared-add"),
        ) { Text(L10n.action_add(ctx)) }
    }
}
