package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.InvitationCard
import uniffi.mailcal_bindings.InvitationResponse

/**
 * Accept / Maybe / Decline, and the two controls that ride beside them on the transports that have
 * them.
 *
 * Its own file because it is the only part of the card that *writes*, and because everything in it
 * is conditional on what the account can actually do, the card itself stays a straight render of
 * what the core computed.
 *
 * # Three gates, none of them a disabled button
 *
 * - **`canRespond`**, the account's calendar cannot RSVP at all. The buttons are then *absent* and
 *   a sentence says why. A greyed-out Accept invites the user to try, wonder, and try again.
 * - **`canComment`**, the transport has nowhere to put a note (CalDAV, JMAP). The field is absent,
 *   because the core **refuses** a note it cannot carry rather than dropping it: an offered field
 *   would not merely lose the text, it would lose the whole answer.
 * - **`canChooseNotify`**, the server sends the reply the moment the status changes and no client
 *   can stop it. The toggle is absent for the same reason.
 *
 * On both harness accounts, and on any CalDAV or JMAP account, this is three buttons and nothing
 * else. That is the truth of the transport, not a missing feature.
 *
 * The three buttons carry their own `contentDescription`s: read out of context, three bare verbs
 * tell a screen-reader user nothing about what they act on.
 */
@Composable
internal fun InvitationRespond(
    card: InvitationCard,
    status: CalendarWriteStatus,
    onRespond: (InvitationResponse, String?, Boolean, String) -> Unit,
) {
    val ctx = LocalContext.current
    if (!card.canRespond) {
        Text(
            text = L10n.invitation_cannot_respond(ctx),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    // Local, and cleared with the card: the note exists only until the answer goes out, and a
    // rebuild after the write must not resurrect it.
    var comment by remember(card.startsAt, card.summary) { mutableStateOf("") }
    // The RFC 5546 default: an invitation asks for a reply, so answering sends one.
    var notifyOrganizer by remember(card.startsAt, card.summary) { mutableStateOf(true) }
    val sending = status == CalendarWriteStatus.SAVING
    // One place assembles the four arguments, so the three buttons cannot drift apart, and so
    // the reply subject is composed exactly once, here, where the locale is.
    val answer = { response: InvitationResponse ->
        onRespond(
            response,
            comment.takeIf { card.canComment },
            if (card.canChooseNotify) notifyOrganizer else true,
            invitationReplySubject(ctx, response, card.summary),
        )
    }

    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (card.canComment) {
            OutlinedTextField(
                value = comment,
                onValueChange = { comment = it },
                label = { Text(L10n.invitation_message_to_organizer(ctx)) },
                singleLine = false,
                maxLines = 3,
                enabled = !sending,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (card.canChooseNotify) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Switch(
                    checked = notifyOrganizer,
                    onCheckedChange = { notifyOrganizer = it },
                    enabled = !sending,
                )
                Text(
                    text = L10n.invitation_notify_organizer(ctx),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { answer(InvitationResponse.ACCEPT) },
                enabled = !sending,
                modifier = Modifier.semantics {
                    contentDescription = L10n.a11y_invitation_accept(ctx)
                },
            ) { Text(L10n.invitation_accept(ctx)) }
            OutlinedButton(
                onClick = { answer(InvitationResponse.TENTATIVE) },
                enabled = !sending,
                modifier = Modifier.semantics {
                    contentDescription = L10n.a11y_invitation_tentative(ctx)
                },
            ) { Text(L10n.invitation_tentative(ctx)) }
            OutlinedButton(
                onClick = { answer(InvitationResponse.DECLINE) },
                enabled = !sending,
                modifier = Modifier.semantics {
                    contentDescription = L10n.a11y_invitation_decline(ctx)
                },
            ) { Text(L10n.invitation_decline(ctx)) }
        }
        // What happened to the answer. A failure must say so in words: a reply the organiser never
        // received, reported as sent, is the failure this whole feature exists to prevent.
        invitationWriteLine(ctx, status)?.let { line ->
            Text(
                text = line,
                style = MaterialTheme.typography.labelSmall,
                color =
                    if (status == CalendarWriteStatus.FAILED) MaterialTheme.colorScheme.error
                    else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 2.dp),
            )
        }
    }
}
