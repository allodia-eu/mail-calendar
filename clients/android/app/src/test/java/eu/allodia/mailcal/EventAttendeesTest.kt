// The attendee row's text, tested without composing a screen. Small rules about what the user reads
// are exactly the kind that regress silently: the second line exists to add something the first line
// does not already say, so an unnamed attendee must not get their own address printed twice.
package eu.allodia.mailcal

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.EventAttendee
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.ResponseStatus

private const val ORGANIZER = "Organiser"

private fun attendee(
    name: String,
    email: String,
    isOrganizer: Boolean = false,
    response: ResponseStatus = ResponseStatus.ACCEPTED,
) = EventAttendee(name = name, email = email, isOrganizer = isOrganizer, response = response)

@RunWith(RobolectricTestRunner::class)
class EventAttendeesTest {
    @Test
    fun a_named_attendee_gets_their_address_on_the_second_line() {
        assertEquals(
            "anna@example.com",
            attendeeSubtitle(attendee("Anna Jansen", "anna@example.com"), ORGANIZER),
        )
    }

    @Test
    fun an_unnamed_attendee_has_no_second_line_because_the_first_is_their_address() {
        assertNull(attendeeSubtitle(attendee("", "b@example.com"), ORGANIZER))
    }

    @Test
    fun an_unnamed_organizer_still_says_they_called_the_meeting() {
        assertEquals(
            ORGANIZER,
            attendeeSubtitle(attendee("", "chair@example.com", isOrganizer = true), ORGANIZER),
        )
    }

    @Test
    fun a_named_organizer_shows_both_the_address_and_the_role() {
        assertEquals(
            "chair@example.com · $ORGANIZER",
            attendeeSubtitle(
                attendee("Chair Person", "chair@example.com", isOrganizer = true),
                ORGANIZER,
            ),
        )
    }

    @Test
    fun every_answer_has_its_own_wording() {
        // Five distinct strings: a status that fell through to another's label would be a
        // confidently wrong statement about somebody's answer.
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        val labels = listOf(
            ResponseStatus.ACCEPTED,
            ResponseStatus.DECLINED,
            ResponseStatus.TENTATIVE,
            ResponseStatus.DELEGATED,
            ResponseStatus.NEEDS_ACTION,
        ).map { attendeeResponseText(ctx, it) }
        assertEquals(labels.size, labels.toSet().size)
        assertEquals(0, labels.count { it.isEmpty() })
    }

    @Test
    fun the_editor_carries_the_attendees_through_so_they_can_be_shown_read_only() {
        // The editor prefills from the same detail read; without this the list would be empty in
        // the editor while the detail screen showed it, on the same event.
        val rows = listOf(attendee("Anna Jansen", "anna@example.com"))
        val detail = EventDetail(
            account = "acct",
            key = "/cal/e.ics",
            calendar = "work",
            title = "Standup",
            allDay = false,
            timezone = "Europe/Amsterdam",
            start = "2026-01-05T09:30:00",
            end = "2026-01-05T10:00:00",
            location = null,
            notes = null,
            reminderMinutes = null,
            recurrence = null,
            repeatSummary = null,
            isRecurring = false,
            canWrite = true,
            occurrenceStart = "",
            attendees = rows,
        )
        assertEquals(rows, EventEditorState.edit(detail, "Work").editing?.attendees)
    }
}
