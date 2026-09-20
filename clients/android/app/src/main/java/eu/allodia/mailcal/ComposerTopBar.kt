// The composer's app bar: Close on the left, and Attach, Signature, Save as draft and Send on the
// right.
//
// The app bar IS this platform's action bar, which is where docs/signatures.md puts the signature
// control and docs/drafts.md the Save action: each is something you do to the message, not a field
// you address it with. macOS and iOS draw the same row above the editor, having no app bar.
//
// Its own file rather than a block inside RichComposeScreen.kt, which is at the 500-line limit.
package eu.allodia.mailcal

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ComposerTopBar(
    title: String,
    onClose: () -> Unit,
    onAttach: () -> Unit,
    // The signature picker, or null when signatures are off for this composer or the user has
    // written none: with an empty library the menu would offer nothing but "None", a control that
    // cannot do anything.
    signaturePicker: (@Composable () -> Unit)?,
    // Saving the draft, or null when this composer keeps none. Never disabled: a draft is
    // unfinished by definition, so there is no state this refuses, and pressing it on an unchanged
    // message reaches no server (`docs/drafts.md`).
    onSaveDraft: (() -> Unit)?,
    sendEnabled: Boolean,
    onSend: () -> Unit,
) {
    val ctx = LocalContext.current
    TopAppBar(
        title = { Text(title) },
        navigationIcon = {
            IconButton(onClick = onClose) {
                Icon(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = L10n.action_cancel(ctx),
                )
            }
        },
        actions = {
            IconButton(onClick = onAttach) {
                Icon(
                    painter = painterResource(R.drawable.ic_attachment),
                    contentDescription = L10n.action_attach(ctx),
                )
            }
            signaturePicker?.invoke()
            if (onSaveDraft != null) {
                IconButton(onClick = onSaveDraft) {
                    Icon(
                        painter = painterResource(R.drawable.ic_save_draft),
                        contentDescription = L10n.action_save_draft(ctx),
                    )
                }
            }
            IconButton(enabled = sendEnabled, onClick = onSend) {
                Icon(
                    painter = painterResource(R.drawable.ic_send),
                    contentDescription = L10n.action_send(ctx),
                )
            }
        },
    )
}
