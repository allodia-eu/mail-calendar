// One account's page in Settings → Accounts, and its first questions: how it is shared, what it
// is used for (docs/accounts.md rule 13), which other accounts fill what it is not (rule 12), and,
// for one that signs in at its provider, signing in again (docs/provider-oauth.md rule 11). The
// servers, the mail settings and the removal follow, in that order (docs/settings.md).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.LinkSlot

/**
 * One account's page. [syncMode] is how it is shared with the other devices, null in a build with
 * no Allodia sign-in; [mail] draws its mail settings, and nothing while it has no mailbox listed.
 */
@Composable
internal fun AccountPage(
    entry: AccountEntry,
    ui: AccountsSettingsUi,
    syncMode: AllodiaAccountSyncMode?,
    onSetSyncMode: (AllodiaAccountSyncMode) -> Unit,
    mail: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    Text(
        kindLabel(ctx, entry.kind),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(modifier = Modifier.height(8.dp))
    // Whether this one travels: first, because it decides whether anything below it is anybody
    // else's business (docs/settings.md).
    if (syncMode != null) {
        AccountSyncModePicker(syncMode, onSetSyncMode)
        Spacer(modifier = Modifier.height(12.dp))
    }
    SettingsGroupCard(
        L10n.settings_account_uses_heading(ctx),
        L10n.settings_account_uses_description(ctx),
    ) {
        entry.uses.forEach { use -> UseRow(entry, use.capability, ui) }
    }
    val pickers = AccountSettingsRules.linkPickers(entry)
    if (pickers.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        SettingsGroupCard(
            L10n.settings_account_links_heading(ctx),
            L10n.settings_account_links_description(ctx),
        ) {
            pickers.forEach { picker -> LinkRow(entry, picker, ui) }
        }
    }
    val expired = entry.id in ui.expired
    if (AccountSettingsRules.signsInAtProvider(entry.kind)) {
        Spacer(modifier = Modifier.height(8.dp))
        SignInGroup(entry, expired, ui)
    }
    entry.endpoints?.let { endpoints ->
        Spacer(modifier = Modifier.height(8.dp))
        ServersGroup(entry, endpoints, expired, ui)
    }
    if (entry.id in ui.pastedSecret) {
        Spacer(modifier = Modifier.height(8.dp))
        SecretGroup(entry, ui)
    }
    mail()
    Spacer(modifier = Modifier.height(16.dp))
    RemoveAccount(entry, ui)
}

/** One use's switch, with the line that says why it cannot be flipped, when it cannot. */
@Composable
private fun UseRow(entry: AccountEntry, capability: AccountCapability, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val rule = AccountSettingsRules.switch(entry, capability)
    val pending = ui.state.pendingUse?.takeIf {
        it.accountId == entry.id && it.capability == capability
    }
    val waiting = ui.state.pendingUse != null || ui.state.signingIn == entry.id
    // Switching off deletes what the device holds of the use, so it is asked first.
    var confirmingOff by remember(entry.id, capability) { mutableStateOf(false) }
    val checked = pending?.on ?: rule.active
    // The whole row is the switch, so it is one target and one announcement.
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(
                value = checked,
                enabled = rule.enabled && !waiting,
                role = Role.Switch,
                onValueChange = { on ->
                    when {
                        !on -> confirmingOff = true
                        // Chosen, and waiting on the provider: switching it on asks again.
                        rule.asks -> ui.onSignInAgain(entry.id, listOf(capability))
                        else -> ui.onSetUse(entry.id, capability, true)
                    }
                },
            )
            .padding(vertical = 4.dp)
            .testTag("account-use-${capability.name}"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(useName(ctx, capability), style = MaterialTheme.typography.bodyLarge)
            useNoteText(ctx, rule.note)?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        Switch(checked = checked, onCheckedChange = null, enabled = rule.enabled && !waiting)
    }
    if (confirmingOff) {
        AlertDialog(
            onDismissRequest = { confirmingOff = false },
            title = { Text(L10n.account_use_off_title(ctx, useName(ctx, capability))) },
            text = { Text(L10n.account_use_off_message(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingOff = false
                        ui.onSetUse(entry.id, capability, false)
                    },
                    modifier = Modifier.testTag("account-use-off-confirm"),
                ) { Text(L10n.action_switch_off(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmingOff = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

private fun useNoteText(ctx: android.content.Context, note: UseNote): String? = when (note) {
    UseNote.LAST -> L10n.settings_account_use_last(ctx)
    UseNote.NEEDS_CONTACTS -> L10n.settings_account_colleagues_needs_contacts(ctx)
    UseNote.WITHHELD -> L10n.settings_account_use_withheld(ctx)
    UseNote.NOT_OFFERED -> L10n.settings_account_use_not_offered(ctx)
    UseNote.NONE -> null
}

/** The title of a link slot's picker. */
internal fun linkTitle(ctx: android.content.Context, slot: LinkSlot): String = when (slot) {
    LinkSlot.CALENDAR -> L10n.settings_account_link_calendar(ctx)
    LinkSlot.CONTACTS -> L10n.settings_account_link_contacts(ctx)
    LinkSlot.MAIL -> L10n.settings_account_link_mail(ctx)
}

/**
 * One link slot: a picker with "None" first, and, while nothing is linked, the suggested account
 * named under it with a button that links it. A suggestion is the person's to confirm: the picker
 * still reads None until they do (docs/accounts.md rule 12).
 */
@Composable
private fun LinkRow(entry: AccountEntry, picker: LinkPicker, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    val current = picker.selected?.let { picker.options[it].address }
        ?: L10n.settings_account_link_none(ctx)
    Text(linkTitle(ctx, picker.slot), style = MaterialTheme.typography.labelLarge)
    Box {
        TextButton(
            onClick = { expanded = true },
            modifier = Modifier.testTag("account-link-${picker.slot.name}"),
        ) {
            Text(current, maxLines = 1)
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = linkTitle(ctx, picker.slot),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(L10n.settings_account_link_none(ctx)) },
                onClick = {
                    expanded = false
                    if (picker.selected != null) ui.onSetLink(entry.id, picker.slot, null)
                },
            )
            picker.options.forEachIndexed { index, option ->
                DropdownMenuItem(
                    text = { Text(option.address) },
                    onClick = {
                        expanded = false
                        if (picker.selected != index) ui.onSetLink(entry.id, picker.slot, option.id)
                    },
                )
            }
        }
    }
    picker.suggested?.let { index ->
        val suggestion = picker.options[index]
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                L10n.settings_account_link_suggested(ctx, suggestion.address),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            TextButton(
                onClick = { ui.onSetLink(entry.id, picker.slot, suggestion.id) },
                modifier = Modifier.testTag("account-link-suggested-${picker.slot.name}"),
            ) { Text(L10n.action_link(ctx)) }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
}

/**
 * "Sign in again", for an account that signs in at its provider's page: the remedy for an expired
 * sign-in and for a permission the provider withheld.
 */
@Composable
private fun SignInGroup(entry: AccountEntry, expired: Boolean, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val description = when {
        expired -> L10n.signin_expired_prompt(ctx, entry.address)
        AccountSettingsRules.needsPermission(entry) -> L10n.settings_account_needs_permission(ctx)
        else -> L10n.settings_account_signin_description(ctx)
    }
    val signingIn = ui.state.signingIn == entry.id
    SettingsGroupCard(L10n.settings_account_signin_heading(ctx), description) {
        OutlinedButton(
            onClick = { ui.onSignInAgain(entry.id, emptyList()) },
            enabled = ui.state.signingIn == null,
            modifier = Modifier.testTag("account-sign-in-again"),
        ) {
            if (signingIn) {
                CircularProgressIndicator(modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.width(8.dp))
            }
            Text(L10n.signin_expired_action(ctx))
        }
        if (signingIn) {
            Text(
                L10n.settings_account_signin_waiting(ctx, entry.address),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Removing the account, confirmed and naming the accounts that lose their link to it. Last. */
@Composable
private fun RemoveAccount(entry: AccountEntry, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var confirming by remember(entry.id) { mutableStateOf(false) }
    Button(
        onClick = { confirming = true },
        colors = androidx.compose.material3.ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.error,
            contentColor = MaterialTheme.colorScheme.onError,
        ),
        modifier = Modifier.fillMaxWidth().testTag("account-remove"),
    ) { Text(L10n.action_remove_account(ctx)) }
    if (confirming) {
        RemoveAccountDialog(
            address = entry.address,
            unlinked = AccountSettingsRules.unlinked(entry),
            onConfirm = {
                confirming = false
                ui.onRemove(entry.id)
            },
            onDismiss = { confirming = false },
        )
    }
}

/**
 * The removal confirmation, naming the accounts that lose their link (docs/accounts.md rule 12).
 * Shared with the account switcher's own removal, so the two routes say the same thing.
 */
@Composable
internal fun RemoveAccountDialog(
    address: String,
    unlinked: String?,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val message = L10n.remove_account_message(ctx, address) +
        (unlinked?.let { " " + L10n.remove_account_unlinks(ctx, it) } ?: "")
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(L10n.remove_account_title(ctx)) },
        text = { Text(message, modifier = Modifier.testTag("account-remove-message")) },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                modifier = Modifier.testTag("account-remove-confirm"),
            ) { Text(L10n.action_remove(ctx)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_cancel(ctx)) }
        },
    )
}
