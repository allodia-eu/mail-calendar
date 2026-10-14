// Who a meeting invites, as the event editor's controls hold it: a plain class, for the reason
// EventEditorState is one, so the JVM suite drives it without composing a screen.
//
// The list holds the rows an organiser may edit, never the organiser. Addresses arrive through the
// composer's own recipient field, so they complete from the same people index as To does, and each
// finished address moves out of the field into a row of its own. Whether one may be added is the
// core's answer (`check_meeting_invitee`), and what a save sends is worked out from the list by the
// core too (`meeting_invitee_patch`), so this class decides neither.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.InviteeCheck
import uniffi.mailcal_bindings.MeetingInvitee
import uniffi.mailcal_bindings.MeetingInviteeRole

internal class InviteeRoster(
    private val editing: EditTarget?,
    private val checkOf: InviteeCheckOf,
    /** Who organises the meeting; on a create it follows the calendar picked. */
    private val organiser: () -> String,
) {
    var invitees by mutableStateOf(
        editing?.takeIf { it.canEditInvitees }?.attendees.orEmpty().mapNotNull { attendee ->
            val role = attendee.role
            if (attendee.isOrganizer || role == null) {
                null
            } else {
                MeetingInvitee(attendee.name.ifEmpty { null }, attendee.email, role)
            }
        },
    )
        private set

    /** The field's value, comma-separated as the composer's are. */
    var field by mutableStateOf("")
        private set

    /** Why the last address could not be added, or `null` when nothing was refused. */
    var refusal by mutableStateOf<InviteeCheck?>(null)
        private set

    /**
     * Whether the one invitee left on an existing meeting must stay: a meeting is called off by
     * deleting it, and a roster change that leaves nobody invited is refused. A row nobody edits
     * here, such as a chair, still counts as invited.
     */
    val keepsLastInvitee: Boolean
        get() = invitees.size == 1 && editing?.attendees.orEmpty().let { stored ->
            stored.isNotEmpty() && stored.none { !it.isOrganizer && it.role == null }
        }

    /** The field changed: every finished address in it moves into the list. */
    fun fieldChanged(value: String) {
        val finished = committedRecipients(value)
        if (finished.isEmpty()) {
            field = value
            return
        }
        finished.forEach(::add)
        field = currentRecipientToken(value)
    }

    /**
     * Adds what is still being typed. A Save does this first, so an address typed and not added is
     * not lost; `false` when it cannot be invited, and [refusal] says why.
     */
    fun takeTyped(): Boolean {
        val typed = currentRecipientToken(field)
        if (typed.isEmpty()) return true
        val added = add(typed)
        if (added) field = ""
        return added
    }

    fun remove(index: Int) {
        invitees = invitees.filterIndexed { at, _ -> at != index }
    }

    fun setOptional(index: Int, optional: Boolean) {
        val role = if (optional) MeetingInviteeRole.OPTIONAL else MeetingInviteeRole.REQUIRED
        invitees = invitees.mapIndexed { at, invitee ->
            if (at == index) invitee.copy(role = role) else invitee
        }
    }

    private fun add(typed: String): Boolean {
        // The rows nobody edits here are on the meeting too, so typing one is not a new invitee.
        val fixed = editing?.attendees.orEmpty()
            .filter { !it.isOrganizer && it.role == null }
            .map { it.email }
        val check = checkOf(typed, fixed + invitees.map { it.email }, organiser())
        if (check is InviteeCheck.Valid) {
            invitees = invitees + MeetingInvitee(null, check.address, MeetingInviteeRole.REQUIRED)
            refusal = null
            return true
        }
        refusal = check
        return false
    }
}
