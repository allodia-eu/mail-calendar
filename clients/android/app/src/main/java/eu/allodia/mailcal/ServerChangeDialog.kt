// The question a calendar or address book server found on another site asks
// (docs/account-autodetect.md rule 14). Not now is the dismiss action, and back or a tap outside
// answers it too, so nothing but the Allow button allows.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ControlState

/** [prompt]'s question while one is open, as the core drew it; [onAllow] sends the change it shows. */
@Composable
internal fun ServerChangeDialog(prompt: ServerChangePrompt, onAllow: (ShownServerChange) -> Unit) {
    val shown = prompt.shown ?: return
    val ctx = LocalContext.current
    val question = shown.question
    val copy = ServerChangeRules.copy(ctx, question.change, shown.address)
    AlertDialog(
        onDismissRequest = { prompt.close() },
        modifier = Modifier.testTag("server-change-dialog"),
        title = { Text(copy.title) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Column {
                    Text(copy.body)
                    Text(ServerChangeRules.emphasised(copy.before, question.change.previousSite))
                    Text(ServerChangeRules.emphasised(copy.now, question.change.proposedSite))
                }
                Text(copy.advice)
                Text(copy.unsure)
                if (question.failed) {
                    Text(
                        L10n.server_changed_failed(ctx),
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.testTag("server-change-failed"),
                    )
                }
            }
        },
        confirmButton = {
            ControlTextButton(
                label = L10n.server_changed_allow(ctx),
                control = question.allow,
                tag = "server-change-allow",
            ) { onAllow(shown) }
        },
        dismissButton = {
            // Waits for an Allow under way, which closing would not stop.
            ControlTextButton(
                label = L10n.server_changed_not_now(ctx),
                control = ControlState(enabled = !question.allow.running, running = false),
                tag = "server-change-not-now",
            ) { prompt.close() }
        },
    )
}
