// The editor's invitee decisions, tested without composing a screen: where the list is offered,
// what a create carries, what an organiser's edit sends, and how the field fills the list. The
// core's two answers are stood in for here, since nothing in this suite loads the cdylib; the real
// ones are tested in `mailcal-bindings` (`meeting_editor_tests.rs`).
package eu.allodia.mailcal

import java.time.LocalDateTime
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EventAttendee
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.InviteeCheck
import uniffi.mailcal_bindings.MeetingInvitee
import uniffi.mailcal_bindings.MeetingInviteePatch
import uniffi.mailcal_bindings.MeetingInviteeRole
import uniffi.mailcal_bindings.ResponseStatus

private val NOW = LocalDateTime.of(2026, 8, 1, 9, 15, 0)
private val REQUIRED = MeetingInviteeRole.REQUIRED
private val OPTIONAL = MeetingInviteeRole.OPTIONAL

/** The core's check, in the shape that matters here: an `@`, not already listed, not the organiser. */
private val check: InviteeCheckOf = { typed, invited, organiser ->
    val address = typed.trim()
    when {
        !address.contains('@') -> InviteeCheck.NotAnAddress
        address.equals(organiser, ignoreCase = true) -> InviteeCheck.Organiser
        invited.any { it.equals(address, ignoreCase = true) } -> InviteeCheck.AlreadyInvited
        else -> InviteeCheck.Valid(address)
    }
}

/** The core's patch over the rows an organiser may edit; `null` when nothing changed. */
private val patch: InviteePatchOf = { attendees, invitees ->
    val editable = attendees.filter { !it.isOrganizer && it.role != null }
    val upsert = invitees.filter { invitee ->
        editable.none { it.email == invitee.email && it.role == invitee.role }
    }
    val remove = editable.filter { row -> invitees.none { it.email == row.email } }.map { it.email }
    if (upsert.isEmpty() && remove.isEmpty()) null else MeetingInviteePatch(upsert, remove)
}

private fun attendee(email: String, role: MeetingInviteeRole?, isOrganizer: Boolean = false) =
    EventAttendee(name = "", email = email, isOrganizer = isOrganizer, role = role, response = ResponseStatus.NEEDS_ACTION)

private fun meeting(occurrence: String = "", canEditInvitees: Boolean = true) = EventDetail(
    account = "acct",
    key = "/cal/m.ics",
    calendar = "work",
    title = "Planning",
    allDay = false,
    timezone = "Europe/Amsterdam",
    start = "2026-08-03T10:00:00",
    end = "2026-08-03T11:00:00",
    location = null,
    notes = null,
    reminderMinutes = null,
    recurrence = null,
    repeatSummary = null,
    repeatDraft = null,
    isRecurring = occurrence.isNotEmpty(),
    canWrite = true,
    canEditInvitees = canEditInvitees,
    occurrenceStart = occurrence,
    attendees = listOf(
        attendee("me@example.org", null, isOrganizer = true),
        attendee("grace@example.org", REQUIRED),
        attendee("chair@example.org", null),
    ),
)

private fun creating(canInvite: Boolean) = EventEditorState.create(
    CalendarChoice("acct", "work", "Work", "me@example.org", canInvite),
    "Europe/Amsterdam",
    NOW,
    inviteePatchOf = patch,
    inviteeCheckOf = check,
).apply { title = "Planning" }

private fun editing(detail: EventDetail) =
    EventEditorState.edit(detail, "Work", inviteePatchOf = patch, inviteeCheckOf = check)

class EventInviteesTest {
    @Test
    fun a_create_invites_only_on_a_calendar_whose_server_sends_invitations() {
        val on = creating(canInvite = true)
        assertTrue(on.canInvite)
        on.roster.fieldChanged("grace@example.org")
        assertTrue(on.roster.takeTyped())
        assertEquals(listOf(MeetingInvitee(null, "grace@example.org", REQUIRED)), on.createArgs().invitees)

        val off = creating(canInvite = false)
        assertFalse(off.canInvite)
        off.roster.fieldChanged("grace@example.org")
        off.roster.takeTyped()
        assertNull(off.createArgs().invitees)
    }

    @Test
    fun a_create_with_nobody_on_the_list_is_an_appointment() {
        assertNull(creating(canInvite = true).createArgs().invitees)
    }

    @Test
    fun a_finished_address_moves_out_of_the_field_into_the_list() {
        val editor = creating(canInvite = true)
        editor.roster.fieldChanged("grace@example.org, ada")
        assertEquals(listOf("grace@example.org"), editor.roster.invitees.map { it.email })
        assertEquals("ada", editor.roster.field)
    }

    @Test
    fun a_refused_address_stays_in_the_field_with_the_reason() {
        val editor = creating(canInvite = true)
        for ((typed, said) in listOf(
            "not an address" to InviteeCheck.NotAnAddress,
            "ME@example.org" to InviteeCheck.Organiser,
        )) {
            editor.roster.fieldChanged(typed)
            assertFalse(editor.roster.takeTyped())
            assertEquals(said, editor.roster.refusal)
            assertEquals(typed, editor.roster.field)
        }
        editor.roster.fieldChanged("grace@example.org")
        assertTrue(editor.roster.takeTyped())
        assertNull(editor.roster.refusal)
        editor.roster.fieldChanged("GRACE@example.org")
        assertFalse(editor.roster.takeTyped())
        assertEquals(InviteeCheck.AlreadyInvited, editor.roster.refusal)
    }

    @Test
    fun an_organiser_opens_the_list_with_the_rows_they_may_edit_and_sends_what_changed() {
        val editor = editing(meeting())
        assertTrue(editor.canInvite)
        assertEquals(listOf(MeetingInvitee(null, "grace@example.org", REQUIRED)), editor.roster.invitees)
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).invitees)

        // The chair is on the meeting already, though no control here may change their row.
        editor.roster.fieldChanged("chair@example.org")
        assertFalse(editor.roster.takeTyped())

        editor.roster.setOptional(0, true)
        editor.roster.fieldChanged("ada@example.org")
        editor.roster.takeTyped()
        assertEquals(
            MeetingInviteePatch(
                upsert = listOf(
                    MeetingInvitee(null, "grace@example.org", OPTIONAL),
                    MeetingInvitee(null, "ada@example.org", REQUIRED),
                ),
                remove = emptyList(),
            ),
            editor.updateArgs(thisOccurrenceOnly = false).invitees,
        )
        editor.roster.remove(0)
        assertEquals(listOf("grace@example.org"), editor.updateArgs(thisOccurrenceOnly = false).invitees?.remove)
    }

    @Test
    fun somebody_elses_meeting_offers_no_list_and_sends_no_roster() {
        val editor = editing(meeting(canEditInvitees = false))
        assertFalse(editor.canInvite)
        assertTrue(editor.roster.invitees.isEmpty())
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).invitees)
    }

    @Test
    fun a_changed_roster_settles_the_scope_question_and_never_travels_with_one_occurrence() {
        val editor = editing(meeting(occurrence = "2026-08-03T10:00:00"))
        assertTrue(editor.asksAboutTheSeries)
        editor.roster.remove(0)
        assertFalse(editor.asksAboutTheSeries)
        assertEquals(listOf("grace@example.org"), editor.updateArgs(thisOccurrenceOnly = false).invitees?.remove)
        assertNull(editor.updateArgs(thisOccurrenceOnly = true).invitees)
    }

    @Test
    fun an_existing_meeting_keeps_its_last_invitee_and_a_create_does_not_have_to() {
        val meeting = editing(meeting())
        assertTrue("the chair is still on the meeting", !meeting.roster.keepsLastInvitee)
        val withoutChair = editing(meeting().copy(attendees = meeting().attendees.dropLast(1)))
        assertTrue(withoutChair.roster.keepsLastInvitee)
        withoutChair.roster.fieldChanged("ada@example.org")
        withoutChair.roster.takeTyped()
        assertFalse(withoutChair.roster.keepsLastInvitee)

        val create = creating(canInvite = true)
        create.roster.fieldChanged("ada@example.org")
        create.roster.takeTyped()
        assertFalse("a create may become an appointment", create.roster.keepsLastInvitee)
    }
}
