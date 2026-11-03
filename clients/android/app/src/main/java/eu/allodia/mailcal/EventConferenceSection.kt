// An event's video meetings in its detail (docs/conferencing.md): Join first, then what a person
// needs to get in another way, the meeting ID, the passcode and the numbers to dial. The Android
// twin of Apple's EventConferenceView and Windows' EventConferenceSection.cs; the decisions are
// EventMeeting.kt.
//
// Every string here came from whoever wrote the event, so each is drawn by `Text`, never parsed,
// and the links leave through the OS on a tap, never into a view of the app's own. Nothing here
// logs: a Zoom link carries its passcode.
package eu.allodia.mailcal

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.PersistableBundle
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import uniffi.mailcal_bindings.ConferenceDialIn
import uniffi.mailcal_bindings.EventConference

/** Where a meeting's links go. The detail hands the OS's; a test records them. */
internal class MeetingActions(
    val join: (String) -> Unit,
    val dial: (String) -> Unit,
    val copy: (String) -> Unit,
) {
    companion object {
        fun of(context: Context) = MeetingActions(
            join = { openThroughOs(context, Intent(Intent.ACTION_VIEW, it.toUri())) },
            // ACTION_DIAL fills in the dialler and waits for the person to press call, so the app
            // needs no CALL_PHONE permission and nothing is dialled behind anyone's back.
            dial = { openThroughOs(context, Intent(Intent.ACTION_DIAL, it.toUri())) },
            copy = { copyLink(context, it) },
        )
    }
}

/**
 * One meeting: its heading with Join, then each row it has a value for. [index] is its place in
 * the core's order, which keeps each meeting's test tags its own: the first carries the bare tags,
 * a later one the same tags suffixed with its index.
 */
@Composable
internal fun EventConferenceSection(
    conference: EventConference,
    index: Int,
    actions: MeetingActions,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    fun tag(name: String) = if (index == 0) name else "$name-$index"
    // Already through the link policy, and only for a meeting that is ready: the core's answer.
    val join = conference.joinUrl

    Column(modifier = modifier.fillMaxWidth().padding(top = 16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(
                painterResource(R.drawable.ic_videocam),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.width(12.dp))
            Text(
                EventMeeting.title(conference, L10n.event_video_meeting(ctx)),
                modifier = Modifier.weight(1f).testTag(tag("event-meeting-service")),
                style = MaterialTheme.typography.titleMedium,
            )
            if (join != null) {
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = { actions.join(join) },
                    modifier = Modifier.testTag(tag("event-join")),
                ) { Text(L10n.event_join(ctx)) }
            }
        }
        EventMeeting.status(
            conference,
            L10n.event_meeting_preparing(ctx),
            L10n.event_meeting_failed(ctx),
        )?.let { Secondary(it, Modifier.padding(start = 36.dp, top = 4.dp)) }

        Column(modifier = Modifier.padding(start = 36.dp)) {
            // The link Join opens, so a meeting with nothing to join offers nothing to copy either.
            if (join != null) {
                TextButton(
                    onClick = { actions.copy(join) },
                    modifier = Modifier.alignedText().testTag(tag("event-meeting-copy-link")),
                ) {
                    Icon(
                        painterResource(R.drawable.ic_content_copy),
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text(L10n.action_copy_link(ctx))
                }
            }
            conference.meetingId?.takeIf(EventMeeting::shows)?.let {
                Labeled(L10n.event_meeting_id(ctx), it, Modifier.testTag(tag("event-meeting-id")))
            }
            conference.passcode?.takeIf(EventMeeting::shows)?.let {
                Labeled(
                    L10n.event_meeting_passcode(ctx),
                    it,
                    Modifier.testTag(tag("event-meeting-passcode")),
                )
            }
            if (conference.dialIn.isNotEmpty()) {
                Caption(L10n.event_meeting_dial_in(ctx), Modifier.padding(top = 8.dp))
                // In the core's order, one row each: a server can list one number twice with a
                // different PIN.
                conference.dialIn.forEach { DialInRow(it, actions) }
            }
            conference.notes?.takeIf(EventMeeting::shows)?.let {
                SelectionContainer { Secondary(it, Modifier.padding(top = 8.dp)) }
            }
        }
    }
}

// A number opens the dialler only when the core gave it a URI to dial; a number the link policy
// refused is text.
@Composable
private fun DialInRow(number: ConferenceDialIn, actions: MeetingActions) {
    val ctx = LocalContext.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        val dial = number.uri
        if (dial != null) {
            TextButton(onClick = { actions.dial(dial) }, modifier = Modifier.alignedText()) {
                Icon(
                    painterResource(R.drawable.ic_call),
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(number.number)
            }
        } else {
            SelectionContainer {
                Text(number.number, style = MaterialTheme.typography.bodyLarge)
            }
        }
        number.pin?.takeIf(EventMeeting::shows)?.let {
            SelectionContainer { Secondary(L10n.event_meeting_pin(ctx, it)) }
        }
    }
}

// A text button draws its content inside its own padding, so its icon starts that far in from the
// captions above and below it. Shifted back by that padding, the icon lines up with them and the
// ripple keeps its room.
// `offset` mirrors itself in a right-to-left layout, so the start padding is read in LTR terms.
private fun Modifier.alignedText(): Modifier =
    offset(x = -ButtonDefaults.TextButtonContentPadding.calculateStartPadding(LayoutDirection.Ltr))

@Composable
private fun Labeled(label: String, value: String, modifier: Modifier = Modifier) {
    Column(modifier = Modifier.padding(top = 8.dp)) {
        Caption(label)
        SelectionContainer {
            Text(value, modifier = modifier, style = MaterialTheme.typography.bodyLarge)
        }
    }
}

@Composable
private fun Caption(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun Secondary(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

private fun openThroughOs(context: Context, intent: Intent) {
    try {
        context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        // Nothing on the device handles it (no browser, no dialler on a tablet): the tap does
        // nothing rather than taking the app down.
    }
}

// Marked sensitive, because a Zoom link carries its passcode: Android 13 and later then keep it
// out of the clipboard preview and confirm the copy themselves. Before that the confirmation is
// ours.
private fun copyLink(context: Context, link: String) {
    val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return
    val clip = ClipData.newPlainText(null, link)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        clip.description.extras = PersistableBundle().apply {
            putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
        }
        clipboard.setPrimaryClip(clip)
    } else {
        clipboard.setPrimaryClip(clip)
        Toast.makeText(context, L10n.event_meeting_link_copied(context), Toast.LENGTH_SHORT).show()
    }
}
