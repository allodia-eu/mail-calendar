// Swipe-with-undo for the message list: the state machine a completed swipe runs through, and the
// Compose effect that drives it from the Snackbar.
//
// Delete and Archive are DEFERRED: the row hides locally the moment you swipe, but no intent is
// dispatched until the Snackbar goes away. Undo therefore cancels the action outright, nothing
// ever reached the server, so there is no "un-move" to get wrong (an IMAP move mints a new UID, so
// the key we hold would be dead anyway). Star is different: it isn't destructive and the row stays
// in place, so it applies immediately and Undo un-stars.
//
// The decisions live in [SwipeUndoController], a plain class rather than a tangle of `remember`s in
// MailboxScreen, so the commit/revert/supersede rules are unit-testable without composing a screen.
package eu.allodia.mailcal

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.SwipeActionKind

// How long a committed Delete/Archive stays hidden client-side after its intent is dispatched. The
// core hides the row itself (an optimistic removal it publishes before any network round-trip), so
// this only has to outlast one snapshot hop. It matters when the core's edit is REJECTED: the core
// then restores the row, and un-hiding here lets it reappear instead of staying invisible until the
// app restarts.
internal const val COMMIT_HIDE_GRACE_MS = 4_000L

// A swipe waiting out its undo window. `id` makes every swipe distinct even when the same message
// is swiped the same way twice: without it two equal `PendingSwipe`s would not re-key the
// LaunchedEffect below, and the second swipe's Snackbar would never run.
internal data class PendingSwipe(
    val id: Long,
    val account: String,
    val key: String,
    val action: SwipeActionKind,
) {
    // Matches the `hiddenRowKeys` entries the list filters on.
    val rowKey: String get() = rowKey(account, key)

    // Star toggles a flag in place, the row does not leave the list, so hiding it would be a lie.
    val hidesRow: Boolean get() = action != SwipeActionKind.STAR
}

// The identity a hidden row is tracked under. The account is part of it because a provider key is
// unique only WITHIN an account, so the unified inbox can show two rows with the same key.
internal fun rowKey(account: String, key: String): String = "$account:$key"

/**
 * Owns what a swipe does and when. Holds Compose snapshot state, so a screen can read [pending] and
 * [hiddenRowKeys] directly, but it is a plain class with no composition of its own, the rules
 * below are the interesting part, and they are tested without a UI.
 *
 * The rules:
 * - A Delete/Archive swipe hides the row and dispatches **nothing**; [commit] dispatches, [revert]
 *   throws the action away.
 * - A Star swipe dispatches immediately (the row stays, so a delayed star would look broken);
 *   [commit] is then a no-op and [revert] un-stars.
 * - [commit]/[revert] arrive from a coroutine that may have been cancelled by a *newer* swipe, so
 *   they only clear [pending] when it is still their own, the newer swipe owns the state now.
 */
internal class SwipeUndoController(
    private val onDelete: (account: String, key: String) -> Unit,
    private val onArchive: (account: String, key: String) -> Unit,
    private val onSetFlagged: (account: String, key: String, flagged: Boolean) -> Unit,
) {
    /** The swipe currently inside its undo window, or `null`. */
    var pending by mutableStateOf<PendingSwipe?>(null)
        private set

    /** Rows hidden while their swipe is pending (or briefly after it commits). */
    var hiddenRowKeys by mutableStateOf<Set<String>>(emptySet())
        private set

    private var counter = 0L

    /** Records a completed swipe, applying Star at once and hiding the row for Delete/Archive. */
    fun onSwipe(account: String, key: String, action: SwipeActionKind) {
        counter += 1
        val swipe = PendingSwipe(counter, account, key, action)
        if (swipe.hidesRow) {
            hiddenRowKeys = hiddenRowKeys + swipe.rowKey
        } else {
            onSetFlagged(account, key, true)
        }
        pending = swipe
    }

    /** The undo window closed without an Undo: dispatch the deferred action. */
    fun commit(swipe: PendingSwipe) {
        when (swipe.action) {
            SwipeActionKind.DELETE -> onDelete(swipe.account, swipe.key)
            SwipeActionKind.ARCHIVE -> onArchive(swipe.account, swipe.key)
            // Star was applied the moment the row was swiped; committing is a no-op.
            SwipeActionKind.STAR -> Unit
        }
        clearIfCurrent(swipe)
    }

    /** The user tapped Undo. */
    fun revert(swipe: PendingSwipe) {
        when (swipe.action) {
            // Nothing was dispatched: just put the row back.
            SwipeActionKind.DELETE, SwipeActionKind.ARCHIVE -> releaseHide(swipe)
            SwipeActionKind.STAR -> onSetFlagged(swipe.account, swipe.key, false)
        }
        clearIfCurrent(swipe)
    }

    /**
     * Stops hiding `swipe`'s row. Called by [revert] straight away, and by the screen a
     * [COMMIT_HIDE_GRACE_MS] after a commit, by then the core has published a snapshot without the
     * row, so this is a no-op unless the core *rejected* the edit and restored it.
     */
    fun releaseHide(swipe: PendingSwipe) {
        hiddenRowKeys = hiddenRowKeys - swipe.rowKey
    }

    /** Whether `row` is currently hidden by a pending or just-committed swipe. */
    fun isHidden(row: SnapshotRow): Boolean =
        row is SnapshotRow.Flat && rowKey(row.row.account, row.row.key) in hiddenRowKeys

    /** Drops the visible rows a swipe is hiding. */
    fun visibleRows(rows: List<SnapshotRow>): List<SnapshotRow> =
        if (hiddenRowKeys.isEmpty()) rows else rows.filterNot(::isHidden)

    // A commit/revert can arrive from a coroutine a newer swipe already cancelled. Clearing
    // `pending` unconditionally there would wipe the newer swipe before its own Snackbar ran.
    private fun clearIfCurrent(swipe: PendingSwipe) {
        if (pending?.id == swipe.id) pending = null
    }
}

// Runs `pending`'s undo window: shows the Snackbar, then either commits the action or reverts it.
//
// Keyed on the pending swipe, so a second swipe cancels this effect, and the `finally` commits the
// first one, which is what a user swiping two messages in a row expects. Cancellation for any other
// reason (leaving the screen, opening a message) lands there too, so a swipe is never silently
// dropped. The one gap: killing the app inside the undo window loses a deferred action.
@Composable
internal fun SwipeUndoEffect(
    pending: PendingSwipe?,
    snackbarHostState: SnackbarHostState,
    onCommit: (PendingSwipe) -> Unit,
    onRevert: (PendingSwipe) -> Unit,
) {
    val ctx = LocalContext.current
    LaunchedEffect(pending) {
        if (pending == null) return@LaunchedEffect
        var undone = false
        try {
            val result = snackbarHostState.showSnackbar(
                message = swipeDoneLabel(ctx, pending.action),
                actionLabel = L10n.action_undo(ctx),
                withDismissAction = false,
                duration = SnackbarDuration.Short,
            )
            undone = result == SnackbarResult.ActionPerformed
        } finally {
            if (undone) onRevert(pending) else onCommit(pending)
        }
    }
}

// The Snackbar text for a completed swipe, past tense, because by the time it shows, the row has
// already left the list (or been starred).
internal fun swipeDoneLabel(ctx: android.content.Context, action: SwipeActionKind): String =
    when (action) {
        SwipeActionKind.DELETE -> L10n.swipe_done_delete(ctx)
        SwipeActionKind.ARCHIVE -> L10n.swipe_done_archive(ctx)
        SwipeActionKind.STAR -> L10n.swipe_done_star(ctx)
    }

// The settings label for a swipe action.
internal fun swipeActionLabel(ctx: android.content.Context, action: SwipeActionKind): String =
    when (action) {
        SwipeActionKind.DELETE -> L10n.swipe_action_delete(ctx)
        SwipeActionKind.ARCHIVE -> L10n.swipe_action_archive(ctx)
        SwipeActionKind.STAR -> L10n.swipe_action_star(ctx)
    }
