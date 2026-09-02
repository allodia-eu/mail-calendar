// The event editor's state and the payloads it produces, a plain class, deliberately, so the whole
// of the create/edit logic (validation, the all-day inclusive↔exclusive conversion, which fields are
// frozen on edit, the wall-clock-vs-UTC create form) is testable on the JVM without composing a
// screen (AGENTS.md).
//
// The one rule that is load-bearing and easy to get wrong: **times are the event's own wall clock.**
// On CREATE that is the device's zone (so a created event reads back the same clock on edit, see
// build_event_draft's `timezone`). On EDIT it is the event's own zone, which the detail read already
// gave us. The editor never converts between zones; it edits a wall clock and states which zone it is
// in, and the core keeps the event in that zone.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import uniffi.mailcal_bindings.EventAttendee
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.EventRecurrence
import uniffi.mailcal_bindings.RecurrenceChange
import uniffi.mailcal_bindings.RepeatDraft
import uniffi.mailcal_bindings.RepeatSummary
import uniffi.mailcal_bindings.SimpleRecurrence

/**
 * What a save should send for the repeat rule, decided by the core: see `mailcal_account`'s
 * `recurrence_change_of`. Taken as a function so [EventEditorState] stays a plain class the JVM
 * suite can drive: nothing in `app/src/test` loads the cdylib, and a direct call would need it.
 */
internal typealias RepeatChangeOf = (RepeatDraft?, Boolean) -> RecurrenceChange?

/** The real one. The default everywhere but a test. */
internal val coreRepeatChangeOf: RepeatChangeOf =
    { draft, wasRepeating -> uniffi.mailcal_bindings.repeatChangeOf(draft, wasRepeating) }

/** A calendar a create can target, or the calendar an edited event sits in. */
internal data class CalendarChoice(val account: String, val id: String, val name: String)

/** The event an editor is editing (absent when creating). */
internal data class EditTarget(
    val account: String,
    val key: String,
    /** The event's own zone, `""` for floating/all-day. */
    val zone: String,
    val isRecurring: Boolean,
    val reminderMinutes: Int?,
    val recurrence: EventRecurrence?,
    /** The rule as a sentence's parts, decided by the core, see [recurrenceSummary]. */
    val repeatSummary: RepeatSummary?,
    /**
     * The rule as the editor's controls hold it, or `null` when the core would not open it:
     * a rule too rich to state, or one whose controls this app does not have.
     */
    val seededRepeat: RepeatDraft?,
    /**
     * The occurrence this editor was opened on, as the core resolved it, or `""` when it was
     * opened on the series. Non-empty is what makes Save **ask** which occurrences it meant.
     */
    val occurrence: String,
    /** Everyone on the event, organiser first. Shown read-only, see [AttendeeList]. */
    val attendees: List<EventAttendee>,
)

/** The arguments a create dispatches (`Intent.CreateEvent`). */
internal data class CreateArgs(
    val title: String,
    val start: String,
    val end: String,
    val account: String?,
    val calendar: String?,
    val allDay: Boolean,
    val timezone: String?,
    val notes: String?,
    val location: String?,
    /** The rule a new event starts with, or `null` for a one-off. */
    val recurrence: SimpleRecurrence?,
)

/** The arguments an edit dispatches (`Intent.UpdateEvent`). */
internal data class UpdateArgs(
    val account: String,
    val key: String,
    val title: String?,
    val start: String?,
    val end: String?,
    val notes: String?,
    val location: String?,
    val occurrence: String?,
    /**
     * What happens to the repeat rule: `null` leaves the series alone, `Set` replaces the rule,
     * `Clear` makes the event a single one. Never sent beside an `occurrence`: a rule belongs to
     * the series, and the core refuses the pairing.
     */
    val recurrence: RecurrenceChange?,
    /**
     * The occurrence [start] and [end] were read from, when this save means the series but the
     * editor was opened on one occurrence.
     *
     * The core turns the edit into the shift the user made rather than writing an occurrence's
     * clocks onto the series; without it a rule change from a later occurrence moves the series'
     * start there and every earlier occurrence stops existing.
     */
    val timesFromOccurrence: String?,
)

/**
 * The mutable state of an open editor. Construct via [create] or [edit]; the composable binds its
 * fields directly. All the decisions, validity, the frozen-on-edit fields, the two payload shapes:
 * are methods here, so a test drives them without a composition.
 */
internal class EventEditorState private constructor(
    val editing: EditTarget?,
    /** The zone the wall clocks in [startTime]/[endTime] are in, the device's on create, the
     *  event's own on edit. Empty for a floating or all-day event. */
    val zone: String,
    initialTitle: String,
    initialAllDay: Boolean,
    initialStart: LocalDateTime,
    initialEnd: LocalDateTime,
    initialLocation: String,
    initialNotes: String,
    initialCalendar: CalendarChoice?,
    initialRepeat: RepeatDraft?,
    private val repeatChangeOf: RepeatChangeOf,
) {
    var title by mutableStateOf(initialTitle)
    var allDay by mutableStateOf(initialAllDay)
    var startDate by mutableStateOf(initialStart.toLocalDate())
    var startTime by mutableStateOf(initialStart.toLocalTime())
    var endDate by mutableStateOf(initialEnd.toLocalDate())
    var endTime by mutableStateOf(initialEnd.toLocalTime())
    var location by mutableStateOf(initialLocation)
    var notes by mutableStateOf(initialNotes)
    var calendar by mutableStateOf(initialCalendar)

    /**
     * What the repeat controls hold, or `null` for "does not repeat". Seeded from the core and
     * passed back to it. The `stored` rule it carries is what tells a rule that changed from one
     * that did not, and what keeps the parts no control here models.
     */
    var repeatDraft by mutableStateOf(initialRepeat)

    val isEditing: Boolean get() = editing != null

    /**
     * Whether the repeat controls are offered at all. An event that does not repeat can always be
     * given a rule; one that already repeats can only be changed when the core handed over a
     * draft, which it does not for a rule it could not state in full.
     */
    val canEditRepeat: Boolean
        get() = editing.let { it == null || !it.isRecurring || it.seededRepeat != null }

    /**
     * What this save should send for the repeat rule, or `null` to leave the series alone.
     *
     * The core decides it: a repeat changed and changed back is not a change, and the parts no
     * control here models are put back by the same call.
     */
    val repeatChange: RecurrenceChange?
        get() = when {
            !canEditRepeat -> null
            // Nothing chosen on an event that does not repeat. There is no question to put, which
            // is also why the JVM suite can drive every non-repeating case without the cdylib.
            repeatDraft == null && editing?.seededRepeat == null -> null
            else -> repeatChangeOf(repeatDraft, editing?.seededRepeat != null)
        }

    /**
     * Whether saving has to ask *This event / All events* first, true exactly when this editor
     * was opened on one occurrence of a series. The same question [asksAboutTheSeries] puts for a
     * drag, about the same thing.
     *
     * **A changed repeat settles the question**, so it is not put: a rule belongs to the series,
     * and one occurrence is an instance of a rule rather than a holder of one. The controls say so
     * before the user touches them.
     */
    val asksAboutTheSeries: Boolean
        get() = editing?.occurrence?.isNotEmpty() == true && repeatChange == null

    // All-day and the calendar are set at create and are display-only on edit: the engine's patcher
    // refuses a form change (all-day↔timed) or a calendar move. So the toggle and the picker are
    // enabled only when creating.
    val canEditForm: Boolean get() = editing == null

    /** Whether the title is present and the interval is non-empty (all-day: end day ≥ start day). */
    val valid: Boolean
        get() = title.isNotBlank() &&
            if (allDay) !endDate.isBefore(startDate) else endAt().isAfter(startAt())

    private fun startAt() = LocalDateTime.of(startDate, startTime)

    private fun endAt() = LocalDateTime.of(endDate, endTime)

    /** The create-intent arguments for the current fields. */
    fun createArgs(): CreateArgs =
        if (allDay) {
            CreateArgs(
                title = title.trim(),
                start = startDate.toString(),
                // The on-screen end day is inclusive; the engine wants the exclusive next day.
                end = endDate.plusDays(1).toString(),
                account = calendar?.account,
                calendar = calendar?.id,
                allDay = true,
                timezone = null,
                notes = notes.ifBlank { null },
                location = location.ifBlank { null },
                recurrence = newRule(),
            )
        } else {
            CreateArgs(
                title = title.trim(),
                start = wallClock(startAt()),
                end = wallClock(endAt()),
                account = calendar?.account,
                calendar = calendar?.id,
                allDay = false,
                // A wall clock in the device's zone, so the event is created there, not in UTC.
                timezone = zone.ifBlank { null },
                notes = notes.ifBlank { null },
                location = location.ifBlank { null },
                recurrence = newRule(),
            )
        }

    /**
     * The rule a create sends: whatever the controls hold, as a plain rule rather than one of the
     * three answers an edit gives.
     */
    private fun newRule(): SimpleRecurrence? = (repeatChange as? RecurrenceChange.Set)?.rule

    /** The update-intent arguments for the current fields. Only valid while [isEditing]. */
    /**
     * The payload a Save dispatches.
     *
     * [thisOccurrenceOnly] splits an override out of the series instead of rewriting it. Both
     * edges always travel: an occurrence's own times are not the series', so a single-occurrence
     * edit naming neither would move it onto the master's clock.
     */
    fun updateArgs(thisOccurrenceOnly: Boolean): UpdateArgs {
        val target = requireNotNull(editing) { "updateArgs() on a create editor" }
        val start: String
        val end: String
        if (allDay) {
            start = startDate.toString()
            end = endDate.plusDays(1).toString()
        } else {
            start = wallClock(startAt())
            end = wallClock(endAt())
        }
        return UpdateArgs(
            account = target.account,
            key = target.key,
            title = title.trim(),
            start = start,
            end = end,
            // Empty clears; a value sets.
            notes = notes,
            location = location,
            occurrence = target.occurrence.takeIf { thisOccurrenceOnly && it.isNotEmpty() },
            // A rule belongs to the series, so it never travels with an occurrence. The screen
            // does not offer that combination, and this is the second place it cannot happen.
            recurrence = if (thisOccurrenceOnly) null else repeatChange,
            // The clocks above are this occurrence's, so a save meant for the series says where
            // they came from and the core shifts the series by that much instead of moving it.
            timesFromOccurrence = target.occurrence.takeIf { !thisOccurrenceOnly && it.isNotEmpty() },
        )
    }

    companion object {
        private val WALL: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss")

        private fun wallClock(dt: LocalDateTime): String = dt.format(WALL)

        /**
         * A fresh editor.
         *
         * From the "New event" button, [now] is the clock and the editor opens at the next whole
         * hour for an hour, the sensible default when the user has said nothing about *when*.
         * From a **drag on the grid** the user has said exactly when, so [exact] is set: [now] is
         * taken as the start verbatim and [minutes] as the length, and nothing is rounded on top of
         * a time they drew out by hand.
         */
        fun create(
            default: CalendarChoice?,
            zone: String,
            now: LocalDateTime,
            minutes: Int = 60,
            exact: Boolean = false,
            repeatChangeOf: RepeatChangeOf = coreRepeatChangeOf,
        ): EventEditorState {
            val start = if (exact) now else now.plusHours(1).withMinute(0).withSecond(0).withNano(0)
            return EventEditorState(
                editing = null,
                zone = zone,
                initialTitle = "",
                initialAllDay = false,
                initialStart = start,
                initialEnd = start.plusMinutes(minutes.toLong()),
                initialLocation = "",
                initialNotes = "",
                initialCalendar = default,
                initialRepeat = null,
                repeatChangeOf = repeatChangeOf,
            )
        }

        /** An editor prefilled from a stored event's detail. */
        fun edit(
            detail: EventDetail,
            calendarName: String,
            repeatChangeOf: RepeatChangeOf = coreRepeatChangeOf,
        ): EventEditorState {
            val start = parseWall(detail.start)
            // The detail's all-day end is exclusive; show the inclusive last day.
            val end = if (detail.allDay) parseWall(detail.end).minusDays(1) else parseWall(detail.end)
            return EventEditorState(
                editing = EditTarget(
                    account = detail.account,
                    key = detail.key,
                    zone = detail.timezone,
                    isRecurring = detail.isRecurring,
                    reminderMinutes = detail.reminderMinutes,
                    recurrence = detail.recurrence,
                    repeatSummary = detail.repeatSummary,
                    seededRepeat = detail.repeatDraft,
                    occurrence = detail.occurrenceStart,
                    attendees = detail.attendees,
                ),
                zone = detail.timezone,
                initialTitle = detail.title,
                initialAllDay = detail.allDay,
                initialStart = start,
                initialEnd = end,
                initialLocation = detail.location ?: "",
                initialNotes = detail.notes ?: "",
                initialCalendar = CalendarChoice(detail.account, detail.calendar, calendarName),
                initialRepeat = detail.repeatDraft,
                repeatChangeOf = repeatChangeOf,
            )
        }
    }
}

/** Parse `YYYY-MM-DDTHH:MM:SS` or a bare `YYYY-MM-DD` (all-day) into a wall clock at midnight. */
internal fun parseWall(value: String): LocalDateTime =
    if (value.contains('T')) LocalDateTime.parse(value) else LocalDate.parse(value).atStartOfDay()

/** A reminder offset, bucketed for display, pure, so the JDK-17 CLDR trap can't reach it. */
internal sealed interface ReminderBucket {
    data object None : ReminderBucket

    data object AtStart : ReminderBucket

    data class Minutes(val n: Int) : ReminderBucket

    data class Hours(val n: Int) : ReminderBucket

    data class Days(val n: Int) : ReminderBucket
}

/** Buckets minutes-before into the coarsest exact unit (a day, an hour, else minutes). */
internal fun reminderBucket(minutes: Int?): ReminderBucket = when {
    minutes == null -> ReminderBucket.None
    minutes <= 0 -> ReminderBucket.AtStart
    minutes % 1440 == 0 -> ReminderBucket.Days(minutes / 1440)
    minutes % 60 == 0 -> ReminderBucket.Hours(minutes / 60)
    else -> ReminderBucket.Minutes(minutes)
}
