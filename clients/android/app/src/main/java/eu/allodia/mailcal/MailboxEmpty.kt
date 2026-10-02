package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.EmptyReason

/**
 * Why the mail list has no rows, drawn over the rows themselves.
 *
 * The badge beside the folder counts what the server holds over all time, while the list can only
 * show what sync depth kept (`docs/folder-pane.md`, rule 5). A folder of older mail then badges its
 * unread above nothing at all, and without this nothing on screen reconciles the two numbers.
 *
 * Which case it is comes from the core, so this composable picks no wording of its own; the action
 * appears only where widening the depth would actually find something.
 */
@Composable
internal fun MailboxEmpty(
    reason: EmptyReason?,
    onOpenSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (reason == null) return
    val ctx = LocalContext.current
    Column(
        modifier = modifier.fillMaxSize().padding(horizontal = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        when (reason) {
            is EmptyReason.NoMail -> Text(
                text = L10n.mailbox_empty_no_mail(ctx),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            is EmptyReason.OutsideSyncDepth -> {
                Text(
                    text = L10n.mailbox_empty_windowed(ctx, reason.months.toInt()),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
                Text(
                    text = L10n.mailbox_empty_windowed_body(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp),
                )
                TextButton(onClick = onOpenSettings, modifier = Modifier.padding(top = 8.dp)) {
                    Text(L10n.mailbox_empty_change(ctx))
                }
            }
        }
    }
}
