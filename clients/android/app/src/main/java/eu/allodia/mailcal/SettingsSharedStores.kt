// The Shared mailboxes and calendars section of an account's page (docs/shared-mailboxes.md rules 3
// and 10): a switch per store the account's sign-in opens besides its own, saying what it holds, the
// line under them and, on a server that lists none, a field for one's address. Switching one off is
// confirmed first, because it deletes what the device holds of it (rule 7).
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
import uniffi.mailcal_bindings.AccountSharedStore
import uniffi.mailcal_bindings.AccountSharedStores
import uniffi.mailcal_bindings.SharedStoreHolds
import uniffi.mailcal_bindings.SharedStoresNote

/** The test tag the section and its rows are found by. */
internal const val SHARED_TAG = "account-shared"

/** The section, as the page's record has it. */
@Composable
internal fun SharedStoresGroup(entry: AccountEntry, shared: AccountSharedStores, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    Spacer(modifier = Modifier.height(8.dp))
    SettingsGroupCard(L10n.settings_account_shared_heading(ctx), L10n.settings_account_shared_description(ctx)) {
        shared.stores.forEach { store -> SharedStoreRow(entry.id, store, ui) }
        shared.address?.let { AddressField(entry.id, it, ui) }
        shared.note?.let {
            Text(
                sharedNoteText(ctx, it),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp).testTag("$SHARED_TAG-note"),
            )
        }
    }
}

internal fun sharedNoteText(ctx: Context, note: SharedStoresNote): String = when (note) {
    SharedStoresNote.NONE_SHARED -> L10n.settings_account_shared_none(ctx)
    SharedStoresNote.BY_ADDRESS -> L10n.settings_account_shared_by_address(ctx)
    SharedStoresNote.NOT_CONNECTED -> L10n.settings_account_shared_not_connected(ctx)
    SharedStoresNote.UNREACHABLE -> L10n.settings_account_shared_unreachable(ctx)
}

/** What a store holds, in words. */
internal fun sharedHoldsText(ctx: Context, holds: SharedStoreHolds): String = when (holds) {
    SharedStoreHolds.MAIL -> L10n.settings_account_shared_holds_mail(ctx)
    SharedStoreHolds.CALENDAR -> L10n.settings_account_shared_holds_calendar(ctx)
    SharedStoreHolds.MAIL_AND_CALENDAR -> L10n.settings_account_shared_holds_both(ctx)
}

/** What a switch says beneath its title: that the server no longer lists it, or its label beside what it holds. */
internal fun sharedStoreNote(ctx: Context, store: AccountSharedStore): String {
    if (store.withdrawn) return L10n.settings_account_shared_withdrawn(ctx)
    val holds = sharedHoldsText(ctx, store.holds)
    return store.label?.let { "$it · $holds" } ?: holds
}

/** One store's switch: the whole row is the target, as a use's is. */
@Composable
private fun SharedStoreRow(accountId: String, store: AccountSharedStore, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var confirmingOff by remember(accountId, store.handle) { mutableStateOf(false) }
    val set = { on: Boolean -> ui.onChange(AccountPageIntent.SetSharedStore(accountId, store.handle, on)) }
    ControlSwitchRow(
        title = store.title,
        note = sharedStoreNote(ctx, store),
        on = store.on,
        control = store.control,
        tag = "$SHARED_TAG-${store.handle}",
    ) { on -> if (on) set(true) else confirmingOff = true }
    if (confirmingOff) {
        ConfirmDialog(
            title = L10n.settings_account_shared_off_title(ctx, store.title),
            message = L10n.settings_account_shared_off_message(ctx),
            action = L10n.action_switch_off(ctx),
            tag = "$SHARED_TAG-off",
        ) { yes ->
            confirmingOff = false
            if (yes) set(false)
        }
    }
}

/**
 * A field for the address of a store shared with the account, for a server that lists none,
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
            ui.onChange(AccountPageIntent.AddSharedStore(accountId, address))
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
            modifier = Modifier.weight(1f).testTag("$SHARED_TAG-address"),
        )
        Spacer(modifier = Modifier.width(8.dp))
        ControlButton(
            label = L10n.action_add(ctx),
            control = field.add,
            tag = "$SHARED_TAG-add",
            ready = typed.isNotBlank(),
        ) { add() }
    }
}
