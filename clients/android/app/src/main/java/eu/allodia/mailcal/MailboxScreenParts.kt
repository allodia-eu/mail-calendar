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
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.SendStatus

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
        // Not a failure: the message is in the Outbox and goes out by itself. Saying it
        // failed would invite writing it a second time, and then both arrive. The hint is the
        // passing form of what the Outbox row says for as long as the message is waiting.
        SendStatus.QUEUED -> L10n.send_status_queued(ctx) to MaterialTheme.colorScheme.onSurfaceVariant
        // A warning, never a failure: the message may already be with its recipients, and
        // reading it as failed invites writing it again. The glyph below carries the warning.
        SendStatus.UNCONFIRMED ->
            L10n.send_status_unconfirmed(ctx) to MaterialTheme.colorScheme.onSurfaceVariant
        // Refused by the server: it stays in the Outbox until the user sends it again, edits it or
        // discards it, and the banner says where to find it.
        SendStatus.NOT_SENT -> L10n.send_status_not_sent(ctx) to MaterialTheme.colorScheme.error
        // The send never reached the Outbox; only the composer's draft keeps it.
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
        if (status == SendStatus.UNCONFIRMED) {
            Icon(
                painter = painterResource(R.drawable.ic_warning),
                contentDescription = null,
                tint = color,
                modifier = Modifier.size(16.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(text = text, style = MaterialTheme.typography.bodyMedium, color = color)
    }
}

@androidx.compose.runtime.Composable
internal fun AccountSwitcher(
    accounts: List<AccountRow>,
    selectedAccount: String?,
    marks: Map<String, AccountMark>,
    onSelectAccount: (String?) -> Unit,
    onRemoveAccount: (String) -> Unit,
    modifier: Modifier = Modifier,
    // What an account's removal names in its confirmation: the account, and those losing their link.
    removalOf: (String) -> AccountRemoval? = { null },
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
                    // The glyph of the account's first notice, its sentences for a screen reader
                    // (docs/notices.md rule 6).
                    leadingIcon = marks[account.id]?.let { mark -> { AccountMarkGlyph(mark) } },
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
        }
        accountToRemove?.let { account ->
            RemoveAccountDialog(removalOf(account.id) ?: AccountRemoval(account.email, null)) { yes ->
                accountToRemove = null
                if (yes) onRemoveAccount(account.id)
            }
        }
    }
}
