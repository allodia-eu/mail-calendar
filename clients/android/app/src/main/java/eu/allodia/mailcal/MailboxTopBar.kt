// The mailbox's top row: the drawer button, the account switcher, search and settings. Split out
// of MailboxScreen.kt to keep each file under the 500-line limit, and because the row now has a
// sibling that replaces it: while rows are selected, MailSelectionBar takes this place rather than
// stacking under it, which is the Material selection-mode pattern and keeps the list where it was.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow

@Composable
internal fun MailboxTopBar(
    search: SearchBarState,
    accounts: List<AccountRow>,
    selectedAccount: String?,
    unreachableAccounts: List<String>,
    onSelectAccount: (String?) -> Unit,
    onAddAccount: () -> Unit,
    onRemoveAccount: (String) -> Unit,
    onOpenDrawer: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (search.open) {
            SearchField(state = search, modifier = Modifier.weight(1f))
        } else {
            // Hamburger always opens the folder navigation drawer.
            IconButton(onClick = onOpenDrawer) {
                Icon(
                    painter = painterResource(R.drawable.ic_menu),
                    contentDescription = L10n.a11y_open_folders(ctx),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            // The account switcher keeps a guaranteed share of the width so it never
            // collapses behind the search + settings icons on a narrow phone.
            AccountSwitcher(
                accounts = accounts,
                selectedAccount = selectedAccount,
                unreachableAccounts = unreachableAccounts,
                onSelectAccount = onSelectAccount,
                onAddAccount = onAddAccount,
                onRemoveAccount = onRemoveAccount,
                modifier = Modifier.weight(1f),
            )
            // Search collapses to a magnifier to save space, expanding to the field on tap.
            IconButton(onClick = search::openSearch) {
                Icon(
                    painter = painterResource(R.drawable.ic_search),
                    contentDescription = L10n.search_placeholder(ctx),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            // Conversation grouping, language, time zone, per-account fetch depth + sync
            // behaviour, the default quote style, and the database reset live in Settings.
            IconButton(onClick = onOpenSettings) {
                Icon(
                    painter = painterResource(R.drawable.ic_settings),
                    contentDescription = L10n.settings_title(ctx),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
