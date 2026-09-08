// The "your name" field on an account's settings card: what recipients see beside the address on
// the mail this account sends (docs/sending.md).
//
// Its own file so SettingsViews stays under the 500-line limit, and because this is the one
// control on the card whose effect a stranger can see; everything else there is about how much of
// the account this device keeps.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountSyncRow

/**
 * The name field for one account, or the note that the name is not this person's to change.
 *
 * The edit is committed on **done**, not on every keystroke: a per-keystroke write would push a
 * half-typed name to the provider and rebuild the snapshot under the cursor.
 */
@Composable
internal fun SenderNameField(account: AccountSyncRow, onSetSenderName: (String, String) -> Unit) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(L10n.settings_sender_name_heading(ctx), style = MaterialTheme.typography.titleSmall)
        Text(
            L10n.settings_sender_name_description(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(4.dp))
        if (account.senderNameEditable) {
            // Keyed on the account id so switching cards reseeds rather than carrying the last
            // account's text across.
            var draft by remember(account.accountId) { mutableStateOf(account.senderName) }
            OutlinedTextField(
                value = draft,
                onValueChange = { draft = it },
                singleLine = true,
                label = { Text(L10n.settings_sender_name_heading(ctx)) },
                keyboardOptions =
                    androidx.compose.foundation.text.KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions =
                    androidx.compose.foundation.text.KeyboardActions(
                        onDone = {
                            if (draft != account.senderName) {
                                onSetSenderName(account.accountId, draft)
                            }
                        }
                    ),
                modifier = Modifier.fillMaxWidth(),
            )
        } else {
            // The name is the organisation's. Show it, and say why there is no field, rather
            // than a disabled box that reads as a bug.
            Text(account.senderName.ifEmpty { account.email })
            Text(
                L10n.settings_sender_name_managed(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
