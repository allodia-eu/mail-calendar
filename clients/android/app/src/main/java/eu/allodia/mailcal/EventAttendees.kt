// Who is on an event. Shared by the detail screen and the editor, because the two must not describe
// the same meeting differently.
//
// Read-only everywhere: changing an attendee list means sending iTIP updates to the people on it,
// which is a separate feature. The editor therefore shows the list and says it cannot be changed
// here, rather than offering a control that would silently drop the change.
//
// Every string on these rows is attacker-controlled, it came from whoever sent the invitation. The
// core has already stripped control characters and bidi overrides and bounded the length, and
// Compose's Text renders it as text, so there is nothing further to escape.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.EventAttendee
import uniffi.mailcal_bindings.ResponseStatus

/** The attendee rows, in the order the core put them (organiser first). */
@Composable
internal fun AttendeeList(attendees: List<EventAttendee>, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    Column(modifier = modifier) {
        for (attendee in attendees) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    // An attendee with no display name is shown by address rather than by an
                    // invented name, so the second line is dropped instead of repeating the first.
                    Text(
                        text = attendee.name.ifEmpty { attendee.email },
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    attendeeSubtitle(attendee, L10n.event_attendee_organizer(ctx))?.let {
                        Text(
                            text = it,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))
                Text(
                    text = attendeeResponseText(ctx, attendee.response),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/**
 * The second line under an attendee: their address (when the first line used their name) and
 * whether they called the meeting. `null` when there is nothing left to say.
 *
 * Takes the organiser label rather than a Context so the rule is a plain function a test can drive.
 */
internal fun attendeeSubtitle(attendee: EventAttendee, organizerLabel: String): String? {
    val parts = buildList {
        if (attendee.name.isNotEmpty()) add(attendee.email)
        if (attendee.isOrganizer) add(organizerLabel)
    }
    return parts.takeIf { it.isNotEmpty() }?.joinToString(" · ")
}

/**
 * How one attendee answered, localised. Third person, this is somebody else's answer, unlike the
 * invitation card's "You accepted".
 */
internal fun attendeeResponseText(ctx: Context, response: ResponseStatus): String = when (response) {
    ResponseStatus.ACCEPTED -> L10n.event_attendee_accepted(ctx)
    ResponseStatus.DECLINED -> L10n.event_attendee_declined(ctx)
    ResponseStatus.TENTATIVE -> L10n.event_attendee_tentative(ctx)
    ResponseStatus.DELEGATED -> L10n.event_attendee_delegated(ctx)
    ResponseStatus.NEEDS_ACTION -> L10n.event_attendee_needs_action(ctx)
}
