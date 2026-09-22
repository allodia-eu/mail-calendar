// What this client says about mail arriving, and about it not arriving. Three surfaces, kept
// apart on purpose (docs/sync-progress.md):
//
//   the BAR    a download the user is waiting on, and the only one allowed a row of layout
//   the HINT   a pass nobody started, named in the strip under the list, never in a bar
//   the PAUSE  an account whose server asked to be left alone for a while
//
// Split out of MailboxScreenParts.kt to keep both files under the size limit.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.SyncProgressSnapshot

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

// What the core has to say about mail arriving, or not: an account whose server asked us to
// wait, or which accounts are pulling mail down right now and how far through their folders.
// Renders nothing whenever there is nothing to say, which is almost always.
//
// A caption, never a bar: a pass the user did not start may not take a row of layout. It shares
// the strip under the list with the bar, which wins it when both are up, that is the download
// the user is waiting on.
@androidx.compose.runtime.Composable
internal fun SyncStatus(progress: SyncProgressSnapshot?, accounts: List<AccountRow>, ctx: Context) {
    val caption = syncStatusCaption(ctx, progress, accounts) ?: return
    Text(
        text = caption,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

// The caption itself, apart from the composable so the JVM suite can read it, the account naming
// and the folder sums are the part that can be wrong, and neither needs a renderer to prove.
//
// A pause takes the line ahead of the hint: a pass that is downloading is already evident from
// the list filling, and a pass that is waiting is evident from nothing at all. The core keeps the
// two sets disjoint, so this only has to order them.
internal fun syncStatusCaption(
    ctx: Context,
    progress: SyncProgressSnapshot?,
    accounts: List<AccountRow>,
): String? = syncPauseCaption(ctx, progress, accounts) ?: syncHintCaption(ctx, progress, accounts)

// The paused notice: a server answered promptly and asked to be left alone for a while. Not an
// outage; the account keeps its mail, its credential and its badge.
//
// A phone has no hover to put a detail behind, so the line says the whole sentence. The strip is
// full width; there is room. The wait is stated as an approximation: it was rounded up before it
// got here, and nothing re-reads the clock while the line is up.
internal fun syncPauseCaption(
    ctx: Context,
    progress: SyncProgressSnapshot?,
    accounts: List<AccountRow>,
): String? {
    val paused = progress?.throttled.orEmpty()
    if (paused.isEmpty()) return null
    // Several at once are not named, exactly as with the hint: a status line cannot name them
    // all, and their waits have no shared end to state.
    if (paused.size > 1) {
        return L10n.sync_paused_detail_accounts(ctx, paused.size)
    }
    val only = paused[0]
    val name = accountName(accounts, only.accountId)
    val minutes = only.resumesInMinutes
        // Two refusals in three name no instant. "Shortly" is the honest answer; a figure here
        // would be one we made up.
        ?: return L10n.sync_paused_detail_soon(ctx, name)
    return L10n.sync_paused_detail(ctx, name, minutes.toInt())
}

// The background-sync hint: which accounts are pulling mail down right now, and how far through
// their folders they are. Null whenever nothing is arriving unasked, which is almost always, the
// core admits an account only once its background pass has actually committed mail, so a poll
// that finds nothing draws nothing.
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
    val name = accountName(accounts, only.accountId)
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

// Names an account from the app's own account list, which is where every other surface gets the
// address; the id is the fallback for one removed mid-pass.
private fun accountName(accounts: List<AccountRow>, id: String): String =
    accounts.firstOrNull { it.id == id }?.email ?: id
