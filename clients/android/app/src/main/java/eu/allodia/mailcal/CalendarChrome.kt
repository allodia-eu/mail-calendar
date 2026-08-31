// The calendar's header: the period title, the view switcher, "back to today", and new-event.
//
// The "back to today" affordance is the one Samsung gets right and most calendars get wrong: a
// calendar glyph with *today's date number inside it*, shown only when today is not on screen. It
// tells you where you'd land and disappears once you're there, so it never sits in the bar as dead
// chrome.
package eu.allodia.mailcal

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import uniffi.mailcal_bindings.CalendarWriteStatus

/** The calendar's header row. */
@Composable
internal fun CalendarHeader(
    title: String,
    mode: CalendarMode,
    onModeChange: (CalendarMode) -> Unit,
    today: LocalDate,
    todayVisible: Boolean,
    writeStatus: CalendarWriteStatus,
    canCreateEvent: Boolean,
    onBackToToday: () -> Unit,
    onNewEvent: () -> Unit,
    onRefresh: () -> Unit,
    onManageCalendars: () -> Unit,
    onOpenCalendarSettings: () -> Unit,
) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text = title, style = MaterialTheme.typography.titleLarge)
        Row(verticalAlignment = Alignment.CenterVertically) {
            // The result of the user's last create/edit/delete: a spinner while it settles, a
            // brief check when saved, a tap-to-retry warning when it could not be confirmed. The
            // retry is a refresh, a full sync reconciles the local view (see CalendarWriteStatus).
            CalendarWriteBadge(status = writeStatus, onRetry = onRefresh)
            // Only offered when it would actually take you somewhere.
            if (!todayVisible) {
                BackToTodayButton(today = today, onClick = onBackToToday)
            }
            // Disabled, not hidden, when no calendar on any account can take an event, so the
            // header keeps its shape while a read-only setup is still browsable.
            IconButton(onClick = onNewEvent, enabled = canCreateEvent) {
                Icon(
                    painter = painterResource(R.drawable.ic_add),
                    contentDescription = L10n.action_new_event(ctx),
                )
            }
            ViewMenu(
                mode = mode,
                onModeChange = onModeChange,
                onRefresh = onRefresh,
                onManageCalendars = onManageCalendars,
                onOpenCalendarSettings = onOpenCalendarSettings,
            )
        }
    }
}

/**
 * The small write-status badge in the header. Renders the mapped [CalendarWriteIndicator]: a spinner
 * while `Saving`, a check on `Saved`, and a tap-to-retry warning when the write could not be confirmed
 * (`Failed`). Hidden when `Idle`. The warning's tap is `onRetry`, a refresh, not a re-send.
 */
@Composable
internal fun CalendarWriteBadge(status: CalendarWriteStatus, onRetry: () -> Unit) {
    val ctx = LocalContext.current
    when (CalendarWriteIndicator.of(status)) {
        CalendarWriteIndicator.Hidden -> Unit
        CalendarWriteIndicator.Spinner ->
            CircularProgressIndicator(
                strokeWidth = 2.dp,
                modifier = Modifier
                    .padding(horizontal = 12.dp)
                    .size(18.dp)
                    .semantics { contentDescription = L10n.calendar_saving(ctx) },
            )

        CalendarWriteIndicator.Saved ->
            Icon(
                painter = painterResource(R.drawable.ic_check),
                contentDescription = L10n.calendar_saved(ctx),
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(horizontal = 12.dp),
            )

        CalendarWriteIndicator.Warning ->
            IconButton(
                onClick = onRetry,
                modifier = Modifier.semantics {
                    contentDescription = L10n.calendar_save_unconfirmed(ctx)
                },
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_warning),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                )
            }
    }
}

// The Samsung affordance: a calendar-shaped outline with today's date number in it.
@Composable
private fun BackToTodayButton(today: LocalDate, onClick: () -> Unit) {
    val ctx = LocalContext.current
    val label = L10n.calendar_back_to_today(ctx)
    IconButton(
        onClick = onClick,
        modifier = Modifier.semantics { contentDescription = label },
    ) {
        Box(
            modifier = Modifier
                .size(24.dp)
                .border(
                    width = 1.5.dp,
                    color = LocalContentColor.current,
                    shape = RoundedCornerShape(6.dp),
                ),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = "${today.dayOfMonth}",
                style = MaterialTheme.typography.labelSmall,
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
                color = LocalContentColor.current,
            )
        }
    }
}

// Day / 3 days / work week / week / agenda, plus Refresh. Every grid shape is one time grid with a
// different column count, so the views are a single choice, not five features.
@Composable
private fun ViewMenu(
    mode: CalendarMode,
    onModeChange: (CalendarMode) -> Unit,
    onRefresh: () -> Unit,
    onManageCalendars: () -> Unit,
    onOpenCalendarSettings: () -> Unit,
) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.calendar_view_label(ctx),
            )
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            CalendarMode.entries.forEach { entry ->
                DropdownMenuItem(
                    text = { Text(calendarModeLabel(ctx, entry)) },
                    onClick = {
                        onModeChange(entry)
                        open = false
                    },
                    trailingIcon = {
                        if (entry == mode) {
                            Icon(painter = painterResource(R.drawable.ic_check), contentDescription = null)
                        }
                    },
                )
            }
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(L10n.calendar_manage(ctx)) },
                onClick = {
                    onManageCalendars()
                    open = false
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.calendar_open_settings(ctx)) },
                onClick = {
                    onOpenCalendarSettings()
                    open = false
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_refresh(ctx)) },
                onClick = {
                    onRefresh()
                    open = false
                },
            )
        }
    }
}

/** The menu label for a view. */
internal fun calendarModeLabel(ctx: android.content.Context, mode: CalendarMode): String =
    when (mode) {
        CalendarMode.DAY -> L10n.calendar_view_day(ctx)
        CalendarMode.THREE_DAY -> L10n.calendar_view_three_day(ctx)
        CalendarMode.WORK_WEEK -> L10n.calendar_view_work_week(ctx)
        CalendarMode.WEEK -> L10n.calendar_view_week(ctx)
        CalendarMode.MONTH -> L10n.calendar_view_month(ctx)
        CalendarMode.AGENDA -> L10n.calendar_view_agenda(ctx)
    }

/**
 * The event a tap opened, and which occurrence of it the user was looking at.
 *
 * [occurrenceStart] is the token the surface that drew the block carried, empty when there is none
 * to name, a one-off event, or an agenda row, which lists the series rather than any one of its
 * occurrences. Non-empty is what makes a delete **ask** first, so this travels with the reference
 * rather than being re-derived from the detail: the detail describes the series, and by then which
 * day the user tapped is no longer knowable.
 */
internal data class EventOpen(
    val account: String,
    val key: String,
    val occurrenceStart: String,
) {
    /** Whether a write from here has to ask *This event · All events* first. */
    val asksAboutTheSeries: Boolean get() = occurrenceStart.isNotEmpty()
}

/**
 * "This event, or all of them?", asked before a drag or a delete on a **repeating** event writes
 * anything. [title] says which act is being scoped; the two answers are the same either way.
 *
 * The core deliberately has no default here (`EventEdit.occurrence`), and neither does this: moving
 * one Tuesday standup and rewriting every Tuesday to eternity are different acts, and only the user
 * knows which they meant. Dismissing writes nothing, so the safe answer is always available without
 * choosing one of the two.
 */
@Composable
internal fun EventSeriesScopeDialog(
    title: String,
    onDismiss: () -> Unit,
    onThisEvent: () -> Unit,
    onAllEvents: () -> Unit,
) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        confirmButton = {
            TextButton(onClick = onThisEvent) { Text(L10n.event_series_scope_this(ctx)) }
        },
        dismissButton = {
            TextButton(onClick = onAllEvents) { Text(L10n.event_series_scope_all(ctx)) }
        },
    )
}
