package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.SyncProgressSnapshot

// A transient banner above the list: a spinner while a send is in flight, then a brief
// "Message sent" / "Couldn't send" confirmation.
@androidx.compose.runtime.Composable
internal fun SendStatusBanner(status: SendStatus, ctx: Context) {
    val (text, color) = when (status) {
        SendStatus.IDLE -> return
        SendStatus.SENDING -> L10n.send_status_sending(ctx) to MaterialTheme.colorScheme.onSurfaceVariant
        SendStatus.SENT -> L10n.send_status_sent(ctx) to MaterialTheme.colorScheme.primary
        // No transient hint: the standing UnfiledCopy question already says this, and says it with a button.
        SendStatus.SENT_NOT_FILED -> return
        SendStatus.FAILED -> L10n.send_status_failed(ctx) to MaterialTheme.colorScheme.error
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (status == SendStatus.SENDING) {
            CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(text = text, style = MaterialTheme.typography.bodyMedium, color = color)
    }
}

@androidx.compose.runtime.Composable
internal fun ConnectionIssuesBanner(
    issues: List<ConnectionIssue>,
    onRetry: () -> Unit,
    ctx: Context,
) {
    if (issues.isEmpty()) {
        return
    }
    var showDetails by remember { mutableStateOf(false) }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.errorContainer)
            .padding(horizontal = 16.dp, vertical = 8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(
                painter = painterResource(R.drawable.ic_warning),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onErrorContainer,
            )
            Spacer(modifier = Modifier.width(8.dp))
            // Friendly + compact: name the affected accounts; the raw error hides behind "Details".
            Text(
                text = L10n.connectivity_accounts_affected(ctx, issues.joinToString(", ") { it.email }),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onErrorContainer,
                modifier = Modifier.weight(1f),
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = { showDetails = true }) { Text(L10n.connectivity_details(ctx)) }
            TextButton(onClick = onRetry) { Text(L10n.action_retry(ctx)) }
        }
    }
    if (showDetails) {
        AlertDialog(
            onDismissRequest = { showDetails = false },
            confirmButton = {
                TextButton(onClick = { showDetails = false }) { Text(L10n.action_close(ctx)) }
            },
            title = { Text(L10n.connectivity_not_connected(ctx)) },
            // The core already prefixes each line with its account address.
            text = { Text(issues.joinToString("\n\n") { it.detail }) },
        )
    }
}

@androidx.compose.runtime.Composable
internal fun SyncProgressBar(progress: SyncProgressSnapshot?, ctx: Context) {
    if (progress == null || !progress.active) return
    val total = progress.total
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (total != null && total > 0uL) {
            LinearProgressIndicator(
                progress = { progress.fetched.toFloat() / total.toFloat() },
                modifier = Modifier.weight(1f),
            )
        } else {
            LinearProgressIndicator(modifier = Modifier.weight(1f))
        }
        Spacer(modifier = Modifier.width(8.dp))
        val caption = if (total != null) {
            L10n.sync_downloading(ctx, syncCount(progress.fetched), syncCount(total))
        } else {
            L10n.sync_downloading_indeterminate(ctx, syncCount(progress.fetched))
        }
        Text(
            text = caption,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun syncCount(value: ULong): String = "%,d".format(value.toLong())

// The background-sync hint: which accounts are pulling mail down right now, and how far through
// their folders they are. Renders nothing whenever nothing is arriving unasked, which is almost
// always, the core admits an account only once its background pass has actually committed mail,
// so a poll that finds nothing draws nothing.
//
// A caption, never a bar: a pass the user did not start may not take a row of layout. It shares
// the strip under the list with the bar, which wins it when both are up, that is the download
// the user is waiting on.
@androidx.compose.runtime.Composable
internal fun SyncHint(progress: SyncProgressSnapshot?, accounts: List<AccountRow>, ctx: Context) {
    val caption = syncHintCaption(ctx, progress, accounts) ?: return
    Text(
        text = caption,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

// The caption itself, apart from the composable so the JVM suite can read it, the account naming
// and the folder sums are the part that can be wrong, and neither needs a renderer to prove.
internal fun syncHintCaption(
    ctx: Context,
    progress: SyncProgressSnapshot?,
    accounts: List<AccountRow>,
): String? {
    val syncing = progress?.accounts.orEmpty()
    if (syncing.isEmpty()) return null
    // Several at once carry no counts: one account in its folders and another in its bodies have
    // no shared unit to add up, and a status line cannot name them all anyway.
    if (syncing.size > 1) {
        return L10n.sync_hint_accounts(ctx, syncing.size)
    }
    val only = syncing[0]
    // Named from the app's own account list, which is where every other surface gets the address;
    // the id is a fallback for an account removed mid-pass.
    val name = accounts.firstOrNull { it.id == only.accountId }?.email ?: only.accountId
    if (only.warmingBodies) {
        return L10n.sync_hint_bodies(ctx, name, syncCount(only.bodiesDone.toULong()))
    }
    return L10n.sync_hint_account(
        ctx,
        name,
        only.foldersDone.toString(),
        only.foldersTotal.toString(),
    )
}

// Shown on the calendar when a Microsoft account's calendar is withheld for lack of the calendar
// OAuth scope (connected before calendar support, or revoked consent). Mail is unaffected, this
// is a permission prompt, not an outage, so it uses the tertiary (informational) container, not
// the error one. "Reconnect" re-runs that account's Microsoft sign-in, which upgrades its token
// in place with the calendar scope; the banner clears once the calendar connects.
@androidx.compose.runtime.Composable
internal fun CalendarReauthBanner(
    emails: List<String>,
    onReconnect: (email: String) -> Unit,
    ctx: Context,
) {
    if (emails.isEmpty()) return
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.tertiaryContainer)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_warning),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onTertiaryContainer,
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = L10n.calendar_reauth_prompt(ctx, emails.joinToString(", ")),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            modifier = Modifier.weight(1f),
        )
        Spacer(modifier = Modifier.width(8.dp))
        // One button re-auths the first affected account; when several are affected the banner
        // re-renders after each clears, walking through them one sign-in at a time.
        TextButton(onClick = { emails.firstOrNull()?.let(onReconnect) }) {
            Text(L10n.calendar_reauth_action(ctx))
        }
    }
}

// Shown on the mailbox when a Microsoft account's mail write/send is withheld for lack of the
// Mail.ReadWrite / Mail.Send OAuth scopes (connected before those scopes, or revoked consent), so a
// send or a mail action was refused with `403 ErrorAccessDenied`. Reading is unaffected, a
// permission prompt, not an outage, so it uses the tertiary (informational) container, like the
// calendar one. "Reconnect" re-runs that account's Microsoft sign-in, which re-grants the full
// scope set (clearing this and any calendar prompt); the banner clears once a send/action succeeds.
@androidx.compose.runtime.Composable
internal fun MailReauthBanner(
    emails: List<String>,
    onReconnect: (email: String) -> Unit,
    ctx: Context,
) {
    if (emails.isEmpty()) return
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.tertiaryContainer)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_warning),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onTertiaryContainer,
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = L10n.mail_reauth_prompt(ctx, emails.joinToString(", ")),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            modifier = Modifier.weight(1f),
        )
        Spacer(modifier = Modifier.width(8.dp))
        // One button re-auths the first affected account; when several are affected the banner
        // re-renders after each clears, walking through them one sign-in at a time.
        TextButton(onClick = { emails.firstOrNull()?.let(onReconnect) }) {
            Text(L10n.mail_reauth_action(ctx))
        }
    }
}

/// A banner for accounts whose stored sign-in the server has stopped accepting, an expired or
/// revoked OAuth grant, or a refused password. Distinct from [ConnectionIssuesBanner]: the server
/// answered, so "Try again" would never help; only a fresh sign-in does. An OAuth account gets a
/// button that re-runs its own flow, including a JMAP account connected by signing in, which
/// re-authorises its own persisted grant; a password or pasted-secret JMAP account is pointed at
/// Settings, since there is no browser flow to launch.
@androidx.compose.runtime.Composable
internal fun SignInExpiredBanner(
    accounts: List<ExpiredSignIn>,
    onSignIn: (account: ExpiredSignIn) -> Unit,
    ctx: Context,
) {
    val first = accounts.firstOrNull() ?: return
    val canRelaunch = first.provider == AccountProvider.MICROSOFT ||
        first.provider == AccountProvider.GOOGLE ||
        first.provider == AccountProvider.JMAP_OAUTH
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.tertiaryContainer)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_warning),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onTertiaryContainer,
        )
        Spacer(modifier = Modifier.width(8.dp))
        val names = accounts.joinToString(", ") { it.email }
        Text(
            text = if (canRelaunch) {
                L10n.signin_expired_prompt(ctx, names)
            } else {
                L10n.signin_expired_prompt_settings(ctx, names)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
            modifier = Modifier.weight(1f),
        )
        if (canRelaunch) {
            Spacer(modifier = Modifier.width(8.dp))
            // One button signs the first affected account back in; with several affected the
            // banner re-renders after each clears, walking through them one at a time.
            TextButton(onClick = { onSignIn(first) }) {
                Text(L10n.signin_expired_action(ctx))
            }
        }
    }
}

@androidx.compose.runtime.Composable
internal fun OfflineBanner(offline: Boolean, ctx: Context) {
    if (!offline) return
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.secondaryContainer)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_warning),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSecondaryContainer,
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = L10n.connectivity_offline_banner(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
        )
    }
}

@androidx.compose.runtime.Composable
internal fun AccountSwitcher(
    accounts: List<AccountRow>,
    selectedAccount: String?,
    unreachableAccounts: List<String>,
    onSelectAccount: (String?) -> Unit,
    onAddAccount: () -> Unit,
    onRemoveAccount: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    // The account a remove-confirmation dialog is open for (null = none).
    var accountToRemove by remember { mutableStateOf<AccountRow?>(null) }
    val label = selectedAccount
        ?.let { id -> accounts.firstOrNull { it.id == id }?.email ?: id }
        ?: L10n.sidebar_all_inboxes(ctx)
    Box(modifier = modifier) {
        TextButton(onClick = { expanded = true }) {
            Text(
                text = label,
                style = MaterialTheme.typography.titleLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = L10n.a11y_switch_account(ctx),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(L10n.sidebar_all_inboxes(ctx)) },
                onClick = {
                    expanded = false
                    onSelectAccount(null)
                },
            )
            accounts.forEach { account ->
                DropdownMenuItem(
                    text = { Text(account.email) },
                    onClick = {
                        expanded = false
                        onSelectAccount(account.id)
                    },
                    // A leading warning badge when this account's server couldn't be reached on
                    // its last sync (while online), a per-account outage, distinct from the
                    // device-wide offline banner.
                    leadingIcon = if (account.id in unreachableAccounts) {
                        {
                            Icon(
                                painter = painterResource(R.drawable.ic_warning),
                                contentDescription = L10n.connectivity_account_unreachable(ctx),
                                tint = MaterialTheme.colorScheme.error,
                            )
                        }
                    } else {
                        null
                    },
                    // A trailing bin removes this account (after a confirmation).
                    trailingIcon = {
                        IconButton(onClick = {
                            expanded = false
                            accountToRemove = account
                        }) {
                            Icon(
                                painter = painterResource(R.drawable.ic_delete),
                                contentDescription = L10n.action_remove_account(ctx),
                            )
                        }
                    },
                )
            }
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(L10n.action_add_account(ctx)) },
                onClick = {
                    expanded = false
                    onAddAccount()
                },
            )
        }
        accountToRemove?.let { account ->
            AlertDialog(
                onDismissRequest = { accountToRemove = null },
                title = { Text(L10n.remove_account_title(ctx)) },
                text = { Text(L10n.remove_account_message(ctx, account.email)) },
                confirmButton = {
                    TextButton(onClick = {
                        onRemoveAccount(account.id)
                        accountToRemove = null
                    }) { Text(L10n.action_remove(ctx)) }
                },
                dismissButton = {
                    TextButton(onClick = { accountToRemove = null }) {
                        Text(L10n.action_cancel(ctx))
                    }
                },
            )
        }
    }
}
