// The event editor screen, one full-screen form for both create and edit, matching the flow of the
// platform calendar the user knows (Samsung's): title, all-day, start/end, calendar, location, notes,
// with reminder and recurrence shown but not yet editable. An opaque Surface
// overlay, like the calendar manager, because it is composed over the grid.
//
// The state and every decision live in EventEditorState; this file is the chrome that binds to it.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.SeriesEditWarning

@Composable
internal fun EventEditorScreen(
    editor: EventEditorState,
    calendars: List<CalendarRow>,
    onCancel: () -> Unit,
    onCreate: (CreateArgs) -> Unit,
    onUpdate: (UpdateArgs) -> Unit,
    /**
     * What a whole-series save of this payload would cost, the core's decision, asked with the
     * edit in hand. A lambda so the editor stays a pure form with no reach into the app.
     */
    warningFor: (UpdateArgs) -> SeriesEditWarning?,
) {
    val ctx = LocalContext.current
    val locale = currentLocale()
    var picking by remember { mutableStateOf(false) }
    val titleFocus = remember { FocusRequester() }
    // A Save waiting on the series-edit warning, holding the payload it will send if confirmed.
    // Nothing is written until the user answers, and dismissing leaves the editor open with the
    // form untouched, so the way out is never "lose what I typed".
    var confirmingSave by remember { mutableStateOf<PendingSeriesSave?>(null) }
    // A Save on an occurrence of a series, waiting for the user to say which occurrences they
    // meant. Nothing is written until they answer.
    var askingScope by remember { mutableStateOf(false) }

    // The caret opens where the work starts, the empty title on a new event, the same rule the
    // composer's To follows (docs/calendar.md, docs/contacts.md §4). Not on edit: the event already
    // has a title, and raising the keyboard over the form would hide the dates that are usually
    // what the user came to change. A `FocusRequester` cannot be asked before its node is attached,
    // so this runs as an effect rather than inline in composition.
    LaunchedEffect(editor.isEditing) {
        if (!editor.isEditing) {
            titleFocus.requestFocus()
        }
    }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Cancel, title, Save.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = onCancel) { Text(L10n.action_cancel(ctx)) }
                Text(
                    text = if (editor.isEditing) L10n.event_edit_title(ctx) else L10n.event_new_title(ctx),
                    style = MaterialTheme.typography.titleMedium,
                )
                TextButton(
                    enabled = editor.valid,
                    onClick = {
                        // Straight through on a create, and on an edit of something that can only
                        // mean the whole series. An occurrence of a series is asked about first:
                        // which occurrences, and then what saving all of them costs.
                        when {
                            !editor.isEditing -> onCreate(editor.createArgs())
                            editor.asksAboutTheSeries -> askingScope = true
                            else -> {
                                val args = editor.updateArgs(thisOccurrenceOnly = false)
                                val warning = warningFor(args)
                                if (warning == null) onUpdate(args)
                                else confirmingSave = PendingSeriesSave(args, warning)
                            }
                        }
                    },
                ) { Text(L10n.action_save(ctx)) }
            }
            HorizontalDivider()

            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(16.dp),
            ) {
                OutlinedTextField(
                    value = editor.title,
                    onValueChange = { editor.title = it },
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("event-title")
                        .focusRequester(titleFocus),
                    singleLine = true,
                    label = { Text(L10n.event_title_label(ctx)) },
                )
                Spacer(Modifier.height(12.dp))

                // All-day is set at create and frozen on edit (the patcher refuses a form change).
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(L10n.calendar_all_day(ctx), modifier = Modifier.weight(1f))
                    Switch(
                        checked = editor.allDay,
                        onCheckedChange = { editor.allDay = it },
                        enabled = editor.canEditForm,
                    )
                }
                Spacer(Modifier.height(4.dp))

                DateTimeRow(
                    label = L10n.event_start(ctx),
                    date = editor.startDate,
                    time = editor.startTime,
                    allDay = editor.allDay,
                    locale = locale,
                    onDate = { picked ->
                        // Keep the end at or after the start when the start is dragged past it.
                        if (editor.endDate.isBefore(picked)) editor.endDate = picked
                        editor.startDate = picked
                    },
                    onTime = { editor.startTime = it },
                )
                DateTimeRow(
                    label = L10n.event_end(ctx),
                    date = editor.endDate,
                    time = editor.endTime,
                    allDay = editor.allDay,
                    locale = locale,
                    onDate = { editor.endDate = it },
                    onTime = { editor.endTime = it },
                )
                Spacer(Modifier.height(8.dp))
                HorizontalDivider()

                // Calendar, a picker on create, display-only on edit (no cross-calendar move yet).
                CalendarField(
                    editor = editor,
                    calendars = calendars,
                    onPick = { picking = true },
                )
                HorizontalDivider()

                // Location: settable on create and edit alike, the engine's create draft carries it.
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = editor.location,
                    onValueChange = { editor.location = it },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    label = { Text(L10n.event_location(ctx)) },
                )

                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = editor.notes,
                    onValueChange = { editor.notes = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(L10n.event_notes(ctx)) },
                )
                Spacer(Modifier.height(8.dp))

                // Reminder: shown, not yet editable. The repeat is a set of controls when the
                // core handed over a draft, and the sentence it already decided when it did not.
                ReadOnlyRow(
                    label = L10n.event_reminder(ctx),
                    value = reminderText(ctx, editor.editing?.reminderMinutes),
                )
                if (editor.canEditRepeat) {
                    EventRepeatSection(editor = editor, start = editor.startDate, locale = locale)
                } else {
                    ReadOnlyRow(
                        label = L10n.event_repeat(ctx),
                        value = recurrenceSummary(
                            ctx,
                            editor.editing?.repeatSummary,
                            editor.editing?.isRecurring == true,
                            locale,
                        ),
                    )
                    Text(
                        text = L10n.event_repeat_locked(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
                // Only when the answer is settled. An editor opened on one occurrence asks at
                // Save which occurrences were meant, so stating the answer up here would tell
                // the user something the next dialog contradicts.
                if (editor.editing?.isRecurring == true && !editor.asksAboutTheSeries &&
                    editor.editing?.occurrence?.isEmpty() == true
                ) {
                    Text(
                        text = L10n.event_series_note(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }

                // Attendees: shown so an edit is not made blind to who is coming, and stated to be
                // read-only rather than offered as a field that would quietly drop the change:
                // editing them means sending iTIP updates, which is its own feature.
                val attendees = editor.editing?.attendees.orEmpty()
                if (attendees.isNotEmpty()) {
                    ReadOnlyLabel(L10n.event_attendees(ctx))
                    AttendeeList(attendees)
                    Text(
                        text = L10n.event_attendees_read_only(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        }
    }

    if (picking) {
        CalendarPickerSheet(
            calendars = calendars,
            selected = editor.calendar,
            onPick = { choice ->
                editor.calendar = choice
                picking = false
            },
            onDismiss = { picking = false },
        )
    }

    // Which occurrences this save meant. Asked before the warning, because the answer decides
    // whether a warning is owed at all: *This event* writes an override of its own and costs no
    // other occurrence anything. Dismissing writes nothing and leaves the form untouched.
    if (askingScope) {
        EventSeriesScopeDialog(
            title = L10n.event_series_scope_title(ctx),
            onDismiss = { askingScope = false },
            onThisEvent = {
                askingScope = false
                onUpdate(editor.updateArgs(thisOccurrenceOnly = true))
            },
            onAllEvents = {
                askingScope = false
                val args = editor.updateArgs(thisOccurrenceOnly = false)
                val warning = warningFor(args)
                if (warning == null) onUpdate(args) else confirmingSave = PendingSeriesSave(args, warning)
            },
        )
    }

    // What this save costs the occurrences the user singled out. Dismissing writes nothing.
    confirmingSave?.let { pending ->
        val text = seriesWarningText(ctx, pending.warning)
        AlertDialog(
            onDismissRequest = { confirmingSave = null },
            title = { Text(L10n.event_series_warning_title(ctx)) },
            text = text?.let { { Text(it) } },
            confirmButton = {
                TextButton(onClick = {
                    confirmingSave = null
                    onUpdate(pending.args)
                }) { Text(L10n.action_save(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmingSave = null }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

/**
 * A Save answered "all events" and waiting on the warning, holding both the payload it will send
 * and the sentence it is putting first.
 */
internal data class PendingSeriesSave(val args: UpdateArgs, val warning: SeriesEditWarning)

// The calendar row: a colour dot + the calendar's name, tappable to open the picker on create.
@Composable
private fun CalendarField(
    editor: EventEditorState,
    calendars: List<CalendarRow>,
    onPick: () -> Unit,
) {
    val ctx = LocalContext.current
    val dark = LocalAppDark.current
    val choice = editor.calendar
    val row = calendars.firstOrNull { it.account == choice?.account && it.id == choice?.id }
    val name = row?.name ?: choice?.name.orEmpty()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (editor.canEditForm) Modifier.clickable(onClick = onPick) else Modifier)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
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
        Column(modifier = Modifier.weight(1f)) {
            Text(L10n.event_calendar(ctx), style = MaterialTheme.typography.labelSmall)
            Text(
                text = name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun ReadOnlyRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.width(96.dp))
        Text(
            text = value,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// The heading over a read-only block that is a list rather than a single value (attendees), so it
// lines up with the label column of the ReadOnlyRows above it.
@Composable
private fun ReadOnlyLabel(label: String) {
    Text(
        text = label,
        style = MaterialTheme.typography.bodyMedium,
        modifier = Modifier.padding(top = 14.dp),
    )
}

// A labelled date (+ time when not all-day) row. Tapping a value opens the platform picker.
@Composable
private fun DateTimeRow(
    label: String,
    date: LocalDate,
    time: LocalTime,
    allDay: Boolean,
    locale: Locale,
    onDate: (LocalDate) -> Unit,
    onTime: (LocalTime) -> Unit,
) {
    val ctx = LocalContext.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.width(64.dp))
        TextButton(onClick = { pickDate(ctx, date, onDate) }) {
            Text(date.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale)))
        }
        if (!allDay) {
            TextButton(onClick = { pickTime(ctx, time, onTime) }) {
                Text(time.format(DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT).withLocale(locale)))
            }
        }
    }
}

@Composable
private fun currentLocale(): Locale {
    val configuration = LocalConfiguration.current
    return configuration.locales.takeIf { !it.isEmpty }?.get(0) ?: Locale.getDefault()
}

internal fun pickDate(ctx: Context, initial: LocalDate, onPick: (LocalDate) -> Unit) {
    android.app.DatePickerDialog(
        ctx,
        { _, year, month, day -> onPick(LocalDate.of(year, month + 1, day)) },
        initial.year,
        initial.monthValue - 1,
        initial.dayOfMonth,
    ).show()
}

private fun pickTime(ctx: Context, initial: LocalTime, onPick: (LocalTime) -> Unit) {
    android.app.TimePickerDialog(
        ctx,
        { _, hour, minute -> onPick(LocalTime.of(hour, minute)) },
        initial.hour,
        initial.minute,
        true,
    ).show()
}

/** The reminder summary, localised. The bucketing (pure) is [reminderBucket]. */
internal fun reminderText(ctx: Context, minutes: Int?): String = when (val b = reminderBucket(minutes)) {
    ReminderBucket.None -> L10n.event_reminder_none(ctx)
    ReminderBucket.AtStart -> L10n.event_reminder_at_start(ctx)
    is ReminderBucket.Minutes -> L10n.event_reminder_minutes(ctx, b.n)
    is ReminderBucket.Hours -> L10n.event_reminder_hours(ctx, b.n)
    is ReminderBucket.Days -> L10n.event_reminder_days(ctx, b.n)
}
