// The Composing-section settings cards that go beyond the quote style: the default send account
// (which account new mail in the combined inbox goes out from) and the two swipe-action pickers.
// State lives in the Rust core (persisted preferences); these render it and dispatch the setters,
// which re-signal SETTINGS. Kept out of SettingsViews.kt so each file stays small (gradle
// auto-globs the package).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings

// Which account new mail composes from when the combined inbox is showing. Only meaningful with
// more than one account, so with a single account the card explains itself instead of offering a
// choice of one. `selected` is the STORED id, which may name an account that has since been removed
// the core then falls back to the first, which is what we show.
@Composable
internal fun DefaultSendAccountCard(
    accounts: List<AccountRow>,
    selected: String?,
    onSelect: (String?) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    val effective = accounts.firstOrNull { it.id == selected } ?: accounts.firstOrNull()
    if (accounts.size <= 1) {
        Text(
            text = effective?.email ?: L10n.settings_accounts_empty(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        return
    }
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(effective?.email.orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = L10n.settings_send_account_heading(ctx),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            accounts.forEach { account ->
                DropdownMenuItem(
                    text = { Text(account.email) },
                    onClick = {
                        expanded = false
                        onSelect(account.id)
                    },
                )
            }
        }
    }
}

// The two swipe directions, each an independent Trash / Archive / Star picker.
@Composable
internal fun SwipeActionsCard(
    swipe: SwipeSettings,
    onSetLeft: (SwipeActionKind) -> Unit,
    onSetRight: (SwipeActionKind) -> Unit,
) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        SwipeDirectionPicker(L10n.settings_swipe_left(ctx), swipe.left, onSetLeft)
        SwipeDirectionPicker(L10n.settings_swipe_right(ctx), swipe.right, onSetRight)
    }
}

@Composable
private fun SwipeDirectionPicker(
    label: String,
    selected: SwipeActionKind,
    onSelect: (SwipeActionKind) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    Text(label, style = MaterialTheme.typography.labelLarge)
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(swipeActionLabel(ctx, selected), maxLines = 1)
            Icon(painter = painterResource(R.drawable.ic_arrow_drop_down), contentDescription = label)
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            SwipeActionKind.entries.forEach { action ->
                DropdownMenuItem(
                    text = { Text(swipeActionLabel(ctx, action)) },
                    onClick = {
                        expanded = false
                        onSelect(action)
                    },
                )
            }
        }
    }
}
