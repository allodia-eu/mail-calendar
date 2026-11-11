// One account's page in Settings → Accounts, and its first questions: how it is shared, what it
// is used for (docs/accounts.md rule 13), which other accounts fill what it is not (rule 12), what
// happens to an invitation answer its calendar server cannot pass on, the mailboxes its sign-in
// opens besides its own (SettingsSharedMailboxes.kt), and how it signs in:
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
import uniffi.mailcal_bindings.AccountReplyFallback
import uniffi.mailcal_bindings.AccountUseSwitch
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.ReplyFallbackChoice
import uniffi.mailcal_bindings.AccountPage as CoreAccountPage

/**
 * One account's page: its uses, links and removal as the core decided them ([page]), the rest from
 * [entry]. [syncMode] is how it is shared with the other devices, null in a build with no Allodia
 * sign-in.
 */
@Composable
internal fun AccountPage(
    entry: AccountEntry,
    page: CoreAccountPage,
    ui: AccountsSettingsUi,
    syncMode: AllodiaAccountSyncMode?,
    onSetSyncMode: (AllodiaAccountSyncMode) -> Unit,
) {
    val ctx = LocalContext.current
    Text(
        kindLine(ctx, entry),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(modifier = Modifier.height(8.dp))
    AccountNoticesGroup(NoticeRules.about(ui.notices, entry.id), ui.noticeHandlers)
    // Whether this one travels: first, because it decides whether anything below it is anybody
    // else's business (docs/settings.md).
    if (syncMode != null) {
        AccountSyncModePicker(syncMode, onSetSyncMode)
        Spacer(modifier = Modifier.height(12.dp))
    }
    page.senderName?.let { name ->
        AccountSenderNameGroup(entry, name, ui)
        Spacer(modifier = Modifier.height(8.dp))
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
            controls = SeparateControls.of(form.section),
            error = form.section.failed?.let { refusalSaid(ctx, it) },
            onConnect = ui.onConnectSeparate,
            onDismiss = { ui.state.separate.close() },
            onSignIn = ui.onSeparateProviderSignIn,
            onCancelSignIn = { browser -> separateCancel(browser)?.let(ui.onChange) },
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
    page.replyFallback?.let { picker ->
        Spacer(modifier = Modifier.height(8.dp))
        ReplyFallbackGroup(entry, picker, ui)
    }
    page.shared?.let { SharedMailboxesGroup(entry, it, ui) }
    page.signIn?.let { section ->
        Spacer(modifier = Modifier.height(8.dp))
        AccountSignInGroup(entry, section, ui)
    }
    entry.mailSync?.let { technology ->
        Spacer(modifier = Modifier.height(8.dp))
        AccountSyncTechnologyCard(technology)
    }
    page.servers?.let { servers ->
        Spacer(modifier = Modifier.height(8.dp))
        ServersGroup(entry, servers, ui)
    }
    page.mail?.let { mail ->
        Spacer(modifier = Modifier.height(8.dp))
        AccountMailGroup(entry, mail, ui)
    }
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
    val control = switch.control
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
            on && switch.asksProvider -> ui.onSignIn(entry.id, listOf(capability))
            !on && switch.confirmsOff -> confirmingOff = true
            else -> set(on)
        }
    }
    // A calendar or address book the account's sign-in does not reach: one of its own (rule 18).
    if (switch.signsInSeparately) {
        ControlTextButton(
            L10n.settings_account_use_separate_sign_in(ctx),
            control.copy(running = false),
            "account-use-separate-${capability.name}",
        ) { ui.onSeparateSignIn(entry.id) }
    }
    // Its server was found on another site: the question that allows it
    // (docs/account-autodetect.md rule 14). Like the button above, it opens something and starts
    // nothing, so it is only held while a change runs.
    if (switch.reviewsServerChange) {
        ControlTextButton(
            L10n.server_changed_review(ctx),
            control.copy(running = false),
            "account-use-review-${capability.name}",
        ) { ui.onReviewServerChange(entry.id) }
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
            // The picker above shows the link running.
            ControlTextButton(
                L10n.action_link(ctx),
                picker.control.copy(running = false),
                "account-link-suggested-${picker.slot.name}",
            ) { link(suggestion.id) }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
}

/** The reply setting's choices, in the order its picker lists them. */
private val replyFallbackChoices = listOf(ReplyFallbackChoice.ASK, ReplyFallbackChoice.ALWAYS, ReplyFallbackChoice.NEVER)

/**
 * What happens to an invitation answer the account's calendar server cannot pass on: asked each
 * time with a notice, emailed, or not (docs/invitations.md).
 */
@Composable
private fun ReplyFallbackGroup(entry: AccountEntry, picker: AccountReplyFallback, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    SettingsGroupCard(
        L10n.settings_reply_fallback_heading(ctx),
        L10n.settings_reply_fallback_description(ctx),
    ) {
        ControlDropdown(
            title = L10n.settings_reply_fallback_label(ctx),
            labels = listOf(
                L10n.settings_reply_fallback_ask(ctx),
                L10n.settings_reply_fallback_always(ctx),
                L10n.settings_reply_fallback_never(ctx),
            ),
            selected = replyFallbackChoices.indexOf(picker.choice).coerceAtLeast(0),
            control = picker.control,
            tag = "account-reply-fallback",
        ) { at -> ui.onChange(AccountPageIntent.SetReplyFallback(entry.id, replyFallbackChoices[at])) }
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
