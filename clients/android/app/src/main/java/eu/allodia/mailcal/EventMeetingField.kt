package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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

// The editor's Video meeting row (docs/conferencing.md, rules 9 to 11): the label and the current
// pick, laid out like the calendar row above it, opening a menu of what the calendar offers, and
// beneath it, while it holds a choice the event does not have yet, that the link comes at the save
// (rule 10). The decisions are EventEditorMeeting.kt's; this only draws them.

/**
 * The words an option is shown by: the service's brand name, with the account that makes it for a
 * choice made through an account ([through] puts the two together), else the catalog's.
 */
internal fun meetingOptionLabel(
    option: MeetingRowOption,
    none: String,
    generic: String,
    through: (service: String, account: String) -> String,
): String {
    if (option.selection == MeetingSelection.None) return none
    val name = option.name ?: return generic
    return option.through?.let { through(name, it) } ?: name
}

@Composable
internal fun MeetingField(editor: EventEditorState) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val options = editor.meetingOptions
    val none = L10n.event_meeting_none(ctx)
    val generic = L10n.event_video_meeting(ctx)
    val through = { service: String, account: String -> L10n.event_meeting_through(ctx, service, account) }
    val current = options.firstOrNull { it.selection == editor.effectiveMeeting } ?: options.first()
    Column {
        Box {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { open = true }
                    .padding(vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(L10n.event_video_meeting(ctx), style = MaterialTheme.typography.labelSmall)
                    Text(
                        text = meetingOptionLabel(current, none, generic, through),
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Icon(painter = painterResource(R.drawable.ic_arrow_drop_down), contentDescription = null)
            }
            DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                options.forEach { option ->
                    DropdownMenuItem(
                        text = { Text(meetingOptionLabel(option, none, generic, through)) },
                        onClick = {
                            editor.meeting = option.selection
                            open = false
                        },
                    )
                }
            }
        }
        if (editor.announcesMeeting) {
            Text(
                text = L10n.event_meeting_added_on_save(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.testTag("event-meeting-added-on-save"),
            )
        }
    }
}
