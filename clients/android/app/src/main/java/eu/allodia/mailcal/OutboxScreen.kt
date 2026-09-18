// The Outbox: every account's unsent messages, in one list (docs/sending.md).
//
// Its own screen rather than a mode of MailboxScreen, which is at the 500-line limit and carries
// a parameter list to match. Nothing here is a stored message: a queued send has no provider key,
// no sender to show (it is the user), no read or flagged state and no date it arrived. What it has
// instead is a reason it is still here, which no mail row carries, so it shares no row widget with
// the mailbox either.
//
// The drawer stays reachable behind it, so this is a destination inside FolderDrawerScaffold like
// any folder, and leaving is a tap on one.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.QueuedRow

/** The three things a queued send can be asked to do. */
internal enum class QueuedAction { SEND_NOW, EDIT, CANCEL }

@Composable
internal fun OutboxScreen(
    queued: List<QueuedRow>,
    accounts: List<AccountRow>,
    onOpenDrawer: () -> Unit,
    // Each action names a queued send by its account **and** its op id: an op id is unique only
    // within its own account's queue, and this list holds every account's at once.
    onAct: (account: String, op: ULong, action: QueuedAction) -> Unit,
) {
    val ctx = LocalContext.current
    val rows = remember(queued, accounts) { outboxRows(queued, accounts, ctx) }
    Column(modifier = Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onOpenDrawer) {
                Icon(
                    painter = painterResource(R.drawable.ic_menu),
                    contentDescription = L10n.a11y_open_folders(ctx),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Column(modifier = Modifier.padding(start = 8.dp)) {
                Text(L10n.folder_outbox(ctx), style = MaterialTheme.typography.titleMedium)
                // The same sentence the drawer badge speaks, so the two agree. Never a count of
                // conversations: the core builds no mail list here, so that number is zero.
                // Nothing at all when the list is empty, where the empty state below already
                // says it and "0 waiting to send" above it says it twice.
                if (rows.isNotEmpty()) {
                    Text(
                        L10n.a11y_outbox_count(ctx, rows.size),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        HorizontalDivider()
        if (rows.isEmpty()) {
            // Reachable without navigating: cancelling the last row empties the list under the
            // user, and an empty list with no words in it reads as a failure to load.
            Text(
                text = L10n.outbox_empty(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(24.dp),
            )
            return@Column
        }
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(rows, key = { "${it.account}:${it.op}" }) { row ->
                QueuedSendRow(row = row, onAct = onAct)
                HorizontalDivider()
            }
        }
    }
}

/**
 * One unsent message: who it is for, what it says, where the send has got to, and which account it
 * goes out from.
 *
 * The account is on **every** row, not only an ambiguous one (docs/folder-pane.md, rule 18): this
 * list is the one place in the app holding every account's mail at once, and which identity a
 * message is waiting on is usually the whole of why it is waiting.
 */
@Composable
private fun QueuedSendRow(
    row: OutboxRowItem,
    onAct: (account: String, op: ULong, action: QueuedAction) -> Unit,
) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                // Merged, not cleared-and-set: the row is announced once, as a list of messages
                // is not what this is, but its four texts stay in the semantics tree. Replacing
                // them with a hand-built sentence prunes them out of it, which is a row that
                // reads correctly to the eye and is unreachable to everything else.
                .semantics(mergeDescendants = true) {},
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = row.subject,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = row.recipients,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = "${row.stateText} · ${row.accountText}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        // Absent, not disabled, on a message that cannot be acted on: a menu that opens onto
        // nothing is a worse offer than no menu, and this is the Apple client's answer to the
        // same rule.
        if (row.actionable) {
            QueuedRowMenu(row = row, onAct = onAct, ctx = ctx)
        }
    }
}

@Composable
private fun QueuedRowMenu(
    row: OutboxRowItem,
    onAct: (account: String, op: ULong, action: QueuedAction) -> Unit,
    ctx: android.content.Context,
) {
    var open by remember { mutableStateOf(false) }
    Column {
        IconButton(onClick = { open = true }, modifier = Modifier.size(48.dp)) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(L10n.action_send_now(ctx)) },
                onClick = {
                    open = false
                    onAct(row.account, row.op, QueuedAction.SEND_NOW)
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_edit_queued(ctx)) },
                onClick = {
                    open = false
                    onAct(row.account, row.op, QueuedAction.EDIT)
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_cancel_send(ctx)) },
                onClick = {
                    open = false
                    onAct(row.account, row.op, QueuedAction.CANCEL)
                },
            )
        }
    }
}
