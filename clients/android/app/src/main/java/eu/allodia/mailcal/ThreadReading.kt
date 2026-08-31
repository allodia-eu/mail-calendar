// The conversation strip shown in the reading screen (Gmail/Outlook-mobile style): when the
// opened message belongs to a multi-message thread, the reading screen shows that message in
// full and lists the OTHER messages on the thread as a compact, tappable strip above it. Tapping
// a card opens that message, the reading body the core fetches is single-slot, so one message is
// shown in full at a time and the previously-open one moves back into this strip. Split out of
// ReadingScreen.kt so each file stays under the 500-line limit.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.first
import uniffi.mailcal_bindings.ThreadMessage

// The strip of the conversation's OTHER messages, above the message shown in full. Ordered oldest
// first so it reads top-to-bottom like the thread, and capped in height (scrolling within) so a
// long conversation never crowds out the open message's body. The currently-open message
// (`focusedKey`) is excluded, it's shown in full below the strip.
@Composable
internal fun ConversationStrip(
    conversation: List<ThreadMessage>,
    focusedKey: String,
    subject: String,
    activeZoneId: String?,
    onOpen: (OpenedMessage) -> Unit,
) {
    // `conversation` is newest-first; reverse to oldest-first for the strip.
    val others = conversation.filter { it.key != focusedKey }.reversed()
    if (others.isEmpty()) {
        return
    }
    val scrollState = rememberScrollState()
    // Rest at the BOTTOM by default, so the newest of the older messages sits directly above the
    // open message (which renders just below this strip) and older ones are up-screen, scroll up
    // to reach them. Wait for the content to be measured (maxValue > 0, i.e. it actually overflows)
    // before jumping; re-runs when the visible set changes (opening a different message on the
    // thread). If it all fits, maxValue stays 0 and there's nothing to scroll.
    LaunchedEffect(others) {
        snapshotFlow { scrollState.maxValue }.first { it > 0 }
        scrollState.scrollTo(scrollState.maxValue)
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(max = 220.dp)
            .verticalScroll(scrollState),
    ) {
        others.forEach { message ->
            ConversationStripCard(message, subject, activeZoneId, onOpen)
            HorizontalDivider()
        }
    }
}

// One collapsed message in the strip: avatar, sender, "Sent" badge, preview, date. Tapping it
// opens that message in the reading screen (the same `OpenedMessage` a list row would produce).
@Composable
private fun ConversationStripCard(
    message: ThreadMessage,
    subject: String,
    activeZoneId: String?,
    onOpen: (OpenedMessage) -> Unit,
) {
    val ctx = LocalContext.current
    // Read the ambient clock setting here: the `clickable` lambda is not a composition scope.
    val use24Hour = LocalUse24Hour.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                onOpen(
                    OpenedMessage(
                        account = message.account,
                        key = message.key,
                        subject = subject,
                        from = message.from,
                        avatar = message.avatar,
                        date = localDateTime(message.date, activeZoneId, use24Hour),
                    ),
                )
            }
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AvatarView(
            avatar = message.avatar,
            diameter = 32.dp,
            modifier = Modifier.testTag("thread-message-avatar"),
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = message.from.ifEmpty { L10n.mail_no_subject(ctx) },
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = if (message.unread) FontWeight.SemiBold else FontWeight.Normal,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (message.outgoing) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Text(
                        text = L10n.thread_sent(ctx),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier
                            .clip(RoundedCornerShape(4.dp))
                            .background(MaterialTheme.colorScheme.surfaceVariant)
                            .padding(horizontal = 6.dp, vertical = 1.dp),
                    )
                }
                if (message.hasAttachment) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Icon(
                        painter = painterResource(R.drawable.ic_attachment),
                        contentDescription = L10n.a11y_has_attachment(ctx),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(14.dp),
                    )
                }
            }
            if (message.preview.isNotEmpty()) {
                Text(
                    text = message.preview,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = localDateTime(message.date, activeZoneId, LocalUse24Hour.current),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
