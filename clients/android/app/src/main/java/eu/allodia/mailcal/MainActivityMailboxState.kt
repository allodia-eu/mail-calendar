// What the mailbox list reads back out of the core: the snapshot, the window it shows, and the
// two log lines that watch it.
//
// Split from MainActivityCore.kt so each file stays under the 500-line limit. That one is about
// getting the core up and keeping it connected (boot, accounts, reachability, opening a message);
// this is about the one snapshot the list renders, which is read on every surface signal and is
// the hottest path in the app.
package eu.allodia.mailcal

import android.os.SystemClock
import android.util.Log
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.SnapshotRow

private const val TAG = "Mailcal"

// The render log is sampled rather than written on every reload: this runs on every surface
// signal, which during a sync is many a second, and a line per render buries everything else in
// the file a user attaches to a support request.
private const val RENDER_LOG_MIN_INTERVAL_MS = 5_000L
private const val RENDER_LOG_ROW_STEP = 500L

private data class RenderLogSample(
    val rowsBucket: Long,
    val totalBucket: Long,
    val mode: String,
    val accounts: Int,
    val atMs: Long,
)

private var lastRenderLog: RenderLogSample? = null

internal fun MainActivity.showMore() {
    val instance = app ?: return
    if (loadMorePending || rows.size.toULong() >= totalRows) return
    loadMorePending = true
    instance.dispatch(Intent.ShowMore)
}

internal fun MainActivity.reload() {
    val instance = app ?: return
    val snapshot = instance.mailboxList()
    rows = snapshot.rows
    totalRows = snapshot.total
    loadMorePending = false
    mode = snapshot.mode
    accounts = snapshot.accounts
    selectedAccount = snapshot.selectedAccount
    accountFolders = snapshot.accountFolders
    unifiedUnread = snapshot.unifiedUnread
    outbox = snapshot.outbox
    showingOutbox = snapshot.showingOutbox
    showingDrafts = snapshot.showingDrafts
    selectedFolder = snapshot.selected
    searchHorizon = snapshot.searchHorizon
    // Re-resolve the connection-issues emails now the switcher list is (re)populated, so an outaged
    // account seeded at boot shows its address rather than its raw id in the banner. Only when
    // there's actually an outage, a healthy reload (the common case, fired repeatedly during a
    // sync) shouldn't spend a connectivity() FFI pull each time; the CONNECTIVITY signal covers it.
    if (connectivity?.unreachableAccounts?.isNotEmpty() == true) {
        refreshConnectivity()
    }
    driveShowcaseOpenIfReady()
    logMailboxRender()
}

// Screenshot only: the two screens that open a designated message once its row has loaded.
//
// `reply` opens the message the sample reply answers, the reading screen then opens its composer
// straight away (initialComposing), pre-filled with the sample text. `invitation` opens the
// meeting invitation and stops there: the card *is* the reading screen, and the core has already
// primed the calendar, so it comes up with its day preview expanded.
//
// Fires once per launch; inert on any other screen.
private fun MainActivity.driveShowcaseOpenIfReady() {
    if (didDriveShowcaseOpen || !ShowcaseMode.isOn(this)) return
    val target = when (ShowcaseMode.screen(this)) {
        ShowcaseScreen.REPLY -> ShowcaseMode.replyTarget(this).let { it.account to it.messageKey }
        ShowcaseScreen.INVITATION ->
            ShowcaseMode.invitationTarget().let { it.account to it.messageKey }
        else -> return
    }
    val instance = app ?: return
    val present = rows.filterIsInstance<SnapshotRow.Flat>()
        .any { it.row.account == target.first && it.row.key == target.second }
    if (!present) return
    didDriveShowcaseOpen = true
    openMessageByKey(instance, target.first, target.second)
}

private fun MainActivity.logMailboxRender() {
    val message = "rendered ${rows.size} of ${totalRows} rows ($mode), ${accounts.size} accounts"
    val now = SystemClock.elapsedRealtime()
    val sample = RenderLogSample(
        rowsBucket = rows.size.toLong() / RENDER_LOG_ROW_STEP,
        totalBucket = totalRows.toLong() / RENDER_LOG_ROW_STEP,
        mode = mode.name,
        accounts = accounts.size,
        atMs = now,
    )
    val previous = lastRenderLog
    val shouldLog = previous == null ||
        previous.rowsBucket != sample.rowsBucket ||
        previous.totalBucket != sample.totalBucket ||
        previous.mode != sample.mode ||
        previous.accounts != sample.accounts ||
        now - previous.atMs >= RENDER_LOG_MIN_INTERVAL_MS
    if (shouldLog) {
        logUiInfo(message)
        lastRenderLog = sample
    } else {
        Log.d(TAG, message)
    }
}

internal fun MainActivity.updateSendStatus(status: SendStatus) {
    sendStatus = status
}
