// The event detail screen, what a tap on any event opens. Title, time, calendar, location, notes,
// and the reminder/recurrence summaries, with an Edit + Delete action bar at the bottom (matching the
// platform calendar the user knows). An opaque Surface overlay, composed over the grid.
//
// Delete and Edit are gated on the event's `canWrite`: a read-only calendar's event opens read-only,
// with no action bar, an affordance that can never fire is just a mystery.
package eu.allodia.mailcal

import androidx.annotation.DrawableRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.EventDetail

@Composable
internal fun EventDetailScreen(
    detail: EventDetail,
    calendars: List<CalendarRow>,
    onBack: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    /**
     * Whether the user opened one **occurrence** of a series, so a delete has a scope question to
     * put ([EventSeriesScopeDialog], raised by the caller). When it has, this screen's own generic
     * confirm is skipped: one delete should raise one dialog, and *This event · All events* already
     * carries a way out.
     */
    asksAboutTheSeries: Boolean,
) {
    val ctx = LocalContext.current
    val configuration = LocalConfiguration.current
    val locale = remember(configuration) {
        configuration.locales.takeIf { !it.isEmpty }?.get(0) ?: Locale.getDefault()
    }
    val dark = LocalAppDark.current
    var confirmingDelete by remember { mutableStateOf(false) }
    val row = calendars.firstOrNull { it.account == detail.account && it.id == detail.calendar }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.fillMaxSize()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 4.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onBack) {
                    Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = L10n.action_close(ctx))
                }
            }
            HorizontalDivider()

            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp, vertical = 16.dp),
            ) {
                Text(
                    text = detail.title.ifEmpty { L10n.event_no_title(ctx) },
                    style = MaterialTheme.typography.headlineSmall,
                )
                Spacer(Modifier.height(12.dp))
                Text(
                    text = detailTime(detail, locale),
                    style = MaterialTheme.typography.bodyLarge,
                )
                if (detail.timezone.isNotEmpty()) {
                    Text(
                        text = detail.timezone,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(Modifier.height(16.dp))

                // Calendar, with its colour dot.
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (row != null) {
                        val swatch = row.color.swatch(dark)
                        Box(
                            modifier = Modifier
                                .size(16.dp)
                                .clip(CircleShape)
                                .background(parseHexColor(swatch.background))
                                .border(1.dp, parseHexColor(swatch.border), CircleShape),
                        )
                        Spacer(Modifier.width(12.dp))
                    }
                    Text(row?.name ?: detail.calendar, style = MaterialTheme.typography.bodyLarge)
                }

                detail.location?.takeIf { it.isNotBlank() }?.let {
                    DetailRow(label = L10n.event_location(ctx), value = it)
                }
                detail.notes?.takeIf { it.isNotBlank() }?.let {
                    DetailRow(label = L10n.event_notes(ctx), value = it)
                }
                DetailRow(
                    label = L10n.event_reminder(ctx),
                    value = reminderText(ctx, detail.reminderMinutes),
                )
                DetailRow(
                    label = L10n.event_repeat(ctx),
                    value = recurrenceSummary(
                        ctx,
                        detail.repeatSummary,
                        detail.isRecurring,
                        locale,
                    ),
                )
                // No heading at all for an appointment nobody was invited to, an empty
                // "Attendees" label would read as "we looked and found none", which is a different
                // statement from "this is not a meeting".
                if (detail.attendees.isNotEmpty()) {
                    Column(modifier = Modifier.padding(top = 16.dp)) {
                        Text(
                            L10n.event_attendees(ctx),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        AttendeeList(detail.attendees)
                    }
                }
            }

            // Edit + Delete, only for a writable calendar's event.
            if (detail.canWrite) {
                HorizontalDivider()
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 8.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    ActionButton(R.drawable.ic_edit, L10n.action_edit(ctx), onEdit)
                    ActionButton(R.drawable.ic_delete, L10n.action_delete(ctx)) {
                        if (asksAboutTheSeries) onDelete() else confirmingDelete = true
                    }
                }
            }
        }
    }

    if (confirmingDelete) {
        AlertDialog(
            onDismissRequest = { confirmingDelete = false },
            title = { Text(L10n.event_delete_confirm(ctx)) },
            text = if (detail.isRecurring) {
                { Text(L10n.event_series_note(ctx)) }
            } else {
                null
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmingDelete = false
                    onDelete()
                }) { Text(L10n.action_delete(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmingDelete = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

@Composable
private fun ActionButton(
    @DrawableRes icon: Int,
    label: String,
    onClick: () -> Unit,
) {
    TextButton(onClick = onClick) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(painterResource(icon), contentDescription = null)
            Text(label, style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Composable
private fun DetailRow(label: String, value: String) {
    Column(modifier = Modifier.padding(top = 16.dp)) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}

/**
 * The event's time as one line, in its own wall clock. All-day shows the inclusive day(s); a timed
 * event shows the date and a start–end time range, collapsing the date when start and end share one.
 */
internal fun detailTime(detail: EventDetail, locale: Locale): String {
    val dateFmt = DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL).withLocale(locale)
    val timeFmt = DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT).withLocale(locale)
    val start = parseWall(detail.start)
    if (detail.allDay) {
        // The stored end is exclusive; show the inclusive last day.
        val lastDay = parseWall(detail.end).toLocalDate().minusDays(1)
        return if (lastDay == start.toLocalDate()) {
            start.toLocalDate().format(dateFmt)
        } else {
            "${start.toLocalDate().format(dateFmt)} – ${lastDay.format(dateFmt)}"
        }
    }
    val end = parseWall(detail.end)
    return if (start.toLocalDate() == end.toLocalDate()) {
        "${start.toLocalDate().format(dateFmt)}, ${start.toLocalTime().format(timeFmt)} – ${end.toLocalTime().format(timeFmt)}"
    } else {
        "${start.format(dateFmt)} ${start.toLocalTime().format(timeFmt)} – ${end.format(dateFmt)} ${end.toLocalTime().format(timeFmt)}"
    }
}
