package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.UnfiledCopy

/**
 * The modal shown when a message went out but its copy never reached the account's Sent folder.
 *
 * Loud on purpose. A Sent copy is how a person checks that a message really left, so losing one
 * silently is worse than most failures that *do* interrupt, and nothing later recovers it, since
 * there is no copy on the server for a sync to find. The moment it happens is the only moment to
 * say so.
 *
 * The copy says what is true and no more: the message **was sent**. Wording this as a failed send
 * would make the user's next move "send it again", which is exactly the wrong one.
 *
 * Not dismissible by back press or an outside tap: the core holds the question until it is
 * answered, so a modal that vanished on a stray tap would leave a question nobody can see.
 *
 * @param onRetry file the copy, sends nothing, and is safe to press twice.
 * @param onDismiss accept the missing copy and close.
 */
@Composable
internal fun UnfiledCopyPrompt(unfiled: UnfiledCopy?, onRetry: () -> Unit, onDismiss: () -> Unit) {
    if (unfiled == null) return
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = {},
        properties = androidx.compose.ui.window.DialogProperties(
            dismissOnBackPress = false,
            dismissOnClickOutside = false,
        ),
        modifier = Modifier.testTag("unfiled-copy-prompt"),
        title = { Text(L10n.unfiled_copy_title(ctx)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // The subject is the user's own text, but it still goes into a Text so it renders
                // as text and never as markup (docs/rendering-security.md).
                Text(L10n.unfiled_copy_body(ctx, unfiled.subject))
            }
        },
        confirmButton = {
            TextButton(onClick = onRetry, enabled = !unfiled.retrying) {
                if (unfiled.retrying) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                } else {
                    Text(L10n.unfiled_copy_retry(ctx))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !unfiled.retrying) {
                Text(L10n.unfiled_copy_dismiss(ctx))
            }
        },
    )
}
