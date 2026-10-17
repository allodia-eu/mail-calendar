// The "already set up" refusal's offer (docs/accounts.md rules 15 and 17): the account an add was
// refused for takes the credential the person just gave, keeping its mail and settings. The
// refusal itself stays on the form beneath, as every add failure does.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

/** The offer, with its button held while the account takes the credential ([updating]). */
@Composable
internal fun AlreadySetUpDialog(updating: Boolean, onUpdate: () -> Unit, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(L10n.account_already_set_up(ctx)) },
        text = { Text(L10n.account_already_set_up_update_note(ctx)) },
        confirmButton = {
            TextButton(
                onClick = onUpdate,
                enabled = !updating,
                modifier = Modifier.testTag("setup-update-existing"),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (updating) {
                        CircularProgressIndicator(modifier = Modifier.size(18.dp))
                        Spacer(modifier = Modifier.width(8.dp))
                    }
                    Text(L10n.account_already_set_up_update(ctx))
                }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !updating) { Text(L10n.action_cancel(ctx)) }
        },
    )
}
