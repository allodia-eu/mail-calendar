// Settings → Videoconferencing's list of the accounts that make video meetings (docs/settings.md
// category 4): each service through its account, in its state, opening that account's page, where
// its one switch is. No switch of its own here: two switches for one setting would be two places to
// change it, and two places to ask a provider for its permission.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.MeetingAccount

/** An account's Video meetings use, as its row says it. */
internal fun meetingAccountState(ctx: Context, state: CapabilityState): String = when (state) {
    CapabilityState.ON -> L10n.settings_toggle_on(ctx)
    CapabilityState.OFF -> L10n.settings_toggle_off(ctx)
    CapabilityState.NEEDS_PERMISSION -> L10n.settings_meeting_account_needs_permission(ctx)
    CapabilityState.NOT_OFFERED -> L10n.settings_meeting_account_not_offered(ctx)
}

/** The card above the Jitsi server, drawn only while an account makes meetings. */
@Composable
internal fun MeetingAccountsCard(accounts: List<MeetingAccount>, onOpenAccount: (String) -> Unit) {
    if (accounts.isEmpty()) return
    val ctx = LocalContext.current
    SettingsGroupCard(L10n.settings_meeting_accounts(ctx), L10n.settings_meeting_accounts_hint(ctx)) {
        MeetingAccountsList(accounts, onOpenAccount)
    }
    Spacer(modifier = Modifier.height(8.dp))
}

/** The rows, one per account. */
@Composable
private fun MeetingAccountsList(accounts: List<MeetingAccount>, onOpenAccount: (String) -> Unit) {
    val ctx = LocalContext.current
    Column {
        accounts.forEachIndexed { index, row ->
            if (index > 0) HorizontalDivider()
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClickLabel = L10n.use_outage_open_account(ctx)) { onOpenAccount(row.account.id) }
                    .padding(vertical = 8.dp)
                    .testTag("meeting-account-${row.account.id}"),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(row.service, style = MaterialTheme.typography.bodyLarge)
                    Text(
                        accountName(ctx, row.account),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    meetingAccountState(ctx, row.state),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Icon(
                    painter = painterResource(R.drawable.ic_keyboard_arrow_right),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
