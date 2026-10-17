// The meeting's invitee list inside the event editor: who it invites, and whether each one is
// required or optional, as the organiser edits it.
//
// Offered only where the core says a meeting may be organised (docs/calendar.md, "Organising a
// meeting"): on a create, a calendar whose server sends the invitations; on an edit, a meeting this
// account organises. Somebody else's meeting shows its list read-only, with the sentence saying so,
// and a create on a calendar that cannot invite says why nobody can be added. The state is
// InviteeRoster's; this file only draws it.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.InviteeCheck
import uniffi.mailcal_bindings.MeetingInvitee
import uniffi.mailcal_bindings.MeetingInviteeRole
import uniffi.mailcal_bindings.RecipientMatch

@Composable
internal fun InviteeSection(
    editor: EventEditorState,
    suggestionsFor: ((String) -> List<RecipientMatch>)?,
) {
    val ctx = LocalContext.current
    val editing = editor.editing
    val attendees = editing?.attendees.orEmpty()
    if (!editor.canInvite) {
        if (editing == null) {
            Note(L10n.event_invitees_unavailable(ctx))
        } else if (attendees.isNotEmpty()) {
            // Shown so an edit is not made blind to who is coming, and stated to be read-only
            // rather than offered as a field that would quietly drop the change.
            Heading(L10n.event_attendees(ctx))
            AttendeeList(attendees)
            Note(L10n.event_attendees_read_only(ctx))
        }
        return
    }
    val roster = editor.roster
    Heading(L10n.event_attendees(ctx))
    // The rows no control here may change: the organiser, and a role the editor cannot state.
    AttendeeList(attendees.filter { it.isOrganizer || it.role == null })
    roster.invitees.forEachIndexed { index, invitee ->
        InviteeRow(
            invitee = invitee,
            answer = attendees.firstOrNull { it.email.equals(invitee.email, ignoreCase = true) }
                ?.let { attendeeResponseText(ctx, it.response) },
            onOptional = { roster.setOptional(index, it) },
            onRemove = { roster.remove(index) },
            removable = !roster.keepsLastInvitee,
        )
    }
    Row(verticalAlignment = Alignment.Top) {
        RecipientField(
            label = L10n.event_invitee_field(ctx),
            value = roster.field,
            onValue = roster::fieldChanged,
            suggestionsFor = suggestionsFor,
            modifier = Modifier.weight(1f),
            onSubmit = { roster.takeTyped() },
        )
        TextButton(
            onClick = { roster.takeTyped() },
            modifier = Modifier.padding(top = 8.dp),
        ) { Text(L10n.event_invitee_add(ctx)) }
    }
    roster.refusal?.let { refused ->
        Text(
            text = inviteeRefusal(ctx, refused).orEmpty(),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
    Note(L10n.event_invitees_send_note(ctx))
}

@Composable
private fun InviteeRow(
    invitee: MeetingInvitee,
    answer: String?,
    onOptional: (Boolean) -> Unit,
    onRemove: () -> Unit,
    removable: Boolean,
) {
    val ctx = LocalContext.current
    val shown = invitee.name?.ifEmpty { null } ?: invitee.email
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(shown, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (shown != invitee.email) {
                Text(
                    text = invitee.email,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            answer?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        val optional = invitee.role == MeetingInviteeRole.OPTIONAL
        // One toggle with its label, so a screen reader announces the box and what it means
        // together.
        Row(
            modifier = Modifier.toggleable(value = optional, role = Role.Checkbox, onValueChange = onOptional),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Checkbox(checked = optional, onCheckedChange = null)
            Text(L10n.event_attendee_optional(ctx), style = MaterialTheme.typography.bodyMedium)
        }
        IconButton(onClick = onRemove, enabled = removable) {
            Icon(
                painter = painterResource(R.drawable.ic_close),
                // Named for the person, so the fourth of four identical buttons is still
                // distinguishable to a screen reader; and, when the core would refuse it, why not.
                contentDescription = if (removable) {
                    L10n.event_invitee_remove(ctx, shown)
                } else {
                    "${L10n.event_invitee_remove(ctx, shown)}. ${L10n.event_invitee_last(ctx)}"
                },
            )
        }
    }
}

/** What the field says when an address cannot be added; `null` for one that can. */
internal fun inviteeRefusal(ctx: Context, check: InviteeCheck): String? = when (check) {
    is InviteeCheck.Valid -> null
    InviteeCheck.NotAnAddress -> L10n.event_invitee_not_an_address(ctx)
    InviteeCheck.AlreadyInvited -> L10n.event_invitee_already_invited(ctx)
    InviteeCheck.Organiser -> L10n.event_invitee_organiser(ctx)
}

@Composable
private fun Heading(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        modifier = Modifier.padding(top = 14.dp),
    )
}

@Composable
private fun Note(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 8.dp),
    )
}
