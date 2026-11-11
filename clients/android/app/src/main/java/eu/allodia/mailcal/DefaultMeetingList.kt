// Settings → Videoconferencing's default per account (docs/conferencing.md, rule 10): the video
// meeting a new event in one of the account's calendars takes once it has an invitee. The core
// lists the accounts, what each offers and what is chosen; this draws a menu per account.
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.MeetingDefault
import uniffi.mailcal_bindings.MeetingOption

/** The card below the Jitsi server, drawn only while an account's calendars offer a meeting. */
@Composable
internal fun DefaultMeetingCard(
    defaults: List<MeetingDefault>,
    onSetDefaultMeeting: (account: String, choice: String?) -> Unit,
) {
    if (defaults.isEmpty()) return
    val ctx = LocalContext.current
    Spacer(modifier = Modifier.height(8.dp))
    SettingsGroupCard(L10n.settings_default_meeting(ctx), L10n.settings_default_meeting_hint(ctx)) {
        Column {
            defaults.forEachIndexed { index, row ->
                if (index > 0) HorizontalDivider()
                DefaultMeetingRow(row) { onSetDefaultMeeting(row.account, it) }
            }
        }
    }
}

/** One account: its address, and what is chosen, opening a menu of None and then its choices. */
@Composable
private fun DefaultMeetingRow(row: MeetingDefault, onPick: (String?) -> Unit) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val none = L10n.event_meeting_none(ctx)
    fun label(choice: MeetingOption): String =
        choice.through?.let { L10n.event_meeting_through(ctx, choice.name, it) } ?: choice.name
    val chosen = row.choices.firstOrNull { it.id == row.chosen }
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { open = true }
                .padding(vertical = 8.dp)
                .testTag("settings-default-meeting-${row.account}"),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    row.accountAddress,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = chosen?.let { label(it) } ?: none,
                    style = MaterialTheme.typography.bodyLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Icon(painter = painterResource(R.drawable.ic_arrow_drop_down), contentDescription = null)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(none) },
                onClick = {
                    onPick(null)
                    open = false
                },
            )
            row.choices.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(label(choice)) },
                    onClick = {
                        onPick(choice.id)
                        open = false
                    },
                )
            }
        }
    }
}
