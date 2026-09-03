// The mailbox screen's own state, the parts that have to outlive it.
//
// Opening a message (and Settings, and the calendar tab) REPLACES the list branch of MainScreen
// rather than covering it, so everything `remember`ed inside MailboxScreen dies with that
// composition. That cost the user their place in the list, and dropped them out of a search whose
// query the core was still applying. MainActivity holds one [MailboxUiState] instead.
//
// The decisions live here as plain classes rather than a tangle of `remember`s, so they are
// unit-testable without composing a screen.
package eu.allodia.mailcal

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.SearchScope
import uniffi.mailcal_bindings.SnapshotRow

/**
 * Everything the mailbox screen keeps between visits. Built once, by the activity, with the two
 * search dispatches the core answers ([SearchBarState] owns the rest).
 */
internal class MailboxUiState(
    onSearch: (query: String?) -> Unit,
    onSetSearchScope: (SearchScope) -> Unit,
) {
    /** Where the list is scrolled to. */
    val list = MailListPosition()

    /**
     * The search chrome. It outlives the screen for a sharper reason than the position: the core
     * clears its query on nothing but this client asking, so the query survives every screen swap,
     * and a chrome that did not would leave the list narrowed with nothing on screen saying so.
     */
    val search = SearchBarState(onSearch, onSetSearchScope)
}

// The identity of a row in the list. The account is part of it because a provider key / thread id
// is unique only WITHIN an account, so two accounts can collide on one in the unified view, and
// reusing a LazyColumn slot across them would misroute a swipe.
internal fun listKey(row: SnapshotRow): String = when (row) {
    is SnapshotRow.Flat -> "m:${row.row.account}:${row.row.key}"
    is SnapshotRow.Thread -> "t:${row.row.account}:${row.row.threadId}"
}

/**
 * The mailbox list's scroll position plus what the list should do when mail lands at its head.
 *
 * Holds Compose snapshot state, so a screen reads [pinnedToTop] and [showNewMailPill] directly,
 * but it composes nothing itself.
 */
internal class MailListPosition {
    /** Handed to the `LazyColumn`, so index and offset outlive the list's composition. */
    val listState = LazyListState()

    /**
     * Whether the list is at the very top. Recorded only when a scroll SETTLES (a user drag/fling
     * or a programmatic scroll), never on a data change: prepending a row re-anchors LazyColumn to
     * keep the old top item in view, bumping `firstVisibleItemIndex` to 1, and reading the position
     * after that would wrongly conclude the user had scrolled away. Starts true so a cold start
     * lands at the top.
     */
    var pinnedToTop by mutableStateOf(true)
        private set

    /** Whether the "jump to new mail" pill is up. */
    var showNewMailPill by mutableStateOf(false)
        private set

    // The row that was at the head last time we looked. Without it, re-entering the list re-runs
    // the arrival effect below on an unchanged head and claims mail arrived while the user was
    // reading one.
    private var headKey: String? = null

    /** Where a settled scroll left the list. */
    fun scrollSettled(index: Int, offset: Int) {
        pinnedToTop = index == 0 && offset == 0
        // Reaching the top, by the pill or by scrolling up, is what dismisses it.
        if (pinnedToTop) showNewMailPill = false
    }

    /**
     * The row now at the head of the list, on every pass. A new one means mail arrived (IMAP IDLE,
     * a sync, a new-account download).
     *
     * @return true when the list should animate to the top, because the user was already there.
     * Otherwise the pill goes up, so they can jump up without losing their place.
     */
    fun headOfList(key: String?): Boolean {
        if (key == null || key == headKey) return false
        headKey = key
        if (pinnedToTop) return true
        showNewMailPill = true
        return false
    }

    /** The pill was tapped; the scroll it starts settles into [scrollSettled] a moment later. */
    fun dismissNewMailPill() {
        showNewMailPill = false
    }
}
