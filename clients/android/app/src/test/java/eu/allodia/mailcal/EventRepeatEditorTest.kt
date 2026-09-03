// What the repeat controls send, and (more to the point) what they refuse to send.
//
// The rebuild itself is the core's and is tested there; the core is stubbed here because nothing in
// this suite loads the cdylib. What is this client's, and is tested here, is which of the three
// answers a save carries: no rule at all beside a single occurrence, a settled scope question once
// the rule has moved, and no controls at all over a rule the core would not state.
package eu.allodia.mailcal

import java.time.LocalDate
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.EventRecurrence
import uniffi.mailcal_bindings.RecurrenceChange
import uniffi.mailcal_bindings.RecurrenceEnd
import uniffi.mailcal_bindings.RecurrenceFrequency
import uniffi.mailcal_bindings.RecurrenceWeekday
import uniffi.mailcal_bindings.RepeatDraft
import uniffi.mailcal_bindings.SimpleRecurrence

private val WEEKLY_RULE = SimpleRecurrence(
    frequency = RecurrenceFrequency.WEEKLY,
    interval = 1u,
    days = emptyList(),
    monthDays = emptyList(),
    months = emptyList(),
    end = RecurrenceEnd.Never,
)

private val WEEKLY_DRAFT = RepeatDraft(
    frequency = RecurrenceFrequency.WEEKLY,
    interval = 1u,
    weekdays = listOf(RecurrenceWeekday.WEDNESDAY),
    end = RecurrenceEnd.Never,
    stored = WEEKLY_RULE,
)

/**
 * The core's decision, stated the way the core states it, but without the cdylib: a draft equal to
 * the rule it was seeded from is not a change, and anything else is a `Set`.
 */
private val stubChangeOf: RepeatChangeOf = { draft, wasRepeating ->
    when {
        draft == null -> if (wasRepeating) RecurrenceChange.Clear else null
        draft.stored == null -> RecurrenceChange.Set(rebuild(draft))
        rebuild(draft) == draft.stored -> null
        else -> RecurrenceChange.Set(rebuild(draft))
    }
}

private fun rebuild(draft: RepeatDraft) = SimpleRecurrence(
    frequency = draft.frequency,
    interval = draft.interval,
    days = emptyList(),
    monthDays = emptyList(),
    months = emptyList(),
    end = draft.end,
)

private fun detail(
    isRecurring: Boolean,
    recurrence: EventRecurrence?,
    repeatDraft: RepeatDraft?,
    occurrence: String = "",
) = EventDetail(
    account = "acct",
    key = "/cal/e.ics",
    calendar = "work",
    title = "Standup",
    allDay = false,
    timezone = "Europe/Amsterdam",
    start = "2026-08-26T09:00:00",
    end = "2026-08-26T09:30:00",
    location = null,
    notes = null,
    reminderMinutes = null,
    recurrence = recurrence,
    repeatSummary = null,
    repeatDraft = repeatDraft,
    isRecurring = isRecurring,
    canWrite = true,
    occurrenceStart = occurrence,
    attendees = emptyList(),
)

class EventRepeatEditorTest {
    private fun editorOn(
        isRecurring: Boolean = true,
        recurrence: EventRecurrence? = EventRecurrence.Simple(WEEKLY_RULE),
        repeatDraft: RepeatDraft? = WEEKLY_DRAFT,
        occurrence: String = "",
    ) = EventEditorState.edit(
        detail(isRecurring, recurrence, repeatDraft, occurrence),
        "Work",
        stubChangeOf,
    )

    @Test
    fun a_save_that_never_touched_the_repeat_says_nothing_about_it() {
        assertNull(editorOn().updateArgs(thisOccurrenceOnly = false).recurrence)
    }

    @Test
    fun a_changed_repeat_is_sent_as_a_set() {
        val editor = editorOn()
        editor.repeatDraft = editor.repeatDraft?.copy(interval = 2u)

        val change = editor.updateArgs(thisOccurrenceOnly = false).recurrence
        assertTrue(change is RecurrenceChange.Set)
        assertEquals(2u, (change as RecurrenceChange.Set).rule.interval)
    }

    @Test
    fun choosing_does_not_repeat_clears_the_series() {
        val editor = editorOn()
        editor.repeatDraft = null
        assertEquals(
            RecurrenceChange.Clear,
            editor.updateArgs(thisOccurrenceOnly = false).recurrence,
        )
    }

    /** A rule belongs to the series. The core refuses the pairing, and the editor never builds it. */
    @Test
    fun a_rule_never_travels_with_a_single_occurrence() {
        val editor = editorOn(occurrence = "2026-09-02T09:00:00")
        editor.repeatDraft = editor.repeatDraft?.copy(interval = 3u)

        val args = editor.updateArgs(thisOccurrenceOnly = true)
        assertEquals("2026-09-02T09:00:00", args.occurrence)
        assertNull(args.recurrence)
    }

    /**
     * Opened on one occurrence, a save normally asks which occurrences it meant. A changed rule
     * answers that question on its own, so it is not put.
     */
    @Test
    fun a_changed_repeat_settles_the_scope_question() {
        val editor = editorOn(occurrence = "2026-09-02T09:00:00")
        assertTrue(editor.asksAboutTheSeries)

        editor.repeatDraft = editor.repeatDraft?.copy(interval = 2u)
        assertFalse(editor.asksAboutTheSeries)
    }

    /**
     * A rule the core would not state is shown and not offered: the client never seeds an editor
     * from a partial picture, because saving it back would drop the rest.
     */
    @Test
    fun a_rule_too_rich_to_state_offers_no_controls() {
        val editor = editorOn(recurrence = EventRecurrence.Complex, repeatDraft = null)
        assertFalse(editor.canEditRepeat)
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).recurrence)
    }

    @Test
    fun an_event_that_does_not_repeat_can_be_given_a_rule() {
        val editor = editorOn(isRecurring = false, recurrence = null, repeatDraft = null)
        assertTrue(editor.canEditRepeat)
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).recurrence)

        editor.repeatDraft = RepeatDraft(
            frequency = RecurrenceFrequency.DAILY,
            interval = 1u,
            weekdays = listOf(RecurrenceWeekday.WEDNESDAY),
            end = RecurrenceEnd.Never,
            stored = null,
        )
        val change = editor.updateArgs(thisOccurrenceOnly = false).recurrence
        assertTrue(change is RecurrenceChange.Set)
        assertEquals(RecurrenceFrequency.DAILY, (change as RecurrenceChange.Set).rule.frequency)
    }

    @Test
    fun a_create_carries_the_rule_as_a_plain_rule_rather_than_an_answer() {
        val editor = EventEditorState.create(
            CalendarChoice("acct", "work", "Work"),
            "Europe/Amsterdam",
            java.time.LocalDateTime.of(2026, 8, 26, 9, 0),
            repeatChangeOf = stubChangeOf,
        )
        editor.title = "Standup"
        assertNull(editor.createArgs().recurrence)

        editor.repeatDraft = RepeatDraft(
            frequency = RecurrenceFrequency.WEEKLY,
            interval = 2u,
            weekdays = listOf(RecurrenceWeekday.WEDNESDAY),
            end = RecurrenceEnd.AfterCount(8u),
            stored = null,
        )
        val rule = editor.createArgs().recurrence
        assertEquals(RecurrenceFrequency.WEEKLY, rule?.frequency)
        assertEquals(2u, rule?.interval)
        assertEquals(RecurrenceEnd.AfterCount(8u), rule?.end)
    }

    // --- The pure control logic -----------------------------------------------------------

    /** A weekly rule that names no day is not a rule, so the last day ticked stays ticked. */
    @Test
    fun the_weekday_row_never_empties() {
        val order = localWeekOrder(Locale.UK)
        val one = listOf(RecurrenceWeekday.WEDNESDAY)
        assertEquals(one, toggledWeekdays(one, RecurrenceWeekday.WEDNESDAY, order))
    }

    @Test
    fun ticking_a_weekday_returns_the_row_in_week_order() {
        val order = localWeekOrder(Locale.UK)
        val ticked = toggledWeekdays(listOf(RecurrenceWeekday.FRIDAY), RecurrenceWeekday.MONDAY, order)
        assertEquals(listOf(RecurrenceWeekday.MONDAY, RecurrenceWeekday.FRIDAY), ticked)
    }

    @Test
    fun the_week_starts_where_the_locale_starts_it() {
        assertEquals(RecurrenceWeekday.MONDAY, localWeekOrder(Locale.UK).first())
        assertEquals(RecurrenceWeekday.SUNDAY, localWeekOrder(Locale.US).first())
        assertEquals(7, localWeekOrder(Locale.US).toSet().size)
    }

    @Test
    fun a_rule_first_chosen_falls_on_the_events_own_weekday() {
        // 26 August 2026 is a Wednesday.
        assertEquals(RecurrenceWeekday.WEDNESDAY, recurrenceWeekday(LocalDate.of(2026, 8, 26)))
    }
}
