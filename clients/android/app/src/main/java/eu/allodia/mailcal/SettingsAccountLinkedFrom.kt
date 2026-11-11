// The accounts taking a use from this one through a link (docs/accounts.md rule 12), on its page:
// each named with what it takes, and Stop linking, which the core runs once the person confirms it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinkedFrom
import uniffi.mailcal_bindings.AccountPageIntent

/** "Linked to": one row per account taking a use from [entry]. */
@Composable
internal fun LinkedFromGroup(entry: AccountEntry, linked: List<AccountLinkedFrom>, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    SettingsGroupCard(L10n.settings_account_linked_from_heading(ctx), null) {
        linked.forEach { taking -> LinkedFromRow(entry, taking, ui) }
    }
}

/** An account taking a use from [entry]: its name, what it takes, and Stop linking, asked first. */
@Composable
private fun LinkedFromRow(entry: AccountEntry, taking: AccountLinkedFrom, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val name = accountName(ctx, taking.account)
    var confirming by remember(entry.id, taking.account.id) { mutableStateOf(false) }
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // The name and what it takes are read as one.
        Column(modifier = Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(name, style = MaterialTheme.typography.bodyLarge)
            Text(
                linkedFromLine(ctx, taking),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        ControlTextButton(
            L10n.settings_account_stop_linking(ctx),
            taking.stop,
            "account-stop-linking-${taking.account.id}",
        ) { confirming = true }
    }
    if (confirming) {
        ConfirmDialog(
            title = L10n.settings_account_stop_linking_title(ctx, name),
            message = stopLinkingMessage(ctx, taking),
            action = L10n.settings_account_stop_linking(ctx),
            tag = "account-stop-linking",
        ) { yes ->
            confirming = false
            if (yes) ui.onChange(AccountPageIntent.StopLinking(entry.id, taking.account.id))
        }
    }
}
