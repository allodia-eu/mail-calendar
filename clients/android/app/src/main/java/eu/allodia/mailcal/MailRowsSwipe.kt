// The swipe-to-act gesture on a mailbox-list flat row, split out of MailRows.kt: the commit
// threshold, the row wrapped in a SwipeToDismissBox, and the background it reveals mid-swipe.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxState
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.RecipientSuggestion
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings

// How far a row must be dragged before the swipe counts, as a fraction of the row's width.
//
// Material3's default `positionalThreshold` is a FIXED 56.dp regardless of row width, on a phone
// that is barely a seventh of the row, which is why an accidental brush used to Trash mail. A
// proportional threshold makes the gesture deliberate. (The library's velocity threshold, 125.dp/s,
// is a private constant with no knob, so a hard flick still commits; the undo Snackbar is the net.)
private const val SWIPE_COMMIT_FRACTION = 0.4f

// Swipe a flat row to run its configured action. Each direction is bound independently in Settings
// (Trash / Archive / Star); `onSwipe` parks the action in the caller's undo window rather than
// dispatching it here. Star leaves the row in the list, so once the action is parked we `reset()`
// the box back to rest, otherwise that row stays stuck off-screen at its dismissed anchor, unable
// to be swiped again.
@androidx.compose.runtime.Composable
internal fun SwipeableFlatMessageRow(
    message: FlatRow,
    activeZoneId: String?,
    inJunkFolder: Boolean,
    swipe: SwipeSettings,
    onSwipe: (account: String, key: String, action: SwipeActionKind) -> Unit,
    accounts: List<AccountRow>,
    selected: Boolean,
    selecting: Boolean,
    onToggleSelect: () -> Unit,
    onOpen: (OpenedMessage) -> Unit,
    onSetRead: (account: String, key: String, read: Boolean) -> Unit,
    onSetFlagged: (account: String, key: String, flagged: Boolean) -> Unit,
    onDelete: (account: String, key: String) -> Unit,
    onPermanentlyDelete: (account: String, key: String) -> Unit,
    onMarkAsSpam: (account: String, key: String) -> Unit,
    onMarkAsNotSpam: (account: String, key: String) -> Unit,
    onReply: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    onForward: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    replyRecipients: (account: String, key: String, replyAll: Boolean) -> RecipientSuggestion?,
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // The signature library + lookups for the reply/forward composer, or null to leave signatures
    // out (a screenshot run, a test).
    signatures: ComposerSignatures? = null,
) {
    // `onDismiss` is remembered once (see below), so reading `swipe`/`onSwipe`/`message` directly
    // would pin whatever they were at first composition. Today the Settings screen unmounts this
    // list, which hides that, but a settings change that ever lands without a remount would
    // silently keep running the OLD action. `rememberUpdatedState` keeps the closure current.
    val currentSwipe by rememberUpdatedState(swipe)
    val currentOnSwipe by rememberUpdatedState(onSwipe)
    val currentMessage by rememberUpdatedState(message)
    val scope = rememberCoroutineScope()
    // `remember`, not `rememberSwipeToDismissBoxState`, that one is a `rememberSaveable`, and a
    // LazyColumn keeps an item's saved state under its key after the item leaves the composition.
    // A row the caller hid on swipe and put back on Undo would therefore return still settled at
    // its dismissed anchor, and `SwipeToDismissBox` would park the same swipe again the moment it
    // reappeared, an Undo that undoes nothing. A row entering the list starts at rest.
    val dismissState = remember {
        SwipeToDismissBoxState(
            initialValue = SwipeToDismissBoxValue.Settled,
            positionalThreshold = { distance -> distance * SWIPE_COMMIT_FRACTION },
        )
    }
    // ONE stable lambda instance, deliberately. `SwipeToDismissBox` runs `onDismiss` from a
    // `LaunchedEffect(state.settledValue, onDismiss)`, so a fresh lambda per recomposition
    // re-keys that effect and parks the SAME swipe again for every recomposition that lands
    // while the box is still settled away, and parking the Trash action N times is not a
    // cosmetic bug. (This replaces the old `confirmValueChange` veto, which Compose 1.9+
    // invokes repeatedly during a drag rather than once at settle: that regression dispatched
    // a single swipe eight times. See MailRowSwipeTest.)
    val onDismiss = remember(dismissState, scope) {
        { value: SwipeToDismissBoxValue ->
            val action = when (value) {
                SwipeToDismissBoxValue.StartToEnd -> currentSwipe.right
                SwipeToDismissBoxValue.EndToStart -> currentSwipe.left
                SwipeToDismissBoxValue.Settled -> null
            }
            if (action != null) {
                currentOnSwipe(currentMessage.account, currentMessage.key, action)
            }
            scope.launch { dismissState.reset() }
            Unit
        }
    }
    SwipeToDismissBox(
        state = dismissState,
        onDismiss = onDismiss,
        // No swiping while rows are being selected: the drag that picks a set of messages and the
        // drag that trashes one are the same gesture, and only one of them can win.
        gesturesEnabled = !selecting,
        // The background shows the action THAT direction will run, rather than one icon standing in
        // for both. `dismissDirection` is state-backed, so reading it here re-renders as the drag
        // crosses the middle.
        backgroundContent = {
            val action = when (dismissState.dismissDirection) {
                SwipeToDismissBoxValue.StartToEnd -> currentSwipe.right
                SwipeToDismissBoxValue.EndToStart -> currentSwipe.left
                SwipeToDismissBoxValue.Settled -> null
            }
            if (action != null) SwipeBackground(action)
        },
    ) {
        FlatMessageRow(
            message = message,
            activeZoneId = activeZoneId,
            inJunkFolder = inJunkFolder,
            accounts = accounts,
            selected = selected,
            selecting = selecting,
            onToggleSelect = onToggleSelect,
            onOpen = onOpen,
            onSetRead = onSetRead,
            onSetFlagged = onSetFlagged,
            onDelete = onDelete,
            onPermanentlyDelete = onPermanentlyDelete,
            onMarkAsSpam = onMarkAsSpam,
            onMarkAsNotSpam = onMarkAsNotSpam,
            onReply = onReply,
            onForward = onForward,
            replyRecipients = replyRecipients,
            suggestionsFor = suggestionsFor,
            signatures = signatures,
        )
    }
}

// The affordance revealed behind a row mid-swipe: the icon and colour of the action that direction
// is bound to, so the user sees what will happen before they let go.
@androidx.compose.runtime.Composable
private fun SwipeBackground(action: SwipeActionKind) {
    val ctx = LocalContext.current
    val container = when (action) {
        SwipeActionKind.DELETE -> MaterialTheme.colorScheme.errorContainer
        SwipeActionKind.ARCHIVE -> MaterialTheme.colorScheme.secondaryContainer
        SwipeActionKind.STAR -> MaterialTheme.colorScheme.tertiaryContainer
    }
    val content = when (action) {
        SwipeActionKind.DELETE -> MaterialTheme.colorScheme.onErrorContainer
        SwipeActionKind.ARCHIVE -> MaterialTheme.colorScheme.onSecondaryContainer
        SwipeActionKind.STAR -> MaterialTheme.colorScheme.onTertiaryContainer
    }
    Row(
        modifier = Modifier
            .fillMaxSize()
            .background(container)
            .padding(horizontal = 20.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SwipeBackgroundIcon(action, content, swipeActionLabel(ctx, action))
        SwipeBackgroundIcon(action, content, null)
    }
}

@androidx.compose.runtime.Composable
private fun SwipeBackgroundIcon(
    action: SwipeActionKind,
    tint: Color,
    contentDescription: String?,
) {
    when (action) {
        SwipeActionKind.DELETE -> Icon(painterResource(R.drawable.ic_delete), contentDescription, tint = tint)
        SwipeActionKind.STAR -> Icon(painterResource(R.drawable.ic_star), contentDescription, tint = tint)
        SwipeActionKind.ARCHIVE -> Icon(
            painter = painterResource(R.drawable.ic_archive),
            contentDescription = contentDescription,
            tint = tint,
        )
    }
}
