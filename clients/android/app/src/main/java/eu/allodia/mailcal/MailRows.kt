// The mailbox-list row composables for the Android client: the flat message row, with its flag and
// its menu, and the row furniture both kinds share. The threaded
// conversation row is in MailRowsThread.kt; the swipe-to-act gesture that wraps a flat row is
// in MailRowsSwipe.kt.
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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.SelectedRow

// Amber for the flagged star, mirroring the macOS client's orange `flag.fill`. Only the filled
// star is vendored, so flagged state is shown by tint/alpha on the one glyph rather than by
// swapping to an outline variant (adding `star` at fill 0 would be one more ic_*.xml).
internal val FlaggedAmber = Color(0xFFFFB300)


// Widened from private: called from MailRowsSwipe.kt's SwipeableFlatMessageRow.
@OptIn(ExperimentalFoundationApi::class)
@androidx.compose.runtime.Composable
internal fun FlatMessageRow(
    message: FlatRow,
    activeZoneId: String?,
    // The list's selection mode: whether this row is picked, whether the mode is on at all (a tap
    // then toggles rather than opens), and how to toggle it (docs/list-selection.md).
    selected: Boolean,
    selecting: Boolean,
    onToggleSelect: () -> Unit,
    // Opens the message, or resumes it in a composer when the row says it is a draft, wherever it
    // is listed (docs/drafts.md).
    onOpen: (opened: OpenedMessage, draft: Boolean) -> Unit,
    // The row's ⋮ menu, which the core answers (docs/row-actions.md).
    menus: RowMenus = RowMenus.NONE,
) {
    val ctx = LocalContext.current
    // Read the ambient clock setting here, in the composable body: the `clickable` lambda below is
    // not a composition scope and cannot read a CompositionLocal.
    val use24Hour = LocalUse24Hour.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            // The selected row carries the theme's own selection colour rather than a tint of our
            // own, so it reads as selected to a person and to the accessibility layer alike.
            .background(
                if (selected) {
                    MaterialTheme.colorScheme.secondaryContainer
                } else {
                    MaterialTheme.colorScheme.surface
                },
            )
            .combinedClickable(
                // A long press enters selection mode; once in it, a tap adds or removes a row
                // instead of opening it. A phone has no modifier keys, so this is the mode the
                // contract gives it (docs/list-selection.md).
                onLongClick = onToggleSelect,
                onClick = {
                    if (selecting) {
                        onToggleSelect()
                    } else {
                        onOpen(
                            OpenedMessage(
                                account = message.account,
                                key = message.key,
                                subject = message.subject,
                                from = message.from,
                                avatar = message.avatar,
                                // The reading header keeps the full date; the row shows the short one.
                                date = localDateTime(message.date, activeZoneId, use24Hour),
                            ),
                            message.draft,
                        )
                    }
                },
            )
            .padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.Top,
    ) {
        // This client is single-pane: the list is always the compact layout, where bold subject
        // and sender carry unread state and the contract reserves no separate dot gutter.
        AvatarView(message.avatar, modifier = Modifier.testTag("mail-avatar"))
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = message.subject.ifEmpty { L10n.mail_no_subject(ctx) },
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (message.unread) FontWeight.Bold else FontWeight.Normal,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                // A small amber star marks flagged messages at a glance (cf. macOS `flag.fill`).
                if (message.flagged) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Icon(
                        painter = painterResource(R.drawable.ic_star),
                        contentDescription = L10n.a11y_flagged(ctx),
                        tint = FlaggedAmber,
                        modifier = Modifier.size(15.dp),
                    )
                }
                // A paperclip marks messages with a non-inline attachment, after the flag:
                // matching macOS (SF Symbol "paperclip") and Windows (Segoe glyph E723).
                if (message.hasAttachment) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Icon(
                        painter = painterResource(R.drawable.ic_attachment),
                        contentDescription = L10n.a11y_has_attachment(ctx),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(15.dp),
                    )
                }
                // A compact relative timestamp trailing the subject (today → time, this week →
                // weekday, else date), far narrower than the full date, so the title gets the room.
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = relativeDate(message.date, activeZoneId, LocalUse24Hour.current),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
            // Sender + a body preview snippet, so a truncated subject still has context.
            Text(
                text = senderAndPreview(message.from, message.preview, unread = message.unread),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        // The menu holds the rest, so the row's width goes to the title and preview; the flagged
        // state still shows as the inline amber star above.
        RowMenuButton(
            row = SelectedRow.Message(message.account, message.key),
            account = message.account,
            menus = menus,
        )
    }
}

/**
 * How heavily a list row's **sender** is drawn.
 *
 * Its own function, and not an inline `if`, because it is the one part of the unread treatment that
 * is a decision rather than a layout: the subject and the sender have to move together, on this
 * platform and on the others, and a rule three clients implement separately is a rule that drifts.
 * Bold and Medium rather than Bold and Normal, the sender line is already secondary text, so
 * dropping it to Normal when read would make the read rows harder to scan than they are today.
 */
internal fun unreadSenderWeight(unread: Boolean): FontWeight =
    if (unread) FontWeight.Bold else FontWeight.Medium

// The second list-row line: the sender, slightly emphasised, followed by a lighter body preview
// snippet, so a truncated subject still carries context (cf. Thunderbird). The snippet's runs of
// whitespace are collapsed so it stays a clean single stretch of text.
// Widened from private: called from MailRowsThread.kt's ThreadConversationRow.
@androidx.compose.runtime.Composable
internal fun senderAndPreview(
    sender: String,
    preview: String,
    unread: Boolean = false,
): androidx.compose.ui.text.AnnotatedString {
    val senderColor = MaterialTheme.colorScheme.onSurfaceVariant
    val previewColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    val snippet = preview.replace(Regex("\\s+"), " ").trim()
    return androidx.compose.ui.text.buildAnnotatedString {
        if (sender.isNotEmpty()) {
            // The weight is on the SENDER SPAN, not the whole line: sender and preview share one
            // Text here (they wrap as one paragraph), so bolding the string would bold the snippet
            // too, and a mailbox of unread rows in solid bold distinguishes nothing.
            withStyle(
                androidx.compose.ui.text.SpanStyle(
                    color = senderColor,
                    fontWeight = unreadSenderWeight(unread),
                )
            ) {
                append(sender)
            }
        }
        if (snippet.isNotEmpty()) {
            withStyle(androidx.compose.ui.text.SpanStyle(color = previewColor)) {
                if (sender.isNotEmpty()) append(" ,  ")
                append(snippet)
            }
        }
    }
}
