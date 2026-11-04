// The editor's Video meeting row (docs/conferencing.md, rules 9 to 11), tested without composing a
// screen: what it offers, what it opens showing, and what a save sends.
package eu.allodia.mailcal

import java.time.LocalDateTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EditorMeeting
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.MeetingChange
import uniffi.mailcal_bindings.MeetingOption
import uniffi.mailcal_bindings.MeetingWords

private val NOW = LocalDateTime.of(2026, 8, 1, 9, 15, 0)
private val TEAMS = MeetingOption("server:teams", "Microsoft Teams")
private val JITSI = MeetingOption("jitsi", "Jitsi Meet")

private fun calendar(choices: List<MeetingOption>, id: String = "work") =
    CalendarChoice("acct", id, id, meetingChoices = choices)

private fun editing(meeting: EditorMeeting?, choices: List<MeetingOption>) = EventEditorState.edit(
    EventDetail(
        account = "acct",
        key = "/cal/e.ics",
        calendar = "work",
        title = "Standup",
        allDay = false,
        timezone = "Europe/Amsterdam",
        start = "2026-08-03T09:00:00",
        end = "2026-08-03T09:30:00",
        location = null,
        notes = null,
        reminderMinutes = null,
        recurrence = null,
        repeatSummary = null,
        repeatDraft = null,
        isRecurring = false,
        canWrite = true,
        canEditInvitees = false,
        myAnswer = null,
        occurrenceStart = "",
        attendees = emptyList(),
        editorMeeting = meeting,
    ),
    "Work",
    choices,
)

class EventEditorMeetingTest {
    @Test
    fun a_new_event_starts_with_no_meeting_and_offers_the_calendars_choices() {
        val editor = EventEditorState.create(calendar(listOf(TEAMS, JITSI)), "UTC", NOW)
        editor.title = "Review"
        assertTrue(editor.showsMeetingRow)
        assertEquals(
            listOf(MeetingSelection.None, MeetingSelection.Choice("server:teams"), MeetingSelection.Choice("jitsi")),
            editor.meetingOptions.map { it.selection },
        )
        assertNull(editor.createArgs().meeting)

        editor.meeting = MeetingSelection.Choice("jitsi")
        assertEquals(MeetingPick.Set("jitsi"), editor.createArgs().meeting)
    }

    @Test
    fun a_calendar_with_no_choices_draws_no_row() {
        assertFalse(EventEditorState.create(calendar(emptyList()), "UTC", NOW).showsMeetingRow)
    }

    @Test
    fun a_choice_the_new_calendar_lacks_goes_back_to_none() {
        val editor = EventEditorState.create(calendar(listOf(TEAMS, JITSI)), "UTC", NOW)
        editor.title = "Review"
        editor.meeting = MeetingSelection.Choice("server:teams")
        editor.calendar = calendar(listOf(JITSI), id = "home")
        assertEquals(MeetingSelection.None, editor.effectiveMeeting)
        assertNull(editor.createArgs().meeting)
    }

    @Test
    fun an_edit_opens_on_the_meeting_and_leaves_it_alone_until_the_row_changes() {
        val editor = editing(EditorMeeting("Microsoft Teams", "server:teams"), listOf(TEAMS, JITSI))
        assertEquals(MeetingSelection.Choice("server:teams"), editor.meeting)
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).meeting)

        editor.meeting = MeetingSelection.Choice("jitsi")
        assertEquals(MeetingPick.Set("jitsi"), editor.updateArgs(thisOccurrenceOnly = false).meeting)

        editor.meeting = MeetingSelection.None
        assertEquals(MeetingPick.Remove, editor.updateArgs(thisOccurrenceOnly = false).meeting)
        // A meeting belongs to the series.
        assertNull(editor.updateArgs(thisOccurrenceOnly = true).meeting)
    }

    @Test
    fun a_meeting_no_choice_makes_is_shown_by_name_and_can_only_be_taken_off() {
        val editor = editing(EditorMeeting("Zoom", null), listOf(JITSI))
        assertEquals(MeetingSelection.Current, editor.meeting)
        assertEquals(
            listOf("None", "Zoom", "Jitsi Meet"),
            editor.meetingOptions.map { meetingOptionLabel(it, "None", "Video meeting") },
        )
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).meeting)

        editor.meeting = MeetingSelection.None
        assertEquals(MeetingPick.Remove, editor.updateArgs(thisOccurrenceOnly = false).meeting)
    }

    @Test
    fun a_meeting_on_a_calendar_that_offers_none_can_still_be_taken_off() {
        val editor = editing(EditorMeeting(null, "jitsi"), emptyList())
        assertTrue(editor.showsMeetingRow)
        assertEquals(
            listOf("None", "Video meeting"),
            editor.meetingOptions.map { meetingOptionLabel(it, "None", "Video meeting") },
        )
        assertEquals(MeetingSelection.Choice("jitsi"), editor.effectiveMeeting)
    }

    @Test
    fun a_pick_carries_the_words_it_is_sent_with() {
        val words = MeetingWords("Video meeting", "Join", "Meeting ID", "Passcode", "Join by phone", "Conference ID")
        assertEquals(MeetingChange.Set("jitsi", words), MeetingPick.Set("jitsi").change(words))
        assertEquals(MeetingChange.Remove, MeetingPick.Remove.change(words))
    }
}
