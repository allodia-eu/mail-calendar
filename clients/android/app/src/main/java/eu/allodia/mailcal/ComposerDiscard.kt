// The unsent-draft guard for the Android composer, the same prompt macOS and Windows
// already raise, reaching Android because the back button reached it first.
//
// On the desktops the guard exists because the composer is an inline pane: a click on another
// message stayed reachable while you wrote, and would have thrown the draft away silently. Android's
// composer is a full-screen dialog with nothing clickable behind it, which is why iPhone and iPad
// still short-circuit the guard, but Android has a back button and a back gesture, and an edge
// swipe is far easier to hit by accident than any click. Same loss, same answer: ask first.
//
// The dirtiness rule is the desktops', deliberately: header fields are compared against what the
// composer OPENED with, and the body against the seed captured once the quote and signature were
// in (see configureComposerWebView). A reply nobody typed into is not a draft. The comparison
// happens here in the client and yields one boolean, the document is never logged, stored, or
// shipped (docs/composer-security.md, gate 8).
package eu.allodia.mailcal

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext

/**
 * Whether the composer's header fields hold anything the user put there, the half of "is there a
 * draft to lose?" that needs no round trip into the editor.
 *
 * Compared against what the composer opened with, never against empty: the core pre-fills a reply's
 * To (and a reply-all's Cc), and a mail link may pre-fill every one of the four. Stopping someone to
 * ask about a message they never typed into is exactly the noise this guard must not create. Typing
 * something and then deleting it lands back on the opening values and counts as clean, which is true
 * there is nothing left to lose.
 */
internal fun composerHeadersEdited(
    to: String,
    initialTo: String,
    cc: String,
    initialCc: String,
    bcc: String,
    initialBcc: String,
    subject: String,
    initialSubject: String,
    attachments: Int,
): Boolean = to != initialTo ||
    cc != initialCc ||
    bcc != initialBcc ||
    subject != initialSubject ||
    attachments > 0

/**
 * The "Discard draft?" confirmation. Wording and button roles match the macOS confirmation dialog
 * and the Windows ContentDialog, down to "Keep editing" rather than "Cancel", next to "Discard", a
 * button labelled Cancel reads ambiguously as "cancel the draft".
 */
@Composable
internal fun DiscardDraftDialog(onDiscard: () -> Unit, onKeepEditing: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onKeepEditing,
        title = { Text(L10n.compose_discard_title(ctx)) },
        text = { Text(L10n.compose_discard_message(ctx)) },
        confirmButton = {
            TextButton(
                onClick = onDiscard,
                colors = ButtonDefaults.textButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
            ) {
                Text(L10n.action_discard(ctx))
            }
        },
        dismissButton = {
            TextButton(onClick = onKeepEditing) { Text(L10n.action_keep_editing(ctx)) }
        },
    )
}
