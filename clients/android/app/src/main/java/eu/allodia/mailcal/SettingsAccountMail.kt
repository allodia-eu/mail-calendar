// An account's mail settings on its page in Settings → Accounts, as the core's page record says
// them: the name it sends under, how much of its mail this device keeps, and how that mail arrives
// (docs/settings.md). Every control goes through ControlComponents.kt and asks the core for the
// change, so nothing here decides what may be pressed.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCountChoice
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountMailSettings
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSenderName

/**
 * The name this account sends under, drawn directly under the sharing control: the one thing on the
 * page a recipient can see (docs/settings.md). Where the provider holds the name the page shows it
 * and says why there is no field, rather than a disabled box that reads as a bug.
 */
@Composable
internal fun AccountSenderNameGroup(entry: AccountEntry, name: AccountSenderName, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    SettingsGroupCard(
        L10n.settings_sender_name_heading(ctx),
        L10n.settings_sender_name_description(ctx),
    ) {
        if (name.editable) {
            ControlTextField(
                title = L10n.settings_sender_name_heading(ctx),
                text = name.name,
                control = name.control,
                tag = "account-sender-name",
            ) { typed -> ui.onChange(AccountPageIntent.SetSenderName(entry.id, typed)) }
        } else {
            Text(name.name)
            Text(
                L10n.settings_sender_name_managed(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** How much of the account's mail this device keeps, and how that mail arrives. */
@Composable
internal fun AccountMailGroup(entry: AccountEntry, mail: AccountMailSettings, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val change = ui.onChange
    SettingsGroupCard(L10n.nav_mail(ctx), L10n.settings_sync_description(ctx)) {
        CountDropdown(
            title = L10n.settings_sync_depth_heading(ctx),
            note = L10n.settings_sync_depth_description(ctx),
            choice = mail.depth,
            tag = "account-sync-depth",
            label = { months ->
                if (months == 0) L10n.sync_depth_all(ctx) else L10n.sync_depth_months(ctx, months)
            },
        ) { months -> change(AccountPageIntent.SetSyncDepth(entry.id, months)) }
        Spacer(modifier = Modifier.height(12.dp))
        CountDropdown(
            title = L10n.settings_message_size_heading(ctx),
            note = L10n.settings_message_size_description(ctx),
            choice = mail.size,
            tag = "account-message-size",
            label = { megabytes ->
                if (megabytes == 0) {
                    L10n.message_size_unlimited(ctx)
                } else {
                    L10n.message_size_megabytes(ctx, megabytes)
                }
            },
        ) { megabytes -> change(AccountPageIntent.SetMessageSizeLimit(entry.id, megabytes)) }
        Spacer(modifier = Modifier.height(12.dp))
        val push = mail.push
        if (push != null) {
            ControlDropdown(
                title = L10n.settings_sync_strategy_label(ctx),
                labels = listOf(
                    L10n.settings_sync_strategy_push(ctx),
                    L10n.settings_sync_strategy_poll(ctx),
                ),
                selected = if (push.on) 0 else 1,
                control = push.control,
                tag = "account-sync-strategy",
            ) { at -> change(AccountPageIntent.SetPush(entry.id, at == 0)) }
        } else {
            Text(
                L10n.settings_sync_idle_unsupported(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        mail.interval?.let { interval ->
            Spacer(modifier = Modifier.height(8.dp))
            CountDropdown(
                title = L10n.settings_sync_interval_label(ctx),
                note = null,
                choice = interval,
                tag = "account-poll-interval",
                label = { minutes -> L10n.settings_sync_interval_minutes(ctx, minutes) },
            ) { minutes -> change(AccountPageIntent.SetPollInterval(entry.id, minutes)) }
        }
        if (push?.on == true) {
            Spacer(modifier = Modifier.height(8.dp))
            Text(L10n.settings_sync_folders_heading(ctx), style = MaterialTheme.typography.labelLarge)
            Text(
                L10n.settings_sync_folders_note(ctx, mail.watchLimit.toInt()),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            mail.folders.forEach { folder ->
                // The same word the folder pane uses (docs/folder-pane.md rule 13).
                ControlSwitchRow(
                    title = folderLabel(folder.role, folder.name, ctx),
                    note = null,
                    on = folder.on,
                    control = folder.control,
                    tag = "account-watched-${folder.key}",
                ) { on -> change(AccountPageIntent.SetWatchedFolder(entry.id, folder.key, on)) }
            }
        }
    }
}

/** A dropdown over [choice]'s counts, worded by [label], with [note] beneath its title. */
@Composable
private fun CountDropdown(
    title: String,
    note: String?,
    choice: AccountCountChoice,
    tag: String,
    label: (Int) -> String,
    onPick: (UShort) -> Unit,
) {
    ControlDropdown(
        title = title,
        labels = choice.options.map { label(it.toInt()) },
        selected = choice.selected.toInt(),
        control = choice.control,
        tag = tag,
        note = note,
    ) { at -> onPick(choice.options[at]) }
}
