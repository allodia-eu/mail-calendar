// Reusable building blocks for the unified Settings screen (see SettingsScreen.kt): the
// language override card and the per-account "New mail" card (fetch depth + push/poll behaviour +
// watched folders); the quote-style card lives in SettingsQuoteStyle.kt. Split out of SettingsScreen so each
// file stays under the 500-line limit (gradle auto-globs the package). State lives in the Rust
// core (the SyncSettingsSnapshot / persisted preferences); these only render it and dispatch
// the setters, which re-signal SETTINGS. The language override is the one exception, it is a
// host concern (AppCompat's per-app locale), applied immediately by recreating the activity.
package eu.allodia.mailcal

import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.os.LocaleListCompat
import uniffi.mailcal_bindings.AccountSyncRow
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.SyncSettingsSnapshot
import uniffi.mailcal_bindings.SyncStrategyKind

// The app-language override as tappable rows: "System default", then one row per language the
// catalog ships, each labelled with its own endonym ("Deutsch", never "German"). The rows come
// from L10n.LOCALES, so adding a language to messages/ adds it here. The choice is applied
// through AppCompatDelegate, which owns the persisted per-app locale (a catalog locale, or the
// system default) and recreates the activity so the new language takes effect right away, no
// manual restart, unlike the Windows client.
@Composable
internal fun LanguageSettingsCard() {
    val ctx = LocalContext.current
    val active = activeLanguageCode()
    Column(modifier = Modifier.fillMaxWidth()) {
        // System default clears the override; a catalog locale forces that language.
        LanguageOption(L10n.settings_language_system(ctx), selected = active == null) {
            applyLocale(LocaleListCompat.getEmptyLocaleList())
        }
        L10n.LOCALES.forEach { code ->
            LanguageOption(L10n.languageName(ctx, code), selected = active == code) {
                applyLocale(LocaleListCompat.forLanguageTags(code))
            }
        }
    }
}

// A selectable language row, styled like the sync-strategy rows so the whole screen reads as one.
@Composable
private fun LanguageOption(label: String, selected: Boolean, onSelect: () -> Unit) {
    StrategyRow(label = label, selected = selected, onSelect = onSelect)
}

// The active override as a catalog locale code, or null when no per-app locale is set (follow the
// system). A locale the app doesn't ship reads as null too, L10n falls back to the base locale in
// that case, so the ticked row always matches the language actually on screen.
private fun activeLanguageCode(): String? =
    AppCompatDelegate.getApplicationLocales()[0]?.language?.takeIf { it in L10n.LOCALES }

// Writes the per-app locale. AppCompat persists it and recreates the activity, so the new
// language takes effect right away (which also dismisses the Settings screen).
private fun applyLocale(locales: LocaleListCompat) {
    AppCompatDelegate.setApplicationLocales(locales)
}

/**
 * The three positions an account can be shared in, and what each one means.
 *
 * A single-choice segmented row rather than a switch and a button: the two questions underneath:
 * is this account on my other devices, and does this device exchange changes about it, are not
 * independent in any way somebody can act on, and splitting them produced a screen where turning
 * the switch off changed nothing the person could see. Its Apple, Windows and Linux twins use each
 * platform's own equivalent.
 *
 * The subtext is the selected position's, because three subtexts at once is a paragraph nobody
 * reads and the one that matters is the one in force.
 */
@Composable
internal fun AccountSyncModePicker(
    mode: AllodiaAccountSyncMode,
    onSelect: (AllodiaAccountSyncMode) -> Unit,
) {
    val ctx = LocalContext.current
    val options = listOf(
        AllodiaAccountSyncMode.ON to L10n.settings_account_sync_on(ctx),
        AllodiaAccountSyncMode.PAUSED to L10n.settings_account_sync_paused(ctx),
        AllodiaAccountSyncMode.OFF to L10n.settings_account_sync_off(ctx),
    )
    Text(
        L10n.settings_account_sync_heading(ctx),
        style = MaterialTheme.typography.bodyLarge,
    )
    Spacer(modifier = Modifier.height(4.dp))
    SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
        options.forEachIndexed { index, (option, label) ->
            SegmentedButton(
                selected = mode == option,
                onClick = { if (mode != option) onSelect(option) },
                shape = SegmentedButtonDefaults.itemShape(index = index, count = options.size),
            ) { Text(label) }
        }
    }
    Spacer(modifier = Modifier.height(4.dp))
    Text(
        when (mode) {
            AllodiaAccountSyncMode.ON -> L10n.settings_account_sync_on_hint(ctx)
            AllodiaAccountSyncMode.PAUSED -> L10n.settings_account_sync_paused_hint(ctx)
            AllodiaAccountSyncMode.OFF -> L10n.settings_account_sync_off_hint(ctx)
        },
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

// One account's mail settings: how far back to download (fetch depth), whether to receive mail as
// it arrives (IMAP IDLE push, shown only when the server supports it) or on a schedule, and which
// folders to watch for push.
@Composable
internal fun AccountSyncCard(
    account: AccountSyncRow,
    settings: SyncSettingsSnapshot,
    onSetSyncDepth: (account: String, months: UShort) -> Unit,
    onSetMessageSize: (account: String, megabytes: UShort) -> Unit,
    onSetStrategy: (account: String, strategy: SyncStrategyKind) -> Unit,
    onSetPollInterval: (account: String, minutes: UShort) -> Unit,
    onSetPushFolder: (account: String, folder: String, subscribed: Boolean) -> Unit,
    // How this account is shared with the person's other devices, and how to change it. Null when
    // this build carries no Allodia sign-in, and the block is then absent rather than dead.
    syncMode: AllodiaAccountSyncMode? = null,
    onSetSyncMode: (account: String, mode: AllodiaAccountSyncMode) -> Unit = { _, _ -> },
) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            account.email,
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(modifier = Modifier.height(8.dp))
        // How this one is shared. First in the card, because it decides whether anything below
        // it is anybody else's business.
        if (syncMode != null) {
            AccountSyncModePicker(syncMode) { mode -> onSetSyncMode(account.accountId, mode) }
            Spacer(modifier = Modifier.height(12.dp))
        }
        // Fetch depth, how far back this account downloads mail (per-account).
        FetchDepthPicker(account, settings.syncDepths) { months ->
            onSetSyncDepth(account.accountId, months)
        }
        Spacer(modifier = Modifier.height(12.dp))
        // Message size, the largest message kept offline (per-account).
        MessageSizePicker(account, settings.messageSizeLimitsMb) { megabytes ->
            onSetMessageSize(account.accountId, megabytes)
        }
        Spacer(modifier = Modifier.height(12.dp))
        // The strategy choice. Push is offered only when the server advertises IDLE;
        // otherwise we explain it and show the polling intervals only.
        if (account.idleSupported) {
            StrategyRow(
                label = L10n.settings_sync_strategy_push(ctx),
                selected = account.strategy == SyncStrategyKind.PUSH,
            ) { onSetStrategy(account.accountId, SyncStrategyKind.PUSH) }
            StrategyRow(
                label = L10n.settings_sync_strategy_poll(ctx),
                selected = account.strategy == SyncStrategyKind.POLL,
            ) { onSetStrategy(account.accountId, SyncStrategyKind.POLL) }
        } else {
            Text(
                L10n.settings_sync_idle_unsupported(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.height(8.dp))
        when (account.strategy) {
            SyncStrategyKind.PUSH ->
                PushFolders(account, settings.maxPushFolders.toInt(), onSetPushFolder)
            SyncStrategyKind.POLL -> PollIntervals(account, settings, onSetPollInterval)
        }
    }
}

// The per-account fetch-depth dropdown, built from the shared depth set so it never hardcodes.
// "All time" is the 0-month sentinel; a month count otherwise.
@Composable
private fun FetchDepthPicker(
    account: AccountSyncRow,
    depths: List<UShort>,
    onSelect: (UShort) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    fun label(months: UShort): String =
        if (months == 0.toUShort()) L10n.sync_depth_all(ctx) else L10n.sync_depth_months(ctx, months.toInt())
    Text(L10n.settings_sync_depth_heading(ctx), style = MaterialTheme.typography.labelLarge)
    Text(
        L10n.settings_sync_depth_description(ctx),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(label(account.syncDepthMonths), maxLines = 1)
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = L10n.settings_sync_depth_label(ctx),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            depths.forEach { months ->
                DropdownMenuItem(
                    text = { Text(label(months)) },
                    onClick = {
                        expanded = false
                        onSelect(months)
                    },
                )
            }
        }
    }
}

// The per-account message-size dropdown, built from the shared option set so it never hardcodes.
// "Any size" is the 0-megabyte sentinel; a megabyte count otherwise.
@Composable
private fun MessageSizePicker(
    account: AccountSyncRow,
    limits: List<UShort>,
    onSelect: (UShort) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    fun label(megabytes: UShort): String =
        if (megabytes == 0.toUShort()) {
            L10n.message_size_unlimited(ctx)
        } else {
            L10n.message_size_megabytes(ctx, megabytes.toInt())
        }
    Text(L10n.settings_message_size_heading(ctx), style = MaterialTheme.typography.labelLarge)
    Text(
        L10n.settings_message_size_description(ctx),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(label(account.messageSizeLimitMb), maxLines = 1)
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = L10n.settings_message_size_label(ctx),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            limits.forEach { megabytes ->
                DropdownMenuItem(
                    text = { Text(label(megabytes)) },
                    onClick = {
                        expanded = false
                        onSelect(megabytes)
                    },
                )
            }
        }
    }
}

@Composable
internal fun StrategyRow(label: String, selected: Boolean, onSelect: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onSelect).padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = onSelect)
        Text(label, style = MaterialTheme.typography.bodyLarge)
    }
}

// The poll-interval radio list, built from the shared interval set so it never hardcodes.
@Composable
private fun PollIntervals(
    account: AccountSyncRow,
    settings: SyncSettingsSnapshot,
    onSetPollInterval: (account: String, minutes: UShort) -> Unit,
) {
    val ctx = LocalContext.current
    Text(L10n.settings_sync_interval_label(ctx), style = MaterialTheme.typography.labelLarge)
    settings.pollIntervals.forEach { minutes ->
        StrategyRow(
            label = L10n.settings_sync_interval_minutes(ctx, minutes.toInt()),
            selected = account.pollIntervalMins == minutes,
        ) { onSetPollInterval(account.accountId, minutes) }
    }
}

// The push-folder checklist. Unchecked folders are disabled once the account is at the cap.
@Composable
private fun PushFolders(
    account: AccountSyncRow,
    maxFolders: Int,
    onSetPushFolder: (account: String, folder: String, subscribed: Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    Text(L10n.settings_sync_folders_heading(ctx), style = MaterialTheme.typography.labelLarge)
    Text(
        L10n.settings_sync_folders_note(ctx, maxFolders),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    account.folders.forEach { folder ->
        val enabled = folder.subscribed || !account.atPushLimit
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = enabled) {
                    onSetPushFolder(account.accountId, folder.key, !folder.subscribed)
                }
                .padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Checkbox(
                checked = folder.subscribed,
                onCheckedChange = { checked -> onSetPushFolder(account.accountId, folder.key, checked) },
                enabled = enabled,
            )
            Text(
                folderLabel(folder.role, folder.name, LocalContext.current),
                style = MaterialTheme.typography.bodyLarge,
            )
        }
    }
}
