// The event editor's decisions, tested without composing a screen: the two payload shapes (a zoned
// wall-clock create, an event-zone edit), the all-day inclusive↔exclusive conversion that bites in
// both directions, the fields frozen on edit, and validity.
package eu.allodia.mailcal

import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EventAttendee
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.EventRecurrence

private val NOW = LocalDateTime.of(2026, 8, 1, 9, 15, 0)

private fun detail(
    allDay: Boolean,
    timezone: String,
    start: String,
    end: String,
    isRecurring: Boolean = false,
    reminderMinutes: Int? = null,
    recurrence: EventRecurrence? = null,
    attendees: List<EventAttendee> = emptyList(),
) = EventDetail(
    account = "acct",
    key = "/cal/e.ics",
    calendar = "work",
    title = "Standup",
    allDay = allDay,
    timezone = timezone,
    start = start,
    end = end,
    location = "Room 2",
    notes = "bring the roadmap",
    reminderMinutes = reminderMinutes,
    recurrence = recurrence,
    repeatSummary = null,
    repeatDraft = null,
    isRecurring = isRecurring,
    canWrite = true,
    occurrenceStart = "",
    attendees = attendees,
)

class EventEditorTest {
    @Test
    fun a_created_timed_event_is_a_wall_clock_in_the_device_zone() {
        // The whole point of passing a zone: the event is created at the clock the user typed, in
        // their zone, not silently converted to UTC (which read back an hour off on edit).
        val editor = EventEditorState.create(
            CalendarChoice("acct", "work", "Work"),
            "Europe/Amsterdam",
            NOW,
        )
        editor.title = "Lunch"
        editor.location = "Room 6" // a create can set a location now (engine PR)
        val args = editor.createArgs()
        assertEquals("Lunch", args.title)
        assertEquals("2026-08-01T10:00:00", args.start) // next whole hour after 09:15
        assertEquals("2026-08-01T11:00:00", args.end)
        assertEquals("Europe/Amsterdam", args.timezone)
        assertEquals("acct", args.account)
        assertEquals("work", args.calendar)
        assertFalse(args.allDay)
        assertNull(args.notes)
        assertEquals("Room 6", args.location)
    }

    @Test
    fun a_created_event_with_no_location_sends_none() {
        // Empty stays absent, the core turns a null into no LOCATION line at all.
        val editor = EventEditorState.create(CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", NOW)
        editor.title = "Lunch"
        assertNull(editor.createArgs().location)
    }

    @Test
    fun a_created_all_day_event_sends_an_exclusive_end_and_no_zone() {
        val editor = EventEditorState.create(CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", NOW)
        editor.title = "Holiday"
        editor.allDay = true // one day: start and end both fall on 2026-08-01
        val args = editor.createArgs()
        assertTrue(args.allDay)
        assertEquals("2026-08-01", args.start)
        assertEquals("2026-08-02", args.end) // exclusive
        assertNull(args.timezone)
    }

    @Test
    fun editing_prefills_the_events_own_wall_clock_and_updates_it() {
        val editor = EventEditorState.edit(
            detail(allDay = false, timezone = "Europe/Amsterdam", start = "2026-01-05T09:30:00", end = "2026-01-05T10:00:00"),
            "Work",
        )
        assertTrue(editor.isEditing)
        assertEquals("Standup", editor.title)
        assertEquals(LocalDate.of(2026, 1, 5), editor.startDate)
        assertEquals(LocalTime.of(9, 30), editor.startTime)
        assertEquals("Room 2", editor.location)

        editor.title = "Standup (kort)"
        val args = editor.updateArgs(thisOccurrenceOnly = false)
        assertEquals("acct", args.account)
        assertEquals("/cal/e.ics", args.key)
        assertEquals("Standup (kort)", args.title)
        assertEquals("2026-01-05T09:30:00", args.start)
        assertEquals("2026-01-05T10:00:00", args.end)
        assertEquals("Room 2", args.location)
        assertNull(args.occurrence) // v1 edits the whole series
    }

    @Test
    fun editing_an_all_day_event_shows_the_inclusive_day_and_saves_the_exclusive_one() {
        // The detail's end is exclusive (04-02 for a one-day event on the 1st). The editor must show
        // the 1st, and save the 2nd again, an off-by-one here is a one-day event that grows to two.
        val editor = EventEditorState.edit(
            detail(allDay = true, timezone = "", start = "2026-04-01", end = "2026-04-02"),
            "Work",
        )
        assertTrue(editor.allDay)
        assertEquals(LocalDate.of(2026, 4, 1), editor.startDate)
        assertEquals(LocalDate.of(2026, 4, 1), editor.endDate)
        val args = editor.updateArgs(thisOccurrenceOnly = false)
        assertEquals("2026-04-01", args.start)
        assertEquals("2026-04-02", args.end)
    }

    @Test
    fun all_day_and_calendar_are_frozen_on_edit_but_free_on_create() {
        assertTrue(EventEditorState.create(null, "Europe/Amsterdam", NOW).canEditForm)
        assertFalse(
            EventEditorState.edit(
                detail(allDay = false, timezone = "Europe/Amsterdam", start = "2026-01-05T09:30:00", end = "2026-01-05T10:00:00"),
                "Work",
            ).canEditForm,
        )
    }

    @Test
    fun an_editor_is_invalid_without_a_title_or_a_positive_interval() {
        val editor = EventEditorState.create(CalendarChoice("acct", "work", "Work"), "Europe/Amsterdam", NOW)
        assertFalse("a blank title is invalid", editor.valid)
        editor.title = "X"
        assertTrue(editor.valid)
        editor.endDate = editor.startDate
        editor.endTime = editor.startTime
        assertFalse("end must be after start", editor.valid)
    }

    @Test
    fun reminders_bucket_into_the_coarsest_exact_unit() {
        assertEquals(ReminderBucket.None, reminderBucket(null))
        assertEquals(ReminderBucket.AtStart, reminderBucket(0))
        assertEquals(ReminderBucket.Minutes(15), reminderBucket(15))
        assertEquals(ReminderBucket.Hours(2), reminderBucket(120))
        assertEquals(ReminderBucket.Days(1), reminderBucket(1440))
    }
}
