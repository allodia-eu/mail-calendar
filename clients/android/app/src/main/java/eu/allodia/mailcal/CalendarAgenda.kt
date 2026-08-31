// The agenda: the flat, soonest-first event list, and the new-event dialog.
//
// The agenda is deliberately NOT the time grid with one column, it is an unbounded list over the
// engine's own ordering, so forcing it through the grid's layout solver would buy nothing. It stays
// the list it always was; the grid (CalendarGrid.kt) is the new surface beside it.
//
// Split out of CalendarScreen.kt, which now hosts the pager.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.EventRow

// The agenda list: events soonest-first (already ordered by the engine), each row showing the title
// and the start localised in the active display zone, with swipe-to-delete.
@Composable
internal fun AgendaList(
    events: List<EventRow>,
    activeZoneId: String?,
    use24Hour: Boolean,
    onDeleteEvent: (account: String, key: String) -> Unit,
    onOpenEvent: (EventOpen) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    Column(modifier = modifier.fillMaxSize()) {
        if (events.isEmpty()) {
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = L10n.calendar_no_events(ctx),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            LazyColumn(modifier = Modifier.weight(1f)) {
                // The provider key is stable per event, tying each LazyColumn slot (and its
                // remembered swipe state) to a specific event, cf. the mailbox list.
                items(events, key = { "${it.account}:${it.key}" }) { event ->
                    if (event.canWrite) {
                        SwipeableEventRow(
                            event = event,
                            activeZoneId = activeZoneId,
                            use24Hour = use24Hour,
                            onDelete = onDeleteEvent,
                            onOpen = { onOpenEvent(EventOpen(event.account, event.key, "")) },
                        )
                    } else {
                        // A read-only provider's event gets no delete affordance at all, hidden,
                        // not disabled: a swipe that reveals a trash can and then refuses is worse
                        // than no swipe. It still opens for reading, though.
                        EventRowContent(
                            event = event,
                            activeZoneId = activeZoneId,
                            use24Hour = use24Hour,
                            onOpen = { onOpenEvent(EventOpen(event.account, event.key, "")) },
                        )
                    }
                }
            }
        }
    }
}

// Swipe an agenda row to delete the event (Intent.DeleteEvent). Mirrors the mail row: the delete
// is dispatched once the box settles away, and then we `reset()` it back to rest, the demo
// provider's edit is a no-op, so the row stays until the next snapshot and must not be left
// parked off-screen. See MailRows.kt for why `onDismiss` must be ONE remembered instance.
@Composable
private fun SwipeableEventRow(
    event: EventRow,
    activeZoneId: String?,
    use24Hour: Boolean,
    onDelete: (account: String, key: String) -> Unit,
    onOpen: () -> Unit,
) {
    val currentEvent by rememberUpdatedState(event)
    val currentOnDelete by rememberUpdatedState(onDelete)
    val scope = rememberCoroutineScope()
    val dismissState = rememberSwipeToDismissBoxState()
    val onDismiss = remember(dismissState, scope) {
        { _: SwipeToDismissBoxValue ->
            currentOnDelete(currentEvent.account, currentEvent.key)
            scope.launch { dismissState.reset() }
            Unit
        }
    }
    SwipeToDismissBox(
        state = dismissState,
        onDismiss = onDismiss,
        backgroundContent = { EventDismissBackground() },
    ) {
        EventRowContent(event = event, activeZoneId = activeZoneId, use24Hour = use24Hour, onOpen = onOpen)
    }
}

// The red Trash affordance revealed behind an event row mid-swipe (cf. the mailbox list).
@Composable
private fun EventDismissBackground() {
    Row(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.errorContainer)
            .padding(horizontal = 20.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        EventDismissIcon()
        EventDismissIcon()
    }
}

@Composable
private fun EventDismissIcon() {
    Icon(
        painter = painterResource(R.drawable.ic_delete),
        contentDescription = L10n.action_delete_event(LocalContext.current),
        tint = MaterialTheme.colorScheme.onErrorContainer,
    )
}

@Composable
private fun EventRowContent(
    event: EventRow,
    activeZoneId: String?,
    use24Hour: Boolean,
    onOpen: (() -> Unit)? = null,
) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .then(if (onOpen != null) Modifier.clickable(onClick = onOpen) else Modifier)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_calendar_month),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = event.title.ifEmpty { L10n.event_no_title(ctx) },
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            // A list row has no border to dash and no gutter to hatch, so the hold says itself in
            // words, which is the disclosure the dashes only stand in for anyway
            // (docs/calendar.md §4).
            if (isAwaitingResponse(event.participation)) {
                Text(
                    text = L10n.a11y_invitation_awaiting_response(ctx),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        // Reuse the mail helper: a `Z`-suffixed UTC instant is localised to the active zone.
        Text(
            text = localDateTime(event.start, activeZoneId, use24Hour),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
