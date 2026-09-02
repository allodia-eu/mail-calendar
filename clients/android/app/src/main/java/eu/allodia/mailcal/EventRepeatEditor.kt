// The repeat controls inside the event editor: a frequency, how many periods to skip, which
// weekdays a weekly rule falls on, and what ends it.
//
// Four controls, which is less than a rule can say. The parts they do not model (a monthly series
// pinned to the month's last day, or to a weekday's position in it) ride along in the draft's
// `stored` rule and are put back by the core, so an edit that never touched the repeat cannot
// rewrite it. Which rules may be opened at all is the core's answer too: `EventDetail.repeatDraft`
// is absent for a rule it could not state in full, and then the summary is shown with no controls.
//
// The pure parts (which choice a draft is, and the sentence a stepper shows) are plain functions
// so the JVM suite drives them without composing a screen.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.util.Locale
import uniffi.mailcal_bindings.RecurrenceEnd
import uniffi.mailcal_bindings.RecurrenceFrequency
import uniffi.mailcal_bindings.RecurrenceWeekday
import uniffi.mailcal_bindings.RepeatDraft

/** What the frequency picker offers, including the choice not to repeat. */
internal enum class RepeatChoice {
    NEVER,
    DAILY,
    WEEKLY,
    MONTHLY,
    YEARLY,
    ;

    val frequency: RecurrenceFrequency?
        get() = when (this) {
            NEVER -> null
            DAILY -> RecurrenceFrequency.DAILY
            WEEKLY -> RecurrenceFrequency.WEEKLY
            MONTHLY -> RecurrenceFrequency.MONTHLY
            YEARLY -> RecurrenceFrequency.YEARLY
        }

    fun label(ctx: Context): String = when (this) {
        NEVER -> L10n.event_repeat_none(ctx)
        DAILY -> L10n.event_repeat_daily(ctx)
        WEEKLY -> L10n.event_repeat_weekly(ctx)
        MONTHLY -> L10n.event_repeat_monthly(ctx)
        YEARLY -> L10n.event_repeat_yearly(ctx)
    }

    /**
     * "Every 3 weeks": the interval stepper's own label. Never the frequency word: the picker
     * directly above already shows it, and a stepper repeating it reads as a duplicate rather than
     * as the period it sets.
     */
    fun intervalLabel(ctx: Context, interval: UInt): String {
        val count = interval.toInt()
        val many = count > 1
        return when (this) {
            NEVER -> label(ctx)
            DAILY -> if (many) L10n.event_repeat_sum_daily_n(ctx, count) else L10n.event_repeat_every_day(ctx)
            WEEKLY -> if (many) L10n.event_repeat_every_weeks(ctx, count) else L10n.event_repeat_every_week(ctx)
            MONTHLY ->
                if (many) L10n.event_repeat_every_months(ctx, count) else L10n.event_repeat_every_month(ctx)
            YEARLY -> if (many) L10n.event_repeat_every_years(ctx, count) else L10n.event_repeat_every_year(ctx)
        }
    }

    companion object {
        fun of(frequency: RecurrenceFrequency?): RepeatChoice = when (frequency) {
            null -> NEVER
            RecurrenceFrequency.DAILY -> DAILY
            RecurrenceFrequency.WEEKLY -> WEEKLY
            RecurrenceFrequency.MONTHLY -> MONTHLY
            RecurrenceFrequency.YEARLY -> YEARLY
        }
    }
}

/** What the "Ends" picker offers. */
internal enum class RepeatEndChoice {
    NEVER,
    ON_DATE,
    AFTER_COUNT,
    ;

    fun label(ctx: Context): String = when (this) {
        NEVER -> L10n.event_repeat_ends_never(ctx)
        ON_DATE -> L10n.event_repeat_ends_on_date(ctx)
        AFTER_COUNT -> L10n.event_repeat_ends_after_count(ctx)
    }

    companion object {
        fun of(end: RecurrenceEnd): RepeatEndChoice = when (end) {
            is RecurrenceEnd.Never -> NEVER
            is RecurrenceEnd.OnDate -> ON_DATE
            is RecurrenceEnd.AfterCount -> AFTER_COUNT
        }
    }
}

/**
 * The most periods, and the most instances, either stepper will go to. Well under the core's own
 * ceiling, which refuses a rule no calendar could draw.
 */
internal const val REPEAT_CEILING = 999

/**
 * The weekdays in the order this device's locale starts its week on, so the row reads the way every
 * other calendar on the phone does.
 */
internal fun localWeekOrder(locale: Locale): List<RecurrenceWeekday> {
    val week = listOf(
        RecurrenceWeekday.MONDAY,
        RecurrenceWeekday.TUESDAY,
        RecurrenceWeekday.WEDNESDAY,
        RecurrenceWeekday.THURSDAY,
        RecurrenceWeekday.FRIDAY,
        RecurrenceWeekday.SATURDAY,
        RecurrenceWeekday.SUNDAY,
    )
    val first = java.time.temporal.WeekFields.of(locale).firstDayOfWeek.value - 1
    return week.drop(first) + week.take(first)
}

/** The weekday a rule first chosen on this event should fall on. */
internal fun recurrenceWeekday(date: LocalDate): RecurrenceWeekday = when (date.dayOfWeek) {
    DayOfWeek.MONDAY -> RecurrenceWeekday.MONDAY
    DayOfWeek.TUESDAY -> RecurrenceWeekday.TUESDAY
    DayOfWeek.WEDNESDAY -> RecurrenceWeekday.WEDNESDAY
    DayOfWeek.THURSDAY -> RecurrenceWeekday.THURSDAY
    DayOfWeek.FRIDAY -> RecurrenceWeekday.FRIDAY
    DayOfWeek.SATURDAY -> RecurrenceWeekday.SATURDAY
    DayOfWeek.SUNDAY -> RecurrenceWeekday.SUNDAY
}

/** The platform's own day-of-week for one of the core's, for its locale data. */
internal fun javaDayOfWeek(day: RecurrenceWeekday): DayOfWeek = when (day) {
    RecurrenceWeekday.MONDAY -> DayOfWeek.MONDAY
    RecurrenceWeekday.TUESDAY -> DayOfWeek.TUESDAY
    RecurrenceWeekday.WEDNESDAY -> DayOfWeek.WEDNESDAY
    RecurrenceWeekday.THURSDAY -> DayOfWeek.THURSDAY
    RecurrenceWeekday.FRIDAY -> DayOfWeek.FRIDAY
    RecurrenceWeekday.SATURDAY -> DayOfWeek.SATURDAY
    RecurrenceWeekday.SUNDAY -> DayOfWeek.SUNDAY
}

/**
 * Ticks or unticks one weekday, returning the row in week order.
 *
 * At least one day stays ticked: a weekly rule that names none is not a rule, and the core would
 * refuse it, so unticking the last one is a no-op.
 */
internal fun toggledWeekdays(
    current: List<RecurrenceWeekday>,
    day: RecurrenceWeekday,
    order: List<RecurrenceWeekday>,
): List<RecurrenceWeekday> {
    val next = when {
        !current.contains(day) -> current + day
        current.size > 1 -> current - day
        else -> return current
    }
    return order.filter { next.contains(it) }
}

@Composable
internal fun EventRepeatSection(
    editor: EventEditorState,
    /** The event's start, where a rule chosen for the first time takes its weekday from. */
    start: LocalDate,
    locale: Locale,
) {
    val ctx = LocalContext.current
    val draft = editor.repeatDraft
    val choice = RepeatChoice.of(draft?.frequency)

    RepeatPickerRow(
        label = L10n.event_repeat(ctx),
        value = choice.label(ctx),
        options = RepeatChoice.entries.map { it to it.label(ctx) },
        onPick = { picked ->
            editor.repeatDraft = picked.frequency?.let { frequency ->
                draft?.copy(frequency = frequency)
                    ?: RepeatDraft(
                        frequency = frequency,
                        interval = 1u,
                        weekdays = listOf(recurrenceWeekday(start)),
                        end = RecurrenceEnd.Never,
                        stored = null,
                    )
            }
        },
    )

    if (draft != null) {
        StepperRow(
            label = choice.intervalLabel(ctx, draft.interval),
            onStep = { by ->
                val next = (draft.interval.toInt() + by).coerceIn(1, REPEAT_CEILING)
                editor.repeatDraft = draft.copy(interval = next.toUInt())
            },
        )

        if (draft.frequency == RecurrenceFrequency.WEEKLY) {
            val order = localWeekOrder(locale)
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                order.forEach { day ->
                    val name = javaDayOfWeek(day).getDisplayName(TextStyle.FULL_STANDALONE, locale)
                    FilterChip(
                        selected = draft.weekdays.contains(day),
                        onClick = {
                            editor.repeatDraft =
                                draft.copy(weekdays = toggledWeekdays(draft.weekdays, day, order))
                        },
                        label = {
                            Text(javaDayOfWeek(day).getDisplayName(TextStyle.NARROW_STANDALONE, locale))
                        },
                        modifier = Modifier.semantics { contentDescription = name },
                    )
                }
            }
        }

        RepeatPickerRow(
            label = L10n.event_repeat_ends(ctx),
            value = RepeatEndChoice.of(draft.end).label(ctx),
            options = RepeatEndChoice.entries.map { it to it.label(ctx) },
            onPick = { picked ->
                editor.repeatDraft = draft.copy(
                    end = when (picked) {
                        RepeatEndChoice.NEVER -> RecurrenceEnd.Never
                        // A year out: far enough to be a deliberate choice, near enough to reach.
                        RepeatEndChoice.ON_DATE ->
                            RecurrenceEnd.OnDate("${start.plusYears(1)}T00:00:00")
                        RepeatEndChoice.AFTER_COUNT -> RecurrenceEnd.AfterCount(10u)
                    },
                )
            },
        )

        when (val end = draft.end) {
            is RecurrenceEnd.OnDate -> {
                val on = LocalDate.parse(end.date.substringBefore('T'))
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        L10n.event_repeat_ends_date(ctx),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.width(96.dp),
                    )
                    TextButton(onClick = { pickDate(ctx, on) { picked ->
                        editor.repeatDraft = draft.copy(end = RecurrenceEnd.OnDate("${'$'}{picked}T00:00:00"))
                    } }) {
                        Text(
                            on.format(
                                DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale),
                            ),
                        )
                    }
                }
            }
            is RecurrenceEnd.AfterCount -> StepperRow(
                label = L10n.event_repeat_ends_times(ctx, end.count.toInt()),
                onStep = { by ->
                    val next = (end.count.toInt() + by).coerceIn(1, REPEAT_CEILING)
                    editor.repeatDraft = draft.copy(end = RecurrenceEnd.AfterCount(next.toUInt()))
                },
            )
            is RecurrenceEnd.Never -> Unit
        }

        if (editor.editing?.occurrence?.isNotEmpty() == true) {
            Text(
                text = L10n.event_repeat_series_note(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}

@Composable
private fun <T> RepeatPickerRow(
    label: String,
    value: String,
    options: List<Pair<T, String>>,
    onPick: (T) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.width(96.dp))
        Box {
            TextButton(onClick = { open = true }) { Text(value) }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                options.forEach { (option, text) ->
                    DropdownMenuItem(
                        text = { Text(text) },
                        onClick = {
                            onPick(option)
                            open = false
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun StepperRow(label: String, onStep: (Int) -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        IconButton(onClick = { onStep(-1) }) {
            Icon(
                painter = painterResource(R.drawable.ic_keyboard_arrow_down),
                contentDescription = L10n.action_decrease(ctx),
            )
        }
        IconButton(onClick = { onStep(1) }) {
            Icon(
                painter = painterResource(R.drawable.ic_keyboard_arrow_up),
                contentDescription = L10n.action_increase(ctx),
            )
        }
    }
}
