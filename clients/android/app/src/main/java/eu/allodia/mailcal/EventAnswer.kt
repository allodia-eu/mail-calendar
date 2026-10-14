// An invitee's answer in an event's detail: what they answered, and Accept, Maybe and Decline to
// change it.
//
// Offered only where the core sets `EventDetail.myAnswer`: the account is invited, does not
// organise the meeting, and has a way to tell the organiser. The answer is for the whole meeting,
// and goes the way the invitation card's does (docs/calendar.md, "Answering from the calendar").
// The words are the card's, so the two places a meeting is answered read the same.
//
// Accept and Maybe go at once and the detail stays open; Decline asks first, because a declined
// meeting leaves the calendar and the detail with it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ResponseStatus

/** Whether [response] is the answer already given; that button is shown chosen and inert. */
internal fun isCurrentAnswer(current: ResponseStatus, response: InvitationResponse): Boolean =
    when (response) {
        InvitationResponse.ACCEPT -> current == ResponseStatus.ACCEPTED
        InvitationResponse.TENTATIVE -> current == ResponseStatus.TENTATIVE
        InvitationResponse.DECLINE -> current == ResponseStatus.DECLINED
    }

/**
 * The answer, under a line saying what was answered so far. [onAnswer] gets the reply subject.
 * While [saving], no button can be pressed: an answer is on its way, and a second would race it.
 */
@Composable
internal fun EventAnswer(
    current: ResponseStatus,
    title: String,
    saving: Boolean,
    onAnswer: (InvitationResponse, String) -> Unit,
) {
    val ctx = LocalContext.current
    var confirmingDecline by remember { mutableStateOf(false) }
    Column(modifier = Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            invitationResponseLine(ctx, current),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for ((response, label, spoken) in listOf(
                Triple(InvitationResponse.ACCEPT, L10n.invitation_accept(ctx), L10n.a11y_invitation_accept(ctx)),
                Triple(InvitationResponse.TENTATIVE, L10n.invitation_tentative(ctx), L10n.a11y_invitation_tentative(ctx)),
                Triple(InvitationResponse.DECLINE, L10n.invitation_decline(ctx), L10n.a11y_invitation_decline(ctx)),
            )) {
                val described = Modifier.semantics { contentDescription = spoken }
                val choose = {
                    if (response == InvitationResponse.DECLINE) {
                        confirmingDecline = true
                    } else {
                        onAnswer(response, invitationReplySubject(ctx, response, title))
                    }
                }
                // The answer already given: sending it again changes nothing, so it is the one
                // button that cannot be pressed.
                if (isCurrentAnswer(current, response)) {
                    Button(onClick = choose, enabled = false, modifier = described) { Text(label) }
                } else {
                    OutlinedButton(onClick = choose, enabled = !saving, modifier = described) { Text(label) }
                }
            }
        }
    }
    if (confirmingDecline) {
        AlertDialog(
            onDismissRequest = { confirmingDecline = false },
            title = { Text(L10n.event_decline_confirm_title(ctx)) },
            text = { Text(L10n.event_decline_confirm_body(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingDecline = false
                        val response = InvitationResponse.DECLINE
                        onAnswer(response, invitationReplySubject(ctx, response, title))
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
                ) { Text(L10n.invitation_decline(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmingDecline = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}
