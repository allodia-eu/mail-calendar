// The Shared mailboxes section of an account's page (docs/shared-mailboxes.md rule 3), drawn from
// the page's shared mailboxes: a switch per mailbox the account's sign-in opens besides its own,
// the line under them and, on a server that lists none, a field for one's address. Switching one
// off is confirmed first, because it deletes what the device holds of it (rule 7).
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSharedAddress
import uniffi.mailcal_bindings.AccountSharedMailbox
import uniffi.mailcal_bindings.AccountSharedMailboxes
import uniffi.mailcal_bindings.SharedMailboxesNote

/** The section, as the page's record has it. */
@Composable
internal fun SharedMailboxesGroup(entry: AccountEntry, shared: AccountSharedMailboxes, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    Spacer(modifier = Modifier.height(8.dp))
    SettingsGroupCard(
        L10n.settings_account_shared_heading(ctx),
        L10n.settings_account_shared_description(ctx),
    ) {
        shared.mailboxes.forEach { mailbox -> SharedMailboxRow(entry.id, mailbox, ui) }
        shared.address?.let { AddressField(entry.id, it, ui) }
        shared.note?.let {
            Text(
                sharedNoteText(ctx, it),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp).testTag("account-shared-note"),
            )
        }
    }
}

internal fun sharedNoteText(ctx: Context, note: SharedMailboxesNote): String = when (note) {
    SharedMailboxesNote.NONE_SHARED -> L10n.settings_account_shared_none(ctx)
    SharedMailboxesNote.BY_ADDRESS -> L10n.settings_account_shared_by_address(ctx)
    SharedMailboxesNote.NOT_CONNECTED -> L10n.settings_account_shared_not_connected(ctx)
    SharedMailboxesNote.UNREACHABLE -> L10n.settings_account_shared_unreachable(ctx)
}

/** One mailbox's switch: the whole row is the target, as a use's is. */
@Composable
private fun SharedMailboxRow(accountId: String, mailbox: AccountSharedMailbox, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var confirmingOff by remember(accountId, mailbox.handle) { mutableStateOf(false) }
    val set = { on: Boolean -> ui.onChange(AccountPageIntent.SetSharedMailbox(accountId, mailbox.handle, on)) }
    ControlSwitchRow(
        title = mailbox.title,
        note = if (mailbox.withdrawn) L10n.settings_account_shared_withdrawn(ctx) else mailbox.label,
        on = mailbox.on,
        control = mailbox.control,
        tag = "account-shared-${mailbox.handle}",
    ) { on -> if (on) set(true) else confirmingOff = true }
    if (confirmingOff) {
        ConfirmDialog(
            title = L10n.settings_account_shared_off_title(ctx, mailbox.title),
            message = L10n.settings_account_shared_off_message(ctx),
            action = L10n.action_switch_off(ctx),
            tag = "account-shared-off",
        ) { yes ->
            confirmingOff = false
            if (yes) set(false)
        }
    }
}

/**
 * A field for the address of a mailbox shared with the account, for a server that lists none,
 * drawn with the address being added or the one the last add found nothing at. Sent with Done or
 * Add, never per keystroke: each asks the server.
 */
@Composable
private fun AddressField(accountId: String, field: AccountSharedAddress, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var typed by remember(accountId, field.text) { mutableStateOf(field.text) }
    val add = {
        val address = typed.trim()
        if (address.isNotEmpty() && field.add.enabled) {
            ui.onChange(AccountPageIntent.AddSharedMailbox(accountId, address))
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = typed,
            onValueChange = { typed = it },
            singleLine = true,
            label = { Text(L10n.settings_account_shared_address(ctx)) },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { add() }),
            modifier = Modifier.weight(1f).testTag("account-shared-address"),
        )
        Spacer(modifier = Modifier.width(8.dp))
        ControlButton(
            label = L10n.action_add(ctx),
            control = field.add,
            tag = "account-shared-add",
            ready = typed.isNotBlank(),
        ) { add() }
    }
}
