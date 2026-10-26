// The question a calendar or address book server found on another site asks
// (docs/account-autodetect.md rule 14). Not now is the dismiss action, and back or a tap outside
// answers it too, so nothing but the Allow button allows.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

/** [prompt]'s question while one is open; [onAllow] runs the blocking accept off the main thread. */
@Composable
internal fun ServerChangeDialog(prompt: ServerChangePrompt, onAllow: () -> Unit) {
    val shown = prompt.shown ?: return
    val ctx = LocalContext.current
    val copy = ServerChangeRules.copy(ctx, shown.change, shown.address)
    val allowing = prompt.allowing
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
                    Text(ServerChangeRules.emphasised(copy.before, shown.change.previousSite))
                    Text(ServerChangeRules.emphasised(copy.now, shown.change.proposedSite))
                }
                Text(copy.advice)
                Text(copy.unsure)
                if (prompt.failed) {
                    Text(
                        L10n.server_changed_failed(ctx),
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.testTag("server-change-failed"),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = onAllow,
                enabled = !allowing,
                modifier = Modifier.testTag("server-change-allow"),
            ) {
                if (allowing) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                } else {
                    Text(L10n.server_changed_allow(ctx))
                }
            }
        },
        dismissButton = {
            TextButton(
                onClick = { prompt.close() },
                enabled = !allowing,
                modifier = Modifier.testTag("server-change-not-now"),
            ) { Text(L10n.server_changed_not_now(ctx)) }
        },
    )
}
