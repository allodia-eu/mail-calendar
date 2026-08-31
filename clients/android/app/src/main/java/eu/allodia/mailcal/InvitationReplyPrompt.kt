// The "the organiser wasn't told" dialog: what a calendar server's own report turns into when it
// says it could not deliver the reply it promised to send (RFC 6638 §3.2.9, docs/invitations.md).
//
// The twin of Apple's InvitationReplyPromptView.swift and Windows's InvitationReplyPromptDialog.cs.
// The decision set is identical on all three, send or not, remember or not, and only the chrome
// differs: this is an AlertDialog with a checkbox, where a macOS sheet hosts a Toggle, because
// SwiftUI's alert takes buttons and nothing else.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import uniffi.mailcal_bindings.ReplyPrompt

/**
 * Asks whether to email the invitation reply ourselves, after the calendar server reported it could
 * not. Renders nothing when [prompt] is null, which is also how the core says *close this*: it
 * clears the question the instant it is answered.
 *
 * ## What this must not be mistaken for
 *
 * The RSVP itself **worked**, the answer is stored and the meeting on screen is right. What failed
 * is the message to the organiser, so the sentence says the answer is saved before it says what did
 * not happen. A dialog opening with "couldn't send" would invite the user to answer again, which
 * writes the same PARTSTAT and fails the same way.
 *
 * ## Why the recipient is named
 *
 * Pressing the button sends mail from the user's account to someone they did not choose in this
 * moment. That consent is not informed unless the address is on screen, so the body carries
 * [ReplyPrompt.organizer] rather than the words "the organiser". The RFC 6638 status code is
 * deliberately absent: it rides the prompt for the diagnostics log, and "5.2" explains nothing to
 * the person reading this.
 *
 * ## Why it cannot be dismissed
 *
 * Back and outside-tap are off, because neither answers the question, the core would still be
 * holding one the user can no longer see. Both buttons resolve it, and "Don't send" is a complete
 * way out, so nobody is trapped.
 *
 * @param onAnswer `(send, remember)`, the two independent halves the core's `AnswerReplyPrompt`
 *   takes. `remember` applies to whichever button was pressed: ticked plus "Don't send" is a
 *   standing *no*, not a standing yes.
 */
@Composable
internal fun InvitationReplyPrompt(prompt: ReplyPrompt?, onAnswer: (Boolean, Boolean) -> Unit) {
    if (prompt == null) return
    val ctx = LocalContext.current
    // Off by default: a standing choice for every future meeting on this account is a bigger
    // decision than the one being asked, and not the one the user came here to make. Plain
    // `remember` rather than `rememberSaveable` on purpose, the question does not survive process
    // death (the core holds it in memory), so a tick that did would outlive what it applied to.
    var alwaysDoThis by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = {},
        properties = DialogProperties(dismissOnBackPress = false, dismissOnClickOutside = false),
        modifier = Modifier.testTag("invitation-reply-prompt"),
        title = { Text(L10n.invitation_reply_undelivered_title(ctx)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // Both interpolated values are attacker-controlled, the meeting's title and the
                // organiser's address come from mail somebody else wrote, so they go into a Text,
                // which renders them as text and never as markup (docs/rendering-security.md).
                Text(
                    L10n.invitation_reply_undelivered_body(ctx, prompt.summary, prompt.organizer),
                )
                // One toggleable row rather than a bare Checkbox beside a label: the label is part
                // of the target, and a screen reader announces the pair once as a checkbox.
                Row(
                    modifier = Modifier.toggleable(
                        value = alwaysDoThis,
                        role = Role.Checkbox,
                        onValueChange = { alwaysDoThis = it },
                    ),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = alwaysDoThis, onCheckedChange = null)
                    Text(L10n.invitation_reply_undelivered_remember(ctx))
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onAnswer(true, alwaysDoThis) }) {
                Text(L10n.invitation_reply_undelivered_send(ctx))
            }
        },
        dismissButton = {
            TextButton(onClick = { onAnswer(false, alwaysDoThis) }) {
                Text(L10n.invitation_reply_undelivered_dismiss(ctx))
            }
        },
    )
}
