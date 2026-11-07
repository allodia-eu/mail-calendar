// One account's page in Settings → Accounts, and its first questions: how it is shared, what it
// is used for (docs/accounts.md rule 13), which other accounts fill what it is not (rule 12), the
// mailboxes its sign-in opens besides its own (SettingsSharedMailboxes.kt), and how it signs in:
// again, for Microsoft and Google (docs/provider-oauth.md rule 11), or switched, for IMAP and JMAP
// (docs/accounts.md rule 17). The servers, the mail settings and the removal follow, in that order
// (docs/settings.md).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinkPicker
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountUseSwitch
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.AccountPage as CoreAccountPage

/**
 * One account's page: its uses, links and removal as the core decided them ([page]), the rest from
 * [entry]. [syncMode] is how it is shared with the other devices, null in a build with no Allodia
 * sign-in; [mail] draws its mail settings, and nothing while it has no mailbox listed.
 */
@Composable
internal fun AccountPage(
    entry: AccountEntry,
    page: CoreAccountPage,
    ui: AccountsSettingsUi,
    syncMode: AllodiaAccountSyncMode?,
    onSetSyncMode: (AllodiaAccountSyncMode) -> Unit,
    mail: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    Text(
        kindLine(ctx, entry),
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
        page.uses.forEach { switch -> UseRow(entry, switch, ui) }
    }
    ui.state.separate.form?.takeIf { it.accountId == entry.id }?.let { form ->
        SeparateServerDialog(
            form = form,
            controls = ui.state.separate.controls,
            error = ui.state.separate.error,
            onConnect = ui.onConnectSeparate,
            onDismiss = { ui.state.separate.close() },
            onSignIn = ui.onSeparateProviderSignIn,
            onShown = ui.onSeparateFormShown,
            onLeave = { ui.state.separate.left() },
        )
    }
    if (page.links.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        SettingsGroupCard(
            L10n.settings_account_links_heading(ctx),
            L10n.settings_account_links_description(ctx),
        ) {
            page.links.forEach { picker -> LinkRow(entry, picker, ui) }
        }
    }
    SharedMailboxesGroup(entry, ui)
    val expired = entry.id in ui.expired
    if (AccountSettingsRules.signsInAtProvider(entry.kind)) {
        Spacer(modifier = Modifier.height(8.dp))
        SignInGroup(entry, expired, ui)
    }
    // Where Microsoft and Google sign in again, an IMAP or JMAP account says how it signs in and
    // offers the other way (docs/accounts.md rule 17).
    if (AccountSignInRules.switchable(entry.kind)) {
        Spacer(modifier = Modifier.height(8.dp))
        AccountSignInGroup(entry, ui)
    } else if (AccountSignInRules.asksOptions(entry.kind)) {
        // A calendar and contacts account has one only at a Nextcloud server, which its server is
        // asked about as the page opens (docs/nextcloud-sign-in.md rule 7).
        LaunchedEffect(entry.id) { ui.onAskSignInOptions(entry.id) }
        if (AccountSignInRules.hasSection(entry, ui.state.signIn.options[entry.id])) {
            Spacer(modifier = Modifier.height(8.dp))
            AccountSignInGroup(entry, ui)
        }
    }
    entry.mailSync?.let { technology ->
        Spacer(modifier = Modifier.height(8.dp))
        AccountSyncTechnologyCard(technology)
    }
    entry.endpoints?.let { endpoints ->
        Spacer(modifier = Modifier.height(8.dp))
        ServersGroup(entry, endpoints, expired, ui)
    }
    mail()
    Spacer(modifier = Modifier.height(16.dp))
    RemoveAccount(entry, page.remove, ui)
}

/**
 * One use's switch, with what is wrong with it and why it cannot be pressed beneath. A use the
 * provider withholds goes to its sign-in when switched on; one that deletes something when switched
 * off asks first.
 */
@Composable
private fun UseRow(entry: AccountEntry, switch: AccountUseSwitch, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val capability = switch.capability
    // A sign-in at the provider for this account holds its switches until it answers.
    val held = ui.state.signingIn == entry.id
    val control = if (held) switch.control.copy(enabled = false) else switch.control
    var confirmingOff by remember(entry.id, capability) { mutableStateOf(false) }
    val set = { on: Boolean -> ui.onChange(AccountPageIntent.SetUse(entry.id, capability, on)) }
    ControlSwitchRow(
        title = useName(ctx, capability),
        note = switchNote(ctx, switch),
        on = switch.on,
        control = control,
        tag = "account-use-${capability.name}",
    ) { on ->
        when {
            on && switch.asksProvider -> ui.onSignInAgain(entry.id, listOf(capability))
            !on && switch.confirmsOff -> confirmingOff = true
            else -> set(on)
        }
    }
    // A calendar or address book the account's sign-in does not reach: one of its own (rule 18).
    if (UnreachedRules.offersSeparate(entry, capability)) {
        TextButton(
            onClick = { ui.onSeparateSignIn(entry.id) },
            enabled = control.enabled,
            modifier = Modifier.testTag("account-use-separate-${capability.name}"),
        ) { Text(L10n.settings_account_use_separate_sign_in(ctx)) }
    }
    // Its server was found on another site: the question that allows it
    // (docs/account-autodetect.md rule 14).
    if (switch.reviewsServerChange) {
        TextButton(
            onClick = { ui.onReviewServerChange(entry.id) },
            enabled = control.enabled,
            modifier = Modifier.testTag("account-use-review-${capability.name}"),
        ) { Text(L10n.server_changed_review(ctx)) }
    }
    if (confirmingOff) {
        ConfirmDialog(
            title = L10n.account_use_off_title(ctx, useName(ctx, capability)),
            message = L10n.account_use_off_message(ctx),
            action = L10n.action_switch_off(ctx),
            tag = "account-use-off",
        ) { yes ->
            confirmingOff = false
            if (yes) set(false)
        }
    }
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
private fun LinkRow(entry: AccountEntry, picker: AccountLinkPicker, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val link = { target: String? -> ui.onChange(AccountPageIntent.SetLink(entry.id, picker.slot, target)) }
    ControlDropdown(
        title = linkTitle(ctx, picker.slot),
        labels = listOf(L10n.settings_account_link_none(ctx)) + picker.options.map { accountName(ctx, it) },
        selected = picker.selected?.let { it.toInt() + 1 } ?: 0,
        control = picker.control,
        tag = "account-link-${picker.slot.name}",
    ) { at -> link(picker.options.getOrNull(at - 1)?.id) }
    picker.suggested?.let { picker.options.getOrNull(it.toInt()) }?.let { suggestion ->
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                L10n.settings_account_link_suggested(ctx, accountName(ctx, suggestion)),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            TextButton(
                onClick = { link(suggestion.id) },
                enabled = picker.control.enabled,
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
        expired -> L10n.signin_expired_prompt(ctx, accountName(ctx, entry))
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
private fun RemoveAccount(entry: AccountEntry, control: ControlState, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var confirming by remember(entry.id) { mutableStateOf(false) }
    ControlButton(
        label = L10n.action_remove_account(ctx),
        control = control,
        tag = "account-remove",
        modifier = Modifier.fillMaxWidth(),
        destructive = true,
    ) { confirming = true }
    if (confirming) {
        RemoveAccountDialog(accountRemoval(ctx, entry)) { yes ->
            confirming = false
            if (yes) ui.onChange(AccountPageIntent.Remove(entry.id))
        }
    }
}

/**
 * The removal confirmation, naming the accounts that lose their link (docs/accounts.md rule 12).
 * Shared with the account switcher's own removal, so the two routes say the same thing.
 */
@Composable
internal fun RemoveAccountDialog(removal: AccountRemoval, onAnswer: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    ConfirmDialog(
        title = L10n.remove_account_title(ctx),
        message = L10n.remove_account_message(ctx, removal.name) +
            (removal.unlinked?.let { " " + L10n.remove_account_unlinks(ctx, it) } ?: ""),
        action = L10n.action_remove(ctx),
        tag = "account-remove",
        onAnswer = onAnswer,
    )
}
