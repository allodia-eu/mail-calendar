// The composer's From dropdown: pick which configured account a message is sent from. The core
// sends as, and through, that account (its identity AND its outbox), so this is an account
// picker, not a free-text From header.
//
// The caller decides which account it opens on: the one that received the mail being replied
// to/forwarded, else the selected mailbox's account, else the app-level default send account
// (Settings → Composing). Kept in its own file so RichComposeScreen.kt stays small.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import uniffi.mailcal_bindings.AccountRow

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun FromAccountField(
    accounts: List<AccountRow>,
    selected: AccountRow?,
    onSelect: (AccountRow) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    // With one account there is nothing to choose, so the field renders as a plain read-only row
    // rather than a menu that opens onto a single item. It stays visible either way, the From
    // address is never hidden.
    val pickable = accounts.size > 1
    ExposedDropdownMenuBox(
        expanded = expanded && pickable,
        onExpandedChange = { if (pickable) expanded = it },
        modifier = Modifier.fillMaxWidth(),
    ) {
        OutlinedTextField(
            value = selected?.email ?: "",
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text(L10n.compose_from(ctx)) },
            trailingIcon = {
                if (pickable) {
                    ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded)
                }
            },
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(
                    androidx.compose.material3.ExposedDropdownMenuAnchorType.PrimaryNotEditable,
                    pickable,
                ),
        )
        ExposedDropdownMenu(expanded = expanded && pickable, onDismissRequest = { expanded = false }) {
            accounts.forEach { account ->
                DropdownMenuItem(
                    text = { Text(account.email) },
                    onClick = {
                        expanded = false
                        onSelect(account)
                    },
                )
            }
        }
    }
}
