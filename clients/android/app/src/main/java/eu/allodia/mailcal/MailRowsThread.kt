// The threaded conversation row for the mailbox list: a header carrying the subject, the message
// count and the latest sender. The flat message row and the shared row furniture are in
// MailRows.kt; the swipe-to-act gesture that wraps a flat row is in MailRowsSwipe.kt.

package eu.allodia.mailcal

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Badge
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ThreadRow

// A conversation row: a header (subject, message count, latest sender). Tapping it opens the
// conversation's latest message (received or sent) in the reading screen, where the older
// messages sit as a collapsed strip that opens each on tap (Gmail/Outlook-mobile style), rather
// than expanding inline in the list. Long-press the header to archive the conversation. Only real
// multi-message conversations reach here: the core projects a lone message as a flat row.
@OptIn(ExperimentalFoundationApi::class)
@androidx.compose.runtime.Composable
internal fun ThreadConversationRow(
    thread: ThreadRow,
    activeZoneId: String?,
    // The same selection mode the flat row takes. A conversation row stands for its whole thread,
    // which the core expands itself (docs/list-selection.md, rule 2).
    selected: Boolean,
    selecting: Boolean,
    onToggleSelect: () -> Unit,
    onOpenThread: () -> Unit,
) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(
                    if (selected) {
                        MaterialTheme.colorScheme.secondaryContainer
                    } else {
                        MaterialTheme.colorScheme.surface
                    },
                )
                // Long press selects the conversation, as it does on a flat row. It used to open a
                // one-item "Archive conversation" menu; the selection bar offers that action over
                // the same rows and five more beside it, so the menu had nothing left of its own.
                .combinedClickable(
                    onClick = { if (selecting) onToggleSelect() else onOpenThread() },
                    onLongClick = onToggleSelect,
                )
                .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.Top,
        ) {
            // The latest sender is who the row names. The count badge already marks a conversation;
            // bold subject and sender carry unread state on this compact, single-pane list.
            AvatarView(thread.avatar, modifier = Modifier.testTag("thread-avatar"))
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = thread.subject.ifEmpty { L10n.mail_no_subject(ctx) },
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = if (thread.unreadCount > 0u) FontWeight.Bold else FontWeight.Normal,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    // The message count marks this as a conversation at a glance (a real thread is
                    // always > 1 here, the core projects a lone message as a flat row).
                    if (thread.messageCount > 1u) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Badge { Text("${thread.messageCount}") }
                    }
                    // A paperclip when any message in the conversation has an attachment (matching
                    // macOS/Windows thread rows).
                    if (thread.hasAttachment) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Icon(
                            painter = painterResource(R.drawable.ic_attachment),
                            contentDescription = L10n.a11y_has_attachment(ctx),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(15.dp),
                        )
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = relativeDate(thread.latestDate, activeZoneId, LocalUse24Hour.current),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
                // Latest sender + a preview snippet of the representative message.
                Text(
                    text = senderAndPreview(
                        thread.latestFrom,
                        thread.preview,
                        unread = thread.unreadCount > 0u,
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
